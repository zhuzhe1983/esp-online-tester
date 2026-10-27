//! C139 V1.0 schematic, M5GFX c6f92dc / M5Unified 5580ff6.
//! ET047TC1 physical i80 routes; GT911 one-contact scans; BMI270 explicit samples.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State},bmi270::{Bmi270,Bmi270Sample}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,LcdI80Pins};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,et047tc1::Et047tc1};
pub struct PaperS3 {i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,output:u64,enabled:u64,panel:Et047tc1,touch:Arc<Mutex<Gt911State>>,touch_irq:bool,imu:Arc<Mutex<Bmi270Sample>>,gpio_events:u64,cycle:u64}
impl PaperS3 {
    pub fn new()->Self {Self{i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),output:0,enabled:0,panel:Et047tc1::new(),touch:Arc::new(Mutex::new(Gt911State::new(540,960,crate::periph::CPU_HZ/200))),touch_irq:true,imu:Default::default(),gpio_events:0,cycle:0}}
    fn powered(&self)->bool {let mask=(1<<45)|(1<<46);self.enabled&self.output&mask==mask}
    fn sync_touch(&mut self) {let mut t=self.touch.lock().expect("PaperS3 touch poisoned");t.advance_to(self.cycle);let level=!t.ready;if level!=self.touch_irq {self.touch_irq=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:48,level});}}
}
impl Default for PaperS3 {fn default()->Self {Self::new()}}
impl BoardModel for PaperS3 {
    fn name(&self)->&'static str {"papers3"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        let previous=self.output;self.cycle=cycle;self.output=output;self.enabled=enabled;self.gpio_events+=changes.len() as u64;
        let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=high(42);let sda=self.i2c.drive(scl,high(41));for (i,(pin,level)) in [(41,sda),(42,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        if !self.powered() {self.panel.stop_scan();return;}
        if previous&(1<<17)!=0 && output&(1<<17)==0 {self.panel.begin_scan();}
        if previous&(1<<15)==0 && output&(1<<15)!=0 {self.panel.latch();}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x5d,Box::new(Gt911::new(self.touch.clone())));self.i2c.attach(0x68,Box::new(Bmi270::new(self.imu.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((41,42))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(4,true),(5,true),(41,self.levels[0]),(42,self.levels[1]),(48,self.touch_irq)]}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_touch();std::mem::take(&mut self.edges)}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.sync_touch();}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn lcd_i80(&mut self,pins:LcdI80Pins,data_phase:bool,words:&[u16]) {
        let data=[6,14,7,12,9,11,8,10];
        if data_phase && pins.width==8 && pins.clock&(1<<16)!=0 && pins.select&(1<<13)!=0 && data.iter().enumerate().all(|(i,p)|pins.data[i]&(1<<p)!=0) && self.powered() && self.panel.scanning && self.output&(1<<15)==0 && self.output&(1<<18)!=0 {self.panel.line(words);}
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((540,960,self.panel.portrait(),self.panel.version))}
    fn display_version(&self)->u64 {self.panel.version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn display_quiet_push(&self)->bool {true}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.touch.lock().expect("PaperS3 touch poisoned").update(x,y,down);self.sync_touch();}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()){return false;}
        let mut imu=self.imu.lock().expect("PaperS3 IMU poisoned");imu.accel_g=accel_g;imu.gyro_dps=gyro_dps;imu.version=imu.version.wrapping_add(1);true
    }
    fn report(&self)->String {format!("[papers3] ET047TC1 bw frames={} row={} powered={} GT911=single-contact RTC/TF/audio/power-key=absent gray/network=unverified",self.panel.frames,self.panel.row,self.powered())}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_power_and_touch_are_distinct_from_paperdiy() {
        let mut b=PaperS3::new();assert_eq!(b.name(),"papers3");assert_eq!(b.named_pin("power"),None);let mask=(1<<45)|(1<<46)|(1<<18)|(1<<17)|(1<<15);
        b.gpio_output_at(1,&[],mask,1<<46);assert!(!b.powered());b.gpio_output_at(2,&[],mask,(1<<45)|(1<<46));assert!(b.powered());
        let devices=b.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((41,42))));assert!(!devices.iter().any(|(_,a,_)|*a==0x6e||*a==0x51));
        b.touch(100,200,true);assert!(b.take_edges().iter().any(|e|e.pin==48&&!e.level));let t=b.touch.lock().unwrap();assert_eq!((t.x,t.y),(100,200));
    }
}
