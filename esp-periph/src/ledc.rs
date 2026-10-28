//! Low-speed LEDC channels shared by S3, C3 and C6, and the classic ESP32's two speed groups.
//! Register layout follows the ESP-IDF 5.5 `soc/ledc_reg.h` and `hal/ledc_ll.h` for each target.
//! Static duty updates are modeled; hardware fade sequences are not yet modeled.
use crate::{Device, Gpio, RegRam, WriteEffect, APB_HZ};
use emu_core::ClockDomain;

#[derive(Clone, Copy)]
/// `P4`: the C6's register fields (and its external clock) with the S3's eight channels; a
/// channel's fade step count is in the gamma RAM at 0x400 (IDF v5.5.4 esp32p4 hal/ledc_ll.h,
/// register/hw_ver3/soc/ledc_reg.h).
pub enum LedcLayout { S3, C3, C6, P4 }

/// `CLASSIC` marks a classic ESP32 speed group: S3's eight channels, C6-width timer fields and a
/// configuration write that drops a pending duty update. S3/C3/C6 use the default.
pub struct Ledc<const CLASSIC: bool = false> {
    layout: LedcLayout,
    regs: RegRam,
    duty: [u32; 8],
    pending: [bool; 8],
    unsupported_fade: [bool; 8],
    // Phase units cancel the divider's factor of 256, so even C6's maximum period fits u64.
    phase: [u64; 4],
    active: u8,
    timer_params: [u32; 4],
    raw: u32,
    ena: u32,
    /// C6 selects and gates its source clock in PCR, outside the LEDC block.
    pub external_clock_hz: u64,
    pub clock_enabled: bool,
}

impl Ledc {
    pub fn new(layout: LedcLayout) -> Self { Self::with_layout(layout) }
    /// The physical high-time fraction, including matrix inversion, normalized to 65535.
    /// No pulse edges are synthesized; callers observe channel configuration separately.
    pub fn output(&self, gpio: &Gpio, pin: u8) -> Option<(f64, u32)> {
        let matrix = *gpio.func_out_sel.get(pin as usize)?;
        if gpio.enable & (1u64 << pin) == 0 { return None; }
        let base = match self.layout { LedcLayout::S3 => 73, LedcLayout::C3 => 45, LedcLayout::C6 => 0, LedcLayout::P4 => 126 };
        let bits = if matches!(self.layout, LedcLayout::S3 | LedcLayout::P4) { 9 } else { 8 };
        let channel = (matrix & ((1 << bits) - 1)).checked_sub(base)? as usize;
        let timer = self.channel_timer_settings(channel)?;
        let hz = self.source_hz();
        if hz == 0 { return None; }
        let (hz, mut duty) = self.channel_wave(channel, timer, hz);
        if matrix & (1 << bits) != 0 { duty = 65535 - duty; }
        Some((hz, duty))
    }
}

/// Classic speed groups: their source clock and matrix signals are chosen by the classic adapter.
impl Ledc<true> {
    pub fn new_classic() -> Self { Self::with_layout(LedcLayout::S3) }
    /// Frequency and normalized duty of one channel clocked at `hz`.
    pub fn channel_output(&self, channel: usize, hz: u64) -> Option<(f64, u32)> {
        let timer = self.channel_timer_settings(channel)?;
        if hz == 0 { return None; }
        Some(self.channel_wave(channel, timer, hz))
    }
    pub fn channel_timer(&self, channel: usize) -> usize { (self.regs.read(channel as u32 * 0x14) & 3) as usize }
    pub fn timer_conf(&self, timer: usize) -> u32 { self.timer_params[timer] }
    pub fn signal_level(&self, channel: usize) -> (bool, bool) {
        let conf = self.regs.read(channel as u32 * 0x14);
        let enabled = conf & 4 != 0 && !self.unsupported_fade[channel];
        (if enabled { self.duty[channel] != 0 } else { conf & 8 != 0 }, enabled)
    }
    pub fn next_wrap(&self, source: impl Fn(usize, u32) -> u64) -> Option<u64> {
        (0..4).filter_map(|n| {
            let pending = (0..self.channels()).any(|ch| self.pending[ch] && self.ena & (1 << (4 + ch)) != 0 && self.regs.read(ch as u32 * 0x14) & 3 == n as u32);
            if self.ena & (1 << n) == 0 && !pending { return None; }
            let (period, div) = self.timer(n)?;
            let hz = source(n, self.timer_params[n]);
            (hz != 0).then(|| ((APB_HZ / 256 * div * period) - self.phase[n]).div_ceil(hz))
        }).min()
    }
    pub fn any_timer(&self) -> bool { self.active != 0 }
}

