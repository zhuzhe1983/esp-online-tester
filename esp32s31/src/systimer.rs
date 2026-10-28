//! S31 SYSTIMER, fixed IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! systimer_reg.h, hp_sys_clkrst_reg.h, systimer_ll.h and systimer_hal.c.
//! Two52-bit counters, three comparators, paired WT/SS status and /2.5 source clock.
//! Synchronization is functional on the next explicit timer tick, not measured CDC
//! latency. ETM configuration is retained; no ETM consumer or sleep domain is invented.
use emu_core::Fault;
pub const BASE: u32 = 0x2039_9000;
pub const CLOCK: u32 = 0x2058_7120;
const MASK: u64 = (1 << 52) - 1;
#[derive(Default)]
struct Unit { count: u64, latch: u64, load: u64, pending_load: Option<u64>, snapshot: bool, valid: bool }
#[derive(Default)]
struct Alarm { target: u64, conf: u32, active_conf: u32, real: u64, pending: Option<(u64, u32)>, armed: bool }
pub struct Systimer { clock: u32, conf: u32, units: [Unit; 2], alarms: [Alarm; 3], ena: u32, raw: u32, date: u32, fraction: u128, denominator: u128, pub events: [u64; 3], pub ticks: u64 }
impl Default for Systimer {
    fn default() -> Self { Self { clock: 0, conf: 0x46000000, units: std::array::from_fn(|_| Unit::default()), alarms: std::array::from_fn(|_| Alarm::default()), ena: 0, raw: 0, date: 37823088, fraction: 0, denominator: 0, events: [0; 3], ticks: 0 } }
}

