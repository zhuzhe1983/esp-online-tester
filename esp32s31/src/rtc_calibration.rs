//! TIMG0 clock measurement, fixed IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! timer_group_reg.h, hp_sys_clkrst_reg.h, clk_tree_ll.h and rtc_time.c.
//! Counts XTAL ticks over target-clock cycles; nominal oscillator rates are inputs,
//! not measured silicon calibration. General-purpose TIMG timers are separate.
use emu_core::Fault;
pub const BASE: u32 = 0x2058_0000;
pub const CLOCK: u32 = 0x2058_7118;
const XTAL_HZ: u64 = 40_000_000;
const CYCLING: u32 = 1 << 12;
const START: u32 = 1 << 31;
pub struct Calibration { clock: u32, config: u32, timeout_config: u32, ready: bool, timeout: bool, valid: bool, value: u32, elapsed: u64, phase: u128, running: bool, cpu_fraction: u128, phase_hz: u64, pub measurements: u64 }
impl Default for Calibration {
    fn default() -> Self { Self { clock: 17, config: CYCLING | 1 << 16, timeout_config: 0xffffff98, ready: false, timeout: false, valid: false, value: 0, elapsed: 0, phase: 0, running: true, cpu_fraction: 0, phase_hz: 0, measurements: 0 } }
}
impl Calibration {
    pub fn is_mmio(address: u32) -> bool { matches!(address & !3, CLOCK | 0x20580068 | 0x2058006c | 0x20580080) }
    pub fn source(&self) -> u32 { (self.clock >> 1) & 15 }
    pub fn reset(&mut self) { let clock = self.clock; *self = Self::default(); self.clock = clock; }
    fn start(&mut self) { self.ready = false; self.timeout = false; self.valid = false; self.elapsed = 0; self.phase = 0; self.running = self.config & (START | CYCLING) != 0; }
    pub fn read(&self, address: u32, group_clock: u32) -> Result<u32, Fault> {
        if address == CLOCK { return Ok(self.clock); }
        if group_clock & 3 != 1 { return Err(Fault::Prohibited); }
        match address.wrapping_sub(BASE) {
            0x68 => Ok(self.config | u32::from(self.ready) << 15),
            0x6c => Ok(self.value << 7 | u32::from(self.valid)),
            0x80 => Ok(self.timeout_config | u32::from(self.timeout)), _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32, group_clock: u32) -> Result<(), Fault> {
        if address == CLOCK {
            // Bounded sources connected to existing real clock/power models.
            // APLL, BBPLL, RC_FAST digital routing, external32K/pad and core
            // clock-gate paths are not fabricated.
            if !matches!((value >> 1) & 15, 0 | 2 | 4..=6 | 8 | 12) { return Err(Fault::Prohibited); }
            self.clock = value & 0x1fffff; return Ok(());
        }
        if group_clock & 3 != 1 { return Err(Fault::Prohibited); }
        match address.wrapping_sub(BASE) {
            0x68 => {
                let next = value & 0xffff1000;
                if next >> 16 & 0x7fff == 0 { return Err(Fault::Prohibited); } // zero-cycle request is outside fixed LL contract
                let changed = (next ^ self.config) & (START | CYCLING); self.config = next;
                if changed != 0 { self.start(); }
            }
            0x6c => {}, // hardware output, not writable by guest
            0x80 => self.timeout_config = value & 0xfffffff8,
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    /// Explicit XTAL-domain input. APB access gating does not manufacture or
    /// advance measurements; target-clock gating leads to a genuine timeout.
    pub fn advance_xtal(&mut self, mut ticks: u64, source_hz: u64, group_clock: u32) {
        if ticks == 0 || group_clock & 2 != 0 || !self.running { return; }
        let source_hz = if self.clock & 1 != 0 { source_hz } else { 0 };
        let denominator = u128::from(XTAL_HZ) * u128::from((self.clock >> 5) + 1);
        let target = u128::from((self.config >> 16) & 0x7fff) * denominator;
        let threshold = u64::from(self.timeout_config >> 7);
        loop {
            let finish = if source_hz == 0 { u64::MAX } else { ((target.saturating_sub(self.phase)).div_ceil(u128::from(source_hz))).min(u128::from(u64::MAX)) as u64 };
            let timeout = threshold.saturating_add(1).saturating_sub(self.elapsed);
            let step = ticks.min(finish.min(timeout)); self.elapsed = self.elapsed.saturating_add(step);
            self.phase += u128::from(step) * u128::from(source_hz); ticks -= step;
            if finish <= timeout && step == finish {
                self.value = (self.elapsed & 0x1ffffff) as u32; self.measurements = self.measurements.wrapping_add(1);
                if self.config & CYCLING == 0 { self.ready = true; self.running = false; return; }
                self.valid = true; self.elapsed = 0; self.phase = 0;
                // Repeating identical complete windows can be batched safely.
                let period = target.div_ceil(u128::from(source_hz)) as u64;
                if let Some(windows) = ticks.checked_div(period) { self.measurements = self.measurements.wrapping_add(windows); ticks %= period; }
            } else if step == timeout { self.timeout = true; self.running = false; return; }
            if ticks == 0 { return; }
        }
    }
    /// Explicit development timing opt-in, one nominal CPU quantum per scheduler
    /// round. Uses fractional XTAL ticks; reads never complete a measurement.
    pub fn advance_cpu(&mut self, cycles: u64, cpu_hz: u64, source_hz: u64, group_clock: u32) {
        if cpu_hz == 0 { return; }
        if self.phase_hz != 0 && self.phase_hz != cpu_hz { self.cpu_fraction = self.cpu_fraction * u128::from(cpu_hz) / u128::from(self.phase_hz); }
        self.phase_hz = cpu_hz;
        let total = self.cpu_fraction + u128::from(cycles) * u128::from(XTAL_HZ);
        let ticks = total / u128::from(cpu_hz); self.cpu_fraction = total % u128::from(cpu_hz);
        self.advance_xtal(ticks.min(u128::from(u64::MAX)) as u64, source_hz, group_clock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_count_scales_with_divider_and_read_only_flags_cannot_finish_it() {
        let mut c = Calibration::default(); c.write(CLOCK, 1 | 12 << 1 | 9 << 5, 0).unwrap();
        c.write(BASE + 0x68, START | 3 << 16, 0x921).unwrap();
        c.write(BASE + 0x6c, u32::MAX, 0x921).unwrap(); assert_eq!(c.read(BASE + 0x6c, 0x921), Ok(0));
        c.advance_xtal(29, XTAL_HZ, 0x921); assert_eq!(c.read(BASE + 0x68, 0x921).unwrap() & 0x8000, 0);
        c.advance_xtal(1, XTAL_HZ, 0x921); assert_eq!(c.read(BASE + 0x6c, 0x921), Ok(30 << 7));
        assert_ne!(c.read(BASE + 0x68, 0x921).unwrap() & 0x8000, 0); assert_eq!(c.measurements, 1);
        c.advance_xtal(100, XTAL_HZ, 0x921); assert_eq!(c.measurements, 1);
    }
    #[test]
    fn absent_or_gated_target_times_out_and_restart_clears_old_result_flags() {
        let mut c = Calibration::default(); c.write(BASE + 0x80, 3 << 3 | 3 << 7 | 1, 0x921).unwrap();
        c.advance_xtal(3, 0, 0x921); assert_eq!(c.read(BASE + 0x80, 0x921).unwrap() & 1, 0);
        c.advance_xtal(1, 0, 0x921); assert_eq!(c.read(BASE + 0x80, 0x921).unwrap() & 1, 1);
        c.write(BASE + 0x68, START | 1 << 16, 0x921).unwrap(); assert_eq!(c.read(BASE + 0x80, 0x921).unwrap() & 1, 0);
        c.write(CLOCK, 12 << 1, 0).unwrap(); c.advance_xtal(4, XTAL_HZ, 0x921); assert_eq!(c.measurements, 0); assert!(c.timeout);
        c.reset(); assert!(!c.timeout); assert_eq!(c.read(BASE + 0x68, 0x920), Err(Fault::Prohibited));
        c.advance_xtal(100, XTAL_HZ, 0x923); assert_eq!(c.measurements, 0);
    }
    #[test]
    fn partial_period_batching_matches_single_tick_advances_and_reset_preserves_mux() {
        let mut a = Calibration::default(); let mut b = Calibration::default();
        for c in [&mut a, &mut b] { c.write(CLOCK, 1 | 12 << 1 | 9 << 5, 0).unwrap(); c.advance_xtal(9, XTAL_HZ, 0x921); }
        a.advance_xtal(101, XTAL_HZ, 0x921);
        for _ in 0..101 { b.advance_xtal(1, XTAL_HZ, 0x921); }
        assert_eq!((a.measurements, a.elapsed, a.phase, a.value), (b.measurements, b.elapsed, b.phase, b.value));
        assert_eq!(a.measurements, 11); a.reset(); assert_eq!(a.source(), 12); assert_eq!(a.measurements, 0);
        assert_eq!(a.read(BASE + 0x68, 0x921), Ok(CYCLING | 1 << 16));
    }

    #[test]
    fn periodic_and_fractional_clock_advance_have_real_counter_boundaries() {
        let mut c = Calibration::default(); c.write(CLOCK, 1 | 12 << 1, 0).unwrap();
        c.advance_cpu(1, 80_000_000, XTAL_HZ, 0x921); assert_eq!(c.measurements, 0);
        c.advance_cpu(1, 80_000_000, XTAL_HZ, 0x921); assert_eq!(c.measurements, 1);
        assert_eq!(c.read(BASE + 0x6c, 0x921), Ok(1 << 7 | 1)); assert!(!c.ready);
        c.advance_xtal(100, XTAL_HZ, 0x921); assert_eq!(c.measurements, 101);
        c.write(BASE + 0x68, 1 << 16, 0x921).unwrap(); c.advance_xtal(u64::MAX, XTAL_HZ, 0x921); assert_eq!(c.measurements, 101);
        assert_eq!(c.write(CLOCK, 1 | 10 << 1, 0), Err(Fault::Prohibited)); // no invented external32K source
    }
}
