//! The LilyGO TTGO T-Display: an ESP32-D0WDQ6-V3 module (4 MB flash, no PSRAM) with an ST7789V
//! 1.14" 135×240 IPS panel on 4-wire SPI with a D/C line, two buttons and a battery divider.
//!
//! Wiring: SCLK GPIO 18, MOSI GPIO 19 (no MISO), CS GPIO 5, D/C GPIO 16, RST GPIO 23, backlight
//! GPIO 4 (active high); right button GPIO 35 (input only, pulled up on the board), left button
//! GPIO 0 (BOOT); battery divider on GPIO 34 (ADC1 channel 6) with its enable on GPIO 14; I2C
//! header SDA 21 / SCL 22 with nothing attached.
//!
//! The panel is selected by its pins, not by the SPI host: MicroPython's `SPI(2)` is VSPI
//! (classic SPI3) and Arduino's TFT_eSPI uses the same VSPI, but either can be routed through
//! the GPIO matrix from HSPI (SPI2) as well. The bus hands the board the routes of each
//! transaction (`SpiPins`); bytes whose SCLK/MOSI are on GPIO 18/19 while CS is low reach the
//! controller, everything else is counted and dropped.
use esp_soc::board::SpiPins;
use esp_soc::devices::DcsPanel;
use esp_soc::BoardModel;

pub const PIN_LCD_SCLK: u8 = 18;
pub const PIN_LCD_MOSI: u8 = 19;
pub const PIN_LCD_CS: u8 = 5;
pub const PIN_LCD_DC: u8 = 16;
pub const PIN_LCD_RST: u8 = 23;
pub const PIN_LCD_BL: u8 = 4;
pub const PIN_BTN_LEFT: u8 = 0;
pub const PIN_BTN_RIGHT: u8 = 35;
pub const PIN_ADC_EN: u8 = 14;
pub const PIN_BAT_ADC: u8 = 34;

/// The 1.14" glass shows 135 of the ST7789's 240 RAM columns and 240 of its 320 rows: RAM
/// columns 52..186 and rows 40..279. Derived from how the drivers address it: st7789py's
/// 135×240 table puts rotation 0 (MADCTL 0x00) at column offset 52, row offset 40, and rotation 2
/// (MADCTL 0xC0, MX|MY: column c lands at RAM 239-c, row r at 319-r) at column offset 53, row
/// offset 40 — 239-(53+134)=52 and 319-(40+239)=40, the same rectangle. The window is the
/// glass's; MADCTL only changes how firmware addresses it.
pub const LCD_VISIBLE_COLS: usize = 135;
pub const LCD_VISIBLE_ROWS: usize = 240;
pub const LCD_COL_OFFSET: usize = 52;
pub const LCD_ROW_OFFSET: usize = 40;

/// What the glass shows, as a portrait frame: RAM rows 40..279 top to bottom and RAM columns
/// 52..186 left to right. Unlike the Waveshare C6 module the scan is not mirrored: the drivers'
/// upright portrait (rotation 0) is MADCTL 0x00, with no MX. Red and blue are exchanged unless
/// MADCTL's BGR bit is set, as on the C6 module (st7789py sets BGR by default, TFT_eSPI's
/// T-Display setup too). INVON is not applied: on this IPS module it compensates the panel's
/// polarity. Asleep or off, the glass is black.
pub fn lcd_visible(p: &DcsPanel) -> Vec<u16> {
    let mut out = Vec::with_capacity(LCD_VISIBLE_COLS * LCD_VISIBLE_ROWS);
    let exchange = p.madctl & 0x08 == 0;
    let blank = !p.on || p.sleeping;
    for y in 0..LCD_VISIBLE_ROWS {
        let row = (LCD_ROW_OFFSET + y) * p.cols + LCD_COL_OFFSET;
        for x in 0..LCD_VISIBLE_COLS {
            let mut px = p.gram[row + x];
            if exchange { px = (px & 0x07e0) | ((px & 0xf800) >> 11) | ((px & 0x001f) << 11); }
            if blank { px = 0; }
            out.push(px);
        }
    }
    out
}

