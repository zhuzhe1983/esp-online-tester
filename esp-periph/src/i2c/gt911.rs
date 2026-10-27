//! Goodix GT911 single-contact scanning subset. GT911 datasheet Rev09 and
//! M5GFX c6f92dc Touch_GT911 use 0x814e ready/count, 0x8150 coordinates, clear by writing 0.
//! This is separate from the older S3 Waveshare always-ready fixture endpoint.
use super::I2cDevice;
use std::sync::{Arc,Mutex};
pub struct Gt911State {pub cycle:u64,pub powered:bool,pub ready:bool,pub x:u16,pub y:u16,pub down:bool,report:(u16,u16,bool),next_scan:u64,period:u64,width:u16,height:u16}
impl Gt911State {
    pub fn new(width:u16,height:u16,period:u64)->Self {Self{cycle:0,powered:true,ready:false,x:0,y:0,down:false,report:(0,0,false),next_scan:0,period:period.max(1),width:width.clamp(1,4096),height:height.clamp(1,4096)}}
    pub fn update(&mut self,x:u16,y:u16,down:bool) {self.x=x.min(self.width-1);self.y=y.min(self.height-1);self.down=down;}
    // Continue zero-contact scans after release: the driver may discard one stale
    // report before waiting for a fresh scan. Fixed period is digital, not idle-power timing.
    pub fn advance_to(&mut self,cycle:u64) {self.cycle=cycle;if self.powered && !self.ready && cycle>=self.next_scan {self.report=(self.x,self.y,self.down);self.ready=true;}}
    fn ack(&mut self) {self.ready=false;self.next_scan=self.cycle.saturating_add(self.period);}
}
// Configuration version/trigger bytes are unknown (zero). Writes are bounded RAM;
// they do not implement firmware loading, trigger modes or resolution reconfiguration.
pub struct Gt911 {state:Arc<Mutex<Gt911State>>,address:u16,phase:u8,config:[u8;186]}
impl Gt911 {
    pub fn new(state:Arc<Mutex<Gt911State>>)->Self {let mut config=[0;186];{let s=state.lock().expect("GT911 state poisoned");config[1..3].copy_from_slice(&s.width.to_le_bytes());config[3..5].copy_from_slice(&s.height.to_le_bytes());}config[5]=2;Self{state,address:0,phase:0,config}}
}
impl I2cDevice for Gt911 {
    fn start(&mut self,read:bool)->bool {if !read{self.phase=0;}self.state.lock().expect("GT911 state poisoned").powered}
    fn write(&mut self,b:u8)->bool {
        match self.phase {0=>{self.address=(b as u16)<<8;self.phase=1;},1=>{self.address|=b as u16;self.phase=2;},_=>{
            if self.address==0x814e && b==0 {self.state.lock().expect("GT911 state poisoned").ack();}
            if let Some(i)=self.address.checked_sub(0x8047).filter(|i|(*i as usize)<self.config.len()) {self.config[i as usize]=b;}
            self.address=self.address.wrapping_add(1);
        }}true
    }
    fn read(&mut self)->u8 {
        let a=self.address;self.address=self.address.wrapping_add(1);let s=self.state.lock().expect("GT911 state poisoned");let (x,y,down)=s.report;
        match a {0x8140=>b'9',0x8141|0x8142=>b'1',0x814e=>if s.ready {0x80|u8::from(down)}else{0},0x8150=>x as u8,0x8151=>(x>>8) as u8,0x8152=>y as u8,0x8153=>(y>>8) as u8,0x8154=>20,0x8047..=0x8100=>self.config[(a-0x8047) as usize],_=>0}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn read(d:&mut Gt911,a:u16,n:usize)->Vec<u8> {assert!(d.start(false));for b in a.to_be_bytes(){assert!(d.write(b));}assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    #[test]
    fn ready_clear_rescan_release_and_bounded_coordinates() {
        let state=Arc::new(Mutex::new(Gt911State::new(540,960,100)));let mut d=Gt911::new(state.clone());assert_eq!(read(&mut d,0x8140,4),b"911\0");assert_eq!(read(&mut d,0x814e,1),[0]);
        {let mut s=state.lock().unwrap();s.update(999,2000,true);s.advance_to(1);}
        assert_eq!(read(&mut d,0x814e,6),[0x81,0,0x1b,2,0xbf,3]);assert!(d.start(false));for b in [0x81,0x4e,0]{d.write(b);}assert_eq!(read(&mut d,0x814e,1),[0]);
        state.lock().unwrap().advance_to(100);assert_eq!(read(&mut d,0x814e,1),[0]);state.lock().unwrap().advance_to(101);assert_eq!(read(&mut d,0x814e,1),[0x81]);
        {let mut s=state.lock().unwrap();s.update(20,30,false);s.ack();s.advance_to(201);}
        assert_eq!(read(&mut d,0x814e,6),[0x80,0,20,0,30,0]);state.lock().unwrap().powered=false;assert!(!d.start(true));
    }
}
