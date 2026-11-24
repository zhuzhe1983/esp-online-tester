//! Digital subsets from the pinned C5 Touch2.8 BSP and CH32 component1.0.1.
//! CH32V003 is an MCU running board firmware, not a generic expander chip.
//! Only its 8-bit direction/output/input and PWM commands are modeled here.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Ch32Port{pub direction:u8,pub output:u8,pub pwm:Option<u8>,pub inputs:u8,falling:[u64;8],pub writes:u64}
impl Ch32Port{
    pub fn new()->Self{Self{direction:0xff,output:0,pwm:None,inputs:0,falling:[0;8],writes:0}}
    pub fn levels(&self)->u8{(self.output&self.direction)|(self.inputs&!self.direction)}
    pub fn high(&self,pin:u8)->bool{self.levels()&(1<<pin)!=0}
    pub fn falling_edges(&self,pin:usize)->u64{self.falling[pin]}
    fn write(&mut self,reg:u8,value:u8){
        let old=self.levels();match reg{2=>self.direction=value,3=>self.output=value,5=>self.pwm=Some(value),_=>return}
        let falling=old&!self.levels();for(pin,count)in self.falling.iter_mut().enumerate(){if falling&(1<<pin)!=0{*count=count.wrapping_add(1);}}
        self.writes+=1;
    }
}
pub struct Ch32V003{pub state:Arc<Mutex<Ch32Port>>,reg:Option<u8>,first:bool,used:bool}
impl Ch32V003{pub fn new(state:Arc<Mutex<Ch32Port>>)->Self{Self{state,reg:None,first:true,used:false}}}
impl I2cDevice for Ch32V003{
    fn start(&mut self,read:bool)->bool{self.used=false;if read{self.reg==Some(4)}else{self.first=true;self.reg=None;true}}
    fn write(&mut self,value:u8)->bool{
        if self.first{self.first=false;self.reg=matches!(value,2..=5).then_some(value);return self.reg.is_some();}
        let Some(reg)=self.reg else{return false;};if self.used||reg==4{return false;}
        self.used=true;self.state.lock().expect("CH32V003 state").write(reg,value);true
    }
    fn read(&mut self)->u8{if self.reg!=Some(4)||self.used{return 0xff;}self.used=true;self.state.lock().expect("CH32V003 state").levels()}
}

/// A digital contact source. IRQ is an event latch, not a claim about silicon
/// scan timing or pulse duration. A late ACK cannot swallow a newer host event.
pub struct Cst3530State{down:bool,x:u16,y:u16,reset_high:bool,reset_count:u64,sequence:u64,pending:bool,pub reports:u64}
impl Cst3530State{
    pub fn new()->Self{Self{down:false,x:0,y:0,reset_high:false,reset_count:0,sequence:0,pending:false,reports:0}}
    pub fn set_reset(&mut self,high:bool){if self.reset_high&&!high{self.down=false;self.pending=false;self.sequence=self.sequence.wrapping_add(1);self.reset_count=self.reset_count.wrapping_add(1);}self.reset_high=high;}
    pub fn update(&mut self,x:u16,y:u16,down:bool){if !self.reset_high{return;}self.x=x.min(239);self.y=y.min(319);self.down=down;self.sequence=self.sequence.wrapping_add(1);self.pending=true;}
    pub fn irq_high(&self)->bool{!self.reset_high||!self.pending}
    // The source driver does not define bytes0..2 or a calibrated pressure.
    // Those fields are zero; only its count/coordinate/event contract is exposed.
    fn report(&self)->[u8;9]{[0,0,0,u8::from(self.down),self.x as u8,self.y as u8,0,((self.x>>8)&15)as u8|(((self.y>>8)&15)as u8)<<4,if self.down{0x20}else{0}]}
}
pub struct Cst3530{state:Arc<Mutex<Cst3530State>>,command:u32,count:u8,selector:bool,data:[u8;9],offset:usize,snapshot:Option<u64>,reset_count:u64}
impl Cst3530{pub fn new(state:Arc<Mutex<Cst3530State>>)->Self{Self{state,command:0,count:0,selector:false,data:[0;9],offset:0,snapshot:None,reset_count:0}}}
impl I2cDevice for Cst3530{
    fn start(&mut self,read:bool)->bool{
        let mut s=self.state.lock().expect("CST3530 state");
        if self.reset_count!=s.reset_count{self.reset_count=s.reset_count;self.selector=false;self.snapshot=None;self.count=0;}
        if !s.reset_high{return false;}
        if read{if !self.selector{return false;}self.data=s.report();self.offset=0;self.snapshot=Some(s.sequence);s.reports+=1;}
        else{self.command=0;self.count=0;self.selector=false;}true
    }
    fn write(&mut self,value:u8)->bool{
        if self.count>=4{return false;}self.command=(self.command<<8)|u32::from(value);self.count+=1;
        if self.count<4{return true;}
        match self.command{
            0xd007_0000=>{self.selector=true;true},
            0xd000_02ab=>{let mut s=self.state.lock().expect("CST3530 state");if self.snapshot==Some(s.sequence){s.pending=false;}self.snapshot=None;true},
            _=>false,
        }
    }
    fn read(&mut self)->u8{let value=self.data.get(self.offset).copied().unwrap_or(0xff);self.offset=self.offset.saturating_add(1);value}
    fn stop(&mut self){if self.count!=4{self.selector=false;}}
}

