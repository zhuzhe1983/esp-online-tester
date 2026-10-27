//! TI LP5562 direct PWM register subset (SNVS820B, December 2016, sections 7.5/7.6).
//! No program engines, external clock, logarithmic dimming or electrical light model.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Lp5562State {regs:[u8;113],pub version:u64}
impl Default for Lp5562State {fn default()->Self {let mut s=Self{regs:[0;113],version:0};s.reset();s}}
impl Lp5562State {
    fn reset(&mut self) {self.regs.fill(0);for r in [5,6,7,15] {self.regs[r]=0xaf;}self.regs[0x70]=0x39;self.version=self.version.wrapping_add(1);}
    /// White channel PWM; zero when disabled, unclocked, zero-current or engine mapped.
    /// Nonzero current is an enable gate only, not a calibrated brightness conversion.
    pub fn white_pwm(&self)->u8 {if self.regs[0]&0x40!=0 && self.regs[8]&1!=0 && self.regs[15]!=0 && self.regs[0x70]&0xc0==0 {self.regs[14]} else {0}}
}
pub struct Lp5562 {state:Arc<Mutex<Lp5562State>>,ptr:u8,first:bool}
impl Lp5562 {pub fn new(state:Arc<Mutex<Lp5562State>>)->Self {Self{state,ptr:0,first:true}}}
impl I2cDevice for Lp5562 {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}true}
    fn write(&mut self,b:u8)->bool {
        if self.first {self.ptr=b;self.first=false;return true;}
        let mut s=self.state.lock().expect("LP5562 state mutex poisoned");
        if self.ptr==13 && b==0xff {s.reset();}
        else if self.ptr<=0x70 && self.ptr!=12 && self.ptr!=13 {s.regs[self.ptr as usize]=b;s.version=s.version.wrapping_add(1);}
        self.ptr=self.ptr.wrapping_add(1);true
    }
    fn read(&mut self)->u8 {let v=self.state.lock().expect("LP5562 state mutex poisoned").regs.get(self.ptr as usize).copied().unwrap_or(0);self.ptr=self.ptr.wrapping_add(1);v}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d:&mut Lp5562,r:u8,v:u8) {d.start(false);d.write(r);d.write(v);}
    #[test]
    fn white_pwm_obeys_reset_enable_clock_current_and_mapping() {
        let s=Arc::new(Mutex::new(Lp5562State::default()));let mut d=Lp5562::new(s.clone());
        d.start(false);d.write(5);d.start(true);assert_eq!((0..3).map(|_|d.read()).collect::<Vec<_>>(),[0xaf;3]);
        wr(&mut d,14,128);assert_eq!(s.lock().unwrap().white_pwm(),0);
        wr(&mut d,0,0x40);wr(&mut d,8,1);wr(&mut d,0x70,0);assert_eq!(s.lock().unwrap().white_pwm(),128);
        wr(&mut d,0x70,0x40);assert_eq!(s.lock().unwrap().white_pwm(),0,"unimplemented engine must not light output");
        wr(&mut d,0x70,0);wr(&mut d,15,0);assert_eq!(s.lock().unwrap().white_pwm(),0);wr(&mut d,15,0xaf);assert_eq!(s.lock().unwrap().white_pwm(),128);
        let v=s.lock().unwrap().version;wr(&mut d,13,0xff);assert!(s.lock().unwrap().version>v);assert_eq!(s.lock().unwrap().white_pwm(),0);
        wr(&mut d,0xff,123);d.start(false);d.write(0xff);d.start(true);assert_eq!(d.read(),0);
    }
}
