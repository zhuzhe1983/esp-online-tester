//! VL53L3CX digital subset used by StampFly's ST VL53LX 1.2.8 driver.
//! Register/algorithm source: m5stack/M5StampFly@2b96752c72c465c63b260996046c71c7675f81b2,
//! lib/vl53l3c/{vl53lx_register_map.h,vl53lx_api_core.c,vl53lx_core.c}.
//! The host still processes raw 24-bin histograms. This is an ideal one-target model,
//! not an optical/flight simulation or an implementation of the sensor's private MCU.
use super::I2cDevice;
use std::sync::{Arc, Mutex};

const BOOT_US: u64 = 1200; // vl53lx_ll_device.h: VL53LX_FIRMWARE_BOOT_TIME_US.
const FAST_OSC: u16 = 0xbccc; // vl53lx_api_core.c: published fallback oscillator calibration.

pub struct Vl53l3cxState {
    regs: Box<[u8; 65536]>,
    clock_hz: u64,
    now: u64,
    enabled: bool,
    boot_at: Option<u64>,
    range_at: Option<u64>,
    nvm_power_at: Option<u64>,
    nvm_read_at: Option<u64>,
    ready: bool,
    running: bool,
    stream: u8,
    distance_mm: Option<f64>,
}

impl Vl53l3cxState {
    pub fn new(clock_hz: u64) -> Self {
        let mut state = Self { regs: Box::new([0; 65536]), clock_hz: clock_hz.max(1), now: 0, enabled: false,
            boot_at: None, range_at: None, nvm_power_at: None, nvm_read_at: None, ready: false, running: false, stream: 0, distance_mm: None };
        state.reset(); state
    }
    fn reset(&mut self) {
        self.regs.fill(0);
        self.regs[0] = 1; self.regs[1] = 0x29;
        self.regs[6..8].copy_from_slice(&FAST_OSC.to_be_bytes());
        self.regs[0x30] = 0x10; self.regs[0x85] = 1;
        self.regs[0xde..0xe0].copy_from_slice(&0x0100u16.to_be_bytes());
        self.regs[0x10f..0x112].copy_from_slice(&[0xea, 0xaa, 0x10]);
        self.regs[0x11e..0x13e].fill(0xff);
        self.regs[0x13e] = 0xc7; self.regs[0x13f] = 0xff;
        self.regs[0x790..0x794].fill(0xff);
        self.ready = false; self.running = false; self.range_at = None; self.stream = 0;
        self.nvm_power_at = None; self.nvm_read_at = None;
    }
    fn cycles(&self, us: u64) -> u64 { (u128::from(us) * u128::from(self.clock_hz)).div_ceil(1_000_000).min(u128::from(u64::MAX)) as u64 }
    pub fn set_enabled(&mut self, cycle: u64, enabled: bool) {
        self.advance_to(cycle);
        if self.enabled == enabled { return; }
        self.enabled = enabled; self.reset();
        self.boot_at = enabled.then(|| self.now.saturating_add(self.cycles(BOOT_US)));
    }
    pub fn set_distance_mm(&mut self, distance: Option<f64>) -> bool {
        if distance.is_some_and(|v| !v.is_finite() || !(0.0..=4000.0).contains(&v)) { return false; }
        self.distance_mm = distance; true
    }
    pub fn irq_high(&self) -> bool { if !self.enabled { true } else { self.ready == (self.regs[0x30] & 0x10 == 0) } }
    pub fn next_deadline(&self) -> Option<u64> { self.boot_at.into_iter().chain(self.range_at).min() }
    pub fn advance_to(&mut self, cycle: u64) {
        self.now = self.now.max(cycle);
        if self.boot_at.is_some_and(|at| at <= self.now) { self.boot_at = None; self.regs[0xe5] = 1; }
        if self.range_at.is_some_and(|at| at <= self.now) { self.range_at = None; self.measure(); }
    }
    fn word(&self, reg: usize) -> u16 { u16::from_be_bytes([self.regs[reg], self.regs[reg + 1]]) }
    fn range_us(&self) -> Option<u64> {
        // ST calc_macro_period_us/calc_timeout_us, then API's six windows + 1700us guard.
        let timeout = self.word(0x5e); let shift = u32::from(timeout >> 8);
        if shift > 20 || self.regs[0x60] > 15 { return None; }
        let clocks = (u64::from(timeout & 255) << shift) + 1;
        let pll = (1u64 << 30) / u64::from(FAST_OSC);
        let macro_period = (((2304 * pll) >> 6) * (2 * (u64::from(self.regs[0x60]) + 1))) >> 6;
        let us = ((clocks * macro_period + 0x800) >> 12) * 6 + 1700;
        (us <= 10_000_000).then_some(us)
    }
    fn patch_supported(&self) -> bool {
        // Recognize the published phasecal patch; its private MCU opcodes are not executed.
        self.regs[0x470] == 1 && self.regs[0x472..0x476] == [0, 7, 0, 7]
            && self.regs[0x476..0x47b] == [0x29, 0xc9, 0x0e, 0x40, 0x28]
            && matches!(self.regs[0x47b], 0 | 0x10 | 0x20 | 0x40)
            && self.regs[0x496..0x49c] == [3, 0x6d, 3, 0x6f, 7, 0x29]
    }
    fn schedule(&mut self) {
        if !self.ready && self.running && self.patch_supported() && self.range_at.is_none() {
            self.range_at = self.range_us().map(|us| self.now.saturating_add(self.cycles(us)));
        }
    }
    fn measure(&mut self) {
        self.ready = true;
        self.regs[0x88] = 4 | (((self.regs[0x82] ^ 2) & 2) << 4);
        self.regs[0x89] = 9; self.regs[0x8a] = 0; self.regs[0x8b] = self.stream;
        self.regs[0x8c..0x8e].copy_from_slice(&0x1000u16.to_be_bytes());
        // Phase calibration and nominal pulse are synthetic, never a captured device identity.
        let zero_phase = 4096u16;
        self.regs[0xd6..0xd8].copy_from_slice(&zero_phase.to_be_bytes());
        self.regs[0xd8] = self.regs[0x47];
        // Nominal calibration uses ST tuning default histogram gain=1987 (11 fractional bits).
        // A broad triangular pulse, fixed low ambient and SPAD count are idealized scene data.
        let phase_per_mm = (4.0 * 512.0 * 4194304.0 * 2048.0 / 1987.0) / (f64::from((1u32 << 30) / u32::from(FAST_OSC)) * 37463.0);
        let center = self.distance_mm.map(|mm| (f64::from(zero_phase) + mm * phase_per_mm) / 2048.0 - 0.5);
        let seq_start = if self.stream & 1 == 0 { 0x64 } else { 0x67 };
        let period = 2.0 * (f64::from(self.regs[if self.stream & 1 == 0 { 0x60 } else { 0x63 }]) + 1.0);
        for i in 0..24 {
            let packed = self.regs[seq_start + i / 8];
            let group = if i % 8 < 4 { packed & 15 } else { packed >> 4 };
            let bin = usize::from(group & 7) * 4 + i % 4;
            let signal = if group & 7 == 7 { 0.0 } else { center.map_or(0.0, |c| (1.0 - (((bin as f64 - c + period / 2.0).rem_euclid(period) - period / 2.0) / 2.0).abs()).max(0.0) * 10000.0) };
            let value = (100.0 + signal) as u32;
            let bytes = value.to_be_bytes();
            self.regs[0x8e + i * 3..0x91 + i * 3].copy_from_slice(&bytes[1..]);
        }
        self.regs[0xd9] = self.regs[0xd5] >> 2; self.regs[0xda] = self.regs[0xd5] & 3;
        if self.regs[0x87] & 0xf0 == 0x10 { self.running = false; }
    }
    fn write_reg(&mut self, reg: u16, value: u8) {
        let index = usize::from(reg);
        match reg {
            0 => {
                self.reset(); self.regs[0] = value & 1;
                self.boot_at = (value & 1 != 0).then(|| self.now.saturating_add(self.cycles(BOOT_US)));
            }
            1 => self.regs[1] = value & 0x7f,
            0x31 | 0x88..=0x13f | 0x790..=0x793 => {}
            0x86 => { self.regs[index] = value; if value & 1 != 0 { if self.ready { self.stream = if self.stream == 255 { 128 } else { self.stream + 1 }; } self.ready = false; self.schedule(); } }
            0x87 => {
                self.regs[index] = value;
                self.running = self.regs[0xe5] & 1 != 0 && self.regs[0x85] & 1 != 0 && matches!(value & 0xf0, 0x10 | 0x20) && value & 2 != 0;
                if self.running { self.schedule(); } else { self.range_at = None; self.ready = false; self.stream = 0; }
            }
            0x781 => { self.regs[index] = value; self.nvm_power_at = (value & 1 != 0).then_some(self.now); self.nvm_read_at = None; self.regs[0x790..0x794].fill(0xff); }
            0x783 => {
                self.regs[index] = value;
                let powered = self.nvm_power_at.is_some_and(|at| self.now.saturating_sub(at) >= self.cycles(50));
                let valid = powered && self.regs[0x780] == 1 && self.regs[0x683] == 5 && self.regs[0x85] & 1 == 0;
                if value & 1 == 0 { self.nvm_read_at = valid.then_some(self.now); }
                else if valid && self.nvm_read_at.take().is_some_and(|at| self.now.saturating_sub(at) >= self.cycles(5)) {
                    // Synthetic uncalibrated NVM: zero offsets/xtalk, published nominal oscillator,
                    // all SPADs enabled. No per-device serial, measured calibration or factory dump.
                    let base = usize::from(self.regs[0x794]) * 4;
                    for i in 0..4 { self.regs[0x790 + i] = match base + i { 0x14 | 0x1c => 0xbc, 0x15 | 0x1d => 0xcc, 0x48..=0x67 => 0xff, _ => 0 }; }
                }
            }
            0x85 => { self.regs[index] = value; if value & 1 == 0 { self.running = false; self.range_at = None; self.ready = false; } }
            _ => self.regs[index] = value,
        }
    }
    fn read_reg(&self, reg: u16) -> u8 { if reg == 0x31 { u8::from(self.irq_high()) } else { self.regs[usize::from(reg)] } }
}

