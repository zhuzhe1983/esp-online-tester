//! M5Stack Core2 K010 variants: manufacturer schematics and M5Core2 67099371.
//! ILI9342C SPI G18/G23/G38/G5/G15; FT6336U on G21/G22 with INT on G39.
//! The AXP192 and AXP2101 boards supply the glass and its reset with different outputs.
use std::{cell::{Cell,RefCell},rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,axp192::{AXP192_ADDRESS,Axp192,Axp192State},ft6336::{Ft6336g,Ft6336State},mpu6886::{Mpu6886,Mpu6886Sample},bmi270::{Bmi270,Bmi270Sample},ina3221::{Ina3221,Ina3221State}};
use esp_periph::sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard};
use esp_soc::board::{BoardEdge,BoardModel,SpiPins,VirtualCycle};
use esp_soc::devices::{DcsPanel,bm8563::{Bm8563,Bm8563State},i2c_bitbang::I2cBitBang};
use crate::periph::CPU_HZ;
use super::tbeam::axp2101::{Axp2101,AxpDevice};

const LCD_W:usize=320;
const LCD_H:usize=240;
const TOUCH_IRQ:u8=39;
const TOUCH_I2C:u8=0x38;
const RTC_I2C:u8=0x51;
const IMU_I2C:u8=0x68;

type TfSlot = Rc<RefCell<Option<SpiSdCard>>>;

#[derive(Clone,Copy,Eq,PartialEq)]
pub enum Revision {Original,V11,V13,Aws,AwsV13}

impl Revision {
    fn uses_bmi270(self)->bool {matches!(self,Self::V13|Self::AwsV13)}
    fn name(self)->&'static str {
        match self {Self::Original=>"core2",Self::V11=>"core2-v11",Self::V13=>"core2-v13",Self::Aws=>"core2-aws",Self::AwsV13=>"core2-aws-v13"}
    }
}

enum Pmic { Axp192(Arc<Mutex<Axp192State>>), Axp2101(Arc<Mutex<Axp2101>>) }

struct DisplayRails { logic:bool, reset_high:bool, backlight:bool }
impl DisplayRails {fn ready(&self)->bool {self.logic&&self.reset_high}}

struct Core2Power {pmic:Pmic,reset_count:Cell<u64>}
impl Core2Power {
    fn new(revision:Revision)->Self {
        let pmic=if revision==Revision::V11 {Pmic::Axp2101(Arc::new(Mutex::new(Axp2101::new())))}
            else {Pmic::Axp192(Arc::new(Mutex::new(Axp192State::new(CPU_HZ))))};
        Self{pmic,reset_count:Cell::new(0)}
    }
    fn device(&self)->Box<dyn I2cDevice> {
        match &self.pmic {Pmic::Axp192(state)=>Box::new(Axp192::new(state.clone())),Pmic::Axp2101(state)=>Box::new(AxpDevice::new(state.clone()))}
    }
    fn rails(&self)->DisplayRails {
        match &self.pmic {
            Pmic::Axp192(state)=>{
                let state=state.lock().unwrap();
                DisplayRails{logic:state.panel_powered(),reset_high:state.gpio34_high(4),backlight:state.backlight_on()}
            }
            Pmic::Axp2101(state)=>{
                let state=state.lock().unwrap();
                // Core2 v1.1 schematic and M5GFX 28d47949 pmic_ops.hpp:
                // ALDO4 is LCD logic, ALDO2 releases reset, BLDO1 is the backlight.
                let enabled=if state.power_off_requested {0}else{state.regs[0x90]};
                DisplayRails{logic:enabled&8!=0,reset_high:enabled&2!=0,backlight:enabled&16!=0}
            }
        }
    }
    fn key(&self,cycle:VirtualCycle,pressed:bool) {
        match &self.pmic {
            Pmic::Axp192(state)=>state.lock().unwrap().button(cycle,pressed),
            Pmic::Axp2101(state)=>state.lock().unwrap().button(cycle,pressed),
        }
    }
    fn advance_key(&self,cycle:VirtualCycle) {
        match &self.pmic {
            Pmic::Axp192(state)=>state.lock().unwrap().advance_to(cycle),
            Pmic::Axp2101(state)=>state.lock().unwrap().advance_to(cycle),
        }
    }
}

