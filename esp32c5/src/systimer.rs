//! C5 lazy 16 MHz system timer, synchronized counters/alarms and alarm ETM sources.
//! ESP-IDF v5.5.4@735507283d5b2f9fb363a1901172dbd9e847945d:
//! soc/esp32c5/{register/soc/{systimer,pcr}_reg.h,include/soc/interrupts.h};
//! C5 TRM v1.1 chapter14. Nominal CPU_HZ=240MHz, no divider-edge jitter,
//! CPU/debug stalls, RC_FAST, ETM routing or clock-domain crossing latency.
const BASE: u32 = 0x6000_a000;
const PCR_CONF: u32 = 0x6009_606c;
const PCR_FUNC: u32 = 0x6009_6070;
const MASK: u64 = (1 << 52) - 1;
const HALF: u64 = 1 << 51;
const CYCLES_PER_TICK: u64 = 240_000_000 / 16_000_000;
const CONF_MASK: u32 = 0xffc0_0003;
const PERIOD_MASK: u32 = 0x03ff_ffff;

pub(crate) struct C5Systimer {
    conf: u32, unit: [u64; 2], snapshot: [u64; 2], valid: [bool; 2], load: [u64; 2],
    target: [u64; 3], target_conf: [u32; 3], active_target: [u64; 3], period: [u64; 3],
    actual: [u64; 3], armed: [bool; 3], int_ena: u32, int_raw: u32, date: u32,
    pcr_conf: u32, pcr_func: u32, last_cycle: u64, phase: u64, etm: [u64; 3],
    pub(crate) deadline: Option<u64>,
}