#[cfg(test)]mod tests{
    use super::*;
    fn write(d:&mut dyn I2cDevice,bytes:&[u8])->bool{d.start(false)&&bytes.iter().all(|b|d.write(*b))}
    #[test]fn ch32_direction_preload_pwm_and_unsupported_adc_are_distinct(){
        let state=Arc::new(Mutex::new(Ch32Port::new()));let mut d=Ch32V003::new(state.clone());
        assert!(write(&mut d,&[2,0]));assert!(write(&mut d,&[3,3]));assert_eq!(state.lock().unwrap().levels(),0,"input pins are not driven by output preload");
        assert!(write(&mut d,&[2,3]));assert_eq!(state.lock().unwrap().levels(),3);
        assert!(write(&mut d,&[3,2]));assert_eq!(state.lock().unwrap().falling_edges(0),1);
        assert!(write(&mut d,&[5,128]));assert_eq!(state.lock().unwrap().pwm,Some(128));
        state.lock().unwrap().inputs=0x80;assert!(write(&mut d,&[4]));assert!(d.start(true));assert_eq!(d.read(),0x82);assert_eq!(d.read(),0xff);
        for command in[0,1,6,7,255]{assert!(!write(&mut d,&[command]));}
        assert!(write(&mut d,&[2]));assert!(!d.start(true),"the driver caches direction; unsupported wire readback is not invented");
        assert!(!write(&mut d,&[3,1,2]),"the 8-bit V003 protocol is not a V006 16-bit alias");
    }
    #[test]fn cst3530_uses_big_endian_selector_and_does_not_ack_unknown_identity(){
        let state=Arc::new(Mutex::new(Cst3530State::new()));let mut d=Cst3530::new(state.clone());assert!(!d.start(false));
        state.lock().unwrap().set_reset(true);state.lock().unwrap().update(200,300,true);
        assert!(write(&mut d,&[0xd0,7,0,0]));d.stop();assert!(d.start(true));let a:Vec<_>=(0..9).map(|_|d.read()).collect();
        assert_eq!((a[3],a[4],a[5],a[7],a[8]),(1,200,44,0x10,0x20));assert!(!state.lock().unwrap().irq_high());
        state.lock().unwrap().update(200,300,false);assert!(write(&mut d,&[0xd0,0,2,0xab]));assert!(!state.lock().unwrap().irq_high(),"old report ACK must preserve newer release");
        assert!(write(&mut d,&[0xd0,7,0,0]));assert!(d.start(true));assert_eq!((0..9).map(|_|d.read()).collect::<Vec<_>>()[3],0);
        assert!(write(&mut d,&[0xd0,0,2,0xab]));assert!(state.lock().unwrap().irq_high());
        assert!(!write(&mut d,&[0,0,7,0xd0]));assert!(!write(&mut d,&[0xd0,3,0,0]));
        assert!(write(&mut d,&[0xd0,7,0,0]));state.lock().unwrap().set_reset(false);assert!(!d.start(true));
        state.lock().unwrap().set_reset(true);assert!(!d.start(true),"hardware reset clears a prior selector");
    }
}
