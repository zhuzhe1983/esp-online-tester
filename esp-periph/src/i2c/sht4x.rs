//! SHT4x digital measurement subset. Sensirion datasheet v7.3 (June2026),
//! tables5/7/8 and section4.6: conversion time, CRC8 and sample scaling.
//! Heater/analog/noise effects are not modeled; the caller supplies a synthetic environment.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Sht4xState {pub temperature_c:f64,pub humidity_rh:f64,pub serial:u32,pub cycle:u64,pub powered:bool}
impl Default for Sht4xState {fn default()->Self {Self{temperature_c:25.0,humidity_rh:50.0,serial:1,cycle:0,powered:true}}}
pub struct Sht4x {delays:[u64;4],abort_reset:bool,state:Arc<Mutex<Sht4xState>>,cycles_per_us:u64,ready_at:u64,pending:bool,writing:bool,read_at:usize,reply:[u8;6]}
impl Sht4x {
    pub fn new(state:Arc<Mutex<Sht4xState>>,cycles_per_us:u64)->Self {Self{delays:[8300,4500,1600,1000],abort_reset:false,state,cycles_per_us:cycles_per_us.max(1),ready_at:0,pending:false,writing:false,read_at:6,reply:[0xff;6]}}
    /// ENV-IV's pinned SHT40 D1 v2 (2021-07) table4: 8.2/4.5/1.7ms.
    /// Default v7.3 users retain their existing timings and command behavior.
    pub fn with_sht40_d1_v2(mut self)->Self {self.delays=[8200,4500,1700,0];self.abort_reset=true;self}
    /// Physical power edge: discard a pending sample, wait the specified 1ms startup.
    pub fn reset_at(&mut self,cycle:u64) {self.pending=false;self.writing=false;self.read_at=6;self.reply=[0xff;6];self.ready_at=cycle.saturating_add(self.cycles_per_us.saturating_mul(1000));}
    fn crc(bytes:[u8;2])->u8 {let mut crc=0xffu8;for b in bytes {crc^=b;for _ in 0..8 {crc=if crc&0x80!=0 {(crc<<1)^0x31}else{crc<<1};}}crc}
    fn words(&mut self,words:[u16;2]) {for (i,word) in words.into_iter().enumerate(){let b=word.to_be_bytes();self.reply[i*3..i*3+3].copy_from_slice(&[b[0],b[1],Self::crc(b)]);}}
}
impl I2cDevice for Sht4x {
    fn start(&mut self,read:bool)->bool {
        let s=self.state.lock().expect("SHT4x state poisoned");if !s.powered {self.pending=false;return false;}
        self.writing= !read;self.read_at=6;
        if read {if !self.pending || s.cycle<self.ready_at {return false;}self.pending=false;self.read_at=0;}
        true
    }
    fn write(&mut self,command:u8)->bool {
        if !self.writing {return false;}self.writing=false;
        let (cycle,powered,t,h,serial)={let s=self.state.lock().expect("SHT4x state poisoned");(s.cycle,s.powered,s.temperature_c,s.humidity_rh,s.serial)};
        if !powered || (cycle<self.ready_at && !(self.abort_reset && command==0x94)) {return false;}
        let delay=match command {0xfd=>self.delays[0],0xf6=>self.delays[1],0xe0=>self.delays[2],0x89=>self.delays[3],0x94=>1000,_=>return false};
        self.ready_at=cycle.saturating_add(self.cycles_per_us.saturating_mul(delay));self.pending=command!=0x94;
        match command {
            0xfd|0xf6|0xe0=>{if !t.is_finite() || !h.is_finite(){self.pending=false;return false;}self.words([((t.clamp(-40.0,125.0)+45.0)*65535.0/175.0).round() as u16,((h.clamp(0.0,100.0)+6.0)*65535.0/125.0).round() as u16]);},
            0x89=>self.words([(serial>>16) as u16,serial as u16]),
            _=>{}
        }true
    }
    fn read(&mut self)->u8 {let b=self.reply.get(self.read_at).copied().unwrap_or(0xff);self.read_at=self.read_at.saturating_add(1);b}
    fn stop(&mut self) {self.writing=false;self.read_at=6;}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn env4_profile_abort_and_power_reset_do_not_change_default_profile() {
        for (profile,delay) in [(false,8300),(true,8200)] {
            let state=Arc::new(Mutex::new(Sht4xState::default()));let mut d=Sht4x::new(state.clone(),1);
            if profile {d=d.with_sht40_d1_v2();}
            assert!(d.start(false));assert!(d.write(0xfd));
            state.lock().unwrap().cycle=delay-1;assert!(!d.start(true));
            state.lock().unwrap().cycle=delay;assert!(d.start(true));assert_eq!(d.read(),0x66);
            assert!(d.start(false));assert!(d.write(0xfd));
            state.lock().unwrap().cycle=delay+1;assert!(d.start(false));assert_eq!(d.write(0x94),profile);
            d.reset_at(delay+1);state.lock().unwrap().cycle=delay+1000;assert!(d.start(false));assert!(!d.write(0xfd));
            state.lock().unwrap().cycle=delay+1001;assert!(!d.start(true));assert!(d.start(false));assert!(d.write(0xfd));
        }
    }
    #[test]
    fn timed_sample_crc_one_shot_read_and_reset() {
        assert_eq!(Sht4x::crc([0xbe,0xef]),0x92);
        let s=Arc::new(Mutex::new(Sht4xState::default()));let mut d=Sht4x::new(s.clone(),1);
        assert!(!d.start(true));assert!(d.start(false));assert!(d.write(0xfd));d.stop();assert!(!d.start(true));
        s.lock().unwrap().cycle=8299;assert!(!d.start(true));s.lock().unwrap().cycle=8300;assert!(d.start(true));
        let bytes:Vec<_>=(0..6).map(|_|d.read()).collect();assert_eq!(&bytes[..2],&[0x66,0x66]);assert_eq!(bytes[2],Sht4x::crc([0x66,0x66]));assert_eq!(bytes[5],Sht4x::crc([bytes[3],bytes[4]]));assert_eq!(d.read(),0xff);assert!(!d.start(true));
        assert!(d.start(false));assert!(d.write(0x94));s.lock().unwrap().cycle=9300;assert!(!d.start(true));assert!(d.start(false));assert!(!d.write(0x39),"heater unsupported");
        s.lock().unwrap().powered=false;assert!(!d.start(false));
    }
}
