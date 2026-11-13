//! A Bosch BMI270 6-axis IMU on I2C (0x68 with SDO low, 0x69 with SDO high), as the Bosch
//! BMI270 Sensor API (bmi2.c / bmi270.c, `bmi270_init`) and Espressif's `bmi270_sensor`
//! component drive it. Register numbers and reset values from the datasheet
//! (BST-BMI270-DS000, section 5 "Register description"); the handshake from section 4.4
//! ("Initialization"):
//!
//! - `bmi2_sec_init` reads CHIP_ID (0x00, 0x24), soft-resets with CMD 0x7E = 0xB6 (all
//!   registers back to their reset values, the feature engine unloaded), and reads CHIP_ID again.
//! - `write_config_file` turns advanced power save off (PWR_CONF 0x7C bit 0), writes
//!   INIT_CTRL 0x59 = 0, then uploads the 8 KiB configuration file in bursts: INIT_ADDR_0/1
//!   (0x5B/0x5C) take the word offset of the chunk (bits 3:0 and 11:4), the chunk is written to
//!   INIT_DATA 0x5E (which does not advance the register pointer). INIT_CTRL = 1 starts the
//!   feature engine; after up to 20 ms INTERNAL_STATUS 0x21 bits 3:0 read 0x1 "init OK"
//!   (0x2 "init_err" when the file was incomplete). The driver checks exactly that value.
//! - The feature engine's configuration pages (FEAT_PAGE 0x2F selects a page of 16 bytes at
//!   0x30-0x3F) are kept as plain memory, which is all `bmi270_init` and the sensor-config calls
//!   need (they read-modify-write feature words; no feature actually runs).
//! - ACC_CONF 0x40 / ACC_RANGE 0x41 / GYR_CONF 0x42 / GYR_RANGE 0x43 and PWR_CTRL 0x7D
//!   (aux bit 0, gyr bit 1, acc bit 2, temp bit 3) are kept. Data registers 0x0C-0x11 (accel x,
//!   y, z) and 0x12-0x17 (gyro x, y, z) are little-endian two's complement in the configured
//!   range, zero for a disabled sensor; STATUS 0x03 has drdy_acc (bit 7), drdy_gyr (bit 6) for
//!   an enabled sensor and cmd_rdy (bit 4). SENSORTIME 0x18-0x1A counts 39.0625 us; TEMPERATURE
//!   0x22/0x23 is 1/512 K per LSB around 23 C (we report 25 C).
//!
//! The device is at rest, face up: accel (0, 0, +1 g), gyro 0. With the shared scripted motion
//! (`ImuMotion` mode 1) the handling script is played in the board's screen frame instead.
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use esp_periph::i2c::I2cDevice;
use super::imu_motion::{motion_script_screen, ImuMotion};

/// SDO high, as on the ESP-Mosaico.
pub const BMI270_ADDRESS: u8 = 0x69;
pub const BMI270_CHIP_ID: u8 = 0x24;
/// Size of the configuration file the Bosch API uploads (`bmi270_config_file`).
pub const BMI270_CONFIG_SIZE: usize = 8192;

const CHIP_ID: usize = 0x00;
const STATUS: usize = 0x03;
const DATA_ACC: usize = 0x0c;
const SENSORTIME: usize = 0x18;
const INTERNAL_STATUS: usize = 0x21;
const TEMPERATURE: usize = 0x22;
const FEAT_PAGE: u8 = 0x2f;
const FEATURES: u8 = 0x30;
const ACC_CONF: usize = 0x40;
const ACC_RANGE: usize = 0x41;
const GYR_CONF: usize = 0x42;
const GYR_RANGE: usize = 0x43;
const INIT_CTRL: u8 = 0x59;
const INIT_ADDR_0: usize = 0x5b;
const INIT_ADDR_1: usize = 0x5c;
const INIT_DATA: u8 = 0x5e;
const PWR_CONF: usize = 0x7c;
const PWR_CTRL: usize = 0x7d;
const CMD: u8 = 0x7e;

