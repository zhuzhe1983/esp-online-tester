//! Classic ESP32 timer-group layout: T0/T1 and LACT.
//! ESP-IDF v5.5.4 components/soc/esp32/register/soc/timer_group_reg.h:
//! T0 edge/level bits 12/11 (42-53), LACT enable/direction/reload/divider (397-420),
//! alarm enable bit 10 (433-438). Timing and reset completion are inferred.
use emu_core::ClockDomain;
use esp_periph::{Device, TimerGroup, WriteEffect};
use esp_periph::timg::Timer;

pub struct ClassicTimer {
    pub timer: TimerGroup,
    lact: Timer,
    lact_rtc: u32,
}

impl ClassicTimer {
    pub fn new(_group: usize) -> Self {
        Self { timer: TimerGroup::new(), lact: Timer::default(), lact_rtc: 0 }
    }

    // Lines follow INT_RAW and the per-timer EDGE/LEVEL_INT_EN bits; INT_ENA masks only INT_ST.
    // esp-hal 1.1.2 src/timer/timg.rs:520 says INT_ENA is ineffective on ESP32; a T0 level
    // interrupt masking test confirms this on silicon (PR #314). Edge lines and LACT extrapolate
    // the rule. IDF 5.x can mask an armed alarm with INT_ENA alone and leave the line asserted.
    fn irq_bits(&self) -> u64 {
        let pending = self.timer.int_raw;
        let mut bits = 0;
        for n in 0..2 {
            if pending & (1 << n) != 0 {
                let config = self.timer.t[n].config;
                if config & (1 << 11) != 0 {
                    bits |= 1 << n;
                }
                if config & (1 << 12) != 0 {
                    bits |= 1 << (4 + n);
                }
            }
        }
        if pending & (1 << 3) != 0 {
            if self.lact.config & (1 << 11) != 0 {
                bits |= 1 << 3;
            }
            if self.lact.config & (1 << 12) != 0 {
                bits |= 1 << 7;
            }
        }
        bits
    }
}

