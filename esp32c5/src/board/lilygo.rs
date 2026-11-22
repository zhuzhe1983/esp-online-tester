//! Exact LilyGO C5 boards; the C5 core still accepts SRAM applications only.
//! Sources: Xinyuan-LilyGO/T7-C5@fbab1a3b8b676dfcc7b1535d514755206602370c
//! readme.md and T-Dongle-C5@0fe7367959509af38b45bd1b81471018932df625
//! include/pin_config.h, lib/lcd_st7735/st7735.{h,cpp}, examples/Factory/Factory.ino.
//! The Dongle currently decodes software-driven SPI only. Ordinary Arduino's
//! hardware SPI, PSRAM, SD, APA102 and Wi-Fi are not emulated by this C5 subset.
use esp_soc::{BoardModel, devices::{DcsPanel, SpiBitBang}, pins::PinRoutes};

pub struct T7C5;
impl BoardModel for T7C5 {
    fn name(&self) -> &'static str { "lilygo-t7-c5" }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(28, true)] }
    fn named_pin(&self, name: &str) -> Option<u8> { matches!(name, "boot" | "button").then_some(28) }
    fn report(&self) -> String { "[lilygo-t7-c5] BOOT G28; SRAM app-direct/UART/GPIO only; no display, RF, battery ADC or Qwiic peripherals".into() }
}

pub struct TDongleC5 {
    panel: DcsPanel, spi: SpiBitBang, backlight: bool, reset: bool, generation: u64,
}
impl Default for TDongleC5 { fn default() -> Self { Self::new() } }
impl TDongleC5 {
    pub fn new() -> Self { Self { panel: DcsPanel::st7735(), spi: SpiBitBang::new(6, 2, 10), backlight: false, reset: false, generation: 1 } }
    fn changed(&mut self) { self.generation = self.generation.wrapping_add(1); }
}
impl BoardModel for TDongleC5 {
    fn name(&self) -> &'static str { "lilygo-t-dongle-c5" }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(28, true)] }
    fn named_pin(&self, name: &str) -> Option<u8> { matches!(name, "boot" | "button").then_some(28) }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_waveform_at(&mut self, _cycle: u64, routes: PinRoutes<'_>) {
        // Factory code explicitly drives G0 LOW to enable the LCD backlight.
        let backlight = routes.software_output(0) == Some(false);
        if self.backlight != backlight { self.backlight = backlight; self.changed(); }
        let reset = routes.software_output(1) == Some(false);
        if self.reset != reset {
            self.reset = reset;
            if reset { self.panel.reset(); self.spi = SpiBitBang::new(6, 2, 10); }
            self.changed();
        }
        let (cs, dc, mosi, sclk) = (routes.software_output(10), routes.software_output(3), routes.software_output(2), routes.software_output(6));
        if reset || dc.is_none() || mosi.is_none() || sclk.is_none() {
            self.spi.pin(10, true); // Never assemble pixels from disconnected or peripheral-routed GPIOs.
            return;
        }
        self.panel.dc = dc == Some(true);
        self.spi.pin(10, cs != Some(false));
        self.spi.pin(2, mosi == Some(true));
        if let Some(byte) = self.spi.pin(6, sclk == Some(true)) { self.panel.byte(byte); self.changed(); }
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let p = &self.panel;
        let visible = self.backlight && !self.reset && p.on && !p.sleeping;
        // Official driver: ST7735 green-tab 80x160, column offset26, row offset1.
        // Native glass coordinates remain portrait; MADCTL is handled in GRAM writes.
        let mut pixels = Vec::with_capacity(80 * 160);
        for y in 1..161 { for x in 26..106 {
            let mut px = if visible { p.gram[y * p.cols + x] } else { 0 };
            if p.madctl & 0x08 == 0 { px = (px & 0x07e0) | ((px & 0xf800) >> 11) | ((px & 0x001f) << 11); }
            pixels.push(px);
        } }
        Some((80, 160, pixels, self.generation))
    }
    fn display_version(&self) -> u64 { self.generation }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { Some((self.panel.gram.clone(), self.panel.cols, self.panel.rows)) }
    fn report(&self) -> String { format!("[lilygo-t-dongle-c5] ST7735 80x160 software-SPI pixels={} frames={} backlight={}; SRAM diagnostic only; hardware SPI/PSRAM/SD/APA102/RF unavailable", self.panel.pixels_written, self.panel.frames, self.backlight) }
}