pub struct Bmi270 {
    regs: [u8; 256],
    /// Feature engine memory: 8 pages of 16 bytes behind 0x30-0x3F.
    pages: [[u8; 16]; 8],
    /// The configuration file as uploaded, by byte offset.
    config: Vec<u8>,
    config_bytes: usize,
    ptr: u8,
    first: bool,
    motion: Arc<ImuMotion>,
    /// Rate of `motion.cycle` (ticks per second of guest time).
    clock_hz: u64,
    pub resets: u32,
}

impl Bmi270 {
    pub fn new(motion: Arc<ImuMotion>, clock_hz: u64) -> Self {
        let mut d = Bmi270 { regs: [0; 256], pages: [[0; 16]; 8], config: vec![0; BMI270_CONFIG_SIZE], config_bytes: 0,
                             ptr: 0, first: true, motion, clock_hz, resets: 0 };
        d.reset();
        d
    }
    /// Reset values, datasheet section 5.2.
    fn reset(&mut self) {
        self.regs = [0; 256];
        self.regs[CHIP_ID] = BMI270_CHIP_ID;
        self.regs[STATUS] = 0x10;           // cmd_rdy
        self.regs[ACC_CONF] = 0xa8;         // acc_odr 100 Hz, acc_bwp normal, perf mode
        self.regs[ACC_RANGE] = 0x02;        // +-8 g
        self.regs[GYR_CONF] = 0xa9;         // gyr_odr 200 Hz
        self.regs[0x4c] = 0x83;             // AUX_IF_CONF
        self.regs[PWR_CONF] = 0x03;         // adv_power_save, fifo_self_wake_up
        self.pages = [[0; 16]; 8];
        self.config_bytes = 0;
    }
    pub fn initialized(&self) -> bool { self.regs[INTERNAL_STATUS] & 0x0f == 0x01 }
    fn seconds(&self) -> f64 { self.motion.cycle.load(Relaxed) as f64 / self.clock_hz as f64 }
    /// Sample the outputs, sensortime and status for a read.
    fn latch(&mut self) {
        let (accel, gyro) = if self.motion.mode.load(Relaxed) != 0 { motion_script_screen(self.seconds()) } else { ([0.0, 0.0, 1.0], [0.0; 3]) };
        let pwr = self.regs[PWR_CTRL];
        // ACC_RANGE bits 1:0: +-2/4/8/16 g -> 16384 >> range LSB/g; GYR_RANGE bits 2:0:
        // +-2000/1000/500/250/125 dps -> 16.4 << range LSB/dps (datasheet 5.2, ACC_RANGE/GYR_RANGE).
        let acc_lsb = f64::from(16384u32 >> (self.regs[ACC_RANGE] & 3));
        let gyr_lsb = 16.384 * f64::from(1u32 << (self.regs[GYR_RANGE] & 7).min(4));
        let q = |v: f64| { let r = if v < 0.0 { v - 0.5 } else { v + 0.5 }; (r.clamp(-32768.0, 32767.0) as i32 as i16).to_le_bytes() };
        for axis in 0..3 {
            let a = if pwr & 4 != 0 { q(accel[axis] * acc_lsb) } else { [0; 2] };
            let g = if pwr & 2 != 0 { q(gyro[axis] * gyr_lsb) } else { [0; 2] };
            self.regs[DATA_ACC + 2 * axis..DATA_ACC + 2 * axis + 2].copy_from_slice(&a);
            self.regs[DATA_ACC + 6 + 2 * axis..DATA_ACC + 8 + 2 * axis].copy_from_slice(&g);
        }
        self.regs[STATUS] = 0x10 | if pwr & 4 != 0 { 0x80 } else { 0 } | if pwr & 2 != 0 { 0x40 } else { 0 };
        let ticks = (self.seconds() * 25600.0) as u32;  // 39.0625 us per LSB
        self.regs[SENSORTIME..SENSORTIME + 3].copy_from_slice(&ticks.to_le_bytes()[..3]);
        // 0x0000 is 23 C, 1/512 K per LSB; 0x8000 means invalid (temperature sensor off).
        let temp: i16 = if pwr & 0x0e != 0 { 2 * 512 } else { i16::MIN };
        self.regs[TEMPERATURE..TEMPERATURE + 2].copy_from_slice(&temp.to_le_bytes());
    }
    fn store(&mut self, b: u8) {
        let page = (self.regs[FEAT_PAGE as usize] & 7) as usize;
        match self.ptr {
            CMD => { if b == 0xb6 { self.reset(); self.resets += 1; } }
            INIT_DATA => {
                // Word offset from INIT_ADDR, then advance it like the sensor does.
                let word = usize::from(self.regs[INIT_ADDR_0] & 0x0f) | usize::from(self.regs[INIT_ADDR_1]) << 4;
                let at = 2 * word + self.config_bytes % 2;
                if at < self.config.len() { self.config[at] = b; }
                self.config_bytes += 1;
                if self.config_bytes.is_multiple_of(2) {
                    let next = word + 1;
                    self.regs[INIT_ADDR_0] = (next & 0x0f) as u8;
                    self.regs[INIT_ADDR_1] = (next >> 4) as u8;
                }
                return;
            }
            INIT_CTRL => {
                self.regs[INIT_CTRL as usize] = b;
                if b & 1 != 0 {
                    // The whole file must be there for the feature engine to start.
                    let ok = self.config_bytes >= BMI270_CONFIG_SIZE;
                    self.regs[INTERNAL_STATUS] = if ok { 0x01 } else { 0x02 };
                }
            }
            r @ FEATURES..=0x3f => self.pages[page][usize::from(r - FEATURES)] = b,
            // Read-only: chip id, error, status, data, sensortime, events, internal status, temperature.
            0x00..=0x23 => {}
            r => self.regs[usize::from(r)] = b,
        }
        self.ptr = self.ptr.wrapping_add(1);
    }
}

