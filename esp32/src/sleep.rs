//! Classic deep sleep only. Sources: ESP-IDF v5.5.4 components/soc/esp32/
//! register/soc/rtc_cntl_reg.h and components/esp_hw_support/port/esp32/rtc_sleep.c.
//! Entry is synchronous at SLEEP_EN; analog sequencing/delays are not simulated.
use crate::bus::SocBus;

impl SocBus {
    #[inline(never)]
    pub(crate) fn enter_deep_sleep(&mut self) {
        let rtc = &self.periph.rtc.0.ram;
        if rtc.read(0x84) & (1 << 31) == 0 {
            return; // DIG_PWC.DG_WRAP_PD_EN: light sleep is not modeled.
        }
        // PWC: SLOWMEM PD_EN/FORCE_PU/FORCE_PD = 17/16/15;
        // FASTMEM = 14/13/12 (rtc_cntl_reg.h:1153-1188).
        // Powered-down RAM becomes zero: deterministic model choice, not silicon contents.
        let pwc = rtc.read(0x80);
        if pwc & (1 << 15) != 0 || pwc & (1 << 17) != 0 && pwc & (1 << 16) == 0 {
            self.rtc_slow.fill(0);
            self.written(0x5000_0000, self.rtc_slow.len());
        }
        if pwc & (1 << 12) != 0 || pwc & (1 << 14) != 0 && pwc & (1 << 13) == 0 {
            self.rtc_fast.fill(0);
            self.written(0x400c_0000, self.rtc_fast.len());
        }
        // The powered-down digital domain cannot keep running a timer watchdog
        // or DMA/radio/PWM engines. Use their existing reset/clock gates once at
        // entry; no sleep condition is added to any device tick.
        self.periph.timg = std::array::from_fn(crate::timers::ClassicTimer::new);
        self.periph.wifi.clock_enabled = false;
        self.periph.ledc.clock_enabled = false;
        self.periph.rmt.clock_enabled = false;
        for i2s in &mut self.periph.i2s {
            i2s.clock_enabled = false;
        }
        self.deep_sleep = true;
        self.ext0_wake();
        self.timer_wake();
    }

    /// Called only on entry and from the tick while asleep. RTC_TIMER_TRIG_EN is bit 3 of the cause:
    /// ESP-IDF v5.5.4 esp_hw_support/port/esp32/include/soc/rtc.h:587.
    pub(crate) fn timer_wake(&mut self) {
        if self.deep_sleep && self.periph.rtc.sleep_ticks_left() == Some(0) { self.wake(1 << 3); }
    }

    fn wake(&mut self, cause: u32) {
        let rtc = &mut self.periph.rtc.0;
        rtc.ram.write(0x34, (rtc.ram.read(0x34) & !0x7ff) | cause);
        rtc.ram.write(0x18, rtc.ram.read(0x18) & !(1 << 31));
        rtc.reset_cause = 5; // esp_rom/esp32/include/esp32/rom/rtc.h DEEPSLEEP_RESET
        rtc.sw_reset = true;
        self.irq_dirty = true;
    }

    /// Called only on entry and GPIO input changes, never from tick/dispatch.
    pub(crate) fn ext0_wake(&mut self) {
        if !self.deep_sleep {
            return;
        }
        let rtc = &mut self.periph.rtc.0;
        // WAKEUP_ENA[22:11], EXT0 trigger bit 0; WAKEUP_CAUSE[10:0].
        // rtc_cntl_reg.h:505-529; soc/rtc.h RTC_EXT0_TRIG_EN.
        if rtc.ram.read(0x34) & (1 << 11) == 0 {
            return;
        }
        let pwc = rtc.ram.read(0x80);
        // RTC peripheral domain must be powered (PWC PD_EN/FORCE_PU/FORCE_PD).
        if pwc & (1 << 18) != 0 || pwc & (1 << 20) != 0 && pwc & (1 << 19) == 0 {
            return;
        }
        let Some(pin) = self.periph.adc.ext0_pad() else {
            return;
        };
        let high = rtc.ram.read(0x60) & (1 << 30) != 0; // EXT_WAKEUP0_LV
        if (self.periph.gpio.gpio.input & (1 << pin) != 0) != high {
            return;
        }
        // rtc_sleep.c:249-270: only GPIO/SDIO can reject on classic, not EXT0.
        // An already active EXT0 level therefore immediately wakes, not SLP_REJECT.
        self.wake(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::Bus as _;

    fn asleep_with_alarm_at(b: &mut SocBus, target: u64) {
        b.write32(0x3ff4_8004, target as u32).unwrap(); // RTC_CNTL_SLP_TIMER0_REG
        b.write32(0x3ff4_8008, (target >> 32) as u32).unwrap(); // no MAIN_TIMER_ALARM_EN, as IDF
        b.write32(0x3ff4_8038, 1 << (11 + 3)).unwrap(); // WAKEUP_ENA: RTC_TIMER_TRIG_EN
        b.write32(0x3ff4_8084, 1 << 31).unwrap();
        b.write32(0x3ff4_8018, 1 << 31).unwrap();
    }

    #[test]
    fn the_rtc_timer_wakes_deep_sleep_at_its_alarm() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        asleep_with_alarm_at(&mut b, 100);
        assert!(b.deep_sleep);
        b.periph.rtc.0.slow_ticks = 99;
        b.tick(1);
        assert!(!b.periph.rtc.0.sw_reset, "not before the alarm");
        b.periph.rtc.0.slow_ticks = 100;
        b.tick(1);
        assert!(b.periph.rtc.0.sw_reset);
        assert_eq!(b.periph.rtc.0.reset_cause, 5);
        assert_eq!(b.read32(0x3ff4_8038).unwrap() & 0x7ff, 1 << 3, "WAKEUP_CAUSE");
        assert_eq!(b.read32(0x3ff4_8018).unwrap() & (1 << 31), 0, "SLEEP_EN clears");
    }

    #[test]
    fn an_alarm_already_past_wakes_at_once() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.periph.rtc.0.slow_ticks = 500;
        asleep_with_alarm_at(&mut b, 100);
        assert!(b.periph.rtc.0.sw_reset);
    }

