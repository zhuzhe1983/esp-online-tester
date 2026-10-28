//! Boards around the ESP32-P4. One so far: Waveshare's ESP32-P4-WIFI6-Touch-LCD-3.5, the board the
//! rev >= 3 model was checked against (a v3.2 chip). Its wiring, from Waveshare's BSP
//! `waveshare/esp32_p4_wifi6_touch_lcd_3_5` 2.0.2 (include/bsp/esp32_p4_wifi6_touch_lcd_3_5.h,
//! esp32_p4_wifi6_touch_lcd_3_5.c): an ST7796 320 x 480 on GPSPI2 (MOSI 20, CLK 21, CS 23, D/C 26,
//! RST 27) with its backlight on LEDC from GPIO 28; FT5x06-family touch on I2C (SCL 8, SDA 7,
//! RST 29, INT 50); an ES8311 codec on I2S1 (CONFIG_BSP_I2S_NUM 1; DOUT 9, LRCK 10, DIN 11, SCLK 12, MCLK 13) and the
//! speaker amplifier's enable on GPIO 53; an SD slot on slot 0; an ESP32-C6 on SDIO (ESP-Hosted).
//! Modelled: the panel, the touch controller, the codec's registers (its audio is I2S1's).
//! Not yet: the backlight level, the codec's volume.
use esp_soc::devices::{DcsPanel, Ft5x06, Ft5x06Points, FT5X06_ADDRESS};
use esp_periph::i2c::{I2cDevice, Reg8Device};
use esp_soc::board::{BoardEdge, VirtualCycle};
use esp_soc::{Board, BoardModel, NoBoard};

pub fn make_board(name: &str) -> Option<Board> {
    match name {
        "none" | "bare" => Some(Box::new(NoBoard)),
        "waveshare-p4-lcd35" | "p4-lcd35" | "lcd35" => Some(Box::new(WaveshareP4Lcd35::new())),
        _ => None,
    }
}

pub const PIN_LCD_DC: u8 = 26;
pub const PIN_LCD_RST: u8 = 27;
pub const PIN_LCD_BL: u8 = 28;
pub const PIN_PA_EN: u8 = 53;
pub const PIN_TOUCH_INT: u8 = 50;
/// The FT5x06's report period while touched, ID_G_PERIODACTIVE: the driver's init writes 12 ms
/// (esp_lcd_touch_ft5x06.c); its INT pulses low once per report, and once more on release.
const TOUCH_PERIOD: VirtualCycle = 12 * crate::periph::CPU_HZ / 1000;
const TOUCH_PULSE: VirtualCycle = crate::periph::CPU_HZ / 10_000;
const MIN_TOUCH: VirtualCycle = 2 * TOUCH_PERIOD + TOUCH_PULSE;
pub const LCD_COLS: usize = 320;
pub const LCD_ROWS: usize = 480;

/// What the glass shows, upright in portrait: the controller's RAM with its columns reversed
/// (the BSP sets MADCTL.MX to draw the right way round, `esp_lcd_panel_mirror(panel, true, false)`),
/// and red and blue exchanged unless MADCTL's BGR bit is set (the BSP's colour order is BGR,
/// BSP_LCD_COLOR_SPACE in include/bsp/display.h). Pixels arrive high byte first
/// (BSP_LCD_BIGENDIAN). INVON is not applied: the BSP turns it on for this IPS glass.
pub fn lcd_visible(p: &DcsPanel) -> Vec<u16> {
    let mut out = Vec::with_capacity(LCD_COLS * LCD_ROWS);
    let exchange = p.madctl & 0x08 == 0;
    for r in 0..p.rows {
        for x in 0..p.cols {
            let mut px = p.gram[r * p.cols + (p.cols - 1 - x)];
            if exchange { px = (px & 0x07e0) | ((px & 0xf800) >> 11) | ((px & 0x001f) << 11); }
            if !p.on || p.sleeping { px = 0; }
            out.push(px);
        }
    }
    out
}

