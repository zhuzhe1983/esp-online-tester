//! Boards wired for the classic ESP32, independent of S3 pin assignments.
//!
//! - `TtgoTDisplay` — the LilyGO TTGO T-Display: an ST7789 135×240 on VSPI, two buttons
//! - `NoBoard` — a bare module (`none`, `bare`, `esp32dev`): nothing on the pins, console only
//! - `esp32dev-loopback` — the SPI loopback fixture in `crate::spi`
pub mod tdisplay;
pub mod aithinker;
pub mod atom_lite;
pub mod atom_matrix;
pub mod atom_voice;

pub use tdisplay::*;

pub fn make_board(name: &str) -> Option<esp_soc::Board> {
    match name {
        "atom-matrix" | "m5stack-c008-b" => Some(Box::new(atom_matrix::AtomMatrix::new())),
        "atom-voice" | "m5stack-c008-c" => Some(Box::new(atom_voice::AtomVoice::new())),
        "atom-lite" | "m5stack-c008" => Some(Box::new(atom_lite::AtomLite::new())),
        "none" | "bare" | "esp32dev" => Some(Box::new(esp_soc::NoBoard)),
        "ttgo-tdisplay" | "tdisplay" => Some(Box::new(TtgoTDisplay::new())),
        "ai-thinker-cam" | "esp32cam" => Some(Box::new(aithinker::AiThinkerCam::default())),
        _ => crate::spi::make_board(name),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn classic_boards_reject_s3_names() {
        for name in ["none", "bare", "esp32dev", "ttgo-tdisplay", "tdisplay"] { assert!(super::make_board(name).is_some()); }
        for name in ["waveshare-amoled18-v2", "waveshare-lcd4b", "atech14", "waveshare-c6-lcd147"] { assert!(super::make_board(name).is_none()); }
    }
}
