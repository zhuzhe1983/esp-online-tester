//! Independent ESP32-S2 SRAM app-direct diagnostic model.
//!
//! Reuses only compatible Xtensa integer instruction semantics. S2 memory aliases,
//! UART/GPIO addresses, absent ISA options and board wiring are selected here.
//! A bounded data-PSRAM MMU subset is optional. No mask ROM, mapped flash,
//! instruction PSRAM/cache timing, RTC memory, USB, Wi-Fi or ESP-IDF boot.
pub mod board;
pub mod bus;
pub mod core;
pub mod image;
pub mod soc;
pub mod psram;
pub mod waveshare;

pub use bus::S2Bus;
pub use core::S2Core;
pub use soc::{machine, Machine, S2};
