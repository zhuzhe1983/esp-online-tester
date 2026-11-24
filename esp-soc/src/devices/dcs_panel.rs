/// A display controller of the ST77xx family driven over 4-wire SPI with a D/C line: the
/// MIPI DCS commands that place pixels are interpreted (column/page window, RAMWR, MADCTL,
/// COLMOD, inversion, on/off, sleep, software reset, and on the ST7789 RAMCTRL's byte order); the
/// panel-specific porch, gamma and voltage commands are accepted and ignored. The RAM is the controller's, not the glass's — a
/// module's visible window is the board's business.
pub struct DcsPanel {
    pub cols: usize, pub rows: usize,
    /// row-major, `row * cols + col`, RGB565 as the firmware sent it
    pub gram: Vec<u16>,
    pub madctl: u8, pub colmod: u8, pub inverted: bool, pub sleeping: bool, pub on: bool,
    /// RAMCTRL (B0h) second parameter bit 3, ST7789 only: 16-bit pixels arrive low byte first.
    /// Firmware sets it to write a little-endian frame buffer straight out (LVGL without
    /// `LV_COLOR_16_SWAP`); Waveshare's driver for the 1.47" module does (`0xB0: 0x00, 0xE8`).
    pub little_endian: bool,
    ramctrl: bool,
    /// the D/C line: low = command, high = parameter or pixel data. Idles low, as the GPIO does.
    pub dc: bool,
    cmd: u8, args: [u8; 4], argn: u8,
    x0: u16, x1: u16, y0: u16, y1: u16, xc: u16, yc: u16,
    pixel_hi: Option<u8>,
    /// RAMWR commands seen
    pub frames: u64,
    pub pixels_written: u64,
    pub resets: u64,
}

impl DcsPanel {
    /// A controller with `cols` × `rows` of RAM, in its power-on state (asleep, display off,
    /// 18-bit colour, the window the whole RAM).
    pub fn new(cols: usize, rows: usize) -> Self {
        DcsPanel { cols, rows, gram: vec![0; cols * rows], madctl: 0, colmod: 0x66, inverted: false, sleeping: true, on: false, dc: false,
                   little_endian: false, ramctrl: false, cmd: 0, args: [0; 4], argn: 0, x0: 0, x1: cols as u16 - 1, y0: 0, y1: rows as u16 - 1, xc: 0, yc: 0,
                   pixel_hi: None, frames: 0, pixels_written: 0, resets: 0 }
    }
    /// ST7735: 132 × 162 of RAM behind the 0.96" and 1.8" modules.
    pub fn st7735() -> Self { Self::new(132, 162) }
    /// ST7789: 240 × 320 of RAM behind the 1.47", 1.69" and 2.0" modules.
    pub fn st7789() -> Self { let mut p = Self::new(240, 320); p.ramctrl = true; p }

    /// A hardware or software reset: every register back to power-on, the RAM kept (as on silicon).
    pub fn reset(&mut self) {
        let (gram, resets, dc, ramctrl) = (std::mem::take(&mut self.gram), self.resets + 1, self.dc, self.ramctrl);
        *self = Self::new(self.cols, self.rows);
        self.gram = gram; self.resets = resets; self.dc = dc; self.ramctrl = ramctrl;
    }

