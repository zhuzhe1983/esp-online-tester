//! M5Stack StackChan K151: CoreS3 host plus the PY32/Si12T/SCSCL robot base.
//! Sch_M5_CoreS3_v1.0.pdf, StackChan-BSP e2f3b02 and M5GFX 7cb26b44:
//! SPI CLK36/MOSI37/DC35/CS3, I2C SDA12/SCL11; touch INT via AW9523 P1_2 -> G21.
//! ILI9342C digital subset; no claim for the E panel's analog initialization.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample},ft6336::Ft6336State};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};
use esp_soc::uart::{UartRoute,UartInput};
use super::stackchan_host::{HostState,HostDevice,HostTouch};
use super::stackchan_base::StackChanBase;

pub struct StackChan {panel:DcsPanel,command:u8,read_at:usize,read_index:u8,reset_version:u64,display_changes:u64,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,host:Arc<Mutex<HostState>>,touch:Arc<Mutex<Ft6336State>>,imu:Arc<Mutex<Bmi270Sample>>,base:StackChanBase,irq_high:bool,cycle:VirtualCycle,gpio_events:u64}
impl StackChan {
    pub fn new()->Self {
        // Explicit USB-powered base profile; VM_EN remains guest controlled.
        let mut base=StackChanBase::new();base.set_powered(true);
        Self{panel:DcsPanel::new(320,240),command:0,read_at:0,read_index:0,reset_version:0,display_changes:0,i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),host:Arc::new(Mutex::new(HostState::new())),touch:Default::default(),imu:Default::default(),base,irq_high:true,cycle:0,gpio_events:0}}
    fn sync_host(&mut self) {
        let mut host=self.host.lock().expect("StackChan host mutex poisoned");
        if host.reset_version!=self.reset_version {self.reset_version=host.reset_version;self.panel.reset();self.command=0;self.read_at=0;self.read_index=0;self.display_changes+=1;}
        let mut touch=self.touch.lock().expect("StackChan touch mutex poisoned");touch.powered=host.touch_reset();if !touch.powered {touch.reset();host.touch_level(true);}
        let high=!host.irq();if high!=self.irq_high {self.irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:21,level:high});}
    }
}
impl Default for StackChan {fn default()->Self {Self::new()}}
impl BoardModel for StackChan {
    fn name(&self)->&'static str {"stackchan"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.gpio_events+=changes.len() as u64;for &(pin,high) in changes {if pin==35 {self.panel.dc=high;}}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(11);let sda=self.i2c.drive(scl,released(12));
        for (index,(pin,level)) in [(12,sda),(11,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        self.sync_host();
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.sync_host();
        if !self.host.lock().expect("StackChan host mutex poisoned").lcd_reset() || pins.sclk&(1<<36)==0 || pins.cs&(1<<3)==0 || (pins.mosi&(1<<37)==0 && pins.miso!=Some(37)) {return vec![0xff;rx_len];}
        for &byte in tx {
            if !self.panel.dc {self.command=byte;self.read_at=0;self.display_changes+=1;}
            else if self.command==0xd9 {self.read_index=byte&15;}
            self.panel.byte(byte);
        }
        // ILITEK ILI9342C V102 §§8.2.3 / 8.2.46–48: ID E3/00/00; EXTC ID4=00/93/42.
        // SPI peripheral accounts for the guest's dummy phase, as with the other DCS boards.
        (0..rx_len).map(|_| {let index=self.read_at+usize::from(self.read_index.saturating_sub(1));let value=match self.command {0x04=>[0xe3,0,0].get(self.read_at).copied().unwrap_or(0),0xda=>0xe3,0xdb|0xdc=>0,0xd3=>[0,0x93,0x42].get(index).copied().unwrap_or(0),_=>0};self.read_at=self.read_at.saturating_add(1);value}).collect()
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();
        self.i2c.attach(0x58,Box::new(HostDevice::new(self.host.clone(),false)));self.i2c.attach(0x34,Box::new(HostDevice::new(self.host.clone(),true)));
        self.i2c.attach(0x38,Box::new(HostTouch::new(self.host.clone(),self.touch.clone())));self.i2c.attach(0x69,Box::new(Bmi270::new(self.imu.clone())));
        for (address,device) in self.base.i2c_devices() {self.i2c.attach(address,device);}
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((12,11))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.sync_host();self.base.advance_to(cycle);}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_host();std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(21,self.irq_high),(12,self.levels[0]),(11,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"=>Some(0),_=>None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {self.advance_to(cycle);self.base.button(id,pressed)}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.cycle=cycle;self.touch(x,y,down);}
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        self.sync_host();let mut touch=self.touch.lock().expect("StackChan touch mutex poisoned");if !touch.powered {return;}touch.update(x.min(319),y.min(239),down);drop(touch);
        self.host.lock().expect("StackChan host mutex poisoned").touch_level(!down);self.sync_host();
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let h=self.host.lock().expect("StackChan host mutex poisoned");Some((320,240,if h.lcd_reset()&&h.backlight()&&self.panel.on&&!self.panel.sleeping {self.panel.gram.clone()} else {vec![0;320*240]},self.panel.pixels_written+self.panel.resets+self.display_changes+h.version))}
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes+self.host.lock().expect("StackChan host mutex poisoned").version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),320,240))}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}let mut sample=self.imu.lock().expect("StackChan IMU mutex poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true}
    fn uses_uart_pins(&self)->bool {true}
    fn uart_tx(&mut self,cycle:VirtualCycle,route:UartRoute,byte:u8) {self.base.uart_tx(cycle,route,byte);}
    fn uart_rx(&mut self,cycle:VirtualCycle)->Vec<UartInput> {self.base.uart_rx(cycle)}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some(self.base.leds())}
    fn motion_snapshot(&self)->Option<String> {Some(self.base.motion_snapshot())}
    fn report(&self)->String {format!("[stackchan] ILI9342C frames={} pixels={} FT6336U=single-contact Si12T=3-pad PY32=12-RGB servos={} camera/audio/NFC/RTC=unimplemented",self.panel.frames,self.panel.pixels_written,self.base.motion_snapshot())}
}

