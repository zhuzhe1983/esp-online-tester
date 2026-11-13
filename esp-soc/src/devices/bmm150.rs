//! A Bosch BMM150 3-axis geomagnetic sensor on I2C (0x10-0x13 by CSB/SDO), as the Bosch BMM150
//! Sensor API (bmm150.c) drives it. Datasheet BST-BMM150-DS001, section 5 ("Register map"):
//!
//! - Power control 0x4B bit 0 leaves suspend mode (bits 7 and 1 together: soft reset). In
//!   suspend only 0x4B is accessible: everything else reads 0x00 and ignores writes (5.2), so
//!   CHIP_ID 0x40 = 0x32 is visible only when powered. `bmm150_init` sets the bit, waits 3 ms,
//!   checks the chip ID and reads the trim registers.
//! - Op mode 0x4C bits 2:1 (00 normal, 01 forced, 11 sleep, the reset value) and ODR bits 5:3;
//!   REP_XY 0x51 and REP_Z 0x52 (the presets) are kept.
//! - Data 0x42-0x49 (5.3): X and Y 13-bit (LSB register bits 7:3), Z 15-bit (bits 7:1), RHALL
//!   14-bit (bits 7:2) with data-ready in bit 0, all two's complement little endian.
//! - Trim registers (the API's `read_trim_registers`): 0x5D dig_x1, 0x5E dig_y1, 0x62/0x63
//!   dig_z4, 0x64 dig_x2, 0x65 dig_y2, 0x68/0x69 dig_z2, 0x6A/0x6B dig_z1, 0x6C/0x6D dig_xyz1,
//!   0x6E/0x6F dig_z3, 0x70 dig_xy2, 0x71 dig_xy1.
//!
//! The raw values are the inverse of the API's compensation (`compensate_x/y/z`) for the field
//! the board sets, with RHALL equal to dig_xyz1 (so the temperature-like term drops out): with
//! the default trims the driver reports `field` in uT.
use esp_periph::i2c::I2cDevice;

pub const BMM150_CHIP_ID: u8 = 0x32;

/// Plausible factory trims (the range real parts report).
#[derive(Clone, Copy, Debug)]
pub struct Bmm150Trim { pub x1: i8, pub y1: i8, pub x2: i8, pub y2: i8, pub z1: u16, pub z2: i16, pub z3: i16, pub z4: i16, pub xy1: u8, pub xy2: i8, pub xyz1: u16 }
impl Default for Bmm150Trim {
    fn default() -> Self { Bmm150Trim { x1: 0, y1: 0, x2: -3, y2: -3, z1: 23_920, z2: 748, z3: 0, z4: 0, xy1: 29, xy2: -3, xyz1: 6926 } }
}

pub struct Bmm150 {
    regs: [u8; 256],
    ptr: u8,
    first: bool,
    trim: Bmm150Trim,
    /// The field the sensor sees, in uT (sensor axes). Earth-like by default.
    pub field: [f64; 3],
}

