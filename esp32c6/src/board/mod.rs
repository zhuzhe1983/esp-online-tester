//! Boards around the ESP32-C6. The SoC model emits generic events (GPIO edges, RMT symbol
//! streams, SPI bytes); a `BoardModel` interprets them as the devices wired to the pins. One file
//! per board.
//!
//! - `WaveshareC6Lcd147` — the Waveshare ESP32-C6-LCD-1.47: an ST7789 172×320 panel on SPI2, a
//!   WS2812 on GPIO 8 (RMT), a BOOT button on GPIO 9 and a TF card slot on the same SPI bus
//!   (not modelled)
//! - `XiaoEsp32C6` — the Seeed Studio XIAO ESP32-C6: a low-active user LED on GPIO 15, the BOOT
//!   button on GPIO 9 and the RF switch on GPIO 3 and GPIO 14
//! - `NoBoard` — a bare module: nothing on the pins (any ESP32-C6 firmware, console only)
pub use esp_soc::board::{Board, BoardEdge, BoardModel, NoBoard, VirtualCycle};

pub mod waveshare_lcd147;
pub mod waveshare_lcd_family;
pub mod waveshare_touch28;
pub mod waveshare_lcd273;
pub mod waveshare_epaper154;
pub mod waveshare_amoled;
pub mod waveshare_amoled_compact;
pub mod xiao;
pub mod nanoc6;
pub mod nesso_n1;
pub mod stamp_c6lora;
pub mod lilygo;
pub mod lilygo_tqt;
pub mod lilygo_additional;

pub use waveshare_lcd147::{
    lcd_visible, WaveshareC6Lcd147, LCD_COL_OFFSET, LCD_VISIBLE_COLS, PIN_BOOT, PIN_LCD_BL, PIN_LCD_CS, PIN_LCD_DC,
    PIN_LCD_RST, PIN_LED,
};
pub use xiao::XiaoEsp32C6;

/// Board by name: `none` (a bare module), `waveshare-c6-lcd147` or `xiao-esp32c6`.
pub fn make_board(name: &str) -> Option<Board> {
    if let Some(variant)=waveshare_lcd_family::LcdVariant::from_name(name) { return Some(Box::new(waveshare_lcd_family::WaveshareLcd::new(variant))); }
    match name {
        "waveshare-c6-touch-amoled18" => Some(Box::new(waveshare_amoled_compact::CompactAmoled::new(waveshare_amoled_compact::Variant::Touch18V2))),
        "waveshare-c6-touch-amoled18-v1" => Some(Box::new(waveshare_amoled_compact::CompactAmoled::new(waveshare_amoled_compact::Variant::Touch18V1))),
        "waveshare-c6-touch-amoled132" => Some(Box::new(waveshare_amoled_compact::CompactAmoled::new(waveshare_amoled_compact::Variant::Touch132))),
        "waveshare-c6-lcd273" => Some(Box::new(waveshare_lcd273::Lcd273::new())),
        "waveshare-c6-epaper154" => Some(Box::new(waveshare_epaper154::Epaper154::new())),
        "waveshare-c6-touch-amoled216" => Some(Box::new(waveshare_amoled::WaveshareAmoled::new(waveshare_amoled::AmoledVariant::Touch216))),
        "waveshare-c6-touch-amoled164" => Some(Box::new(waveshare_amoled::WaveshareAmoled::new(waveshare_amoled::AmoledVariant::Touch164))),
        "waveshare-c6-touch28" => Some(Box::new(waveshare_touch28::Touch28::new())),
        "lilygo-t7-c6" => Some(Box::new(lilygo::LilygoC6::t7_c6())),
        "lilygo-t-knob" => Some(Box::new(lilygo::LilygoC6::t_knob())),
        "lilygo-t-qt-c6" => Some(Box::new(lilygo_tqt::TQtC6::new())),
        "lilygo-t-lora-c6" => Some(Box::new(lilygo_additional::TLoRaC6::new())),
        "lilygo-t-pico-c6" => Some(Box::new(lilygo_additional::TPicoC6::new())),
        "stamp-c6lora" | "m5stack-s012" => Some(Box::new(stamp_c6lora::StampC6LoRa::new())),
        "nesso-n1" | "m5stack-dk001" => Some(Box::new(nesso_n1::NessoN1::new())),
        "nanoc6" | "m5stack-c125" => Some(Box::new(nanoc6::NanoC6::new())),
        "none" | "bare" => Some(Box::new(NoBoard)),
        "waveshare-c6-lcd147" | "c6-lcd147" | "lcd147" => Some(Box::new(WaveshareC6Lcd147::new())),
        "xiao-esp32c6" | "xiao-c6" | "xiao" => Some(Box::new(XiaoEsp32C6::new())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_c6_boards_are_named_and_the_others_are_not() {
        for name in ["none", "bare", "waveshare-c6-lcd147", "c6-lcd147", "lcd147", "xiao-esp32c6", "xiao-c6", "xiao"] {
            assert!(super::make_board(name).is_some(), "{name}");
        }
        for name in ["atech14", "waveshare-lcd4b", "ttgo-tdisplay", "waveshare-cam"] {
            assert!(super::make_board(name).is_none(), "{name}");
        }
    }
}

#[cfg(test)]
mod waveshare_new_lcd_tests;
