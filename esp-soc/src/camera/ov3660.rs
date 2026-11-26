//! OV3660 SCCB/configuration subset, esp32-camera v2.1.4 sensors/ov3660.c,
//! ov3660_regs.h and ov3660_settings.h. No ISP, PLL, exposure or JPEG encoder.
use esp_periph::i2c::I2cDevice;
use std::{collections::HashMap,sync::{Arc,Mutex}};

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq)]
pub struct Ov3660State {
    pub width:u32,pub height:u32,
    /// FORMAT_CTRL00 (0x4300): 0x61 RGB565, 0x30 YUYV, 0x10 Y8.
    pub format:u8,pub streaming:bool,pub jpeg:bool,pub mirror:bool,pub flip:bool,
    /// Inclusive array window; the driver's full 4:3 window is 2080 x 1548.
    pub window:Option<[u32;4]>,
}
pub struct Ov3660 {regs:HashMap<u16,u8>,addr:u16,phase:u8,state:Arc<Mutex<Ov3660State>>}
impl Ov3660 {
    pub fn new(state:Arc<Mutex<Ov3660State>>)->Self {let mut s=Self{regs:HashMap::new(),addr:0,phase:0,state};s.reset();s}
    pub fn reset(&mut self) {self.regs.clear();self.regs.insert(0x300a,0x36);self.regs.insert(0x300b,0x60);self.sync();}
    fn get(&self,r:u16)->u8 {*self.regs.get(&r).unwrap_or(&0)}
    fn word(&self,r:u16,mask:u8)->u32 {(u32::from(self.get(r)&mask)<<8)|u32::from(self.get(r+1))}
    fn sync(&self) {
        let width=self.word(0x3808,15);let height=self.word(0x380a,7);
        *self.state.lock().unwrap()=Ov3660State{width,height,format:self.get(0x4300),jpeg:self.get(0x3821)&0x20!=0,
            mirror:self.get(0x3821)&2!=0,flip:self.get(0x3820)&2!=0,
            streaming:width!=0&&height!=0&&width<=2048&&height<=1536&&self.get(0x3008)&0x40==0,
            window:(0x3800..=0x3807).all(|r|self.regs.contains_key(&r)).then(||[self.word(0x3800,15),self.word(0x3802,7),self.word(0x3804,15),self.word(0x3806,7)])};
    }
}
impl I2cDevice for Ov3660 {
    fn start(&mut self,read:bool)->bool {if !read {self.phase=0;}true}
    fn write(&mut self,b:u8)->bool {
        match self.phase {0=>{self.addr=u16::from(b)<<8;self.phase=1;},1=>{self.addr|=u16::from(b);self.phase=2;},_=>{
            if self.addr==0x3008&&b&0x80!=0 {self.reset();}
            if ![0x300a,0x300b].contains(&self.addr) {self.regs.insert(self.addr,if self.addr==0x3008 {b&!0x80}else{b});self.sync();}
            self.addr=self.addr.wrapping_add(1);
        }}true
    }
    fn read(&mut self)->u8 {let b=self.get(self.addr);self.addr=self.addr.wrapping_add(1);b}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn write(s:&mut Ov3660,r:u16,bytes:&[u8]) {assert!(s.start(false));for b in r.to_be_bytes().iter().chain(bytes) {assert!(s.write(*b));}s.stop();}
    fn read(s:&mut Ov3660,r:u16)->u8 {write(s,r,&[]);assert!(s.start(true));s.read()}
    #[test]
    fn id_pointer_geometry_modes_standby_and_reset() {
        let state=Arc::new(Mutex::new(Ov3660State::default()));let mut s=Ov3660::new(state.clone());
        assert_eq!(read(&mut s,0x300a),0x36);assert_eq!(s.read(),0x60);write(&mut s,0x300a,&[0,0]);assert_eq!(read(&mut s,0x300a),0x36);
        assert!(!state.lock().unwrap().streaming);write(&mut s,0x3808,&[1,0x40,0,0xf0]);write(&mut s,0x4300,&[0x61]);
        let config=*state.lock().unwrap();assert_eq!((config.width,config.height),(320,240));
        assert!(state.lock().unwrap().streaming);write(&mut s,0x3008,&[0x42]);assert!(!state.lock().unwrap().streaming);
        write(&mut s,0x3008,&[2]);write(&mut s,0x3821,&[0x20]);assert!(state.lock().unwrap().jpeg);
        write(&mut s,0x3808,&[8,1]);assert!(!state.lock().unwrap().streaming);
        write(&mut s,0x3008,&[0x82]);assert_eq!(*state.lock().unwrap(),Ov3660State::default());assert_eq!(read(&mut s,0x300b),0x60);assert_eq!(read(&mut s,0x3008),2);
    }
}
