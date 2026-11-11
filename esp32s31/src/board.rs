//! Boards around the ESP32-S31. One so far: Espressif's ESP-Mosaico (board revision v1.2), wiring
//! from esp-mosaico-bsp (`esp_mosaico.h`) and the Wokwi board-esp-mosaico docs: a CO5300 480 x 480
//! AMOLED on GP-SPI2 in QSPI mode (CLK 42, CS 50, D0-D3 36/51/35/9, TE 43), its RESET on GPIO 44
//! (shared with the touch controller's), and the display supply enabled by GPIO 60 driven low.
//! Modelled: the panel and its reset and power gates. Not yet: TE, the CST9220 touch (I2C 0x5A).
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
}
impl Default for Mosaico { fn default() -> Self { Self::new() } }
impl Mosaico {
    pub const WIDTH: usize = 480;
    pub const HEIGHT: usize = 480;
    pub fn new() -> Self {
        Mosaico { panel: Co5300::new(Self::WIDTH, Self::HEIGHT, 0), powered: false, in_reset: false, gpio_events: 0, dropped: 0 }
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
    fn spi_transfer(&mut self, host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
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
        format!("[emu] mosaico: CO5300 {} RAMWR, {} pixels; power {}, reset {}; {} transactions dropped; gpio events {}",
                self.panel.frames, self.panel.pixels_written, if self.powered { "on" } else { "off" },
                if self.in_reset { "held" } else { "released" }, self.dropped, self.gpio_events)
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
