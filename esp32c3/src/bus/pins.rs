use crate::periph::Peripherals;
use esp_soc::{board::{SpiPins, SpiQspiPins}, pins::{ChipPins, SpiQspiNative}};

impl Peripherals {
    pub(crate) fn i2c_pin(&self, signal: usize) -> Option<u8> { ChipPins::C3.routes(&self.gpio, &self.io_mux).i2c_pin(signal) }
    pub(crate) fn spi2_pins(&self) -> SpiPins {
        ChipPins::C3.routes(&self.gpio, &self.io_mux).spi_pins(&self.spi2, [63, 64, 65, 68, 69, 70, 71, 72, 73], 128, &[(2, 6, 7, 2, &[10])])
    }
    /// ESP-IDF v5.5.4 C3 gpio_sig_map.h and io_mux_reg.h: FSPI signals
    /// CLK63/D65/Q64/WP67/HD66, CS68..73; native function2 on6/7/2/5/4/10.
    pub(crate) fn spi2_phase_pins(&self) -> SpiQspiPins {
        ChipPins::C3.routes(&self.gpio, &self.io_mux).spi_qspi_pins(&self.spi2,
            [63,65,64,67,66,68,69,70,71,72,73],
            &[SpiQspiNative{function:2,sclk:6,data:[7,2,5,4],cs:&[10]}])
    }
}
