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
pub mod xiao;

pub use waveshare_lcd147::{
    lcd_visible, WaveshareC6Lcd147, LCD_COL_OFFSET, LCD_VISIBLE_COLS, PIN_BOOT, PIN_LCD_BL, PIN_LCD_CS, PIN_LCD_DC,
    PIN_LCD_RST, PIN_LED,
};
pub use xiao::XiaoEsp32C6;

/// Board by name: `none` (a bare module), `waveshare-c6-lcd147` or `xiao-esp32c6`.
pub fn make_board(name: &str) -> Option<Board> {
    match name {
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
