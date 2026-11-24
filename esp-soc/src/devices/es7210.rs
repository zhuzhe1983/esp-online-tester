//! ES7210 digital control-port subset; no microphone samples or analog/clock state machine.
//! Everest ES7210 Rev21.0 (January 2023), sections 5 and 7 pp5,12-25, official copy:
//! https://files.waveshare.com/wiki/common/ES7210_DS.pdf
//! RST_DIG excludes control registers; RST_REGS resets them except its own latch.
//! Only documented single-register transfers are supported. EQ coefficient defaults,
//! reserved registers and dynamic status are not guessed from driver write sequences.
use esp_periph::i2c::I2cDevice;

pub struct Es7210 {regs:[u8;0x4d],ptr:u8,first:bool,written:bool}
impl Default for Es7210 {fn default()->Self {Self::new()}}
impl Es7210 {
    pub fn new()->Self {let mut d=Self{regs:[0;0x4d],ptr:0,first:true,written:false};d.reset_registers(false);d}
    fn reset_registers(&mut self,held:bool) {
        self.regs.fill(0);
        // Use the register headings in section 7. The contents page disagrees
        // for 20h; its actual register heading on page 19 specifies 26h.
        for (r,v) in [(0,0x32),(1,0x20),(2,2),(3,4),(4,1),(7,0x20),(8,0x10),
            (9,0x40),(0x0a,0x40),(0x0d,9),(0x18,0xf7),(0x19,0xf7),
            (0x1b,0xbf),(0x1c,0xbf),(0x1d,0xbf),(0x1e,0xbf),
            (0x20,0x26),(0x21,0x26),(0x22,6),(0x23,0x26),
            (0x3d,0x72),(0x3e,0x10),(0x40,0x80),(0x41,0x71),(0x42,0x71),(0x4b,0xff),(0x4c,0xff)] {self.regs[r]=v;}
        if held {self.regs[0]|=4;}
    }
    fn supported(r:u8)->bool {matches!(r,0..=0x0a|0x0c..=0x0d|0x10..=0x1e|0x20..=0x23|0x3d..=0x4c)}
}
impl I2cDevice for Es7210 {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;self.written=false;true}else{Self::supported(self.ptr)}}
    fn write(&mut self,b:u8)->bool {
        if self.first {self.ptr=b;self.first=false;return Self::supported(b);}
        if self.written||!Self::supported(self.ptr) {return false;}
        self.written=true;
        if self.ptr==0 {
            if b&4!=0 {self.reset_registers(true);} else {self.regs[0]=b;}
        } else if !(0x3d..=0x3f).contains(&self.ptr)&&self.regs[0]&4==0 {self.regs[self.ptr as usize]=b;}
        true
    }
    fn read(&mut self)->u8 {if Self::supported(self.ptr) {self.regs[self.ptr as usize]}else{0xff}}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut Es7210,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn read(d:&mut Es7210,r:u8)->u8 {assert!(d.start(false));assert!(d.write(r));assert!(d.start(true));d.read()}
    #[test]
    fn published_identity_is_read_only_and_digital_reset_keeps_configuration() {
        let mut d=Es7210::new();
        assert_eq!([read(&mut d,0x3d),read(&mut d,0x3e),read(&mut d,0x3f)],[0x72,0x10,0]);
        write(&mut d,0x3d,0);assert_eq!(read(&mut d,0x3d),0x72);
        write(&mut d,0x43,0x18);write(&mut d,0,0x32);write(&mut d,0,0x12);
        assert_eq!(read(&mut d,0x43),0x18,"digital reset excludes control registers");
        assert_eq!(read(&mut d,0),0x12);
    }
    #[test]
    fn register_reset_is_held_until_the_guest_releases_it() {
        let mut d=Es7210::new();write(&mut d,0x43,0x18);write(&mut d,0x4b,0);write(&mut d,0,0xff);
        assert_eq!(read(&mut d,0),0x36,"only RST_REGS survives its reset");
        assert_eq!(read(&mut d,0x43),0);assert_eq!(read(&mut d,0x4b),0xff);
        write(&mut d,0x43,0x18);assert_eq!(read(&mut d,0x43),0);
        write(&mut d,0,0x32);write(&mut d,0x43,0x18);assert_eq!(read(&mut d,0x43),0x18);
        assert_eq!([read(&mut d,0x20),read(&mut d,0x21),read(&mut d,0x22),read(&mut d,0x23)],[0x26,0x26,6,0x26]);
    }
    #[test]
    fn missing_status_reserved_addresses_and_bursts_are_not_acknowledged() {
        let mut d=Es7210::new();
        for r in [0x0b,0x0e,0x0f,0x1f,0x24,0x38,0x3c,0x4d,0xff] {
            assert!(d.start(false));assert!(!d.write(r));assert!(!d.start(true));assert_eq!(d.read(),0xff);assert!(!d.write(0));
        }
        write(&mut d,0x43,0x18);assert!(!d.write(0x19));assert_eq!(read(&mut d,0x44),0);
    }
}
