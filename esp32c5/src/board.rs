//! Stamp-C5 S016/S016-DIP: official shared schematic (URL V0.3, title V0.2).
//! ESP32-C5HF4, 4 MiB flash. Blue D1 is VSYS_3V3 -> LED/R1 -> GPIO28.
//! The red nCHG lamp is charger-controlled; there is no RGB or user button here.
use esp_soc::{BoardModel, pins::PinRoutes};
pub struct StampC5 { dip: bool, leds: [[u8; 3]; 1], updates: u64 }
impl StampC5 {
    // Generation 1 publishes the board's real initial off state to observers whose
    // output cursor starts at 0. Later generations represent actual state changes.
    pub fn new(dip: bool) -> Self { Self { dip, leds: [[0; 3]], updates: 1 } }
}
pub fn make_board(name: &str) -> Option<esp_soc::Board> {
    match name {
        "stamp-c5" | "m5stack-s016" => Some(Box::new(StampC5::new(false))),
        "stamp-c5-dip" | "m5stack-s016-dip" => Some(Box::new(StampC5::new(true))),
        "none" | "bare" => Some(Box::new(esp_soc::NoBoard)),
        _ => None,
    }
}
impl BoardModel for StampC5 {
    fn name(&self) -> &'static str { if self.dip { "stamp-c5-dip" } else { "stamp-c5" } }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_waveform_at(&mut self, _cycle: u64, routes: PinRoutes<'_>) {
        let color = if routes.software_output(28) == Some(false) { [0, 0, 255] } else { [0; 3] };
        if color != self.leds[0] { self.leds[0] = color; self.updates = self.updates.wrapping_add(1); }
    }
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { Some((&self.leds, self.updates)) }
    fn report(&self) -> String { format!("[{}] blue={:?} generation={}; charger/RF/analog unimplemented", self.name(), self.leds[0], self.updates) }
}
