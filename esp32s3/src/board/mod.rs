//! Boards around the ESP32-S3. The SoC model emits generic events (GPIO edges, RMT symbol
//! streams, SPI bytes, LCD_CAM frames, I2S samples); a `BoardModel` interprets them as the
//! devices wired to the pins. One file per board; the device models they share (DCS panels,
//! WS2812 chains, bit-banged SPI) live in `esp_soc::devices`.
//!
//! - `Atech14` — the Atech 14-port motherboard with the Knob, Light Grid and TFT modules
//! - `WaveshareCam` — ESP32-S3-CAM-OV5640
//! - `WaveshareLcd4b` — ESP32-S3-Touch-LCD-4B
//! - `WaveshareAmoled18V2` — ESP32-S3-Touch-AMOLED-1.8 V2
//! - `StopWatch` — M5Stack StopWatch
//! - `NoBoard` — a bare module: nothing on the pins (any ESP32-S3 firmware, console only)
pub use esp_soc::board::{Board, BoardEdge, BoardModel, NoBoard, VirtualCycle};

pub mod atech14;
pub mod atoms3;
pub mod stopwatch;
pub mod papermono;
pub mod cardputer_adv;
pub mod cardputer;
mod cardputer_lcd;
pub mod waveshare_amoled18;
pub mod waveshare_cam;
pub mod waveshare_lcd4b;

pub use atech14::*;
pub use atoms3::*;
pub use stopwatch::*;
pub use papermono::*;
pub use cardputer_adv::*;
pub use cardputer::*;
pub use waveshare_amoled18::*;
pub use waveshare_cam::*;
pub use waveshare_lcd4b::*;

/// Board by name: `atech14` (default), `none`, `stopwatch` or a supported Waveshare board.
pub fn make_board(name: &str) -> Option<Board> {
    match name {
        "atech14" | "atech" => Some(Box::new(Atech14::new())),
        "none" | "bare" => Some(Box::new(NoBoard)),
        "atoms3" | "m5stack-atoms3" => Some(Box::new(AtomS3::new())),
        "papermono" | "m5stack-papermono" => Some(Box::new(PaperMono::new())),
        "cardputer" | "m5stack-cardputer" => Some(Box::new(Cardputer::new())),
        "cardputer-adv" | "m5stack-cardputer-adv" => Some(Box::new(CardputerAdv::new())),
        "stopwatch" | "m5stack-stopwatch" => Some(Box::new(StopWatch::new())),
        "waveshare-cam" | "waveshare" => Some(Box::new(WaveshareCam::new())),
        "waveshare-lcd4b" | "lcd4b" => Some(Box::new(WaveshareLcd4b::new())),
        "waveshare-amoled18-v2" | "amoled18-v2" => Some(Box::new(WaveshareAmoled18V2::new())),
        _ => None,
    }
}
