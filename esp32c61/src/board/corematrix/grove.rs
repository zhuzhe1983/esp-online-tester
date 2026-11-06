//! C158 schematic v0.7: Port A G0/SDA and G1/SCL share the internal bus;
//! IOE1 IO5 controls the socket's switched 5 V supply.
//! Reuse the audited U001-D/U136 digital protocols, never host readback values.
use super::{Wiring,CPU_HZ};
use std::rc::Rc;
use esp_periph::i2c::{I2cDevice,env4::{Env4,Environment},bh1750::Bh1750};
use esp_soc::devices::i2c_bitbang::I2cBitBang;

#[derive(Default)]
pub(super) struct Grove {env:Option<Box<Env4>>,light:Option<Box<Bh1750>>}
impl Grove {
    pub(super) fn set_power(&mut self,powered:bool) {
        if let Some(env)=&mut self.env {env.set_power(powered);}
        if let Some(light)=&mut self.light {light.set_power(powered);}
    }
    pub(super) fn advance_to(&mut self,cycle:u64) {
        if let Some(env)=&mut self.env {env.advance_to(cycle);}
        if let Some(light)=&mut self.light {light.advance_to(cycle);}
    }
    pub(super) fn environment(&mut self,cycle:u64,connected:bool,sample:Environment) {self.env.get_or_insert_with(||Box::new(Env4::new(CPU_HZ))).update(cycle,connected,sample);}
    pub(super) fn illuminance(&mut self,cycle:u64,connected:bool,lux:f64) {self.light.get_or_insert_with(||Box::new(Bh1750::new(CPU_HZ))).update(cycle,connected,lux);}
}
struct EnvDevice {wiring:Rc<Wiring>,address:u8}
impl I2cDevice for EnvDevice {
    fn start_address(&mut self,configured:u8,address:u8,read:bool)->bool {
        let general=self.address==0x44&&address==0&&!read;
        if !general&&configured!=address {return false;}self.wiring.sync();
        self.wiring.grove.lock().expect("CoreMatrix Grove poisoned").env.as_mut().is_some_and(|env|env.start(self.address,read,general))
    }
    fn start(&mut self,read:bool)->bool {self.start_address(self.address,self.address,read)}
    fn write(&mut self,b:u8)->bool {self.wiring.grove.lock().expect("CoreMatrix Grove poisoned").env.as_mut().is_some_and(|env|env.write(self.address,b))}
    fn read(&mut self)->u8 {self.wiring.grove.lock().expect("CoreMatrix Grove poisoned").env.as_mut().map_or(0xff,|env|env.read(self.address))}
}
struct LightDevice(Rc<Wiring>);
impl I2cDevice for LightDevice {
    fn start(&mut self,read:bool)->bool {self.0.sync();self.0.grove.lock().expect("CoreMatrix Grove poisoned").light.as_mut().is_some_and(|light|light.start(read))}
    fn write(&mut self,b:u8)->bool {self.0.grove.lock().expect("CoreMatrix Grove poisoned").light.as_mut().is_some_and(|light|light.write(b))}
    fn read(&mut self)->u8 {self.0.grove.lock().expect("CoreMatrix Grove poisoned").light.as_mut().map_or(0xff,|light|light.read())}
    fn stop(&mut self) {if let Some(light)=&mut self.0.grove.lock().expect("CoreMatrix Grove poisoned").light {light.stop();}}
}
pub(super) fn attach(bus:&mut I2cBitBang,wiring:Rc<Wiring>) {
    for address in [0x44,0x76] {bus.attach(address,Box::new(EnvDevice{wiring:wiring.clone(),address}));}
    bus.attach(0x23,Box::new(LightDevice(wiring)));
}
