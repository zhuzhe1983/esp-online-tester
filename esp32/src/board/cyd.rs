//! The ESP32-2432S028 "Cheap Yellow Display": an ESP32-WROOM-32 with a 2.8" 240×320 ILI9341 TFT
//! and an XPT2046 resistive touch controller, on two separate SPI buses.
//!
//! Wiring: TFT SCLK GPIO 14, MOSI GPIO 13, MISO GPIO 12, CS GPIO 15, D/C GPIO 2, backlight GPIO 21
//! (active high); touch CLK 25, MOSI 32, MISO 39, CS 33, IRQ 36. The two devices are told apart by
//! their chip-select lines, not by SPI host: TFT_eSPI and a second `SPIClass` may both be on VSPI.
//!
//! The panel is shown as the firmware uses it: with MADCTL.MV set (TFT_eSPI rotations 1 and 3) the
//! controller's RAM is transposed, so the visible frame is the RAM transposed back, landscape
//! 320×240; with MV clear (rotations 0 and 2) it is the RAM as it is, portrait 240×320. Red and
//! blue are exchanged unless MADCTL's BGR bit is set. Touch coordinates from the UI/`touch` script
//! are pixels of that frame; the XPT2046's 12-bit X/Y are an approximate linear map of the same
//! spot on the glass in landscape (not a calibrated transfer curve), so they do not change with
//! the rotation, as on the real panel.
use esp_soc::devices::DcsPanel;
use esp_soc::BoardModel;

pub const PIN_LCD_CS: u8 = 15;
pub const PIN_LCD_DC: u8 = 2;
pub const PIN_LCD_BL: u8 = 21;
pub const PIN_TOUCH_CS: u8 = 33;
pub const PIN_TOUCH_IRQ: u8 = 36;
pub const LCD_W: usize = 320;
pub const LCD_H: usize = 240;

/// XPT2046 as the Paul Stoffregen library drives it: a control byte (bit 7 set) starts a
/// conversion whose 12-bit result comes out on the two bytes after it, MSB first, one BUSY bit
/// ahead (`transfer16() >> 3` is the value).
#[derive(Default)]
pub struct Xpt2046 { pub down: bool, pub x: u16, pub y: u16, queue: Vec<u8>, pub conversions: u64 }
impl Xpt2046 {
    fn value(&self, ctl: u8) -> u16 {
        // A2..A0 = ctl bits 6..4: 1 = Y, 3 = Z1, 4 = Z2, 5 = X (XPT2046 datasheet table 1)
        let raw_x = 200 + (self.x as u32 * 3700 / LCD_W as u32) as u16;
        let raw_y = 200 + (self.y as u32 * 3700 / LCD_H as u32) as u16;
        match (ctl >> 4) & 7 {
            1 => if self.down { raw_y } else { 0 },
            5 => if self.down { raw_x } else { 0 },
            3 => if self.down { 1500 } else { 0 },       // library: z = Z1 + 4095 - Z2, touched when >= 400
            4 => if self.down { 500 } else { 4095 },
            _ => 0,
        }
    }
    pub fn transfer(&mut self, tx: &[u8], rx_len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(tx.len().max(rx_len));
        for i in 0..tx.len().max(rx_len) {
            out.push(if self.queue.is_empty() { 0 } else { self.queue.remove(0) });
            if let Some(&b) = tx.get(i) {
                if b & 0x80 != 0 {
                    let v = self.value(b) & 0xfff;
                    self.queue = vec![((v >> 5) & 0x7f) as u8, (v << 3) as u8];
                    self.conversions += 1;
                }
            }
        }
        out
    }
}

pub struct Cyd {
    pub panel: DcsPanel,
    pub tft_cs_high: bool,
    pub touch_cs_high: bool,
    pub touch: Xpt2046,
    pub backlight: bool,
    pub gpio_events: u64,
    pub spi_bytes: u64, pub spi_dropped: u64,
}
impl Default for Cyd { fn default() -> Self { Self::new() } }
impl Cyd {
    pub fn new() -> Self {
        Cyd { panel: DcsPanel::new(240, 320), tft_cs_high: true, touch_cs_high: true, touch: Xpt2046::default(), backlight: false, gpio_events: 0, spi_bytes: 0, spi_dropped: 0 }
    }
}

