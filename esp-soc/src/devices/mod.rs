//! Device models that are not tied to a chip: the things a board wires to the pins, fed with
//! what the SoC model already decoded (GPIO levels, SPI bytes, RMT bit streams). A board owns
//! them and adds what is module-specific — pin numbers, the visible window of a glass, where
//! each LED of a chain sits physically.
pub mod co5300;
pub mod ssd1677;
pub mod ed2208;
pub mod i2c_bitbang;
pub mod dcs_panel;
pub mod spi_bitbang;
pub mod ws2812;
pub mod sx1262;

pub use dcs_panel::DcsPanel;
pub use spi_bitbang::SpiBitBang;
pub use ws2812::Ws2812Chain;

pub mod et047tc1;
