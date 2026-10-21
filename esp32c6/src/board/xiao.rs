//! The Seeed Studio XIAO ESP32-C6: an ESP32-C6FH4 module (4 MB flash, no PSRAM) on a 21×17 mm
//! board, with a user LED on GPIO 15, the BOOT button on GPIO 9 and the RF switch on GPIO 3
//! and GPIO 14.
//!
//! Wiring, from the Seeed wiki's pinout and schematic (XIAO_ESP32_C6 v1.0) and the Arduino
//! variant `variants/XIAO_ESP32C6/pins_arduino.h`:
//!
//! | Function | Pin | Notes |
//! | --- | --- | --- |
//! | user LED | GPIO 15 | low active: the anode goes to 3V3 through a resistor, the cathode to the pin. Orange |
//! | BOOT button | GPIO 9 | active low, pulled up on the board |
//! | RF switch power | GPIO 3 | low active: low powers the switch |
//! | RF switch port | GPIO 14 | low = the onboard ceramic antenna, high = the external U.FL |
//!
//! The rest of the header: D0/A0 GPIO 0, D1/A1 GPIO 1, D2/A2 GPIO 2, D3 GPIO 21, D4/SDA GPIO 22,
//! D5/SCL GPIO 23, D6/TX GPIO 16, D7/RX GPIO 17, D8/SCK GPIO 19, D9/MISO GPIO 20, D10/MOSI
//! GPIO 18; the back pads are MTMS GPIO 4, MTDI GPIO 5, MTCK GPIO 6 and MTDO GPIO 7. `EN` is the
//! chip's reset, not a GPIO. Nothing is wired to those pins on this board, so they are left
//! alone here: only the LED, the button and the RF switch are modelled.
//!
//! The LED's polarity matters: a firmware that blinks GPIO 15 the usual way (high = on) lights
//! it here exactly as it does on the board — the pin sinks the current — and one that writes the
//! other way round sees it inverted, which is what the hardware does.
use esp_soc::BoardModel;

/// The user LED: GPIO 15, low active.
pub const PIN_LED: u8 = 15;
/// The BOOT button: GPIO 9, active low, pulled up on the board.
pub const PIN_BOOT: u8 = 9;
/// The RF switch's power enable: GPIO 3, low active (low powers the switch).
pub const PIN_RF_POWER: u8 = 3;
/// The RF switch's port select: GPIO 14, low = the onboard ceramic antenna, high = the external
/// U.FL connector.
pub const PIN_RF_ANT: u8 = 14;

/// The user LED's colour: orange (the Seeed wiki calls it the orange LED; the charging LED next
/// to the USB-C connector is red and is not on a GPIO).
pub const LED_COLOUR: [u8; 3] = [0xff, 0xa5, 0x00];

/// The Seeed Studio XIAO ESP32-C6.
pub struct XiaoEsp32C6 {
    /// the user LED: lit while GPIO 15 is driven low
    pub led_on: bool,
    /// the LED's colour in the UI: the LED's colour while it is lit, black while it is off
    pub led_colour: [[u8; 3]; 1],
    /// how often the LED changed state
    pub led_updates: u64,
    /// the RF switch's power enable (GPIO 3, low active): true while the switch is powered
    pub rf_powered: bool,
    /// the RF switch's port (GPIO 14): true = the external U.FL antenna, false = the onboard
    /// ceramic antenna
    pub rf_external: bool,
    pub gpio_events: u64,
}
impl Default for XiaoEsp32C6 { fn default() -> Self { Self::new() } }
impl XiaoEsp32C6 {
    pub fn new() -> Self {
        XiaoEsp32C6 { led_on: false, led_colour: [[0, 0, 0]], led_updates: 0, rf_powered: false, rf_external: false, gpio_events: 0 }
    }
}
impl BoardModel for XiaoEsp32C6 {
    fn name(&self) -> &'static str { "xiao-esp32c6" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) { self.gpio_events += changes.len() as u64; }
    /// The LED and the RF switch follow the drive masks, not the edges: an edge says what the
    /// pin did, the masks say what the pin is doing now (a released pin is high whatever the
    /// last edge was).
    fn gpio_output_at(&mut self, _cycle: u64, changes: &[(u8, bool)], enabled: u64, output: u64) {
        self.gpio_changes(changes);      // the edges are counted once, here
        // The LED is low active: it is lit while GPIO 15 is driven low, and off when the pin is
        // released or driven high.
        let on = enabled & (1 << PIN_LED) != 0 && output & (1 << PIN_LED) == 0;
        if on != self.led_on {
            self.led_on = on;
            self.led_updates += 1;
            self.led_colour = if on { [LED_COLOUR] } else { [[0, 0, 0]] };
        }
        // The RF switch's power enable is low active too: low powers the switch.
        self.rf_powered = enabled & (1 << PIN_RF_POWER) != 0 && output & (1 << PIN_RF_POWER) == 0;
        // GPIO 14 picks the antenna: low = the onboard ceramic, high = the external U.FL.
        self.rf_external = enabled & (1 << PIN_RF_ANT) != 0 && output & (1 << PIN_RF_ANT) != 0;
    }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    /// The LED as a one-LED strip, so the page can draw it.
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { Some((&self.led_colour, self.led_updates)) }
    /// The BOOT button: active low and pulled up, so it reads high while it is released.
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_BOOT, true)] }
    fn named_pin(&self, name: &str) -> Option<u8> { match name { "boot" | "btn" | "btn1" => Some(PIN_BOOT), _ => None } }
    fn report(&self) -> String {
        format!("[emu] xiao-esp32c6: led {} (GPIO {}, low active, {} updates); rf switch {} (GPIO {} power), antenna {} (GPIO {}); gpio events {}",
                if self.led_on { "on" } else { "off" }, PIN_LED, self.led_updates,
                if self.rf_powered { "powered" } else { "off" }, PIN_RF_POWER,
                if self.rf_external { "external U.FL" } else { "onboard ceramic" }, PIN_RF_ANT, self.gpio_events)
    }
}
