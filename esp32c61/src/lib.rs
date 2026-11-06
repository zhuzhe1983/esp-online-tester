//! ESP32-C61 rev1: independent memory map, standard CLIC and external-memory MMU.
pub mod board;
pub mod bus;
pub mod periph;
pub mod soc;
pub use esp_soc::Stop;
pub use soc::{machine, Machine, C61};
mod adc;