#[cfg(test)] mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<36,mosi:1<<37,cs:1<<3,..Default::default()}}
    fn wr(d:&mut dyn I2cDevice,reg:u8,values:&[u8]) {assert!(d.start(false));assert!(d.write(reg));for v in values {assert!(d.write(*v));}d.stop();}
    fn rd(d:&mut dyn I2cDevice,reg:u8,n:usize)->Vec<u8> {wr(d,reg,&[]);assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    fn cmd(b:&mut StackChan,c:u8,data:&[u8]) {b.gpio_changes(&[(35,false)]);b.spi_transfer_pins(2,pins(),&[c],0);b.gpio_changes(&[(35,true)]);b.spi_transfer_pins(2,pins(),data,0);}
    #[test] fn display_requires_its_physical_route_reset_and_backlight() {
        let mut b=StackChan::new();let mut devices=b.i2c_devices();let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();
        assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0xff;3]);wr(aw,3,&[2]);
        assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0xe3,0,0]);assert_eq!(b.spi_transfer_pins(2,SpiPins{cs:1<<4,..pins()},&[4],3),[0xff;3]);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0,7,0xe0]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        let axp=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x34).unwrap().2.as_mut();wr(axp,0x90,&[0x80]);wr(axp,0x99,&[28]);assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);wr(axp,0x90,&[0]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();wr(aw,3,&[0]);b.advance_to(1);assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0xff;3]);assert!(!b.panel.on);
    }
    #[test] fn ft6336_reaches_guest_registers_through_aw_interrupt_and_resets() {
        let mut b=StackChan::new();let mut devices=b.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((12,11))));
        let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();wr(aw,2,&[1]);wr(aw,5,&[4]);b.touch_at(10,123,234,true);assert!(b.input_levels().contains(&(21,false)));
        let touch=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert_eq!(rd(touch,2,5),[1,0x80,123,0,234]);assert_eq!(rd(touch,2,5),[1,0x80,123,0,234]);b.touch_at(11,123,234,false);assert_eq!(rd(touch,2,1),[0]);
        let aw=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x58).unwrap().2.as_mut();assert_eq!(rd(aw,1,1)[0]&4,4);b.advance_to(12);assert!(b.input_levels().contains(&(21,true)));wr(aw,2,&[0]);b.advance_to(13);
        let touch=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert!(!touch.start(false));
    }
    #[test] fn board_registration_does_not_alias_other_products() {let b=super::super::make_board("m5stack-k151").unwrap();assert_eq!(b.name(),"stackchan");assert!(b.named_pin("power").is_none());assert!(b.motion_snapshot().is_some());assert!(super::super::make_board("none").unwrap().motion_snapshot().is_none());}
}
