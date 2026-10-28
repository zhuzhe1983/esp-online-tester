//! T-Beam V1.2 AXP2101 digital register subset; not an analog battery simulation.
//! Register/IRQ definitions: LilyGo-LoRa-Series 1991b1ed, bundled XPowersLib
//! REG/AXP2101Constants.h, XPowersParams.hpp and XPowersAXP2101.tpp.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use crate::periph::CPU_HZ;
pub struct Axp2101 {pub regs:[u8;256],pub power_off_requested:bool,pressed:Option<u64>,long_sent:bool}
impl Axp2101 {
    pub fn new()->Self {
        let mut regs=[0;256];
        // Explicit virtual USB supply, no battery fitted. DCDC1 is board's 3.3 V
        // startup supply; other channels only become active through guest writes.
        for (a,v) in [(0,0x20),(1,0x10),(3,0x4a),(0x20,0x20),(0x27,0x14),(0x80,1),(0x82,18)] {regs[a]=v;}
        Self{regs,power_off_requested:false,pressed:None,long_sent:false}
    }
    pub fn irq(&self)->bool {(0..3).any(|i|self.regs[0x40+i]&self.regs[0x48+i]!=0)}
    fn long_ticks(&self)->u64 {CPU_HZ*(2+u64::from((self.regs[0x27]>>4)&3))/2}
    pub fn next_deadline(&self)->Option<u64> {self.pressed.filter(|_|!self.long_sent).map(|t|t.saturating_add(self.long_ticks()))}
    pub fn advance_to(&mut self,cycle:u64) {if self.next_deadline().is_some_and(|t|t<=cycle) {self.regs[0x49]|=4;self.long_sent=true;}}
    pub fn button(&mut self,cycle:u64,pressed:bool) {
        self.advance_to(cycle);
        if pressed && self.pressed.is_none() {self.pressed=Some(cycle);self.long_sent=false;self.regs[0x49]|=2;}
        if !pressed {if let Some(start)=self.pressed.take() {self.regs[0x49]|=1;if !self.long_sent && cycle.saturating_sub(start)>=CPU_HZ/100 {self.regs[0x49]|=8;}}}
    }
    fn read(&self,address:u8)->u8 {
        match address {
            // ADC is a fixed virtual profile and respects channel enable bits.
            0x38|0x39 if self.regs[0x30]&4!=0 => (5000u16 >> if address==0x38 {8}else{0}) as u8,
            0x3a|0x3b if self.regs[0x30]&8!=0 => (3300u16 >> if address==0x3a {8}else{0}) as u8,
            _=>self.regs[usize::from(address)],
        }
    }
    fn write(&mut self,address:u8,value:u8) {
        match address {
            0x48..=0x4a=>self.regs[usize::from(address)]&=!value,
            0x10=>{self.regs[0x10]=value;self.power_off_requested|=value&1!=0;}
            0x04..=0x07|0x12..=0x1a|0x22..=0x30|0x40..=0x42|0x50..=0x6a|0x80..=0x9a=>self.regs[usize::from(address)]=value,
            _=>{},
        }
    }
}
pub struct AxpDevice {pub state:Arc<Mutex<Axp2101>>,pointer:u8,address:bool}
impl AxpDevice {pub fn new(state:Arc<Mutex<Axp2101>>)->Self {Self{state,pointer:0,address:true}}}
impl I2cDevice for AxpDevice {
    fn pins(&self)->Option<(u8,u8)> {Some((21,22))}
    fn start(&mut self,read:bool)->bool {if !read {self.address=true;}true}
    fn write(&mut self,value:u8)->bool {if self.address {self.pointer=value;self.address=false;}else {self.state.lock().unwrap().write(self.pointer,value);self.pointer=self.pointer.wrapping_add(1);}true}
    fn read(&mut self)->u8 {let value=self.state.lock().unwrap().read(self.pointer);self.pointer=self.pointer.wrapping_add(1);value}
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn id_is_read_only_and_adc_requires_enable() {let mut p=Axp2101::new();p.write(3,0);assert_eq!(p.read(3),0x4a);assert_eq!(p.read(0x38),0);p.write(0x30,12);assert_eq!(u16::from_be_bytes([p.read(0x38),p.read(0x39)]),5000);assert_eq!(p.read(0)&8,0);}
    #[test] fn key_irq_is_masked_and_write_one_to_clear() {let mut p=Axp2101::new();p.button(0,true);p.button(CPU_HZ/5,false);assert!(!p.irq());p.write(0x41,8);assert!(p.irq());p.write(0x49,8);assert!(!p.irq());assert_eq!(p.read(0x49),3);}
    #[test] fn hold_sets_long_once_and_never_short_on_release() {let mut p=Axp2101::new();p.write(0x41,12);p.button(0,true);let d=p.next_deadline().unwrap();p.advance_to(d-1);assert!(!p.irq());p.advance_to(d);assert!(p.irq());p.write(0x49,4);p.advance_to(d+CPU_HZ);assert!(!p.irq());p.button(d+CPU_HZ,false);assert_eq!(p.read(0x49)&12,0);}
}
