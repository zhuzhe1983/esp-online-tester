//! Native sleep-mux, LP elapsed/wake timer and requested clock-correction model.
//! Exact factory SDK/ROM: +034 programs LP duration; +038 is elapsed LP cycles;
//! +040 fine/+044 half-slot correction are latched by +030.bit3 after wake.
//! Timings are configurable functional assumptions, NOT silicon calibrated.
//! No guessed IRQ0: correction completion is NOT an HCI/slot-timer event here.
//! DPORT integer LP divider is modeled for the observed A=0/B=1 setting only.
const ON: u32 = 1 << 2;
const CORRECT: u32 = 1 << 3;
const SOFT_WAKE: u32 = 1 << 4;
const STATUS: u32 = 1 << 15;
const REF_HZ: u64 = 240_000_000;
pub struct BtSleep {
    mask: u32,
    raw: u32,
    control: u32,
    programmed: u32,
    sleeping: bool,
    pending: Option<(bool, u64)>,
    latency: u64,
    correction_latency: u64,
    enabled: bool,
    reset: bool,
    elapsed: u32,
    phase: u128,
    counting: bool,
    timer_armed: bool,
    lp_num: u64,
    lp_den: u64,
    lp_config: Option<(u32, u32)>,
    fine: u32,
    coarse: u32,
    correction_valid: bool,
    correction: Option<(u64, u64)>,
    correction_ready: Option<u64>,
    pub entries: u64,
    pub wakes: u64,
    pub timer_wakes: u64,
    pub corrections: u64,
    pub clock_events: u64,
    pub log: bool,
}
impl Default for BtSleep {
    fn default() -> Self {
        Self::with_latency(24000)
    }
}
impl BtSleep {
    pub fn with_latency(latency: u64) -> Self {
        Self {
            mask: 0,
            raw: 0,
            control: 0,
            programmed: 0,
            sleeping: false,
            pending: None,
            latency: latency.max(1),
            correction_latency: 240,
            enabled: false,
            reset: false,
            elapsed: 0,
            phase: 0,
            counting: false,
            timer_armed: false,
            // Standalone test/environment default. SoC immediately configures real DPORT words.
            lp_num: 500_000,
            lp_den: REF_HZ,
            lp_config: None,
            fine: 0,
            coarse: 0,
            correction_valid: false,
            correction: None,
            correction_ready: None,
            entries: 0,
            wakes: 0,
            timer_wakes: 0,
            corrections: 0,
            clock_events: 0,
            log: false,
        }
    }
    /// Nominal source policy follows existing classic clock fixtures: XTAL40MHz,
    /// RC8MHz, XTAL32k32768Hz, RTC slow150kHz. Not measured oscillator calibration.
    /// Fractional A!=0/B!=1 is explicitly unsupported, never silently guessed.
    pub fn configure_lp_clock(&mut self, integer: u32, frac: u32) {
        let cfg = (integer & 0xfff, frac & 0x0fff_ffff);
        if self.lp_config == Some(cfg) {
            return;
        }
        self.lp_config = Some(cfg);
        self.phase = 0;
        self.lp_num = if frac & 0x00ff_ffff != 1 {
            0
        } else {
            match (frac >> 24) & 15 {
                1 => 150_000,
                2 => 8_000_000,
                4 => 40_000_000,
                8 => 32768,
                _ => 0,
            }
        };
        self.lp_den = REF_HZ * (u64::from(integer & 0xfff) + 1);
    }
    /// Functional control latency in reference ticks, not a measured silicon value.
    /// Applies to future requests; an in-flight transaction keeps its deadline.
    pub fn set_correction_latency(&mut self, ticks: u64) {
        self.correction_latency = ticks.max(1);
    }
    pub fn lp_clock_valid(&self) -> bool {
        self.lp_num != 0
    }
    pub fn set_control(&mut self, enabled: bool, reset: bool) {
        if reset {
            let (log, cfg, latency) = (self.log, self.lp_config, self.correction_latency);
            *self = Self::with_latency(self.latency);
            self.log = log;
            self.correction_latency = latency;
            if let Some((n, f)) = cfg {
                self.configure_lp_clock(n, f);
            }
        }
        self.enabled = enabled && !reset;
        self.reset = reset;
    }
    pub fn irq(&self) -> bool {
        self.mask & self.raw != 0
    }
    pub fn active(&self) -> bool {
        self.enabled
            && (self.pending.is_some()
                || self.correction.is_some()
                || self.counting && self.lp_clock_valid())
    }
    /// Level latch from the *live* native CLKN full-slot boundary. Native ROM
    /// IRQ0 dispatches r_ld_fm_clk_isr and wakeup_end, distinct from bit9 EA timer.
    pub fn native_clock_event(&mut self) {
        if self.enabled && !self.reset {
            self.raw |= 1;
            self.clock_events = self.clock_events.wrapping_add(1);
        }
    }
    /// Cross-peripheral rendezvous so counter gating/correction is independent
    /// of the host bus's tick chunk size.
    pub fn next_event_ticks(&self) -> Option<u64> {
        if !self.enabled {
            return None;
        }
        let mut next = self.pending.map(|(_, left)| left);
        if let Some((left, _)) = self.correction {
            next = Some(next.map_or(left, |n| n.min(left)));
        }
        if self.counting && self.timer_armed && self.lp_clock_valid() {
            let left = self.lp_deadline();
            next = Some(next.map_or(left, |n| n.min(left)));
        }
        next
    }
    pub fn take_native_correction(&mut self) -> Option<u64> {
        self.correction_ready.take()
    }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x0c => self.mask,
            0x10 => self.mask & self.raw,
            0x14 => self.raw,
            0x30 => self.control | if self.sleeping { STATUS } else { 0 },
            0x34 => self.programmed,
            0x38 => self.elapsed,
            0x40 => self.fine,
            0x44 => self.coarse,
            _ => 0,
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        if self.log {
            eprintln!("BT_SLEEP_WRITE +{off:03x}={v:08x}");
        }
        if self.reset {
            return;
        }
        match off {
            0x0c => {
                self.mask = v;
                return;
            }
            0x18 => {
                self.raw &= !v;
                return;
            }
            0x34 => {
                self.programmed = v;
                self.timer_armed = self.counting && v != 0;
                return;
            }
            0x40 => {
                self.fine = v & 1023;
                return;
            }
            0x44 => {
                self.coarse = v & 0x8fff_ffff;
                self.correction_valid = v & (1 << 31) != 0;
                return;
            }
            _ => (),
        }
        if off != 0x30 {
            return;
        }
        let old = self.control;
        self.control = v & !STATUS;
        if self.correction.is_some() {
            self.control |= CORRECT;
        }
        if v & CORRECT != 0
            && self.correction.is_none()
            && self.pending.is_none()
            && self.correction_valid
            && self.fine <= 624
            && !self.sleeping
            && !self.counting
        {
            // Exact native ROM conversion encodes 2*floor(us/625) half-slots and
            // fine=625-remainder_us (SDK zero624, older ROM zero0). The
            // codec is ambiguous by1us at624/310; select the SDK zero encoding.
            // This is an explicit functional quantization policy, not chip timing.
            // Decode that observed contract;
            // do not substitute an arbitrary successful timestamp.
            let rem_us = if self.fine == 624 || self.fine == 0 {
                0
            } else {
                625 - self.fine
            };
            let delta = u64::from(self.coarse & 0x0fff_ffff) * 75000 + u64::from(rem_us) * 240;
            self.correction = Some((self.correction_latency, delta));
        }
        if v & SOFT_WAKE != 0 && (self.sleeping || self.pending.is_some()) {
            self.timer_armed = false;
            if !matches!(self.pending, Some((false, _))) {
                self.pending = Some((false, self.latency));
            }
        } else if v & ON == 0 && !self.sleeping {
            self.pending = None;
        } else if old & ON == 0 && v & ON != 0 && !self.sleeping {
            self.pending = Some((true, self.latency));
        }
    }
    fn lp_deadline(&self) -> u64 {
        let counts = self.programmed.wrapping_sub(self.elapsed).max(1);
        let n = u128::from(counts) * u128::from(self.lp_den) - self.phase;
        n.div_ceil(u128::from(self.lp_num))
            .min(u128::from(u64::MAX)) as u64
    }
    pub fn advance(&mut self, mut ticks: u64) {
        if !self.enabled {
            return;
        }
        while ticks > 0 {
            let timer = self.counting && self.timer_armed && self.lp_clock_valid();
            let timer_left = if timer { self.lp_deadline() } else { u64::MAX };
            let mut step = ticks.min(timer_left);
            if let Some((_, left)) = self.pending {
                step = step.min(left);
            }
            if let Some((left, _)) = self.correction {
                step = step.min(left);
            }
            if self.counting && self.lp_clock_valid() {
                let total = self.phase + u128::from(step) * u128::from(self.lp_num);
                self.elapsed = self
                    .elapsed
                    .wrapping_add((total / u128::from(self.lp_den)) as u32);
                self.phase = total % u128::from(self.lp_den);
            }
            if let Some((sleep, left)) = self.pending {
                if step < left {
                    self.pending = Some((sleep, left - step));
                } else {
                    self.pending = None;
                    self.sleeping = sleep;
                    if sleep {
                        self.elapsed = 0;
                        self.phase = 0;
                        self.counting = true;
                        self.timer_armed = self.programmed != 0;
                        self.entries = self.entries.wrapping_add(1);
                    } else {
                        self.counting = false;
                        self.timer_armed = false;
                        self.control &= !(ON | SOFT_WAKE);
                        self.wakes = self.wakes.wrapping_add(1);
                        self.raw |= 4;
                    }
                }
            }
            if let Some((left, delta)) = self.correction {
                if step < left {
                    self.correction = Some((left - step, delta));
                } else {
                    self.correction = None;
                    self.correction_valid = false;
                    self.coarse &= !(1 << 31);
                    self.control &= !CORRECT;
                    self.correction_ready = Some(delta);
                    self.corrections = self.corrections.wrapping_add(1);
                }
            }
            if timer && step == timer_left && self.pending.is_none() && self.counting {
                self.timer_armed = false;
                self.pending = Some((false, self.latency));
                self.timer_wakes = self.timer_wakes.wrapping_add(1);
            }
            ticks -= step;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bit31_is_not_trigger_and_status_is_read_only() {
        let mut d = BtSleep::with_latency(10);
        d.set_control(true, false);
        d.write(0x30, 0x80008000);
        d.advance(1000);
        assert_eq!(d.read(0x30), 0x80000000);
        d.write(0x30, 0x80000007);
        d.advance(9);
        for _ in 0..50 {
            assert_eq!(d.read(0x30) & STATUS, 0);
        }
        d.advance(1);
        assert_ne!(d.read(0x30) & STATUS, 0);
        assert_eq!(d.entries, 1);
    }
    #[test]
    fn gates_reset_and_softwake_have_distinct_timed_transitions() {
        let mut d = BtSleep::with_latency(10);
        d.write(0x30, 7);
        d.advance(100);
        assert_eq!(d.read(0x30) & STATUS, 0);
        d.set_control(true, false);
        d.advance(10);
        d.set_control(false, false);
        assert_ne!(d.read(0x30) & STATUS, 0);
        d.write(0x30, d.read(0x30) | SOFT_WAKE);
        d.advance(100);
        assert_ne!(d.read(0x30) & STATUS, 0);
        d.set_control(true, false);
        d.advance(10);
        assert_eq!(d.read(0x30) & (STATUS | ON | SOFT_WAKE), 0);
        assert_eq!((d.entries, d.wakes), (1, 1));
        d.write(0x30, 7);
        d.set_control(true, true);
        d.write(0x30, 7);
        d.advance(100);
        assert_eq!(d.read(0x30), 0);
    }
    #[test]
    fn real_softwake_latches_maskable_acknowledged_source_six() {
        let mut p = crate::periph::Peripherals::new([0; 6]);
        p.write32(0x3ff000cc, 0x30800);
        p.write32(0x3ff71030, 7);
        p.tick(24000);
        let control = p.read32(0x3ff71030);
        p.write32(0x3ff71030, control | 16);
        p.tick(24000);
        assert_eq!(p.read32(0x3ff71014), 4);
        assert_eq!(p.source_status(0)[0] & (1 << 6), 0);
        p.write32(0x3ff7100c, 4);
        assert_ne!(p.source_status(0)[0] & (1 << 6), 0);
        p.write32(0x3ff71014, 0);
        assert_ne!(p.source_status(0)[0] & (1 << 6), 0);
        p.write32(0x3ff71018, 4);
        assert_eq!(p.source_status(0)[0] & (1 << 6), 0);
    }
    #[test]
    fn soc_duration_configuration_and_domain_reset_do_not_create_timer_interrupts() {
        let mut p = crate::periph::Peripherals::new([0; 6]);
        p.write32(0x3ff000cc, 0x30800);
        p.write32(0x3ff71034, 500);
        p.tick(999999);
        assert_eq!(p.bt_sleep.entries, 0);
        p.write32(0x3ff71030, 7);
        p.tick(24000);
        assert_eq!(p.read32(0x3ff71030) & STATUS, STATUS);
        assert_eq!(p.read32(0x3ff71034), 500);
        p.write32(0x3ff000d0, 1 << 9);
        assert_eq!(p.read32(0x3ff71030), 0);
        assert_eq!(p.read32(0x3ff71034), 0);
    }
    #[test]
    fn bus_wake_irq_edge_ack_alias_and_guest_reboot_are_presented() {
        use emu_core::Bus;
        let mut b = crate::bus::SocBus::new(4 << 20, [0; 6]);
        b.write32(0x3ff000cc, 0x30800).unwrap();
        b.write32(0x3ff7100c, 4).unwrap();
        b.write32(0x3ff71030, 7).unwrap();
        b.tick(24000);
        let v = b.read32(0x3ff71030).unwrap();
        assert_ne!(v & STATUS, 0);
        b.write32(0x3ff71030, v | SOFT_WAKE).unwrap();
        b.irq_dirty = false;
        b.tick(24000);
        assert!(
            b.irq_dirty,
            "autonomous wake invalidates cached IRQ presentation"
        );
        assert_ne!(b.periph.source_status(0)[0] & (1 << 6), 0);
        b.irq_dirty = false;
        b.write32(0x60031018, 4).unwrap();
        assert!(b.irq_dirty);
        assert_eq!(b.periph.source_status(0)[0] & (1 << 6), 0);
        b.write32(0x3ff71030, 7).unwrap();
        esp_soc::SocBus::reboot(&mut b, [0; 6]);
        assert_eq!(b.read32(0x3ff71030).unwrap(), 0);
        assert!(!b.periph.bt_sleep.active());
        assert_eq!(b.periph.source_status(0)[0] & (1 << 6), 0);
    }
}

#[cfg(test)]
mod lp_tests {
    use super::*;
    fn fixture() -> BtSleep {
        let mut b = BtSleep::with_latency(10);
        b.set_control(true, false);
        b
    }
    fn enter(b: &mut BtSleep) {
        b.write(0x30, 7);
        b.advance(10);
        assert_ne!(b.read(0x30) & STATUS, 0);
    }
    fn wake(b: &mut BtSleep) {
        b.write(0x30, b.read(0x30) | SOFT_WAKE);
        b.advance(10);
        assert_eq!(b.read(0x30) & STATUS, 0);
        b.write(0x18, 4);
    }
    #[test]
    fn elapsed_uses_guest_time_reads_are_pure_and_read_only() {
        let mut b = fixture();
        b.advance(1000);
        assert_eq!(b.read(0x38), 0);
        enter(&mut b);
        b.advance(479);
        for _ in 0..100 {
            assert_eq!(b.read(0x38), 0);
        }
        b.advance(1);
        assert_eq!(b.read(0x38), 1);
        b.write(0x38, 999);
        assert_eq!(b.read(0x38), 1);
        b.advance(480 * 6);
        assert_eq!(b.read(0x38), 7);
        wake(&mut b);
        let v = b.read(0x38);
        b.advance(240_000);
        assert_eq!(b.read(0x38), v);
    }
    #[test]
    fn programmed_duration_has_no_idle_irq_but_elapsed_deadline_wakes() {
        let mut b = fixture();
        b.write(0x34, 2);
        b.write(0x0c, 4);
        b.advance(9000);
        assert!(!b.irq());
        enter(&mut b);
        b.advance(959);
        assert_eq!(b.timer_wakes, 0);
        b.advance(1);
        assert_eq!(b.timer_wakes, 1);
        assert_eq!(b.read(0x14), 0);
        b.advance(9);
        assert!(!b.irq());
        b.advance(1);
        assert!(b.irq());
        assert_eq!(b.read(0x14), 4);
        assert_eq!(b.wakes, 1);
        b.write(0x0c, 1);
        assert!(!b.irq());
        b.write(0x18, 1);
        assert_eq!(b.read(0x14), 4);
        b.write(0x18, 4);
        assert_eq!(b.read(0x14), 0);
        b.advance(99999);
        assert_eq!(b.wakes, 1);
    }
    #[test]
    fn soft_wake_cancels_programmed_timer_and_gate_pauses_phase() {
        let mut b = fixture();
        b.write(0x34, 5);
        enter(&mut b);
        b.advance(479);
        b.set_control(false, false);
        b.advance(50000);
        assert_eq!(b.read(0x38), 0);
        b.set_control(true, false);
        b.advance(1);
        assert_eq!(b.read(0x38), 1);
        wake(&mut b);
        b.advance(99999);
        assert_eq!(b.timer_wakes, 0);
        assert_eq!(b.wakes, 1);
    }
    #[test]
    fn elapsed_and_programmed_deadline_wrap_in_u32_domain() {
        let mut b = fixture();
        enter(&mut b);
        b.elapsed = u32::MAX - 1;
        b.advance(480 * 3);
        assert_eq!(b.read(0x38), 1);
        b.elapsed = 0xfffffff0;
        b.phase = 0;
        b.write(0x34, 16);
        b.advance(480 * 31);
        assert_eq!(b.timer_wakes, 0);
        b.advance(480);
        assert_eq!(b.read(0x38), 16);
        assert_eq!(b.timer_wakes, 1);
        b.advance(10);
        assert_eq!(b.read(0x14), 4);
    }
    #[test]
    fn integer_clock_source_selection_and_unsupported_fraction_are_explicit() {
        let mut b = fixture();
        b.configure_lp_clock(79, 0x04000001);
        enter(&mut b);
        b.advance(480);
        assert_eq!(b.read(0x38), 1);
        b.configure_lp_clock(39, 0x04000001);
        b.advance(480);
        assert_eq!(b.read(0x38), 3);
        b.configure_lp_clock(39, 0x04001001);
        assert!(!b.lp_clock_valid());
        b.advance(240000);
        assert_eq!(b.read(0x38), 3);
        b.configure_lp_clock(7, 0x02000001);
        b.advance(240);
        assert_eq!(b.read(0x38), 4);
        b.configure_lp_clock(0, 0x0c000001);
        assert!(!b.lp_clock_valid());
    }
    #[test]
    fn timed_correction_latches_parameters_never_invents_irq_zero() {
        let mut b = fixture();
        enter(&mut b);
        b.advance(480 * 100);
        wake(&mut b);
        b.write(0x0c, 1);
        b.write(0x40, 250);
        b.write(0x44, 0x80000002);
        b.set_correction_latency(17);
        b.write(0x30, 0xb);
        b.advance(16);
        assert_eq!(b.take_native_correction(), None);
        assert_eq!(b.read(0x30) & CORRECT, CORRECT);
        b.write(0x40, 624);
        b.write(0x44, 0x80000008);
        b.write(0x30, 0xb);
        b.advance(1);
        assert_eq!(b.take_native_correction(), Some(240000));
        assert_eq!(b.corrections, 1);
        assert_eq!(b.read(0x30) & CORRECT, 0);
        assert_eq!(b.read(0x14), 0);
        assert!(!b.irq());
        b.advance(10000);
        assert_eq!(b.take_native_correction(), None);
    }
    #[test]
    fn correction_requires_valid_latch_and_is_cancelled_by_reset() {
        let mut b = fixture();
        b.write(0x40, 624);
        b.write(0x30, CORRECT);
        b.advance(999);
        assert_eq!(b.corrections, 0);
        b.write(0x44, 0x80000004);
        b.write(0x30, CORRECT);
        b.advance(100);
        b.set_control(true, true);
        b.advance(9999);
        assert_eq!(b.read(0x38), 0);
        assert_eq!(b.read(0x40), 0);
        assert_eq!(b.read(0x44), 0);
        assert_eq!(b.read(0x30), 0);
        assert_eq!(b.take_native_correction(), None);
        b.write(0x44, 0x80000004);
        assert_eq!(b.read(0x44), 0);
    }
    #[test]
    fn correction_gate_and_latency_fixture_survive_reset_without_early_completion() {
        let mut b = fixture();
        b.set_correction_latency(7);
        b.set_control(true, true);
        b.set_control(true, false);
        b.write(0x40, 624);
        b.write(0x44, 0x80000004);
        b.write(0x30, CORRECT);
        b.advance(6);
        b.set_control(false, false);
        b.advance(1000);
        assert_eq!(b.take_native_correction(), None);
        b.set_control(true, false);
        b.advance(1);
        assert_eq!(b.take_native_correction(), Some(300000));
    }
    #[test]
    fn sleep_timer_and_wake_are_chunk_invariant() {
        let (mut a, mut b) = (fixture(), fixture());
        for x in [&mut a, &mut b] {
            x.write(0x34, 32);
            x.write(0x30, 7);
        }
        a.advance(30000);
        for _ in 0..30000 {
            b.advance(1);
        }
        for off in [0x30, 0x34, 0x38, 0x14] {
            assert_eq!(a.read(off), b.read(off));
        }
        assert_eq!(a.wakes, 1);
        assert_eq!(a.timer_wakes, 1);
    }
    #[test]
    fn soc_lp_domain_runs_with_lc_gated_and_correction_keeps_old_sample_atomic() {
        use crate::periph::Peripherals;
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff000d0, 0);
        p.write32(0x3ff000cc, 1 << 11);
        p.write32(0x3ff000d4, 79);
        p.write32(0x3ff000d8, 0x04000001);
        p.write32(0x3ff71030, 7);
        p.tick(24000);
        p.tick(240000);
        assert_eq!(p.read32(0x3ff71038), 500);
        p.write32(0x3ff71030, 0x8017);
        p.tick(24000);
        assert_eq!(p.read32(0x3ff71038), 550);
        p.write32(0x3ff71018, 4);
        p.write32(0x3ff71040, 624);
        p.write32(0x3ff71044, 0x80000004);
        p.write32(0x3ff71030, 0xb);
        p.tick(239);
        assert_eq!(p.bt_sleep.corrections, 0);
        p.tick(1);
        assert_eq!(p.bt_sleep.corrections, 1);
        assert_eq!(p.read32(0x3ff7101c), 0);
        assert_eq!(p.read32(0x3ff71014), 0);
        p.write32(0x3ff7101c, 1 << 31);
        p.tick(1000);
        assert_eq!(p.bt_clock.completed, 0);
        p.write32(0x3ff000cc, 0x30800);
        p.tick(240);
        assert_eq!(p.bt_clock.completed, 1);
        assert!(p.read32(0x3ff7101c) & 0xfffffff >= 4);
    }
}
