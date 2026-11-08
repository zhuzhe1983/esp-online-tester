//! Byte-level UART endpoints. A device receives on `tx_pins` and transmits on `rx_pin`.
use esp_periph::{Gpio, RegRam, Uart, uart::INT_FRM_ERR};
use crate::pins::ChipPins;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct UartRoute {
    pub port: usize,
    /// Every non-inverted, enabled TX output, including GPIO matrix fan-out.
    pub tx_pins: u64,
    pub rx_pin: Option<u8>,
    /// None when no source or divider is configured.
    pub baud: Option<u32>,
}
impl UartRoute {
    /// Route decoded by a chip with its own GPIO matrix and IO_MUX layout.
    pub fn new(port: usize, tx_pins: u64, rx_pin: Option<u8>, baud: Option<u32>) -> Self {
        Self { port, tx_pins, rx_pin, baud }
    }
    /// Three percent tolerance for divider rounding and oscillator error.
    pub fn matches_baud(&self, baud: u32) -> bool {
        baud != 0 && self.baud.is_some_and(|actual| actual.abs_diff(baud) as u64 * 100 <= baud as u64 * 3)
    }
    pub fn transmits_on(&self, pin: u8) -> bool { pin < 64 && self.tx_pins & (1u64 << pin) != 0 }
}

#[non_exhaustive]
pub struct UartInput {
    pub pin: u8,
    pub baud: u32,
    /// Completed characters; the ordinary 128-byte RX FIFO/overflow rules apply.
    pub data: Vec<u8>,
}
impl UartInput {
    pub fn new(pin: u8, baud: u32, data: Vec<u8>) -> Self { Self { pin, baud, data } }
}

/// Chip wiring from the IDF GPIO signal and IO_MUX tables.
pub struct UartPins {
    chip: ChipPins,
    signals: &'static [usize],
    native: &'static [(u8, u8, u32)],
}
impl UartPins {
    // IDF v5.5.4 gpio_sig_map.h:46,52,360; components/soc/esp32/register/soc/io_mux_reg.h:122,136
    // (FUNC_U0TXD/U0RXD 0), 181,189 (FUNC_SD_DATA2_U1RXD/SD_DATA3_U1TXD 4) and 242,250
    // (FUNC_GPIO16_U2RXD/GPIO17_U2TXD 4), used by uart_pins.h U*_MUX_FUNC.
    pub const ESP32: Self = Self { chip: ChipPins::ESP32, signals: &[14, 17, 198], native: &[(1, 3, 0), (10, 9, 4), (17, 16, 4)] };
    pub const S3: Self = Self { chip: ChipPins::S3, signals: &[12, 15, 18], native: &[(43, 44, 0), (17, 18, 2)] };
    pub const C3: Self = Self { chip: ChipPins::C3, signals: &[6, 9], native: &[(21, 20, 0)] };
    pub const C6: Self = Self { chip: ChipPins::C6, signals: &[6, 9], native: &[(16, 17, 0)] };

    pub fn route(&self, port: usize, gpio: &Gpio, mux: &RegRam, baud: Option<u32>) -> UartRoute {
        let mut route = UartRoute { port, tx_pins: 0, rx_pin: None, baud };
        let Some(&signal) = self.signals.get(port) else { return route };
        let pins = self.chip.routes(gpio, mux);
        let native = self.native.get(port);
        for pin in 0..64 {
            let direct = native.is_some_and(|&(tx, _, f)| tx as usize == pin && pins.function(pin, f));
            if direct || (gpio.enable & (1u64 << pin) != 0 && pins.matrix_output(pin, signal as u32)) {
                route.tx_pins |= 1u64 << pin;
            }
        }
        route.rx_pin = if gpio.func_in_sel[signal] & self.chip.input_select != 0 {
            pins.input_pin(signal)
        } else {
            native.and_then(|&(_, rx, f)| pins.input_function(rx as usize, f).then_some(rx))
        };
        route
    }
}

pub fn uart_pin_input(uarts: &mut [Uart], input: &UartInput, routes: &[UartRoute]) {
    if input.data.is_empty() { return; }
    for (uart, route) in uarts.iter_mut().zip(routes) {
        if route.rx_pin != Some(input.pin) { continue; }
        if route.matches_baud(input.baud) { uart.wire_input(&input.data); }
        else { uart.int_raw |= INT_FRM_ERR; }
    }
}
