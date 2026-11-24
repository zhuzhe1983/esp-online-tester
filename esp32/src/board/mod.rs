//! Boards wired for the classic ESP32, independent of S3 pin assignments.
//!
//! - `TtgoTDisplay` — the LilyGO TTGO T-Display: an ST7789 135×240 on VSPI, two buttons
//! - `UlanziTc001` — the Ulanzi TC001 pixel clock: a 32×8 WS2812 matrix on RMT, three buttons
//! - `NoBoard` — a bare module (`none`, `bare`, `esp32dev`): nothing on the pins, console only
//! - `esp32dev-loopback` — the SPI loopback fixture in `crate::spi`
pub mod tdisplay;
pub mod tbeam;
pub mod aithinker;
pub mod atom_lite;
pub mod atom_matrix;
pub mod atom_voice;
pub mod atom_matrix_v11;
pub mod stickt2;
pub mod stamppico;
pub mod ulanzi;
pub mod cyd;
pub mod lilygo_additional;
mod m5stack_basic;
pub mod core2;
pub mod m5_tough;
pub mod stickc;
pub mod stickc_plus;
pub mod waveshare_touch35;
pub mod waveshare_servo;
pub mod waveshare_devkit;
pub mod waveshare_robot;
pub mod waveshare_epaper_driver;

pub use tdisplay::*;

/// Source-documented capacities only. Robot/servo boards intentionally return
/// None: their 4 MiB diagnostic defaults do not identify fitted Flash silicon.
pub fn documented_memory_mib(name: &str) -> Option<(usize,usize)> {
    waveshare_devkit::memory_mib(name).or(match name {
        waveshare_touch35::NAME => Some(waveshare_touch35::MEMORY_MIB),
        waveshare_epaper_driver::NAME => Some(waveshare_epaper_driver::MEMORY_MIB),
        _ => None,
    })
}

pub fn make_board(name: &str) -> Option<esp_soc::Board> {
    match name {
        "waveshare-epaper-esp32-driver-v3" => Some(Box::new(waveshare_epaper_driver::EpaperDriver::new())),
        "waveshare-general-driver-robots" => Some(Box::new(waveshare_robot::RobotDriver::new())),
        "waveshare-servo-driver-esp32" => Some(Box::new(waveshare_servo::ServoDriver::new())),
        "waveshare-esp32-touch35" => Some(Box::new(waveshare_touch35::WaveshareTouch35::new())),
        "stickc-plus" | "m5stack-k016-p" => Some(Box::new(stickc_plus::StickCPlus::new())),
        "core2" => Some(Box::new(core2::Core2::new())),
        "core2-v11" => Some(Box::new(core2::Core2::with_revision(core2::Revision::V11))),
        "core2-v13" => Some(Box::new(core2::Core2::with_revision(core2::Revision::V13))),
        "core2-aws" => Some(Box::new(core2::Core2::with_revision(core2::Revision::Aws))),
        "core2-aws-v13" => Some(Box::new(core2::Core2::with_revision(core2::Revision::AwsV13))),
        "stickt2" | "m5stack-k016-t2" => Some(Box::new(stickt2::StickT2::new())),
        "stamppico" => Some(Box::new(stamppico::StampPico::new())),
        "lilygo-t-beam-v1-2" => Some(Box::new(tbeam::TBeam::new())),
        "atom-matrix-v11" | "m5stack-c008-b-v11" => Some(Box::new(atom_matrix_v11::AtomMatrixV11::new())),
        "atom-matrix" | "m5stack-c008-b" => Some(Box::new(atom_matrix::AtomMatrix::new())),
        "atom-voice" | "m5stack-c008-c" => Some(Box::new(atom_voice::AtomVoice::new())),
        "atom-lite" | "m5stack-c008" => Some(Box::new(atom_lite::AtomLite::new())),
        "none" | "bare" | "esp32dev" => Some(Box::new(esp_soc::NoBoard)),
        "ttgo-tdisplay" | "tdisplay" => Some(Box::new(TtgoTDisplay::new())),
        "ai-thinker-cam" | "esp32cam" => Some(Box::new(aithinker::AiThinkerCam::default())),
        "m5-tough" => Some(Box::new(m5_tough::M5Tough::new())),
        "cyd" | "esp32-2432s028" => Some(Box::new(cyd::Cyd::new())),
        "stickc" => Some(Box::new(stickc::StickC::new())),
        "m5stack-basic" => Some(Box::new(m5stack_basic::M5StackBasic::new())),
        "ulanzi-tc001" | "tc001" => Some(Box::new(ulanzi::UlanziTc001::new())),
        _ => waveshare_devkit::make_board(name).or_else(|| lilygo_additional::make_board(name)).or_else(|| crate::spi::make_board(name)),
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
