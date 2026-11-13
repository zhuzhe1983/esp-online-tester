//! Device models that are not tied to a chip: the things a board wires to the pins, fed with
//! what the SoC model already decoded (GPIO levels, SPI bytes, RMT bit streams). A board owns
//! them and adds what is module-specific — pin numbers, the visible window of a glass, where
//! each LED of a chain sits physically.
pub mod bmi270;
pub mod bmm150;
pub mod co5300;
pub mod dcs_panel;
pub mod ft5x06;
pub mod imu_motion;
pub mod spi_bitbang;
pub mod ws2812;

pub use bmi270::{Bmi270, BMI270_ADDRESS};
pub use bmm150::{Bmm150, Bmm150Trim};
pub use dcs_panel::DcsPanel;
pub use ft5x06::{Ft5x06, Ft5x06Points, FT5X06_ADDRESS};
pub use spi_bitbang::SpiBitBang;
pub use ws2812::Ws2812Chain;
