//! MCPWM register model for independently routed, up-counting PWM generators.
use crate::{Device, Gpio, RegRam, WriteEffect, APB_HZ};
use emu_core::ClockDomain;

pub struct Mcpwm {
    regs: RegRam,
    timers: [u32; 3],
    compare: [[u32; 2]; 3],
    pending: [[bool; 2]; 3],
    phase: [u64; 3],
    clock_remainder: [u64; 3],
    raw: u32,
    pub clock_enabled: bool,
    pub source_hz: u64,
    signal_base: u32,
    matrix_bits: u32,
}
impl Mcpwm {
    pub fn new(signal_base: u32, matrix_bits: u32) -> Self {
        let mut regs = RegRam::new();
        regs.write(0x10c, 0x55);
        for i in 0..3 {
            regs.write(4 + i * 16, 255 << 8);
            regs.write(0x4c + i * 0x38, 32);
            regs.write(0x58 + i * 0x38, 3 << 15);
        }
        Self { regs, timers: [255 << 8; 3], compare: [[0; 2]; 3], pending: [[false; 2]; 3],
            phase: [0; 3], clock_remainder: [0; 3], raw: 0, clock_enabled: false, source_hz: 160_000_000, signal_base, matrix_bits }
    }
    fn timer_for(&self, operator: usize) -> usize { ((self.regs.read(0x38) >> (operator * 2)) & 3) as usize }
    fn settings(&self, timer: usize) -> Option<(u64, u64)> {
        if !self.clock_enabled || timer >= 3 { return None; }
        let control = self.regs.read(8 + timer as u32 * 16);
        if control & 7 < 2 || control & 7 > 4 || (control >> 3) & 3 != 1 { return None; }
        let divider = ((self.regs.read(0) & 255) + 1) as u64 * ((self.timers[timer] & 255) + 1) as u64;
        Some((divider, ((self.timers[timer] >> 8) & 65535) as u64 + 1))
    }
    fn latch(&mut self, operator: usize, events: u32, force: bool) {
        let global = self.regs.read(0x10c);
        if global & 1 == 0 || global & (1 << (2 + operator * 2)) == 0 { return; }
        let base = 0x3c + operator as u32 * 0x38;
        for cmp in 0..2 {
            let method = (self.regs.read(base) >> (cmp * 4)) & 15;
            if force || method == 0 || (method & 8 == 0 && method & events != 0) {
                self.compare[operator][cmp] = self.regs.read(base + 4 + cmp as u32 * 4) & 65535;
                self.pending[operator][cmp] = false;
            }
        }
    }
    pub fn output(&self, gpio: &Gpio, pin: u8) -> Option<(f64, u32)> {
        let route = *gpio.func_out_sel.get(pin as usize)?;
        if gpio.enable & (1u64 << pin) == 0 { return None; }
        let channel = (route & ((1 << self.matrix_bits) - 1)).checked_sub(self.signal_base)? as usize;
        if channel >= 6 { return None; }
        let operator = channel / 2;
        let generator = channel % 2;
        let (divider, period) = self.settings(self.timer_for(operator))?;
        let base = 0x3c + operator as u32 * 0x38;
        // Carrier modulation, dead time and alternate source routing do not describe a servo pulse.
        if self.regs.read(base + 0x28) & 1 != 0 || self.regs.read(base + 0x1c) & 0x7f00 != 0 { return None; }
        if self.regs.read(base + 0x1c) & (1 << (15 + generator)) == 0 { return None; }
        let force = (self.regs.read(base + 0x10) >> (6 + generator * 2)) & 3;
        let action = self.regs.read(base + 0x14 + generator as u32 * 4);
        if action & ((3 << 2) | (15 << 8)) != 0 { return None; }
        let mut high = match force {
            1 => 0,
            2 => period,
            _ => {
                let empty = action & 3;
                let mut selected = None;
                for (cmp, &tick) in self.compare[operator].iter().enumerate() {
                    let compare_action = (action >> (4 + cmp * 2)) & 3;
                    if compare_action == 0 { continue; }
                    if selected.is_some() { return None; }
                    let tick = tick as u64;
                    if tick > period { return None; }
                    selected = Some(match (empty, compare_action) {
                        (2, 1) => tick,
                        (1, 2) => period - tick,
                        _ => return None,
                    });
                }
                selected?
            }
        };
        if route & (1 << self.matrix_bits) != 0 { high = period - high; }
        Some((self.source_hz as f64 / divider as f64 / period as f64, ((high * 65535 + period / 2) / period) as u32))
    }
}
impl Device for Mcpwm {
    fn read(&mut self, off: u32) -> u32 {
        if (0x10..=0x30).contains(&off) && off.is_multiple_of(16) {
            let timer = ((off - 0x10) / 16) as usize;
            return self.settings(timer).map_or(0, |(divider, _)| (self.phase[timer] / divider) as u32);
        }
        for operator in 0..3 {
            if off == 0x3c + operator as u32 * 0x38 {
                return self.regs.read(off) & !0x300
                    | ((self.pending[operator][0] as u32) << 8)
                    | ((self.pending[operator][1] as u32) << 9);
            }
        }
        match off {
            0x114 => self.raw,
            0x118 => self.raw & self.regs.read(0x110),
            0x11c => 0,
            _ => self.regs.read(off),
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        match off {
            0x114 | 0x118 => {}
            0x11c => self.raw &= !value,
            0x10c => {
                let changed = self.regs.read(off) ^ value;
                self.regs.write(off, value);
                if changed & 2 != 0 {
                    for timer in 0..3 {
                        self.timers[timer] = self.regs.read(4 + timer as u32 * 16);
                    }
                }
                for operator in 0..3 {
                    if changed & (2 | (1 << (3 + operator * 2))) != 0 { self.latch(operator, 0, true); }
                }
            }
            _ => {
                let old = self.regs.read(off);
                self.regs.write(off, value);
                for timer in 0..3 {
                    if off == 4 + timer as u32 * 16 && value & (3 << 24) == 0 { self.timers[timer] = value; }
                    if off == 8 + timer as u32 * 16 && value & 7 >= 2 && old & 7 < 2 { self.phase[timer] = 0; }
                }
                for operator in 0..3 {
                    let base = 0x3c + operator as u32 * 0x38;
                    if off == base + 4 || off == base + 8 {
                        self.pending[operator][((off - base - 4) / 4) as usize] = true;
                        self.latch(operator, 0, false);
                    }
                }
            }
        }
        WriteEffect::NONE
    }
    fn clock(&self) -> Option<ClockDomain> { if (0..3).any(|timer| self.settings(timer).is_some()) { Some(ClockDomain::Apb) } else { None } }
    fn irq_sources(&self) -> u64 { u64::from(self.raw & self.regs.read(0x110) != 0) }
    fn tick(&mut self, ticks: u64) {
        for timer in 0..3 {
            let Some((divider, period)) = self.settings(timer) else { continue; };
            let scaled = self.clock_remainder[timer] as u128 + ticks as u128 * self.source_hz as u128;
            self.clock_remainder[timer] = (scaled % APB_HZ as u128) as u64;
            let old = self.phase[timer] as u128;
            let total = old + scaled / APB_HZ as u128;
            let wrap = (divider * period) as u128;
            let full = total >= wrap;
            for operator in 0..3 {
                if self.timer_for(operator) != timer { continue; }
                for cmp in 0..2 {
                    let target = self.compare[operator][cmp] as u128 * divider as u128;
                    if (old < target && total >= target) || full { self.raw |= 1 << (15 + operator + cmp * 3); }
                }
                if full { self.latch(operator, 3, false); }
            }
            self.phase[timer] = (total % wrap) as u64;
            if full {
                self.raw |= (1 << (timer + 3)) | (1 << (timer + 6));
                let cfg = self.regs.read(4 + timer as u32 * 16);
                if cfg & (1 << 24) != 0 { self.timers[timer] = cfg; }
                let control = self.regs.read(8 + timer as u32 * 16);
                if control & 7 != 2 {
                    self.regs.write(8 + timer as u32 * 16, control & !7);
                    self.raw |= 1 << timer;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcpwm_latches_compare_at_timer_boundary_and_routes_generator() {
        let mut pwm = Mcpwm::new(160, 9);
        pwm.clock_enabled = true;
        pwm.write(0, 15);
        pwm.write(4, (19999 << 8) | 9);
        pwm.write(0x3c, 1);
        pwm.write(0x40, 1500);
        pwm.write(0x50, 2 | (1 << 4));
        pwm.write(8, 2 | (1 << 3));
        let mut gpio = Gpio::new();
        gpio.enable = 1 << 4;
        gpio.func_out_sel[4] = 160;
        assert_eq!(pwm.output(&gpio, 4), Some((50.0, 0)));
        assert_ne!(pwm.read(0x3c) & 256, 0);
        pwm.tick(1_600_000);
        assert_eq!(pwm.output(&gpio, 4), Some((50.0, 4915)));
        assert_eq!(pwm.read(0x3c) & 256, 0);
        pwm.write(0x40, 2000);
        pwm.tick(100_000);
        assert_eq!(pwm.output(&gpio, 4), Some((50.0, 4915)));
        assert_eq!(pwm.read(0x10), 1250);
        pwm.tick(1_500_000);
        assert_eq!(pwm.output(&gpio, 4), Some((50.0, 6554)));
        pwm.write(0x110, 1 << 3);
        assert_ne!(pwm.irq_sources(), 0);
        pwm.write(0x11c, 1 << 3);
        assert_eq!(pwm.irq_sources(), 0);
        gpio.func_out_sel[4] = 166;
        assert!(pwm.output(&gpio, 4).is_none());
        gpio.func_out_sel[4] = 160 | (1 << 9);
        assert_eq!(pwm.output(&gpio, 4), Some((50.0, 58982)));
        gpio.enable = 0;
        assert!(pwm.output(&gpio, 4).is_none());
        gpio.enable = 1 << 4;
        pwm.clock_enabled = false;
        assert!(pwm.output(&gpio, 4).is_none());
    }
    #[test]
    fn generators_keep_independent_compare_values_and_reject_unsupported_waveforms() {
        let mut pwm = Mcpwm::new(166, 9);
        pwm.clock_enabled = true;
        pwm.write(0, 15);
        pwm.write(4, (19999 << 8) | 9);
        pwm.write(8, 2 | (1 << 3));
        pwm.write(0x40, 1000);
        pwm.write(0x44, 2000);
        pwm.write(0x50, 2 | (1 << 4));
        pwm.write(0x54, 2 | (1 << 6));
        let mut gpio = Gpio::new();
        gpio.enable = (1 << 4) | (1 << 5);
        gpio.func_out_sel[4] = 166;
        gpio.func_out_sel[5] = 167;
        assert_eq!(pwm.output(&gpio, 4), Some((50.0, 3277)));
        assert_eq!(pwm.output(&gpio, 5), Some((50.0, 6554)));
        pwm.write(0x64, 1);
        assert!(pwm.output(&gpio, 4).is_none());
        pwm.write(0x64, 0);
        pwm.write(8, 2 | (2 << 3));
        assert!(pwm.output(&gpio, 4).is_none());
    }
    #[test]
    fn dead_time_and_extra_generator_actions_have_no_snapshot() {
        let mut pwm = Mcpwm::new(87, 8);
        pwm.clock_enabled = true;
        pwm.write(4, 99 << 8);
        pwm.write(8, 2 | (1 << 3));
        pwm.write(0x40, 25);
        pwm.write(0x44, 75);
        pwm.write(0x50, 2 | (1 << 4));
        pwm.write(0x54, 2 | (1 << 6));
        let mut gpio = Gpio::new();
        gpio.enable = 1;
        for generator in 0..2 {
            gpio.func_out_sel[0] = 87 + generator;
            let expected = pwm.output(&gpio, 0);
            assert!(expected.is_some());
            pwm.write(0x58, (3 << 15) & !(1 << (15 + generator)));
            assert!(pwm.output(&gpio, 0).is_none());
            pwm.write(0x58, 3 << 15);
            let off = 0x50 + generator * 4;
            let action = pwm.read(off);
            for shift in [2, 8, 10] {
                for value in 1..=3 {
                    pwm.write(off, action | (value << shift));
                    assert!(pwm.output(&gpio, 0).is_none());
                }
            }
            pwm.write(off, action);
            assert_eq!(pwm.output(&gpio, 0), expected);
        }
    }
    #[test]
    fn source_clock_scales_frequency_and_counter() {
        let mut pwm = Mcpwm::new(87, 8);
        pwm.clock_enabled = true;
        pwm.source_hz = 40_000_000;
        pwm.write(4, 99 << 8);
        pwm.write(8, 2 | (1 << 3));
        pwm.write(0x40, 50);
        pwm.write(0x50, 2 | (1 << 4));
        let mut gpio = Gpio::new();
        gpio.enable = 1;
        gpio.func_out_sel[0] = 87;
        assert_eq!(pwm.output(&gpio, 0), Some((400_000.0, 32768)));
        pwm.tick(1);
        assert_eq!(pwm.read(0x10), 0);
        pwm.tick(1);
        assert_eq!(pwm.read(0x10), 1);
    }
}
