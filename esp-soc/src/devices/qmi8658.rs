//! QST QMI8658A/C Rev A digital register subset. Source PDFs and limitations:
//! docs/platform/qmi8658-register-model.md. This does not change the S3 legacy
//! Qmi8658 device. Sensor noise, filter dynamics and physical board axes are not
//! calibrated by this model; unimplemented engines never complete successfully.
use super::imu_motion::{motion_script_screen, ImuMotion};
use esp_periph::i2c::I2cDevice;
use std::{cell::RefCell, rc::Rc, sync::{atomic::Ordering::Relaxed, Arc}};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qmi8658Variant { A, C }

#[derive(Clone, Copy)]
struct Rate { numerator: u64, denominator: u64 }
impl Rate {
    fn count(self, elapsed: u64, hz: u64) -> u64 {
        ((u128::from(elapsed) * u128::from(self.numerator))
            / (u128::from(hz) * u128::from(self.denominator))).min(u128::from(u64::MAX)) as u64
    }
    fn at(self, index: u64, hz: u64) -> u64 {
        (u128::from(index) * u128::from(hz) * u128::from(self.denominator))
            .div_ceil(u128::from(self.numerator)).min(u128::from(u64::MAX)) as u64
    }
    fn faster_than(self, other: Self) -> bool {
        u128::from(self.numerator) * u128::from(other.denominator)
            > u128::from(other.numerator) * u128::from(self.denominator)
    }
}