// Each variant gets its own non-generic code, so S3/C3/C6 compile exactly one copy of each
// method as before; a generic impl would be instantiated per calling crate.
macro_rules! ledc_impl { ($classic:literal) => {
impl Ledc<$classic> {
    fn with_layout(layout: LedcLayout) -> Self {
        Self { layout, regs: RegRam::new(), duty: [0; 8], pending: [false; 8],
            unsupported_fade: [false; 8], phase: [0; 4], active: 0, timer_params: [0; 4], raw: 0, ena: 0, external_clock_hz: 0, clock_enabled: true }
    }
    fn channels(&self) -> usize { if matches!(self.layout, LedcLayout::S3 | LedcLayout::P4) { 8 } else { 6 } }
    fn shift(&self) -> u32 { u32::from($classic || matches!(self.layout, LedcLayout::C6 | LedcLayout::P4)) }
    fn source_hz(&self) -> u64 {
        if !self.clock_enabled { return 0; }
        if matches!(self.layout, LedcLayout::C6 | LedcLayout::P4) { return self.external_clock_hz; }
        match self.regs.read(0xd0) & 3 { 1 => APB_HZ, 2 => 17_500_000, 3 => 40_000_000, _ => 0 }
    }
    fn timer_settings(&self, n: usize) -> Option<(u64, u64)> {
        let conf = self.timer_params[n];
        let shift = self.shift();
        let resolution = conf & (0xf | (shift << 4));
        let divider = (conf >> (4 + shift)) & 0x3ffff;
        if resolution == 0 || resolution > 14 + shift * 6 || divider < 256 { return None; }
        Some((1u64 << resolution, divider as u64))
    }
    fn timer(&self, n: usize) -> Option<(u64, u64)> {
        if self.regs.read(0xa0 + n as u32 * 8) & (3 << (22 + self.shift())) != 0 { return None; }
        self.timer_settings(n)
    }
    /// Timer period and divider of an enabled channel with a modelled duty.
    #[inline(always)]
    fn channel_timer_settings(&self, channel: usize) -> Option<(u64, u64)> {
        if channel >= self.channels() || self.unsupported_fade[channel] { return None; }
        let conf = self.regs.read(channel as u32 * 0x14);
        if conf & 4 == 0 { return None; }
        self.timer((conf & 3) as usize)
    }
    /// Frequency and duty normalized to 65535 for a channel clocked at `hz`.
    #[inline(always)]
    fn channel_wave(&self, channel: usize, (period, divider): (u64, u64), hz: u64) -> (f64, u32) {
        let full = period * 16;
        let high = (self.duty[channel] as u64).min(full);
        let duty = ((high * 65535 + full / 2) / full) as u32;
        (hz as f64 * 256.0 / divider as f64 / period as f64, duty)
    }
    /// Advance the timers, each clocked at `source(timer, timer_conf)` Hz.
    #[inline(always)]
    pub fn tick_with_source(&mut self, ticks: u64, source: impl Fn(usize, u32) -> u64) {
        for n in 0..4 {
            let Some((period, divider)) = self.timer(n) else { continue; };
            let hz = source(n, self.timer_params[n]);
            let modulus = (APB_HZ / 256) * divider * period;
            let (phase, wrapped) = match ticks.checked_mul(hz).and_then(|delta| self.phase[n].checked_add(delta)) {
                Some(total) => (if total < modulus { total } else { total % modulus }, total >= modulus),
                None => {
                    let total = self.phase[n] as u128 + ticks as u128 * hz as u128;
                    ((total % modulus as u128) as u64, total >= modulus as u128)
                }
            };
            self.phase[n] = phase;
            if !wrapped { continue; }
            self.raw |= 1 << n;
            for ch in 0..self.channels() {
                let off = ch as u32 * 0x14;
                if self.pending[ch] && self.regs.read(off) & 3 == n as u32 {
                    self.duty[ch] = self.regs.read(off + 8);
                    self.pending[ch] = false;
                    let conf = self.regs.read(off + 12);
                    self.regs.write(off + 12, conf & !(1 << 31));
                    self.raw |= 1 << (4 + ch);
                }
            }
        }
    }
}

impl Device for Ledc<$classic> {
    fn read(&mut self, off: u32) -> u32 {
        if off < self.channels() as u32 * 0x14 && off % 0x14 == 0x10 { return self.duty[(off / 0x14) as usize]; }
        if (0xa4..=0xbc).contains(&off) && off % 8 == 4 {
            let n = ((off - 0xa4) / 8) as usize;
            return self.timer_settings(n).map_or(0, |(_, div)| (self.phase[n] / (APB_HZ / 256 * div)) as u32);
        }
        match off { 0xc0 => self.raw, 0xc4 => self.raw & self.ena, 0xc8 => self.ena, 0xcc => 0, _ => self.regs.read(off) }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        match off {
            0xc0 | 0xc4 => {}, 0xc8 => self.ena = value, 0xcc => self.raw &= !value,
            _ if off < self.channels() as u32 * 0x14 => {
                let n = (off / 0x14) as usize;
                match off % 0x14 {
                    0x10 => {},
                    0x08 => self.regs.write(off, value & if self.shift() == 1 { 0x1ffffff } else { 0x7ffff }),
                    0x04 => self.regs.write(off, value & if self.shift() == 1 { 0xfffff } else { 0x3fff }),
                    0x00 => {
                        self.regs.write(off, value & !(1 << 4));
                        if $classic { self.pending[n] = false; }
                        if value & (1 << 4) != 0 && self.regs.read(off + 12) & (1 << 31) != 0 {
                            let fade = match self.layout {
                                LedcLayout::C6 => (self.regs.read(0x100 + n as u32 * 16) >> 11) & 0x3ff,
                                LedcLayout::P4 => (self.regs.read(0x400 + n as u32 * 0x40) >> 21) & 0x3ff,   // range 0's DUTY_NUM
                                _ => self.regs.read(off + 12) & 0x3ff,
                            };
                            self.unsupported_fade[n] = fade != 0;
                            self.pending[n] = fade == 0;
                        }
                    }
                    _ => self.regs.write(off, value),
                }
            }
            _ if (0xa0..=0xb8).contains(&off) && off.is_multiple_of(8) => {
                self.regs.write(off, value & !(1 << (25 + self.shift())));
                if value & (1 << (25 + self.shift())) != 0 { self.timer_params[((off - 0xa0) / 8) as usize] = value; }
                if value & (1 << (23 + self.shift())) != 0 { self.phase[((off - 0xa0) / 8) as usize] = 0; }
                let n = ((off - 0xa0) / 8) as usize;
                self.active = (self.active & !(1 << n)) | (u8::from(self.timer(n).is_some()) << n);
            }
            _ => self.regs.write(off, value),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { u64::from(self.raw & self.ena != 0) }
    fn clock(&self) -> Option<ClockDomain> { if self.active != 0 && self.source_hz() != 0 { Some(ClockDomain::Apb) } else { None } }
    fn tick(&mut self, ticks: u64) {
        let hz = self.source_hz();
        if hz == 0 { return; }
        self.tick_with_source(ticks, |_, _| hz);
    }
}
} }
ledc_impl!(false);
ledc_impl!(true);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_duty_latches_at_wrap_and_preserves_fractional_divider() {
        for (layout, signal) in [(LedcLayout::S3, 73), (LedcLayout::C3, 45), (LedcLayout::C6, 0)] {
            let mut ledc = Ledc::new(layout);
            ledc.external_clock_hz = 40_000_000;
            ledc.write(0xd0, 3);
            let mut gpio = Gpio::new();
            gpio.enable = (1 << 4) | (1 << 5);
            gpio.func_out_sel[4] = signal;
            let bits = if matches!(layout, LedcLayout::S3) { 9 } else { 8 };
            gpio.func_out_sel[4] |= 1 << (bits + 1);
            gpio.func_out_sel[5] = signal | (1 << bits);
            // XTAL / 1.5 / 256, 25% duty, both pins attached to the same channel.
            let shift = ledc.shift();
            ledc.write(0xa0, 8 | (384 << (4 + shift)));
            assert!(ledc.timer_settings(0).is_none());
            ledc.write(0xa0, 8 | (384 << (4 + shift)) | (1 << (25 + shift)));
            ledc.write(8, 64 << 4);
            ledc.write(12, 1 << 31);
            ledc.write(0, 4 | (1 << 4));
            assert_eq!(ledc.read(16), 0);
            ledc.tick(767);
            assert_eq!(ledc.read(16), 0);
            ledc.tick(1);
            assert_eq!(ledc.read(16), 64 << 4);
            let (hz, duty) = ledc.output(&gpio, 4).unwrap();
            assert!((hz - 40_000_000.0 / 384.0).abs() < 0.001);
            assert_eq!(duty, 16384);
            assert_eq!(ledc.output(&gpio, 5).unwrap().1, 49151);
            assert_eq!(ledc.read(12) >> 31, 0);
            ledc.write(0xc8, 1 << 4);
            assert_eq!(ledc.irq_sources(), 1);
            ledc.write(0xcc, 1 << 4);
            assert_eq!(ledc.irq_sources(), 0);
            ledc.tick(200);
            let counter = ledc.read(0xa4);
            assert!(counter > 0);
            ledc.write(0xa0, 8 | (384 << (4 + shift)) | (1 << (22 + shift)));
            ledc.tick(1000);
            assert_eq!(ledc.read(0xa4), counter);
            assert!(ledc.output(&gpio, 4).is_none());
        }
    }
    /// The P4's eighth channel at 0x8C, its matrix signal 126 + 7 (IDF v5.5.4 esp32p4
    /// gpio_sig_map.h:235), and a plain duty, whose gamma range 0 has DUTY_NUM 0 at 0x400 + 7 * 0x40.
    #[test]
    fn p4_channel_seven_and_its_gamma_ram() {
        let mut ledc = Ledc::new(LedcLayout::P4);
        ledc.external_clock_hz = 80_000_000;
        ledc.write(0xa0, 10 | (256 << 5) | (1 << 26));           // timer 0: 10-bit, divider 1, PARA_UP
        ledc.write(0x8c + 8, 512 << 4);                          // channel 7 duty: half of 1024 (4 fraction bits)
        ledc.write(0x8c + 12, 1 << 31);                          // DUTY_START
        ledc.write(0x8c, 4 | (1 << 4));                          // SIG_OUT_EN, PARA_UP, timer 0
        ledc.tick(1 << 20);
        let mut gpio = Gpio::with_pins(57);
        gpio.enable = 1 << 28;
        gpio.func_out_sel[28] = 126 + 7;
        assert_eq!(ledc.read(0x8c + 16), 512 << 4);
        assert_eq!(ledc.output(&gpio, 28).unwrap().1, 32768);
        ledc.write(0x400 + 7 * 0x40, 3 << 21);                   // a fade of three steps: not modelled
        ledc.write(0x8c + 12, 1 << 31);
        ledc.write(0x8c, 4 | (1 << 4));
        assert!(ledc.output(&gpio, 28).is_none());
    }

    #[test]
    fn c6_twenty_bit_timer_and_duty_do_not_truncate_or_overflow() {
        let mut ledc = Ledc::new(LedcLayout::C6);
        ledc.external_clock_hz = 80_000_000;
        ledc.write(0xa0, 20 | (0x3ffff << 5) | (1 << 26));
        ledc.write(8, 1 << 24);
        ledc.write(12, 1 << 31);
        ledc.write(0, 4 | (1 << 4));
        ledc.tick(u32::MAX as u64);
        let mut gpio = Gpio::new();
        gpio.enable = 1;
        gpio.func_out_sel[0] = 0;
        assert_eq!(ledc.read(16), 1 << 24);
        assert_eq!(ledc.output(&gpio, 0).unwrap().1, 65535);
        assert!(ledc.read(0xa4) < 1 << 20);
        ledc.clock_enabled = false;
        let count = ledc.read(0xa4);
        ledc.tick(u64::MAX);
        assert_eq!(ledc.read(0xa4), count);
        assert!(ledc.output(&gpio, 0).is_none());
    }

    #[test]
    fn idle_clock_and_large_batches_match_exact_phase() {
        let mut ledc = Ledc::new(LedcLayout::C6);
        assert_eq!(ledc.clock(), None);
        ledc.external_clock_hz = 17_500_000;
        ledc.write(0xa0, 20 | (0x3ffff << 5) | (1 << 26));
        assert_eq!(ledc.clock(), Some(ClockDomain::Apb));
        let modulus = (APB_HZ as u128 / 256) * 0x3ffff * (1 << 20);
        let mut phase = 0;
        for ticks in [1, 1000, u32::MAX as u64, u64::MAX, 1] {
            phase = (phase + ticks as u128 * 17_500_000) % modulus;
            ledc.tick(ticks);
            assert_eq!(ledc.phase[0] as u128, phase);
        }
        ledc.write(0xa0, 1 << 23);
        assert_eq!(ledc.clock(), None);
    }

    #[test]
    fn unsupported_fade_does_not_claim_successful_duty_or_completion() {
        let mut ledc = Ledc::new(LedcLayout::C3);
        ledc.write(0xd0, 1);
        ledc.write(0xa0, 8 | (256 << 4) | (1 << 25));
        ledc.write(8, 128 << 4);
        ledc.write(12, (1 << 31) | (4 << 20) | 1);
        ledc.write(0, 4 | (1 << 4));
        ledc.tick(80000);
        let mut gpio = Gpio::new();
        gpio.enable = 1;
        gpio.func_out_sel[0] = 45;
        assert!(ledc.output(&gpio, 0).is_none());
        assert_eq!(ledc.read(0xc0) & (1 << 4), 0);
        assert_ne!(ledc.read(12) & (1 << 31), 0);
    }

}
