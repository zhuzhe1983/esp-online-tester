//! T-Display-P4 V1.0 TFT assembly: GPIO35 BOOT and XL9535 on I2C0/G7/G8.
//! Manufacturer driver a47cf73e and XINLUDA XL95x5 datasheet receipts:
//! docs/platform/research/lilygo-p4-profiles-2026-11-22.json.
//! The MIPI-DSI display, touch, radio and C6 coprocessor are deliberately absent.
//! Expander input peripherals and analog power are not simulated; undriven
//! inputs use a documented deterministic high level, not a physical pull-up claim.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,tca9555::{Tca9555,Tca9555State}};
use esp_soc::{BoardModel,board::BoardEdge};

struct Xl9535 {inner:Tca9555}
impl I2cDevice for Xl9535 {
    fn pins(&self)->Option<(u8,u8)> {Some((7,8))}
    fn start(&mut self,read:bool)->bool {self.inner.start(read)}
    fn write(&mut self,byte:u8)->bool {self.inner.write(byte)}
    fn read(&mut self)->u8 {self.inner.read()}
    fn stop(&mut self) {self.inner.stop();}
}

pub struct TDisplayP4 {boot_high:bool,edges:Vec<BoardEdge>,expander:Arc<Mutex<Tca9555State>>}
impl Default for TDisplayP4 {fn default()->Self {Self::new()}}
impl TDisplayP4 {
    pub fn new()->Self {Self{boot_high:true,edges:Vec::new(),expander:Arc::new(Mutex::new(Tca9555State::default()))}}
}
impl BoardModel for TDisplayP4 {
    fn name(&self)->&'static str {"lilygo-t-display-p4"}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(35,self.boot_high),(5,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"button"|"btnA"=>Some(35),_=>None}}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        if self.named_pin(id)!=Some(35) {return false;}
        let high=!pressed;
        if high!=self.boot_high {self.boot_high=high;self.edges.push(BoardEdge{cycle,pin:35,level:high});}
        true
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        // RST resets only the P4; keep the external expander's registers.
        vec![(0,0x20,Box::new(Xl9535{inner:Tca9555::new(self.expander.clone())}))]
    }
    fn report(&self)->String {
        format!("[lilygo-t-display-p4] V1.0: GPIO35 BOOT; XL9535 digital registers on I2C0/G7/G8, driven_high={:#06x}; MIPI-DSI/touch/C6/RF/camera/power unmodeled",self.expander.lock().expect("XL9535 state mutex poisoned").driven_high())
    }
}