    /// One byte from the SPI master, a command or data depending on `dc`.
    pub fn byte(&mut self, b: u8) {
        if !self.dc {
            self.cmd = b; self.argn = 0; self.pixel_hi = None;
            match b {
                0x01 => self.reset(),
                0x11 => self.sleeping = false, 0x10 => self.sleeping = true,
                0x20 => self.inverted = false, 0x21 => self.inverted = true,
                0x28 => self.on = false, 0x29 => self.on = true,
                0x2c => { self.xc = self.x0; self.yc = self.y0; self.frames += 1; }
                _ => {}
            }
            return;
        }
        match self.cmd {
            0x2a | 0x2b => {
                if (self.argn as usize) < 4 { self.args[self.argn as usize] = b; self.argn += 1; }
                if self.argn == 4 {
                    let s = u16::from_be_bytes([self.args[0], self.args[1]]);
                    let e = u16::from_be_bytes([self.args[2], self.args[3]]);
                    if self.cmd == 0x2a { self.x0 = s; self.x1 = e; self.xc = s; } else { self.y0 = s; self.y1 = e; self.yc = s; }
                }
            }
            0x36 => self.madctl = b,
            0x3a => self.colmod = b,
            0xb0 if self.ramctrl => { if self.argn == 1 { self.little_endian = b & 0x08 != 0; } self.argn = self.argn.saturating_add(1); }
            // DCS RAMWRC continues the existing address cursor. Waveshare's
            // AXS15231B QSPI BSP emits CASET, then 2Ch for the first vertical
            // chunk and 3Ch for the later chunks without another RASET.
            0x2c | 0x3c => match self.pixel_hi.take() {
                None => self.pixel_hi = Some(b),
                Some(first) => self.write_pixel(if self.little_endian { u16::from_le_bytes([first, b]) } else { u16::from_be_bytes([first, b]) }),
            },
            _ => {}
        }
    }

    fn write_pixel(&mut self, px: u16) {
        // address counters in the controller's frame; MADCTL MV swaps which counter is "column",
        // MX / MY mirror
        let (mut col, mut row) = (self.xc as usize, self.yc as usize);
        if self.madctl & 0x20 != 0 { std::mem::swap(&mut col, &mut row); }
        if self.madctl & 0x40 != 0 { col = self.cols - 1 - col.min(self.cols - 1); }
        if self.madctl & 0x80 != 0 { row = self.rows - 1 - row.min(self.rows - 1); }
        if col < self.cols && row < self.rows { self.gram[row * self.cols + col] = px; self.pixels_written += 1; }
        // advance within the window, x fastest, wrapping to the window's origin
        if self.xc >= self.x1 { self.xc = self.x0; if self.yc >= self.y1 { self.yc = self.y0; } else { self.yc += 1; } } else { self.xc += 1; }
    }

    /// Bounding box of non-zero RAM, `(col0, row0, col1, row1)` — for locating a module's visible window.
    pub fn bbox(&self) -> Option<(usize, usize, usize, usize)> {
        let (mut c0, mut c1, mut r0, mut r1) = (usize::MAX, 0, usize::MAX, 0);
        for r in 0..self.rows { for c in 0..self.cols { if self.gram[r * self.cols + c] != 0 { c0 = c0.min(c); c1 = c1.max(c); r0 = r0.min(r); r1 = r1.max(r); } } }
        if c0 == usize::MAX { None } else { Some((c0, r0, c1, r1)) }
    }

