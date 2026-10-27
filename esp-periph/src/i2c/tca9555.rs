//! TCA9555 digital port subset, TI SCPS200E (April 2019), sections 9.3.3,
//! 9.5.4 and 9.6.1. Registers advance within their two-byte pair, not 0..7.
//! Input/output contention, propagation delays and analog thresholds are not modeled.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Tca9555State {pub powered:bool,pub inputs:u16,output:u16,polarity:u16,config:u16,observed:u16}
impl Default for Tca9555State {
    fn default()->Self {Self{powered:true,inputs:0xffff,output:0xffff,polarity:0,config:0xffff,observed:0xffff}}
}
impl Tca9555State {
    /// Only explicitly driven outputs count as a rail/reset enable.
    pub fn driven_high(&self)->u16 {if self.powered {self.output & !self.config} else {0}}
    pub fn levels(&self)->u16 {(self.inputs & self.config)|(self.output & !self.config)}
    pub fn irq_high(&self)->bool {!self.powered || (self.levels()^self.observed)&self.config==0}
    pub fn set_input(&mut self,pin:u8,high:bool) {if pin<16 {let mask=1u16<<pin;self.inputs=(self.inputs & !mask)|if high {mask}else{0};}}
    pub fn reset(&mut self) {self.output=0xffff;self.polarity=0;self.config=0xffff;self.observed=self.inputs;}
}
pub struct Tca9555 {state:Arc<Mutex<Tca9555State>>,ptr:u8,first:bool,valid:bool}
impl Tca9555 {pub fn new(state:Arc<Mutex<Tca9555State>>)->Self {Self{state,ptr:0,first:true,valid:true}}}
impl I2cDevice for Tca9555 {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}self.state.lock().expect("TCA9555 state mutex poisoned").powered}
    fn write(&mut self,byte:u8)->bool {
        if self.first {self.first=false;self.valid=byte<8;if self.valid {self.ptr=byte;}return self.valid;}
        if !self.valid {return false;}
        let mut state=self.state.lock().expect("TCA9555 state mutex poisoned");let shift=(self.ptr&1)*8;
        let register=match self.ptr&6 {2=>Some(&mut state.output),4=>Some(&mut state.polarity),6=>Some(&mut state.config),_=>None};
        if let Some(register)=register {*register=(*register & !(0xffu16<<shift))|(u16::from(byte)<<shift);}
        self.ptr^=1;true
    }
    fn read(&mut self)->u8 {
        if !self.valid {return 0xff;}
        let mut state=self.state.lock().expect("TCA9555 state mutex poisoned");let shift=(self.ptr&1)*8;
        let value=match self.ptr&6 {0=>{let levels=state.levels();state.observed=(state.observed & !(0xffu16<<shift))|(levels & (0xffu16<<shift));levels^(state.polarity & state.config)},2=>state.output,4=>state.polarity,_=>state.config};
        self.ptr^=1;(value>>shift) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Tca9555,r:u8,values:&[u8]) {assert!(d.start(false));assert!(d.write(r));for &b in values {assert!(d.write(b));}}
    fn read(d:&mut Tca9555,r:u8,n:usize)->Vec<u8> {write(d,r,&[]);assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    #[test]
    fn latch_direction_polarity_and_pair_wrap_do_not_invent_input_drives() {
        let s=Arc::new(Mutex::new(Tca9555State::default()));let mut d=Tca9555::new(s.clone());
        assert_eq!(read(&mut d,6,4),[255;4]);assert_eq!(s.lock().unwrap().driven_high(),0);
        write(&mut d,2,&[0x60,2]);assert_eq!(s.lock().unwrap().driven_high(),0);
        write(&mut d,6,&[0x9f,0xfd]);assert_eq!(s.lock().unwrap().driven_high(),0x260);
        s.lock().unwrap().set_input(7,false);assert_eq!(read(&mut d,0,2),[0x7f,0xff]);
        write(&mut d,4,&[0x80,0]);assert_eq!(read(&mut d,0,2),[0xff,0xff]);assert_eq!(read(&mut d,2,4),[0x60,2,0x60,2]);
        write(&mut d,0,&[0,0]);assert_eq!(read(&mut d,0,2),[0xff,0xff]);
        write(&mut d,2,&[1,2,3]);assert_eq!(read(&mut d,2,2),[3,2]);
        assert!(d.start(false));assert!(!d.write(255));assert!(!d.write(0));assert_eq!(d.read(),255);
    }
    #[test]
    fn interrupt_clears_only_the_read_port_or_returned_input_and_never_output_changes() {
        let s=Arc::new(Mutex::new(Tca9555State::default()));let mut d=Tca9555::new(s.clone());
        {let mut s=s.lock().unwrap();s.set_input(7,false);s.set_input(8,false);assert!(!s.irq_high());}
        assert_eq!(read(&mut d,0,1),[0x7f]);assert!(!s.lock().unwrap().irq_high());assert_eq!(read(&mut d,1,1),[0xfe]);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().set_input(7,true);assert!(!s.lock().unwrap().irq_high());s.lock().unwrap().set_input(7,false);assert!(s.lock().unwrap().irq_high());
        write(&mut d,6,&[0,0]);write(&mut d,2,&[0,0]);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().powered=false;assert!(!d.start(true));s.lock().unwrap().reset();assert_eq!(s.lock().unwrap().driven_high(),0);
    }
}
