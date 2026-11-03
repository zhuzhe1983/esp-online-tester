//! INA3221 digital conversion subset, TI SBOS576C §§7.4–7.5, 8.2.
//! Explicit ideal input voltages, conversion periods and CVRF; no battery,
//! current-load dynamics, averaging noise or alert-pin behavior is simulated.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Ina3221State {
    regs:[u16;18], sample:[[f64;2];3], clock_hz:u64, cycle:u64,
    deadline:Option<u64>, pub conversions:u64,
}
impl Ina3221State {
    pub fn new(clock_hz:u64)->Self {
        let mut s=Self{regs:[0;18],sample:[[0.0;2];3],clock_hz:clock_hz.max(1),cycle:0,deadline:None,conversions:0};s.reset();s
    }
    fn reset(&mut self) {
        self.regs.fill(0);self.regs[0]=0x7127;
        self.regs[7..=12].fill(0x7ff8);self.regs[14]=0x7ffe;
        self.regs[15]=2;self.regs[16]=0x2710;self.regs[17]=0x2328;self.restart();
    }
    /// Bus V (0..26 V), shunt V (-163.84..163.8 mV), before the next conversion.
    pub fn set_sample(&mut self,channel:usize,bus_v:f64,shunt_v:f64)->bool {
        if channel>=3 || !bus_v.is_finite() || !shunt_v.is_finite() || !(0.0..=26.0).contains(&bus_v) || !(-0.16384..=0.1638).contains(&shunt_v) {return false;}
        self.sample[channel]=[bus_v,shunt_v];true
    }
    fn period(&self)->Option<u64> {
        let config=self.regs[0];let mode=config&7;
        if mode==0 || mode==4 {return None;}
        let channels=((config>>12)&7).count_ones() as u64;if channels==0 {return None;}
        let times=[140u64,204,332,588,1100,2116,4156,8244];
        let average=[1u64,4,16,64,128,256,512,1024][((config>>9)&7) as usize];
        let us=(if mode&1!=0 {times[((config>>3)&7) as usize]}else{0})+(if mode&2!=0 {times[((config>>6)&7) as usize]}else{0});
        Some((u128::from(us*average*channels)*u128::from(self.clock_hz)).div_ceil(1_000_000).min(u128::from(u64::MAX)) as u64)
    }
    fn restart(&mut self) {if self.period().is_some() {self.regs[15]&=!1;}self.deadline=self.period().map(|p|self.cycle.saturating_add(p.max(1)));}
    pub fn next_deadline(&self)->Option<u64> {self.deadline}
    pub fn advance_to(&mut self,cycle:u64) {
        self.cycle=self.cycle.max(cycle);
        let Some(at)=self.deadline else {return;};if at>self.cycle {return;}
        let mode=self.regs[0]&7;
        for channel in 0..3 {if self.regs[0]&(1<<(14-channel))!=0 {
            if mode&1!=0 {self.regs[1+channel*2]=(((self.sample[channel][1]/0.000040).round() as i16)<<3) as u16;}
            if mode&2!=0 {self.regs[2+channel*2]=(((self.sample[channel][0]/0.008).round() as i16)<<3) as u16;}
        }}
        let count=if mode>=5 {let period=self.period().unwrap_or(1).max(1);let count=(self.cycle-at)/period+1;self.deadline=at.checked_add(count.saturating_mul(period)).filter(|next|*next>self.cycle);count}else{self.deadline=None;1};
        self.conversions=self.conversions.saturating_add(count);self.regs[15]|=1;
    }
    fn read_register(&mut self,reg:u8)->u16 {
        let value=match reg {0xfe=>0x5449,0xff=>0x3220,_=>self.regs.get(usize::from(reg)).copied().unwrap_or(0)};
        if reg==15 {self.regs[15]&=!1;}value
    }
    fn write_register(&mut self,reg:u8,value:u16) {
        match reg {
            0 if value&0x8000!=0=>self.reset(),
            0=>{
                let waking=matches!(self.regs[0]&7,0|4);self.regs[0]=value&0x7fff;self.restart();
                if waking {let recovery=(u128::from(self.clock_hz)*40).div_ceil(1_000_000).min(u128::from(u64::MAX)) as u64;self.deadline=self.deadline.map(|at|at.saturating_add(recovery));}
            },
            7..=12|14|16|17=>self.regs[usize::from(reg)]=value,
            15=>self.regs[15]=(self.regs[15]&0x03ff)|(value&0x7c00),
            _=>{},
        }
    }
}
pub struct Ina3221 {state:Arc<Mutex<Ina3221State>>,pins:(u8,u8),register:u8,first:bool,high:Option<u8>,read_word:u16,read_high:bool}
impl Ina3221 {
    pub fn with_pins(state:Arc<Mutex<Ina3221State>>,pins:(u8,u8))->Self {Self{state,pins,register:0,first:true,high:None,read_word:0,read_high:true}}
}
impl I2cDevice for Ina3221 {
    fn pins(&self)->Option<(u8,u8)> {Some(self.pins)}
    fn start(&mut self,read:bool)->bool {if read {self.read_high=true;}else{self.first=true;self.high=None;}true}
    fn write(&mut self,byte:u8)->bool {
        if self.first {self.register=byte;self.first=false;}else if let Some(high)=self.high.take() {self.state.lock().expect("INA3221 state mutex poisoned").write_register(self.register,u16::from_be_bytes([high,byte]));}else{self.high=Some(byte);}true
    }
    fn read(&mut self)->u8 {
        if self.read_high {self.read_word=self.state.lock().expect("INA3221 state mutex poisoned").read_register(self.register);self.read_high=false;(self.read_word>>8) as u8}else{self.read_high=true;self.read_word as u8}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Ina3221,reg:u8,value:u16) {d.start(false);d.write(reg);for b in value.to_be_bytes() {d.write(b);}}
    fn read(d:&mut Ina3221,reg:u8)->u16 {d.start(false);d.write(reg);d.start(true);u16::from_be_bytes([d.read(),d.read()])}
    #[test]
    fn identities_conversion_period_and_cvrf() {
        let state=Arc::new(Mutex::new(Ina3221State::new(1_000_000)));let mut d=Ina3221::with_pins(state.clone(),(3,4));
        assert_eq!(read(&mut d,0xfe),0x5449);assert_eq!(read(&mut d,0xff),0x3220);assert_eq!(read(&mut d,0),0x7127);
        assert!(state.lock().unwrap().set_sample(1,4.0,-0.001));assert_eq!(state.lock().unwrap().next_deadline(),Some(6600));
        state.lock().unwrap().advance_to(6599);assert_eq!(read(&mut d,4),0);
        state.lock().unwrap().advance_to(6600);assert_eq!(read(&mut d,4),4000);assert_eq!(read(&mut d,3),(-200i16) as u16);
        assert_eq!(read(&mut d,15)&1,1);assert_eq!(read(&mut d,15)&1,0);
        write(&mut d,4,0xffff);assert_eq!(read(&mut d,4),4000);
        write(&mut d,0xfe,0);assert_eq!(read(&mut d,0xfe),0x5449);
    }
    #[test]
    fn triggered_channel_powerdown_and_reset() {
        let state=Arc::new(Mutex::new(Ina3221State::new(1_000_000)));let mut d=Ina3221::with_pins(state.clone(),(3,4));
        assert!(!state.lock().unwrap().set_sample(3,4.0,0.0));assert!(!state.lock().unwrap().set_sample(0,f64::NAN,0.0));
        assert!(state.lock().unwrap().set_sample(1,3.8,0.01));write(&mut d,0,0x2002);
        assert_eq!(state.lock().unwrap().next_deadline(),Some(140));state.lock().unwrap().advance_to(140);
        assert_eq!(read(&mut d,4),3800);assert_eq!(read(&mut d,3),0);assert_eq!(state.lock().unwrap().next_deadline(),None);
        write(&mut d,0,0);state.lock().unwrap().advance_to(100000);assert_eq!(read(&mut d,4),3800);assert_eq!(read(&mut d,15)&1,1);
        write(&mut d,0,0x2002);assert_eq!(state.lock().unwrap().next_deadline(),Some(100180));
        write(&mut d,0,0x8000);assert_eq!(read(&mut d,0),0x7127);assert_eq!(read(&mut d,4),0);assert_eq!(state.lock().unwrap().next_deadline(),Some(106600));
    }
}
