//! GC0308 SCCB register subset: manufacturer's GC0308 datasheet pp10-18/27-28,
//! and esp32-camera v2.1.4 sensors/gc0308.c. Camera analog processing is not modeled.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};
use crate::picture::PixelFormat;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct Gc0308Output {pub width:u32,pub height:u32,pub format:PixelFormat,pub window:[u32;4],pub mirror:bool,pub flip:bool}

pub struct Gc0308State {pub powered:bool,pub regs:[[u8;256];2],pub page:usize,pub writes:u64}
impl Default for Gc0308State {fn default()->Self {let mut s=Self{powered:false,regs:[[0;256];2],page:0,writes:0};s.reset();s}}
impl Gc0308State {
    pub fn reset(&mut self) {
        self.regs=[[0;256];2];self.page=0;
        for (r,v) in [(0,0x9b),(1,0x6a),(2,0x70),(4,0x96),(9,1),(10,0xe8),(11,2),(12,0x88),(0x24,0xa2),(0x26,0x3f),(0x49,1),(0x4a,0xe0),(0x4b,2),(0x4c,0x80)] {self.regs[0][r]=v;}
        self.regs[1][0x53]=0x82;self.regs[1][0x54]=3;self.regs[1][0x55]=3;
    }
    pub fn power(&mut self,powered:bool) {if self.powered!=powered {self.reset();self.powered=powered;}}
    /// Digital output gate/format/array ROI and the exact eight subsample tables
    /// in esp32-camera v2.1.4 gc0308.c. Other subsample patterns are withheld.
    /// Datasheet pp9,14-17,28: window includes eight non-output rows/columns;
    /// output_en=0x25 gates DVP; crop 0x46..4c acts after subsampling.
    pub fn output(&self)->Option<Gc0308Output> {
        let r=&self.regs[0];let s=&self.regs[1];if !self.powered||r[0x25]&15!=15||r[0x15]&0xc0!=0||r[0x1a]&1!=0||s[0x50]&0x10!=0{return None;}
        let format=match r[0x24]&31 {2=>PixelFormat::Yuyv,6=>PixelFormat::Rgb565,17=>PixelFormat::Grayscale,_=>return None};
        let word=|at:usize,mask:u8|(u32::from(r[at]&mask)<<8)|u32::from(r[at+1]);
        let(bw,bh)=(word(0x0b,3).checked_sub(8)?,word(9,1).checked_sub(8)?);let(col,row)=(word(7,3),word(5,1));
        if bw==0||bh==0||col+bw>640||row+bh>480{return None;}
        let ratio=match [s[0x54],s[0x56],s[0x57],s[0x58],s[0x59]] {
            [0x55,0,0,0,0]=>(1,5),[0x44,0,0,0,0]=>(1,4),[0x33,0,0,0,0]=>(1,3),[0x22,0,0,0,0]=>(1,2),
            [0x77,2,0x46,2,0x46]=>(4,7),[0x55,2,4,2,4]=>(3,5),[0x33,2,0,2,0]=>(2,3),[0x11,0,0,0,0]=>(1,1),_=>return None};
        let(num,den)=ratio;let(mut width,mut height)=(bw*num/den,bh*num/den);let(mut x,mut y)=(0,0);
        if r[0x46]&0x80!=0 {x=(u32::from(r[0x46]&7)<<8)|u32::from(r[0x48]);y=(u32::from(r[0x46]&0x30)<<4)|u32::from(r[0x47]);let(w,h)=(word(0x4b,3),word(0x49,1));if x+w>width||y+h>height{return None;}width=w;height=h;}
        if width==0||height==0||format==PixelFormat::Yuyv&&!width.is_multiple_of(2){return None;}
        Some(Gc0308Output{width,height,format,window:[col+x*den/num,row+y*den/num,col+(x+width)*den/num-1,row+(y+height)*den/num-1],mirror:r[0x14]&1!=0,flip:r[0x14]&2!=0})
    }
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
    #[test]
    fn output_enable_roi_subsample_crop_and_invalid_patterns() {
        let mut s=Gc0308State::default();s.power(true);assert!(s.output().is_none());
        s.regs[0][0x25]=15;s.regs[1][0x54]=0x11;
        let out=s.output().unwrap();assert_eq!((out.width,out.height,out.format,out.window),(640,480,PixelFormat::Yuyv,[0,0,639,479]));
        s.regs[1][0x54]=0x22;let out=s.output().unwrap();assert_eq!((out.width,out.height),(320,240));
        s.regs[0][0x46]=0x80;s.regs[0][0x48]=4;s.regs[0][0x47]=2;
        for(r,v)in[(0x49,0),(0x4a,4),(0x4b,0),(0x4c,8)]{s.regs[0][r]=v;}
        let out=s.output().unwrap();assert_eq!((out.width,out.height,out.window),(8,4,[8,4,23,11]));
        s.regs[0][0x24]=0xb1;assert_eq!(s.output().unwrap().format,PixelFormat::Grayscale);
        s.regs[0][0x24]=0xa6;assert_eq!(s.output().unwrap().format,PixelFormat::Rgb565);
        s.regs[0][0x48]=255;s.regs[0][0x4b]=2;assert!(s.output().is_none());s.regs[0][0x46]=0;
        s.regs[1][0x54]=0xff;assert!(s.output().is_none());s.regs[1][0x54]=0x11;s.regs[0][0x25]=7;assert!(s.output().is_none());
        s.power(false);assert!(s.output().is_none());
    }
}
