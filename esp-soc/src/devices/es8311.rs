//! ES8311 control-port subset. No ADC/DAC samples, clock-state completion or sound.
//! Everest ES8311 Datasheet Rev6.0 (May2019), section5 and section8 pp14-29;
//! official Waveshare retained copy at ESP32-C6-LCD-0.85 b73e5629985866bce242931464f24154716fcaeb:
//! Example/Arduino/libraries/ESP32-audioI2S-master/examples/ES8311/ES8311_DS.pdf.
//! The documented single-register I2C transactions are modeled. Burst transfers,
//! FLAG_CSM_CHIP/automute timing and undefined register addresses remain unsupported.
//! RESET00h controls digital blocks but explicitly excludes the control port;
//! FAh.INI_REG resets the configuration registers except FAh itself.
use esp_periph::i2c::I2cDevice;

pub struct Es8311 {regs:[u8;256],ptr:u8,first:bool,written:bool}
impl Default for Es8311 {fn default()->Self {Self::new()}}
impl Es8311 {
    pub fn new()->Self {let mut d=Self{regs:[0;256],ptr:0,first:true,written:false};d.reset_registers();d}
    fn reset_registers(&mut self) {
        let fa=self.regs[0xfa];self.regs.fill(0);self.regs[0xfa]=fa;
        // Section8 register heading defaults; reserved-bit description discrepancies
        // do not override the explicit whole-register defaults.
        for (r,v) in [(0,0x1f),(3,0x10),(4,0x10),(6,3),(8,0xff),(0x0c,0x20),
            (0x0d,0xfc),(0x0e,0x6a),(0x10,0x13),(0x11,0x7c),(0x12,2),(0x13,0x40),
            (0x14,0x10),(0x16,4),(0x1b,0x0c),(0x1c,0x4c),(0x37,8),(0xfd,0x83),(0xfe,0x11)] {self.regs[r]=v;}
    }
    fn supported(r:u8)->bool {r<=0x45||r==0xfa||r>=0xfd}
}
impl I2cDevice for Es8311 {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;self.written=false;true}else{Self::supported(self.ptr)}}
    fn write(&mut self,b:u8)->bool {
        if self.first {self.ptr=b;self.first=false;return Self::supported(b);}
        if self.written||!Self::supported(self.ptr) {return false;}
        self.written=true;
        if self.ptr==0xfa {self.regs[0xfa]=b&3;if b&1!=0 {self.reset_registers();}}
        else if self.ptr<=0x45&&self.regs[0xfa]&1==0 {self.regs[self.ptr as usize]=b;}
        // FDh/FEh/FFh are read-only published chip identifiers, not per-unit IDs.
        true
    }
    fn read(&mut self)->u8 {if Self::supported(self.ptr) {self.regs[self.ptr as usize]}else{0xff}}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Es8311,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn read(d:&mut Es8311,r:u8)->u8 {assert!(d.start(false));assert!(d.write(r));assert!(d.start(true));d.read()}
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