/// The LilyGO TTGO T-Display.
pub struct TtgoTDisplay {
    /// the ST7789V; `lcd_visible` is what the glass shows
    pub panel: DcsPanel,
    /// the CS line, high (deselected) until the firmware drives GPIO 5 low
    pub cs_high: bool,
    pub backlight: bool,
    pub adc_enable: bool,
    display_changes: u64,
    pub gpio_events: u64,
    /// SPI bytes delivered to the controller / dropped (CS high, or not on GPIO 18/19)
    pub spi_bytes: u64, pub spi_dropped: u64,
}
impl Default for TtgoTDisplay { fn default() -> Self { Self::new() } }
impl TtgoTDisplay {
    pub fn new() -> Self {
        TtgoTDisplay { panel: DcsPanel::st7789(), cs_high: true, backlight: false, adc_enable: false, display_changes: 0, gpio_events: 0, spi_bytes: 0, spi_dropped: 0 }
    }
    /// Whether a transaction with these routes is on the panel's wires. Routes the bus could
    /// not decode (no SCLK or MOSI pin known) are taken as the panel's: the module has one SPI
    /// device, and a transaction nobody else can receive is its.
    fn on_panel_wires(pins: &SpiPins) -> bool {
        let routed = pins.sclk != 0 || pins.mosi != 0;
        !routed || (pins.sclk & (1 << PIN_LCD_SCLK) != 0 && pins.mosi & (1 << PIN_LCD_MOSI) != 0)
    }
}
impl BoardModel for TtgoTDisplay {
    fn name(&self) -> &'static str { "ttgo-tdisplay" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, level) in changes {
            self.gpio_events += 1;
            match pin {
                PIN_LCD_CS => self.cs_high = level,
                PIN_LCD_DC => self.panel.dc = level,
                PIN_LCD_RST => if !level { self.panel.reset(); self.display_changes = self.display_changes.wrapping_add(1); },
                PIN_LCD_BL => if self.backlight != level { self.backlight = level; self.display_changes = self.display_changes.wrapping_add(1); },
                PIN_ADC_EN => self.adc_enable = level,
                _ => {}
            }
        }
    }
    fn uses_spi_pins(&self) -> bool { true }
    fn spi_transfer_pins(&mut self, _host: u8, pins: SpiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if !self.cs_high && Self::on_panel_wires(&pins) {
            self.spi_bytes += tx.len() as u64;
            // DCS power/colour commands change visible output without writing GRAM.
            // Observe the completed transfer, including MADCTL parameter bytes.
            let before = (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets);
            for &b in tx { self.panel.byte(b); }
            if before != (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets) {
                self.display_changes = self.display_changes.wrapping_add(1);
            }
        } else {
            self.spi_dropped += tx.len() as u64;
        }
        vec![0xff; rx_len]   // no MISO on the module
    }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_BTN_LEFT, true), (PIN_BTN_RIGHT, true)] }
    fn named_pin(&self, name: &str) -> Option<u8> {
        match name { "boot" | "btn1" | "left" => Some(PIN_BTN_LEFT), "btn2" | "right" => Some(PIN_BTN_RIGHT), _ => None }
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        Some((LCD_VISIBLE_COLS as u32, LCD_VISIBLE_ROWS as u32, if self.backlight { lcd_visible(&self.panel) } else { vec![0; LCD_VISIBLE_COLS * LCD_VISIBLE_ROWS] }, self.display_version()))
    }
    fn display_version(&self) -> u64 { self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.display_changes) }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { Some((self.panel.gram.clone(), self.panel.cols, self.panel.rows)) }
    fn report(&self) -> String {
        let p = &self.panel;
        format!("[emu] tdisplay: {} RAMWR, {} pixels, madctl={:#x} colmod={:#x} inverted={} on={} backlight={} bbox={:?}; spi {} bytes to the panel, {} dropped (CS high or off GPIO 18/19); adc_en={}; gpio events {}",
                p.frames, p.pixels_written, p.madctl, p.colmod, p.inverted, p.on, self.backlight, p.bbox(), self.spi_bytes, self.spi_dropped, self.adc_enable, self.gpio_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::Bus;

    /// the panel's routes as the bus reports them: SCLK on 18, MOSI on 19, CS 5 low
    fn routes() -> SpiPins { SpiPins { sclk: 1 << PIN_LCD_SCLK, mosi: 1 << PIN_LCD_MOSI, miso: None, cs: 1 << PIN_LCD_CS } }
    fn cmd(b: &mut TtgoTDisplay, c: u8, params: &[u8]) {
        b.gpio_changes(&[(PIN_LCD_CS, false), (PIN_LCD_DC, false)]); b.spi_transfer_pins(3, routes(), &[c], 0);
        if !params.is_empty() { b.gpio_changes(&[(PIN_LCD_DC, true)]); b.spi_transfer_pins(3, routes(), params, 0); }
        b.gpio_changes(&[(PIN_LCD_CS, true)]);
    }
    /// st7789py's bring-up: hard reset, SLPOUT, COLMOD 16-bit, INVON, DISPON, then MADCTL
    fn bring_up(b: &mut TtgoTDisplay, madctl: u8) {
        b.gpio_changes(&[(PIN_LCD_RST, true), (PIN_LCD_RST, false), (PIN_LCD_RST, true)]);
        cmd(b, 0x01, &[]); cmd(b, 0x11, &[]); cmd(b, 0x3a, &[0x55]); cmd(b, 0x21, &[]); cmd(b, 0x29, &[]);
        cmd(b, 0x36, &[madctl]);
        b.gpio_changes(&[(PIN_LCD_BL, true)]);
    }
    fn be(v: u16) -> [u8; 2] { v.to_be_bytes() }
    fn window(b: &mut TtgoTDisplay, x0: u16, x1: u16, y0: u16, y1: u16) {
        let (xs, xe, ys, ye) = (be(x0), be(x1), be(y0), be(y1));
        cmd(b, 0x2a, &[xs[0], xs[1], xe[0], xe[1]]); cmd(b, 0x2b, &[ys[0], ys[1], ye[0], ye[1]]);
    }

    /// Rotation 0 of st7789py (MADCTL 0x08: BGR, no mirroring, offsets column 52, row 40): the
    /// driver's (x, y) is the glass's (x, y). The glass is the fixed RAM window.
    #[test]
    fn rotation_0_lands_where_the_driver_puts_it() {
        let mut b = TtgoTDisplay::new();
        bring_up(&mut b, 0x08);
        assert!(b.panel.resets == 2 && !b.panel.sleeping && b.panel.on && b.backlight, "the RST edge and SWRESET both reset: {}", b.panel.resets);
        window(&mut b, 52 + 3, 52 + 4, 40 + 7, 40 + 7);
        cmd(&mut b, 0x2c, &[0xf8, 0x00, 0x07, 0xe0]);              // red, green, high byte first
        let (w, h, px, _) = b.display().unwrap();
        assert_eq!((w, h), (135, 240));
        assert_eq!((px[7 * 135 + 3], px[7 * 135 + 4]), (0xf800, 0x07e0), "BGR set: colours as sent");
        assert_eq!(px[7 * 135 + 5], 0);
        assert_eq!(b.panel.bbox(), Some((55, 47, 56, 47)), "RAM: column 52+x, row 40+y");
        // the four corners of the driver's 135x240 are the four corners of the glass
        window(&mut b, 52, 52, 40, 40); cmd(&mut b, 0x2c, &[0xff, 0xff]);
        window(&mut b, 52 + 134, 52 + 134, 40 + 239, 40 + 239); cmd(&mut b, 0x2c, &[0xff, 0xff]);
        let (_, _, px, _) = b.display().unwrap();
        assert_eq!((px[0], px[239 * 135 + 134]), (0xffff, 0xffff));
        assert_eq!(px.iter().filter(|&&p| p != 0).count(), 4);
    }

    /// Rotation 1 (MADCTL 0x68: MX | MV | BGR, offsets column 40, row 53), the app's landscape:
    /// the driver's x walks RAM rows, its y RAM columns mirrored, so landscape (x, y) is glass
    /// (134 - y, x) — the landscape's top edge is the portrait glass's right edge.
    #[test]
    fn rotation_1_is_the_glass_turned() {
        let mut b = TtgoTDisplay::new();
        bring_up(&mut b, 0x68);
        window(&mut b, 40 + 10, 40 + 11, 53 + 20, 53 + 20);
        cmd(&mut b, 0x2c, &[0x07, 0xe0, 0x00, 0x1f]);
        let (_, _, px, _) = b.display().unwrap();
        assert_eq!((px[10 * 135 + 114], px[11 * 135 + 114]), (0x07e0, 0x001f));
        assert_eq!(px.iter().filter(|&&p| p != 0).count(), 2);
        // the landscape's four corners (240x135) are the glass's four corners
        for (x, y) in [(0, 0), (239, 0), (0, 134), (239, 134)] { window(&mut b, 40 + x, 40 + x, 53 + y, 53 + y); cmd(&mut b, 0x2c, &[0xff, 0xff]); }
        let (_, _, px, _) = b.display().unwrap();
        assert_eq!((px[134], px[239 * 135 + 134], px[0], px[239 * 135]), (0xffff, 0xffff, 0xffff, 0xffff), "landscape (0,0) top-right, (239,0) bottom-right, (0,134) top-left");
        assert_eq!(b.panel.bbox(), Some((52, 40, 186, 279)), "RAM window of the glass");
    }

    /// Rotation 2 (0xC8, MX | MY | BGR, offsets 53, 40) reaches the same RAM rectangle from the
    /// other corner: the glass is fixed, the driver's offsets differ by one because MX mirrors
    /// 240 columns around 239.
    #[test]
    fn rotation_2_covers_the_same_glass() {
        let mut b = TtgoTDisplay::new();
        bring_up(&mut b, 0xc8);
        window(&mut b, 53, 53 + 134, 40, 40 + 239);
        cmd(&mut b, 0x2c, &[0xff, 0xff]);                          // one pixel at the driver's (0, 0)
        let (_, _, px, _) = b.display().unwrap();
        assert_eq!(px[239 * 135 + 134], 0xffff, "the driver's top-left is the glass's bottom-right");
        for _ in 1..135 * 240 { b.gpio_changes(&[(PIN_LCD_CS, false), (PIN_LCD_DC, true)]); b.spi_transfer_pins(3, routes(), &[0xff, 0xff], 0); }
        assert_eq!(b.panel.bbox(), Some((52, 40, 186, 279)));
        assert!(b.display().unwrap().2.iter().all(|&p| p == 0xffff), "every glass pixel written exactly through the window");
    }

    #[test]
    fn cs_high_and_other_routes_drop_bytes_colour_order_and_blanking_follow_the_controller() {
        let mut b = TtgoTDisplay::new();
        bring_up(&mut b, 0x00);                                    // RGB order: the glass exchanges red and blue
        window(&mut b, 52, 53, 40, 40);
        cmd(&mut b, 0x2c, &[0xf8, 0x00, 0x00, 0x1f]);
        let (_, _, px, _) = b.display().unwrap();
        assert_eq!((px[0], px[1]), (0x001f, 0xf800));
        assert_eq!(b.spi_dropped, 0);
        // CS high: the controller ignores the clock
        b.gpio_changes(&[(PIN_LCD_CS, true), (PIN_LCD_DC, true)]);
        b.spi_transfer_pins(3, routes(), &[0x07, 0xe0], 0);
        assert_eq!((b.panel.pixels_written, b.spi_dropped), (2, 2));
        // CS low but the clock on another GPIO (HSPI on its IO_MUX pins): not this panel
        b.gpio_changes(&[(PIN_LCD_CS, false)]);
        b.spi_transfer_pins(2, SpiPins { sclk: 1 << 14, mosi: 1 << 13, miso: None, cs: 1 << PIN_LCD_CS }, &[0x07, 0xe0], 0);
        assert_eq!((b.panel.pixels_written, b.spi_dropped), (2, 4));
        // routes the bus could not decode are taken as the panel's
        assert_eq!(b.spi_transfer_pins(3, SpiPins::default(), &[0x07, 0xe0], 3), vec![0xff; 3], "MISO is not connected");
        assert_eq!(b.panel.pixels_written, 3);
        // a routed transaction on the right wires, from either host
        b.spi_transfer_pins(2, routes(), &[0x07, 0xe0], 0);
        assert_eq!(b.panel.pixels_written, 4);
        // the glass is black while the controller sleeps or the display is off; RAM is kept
        cmd(&mut b, 0x28, &[]);
        assert!(b.display().unwrap().2.iter().all(|&p| p == 0));
        cmd(&mut b, 0x29, &[]); cmd(&mut b, 0x10, &[]);
        assert!(b.display().unwrap().2.iter().all(|&p| p == 0));
        cmd(&mut b, 0x11, &[]);
        assert_eq!(b.display().unwrap().2[0], 0x07e0, "the window wrapped: the last greens overwrote the first pixels");
        // the reset line: registers back to power-on, RAM kept
        b.gpio_changes(&[(PIN_LCD_RST, false)]);
        assert!(b.panel.sleeping && b.panel.madctl == 0 && b.panel.gram[40 * 240 + 52] == 0x07e0);
        assert_eq!(b.gram().unwrap().1, 240);
    }

    #[test]
    fn visible_control_changes_invalidate_the_frame_without_pixel_writes() {
        let mut b = TtgoTDisplay::new();
        bring_up(&mut b, 0x08);
        window(&mut b, 52, 52, 40, 40);
        cmd(&mut b, 0x2c, &[0xf8, 0]);
        let writes = b.panel.pixels_written;
        for (off, on) in [(0x28, 0x29), (0x10, 0x11)] {
            let before = b.display_version();
            cmd(&mut b, off, &[]);
            assert_ne!(b.display_version(), before);
            assert!(b.display().unwrap().2.iter().all(|&p| p == 0));
            let dark = b.display_version();
            cmd(&mut b, on, &[]);
            assert_ne!(b.display_version(), dark);
            assert_eq!(b.display().unwrap().2[0], 0xf800);
        }
        let lit = b.display_version();
        b.gpio_changes(&[(PIN_LCD_BL, false)]);
        assert_ne!(b.display_version(), lit);
        assert!(b.display().unwrap().2.iter().all(|&p| p == 0));
        b.gpio_changes(&[(PIN_LCD_BL, true)]);
        assert_eq!(b.display().unwrap().2[0], 0xf800);
        let colour = b.display_version();
        cmd(&mut b, 0x36, &[0]); // MADCTL BGR is a parameter, not a command byte.
        assert_ne!(b.display_version(), colour);
        assert_eq!(b.display().unwrap().2[0], 0x001f);
        assert_eq!(b.panel.pixels_written, writes);
        assert_eq!(b.display().unwrap().3, b.display_version());
    }

    #[test]
    fn buttons_read_released_and_have_names() {
        let b = TtgoTDisplay::new();
        assert_eq!(b.input_levels(), vec![(PIN_BTN_LEFT, true), (PIN_BTN_RIGHT, true)]);
        assert_eq!((b.named_pin("boot"), b.named_pin("left"), b.named_pin("btn1")), (Some(0), Some(0), Some(0)));
        assert_eq!((b.named_pin("right"), b.named_pin("btn2"), b.named_pin("knob")), (Some(35), Some(35), None));
        assert!(b.uses_spi_pins() && b.uses_gpio_edges());
        assert!(b.report().starts_with("[emu] tdisplay: 0 RAMWR"));
    }

    /// The whole path through the classic SoC bus: the firmware's GPIO-matrix routes (VSPICLK_out
    /// 63 on GPIO 18, VSPID_out 65 on GPIO 19, as the IDF driver routes a non-IO_MUX pin set),
    /// CS/DC/RST as plain GPIO outputs, and SPI3 CPU transfers out of W0.
    #[test]
    fn spi3_transfers_through_the_bus_reach_the_glass() {
        const GPIO: u32 = 0x3ff4_4000;
        const IO_MUX: u32 = 0x3ff4_9000;
        const SPI3: u32 = 0x3ff6_5000;
        // IO_MUX pad registers (soc/esp32/register/soc/io_mux_reg.h): GPIO0 0x44, GPIO4 0x48, GPIO5 0x6c,
        // GPIO16 0x4c, GPIO18 0x70, GPIO19 0x74, GPIO23 0x8c, GPIO35 0x18; MCU_SEL bits 14:12 (2 = GPIO), FUN_IE bit 9
        let mut b = crate::bus::SocBus::new(4 << 20, [0; 6]);
        b.board = Box::new(TtgoTDisplay::new());
        b.attach_board_devices();
        for off in [0x44, 0x18] { b.write32(IO_MUX + off, (2 << 12) | (1 << 9)).unwrap(); }
        assert_eq!(b.read32(GPIO + 0x3c).unwrap() & 1, 1, "GPIO 0 (left button) reads released");
        assert_eq!(b.read32(GPIO + 0x40).unwrap() & (1 << 3), 1 << 3, "GPIO 35 (right button) reads released");
        for off in [0x48, 0x6c, 0x4c, 0x70, 0x74, 0x8c] { b.write32(IO_MUX + off, 2 << 12).unwrap(); }
        for (pin, signal) in [(4, 256), (5, 256), (16, 256), (23, 256), (18, 63), (19, 65)] { b.write32(GPIO + 0x530 + 4 * pin, signal).unwrap(); }
        b.write32(GPIO + 0x08, (1 << 5) | (1 << 23)).unwrap();                       // OUT_W1TS: CS and RST idle high
        b.write32(GPIO + 0x24, (1 << 4) | (1 << 5) | (1 << 16) | (1 << 18) | (1 << 19) | (1 << 23)).unwrap();   // ENABLE_W1TS
        b.write32(GPIO + 0x0c, 1 << 23).unwrap(); b.write32(GPIO + 0x08, 1 << 23).unwrap();   // hard reset
        let xfer = |b: &mut crate::bus::SocBus, dc: bool, bytes: &[u8]| {
            b.write32(GPIO + if dc { 0x08 } else { 0x0c }, 1 << 16).unwrap();
            b.write32(GPIO + 0x0c, 1 << 5).unwrap();
            for (i, chunk) in bytes.chunks(4).enumerate() { let mut w = 0u32; for (k, &v) in chunk.iter().enumerate() { w |= u32::from(v) << (8 * k); } b.write32(SPI3 + 0x80 + 4 * i as u32, w).unwrap(); }
            b.write32(SPI3 + 0x1c, 1 << 27).unwrap();                                       // USER: MOSI phase
            b.write32(SPI3 + 0x28, bytes.len() as u32 * 8 - 1).unwrap();                     // MOSI_DLEN
            b.write32(SPI3, 1 << 18).unwrap();                                              // CMD.USR
            assert_eq!(b.read32(SPI3).unwrap() & (1 << 18), 0, "the transaction completes at once");
            b.write32(GPIO + 0x08, 1 << 5).unwrap();
        };
        let cmd = |b: &mut crate::bus::SocBus, c: u8, params: &[u8]| { xfer(b, false, &[c]); if !params.is_empty() { xfer(b, true, params); } };
        cmd(&mut b, 0x11, &[]); cmd(&mut b, 0x3a, &[0x55]); cmd(&mut b, 0x29, &[]); cmd(&mut b, 0x36, &[0x68]);
        cmd(&mut b, 0x2a, &[0, 40 + 10, 0, 40 + 11]); cmd(&mut b, 0x2b, &[0, 53 + 20, 0, 53 + 20]);
        cmd(&mut b, 0x2c, &[0x07, 0xe0, 0x00, 0x1f]);
        b.write32(GPIO + 0x08, 1 << 4).unwrap();                                            // backlight on
        let (w, h, px, _) = b.board.display().unwrap();
        assert_eq!((w, h), (135, 240));
        assert_eq!((px[10 * 135 + 114], px[11 * 135 + 114]), (0x07e0, 0x001f));
        assert_eq!(px.iter().filter(|&&p| p != 0).count(), 2);
        // with CS left high the same bytes do not reach the controller
        b.write32(GPIO + 0x08, 1 << 16).unwrap();
        b.write32(SPI3 + 0x80, 0xe007).unwrap(); b.write32(SPI3 + 0x28, 15).unwrap(); b.write32(SPI3, 1 << 18).unwrap();
        assert_eq!(b.board.display().unwrap().2.iter().filter(|&&p| p != 0).count(), 2);
        assert_eq!(b.periph.spi[1].transfers, 13, "7 commands, 5 parameter blocks, 1 dropped");
        let report = b.board.report();
        assert!(report.contains("1 RAMWR, 2 pixels, madctl=0x68") && report.contains("backlight=true") && report.contains("spi 21 bytes to the panel, 2 dropped"), "{report}");
    }
}
