//! Timer group: two 54-bit timers on APB with prescaler and alarm, plus the RTC calibration register.
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;
use crate::{RTC_SLOW_HZ, XTAL_HZ};
use emu_core::ClockDomain;

// ------------------------------------------------------------------ Timer group (T0/T1 + WDT + RTC calibration)
pub struct TimerGroup {
    ram: RegRam,
    pub t: [Timer; 2],
    pub int_raw: u32, pub int_ena: u32,
}
#[derive(Default, Clone, Copy)]
pub struct Timer { pub config: u32, pub count: u64, pub latch: u64, pub alarm: u64, pub load: u64, pub prescale_acc: u64 }
impl Timer {
    pub fn read(&self, o: u32) -> u32 {
        match o { 0x0 => self.config, 0x4 => self.latch as u32, 0x8 => (self.latch >> 32) as u32, 0x10 => self.alarm as u32, 0x14 => (self.alarm >> 32) as u32, 0x18 => self.load as u32, 0x1c => (self.load >> 32) as u32, _ => 0 }
    }
    pub fn write(&mut self, o: u32, v: u32, width_mask: u64) {
        match o {
            0x0 => self.config = v,
            0xc => self.latch = self.count,
            0x10 => self.alarm = (self.alarm & !0xffff_ffff) | v as u64, 0x14 => self.alarm = (self.alarm & 0xffff_ffff) | ((v as u64 & (width_mask >> 32)) << 32),
            0x18 => self.load = (self.load & !0xffff_ffff) | v as u64, 0x1c => self.load = (self.load & 0xffff_ffff) | ((v as u64 & (width_mask >> 32)) << 32),
            0x20 => self.count = self.load,
            _ => {}
        }
    }
    /// Advances by `apb_ticks`, calling `on_alarm` when the alarm fires. Inlined so each caller keeps
    /// its own constant `width_mask` (54 bits on S3/C3/C6, 64 on the classic ESP32).
    #[inline(always)]
    pub fn step(&mut self, apb_ticks: u64, width_mask: u64, mut on_alarm: impl FnMut()) {
        let t = self;
        if t.config & (1 << 31) == 0 { return; }   // TIMG_T0_EN
        let div = Self::divisor(t.config);
        t.prescale_acc += apb_ticks;
        let inc = t.config & (1 << 30) != 0;   // TIMG_T0_INCREASE
        let alarm = t.alarm;
        // A one-shot alarm armed at or behind the counter fires at once ("If the current count
        // value has exceeded the alarm value, the alarm event will be triggered immediately",
        // IDF v5.5.1 docs/en/api-reference/peripherals/gptimer.rst). esp-rtos arms 0 ticks, on a
        // counter it has just reset to 0, for a wakeup that is already due.
        // One-shot only, on purpose: an auto-reload alarm armed at or behind the counter still
        // waits for the wrap, because firing it at once would also mean modelling the reload that
        // follows. Nothing arms one there: esp-hal 1.2.2's `PeriodicTimer::start` resets the
        // counter to 0 and arms one period ahead, and IDF's gptimer arms ahead too.
        let reached = if inc { t.count >= alarm } else { t.count <= alarm };
        if t.config & (1 << 10) != 0 && t.config & (1 << 29) == 0 && reached {   // ALARM_EN, no autoreload
            on_alarm();
            t.config &= !(1 << 10);
        }
        // Below the divisor no counter step is due: keep the residual and skip the division,
        // which a tick shorter than the divisor would otherwise pay for a zero quotient.
        if t.prescale_acc < div { return; }
        let mut steps = t.prescale_acc / div;
        t.prescale_acc %= div;
        // Steps to the alarm from `c`, if counting reaches it (not already at or past it).
        let gap = |c: u64| if inc { alarm.checked_sub(c) } else { c.checked_sub(alarm) }.filter(|&d| d > 0);
        if t.config & (1 << 10) != 0 {   // TIMG_T0_ALARM_EN
            if let Some(d) = gap(t.count).filter(|&d| d <= steps) {
                on_alarm();
                steps -= d;
                if t.config & (1 << 29) != 0 {   // autoreload
                    // q256: a tick can be longer than the reload period, so the steps after the
                    // alarm count from `load` and cross again every `gap(load)` steps.
                    t.count = t.load;
                    if let Some(p) = gap(t.load) { steps %= p; }
                } else { t.count = t.alarm; t.config &= !(1 << 10); }
            }
        }
        t.count = if inc { t.count.wrapping_add(steps) & width_mask } else { t.count.wrapping_sub(steps) & width_mask };
    }
    /// Counter steps to an armed alarm that counting will reach, and the prescaler divider.
    #[inline(always)]
    pub fn alarm_gap(&self) -> Option<(u64, u64)> {
        if self.config & (1 << 31) == 0 || self.config & (1 << 10) == 0 { return None; }   // TIMG_T0_EN, ALARM_EN
        let div = Self::divisor(self.config);
        let one_shot = self.config & (1 << 29) == 0;
        let steps = if self.config & (1 << 30) != 0 {
            if self.count >= self.alarm { return one_shot.then_some((0, div)) } else { self.alarm - self.count }
        } else if self.count <= self.alarm { return one_shot.then_some((0, div)) } else { self.count - self.alarm };
        Some((steps, div))
    }
    /// The prescaler's divisor from TnCONFIG.DIVIDER: "when TIMGn_Tx_DIVIDER is either 1 or 2, the
    /// clock divisor is 2; when TIMGn_Tx_DIVIDER is 0, the clock divisor is 65536" (the ESP32 TRM,
    /// 11.2.1, as esp-hal 1.2.2 `src/timer/timg.rs` quotes it and computes its ticks). IDF writes 0
    /// for 65536 and never 1 (IDF v5.5.1 components/hal/esp32s3/include/hal/timer_ll.h).
    #[inline(always)]
    fn divisor(config: u32) -> u64 {
        match (config >> 13) & 0xffff { 0 => 65536, 1 => 2, d => u64::from(d) }
    }
    /// APB ticks to the alarm, saturating: 64-bit counters (classic ESP32) can be further away
    /// than `u64::MAX` APB ticks.
    pub fn deadline(&self) -> Option<u64> {
        self.alarm_gap().map(|(steps, div)| steps.saturating_mul(div).saturating_sub(self.prescale_acc))
    }
}
impl TimerGroup {
    /// TnCONFIG resets to 0x6000_2000: counting up, auto-reload, divider 1 (IDF v5.5.4
    /// components/soc/esp32s3/register/soc/timer_group_reg.h; the C3's and C6's are the same).
    /// esp-hal keeps the divider it finds, so a zero here (divisor 65536) made every alarm shorter
    /// than 1.6 ms round to zero ticks and never fire.
    pub fn new() -> Self {
        let t = Timer { config: 0x6000_2000, ..Timer::default() };
        TimerGroup { ram: RegRam::new(), t: [t; 2], int_raw: 0, int_ena: 0 }
    }
    pub fn tick(&mut self, apb_ticks: u64) {
        for i in 0..2 {
            self.t[i].step(apb_ticks, (1 << 54) - 1, || self.int_raw |= 1 << i);
        }
    }
    pub fn read(&mut self, off: u32) -> u32 {
        let (i, o) = if off < 0x24 { (0usize, off) } else if off < 0x48 { (1usize, off - 0x24) } else { (2, off) };
        if i < 2 {
            return self.t[i].read(o);
        }
        match off {
            0x68 => (self.ram.read(off) & !(1 << 15)) | (1 << 15),                       // RTCCALICFG: always RDY
            0x6c => { let n = (self.ram.read(0x68) >> 16) & 0x7fff; ((n as u64 * XTAL_HZ / RTC_SLOW_HZ) as u32) << 7 }   // RTCCALICFG1 value
            0x70 => self.int_ena, 0x74 => self.int_raw, 0x78 => self.int_raw & self.int_ena,
            0xf8 => 0x2006191,
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        let (i, o) = if off < 0x24 { (0usize, off) } else if off < 0x48 { (1usize, off - 0x24) } else { (2, off) };
        if i < 2 {
            self.t[i].write(o, v, (1 << 54) - 1);
            return;
        }
        match off {
            0x70 => self.int_ena = v, 0x7c => self.int_raw &= !v,
            _ => self.ram.write(off, v),
        }
    }
}
impl Default for TimerGroup { fn default() -> Self { Self::new() } }

impl Device for TimerGroup {
    fn read(&mut self, off: u32) -> u32 { TimerGroup::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { TimerGroup::write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { (self.int_raw & self.int_ena & 3) as u64 }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::Apb) }
    fn tick(&mut self, apb_ticks: u64) { TimerGroup::tick(self, apb_ticks) }
    /// APB ticks until the earliest armed alarm.
    fn has_deadline(&self) -> bool { true }
    fn next_deadline(&self) -> Option<u64> {
        let mut best: Option<u64> = None;
        for t in &self.t {
            let Some((steps, div)) = t.alarm_gap() else { continue };
            let apb = (steps * div).saturating_sub(t.prescale_acc);
            best = Some(best.map_or(apb, |b| b.min(apb)));
        }
        best
    }
}

#[cfg(test)]
mod prescaler_tests {
    use super::Timer;

