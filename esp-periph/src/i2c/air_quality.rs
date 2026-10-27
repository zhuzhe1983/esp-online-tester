//! SCD40/SEN55 digital subset. Sensirion SCD4x datasheet v1.2 (May 2022),
//! SEN5x v2-D1 (March 2022), sections 3 / 6.1. Virtual samples, not air physics.
//! SCD40 STOP uses a 450 ms digital completion point within the specified 500 ms
//! maximum; exact analog completion and warm-up latency are not calibrated.
use super::I2cDevice;
use std::sync::{Arc,Mutex};
#[derive(Clone,Copy,PartialEq,Eq)] pub enum Kind {Scd40,Sen55}
pub struct AirSensorState {pub powered:bool,pub cycle:u64,pub kind:Kind,pub values:Vec<u16>,period:Option<u64>,next:u64,busy_until:u64,has_sample:bool,temp_offset:[u16;3],pub commands:u64,pub unsupported:u64}
impl AirSensorState {
 pub fn new(kind:Kind)->Self {Self{powered:true,cycle:0,kind,values:if kind==Kind::Scd40{vec![420,26214,32768]}else{vec![50,70,90,100,5000,5000,1000,10]},period:None,next:0,busy_until:0,has_sample:false,temp_offset:[0;3],commands:0,unsupported:0}}
 pub fn power(&mut self,on:bool){if self.powered!=on{self.period=None;self.next=0;self.has_sample=false;self.busy_until=if on{self.cycle.saturating_add(240_000_000)}else{0};self.powered=on;}}
 pub fn ready(&self)->bool {self.powered&&self.period.is_some()&&self.cycle>=self.next}
 fn duration(&self,ms:u64)->u64 {self.cycle.saturating_add(ms.saturating_mul(240_000))}
 fn reset(&mut self){self.period=None;self.next=0;self.has_sample=false;self.temp_offset=[0;3];self.busy_until=self.duration(if self.kind==Kind::Scd40{20}else{100});}
}
pub fn sensirion_crc(bytes:&[u8])->u8 {let mut c=255u8;for b in bytes{c^=*b;for _ in 0..8{c=if c&128!=0{(c<<1)^0x31}else{c<<1};}}c}
fn words(words:&[u16])->Vec<u8>{words.iter().flat_map(|w|{let b=w.to_be_bytes();[b[0],b[1],sensirion_crc(&b)]}).collect()}
pub struct AirSensor {state:Arc<Mutex<AirSensorState>>,tx:Vec<u8>,rx:Vec<u8>,at:usize,command:u16,valid:bool,consume:bool}
impl AirSensor {
 pub fn new(state:Arc<Mutex<AirSensorState>>)->Self {Self{state,tx:Vec::new(),rx:Vec::new(),at:0,command:0,valid:false,consume:false}}
 fn length(kind:Kind,c:u16)->Option<usize>{match(kind,c){(Kind::Scd40,0x21b1|0x21ac|0x3f86|0xe4b8|0xec05|0x2318|0x3682|0x3646)=>Some(2),(Kind::Scd40,0x241d)=>Some(5),(Kind::Sen55,0x0021|0x0104|0x0202|0x03c4|0xd304|0xd014|0xd033|0xd100|0xd206|0xd210)=>Some(2),(Kind::Sen55,0x60b2)=>Some(11),_=>None}}
 fn execute(&mut self){let mut s=self.state.lock().expect("Air sensor mutex poisoned");if self.tx.len()<2{return;}let c=u16::from_be_bytes([self.tx[0],self.tx[1]]);if !self.valid || Self::length(s.kind,c)!=Some(self.tx.len()){self.valid=false;self.tx.clear();return;}s.commands+=1;self.command=c;self.consume=false;match(s.kind,c){(Kind::Scd40,0x21b1|0x21ac)=>{let ms=if c==0x21b1{5000}else{30000};s.period=Some(ms*240_000);s.next=s.duration(ms);},(Kind::Sen55,0x0021)=>{s.period=Some(240_000_000);s.next=s.duration(1000);s.busy_until=s.duration(50);},(Kind::Scd40,0x3f86)|(Kind::Sen55,0x0104)=>{s.period=None;s.busy_until=s.duration(if s.kind==Kind::Scd40{450}else{200});},(Kind::Scd40,0x3646)|(Kind::Sen55,0xd304)=>s.reset(),(Kind::Scd40,0x241d)=>s.temp_offset[0]=u16::from_be_bytes([self.tx[2],self.tx[3]]),(Kind::Sen55,0x60b2)=>{for i in 0..3{s.temp_offset[i]=u16::from_be_bytes([self.tx[2+3*i],self.tx[3+3*i]]);}},_=>{}}self.tx.clear();}
}
impl I2cDevice for AirSensor {
 fn start(&mut self,read:bool)->bool {if !read{self.tx.clear();self.rx.clear();self.at=0;self.valid=true;self.consume=false;}let s=self.state.lock().expect("Air sensor mutex poisoned");if !s.powered||s.cycle<s.busy_until{return false;}
  if !read{return true;}
  if !self.valid{return false;}let c=self.command;self.at=0;self.consume=false;self.rx=match(s.kind,c){(Kind::Scd40,0xe4b8)|(Kind::Sen55,0x0202)=>words(&[u16::from(s.ready())]),(Kind::Scd40,0xec05)=>{if !s.ready(){return false;}self.consume=true;words(&s.values[..3])},(Kind::Sen55,0x03c4)=>{self.consume=true;if s.ready()||s.has_sample{words(&s.values[..8])}else{words(&[65535,65535,65535,65535,32767,32767,32767,32767])}},(Kind::Scd40,0x2318)=>words(&s.temp_offset[..1]),(Kind::Scd40,0x3682)=>words(&[0,0,0]),(Kind::Sen55,0xd014)=>words(&[0x5345,0x4e35,0x3500,0,0,0,0,0,0,0,0,0,0,0,0,0]),(Kind::Sen55,0xd033)=>words(&[0;16]),(Kind::Sen55,0xd100)=>words(&[0x0100]),(Kind::Sen55,0xd206)=>words(&[0,0]),_=>return false};true}
 fn write(&mut self,b:u8)->bool {if self.tx.len()>=11{return false;}self.tx.push(b);if self.tx.len()==2{let s=self.state.lock().expect("Air sensor mutex poisoned");let c=u16::from_be_bytes([self.tx[0],self.tx[1]]);self.valid=Self::length(s.kind,c).is_some() && (s.kind!=Kind::Scd40 || s.period.is_none() || matches!(c,0x3f86|0xe4b8|0xec05));drop(s);if !self.valid{self.state.lock().expect("Air sensor mutex poisoned").unsupported+=1;}}
  if self.tx.len()>2&&(self.tx.len()-2).is_multiple_of(3){let n=self.tx.len();self.valid&=sensirion_crc(&self.tx[n-3..n-1])==b;}self.valid}
 fn read(&mut self)->u8 {let v=self.rx.get(self.at).copied().unwrap_or(255);self.at=self.at.saturating_add(1);if self.consume&&self.at==self.rx.len(){let mut s=self.state.lock().expect("Air sensor mutex poisoned");if let Some(period)=s.period{if s.ready(){s.has_sample=true;let elapsed=s.cycle.saturating_sub(s.next);s.next=s.next.saturating_add((elapsed/period+1).saturating_mul(period));}}self.consume=false;}v}
 fn stop(&mut self){self.execute();}
}
#[cfg(test)] mod tests {
 use super::*;
 fn cmd(d:&mut AirSensor,c:u16){assert!(d.start(false));for b in c.to_be_bytes(){assert!(d.write(b));}d.stop();}
 #[test] fn crc_periodic_readiness_complete_read_and_stop(){assert_eq!(sensirion_crc(&[0xbe,0xef]),0x92);let s=Arc::new(Mutex::new(AirSensorState::new(Kind::Scd40)));let mut d=AirSensor::new(s.clone());cmd(&mut d,0x21b1);cmd(&mut d,0xe4b8);assert!(d.start(true));assert_eq!((0..3).map(|_|d.read()).collect::<Vec<_>>(),words(&[0]));s.lock().unwrap().cycle=5*240_000_000;cmd(&mut d,0xec05);assert!(d.start(true));assert_eq!(d.read(),1);assert!(s.lock().unwrap().ready());for _ in 1..9{d.read();}assert!(!s.lock().unwrap().ready());s.lock().unwrap().cycle=13*240_000_000;cmd(&mut d,0xec05);assert!(d.start(true));for _ in 0..9{d.read();}assert_eq!(s.lock().unwrap().next,15*240_000_000,"Reading late does not restart the free-running measurement interval");cmd(&mut d,0x3f86);assert!(!d.start(false));s.lock().unwrap().cycle+=120_000_000;assert!(d.start(false));assert!(d.write(0x24));assert!(d.write(0x1d));assert!(d.write(0));assert!(d.write(1));assert!(!d.write(0));d.stop();assert_eq!(s.lock().unwrap().temp_offset[0],0);}
 #[test] fn sensor_power_unsupported_and_values_keep_crc(){let s=Arc::new(Mutex::new(AirSensorState::new(Kind::Sen55)));let mut d=AirSensor::new(s.clone());cmd(&mut d,0x0021);s.lock().unwrap().cycle=240_000_000;cmd(&mut d,0x03c4);assert!(d.start(true));let bytes=(0..24).map(|_|d.read()).collect::<Vec<_>>();for b in bytes.as_chunks::<3>().0{assert_eq!(sensirion_crc(&b[..2]),b[2]);}assert_eq!(u16::from_be_bytes([bytes[0],bytes[1]]),50);s.lock().unwrap().power(false);assert!(!d.start(false));s.lock().unwrap().power(true);assert!(!d.start(false));s.lock().unwrap().cycle+=240_000_000;assert!(d.start(false));assert!(d.write(0xff));assert!(!d.write(0xff));d.stop();assert!(!d.start(true));assert_eq!(s.lock().unwrap().unsupported,1);}
}
