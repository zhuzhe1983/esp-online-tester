//! The Waveshare ESP32-C6-LCD-1.47: the board the C6 model was brought up against — an ST7789
//! 172×320 panel on SPI2, one WS2812 on GPIO 8 (RMT), a BOOT button on GPIO 9 and a TF card slot
//! on the same SPI bus (not modelled).
use esp_soc::devices::{DcsPanel, Ws2812Chain};
use esp_soc::BoardModel;

/// The WS2812 on GPIO 8.
pub const PIN_LED: u8 = 8;
/// The BOOT button on GPIO 9.
pub const PIN_BOOT: u8 = 9;
pub const PIN_LCD_CS: u8 = 14;
pub const PIN_LCD_DC: u8 = 15;
pub const PIN_LCD_RST: u8 = 21;
pub const PIN_LCD_BL: u8 = 22;

/// The 1.47" module shows 172 of the ST7789's 240 RAM columns: RAM columns 34..206.
pub const LCD_VISIBLE_COLS: usize = 172;
pub const LCD_COL_OFFSET: usize = 34;

/// What the glass shows: the module's 172 columns, in the direction the mirrored scan puts
/// them (firmware for this module sets MADCTL.MX), and red and blue exchanged unless MADCTL's
/// BGR bit is set. That is this module's glass, seen on the board: with BGR clear, a firmware's
/// green text was blue and its cyan title cyan (its pixels were also in the wrong byte order,
/// which the controller model takes from RAMCTRL); the scanner build with BGR set shows LVGL's
/// red as red. INVON is not applied: on this IPS module it compensates the panel's polarity.
pub fn lcd_visible(p: &DcsPanel) -> Vec<u16> {
    let mut out = Vec::with_capacity(LCD_VISIBLE_COLS * p.rows);
    let exchange = p.madctl & 0x08 == 0;
    for r in 0..p.rows {
        for x in 0..LCD_VISIBLE_COLS {
            let c = LCD_COL_OFFSET + LCD_VISIBLE_COLS - 1 - x;
            let mut px = p.gram[r * p.cols + c];
            if exchange { px = (px & 0x07e0) | ((px & 0xf800) >> 11) | ((px & 0x001f) << 11); }
            if !p.on || p.sleeping { px = 0; }
            out.push(px);
        }
    }
    out
}

/// The Waveshare ESP32-C6-LCD-1.47.
pub struct WaveshareC6Lcd147 {
    /// the ST7789; `lcd_visible` is what the glass shows
    pub panel: DcsPanel,
    pub led: Ws2812Chain,
    pub backlight: bool, pub gpio_events: u64,
}
impl Default for WaveshareC6Lcd147 { fn default() -> Self { Self::new() } }
impl WaveshareC6Lcd147 {
    pub fn new() -> Self { WaveshareC6Lcd147 { panel: DcsPanel::st7789(), led: Ws2812Chain::new(1), backlight: false, gpio_events: 0 } }
}
impl BoardModel for WaveshareC6Lcd147 {
    fn name(&self) -> &'static str { "waveshare-c6-lcd147" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, level) in changes {
            self.gpio_events += 1;
            match pin {
                PIN_LCD_DC => self.panel.dc = level,
                PIN_LCD_RST => if !level { self.panel.reset(); },
                PIN_LCD_BL => self.backlight = level,
                _ => {}
            }
        }
    }
    fn rmt_frame(&mut self, _pin: u8, bits: &[bool]) { self.led.from_bits(bits); }
    fn spi_tx(&mut self, host: u8, data: &[u8]) { if host == 2 { for &b in data { self.panel.byte(b); } } }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> { Some((LCD_VISIBLE_COLS as u32, self.panel.rows as u32, lcd_visible(&self.panel), self.panel.pixels_written)) }
    fn display_version(&self) -> u64 { self.panel.pixels_written }
    // LVGL redraws continuously through DMA: push at the regular interval, not on quiet
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { Some((self.panel.gram.clone(), self.panel.cols, self.panel.rows)) }
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { Some((&self.led.leds, self.led.updates)) }
    fn named_pin(&self, name: &str) -> Option<u8> { match name { "boot" | "btn" | "btn1" => Some(PIN_BOOT), _ => None } }
    fn report(&self) -> String {
        let p = &self.panel;
        format!("[emu] lcd147: {} RAMWR, {} pixels, madctl={:#x} colmod={:#x} inverted={} on={} backlight={} bbox={:?}; led {:?} ({} updates); gpio events {}",
                p.frames, p.pixels_written, p.madctl, p.colmod, p.inverted, p.on, self.backlight, p.bbox(), self.led.leds[0], self.led.updates, self.gpio_events)
    }
}