    /// `Timer::step` as it was before the sub-divisor early return: it always divides.
    fn reference(t: &mut Timer, apb_ticks: u64, width_mask: u64, alarms: &mut u32) {
        if t.config & (1 << 31) == 0 { return; }
        let div = Timer::divisor(t.config);
        t.prescale_acc += apb_ticks;
        let mut steps = t.prescale_acc / div;
        t.prescale_acc %= div;
        let inc = t.config & (1 << 30) != 0;
        let alarm = t.alarm;
        let reached = if inc { t.count >= alarm } else { t.count <= alarm };
        if t.config & (1 << 10) != 0 && t.config & (1 << 29) == 0 && reached {
            *alarms += 1;
            t.config &= !(1 << 10);
        }
        if steps == 0 { return; }
        let gap = |c: u64| if inc { alarm.checked_sub(c) } else { c.checked_sub(alarm) }.filter(|&d| d > 0);
        if t.config & (1 << 10) != 0 {
            if let Some(d) = gap(t.count).filter(|&d| d <= steps) {
                *alarms += 1;
                steps -= d;
                if t.config & (1 << 29) != 0 {
                    t.count = t.load;
                    if let Some(p) = gap(t.load) { steps %= p; }
                } else { t.count = t.alarm; t.config &= !(1 << 10); }
            }
        }
        t.count = if inc { t.count.wrapping_add(steps) & width_mask } else { t.count.wrapping_sub(steps) & width_mask };
    }

