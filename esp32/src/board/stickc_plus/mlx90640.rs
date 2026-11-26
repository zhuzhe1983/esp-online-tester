//! T-Lite MLX90640 digital acquisition subset. M5StickC-Plus-TLite-FW
//! 5740227 src/mlx90640.cpp: 16-bit BE word pointer/data, EEPROM0x2400,
//! 832-word frame0x0400, status0x8000 ready bit3/subpage0, control0x800D.
//! EEPROM below is an explicitly VIRTUAL nominal calibration, never a silicon dump.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
const HZ:u64=240_000_000;
pub struct Mlx90640 {pub cycle:u64,pub ready:bool,pub subpage:bool,pub control:u16,pub frames:u64,pub reads:u64,pub writes:u64,next:u64,scene:[u16;5],ram:Vec<u16>,eeprom:Vec<u16>}
impl Default for Mlx90640 {fn default()->Self{Self::new()}}
impl Mlx90640 {
    pub fn new()->Self {
        let mut e=vec![0;832];e[7]=0x5349;e[8]=0x4d00;e[16]=0;e[17]=8000;e[32]=0;e[33]=1024;e[48]=1000;e[49]=29127;e[50]=8;e[51]=0x01ff;e[52]=0x1111;e[54]=0x0101;e[55]=0x0101;e[56]=0x2f00;e[57]=100;e[63]=0x1440;e[64..].fill(0x10);
        Self{cycle:0,ready:false,subpage:false,control:0x1901,frames:0,reads:0,writes:0,next:0,scene:[8000,9000,80,60,12],ram:vec![0;832],eeprom:e}
    }
    fn period(&self)->u64{let rate=(self.control>>7)&7;(HZ*2)>>rate}
    pub fn advance_to(&mut self,cycle:u64){self.cycle=cycle.max(self.cycle);if !self.ready&&self.cycle>=self.next {self.subpage= !self.subpage;self.ready=true;self.frames=self.frames.wrapping_add(1);for y in 0..24{for x in 0..32{let dx=x*5+2-i32::from(self.scene[2]);let dy=y*5+2-i32::from(self.scene[3]);self.ram[(y*32+x)as usize]=if dx*dx+dy*dy<=i32::from(self.scene[4]).pow(2){self.scene[1]}else{self.scene[0]};}}self.ram[768]=1000;self.ram[778]=1000;self.ram[800]=1000;self.ram[810]=(-8224i16)as u16;}}
    pub fn set_scene(&mut self,scene:[u16;5])->bool{if scene[0]==0||scene[1]==0||scene[0]>16383||scene[1]>16383||scene[2]>=160||scene[3]>=120||scene[4]>160{return false;}self.scene=scene;true}
    fn word(&mut self,a:u16)->u16{self.reads=self.reads.wrapping_add(1);match a{0x0400..=0x073f=>self.ram[usize::from(a-0x0400)],0x2400..=0x273f=>self.eeprom[usize::from(a-0x2400)],0x8000=>0x30|if self.ready{8}else{0}|u16::from(self.subpage),0x800d=>self.control,_=>0}}
    fn write(&mut self,a:u16,w:u16){self.writes=self.writes.wrapping_add(1);match a{0x8000=>{if w&8==0{self.ready=false;self.next=self.cycle.saturating_add(self.period());}},0x800d=>{self.control=w;},_=>{}}}
}
pub struct Endpoint {pub state:Arc<Mutex<Mlx90640>>,pointer:u16,phase:u8,high:u8,read_low:Option<u8>}
impl Endpoint {pub fn new(state:Arc<Mutex<Mlx90640>>)->Self{Self{state,pointer:0,phase:0,high:0,read_low:None}}}
impl I2cDevice for Endpoint {
    fn start(&mut self,read:bool)->bool{if !read{self.phase=0;}self.read_low=None;true}
    fn write(&mut self,b:u8)->bool{match self.phase{0=>{self.pointer=u16::from(b)<<8;self.phase=1;},1=>{self.pointer|=u16::from(b);self.phase=2;},2=>{self.high=b;self.phase=3;},_=>{self.state.lock().unwrap().write(self.pointer,u16::from_be_bytes([self.high,b]));self.pointer=self.pointer.wrapping_add(1);self.phase=2;}}true}
    fn read(&mut self)->u8{if let Some(low)=self.read_low.take(){self.pointer=self.pointer.wrapping_add(1);low}else{let [h,l]=self.state.lock().unwrap().word(self.pointer).to_be_bytes();self.read_low=Some(l);h}}
}
#[cfg(test)]mod tests {
    use super::*;
    fn read(d:&mut Endpoint,a:u16,n:usize)->Vec<u8>{d.start(false);for b in a.to_be_bytes(){d.write(b);}d.start(true);(0..n).map(|_|d.read()).collect()}
    #[test]fn nominal_words_frame_snapshot_rate_clear_and_readonly_calibration(){let s=Arc::new(Mutex::new(Mlx90640::new()));let mut d=Endpoint::new(s.clone());assert_eq!(read(&mut d,0x8000,2),[0,0x30]);s.lock().unwrap().advance_to(1);assert_eq!(read(&mut d,0x8000,2),[0,0x39]);assert_eq!(read(&mut d,0x400,2),8000u16.to_be_bytes());assert!(s.lock().unwrap().set_scene([7000,10000,2,2,1]));assert_eq!(read(&mut d,0x400,2),8000u16.to_be_bytes());d.start(false);for b in[0x80,0,0,0x30]{d.write(b);}assert_eq!(read(&mut d,0x8000,2),[0,0x31]);s.lock().unwrap().advance_to(1+HZ/2);assert_eq!(read(&mut d,0x400,4),[0x27,0x10,0x1b,0x58]);assert_eq!(read(&mut d,0x2407,4),[0x53,0x49,0x4d,0]);d.start(false);for b in[0x24,7,0,0]{d.write(b);}assert_eq!(read(&mut d,0x2407,4),[0x53,0x49,0x4d,0]);assert!(!s.lock().unwrap().set_scene([0,1,0,0,0]));}
}
