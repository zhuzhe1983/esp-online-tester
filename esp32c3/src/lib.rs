//! ESP32-C3: the RISC-V sibling of the ESP32-S3 model in this workspace.
//!
//! One RV32IMC core at 160 MHz, 400 KB SRAM, no PSRAM, an 8 MB flash cache window per bus.
//! Peripheral models are shared with `esp32s3` where the IP is the same (see `periph.rs`).
pub mod bus;
pub mod periph;
pub mod soc;

pub use esp_soc::Stop;
pub use soc::{machine, Machine, C3};

mod ble;
pub mod ble_lc;

pub mod wifi;
