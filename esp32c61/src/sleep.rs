//! C61 PMU/LP timer digital sleep contract, ESP-IDF v5.5.4 (735507283d5b2f9f):
//! soc/esp32c61/register/soc/{pmu,lp_timer,lp_aon,lp_clkrst}_reg.h,
//! hal/esp32c61/include/hal/{pmu,lp_timer,lp_aon}_ll.h and lp_timer_hal.c.
//! Timer wake only; no analog sequencing, GPIO wake, or CPU power-domain retention.
use emu_core::ClockDomain;
use esp_periph::{Device, RegRam, WriteEffect, RST_POWERON, RST_SW_CPU, RST_SW_SYS};

const CPU_HZ: u64 = 160_000_000;
pub const RC_SLOW_HZ: u64 = 136_000; // soc/esp32c61/include/soc/clk_tree_defs.h
const TIMER_MASK: u64 = (1 << 48) - 1;
const TIMER_WAKE: u32 = 8; // esp32c61/rom/rtc.h TIMER_EXPIRE
const WAKE_RAW: u32 = 1 << 31;
const REJECT_RAW: u32 = 1 << 30;
const DEEP_SLEEP_RESET: u32 = 5;

/// C61 TIMG calibration result. PCR CTRL_32K_CONF +0x10c selects the clock;
/// C61 rtc_time.c counts XTAL ticks, using FOSC_TICK_NUM+1 for divided RC_FAST.
/// This avoids calibrating at the shared 150 kHz while the C61 counter uses 136 kHz.
pub fn rtc_calibration_value(timg_config: u32, pcr_32k: u32) -> u32 {
    let samples = u64::from((timg_config >> 16) & 0x7fff);
    let (hz, div) = match pcr_32k & 7 {
        1 | 2 => (32768, 1),
        4 => (17_500_000, u64::from((pcr_32k >> 8) & 0xff) + 1),
        _ => (RC_SLOW_HZ, 1),
    };
    ((samples * 40_000_000 * div / hz) as u32) << 7
}