pub struct Qmi8658 {
    variant: Qmi8658Variant,
    motion: Arc<ImuMotion>,
    hz: u64,
    regs: [u8; 256],
    ptr: u8,
    first: bool,
    reset_until: Option<u64>,
    command_until: Option<u64>,
    ahb_gating_disabled: bool,
    epoch: u64,
    counts: [u64; 2],
    timestamp: u32,
    lock_until: Option<u64>,
    locked: bool,
    host_sample: Option<([f64; 3], [f64; 3])>,
}
impl Qmi8658 {
    pub fn new(motion: Arc<ImuMotion>, clock_hz: u64) -> Self { Self::with_variant(motion, clock_hz, Qmi8658Variant::A) }
    pub fn new_c(motion: Arc<ImuMotion>, clock_hz: u64) -> Self { Self::with_variant(motion, clock_hz, Qmi8658Variant::C) }
    pub fn with_variant(motion: Arc<ImuMotion>, hz: u64, variant: Qmi8658Variant) -> Self {
        assert_ne!(hz, 0, "QMI8658 requires a guest clock rate");
        let mut d = Self { variant, motion, hz, regs: [0; 256], ptr: 0, first: true,
            reset_until: None, command_until: None, ahb_gating_disabled: false,
            epoch: 0, counts: [0; 2], timestamp: 0, lock_until: None, locked: false,
            host_sample: None };
        d.reset(d.now()); d
    }
    /// Both Rev A PDFs §16.3.1: SA0 low is 6Bh; high or reset-time internal
    /// pull-up (unconnected SA0) is 6Ah. The board must establish the real strap.
    pub const fn i2c_address(sa0_high: bool) -> u8 { if sa0_high { 0x6a } else { 0x6b } }
    pub fn variant(&self) -> Qmi8658Variant { self.variant }
    /// Set a held external stimulus in sensor axes (g and degrees/second).
    /// Finish already-due conversions first, so an input cannot rewrite a
    /// sample whose timestamp precedes the input. Configured ODR, enables,
    /// range, byte order and SyncSample locking still control guest registers.
    pub fn set_host_sample(&mut self, accel_g: [f64; 3], gyro_dps: [f64; 3]) -> bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v| v.is_finite()) { return false; }
        self.refresh();
        self.host_sample = Some((accel_g, gyro_dps));
        true
    }
    pub fn host_sample_active(&self) -> bool { self.host_sample.is_some() }
    /// Explicitly selecting either deterministic mode clears the host hold.
    /// Invalid mode values do not disturb a previously accepted host sample.
    pub fn set_motion_mode(&mut self, mode: u32) -> bool {
        if mode > 1 { return false; }
        self.refresh();
        self.host_sample = None;
        self.motion.mode.store(mode, Relaxed);
        true
    }
    fn now(&self) -> u64 { self.motion.cycle.load(Relaxed) }
    fn microseconds(&self, us: u64) -> u64 {
        (u128::from(self.hz) * u128::from(us)).div_ceil(1_000_000).min(u128::from(u64::MAX)) as u64
    }
    fn reset(&mut self, now: u64) {
        self.regs = [0; 256];
        self.regs[0] = 5;
        // Detailed Table21 says7C, while overview Table19 says68: use the
        // explicit chip-information table and retain the source conflict.
        self.regs[1] = 0x7c;
        self.regs[2] = 0x20;
        self.reset_until = Some(now.saturating_add(self.microseconds(15_000)));
        self.command_until = None; self.ahb_gating_disabled = false;
        self.timestamp = 0; self.restart_samples(now);
    }
    fn restart_samples(&mut self, now: u64) {
        self.epoch = now; self.counts = [0; 2]; self.regs[0x2e] = 0;
        self.locked = false; self.lock_until = None;
    }
    fn rates(&self) -> [Option<Rate>; 2] {
        if self.regs[2] & 1 != 0 || self.reset_until.is_some() { return [None; 2]; }
        let enabled = self.regs[8];
        let gyro_clock = enabled & 2 != 0;
        let normal = |odr: u8| (odr <= 8).then(|| Rate { numerator: 71_744, denominator: 10 << odr });
        let a = if enabled & 1 == 0 { None } else if gyro_clock { normal(self.regs[3] & 15) } else {
            match self.regs[3] & 15 {
                n @ 3..=8 => Some(Rate { numerator: 1000, denominator: 1 << (n - 3) }),
                12 => Some(Rate { numerator: 128, denominator: 1 }),
                13 => Some(Rate { numerator: 21, denominator: 1 }),
                14 => Some(Rate { numerator: 11, denominator: 1 }),
                15 => Some(Rate { numerator: 3, denominator: 1 }),
                _ => None,
            }
        };
        let g = if gyro_clock && enabled & 0x10 == 0 { normal(self.regs[4] & 15) } else { None };
        [a, g]
    }
    fn samples_at(&self, cycle: u64) -> ([f64; 3], [f64; 3]) {
        if let Some(sample) = self.host_sample { sample }
        else if self.motion.mode.load(Relaxed) == 0 { ([0.0, 0.0, 1.0], [0.0; 3]) }
        else { motion_script_screen(cycle as f64 / self.hz as f64) }
    }
    fn sample(&mut self, sensor: usize, at: u64) {
        let (a, g) = self.samples_at(at);
        let (v, scale, base) = if sensor == 0 {
            (a, f64::from(16_384u32 >> ((self.regs[3] >> 4) & 3)), 0x35)
        } else { (g, f64::from(2048u32 >> ((self.regs[4] >> 4) & 7)), 0x3b) };
        for (axis, value) in v.into_iter().enumerate() {
            let n = value * scale;
            let n = (if n < 0.0 { n - 0.5 } else { n + 0.5 }).clamp(-32768.0, 32767.0) as i16;
            self.regs[base + axis * 2..base + axis * 2 + 2].copy_from_slice(&n.to_le_bytes());
        }
        self.regs[0x33..0x35].copy_from_slice(&(25i16 * 256).to_le_bytes());
        self.regs[0x2e] |= 1 << sensor;
    }
    fn refresh(&mut self) {
        let now = self.now();
        if let Some(end) = self.reset_until {
            if now < end { return; }
            self.reset_until = None; self.regs[0x4d] = 0x80;
        }
        if self.command_until.is_some_and(|end| now >= end) {
            match self.regs[0x0a] {
                0x10 => {
                    // Explicit virtual unknown firmware version and serial, not
                    // an invented real part identifier. See source §5.10.6.11.
                    self.regs[0x49..0x4c].fill(0); self.regs[0x51..0x57].fill(0);
                }
                0x12 => self.ahb_gating_disabled = self.regs[0x0b] == 1,
                _ => {}
            }
            self.regs[0x2d] |= 0x80; self.command_until = None;
        }
        let rates = self.rates();
        let cap = self.lock_until.unwrap_or(now).min(now);
        let at = |cycle: u64, rate: Option<Rate>| rate.map_or(0, |r| r.count(cycle.saturating_sub(self.epoch), self.hz));
        let counts_now = rates.map(|r| at(now, r));
        let counts_cap = rates.map(|r| at(cap, r));
        let fastest = match rates {
            [Some(a), Some(g)] => usize::from(!a.faster_than(g)),
            [Some(_), None] => 0,
            _ => 1,
        };
        let sampled_stamp = self.timestamp.wrapping_add(counts_cap[fastest].saturating_sub(self.counts[fastest]) as u32) & 0xffffff;
        self.timestamp = self.timestamp.wrapping_add(counts_now[fastest].saturating_sub(self.counts[fastest]) as u32) & 0xffffff;
        if !self.locked {
            let mut changed = false;
            for sensor in 0..2 {
                if counts_cap[sensor] > self.counts[sensor] {
                    let when = self.epoch.saturating_add(rates[sensor].unwrap().at(counts_cap[sensor], self.hz));
                    self.sample(sensor, when); changed = true;
                }
            }
            if changed { self.regs[0x30..0x33].copy_from_slice(&sampled_stamp.to_le_bytes()[..3]); }
            if self.lock_until.is_some_and(|end| now >= end) { self.locked = true; self.lock_until = None; }
        }
        self.counts = counts_now;
    }
    fn lock_delay(&self) -> u64 {
        // Rev A Tables40/41; use the documented maximum lock delay for this ODR.
        let us = if self.regs[8] & 2 != 0 {
            match self.regs[4] & 15 { 0 | 1 => 2, 2 => 4, 3 => 6, _ => 12 }
        } else {
            match self.regs[3] & 15 { 3 => 6, 4 => 12, 5 => 24, 6..=8 => 48, 12 => 40, 13 => 100, 14 => 200, 15 => 270, _ => 0 }
        };
        self.microseconds(us)
    }
    fn known(reg: u8) -> bool {
        matches!(reg, 0..=0x17 | 0x2d..=0x40 | 0x46 | 0x49..=0x56 | 0x59..=0x5c | 0x60)
    }
    fn store(&mut self, value: u8) -> bool {
        let now = self.now();
        let reg = self.ptr;
        if reg == 0x60 {
            if value != 0xb0 { return false; }
            self.reset(now); return true;
        }
        if self.reset_until.is_some() { return false; }
        let supported = match reg {
            2 => value & if self.variant == Qmi8658Variant::A { 0x02 } else { 0x1a } == 0,
            3 => value & 0x80 == 0 && (value >> 4) & 7 <= 3 && matches!(value & 15, 0..=8 | 12..=15),
            4 => value & 0x80 == 0 && value & 15 <= 8 && (self.variant == Qmi8658Variant::A || value & 0x70 != 0x70),
            5 | 7 => value == 0, // reserved registers: only their stated reset value
            6 => value & !0x77 == 0,
            8 => value & !0xb3 == 0,
            9 => value & !0xc0 == 0, // no activity-engine completion
            0x0b..=0x13 => true,
            0x14 => value & !0x0c == 0, // FIFO bypass only; size retained
            0x0a => {
                if value == 0 { self.command_until = None; self.regs[0x2d] &= !0x80; true }
                else if matches!(value, 0x10 | 0x12) && self.command_until.is_none() && self.regs[0x2d] & 0x80 == 0
                    && (value != 0x12 || self.regs[0x0b] <= 1)
                {
                    // Command work is digital and atomic at the next guest tick;
                    // the datasheet does not specify COPY_USID execution latency.
                    self.command_until = Some(now.saturating_add(1)); true
                } else { false }
            }
            _ => false,
        };
        if !supported { return false; }
        let old = self.regs[usize::from(reg)];
        self.regs[usize::from(reg)] = value;
        let restart = match reg {
            2 => (old ^ value) & 1 != 0,
            3 | 4 => old != value,
            8 => (old ^ value) & 0x13 != 0,
            _ => false,
        };
        if restart { self.restart_samples(now); }
        else if reg == 8 && (old ^ value) & 0x80 != 0 { self.locked = false; self.lock_until = None; }
        if reg == 8 && value & 3 != 0 { self.regs[0x4d] = 0; }
        true
    }
    fn read_value(&mut self, reg: u8) -> u8 {
        if reg == 0x2d {
            let command = self.regs[0x2d] & 0x80;
            if self.regs[8] & 0x80 != 0 {
                let avail = self.regs[0x2e] & 3 != 0;
                if avail && !self.locked && self.lock_until.is_none() {
                    self.lock_until = Some(self.now().saturating_add(self.lock_delay()));
                }
                return command | u8::from(avail) | (u8::from(self.locked) << 1);
            }
            // No fabricated DRDY pulse width or external INT pin edges. Only
            // CTRL9's defined status bit is exposed in non-SyncSample mode.
            return command;
        }
        let actual = if (0x33..=0x40).contains(&reg) && self.regs[2] & 0x20 != 0 { 0x33 + ((reg - 0x33) ^ 1) } else { reg };
        let result = if Self::known(actual) { self.regs[usize::from(actual)] } else { 0xff };
        if reg == 0x3a { self.regs[0x2e] &= !1; }
        if reg == 0x40 { self.regs[0x2e] &= !2; }
        let last = if self.regs[8] & 2 != 0 { 0x40 } else { 0x3a };
        if reg == last { self.locked = false; self.lock_until = None; }
        result
    }
}
impl I2cDevice for Qmi8658 {
    fn start(&mut self, read: bool) -> bool {
        self.refresh();
        if !read { self.first = true; }
        true
    }
    fn write(&mut self, value: u8) -> bool {
        self.refresh();
        if self.first { self.first = false; self.ptr = value; return Self::known(value); }
        if !self.store(value) { return false; }
        if self.regs[2] & 0x40 != 0 { self.ptr = self.ptr.wrapping_add(1); }
        true
    }
    fn read(&mut self) -> u8 {
        self.refresh();
        let value = self.read_value(self.ptr);
        if self.regs[2] & 0x40 != 0 { self.ptr = self.ptr.wrapping_add(1); }
        value
    }
}

