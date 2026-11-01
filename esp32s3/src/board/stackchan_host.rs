//! StackChan CoreS3 host's AW9523B and AXP2101 digital control subset.
//! Wiring: Sch_M5_CoreS3_v1.0.pdf; AW9523B-EN.pdf registers 00–07/10–13/7F;
//! M5GFX 7cb26b44 and M5Unified 44952cb8. USB supply is an explicit fixed
//! virtual profile; charging, battery dynamics, power-latch and sleep are not modeled.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State}};

pub(super) struct HostState {aw:[u8;256],axp:[u8;256],input:[u8;2],pending:[u8;2],pub version:u64,pub reset_version:u64}
impl HostState {
    pub fn new()->Self {
        let mut state=Self{aw:[0;256],axp:[0;256],input:[0xff;2],pending:[0;2],version:0,reset_version:0};state.reset_aw();
        // AXP2101 ID, USB present, no emulated battery. DCDC1 supplies the host.
        for (r,v) in [(0,0x20),(1,0x10),(3,0x4a),(0x20,0x20),(0x27,0x14),(0x80,1),(0x82,18)] {state.axp[r]=v;}
        state
    }
    fn reset_aw(&mut self) {
        // AD1=AD0=GND (address58): power-on output levels and direction are 0.
        self.aw.fill(0);self.aw[0x10]=0x23;self.aw[0x12]=0xff;self.aw[0x13]=0xff;self.pending=[0;2];self.reset_version+=1;self.version+=1;
    }
    fn output(&self,port:usize,bit:u8)->bool {self.aw[0x12+port]&bit!=0 && self.aw[4+port]&bit==0 && self.aw[2+port]&bit!=0}
    pub fn lcd_reset(&self)->bool {self.output(1,2)}
    pub fn touch_reset(&self)->bool {self.output(0,1)}
    pub fn backlight(&self)->bool {self.axp[0x90]&0x80!=0 && self.axp[0x99]!=0}
    pub fn touch_level(&mut self,high:bool) {
        let old=self.input[1];if high {self.input[1]|=4;} else {self.input[1]&=!4;}
        self.pending[1]|=(old^self.input[1])&self.aw[5]&self.aw[0x13]&!self.aw[7];
    }
    pub fn irq(&self)->bool {(0..2).any(|p|self.pending[p]&self.aw[4+p]&self.aw[0x12+p]&!self.aw[6+p]!=0)}
    fn aw_read(&mut self,reg:u8)->u8 {
        if reg<2 {let p=usize::from(reg);self.pending[p]=0;return (self.input[p]&self.aw[4+p])|(self.aw[2+p]&!self.aw[4+p]);}
        self.aw[usize::from(reg)]
    }
    fn aw_write(&mut self,reg:u8,value:u8) {
        let old_reset=self.lcd_reset();
        match reg {0x02..=0x07|0x11..=0x13|0x20..=0x2f=>{self.aw[usize::from(reg)]=value;self.version+=1;},0x7f if value==0=>self.reset_aw(),_=>{}}
        if old_reset && !self.lcd_reset() {self.reset_version+=1;}
    }
    fn axp_read(&self,reg:u8)->u8 {
        // Enabled ADC channels see fixed USB=5V / VSYS=3.3V; no battery sample.
        match reg {0x38|0x39 if self.axp[0x30]&4!=0=>(5000u16>>if reg==0x38 {8} else {0}) as u8,0x3a|0x3b if self.axp[0x30]&8!=0=>(3300u16>>if reg==0x3a {8} else {0}) as u8,_=>self.axp[usize::from(reg)]}
    }
    fn axp_write(&mut self,reg:u8,value:u8) {
        match reg {0x48..=0x4a=>self.axp[usize::from(reg)]&=!value,0x04..=0x07|0x10|0x12..=0x1a|0x20..=0x30|0x40..=0x42|0x50..=0x6a|0x80..=0x9a=>self.axp[usize::from(reg)]=value,_=>{}}
        self.version+=1;
    }
}

pub(super) struct HostDevice {state:Arc<Mutex<HostState>>,axp:bool,ptr:u8,first:bool}
impl HostDevice {pub fn new(state:Arc<Mutex<HostState>>,axp:bool)->Self {Self{state,axp,ptr:0,first:true}}}
impl I2cDevice for HostDevice {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}true}
    fn write(&mut self,value:u8)->bool {if self.first {self.ptr=value;self.first=false;} else {let mut state=self.state.lock().expect("StackChan host mutex poisoned");if self.axp {state.axp_write(self.ptr,value);} else {state.aw_write(self.ptr,value);}self.ptr=self.ptr.wrapping_add(1);}true}
    fn read(&mut self)->u8 {let mut state=self.state.lock().expect("StackChan host mutex poisoned");let value=if self.axp {state.axp_read(self.ptr)} else {state.aw_read(self.ptr)};self.ptr=self.ptr.wrapping_add(1);value}
}

pub(super) struct HostTouch {device:Ft6336g,state:Arc<Mutex<HostState>>,touch:Arc<Mutex<Ft6336State>>}
impl HostTouch {pub fn new(state:Arc<Mutex<HostState>>,touch:Arc<Mutex<Ft6336State>>)->Self {Self{device:Ft6336g::new(touch.clone()),state,touch}}}
impl I2cDevice for HostTouch {
    fn start(&mut self,read:bool)->bool {
        let powered=self.state.lock().expect("StackChan host mutex poisoned").touch_reset();let mut touch=self.touch.lock().expect("StackChan touch mutex poisoned");touch.powered=powered;if !powered {touch.reset();}drop(touch);self.device.start(read)
    }
    fn write(&mut self,value:u8)->bool {self.device.write(value)}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn aw_identity_reset_direction_and_masked_touch_interrupt() {
        let mut h=HostState::new();assert_eq!(h.aw_read(0x10),0x23);h.aw_write(0x10,0);assert_eq!(h.aw_read(0x10),0x23);assert!(!h.lcd_reset());
        h.aw_write(3,2);assert!(h.lcd_reset());h.aw_write(5,4);h.touch_level(false);assert!(h.irq());assert_eq!(h.aw_read(1)&4,0);assert!(!h.irq());
        h.aw_write(7,4);h.touch_level(true);assert!(!h.irq());h.aw_write(7,0);h.touch_level(false);assert!(h.irq());
        h.aw_write(0x7f,0);assert!(!h.irq());assert!(!h.lcd_reset());assert!(!h.touch_reset());assert_eq!(h.aw_read(5),0);
    }
    #[test] fn pmic_identity_backlight_and_adc_have_real_enable_registers() {
        let mut h=HostState::new();h.axp_write(3,0);assert_eq!(h.axp_read(3),0x4a);assert!(!h.backlight());h.axp_write(0x90,0x80);assert!(!h.backlight());h.axp_write(0x99,28);assert!(h.backlight());h.axp_write(0x90,0);assert!(!h.backlight());
        assert_eq!(h.axp_read(0x38),0);h.axp_write(0x30,12);assert_eq!(u16::from_be_bytes([h.axp_read(0x38),h.axp_read(0x39)]),5000);assert_eq!(h.axp_read(0)&8,0);
    }
}