impl I2cDevice for Bmi270 {
    fn start(&mut self, read: bool) -> bool {
        if read { self.latch(); } else { self.first = true; }
        true
    }
    fn write(&mut self, b: u8) -> bool {
        if self.first {
            self.ptr = b;
            self.first = false;
            // INIT_DATA bursts restart at the word INIT_ADDR names.
            if b == INIT_DATA { self.config_bytes &= !1; }
        } else { self.store(b); }
        true
    }
    fn read(&mut self) -> u8 {
        let v = match self.ptr {
            r @ FEATURES..=0x3f => self.pages[(self.regs[FEAT_PAGE as usize] & 7) as usize][usize::from(r - FEATURES)],
            INIT_DATA => {
                // Reads of INIT_DATA return the uploaded file from INIT_ADDR on.
                let word = usize::from(self.regs[INIT_ADDR_0] & 0x0f) | usize::from(self.regs[INIT_ADDR_1]) << 4;
                return self.config.get(2 * word).copied().unwrap_or(0);
            }
            r => self.regs[usize::from(r)],
        };
        self.ptr = self.ptr.wrapping_add(1);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d: &mut Bmi270, reg: u8, data: &[u8]) { d.start(true); d.start(false); d.write(reg); for &b in data { d.write(b); } d.stop(); }
    fn rd(d: &mut Bmi270, reg: u8, n: usize) -> Vec<u8> { d.start(false); d.write(reg); d.start(true); let v = (0..n).map(|_| d.read()).collect(); d.stop(); v }
    fn init(d: &mut Bmi270) {
        wr(d, CMD, &[0xb6]);
        wr(d, PWR_CONF as u8, &[0x02]);
        wr(d, INIT_CTRL, &[0]);
        for chunk in (0..BMI270_CONFIG_SIZE).step_by(32) {
            let word = chunk / 2;
            wr(d, INIT_ADDR_0 as u8, &[(word & 0x0f) as u8, (word >> 4) as u8]);
            wr(d, INIT_DATA, &(0..32).map(|i| (chunk + i) as u8).collect::<Vec<_>>());
        }
        wr(d, INIT_CTRL, &[1]);
    }

