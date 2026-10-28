//! S31 dedicated PSRAM pads, separate from ordinary GPIO IO_MUX.
//! ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! iomux_mspi_pin_{reg,struct}.h and esp_hal_mspi/esp32s31/include/hal/mspi_ll.h.
//! Delay/drive/pull fields are configuration,not analog waveforms or timing-margin proof.
use emu_core::Fault;
pub const BASE: u32 = 0x2058_4000;
pub struct MspiPads { clock: u32, regs: [u32; 11] }
impl Default for MspiPads {
    fn default() -> Self { let mut regs = [0x4000; 11]; regs[8] = 0x20000; Self { clock: 1, regs } }
}
impl MspiPads {
    pub fn is_mmio(address: u32) -> bool { matches!((address & !3).wrapping_sub(BASE), 0 | 0x1c..=0x44) }
    fn index(address: u32) -> Result<usize, Fault> {
        if address & 3 != 0 { return Err(Fault::Misaligned); }
        let off = address.wrapping_sub(BASE);
        if !(0x1c..=0x44).contains(&off) { return Err(Fault::Unmapped); }
        Ok(((off - 0x1c) / 4) as usize)
    }
    fn hold(index: usize) -> u32 { if index == 8 { 1 << 22 } else { 1 << 15 } }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        if address == BASE { return Ok(self.clock); }
        Ok(self.regs[Self::index(address)?])
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address == BASE { self.clock = value & 1; return Ok(()); } // automatic gating policy,not an off switch
        let i = Self::index(address)?;
        // Configuration remains readable/writable while HOLD is asserted. The
        // model deliberately refuses transfers instead of inventing the frozen
        // physical pin waveform or a sampling order for simultaneous writes.
        self.regs[i] = value & if i == 8 { 0x7fffff } else { 0xffff }; Ok(())
    }
    pub fn held(&self) -> bool { self.regs.iter().enumerate().any(|(i, v)| v & Self::hold(i) != 0) }
    pub fn dqs_requested(&self) -> bool { self.regs[8] & 1 != 0 }
    /// The functional backing-memory path performs real external accesses rather
    /// than cache hits; a frozen transfer cannot be represented as successful data.
    /// DQS is additionally required by a future DDR serial-controller path. Merely
    /// enabling DQS does not attach a die or make a protocol command succeed.
    pub fn check_transfer(&self, ddr: bool) -> Result<(), Fault> {
        if self.held() || ddr && !self.dqs_requested() { Err(Fault::Prohibited) } else { Ok(()) }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dedicated_pads_use_exact_bounds_resets_and_dqs_layout() {
        let mut p = MspiPads::default(); assert_eq!(p.read(BASE), Ok(1)); assert_eq!(p.read(BASE + 0x18), Err(Fault::Unmapped));
        for i in 0..11 { assert_eq!(p.read(BASE + 0x1c + i * 4), Ok(if i == 8 { 0x20000 } else { 0x4000 })); }
        assert_eq!(p.read(BASE + 0x45), Err(Fault::Misaligned)); assert_eq!(p.write(BASE + 0x48, 0), Err(Fault::Unmapped));
        assert_eq!(p.check_transfer(false), Ok(())); assert_eq!(p.check_transfer(true), Err(Fault::Prohibited));
        p.write(BASE, 0).unwrap(); p.write(BASE + 0x3c, 0x20001).unwrap(); assert_eq!(p.check_transfer(true), Ok(()));
        p.write(BASE + 0x1c, 0xffff4000).unwrap(); assert_eq!(p.read(BASE + 0x1c), Ok(0x4000));
    }
    #[test]
    fn held_pads_keep_configuration_readback_but_refuse_dynamic_transfers() {
        let mut p = MspiPads::default(); p.write(BASE + 0x3c, 0x420001).unwrap();
        p.write(BASE + 0x3c, 0x420000).unwrap(); assert!(!p.dqs_requested()); assert_eq!(p.check_transfer(false), Err(Fault::Prohibited));
        p.write(BASE + 0x3c, 0x20000).unwrap(); assert!(!p.dqs_requested()); assert_eq!(p.check_transfer(false), Ok(()));
        for i in 0..11 {
            let a = BASE + 0x1c + i * 4; let v = p.read(a).unwrap(); p.write(a, v | MspiPads::hold(i as usize)).unwrap();
            assert!(p.held()); assert_eq!(p.check_transfer(false), Err(Fault::Prohibited));
            p.write(a, v).unwrap(); assert!(!p.held());
        }
    }
}
