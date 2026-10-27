//! S31 RTC slow-clock counter/snapshot subset. ESP-IDF v6.1
//! fff9895c82d744c7237be8847347bdd1b07c6643: rtc_timer_reg.h,
//! rtc_timer_ll.h and lp_clkrst_reg.h. Alarm/wakeup/REGDMA and LP hart
//! delivery are not implemented; requesting those features faults explicitly.
use emu_core::Fault;
pub const BASE: u32 = 0x2080_0000;
pub const RESET: u32 = 0x2070_100c;
const MASK: u64 = (1 << 48) - 1;
const DONE: u32 = 1 << 29;
const OVERFLOW: u32 = 1 << 30;
#[derive(Default)]
pub struct RtcTimer {
    counter: u64, snapshot: u64, target: [u64; 2], raw: [u32; 2],
    reset: bool, phase: u128, phase_hz: u64,
}
impl RtcTimer {
    pub fn counter(&self) -> u64 { self.counter }
    pub fn reset_read(&self) -> u32 { u32::from(self.reset) << 31 }
    pub fn reset_write(&mut self, value: u32) { if value & (1 << 31) != 0 { *self = Self::default(); self.reset = true; } else { self.reset = false; } }
    pub fn advance(&mut self, ticks: u64) {
        if self.reset { return; }
        let total = u128::from(self.counter) + u128::from(ticks);
        if total > u128::from(MASK) { for raw in &mut self.raw { *raw |= OVERFLOW; } }
        self.counter = (total & u128::from(MASK)) as u64;
    }
    /// Optional functional timing input: the RV32 interpreter accounts one cycle
    /// per instruction, not measured S31 latency. No host-wall-clock input is used.
    /// Preserve fractional phase across clock changes (rounding <1/new_hz tick).
    pub fn advance_cpu_cycles(&mut self, cycles: u64, cpu_hz: u64, slow_hz: u64) {
        if self.reset || cpu_hz == 0 { return; }
        if self.phase_hz != 0 && self.phase_hz != cpu_hz { self.phase = self.phase * u128::from(cpu_hz) / u128::from(self.phase_hz); }
        self.phase_hz = cpu_hz;
        let scaled = u128::from(cycles) * u128::from(slow_hz) + self.phase;
        self.phase = scaled % u128::from(cpu_hz);
        let ticks = scaled / u128::from(cpu_hz);
        // Preserve 48-bit modulo and overflow even for an arbitrarily large input.
        if ticks > u128::from(MASK) { for raw in &mut self.raw { *raw |= OVERFLOW; } }
        self.advance((ticks & u128::from(MASK)) as u64);
    }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        Ok(match off {
            0 | 8 => self.target[(off / 8) as usize] as u32,
            4 | 12 => (self.target[(off / 8) as usize] >> 32) as u32,
            0x10 | 0x24 | 0x2c | 0x30 | 0x34 | 0x3c | 0x40 | 0x44 => 0,
            0x14 | 0x1c => self.snapshot as u32, 0x18 | 0x20 => (self.snapshot >> 32) as u32,
            0x28 => self.raw[0], 0x38 => self.raw[1],
            _ => return Err(Fault::Unmapped),
        })
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        match off {
            0 | 8 => { let t = &mut self.target[(off / 8) as usize]; *t = (*t & !0xffff_ffff) | u64::from(value); }
            4 | 12 if value & !0xffff == 0 => { let t = &mut self.target[(off / 8) as usize]; *t = (*t & 0xffff_ffff) | (u64::from(value) << 32); }
            0x10 if value & !(1 << 27) == 0 => {
                if value & (1 << 27) != 0 && !self.reset { self.snapshot = self.counter; for raw in &mut self.raw { *raw |= DONE; } }
            }
            0x14 | 0x18 | 0x1c | 0x20 | 0x2c | 0x3c => {}, // read-only
            0x28 | 0x34 => self.raw[0] &= !(value & (DONE | OVERFLOW)),
            0x38 | 0x44 => self.raw[1] &= !(value & (DONE | OVERFLOW)),
            0x24 | 0x30 | 0x40 if value == 0 => {},
            4 | 12 | 0x10 | 0x24 | 0x30 | 0x40 => return Err(Fault::Prohibited),
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_is_triggered_and_latched_not_a_live_or_constant_ready_read() {
        let mut t = RtcTimer::default(); t.advance(0x1_ffff_ffff);
        assert_eq!(t.read(0x28), Ok(0)); assert_eq!(t.read(0x14), Ok(0));
        t.write(0x10, 1 << 27).unwrap(); assert_eq!(t.read(0x10), Ok(0));
        t.advance(7); assert_eq!(t.read(0x14), Ok(u32::MAX)); assert_eq!(t.read(0x18), Ok(1));
        assert_eq!(t.read(0x1c), t.read(0x14)); assert_eq!(t.read(0x38), Ok(DONE));
        t.write(0x34, DONE).unwrap(); assert_eq!(t.read(0x28), Ok(0)); assert_eq!(t.read(0x38), Ok(DONE));
        t.write(0x14, 1).unwrap(); assert_eq!(t.read(0x14), Ok(u32::MAX));
        t.write(0x10, 1 << 27).unwrap(); assert_eq!(t.read(0x14), Ok(6));
        assert_eq!(t.write(4, 1 << 31), Err(Fault::Prohibited)); assert_eq!(t.write(0x30, DONE), Err(Fault::Prohibited));
        assert_eq!(t.write(0x10, 1 << 26), Err(Fault::Prohibited));
        t.reset_write(1 << 31); t.advance(99); t.write(0x10, 1 << 27).unwrap();
        assert_eq!(t.counter(), 0); assert_eq!(t.read(0x28), Ok(0));
        t.reset_write(0); t.advance(MASK); t.advance(1); assert_eq!(t.counter(), 0); assert_eq!(t.read(0x28), Ok(OVERFLOW));
    }
    #[test]
    fn optional_cycle_conversion_preserves_fraction_and_clock_change() {
        let mut t = RtcTimer::default();
        for _ in 0..999 { t.advance_cpu_cycles(1, 40_000_000, 136_000); }
        assert_eq!(t.counter(), 3); t.advance_cpu_cycles(2, 80_000_000, 136_000);
        assert_eq!(t.counter(), 3); t.advance_cpu_cycles(80_000_000 - 2, 80_000_000, 136_000);
        assert_eq!(t.counter(), 136_003);
        let before = t.counter(); t.advance_cpu_cycles(100, 0, 136_000); assert_eq!(t.counter(), before);
        t.reset_write(1 << 31); t.advance_cpu_cycles(40_000_000, 40_000_000, 136_000); assert_eq!(t.counter(), 0);
    }
}