    #[test]
    fn chip_id_and_reset_values() {
        let mut d = Bmi270::new(Arc::default(), 1_000_000_000);
        assert_eq!(rd(&mut d, 0x00, 1), [0x24]);
        assert_eq!(rd(&mut d, 0x40, 4), [0xa8, 0x02, 0xa9, 0x00]);
        assert_eq!(rd(&mut d, 0x7c, 2), [0x03, 0x00]);
    }

    #[test]
    fn init_handshake_needs_the_whole_config_file() {
        let mut d = Bmi270::new(Arc::default(), 1_000_000_000);
        wr(&mut d, INIT_CTRL, &[0]);
        wr(&mut d, INIT_DATA, &[0; 64]);
        wr(&mut d, INIT_CTRL, &[1]);
        assert_eq!(rd(&mut d, 0x21, 1)[0] & 0x0f, 0x02, "partial file: init_err");
        init(&mut d);
        assert_eq!(rd(&mut d, 0x21, 1)[0] & 0x0f, 0x01, "message: init OK");
        assert_eq!(d.config[0x1234], 0x34, "the file lands at INIT_ADDR offsets");
        wr(&mut d, INIT_ADDR_0 as u8, &[0x0a, 0x09]);   // word 0x9a -> byte 0x134
        assert_eq!(rd(&mut d, INIT_DATA, 1), [0x34]);
        wr(&mut d, CMD, &[0xb6]);
        assert_eq!(rd(&mut d, 0x21, 1), [0], "soft reset unloads the feature engine");
        assert_eq!(d.resets, 2);
    }

    #[test]
    fn feature_pages() {
        let mut d = Bmi270::new(Arc::default(), 1_000_000_000);
        wr(&mut d, FEAT_PAGE, &[1]);
        wr(&mut d, 0x30, &[0x11, 0x22]);
        wr(&mut d, FEAT_PAGE, &[2]);
        assert_eq!(rd(&mut d, 0x30, 2), [0, 0]);
        wr(&mut d, FEAT_PAGE, &[1]);
        assert_eq!(rd(&mut d, 0x30, 2), [0x11, 0x22]);
    }

    #[test]
    fn data_registers_follow_power_and_range() {
        let motion = Arc::new(ImuMotion::default());
        let mut d = Bmi270::new(motion.clone(), 1_000_000_000);
        init(&mut d);
        assert_eq!(rd(&mut d, 0x0c, 12), [0; 12], "sensors off");
        assert_eq!(rd(&mut d, 0x03, 1), [0x10]);
        wr(&mut d, PWR_CTRL as u8, &[0x0e]);
        wr(&mut d, ACC_RANGE as u8, &[0x00]);           // +-2 g
        assert_eq!(rd(&mut d, 0x03, 1), [0xd0]);
        let v = rd(&mut d, 0x0c, 12);
        assert_eq!(v, [0, 0, 0, 0, 0x00, 0x40, 0, 0, 0, 0, 0, 0], "+1 g on z at 16384 LSB/g, gyro 0");
        wr(&mut d, ACC_RANGE as u8, &[0x02]);           // +-8 g
        assert_eq!(rd(&mut d, 0x10, 2), [0x00, 0x10]);
        assert_eq!(rd(&mut d, 0x22, 2), 1024i16.to_le_bytes());
        motion.cycle.store(2_000_000_000, Relaxed);
        assert_eq!(rd(&mut d, 0x18, 3), [0x00, 0xc8, 0x00], "2 s at 39.0625 us per LSB");
        motion.mode.store(1, Relaxed);
        motion.cycle.store(6_500_000_000, Relaxed);
        let tilted = rd(&mut d, 0x0c, 6);
        assert_ne!(tilted, [0, 0, 0, 0, 0x00, 0x10], "the shared script moves the board");
    }
}
