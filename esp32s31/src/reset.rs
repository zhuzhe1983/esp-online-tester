//! S31 HP-core reset gates and software reset requests.
//! ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643: lp_clkrst_reg.h
//! and hal/esp32s31/include/hal/cpu_utility_ll.h. Requests stop the development
//! runner; LP-triggered reset-domain reconstruction and pulse timing
//! are not implemented. Header vector prose still contains P4 addresses, so
//! non-default vectors are rejected rather than mapped to those addresses.
use emu_core::Fault;
pub const BASE: u32 = 0x2070_103c;
const RESET: u32 = 0x4001_0000;
const WDT_GATE: u32 = 1 << 19;
const SW_RESET: u32 = 1 << 20;
const LOCKUP_GATE: u32 = 1 << 31;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause { Software, Lockup } // reset_reasons.h: CPU_SW=0x0c, CPU_LOCKUP=0x1a.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuReset { pub cores: [Option<Cause>; 2] }
pub struct ResetControl { controls: [u32; 2], pub request: Option<CpuReset> }
impl Default for ResetControl { fn default() -> Self { Self { controls: [RESET; 2], request: None } } }
impl ResetControl {
    pub fn is_mmio(address: u32) -> bool { (BASE..BASE + 8).contains(&address) }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        match address.checked_sub(BASE) { Some(0) => Ok(self.controls[0]), Some(4) => Ok(self.controls[1]), _ => Err(Fault::Unmapped) }
    }
    pub fn watchdog_targets(&self) -> [bool; 2] { self.controls.map(|c| c & WDT_GATE != 0) }
    fn request(&mut self, core: usize, cause: Cause) {
        let mut r = self.request.unwrap_or(CpuReset { cores: [None, None] });
        // Pending requests are diagnostic stop points; no reset priority/sequencing
        // is inferred after the first request for a core.
        r.cores[core].get_or_insert(cause); self.request = Some(r);
    }
    /// Explicit already-qualified CPU lockup event. The RV32 core does not yet
    /// classify nested traps as S31 lockup; ordinary exceptions never assert it.
    pub fn lockup_input(&mut self, core: usize, active: bool) -> Result<(), Fault> {
        let control = *self.controls.get(core).ok_or(Fault::Unmapped)?;
        if active && control & LOCKUP_GATE != 0 { self.request(core, Cause::Lockup); }
        Ok(())
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        let core = match address.checked_sub(BASE) { Some(0) => 0, Some(4) => 1, _ => return Err(Fault::Unmapped) };
        // Stall/debug and alternate reset vectors are explicit gaps.
        // Retain reset-length programming but do not claim a physical pulse width.
        // Guest RMW (esp_cpu_reset → hpcoreN_sw_reset=1) keeps STAT_VECTOR_SEL=1 and
        // may also set LOCKUP_RESET_EN; only the low/OCD-halt bits in 0x2000_ffff are rejected.
        if value & 0x6000_ffff != 1 << 30 { return Err(Fault::Prohibited); }
        self.controls[core] = value & 0xc00f_0000;
        if value & SW_RESET != 0 { self.request(core, Cause::Software); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn core_gates_are_independent_and_software_reset_is_a_write_trigger() {
        let mut r = ResetControl::default(); assert_eq!(r.read(BASE), Ok(RESET)); assert_eq!(r.watchdog_targets(), [false, false]);
        r.write(BASE, RESET | WDT_GATE).unwrap(); assert_eq!(r.watchdog_targets(), [true, false]);
        r.write(BASE + 4, 0x4007_0000 | WDT_GATE).unwrap(); assert_eq!(r.watchdog_targets(), [true, true]);
        r.write(BASE, RESET | SW_RESET).unwrap(); assert_eq!(r.read(BASE), Ok(RESET));
        assert_eq!(r.request, Some(CpuReset { cores: [Some(Cause::Software), None] }));
        r.write(BASE + 4, RESET | SW_RESET).unwrap(); assert_eq!(r.request, Some(CpuReset { cores: [Some(Cause::Software), Some(Cause::Software)] }));
        for value in [0, RESET | 1, RESET | (1 << 15), RESET | (1 << 29)] {
            assert_eq!(r.write(BASE, value), Err(Fault::Prohibited)); assert_eq!(r.read(BASE), Ok(RESET));
        }
        assert_eq!(r.read(BASE + 8), Err(Fault::Unmapped)); assert_eq!(r.write(BASE + 1, RESET), Err(Fault::Unmapped));
    }
    #[test]
    fn lockup_requires_both_a_qualified_event_and_its_own_core_gate() {
        let mut r = ResetControl::default(); r.lockup_input(0, true).unwrap(); assert_eq!(r.request, None);
        r.write(BASE, RESET | LOCKUP_GATE).unwrap(); assert_eq!(r.request, None);
        r.lockup_input(0, false).unwrap(); r.lockup_input(1, true).unwrap(); assert_eq!(r.request, None);
        r.lockup_input(0, true).unwrap(); assert_eq!(r.request, Some(CpuReset { cores: [Some(Cause::Lockup), None] }));
        assert_eq!(r.lockup_input(usize::MAX, true), Err(Fault::Unmapped));
    }
}
