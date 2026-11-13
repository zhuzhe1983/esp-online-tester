//! Functional S31 mode0 counter/interrupt control, distinct from analog mode1.
//! Fixed ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! lp_analog_peri_reg.h,brownout_ll.h,brownout_hal.c,psdet_types.h.
//! Input is an explicitly qualified comparator sample and detector-domain edges;
//! no voltage threshold conversion,oscillator rate or physical response timing.
use emu_core::Fault;
const BASE: u32 = 0x2070_2000;
const IRQ: u32 = 1 << 31;
pub struct Brownout { control: u32, source: u32, count: u64, raw: [u32; 2], ena: [u32; 2] }
impl Default for Brownout {
    fn default() -> Self { Self { control: 0x0ffc_0100, source: 63, count: 0, raw: [0; 2], ena: [0; 2] } }
}
impl Brownout {
    pub fn is_mmio(address: u32) -> bool { matches!(address.wrapping_sub(BASE), 0 | 8 | 0x20 | 0x24 | 0x28 | 0x2c | 0x30 | 0x34 | 0x38 | 0x3c) }
    pub fn irq(&self) -> bool { self.raw[0] & self.ena[0] != 0 }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        match address.wrapping_sub(BASE) {
            0 => Ok(self.control), 8 => Ok(self.source),
            off @ (0x20 | 0x24 | 0x28 | 0x2c | 0x30 | 0x34 | 0x38 | 0x3c) => {
                let bank = usize::from(off >= 0x30);
                Ok(match off & 15 { 0 => self.raw[bank], 4 => self.raw[bank] & self.ena[bank], 8 => self.ena[bank], _ => 0 })
            }
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        match address.wrapping_sub(BASE) {
            0 => { self.control = value & 0xffff_ffc0; if value & (1 << 28) != 0 { self.count = 0; } }
            8 => self.source = value & 63,
            off @ (0x20 | 0x24 | 0x28 | 0x2c | 0x30 | 0x34 | 0x38 | 0x3c) => {
                let bank = usize::from(off >= 0x30);
                match off & 15 { 8 => self.ena[bank] = value & IRQ, 12 => self.raw[bank] &= !(value & IRQ), _ => {} }
            }
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    /// A bounded functional qualification counter. No progress on reads,zero
    /// elapsed input,disabled detection or asserted counter-clear. Returned true
    /// is the documented digital-system reset request,not a silent host reset.
    pub(crate) fn advance(&mut self, tripped: bool, ticks: u64) -> Result<bool, Fault> {
        if ticks == 0 { return Ok(false); }
        if !tripped || self.control & (1 << 29 | 1 << 28) != 1 << 29 { self.count = 0; return Ok(false); }
        // These policies are readable configuration,not implemented analog/RF
        // shutdown engines. A real input requiring them must not report success.
        if self.control & 0xc0 != 0 || self.control & (IRQ | 1 << 30) == IRQ { return Err(Fault::Prohibited); }
        self.count = self.count.saturating_add(ticks).min(1024);
        if self.count >= u64::from(self.control >> 8 & 1023) { self.raw[0] |= IRQ; self.raw[1] |= IRQ; }
        Ok(self.control & IRQ != 0 && self.count >= u64::from(self.control >> 18 & 1023))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_detector_edges_and_independent_interrupt_banks() {
        let mut b = Brownout::default(); assert_eq!(b.read(BASE), Ok(0x0ffc0100)); assert_eq!(b.read(BASE + 8), Ok(63));
        b.write(BASE, 1 << 29 | 2 << 8 | 4 << 18).unwrap(); b.write(BASE + 0x38, IRQ).unwrap();
        assert_eq!(b.advance(true, 0), Ok(false)); assert_eq!(b.read(BASE + 0x20), Ok(0));
        b.advance(true, 1).unwrap(); assert_eq!(b.read(BASE + 0x20), Ok(0)); b.advance(false, 0).unwrap(); b.advance(true, 1).unwrap();
        assert_eq!(b.read(BASE + 0x20), Ok(IRQ)); assert_eq!(b.read(BASE + 0x34), Ok(IRQ)); assert!(!b.irq());
        b.write(BASE + 0x28, IRQ).unwrap(); assert!(b.irq()); b.write(BASE + 0x20, IRQ).unwrap(); assert!(b.irq());
        b.write(BASE + 0x2c, IRQ).unwrap(); assert!(!b.irq()); assert_eq!(b.read(BASE + 0x30), Ok(IRQ));
        b.advance(true, 1).unwrap(); assert!(b.irq()); b.write(BASE + 0x3c, IRQ).unwrap(); assert_eq!(b.read(BASE + 0x30), Ok(0));
        b.write(BASE, 1 << 29 | 1 << 28 | 2 << 8).unwrap(); b.write(BASE + 0x2c, IRQ).unwrap(); b.advance(true, u64::MAX).unwrap(); assert!(!b.irq());
        b.write(BASE, 1 << 29 | 2 << 8).unwrap(); b.advance(true, 1).unwrap(); assert!(!b.irq()); b.advance(false, 99).unwrap(); b.advance(true, 1).unwrap(); assert!(!b.irq());
    }
    #[test]
    fn system_reset_and_unsupported_actions_remain_distinct() {
        let mut b = Brownout::default(); b.write(BASE, 0xe0000000 | 2 << 8 | 4 << 18).unwrap();
        assert_eq!(b.advance(true, 3), Ok(false)); assert_eq!(b.advance(true, 1), Ok(true));
        for action in [0x40, 0x80, IRQ] {
            let mut b = Brownout::default(); b.write(BASE, 1 << 29 | action).unwrap(); assert_eq!(b.advance(true, 1), Err(Fault::Prohibited));
            assert_eq!(b.read(BASE + 0x20), Ok(0)); assert_eq!(b.count, 0);
        }
        b.write(BASE, 0).unwrap(); assert_eq!(b.advance(true, u64::MAX), Ok(false));
    }
}
