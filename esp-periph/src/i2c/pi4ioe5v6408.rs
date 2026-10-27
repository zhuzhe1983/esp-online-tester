//! PI4IOE5V6408 digital subset: Diodes DS40583 Rev 3-5 (September 2022),
//! pp.8–13. 1=output, explicit high-Z, pulls, latched interrupt with re-arm,
//! read-cleared reset/status. No burst reads, analog contention or timing.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Pi4ioState {pub powered:bool,direction:u8,output:u8,high_z:u8,input_default:u8,pull_enable:u8,pull_up:u8,mask:u8,pending:u8,reset_irq:bool,driven:u8,inputs:u8,nondefault:u8,pub low_edges:[u64;8]}
impl Default for Pi4ioState {fn default()->Self {Self{powered:true,direction:0,output:0,high_z:255,input_default:0,pull_enable:255,pull_up:0,mask:0,pending:0,reset_irq:true,driven:0,inputs:0,nondefault:0,low_edges:[0;8]}}}
impl Pi4ioState {
    pub fn drive(&self,pin:u8)->Option<bool> {if pin<8 && self.powered && self.direction & !self.high_z & (1<<pin)!=0 {Some(self.output&(1<<pin)!=0)} else {None}}
    pub fn input(&self)->u8 {((self.inputs & self.driven)|(self.pull_enable & self.pull_up & !self.driven)) & !self.direction}
    fn observe(&mut self) {let current=(self.input()^self.input_default)&!self.direction;self.pending|=current & !self.nondefault;self.nondefault=current;}
    pub fn set_input(&mut self,pin:u8,level:Option<bool>) {if pin<8 {let b=1u8<<pin;self.driven=(self.driven & !b)|if level.is_some() {b}else{0};self.inputs=(self.inputs & !b)|if level==Some(true) {b}else{0};self.observe();}}
    pub fn irq_high(&self)->bool {!self.powered || (!self.reset_irq && self.pending & !self.mask==0)}
    pub fn reset(&mut self) {let (powered,driven,inputs)=(self.powered,self.driven,self.inputs);*self=Self{powered,driven,inputs,..Default::default()};self.observe();}
}
pub struct Pi4ioe5v6408 {state:Arc<Mutex<Pi4ioState>>,ptr:u8,first:bool,valid:bool,used:bool}
impl Pi4ioe5v6408 {pub fn new(state:Arc<Mutex<Pi4ioState>>)->Self {Self{state,ptr:1,first:true,valid:true,used:false}}}
impl I2cDevice for Pi4ioe5v6408 {
    fn start(&mut self,read:bool)->bool {self.used=false;if !read {self.first=true;}self.state.lock().expect("PI4IO state mutex poisoned").powered}
    fn write(&mut self,v:u8)->bool {
        if self.first {self.first=false;self.ptr=v;self.valid=matches!(v,1|3|5|7|9|11|13|15|17|19);return self.valid;}
        if !self.valid || self.used {return false;}self.used=true;
        let mut s=self.state.lock().expect("PI4IO state mutex poisoned");let before=std::array::from_fn::<_,8,_>(|pin|s.drive(pin as u8));
        match self.ptr {1=>if v&1!=0 {s.reset();},3=>s.direction=v,5=>s.output=v,7=>s.high_z=v,9=>s.input_default=v,11=>s.pull_enable=v,13=>s.pull_up=v,17=>s.mask=v,_=>{}}
        for (pin,old) in before.into_iter().enumerate() {if old!=Some(false) && s.drive(pin as u8)==Some(false) {s.low_edges[pin]=s.low_edges[pin].wrapping_add(1);}}
        s.observe();true
    }
    fn read(&mut self)->u8 {
        if !self.valid || self.used {return 255;}self.used=true;
        let mut s=self.state.lock().expect("PI4IO state mutex poisoned");
        match self.ptr {1=>{let v=0xa0|if s.reset_irq {2}else{0};s.reset_irq=false;v},3=>s.direction,5=>s.output,7=>s.high_z,9=>s.input_default,11=>s.pull_enable,13=>s.pull_up,15=>s.input(),17=>s.mask,19=>{let p=s.pending;s.pending=0;p},_=>255}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Pi4ioe5v6408,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn read(d:&mut Pi4ioe5v6408,r:u8)->u8 {assert!(d.start(false));assert!(d.write(r));assert!(d.start(true));d.read()}
    #[test]
    fn direction_high_z_pulls_and_read_only_inputs() {
        let s=Arc::new(Mutex::new(Pi4ioState::default()));let mut d=Pi4ioe5v6408::new(s.clone());
        assert_eq!(read(&mut d,1),0xa2);assert_eq!(read(&mut d,1),0xa0);assert_eq!(read(&mut d,3),0);assert_eq!(read(&mut d,7),255);
        write(&mut d,13,3);assert_eq!(read(&mut d,15),3);s.lock().unwrap().set_input(0,Some(false));assert_eq!(read(&mut d,15),2);
        write(&mut d,5,3);write(&mut d,3,3);assert_eq!(s.lock().unwrap().drive(0),None);assert_eq!(read(&mut d,15),0,"outputs never read as inputs");
        write(&mut d,7,0xfe);assert_eq!(s.lock().unwrap().drive(0),Some(true));assert_eq!(s.lock().unwrap().drive(1),None);
        write(&mut d,5,0);assert_eq!(s.lock().unwrap().drive(0),Some(false));write(&mut d,15,255);assert_eq!(read(&mut d,15),0);
        write(&mut d,1,1);assert_eq!(read(&mut d,1),0xa2);assert_eq!(read(&mut d,7),255);assert_eq!(s.lock().unwrap().drive(0),None);
        assert!(d.start(false));assert!(!d.write(255));assert!(!d.write(1));assert_eq!(d.read(),255);assert_eq!(read(&mut d,3),0);assert_eq!(d.read(),255,"burst read unsupported");
        s.lock().unwrap().powered=false;assert!(!d.start(true));assert_eq!(s.lock().unwrap().drive(0),None);
    }
    #[test]
    fn masked_latches_clear_on_status_read_and_rearm_only_after_default() {
        let s=Arc::new(Mutex::new(Pi4ioState::default()));let mut d=Pi4ioe5v6408::new(s.clone());read(&mut d,1);
        write(&mut d,13,3);write(&mut d,9,3);read(&mut d,19);assert!(s.lock().unwrap().irq_high());
        write(&mut d,17,1);s.lock().unwrap().set_input(0,Some(false));assert!(s.lock().unwrap().irq_high());assert_eq!(read(&mut d,19)&1,1);
        s.lock().unwrap().set_input(1,Some(false));assert!(!s.lock().unwrap().irq_high());assert_eq!(read(&mut d,15)&3,0);assert!(!s.lock().unwrap().irq_high());assert_eq!(read(&mut d,19),2);assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().set_input(1,Some(false));assert_eq!(read(&mut d,19),0);s.lock().unwrap().set_input(1,None);s.lock().unwrap().set_input(1,Some(false));assert_eq!(read(&mut d,19),2);
    }
}
