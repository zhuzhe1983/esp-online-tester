//! Boards around the ESP32-S31. One so far: Espressif's ESP-Mosaico (board revision v1.2), wiring
//! from esp-mosaico-bsp (`esp_mosaico.h`) and the Wokwi board-esp-mosaico docs: a CO5300 480 x 480
//! AMOLED on GP-SPI2 in QSPI mode (CLK 42, CS 50, D0-D3 36/51/35/9, TE 43), its RESET on GPIO 44
//! (shared with the touch controller's), and the display supply enabled by GPIO 60 driven low.
//! Modelled: the panel and its reset and power gates, the CST9220 touch on I2C0 (0x5A, SDA 56,
//! SCL 3), and the panel's TE output on GPIO 43: a 60 Hz frame with a 1 ms high blanking pulse.
use esp_soc::devices::co5300::Co5300;
use esp_soc::{Board, BoardModel, NoBoard};

pub fn make_board(name: &str) -> Option<Board> {
    match name {
        "none" | "bare" => Some(Box::new(NoBoard)),
        "mosaico" | "esp-mosaico" => Some(Box::new(Mosaico::new())),
        _ => None,
    }
}

pub const PIN_LCD_RESET: u8 = 44;
pub const PIN_LCD_POWER_N: u8 = 60;
pub const PIN_LCD_TE: u8 = 43;
/// CO5300 TE: one frame every 1/60 s, high for the first 1 ms (the blanking interval).
const TE_PERIOD_NS: u64 = 16_666_667;
const TE_HIGH_NS: u64 = 1_000_000;

/// The ESP-Mosaico v1.2.
pub struct Mosaico {
    /// the CO5300; its frame is what the glass shows (480 x 480, no column offset in the BSP)
    pub panel: Co5300,
    /// the supply enable (GPIO 60, active low) and the reset line (GPIO 44, active low)
    pub powered: bool,
    pub in_reset: bool,
    pub gpio_events: u64,
    /// transactions the panel could not take: unpowered or held in reset
    pub dropped: u64,
    /// the finger the CST9220 reports
    pub touch: std::sync::Arc<std::sync::Mutex<crate::cst9220::Touch>>,
    /// the storage NAND on GP-SPI3 (the BSP's MOSAICO_NAND)
    pub nand: crate::spi_nand::SpiNand,
    /// TE: the level now and the edges not yet delivered (`advance_to` takes emulated nanoseconds)
    te_level: bool,
    te_edges: Vec<esp_soc::board::BoardEdge>,
}
impl Default for Mosaico { fn default() -> Self { Self::new() } }
impl Mosaico {
    pub const WIDTH: usize = 480;
    pub const HEIGHT: usize = 480;
    pub fn new() -> Self {
        Mosaico { panel: Co5300::new(Self::WIDTH, Self::HEIGHT, 0), powered: false, in_reset: false, gpio_events: 0, dropped: 0, touch: Default::default(), nand: crate::spi_nand::SpiNand::new(), te_level: false, te_edges: Vec::new() }
    }
}
impl BoardModel for Mosaico {
    fn name(&self) -> &'static str { "mosaico" }
    fn uses_gpio_edges(&self) -> bool { false }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, level) in changes {
            self.gpio_events += 1;
            match pin {
                PIN_LCD_POWER_N => { if level && self.powered { self.panel.reset(); } self.powered = !level; }
                PIN_LCD_RESET => { if !level { self.panel.reset(); } self.in_reset = !level; }
                _ => {}
            }
        }
    }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_LCD_TE, self.te_level)] }
    /// The S31 machine passes emulated time in nanoseconds. TE runs only while the panel is powered
    /// and out of reset; a jump over several frames yields the level at `now` (one edge pair at most).
    fn advance_to(&mut self, now: esp_soc::board::VirtualCycle) {
        let level = self.powered && !self.in_reset && now % TE_PERIOD_NS < TE_HIGH_NS;
        if level != self.te_level {
            self.te_level = level;
            self.te_edges.push(esp_soc::board::BoardEdge { cycle: now, pin: PIN_LCD_TE, level });
        }
    }
    fn take_edges(&mut self) -> Vec<esp_soc::board::BoardEdge> { std::mem::take(&mut self.te_edges) }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn esp_periph::i2c::I2cDevice>)> {
        vec![(0, crate::cst9220::ADDRESS, Box::new(crate::cst9220::Cst9220::new(self.touch.clone(), Self::WIDTH as u16, Self::HEIGHT as u16)))]
    }
    fn touch(&mut self, x: u16, y: u16, down: bool) { *self.touch.lock().unwrap() = crate::cst9220::Touch { down, x, y }; }
    fn spi_transfer(&mut self, host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if host == 3 { return self.nand.transaction(tx, rx_len); }
        if host == 2 {
            if self.powered && !self.in_reset { self.panel.transaction(tx); } else { self.dropped += 1; }
        }
        vec![0xff; rx_len]
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        Some((Self::WIDTH as u32, Self::HEIGHT as u32, self.panel.frame.clone(), self.panel.pixels_written))
    }
    fn display_version(&self) -> u64 { self.panel.pixels_written }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn report(&self) -> String {
        let n = self.nand.stats;
        format!("[emu] mosaico: CO5300 {} RAMWR, {} pixels; power {}, reset {}; {} transactions dropped; gpio events {}\n[emu] mosaico: NAND {} page reads, {} programs, {} erases, {} refused",
                self.panel.frames, self.panel.pixels_written, if self.powered { "on" } else { "off" },
                if self.in_reset { "held" } else { "released" }, self.dropped, self.gpio_events, n.reads, n.programs, n.erases, n.refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panel_takes_commands_only_when_powered_and_out_of_reset() {
        let mut b = Mosaico::new();
        let ramwr = [0x32, 0, 0x2c, 0, 0x12, 0x34];
        b.spi_transfer(2, &ramwr, 0);
        assert_eq!(b.panel.pixels_written, 0);
        assert_eq!(b.dropped, 1);
        b.gpio_changes(&[(PIN_LCD_POWER_N, false), (PIN_LCD_RESET, false)]);
        b.spi_transfer(2, &ramwr, 0);
        assert_eq!(b.panel.pixels_written, 0, "held in reset");
        b.gpio_changes(&[(PIN_LCD_RESET, true)]);
        b.spi_transfer(2, &ramwr, 0);
        assert_eq!(b.panel.frame[0], 0x1234);
    }
}