/// An I2C view of the same sensor object that receives board-level host input.
/// The outer board adapter supplies physical pads and the real SA0 address.
pub struct Qmi8658Port(Rc<RefCell<Qmi8658>>);
impl Qmi8658Port {
    pub fn new(chip: Rc<RefCell<Qmi8658>>) -> Self { Self(chip) }
}
impl I2cDevice for Qmi8658Port {
    fn start(&mut self, read: bool) -> bool { self.0.borrow_mut().start(read) }
    fn start_address_with_clock(&mut self, configured: u8, address: u8, read: bool, hz: Option<u32>) -> bool {
        self.0.borrow_mut().start_address_with_clock(configured, address, read, hz)
    }
    fn write(&mut self, byte: u8) -> bool { self.0.borrow_mut().write(byte) }
    fn read(&mut self) -> u8 { self.0.borrow_mut().read() }
    fn stop(&mut self) { self.0.borrow_mut().stop(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d: &mut Qmi8658, reg: u8, value: u8) -> bool { d.start(false); assert!(d.write(reg)); let ok = d.write(value); d.stop(); ok }
    fn rd(d: &mut Qmi8658, reg: u8, n: usize) -> Vec<u8> { d.start(false); assert!(d.write(reg)); d.start(true); let v = (0..n).map(|_| d.read()).collect(); d.stop(); v }
    fn device(c: bool) -> (Qmi8658, Arc<ImuMotion>) {
        let clock = Arc::new(ImuMotion::default());
        let d = if c { Qmi8658::new_c(clock.clone(), 1_000_000) } else { Qmi8658::new(clock.clone(), 1_000_000) };
        (d, clock)
    }
    fn ready(d: &mut Qmi8658, c: &ImuMotion) { c.cycle.store(15_000, Relaxed); assert_eq!(rd(d, 0x4d, 1), [0x80]); }

    #[test]
    fn qmi_reset_identity_read_only_and_command_protocol_have_real_state() {
        for c in [false, true] {
            let (mut d, clock) = device(c);
            assert_eq!(rd(&mut d, 0, 2), [5, 5], "ADDR_AI reset is disabled");
            assert_eq!(rd(&mut d, 1, 1), [0x7c]);
            assert_eq!(rd(&mut d, 0x4d, 1), [0]);
            assert!(!wr(&mut d, 2, 0x40));
            ready(&mut d, &clock);
            assert!(!wr(&mut d, 0, 0));
            assert!(!wr(&mut d, 0x60, 0x0b), "Table27 B0 wins over prose7.4 typo");
            assert!(wr(&mut d, 0x0a, 0x10));
            assert_eq!(rd(&mut d, 0x2d, 1), [0]);
            clock.cycle.store(15_001, Relaxed);
            assert_eq!(rd(&mut d, 0x2d, 1), [0x80]);
            assert!(!wr(&mut d, 0x0a, 0x10), "host must acknowledge CmdDone");
            assert!(wr(&mut d, 0x0a, 0)); assert_eq!(rd(&mut d, 0x2d, 1), [0]);
            assert!(wr(&mut d, 2, 0x40)); assert_eq!(rd(&mut d, 0x51, 6), [0; 6]);
            for command in [0x04, 0x05, 0x08, 0xa2, 0xaa] { assert!(!wr(&mut d, 0x0a, command)); }
            assert_eq!(rd(&mut d, 0x2d, 1), [0], "unimplemented engines never claim done");
            assert!(wr(&mut d, 0x60, 0xb0));
            clock.cycle.store(30_000, Relaxed); assert_eq!(rd(&mut d, 0x4d, 1), [0]);
            clock.cycle.store(30_001, Relaxed); assert_eq!(rd(&mut d, 0x4d, 1), [0x80]);
            assert_eq!(rd(&mut d, 2, 1), [0x20]);
        }
    }

    #[test]
    fn qmi_exact_rate_enabled_ranges_endian_and_read_consumption() {
        let (mut d, c) = device(false); ready(&mut d, &c);
        assert!(wr(&mut d, 2, 0x40)); assert!(wr(&mut d, 3, 0x14)); // ±4g,500Hz
        assert!(wr(&mut d, 8, 1));
        c.cycle.store(16_999, Relaxed); assert_eq!(rd(&mut d, 0x2e, 1), [0]);
        c.cycle.store(17_000, Relaxed); assert_eq!(rd(&mut d, 0x2e, 1), [1]);
        assert_eq!(rd(&mut d, 0x35, 6), [0, 0, 0, 0, 0, 0x20]);
        assert_eq!(rd(&mut d, 0x2e, 1), [0]);
        assert_eq!(rd(&mut d, 0x33, 2), [0, 25]);
        assert!(wr(&mut d, 2, 0x60));
        c.cycle.store(19_000, Relaxed); assert_eq!(rd(&mut d, 0x39, 2), [0x20, 0]);
        assert!(wr(&mut d, 8, 0)); c.cycle.store(50_000, Relaxed);
        assert_eq!(rd(&mut d, 0x2e, 1), [0], "disabled sensor does not keep asserting ready");
        assert!(wr(&mut d, 2, 0x40)); assert!(wr(&mut d, 3, 0x23)); assert!(wr(&mut d, 4, 0x53)); assert!(wr(&mut d, 8, 3));
        c.cycle.store(51_000, Relaxed); assert_eq!(rd(&mut d, 0x2e, 1), [0], "6DOF setting3 is896.8Hz, not1000Hz");
        c.cycle.store(51_116, Relaxed); assert_eq!(rd(&mut d, 0x2e, 1), [3]);
        assert_eq!(rd(&mut d, 0x39, 2), [0, 0x10]);
        assert_eq!(rd(&mut d, 0x2e, 1), [2]);
        assert_eq!(rd(&mut d, 0x3b, 6), [0; 6]); assert_eq!(rd(&mut d, 0x2e, 1), [0]);
    }

    #[test]
    fn qmi_a_c_are_distinct_and_sa0_low_maps_to_6b() {
        assert_eq!(Qmi8658::i2c_address(false), 0x6b);
        assert_eq!(Qmi8658::i2c_address(true), 0x6a);
        for c in [false, true] {
            let (mut d, clock) = device(c); ready(&mut d, &clock);
            assert_eq!(wr(&mut d, 2, 0x58), !c, "C reserves A's INT enable bits");
            assert_eq!(wr(&mut d, 4, 0x73), !c, "C has no±2048dps range");
            assert!(wr(&mut d, 4, 0x63)); assert!(wr(&mut d, 3, 0x33));
            assert!(!wr(&mut d, 3, 0x43)); assert!(!wr(&mut d, 4, 0x69));
            assert!(!wr(&mut d, 3, 0x80)); assert!(!wr(&mut d, 9, 1));
            assert!(!wr(&mut d, 0x14, 1), "FIFO enable unsupported");
        }
    }

    #[test]
    fn qmi_syncsample_locks_after_source_delay_until_last_enabled_byte() {
        let (mut d, c) = device(false); ready(&mut d, &c);
        c.mode.store(1, Relaxed);
        assert!(wr(&mut d, 2, 0x40)); assert!(wr(&mut d, 3, 0x24)); // ±8g500Hz
        assert!(wr(&mut d, 0x0b, 1)); assert!(wr(&mut d, 0x0a, 0x12));
        c.cycle.store(15_001, Relaxed); assert_eq!(rd(&mut d, 0x2d, 1), [0x80]);
        assert!(d.ahb_gating_disabled); assert!(wr(&mut d, 0x0a, 0)); assert!(wr(&mut d, 8, 0x81));
        c.cycle.store(5_000_001, Relaxed);
        assert_eq!(rd(&mut d, 0x2d, 1), [1]);
        c.cycle.store(5_000_012, Relaxed); assert_eq!(rd(&mut d, 0x2d, 1), [1]);
        c.cycle.store(5_000_013, Relaxed); assert_eq!(rd(&mut d, 0x2d, 1), [3]);
        let held = rd(&mut d, 0x35, 5);
        c.cycle.store(7_000_013, Relaxed); assert_eq!(rd(&mut d, 0x35, 5), held);
        assert_eq!(rd(&mut d, 0x2d, 1), [3]); rd(&mut d, 0x3a, 1);
        assert_eq!(rd(&mut d, 0x2d, 1), [0]);
        c.cycle.store(7_002_013, Relaxed); assert_ne!(rd(&mut d, 0x35, 5), held);
    }

    fn words(bytes: &[u8]) -> Vec<i16> {
        bytes.as_chunks::<2>().0.iter().map(|p| i16::from_le_bytes(*p)).collect()
    }

    #[test]
    fn qmi_host_samples_do_not_rewrite_due_conversions_and_obey_ranges_and_endian() {
        for c in [false, true] {
            let (mut d, clock) = device(c); ready(&mut d, &clock);
            assert!(wr(&mut d, 2, 0x40)); assert!(wr(&mut d, 3, 0x14)); assert!(wr(&mut d, 8, 1));
            // The first conversion became due at 17 ms without an intervening
            // read. Host input at 18.5 ms must not backdate that conversion.
            clock.cycle.store(18_500, Relaxed);
            assert!(d.set_host_sample([0.5, -1.25, 3.0], [12.5, -3.25, 2000.0]));
            assert_eq!(words(&rd(&mut d, 0x35, 6)), [0, 0, 8192]);
            assert_eq!(rd(&mut d, 0x2e, 1), [0]);
            clock.cycle.store(18_999, Relaxed); assert_eq!(rd(&mut d, 0x2e, 1), [0]);
            clock.cycle.store(19_000, Relaxed);
            assert_eq!(words(&rd(&mut d, 0x35, 6)), [4096, -10240, 24576]);
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(!d.set_host_sample([bad, 0.0, 0.0], [0.0; 3]));
                assert!(!d.set_host_sample([0.0; 3], [0.0, bad, 0.0]));
            }
            assert!(d.host_sample_active()); assert!(!d.set_motion_mode(2)); assert!(d.host_sample_active());
            assert!(wr(&mut d, 3, 0x13)); assert!(wr(&mut d, 4, 0x53)); assert!(wr(&mut d, 8, 3));
            clock.cycle.store(20_116, Relaxed);
            assert_eq!(words(&rd(&mut d, 0x35, 12)), [4096, -10240, 24576, 800, -208, 32767]);
            assert_eq!(rd(&mut d, 0x33, 2), [0, 25]);
            assert!(wr(&mut d, 2, 0x60));
            assert_eq!(rd(&mut d, 0x35, 4), [0x10, 0, 0xd8, 0]);
        }
    }

