//! Classic ESP32 (D0WD ECO3): two LX6 cores, mask ROM, cache windows and the boot peripherals.
pub mod adc;
pub mod ble;
pub mod bt_clock;
pub mod phy_noise;
pub mod bt_init;
pub mod ble_clock;
pub mod bt_sleep;
pub mod bus;
mod psram;
pub mod periph;
pub mod soc;
mod sleep;
pub mod timers;
pub mod frc;
pub mod wifi;

pub use esp_soc::Stop;
pub use soc::{machine, Esp32, Machine};

pub mod board;

pub mod i2s;
pub mod camera_fifo;
mod camera_dma;
mod ethernet;
pub mod ledc;
pub mod rmt;
pub mod spi;

pub mod crypto;
