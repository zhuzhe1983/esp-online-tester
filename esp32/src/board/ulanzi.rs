//! The Ulanzi TC001 pixel clock: a classic ESP32 with 4 MB flash (the layout the AWTRIX web
//! flasher writes) behind a 32×8 WS2812 matrix, three buttons, a light sensor, a battery divider
//! and a buzzer.
//!
//! Wiring, from the AWTRIX 3 firmware written for it (Blueforcer/awtrix3 at `a69571703a88`,
//! `src/DisplayManager.cpp`, `src/PeripheryManager.cpp`, ULANZI environment; the pin block moves
//! between versions): matrix data GPIO 32; buttons left GPIO 26,
//! select GPIO 27, right GPIO 14 (EasyButton with the internal pull-up, pressed = low); light
//! sensor GPIO 35 and battery divider GPIO 34 (ADC1); buzzer GPIO 15; I2C SDA 21 / SCL 22 (a
//! temperature/humidity sensor and an RTC, not modelled). The ADC inputs and the buzzer are not
//! modelled either, nor AWTRIX's `RESET_PIN` GPIO 13 (held low for 5 s it erases the settings);
//! it reads high.
//!
//! The 256 LEDs are one chain, wired in rows from the top-left corner, every second row running
//! right to left: AWTRIX's default matrix layout 0, `NEO_MATRIX_TOP + NEO_MATRIX_LEFT +
//! NEO_MATRIX_ROWS + NEO_MATRIX_ZIGZAG`. `MATRIX_PHYSICAL` undoes the zigzag, so `leds()` and
//! `display()` are in reading order. Layouts 1 and 2 are for other panels and are not modelled.
use esp_soc::devices::Ws2812Chain;
use esp_soc::BoardModel;

pub const PIN_MATRIX: u8 = 32;
pub const PIN_BTN_LEFT: u8 = 26;
pub const PIN_BTN_SELECT: u8 = 27;
pub const PIN_BTN_RIGHT: u8 = 14;
pub const MATRIX_W: usize = 32;
pub const MATRIX_H: usize = 8;

const fn zigzag_rows() -> [usize; MATRIX_W * MATRIX_H] {
    let mut map = [0; MATRIX_W * MATRIX_H];
    let mut i = 0;
    while i < map.len() {
        let (row, col) = (i / MATRIX_W, i % MATRIX_W);
        map[i] = row * MATRIX_W + if row % 2 == 0 { col } else { MATRIX_W - 1 - col };
        i += 1;
    }
    map
}
/// Chain LED `i` sits at `MATRIX_PHYSICAL[i]` = y·32 + x on the glass.
pub const MATRIX_PHYSICAL: [usize; MATRIX_W * MATRIX_H] = zigzag_rows();

/// The Ulanzi TC001.
pub struct UlanziTc001 {
    /// the matrix, in reading order: index y·32 + x
    pub matrix: Ws2812Chain,
    /// RMT frames sent to other pins
    pub other_frames: u64,
    pub gpio_events: u64,
}
impl Default for UlanziTc001 { fn default() -> Self { Self::new() } }
impl UlanziTc001 {
    pub fn new() -> Self { UlanziTc001 { matrix: Ws2812Chain::mapped(&MATRIX_PHYSICAL), other_frames: 0, gpio_events: 0 } }
}
impl BoardModel for UlanziTc001 {
    fn name(&self) -> &'static str { "ulanzi-tc001" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) { self.gpio_events += changes.len() as u64; }
    fn rmt_frame(&mut self, pin: u8, bits: &[bool]) {
        if pin == PIN_MATRIX { self.matrix.from_bits(bits) } else { self.other_frames += 1 }
    }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_BTN_LEFT, true), (PIN_BTN_SELECT, true), (PIN_BTN_RIGHT, true)] }
    fn named_pin(&self, name: &str) -> Option<u8> {
        match name { "left" | "btn1" => Some(PIN_BTN_LEFT), "select" | "middle" | "btn2" => Some(PIN_BTN_SELECT), "right" | "btn3" => Some(PIN_BTN_RIGHT), _ => None }
    }
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { Some((&self.matrix.leds, self.matrix.updates)) }
    /// The matrix as a 32×8 frame, each LED's colour as sent (no brightness or diffuser model).
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let px = self.matrix.leds.iter().map(|&[r, g, b]| ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3)).collect();
        Some((MATRIX_W as u32, MATRIX_H as u32, px, self.matrix.updates))
    }
    fn display_version(&self) -> u64 { self.matrix.updates }
    fn display_frames(&self) -> u64 { self.matrix.updates }
    fn report(&self) -> String {
        let lit = self.matrix.leds.iter().filter(|c| c.iter().any(|&v| v != 0)).count();
        format!("[emu] ulanzi-tc001: {} matrix frames, {} of 256 LEDs lit; {} RMT frames on other pins; gpio events {}",
                self.matrix.updates, lit, self.other_frames, self.gpio_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// one GRB frame of `n` LEDs, all dark except chain LED `lit` in pure red
    fn frame(n: usize, lit: usize) -> Vec<bool> {
        (0..n).flat_map(|i| { let grb: u32 = if i == lit { 0x00ff00 } else { 0 }; (0..24).rev().map(move |b| (grb >> b) & 1 != 0) }).collect()
    }

    #[test]
    fn zigzag_rows_map_to_reading_order() {
        assert_eq!(&MATRIX_PHYSICAL[..2], &[0, 1]);
        assert_eq!(MATRIX_PHYSICAL[31], 31);
        assert_eq!(MATRIX_PHYSICAL[32], 63);     // row 1 starts on the right
        assert_eq!(MATRIX_PHYSICAL[63], 32);
        assert_eq!(MATRIX_PHYSICAL[64], 64);
        assert_eq!(MATRIX_PHYSICAL[255], 224);   // the chain ends bottom-left
    }

    #[test]
    fn rmt_frame_on_gpio32_lights_the_matching_pixel() {
        let mut b = UlanziTc001::new();
        b.rmt_frame(PIN_MATRIX, &frame(256, 40));    // row 1, eighth LED from the right
        let (w, h, px, version) = b.display().unwrap();
        assert_eq!((w, h, version), (32, 8, 1));
        assert_eq!(px.iter().filter(|&&p| p != 0).count(), 1);
        assert_eq!(px[32 + 31 - 8], 0xf800);
        assert_eq!(b.leds().unwrap().0[55], [0xff, 0, 0]);
    }

    #[test]
    fn frames_on_other_pins_and_short_frames_leave_the_matrix_alone() {
        let mut b = UlanziTc001::new();
        b.rmt_frame(PIN_MATRIX + 1, &frame(256, 0));
        b.rmt_frame(PIN_MATRIX, &[true; 23]);
        assert_eq!((b.matrix.updates, b.other_frames), (0, 1));
        assert!(b.matrix.leds.iter().all(|c| *c == [0, 0, 0]));
    }

    #[test]
    fn buttons_idle_high_and_have_names() {
        let b = UlanziTc001::new();
        assert!(b.input_levels().iter().all(|&(_, level)| level));
        assert_eq!(b.named_pin("select"), Some(PIN_BTN_SELECT));
        assert_eq!(b.named_pin("boot"), None);
    }
}