/// Whether the firmware addresses the panel in landscape (MADCTL.MV set: TFT_eSPI rotations 1, 3).
pub fn landscape(p: &DcsPanel) -> bool { p.madctl & 0x20 != 0 }

/// What the glass shows, as the firmware addresses it: landscape 320×240 (the RAM transposed) when
/// MADCTL.MV is set, otherwise portrait 240×320 (the RAM as it is); colours exchanged unless BGR,
/// black while asleep or off. Returns (width, height, pixels).
pub fn lcd_visible(p: &DcsPanel) -> (usize, usize, Vec<u16>) {
    let exchange = p.madctl & 0x08 == 0;
    let blank = !p.on || p.sleeping;
    let (w, h) = if landscape(p) { (LCD_W, LCD_H) } else { (LCD_H, LCD_W) };
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h { for x in 0..w {
        let mut px = if landscape(p) { p.gram[x * p.cols + y] } else { p.gram[y * p.cols + x] };
        if exchange { px = (px & 0x07e0) | ((px & 0xf800) >> 11) | ((px & 0x001f) << 11); }
        out.push(if blank { 0 } else { px });
    } }
    (w, h, out)
}

impl BoardModel for Cyd {
    fn name(&self) -> &'static str { "cyd" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, level) in changes {
            self.gpio_events += 1;
            match pin {
                PIN_LCD_CS => self.tft_cs_high = level,
                PIN_LCD_DC => self.panel.dc = level,
                PIN_LCD_BL => self.backlight = level,
                PIN_TOUCH_CS => self.touch_cs_high = level,
                _ => {}
            }
        }
    }
    fn spi_transfer(&mut self, _host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if !self.touch_cs_high {
            return self.touch.transfer(tx, rx_len);
        }
        if !self.tft_cs_high {
            self.spi_bytes += tx.len() as u64;
            for &b in tx { self.panel.byte(b); }
        } else {
            self.spi_dropped += tx.len() as u64;
        }
        vec![0xff; rx_len]
    }
    /// (x, y) are pixels of the frame `display()` shows; a portrait point is the landscape point
    /// (y, x) on the glass, so the raw X/Y follow the glass, not the rotation.
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        let (x, y) = if landscape(&self.panel) { (x, y) } else { (y, x) };
        self.touch.x = x.min(LCD_W as u16 - 1); self.touch.y = y.min(LCD_H as u16 - 1); self.touch.down = down;
    }
    /// T_IRQ is active low while the panel is pressed (the library's `tirqTouched()` waits for its edge).
    fn touch_irq(&self) -> Option<(u8, bool)> { Some((PIN_TOUCH_IRQ, !self.touch.down)) }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_TOUCH_IRQ, !self.touch.down)] }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let (w, h, px) = lcd_visible(&self.panel);
        Some((w as u32, h as u32, px, self.panel.pixels_written))
    }
    fn display_version(&self) -> u64 { self.panel.pixels_written }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn report(&self) -> String {
        let p = &self.panel;
        format!("[emu] cyd: {} RAMWR, {} pixels, madctl={:#x} colmod={:#x} on={} backlight={}; spi {} bytes to the panel, {} dropped; touch {} conversions; gpio events {}",
                p.frames, p.pixels_written, p.madctl, p.colmod, p.on, self.backlight, self.spi_bytes, self.spi_dropped, self.touch.conversions, self.gpio_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// the library's update(): B1, C1<<16, 91<<16 ... as bytes
    fn z_and_x(t: &mut Xpt2046) -> (i32, i32) {
        t.transfer(&[0xb1], 1);
        let z1 = i32::from(u16::from_be_bytes(t.transfer(&[0x00, 0xc1], 2).try_into().unwrap())) >> 3;
        let z2 = i32::from(u16::from_be_bytes(t.transfer(&[0x00, 0x91], 2).try_into().unwrap())) >> 3;
        let x = i32::from(u16::from_be_bytes(t.transfer(&[0x00, 0xd0], 2).try_into().unwrap())) >> 3;
        (z1 + 4095 - z2, x)
    }
    #[test]
    fn untouched_is_below_the_threshold_touched_is_above() {
        let mut b = Cyd::new();
        assert!(z_and_x(&mut b.touch).0 < 400);
        b.touch(160, 120, true);
        let (z, x) = z_and_x(&mut b.touch);
        assert!(z >= 400 && x > 0, "z={z} x={x}");
        b.touch(0, 0, false);
        assert!(z_and_x(&mut b.touch).0 < 400);
    }
    #[test]
    fn landscape_is_the_ram_transposed() {
        let mut b = Cyd::new();
        b.panel.on = true; b.panel.sleeping = false; b.panel.madctl = 0x28;   // MV | BGR: TFT_eSPI rotation 1
        for (cmd, args) in [(0x2au8, [0u8, 10, 1, 63]), (0x2b, [0, 20, 0, 239])] {
            b.gpio_changes(&[(PIN_LCD_CS, false), (PIN_LCD_DC, false)]); b.spi_transfer(3, &[cmd], 0);
            b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_transfer(3, &args, 0);
        }
        b.gpio_changes(&[(PIN_LCD_DC, false)]); b.spi_transfer(3, &[0x2c], 0);
        b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_transfer(3, &[0xf8, 0x00], 0);
        let (w, h, px, _) = b.display().unwrap();
        assert_eq!((w, h), (320, 240));
        assert_eq!(px[20 * 320 + 10], 0xf800, "landscape (10, 20)");
    }
    #[test]
    fn portrait_is_the_ram_as_it_is_and_touch_follows_the_glass() {
        let mut b = Cyd::new();
        b.panel.on = true; b.panel.sleeping = false; b.panel.madctl = 0x08;   // BGR, MV clear: TFT_eSPI rotation 0
        for (cmd, args) in [(0x2au8, [0u8, 10, 0, 239]), (0x2b, [0, 20, 1, 63])] {
            b.gpio_changes(&[(PIN_LCD_CS, false), (PIN_LCD_DC, false)]); b.spi_transfer(3, &[cmd], 0);
            b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_transfer(3, &args, 0);
        }
        b.gpio_changes(&[(PIN_LCD_DC, false)]); b.spi_transfer(3, &[0x2c], 0);
        b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_transfer(3, &[0xf8, 0x00], 0);
        let (w, h, px, _) = b.display().unwrap();
        assert_eq!((w, h), (240, 320));
        assert_eq!(px[20 * 240 + 10], 0xf800, "portrait (10, 20)");
        assert_eq!(px.iter().filter(|&&p| p != 0).count(), 1);
        // portrait (10, 20) is landscape (20, 10) on the glass: the same raw X/Y either way
        b.touch(10, 20, true);
        let portrait = (b.touch.x, b.touch.y);
        b.panel.madctl = 0x28;
        b.touch(20, 10, true);
        assert_eq!(portrait, (b.touch.x, b.touch.y));
        assert_eq!(portrait, (20, 10));
    }

    /// The whole path through the classic SoC bus: CS 15 / D/C 2 / CS 33 as plain GPIO outputs, the TFT on SPI2
    /// (HSPI) and the touch controller on SPI3 (VSPI), CPU transfers out of W0, and a touch pulling GPIO 36 low.
    #[test]
    fn spi_transfers_through_the_bus_reach_the_glass_and_the_touch_controller() {
        use emu_core::Bus;
        use esp_soc::SocBus;
        const GPIO: u32 = 0x3ff4_4000;
        const IO_MUX: u32 = 0x3ff4_9000;
        const SPI2: u32 = 0x3ff6_4000;
        const SPI3: u32 = 0x3ff6_5000;
        let mut b = crate::bus::SocBus::new(4 << 20, [0; 6]);
        b.board = Box::new(Cyd::new());
        b.attach_board_devices();
        // IO_MUX: GPIO2 0x40, GPIO15 0x3c, GPIO33 0x20 (soc/esp32/register/soc/io_mux_reg.h, IDF v5.5.4; same offsets in v4.4); MCU_SEL = 2 (GPIO)
        for off in [0x40, 0x3c, 0x20] { b.write32(IO_MUX + off, 2 << 12).unwrap(); }
        for pin in [2, 15, 33] { b.write32(GPIO + 0x530 + 4 * pin, 256).unwrap(); }
        b.write32(IO_MUX + 0x04, (2 << 12) | (1 << 9)).unwrap();                       // GPIO36 (T_IRQ, 0x04): input enabled
        b.write32(GPIO + 0x08, 1 << 15).unwrap();                                     // OUT_W1TS: TFT CS idle high
        b.write32(GPIO + 0x24, (1 << 2) | (1 << 15)).unwrap();                         // ENABLE_W1TS
        b.write32(GPIO + 0x30, 1 << 1).unwrap();                                      // ENABLE1_W1TS: GPIO 33
        b.write32(GPIO + 0x14, 1 << 1).unwrap();                                      // OUT1_W1TS: touch CS idle high
        let xfer = |b: &mut crate::bus::SocBus, host: u32, bytes: &[u8]| -> u32 {
            for (i, chunk) in bytes.chunks(4).enumerate() { let mut w = 0u32; for (k, &v) in chunk.iter().enumerate() { w |= u32::from(v) << (8 * k); } b.write32(host + 0x80 + 4 * i as u32, w).unwrap(); }
            b.write32(host + 0x1c, (1 << 27) | (1 << 28)).unwrap();                    // USER: MOSI and MISO phases
            b.write32(host + 0x28, bytes.len() as u32 * 8 - 1).unwrap();                // MOSI_DLEN
            b.write32(host + 0x2c, bytes.len() as u32 * 8 - 1).unwrap();                // MISO_DLEN
            b.write32(host, 1 << 18).unwrap();                                          // CMD.USR
            assert_eq!(b.read32(host).unwrap() & (1 << 18), 0, "the transaction completes at once");
            b.read32(host + 0x80).unwrap()
        };
        let cmd = |b: &mut crate::bus::SocBus, c: u8, params: &[u8]| {
            b.write32(GPIO + 0x0c, 1 << 2).unwrap(); b.write32(GPIO + 0x0c, 1 << 15).unwrap();   // D/C low, CS low
            xfer(b, SPI2, &[c]);
            if !params.is_empty() { b.write32(GPIO + 0x08, 1 << 2).unwrap(); xfer(b, SPI2, params); }
            b.write32(GPIO + 0x08, 1 << 15).unwrap();
        };
        cmd(&mut b, 0x11, &[]); cmd(&mut b, 0x3a, &[0x55]); cmd(&mut b, 0x29, &[]); cmd(&mut b, 0x36, &[0x28]);
        cmd(&mut b, 0x2a, &[0, 10, 1, 63]); cmd(&mut b, 0x2b, &[0, 20, 0, 239]);
        cmd(&mut b, 0x2c, &[0xf8, 0x00]);
        let (w, h, px, _) = b.board.display().unwrap();
        assert_eq!((w, h), (320, 240));
        assert_eq!(px[20 * 320 + 10], 0xf800, "landscape (10, 20), red as sent (BGR set): {}", b.board.report());
        assert_eq!(px.iter().filter(|&&p| p != 0).count(), 1);
        // the touch controller: GPIO 36 idles high, a touch pulls it low; the library's pressure read
        let irq = |b: &mut crate::bus::SocBus| b.read32(GPIO + 0x40).unwrap() & (1 << 4) != 0;
        assert!(irq(&mut b), "T_IRQ idles high");
        b.touch_input(160, 120, true);
        assert!(!irq(&mut b), "T_IRQ is low while pressed");
        b.write32(GPIO + 0x18, 1 << 1).unwrap();                                      // OUT1_W1TC: touch CS low
        xfer(&mut b, SPI3, &[0xb1, 0x00, 0xc1, 0x00, 0x91, 0x00]);
        assert!(b.board.report().contains("touch 3 conversions"), "{}", b.board.report());
        b.write32(GPIO + 0x14, 1 << 1).unwrap();
        b.touch_input(160, 120, false);
        assert!(irq(&mut b), "T_IRQ is high again after the release");
    }
}
