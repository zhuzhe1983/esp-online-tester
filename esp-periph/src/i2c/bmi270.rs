//! BMI270 polled acceleration/gyro register subset; host samples, no FIFO or feature engine.
// Bosch BST-BMI270-DS000-08, sections 4.4, 4.6, 5.2 and 8.2; SensorAPI
// 41129fcfe39c583ee5462d79195741945d51c1fe/bmi2_defs.h. See docs/platform/stopwatch-imu.md.
use super::I2cDevice;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug)]
pub struct Bmi270Sample { pub accel_g: [f64; 3], pub gyro_dps: [f64; 3], pub version: u64 }
impl Default for Bmi270Sample {
    fn default() -> Self { Self { accel_g: [0.0, 1.0, 0.0], gyro_dps: [0.0; 3], version: 1 } }
}

pub struct Bmi270 {
    regs: [u8; 128], ptr: u8, first: bool, init_at: usize,
    config: Box<[u8; 8192]>, written: Box<[bool; 8192]>,
    sample: Arc<Mutex<Bmi270Sample>>, version: u64,
}

impl Bmi270 {
    pub const ADDRESS: u8 = 0x68;
    pub fn new(sample: Arc<Mutex<Bmi270Sample>>) -> Self {
        let mut device = Self { regs: [0; 128], ptr: 0, first: true, init_at: 0,
            config: Box::new([0; 8192]), written: Box::new([false; 8192]), sample, version: 0 };
        device.reset(); device
    }
    fn reset(&mut self) {
        self.regs.fill(0); self.written.fill(false); self.init_at = 0; self.version = 0;
        for (reg, value) in [(0x00, 0x24), (0x03, 0x10), (0x40, 0xa8), (0x41, 0x01),
            (0x42, 0xa9), (0x7c, 0x03)] { self.regs[reg] = value; }
    }
    fn latch(&mut self) {
        let sample = *self.sample.lock().expect("BMI270 sample mutex poisoned");
        let enabled = if self.regs[0x21] & 0x0f == 1 { self.regs[0x7d] } else { 0 };
        let bits = (if enabled & 4 != 0 { 0x80 } else { 0 }) | (if enabled & 2 != 0 { 0x40 } else { 0 });
        if self.version != sample.version { self.regs[0x03] |= bits; self.regs[0x1d] |= bits; self.version = sample.version; }
        self.regs[0x03] = 0x10 | (self.regs[0x03] & bits); self.regs[0x1d] &= bits;
        let acc_scale = 32768.0 / (2u32 << (self.regs[0x41] & 3)) as f64;
        let gyro_scale = 32768.0 / (2000u32 >> (self.regs[0x43] & 7).min(4)) as f64;
        for (start, values, scale, power) in [(0x0c, sample.accel_g, acc_scale, 4), (0x12, sample.gyro_dps, gyro_scale, 2)] {
            for (axis, value) in values.into_iter().enumerate() {
                let raw = if enabled & power != 0 && value.is_finite() { (value * scale).round().clamp(-32768.0, 32767.0) as i16 } else { 0 };
                self.regs[start + axis * 2..start + axis * 2 + 2].copy_from_slice(&raw.to_le_bytes());
            }
        }
    }
}

