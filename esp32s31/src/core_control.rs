//! S31 HP-core clock/reset and software-stall controls. ESP-IDF v6.1
//! fff9895c82d744c7237be8847347bdd1b07c6643 register/soc/{hp_sys_clkrst,
//! hp_system,pmu,lp_system}_reg.h and hal/esp32s31/cpu_utility_ll.h.
//! Reset vectors are executed by the real ROM; BOOT_ADDR is only a stored word.
use emu_core::Fault;
pub const CTRL0: u32 = 0x2058_701c;
pub const CTRL1: u32 = CTRL0 + 4;
pub const STALL: u32 = 0x2070_41ec;
pub const STALLED: u32 = 0x2058_6064;
pub const BOOT1: u32 = 0x2070_0164;
/// hp_system_reg.h HP_SYSTEM_CPU_INT_FROM_CPU_0..3 (+0x10..+0x1c), bit0 R/W, reset 0.
/// Each latch drives interrupts.h ETS_CPU_INTR_FROM_CPU_n_SOURCE (65+n) while set.
pub const FROM_CPU0: u32 = 0x2058_6010;
pub struct CoreControl {
    controls: [u32; 2], stall: u32, stalled: [bool; 2], boot1: u32,
    reset_pending: [bool; 2], pub from_cpu: u32, pub resets: [u64; 2], pub last_nonzero_boot1: Option<u32>,
}
impl Default for CoreControl {
    fn default() -> Self { Self { controls: [3, 4], stall: 0, stalled: [false; 2], boot1: 0, reset_pending: [false; 2], from_cpu: 0, resets: [0; 2], last_nonzero_boot1: None } }
}
impl CoreControl {
    pub fn is_mmio(address: u32) -> bool { matches!(address & !3, CTRL0 | CTRL1 | STALL | STALLED | BOOT1) || (FROM_CPU0..FROM_CPU0 + 16).contains(&(address & !3)) }
    pub fn from_cpu_irqs(&self) -> [bool; 4] { std::array::from_fn(|n| self.from_cpu >> n & 1 != 0) }
    pub fn running(&self, core: usize) -> bool { self.controls[core] & 6 == 2 && !self.stalled[core] }
    pub fn stalled(&self) -> [bool; 2] { self.stalled }
    pub fn clic_on(&self, core: usize) -> bool { self.controls[core] & 5 == 1 }
    pub fn take_resets(&mut self) -> [bool; 2] { std::mem::take(&mut self.reset_pending) }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        match address {
            CTRL0 => Ok(self.controls[0]), CTRL1 => Ok(self.controls[1]), STALL => Ok(self.stall),
            a if (FROM_CPU0..FROM_CPU0 + 16).contains(&a) => Ok(self.from_cpu >> ((a - FROM_CPU0) / 4) & 1),
            STALLED => Ok(u32::from(self.stalled[0]) | u32::from(self.stalled[1]) << 1), BOOT1 => Ok(self.boot1),
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        match address {
            CTRL0 | CTRL1 => {
                let core = usize::from(address == CTRL1);
                // FORCE_NORST's priority against reset sources is not specified in the
                // public header. Do not invent it; the normal IDF sequence leaves it zero.
                if value & 8 != 0 { return Err(Fault::Prohibited); }
                if value & 4 != 0 && self.controls[core] & 4 == 0 {
                    self.reset_pending[core] = true; self.resets[core] = self.resets[core].wrapping_add(1);
                }
                self.controls[core] = value & 7;
            }
            STALL => {
                let mut state = self.stalled;
                for (core, shift) in [24, 16].into_iter().enumerate() {
                    let code = (value >> shift) & 255;
                    match code {
                        0x86 => state[core] = true, 0xff => state[core] = false,
                        _ if code == (self.stall >> shift) & 255 => (),
                        _ => return Err(Fault::Prohibited), // Unknown command, not a guessed unstall.
                    }
                }
                self.stall = value & 0xffff_0000; self.stalled = state;
            }
            a if (FROM_CPU0..FROM_CPU0 + 16).contains(&a) => { let n = (a - FROM_CPU0) / 4; self.from_cpu = self.from_cpu & !(1 << n) | (value & 1) << n; }
            STALLED => (), // RO; reports the actual functional stall latch, not clock gating.
            BOOT1 => { self.boot1 = value; if value != 0 { self.last_nonzero_boot1 = Some(value); } }
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn core1_needs_clock_and_reset_release_and_stall_is_independent() {
        let mut c = CoreControl::default(); assert!(c.running(0)); assert!(!c.running(1));
        c.write(BOOT1, 0x2f001002).unwrap(); assert!(!c.running(1));
        for v in [6, 7, 1] { c.write(CTRL1, v).unwrap(); assert!(!c.running(1)); }
        c.write(CTRL1, 3).unwrap(); assert!(c.running(1)); assert!(c.clic_on(1));
        c.write(STALL, 0x00860000).unwrap(); assert!(!c.running(1)); assert_eq!(c.read(STALLED), Ok(2));
        c.write(STALLED, 0).unwrap(); assert_eq!(c.read(STALLED), Ok(2));
        assert_eq!(c.write(STALL, 0x00120000), Err(Fault::Prohibited)); assert!(!c.running(1));
        c.write(STALL, 0x00ff0000).unwrap(); assert!(c.running(1));
        c.write(CTRL1, 2).unwrap(); assert!(c.running(1)); assert!(!c.clic_on(1));
        c.write(CTRL1, 7).unwrap(); assert!(!c.running(1)); assert_eq!(c.take_resets(), [false, true]);
        assert_eq!(c.take_resets(), [false, false]); assert_eq!(c.read(BOOT1), Ok(0x2f001002));
        assert_eq!(c.write(CTRL1, 15), Err(Fault::Prohibited)); assert_eq!(c.read(CTRL1), Ok(7));
    }
}
