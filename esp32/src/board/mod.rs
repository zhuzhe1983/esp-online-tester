//! Boards wired for the classic ESP32, independent of S3 pin assignments.
//!
//! - `TtgoTDisplay` — the LilyGO TTGO T-Display: an ST7789 135×240 on VSPI, two buttons
//! - `UlanziTc001` — the Ulanzi TC001 pixel clock: a 32×8 WS2812 matrix on RMT, three buttons
//! - `NoBoard` — a bare module (`none`, `bare`, `esp32dev`): nothing on the pins, console only
//! - `esp32dev-loopback` — the SPI loopback fixture in `crate::spi`
pub mod tdisplay;
pub mod aithinker;
pub mod ulanzi;

pub use tdisplay::*;

pub fn make_board(name: &str) -> Option<esp_soc::Board> {
    match name {
        "none" | "bare" | "esp32dev" => Some(Box::new(esp_soc::NoBoard)),
        "ttgo-tdisplay" | "tdisplay" => Some(Box::new(TtgoTDisplay::new())),
        "ai-thinker-cam" | "esp32cam" => Some(Box::new(aithinker::AiThinkerCam::default())),
        "ulanzi-tc001" | "tc001" => Some(Box::new(ulanzi::UlanziTc001::new())),
        _ => crate::spi::make_board(name),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn classic_boards_reject_s3_names() {
        for name in ["none", "bare", "esp32dev", "ttgo-tdisplay", "tdisplay", "ulanzi-tc001", "tc001"] { assert!(super::make_board(name).is_some()); }
        for name in ["waveshare-amoled18-v2", "waveshare-lcd4b", "atech14", "waveshare-c6-lcd147"] { assert!(super::make_board(name).is_none()); }
    }
}