// Both schematic variants connect the socket supply to the LCD logic rail.
// Losing that rail resets the card protocol and preserves its backing store.
struct Core2Pmic {device:Box<dyn I2cDevice>,power:Rc<Core2Power>,touch:Arc<Mutex<Ft6336State>>,tf:TfSlot}
impl I2cDevice for Core2Pmic {
    fn start(&mut self,read:bool)->bool {self.device.start(read)}
    fn write(&mut self,byte:u8)->bool {
        let before=self.power.rails();
        let ack=self.device.write(byte);
        let after=self.power.rails();
        if before.ready() && !after.ready() {
            self.power.reset_count.set(self.power.reset_count.get().wrapping_add(1));
            self.touch.lock().unwrap().reset();
        }
        if before.logic && !after.logic {
            if let Some(card)=self.tf.borrow_mut().as_mut() {card.power_cycle();}
        }
        ack
    }
    fn read(&mut self)->u8 {self.device.read()}
}

struct Core2Touch {
    device:Ft6336g,
    state:Arc<Mutex<Ft6336State>>,
    power:Rc<Core2Power>,
    reset_count:u64,
}

impl I2cDevice for Core2Touch {
    fn start(&mut self,read:bool)->bool {
        let mut state=self.state.lock().unwrap();
        state.powered=self.power.rails().ready();
        let reset_count=self.power.reset_count.get();
        if self.reset_count!=reset_count {
            self.reset_count=reset_count;
            self.device=Ft6336g::new(self.state.clone());
        }
        drop(state);self.device.start(read)
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

struct Core2Ina {device:Ina3221,state:Arc<Mutex<Ina3221State>>,cycle:Rc<Cell<VirtualCycle>>}
impl I2cDevice for Core2Ina {
    fn start(&mut self,read:bool)->bool {
        self.state.lock().unwrap().advance_to(self.cycle.get());
        self.device.start(read)
    }
    fn write(&mut self,byte:u8)->bool {self.device.write(byte)}
    fn read(&mut self)->u8 {self.device.read()}
}

enum Core2Imu { Mpu6886(Arc<Mutex<Mpu6886Sample>>), Bmi270(Arc<Mutex<Bmi270Sample>>) }

pub struct Core2 {
    revision:Revision,
    panel:DcsPanel,
    command:u8,
    read_at:usize,
    read_index:u8,
    id_dummy_seen:bool,
    reset_count:u64,
    display_changes:u64,
    power:Rc<Core2Power>,
    tf:TfSlot,
    touch:Arc<Mutex<Ft6336State>>,
    imu:Core2Imu,
    rtc:Arc<Mutex<Bm8563State>>,
    i2c:I2cBitBang,
    i2c_levels:[bool;2],
    edges:Vec<BoardEdge>,
    cycle:Rc<Cell<VirtualCycle>>,
    touch_irq_high:bool,
    panel_touch:Option<(u16,u16)>,
    touch_keys:[bool;3],
    power_held:bool,
}

impl Core2 {
    pub fn new()->Self {Self::with_revision(Revision::Original)}
    pub fn with_revision(revision:Revision)->Self {
        let imu=if revision.uses_bmi270() {Core2Imu::Bmi270(Default::default())}
            else {Core2Imu::Mpu6886(Default::default())};
        Self{
            revision,panel:DcsPanel::new(LCD_W,LCD_H),command:0,read_at:0,read_index:0,id_dummy_seen:false,reset_count:0,display_changes:0,
            power:Rc::new(Core2Power::new(revision)),tf:Default::default(),touch:Default::default(),imu,
            rtc:Arc::new(Mutex::new(Bm8563State::new(CPU_HZ))),i2c:I2cBitBang::new(),i2c_levels:[true;2],edges:Vec::new(),
            cycle:Default::default(),touch_irq_high:true,
            panel_touch:None,touch_keys:[false;3],power_held:false,
        }
    }
    fn contact(&self)->Option<(u16,u16)> {
        // M5Core2.h BtnA/B/C rectangles extend past the LCD down to raw y=279.
        self.panel_touch.or_else(||self.touch_keys.iter().position(|&down|down).map(|index|([65,165,270][index],260)))
    }
    fn update_touch(&mut self) {
        let contact=self.contact().filter(|_|self.power.rails().ready());
        let mut touch=self.touch.lock().unwrap();
        let (x,y)=contact.unwrap_or((touch.x,touch.y));
        touch.update(x,y,contact.is_some());
    }
    fn touch_line_high(&self)->bool {
        self.contact().is_none() || !self.power.rails().ready()
    }
}
impl Default for Core2 {fn default()->Self {Self::new()}}

impl BoardModel for Core2 {
    fn name(&self)->&'static str {self.revision.name()}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {
        for &(pin,level) in changes {
            match pin {
                15=>self.panel.dc=level,
                4 if level=>{if let Some(card)=self.tf.borrow_mut().as_mut() {card.deselect();}}
                _=>{}
            }
        }
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_changes(changes);
        // M5GFX uses a GPIO STOP between hardware I2C messages. The GPIO and
        // the controllers must see the same register-transaction state.
        let released=|pin|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(22u8);let sda=self.i2c.drive(scl,released(21u8));
        for (index,(pin,level)) in [(21,sda),(22,scl)].into_iter().enumerate() {
            if self.i2c_levels[index]!=level {
                self.i2c_levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});
            }
        }
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if pins.sclk&(1<<18)==0 || (!tx.is_empty() && pins.mosi&(1<<23)==0) {return vec![0xff;rx_len];}
        let lcd_selected=pins.cs&(1<<5)!=0;
        let tf_selected=pins.cs&(1<<4)!=0;
        let rails=self.power.rails();
        if !lcd_selected {
            if let (true,Some(card))=(rails.logic,self.tf.borrow_mut().as_mut()) {
                let result=card.transfer(tf_selected,tx,rx_len);
                if pins.miso==Some(38) {return result;}
            }
            return vec![0xff;rx_len];
        }
        if tf_selected && rails.logic && self.tf.borrow().is_some() {return vec![0xff;rx_len];}