impl C5Systimer {
    pub(crate) fn new(cycle: u64) -> Self {
        Self { conf: 0x4600_0000, unit: [0; 2], snapshot: [0; 2], valid: [false; 2], load: [0; 2],
            target: [0; 3], target_conf: [0; 3], active_target: [0; 3], period: [0; 3],
            actual: [0; 3], armed: [false; 3], int_ena: 0, int_raw: 0, date: 36_774_432,
            pcr_conf: 1, pcr_func: 1 << 22, last_cycle: cycle, phase: 0, etm: [0; 3], deadline: None }
    }
    pub(crate) fn contains(address: u32) -> bool { (BASE..=BASE + 0xfc).contains(&address) || matches!(address, PCR_CONF | PCR_FUNC) }
    pub(crate) fn sources(&self) -> [(usize, bool); 3] { std::array::from_fn(|t| (61 + t, self.int_raw & self.int_ena & (1 << t) != 0)) }
    /// Host diagnostic totals, saturating and retained across peripheral reset.
    /// This is an ETM source observation, not an ETM fabric/consumer execution.
    pub(crate) fn etm_events(&self) -> [u64; 3] { self.etm }
    fn clock_running(&self) -> bool { self.pcr_conf & 2 == 0 && self.pcr_func & (1 << 22) != 0 }
    fn enabled(&self, t: usize) -> bool { self.conf & (1 << (24 - t)) != 0 }
    fn periodic(&self, t: usize) -> bool { self.target_conf[t] & (1 << 30) != 0 }
    fn selected(&self, t: usize) -> usize { (self.target_conf[t] >> 31) as usize }
    fn reset_registers(&mut self) {
        self.conf = 0x4600_0000; self.unit = [0; 2]; self.snapshot = [0; 2]; self.valid = [false; 2]; self.load = [0; 2];
        self.target = [0; 3]; self.target_conf = [0; 3]; self.active_target = [0; 3]; self.period = [0; 3];
        self.actual = [0; 3]; self.armed = [false; 3]; self.int_ena = 0; self.int_raw = 0; self.date = 36_774_432; self.phase = 0;
    }
    fn restart_alarm(&mut self, t: usize) {
        self.actual[t] = if self.periodic(t) { self.unit[self.selected(t)].wrapping_add(self.period[t]) & MASK } else { self.active_target[t] };
        self.armed[t] = true;
    }
    fn event(&mut self, t: usize, n: u64) {
        self.int_raw |= 1 << t;
        if self.conf & 2 != 0 { self.etm[t] = self.etm[t].saturating_add(n); }
    }
    fn alarms(&mut self, before: [u64; 2], steps: [u64; 2]) {
        if !self.clock_running() { return; }
        for t in 0..3 {
            if !self.enabled(t) || !self.armed[t] { continue; }
            let u = self.selected(t); let late = before[u].wrapping_sub(self.actual[t]) & MASK;
            // TRM Table14.5-2: compensate a missed target strictly less than
            // 2^51-1 ticks old; the boundary waits through rollover.
            let overdue = late < HALF - 1;
            let distance = if overdue { 0 } else { self.actual[t].wrapping_sub(before[u]) & MASK };
            if steps[u] < distance { continue; }
            if !self.periodic(t) { self.event(t, 1); self.armed[t] = false; continue; }
            let p = self.period[t]; if p == 0 { continue; } // zero periods reject on activation/load
            if overdue {
                // A software counter jump can skip old targets, but it cannot
                // create ETM events in elapsed time that never happened. One
                // immediate comparison is real; retain the periodic grid and
                // count only later targets actually crossed by this advance.
                let skip = late / p + 1; let next = skip * p - late;
                let more = if steps[u] < next { 0 } else { 1 + (steps[u] - next) / p };
                self.event(t, 1 + more); self.actual[t] = self.actual[t].wrapping_add((skip + more).wrapping_mul(p)) & MASK;
            } else {
                let n = 1 + (steps[u] - distance) / p;
                self.event(t, n); self.actual[t] = self.actual[t].wrapping_add(n.wrapping_mul(p)) & MASK;
            }
        }
    }
    fn schedule(&mut self, cycle: u64) {
        self.deadline = None; if !self.clock_running() { return; }
        for t in 0..3 {
            if !self.enabled(t) || !self.armed[t] { continue; }
            let u = self.selected(t); let late = self.unit[u].wrapping_sub(self.actual[t]) & MASK;
            let ticks = if late < HALF - 1 { 0 } else { self.actual[t].wrapping_sub(self.unit[u]) & MASK };
            if ticks != 0 && self.conf & (1 << (30 - u)) == 0 { continue; }
            let cycles = ticks.saturating_mul(CYCLES_PER_TICK).saturating_sub(self.phase);
            if let Some(at) = cycle.checked_add(cycles) { self.deadline = Some(self.deadline.map_or(at, |old| old.min(at))); }
        }
    }
    pub(crate) fn pump(&mut self, cycle: u64) {
        let elapsed = cycle.wrapping_sub(self.last_cycle); self.last_cycle = cycle;
        if self.clock_running() {
            // Keep the sub-tick remainder across irregular pump calls. Each
            // unit sees the same clock edge, even while the other is stopped.
            let tail = elapsed % CYCLES_PER_TICK + self.phase;
            let ticks = elapsed / CYCLES_PER_TICK + tail / CYCLES_PER_TICK; self.phase = tail % CYCLES_PER_TICK;
            let before = self.unit; let mut steps = [0; 2];
            for (u, n) in steps.iter_mut().enumerate() {
                if self.conf & (1 << (30 - u)) != 0 { *n = ticks; self.unit[u] = self.unit[u].wrapping_add(ticks) & MASK; }
            }
            self.alarms(before, steps);
        }
        self.schedule(cycle);
    }
    pub(crate) fn read(&mut self, address: u32, cycle: u64) -> Option<u32> {
        if address & 3 != 0 { return None; } self.pump(cycle);
        if address == PCR_CONF { return Some(self.pcr_conf | if self.pcr_conf & 2 == 0 { 4 } else { 0 }); }
        if address == PCR_FUNC { return Some(self.pcr_func); }
        if self.pcr_conf & 1 == 0 { return None; } // APB gated: do not invent a successful transaction
        let off = address.checked_sub(BASE)?;
        Some(match off {
            0 => self.conf,
            4 | 8 => u32::from(self.valid[((off - 4) / 4) as usize]) << 29,
            0xc..=0x18 => { let u = ((off - 0xc) / 8) as usize; if off & 4 != 0 { (self.load[u] >> 32) as u32 } else { self.load[u] as u32 } },
            0x1c..=0x30 => { let t = ((off - 0x1c) / 8) as usize; if off & 4 != 0 { (self.target[t] >> 32) as u32 } else { self.target[t] as u32 } },
            0x34..=0x3c => self.target_conf[((off - 0x34) / 4) as usize],
            0x40..=0x4c => { let u = ((off - 0x40) / 8) as usize; if off & 4 == 0 { (self.snapshot[u] >> 32) as u32 } else { self.snapshot[u] as u32 } },
            0x50..=0x60 | 0x6c => 0,
            0x64 => self.int_ena, 0x68 => self.int_raw, 0x70 => self.int_raw & self.int_ena,
            0x74..=0x88 => { let t = ((off - 0x74) / 8) as usize; if off & 4 == 0 { (self.actual[t] >> 32) as u32 } else { self.actual[t] as u32 } },
            0xfc => self.date, _ => return None,
        })
    }
    pub(crate) fn write(&mut self, address: u32, value: u32, cycle: u64) -> bool {
        if address & 3 != 0 { return false; } self.pump(cycle);
        if address == PCR_CONF {
            if value & 2 != 0 { self.reset_registers(); } self.pcr_conf = value & 3; self.schedule(cycle); return true;
        }
        if address == PCR_FUNC {
            if value & (1 << 20) != 0 { return false; } // RC_FAST is not a calibrated oscillator here
            self.pcr_func = value & (1 << 22); self.schedule(cycle); return true;
        }
        if self.pcr_conf & 3 != 1 { return false; }
        let Some(off) = address.checked_sub(BASE) else { return false; };
        match off {
            0 => {
                for t in 0..3 { if value & (1 << (24 - t)) != 0 && self.periodic(t) && self.period[t] == 0 { return false; } }
                let old = self.conf; self.conf = value & CONF_MASK;
                for t in 0..3 {
                    if self.enabled(t) && old & (1 << (24 - t)) == 0 { self.restart_alarm(t); }
                    if !self.enabled(t) { self.armed[t] = false; }
                }
            }
            4 | 8 => if value & (1 << 30) != 0 { let u = ((off - 4) / 4) as usize; self.valid[u] = false; self.snapshot[u] = self.unit[u]; self.valid[u] = true; },
            0xc..=0x18 => { let u = ((off - 0xc) / 8) as usize; if off & 4 != 0 { self.load[u] = (self.load[u] & 0xffff_ffff) | u64::from(value & 0xfffff) << 32; } else { self.load[u] = (self.load[u] & !0xffff_ffff) | u64::from(value); } },
            0x1c..=0x30 => { let t = ((off - 0x1c) / 8) as usize; if off & 4 != 0 { self.target[t] = (self.target[t] & 0xffff_ffff) | u64::from(value & 0xfffff) << 32; } else { self.target[t] = (self.target[t] & !0xffff_ffff) | u64::from(value); } },
            0x34..=0x3c => {
                let t = ((off - 0x34) / 4) as usize; let v = value & 0xc3ff_ffff;
                if self.enabled(t) && v & (1 << 30) != 0 && self.period[t] == 0 { return false; }
                let changed = (self.target_conf[t] ^ v) & 0xc000_0000 != 0; self.target_conf[t] = v;
                if changed { self.restart_alarm(t); }
            }
            0x40..=0x4c | 0x68 | 0x70 | 0x74..=0x88 => {}, // hardware-written values
            0x50..=0x58 => if value & 1 != 0 {
                let t = ((off - 0x50) / 4) as usize; let p = u64::from(self.target_conf[t] & PERIOD_MASK);
                if self.periodic(t) && p == 0 { return false; }
                self.active_target[t] = self.target[t]; self.period[t] = p; self.restart_alarm(t);
            },
            0x5c | 0x60 => if value & 1 != 0 { let u = ((off - 0x5c) / 4) as usize; self.unit[u] = self.load[u]; },
            0x64 => self.int_ena = value & 7, 0x6c => self.int_raw &= !(value & 7),
            0xfc => self.date = value, _ => return false,
        }
        self.alarms(self.unit, [0; 2]); self.schedule(cycle); true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(s: &mut C5Systimer, off: u32, value: u32, cycle: u64) { assert!(s.write(BASE + off, value, cycle), "write {off:#x}"); }
    fn r(s: &mut C5Systimer, off: u32, cycle: u64) -> u32 { s.read(BASE + off, cycle).unwrap() }
    fn load_unit(s: &mut C5Systimer, u: u32, value: u64, cycle: u64) {
        w(s, 0xc + u * 8, (value >> 32) as u32, cycle); w(s, 0x10 + u * 8, value as u32, cycle); w(s, 0x5c + u * 4, 1, cycle);
    }
    fn target(s: &mut C5Systimer, t: u32, value: u64, cycle: u64) {
        w(s, 0x1c + t * 8, (value >> 32) as u32, cycle); w(s, 0x20 + t * 8, value as u32, cycle); w(s, 0x50 + t * 4, 1, cycle);
    }
    fn periodic(s: &mut C5Systimer, period: u32, etm: bool, cycle: u64) {
        w(s, 0x34, 1 << 30 | period, cycle); w(s, 0x50, 1, cycle);
        w(s, 0, 0x4700_0000 | if etm { 2 } else { 0 }, cycle);
    }
    #[test]
    fn power_on_snapshot_is_invalid_until_real_update_and_stays_coherent() {
        let mut s = C5Systimer::new(0);
        assert_eq!(r(&mut s, 0, 0), 0x4600_0000); assert_eq!(r(&mut s, 0xfc, 0), 36_774_432);
        assert_eq!(r(&mut s, 4, 99), 0); assert_eq!(r(&mut s, 0x44, 99), 0);
        w(&mut s, 4, 0, 99); assert_eq!(r(&mut s, 4, 99), 0);
        w(&mut s, 4, 1 << 30, 119); assert_eq!(r(&mut s, 4, 119), 1 << 29);
        assert_eq!(r(&mut s, 0x44, 3000), 7); assert_eq!(r(&mut s, 0x40, 3000), 0);
        w(&mut s, 0x44, 123, 3000); assert_eq!(r(&mut s, 0x44, 3000), 7);
        w(&mut s, 4, 1 << 29, 3000); assert_eq!(r(&mut s, 0x44, 3000), 7);
        w(&mut s, 4, 1 << 30, 3000); assert_eq!(r(&mut s, 0x44, 3000), 200);
        assert_eq!(r(&mut s, 8, 3000), 0); w(&mut s, 8, 1 << 30, 3000); assert_eq!(r(&mut s, 0x4c, 3000), 0);
        assert_eq!(s.deadline, None); assert_eq!(s.sources(), [(61, false), (62, false), (63, false)]);
    }
    #[test]
    fn fractional_cycles_are_conserved_without_clock_force_or_alarm_polling() {
        let mut a = C5Systimer::new(123); let mut b = C5Systimer::new(123);
        let mut cycle = 123;
        for delta in [1, 7, 64, 512, 5, 3, 100, 2, 6] { cycle += delta; a.pump(cycle); }
        b.pump(cycle); assert_eq!(a.unit, b.unit); assert_eq!(a.phase, b.phase);
        assert_eq!(a.unit[0], (cycle - 123) / 15); assert_eq!(a.unit[1], 0);
        assert_eq!(a.deadline, None);
        w(&mut a, 0, 0, cycle); let stopped = a.unit[0]; a.pump(cycle + 150); assert_eq!(a.unit[0], stopped);
        w(&mut a, 0, 1 << 30 | 1 << 29, cycle + 150); a.pump(cycle + 300);
        assert_eq!(a.unit, [stopped + 10, 10]);
    }
    #[test]
    fn counter_load_is_masked_staged_and_zero_wt_writes_do_nothing() {
        let mut s = C5Systimer::new(0);
        w(&mut s, 0xc, u32::MAX, 0); w(&mut s, 0x10, u32::MAX, 0);
        assert_eq!(r(&mut s, 0xc, 0), 0xfffff); assert_eq!(s.unit[0], 0);
        w(&mut s, 0x5c, 0, 0); assert_eq!(s.unit[0], 0);
        w(&mut s, 0x5c, 1, 0); assert_eq!(s.unit[0], MASK); assert_eq!(r(&mut s, 0x5c, 0), 0);
        s.pump(15); assert_eq!(s.unit[0], 0); assert_eq!(r(&mut s, 4, 15), 0);
        load_unit(&mut s, 1, 0x1_0000_0003, 15); s.pump(150); assert_eq!(s.unit[1], 0x1_0000_0003);
    }
    #[test]
    fn one_shot_deadline_and_irq_are_exact_and_do_not_repeat_after_clear() {
        let mut s = C5Systimer::new(0); target(&mut s, 0, 32, 0);
        w(&mut s, 0, 0x4700_0000, 0); w(&mut s, 0x64, 1, 0);
        assert_eq!(s.deadline, Some(480)); s.pump(479); assert_eq!(s.int_raw, 0); assert_eq!(s.deadline, Some(480));
        s.pump(480); assert_eq!(s.sources(), [(61, true), (62, false), (63, false)]); assert_eq!(s.deadline, None);
        w(&mut s, 0x6c, 0, 480); assert_eq!(r(&mut s, 0x70, 480), 1);
        w(&mut s, 0x68, 0, 480); assert_eq!(r(&mut s, 0x70, 480), 1);
        w(&mut s, 0x6c, 1, 480); s.pump(10_000); assert_eq!(s.int_raw, 0);
        w(&mut s, 0, 0x4600_0000, 10_000); w(&mut s, 0, 0x4700_0000, 10_000);
        assert_eq!(s.int_raw, 1); // re-enable is an actual overdue comparison
    }
    #[test]
    fn each_c5_source_and_selected_unit_is_independent() {
        let mut s = C5Systimer::new(0);
        w(&mut s, 0x38, 1 << 31, 0); // target1 selects UNIT1
        for t in 0..3 { target(&mut s, t, 10 * (u64::from(t) + 1), 0); }
        w(&mut s, 0, 0x47c0_0002, 0); w(&mut s, 0x64, 7, 0);
        s.pump(300); assert_eq!(s.sources(), [(61, true), (62, false), (63, false)]);
        // UNIT1 is stopped; its future target has no deadline yet.
        assert_eq!(s.deadline, Some(450)); w(&mut s, 0, 0x67c0_0002, 300);
        s.pump(600); assert_eq!(s.sources(), [(61, true), (62, true), (63, true)]);
        assert_eq!(s.etm_events(), [1, 1, 1]); w(&mut s, 0x6c, 2, 600);
        assert_eq!(s.sources(), [(61, true), (62, false), (63, true)]);
    }
    #[test]
    fn staged_target_and_period_require_their_load_trigger() {
        let mut s = C5Systimer::new(0); target(&mut s, 0, 100, 0); w(&mut s, 0, 0x4700_0000, 0);
        w(&mut s, 0x20, 1, 0); w(&mut s, 0x50, 0, 0);
        assert_eq!(r(&mut s, 0x20, 15), 1); assert_eq!(r(&mut s, 0x74, 15), 100);
        assert_eq!(s.int_raw, 0); assert_eq!(s.deadline, Some(1500));
        w(&mut s, 0x50, 1, 15); assert_eq!(s.int_raw, 1); assert_eq!(r(&mut s, 0x74, 15), 1);
        let mut p = C5Systimer::new(0); periodic(&mut p, 40, false, 0);
        w(&mut p, 0x34, 1 << 30 | 1, 0); assert_eq!(p.deadline, Some(600));
        p.pump(15); assert_eq!(p.int_raw, 0); w(&mut p, 0x50, 1, 15); assert_eq!(p.deadline, Some(30));
    }
    #[test]
    fn periodic_catch_up_preserves_grid_irq_and_exact_etm_source_totals() {
        let mut s = C5Systimer::new(0); periodic(&mut s, 5, true, 0); w(&mut s, 0x64, 1, 0);
        assert_eq!(s.deadline, Some(75)); s.pump(376);
        assert_eq!(s.etm_events(), [5, 0, 0]); assert_eq!(r(&mut s, 0x74, 376), 30);
        assert_eq!(s.deadline, Some(450)); assert_eq!(s.int_raw, 1);
        w(&mut s, 0x6c, 1, 376); s.pump(449); assert_eq!(s.int_raw, 0); s.pump(450);
        assert_eq!(s.int_raw, 1); assert_eq!(s.etm_events(), [6, 0, 0]);
        w(&mut s, 0, 0x4700_0000, 450); s.pump(900); assert_eq!(s.etm_events(), [6, 0, 0]);
        assert_eq!(r(&mut s, 0x74, 900), 65); w(&mut s, 0, 0x4700_0002, 900); s.pump(975);
        assert_eq!(s.etm_events(), [7, 0, 0]);
        w(&mut s, 0, 0x4600_0002, 975); s.pump(1200); assert_eq!(s.etm_events(), [7, 0, 0]); assert_eq!(s.deadline, None);
    }
    #[test]
    fn software_counter_jump_does_not_invent_historical_etm_events() {
        let mut s = C5Systimer::new(0); periodic(&mut s, 5, true, 0);
        load_unit(&mut s, 0, 102, 0); assert_eq!(s.etm_events(), [1, 0, 0]);
        assert_eq!(r(&mut s, 0x74, 0), 105); assert_eq!(s.deadline, Some(45));
        s.pump(45); assert_eq!(s.etm_events(), [2, 0, 0]); assert_eq!(r(&mut s, 0x74, 45), 110);
    }
    #[test]
    fn wrap_and_half_range_targets_wait_or_compensate_without_plain_integer_compare() {
        let mut s = C5Systimer::new(0); load_unit(&mut s, 0, MASK - 3, 0); target(&mut s, 0, 2, 0);
        w(&mut s, 0, 0x4700_0000, 0); assert_eq!(s.deadline, Some(90)); s.pump(89); assert_eq!(s.int_raw, 0);
        s.pump(90); assert_eq!(s.int_raw, 1); assert_eq!(s.unit[0], 2);
        let mut h = C5Systimer::new(0); target(&mut h, 0, HALF, 0); w(&mut h, 0, 0x4700_0000, 0);
        assert_eq!(h.int_raw, 0); assert_eq!(h.deadline, Some(HALF * 15));
        let mut missed = C5Systimer::new(0); load_unit(&mut missed, 0, HALF - 2, 0); target(&mut missed, 0, 0, 0);
        w(&mut missed, 0, 0x4700_0000, 0); assert_eq!(missed.int_raw, 1);
        let mut edge = C5Systimer::new(0); load_unit(&mut edge, 0, HALF - 1, 0); target(&mut edge, 0, 0, 0);
        w(&mut edge, 0, 0x4700_0000, 0); assert_eq!(edge.int_raw, 0); assert_eq!(edge.deadline, Some((HALF + 1) * 15));
    }
    #[test]
    fn pcr_apb_gate_is_separate_from_count_clock_and_functional_gate_pauses() {
        let mut s = C5Systimer::new(0); target(&mut s, 0, 5, 0); w(&mut s, 0, 0x4700_0000, 0);
        assert!(s.write(PCR_CONF, 0, 0)); assert_eq!(s.read(BASE, 30), None); assert!(!s.write(BASE, 0, 30));
        assert_eq!(s.unit[0], 2); assert_eq!(s.deadline, Some(75)); assert!(s.write(PCR_CONF, 1, 30));
        assert!(s.write(PCR_FUNC, 0, 30)); assert_eq!(s.deadline, None); s.pump(300); assert_eq!(s.unit[0], 2);
        assert!(s.write(PCR_FUNC, 1 << 22, 300)); assert_eq!(s.deadline, Some(345)); s.pump(345); assert_eq!(s.int_raw, 1);
    }
    #[test]
    fn pcr_reset_clears_real_state_before_ready_and_retains_only_host_observation() {
        let mut s = C5Systimer::new(0); periodic(&mut s, 1, true, 0); s.pump(15); w(&mut s, 4, 1 << 30, 15);
        assert!(s.write(PCR_CONF, 3, 15)); assert_eq!(s.read(PCR_CONF, 15), Some(3)); assert_eq!(s.deadline, None);
        assert_eq!(r(&mut s, 0, 15), 0x4600_0000); assert_eq!(r(&mut s, 4, 150), 0); assert_eq!(s.unit, [0, 0]);
        assert!(!s.write(BASE + 4, 1 << 30, 150)); assert_eq!(s.etm_events(), [1, 0, 0]);
        assert!(s.write(PCR_CONF, 1, 150)); assert_eq!(s.read(PCR_CONF, 150), Some(5)); s.pump(165);
        assert_eq!(s.unit, [1, 0]); assert_eq!(s.int_raw, 0);
    }
    #[test]
    fn unsupported_clocks_unknown_offsets_and_zero_periods_reject_safely() {
        let mut s = C5Systimer::new(0); assert!(!s.write(PCR_FUNC, 1 << 20 | 1 << 22, 0)); assert_eq!(s.read(PCR_FUNC, 0), Some(1 << 22));
        for off in [0x8c, 0xf8, 0x100] { assert_eq!(s.read(BASE + off, 0), None); assert!(!s.write(BASE + off, 1, 0)); }
        assert_eq!(s.read(BASE + 1, 0), None); assert!(!s.write(BASE + 1, 1, 0));
        assert_eq!(s.read(0, 0), None); assert!(!s.write(0, 1, 0));
        w(&mut s, 0x34, 1 << 30, 0); assert!(!s.write(BASE + 0x50, 1, 0)); assert!(!s.write(BASE, 0x4700_0000, 0));
        assert_eq!(s.int_raw, 0); assert_eq!(s.etm_events(), [0; 3]); assert_eq!(s.deadline, None);
        assert!(C5Systimer::contains(BASE)); assert!(C5Systimer::contains(PCR_CONF)); assert!(!C5Systimer::contains(BASE + 0x100));
    }
    #[test]
    fn elapsed_wrap_saturation_and_long_period_catch_up_are_bounded() {
        let mut s = C5Systimer::new(u64::MAX - 8); s.pump(6); assert_eq!(s.unit[0], 1); // 15 elapsed nominal cycles
        let mut p = C5Systimer::new(0); periodic(&mut p, 1, true, 0); p.etm[0] = u64::MAX - 1;
        p.pump(u64::MAX); assert_eq!(p.etm_events()[0], u64::MAX); assert_eq!(p.unit[0], (u64::MAX / 15) & MASK);
        assert_eq!(p.int_raw, 1); assert_eq!(p.deadline, None); // no representable later absolute u64 deadline
    }
}