pub struct Vl53l3cx { state: Arc<Mutex<Vl53l3cxState>>, pins: (u8, u8), ptr: u16, address_bytes: u8 }
impl Vl53l3cx {
    pub fn with_pins(state: Arc<Mutex<Vl53l3cxState>>, pins: (u8, u8)) -> Self { Self { state, pins, ptr: 0, address_bytes: 0 } }
}
impl I2cDevice for Vl53l3cx {
    fn address(&self, _configured: u8) -> u8 { self.state.lock().unwrap().regs[1] }
    fn pins(&self) -> Option<(u8, u8)> { Some(self.pins) }
    fn start(&mut self, read: bool) -> bool {
        if !read { self.address_bytes = 0; }
        self.state.lock().unwrap().enabled
    }
    fn write(&mut self, byte: u8) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state.enabled { return false; }
        match self.address_bytes { 0 => { self.ptr = u16::from(byte) << 8; self.address_bytes = 1; }, 1 => { self.ptr |= u16::from(byte); self.address_bytes = 2; },
            _ => { state.write_reg(self.ptr, byte); self.ptr = self.ptr.wrapping_add(1); } }
        true
    }
    fn read(&mut self) -> u8 {
        let state = self.state.lock().unwrap();
        if !state.enabled { return 0xff; }
        let value = state.read_reg(self.ptr); self.ptr = self.ptr.wrapping_add(1); value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Arc<Mutex<Vl53l3cxState>>, Vl53l3cx) {
        let s = Arc::new(Mutex::new(Vl53l3cxState::new(1_000_000)));
        let d = Vl53l3cx::with_pins(s.clone(), (3, 4)); (s, d)
    }
    fn write(d: &mut Vl53l3cx, reg: u16, data: &[u8]) {
        assert!(d.start(false)); for byte in reg.to_be_bytes().into_iter().chain(data.iter().copied()) { assert!(d.write(byte)); }
    }
    fn read(d: &mut Vl53l3cx, reg: u16, len: usize) -> Vec<u8> {
        write(d, reg, &[]); assert!(d.start(true)); (0..len).map(|_| d.read()).collect()
    }
    fn boot(s: &Arc<Mutex<Vl53l3cxState>>) { let mut s = s.lock().unwrap(); s.set_enabled(0, true); s.advance_to(1200); }
    fn configure(d: &mut Vl53l3cx) {
        // Relevant bytes from the unchanged ST medium/33000us configuration.
        write(d, 0x85, &[0]); write(d, 0x472, &[0, 7, 0, 7, 0x29, 0xc9, 0x0e, 0x40, 0x28, 0]);
        write(d, 0x496, &[3, 0x6d, 3, 0x6f, 7, 0x29]); write(d, 0x470, &[1]); write(d, 0x85, &[1]);
        write(d, 0x5e, &[0, 0x8d, 5, 0, 0x6a, 7]); write(d, 0x64, &[7, 0x11, 0x22, 0x10, 0x12, 0x32]);
        write(d, 0x82, &[0]); write(d, 0x86, &[1, 0x23]);
    }
    #[test]
    fn xshut_boot_address_change_and_two_devices_are_independent() {
        let (a, mut da) = fixture(); let (b, mut db) = fixture();
        assert!(!da.start_address(0x29, 0x29, false)); assert_eq!(da.read(), 255); assert!(!da.write(0));
        a.lock().unwrap().set_enabled(0, true);
        assert!(da.start_address(0x2a, 0x29, false)); assert_eq!(read(&mut da, 0xe5, 1), [0]);
        a.lock().unwrap().advance_to(1199); assert_eq!(read(&mut da, 0xe5, 1), [0]);
        a.lock().unwrap().advance_to(1200); assert_eq!(read(&mut da, 0xe5, 1), [1]);
        write(&mut da, 1, &[0x2a]); boot(&b);
        assert!(!da.start_address(0x29, 0x29, true)); assert!(da.start_address(0x29, 0x2a, true));
        assert!(db.start_address(0x2a, 0x29, true)); assert_eq!(read(&mut da, 0x10f, 3), [0xea, 0xaa, 0x10]);
        a.lock().unwrap().set_enabled(1300, false); assert!(!da.start(true));
        a.lock().unwrap().set_enabled(1400, true); assert_eq!(da.address(0), 0x29); assert_eq!(db.address(0), 0x29);
        assert_eq!(a.lock().unwrap().next_deadline(), Some(2600)); assert_eq!(da.pins(), Some((3, 4)));
    }
    #[test]
    fn irq_waits_for_guest_time_and_clear_rearms_a_latched_histogram() {
        let (s, mut d) = fixture(); boot(&s); configure(&mut d);
        let deadline = s.lock().unwrap().next_deadline().unwrap(); assert!(deadline > 1200);
        s.lock().unwrap().set_distance_mm(Some(500.0));
        s.lock().unwrap().advance_to(deadline - 1); assert!(s.lock().unwrap().irq_high());
        s.lock().unwrap().advance_to(deadline); assert!(!s.lock().unwrap().irq_high());
        let sample = read(&mut d, 0x88, 83); assert_eq!(sample[3], 0); assert_eq!(read(&mut d, 0x31, 1), [0]);
        s.lock().unwrap().set_distance_mm(Some(1000.0)); s.lock().unwrap().advance_to(deadline + 1_000_000);
        assert_eq!(read(&mut d, 0x88, 83), sample); assert_eq!(s.lock().unwrap().next_deadline(), None);
        write(&mut d, 0x82, &[2]); write(&mut d, 0x86, &[1, 0x23]); assert!(s.lock().unwrap().irq_high());
        let next = s.lock().unwrap().next_deadline().unwrap(); s.lock().unwrap().advance_to(next);
        let changed = read(&mut d, 0x88, 83); assert_eq!(changed[3], 1); assert_ne!(changed[6..78], sample[6..78]);
        write(&mut d, 0x30, &[0]); assert!(s.lock().unwrap().irq_high()); write(&mut d, 0x86, &[1]); assert!(!s.lock().unwrap().irq_high());
    }
    #[test]
    fn missing_patch_stop_reset_and_unsupported_modes_do_not_measure() {
        let (s, mut d) = fixture(); boot(&s);
        write(&mut d, 0x87, &[0x23]); assert_eq!(s.lock().unwrap().next_deadline(), None);
        configure(&mut d); assert!(s.lock().unwrap().next_deadline().is_some());
        write(&mut d, 0x87, &[0x80]); assert_eq!(s.lock().unwrap().next_deadline(), None);
        write(&mut d, 0x87, &[0x43]); assert_eq!(s.lock().unwrap().next_deadline(), None);
        configure(&mut d); write(&mut d, 0, &[0]); s.lock().unwrap().advance_to(1_000_000);
        assert_eq!(read(&mut d, 0xe5, 1), [0]); assert!(s.lock().unwrap().irq_high());
        write(&mut d, 0, &[1]); assert_eq!(s.lock().unwrap().next_deadline(), Some(1_001_200));
        s.lock().unwrap().advance_to(1_001_200); assert_eq!(read(&mut d, 0xe5, 1), [1]);
        assert_eq!(read(&mut d, 0x470, 1), [0]);
    }
    #[test]
    fn nvm_requires_power_settling_and_read_trigger_time() {
        let (s, mut d) = fixture(); boot(&s);
        assert_eq!(read(&mut d, 0x790, 4), [255; 4]);
        write(&mut d, 0x85, &[0]); write(&mut d, 0x781, &[1]); write(&mut d, 0x683, &[5]); write(&mut d, 0x780, &[1]);
        write(&mut d, 0x794, &[5]); write(&mut d, 0x783, &[0]); write(&mut d, 0x783, &[1]);
        assert_eq!(read(&mut d, 0x790, 4), [255; 4]);
        s.lock().unwrap().advance_to(1250); write(&mut d, 0x783, &[0]); s.lock().unwrap().advance_to(1254); write(&mut d, 0x783, &[1]);
        assert_eq!(read(&mut d, 0x790, 4), [255; 4]);
        write(&mut d, 0x783, &[0]); s.lock().unwrap().advance_to(1259); write(&mut d, 0x783, &[1]);
        assert_eq!(read(&mut d, 0x790, 4), [0xbc, 0xcc, 0, 0]);
        write(&mut d, 0x781, &[0]); assert_eq!(read(&mut d, 0x790, 4), [255; 4]);
    }
    #[test]
    fn malformed_timeouts_inputs_and_pointer_wrap_are_bounded() {
        let (s, mut d) = fixture(); boot(&s); configure(&mut d);
        for value in [f64::NAN, f64::INFINITY, -1.0, 4000.1] { assert!(!s.lock().unwrap().set_distance_mm(Some(value))); }
        assert!(s.lock().unwrap().set_distance_mm(None));
        write(&mut d, 0x87, &[0]); write(&mut d, 0x5e, &[255, 255]); write(&mut d, 0x87, &[0x23]);
        assert_eq!(s.lock().unwrap().next_deadline(), None);
        write(&mut d, 0xffff, &[0xab]); assert_eq!(read(&mut d, 0xffff, 2), [0xab, 1]);
        write(&mut d, 0x10f, &[0, 0, 0]); assert_eq!(read(&mut d, 0x10f, 3), [0xea, 0xaa, 0x10]);
        s.lock().unwrap().advance_to(u64::MAX); s.lock().unwrap().set_enabled(u64::MAX, false);
        s.lock().unwrap().set_enabled(u64::MAX, true); s.lock().unwrap().advance_to(u64::MAX);
        assert_eq!(read(&mut d, 0xe5, 1), [1]);
    }
    #[test]
    fn no_target_is_flat_ambient_and_stream_count_wraps_to_128() {
        let (s, mut d) = fixture(); boot(&s); configure(&mut d);
        for i in 0..257 {
            let at = s.lock().unwrap().next_deadline().unwrap(); s.lock().unwrap().advance_to(at);
            let expected = if i < 256 { i as u8 } else { 128 };
            assert_eq!(read(&mut d, 0x8b, 1), [expected]);
            assert_eq!(read(&mut d, 0x8e, 3), [0, 0, 100]); assert_eq!(read(&mut d, 0xd9, 2), [25, 0]);
            write(&mut d, 0x86, &[1]);
        }
    }
}
