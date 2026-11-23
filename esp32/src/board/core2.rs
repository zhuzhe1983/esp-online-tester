//! Original M5Stack Core2 K010: Core2 v1.0 schematic and M5Core2 67099371.
//! ILI9342C SPI G18/G23/G38/G5/G15; FT6336U on G21/G22 with INT on G39.
//! AXP192 LDO2 powers LCD logic; IO4 resets LCD/touch; DC3 powers the backlight.
use std::{cell::Cell,rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,axp192::{AXP192_ADDRESS,Axp192,Axp192State},ft6336::{Ft6336g,Ft6336State},mpu6886::{Mpu6886,Mpu6886Sample}};
use esp_soc::board::{BoardEdge,BoardModel,SpiPins,VirtualCycle};
use esp_soc::devices::{DcsPanel,bm8563::{Bm8563,Bm8563State},i2c_bitbang::I2cBitBang};
use crate::periph::CPU_HZ;

const LCD_W:usize=320;
const LCD_H:usize=240;
const TOUCH_IRQ:u8=39;
const TOUCH_I2C:u8=0x38;
const RTC_I2C:u8=0x51;
const IMU_I2C:u8=0x68;

struct Core2Touch {
    device:Ft6336g,
    state:Arc<Mutex<Ft6336State>>,
    pmic:Arc<Mutex<Axp192State>>,
    reset_count:u64,
}

impl I2cDevice for Core2Touch {
    fn start(&mut self,read:bool)->bool {
        let pmic=self.pmic.lock().unwrap();
        let mut state=self.state.lock().unwrap();
        state.powered=pmic.panel_ready();
        if !state.powered || self.reset_count!=pmic.panel_reset_count() {
            self.reset_count=pmic.panel_reset_count();state.reset();
            self.device=Ft6336g::new(self.state.clone());
        }
        drop(state);drop(pmic);self.device.start(read)
    }
    fn write(&mut self,byte:u8)->bool {self.device.write(byte)}
    fn read(&mut self)->u8 {self.device.read()}
}

struct Core2Rtc {device:Bm8563,state:Arc<Mutex<Bm8563State>>,cycle:Rc<Cell<VirtualCycle>>}
impl I2cDevice for Core2Rtc {
    fn start(&mut self,read:bool)->bool {
        self.state.lock().unwrap().advance_to(self.cycle.get());
        self.device.start(read)
    }
    fn write(&mut self,byte:u8)->bool {self.device.write(byte)}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}

pub struct Core2 {
    panel:DcsPanel,
    command:u8,
    read_at:usize,
    read_index:u8,
    reset_count:u64,
    display_changes:u64,
    pmic:Arc<Mutex<Axp192State>>,
    touch:Arc<Mutex<Ft6336State>>,
    imu:Arc<Mutex<Mpu6886Sample>>,
    rtc:Arc<Mutex<Bm8563State>>,
    cycle:Rc<Cell<VirtualCycle>>,
    touch_irq_high:bool,
    panel_touch:Option<(u16,u16)>,
    touch_keys:[bool;3],
    power_held:bool,
}

impl Core2 {
    pub fn new()->Self {
        Self{
            panel:DcsPanel::new(LCD_W,LCD_H),command:0,read_at:0,read_index:0,reset_count:0,display_changes:0,
            pmic:Arc::new(Mutex::new(Axp192State::new(CPU_HZ))),touch:Default::default(),imu:Default::default(),
            rtc:Arc::new(Mutex::new(Bm8563State::new(CPU_HZ))),cycle:Default::default(),touch_irq_high:true,
            panel_touch:None,touch_keys:[false;3],power_held:false,
        }
    }
    fn contact(&self)->Option<(u16,u16)> {
        // M5Core2.h BtnA/B/C rectangles extend past the LCD down to raw y=279.
        self.panel_touch.or_else(||self.touch_keys.iter().position(|&down|down).map(|index|([65,165,270][index],260)))
    }
    fn update_touch(&mut self) {
        let contact=self.contact().filter(|_|self.pmic.lock().unwrap().panel_ready());
        let mut touch=self.touch.lock().unwrap();
        let (x,y)=contact.unwrap_or((touch.x,touch.y));
        touch.update(x,y,contact.is_some());
    }
    fn touch_line_high(&self)->bool {
        self.contact().is_none() || !self.pmic.lock().unwrap().panel_ready()
    }
}
impl Default for Core2 {fn default()->Self {Self::new()}}

