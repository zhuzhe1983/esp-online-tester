//! Physical GPIO matrix and IO_MUX route decoding shared by chip buses and board devices.
use esp_periph::{Gpio, GpSpi, RegRam};
use crate::board::{SpiPins, SpiQspiPins};

/// One native GP-SPI IO_MUX group, in physical D/Q/WP/HD order. Chip adapters
/// supply only documented groups; unsupported chip signals are u32::MAX.
pub struct SpiQspiNative<'a> {
    pub function: u32,
    pub sclk: u8,
    pub data: [u8; 4],
    pub cs: &'a [u8],
}

pub struct ChipPins {
    pub valid: u64,
    pub input_select: u32,
    /// OUT_SEL and OUT_INV, excluding output-enable control bits.
    pub output_mask: u32,
}
impl ChipPins {
    pub const C3: Self = Self { valid: (1 << 22) - 1, input_select: 0x40, output_mask: 0x1ff };
    pub const C6: Self = Self { valid: (1 << 31) - 1, input_select: 0x80, output_mask: 0x1ff };
    pub const S3: Self = Self { valid: ((1u64 << 49) - 1) & !(15 << 22), input_select: 0x80, output_mask: 0x3ff };
    /// Classic ESP32 routes read the linear, S3-numbered pad copy kept by its GPIO adapter.
    pub const ESP32: Self = Self { valid: ((1u64 << 40) - 1) & !((1 << 20) | (1 << 24) | (15 << 28)), input_select: 0x80, output_mask: 0x3ff };
    pub fn routes<'a>(&'a self, gpio: &'a Gpio, mux: &'a RegRam) -> PinRoutes<'a> { PinRoutes { chip: self, gpio, mux } }
}