impl Systimer {
    pub fn is_mmio(address: u32) -> bool { address & !3 == CLOCK || address & !0xfff == BASE }
    pub fn source_rc_fast(&self) -> bool { self.clock & 8 != 0 }
    fn on(&self) -> bool { self.clock & 0x12 == 0x10 }
    pub fn irqs(&self) -> [bool; 3] { std::array::from_fn(|id| self.raw & self.ena & (1 << id) != 0) }
    pub fn counters(&self) -> [u64; 2] { [self.units[0].count, self.units[1].count] }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        if address == CLOCK { return Ok(self.clock); }
        if self.clock & 3 != 1 { return Err(Fault::Prohibited); }
        match address.wrapping_sub(BASE) {
            0 => Ok(self.conf),
            off @ (4 | 8) => Ok(u32::from(self.units[(off / 4 - 1) as usize].valid) << 29),
            off @ (0xc..=0x18) if off & 3 == 0 => { let u = &self.units[((off - 0xc) / 8) as usize]; Ok(if off & 4 != 0 { (u.load >> 32) as u32 } else { u.load as u32 }) }
            off @ (0x1c..=0x30) if off & 3 == 0 => { let a = &self.alarms[((off - 0x1c) / 8) as usize]; Ok(if off & 4 != 0 { (a.target >> 32) as u32 } else { a.target as u32 }) }
            off @ (0x34 | 0x38 | 0x3c) => Ok(self.alarms[((off - 0x34) / 4) as usize].conf),
            off @ (0x40..=0x4c) if off & 3 == 0 => { let u = &self.units[((off - 0x40) / 8) as usize]; Ok(if off & 4 == 0 { (u.latch >> 32) as u32 } else { u.latch as u32 }) }
            0x50 | 0x54 | 0x58 | 0x5c | 0x60 | 0x6c => Ok(0), // WT triggers never read as stored bits
            0x64 => Ok(self.ena), 0x68 => Ok(self.raw), 0x70 => Ok(self.raw & self.ena),
            off @ (0x74..=0x88) if off & 3 == 0 => { let a = &self.alarms[((off - 0x74) / 8) as usize]; Ok(if off & 4 != 0 { a.real as u32 } else { (a.real >> 32) as u32 }) }
            0xfc => Ok(self.date), _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address == CLOCK {
            if value & 4 != 0 { return Err(Fault::Prohibited); } // FORCE_NORST/global reset precedence unknown
            if value & 2 != 0 { *self = Self::default(); }
            self.clock = value & 0x1b; return Ok(());
        }
        if self.clock & 3 != 1 { return Err(Fault::Prohibited); }
        match address.wrapping_sub(BASE) {
            0 => self.conf = value & 0xffc00002,
            off @ (4 | 8) => if value & (1 << 30) != 0 { let u = &mut self.units[(off / 4 - 1) as usize]; u.snapshot = true; u.valid = false; },
            off @ (0xc..=0x18) if off & 3 == 0 => { let u = &mut self.units[((off - 0xc) / 8) as usize]; Self::half(&mut u.load, off & 4 != 0, value); }
            off @ (0x1c..=0x30) if off & 3 == 0 => { let a = &mut self.alarms[((off - 0x1c) / 8) as usize]; Self::half(&mut a.target, off & 4 != 0, value); }
            off @ (0x34 | 0x38 | 0x3c) => self.alarms[((off - 0x34) / 4) as usize].conf = value & 0xc3ffffff,
            off @ (0x50 | 0x54 | 0x58) => if value & 1 != 0 {
                let a = &mut self.alarms[((off - 0x50) / 4) as usize];
                if a.conf & 0x43ffffff == 0x40000000 { return Err(Fault::Prohibited); } // unspecified zero-period comparator
                a.pending = Some((a.target, a.conf));
            },
            off @ (0x5c | 0x60) => if value & 1 != 0 { let u = &mut self.units[((off - 0x5c) / 4) as usize]; u.pending_load = Some(u.load); },
            0x64 => self.ena = value & 7, 0x6c => self.raw &= !(value & 7),
            0x40..=0x4c | 0x68 | 0x70 | 0x74..=0x88 => {}, // RO/SS/WTC only changed by their paired hardware trigger
            0xfc => self.date = value & 0x0fffffff,
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    fn half(word: &mut u64, high: bool, value: u32) { *word = if high { (*word & 0xffffffff) | (u64::from(value & 0xfffff) << 32) } else { (*word & !0xffffffff) | u64::from(value) }; }
    fn running(&self, unit: usize, stalled: [bool; 2]) -> bool {
        self.conf & (1 << (30 - unit)) != 0 && !(0..2).any(|core| stalled[core] && self.conf & (1 << (28 - unit * 2 - core)) != 0)
    }
    /// Explicit timer-domain edges. APB and automatic register-clock policy do
    /// not stop the independent counter. Software stall input is separate from WFI.
    pub fn advance_ticks(&mut self, ticks: u64, stalled: [bool; 2]) {
        if ticks == 0 || !self.on() { return; }
        self.ticks = self.ticks.wrapping_add(ticks);
        let running = std::array::from_fn::<_, 2, _>(|u| self.running(u, stalled));
        // Apply synchronization requests at the first edge; loading replaces
        // that edge's increment. Unloaded counters continue from their real value.
        let mut advances = [0; 2]; let mut loaded = [false; 2];
        for (id, u) in self.units.iter_mut().enumerate() {
            advances[id] = if running[id] { ticks } else { 0 };
            if let Some(value) = u.pending_load.take() { u.count = value; loaded[id] = true; advances[id] = advances[id].saturating_sub(1); }
        }
        for (id, a) in self.alarms.iter_mut().enumerate() {
            if let Some((target, conf)) = a.pending.take() {
                a.active_conf = conf; let unit = (a.active_conf >> 31) as usize;
                a.real = if a.active_conf & (1 << 30) != 0 { self.units[unit].count.wrapping_add(u64::from(a.active_conf & 0x3ffffff)) & MASK } else { target };
                a.armed = true;
            }
            if !a.armed || self.conf & (1 << (24 - id)) == 0 { continue; }
            let unit = (a.active_conf >> 31) as usize;
            if !running[unit] { continue; }
            let now = self.units[unit].count; let periodic = a.active_conf & (1 << 30) != 0;
            let until = if periodic { a.real.wrapping_sub(now) & MASK } else { a.real.saturating_sub(now) };
            if advances[unit] >= until {
                let events = if periodic { 1 + u128::from(advances[unit] - until) / u128::from(a.active_conf & 0x3ffffff) } else { 1 };
                self.raw |= 1 << id; self.events[id] = self.events[id].wrapping_add(events as u64);
                if periodic { a.real = ((u128::from(a.real) + events * u128::from(a.active_conf & 0x3ffffff)) & u128::from(MASK)) as u64; }
                else { a.armed = false; }
            }
        }
        for (id, u) in self.units.iter_mut().enumerate() {
            if u.snapshot { u.latch = u.count.wrapping_add(if loaded[id] { 0 } else { advances[id].min(1) }) & MASK; u.valid = true; u.snapshot = false; }
            u.count = u.count.wrapping_add(advances[id]) & MASK;
        }
    }
    /// Optional deterministic instruction-clock approximation, not wall time or
    /// measured S31 timing. One quantum per dual-hart round, divided by exactly2.5.
    pub fn advance_cpu(&mut self, cycles: u64, cpu_hz: u64, rc_fast_hz: u64, stalled: [bool; 2]) {
        if cpu_hz == 0 || !self.on() { return; }
        let source = if self.source_rc_fast() { rc_fast_hz } else { 40_000_000 };
        let denominator = u128::from(cpu_hz) * 5;
        if self.denominator != 0 && self.denominator != denominator { self.fraction = self.fraction * denominator / self.denominator; }
        self.denominator = denominator;
        let total = self.fraction + u128::from(cycles) * u128::from(source) * 2;
        self.fraction = total % denominator;
        self.advance_ticks((total / denominator).min(u128::from(u64::MAX)) as u64, stalled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn timer() -> Systimer { let mut t = Systimer::default(); t.write(CLOCK, 0x11).unwrap(); t }
    fn w(t: &mut Systimer, off: u32, value: u32) { t.write(BASE + off, value).unwrap(); }
    fn r(t: &Systimer, off: u32) -> u32 { t.read(BASE + off).unwrap() }
    #[test]
    fn snapshot_requires_real_clock_and_reload_masks_52bits_without_read_progress() {
        let mut t = timer(); assert_eq!(r(&t, 4), 0); w(&mut t, 4, 1 << 29); assert_eq!(r(&t, 4), 0);
        w(&mut t, 4, 1 << 30); for _ in 0..10 { assert_eq!(r(&t, 4), 0); }
        t.advance_cpu(2, 40_000_000, 0, [false; 2]); assert_eq!(r(&t, 4), 0);
        t.advance_cpu(3, 40_000_000, 0, [false; 2]); assert_eq!(t.counters(), [2, 0]); assert_eq!(r(&t, 0x44), 1); assert_eq!(r(&t, 4), 1 << 29);
        w(&mut t, 0xc, u32::MAX); w(&mut t, 0x10, u32::MAX); w(&mut t, 0x5c, 0); t.advance_ticks(1, [false; 2]); assert_eq!(t.counters()[0], 3);
        w(&mut t, 0x5c, 1); w(&mut t, 4, 1 << 30); t.advance_ticks(2, [false; 2]); assert_eq!(t.counters()[0], 0);
        assert_eq!(r(&t, 0x40), 0xfffff); assert_eq!(r(&t, 0x44), u32::MAX); w(&mut t, 0x44, 0); assert_eq!(r(&t, 0x44), u32::MAX);
    }
    #[test]
    fn independent_bus_source_and_counter_gates_reset_and_stall_are_observed() {
        let mut t = timer(); w(&mut t, 0, 0x6e000000); // both counters; unit0 stalls only for core1, unit1 for either
        t.advance_ticks(3, [true, false]); assert_eq!(t.counters(), [3, 0]);
        t.advance_ticks(2, [false, true]); assert_eq!(t.counters(), [3, 0]);
        t.write(CLOCK, 0x10).unwrap(); assert_eq!(t.read(BASE), Err(Fault::Prohibited)); t.advance_ticks(3, [false; 2]); assert_eq!(t.counters(), [6, 3]);
        t.write(CLOCK, 1).unwrap(); t.advance_ticks(10, [false; 2]); assert_eq!(t.counters(), [6, 3]);
        t.write(CLOCK, 0x19).unwrap(); t.advance_cpu(10, 40_000_000, 0, [false; 2]); assert_eq!(t.counters(), [6, 3]);
        t.advance_cpu(40, 40_000_000, 17_500_000, [false; 2]); assert_eq!(t.counters(), [13, 10]);
        t.write(CLOCK, 0x13).unwrap(); assert_eq!(t.counters(), [0; 2]); assert_eq!(t.read(BASE), Err(Fault::Prohibited));
        t.write(CLOCK, 0x11).unwrap(); assert_eq!(r(&t, 0), 0x46000000); assert_eq!(r(&t, 4), 0);
        assert_eq!(t.write(CLOCK, 0x15), Err(Fault::Prohibited)); assert_eq!(t.read(BASE + 0x90), Err(Fault::Unmapped));
    }
    #[test]
    fn periodic_wrap_and_missed_oneshot_latch_actual_targets_and_clear_only_via_clr() {
        let mut t = timer(); w(&mut t, 0xc, 0xfffff); w(&mut t, 0x10, 0xfffffffc); w(&mut t, 0x5c, 1); t.advance_ticks(1, [false; 2]);
        w(&mut t, 0x34, 1 << 30 | 3); w(&mut t, 0x50, 1); w(&mut t, 0x34, 0); // active copy must survive later buffer writes
        w(&mut t, 0, 0x41000000); t.advance_ticks(10, [false; 2]); assert_eq!(t.counters()[0], 6); assert_eq!(t.events, [3, 0, 0]); assert_eq!(r(&t, 0x74), 8);
        assert_eq!(t.irqs(), [false; 3]); w(&mut t, 0x64, 1); assert_eq!(t.irqs(), [true, false, false]);
        w(&mut t, 0x68, 1); assert_eq!(r(&t, 0x68), 1); w(&mut t, 0x70, 0); assert_eq!(r(&t, 0x70), 1);
        w(&mut t, 0x6c, 1); assert_eq!(t.irqs(), [false; 3]); t.advance_ticks(1, [false; 2]); assert_eq!(r(&t, 0x68), 0); t.advance_ticks(1, [false; 2]); assert_eq!(t.events[0], 4);
        w(&mut t, 0x6c, 1); w(&mut t, 0, 0x40000000); w(&mut t, 0x20, 1); w(&mut t, 0x50, 1); t.advance_ticks(1, [false; 2]); assert_eq!(t.events[0], 4);
        w(&mut t, 0, 0x41000000); t.advance_ticks(1, [false; 2]); assert_eq!(t.events[0], 5); t.advance_ticks(100, [false; 2]); assert_eq!(t.events[0], 5);
        w(&mut t, 0x34, 1 << 30); assert_eq!(t.write(BASE + 0x50, 1), Err(Fault::Prohibited));
    }
    #[test]
    fn batched_edges_match_individual_edges_and_clock_rate_changes_keep_fraction() {
        let mut a = timer(); let mut b = timer();
        for t in [&mut a, &mut b] { w(t, 0x34, 1 << 30 | 7); w(t, 0x50, 1); w(t, 0, 0x41000000); w(t, 4, 1 << 30); }
        a.advance_ticks(1000, [false; 2]); for _ in 0..1000 { b.advance_ticks(1, [false; 2]); }
        assert_eq!(a.counters(), b.counters()); assert_eq!(a.events, b.events); assert_eq!(r(&a, 0x74), r(&b, 0x74)); assert_eq!(r(&a, 0x44), r(&b, 0x44));
        let mut t = timer(); t.advance_cpu(1, 40_000_000, 0, [false; 2]); t.advance_cpu(3, 80_000_000, 0, [false; 2]); assert_eq!(t.counters()[0], 1);
    }
    #[test]
    fn separate_units_and_three_sources_survive_large_elapsed_input() {
        let mut t = timer(); w(&mut t, 0, 0x61c00000); // both units, all comparators, no stall dependency
        for (id, unit) in [(0, 0), (1, 1), (2, 1)] {
            w(&mut t, 0x34 + id * 4, unit << 31 | 1 << 30 | 1); w(&mut t, 0x50 + id * 4, 1);
        }
        w(&mut t, 0x64, 7); t.advance_ticks(1, [false; 2]); assert_eq!(t.events, [1; 3]); assert_eq!(t.irqs(), [true; 3]);
        w(&mut t, 0x6c, 2); assert_eq!(t.irqs(), [true, false, true]);
        // A counter reload can put the active target exactly at now. The elapsed
        // input must not overflow events+1 or panic for any caller-supplied u64.
        w(&mut t, 0x10, 2); w(&mut t, 0x5c, 1); t.advance_ticks(u64::MAX, [false; 2]);
        assert_eq!(t.irqs(), [true; 3]); assert_eq!(t.counters()[0], 0);
    }
    #[test]
    fn both_harts_receive_actual_alarm_levels_with_permissions_and_alignment() {
        use emu_core::Bus;
        let mut b = crate::S31Bus::new(); b.write32(CLOCK, 0x11).unwrap();
        b.write32(BASE + 0x20, 3).unwrap(); b.write32(BASE + 0x50, 1).unwrap(); b.write32(BASE + 0x64, 1).unwrap(); b.write32(BASE, 0x41000000).unwrap();
        for core in 0..2 { b.interrupts.matrix_write(core, 33 * 4, 0x211); b.interrupts.clic[core].write(0x1044, 1 << 29 | 1 << 8, 15); }
        b.cores.write(crate::core_control::CTRL1, 3).unwrap(); b.systimer.advance_ticks(3, [false; 2]);
        for core in 0..2 { b.set_hart(core); assert!(b.refresh_irq(0).is_some()); }
        b.write32(BASE + 0x6c, 1).unwrap(); assert_eq!(b.refresh_irq(0), None);
        assert_eq!(b.read16(BASE), Err(Fault::Misaligned)); b.permissions.write(crate::pms::HP1 + 0xff0, 1).unwrap(); b.permissions.write(crate::pms::HP1 + 0x60, 0).unwrap();
        assert_eq!(b.read32(BASE), Err(Fault::Prohibited)); assert_eq!(b.read32(CLOCK), Ok(0x11));
    }
}