    /// Most common non-zero pixel values (for checking colour decoding).
    pub fn histogram(&self, top: usize) -> Vec<(u16, usize)> {
        let mut m: std::collections::HashMap<u16, usize> = Default::default();
        for &p in &self.gram { if p != 0 { *m.entry(p).or_insert(0) += 1; } }
        let mut v: Vec<(u16, usize)> = m.into_iter().collect(); v.sort_by_key(|a| std::cmp::Reverse(a.1)); v.truncate(top); v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cmd(p: &mut DcsPanel, c: u8, params: &[u8]) { p.dc = false; p.byte(c); p.dc = true; for &b in params { p.byte(b); } }

    #[test]
    fn a_window_fills_x_fastest_and_wraps() {
        let mut p = DcsPanel::new(4, 3);
        cmd(&mut p, 0x2a, &[0, 1, 0, 2]); cmd(&mut p, 0x2b, &[0, 1, 0, 2]);
        cmd(&mut p, 0x2c, &[0, 1, 0, 2, 0, 3, 0, 4, 0, 5]);
        assert_eq!(p.gram, [0, 0, 0, 0, 0, 5, 2, 0, 0, 3, 4, 0]);
        assert_eq!((p.frames, p.pixels_written, p.bbox()), (1, 5, Some((1, 1, 2, 2))));
    }

    #[test]
    fn ramwrc_continues_y_after_repeated_column_window() {
        let mut p = DcsPanel::new(2, 3);
        cmd(&mut p, 0x3a, &[0x55]);
        cmd(&mut p, 0x2a, &[0, 0, 0, 1]);
        cmd(&mut p, 0x2c, &[0xf8, 0, 0x07, 0xe0]);
        cmd(&mut p, 0x2a, &[0, 0, 0, 1]);
        cmd(&mut p, 0x3c, &[0, 0x1f, 0xff, 0xff]);
        assert_eq!(p.gram, [0xf800, 0x07e0, 0x001f, 0xffff, 0, 0]);
        assert_eq!((p.frames, p.pixels_written), (1, 4));
        cmd(&mut p, 0x2c, &[0, 1]);
        assert_eq!(p.gram, [1, 0x07e0, 0x001f, 0xffff, 0, 0]);
    }

    #[test]
    fn madctl_mirrors_and_swaps_axes() {
        let mut p = DcsPanel::new(3, 2);
        cmd(&mut p, 0x36, &[0xc0]);                       // MX | MY
        cmd(&mut p, 0x2a, &[0, 0, 0, 0]); cmd(&mut p, 0x2b, &[0, 0, 0, 0]); cmd(&mut p, 0x2c, &[0, 7]);
        assert_eq!(p.gram[3 + 2], 7, "row 1, col 2");
        cmd(&mut p, 0x36, &[0x20]);                       // MV: the column counter walks rows
        cmd(&mut p, 0x2a, &[0, 1, 0, 1]); cmd(&mut p, 0x2b, &[0, 2, 0, 2]); cmd(&mut p, 0x2c, &[0, 9]);
        assert_eq!(p.gram[3 + 2], 9, "row 1, col 2");
    }

    #[test]
    fn reset_keeps_the_ram_and_the_dc_line() {
        let mut p = DcsPanel::st7735();
        cmd(&mut p, 0x11, &[]); cmd(&mut p, 0x36, &[0x08]); cmd(&mut p, 0x3a, &[0x05]); cmd(&mut p, 0x2c, &[0xab, 0xcd]);
        assert!(!p.sleeping && p.madctl == 0x08 && p.colmod == 0x05);
        cmd(&mut p, 0x01, &[]);
        assert!(p.sleeping && p.madctl == 0 && p.colmod == 0x66 && p.dc && p.resets == 1);
        assert_eq!((p.gram[0], p.cols, p.rows), (0xabcd, 132, 162));
    }

    /// RAMCTRL's endian bit, as Waveshare's ST7789 driver sets it (`0xB0: 0x00, 0xE8`): pixels
    /// then arrive low byte first. Only the ST7789 has the command, and a reset clears it.
    #[test]
    fn st7789_ramctrl_selects_the_pixel_byte_order() {
        let mut p = DcsPanel::st7789();
        cmd(&mut p, 0x2a, &[0, 0, 0, 1]); cmd(&mut p, 0x2b, &[0, 0, 0, 0]);
        cmd(&mut p, 0x2c, &[0x07, 0xe0]);
        assert_eq!(p.gram[0], 0x07e0, "power-on: high byte first");
        cmd(&mut p, 0xb0, &[0x00, 0xe8]);
        cmd(&mut p, 0x2a, &[0, 0, 0, 1]); cmd(&mut p, 0x2c, &[0xe0, 0x07, 0x07, 0xe0]);
        assert_eq!((p.gram[0], p.gram[1]), (0x07e0, 0xe007), "low byte first: the same green, and what high-byte-first data turns into");
        cmd(&mut p, 0xb0, &[0x00, 0xe0]);
        cmd(&mut p, 0x2a, &[0, 0, 0, 1]); cmd(&mut p, 0x2c, &[0x07, 0xe0]);
        assert_eq!(p.gram[0], 0x07e0, "the bit cleared again");
        cmd(&mut p, 0xb0, &[0x00, 0xe8]); cmd(&mut p, 0x01, &[]);
        assert!(!p.little_endian, "software reset: back to power-on");

        let mut other = DcsPanel::st7735();
        cmd(&mut other, 0xb0, &[0x00, 0xe8]);
        assert!(!other.little_endian, "0xB0 is not RAMCTRL on the ST7735");
    }
}