impl Bmm150 {
    pub fn new(trim: Bmm150Trim) -> Self {
        let mut d = Bmm150 { regs: [0; 256], ptr: 0, first: true, trim, field: [20.0, 0.0, -40.0] };
        d.reset();
        d
    }
    fn reset(&mut self) {
        let power = self.regs[0x4b] & 1;
        self.regs = [0; 256];
        self.regs[0x4b] = power;
        self.regs[0x40] = BMM150_CHIP_ID;
        self.regs[0x4c] = 0x06;             // sleep mode, 10 Hz
        self.regs[0x51] = 0x04;
        self.regs[0x52] = 0x0e;
        let t = self.trim;
        self.regs[0x5d] = t.x1 as u8;
        self.regs[0x5e] = t.y1 as u8;
        self.regs[0x62..0x64].copy_from_slice(&t.z4.to_le_bytes());
        self.regs[0x64] = t.x2 as u8;
        self.regs[0x65] = t.y2 as u8;
        self.regs[0x68..0x6a].copy_from_slice(&t.z2.to_le_bytes());
        self.regs[0x6a..0x6c].copy_from_slice(&t.z1.to_le_bytes());
        self.regs[0x6c..0x6e].copy_from_slice(&t.xyz1.to_le_bytes());
        self.regs[0x6e..0x70].copy_from_slice(&t.z3.to_le_bytes());
        self.regs[0x70] = t.xy2 as u8;
        self.regs[0x71] = t.xy1;
    }
    pub fn powered(&self) -> bool { self.regs[0x4b] & 1 != 0 }
    /// Raw X/Y from uT: compensate_x is ((raw * 256 * (dig_x2 + 160) / 8192) + 8 * dig_x1) / 16
    /// when RHALL == dig_xyz1.
    fn raw_xy(ut: f64, d1: i8, d2: i8) -> i16 {
        let raw = (16.0 * ut - 8.0 * f64::from(d1)) * 8192.0 / (256.0 * (f64::from(d2) + 160.0));
        away(raw).clamp(-4095.0, 4095.0) as i16
    }
    /// Raw Z: compensate_z is (raw - dig_z4) * 131072 / (4 * (dig_z2 + dig_z1 * rhall / 32768)) / 16.
    fn raw_z(&self, ut: f64) -> i16 {
        let t = self.trim;
        let scale = f64::from(t.z2) + f64::from(t.z1) * f64::from(t.xyz1) / 32768.0;
        (away(ut * scale / 2048.0) + f64::from(t.z4)).clamp(-16383.0, 16383.0) as i16
    }
    fn latch(&mut self) {
        if !self.powered() { return; }
        let t = self.trim;
        let x = Self::raw_xy(self.field[0], t.x1, t.x2);
        let y = Self::raw_xy(self.field[1], t.y1, t.y2);
        let z = self.raw_z(self.field[2]);
        self.regs[0x42..0x44].copy_from_slice(&((x << 3) as u16).to_le_bytes());
        self.regs[0x44..0x46].copy_from_slice(&((y << 3) as u16).to_le_bytes());
        self.regs[0x46..0x48].copy_from_slice(&((z << 1) as u16).to_le_bytes());
        // Data ready in normal and forced mode (forced: one measurement, then back to sleep).
        let mode = self.regs[0x4c] >> 1 & 3;
        let drdy = u16::from(mode == 0 || mode == 1);
        self.regs[0x48..0x4a].copy_from_slice(&(t.xyz1 << 2 | drdy).to_le_bytes());
        if mode == 1 { self.regs[0x4c] |= 0x06; }
    }
}

/// Round away from zero: the API's integer compensation truncates toward zero, so a raw value
/// at or just past the target makes both the integer and the float path report it.
fn away(v: f64) -> f64 { v.signum() * v.abs().ceil() }

