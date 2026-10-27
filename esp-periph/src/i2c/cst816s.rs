//! CST816S single-contact digital report used by esp-bsp daf03bb73 (component
//! 1.0.6), esp_lcd_touch_cst816s.c: 02h count, 03h..06h 12-bit X/Y, A7h ID.
//! Sleep A5h=03h and reset follow CloudZao INK4 d12f481f metalio_touch.c.
//! Chip/firmware IDs stay unknown (zero). No gesture synthesis, scan cadence,
//! automatic standby or measured IRQ pulse timing is claimed by this subset.
use super::{I2cDevice,Reg8Device};
use std::sync::{Arc,Mutex};

#[derive(Default)]
pub struct Cst816State {pub powered:bool,pub down:bool,pub x:u16,pub y:u16,pending:bool,asleep:bool}
impl Cst816State {
    pub fn update(&mut self,x:u16,y:u16,down:bool) {if self.powered && !self.asleep {self.x=x.min(4095);self.y=y.min(4095);self.down=down;self.pending=true;}}
    pub fn reset(&mut self) {self.down=false;self.pending=false;self.asleep=false;}
    pub fn irq_high(&self)->bool {!self.powered || self.asleep || !self.pending}
}
pub struct Cst816s {state:Arc<Mutex<Cst816State>>,registers:Reg8Device}
impl Cst816s {pub fn new(state:Arc<Mutex<Cst816State>>)->Self {Self{state,registers:Reg8Device::new("cst816s",&[])}}}
impl I2cDevice for Cst816s {
    fn start(&mut self,read:bool)->bool {let s=self.state.lock().expect("CST816S state mutex poisoned");s.powered && !s.asleep && self.registers.start(read)}
    fn write(&mut self,b:u8)->bool {
        if !self.registers.first {match self.registers.ptr {
            0x01..=0x08|0xa7..=0xa9=>{self.registers.ptr=self.registers.ptr.wrapping_add(1);return true;},
            0xa5 if b==3=>{let mut s=self.state.lock().expect("CST816S state mutex poisoned");s.asleep=true;s.pending=false;s.down=false;},_=>{}}}
        self.registers.write(b)
    }
    fn read(&mut self)->u8 {
        let mut s=self.state.lock().expect("CST816S state mutex poisoned");
        self.registers.regs[2]=u8::from(s.powered && !s.asleep && s.down);
        self.registers.regs[3]=(s.x>>8) as u8&15;self.registers.regs[4]=s.x as u8;
        self.registers.regs[5]=(s.y>>8) as u8&15;self.registers.regs[6]=s.y as u8;
        if self.registers.ptr==2 {s.pending=false;}self.registers.read()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn read(d:&mut Cst816s,r:u8,n:usize)->Vec<u8> {assert!(d.start(false));d.write(r);assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    #[test]
    fn contact_release_report_sleep_and_hardware_reset() {
        let s=Arc::new(Mutex::new(Cst816State::default()));let mut d=Cst816s::new(s.clone());assert!(!d.start(false));s.lock().unwrap().powered=true;
        s.lock().unwrap().update(479,799,true);assert!(!s.lock().unwrap().irq_high());assert_eq!(read(&mut d,2,5),[1,1,0xdf,3,0x1f]);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().update(400,900,false);assert_eq!(read(&mut d,2,5),[0,1,0x90,3,0x84]);assert_eq!(read(&mut d,0xa7,3),[0;3]);
        d.start(false);d.write(0xa5);d.write(3);assert!(!d.start(true));s.lock().unwrap().update(1,1,true);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().reset();assert_eq!(read(&mut d,2,1),[0]);s.lock().unwrap().powered=false;assert!(!d.start(true));
    }
}
