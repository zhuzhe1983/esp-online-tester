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
pub mod tdisplay_s3;
pub mod airquality;
pub mod atoms3;
pub mod atoms3_lite;
pub mod atoms3r;
pub mod atom_voices3r;
pub mod dial;
pub mod dinmeter;
mod stamps3;
pub mod stopwatch;
pub mod papermono;
pub mod paperdiy;
pub mod papercolor;
mod papers3;
pub mod metalio_ink4;
pub mod cardputer_adv;
pub mod cardputer;
pub mod tdeck;
mod tdeck_keyboard;
mod cardputer_lcd;
pub mod waveshare_amoled18;
pub mod waveshare_cam;
pub mod waveshare_lcd4b;

pub use atech14::*;
pub use atoms3::*;
pub use atoms3_lite::*;
pub use atoms3r::*;
pub use dial::*;
pub use dinmeter::*;
pub use stopwatch::*;
pub use papermono::*;
pub use paperdiy::*;
pub use papercolor::*;
pub use metalio_ink4::*;
pub use cardputer_adv::*;
pub use cardputer::*;
pub use waveshare_amoled18::*;
pub use waveshare_cam::*;
pub use waveshare_lcd4b::*;

/// Board by name: `atech14` (default), `none`, `stopwatch` or a supported Waveshare board.
pub fn make_board(name: &str) -> Option<Board> {
    match name {
        "airquality" | "m5stack-k131" => Some(Box::new(airquality::AirQuality::new())),
        "airquality-v11" | "m5stack-k131-v11" => Some(Box::new(airquality::AirQuality::v11())),
        "tdisplay-s3" | "lilygo-t-display-s3" => Some(Box::new(tdisplay_s3::TDisplayS3::new())),
        "atech14" | "atech" => Some(Box::new(Atech14::new())),
        "none" | "bare" => Some(Box::new(NoBoard)),
        "atom-voices3r" | "m5stack-c126-echo" => Some(Box::new(atom_voices3r::AtomVoiceS3R::new())),
        "atoms3r-chatbot" | "m5stack-k147" => Some(Box::new(AtomS3R::chatbot())),
        "atoms3r" | "m5stack-atoms3r" => Some(Box::new(AtomS3R::new())),
        "atoms3" | "m5stack-atoms3" => Some(Box::new(AtomS3::new())),
        "dial" | "m5stack-dial" => Some(Box::new(Dial::new())),
        "stamps3" | "m5stack-s007" => Some(Box::new(stamps3::StampS3::new())),
        "stamps3a" | "m5stack-s007-v033" => Some(Box::new(stamps3::StampS3::s3a())),
        "dinmeter" | "m5stack-dinmeter" => Some(Box::new(DinMeter::new())),
        "dinmeter-v11" | "m5stack-k134-v11" => Some(Box::new(DinMeter::v11())),
        "dial-v11" | "m5stack-dial-v11" => Some(Box::new(Dial::v11())),
        "atoms3-lite" | "m5stack-atoms3-lite" => Some(Box::new(AtomS3Lite::new())),
        "papermono" | "m5stack-papermono" => Some(Box::new(PaperMono::new())),
        "papercolor" | "m5stack-c151" => Some(Box::new(PaperColor::new())),
        "papermono-lite" | "m5stack-c153-lite" => Some(Box::new(PaperMono::lite())),
        "paperdiy" | "m5stack-k159-b" => Some(Box::new(PaperDiy::new())),
        "papers3" | "m5stack-c139" => Some(Box::new(papers3::PaperS3::new())),
        "metalio-ink4" => Some(Box::new(MetalioInk4::new())),
        "tdeck" | "lilygo-t-deck" => Some(Box::new(tdeck::TDeck::new())),
        "cardputer" | "m5stack-cardputer" => Some(Box::new(Cardputer::new())),
        "cardputer-adv" | "m5stack-cardputer-adv" => Some(Box::new(CardputerAdv::new())),
        "stopwatch" | "m5stack-stopwatch" => Some(Box::new(StopWatch::new())),
        "waveshare-cam" | "waveshare" => Some(Box::new(WaveshareCam::new())),
        "waveshare-lcd4b" | "lcd4b" => Some(Box::new(WaveshareLcd4b::new())),
        "waveshare-amoled18-v2" | "amoled18-v2" => Some(Box::new(WaveshareAmoled18V2::new())),
        _ => None,
    }
}

/// Cold board identity setup: ESP-IDF v5.5.1 S3 efuse_reg.h RD_MAC_SPI_SYS_3
/// offset 0x50, PKG_VERSION bits 23:21; M5GFX 65c816a3 identifies PICO as 1.
/// v4.4.8 defines the same offset/field. No changes for other products.
pub fn configure_chip_identity(board:&str,efuse:&mut esp_periph::efuse::Efuse) {
    if matches!(board,"atoms3r"|"atom-voices3r"|"atoms3r-chatbot") {let r=efuse.ram.read(0x50);efuse.ram.write(0x50,(r&!(7<<21))|(1<<21));}
}