    fn state(t: &Timer, alarms: u32) -> (u32, [u64; 5], u32) {
        (t.config, [t.count, t.latch, t.alarm, t.load, t.prescale_acc], alarms)
    }

    #[test]
    fn short_prescaler_advances_match_division_reference() {
        // Divider fields 0 and 1 encode 65536 and 2. A rewritten divider can leave a residual at
        // or above the new divisor; a zero-tick step must still consume it.
        for field in [0u32, 1, 2, 3, 40, 80, 65535] {
            let div = Timer::divisor(field << 13);
            for mask in [(1u64 << 54) - 1, u64::MAX] {
                for flags in 0..16u32 {
                    for residual in [0, div - 1, div, 2 * div + 1] {
                        for alarm in [97, 100, 103] {
                            let config = (field << 13) | ((flags & 1) << 31) | (((flags >> 1) & 1) << 30) | (((flags >> 2) & 1) << 29) | (((flags >> 3) & 1) << 10);
                            let mut a = Timer { config, count: 100, latch: 123, alarm, load: 100, prescale_acc: residual };
                            let mut b = a;
                            let (mut got, mut want) = (0, 0);
                            for ticks in [0, 1, div - 1, 1, div, 3 * div, u32::MAX as u64] {
                                a.step(ticks, mask, || got += 1);
                                reference(&mut b, ticks, mask, &mut want);
                                assert_eq!(state(&a, got), state(&b, want), "field={field} flags={flags} residual={residual} alarm={alarm} ticks={ticks}");
                            }
                        }
                    }
                }
            }
        }
        let mut seed = 0x1234_5678_9abc_def0u64;
        for i in 0..100_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mut a = Timer { config: seed as u32, count: seed.rotate_left(7), latch: seed, alarm: seed.rotate_left(23), load: seed.rotate_left(41), prescale_acc: (seed >> 32) & 0x1ffff };
            let mut b = a;
            let (mut got, mut want) = (0, 0);
            let mask = if i & 1 == 0 { u64::MAX } else { (1u64 << 54) - 1 };
            let ticks = (seed >> 13) & 0x3ffff;
            a.step(ticks, mask, || got += 1);
            reference(&mut b, ticks, mask, &mut want);
            assert_eq!(state(&a, got), state(&b, want), "case={i}");
        }
    }
}