        if self.power.reset_count.get()!=self.reset_count {
            self.reset_count=self.power.reset_count.get();self.panel.reset();
            self.command=0;self.read_at=0;self.read_index=0;self.id_dummy_seen=false;
        }
        if !rails.ready() {return vec![0xff;rx_len];}
        for &byte in tx {
            if !self.panel.dc {
                self.command=byte;self.read_at=0;self.id_dummy_seen=false;
                if matches!(byte,0x01|0x10|0x11|0x28|0x29) {self.display_changes=self.display_changes.wrapping_add(1);}
            } else if self.command==0xd9 {self.read_index=byte&15;}
            self.panel.byte(byte);
        }
        if pins.miso!=Some(38) {return vec![0xff;rx_len];}
        // ILITEK ILI9342C V102 §§8.2.3, 8.2.46–48: ID E3/00/00, ID4 00/93/42.
        // M5GFX b9639027 _read_panel_id clocks RDDID's dummy separately. The
        // classic SPI callback rounds that first bit to a one-byte transaction.
        if self.command==0x04 && tx.is_empty() && rx_len==1 && self.read_at==0 && !self.id_dummy_seen {
            self.id_dummy_seen=true;return vec![0];
        }
        (0..rx_len).map(|_|{
            let index=self.read_at.saturating_add(usize::from(self.read_index.saturating_sub(1)));
            let value=match self.command {0x04=>[0xe3,0,0].get(self.read_at).copied().unwrap_or(0),0xda=>0xe3,0xd3=>[0,0x93,0x42].get(index).copied().unwrap_or(0),_=>0};
            self.read_at=self.read_at.saturating_add(1);value
        }).collect()
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();
        self.i2c.attach(AXP192_ADDRESS,Box::new(Core2Pmic{device:self.power.device(),power:self.power.clone(),touch:self.touch.clone(),tf:self.tf.clone()}));
        self.i2c.attach(TOUCH_I2C,Box::new(Core2Touch{device:Ft6336g::new(self.touch.clone()),state:self.touch.clone(),power:self.power.clone(),reset_count:self.power.reset_count.get()}));
        self.i2c.attach(RTC_I2C,Box::new(Core2Rtc{device:Bm8563::new(self.rtc.clone()),state:self.rtc.clone(),cycle:self.cycle.clone()}));
        let imu:Box<dyn I2cDevice>=match &self.imu {Core2Imu::Mpu6886(s)=>Box::new(Mpu6886::new(s.clone())),Core2Imu::Bmi270(s)=>Box::new(Bmi270::new(s.clone()))};
        self.i2c.attach(IMU_I2C,imu);
        if self.revision==Revision::V11 {
            let state=Arc::new(Mutex::new(Ina3221State::new(CPU_HZ)));
            self.i2c.attach(0x40,Box::new(Core2Ina{device:Ina3221::with_pins(state.clone(),(21,22)),state,cycle:self.cycle.clone()}));
        }
        (0..2).flat_map(|index|self.i2c.controller_devices(Some((21,22))).into_iter().map(move |(address,device)|(index,address,device))).collect()
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        let card=SpiSdCard::new(image,config)?;let description=card.describe();*self.tf.borrow_mut()=Some(card);Ok(description)
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(21,self.i2c_levels[0]),(22,self.i2c_levels[1]),(TOUCH_IRQ,self.touch_line_high())]}
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        self.panel_touch=down.then_some((x.min(319),y.min(239)));self.update_touch();
        self.touch_irq_high=self.touch_line_high();
    }
    fn touch_irq(&self)->Option<(u8,bool)> {Some((TOUCH_IRQ,self.touch_line_high()))}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        if id=="power" {self.power_held=pressed;self.power.key(cycle,pressed);return true;}
        let key=match id {"button-a"=>0,"button-b"=>1,"button-c"=>2,_=>return false};
        self.touch_keys[key]=pressed;self.update_touch();true
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {
        self.cycle.set(cycle);
        if self.power_held {self.power.advance_key(cycle);}
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {
        let high=self.touch_line_high();
        if high!=self.touch_irq_high {
            self.touch_irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle.get(),pin:TOUCH_IRQ,level:high});
        }
        std::mem::take(&mut self.edges)
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let rails=self.power.rails();
        let visible=rails.ready() && rails.backlight && self.power.reset_count.get()==self.reset_count && self.panel.on && !self.panel.sleeping;
        let pixels=if visible {self.panel.gram.clone()} else {vec![0;LCD_W*LCD_H]};
        Some((LCD_W as u32,LCD_H as u32,pixels,self.frame_version(&rails)))
    }
    fn display_version(&self)->u64 {self.frame_version(&self.power.rails())}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),LCD_W,LCD_H))}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        match &self.imu {
            Core2Imu::Mpu6886(sample)=>{
                let mut sample=sample.lock().unwrap();
                sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);
            }
            Core2Imu::Bmi270(sample)=>{
                let mut sample=sample.lock().unwrap();
                sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);
            }
        }
        true
    }
    fn report(&self)->String {
        let rails=self.power.rails();
        let imu=if self.revision.uses_bmi270() {"BMI270"}else{"MPU6886"};
        format!("[{}] SPI ILI9342C: {} frames, {} pixels, MADCTL={:#04x}, COLMOD={:#04x}, display_on={}, logic={}, reset={}, backlight={}; FT6336U/G39; {imu} polled, BM8563 calendar; SPI TF={}",
            self.name(),self.panel.frames,self.panel.pixels_written,self.panel.madctl,self.panel.colmod,self.panel.on,rails.logic,rails.reset_high,rails.backlight,self.tf.borrow().as_ref().map_or_else(||"absent".into(),SpiSdCard::report))
    }
}
impl Core2 {
    fn frame_version(&self,rails:&DisplayRails)->u64 {
        // Hardware reset returns the panel's pixel counter to zero, so include
        // both state generations and current power gates in the presentation key.
        self.panel.pixels_written.wrapping_add(self.display_changes).wrapping_add(self.power.reset_count.get()).wrapping_mul(4)
            | u64::from(rails.ready())<<1 | u64::from(rails.backlight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<18,mosi:1<<23,miso:Some(38),cs:1<<5,keep_cs:false}}
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
    fn device(board:&Core2)->Core2Pmic {
        Core2Pmic{device:board.power.device(),power:board.power.clone(),touch:board.touch.clone(),tf:board.tf.clone()}
    }
    fn power_panel(pmic:&mut dyn I2cDevice) {write(pmic,0x12,&[0x07]);write(pmic,0x95,&[0x84]);write(pmic,0x96,&[2]);}
    #[test]
    fn lcd_requires_pmic_logic_reset_backlight_and_correct_wiring() {
        let mut board=Core2::new();
        let mut pmic=device(&board);
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
    fn tf_requires_its_cs_and_supply_and_restarts_after_a_pmic_rail_cut() {
        let mut board=Core2::new();
        board.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
        let mut devices=board.i2c_devices();
        let pmic=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,AXP192_ADDRESS)).unwrap().2.as_mut();
        let tf=SpiPins{cs:1<<4,..pins()};
        let no_select=SpiPins{cs:0,..pins()};
        let cmd=[0x40,0,0,0,0,0x95];
        assert_eq!(board.spi_transfer_pins(3,tf,&cmd,2),[0xff;2]);
        power_panel(pmic);board.spi_transfer_pins(3,no_select,&[255;10],0);
        board.spi_transfer_pins(3,SpiPins{cs:1<<16,..pins()},&cmd,0);
        assert_eq!(board.spi_transfer_pins(3,tf,&[],2),[0xff;2]);
        board.spi_transfer_pins(3,tf,&cmd,0);
        assert_eq!(board.spi_transfer_pins(3,tf,&[],2),[0xff,1]);
        write(pmic,0x12,&[0]);write(pmic,0x12,&[4]);
        board.spi_transfer_pins(3,tf,&cmd,0);
        assert_eq!(board.spi_transfer_pins(3,tf,&[],2),[0xff;2]);
    }
    #[test]
    fn touchscreen_keys_share_controller_registers_and_use_gpio39_irq() {
        let mut board=Core2::new();
        let mut devices=board.i2c_devices();
        assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((21,22))));
        let touch=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,TOUCH_I2C)).unwrap().2.as_mut();
        assert!(!touch.start(false));power_panel(&mut device(&board));
        board.touch(137,41,true);assert_eq!(board.touch_irq(),Some((39,false)));
        assert_eq!(read(touch,2,5),[1,0x80,137,0,41]);assert_eq!(read(touch,2,1),[1]);
        board.touch(137,41,false);assert_eq!(board.touch_irq(),Some((39,true)));assert_eq!(read(touch,2,1),[0]);
        assert!(board.button_at(100,"button-b",true));board.advance_to(100);
        assert_eq!(board.take_edges(),vec![BoardEdge{cycle:100,pin:39,level:false}]);
        assert_eq!(read(touch,2,5),[1,0x80,165,1,4]);assert_eq!(read(touch,2,1),[1]);
        assert!(board.button_at(120,"button-b",false));board.advance_to(120);
        assert_eq!(board.take_edges(),vec![BoardEdge{cycle:120,pin:39,level:true}]);assert_eq!(read(touch,2,1),[0]);
        assert_eq!(board.named_pin("button-b"),None);
        write(&mut device(&board),0x32,&[0x80]);assert!(!touch.start(false));
    }
    #[test]
    fn revision_power_and_sensor_ids_follow_each_schematic() {
        for (revision,pmic_id,bmi) in [(Revision::Original,3,false),(Revision::V11,0x4a,false),
            (Revision::V13,3,true),(Revision::Aws,3,false),(Revision::AwsV13,3,true)] {
            let mut board=Core2::with_revision(revision);
            let mut devices=board.i2c_devices();
            let axp=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,AXP192_ADDRESS)).unwrap().2.as_mut();
            assert_eq!(read(axp,3,1),[pmic_id]);
            if revision==Revision::V11 {
                power_panel(axp);assert!(!board.power.rails().ready(),"AXP192 commands cannot enable AXP2101 outputs");
                write(axp,0x90,&[0x0a]);assert!(board.power.rails().ready());assert!(!board.power.rails().backlight);
                write(axp,0x90,&[0x1a]);assert!(board.power.rails().backlight);
                let ina=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,0x40)).unwrap().2.as_mut();
                assert_eq!(read(ina,0xff,2),[0x32,0x20]);
            } else {assert!(devices.iter().all(|(_,addr,_)|*addr!=0x40));}
            let imu=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,IMU_I2C)).unwrap().2.as_mut();
            assert_eq!(read(imu,if bmi {0} else {0x75},1),[if bmi {0x24} else {0x19}]);
            assert!(devices.iter().all(|(_,addr,_)|*addr!=0x35),"ATECC608 is intentionally absent");
        }
    }
    #[test]
    fn rtc_calendar_advances_with_guest_time_and_power_key_sets_register_status() {
        let mut board=Core2::new();
        let mut devices=board.i2c_devices();
        let rtc=devices.iter_mut().find(|(n,a,_)|(*n,*a)==(1,RTC_I2C)).unwrap().2.as_mut();
        write(rtc,0,&[0]);write(rtc,2,&[0x59,0,0,1,0,1,0]);
        board.advance_to(CPU_HZ);assert_eq!(read(rtc,2,2),[0,1]);
        let mut pmic=device(&board);
        assert!(board.button_at(CPU_HZ,"power",true));assert!(board.button_at(CPU_HZ+1,"power",false));
        assert_eq!(read(&mut pmic,0x46,1),[2]);write(&mut pmic,0x46,&[2]);assert_eq!(read(&mut pmic,0x46,1),[0]);
    }
}