    #[test]
    fn the_timer_wakes_nothing_unless_enabled_as_a_source() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        asleep_with_alarm_at(&mut b, 100);
        b.write32(0x3ff4_8038, 0).unwrap();
        b.periph.rtc.0.slow_ticks = 500;
        b.periph.rtc.0.sw_reset = false;
        b.tick(1);
        assert!(!b.periph.rtc.0.sw_reset);
    }

    #[test]
    fn a_target_never_written_wakes_nothing() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(0x3ff4_8084, 1 << 31).unwrap();
        b.write32(0x3ff4_8018, 1 << 31).unwrap();
        b.periph.rtc.0.slow_ticks = 500;
        b.tick(1);
        assert!(b.deep_sleep);
        assert!(!b.periph.rtc.0.sw_reset);
    }

    #[test]
    fn a_chip_awake_has_no_alarm_pending() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(0x3ff4_8004, 100).unwrap();
        b.write32(0x3ff4_8038, 1 << (11 + 3)).unwrap();
        assert_eq!(b.periph.rtc.sleep_ticks_left(), None);
    }

    #[test]
    fn an_alarm_past_32_bits_waits_for_its_high_word() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        asleep_with_alarm_at(&mut b, 1 << 32 | 5);
        b.periph.rtc.0.slow_ticks = 5;
        b.tick(1);
        assert!(!b.periph.rtc.0.sw_reset, "the low word alone is not the alarm");
        b.periph.rtc.0.slow_ticks = 1 << 32 | 5;
        b.tick(1);
        assert!(b.periph.rtc.0.sw_reset);
    }

    #[test]
    fn a_timer_wake_keeps_the_wake_enables() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        asleep_with_alarm_at(&mut b, 100);
        b.periph.rtc.0.slow_ticks = 100;
        b.tick(1);
        assert_eq!(b.read32(0x3ff4_8038).unwrap(), 1 << (11 + 3) | 1 << 3, "WAKEUP_ENA kept, WAKEUP_CAUSE timer");
    }

    #[test]
    fn digital_engines_stop_on_power_down() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(0x3ff0_00c0, (1 << 4) | (1 << 21) | (1 << 11) | (1 << 9))
            .unwrap();
        b.write32(0x3ff0_00cc, 0x406).unwrap();
        b.write32(
            0x3ff5_f000,
            (1 << 31) | (1 << 30) | (1 << 11) | (1 << 10) | (2 << 13),
        )
        .unwrap();
        b.write32(0x3ff5_f010, 10).unwrap();
        b.write32(0x3ff5_f098, 1).unwrap();
        assert!(b.periph.wifi.clock_enabled);
        assert!(b.periph.ledc.clock_enabled);
        assert!(b.periph.rmt.clock_enabled);
        assert!(b.periph.i2s.iter().all(|i| i.clock_enabled));
        b.write32(0x3ff4_8084, 1 << 31).unwrap();
        b.write32(0x3ff4_8018, 1 << 31).unwrap();
        b.tick(4096);
        assert_eq!(b.read32(0x3ff5_f09c).unwrap(), 0);
        assert!(!b.periph.wifi.clock_enabled);
        assert!(!b.periph.ledc.clock_enabled);
        assert!(!b.periph.rmt.clock_enabled);
        assert!(b.periph.i2s.iter().all(|i| !i.clock_enabled));
    }
}
