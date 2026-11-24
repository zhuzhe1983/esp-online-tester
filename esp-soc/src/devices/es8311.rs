//! ES8311 control port and steady-state PCM gain/mute/power subset; no analog/serial timing.
//! Everest ES8311 Datasheet Rev6.0 (May2019), section5 and section8 pp14-29;
//! official Waveshare retained copy at ESP32-C6-LCD-0.85 b73e5629985866bce242931464f24154716fcaeb:
//! Example/Arduino/libraries/ESP32-audioI2S-master/examples/ES8311/ES8311_DS.pdf.
//! PCM controls: Rev10.0 (January2021) pp14,17-21,25,28,
//! https://files.waveshare.com/wiki/common/ES8311.DS.pdf (same control bit layout).
//! The documented single-register I2C transactions are modeled. Burst transfers,
//! FLAG_CSM_CHIP/automute timing and undefined register addresses remain unsupported.
//! RESET00h controls digital blocks but explicitly excludes the control port;
//! FAh.INI_REG resets the configuration registers except FAh itself.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Es8311 {state:Arc<Mutex<Es8311State>>,ptr:u8,first:bool,written:bool}
/// Shared by the control port and the physical board PCM boundary. Register writes
/// affect the next DMA interval; no host command can bypass the codec power state.
pub struct Es8311State {regs:[u8;256],gain:[f64;2]}
impl Default for Es8311State {
    fn default()->Self {let mut s=Self{regs:[0;256],gain:[0.0;2]};s.reset_registers();s}
}
impl Es8311State {
    fn reset_registers(&mut self) {
        let fa=self.regs[0xfa];self.regs.fill(0);self.regs[0xfa]=fa;
        // Section8 register heading defaults; reserved-bit description discrepancies
        // do not override the explicit whole-register defaults.
        for (r,v) in [(0,0x1f),(3,0x10),(4,0x10),(6,3),(8,0xff),(0x0c,0x20),
            (0x0d,0xfc),(0x0e,0x6a),(0x10,0x13),(0x11,0x7c),(0x12,2),(0x13,0x40),
            (0x14,0x10),(0x16,4),(0x1b,0x0c),(0x1c,0x4c),(0x37,8),(0xfd,0x83),(0xfe,0x11)] {self.regs[r]=v;}
        self.update_gain();
    }
    // Rev10.0 pp20-21,25: volume 0xBF=0dB, 0.5dB/step; ADC PGA 3dB/step
    // (codes 0..10) and ADC_SCALE 6dB/step. The Rev10 ADC 0x01 heading has a
    // discontinuity (-90.5dB); retain it rather than guessing DAC's -95dB value.
    fn update_gain(&mut self) {
        let volume=|v:u8,adc:bool| if adc&&v==1 {-90.5}else{(f64::from(v)-191.0)*0.5};
        let adc=volume(self.regs[0x17],true)+3.0*f64::from(self.regs[0x14]&15)+6.0*f64::from(self.regs[0x16]&7);
        self.gain=[10f64.powf(adc/20.0),10f64.powf(volume(self.regs[0x32],false)/20.0)];
    }
    fn ready(&self,adc:bool,bits:u32)->bool {
        let r=&self.regs;let sdp=r[if adc{0x0a}else{9}];
        let width=match (sdp>>2)&7 {0=>24,3=>16,4=>32,_=>0};
        // Rev10.0 pp14,17-21,25. Slave I2S, normal polarity, explicit clock
        // enables and released reset only. Power settling/MCLK edge timing is absent.
        r[0xfa]&1==0 && r[0]&0x80!=0 && r[0]&if adc{0x5e}else{0x5d}==0
            && r[1]&if adc{0x3a}else{0x35}==if adc{0x3a}else{0x35}
            && r[0x0d]&if adc{0xf0}else{0xc8}==0
            && sdp&if adc{0x63}else{0xe3}==0 && width!=0 && matches!(bits,16|24|32)
            && if adc {r[0x0e]&0x70==0 && r[0x14]&0x50==0x10 && r[0x14]&15<=10
                && r[0x16]&8==0 && r[0x18]&0xc0==0 && r[0x44]&0x70==0}
            else {r[0x12]&2==0 && r[0x31]&0x68==0 && r[0x34]&0x80==0 && r[0x44]&0x80==0}
    }
    pub fn adc_gain(&self,bits:u32)->f64 {if self.ready(true,bits) {self.gain[0]*if self.regs[0x16]&16!=0 {-1.0}else{1.0}}else{0.0}}
    pub fn dac_gain(&self,bits:u32)->f64 {if self.ready(false,bits) {self.gain[1]*if self.regs[0x31]&16!=0 {-1.0}else{1.0}}else{0.0}}
}
impl Default for Es8311 {fn default()->Self {Self::new()}}
impl Es8311 {
    pub fn new()->Self {Self::with_state(Default::default())}
    pub fn with_state(state:Arc<Mutex<Es8311State>>)->Self {Self{state,ptr:0,first:true,written:false}}
    fn supported(r:u8)->bool {r<=0x45||r==0xfa||r>=0xfd}
}
impl I2cDevice for Es8311 {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;self.written=false;true}else{Self::supported(self.ptr)}}
    fn write(&mut self,b:u8)->bool {
        if self.first {self.ptr=b;self.first=false;return Self::supported(b);}
        if self.written||!Self::supported(self.ptr) {return false;}
        self.written=true;
        let mut state=self.state.lock().expect("ES8311 state poisoned");
        if self.ptr==0xfa {state.regs[0xfa]=b&3;if b&1!=0 {state.reset_registers();}}
        else if self.ptr<=0x45&&state.regs[0xfa]&1==0 {state.regs[self.ptr as usize]=b;state.update_gain();}
        // FDh/FEh/FFh are read-only published chip identifiers, not per-unit IDs.
        true
    }
    fn read(&mut self)->u8 {if Self::supported(self.ptr) {self.state.lock().expect("ES8311 state poisoned").regs[self.ptr as usize]}else{0xff}}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Es8311,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn read(d:&mut Es8311,r:u8)->u8 {assert!(d.start(false));assert!(d.write(r));assert!(d.start(true));d.read()}
    #[test]
    fn shared_pcm_controls_gate_each_direction_and_apply_documented_gain() {
        let state=Arc::new(Mutex::new(Es8311State::default()));
        let mut d=Es8311::with_state(state.clone());
        let gain=||{let s=state.lock().unwrap();[s.adc_gain(16),s.dac_gain(16)]};
        assert_eq!(gain(),[0.0,0.0]);
        for (r,v) in [(0,0x80),(1,0xbf),(9,0x0c),(10,0x0c),(0x0d,1),(0x0e,2),(0x12,0),
            (0x14,0x10),(0x16,0),(0x17,0xbf),(0x32,0xbf)] {write(&mut d,r,v);}
        assert_eq!(gain(),[1.0,1.0]);
        write(&mut d,0x17,0xb3);write(&mut d,0x32,0xb3);
        assert!((gain()[0]-0.501187).abs()<0.000001);assert_eq!(gain()[0],gain()[1]);
        write(&mut d,0x14,0x12);assert!((gain()[0]-1.0).abs()<0.000001);
        write(&mut d,0x16,1);assert!((gain()[0]-1.995262).abs()<0.000001);
        write(&mut d,0x16,0x11);assert!((gain()[0]+1.995262).abs()<0.000001);
        let adc=gain()[0];let dac=gain()[1];
        for (r,v,expected) in [(10,0x4c,[0.0,dac]),(9,0x4c,[adc,0.0]),(0x0e,0x62,[0.0,dac]),
            (0x12,2,[adc,0.0]),(1,0xb5,[0.0,dac]),(1,0xba,[adc,0.0]),(0,0,[0.0,0.0]),
            (0,0x9f,[0.0,0.0]),(0x0d,0xfc,[0.0,0.0]),(0x18,0x80,[0.0,dac]),
            (0x31,0x60,[adc,0.0]),(0x34,0x80,[adc,0.0])] {
            let old=read(&mut d,r);write(&mut d,r,v);assert_eq!(gain(),expected,"reg {r:#x}");write(&mut d,r,old);
        }
        // A second physical control-port adapter and register reset see the same state.
        let mut other=Es8311::with_state(state.clone());assert_eq!(read(&mut other,0x17),0xb3);
        write(&mut other,0xfa,1);assert_eq!(gain(),[0.0,0.0]);
    }
    #[test]
    fn identity_configuration_readback_and_two_different_resets() {
        let mut d=Es8311::new();assert_eq!([read(&mut d,0xfd),read(&mut d,0xfe),read(&mut d,0xff)],[0x83,0x11,0]);
        assert_eq!([read(&mut d,0),read(&mut d,0x0d),read(&mut d,0x1c)],[0x1f,0xfc,0x4c]);
        write(&mut d,0x32,0xbf);write(&mut d,0x0d,1);write(&mut d,0,0x1f);
        assert_eq!(read(&mut d,0x32),0xbf,"digital reset excludes control registers");
        write(&mut d,0,0x80);assert_eq!(read(&mut d,0),0x80);
        write(&mut d,0xfa,1);assert_eq!(read(&mut d,0xfa),1);assert_eq!(read(&mut d,0x32),0);assert_eq!(read(&mut d,0x0d),0xfc);
        write(&mut d,0x32,0xff);assert_eq!(read(&mut d,0x32),0,"register reset held");
        write(&mut d,0xfa,0);write(&mut d,0x32,0x80);assert_eq!(read(&mut d,0x32),0x80);
        write(&mut d,0xfd,0);assert_eq!(read(&mut d,0xfd),0x83);
    }
    #[test]
    fn unsupported_status_and_bursts_do_not_claim_success() {
        let mut d=Es8311::new();assert!(d.start(false));assert!(!d.write(0xfc));assert!(!d.start(true));assert_eq!(d.read(),0xff);
        // A new address phase is accepted independently of the previous unsupported pointer.
        assert!(d.start(false));assert!(!d.write(0x46));
        write(&mut d,3,0x20);assert!(!d.write(0x30));assert_eq!(read(&mut d,4),0x10);
    }
}
