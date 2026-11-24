//! Device models that are not tied to a chip: the things a board wires to the pins, fed with
//! what the SoC model already decoded (GPIO levels, SPI bytes, RMT bit streams). A board owns
//! them and adds what is module-specific — pin numbers, the visible window of a glass, where
//! each LED of a chain sits physically.
pub mod bmi270;
pub mod bmm150;
pub mod co5300;
pub mod ssd1677;
pub mod ed2208;
pub mod uc8179;
pub mod t133a01;
pub mod it8951;
pub mod ssd1681;
pub mod ssd16xx_mono;
pub mod jd796xx;
pub mod uc8151;
pub mod i2c_bitbang;
pub mod es8311;
pub mod es7210;
pub mod dcs_panel;
pub mod ft5x06;
pub mod imu_motion;
pub mod qmi8658;
pub mod tca9554;
pub mod pcf85063;
pub mod spi_bitbang;
pub mod ws2812;
pub mod sx1262;

pub use bmi270::{Bmi270, BMI270_ADDRESS};
pub use bmm150::{Bmm150, Bmm150Trim};
pub use dcs_panel::DcsPanel;
pub use ft5x06::{Ft5x06, Ft5x06Points, FT5X06_ADDRESS};
pub use spi_bitbang::SpiBitBang;
pub use ws2812::Ws2812Chain;

pub mod et047tc1;
pub mod bm8563;
pub mod oled_mono;

pub mod uc81xx;
pub mod ssd1680;
pub mod axp2101;
pub mod axs5106;
pub mod qspi_dcs;

pub mod waveshare_touch_parts;

pub mod waveshare_ch32v006;
pub mod waveshare_ili9488;
pub mod ft3168;
