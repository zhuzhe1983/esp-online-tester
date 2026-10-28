//! GC0308 SCCB register subset: manufacturer's GC0308 datasheet pp10-18/27-28,
//! and esp32-camera v2.1.4 sensors/gc0308.c. Camera analog processing is not modeled.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Gc0308State {pub powered:bool,pub regs:[[u8;256];2],pub page:usize,pub writes:u64}
impl Default for Gc0308State {fn default()->Self {let mut s=Self{powered:false,regs:[[0;256];2],page:0,writes:0};s.reset();s}}
impl Gc0308State {
    pub fn reset(&mut self) {
        self.regs=[[0;256];2];self.page=0;
        for (r,v) in [(0,0x9b),(1,0x6a),(2,0x70),(4,0x96),(9,1),(10,0xe8),(11,2),(12,0x88),(0x24,0xa2),(0x26,0x3f),(0x49,1),(0x4a,0xe0),(0x4b,2),(0x4c,0x80)] {self.regs[0][r]=v;}
        self.regs[1][0x53]=0x82;self.regs[1][0x54]=3;self.regs[1][0x55]=3;
    }
    pub fn power(&mut self,powered:bool) {if self.powered!=powered {self.reset();self.powered=powered;}}
}
pub struct Gc0308 {state:Arc<Mutex<Gc0308State>>,address:u8,expect_address:bool}
impl Gc0308 {pub fn new(state:Arc<Mutex<Gc0308State>>)->Self {Self{state,address:0,expect_address:true}}}
impl I2cDevice for Gc0308 {
    fn start(&mut self,read:bool)->bool {if !read {self.expect_address=true;}self.state.lock().unwrap().powered}
    fn write(&mut self,value:u8)->bool {
        let mut s=self.state.lock().unwrap();if !s.powered {return false;}
        if self.expect_address {self.address=value;self.expect_address=false;return true;}
        if self.address==0xfe {if value&0x80!=0 {s.reset();} else {s.page=usize::from(value&1);}}
        else {let page=s.page;if !(page==0&&self.address==0) {s.regs[page][usize::from(self.address)]=value;}}
        s.writes+=1;self.address=self.address.wrapping_add(1);true
    }
    fn read(&mut self)->u8 {
        let s=self.state.lock().unwrap();if !s.powered {return 0xff;}
        let value=if self.address==0xfe {s.page as u8} else {s.regs[s.page][usize::from(self.address)]};self.address=self.address.wrapping_add(1);value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Gc0308,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn read(d:&mut Gc0308,r:u8)->u8 {assert!(d.start(false));assert!(d.write(r));assert!(d.start(true));d.read()}
    #[test]
    fn power_page_id_reset_and_wrap() {
        let state=Arc::new(Mutex::new(Gc0308State::default()));let mut d=Gc0308::new(state.clone());assert!(!d.start(false));assert!(!d.write(0));assert_eq!(d.read(),0xff);
        state.lock().unwrap().power(true);assert_eq!(read(&mut d,0),0x9b);write(&mut d,0,0);assert_eq!(read(&mut d,0),0x9b);
        write(&mut d,0x24,0xa6);write(&mut d,0xfe,1);assert_eq!(read(&mut d,0x24),0);write(&mut d,0x24,0x77);write(&mut d,0xfe,0);assert_eq!(read(&mut d,0x24),0xa6);
        write(&mut d,0xfe,0x80);assert_eq!(read(&mut d,0x24),0xa2);assert_eq!(read(&mut d,0x25),0);
        write(&mut d,0xff,0x55);assert!(d.write(0));assert_eq!(read(&mut d,0),0x9b);
        state.lock().unwrap().power(false);assert!(!d.start(true));state.lock().unwrap().power(true);assert_eq!(read(&mut d,0xff),0);
    }
}