impl I2cDevice for Bmi270 {
    fn start(&mut self, read: bool) -> bool {
        if read { self.latch(); } else { self.first = true; } true
    }
    fn write(&mut self, value: u8) -> bool {
        if self.first {
            self.ptr = value & 0x7f; self.first = false;
            self.init_at = (((self.regs[0x5c] as usize) << 4) | (self.regs[0x5b] as usize & 15)) * 2;
            return true;
        }
        let reg = self.ptr as usize;
        if reg == 0x5e {
            if self.regs[0x59] == 0 && self.init_at < self.config.len() {
                self.config[self.init_at] = value; self.written[self.init_at] = true;
            }
            self.init_at = self.init_at.saturating_add(1); return true;
        }
        if reg == 0x7e && value == 0xb6 { self.reset(); }
        else if reg == 0x59 {
            self.regs[reg] = value & 1;
            // Accept the complete Bosch config transfer; its embedded feature code
            // is not executed. An incomplete transfer must not report init success.
            self.regs[0x21] = if value & 1 != 0 { if self.written.iter().all(|v| *v) { 1 } else { 2 } } else { 0 };
            self.version = 0;
        } else if (0x2f..=0x5c).contains(&reg) || (0x68..=0x7d).contains(&reg) {
            self.regs[reg] = value;
            if reg == 0x7d { self.version = 0; }
        }
        self.ptr = self.ptr.wrapping_add(1) & 0x7f; true
    }
    fn read(&mut self) -> u8 {
        let reg = self.ptr as usize;
        if reg == 0x5e { let value = self.config.get(self.init_at).copied().unwrap_or(0); self.init_at = self.init_at.saturating_add(1); return value; }
        let value = self.regs[reg];
        // A burst is latched at its read address phase. Reading a full vector clears
        // its data-ready status; INT_STATUS is clear-on-read (DS000-08 sections 4.6/5.2).
        if reg == 0x11 { self.regs[0x03] &= !0x80; }
        if reg == 0x17 { self.regs[0x03] &= !0x40; }
        if reg == 0x1d { self.regs[0x1d] = 0; }
        self.ptr = self.ptr.wrapping_add(1) & 0x7f; value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(device: &mut Bmi270, reg: u8, bytes: &[u8]) { device.start(false); device.write(reg); for value in bytes { device.write(*value); } }
    fn read(device: &mut Bmi270, reg: u8, len: usize) -> Vec<u8> { write(device, reg, &[]); device.start(true); (0..len).map(|_| device.read()).collect() }
    fn ready(device: &mut Bmi270) { write(device, 0x59, &[0]); write(device, 0x5e, &[0; 8192]); write(device, 0x59, &[1]); write(device, 0x7d, &[6]); }
    fn vector(bytes: &[u8]) -> Vec<i16> { bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect() }
    #[test]
    fn config_power_ranges_and_saturation() {
        let sample = Arc::new(Mutex::new(Bmi270Sample { accel_g: [1.0, -1.0, 100.0], gyro_dps: [1000.0, -500.0, f64::NAN], version: 1 }));
        let mut d = Bmi270::new(sample);
        assert_eq!(read(&mut d, 0, 1), [0x24]); assert_eq!(read(&mut d, 0x0c, 12), [0; 12]);
        write(&mut d, 0x59, &[1]); assert_eq!(read(&mut d, 0x21, 1), [2]);
        ready(&mut d); assert_eq!(read(&mut d, 0x21, 1), [1]);
        assert_eq!(vector(&read(&mut d, 0x0c, 12)), [8192, -8192, 32767, 16384, -8192, 0]);
        write(&mut d, 0x41, &[3]); write(&mut d, 0x43, &[2]);
        assert_eq!(vector(&read(&mut d, 0x0c, 12)), [2048, -2048, 32767, 32767, -32768, 0]);
        write(&mut d, 0x7d, &[0]); assert_eq!(read(&mut d, 0x0c, 12), [0; 12]);
        write(&mut d, 0x7e, &[0xb6]); assert_eq!(read(&mut d, 0x21, 1), [0]);
    }
    #[test]
    fn read_snapshot_and_data_ready_follow_host_sample() {
        let sample = Arc::new(Mutex::new(Bmi270Sample::default())); let mut d = Bmi270::new(sample.clone()); ready(&mut d);
        assert_eq!(read(&mut d, 0x1c, 2), [0, 0xc0]); assert_eq!(read(&mut d, 0x1c, 2), [0, 0]);
        write(&mut d, 0x0c, &[]); d.start(true); let first = d.read();
        *sample.lock().unwrap() = Bmi270Sample { accel_g: [-1.0, 0.0, 0.0], gyro_dps: [0.0; 3], version: 2 };
        let mut bytes = vec![first]; bytes.extend((0..5).map(|_| d.read())); assert_eq!(vector(&bytes), [0, 8192, 0]);
        assert_eq!(read(&mut d, 0x1d, 1), [0xc0]); assert_eq!(vector(&read(&mut d, 0x0c, 6)), [-8192, 0, 0]);
    }
}
