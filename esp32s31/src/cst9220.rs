//! Hynitron CST9220 capacitive touch controller (I2C 0x5A) as esp-iot-solution's
//! esp_lcd_touch_cst9220 (master, components/display/lcd_touch/esp_lcd_touch_cst9220) talks to it:
//! a write of two bytes selects a 16-bit register or issues a command (D101 report page, D105
//! sleep, D109 normal, D11E command-mode probe); a read then streams that register. Identity on
//! the HYN212 path: PROJECT_ID (D204) = project 0x6854, chip 0x9220, version; INFO_CHIP_ID (D224)
//! carries the chip byte and the 0xCACA marker; RESOLUTION (D1F8) = 480 x 480. REPORT (D000):
//! records of five bytes (status nibble 6 = pressed, track id in the high nibble, x/y 12 bits)
//! with byte 5 = record count and byte 6 = 0xAB; the host acknowledges with D0 00 AB.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc, Mutex};

pub const ADDRESS: u8 = 0x5a;
const CHIP_ID: u16 = 0x9220;
const PROJECT_ID: u16 = 0x6854;

/// The finger on the glass, shared between the board (which the UI drives) and the controller.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Touch { pub down: bool, pub x: u16, pub y: u16 }

pub struct Cst9220 {
    touch: Arc<Mutex<Touch>>,
    width: u16, height: u16,
    written: Vec<u8>,
    reg: u16,
    pos: usize,
    probed: bool,
    pub sleeping: bool,
    pub reports: u64,
}

impl Cst9220 {
    pub fn new(touch: Arc<Mutex<Touch>>, width: u16, height: u16) -> Self {
        Cst9220 { touch, width, height, written: Vec::new(), reg: 0, pos: 0, probed: false, sleeping: false, reports: 0 }
    }
    fn register(&self) -> Vec<u8> {
        let le16 = |v: u16| v.to_le_bytes();
        match self.reg {
            0x0002 => vec![0, if self.probed { 0x1e } else { 0 }],
            0xd1f8 => [le16(self.width), le16(self.height)].concat(),
            0xd1fc => vec![0x00, 0x00, 0xca, 0xca],
            0xd204 => [le16(PROJECT_ID).to_vec(), le16(CHIP_ID).to_vec(), 1u32.to_le_bytes().to_vec()].concat(),
            0xd224 => [vec![0, (CHIP_ID >> 8) as u8], le16(0xcaca).to_vec()].concat(),
            0xd228 => 0x1234_5678u32.to_le_bytes().to_vec(),
            0xd000 => {
                let t = *self.touch.lock().unwrap();
                let (x, y) = (t.x.min(self.width - 1), t.y.min(self.height - 1));
                let status = if t.down { 0x06 } else { 0x00 };
                // record 0 occupies bytes 0-4, then count and the 0xAB marker
                vec![status, (x >> 4) as u8, (y >> 4) as u8, ((x & 15) << 4 | (y & 15)) as u8, 0, 1, 0xab]
            }
            _ => Vec::new(),
        }
    }
}

impl I2cDevice for Cst9220 {
    fn start(&mut self, read: bool) -> bool {
        if read { self.pos = 0; if self.reg == 0xd000 { self.reports += 1; } } else { self.written.clear(); }
        true
    }
    fn write(&mut self, b: u8) -> bool {
        self.written.push(b);
        if self.written.len() == 2 {
            let w = u16::from(self.written[0]) << 8 | u16::from(self.written[1]);
            match w {
                0xd11e => self.probed = true,
                0xd105 => self.sleeping = true,
                0xd109 => self.sleeping = false,
                _ => {}
            }
            self.reg = w;
        }
        true
    }
    fn read(&mut self) -> u8 {
        let b = self.register().get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(d: &mut Cst9220, reg: u16, n: usize) -> Vec<u8> {
        d.start(false); d.write((reg >> 8) as u8); d.write(reg as u8); d.stop();
        d.start(true); (0..n).map(|_| d.read()).collect()
    }
    #[test]
    fn identity_and_report_as_the_esp_iot_solution_driver_decodes_them() {
        let touch = Arc::new(Mutex::new(Touch::default()));
        let mut d = Cst9220::new(touch.clone(), 480, 480);
        d.start(false); d.write(0xd1); d.write(0x1e); d.stop();
        assert_eq!(read(&mut d, 0x0002, 2)[1], 0x1e, "command-mode probe");
        let p = read(&mut d, 0xd204, 8);
        assert_eq!(u16::from_le_bytes([p[2], p[3]]), 0x9220);
        let c = read(&mut d, 0xd224, 4);
        assert_eq!((c[1], u16::from_le_bytes([c[2], c[3]])), (p[3], 0xcaca));
        assert_eq!(read(&mut d, 0xd1f8, 4), [0xe0, 1, 0xe0, 1]);
        assert_eq!(read(&mut d, 0xd000, 7)[..7], [0, 0, 0, 0, 0, 1, 0xab], "no finger: a release record");
        *touch.lock().unwrap() = Touch { down: true, x: 300, y: 123 };
        let r = read(&mut d, 0xd000, 7);
        assert_eq!(r[0] & 15, 6);
        assert_eq!((u16::from(r[1]) << 4 | u16::from(r[3] >> 4), u16::from(r[2]) << 4 | u16::from(r[3] & 15)), (300, 123));
    }
}
