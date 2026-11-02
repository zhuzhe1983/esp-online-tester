//! The C3's analog register master and ideal-radio calibration handshakes. The 802.11 MAC is the one
//! the S3 has (`esp_soc::wifi::WifiMac`): the C3 blob drives the same registers at the same offsets.
//! No C3 silicon comparison.
use esp_periph::{Device, RegRam, WriteEffect};
pub use esp_soc::wifi::WifiMac;

/// Ideal-radio IQ estimator: the closed driver starts a sample with CTRL bits0/1,
/// then polls DONE. Correlation outputs remain zero, matching the S3 radio model.
/// Completion latency is deterministic, not an analog timing model. The driver polls DONE;
/// evaluate its timestamp on access so this non-interrupting device needs no ticking or deadline.
#[derive(Default)]
pub struct FeIq {
    ram: RegRam,
    pub now_cycles: u64,
    done_at: Option<u64>,
}
impl Device for FeIq {
    fn read(&mut self, off: u32) -> u32 {
        if off == 0x174 {
            return self.ram.read(off) | (u32::from(self.done_at.is_some_and(|t| self.now_cycles >= t)) << 16);
        }
        self.ram.read(off)
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        if off == 0x144 && value & 3 == 3 && self.ram.read(off) & 3 != 3 {
            self.done_at = Some((self.now_cycles / 2 + 80) * 2);
        }
        if off != 0x174 { self.ram.write(off, value); }
        WriteEffect::NONE
    }
}

pub struct I2cMst {
    pub ram: RegRam,
    pub ana: esp_periph::Regi2c,
}
impl Default for I2cMst {
    fn default() -> Self {
        Self::new()
    }
}

impl I2cMst {
    pub fn new() -> Self {
        I2cMst {
            ram: RegRam::new(),
            ana: Default::default(),
        }
    }
    pub fn read(&mut self, off: u32) -> u32 {
        match off {
            0x0 | 0x4 => self.ana.read(0, self.ram.read(off), |_| &0),
            // analog-block handshakes (BBPLL cal, pkdet, txdc/rxdc comparators...): the blob writes a start bit and
            // polls a done bit in 26:24; comparator sign bits 31:30 read as 0 — enough for its search loops to run
            0x40..=0x5c => (self.ram.read(off) & 0x3fff_ffff) | (7 << 24),
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        if off == 0 || off == 4 { self.ana.write(0, v); }
        self.ram.write(off, v);
    }
}

impl Device for I2cMst {
    fn read(&mut self, off: u32) -> u32 {
        I2cMst::read(self, off)
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        I2cMst::write(self, off, v);
        WriteEffect::NONE
    }
}
