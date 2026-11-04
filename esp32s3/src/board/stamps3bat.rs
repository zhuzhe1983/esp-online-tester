//! S015 Stamp-S3Bat: ESP32-S3-PICO-1 N8R8, M5PM1 I2C SDA48/SCL47.
//! Official S015 v0.3 schematic and M5PM1 315d2899 RGB example. One LED is
//! driven by PM1 GPIO0, not ESP G21. Physical PWR only has a polling subset.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,m5::M5pm1State};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle};
use esp_soc::devices::i2c_bitbang::I2cBitBang;
use super::stamps3bat_power::{Power,Controls};

pub struct StampS3Bat {pmic:Arc<Mutex<M5pm1State>>,controls:Arc<Mutex<Controls>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,leds:[[u8;3];1],updates:u64}
impl StampS3Bat {pub fn new()->Self {Self{pmic:Default::default(),controls:Default::default(),i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),leds:[[0;3];1],updates:1}}}
impl Default for StampS3Bat {fn default()->Self {Self::new()}}
impl BoardModel for StampS3Bat {
    fn name(&self)->&'static str {"stamps3bat"}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x6e,Box::new(Power::new(self.pmic.clone(),self.controls.clone())));
        self.leds=[[0;3];1];self.updates=1;
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((48,47))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,_changes:&[(u8,bool)],enabled:u64,output:u64) {
        let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=high(47);let sda=self.i2c.drive(scl,high(48));
        for (i,(pin,level)) in [(48,sda),(47,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn advance_to(&mut self,_cycle:VirtualCycle) {let c=self.controls.lock().expect("S3Bat controls poisoned");self.leds=c.leds;self.updates=c.updates;}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(46,false),(48,self.levels[0]),(47,self.levels[1])]}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn button_at(&mut self,_cycle:VirtualCycle,id:&str,pressed:bool)->bool {id=="power" && self.controls.lock().expect("S3Bat controls poisoned").button(pressed)}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.updates))}
    fn report(&self)->String {let c=self.controls.lock().expect("S3Bat controls poisoned");let p=self.pmic.lock().expect("S3Bat power poisoned");format!("[stamps3bat] PM1 I2C48/47 RGB565 updates={} led-power={} button-poll-reads={} presses={} auto-reset/off/download=unsupported analog/network=unverified",c.updates,p.led_enabled,c.reads,c.presses)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_i2c_route_no_gpio_led_or_fake_power_key() {
        let mut b=StampS3Bat::new();let devices=b.i2c_devices();assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,d)|*a==0x6e && d.pins()==Some((48,47))));
        assert_eq!(b.named_pin("power"),None);assert_eq!(b.named_pin("button"),None);assert!(!b.button_at(0,"power",true));assert!(!b.button_at(0,"g0",true));
        b.rmt_frame(21,&[true;24]);b.rmt_frame(0,&[true;24]);b.advance_to(1);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],1));assert!(b.display().is_none());
    }
}
