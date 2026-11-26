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
mod lilygo_display;
pub mod cloudzao_claw154;
mod lilygo_epaper;
pub mod lilygo_additional;
pub mod lilygo_io;
pub mod airquality;
pub mod atoms3;
pub mod atoms3_lite;
pub mod atoms3r;
pub mod atoms3r_cam;
mod atoms3u;
mod capsule;
pub mod atom_voices3r;
mod vameter;
mod xiaozhi_card;
mod powerhub;
pub mod dial;
pub mod stackchan;
mod stackchan_host;
mod stackchan_base;
mod cores3;
pub mod dinmeter;
mod chain_captain;
mod chain_dualkey;
pub mod seeed_xiao_ips;
pub mod seeed_ee;
pub mod seeed_sensecap;
pub mod seeed_indicator;
pub mod seeed_indicator_lora;
mod tembed_cc1101;
mod stamps3;
mod stamps3bat;
mod stamps3bat_power;
mod stamplc;
mod stamplc_io;
mod stampfly;
mod sticks3;
pub mod stopwatch;
pub mod papermono;
pub mod paperdiy;
pub mod papercolor;
mod papers3;
pub mod metalio_ink4;
pub mod seeed_reterminal;
pub mod cardputer_adv;
pub mod cardputer;
pub mod tdeck;
mod tdeck_keyboard;
mod cardputer_lcd;
mod unit_pir;
pub mod waveshare_amoled18;
pub mod waveshare_amoled_watch;
pub mod waveshare_cam;
pub mod unit_cams3;
pub mod waveshare_co5300_catalog;
mod waveshare_rgb_catalog;
mod waveshare_epaper_catalog;
mod waveshare_rlcd;
mod waveshare_lcd349;
mod waveshare_amoled132;
mod waveshare_dualeye;
mod waveshare_lcd7b;
mod waveshare_serial_rgb;
mod waveshare_knob18;
mod waveshare_lcd2;
mod waveshare_lcd147_plain;
mod waveshare_amoled191;
mod waveshare_round_tca;
mod waveshare_amoled_compact;
mod waveshare_small_spi;
mod waveshare_lcd147;
mod waveshare_lcd169;
mod waveshare_lcd35b;
mod waveshare_lcd35;
mod waveshare_amoled164;
mod waveshare_amoled241;
pub mod waveshare_core;
pub mod waveshare_industrial;
pub mod waveshare_industrial_controls;
pub mod waveshare_extended;
pub mod waveshare_rgb_matrix;
pub mod waveshare_thermal;
mod waveshare_tail_spi;
mod waveshare_s3_geek;
mod waveshare_s3_cellular;
mod waveshare_touch4_v4;
mod waveshare_matrix8;
mod waveshare_ch422_rgb;
pub mod waveshare_epaper133;
pub mod waveshare_qspi;
pub mod waveshare_lcd4b;
pub mod waveshare_lr;
pub mod waveshare_lcd185b;
mod waveshare_lcd185c;
pub mod waveshare_spi_legacy;
pub mod waveshare_spi_catalog;
pub mod waveshare_tca_rgb;

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
        "waveshare-s3-lcd169-v1" | "waveshare-s3-lcd169-v2" => waveshare_lcd169::make_board(name),
        "waveshare-s3-thermal-camera" => Some(Box::<waveshare_thermal::WaveshareThermal>::default()),
        "waveshare-s3-touch-amoled143-sh8601" => Some(Box::new(waveshare_amoled_compact::WaveshareCompact::new(waveshare_amoled_compact::CompactVariant::Amoled143Sh8601))),
        "waveshare-s3-touch-amoled143-co5300" => Some(Box::new(waveshare_amoled_compact::WaveshareCompact::new(waveshare_amoled_compact::CompactVariant::Amoled143Co5300))),
        "waveshare-s3-touch-amoled18-v1" => Some(Box::new(waveshare_amoled_compact::WaveshareCompact::new(waveshare_amoled_compact::CompactVariant::Amoled18V1))),
        "waveshare-s3-touch-amoled18-v2" => Some(Box::new(waveshare_amoled_compact::WaveshareCompact::new(waveshare_amoled_compact::CompactVariant::Amoled18V2))),
        "waveshare-s3-lcd147"=>Some(Box::new(waveshare_lcd147_plain::WaveshareLcd147Plain::new())),
        "waveshare-s3-touch-lcd146"=>Some(Box::new(waveshare_round_tca::WaveshareRoundTca::new(true,true))),
        "waveshare-s3-touch-lcd185"=>Some(Box::new(waveshare_round_tca::WaveshareRoundTca::new(false,true))),
        "waveshare-s3-lcd185"=>Some(Box::new(waveshare_round_tca::WaveshareRoundTca::new(false,false))),
        "waveshare-s3-amoled191-v1"=>Some(Box::new(waveshare_amoled191::WaveshareAmoled191::new(false,false))),
        "waveshare-s3-amoled191-v2"=>Some(Box::new(waveshare_amoled191::WaveshareAmoled191::new(false,true))),
        "waveshare-s3-touch-amoled191-v1"=>Some(Box::new(waveshare_amoled191::WaveshareAmoled191::new(true,false))),
        "waveshare-s3-touch-amoled191-v2"=>Some(Box::new(waveshare_amoled191::WaveshareAmoled191::new(true,true))),
        "waveshare-s3-lcd2"=>Some(Box::new(waveshare_lcd2::WaveshareLcd2::new(false))),
        "waveshare-s3-touch-lcd2"=>Some(Box::new(waveshare_lcd2::WaveshareLcd2::new(true))),
        "waveshare-s3-lcd19" | "waveshare-s3-touch-lcd19" | "waveshare-s3-lcd13" => waveshare_small_spi::make_board(name),
        "waveshare-s3-touch-lcd4-v4" | "waveshare-s3-lcd4-v4" => waveshare_touch4_v4::make_board(name),
        "waveshare-s3-matrix8" => Some(Box::new(waveshare_matrix8::WaveshareMatrix8::new())),
        "waveshare-s3-lcd147b" | "waveshare-s3-touch-lcd147" => waveshare_lcd147::make_board(name),
        "waveshare-s3-touch-lcd35-v2" => Some(Box::new(waveshare_lcd35::WaveshareLcd35::new())),
        "waveshare-s3-touch-lcd35b" => Some(Box::new(waveshare_lcd35b::WaveshareLcd35b::new())),
        "waveshare-s3-touch-amoled241-v2" => Some(Box::new(waveshare_amoled241::WaveshareAmoled241::new())),
        "waveshare-s3-touch-amoled164-rev11" => Some(Box::new(waveshare_amoled164::WaveshareAmoled164::new())),
        "waveshare-s3-touch-lcd185c-v2" => Some(Box::new(waveshare_lcd185c::WaveshareLcd185c::new())),
        "waveshare-s3-touch-lcd185b" => Some(Box::new(waveshare_lcd185b::WaveshareLcd185b::new())),
        "waveshare-s3-lr1121-oled154" | "waveshare-s3-lr1121-epaper29" => waveshare_lr::make_board(name),
        "waveshare-s3-touch-amoled206" => Some(Box::new(waveshare_amoled_watch::WaveshareAmoledWatch::new(waveshare_amoled_watch::AmoledWatchVariant::TouchAmoled206))),
        "waveshare-s3-touch-amoled143c" => Some(Box::new(waveshare_qspi::WaveshareQspi::new(waveshare_qspi::QspiVariant::TouchAmoled143C))),
        "cloudzao-claw-154" => Some(Box::new(cloudzao_claw154::Claw154::new())),
        "lilygo-t-keyboard-s3-pro" => Some(Box::new(lilygo_io::LilygoIo::t_keyboard_s3_pro())),
        "lilygo-t-micro32-s3" => Some(Box::new(lilygo_io::LilygoIo::t_micro32_s3())),
        "lilygo-t-panel-s3" => Some(Box::new(lilygo_io::LilygoIo::t_panel_s3())),
        "lilygo-t-camera-plus-s3" => Some(Box::new(lilygo_io::LilygoIo::t_camera_plus_s3())),
        "lilygo-t-vending" => Some(Box::new(lilygo_io::LilygoIo::t_vending())),
        "lilygo-t-track" => Some(Box::new(lilygo_io::LilygoIo::t_track())),
        "lilygo-t-fpga" => Some(Box::new(lilygo_io::LilygoIo::t_fpga())),
        "lilygo-t7-s3" => Some(Box::new(lilygo_io::LilygoIo::t7_s3())),
        "lilygo-t-connect" => Some(Box::new(lilygo_io::LilygoIo::t_connect())),
        "lilygo-t-eth-elite" => Some(Box::new(lilygo_io::LilygoIo::t_eth_elite())),
        "lilygo-t-energy-s3" => Some(Box::new(lilygo_io::LilygoIo::t_energy_s3())),
        "lilygo-t-sim7670g-s3" => Some(Box::new(lilygo_io::LilygoIo::t_sim7670g_s3())),
        "lilygo-t-rs-s3" => Some(Box::new(lilygo_io::LilygoIo::t_rs_s3())),
        "lilygo-t-eth-lite" => Some(Box::new(lilygo_io::LilygoIo::t_eth_lite())),
        "lilygo-t-relay-s3" => Some(Box::new(lilygo_io::LilygoIo::t_relay_s3())),
        "lilygo-t-sim7080-s3" => Some(Box::new(lilygo_io::LilygoIo::t_sim7080_s3())),
        "lilygo-t-camera-s3" => Some(Box::new(lilygo_io::LilygoIo::t_camera_s3())),
        "lilygo-t-simcam" => Some(Box::new(lilygo_io::LilygoIo::t_simcam())),
        name if name.starts_with("seeed-xiao-epaper-ee") => seeed_ee::make_board(name),
        "seeed-trmnl-75-diy" => Some(Box::new(esp_soc::board::seeed_xiao_epaper::SeeedXiaoEpaper::new(esp_soc::board::seeed_xiao_epaper::Variant::TrmnlS3,crate::periph::CPU_HZ/1000))),
        "seeed-sensecap-watcher" => Some(Box::new(seeed_sensecap::SeeedWatcher::new())),
        "seeed-sensecap-indicator" | "seeed-sensecap-indicator-gx" => Some(Box::new(seeed_indicator::SeeedIndicator::new(seeed_indicator::IndicatorPanel::Gx))),
        "seeed-sensecap-indicator-dx" => Some(Box::new(seeed_indicator::SeeedIndicator::new(seeed_indicator::IndicatorPanel::Dx))),
        "seeed-sensecap-indicator-d1l" => Some(Box::new(seeed_indicator_lora::SeeedIndicatorLoRa::new(seeed_indicator_lora::Variant::D1L))),
        "seeed-sensecap-indicator-d1pro" => Some(Box::new(seeed_indicator_lora::SeeedIndicatorLoRa::new(seeed_indicator_lora::Variant::D1Pro))),
        "seeed-sensecap-indicator-meshtastic" => Some(Box::new(seeed_indicator_lora::SeeedIndicatorLoRa::new(seeed_indicator_lora::Variant::Meshtastic))),
        "seeed-reterminal-sticky" => Some(Box::new(seeed_reterminal::SeeedReTerminal::new(seeed_reterminal::Variant::Sticky))),
        "seeed-reterminal-e1001" => Some(Box::new(seeed_reterminal::SeeedReTerminal::new(seeed_reterminal::Variant::E1001))),
        "seeed-reterminal-e1002" => Some(Box::new(seeed_reterminal::SeeedReTerminal::new(seeed_reterminal::Variant::E1002))),
        "seeed-reterminal-e1004" => Some(Box::new(seeed_reterminal::SeeedReTerminal::new(seeed_reterminal::Variant::E1004))),
        "seeed-reterminal-e1003" => Some(Box::new(seeed_reterminal::SeeedReTerminal::new(seeed_reterminal::Variant::E1003))),
        "seeed-xiao-ips-147" => Some(Box::new(seeed_xiao_ips::SeeedXiaoIps::new(seeed_xiao_ips::IpsSize::Touch147))),
        "seeed-xiao-ips-114" => Some(Box::new(seeed_xiao_ips::SeeedXiaoIps::new(seeed_xiao_ips::IpsSize::Lcd114))),
        "seeed-xiao-ips-096" => Some(Box::new(seeed_xiao_ips::SeeedXiaoIps::new(seeed_xiao_ips::IpsSize::Lcd096))),

        "tembed-cc1101" | "lilygo-t-embed-cc1101" => Some(Box::new(tembed_cc1101::TEmbedCc1101::new())),
        "airquality" | "m5stack-k131" => Some(Box::new(airquality::AirQuality::new())),
        "airquality-v11" | "m5stack-k131-v11" => Some(Box::new(airquality::AirQuality::v11())),
        "tdisplay-s3" | "lilygo-t-display-s3" => Some(Box::new(tdisplay_s3::TDisplayS3::new())),
        "atech14" | "atech" => Some(Box::new(Atech14::new())),
        "none" | "bare" => Some(Box::new(NoBoard)),
        "unit-cams3" | "m5stack-u174" => Some(Box::new(unit_cams3::UnitCamS3::new(false))),
        "unit-cams3-5mp" | "m5stack-u174-b" => Some(Box::new(unit_cams3::UnitCamS3::new(true))),
        "atom-voices3r" | "m5stack-c126-echo" => Some(Box::new(atom_voices3r::AtomVoiceS3R::new())),
        "vameter" | "m5stack-k136" => Some(Box::new(vameter::VAMeter::new())),
        "xiaozhi-card" | "m5stack-k146" => Some(Box::new(xiaozhi_card::XiaozhiCard::new())),
        "powerhub" | "m5stack-c148" | "m5stack-k148" => Some(Box::new(powerhub::PowerHub)),
        "atoms3r-chatbot" | "m5stack-k147" => Some(Box::new(AtomS3R::chatbot())),
        "atoms3r-cam-chatbot" | "m5stack-k147-cam" => Some(Box::new(atoms3r_cam::AtomS3RCam::new())),
        "atoms3r-ext" | "m5stack-c126-ext" => Some(Box::new(atoms3r_cam::AtomS3RCam::with_variant(atoms3r_cam::Variant::Ext))),
        "atoms3r-cam" | "m5stack-c126-cam" => Some(Box::new(atoms3r_cam::AtomS3RCam::with_variant(atoms3r_cam::Variant::Cam))),
        "atoms3r-m12" | "m5stack-c126-m12" => Some(Box::new(atoms3r_cam::AtomS3RCam::with_variant(atoms3r_cam::Variant::M12))),
        "atoms3r-m12-volcengine" | "m5stack-d062-m12" => Some(Box::new(atoms3r_cam::AtomS3RCam::with_variant(atoms3r_cam::Variant::Volcengine))),
        "atoms3r" | "m5stack-atoms3r" => Some(Box::new(AtomS3R::new())),
        "atoms3" | "m5stack-atoms3" => Some(Box::new(AtomS3::new())),
        "stackchan" | "m5stack-k151" => Some(Box::new(stackchan::StackChan::new())),
        "cores3" | "m5stack-k128" => Some(Box::new(cores3::CoreS3::new(cores3::Variant::Standard))),
        "cores3-se" | "m5stack-k128-se" => Some(Box::new(cores3::CoreS3::new(cores3::Variant::Se))),
        "cores3-lite" | "m5stack-k128-lite" => Some(Box::new(cores3::CoreS3::new(cores3::Variant::Lite))),
        "cores3-thread-br" | "m5stack-k149" => Some(Box::new(cores3::CoreS3::new(cores3::Variant::ThreadBr))),
        "dial" | "m5stack-dial" => Some(Box::new(Dial::new())),
        "stamps3" | "m5stack-s007" => Some(Box::new(stamps3::StampS3::new())),
        "stamps3a" | "m5stack-s007-v033" => Some(Box::new(stamps3::StampS3::s3a())),
        "stamps3bat" | "m5stack-s015" => Some(Box::new(stamps3bat::StampS3Bat::new())),
        "stamplc" | "m5stack-k141" => Some(Box::new(stamplc::StamPlc::new())),
        "stampfly" | "m5stack-k138" => Some(Box::new(stampfly::StampFly::new())),
        "stampfly-v11" | "m5stack-k138-v11" => Some(Box::new(stampfly::StampFly::v11())),
        "dinmeter" | "m5stack-dinmeter" => Some(Box::new(DinMeter::new())),
        "chain-captain" | "m5stack-c160" => Some(Box::new(chain_captain::ChainCaptain::new())),
        "dinmeter-v11" | "m5stack-k134-v11" => Some(Box::new(DinMeter::v11())),
        "chain-dualkey" | "m5stack-c147" => Some(Box::new(chain_dualkey::ChainDualKey::new())),
        "dial-v11" | "m5stack-dial-v11" => Some(Box::new(Dial::v11())),
        "atoms3u" | "m5stack-k125" => Some(Box::new(atoms3u::AtomS3U::new())),
        "capsule" | "m5stack-k129" => Some(Box::new(capsule::Capsule::new(false))),
        "capsule-v11" | "m5stack-k129-v11" => Some(Box::new(capsule::Capsule::new(true))),
        "atoms3-lite" | "m5stack-atoms3-lite" => Some(Box::new(AtomS3Lite::new())),
        "papermono" | "m5stack-papermono" => Some(Box::new(PaperMono::new())),
        "papercolor" | "m5stack-c151" => Some(Box::new(PaperColor::new())),
        "papermono-lite" | "m5stack-c153-lite" => Some(Box::new(PaperMono::lite())),
        "paperdiy" | "m5stack-k159-b" => Some(Box::new(PaperDiy::new())),
        "papers3" | "m5stack-c139" => Some(Box::new(papers3::PaperS3::new())),
        "metalio-ink4" => Some(Box::new(MetalioInk4::new())),
        "tdeck" | "lilygo-t-deck" => Some(Box::new(tdeck::TDeck::new())),
        "lilygo-t-deck-pro" => Some(Box::new(lilygo_epaper::LilygoEpaper::t_deck_pro())),
        "lilygo-mini-epaper-s3" => Some(Box::new(lilygo_epaper::LilygoEpaper::mini_epaper_s3())),
        "cardputer" | "m5stack-cardputer" => Some(Box::new(Cardputer::new())),
        "cardputer-adv" | "m5stack-cardputer-adv" => Some(Box::new(CardputerAdv::new())),
        "sticks3" | "m5stack-k150" => Some(Box::new(sticks3::StickS3::new())),
        "stopwatch" | "m5stack-stopwatch" => Some(Box::new(StopWatch::new())),
        "waveshare-cam" | "waveshare" => Some(Box::new(WaveshareCam::new())),
        "waveshare-lcd4b" | "lcd4b" => Some(Box::new(WaveshareLcd4b::new())),
        "waveshare-amoled18-v2" | "amoled18-v2" => Some(Box::new(WaveshareAmoled18V2::new())),
        "waveshare-s3-lcd128" => Some(Box::new(waveshare_spi_legacy::WaveshareLegacySpi::new(waveshare_spi_legacy::LegacySpiVariant::Lcd128))),
        "waveshare-s3-touch-lcd128" => Some(Box::new(waveshare_spi_legacy::WaveshareLegacySpi::new(waveshare_spi_legacy::LegacySpiVariant::TouchLcd128))),
        "waveshare-s3-lcd-driver" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::Driver))),
        "waveshare-s3-lcd-driver-21" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::Driver21))),
        "waveshare-s3-lcd-driver-28" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::Driver28))),
        "waveshare-s3-lcd-driver-4" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::Driver4))),
        "waveshare-s3-touch-lcd21" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::TouchLcd21))),
        "waveshare-s3-touch-lcd21b" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::TouchLcd21B))),
        "waveshare-s3-lcd28b" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::Lcd28B))),
        "waveshare-s3-touch-lcd28b" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::TouchLcd28B))),
        "waveshare-s3-lcd28c" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::Lcd28C))),
        "waveshare-s3-touch-lcd28c" => Some(Box::new(waveshare_tca_rgb::WaveshareTcaRgb::new(waveshare_tca_rgb::TcaRgbVariant::TouchLcd28C))),
        "waveshare-s3-touch-amoled132" => Some(Box::new(waveshare_amoled132::WaveshareAmoled132::new())),
        "waveshare-s3-rlcd42" => Some(Box::new(waveshare_rlcd::WaveshareRlcd::new())),
        "waveshare-s3-touch-lcd349-v2" => Some(Box::new(waveshare_lcd349::WaveshareLcd349::new())),
        "waveshare-s3-dualeye-lcd128" | "waveshare-s3-dualeye-touch-lcd128" => Some(Box::new(waveshare_dualeye::WaveshareDualEye::new(name.contains("touch")))),
        "waveshare-s3-lcd7b" | "waveshare-s3-touch-lcd7b" => Some(Box::new(waveshare_lcd7b::WaveshareLcd7b::new(name.contains("touch")))),
        "waveshare-s3-touch-lcd4b" => Some(Box::new(waveshare_serial_rgb::WaveshareSerialRgb::new(waveshare_serial_rgb::Variant::Lcd4b))),
        "waveshare-s3-knob-touch-lcd18" => Some(Box::new(waveshare_knob18::WaveshareKnob18::new())),
        "waveshare-s3-lcd316" => Some(Box::new(waveshare_serial_rgb::WaveshareSerialRgb::new(waveshare_serial_rgb::Variant::Lcd316))),
        "waveshare-s3-geek" => Some(Box::new(waveshare_s3_geek::WaveshareS3Geek::new())),
        _ => waveshare_s3_cellular::make_board(name).or_else(|| waveshare_tail_spi::make_board(name)).or_else(|| waveshare_ch422_rgb::make_board(name)).or_else(|| waveshare_epaper_catalog::make_board(name)).or_else(|| waveshare_rgb_catalog::make_board(name)).or_else(|| waveshare_co5300_catalog::make_board(name))
            .or_else(|| waveshare_spi_catalog::make_board(name))
            .or_else(|| waveshare_rgb_matrix::make_board(name))
            .or_else(|| waveshare_epaper133::make_board(name))
            .or_else(|| waveshare_core::make_board(name))
            .or_else(|| waveshare_industrial::make_board(name))
            .or_else(|| waveshare_extended::make_board(name))
            .or_else(|| waveshare_industrial_controls::make_board(name))
            .or_else(|| lilygo_display::make_board(name))
            .or_else(|| lilygo_additional::make_board(name)),
    }
}

/// Cold board identity setup: ESP-IDF v5.5.1 S3 efuse_reg.h RD_MAC_SPI_SYS_3
/// offset 0x50, PKG_VERSION bits 23:21; M5GFX 65c816a3 identifies PICO as 1.
/// v4.4.8 defines the same offset/field. No changes for other products.
pub fn configure_chip_identity(board:&str,efuse:&mut esp_periph::efuse::Efuse) {
    if matches!(board,"atoms3r"|"atom-voices3r"|"atoms3r-chatbot"|"stamps3bat"|"atoms3r-cam-chatbot"|"atoms3r-ext"|"atoms3r-cam"|"atoms3r-m12"|"atoms3r-m12-volcengine"|"sticks3"|"m5stack-k150"|"xiaozhi-card"|"m5stack-k146") {let r=efuse.ram.read(0x50);efuse.ram.write(0x50,(r&!(7<<21))|(1<<21));}
}
