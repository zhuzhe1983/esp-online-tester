//! S31 clocked PC/SP, stack-boundary and exception-history capture from actual guest execution.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/{bus_monitor,hp_sys_clkrst}_reg.h and
//! hal/esp32s31/include/hal/assist_debug_ll.h. No debugger, region watchpoint,
//! memory-stall timeout or automatic lockup/reset-domain sequencing is implemented.
use emu_core::Fault;
pub const BASE: u32 = 0x2d00_2000;
pub const CLOCK: u32 = 0x2058_7048;
const SP_MIN: u32 = 1 << 8;
const SP_MAX: u32 = 1 << 9;
const TRACE: u32 = 1 << 12;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct Exception { cause: u32, tval: u32, pc: u32, privilege: u32 }
struct Core { monitor: u32, raw: u32, ena: u32, min: u32, max: u32, spill_pc: u32, record: u32, pc: u32, sp: u32, last_pc: u32, exceptions: [Exception; 2] }
impl Default for Core {
    fn default() -> Self { Self { monitor: 0, raw: 0, ena: 0, min: 0, max: u32::MAX, spill_pc: 0, record: 0, pc: 0, sp: 0, last_pc: 0, exceptions: [Exception::default(); 2] } }
}
pub struct BusMonitor { cores: [Core; 2], clock: u32, auto_gate: bool }
impl Default for BusMonitor {
    fn default() -> Self { Self { cores: [Core::default(), Core::default()], clock: 0, auto_gate: true } }
}
impl BusMonitor {
    fn enabled(&self, core: usize) -> bool { (self.clock >> (core * 4)) & 5 == 1 }
    pub fn clock_read(&self) -> u32 { self.clock }
    pub fn clock_write(&mut self, value: u32) -> Result<(), Fault> {
        // FORCE_NORST interacts with global reset domains which are not implemented.
        if value & 0x88 != 0 { return Err(Fault::Prohibited); }
        self.clock = value & 0xff;
        for core in 0..2 { if value & (4 << (core * 4)) != 0 { self.cores[core] = Core::default(); } }
        Ok(())
    }
    pub fn irq(&self) -> bool { (0..2).any(|i| self.enabled(i) && self.cores[i].raw & self.cores[i].ena != 0) }
    pub fn observe(&mut self, pc: u32, sp: u32) { self.observe_core(0, pc, sp); }
    /// Sample only the hart actually selected by the functional scheduler.
    pub fn observe_core(&mut self, core: usize, pc: u32, sp: u32) {
        if core >= 2 || !self.enabled(core) { return; }
        let c = &mut self.cores[core];
        if c.record & 3 == 3 { c.pc = pc; c.sp = sp; }
        let raw = (if sp < c.min { SP_MIN } else { 0 } | if sp > c.max { SP_MAX } else { 0 }) & c.monitor;
        if raw != 0 { c.raw |= raw; c.spill_pc = pc; }
    }
    /// Fixed LL labels trace slot0 the latest trap and slot1 the previous trap. Preserve
    /// real exception metadata; the runner still stops at each exception. This does not
    /// claim hardware detection of a recursive-handler lockup or an automatic reset.
    pub fn exception(&mut self, cause: u32, tval: u32, pc: u32) { self.exception_core(0, cause, tval, pc); }
    pub fn exception_core(&mut self, core: usize, cause: u32, tval: u32, pc: u32) {
        if core >= 2 || !self.enabled(core) { return; }
        let c = &mut self.cores[core]; c.last_pc = pc;
        if c.monitor & TRACE != 0 {
            c.exceptions[1] = c.exceptions[0]; c.exceptions[0] = Exception { cause: cause & 0xfff, tval, pc, privilege: 3 };
        }
    }
    fn local(off: u32) -> Option<(usize, u32)> { if off < 0x110 { Some(((off / 0x88) as usize, off % 0x88)) } else { None } }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        // This bit enables automatic register-clock gating, not an APB off switch.
        if off == 0x114 { return Ok(self.auto_gate as u32); }
        let (i, off) = Self::local(off).ok_or(Fault::Unmapped)?; let c = &self.cores[i];
        match off {
            0 => Ok(c.monitor), 4 => Ok(c.raw), 8 => Ok(c.ena), 0xc => Ok(0),
            0x38 => Ok(c.min), 0x3c => Ok(c.max), 0x40 => Ok(c.spill_pc),
            0x44 => Ok(c.record), 0x48 => Ok(c.pc), 0x4c => Ok(c.sp), 0x60 => Ok(c.last_pc),
            0x64 => Ok(0), // No active debugger or debug mode.
            0x68..=0x84 => {
                let e = c.exceptions[((off - 0x68) / 16) as usize];
                Ok(match (off - 0x68) % 16 { 0 => e.cause, 4 => e.tval, 8 => e.pc, _ => e.privilege })
            }
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if off == 0x114 { self.auto_gate = value & 1 != 0; return Ok(()); }
        let (i, off) = Self::local(off).ok_or(Fault::Unmapped)?; let c = &mut self.cores[i];
        match off {
            0 => { if value & !(SP_MIN | SP_MAX | TRACE) != 0 { return Err(Fault::Prohibited); } c.monitor = value; }
            8 => { if value & !(SP_MIN | SP_MAX) != 0 { return Err(Fault::Prohibited); } c.ena = value; }
            0xc => c.raw &= !value,
            0x38 => c.min = value, 0x3c => c.max = value, 0x44 => c.record = value & 3,
            4 | 0x40 | 0x48 | 0x4c | 0x60 | 0x64 | 0x68..=0x84 => {}, // Read-only capture/status.
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_uses_guest_values_and_clock_reset_while_other_hart_stays_empty() {
        let mut m = BusMonitor::default(); m.write(0x44, 3).unwrap(); m.observe(0x1234, 0x5678);
        assert_eq!(m.read(0x48), Ok(0)); m.clock_write(0x11).unwrap(); m.observe(0x1234, 0x5678);
        assert_eq!(m.read(0x48), Ok(0x1234)); assert_eq!(m.read(0x4c), Ok(0x5678)); assert_eq!(m.read(0xd0), Ok(0));
        m.clock_write(0x10).unwrap(); m.observe(1, 2); assert_eq!(m.read(0x48), Ok(0x1234));
        m.clock_write(0x15).unwrap(); assert_eq!(m.read(0x48), Ok(0)); assert_eq!(m.read(0x44), Ok(0));
        assert_eq!(m.clock_write(0x89), Err(Fault::Prohibited)); assert_eq!(m.read(0x50), Err(Fault::Unmapped));
    }
    #[test]
    fn stack_bounds_raise_latched_irq_and_real_exceptions_fill_distinct_trace_slots() {
        let mut m = BusMonitor::default(); m.clock_write(1).unwrap(); m.write(0, SP_MIN | SP_MAX | TRACE).unwrap(); m.write(8, SP_MIN | SP_MAX).unwrap();
        m.write(0x38, 100).unwrap(); m.write(0x3c, 200).unwrap(); m.observe(10, 100); m.observe(12, 200); assert!(!m.irq());
        m.observe(14, 99); assert!(m.irq()); assert_eq!(m.read(0x40), Ok(14)); m.write(0xc, SP_MIN).unwrap(); assert!(!m.irq());
        m.observe(16, 201); assert_eq!(m.read(4), Ok(SP_MAX)); m.clock_write(0).unwrap(); assert!(!m.irq()); m.clock_write(1).unwrap(); assert!(m.irq());
        m.exception(5, 0xbad0, 0x1000); m.exception(7, 0xbad4, 0x2000);
        assert_eq!(m.read(0x68), Ok(7)); assert_eq!(m.read(0x6c), Ok(0xbad4)); assert_eq!(m.read(0x70), Ok(0x2000)); assert_eq!(m.read(0x74), Ok(3));
        assert_eq!(m.read(0x78), Ok(5)); assert_eq!(m.read(0x80), Ok(0x1000)); assert_eq!(m.read(0x60), Ok(0x2000));
        m.write(0x68, 0).unwrap(); assert_eq!(m.read(0x68), Ok(7)); assert_eq!(m.write(0, 1), Err(Fault::Prohibited));
    }
    #[test]
    fn core1_capture_uses_its_own_samples_and_exception_history() {
        let mut m = BusMonitor::default(); m.clock_write(0x10).unwrap();
        m.write(0x88, TRACE).unwrap(); m.write(0xcc, 3).unwrap();
        m.observe_core(1, 0x2f001000, 0x2f07e000); m.exception_core(1, 5, 0x2c0003d8, 0x400022ee);
        assert_eq!(m.read(0xd0), Ok(0x2f001000)); assert_eq!(m.read(0xd4), Ok(0x2f07e000));
        assert_eq!(m.read(0xf8), Ok(0x400022ee)); assert_eq!(m.read(0x70), Ok(0));
        m.observe_core(0, 1, 2); assert_eq!(m.read(0xd0), Ok(0x2f001000));
        m.clock_write(0x40).unwrap(); assert_eq!(m.read(0xd0), Ok(0));
    }

}
