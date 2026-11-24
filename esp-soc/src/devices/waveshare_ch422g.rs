//! CH422G static I/O subset, not the CH32V00x MCU expander protocol.
//! Source: WCH CH422DS1_EN §5.4–6.4, retained under
//! docs/platform/research/waveshare-s3-tail-sources/ch422g/.
//! Commands are I2C addresses and carry exactly one byte. Datasheet mode
//! bits are IO_OE=0, A_SCAN=2, OD_EN=4, SLEEP=7. Some Waveshare driver
//! headers instead define OD_EN=2: that conflicts with the primary datasheet.
//! Dynamic LED scanning and sleep are rejected, not silently emulated as GPIO.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;

pub const ADDRESSES:[u8;7]=[0x24,0x23,0x38,0x39,0x3a,0x3b,0x26];
pub struct Ch422gState{mode:u8,out:u8,oc:u8,inputs:[Option<bool>;8],pub falls:[u64;8],pub writes:u64}
impl Default for Ch422gState{fn default()->Self{Self::new()}}
impl Ch422gState{
 pub fn new()->Self{Self{mode:0,out:255,oc:15,inputs:[None;8],falls:[0;8],writes:0}}
 /// Actual push-pull drive; None denotes input/high impedance, not logical high.
 pub fn level(&self,p:u8)->Option<bool>{(p<8&&self.mode&1!=0).then(||self.out&(1<<p)!=0)}
 /// Input sampling includes the documented weak pull-up on undriven IO0..7.
 pub fn sample(&self,p:u8)->bool{if p>=8{return true;}self.level(p).or(self.inputs[p as usize]).unwrap_or(true)}
 pub fn oc_level(&self,p:u8)->Option<bool>{if p>=4{return None;}let high=self.oc&(1<<p)!=0;if high&&self.mode&0x10!=0{None}else{Some(high)}}
 pub fn set_input(&mut self,p:u8,level:Option<bool>){if p<8{self.inputs[p as usize]=level;}}
 pub fn set(&mut self,address:u8,value:u8)->bool{
  if(address==0x24&&value&!0x11!=0)||(address==0x23&&value&!15!=0){return false;}
  let before=std::array::from_fn::<_,8,_>(|i|self.sample(i as u8));
  match address{0x24=>self.mode=value,0x23=>self.oc=value,0x38..=0x3b=>self.out=value,_=>return false}
  for(i,old)in before.into_iter().enumerate(){if old&&!self.sample(i as u8){self.falls[i]=self.falls[i].wrapping_add(1);}}
  self.writes=self.writes.wrapping_add(1);true
 }
}
pub struct Ch422gPort{state:Arc<Mutex<Ch422gState>>,address:u8,used:bool}
impl Ch422gPort{pub fn new(state:Arc<Mutex<Ch422gState>>,address:u8)->Self{Self{state,address,used:false}}}
impl I2cDevice for Ch422gPort{
 fn start(&mut self,read:bool)->bool{self.used=false;ADDRESSES.contains(&self.address)&&read==(self.address==0x26)}
 fn write(&mut self,v:u8)->bool{if self.used||self.address==0x26{return false;}self.used=true;self.state.lock().expect("CH422G").set(self.address,v)}
 fn read(&mut self)->u8{if self.used||self.address!=0x26{return 255;}self.used=true;let s=self.state.lock().expect("CH422G");(0..8).fold(0,|v,i|v|((s.sample(i)as u8)<<i))}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn datasheet_reset_direction_open_drain_and_actual_input(){let mut s=Ch422gState::new();assert_eq!(s.level(0),None);assert!(s.sample(0));s.set_input(0,Some(false));assert!(!s.sample(0));assert_eq!(s.oc_level(0),Some(true));assert!(s.set(0x24,0x11));assert_eq!(s.level(0),Some(true));assert_eq!(s.oc_level(0),None);assert!(s.set(0x23,14));assert_eq!(s.oc_level(0),Some(false));assert!(s.set(0x38,254));assert_eq!(s.level(0),Some(false));assert_eq!(s.falls[0],1);assert!(!s.set(0x24,5),"datasheet A_SCAN is not vendor-header OD_EN");assert!(!s.set(0x24,0x80));assert_eq!(s.level(0),Some(false));}
 #[test]fn address_commands_single_byte_aliases_and_direction_nacks(){let s=Arc::new(Mutex::new(Ch422gState::new()));for at in[0x38,0x39,0x3a,0x3b]{let mut p=Ch422gPort::new(s.clone(),at);assert!(!p.start(true));assert!(p.start(false));assert!(p.write(0xa5));assert!(!p.write(0xff));}let mut mode=Ch422gPort::new(s.clone(),0x24);assert!(mode.start(false));assert!(mode.write(1));let mut read=Ch422gPort::new(s,0x26);assert!(!read.start(false));assert!(read.start(true));assert_eq!(read.read(),0xa5);assert_eq!(read.read(),255);assert!(!read.write(0));}
}
