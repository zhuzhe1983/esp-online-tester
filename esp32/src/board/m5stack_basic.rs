//! M5Stack Basic K001 and K001-V27, documented IPS / 16 MiB revisions.
//! M5Stack Basic pin maps and M5 Core schematic (2017-12-06, sheets 3–5):
//! ILI9342C CLK18, SDA23, CS14, D/C27, RST33, BL32; A/B/C pull GPIO39/38/37 low.
//! ILI9342C V102 §8.2 supplies the display command and address-mode definitions.
use esp_periph::sdmmc::{SdCardConfig, SdStore, card::spi::SpiSdCard};
use esp_soc::board::{BoardModel, SpiPins, VirtualCycle};
use esp_soc::devices::DcsPanel;

const WIDTH: usize = 320;
const HEIGHT: usize = 240;
const LCD_CLK: u8 = 18;
const LCD_DATA: u8 = 23;
const LCD_CS: u8 = 14;
const LCD_DC: u8 = 27;
const LCD_RST: u8 = 33;
const LCD_BL: u8 = 32;
const SD_CS: u8 = 4;
const SD_MISO: u8 = 19;
const BUTTONS: [(&str, u8); 3] = [("button-a", 39), ("button-b", 38), ("button-c", 37)];

pub struct M5StackBasic {
    panel: DcsPanel,
    reset: bool,
    backlight: bool,
    pwm_backlight: Option<bool>,
    display_changes: u64,
    sd: Option<Box<SpiSdCard>>,
}

impl Default for M5StackBasic { fn default() -> Self { Self::new() } }
impl M5StackBasic {
    pub fn new() -> Self {
        Self { panel: DcsPanel::new(WIDTH, HEIGHT), reset: false, backlight: false, pwm_backlight: None, display_changes: 0, sd: None }
    }

    fn lit(&self) -> bool { self.pwm_backlight.unwrap_or(self.backlight) }
}

