//! S31 eFuse read controller using explicit decoded input; no guessed chip identity or programming.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/efuse_reg.h and hal/esp32s31/include/hal/efuse_ll.h.
use emu_core::Fault;
pub const BASE: u32 = 0x2071_5000;
pub const SHADOW_WORDS: usize = 79; // RD_WR_DIS 0x2c through RD_SYS_PART2_DATA7 0x164.

/// Already-decoded read image, not a raw OTP/ECC dump. A caller must identify its provenance.
/// Error words and block0 valid-bit count are explicit inputs, not fabricated ECC success.
#[derive(Clone)]
pub struct ReadImage { pub words: [u32; SHADOW_WORDS], pub errors: [u32; 10], pub block0_valid_bits: u16 }
impl ReadImage {
    /// Synthetic development input only; it is never selected automatically for a product.
    pub fn blank_development() -> Self { Self { words: [0; SHADOW_WORDS], errors: [0; 10], block0_valid_bits: 0 } }
}
pub struct Efuse {
    image: Option<ReadImage>, shadow: [u32; SHADOW_WORDS], errors: [u32; 10], pgm: [u32; 11],
    clk: u32, conf: u32, status: u32, cmd: u32, int_raw: u32, int_ena: u32, date: u32,
    dac: u32, rd_tim: u32, wr_tim1: u32, wr_tim2: u32, bypass: u32,
}
impl Default for Efuse {
    fn default() -> Self {
        Self { image: None, shadow: [0; SHADOW_WORDS], errors: [0; 10], pgm: [0; 11], clk: 2, conf: 0, status: 0,
            cmd: 0, int_raw: 0, int_ena: 0, date: 38834580, dac: 24 | 255 << 9,
            rd_tim: 1 | 2 << 8 | 1 << 16 | 27 << 24, wr_tim1: 1 | 14746 << 8 | 1 << 24,
            wr_tim2: 480 | 240 << 16, bypass: 1 << 13 }
    }
}
impl Efuse {
    pub fn set_read_image(&mut self, image: ReadImage) -> Result<(), String> {
        if image.block0_valid_bits > 1023 { return Err("S31 eFuse block0 valid-bit count exceeds 10-bit field".into()); }
        if image.words[1] & 0x7f != 0 { return Err("S31 eFuse read-protected block images are not supported".into()); }
        self.image = Some(image); Ok(())
    }
    pub fn has_read_image(&self) -> bool { self.image.is_some() }
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
    // v6.1 register field is one bit at19, although its prose lists the older four values.
    pub fn wdt_delay_selector(&self) -> u8 { ((self.shadow[2] >> 19) & 1) as u8 }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        match off {
            0..=0x28 => Ok(self.pgm[off as usize / 4]),
            0x2c..=0x164 => Ok(self.shadow[(off as usize - 0x2c) / 4]),
            0x168..=0x18c => Ok(self.errors[(off as usize - 0x168) / 4]),
            0x190 => Ok(self.date), 0x1c8 => Ok(self.clk), 0x1cc => Ok(self.conf),
            0x1d4 => Ok(self.status), 0x1d8 => Ok(self.cmd), 0x1dc => Ok(self.int_raw),
            0x1e0 => Ok(self.int_raw & self.int_ena), 0x1e4 => Ok(self.int_ena), 0x1e8 => Ok(0),
            0x1ec => Ok(self.dac), 0x1f0 => Ok(self.rd_tim), 0x1f4 => Ok(self.wr_tim1),
            0x1f8 => Ok(self.wr_tim2), 0x1fc => Ok(self.bypass), _ => Err(Fault::Unmapped),
        }
    }
    fn command(&mut self, value: u32) -> Result<(), Fault> {
        if value & 2 != 0 { return Err(Fault::Prohibited); } // OTP programming/ECC/security not modeled.
        if value & 1 == 0 || self.conf != 0x5aa5 { self.cmd = value & 0x3c; return Ok(()); }
        if self.clk & 1 != 0 { return Err(Fault::Prohibited); } // SRAM forced down; no invented success.
        let image = self.image.as_ref().ok_or(Fault::Unmapped)?;
        // Functional zero-latency read, not a physical eFuse timing/state-machine claim.
        // The operation actually transfers the caller's decoded data/error image to RO shadows.
        self.shadow = image.words; self.errors = image.errors;
        self.status = 1 | (image.block0_valid_bits as u32) << 10;
        self.cmd = value & 0x3c; self.int_raw |= 1; Ok(())
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        match off {
            0..=0x28 => self.pgm[off as usize / 4] = value,
            0x2c..=0x18c | 0x1d4 | 0x1dc | 0x1e0 => {}, // RO shadows/errors/status.
            0x190 => self.date = value & 0x0fff_ffff, 0x1c8 => self.clk = value & 0x10007,
            0x1cc => self.conf = value & 0xffff, 0x1d8 => return self.command(value),
            0x1e4 => self.int_ena = value & 3, 0x1e8 => self.int_raw &= !(value & 3),
            0x1ec => self.dac = value & 0x3ffff, 0x1f0 => self.rd_tim = value,
            0x1f4 => self.wr_tim1 = value, 0x1f8 => self.wr_tim2 = value,
            0x1fc => {
                if value & 0xfff != 0 { return Err(Fault::Prohibited); } // No raw-RS bypass input path.
                self.bypass = value & 0x1fe000; // UPDATE is WT; no programming operation is executed.
            }
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_needs_explicit_image_and_real_command_then_shadows_are_read_only() {
        let mut e = Efuse::default(); assert!(!e.has_read_image());
        e.write(0x1d8, 1).unwrap(); assert_eq!(e.read(0x1dc), Ok(0)); // Wrong opcode has no effect.
        e.write(0x1cc, 0x5aa5).unwrap(); assert_eq!(e.write(0x1d8, 1), Err(Fault::Unmapped));
        let mut image = ReadImage::blank_development(); image.words[2] = 0x12345678;
        image.errors[3] = 0x01020304; image.block0_valid_bits = 177; e.set_read_image(image).unwrap();
        assert_eq!(e.read(0x34), Ok(0)); e.write(0x1e4, 1).unwrap(); e.write(0x1d8, 1).unwrap();
        assert_eq!(e.read(0x34), Ok(0x12345678)); assert_eq!(e.read(0x174), Ok(0x01020304));
        assert_eq!(e.read(0x1d4), Ok(1 | 177 << 10)); assert_eq!(e.read(0x1d8), Ok(0)); assert!(e.irq());
        e.write(0x34, 0).unwrap(); assert_eq!(e.read(0x34), Ok(0x12345678));
        e.write(0x1e8, 1).unwrap(); assert!(!e.irq());
    }
    #[test]
    fn power_down_programming_and_unknown_fields_do_not_report_success() {
        let mut e = Efuse::default(); e.set_read_image(ReadImage::blank_development()).unwrap();
        e.write(0x1cc, 0x5aa5).unwrap(); e.write(0x1c8, 1).unwrap();
        assert_eq!(e.write(0x1d8, 1), Err(Fault::Prohibited)); assert_eq!(e.read(0x1dc), Ok(0));
        e.write(0x1c8, 2).unwrap(); assert_eq!(e.write(0x1d8, 2), Err(Fault::Prohibited));
        assert_eq!(e.write(0x1fc, 1), Err(Fault::Prohibited)); assert_eq!(e.read(0x1d0), Err(Fault::Unmapped));
        assert_eq!(e.read(u32::MAX), Err(Fault::Misaligned)); assert_eq!(e.read(0x200), Err(Fault::Unmapped));
        assert_eq!(e.read(0x1f8), Ok(480 | 240 << 16));
    }
}
