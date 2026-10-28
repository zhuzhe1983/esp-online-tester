//! A FocalTech FT5x06-family capacitive touch controller on I2C (address 0x38), as Espressif's
//! `esp_lcd_touch_ft5x06` driver (1.x, esp_lcd_touch_ft5x06.c) reads it: a register file behind
//! an auto-incrementing pointer (the first byte of a write), TD_STATUS at 0x02 with the number of
//! points, then six bytes per point from 0x03: event flag 7:6 and X 11:8, X 7:0, touch ID 7:4
//! and Y 11:8, Y 7:0, weight, area. The driver's init writes thresholds at 0x80-0x89, which are
//! kept and read back. Coordinates are the controller's own; the board maps the glass onto them.
use std::sync::{Arc, Mutex};
use esp_periph::i2c::I2cDevice;

pub const FT5X06_ADDRESS: u8 = 0x38;
/// Points the controller reports at once; the driver takes up to five.
pub const FT5X06_POINTS: usize = 5;

/// The fingers on the glass, shared between the board (which places them) and the controller.
pub type Ft5x06Points = Arc<Mutex<[Option<(u16, u16)>; FT5X06_POINTS]>>;

pub struct Ft5x06 { regs: [u8; 256], ptr: u8, first: bool, points: Ft5x06Points, pub reads: u64 }
impl Ft5x06 {
    pub fn new(points: Ft5x06Points) -> Self {
        let mut regs = [0u8; 256];
        regs[0xa3] = 0x55;                 // ID_G_CIPHER: the FT5x06's chip ID
        regs[0xa8] = 0x11;                 // ID_G_FT5201ID: FocalTech
        Ft5x06 { regs, ptr: 0, first: true, points, reads: 0 }
    }
    /// TD_STATUS and the point records, from the fingers down now. The records are packed: the
    /// n-th finger down is the n-th record, its touch ID its slot.
    fn latch(&mut self) {
        let points = *self.points.lock().unwrap();
        let mut n = 0;
        for (id, p) in points.iter().enumerate() {
            let Some((x, y)) = *p else { continue };
            let r = 3 + 6 * n;
            self.regs[r] = 0x80 | (x >> 8 & 0x0f) as u8;           // event 2: contact
            self.regs[r + 1] = x as u8;
            self.regs[r + 2] = (id as u8) << 4 | (y >> 8 & 0x0f) as u8;
            self.regs[r + 3] = y as u8;
            self.regs[r + 4] = 0x40;                                // weight
            self.regs[r + 5] = 0x10;                                // area
            n += 1;
        }
        for r in 3 + 6 * n..3 + 6 * FT5X06_POINTS { self.regs[r] = 0xff; }
        self.regs[2] = n as u8;
    }
}
impl I2cDevice for Ft5x06 {
    fn start(&mut self, read: bool) -> bool {
        if read { self.latch(); } else { self.first = true; }
        true
    }
    fn write(&mut self, b: u8) -> bool {
        if self.first { self.ptr = b; self.first = false; } else { self.regs[self.ptr as usize] = b; self.ptr = self.ptr.wrapping_add(1); }
        true
    }
    fn read(&mut self) -> u8 {
        if self.ptr == 2 { self.reads += 1; }
        let v = self.regs[self.ptr as usize];
        self.ptr = self.ptr.wrapping_add(1);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read_at(d: &mut Ft5x06, reg: u8, n: usize) -> Vec<u8> {
        d.start(false); d.write(reg); d.start(true);
        (0..n).map(|_| d.read()).collect()
    }
    #[test]
    fn points_read_as_the_driver_decodes_them() {
        let points: Ft5x06Points = Default::default();
        let mut d = Ft5x06::new(points.clone());
        assert_eq!(read_at(&mut d, 0x02, 1), [0]);
        points.lock().unwrap()[0] = Some((300, 470));
        points.lock().unwrap()[1] = Some((5, 6));
        assert_eq!(read_at(&mut d, 0x02, 1), [2]);
        let r = read_at(&mut d, 0x03, 12);
        // esp_lcd_touch_ft5x06_read_data: x = (d[0] & 0x0f) << 8 | d[1], y = (d[2] & 0x0f) << 8 | d[3], id = d[2] >> 4
        assert_eq!(((r[0] as u16 & 0x0f) << 8 | r[1] as u16, (r[2] as u16 & 0x0f) << 8 | r[3] as u16, r[2] >> 4), (300, 470, 0));
        assert_eq!(((r[6] as u16 & 0x0f) << 8 | r[7] as u16, (r[8] as u16 & 0x0f) << 8 | r[9] as u16, r[8] >> 4), (5, 6, 1));
        d.start(false); d.write(0x80); d.write(70); d.write(60);
        assert_eq!(read_at(&mut d, 0x80, 2), [70, 60]);
    }
}