impl Device for ClassicTimer {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x00..=0x47 => self.timer.t[(off / 0x24) as usize].read(off % 0x24),
            0x70 => self.lact.config,
            0x74 => self.lact_rtc,
            0x78..=0x94 => self.lact.read(off - 0x74),
            0x98 => self.timer.int_ena,
            0x9c => self.timer.int_raw,
            0xa0 => self.timer.int_raw & self.timer.int_ena,
            _ => self.timer.read(off),
        }
    }

    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        match off {
            0x00..=0x47 => self.timer.t[(off / 0x24) as usize].write(off % 0x24, value, u64::MAX),
            0x70 => self.lact.config = value,
            0x74 => self.lact_rtc = value,
            0x78..=0x94 => self.lact.write(off - 0x74, value, u64::MAX),
            0x98 => self.timer.int_ena = value,
            0xa4 => self.timer.int_raw &= !value,
            _ => self.timer.write(off, value),
        }
        WriteEffect::NONE
    }

    fn irq_sources(&self) -> u64 {
        self.irq_bits()
    }

    fn clock(&self) -> Option<ClockDomain> {
        Some(ClockDomain::Apb)
    }

    fn tick(&mut self, ticks: u64) {
        let TimerGroup { t, int_raw, .. } = &mut self.timer;
        for (i, timer) in t.iter_mut().enumerate() {
            timer.step(ticks, u64::MAX, || *int_raw |= 1 << i);
        }
        self.lact.step(ticks, u64::MAX, || *int_raw |= 1 << 3);
    }

    fn has_deadline(&self) -> bool {
        true
    }

    fn next_deadline(&self) -> Option<u64> {
        // Timer::deadline saturates; the 64-bit counters can overflow TimerGroup's 54-bit product.
        self.timer.t.iter().chain([&self.lact]).filter_map(Timer::deadline).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_classic_timers_keep_full_width_and_share_alarm_semantics() {
        for (config, load, trigger, latch, count, alarm) in [
            (0, 0x18, 0x20, 0xc, 4, 0x10),
            (0x24, 0x3c, 0x44, 0x30, 0x28, 0x34),
            (0x70, 0x8c, 0x94, 0x80, 0x78, 0x84),
        ] {
            let mut t = ClassicTimer::new(0);
            t.write(load, u32::MAX - 1);
            t.write(load + 4, 0x80000000);
            t.write(trigger, 1);
            t.write(alarm, 1);
            t.write(alarm + 4, 0x80000001);
            t.write(config, (1 << 31) | (1 << 30) | (1 << 10) | (2 << 13));
            assert_eq!(t.next_deadline(), Some(6));
            t.tick(6);
            t.write(latch, 1);
            assert_eq!(t.read(count), 1);
            assert_eq!(t.read(count + 4), 0x80000001);
            assert_ne!(t.read(0x9c), 0);
            assert_eq!(t.next_deadline(), None);
        }
    }

    const RUN: u32 = (1 << 31) | (1 << 30) | (2 << 13);

    /// Arms alarm 1 on each (config, alarm) timer with `int_en` (LEVEL bit 11 / EDGE bit 12),
    /// INT_ENA left 0, and runs until every alarm has fired.
    fn fire(armed: &[(u32, u32)], int_en: u32) -> ClassicTimer {
        let mut t = ClassicTimer::new(0);
        for &(config, alarm) in armed {
            t.write(alarm, 1);
            t.write(config, RUN | (1 << 10) | int_en);
        }
        for _ in 0..4 {
            if let Some(d) = t.next_deadline() { t.tick(d.max(1)); }
        }
        assert_eq!(t.next_deadline(), None, "every armed alarm fired");
        t
    }

    /// esp-hal 1.x arms a classic alarm with LEVEL_INT_EN only and never writes INT_ENA: the
    /// line follows INT_RAW and the timer's own EDGE/LEVEL_INT_EN bits, whatever INT_ENA holds.
    #[test]
    fn alarm_lines_follow_raw_status_and_int_en_bits_not_int_ena() {
        for (config, alarm, raw, level, edge) in [(0x00, 0x10, 1u32, 1u64, 1u64 << 4), (0x24, 0x34, 2, 2, 1 << 5), (0x70, 0x84, 8, 8, 1 << 7)] {
            let mut t = fire(&[(config, alarm)], 1 << 11);
            assert_eq!(t.read(0x9c), raw, "INT_RAW at {config:#x}");
            for ena in [0, raw, 0xf & !raw, 0xf] {
                t.write(0x98, ena);
                t.write(config, RUN | (1 << 11));
                assert_eq!(t.irq_sources(), level, "level line, INT_ENA {ena:#x} at {config:#x}");
                t.write(config, RUN | (1 << 12));
                assert_eq!(t.irq_sources(), edge, "edge line, INT_ENA {ena:#x} at {config:#x}");
                t.write(config, RUN | (1 << 11) | (1 << 12));
                assert_eq!(t.irq_sources(), level | edge, "both lines, INT_ENA {ena:#x} at {config:#x}");
                t.write(config, RUN);
                assert_eq!(t.irq_sources(), 0, "no line without EDGE/LEVEL_INT_EN, INT_ENA {ena:#x} at {config:#x}");
            }
            t.write(config, RUN | (1 << 11) | (1 << 12));
            t.write(0xa4, raw);
            assert_eq!((t.read(0x9c), t.irq_sources()), (0, 0), "INT_CLR with INT_ENA set at {config:#x}");
        }
    }

    /// INT_ST is INT_RAW masked by INT_ENA; INT_ENA reads back as written. Raw T0, T1 and LACT
    /// (0b1011) against every enable mask, then INT_ENA still set with the raw bits cleared.
    #[test]
    fn int_st_is_raw_masked_by_int_ena() {
        let mut t = fire(&[(0x00, 0x10), (0x24, 0x34), (0x70, 0x84)], 1 << 11);
        assert_eq!(t.read(0x9c), 0b1011);
        for ena in 0..16 {
            t.write(0x98, ena);
            assert_eq!((t.read(0x98), t.read(0xa0)), (ena, 0b1011 & ena), "INT_ENA {ena:#x}");
            assert_eq!(t.irq_sources(), 0b1011, "lines unchanged by INT_ENA {ena:#x}");
        }
        t.write(0xa4, 0b0001);
        assert_eq!((t.read(0x9c), t.read(0xa0)), (0b1010, 0b1010), "INT_CLR T0");
        t.write(0xa4, 0b1010);
        assert_eq!((t.read(0x98), t.read(0x9c), t.read(0xa0), t.irq_sources()), (0xf, 0, 0, 0), "INT_ENA set, raw clear");
    }

    #[test]
    fn far_alarms_saturate_instead_of_wrapping() {
        // Divider field 0 means 65536; 2^64 - 1 steps times that overflows u64.
        let mut t = ClassicTimer::new(0);
        for base in [0x00, 0x24, 0x74] {
            let config = if base == 0x74 { 0x70 } else { base };
            t.write(base + 0x10, u32::MAX);
            t.write(base + 0x14, u32::MAX);
            t.write(config, (1 << 31) | (1 << 30) | (1 << 10));
            assert_eq!(t.next_deadline(), Some(u64::MAX), "timer at {base:#x}");
            t.write(config, 0);
        }
    }
}