    #[test]
    fn qmi_host_stimulus_survives_sensor_reset_but_explicit_script_selection_clears_it() {
        let (mut d, clock) = device(true); ready(&mut d, &clock);
        assert!(d.set_host_sample([-4.0, 4.0, 0.5], [-5000.0, 0.0, 0.0]));
        assert!(wr(&mut d, 0x60, 0xb0)); clock.cycle.store(30_000, Relaxed);
        assert_eq!(rd(&mut d, 0x4d, 1), [0x80]); assert!(d.host_sample_active());
        assert!(wr(&mut d, 2, 0x40)); assert!(wr(&mut d, 3, 3)); assert!(wr(&mut d, 8, 1));
        clock.cycle.store(31_000, Relaxed);
        assert_eq!(words(&rd(&mut d, 0x35, 6)), [-32768, 32767, 8192]);
        assert!(wr(&mut d, 8, 0));
        assert!(d.set_host_sample([1.0, 0.0, 0.0], [0.0; 3])); clock.cycle.store(40_000, Relaxed);
        assert_eq!(words(&rd(&mut d, 0x35, 6)), [-32768, 32767, 8192], "disabled ADC does not sample host input");
        assert!(d.set_motion_mode(0)); assert!(!d.host_sample_active()); assert!(wr(&mut d, 8, 1));
        clock.cycle.store(41_000, Relaxed); assert_eq!(words(&rd(&mut d, 0x35, 6)), [0, 0, 16384]);
        assert!(d.set_motion_mode(1)); clock.cycle.store(5_000_000, Relaxed);
        assert_ne!(words(&rd(&mut d, 0x35, 6)), [0, 0, 16384]);
    }

    #[test]
    fn qmi_host_updates_do_not_unlock_syncsample_data() {
        let (mut d, clock) = device(false); ready(&mut d, &clock);
        assert!(wr(&mut d, 2, 0x40)); assert!(wr(&mut d, 3, 0x14)); assert!(wr(&mut d, 8, 0x81));
        assert!(d.set_host_sample([0.25, -0.5, 0.75], [0.0; 3]));
        clock.cycle.store(17_000, Relaxed); assert_eq!(rd(&mut d, 0x2d, 1), [1]);
        clock.cycle.store(17_012, Relaxed); assert_eq!(rd(&mut d, 0x2d, 1), [3]);
        let held = rd(&mut d, 0x35, 5);
        clock.cycle.store(20_000, Relaxed);
        assert!(d.set_host_sample([-1.0, 0.5, 0.25], [0.0; 3]));
        assert_eq!(rd(&mut d, 0x35, 5), held); assert_eq!(rd(&mut d, 0x2d, 1), [3]);
        rd(&mut d, 0x3a, 1); clock.cycle.store(21_000, Relaxed);
        assert_eq!(words(&rd(&mut d, 0x35, 6)), [-8192, 4096, 2048]);
    }
}