/// The four existing LP aliases plus PMU (logical offset 0x1000).
/// `tick` and deadlines are CPU-reference cycles, even while the CPU is held.
pub struct LpSys {
    pub ram: RegRam,
    pub reset_cause: u32,
    pub sw_reset: bool,
    pub slow_ticks: u64,
    pub time_latch: u64,
    previous_latch: u64,
    pmu: RegRam,
    fraction: u64,
    armed: [bool; 2],
    sleep: Option<bool>, // Some(false)=light, Some(true)=deep
}
impl Default for LpSys { fn default() -> Self { Self::new() } }
impl LpSys {
    pub const CLKRST: u32 = 0; pub const AON: u32 = 0x400;
    pub const WDT: u32 = 0x800; pub const TIMER: u32 = 0xc00; pub const PMU: u32 = 0x1000;
    pub fn new() -> Self {
        let mut ram = RegRam::new();
        ram.write(0, 4); // LP FAST_CLK_SEL reset=XTAL_D2, SLOW_CLK_SEL=RC_SLOW
        ram.write(0x438, 1 << 30); // CPU_CORE0_STAT_VECTOR_SEL reset
        ram.write(0x3fc, 36766288); ram.write(0x7fc, 37823232); ram.write(0xffc, 36769936);
        let mut pmu = RegRam::new(); pmu.write(0x1a8, 36770128);
        Self { ram, reset_cause: RST_POWERON, sw_reset: false, slow_ticks: 0, time_latch: 0, previous_latch: 0,
            pmu, fraction: 0, armed: [false; 2], sleep: None }
    }
    pub fn sleeping(&self) -> bool { self.sleep.is_some() }
    pub fn deep_sleeping(&self) -> bool { self.sleep == Some(true) }
    /// CPU reset retains LP time, stores and wake status, while ending any CPU hold.
    pub fn finish_reboot(&mut self) { self.sw_reset = false; self.sleep = None; }
    fn hz(&self) -> u64 { if self.ram.read(0) & 3 == 0 { RC_SLOW_HZ } else { 32768 } }
    fn request_reset(&mut self, cause: u32) { self.reset_cause = cause; self.sw_reset = true; self.sleep = None; }
    fn snapshot(&mut self) { self.previous_latch = self.time_latch; self.time_latch = self.slow_ticks & TIMER_MASK; }
    fn target(&self, timer: usize) -> u64 {
        let off = Self::TIMER + timer as u32 * 8;
        u64::from(self.ram.read(off)) | (u64::from(self.ram.read(off + 4) & 0xffff) << 32)
    }
    fn remaining(&self, timer: usize) -> u64 {
        let delta = self.target(timer).wrapping_sub(self.slow_ticks) & TIMER_MASK;
        if delta & (1 << 47) != 0 { 0 } else { delta }
    }
    fn timer_alarm(&mut self, timer: usize) {
        self.armed[timer] = false;
        let raw = if timer == 0 { 0xc28 } else { 0xc38 };
        self.ram.write(raw, self.ram.read(raw) | WAKE_RAW);
    }
    fn wake_pending(&self) -> bool { self.pmu.read(0x12c) & TIMER_WAKE != 0 && self.ram.read(0xc28) & WAKE_RAW != 0 }
    fn finish_sleep(&mut self, rejected: bool) {
        let deep = self.deep_sleeping(); self.sleep = None;
        self.pmu.write(if rejected { 0x148 } else { 0x144 }, TIMER_WAKE);
        self.pmu.write(0x160, self.pmu.read(0x160) | if rejected { REJECT_RAW } else { WAKE_RAW });
        if self.ram.read(0xc10) & (1 << 30) != 0 { self.snapshot(); }
        if deep && !rejected { self.request_reset(DEEP_SLEEP_RESET); }
    }
    fn begin_sleep(&mut self) {
        // lp_aon_ll_inform_wakeup_type uses RTC_SLEEP_MODE_REG = STORE8 bit0;
        // the prose in rom/rtc.h says STORE9, but its actual macro says STORE8.
        self.sleep = Some(self.ram.read(0x420) & 1 != 0);
        self.pmu.write(0x144, 0);
        if self.ram.read(0xc10) & (1 << 30) != 0 { self.snapshot(); }
        if self.wake_pending() {
            let reject = self.pmu.read(0x128);
            self.finish_sleep(reject & (1 << 31) != 0 && reject & TIMER_WAKE != 0);
        }
    }
    pub fn read(&self, off: u32) -> u32 {
        if off & 3 != 0 || off >= 0x1400 { return 0; }
        if off >= Self::PMU {
            return match off - Self::PMU {
                0x124 | 0x134 | 0x16c | 0x17c => 0, // write-trigger registers
                0x164 => self.pmu.read(0x160) & self.pmu.read(0x168),
                0x174 => self.pmu.read(0x170) & self.pmu.read(0x178),
                o => self.pmu.read(o),
            };
        }
        match off {
            0x010 => (self.ram.read(off) & !0x1f) | (self.reset_cause & 0x1f),
            0xc14 => self.time_latch as u32, 0xc18 => (self.time_latch >> 32) as u32 & 0xffff,
            0xc1c => self.previous_latch as u32, 0xc20 => (self.previous_latch >> 32) as u32 & 0xffff,
            0xc2c => self.ram.read(0xc28) & self.ram.read(0xc30),
            0xc3c => self.ram.read(0xc38) & self.ram.read(0xc40),
            0xc34 | 0xc44 => 0,
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        if off & 3 != 0 || off >= 0x1400 { return; }
        if off >= Self::PMU {
            let o = off - Self::PMU;
            match o {
                0x124 => { if v & (1 << 31) != 0 { self.begin_sleep(); } }
                0x134 => { if v & (1 << 31) != 0 { self.pmu.write(0x148, 0); } }
                0x144 | 0x148 | 0x160 | 0x164 | 0x170 | 0x174 => {},
                0x16c => self.pmu.write(0x160, self.pmu.read(0x160) & !v),
                0x17c => self.pmu.write(0x170, self.pmu.read(0x170) & !v),
                0x168 => self.pmu.write(o, v & 0xf8000000),
                0x178 => self.pmu.write(o, v & 0xfff00000),
                _ => self.pmu.write(o, v),
            }
            return;
        }
        match off {
            0x010 => {
                if v & (1 << 29) != 0 { self.reset_cause = 0; }
                let flag = if v & (1 << 31) != 0 { 0 } else if v & (1 << 30) != 0 { 1 << 5 } else { self.ram.read(off) & (1 << 5) };
                self.ram.write(off, flag);
            }
            0x434 => { if v & (1 << 31) != 0 { self.request_reset(RST_SW_SYS); } self.ram.write(off, v & !(1 << 31)); }
            0x438 => { if v & (1 << 28) != 0 { self.request_reset(RST_SW_CPU); } self.ram.write(off, v & !(1 << 28)); }
            0xc04 | 0xc0c => {
                let timer = ((off - 0xc04) / 8) as usize;
                self.ram.write(off, v & 0xffff); self.armed[timer] = v & (1 << 31) != 0;
                if self.armed[timer] && self.remaining(timer) == 0 { self.timer_alarm(timer); }
            }
            0xc10 => { if v & (1 << 27) != 0 { self.snapshot(); } self.ram.write(off, v & 0xf0000000); }
            0xc14..=0xc20 | 0xc28 | 0xc2c | 0xc38 | 0xc3c => {},
            0xc30 | 0xc40 => self.ram.write(off, v & 0xc0000000),
            0xc34 => self.ram.write(0xc28, self.ram.read(0xc28) & !v),
            0xc44 => self.ram.write(0xc38, self.ram.read(0xc38) & !v),
            _ => self.ram.write(off, v),
        }
    }
    pub fn sleep_deadline_cycles(&self) -> Option<u64> {
        let hz = self.hz();
        (0..2).filter(|&i| self.armed[i]).map(|i| {
            let numerator = u128::from(self.remaining(i)) * u128::from(CPU_HZ);
            numerator.saturating_sub(u128::from(self.fraction)).div_ceil(u128::from(hz)).min(u128::from(u64::MAX)) as u64
        }).min()
    }
    pub fn advance_cycles(&mut self, cycles: u64) {
        let scaled = u128::from(cycles) * u128::from(self.hz()) + u128::from(self.fraction);
        let ticks = (scaled / u128::from(CPU_HZ)) as u64; self.fraction = (scaled % u128::from(CPU_HZ)) as u64;
        for timer in 0..2 { if self.armed[timer] && self.remaining(timer) <= ticks { self.timer_alarm(timer); } }
        if ticks > TIMER_MASK - (self.slow_ticks & TIMER_MASK) {
            self.ram.write(0xc28, self.ram.read(0xc28) | REJECT_RAW);
            self.ram.write(0xc38, self.ram.read(0xc38) | REJECT_RAW);
        }
        self.slow_ticks = self.slow_ticks.wrapping_add(ticks) & TIMER_MASK;
        if self.sleeping() && self.wake_pending() { self.finish_sleep(false); }
    }
}
impl Device for LpSys {
    fn read(&mut self, off: u32) -> u32 { LpSys::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { LpSys::write(self, off, v); WriteEffect::NONE }
    // Interrupt order for the SoC alias: PMU, LP_RTC_TIMER. Neither mask gates PMU's wake input.
    fn irq_sources(&self) -> u64 { u64::from(self.read(0x1164) != 0) | (u64::from(self.read(0xc2c) != 0) << 1) }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::Cpu) }
    fn tick(&mut self, cycles: u64) { self.advance_cycles(cycles); }
    fn has_deadline(&self) -> bool { true }
    fn next_deadline(&self) -> Option<u64> { self.sleep_deadline_cycles() }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn alarm(lp: &mut LpSys, at: u64) {
        lp.write(0xc34, WAKE_RAW);
        lp.write(0xc04, (at >> 32) as u32 & 0xffff); lp.write(0xc00, at as u32);
        lp.write(0xc04, ((at >> 32) as u32 & 0xffff) | WAKE_RAW);
    }
    fn sleep(lp: &mut LpSys, deep: bool) {
        lp.write(0x420, u32::from(deep)); lp.write(0x112c, TIMER_WAKE); lp.write(0x116c, u32::MAX); lp.write(0x1124, WAKE_RAW);
    }
    #[test]
    fn light_sleep_wakes_at_exact_fractional_136khz_boundary() {
        let mut lp = LpSys::new(); lp.advance_cycles(19);
        alarm(&mut lp, 27200); sleep(&mut lp, false);
        assert!(lp.sleeping()); assert!(!lp.deep_sleeping());
        assert_eq!(lp.sleep_deadline_cycles(), Some(32_000_000 - 19));
        lp.advance_cycles(32_000_000 - 20); assert!(lp.sleeping()); assert_eq!(lp.slow_ticks, 27199);
        lp.advance_cycles(1); assert!(!lp.sleeping()); assert_eq!(lp.slow_ticks, 27200);
        assert_eq!(lp.read(0x1144), TIMER_WAKE); assert_eq!(lp.read(0x1160), WAKE_RAW);
        assert_eq!(lp.read(0xc28), WAKE_RAW); assert!(!lp.sw_reset); assert_eq!(lp.reset_cause, RST_POWERON);
        assert_eq!(lp.sleep_deadline_cycles(), None);
    }
    #[test]
    fn deep_wake_requests_reset5_and_lp_state_survives_reboot() {
        let mut lp = LpSys::new(); lp.write(0x408, 0x12345678); alarm(&mut lp, 1360); sleep(&mut lp, true);
        assert!(lp.deep_sleeping()); assert!(!lp.sw_reset);
        lp.advance_cycles(1_600_000); assert!(lp.sw_reset); assert!(!lp.sleeping());
        assert_eq!(lp.reset_cause, 5); assert_eq!(lp.read(0x10) & 31, 5);
        lp.finish_reboot(); assert!(!lp.sw_reset); assert_eq!(lp.read(0x408), 0x12345678);
        assert_eq!(lp.slow_ticks, 1360); assert_eq!(lp.read(0x1144), 8);
        // Writing read-only reset-cause bits cannot forge a power-on reason.
        lp.write(0x10, 1); assert_eq!(lp.reset_cause, 5);
        lp.write(0x10, 1 << 29); assert_eq!(lp.reset_cause, 0);
    }
    #[test]
    fn disabled_timer_and_disabled_wakeup_cannot_resume_cpu() {
        let mut lp = LpSys::new(); sleep(&mut lp, false); lp.advance_cycles(CPU_HZ); assert!(lp.sleeping());
        let mut lp = LpSys::new(); alarm(&mut lp, 136); lp.write(0x1124, WAKE_RAW);
        lp.advance_cycles(CPU_HZ); assert!(lp.sleeping()); assert_eq!(lp.read(0xc28), WAKE_RAW); assert_eq!(lp.read(0x1160), 0);
    }
    #[test]
    fn timer_wrap_high_word_and_overflow_are_not_truncated() {
        for now in [(1 << 32) - 2, TIMER_MASK - 1] {
            let mut lp = LpSys::new(); lp.slow_ticks = now;
            alarm(&mut lp, now.wrapping_add(4) & TIMER_MASK); sleep(&mut lp, false);
            let cycles = lp.sleep_deadline_cycles().unwrap(); lp.advance_cycles(cycles - 1); assert!(lp.sleeping());
            lp.advance_cycles(1); assert!(!lp.sleeping()); assert_eq!(lp.slow_ticks, now.wrapping_add(4) & TIMER_MASK);
            assert_eq!(lp.read(0xc28) & REJECT_RAW != 0, now > (1 << 32));
            lp.write(0xc34, REJECT_RAW); assert_eq!(lp.read(0xc28), WAKE_RAW);
        }
        let mut lp = LpSys::new(); lp.slow_ticks = 100;
        alarm(&mut lp, (1 << 32) + 100); sleep(&mut lp, false); lp.advance_cycles(32_000_000); assert!(lp.sleeping());
    }
    #[test]
    fn snapshot_moves_previous_buffer_and_ignores_guest_writes() {
        let mut lp = LpSys::new(); lp.advance_cycles(80_000_000); lp.write(0xc10, 1 << 27);
        assert_eq!(lp.read(0xc14), 68000); assert_eq!(lp.read(0xc1c), 0); assert_eq!(lp.read(0xc10) & (1 << 27), 0);
        lp.advance_cycles(80_000_000); lp.write(0xc10, 1 << 27);
        assert_eq!(lp.read(0xc14), 136000); assert_eq!(lp.read(0xc1c), 68000);
        lp.write(0xc14, 1); lp.write(0xc18, u32::MAX); assert_eq!(lp.read(0xc14), 136000); assert_eq!(lp.read(0xc18), 0);
    }
    #[test]
    fn pending_timer_reject_and_w1c_masks_obey_c61_positions() {
        let mut lp = LpSys::new(); lp.advance_cycles(10000); alarm(&mut lp, 0);
        lp.write(0x1128, WAKE_RAW | TIMER_WAKE); sleep(&mut lp, true);
        assert!(!lp.sleeping()); assert!(!lp.sw_reset); assert_eq!(lp.read(0x1160), REJECT_RAW); assert_eq!(lp.read(0x1148), TIMER_WAKE);
        lp.write(0x1160, WAKE_RAW); assert_eq!(lp.read(0x1160), REJECT_RAW);
        assert_eq!(lp.irq_sources(), 0); lp.write(0x1168, REJECT_RAW); assert_eq!(lp.irq_sources(), 1);
        lp.write(0xc30, WAKE_RAW); assert_eq!(lp.irq_sources(), 3);
        lp.write(0x116c, 0); assert_eq!(lp.irq_sources(), 3); lp.write(0x116c, REJECT_RAW); assert_eq!(lp.irq_sources(), 2);
        lp.write(0xc34, WAKE_RAW); assert_eq!(lp.irq_sources(), 0);
        lp.write(0x1134, WAKE_RAW); assert_eq!(lp.read(0x1148), 0);
        for off in [1, 0x1001, 0x1400, u32::MAX] { lp.write(off, u32::MAX); assert_eq!(lp.read(off), 0); }
    }
    #[test]
    fn clock_selection_and_calibration_use_consistent_nominal_sources() {
        let mut a = LpSys::new(); let mut b = LpSys::new();
        for _ in 0..1000 { a.advance_cycles(37); } b.advance_cycles(37000);
        assert_eq!((a.slow_ticks, a.fraction), (b.slow_ticks, b.fraction));
        let mut lp = LpSys::new(); lp.write(0, 1); lp.advance_cycles(CPU_HZ); assert_eq!(lp.slow_ticks, 32768);
        assert_eq!(rtc_calibration_value(136 << 16, 3) >> 7, 40000);
        assert_eq!(rtc_calibration_value(1024 << 16, 1) >> 7, 1_250_000);
        assert_eq!(rtc_calibration_value(175 << 16, 4 | (31 << 8)) >> 7, 12800);
        let mut lp = LpSys::new(); lp.advance_cycles(u64::MAX); assert!(lp.slow_ticks <= TIMER_MASK);
    }
}
