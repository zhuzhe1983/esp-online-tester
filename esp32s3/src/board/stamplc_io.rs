//! K141 AW9523B (address 0x59, AD1=0 AD0=1), manufacturer v1.1.1 pp11,17–21.
//! Schematic K141_sch_StamPLC_V10_IO: P0.0..3 drive ULN2003 relay coils,
//! P0.4..7/P1.4..7 have optocoupler outputs with 5.1k pull-downs.
//! Digital output/readback only: no contact/load model, optical thresholds or debounce.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
pub(super) struct Io {pub powered:bool,regs:[u8;256],inputs:u16,pending:u16}
impl Io {
    pub fn new()->Self {let mut s=Self{powered:true,regs:[0;256],inputs:0,pending:0};s.reset();s}
    pub fn reset(&mut self) {self.regs.fill(0);self.regs[2]=15;self.regs[3]=15;self.regs[0x10]=0x23;self.regs[0x12]=255;self.regs[0x13]=255;self.pending=0;}
    pub fn drive(&self,pin:u8)->Option<bool> {
        if pin>=16 || !self.powered {return None;}let p=usize::from(pin/8);let bit=1<<(pin%8);
        if self.regs[4+p]&bit!=0 || self.regs[0x12+p]&bit==0 {return None;}
        let high=self.regs[2+p]&bit!=0;if p==0 && self.regs[0x11]&0x10==0 && high {None}else{Some(high)}
    }
    pub fn relay_mask(&self)->u8 {(0..4).fold(0,|v,pin|v|(u8::from(self.drive(pin)==Some(true))<<pin))}
    pub fn irq_high(&self)->bool {!self.powered || self.pending==0}
    fn levels(&self)->u16 {(0..16).fold(0,|v,pin|v|(u16::from(self.drive(pin).unwrap_or(self.inputs&(1<<pin)!=0))<<pin))}
    #[cfg(test)]
    fn input(&mut self,pin:u8,high:bool) {if pin>=16{return;}let bit=1<<pin;let before=self.inputs;self.inputs=(self.inputs&!bit)|if high {bit}else{0};let input=u16::from_le_bytes([self.regs[4],self.regs[5]]);let gpio=u16::from_le_bytes([self.regs[0x12],self.regs[0x13]]);let masked=u16::from_le_bytes([self.regs[6],self.regs[7]]);self.pending|=(before^self.inputs)&input&gpio&!masked;}
}
pub(super) struct Device {state:Arc<Mutex<Io>>,ptr:u8,first:bool}
impl Device {pub fn new(state:Arc<Mutex<Io>>)->Self {Self{state,ptr:0,first:true}}}
impl I2cDevice for Device {
    fn start(&mut self,read:bool)->bool {if !read{self.first=true;}self.state.lock().expect("StamPLC IO poisoned").powered}
    fn write(&mut self,v:u8)->bool {
        if self.first {self.ptr=v;self.first=false;return true;}let mut s=self.state.lock().expect("StamPLC IO poisoned");
        match self.ptr {2..=7|0x11..=0x13|0x20..=0x2f=>s.regs[self.ptr as usize]=v,0x7f if v==0=>s.reset(),_=>{}}self.ptr=self.ptr.wrapping_add(1);true
    }
    fn read(&mut self)->u8 {let mut s=self.state.lock().expect("StamPLC IO poisoned");let value=if self.ptr<2 {let shift=self.ptr*8;let value=(s.levels()>>shift) as u8;s.pending&=!(255<<shift);value}else{s.regs[self.ptr as usize]};self.ptr=self.ptr.wrapping_add(1);value}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d:&mut Device,r:u8,v:u8){assert!(d.start(false));d.write(r);d.write(v);}
    fn rd(d:&mut Device,r:u8)->u8{assert!(d.start(false));d.write(r);d.start(true);d.read()}
    #[test]
    fn address_straps_open_drain_direction_mode_and_reset_gate_relays() {
        let s=Arc::new(Mutex::new(Io::new()));let mut d=Device::new(s.clone());assert_eq!(rd(&mut d,2),15);assert_eq!(rd(&mut d,3),15);assert_eq!(s.lock().unwrap().relay_mask(),0,"0x59 boots with released P0.0..3, not energized relays");
        wr(&mut d,0x11,0x10);assert_eq!(s.lock().unwrap().relay_mask(),15);wr(&mut d,2,5);assert_eq!(s.lock().unwrap().relay_mask(),5);assert_eq!(rd(&mut d,0)&15,5);
        wr(&mut d,4,1);assert_eq!(s.lock().unwrap().relay_mask(),4);wr(&mut d,0x12,0xfb);assert_eq!(s.lock().unwrap().relay_mask(),0);
        wr(&mut d,0x10,0);assert_eq!(rd(&mut d,0x10),0x23);wr(&mut d,0x7f,0);assert_eq!(rd(&mut d,2),15);assert_eq!(s.lock().unwrap().relay_mask(),0);
        s.lock().unwrap().powered=false;assert!(!d.start(false));
    }
    #[test]
    fn input_interrupt_is_port_read_clear_and_masked() {
        let s=Arc::new(Mutex::new(Io::new()));let mut d=Device::new(s.clone());wr(&mut d,4,0xf0);s.lock().unwrap().input(4,true);assert!(!s.lock().unwrap().irq_high());assert_eq!(rd(&mut d,1)&0xf0,0);assert!(!s.lock().unwrap().irq_high());assert_eq!(rd(&mut d,0)&0xf0,0x10);assert!(s.lock().unwrap().irq_high());wr(&mut d,6,0xf0);s.lock().unwrap().input(4,false);assert!(s.lock().unwrap().irq_high());
    }
}