pub struct PinRoutes<'a> {
    chip: &'a ChipPins,
    gpio: &'a Gpio,
    mux: &'a RegRam,
}
impl PinRoutes<'_> {
    /// Opt-in SDR phase snapshot. Signals: CLK,D,Q,WP,HD,CS0..5. Sources:
    /// ESP-IDF 735507283d5b2f9fb363a1901172dbd9e847945d S3 spi_reg.h,
    /// gpio_sig_map.h and io_mux_reg.h (retained with their content hashes).
    /// Only chip adapters whose register layout matches this source may call it.
    pub fn spi_qspi_pins(&self, spi: &GpSpi, signals: [u32; 11], native: &[SpiQspiNative<'_>]) -> SpiQspiPins {
        let user = spi.read(0x10);
        let ctrl = spi.read(0x08);
        let misc = spi.read(0x20);
        let user1 = spi.read(0x14);
        let user2 = spi.read(0x18);
        let width = |enabled: bool, bits: u32| -> u8 {
            if !enabled { 0 } else if bits & 4 != 0 { 8 } else if bits & 2 != 0 { 4 } else if bits & 1 != 0 { 2 } else { 1 }
        };
        let command_width = width(user & (1 << 31) != 0, (ctrl >> 8) & 7);
        let address_width = width(user & (1 << 30) != 0, (ctrl >> 5) & 7);
        let data_width = width(user & (1 << 27) != 0, (user >> 12) & 7);
        let read_width = width(user & (1 << 28) != 0, (ctrl >> 14) & 7);
        let data_bits = (spi.read(0x1c) & 0x3ffff) + 1;
        let mut pins = SpiQspiPins {
            command_width, address_width, data_width, read_width,
            command_bits: if command_width != 0 { ((user2 >> 28) & 15) as u8 + 1 } else { 0 },
            address_bits: if address_width != 0 { (user1 >> 27) as u8 + 1 } else { 0 },
            data_bits: if data_width != 0 { data_bits } else { 0 },
            read_bits: if read_width != 0 { data_bits } else { 0 },
            dummy_cycles: if user & (1 << 29) != 0 { (user1 & 255) as u16 + 1 } else { 0 },
            sio: user & (1 << 17) != 0,
            full_duplex: user & 1 != 0,
            keep_cs: misc & (1 << 30) != 0,
            clock_idle_high: misc & (1 << 29) != 0,
            clock_output_edge: user & (1 << 9) != 0,
            unsupported_mode: ctrl & (15 << 23) != 0 || misc & ((15 << 16) | (1 << 31)) != 0
                || spi.read(0xe0) & ((1 << 26) | (1 << 28)) != 0 || user & (1 << 15) != 0
                || [command_width, address_width, data_width, read_width].contains(&8)
                || [(command_width, (ctrl >> 8) & 7), (address_width, (ctrl >> 5) & 7),
                    (data_width, (user >> 12) & 7), (read_width, (ctrl >> 14) & 7)]
                    .iter().any(|&(w, bits)| w != 0 && bits.count_ones() > 1),
            ..Default::default()
        };
        for pin in 0..64 {
            if !self.valid_pin(pin) { continue; }
            let native_pad = |n: &SpiQspiNative<'_>, pad: u8| usize::from(pad) == pin && self.function(pin, n.function);
            if self.matrix_output(pin, signals[0]) || native.iter().any(|n| native_pad(n, n.sclk)) {
                pins.sclk |= 1 << pin;
            }
            for lane in 0..4 {
                if self.matrix_output(pin, signals[1 + lane]) || native.iter().any(|n| native_pad(n, n.data[lane])) {
                    pins.data[lane] |= 1 << pin;
                }
            }
            if self.software_output(pin as u8) == Some(false) { pins.software_cs |= 1 << pin; }
            for cs in 0..6 {
                if misc & ((1 << cs) | (1 << (cs + 7))) == 0
                    && (self.matrix_output(pin, signals[5 + cs])
                        || native.iter().any(|n| n.cs.get(cs).is_some_and(|&pad| native_pad(n, pad))))
                {
                    pins.hardware_cs |= 1 << pin;
                }
            }
        }
        if read_width != 0 {
            for lane in 0..4 {
                let signal = signals[1 + lane] as usize;
                if signal >= self.gpio.func_in_sel.len() { continue; }
                pins.input[lane] = self.matrix_input(signal);
                if self.gpio.func_in_sel[signal] & self.chip.input_select == 0 {
                    pins.input[lane] = native.iter().find_map(|n| {
                        let pad = n.data[lane];
                        self.input_function(usize::from(pad), n.function).then_some(pad)
                    });
                }
            }
        }
        pins.cs = pins.hardware_cs | pins.software_cs;
        pins
    }
    /// Programmed GPIO-latch drive, not electrical input/pull resolution or a peripheral output.
    /// IDF v5.5.4 components/soc/{esp32,esp32s3}/register/soc/gpio_reg.h:
    /// FUNC0_OUT_SEL/OUT_INV_SEL/OEN_INV_SEL bits 0..8/9/11 (lines 7065..7092 and
    /// 7790..7813); {esp32c3,esp32c6}: bits 0..7/8/10 (3883..3906 and 4992..5023).
    /// For the GPIO selector, using GPIO_ENABLE regardless of OEN_SEL is inferred
    /// model behavior shared with the classic adapter, not hardware calibration.
    pub fn software_output(&self, pin: u8) -> Option<bool> {
        let pin = usize::from(pin);
        if !self.function(pin, 1) { return None; }
        let signal = (self.chip.output_mask + 1) / 4;
        let route = self.gpio.func_out_sel[pin];
        if route & (signal * 2 - 1) != signal { return None; }
        let enabled = (self.gpio.enable & (1u64 << pin) != 0) ^ (route & (signal * 8) != 0);
        enabled.then(|| (self.gpio.out & (1u64 << pin) != 0) ^ (route & (signal * 2) != 0))
    }
    pub fn valid_pin(&self, pin: usize) -> bool { pin < 64 && self.chip.valid & (1u64 << pin) != 0 }
    fn mux(&self, pin: usize) -> u32 { self.mux.read(4 + 4 * pin as u32) }
    pub fn function(&self, pin: usize, function: u32) -> bool { self.valid_pin(pin) && self.mux(pin) & (7 << 12) == function << 12 }
    pub fn input_function(&self, pin: usize, function: u32) -> bool { self.function(pin, function) && self.mux(pin) & (1 << 9) != 0 }
    /// Matrix input selection and FUN_IE, independent of the output function.
    pub fn input_pin(&self, signal: usize) -> Option<u8> {
        let sel = self.gpio.func_in_sel[signal];
        let invert = self.chip.input_select >> 1;
        let pin = (sel & (invert - 1)) as usize;
        (sel & (self.chip.input_select | invert) == self.chip.input_select && self.valid_pin(pin) && self.mux(pin) & (1 << 9) != 0).then_some(pin as u8)
    }
    pub fn matrix_input(&self, signal: usize) -> Option<u8> { self.input_pin(signal).filter(|&pin| self.function(pin as usize, 1)) }
    pub fn matrix_output(&self, pin: usize, signal: u32) -> bool {
        if !self.function(pin, 1) { return false; }
        let sel = self.gpio.func_out_sel[pin];
        let oen = self.chip.output_mask + 1;
        sel & (self.chip.output_mask | oen << 1) == signal && (sel & oen == 0 || self.gpio.enable & (1 << pin) != 0)
    }
    pub fn output_pins(&self, signal: u32) -> impl Iterator<Item = u8> + '_ {
        (0..49).filter(move |&pin| self.matrix_output(pin, signal)).map(|pin| pin as u8)
    }
    pub fn i2c_pin(&self, signal: usize) -> Option<u8> {
        self.matrix_input(signal).filter(|&pin| self.matrix_output(pin as usize, signal as u32))
    }
    /// Signals are CLK, MISO, MOSI, CS0..5; native tuples hold function, CLK/MOSI/MISO and CS pins.
    /// IDF v5.5.4 components/soc/esp32c6/register/soc/spi_reg.h:365-403,497-559
    /// defines the shared USER phase and MISC CS disable/polarity fields.
    pub fn spi_pins(&self, spi: &GpSpi, signals: [u32; 9], gpio_signal: u32, native: &[(u32, usize, usize, usize, &[usize])]) -> SpiPins {
        let outputs = |signal, role| {
            let mut mask = 0;
            for pin in 0..64 {
                if self.valid_pin(pin)
                    && (self.matrix_output(pin, signal)
                        || native.iter().any(|&(f, c, o, _, _)| {
                            self.mux(pin) & (7 << 12) == f << 12 && [c, o][role] == pin
                        }))
                {
                    mask |= 1 << pin;
                }
            }
            mask
        };
        let user = spi.read(0x10);
        let mut pins = SpiPins {
            sclk: outputs(signals[0], 0),
            // ESP-IDF v5.2.1 S3 spi_reg.h MISC+20h bit30 and
            // spi_ll_master_keep_cs; same bit in v5.5.4 C3/C6.
            keep_cs: spi.read(0x20) & (1 << 30) != 0,
            ..Default::default()
        };
        if user & (1 << 31 | 1 << 30 | 1 << 27) != 0 {
            pins.mosi = outputs(signals[2], 1);
        }
        for pin in 0..64 {
            if !self.valid_pin(pin) {
                continue;
            }
            if self.matrix_output(pin, gpio_signal)
                && self.gpio.enable & (1 << pin) != 0
                && self.gpio.out & (1 << pin) == 0
            {
                pins.cs |= 1 << pin;
            }
            for cs in 0..6 {
                if spi.read(0x20) & ((1 << cs) | (1 << (cs + 7))) == 0
                    && (self.matrix_output(pin, signals[3 + cs])
                        || native.iter().any(|&(f, _, _, _, selects)| {
                            selects.get(cs) == Some(&pin) && self.function(pin, f)
                        }))
                {
                    pins.cs |= 1 << pin;
                }
            }
        }
        if user & (1 << 28) != 0 {
            // IDF v5.4.2 (Cardputer-Adv factory) and v5.5.1 S3 spi_reg.h:
            // v4.4.6 S3 (original Cardputer) and v5.4.2 C3/C6 also use bit 17.
            // USER.SIO reads on D/MOSI, not Q/MISO. gpio_sig_map.h gives
            // D_IN the same index as D_OUT (SPI2 103, SPI3 68); spi_common.c
            // routes both directions when initializing a three-wire bus.
            let sio = user & (1 << 17) != 0;
            let signal = signals[if sio { 2 } else { 1 }] as usize;
            pins.miso = self.matrix_input(signal);
            if self.gpio.func_in_sel[signal] & self.chip.input_select == 0 {
                pins.miso = native.iter().find_map(|&(f, _, mosi, miso, _)| {
                    let pin = if sio { mosi } else { miso };
                    self.input_function(pin, f).then_some(pin as u8)
                });
            }
        }
        pins
    }
}