impl BoardModel for M5StackBasic {
    fn name(&self) -> &'static str { "m5stack-basic" }

    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, high) in changes {
            match pin {
                LCD_DC => self.panel.dc = high,
                LCD_RST => {
                    if !high && !self.reset { self.panel.reset(); }
                    self.reset = !high;
                }
                LCD_BL => {
                    let previous = self.lit();
                    self.backlight = high;
                    if self.lit() != previous { self.display_changes = self.display_changes.wrapping_add(1); }
                }
                SD_CS if high => { if let Some(sd) = &mut self.sd { sd.deselect(); } }
                _ => {}
            }
        }
    }

    fn uses_spi_pins(&self) -> bool { true }
    fn spi_transfer_pins(&mut self, _host: u8, pins: SpiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        // The TFT and card share SCLK/MOSI. Only the card is wired to the VSPI MISO pad.
        if pins.sclk & (1 << LCD_CLK) == 0 || pins.mosi & (1 << LCD_DATA) == 0 { return vec![0xff; rx_len]; }
        if !self.reset && pins.cs & (1 << LCD_CS) != 0 {
            let previous = (self.panel.sleeping, self.panel.on, self.panel.madctl);
            for &byte in tx { self.panel.byte(byte); }
            if (self.panel.sleeping, self.panel.on, self.panel.madctl) != previous {
                self.display_changes = self.display_changes.wrapping_add(1);
            }
        }
        if let Some(sd) = &mut self.sd {
            let selected = pins.cs & (1 << SD_CS) != 0;
            let incoming = sd.transfer(selected, tx, rx_len);
            if selected && pins.miso == Some(SD_MISO) { return incoming; }
        }
        // The factory display write path needs no register or GRAM readback.
        vec![0xff; rx_len]
    }

    fn attach_spi_sd_card(&mut self, image: Box<dyn SdStore>, config: SdCardConfig) -> Result<String, String> {
        let sd = SpiSdCard::new(image, config)?;
        let description = sd.describe();
        self.sd = Some(Box::new(sd));
        Ok(description)
    }

    fn uses_gpio_pwm(&self) -> bool { true }
    fn gpio_pwm_at(&mut self, _cycle: VirtualCycle, pwm: &dyn Fn(u8) -> Option<(f64, u32)>) {
        let previous = self.lit();
        self.pwm_backlight = pwm(LCD_BL).map(|(_, duty)| duty != 0);
        if self.lit() != previous { self.display_changes = self.display_changes.wrapping_add(1); }
    }

    fn input_levels(&self) -> Vec<(u8, bool)> { BUTTONS.map(|(_, pin)| (pin, true)).to_vec() }
    fn named_pin(&self, name: &str) -> Option<u8> {
        BUTTONS.iter().find_map(|&(id, pin)| (id == name).then_some(pin))
    }

    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let pixels = if !self.reset && self.lit() && self.panel.on && !self.panel.sleeping {
            self.panel.gram.clone()
        } else {
            vec![0; WIDTH * HEIGHT]
        };
        Some((WIDTH as u32, HEIGHT as u32, pixels, self.display_version()))
    }
    fn display_version(&self) -> u64 {
        self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.display_changes)
    }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { Some((self.panel.gram.clone(), WIDTH, HEIGHT)) }
    fn report(&self) -> String {
        let mut report = format!("[emu] m5stack-basic: {} RAMWR, {} pixels, madctl={:#04x}, colmod={:#04x}, inverted={}, sleep={}, on={}, backlight={}",
                self.panel.frames, self.panel.pixels_written, self.panel.madctl, self.panel.colmod,
                self.panel.inverted, self.panel.sleeping, self.panel.on, self.lit());
        if let Some(sd) = &self.sd { report += &format!("; SD SPI {}", sd.report()); }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTE: SpiPins = SpiPins { sclk: 1 << LCD_CLK, mosi: 1 << LCD_DATA, miso: None, cs: 1 << LCD_CS, keep_cs: false };

    fn command(board: &mut M5StackBasic, cmd: u8, data: &[u8]) {
        board.gpio_changes(&[(LCD_DC, false)]);
        board.spi_transfer_pins(3, ROUTE, &[cmd], 0);
        board.gpio_changes(&[(LCD_DC, true)]);
        board.spi_transfer_pins(3, ROUTE, data, 0);
    }

    #[test]
    fn selected_panel_requires_the_correct_clock_and_data_pads() {
        let mut board = M5StackBasic::new();
        command(&mut board, 0x11, &[]);
        command(&mut board, 0x29, &[]);
        command(&mut board, 0x3a, &[0x55]);
        command(&mut board, 0x2a, &[0, 1, 0, 2]);
        command(&mut board, 0x2b, &[0, 1, 0, 1]);
        command(&mut board, 0x2c, &[0x07, 0xe0, 0xf8, 0x00]);
        board.spi_transfer_pins(3, SpiPins { cs: 1 << 4, ..ROUTE }, &[0, 0], 0);
        board.spi_transfer_pins(3, SpiPins { sclk: 1 << 19, ..ROUTE }, &[0, 0], 0);
        board.spi_transfer_pins(3, SpiPins { mosi: 1 << 22, ..ROUTE }, &[0, 0], 0);
        assert_eq!(board.panel.pixels_written, 2);
        assert_eq!(board.display().unwrap().2[WIDTH + 1], 0);
        board.gpio_changes(&[(LCD_BL, true)]);
        let (width, height, pixels, lit) = board.display().unwrap();
        assert_eq!((width, height), (320, 240));
        assert_eq!(&pixels[WIDTH + 1..=WIDTH + 2], &[0x07e0, 0xf800]);
        board.gpio_pwm_at(0, &|pin| (pin == LCD_BL).then_some((5000.0, 0)));
        let (_, _, pixels, dark) = board.display().unwrap();
        assert_ne!(lit, dark);
        assert_eq!(pixels[WIDTH + 1], 0);
        board.gpio_pwm_at(0, &|_| None);
        board.gpio_changes(&[(LCD_RST, false)]);
        command(&mut board, 0x11, &[]);
        command(&mut board, 0x29, &[]);
        assert_eq!(board.display().unwrap().2[WIDTH + 1], 0);
        board.gpio_changes(&[(LCD_RST, true)]);
        command(&mut board, 0x11, &[]);
        command(&mut board, 0x29, &[]);
        assert_eq!(board.display().unwrap().2[WIDTH + 1], 0x07e0);
    }

    #[test]
    fn physical_keys_use_external_pullups_and_side_power_is_separate() {
        let board = M5StackBasic::new();
        assert_eq!(board.input_levels(), [(39, true), (38, true), (37, true)]);
        for (id, pin) in [("button-a", 39), ("button-b", 38), ("button-c", 37)] {
            assert_eq!(board.named_pin(id), Some(pin));
        }
        assert_eq!(board.named_pin("power"), None);
    }

    #[test]
    fn tf_card_requires_the_socket_routes_and_is_independent_from_lcd() {
        use esp_periph::sdmmc::MemStore;
        let mut board = M5StackBasic::new();
        let card = SpiPins { cs: 1 << SD_CS, miso: Some(SD_MISO), ..ROUTE };
        let cmd0 = [0x40, 0, 0, 0, 0, 0x95];
        assert_eq!(board.spi_transfer_pins(3, card, &cmd0, 6), [0xff; 6]);
        board.attach_spi_sd_card(Box::new(MemStore(vec![0; 1 << 20])), Default::default()).unwrap();
        board.spi_transfer_pins(3, SpiPins { cs: 0, ..card }, &[0xff; 10], 0);
        board.spi_transfer_pins(3, SpiPins { sclk: 1 << 19, ..card }, &cmd0, 0);
        assert_eq!(board.spi_transfer_pins(3, card, &[], 2), [0xff, 0xff]);
        board.spi_transfer_pins(3, card, &cmd0, 0);
        assert_eq!(board.spi_transfer_pins(3, card, &[], 2), [0xff, 1]);
        board.spi_transfer_pins(3, card, &cmd0, 0);
        assert_eq!(board.spi_transfer_pins(3, SpiPins { miso: Some(23), ..card }, &[], 2), [0xff, 0xff]);
        board.spi_transfer_pins(3, card, &cmd0[..3], 0);
        board.gpio_changes(&[(SD_CS, true)]);
        board.spi_transfer_pins(3, card, &cmd0[3..], 0);
        assert_eq!(board.spi_transfer_pins(3, card, &[], 2), [0xff, 0xff]);
        assert_eq!(board.panel.pixels_written, 0);
    }
}
