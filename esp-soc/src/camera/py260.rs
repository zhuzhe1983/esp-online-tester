//! PY260 module's MEGA_CCM control protocol, not an OV sensor register alias.
//! M5Stack UnitCamS3-UserDemo 50410249 unitcams3-5mp mega_ccm.c / sensor.h;
//! esp32-camera v2.1.4 mega_ccm.c uses the same resolution codes and newer raw
//! format codes (2/3 instead of M5's 0x10/0x11). Both published codes are accepted.
use esp_periph::i2c::I2cDevice;
use std::{collections::HashMap,sync::{Arc,Mutex}};
#[derive(Default,Debug,Clone,Copy,PartialEq,Eq)]
pub struct Py260State {pub width:u32,pub height:u32,pub format:u8,pub streaming:bool}
pub struct Py260 {regs:HashMap<u16,u8>,addr:u16,phase:u8,state:Arc<Mutex<Py260State>>}
impl Py260 {
    pub fn new(state:Arc<Mutex<Py260State>>)->Self {let mut s=Self{regs:HashMap::new(),addr:0,phase:0,state};s.reset();s}
    pub fn reset(&mut self) {self.regs.clear();self.regs.insert(0,3);self.regs.insert(1,0x9e);self.regs.insert(0x102,1);self.sync();}
    fn get(&self,r:u16)->u8 {*self.regs.get(&r).unwrap_or(&0)}
    fn sync(&self) {
        let (width,height)=match self.get(0x121) {1=>(320,240),2=>(640,480),3=>(1280,720),4=>(1600,1200),5=>(1920,1080),6=>(2592,1944),7=>(96,96),8=>(128,128),9=>(320,320),_=>(0,0)};
        *self.state.lock().unwrap()=Py260State{width,height,format:self.get(0x120),streaming:width!=0&&self.get(0x102)&1!=0};
    }
}
impl I2cDevice for Py260 {
    fn start(&mut self,read:bool)->bool {
        if !read {self.phase=0;}else if self.phase==1 {
            // The M5 branch probes ID with SCCB_Read (8-bit pointer), while
            // settings and newer Espressif detection use 16-bit pointers.
            self.addr>>=8;self.phase=2;
        }true
    }
    fn write(&mut self,b:u8)->bool {match self.phase {0=>{self.addr=u16::from(b)<<8;self.phase=1;},1=>{self.addr|=u16::from(b);self.phase=2;},_=>{
        if self.addr==0x102&&b&1==0 {self.reset();}
        if self.addr>2 {self.regs.insert(self.addr,b);self.sync();}self.addr=self.addr.wrapping_add(1);
    }}true}
    fn read(&mut self)->u8 {let b=self.get(self.addr);self.addr=self.addr.wrapping_add(1);b}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wr(s:&mut Py260,r:u16,b:u8) {s.start(false);for v in r.to_be_bytes().into_iter().chain([b]) {assert!(s.write(v));}}
    #[test]
    fn exact_module_identity_legacy_probe_and_resolution_codes() {
        let state=Arc::new(Mutex::new(Py260State::default()));let mut s=Py260::new(state.clone());
        for (reg,expected) in [(0,3),(1,0x9e)] {s.start(false);s.write(reg);s.stop();s.start(true);assert_eq!(s.read(),expected);}
        wr(&mut s,0x121,7);wr(&mut s,0x120,1);assert_eq!(*state.lock().unwrap(),Py260State{width:96,height:96,format:1,streaming:true});
        wr(&mut s,0x121,6);let config=*state.lock().unwrap();assert_eq!((config.width,config.height),(2592,1944));
        wr(&mut s,0x121,0xff);assert!(!state.lock().unwrap().streaming);
        wr(&mut s,0x102,0);assert_eq!(*state.lock().unwrap(),Py260State::default());wr(&mut s,0x102,1);assert!(!state.lock().unwrap().streaming);
        wr(&mut s,0x0000,0x56);s.start(false);s.write(0);s.write(0);s.start(true);assert_eq!((s.read(),s.read()),(3,0x9e));
    }
}