impl I2cDevice for Bmm150 {
    fn start(&mut self, read: bool) -> bool {
        if read { self.latch(); } else { self.first = true; }
        true
    }
    fn write(&mut self, b: u8) -> bool {
        if self.first { self.ptr = b; self.first = false; return true; }
        match self.ptr {
            0x4b => {
                self.regs[0x4b] = b & 1;
                // Soft reset (bits 7 and 1): registers back to reset values, power bit kept.
                if b & 0x82 == 0x82 { self.reset(); }
            }
            _ if !self.powered() => {}
            0x40..=0x4a | 0x5d..=0x71 => {}  // read only
            r => self.regs[usize::from(r)] = b,
        }
        self.ptr = self.ptr.wrapping_add(1);
        true
    }
    fn read(&mut self) -> u8 {
        let r = self.ptr;
        self.ptr = self.ptr.wrapping_add(1);
        if r != 0x4b && !self.powered() { 0 } else { self.regs[usize::from(r)] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wr(d: &mut Bmm150, reg: u8, data: &[u8]) { d.start(false); d.write(reg); for &b in data { d.write(b); } d.stop(); }
    fn rd(d: &mut Bmm150, reg: u8, n: usize) -> Vec<u8> { d.start(false); d.write(reg); d.start(true); let v = (0..n).map(|_| d.read()).collect(); d.stop(); v }

    /// The Bosch API's floating-point compensation (bmm150.c, BMM150_USE_FLOATING_POINT).
    fn compensate(d: &mut Bmm150) -> [f64; 3] {
        let t = rd(d, 0x5d, 2); let t2 = rd(d, 0x62, 4); let t3 = rd(d, 0x68, 10);
        let (x1, y1) = (f64::from(t[0] as i8), f64::from(t[1] as i8));
        let z4 = f64::from(i16::from_le_bytes([t2[0], t2[1]]));
        let (x2, y2) = (f64::from(t2[2] as i8), f64::from(t2[3] as i8));
        let z2 = f64::from(i16::from_le_bytes([t3[0], t3[1]]));
        let z1 = f64::from(u16::from_le_bytes([t3[2], t3[3]]));
        let xyz1 = f64::from(u16::from_le_bytes([t3[4], t3[5]]));
        let z3 = f64::from(i16::from_le_bytes([t3[6], t3[7]]));
        let (xy2, xy1) = (f64::from(t3[8] as i8), f64::from(t3[9]));
        let v = rd(d, 0x42, 8);
        let x = f64::from(i16::from_le_bytes([v[0], v[1]]) >> 3);
        let y = f64::from(i16::from_le_bytes([v[2], v[3]]) >> 3);
        let z = f64::from(i16::from_le_bytes([v[4], v[5]]) >> 1);
        let rhall = f64::from(u16::from_le_bytes([v[6], v[7]]) >> 2);
        let xy = |raw: f64, d1: f64, d2: f64| {
            let r = xyz1 * 16384.0 / rhall - 16384.0;
            let x4 = xy2 * (r * r / 268435456.0);
            let x5 = x4 + r * xy1 / 16384.0;
            let x7 = raw * ((x5 + 256.0) * (d2 + 160.0));
            (x7 / 8192.0 + d1 * 8.0) / 16.0
        };
        let zc = ((z - z4) * 131072.0 - z3 * (rhall - xyz1)) / ((z2 + z1 * rhall / 32768.0) * 4.0) / 16.0;
        [xy(x, x1, x2), xy(y, y1, y2), zc]
    }

    #[test]
    fn suspend_hides_everything_but_power_control() {
        let mut d = Bmm150::new(Bmm150Trim::default());
        assert_eq!(rd(&mut d, 0x40, 1), [0], "chip id reads 0 in suspend");
        wr(&mut d, 0x4c, &[0x00]);
        wr(&mut d, 0x4b, &[0x01]);
        assert_eq!(rd(&mut d, 0x40, 1), [0x32]);
        assert_eq!(rd(&mut d, 0x4c, 1), [0x06], "the write in suspend was ignored");
        wr(&mut d, 0x4b, &[0x00]);
        assert_eq!(rd(&mut d, 0x4b, 1), [0x00]);
        assert_eq!(rd(&mut d, 0x42, 8), [0; 8]);
    }

    #[test]
    fn compensated_field_is_what_the_board_set() {
        let mut d = Bmm150::new(Bmm150Trim::default());
        wr(&mut d, 0x4b, &[0x01]);
        wr(&mut d, 0x4c, &[0x00]);          // normal mode
        let v = rd(&mut d, 0x42, 8);
        assert_eq!(v[6] & 1, 1, "data ready");
        let f = compensate(&mut d);
        for (got, want) in f.iter().zip([20.0, 0.0, -40.0]) { assert!((got - want).abs() < 0.5, "{f:?}"); }
        d.field = [-12.5, 33.0, 10.0];
        let f = compensate(&mut d);
        for (got, want) in f.iter().zip([-12.5, 33.0, 10.0]) { assert!((got - want).abs() < 0.5, "{f:?}"); }
    }

    #[test]
    fn soft_reset_and_presets() {
        let mut d = Bmm150::new(Bmm150Trim::default());
        wr(&mut d, 0x4b, &[0x01]);
        wr(&mut d, 0x51, &[0x07, 0x16]);    // REP_XY/REP_Z, "regular" preset
        assert_eq!(rd(&mut d, 0x51, 2), [0x07, 0x16]);
        wr(&mut d, 0x4b, &[0x83]);
        assert_eq!(rd(&mut d, 0x51, 2), [0x04, 0x0e]);
        assert_eq!(rd(&mut d, 0x40, 1), [0x32], "still powered after a soft reset");
    }
}