/// The Waveshare ESP32-P4-WIFI6-Touch-LCD-3.5.
pub struct WaveshareP4Lcd35 {
    /// the ST7796; `lcd_visible` is what the glass shows
    pub panel: DcsPanel,
    /// the speaker amplifier's enable (GPIO 53)
    pub amp: bool,
    /// the fingers on the FT5x06, in its coordinates: the glass's, portrait 320 x 480 (the BSP
    /// maps them 1:1 with no swap or mirror by default, `bsp_display_start`)
    pub touch: Ft5x06Points,
    pub gpio_events: u64,
    cycle: VirtualCycle,
    /// the next INT pulse, and the end of the one under way
    next_pulse: Option<VirtualCycle>,
    pulse_end: Option<VirtualCycle>,
    /// when the finger went down, and a lift held back until the controller has reported the touch
    down_at: VirtualCycle,
    release_at: Option<VirtualCycle>,
    edges: Vec<BoardEdge>,
}
impl Default for WaveshareP4Lcd35 { fn default() -> Self { Self::new() } }
impl WaveshareP4Lcd35 {
    pub fn new() -> Self {
        WaveshareP4Lcd35 { panel: DcsPanel::new(LCD_COLS, LCD_ROWS), amp: false, touch: Default::default(), gpio_events: 0,
                           cycle: 0, next_pulse: None, pulse_end: None, down_at: 0, release_at: None, edges: Vec::new() }
    }
    fn touched(&self) -> bool { self.touch.lock().unwrap().iter().any(|p| p.is_some()) }
}
impl BoardModel for WaveshareP4Lcd35 {
    fn name(&self) -> &'static str { "waveshare-p4-lcd35" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, level) in changes {
            self.gpio_events += 1;
            match pin {
                PIN_LCD_DC => self.panel.dc = level,
                PIN_LCD_RST => if !level { self.panel.reset(); },
                PIN_PA_EN => self.amp = level,
                _ => {}
            }
        }
    }
    /// On I2C1: the FT5x06 at 0x38, and the ES8311 at 0x18 (ES8311_CODEC_DEFAULT_ADDR 0x30 shifted,
    /// esp_codec_dev 1.x platform/audio_codec_ctrl_i2c.c) as its register file, chip ID 0x8311 at 0xFD-0xFE.
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        vec![(0, FT5X06_ADDRESS, Box::new(Ft5x06::new(self.touch.clone()))), (0, 0x18, Box::new(Reg8Device::new("es8311", &[(0xfd, 0x83), (0xfe, 0x11)])))]
    }
    fn touch(&mut self, x: u16, y: u16, down: bool) { self.touch_at(self.cycle, x, y, down); }
    /// A lift comes at least two report periods after its touch, so a click shorter than that (a
    /// mouse click in a browser running slower than real time) is still a touch the driver reads.
    fn touch_at(&mut self, cycle: VirtualCycle, x: u16, y: u16, down: bool) {
        let cycle = cycle.max(self.cycle);
        if !down && cycle < self.down_at + MIN_TOUCH {
            self.release_at = Some(self.down_at + MIN_TOUCH);
            return;
        }
        if down { self.down_at = cycle; self.release_at = None; }
        self.touch.lock().unwrap()[0] = down.then_some((x.min(LCD_COLS as u16 - 1), y.min(LCD_ROWS as u16 - 1)));
        // a report now: the first of a touch, or the one that says the finger has gone
        self.next_pulse = Some(self.next_pulse.map_or(cycle, |p| p.min(cycle)));
    }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_TOUCH_INT, true)] }
    fn next_deadline(&self) -> Option<VirtualCycle> {
        [self.next_pulse, self.pulse_end, self.release_at].into_iter().flatten().min()
    }
    fn advance_to(&mut self, cycle: VirtualCycle) {
        while let Some(due) = self.next_deadline().filter(|&d| d <= cycle) {
            if self.release_at == Some(due) {
                self.release_at = None;
                self.touch.lock().unwrap()[0] = None;
                let after = self.pulse_end.map_or(due, |end| due.max(end + 1));   // not inside a pulse under way
                self.next_pulse = Some(self.next_pulse.map_or(after, |p| p.min(after)));
                continue;
            }
            if self.pulse_end == Some(due) {
                self.pulse_end = None;
                self.edges.push(BoardEdge { cycle: due, pin: PIN_TOUCH_INT, level: true });
            }
            if self.next_pulse == Some(due) && self.pulse_end.is_none() {
                self.edges.push(BoardEdge { cycle: due, pin: PIN_TOUCH_INT, level: false });
                self.pulse_end = Some(due + TOUCH_PULSE);
                self.next_pulse = self.touched().then_some(due + TOUCH_PERIOD);
            } else if self.next_pulse == Some(due) {
                self.next_pulse = self.pulse_end.map(|end| end + 1);                 // after the pulse under way
            }
        }
        self.cycle = cycle;
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn spi_tx(&mut self, host: u8, data: &[u8]) { if host == 2 { for &b in data { self.panel.byte(b); } } }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> { Some((LCD_COLS as u32, LCD_ROWS as u32, lcd_visible(&self.panel), self.panel.pixels_written)) }
    fn display_version(&self) -> u64 { self.panel.pixels_written }
    // LVGL redraws through DMA: push at the regular interval, not on quiet
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { Some((self.panel.gram.clone(), self.panel.cols, self.panel.rows)) }
    fn report(&self) -> String {
        let p = &self.panel;
        format!("[emu] p4-lcd35: {} RAMWR, {} pixels, madctl={:#x} colmod={:#x} inverted={} on={} bbox={:?}; amplifier {}; gpio events {}",
                p.frames, p.pixels_written, p.madctl, p.colmod, p.inverted, p.on, p.bbox(), if self.amp { "on" } else { "off" }, self.gpio_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The touch INT line: a pulse at the touch, one every report period while held, one after the
    /// release, high in between.
    #[test]
    fn touch_int_pulses_while_held_and_once_on_release() {
        let mut b = WaveshareP4Lcd35::new();
        b.advance_to(1000);
        b.touch_at(1000, 10, 20, true);
        b.advance_to(1000 + 2 * TOUCH_PERIOD + 10);
        b.touch_at(1000 + 2 * TOUCH_PERIOD + 10, 0, 0, false);
        b.advance_to(1000 + 5 * TOUCH_PERIOD);
        let falls: Vec<_> = b.take_edges().iter().filter(|e| !e.level).map(|e| e.cycle).collect();
        let p = TOUCH_PERIOD;
        // the release at 2P + 10 falls inside the pulse begun at 2P, so its report follows that one
        assert_eq!(falls, [1000, 1000 + p, 1000 + 2 * p, 1000 + 2 * p + TOUCH_PULSE + 1]);
        assert!(b.next_deadline().is_none());
    }

    /// A click shorter than two reports is held down until the controller has reported it.
    #[test]
    fn a_short_click_is_still_reported() {
        let mut b = WaveshareP4Lcd35::new();
        b.touch_at(0, 100, 200, true);
        b.touch_at(1000, 100, 200, false);
        b.advance_to(TOUCH_PERIOD + 10);
        assert!(b.touched());
        b.advance_to(MIN_TOUCH + 4 * TOUCH_PERIOD);
        assert!(!b.touched());
        let falls = b.take_edges().iter().filter(|e| !e.level).count();
        assert_eq!(falls, 4);                    // at 0, P, 2P, and the report of the lift
    }
}