impl BoardModel for Core2 {
    fn name(&self)->&'static str {"core2"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {
        for &(pin,level) in changes {if pin==15 {self.panel.dc=level;}}
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if pins.sclk&(1<<18)==0 || pins.cs&(1<<5)==0 || (!tx.is_empty() && pins.mosi&(1<<23)==0) {
            return vec![0xff;rx_len];
        }
        let pmic=self.pmic.lock().unwrap();
        if pmic.panel_reset_count()!=self.reset_count {
            self.reset_count=pmic.panel_reset_count();self.panel.reset();
            self.command=0;self.read_at=0;self.read_index=0;
        }
        if !pmic.panel_ready() {return vec![0xff;rx_len];}
        drop(pmic);
        for &byte in tx {
            if !self.panel.dc {
                self.command=byte;self.read_at=0;
                if matches!(byte,0x01|0x10|0x11|0x28|0x29) {self.display_changes=self.display_changes.wrapping_add(1);}
            } else if self.command==0xd9 {self.read_index=byte&15;}
            self.panel.byte(byte);
        }
        if pins.miso!=Some(38) {return vec![0xff;rx_len];}
        // ILITEK ILI9342C V102 §§8.2.3, 8.2.46–48: ID E3/00/00, ID4 00/93/42.
        (0..rx_len).map(|_|{
            let index=self.read_at.saturating_add(usize::from(self.read_index.saturating_sub(1)));
            let value=match self.command {0x04=>[0xe3,0,0].get(self.read_at).copied().unwrap_or(0),0xda=>0xe3,0xd3=>[0,0x93,0x42].get(index).copied().unwrap_or(0),_=>0};
            self.read_at=self.read_at.saturating_add(1);value
        }).collect()
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let mut bus=I2cBitBang::new();
        bus.attach(AXP192_ADDRESS,Box::new(Axp192::new(self.pmic.clone())));
        bus.attach(TOUCH_I2C,Box::new(Core2Touch{device:Ft6336g::new(self.touch.clone()),state:self.touch.clone(),pmic:self.pmic.clone(),reset_count:self.pmic.lock().unwrap().panel_reset_count()}));
        bus.attach(RTC_I2C,Box::new(Core2Rtc{device:Bm8563::new(self.rtc.clone()),state:self.rtc.clone(),cycle:self.cycle.clone()}));
        bus.attach(IMU_I2C,Box::new(Mpu6886::new(self.imu.clone())));
        (0..2).flat_map(|index|bus.controller_devices(Some((21,22))).into_iter().map(move |(address,device)|(index,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(21,true),(22,true),(TOUCH_IRQ,self.touch_line_high())]}
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        self.panel_touch=down.then_some((x.min(319),y.min(239)));self.update_touch();
        self.touch_irq_high=self.touch_line_high();
    }
    fn touch_irq(&self)->Option<(u8,bool)> {Some((TOUCH_IRQ,self.touch_line_high()))}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        if id=="power" {self.power_held=pressed;self.pmic.lock().unwrap().button(cycle,pressed);return true;}
        let key=match id {"button-a"=>0,"button-b"=>1,"button-c"=>2,_=>return false};
        self.touch_keys[key]=pressed;self.update_touch();true
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {
        self.cycle.set(cycle);
        if self.power_held {self.pmic.lock().unwrap().advance_to(cycle);}
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {
        let high=self.touch_line_high();
        if high==self.touch_irq_high {return Vec::new();}
        self.touch_irq_high=high;vec![BoardEdge{cycle:self.cycle.get(),pin:TOUCH_IRQ,level:high}]
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let pmic=self.pmic.lock().unwrap();
        let visible=pmic.panel_ready() && pmic.backlight_on() && pmic.panel_reset_count()==self.reset_count && self.panel.on && !self.panel.sleeping;
        let pixels=if visible {self.panel.gram.clone()} else {vec![0;LCD_W*LCD_H]};
        Some((LCD_W as u32,LCD_H as u32,pixels,self.frame_version(&pmic)))
    }
    fn display_version(&self)->u64 {self.frame_version(&self.pmic.lock().unwrap())}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),LCD_W,LCD_H))}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        let mut sample=self.imu.lock().unwrap();
        sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn report(&self)->String {
        let pmic=self.pmic.lock().unwrap();
        format!("[core2] SPI ILI9342C: {} frames, {} pixels, MADCTL={:#04x}, COLMOD={:#04x}, display_on={}, LDO2={}, LCD_RST={}, DC3={}; FT6336U/G39; MPU6886 polled, BM8563 calendar",
            self.panel.frames,self.panel.pixels_written,self.panel.madctl,self.panel.colmod,self.panel.on,pmic.panel_powered(),pmic.gpio34_high(4),pmic.backlight_on())
    }
}
impl Core2 {
    fn frame_version(&self,pmic:&Axp192State)->u64 {
        self.panel.pixels_written.wrapping_add(self.display_changes).wrapping_add(pmic.panel_reset_count()).wrapping_mul(4)
            | u64::from(pmic.panel_ready())<<1 | u64::from(pmic.backlight_on())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<18,mosi:1<<23,miso:Some(38),cs:1<<5}}
    fn write(d:&mut dyn I2cDevice,reg:u8,values:&[u8]) {
        assert!(d.start(false));assert!(d.write(reg));for &value in values {assert!(d.write(value));}d.stop();
    }
    fn read(d:&mut dyn I2cDevice,reg:u8,n:usize)->Vec<u8> {
        write(d,reg,&[]);assert!(d.start(true));let bytes=(0..n).map(|_|d.read()).collect();d.stop();bytes
    }
    fn command(board:&mut Core2,command:u8,data:&[u8]) {
        board.gpio_changes(&[(15,false)]);board.spi_transfer_pins(3,pins(),&[command],0);
        board.gpio_changes(&[(15,true)]);board.spi_transfer_pins(3,pins(),data,0);
    }
    fn power_panel(pmic:&mut dyn I2cDevice) {write(pmic,0x12,&[0x07]);write(pmic,0x95,&[0x84]);write(pmic,0x96,&[2]);}
    #[test]
    fn lcd_requires_pmic_logic_reset_backlight_and_correct_wiring() {
        let mut board=Core2::new();
        let mut pmic=Axp192::new(board.pmic.clone());
        assert_eq!(board.spi_transfer_pins(3,pins(),&[4],3),[0xff;3]);
        power_panel(&mut pmic);
        assert_eq!(board.spi_transfer_pins(3,pins(),&[4],3),[0xe3,0,0]);
        for wrong in [SpiPins{sclk:1<<19,..pins()},SpiPins{mosi:1<<19,..pins()},SpiPins{miso:Some(4),..pins()},SpiPins{cs:1<<5>>1,..pins()}] {
            assert_eq!(board.spi_transfer_pins(3,wrong,&[4],3),[0xff;3]);
        }
        command(&mut board,0x11,&[]);command(&mut board,0x29,&[]);command(&mut board,0x3a,&[0x55]);command(&mut board,0x2c,&[0xf8,0,7,0xe0]);
        let (w,h,on,version)=board.display().unwrap();
        assert_eq!((w,h,&on[..2]),(320,240,&[0xf800,0x07e0][..]));
        write(&mut pmic,0x12,&[5]);let off=board.display().unwrap();assert!(off.2.iter().all(|&pixel|pixel==0));assert_ne!(off.3,version);
        write(&mut pmic,0x12,&[7]);assert_eq!(board.display().unwrap().2,on);
        write(&mut pmic,0x96,&[0]);write(&mut pmic,0x96,&[2]);assert!(board.display().unwrap().2.iter().all(|&pixel|pixel==0));
        command(&mut board,0x29,&[]);assert!(board.panel.sleeping);
    }
    #[test]
    fn touchscreen_keys_share_controller_registers_and_use_gpio39_irq() {
        let mut board=Core2::new();
        let mut devices=board.i2c_devices();
        assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((21,22))));
        let touch=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,TOUCH_I2C)).unwrap().2.as_mut();
        assert!(!touch.start(false));power_panel(&mut Axp192::new(board.pmic.clone()));
        board.touch(137,41,true);assert_eq!(board.touch_irq(),Some((39,false)));
        assert_eq!(read(touch,2,5),[1,0x80,137,0,41]);assert_eq!(read(touch,2,1),[1]);
        board.touch(137,41,false);assert_eq!(board.touch_irq(),Some((39,true)));assert_eq!(read(touch,2,1),[0]);
        assert!(board.button_at(100,"button-b",true));board.advance_to(100);
        assert_eq!(board.take_edges(),vec![BoardEdge{cycle:100,pin:39,level:false}]);
        assert_eq!(read(touch,2,5),[1,0x80,165,1,4]);assert_eq!(read(touch,2,1),[1]);
        assert!(board.button_at(120,"button-b",false));board.advance_to(120);
        assert_eq!(board.take_edges(),vec![BoardEdge{cycle:120,pin:39,level:true}]);assert_eq!(read(touch,2,1),[0]);
        assert_eq!(board.named_pin("button-b"),None);
        write(&mut Axp192::new(board.pmic.clone()),0x32,&[0x80]);assert!(!touch.start(false));
    }
    #[test]
    fn rtc_calendar_advances_with_guest_time_and_power_key_sets_register_status() {
        let mut board=Core2::new();
        let mut devices=board.i2c_devices();
        let rtc=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,RTC_I2C)).unwrap().2.as_mut();
        write(rtc,0,&[0]);write(rtc,2,&[0x59,0,0,1,0,1,0]);
        board.advance_to(CPU_HZ);assert_eq!(read(rtc,2,2),[0,1]);
        let mut pmic=Axp192::new(board.pmic.clone());
        assert!(board.button_at(CPU_HZ,"power",true));assert!(board.button_at(CPU_HZ+1,"power",false));
        assert_eq!(read(&mut pmic,0x46,1),[2]);write(&mut pmic,0x46,&[2]);assert_eq!(read(&mut pmic,0x46,1),[0]);
    }
}
