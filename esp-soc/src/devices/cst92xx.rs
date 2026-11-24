//! CST9217/CST9220 source-defined command/report subset (I2C7-bit5A).
//! Derived protocol: manufacturer-vendored MIT SensorLib TouchDrvCST92xx.cpp
//! and CST9xxConstants.h, retained with URL/revision/SHA in S3 source manifest.
//! Known chip-family constants are source facts. Project ID, firmware version,
//! checksum, per-unit calibration, physical IRQ cadence and updater are unknown.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};
#[derive(Clone,Copy,Debug)]pub enum Cst92Kind{Cst9217,Cst9220}
impl Cst92Kind{fn chip_id(self)->u16{match self{Self::Cst9217=>0x9217,Self::Cst9220=>0x9220}}}
pub struct Cst92State{pub kind:Cst92Kind,pub width:u16,pub height:u16,powered:bool,mode:u8,down:bool,x:u16,y:u16,pending:bool,event:u64,epoch:u64}
impl Cst92State{
 pub fn new(kind:Cst92Kind,width:u16,height:u16)->Self{Self{kind,width,height,powered:false,mode:9,down:false,x:0,y:0,pending:false,event:0,epoch:0}}
 pub fn hardware(&mut self,powered:bool,reset_high:bool){let on=powered&&reset_high;if self.powered&&!on{self.mode=9;self.down=false;self.pending=false;self.epoch=self.epoch.wrapping_add(1);self.event=self.event.wrapping_add(1);}self.powered=on;}
 pub fn update(&mut self,x:u16,y:u16,down:bool){if self.powered&&self.mode!=5{self.x=x.min(self.width.saturating_sub(1)).min(4095);self.y=y.min(self.height.saturating_sub(1)).min(4095);self.down=down;self.pending=true;self.event=self.event.wrapping_add(1);}}
 pub fn irq_high(&self)->bool{!self.powered||self.mode==5||!self.pending}
}
pub struct Cst92xx{state:Arc<Mutex<Cst92State>>,selector:u16,select_bytes:u8,index:usize,report:[u8;15],report_event:Option<u64>,epoch:u64}
impl Cst92xx{
 pub fn new(state:Arc<Mutex<Cst92State>>)->Self{Self{state,selector:0xffff,select_bytes:0,index:0,report:[0;15],report_event:None,epoch:0}}
 fn available(&mut self)->bool{let s=self.state.lock().expect("CST92xx");if s.epoch!=self.epoch{self.epoch=s.epoch;self.selector=0xffff;self.select_bytes=0;self.report_event=None;}s.powered&&s.mode!=5}
 fn supported_selector(&self)->bool{matches!(self.selector,0x0002|0xd000|0xd1f8|0xd1fc|0xd204|0xd208)}
}
impl I2cDevice for Cst92xx{
 fn start(&mut self,read:bool)->bool{if !self.available(){return false;}
  if !read{self.select_bytes=0;self.selector=0;self.index=0;return true;}
  if self.select_bytes!=2||!self.supported_selector(){return false;}self.index=0;
  if self.selector==0xd000{let s=self.state.lock().expect("CST92xx");self.report=[0;15];self.report[0]=if s.down{6}else{0};self.report[1]=(s.x>>4)as u8;self.report[2]=(s.y>>4)as u8;self.report[3]=((s.x&15)<<4)as u8|(s.y&15)as u8;self.report[5]=u8::from(s.down);self.report[6]=0xab;self.report_event=Some(s.event);}true}
 fn write(&mut self,b:u8)->bool{if !self.available(){return false;}
  if self.select_bytes<2{self.selector=(self.selector<<8)|u16::from(b);self.select_bytes+=1;if self.select_bytes==2&&self.selector>>8==0xd1{let mode=(self.selector&255)as u8;if matches!(mode,1|5|9|0x1e){let mut s=self.state.lock().expect("CST92xx");s.mode=mode;if mode==5{s.pending=false;s.down=false;}return true;}}return true;}
  if self.selector==0xd000&&b==0xab{let mut s=self.state.lock().expect("CST92xx");if self.report_event==Some(s.event){s.pending=false;}self.report_event=None;return true;}false}
 fn read(&mut self)->u8{if !self.available(){return 255;}let s=self.state.lock().expect("CST92xx");let n=self.index;self.index=self.index.saturating_add(1);
  match self.selector{
   0xd000=>self.report.get(n).copied().unwrap_or(255),
   0x0002=>[0,s.mode,0,0].get(n).copied().unwrap_or(255),
   0xd1f8=>[s.width as u8,(s.width>>8)as u8,s.height as u8,(s.height>>8)as u8].get(n).copied().unwrap_or(255),
   0xd1fc=>[0,0,0xca,0xca].get(n).copied().unwrap_or(255),
   0xd204=>{let id=s.kind.chip_id();[0,0,id as u8,(id>>8)as u8].get(n).copied().unwrap_or(255)},
   0xd208 if n<8=>0,
   _=>255}}
}
#[cfg(test)]mod tests{use super::*;
 fn select(d:&mut Cst92xx,p:u16){assert!(d.start(false));for b in p.to_be_bytes(){assert!(d.write(b));}d.stop();}
 fn read(d:&mut Cst92xx,p:u16,n:usize)->Vec<u8>{select(d,p);assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
 #[test]fn vendor_decoder_observes_press_release_and_ack_does_not_consume_new_contact(){let s=Arc::new(Mutex::new(Cst92State::new(Cst92Kind::Cst9220,480,480)));let mut d=Cst92xx::new(s.clone());assert!(!d.start(false));s.lock().unwrap().hardware(true,true);s.lock().unwrap().update(471,389,true);let b=read(&mut d,0xd000,15);assert_eq!(b[0]&15,6);assert_eq!(b[5]&127,1);assert_eq!(b[6],0xab);assert_eq!(((b[1]as u16)<<4)|((b[3]as u16)>>4),471);assert_eq!(((b[2]as u16)<<4)|((b[3]as u16)&15),389);s.lock().unwrap().update(200,300,false);select(&mut d,0xd000);assert!(d.write(0xab));assert!(!s.lock().unwrap().irq_high());let b=read(&mut d,0xd000,15);assert_eq!((b[0],b[5]),(0,0));select(&mut d,0xd000);assert!(d.write(0xab));assert!(s.lock().unwrap().irq_high());}
 #[test]fn modes_family_constants_unknown_identity_and_reset(){for(kind,id)in[(Cst92Kind::Cst9217,0x9217u16),(Cst92Kind::Cst9220,0x9220)]{let s=Arc::new(Mutex::new(Cst92State::new(kind,466,466)));s.lock().unwrap().hardware(true,true);let mut d=Cst92xx::new(s.clone());select(&mut d,0xd101);assert_eq!(read(&mut d,0x0002,2),[0,1]);assert_eq!(read(&mut d,0xd204,4),[0,0,id as u8,(id>>8)as u8]);assert_eq!(read(&mut d,0xd208,8),[0;8]);select(&mut d,0xa001);assert!(!d.start(true));select(&mut d,0xd105);assert!(!d.start(true));s.lock().unwrap().hardware(true,false);s.lock().unwrap().hardware(true,true);assert!(!d.start(true));assert_eq!(read(&mut d,0xd1f8,4),[0xd2,1,0xd2,1]);}}
}
