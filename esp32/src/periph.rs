//! Classic ESP32 peripheral map. Shared models are adapted only where this chip's register layout
//! predates the S3/C3 layout.
//! Register sources: ESP-IDF v5.5.4 components/soc/esp32/register/soc/{dport,gpio,
//! io_mux,spi,rtc_cntl,uart,efuse}_reg.h and include/soc/hwcrypto_reg.h:43-46.
//! Adapter completion behavior is inferred;
//! no hardware timing validation is claimed.
pub use crate::crypto::{ClassicAes, ClassicRsa, ClassicSha};
use crate::{timers::ClassicTimer, rmt::ClassicRmt, spi::ClassicGpSpi, ledc::ClassicLedc};
use crate::adc::ClassicAdc;
use emu_core::{ClockDomain, ClockTree};
use esp_periph::{
    device_set, mmio, Device, DeviceSet, Dispatch, Gpio, Misc, RegRam, RtcCntl, SpiMem,
    Uart, UartLayout, WriteEffect,
};

// Stable bus-time reference. The selected CPU clock is converted to this domain;
// CCOUNT still counts CPU clocks, and APB/RTC devices keep their own clock rates.
pub const CPU_HZ: u64 = 240_000_000;
pub const PERIPH_BASE: u32 = 0x3ff0_0000;
pub const PERIPH_END: u32 = 0x3ff8_0000;
pub(crate) const NUM_SOURCES: usize = 69;

// ESP-IDF v5.5.4 components/soc/esp32/include/soc/interrupts.h:32-86.
const SRC_WIFI_MAC: usize = 0;
const SRC_TG0_T0: usize = 14;
const SRC_TG0_T1: usize = 15;
const SRC_TG0_WDT: usize = 16;
const SRC_TG0_LACT: usize = 17;
const SRC_TG1_T0: usize = 18;
const SRC_TG1_T1: usize = 19;
const SRC_TG1_WDT: usize = 20;
const SRC_TG1_LACT: usize = 21;
const SRC_GPIO: usize = 22;
const SRC_GPIO_NMI: usize = 23;
const SRC_FROM_CPU0: usize = 24;
const SRC_SPI2: usize = 30;
const SRC_SPI3: usize = 31;
const SRC_LEDC: usize = 43;
const SRC_RMT: usize = 47;
const SRC_I2C0: usize = 49;
const SRC_I2C1: usize = 50;
const SRC_RSA: usize = 51;
const SRC_SPI2_DMA: usize = 53;
const SRC_SPI3_DMA: usize = 54;
const I2C_SIGNALS: [(usize, usize); 2] = [(29, 30), (95, 96)];
const SRC_UART0: usize = 34;
const SRC_UART1: usize = 35;
const SRC_UART2: usize = 36;
const SRC_RTC_CORE: usize = 46;
// IDF v5.5.4 interrupts.h:75,79: edge sources 58/62; source 57 is ETS_TIMER2.
const SRC_TG0_T0_EDGE: usize = 58;
const SRC_TG1_T0_EDGE: usize = 62;

// IDF v5.5.4 register/soc/dport_reg.h:118-141: APP CPU reset, clock and stall, bit 0.
// DPORT maps peripheral sources independently onto the PRO and APP CPU lines.
pub struct Dport {
    pub ram: RegRam,
    pub map: [[u32; NUM_SOURCES]; 2],
}
impl Dport {
    fn new() -> Self {
        let mut ram = RegRam::new();
        ram.write(0xcc, 0xfffc_e030); // IDF v5.5.4 dport_reg.h:1031-1037.
        ram.write(0x2c, 1); // APP CPU held in reset
        Self {
            ram,
            map: [[16; NUM_SOURCES]; 2],
        }
    }
    pub fn core1_control(&self) -> (bool, bool, bool) {
        (self.ram.read(0x30) & 1 != 0, self.ram.read(0x2c) & 1 != 0, self.ram.read(0x34) & 1 != 0)
    }
    pub fn cpu_lines(&self, core: usize, status: &[u32; 3]) -> u32 {
        let mut lines = 0;
        for source in 0..NUM_SOURCES {
            if status[source / 32] & (1 << (source % 32)) != 0 {
                let line = self.map[core][source];
                if line < 32 {
                    lines |= 1 << line;
                }
            }
        }
        lines
    }
}
impl Device for Dport {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x104..=0x214 => self.map[0][((off - 0x104) / 4) as usize],
            0x218..=0x328 => self.map[1][((off - 0x218) / 4) as usize],
            0x3f0 | 0x418 => 1 << 7, // cache controller idle
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, mut v: u32) -> WriteEffect {
        match off {
            0x104..=0x214 => self.map[0][((off - 0x104) / 4) as usize] = v & 31,
            0x218..=0x328 => self.map[1][((off - 0x218) / 4) as usize] = v & 31,
            0x40 | 0x58 => { // PRO/APP CACHE_CTRL: invalidate completes synchronously
                if v & (1 << 4) != 0 {
                    v = (v & !(1 << 4)) | (1 << 5);
                }
                self.ram.write(off, v);
            }
            _ => self.ram.write(off, v),
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 {
        (0..4).fold(0, |bits, n| {
            bits | ((self.ram.read(0xdc + 4 * n) as u64 & 1) << n)
        })
    }
}

pub struct ClassicSpi(pub SpiMem);
impl ClassicSpi {
    fn new(command: bool) -> Self {
        let mut s = SpiMem::new(command);
        s.has_psram = false;
        s.addr_msb_aligned = true;
        Self(s)
    }
    // IDF v5.5.4 soc/esp32/register/soc/spi_reg.h -> esp32s3 spi_mem_reg.h offsets:
    // RD_STATUS 0x10->0x2c, CTRL2 0x14->0x10, CLOCK 0x18->0x14, USER..MISO_DLEN 0x1c-0x2c -> 0x18-0x28,
    // W0..W15 0x80-0xbc -> 0x58-0x94. Not remapped: the classic CACHE_SCTRL (+0x54) lands on the shared
    // FSM slot and reads 0, and SRAM_CMD..SLV_RD_BIT (+0x58-0x64) land on W0..W3; both are used only by
    // the PSRAM path on SPI0, which the classic model does not have.
    fn off(off: u32) -> u32 {
        match off {
            0x10 => 0x2c,
            0x14 => 0x10,
            0x18 => 0x14,
            0x1c => 0x18,
            0x20 => 0x1c,
            0x24 => 0x20,
            0x28 => 0x24,
            0x2c => 0x28,
            0x80..=0xbc => off - 0x28,
            _ => off,
        }
    }
}
impl Device for ClassicSpi {
    fn read(&mut self, off: u32) -> u32 {
        self.0.read(Self::off(off))
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if self.0.write(Self::off(off), v) && self.0.is_spi1 {
            WriteEffect::SPI_EXEC
        } else {
            WriteEffect::NONE
        }
    }
    fn debug(&mut self, on: bool) {
        self.0.log = on;
    }
}

// IDF v5.5.4 register/soc/io_mux_reg.h:41-66: pulls bits 7/8, input bit 9, function bits 12-14.
// Pad offsets and function selectors: io_mux_reg.h:90-359.
const IOMUX: u32 = 0x1000;
const IOMUX_OFFSETS: [u32; 40] = [
    0x44, 0x88, 0x40, 0x84, 0x48, 0x6c, 0x60, 0x64,
    0x68, 0x54, 0x58, 0x5c, 0x34, 0x38, 0x30, 0x3c,
    0x4c, 0x50, 0x70, 0x74, 0x78, 0x7c, 0x80, 0x8c,
    0x90, 0x24, 0x28, 0x2c, u32::MAX, u32::MAX, u32::MAX, u32::MAX,
    0x1c, 0x20, 0x14, 0x18, 0x04, 0x08, 0x0c, 0x10,
];

#[cfg(test)]
#[path = "gpio_irq_tests.rs"]
mod gpio_irq_tests;

/// Classic GPIO and IO_MUX share pad state, so they are one device mounted at both blocks.
pub struct ClassicGpio {
    pub gpio: Gpio,
    io_mux: RegRam,
    /// Pad registers re-laid for the shared pin router: pad n at 4 + 4n, with classic MCU_SEL 2
    /// (GPIO matrix) stored as function 1 and classic function 1 as 2, as on S3/C3/C6.
    route_mux: RegRam,
    external: u64,
    signal_out: [bool; 256],
    signal_oe: [bool; 256],
    rtc_pads: u64,
    // Derived on pad/configuration changes, never by the IRQ polling path.
    level_pins: u64,
    level_status: u64,
    irq_routes: [u64; 4],
    #[cfg(test)]
    level_refreshes: usize,
}
impl ClassicGpio {
    pub(crate) fn routes(&self) -> esp_soc::pins::PinRoutes<'_> {
        esp_soc::pins::ChipPins::ESP32.routes(&self.gpio, &self.route_mux)
    }
    fn new() -> Self {
        Self {
            gpio: Gpio::new(),
            io_mux: RegRam::new(),
            route_mux: RegRam::new(),
            external: 0,
            signal_out: [false; 256],
            signal_oe: [false; 256],
            rtc_pads: 0,
            level_pins: 0,
            level_status: 0,
            irq_routes: [0; 4],
            #[cfg(test)]
            level_refreshes: 0,
        }
    }
    // Classic PINn and matrix select banks precede the shared S3 offsets.
    fn gpio_off(off: u32) -> u32 {
        match off {
            0x88..=0x124 => off - 0x14,
            0x130..=0x52c => off + 0x24,
            0x530..=0x5cc => off + 0x24,
            _ => off,
        }
    }
    pub(crate) fn mux(&self, pin: usize) -> u32 {
        IOMUX_OFFSETS
            .get(pin)
            .filter(|&&off| off != u32::MAX)
            .map_or(0, |&off| self.io_mux.read(off))
    }
    fn route_value(v: u32) -> u32 {
        let function = match (v >> 12) & 7 { 1 => 2, 2 => 1, f => f };
        (v & !(7 << 12)) | (function << 12)
    }
    fn matrix_pad(&self, pin: usize) -> bool {
        (self.mux(pin) >> 12) & 7 == 2
    }
    fn direct_signal(pin: usize) -> Option<usize> {
        Some(match pin {
            2 => 13,
            4 => 12,
            12 => 9,
            13 => 10,
            14 => 8,
            15 => 11,
            5 => 68,
            18 => 63,
            19 => 64,
            21 => 66,
            22 => 67,
            23 => 65,
            _ => return None,
        })
    }
    fn direct_pin(signal: usize) -> Option<usize> {
        (0..40).find(|&pin| Self::direct_signal(pin) == Some(signal))
    }
    fn input_enabled(&self, pin: usize) -> bool {
        self.rtc_pads & (1 << pin) == 0 && self.mux(pin) & (1 << 9) != 0
    }
    fn driven(&self, pin: usize) -> Option<bool> {
        if pin >= 34 || self.rtc_pads & (1 << pin) != 0 {
            return None;
        }
        if (self.mux(pin) >> 12) & 7 == 1 {
            return Self::direct_signal(pin)
                .filter(|&signal| self.signal_oe[signal])
                .map(|signal| self.signal_out[signal]);
        }
        if !self.matrix_pad(pin) {
            return None;
        }
        let cfg = self.gpio.func_out_sel[pin];
        let sig = (cfg & 0x1ff) as usize;
        let gpio_oe = self.gpio.enable & (1 << pin) != 0;
        let (mut level, mut oe) = if sig == 256 {
            (self.gpio.out & (1 << pin) != 0, gpio_oe)
        } else {
            (
                self.signal_out[sig],
                if cfg & (1 << 10) != 0 {
                    gpio_oe
                } else {
                    self.signal_oe[sig]
                },
            )
        };
        level ^= cfg & (1 << 9) != 0;
        oe ^= cfg & (1 << 11) != 0;
        oe.then_some(level)
    }
    fn pad_level(&self, pin: usize) -> bool {
        self.driven(pin)
            .unwrap_or(self.gpio.input & (1 << pin) != 0)
    }
    fn driven_levels(&self) -> [Option<bool>; 40] {
        std::array::from_fn(|pin| self.driven(pin))
    }
    fn note_driven(&mut self, old: [Option<bool>; 40]) {
        let mut changed = false;
        for (pin, was) in old.into_iter().enumerate() {
            let now = self.driven(pin);
            if now != was {
                self.gpio.changes.push((pin as u8, now.unwrap_or(false)));
                changed = true;
            }
        }
        if changed {
            self.refresh_level_status();
        }
    }
    fn input_word(&self) -> u64 {
        (0..40).fold(0, |word, pin| {
            word | ((self.input_enabled(pin) && self.pad_level(pin)) as u64) << pin
        })
    }
    fn refresh_level_status(&mut self) {
        #[cfg(test)]
        { self.level_refreshes += 1; }
        let mut status = 0;
        let mut pins = self.level_pins;
        while pins != 0 {
            let pin = pins.trailing_zeros() as usize;
            pins &= pins - 1;
            let typ = (self.gpio.pin[pin] >> 7) & 7;
            if self.input_enabled(pin)
                && ((typ == 4 && !self.pad_level(pin)) || (typ == 5 && self.pad_level(pin)))
            {
                status |= 1 << pin;
            }
        }
        self.level_status = status;
    }
    fn raw_status(&self) -> u64 {
        (self.gpio.status & ((1 << 40) - 1)) | self.level_status
    }
    fn irq_status(&self, core: usize, nmi: bool) -> u64 {
        let route = match (core, nmi) {
            (0, false) => 2,
            (0, true) => 3,
            (1, false) => 0,
            (1, true) => 1,
            _ => return 0,
        };
        self.raw_status() & self.irq_routes[route]
    }
    pub fn irq(&self, core: usize, nmi: bool) -> bool {
        self.irq_status(core, nmi) != 0
    }
    pub fn set_input(&mut self, pin: u8, level: bool) -> bool {
        if pin >= 40 {
            return false;
        }
        self.external |= 1 << pin;
        let status = self.gpio.status;
        let irq = self.gpio.set_input(pin, level);
        let irq = if self.rtc_pads & (1 << pin) != 0 { self.gpio.status = status; false } else { irq };
        self.refresh_level_status();
        irq
    }
    pub fn release_input(&mut self, pin: u8) {
        if pin >= 40 { return; }
        self.external &= !(1 << pin);
        self.gpio.set_input(pin, true);
        self.sync_pull(pin as usize);
        self.refresh_level_status();
    }
    pub fn restore_inputs(&mut self, old: &Self) {
        for pin in 0..40 { if old.external & (1 << pin) != 0 { self.set_input(pin, old.gpio.input & (1 << pin) != 0); } }
    }
    fn sync_pull(&mut self, pin: usize) {
        if pin >= 34 || (self.external | self.rtc_pads) & (1 << pin) != 0 {
            return;
        }
        let cfg = self.mux(pin);
        if cfg & (1 << 8) != 0 {
            self.gpio.set_input(pin as u8, true);
        } else if cfg & (1 << 7) != 0 {
            self.gpio.set_input(pin as u8, false);
        }
    }
    /// Physical matrix input, retaining inversion. IDF v4.4.7 gpio_reg.h:
    /// GPIO_FUNCm_IN_SEL_CFG source bits 0..5, invert bit 6, matrix-select bit 7.
    /// Unlike the non-inverted input_pin helper, DVP data can use inversion.
    pub(crate) fn matrix_input_route(&self, signal: usize) -> Option<(u8, bool)> {
        let cfg = *self.gpio.func_in_sel.get(signal)?;
        let pin = (cfg & 0x3f) as usize;
        (cfg & (1 << 7) != 0 && pin < 40 && self.matrix_pad(pin) && self.input_enabled(pin))
            .then_some((pin as u8, cfg & (1 << 6) != 0))
    }
    pub fn input_pin(&self, signal: usize) -> Option<u8> {
        esp_soc::pins::ChipPins::ESP32.routes(&self.gpio, &self.route_mux).input_pin(signal)
            .or_else(|| Self::direct_pin(signal).filter(|&pin| (self.mux(pin) >> 12) & 7 == 1 && self.input_enabled(pin)).map(|p| p as u8))
    }
    pub(crate) fn low_outputs(&self) -> u64 {
        (0..34).filter(|&pin| self.driven(pin) == Some(false)).fold(0, |mask, pin| mask | (1 << pin))
    }
    pub fn output_pins(&self, signal: usize) -> u64 {
        (0..34).filter(|&pin| {
            let cfg = self.gpio.func_out_sel[pin];
            (self.matrix_pad(pin) && cfg & 0x3ff == signal as u32 && self.driven(pin).is_some())
                || ((self.mux(pin) >> 12) & 7 == 1 && Self::direct_signal(pin) == Some(signal))
        }).fold(0, |mask, pin| mask | (1 << pin))
    }
    /// Resolve a peripheral input routed through GPIO_FUNCm_IN_SEL_CFG.
    pub fn signal_input(&self, signal: usize) -> Option<bool> {
        if let Some(pin) = Self::direct_pin(signal) {
            if (self.mux(pin) >> 12) & 7 == 1 && self.input_enabled(pin) {
                return Some(self.pad_level(pin));
            }
        }
        let cfg = *self.gpio.func_in_sel.get(signal)?;
        if cfg & (1 << 7) == 0 {
            return None;
        }
        let source = (cfg & 0x3f) as usize;
        let level = match source {
            0x30 => false,
            0x38 => true,
            pin @ 0..=39 if self.matrix_pad(pin) && self.input_enabled(pin) => self.pad_level(pin),
            _ => return None,
        };
        Some(level ^ (cfg & (1 << 6) != 0))
    }
    /// Set one peripheral output and its output-enable signal for GPIO matrix routing.
    pub fn set_output_signal(&mut self, signal: usize, level: bool, enable: bool) {
        if signal >= self.signal_out.len() {
            return;
        }
        let old = self.driven_levels();
        self.signal_out[signal] = level;
        self.signal_oe[signal] = enable;
        self.note_driven(old);
    }
    pub fn output_pin(&self, signal: usize) -> Option<(u8, bool)> {
        let pins = self.output_pins(signal) | self.output_pins(signal | (1 << 9));
        if pins == 0 { return None; }
        let pin = pins.trailing_zeros() as u8;
        Some((pin, self.gpio.func_out_sel[pin as usize] & (1 << 9) != 0))
    }
}
impl Device for ClassicGpio {
    fn read(&mut self, off: u32) -> u32 {
        if off >= IOMUX {
            return self.io_mux.read(off - IOMUX);
        }
        match off {
            0x3c => self.input_word() as u32,
            0x40 => (self.input_word() >> 32) as u32,
            0x44 => self.raw_status() as u32,
            0x50 => (self.raw_status() >> 32) as u32,
            0x60 => self.irq_status(1, false) as u32,
            0x64 => self.irq_status(1, true) as u32,
            0x68 => self.irq_status(0, false) as u32,
            0x6c => self.irq_status(0, true) as u32,
            0x74 => (self.irq_status(1, false) >> 32) as u32,
            0x78 => (self.irq_status(1, true) >> 32) as u32,
            0x7c => (self.irq_status(0, false) >> 32) as u32,
            0x80 => (self.irq_status(0, true) >> 32) as u32,
            _ => self.gpio.read(Self::gpio_off(off)),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let old = self.driven_levels();
        if off >= IOMUX {
            let off = off - IOMUX;
            self.io_mux.write(off, v);
            if let Some(pin) = IOMUX_OFFSETS.iter().position(|&candidate| candidate == off) {
                self.route_mux.write(4 + 4 * pin as u32, Self::route_value(v));
                self.sync_pull(pin);
                self.refresh_level_status();
            }
        } else {
            let pending = self.gpio.changes.len();
            // Translation stays inside the 4 KiB GPIO window. Make that bound
            // explicit so shared Gpio::write retains fixed-chip register indexing.
            let translated = Self::gpio_off(off) & 0xfff;
            self.gpio.write(translated, v);
            self.gpio.changes.truncate(pending);
            self.gpio.out &= (1 << 40) - 1;
            self.gpio.enable &= (1 << 34) - 1;
            self.gpio.status &= (1 << 40) - 1;
            // Shared GPIO output/enable writes also resolve the fallback input
            // word, even when IO_MUX does not select a driven digital output.
            if matches!(translated, 0x4 | 0x8 | 0xc | 0x10 | 0x14 | 0x18 | 0x20 | 0x24 | 0x28 | 0x2c | 0x30 | 0x34) {
                self.refresh_level_status();
            }
            if (0x74..=0x110).contains(&translated) {
                let pin = ((translated - 0x74) / 4) as usize;
                let bit = 1u64 << pin;
                let cfg = self.gpio.pin[pin];
                self.level_pins = (self.level_pins & !bit)
                    | if matches!((cfg >> 7) & 7, 4 | 5) { bit } else { 0 };
                // IDF v5.5.4 components/soc/esp32/register/soc/gpio_reg.h:303,322:
                // INT_ENA bits 13..17, INT_TYPE bits 7..9; identical in v4.4.7
                // components/soc/esp32/include/soc/gpio_reg.h:311,330.
                // Preserve the existing model's
                // APP/APP_NMI/PRO/PRO_NMI routes (bits 13..16).
                // INT_ENA stays independent of INT_TYPE: software-set pending
                // bits remain visible with type 0, as before.
                for (route, mask) in self.irq_routes.iter_mut().enumerate() {
                    *mask = (*mask & !bit) | if cfg & (1 << (13 + route)) != 0 { bit } else { 0 };
                }
                self.refresh_level_status();
            }
        }
        self.note_driven(old);
        WriteEffect::NONE
    }
}

// The classic UART adds UART_MEM_RX_STATUS (uart_reg.h, UART_MEM_RX_STATUS_REG at 0x60):
// bits 13-23 hold the receive FIFO write pointer, modelled as the queued count mod 128.
pub struct ClassicUart(pub Uart);
impl ClassicUart {
    const MEM_RX_STATUS: u32 = 0x60;
    fn new(layout: UartLayout) -> Self {
        Self(Uart::new(layout))
    }
}
impl std::ops::Deref for ClassicUart {
    type Target = Uart;
    fn deref(&self) -> &Uart {
        &self.0
    }
}
impl std::ops::DerefMut for ClassicUart {
    fn deref_mut(&mut self) -> &mut Uart {
        &mut self.0
    }
}
impl Device for ClassicUart {
    fn read(&mut self, off: u32) -> u32 {
        if off == Self::MEM_RX_STATUS {
            ((self.0.rx_pending() % 128) as u32) << 13
        } else {
            self.0.read(off)
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        self.0.write(off, v);
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 {
        self.0.irq() as u64
    }
}

// RTC_CNTL uses the classic register layout; the shared device owns watchdog state.
pub struct ClassicRtc(pub RtcCntl);
impl ClassicRtc {
    fn new() -> Self {
        let mut rtc = RtcCntl::with_wdt_base(0x8c);
        // IDF v5.5.4 rtc_cntl_reg.h:505-523: WAKEUP_ENA resets to 12.
        rtc.ram.write(0x34, 12 << 11);
        Self(rtc)
    }
    /// Synchronous RTC FAST CRC. IDF v5.5.4 esp_rom/esp32/include/esp32/rom/rtc.h:172-181
    /// defines word address and LEN+1 words. The hardware CRC parameters are undocumented:
    /// use CRC-32/ISO-HDLC as a deterministic approximation, not a silicon-verified result.
    #[inline(never)] // Keep the CRC loop out of the bus store/MMIO dispatcher.
    pub(crate) fn memory_crc(&mut self, memory: &[u8]) {
        let conf = self.0.ram.read(0x100);
        let start = ((conf >> 9) & 0x7ff) as usize * 4;
        let len = (((conf >> 20) & 0x7ff) as usize + 1) * 4;
        let crc = if start + len <= memory.len() {
            esp_soc::wifi::fcs(&memory[start..start + len])
        } else {
            // Wrap the 11-bit word address; overflow behavior is an unverified model choice.
            let data: Vec<u8> = (0..len)
                .map(|offset| memory[(start + offset) & 0x1fff])
                .collect();
            esp_soc::wifi::fcs(&data)
        };
        self.0.ram.write(0x104, crc);
        self.0.ram.write(0x100, conf | (1 << 31));
    }
    fn off(off: u32) -> u32 {
        match off {
            0x34 => 0x38,
            0x38 => 0x34, // keep WAKEUP_STATE separate from shared RESET_STATE
            0xb0..=0xbc => off + 0x10,
            _ => off,
        }
    }
}
impl Device for ClassicRtc {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x40 => self.0.ram.read(0x44),
            0x44 => self.0.ram.read(0x44) & self.0.ram.read(0x3c),
            _ => self.0.read(Self::off(off)),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if off == 0x48 {
            self.0.ram.write(0x44, self.0.ram.read(0x44) & !v);
        } else if off == 0x38 {
            // WAKEUP_CAUSE[10:0] is read-only; EXT0 wake sets it in the bus.
            self.0.ram.write(0x34, (v & (0xfff << 11)) | (self.0.ram.read(0x34) & 0x7ff));
        } else if off == 0x100 {
            // IDF v5.5.4 register/soc/rtc_cntl_reg.h:2017-2039: START bit 8,
            // FINISH bit 31, result +0x104. Completion is produced by the bus on START;
            // clearing START clears FINISH. Guest writes cannot forge completion/results.
            self.0.ram.write(off, v & !(1 << 31));
        } else if off == 0x104 {
            // CRC result is read-only.
        } else {
            // The mask states the 4 KiB block range S3/C3/C6 callers already prove, so the shared
            // RtcCntl::write keeps main's code (no extra index masking).
            self.0.write(Self::off(off) & 0xfff, v);
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 {
        (self.0.ram.read(0x44) & self.0.ram.read(0x3c) != 0) as u64
    }
    fn clock(&self) -> Option<ClockDomain> {
        Device::clock(&self.0)
    }
    fn tick(&mut self, ticks: u64) {
        Device::tick(&mut self.0, ticks);
    }
}

// Read-only factory eFuses include revision bits and the MAC address.
pub struct ClassicEfuse {
    pub ram: RegRam,
    cmd: u32,
}
impl ClassicEfuse {
    fn new(mac: [u8; 6]) -> Self {
        let mut r = RegRam::new();
        r.write(0x04, u32::from_be_bytes([mac[2], mac[3], mac[4], mac[5]]));
        let mut crc = 0u8;
        for byte in mac {
            crc ^= byte;
            for _ in 0..8 { crc = (crc >> 1) ^ if crc & 1 != 0 { 0x8c } else { 0 }; }
        }
        r.write(0x08, (crc as u32) << 16 | (mac[0] as u32) << 8 | mac[1] as u32);
        r.write(0x0c, 1 << 15); // ECO1+
        r.write(0x14, 1 << 20); // Revision 2; the loaded ECO3 ROM is selected independently.
        // IDF v5.5.4 register/soc/efuse_reg.h:169,177-183: EFUSE_BLK0_RDATA4_REG EFUSE_RD_ADC_VREF
        // [12:8]. 0x10 is nominal 1100 mV Vref, sign-magnitude negative zero.
        r.write(0x10, 0x10 << 8);
        Self { ram: r, cmd: 0 }
    }
}
impl Device for ClassicEfuse {
    fn read(&mut self, off: u32) -> u32 {
        if off == 0x104 {
            return std::mem::take(&mut self.cmd);
        }
        self.ram.read(off)
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if off == 0x104 {
            self.cmd = v;
        } else {
            self.ram.write(off, v);
        }
        WriteEffect::NONE
    }
}

pub struct Peripherals {
    pub dport: Dport,
    pub uart: [ClassicUart; 3],
    pub spi0: ClassicSpi,
    pub spi1: ClassicSpi,
    pub spi: [ClassicGpSpi; 2],
    pub gpio: ClassicGpio,
    pub ledc: ClassicLedc,
    pub rmt: ClassicRmt,
    pub rtc: ClassicRtc,
    pub efuse: ClassicEfuse,
    pub sha: ClassicSha,
    pub timg: [ClassicTimer; 2],
    pub frc2: crate::frc::Frc2,
    pub i2c: [esp_periph::i2c::I2c<16>; 2],
    pub misc: Misc,
    pub spi_exec: bool,
    clock: ClockTree<1>,
    // Sixth-tick APB phase survives changes of the selected clock source.
    apb_phase: u64,
    pub aes: ClassicAes,
    pub rsa: ClassicRsa,
    pub(crate) crypto_enabled: [bool; 3],
    pub wifi: crate::wifi::WifiMac,
    pub analog: crate::wifi::Analog,
    pub fe: crate::wifi::FrontEnd,
    pub adc: ClassicAdc,
    pub i2s: [crate::i2s::ClassicI2s; 2],
}

device_set! { Peripherals; clock: (clock) CPU_HZ, [(ClockDomain::RtcSlow, 1600)];
    0x00 "DPORT" (dport) => [SRC_FROM_CPU0, SRC_FROM_CPU0 + 1, SRC_FROM_CPU0 + 2, SRC_FROM_CPU0 + 3];
    0x01 "AES" (aes) => [];
    0x02 "RSA" (rsa) => [SRC_RSA];
    0x03 "SHA" (sha) => [];
    0x40 "UART0" (uart[0]) => [SRC_UART0];
    0x42 "SPI1" (spi1) => [];
    0x43 "SPI0" (spi0) => [];
    0x44 "GPIO" (gpio) => [];
    0x4f "I2S0" (i2s[0]) => [32];
    0x6d "I2S1" (i2s[1]) => [33];
    0x46 "FE" (fe) => [];
    // ESP-IDF v3.2.3 soc.h:92,384: FRC2 base and ETS_TIMER2_INTR_SOURCE.
    0x47 "FRC2" (frc2) => [57];
    0x4e "I2C_MST" (analog) => [];
    0x73 "WIFI_MAC" (wifi) => [SRC_WIFI_MAC];
    0x74 "WIFI_MAC" alias (wifi) delta 0x1000 => [];
    0x75 "WIFI_MAC" alias (wifi) delta 0x2000 => [];
    0x48 "RTC_IO" alias (adc.rtc_io) delta -0x400 @ 0x400..=0x4ff => [];
    0x48 "SENS" alias (adc) delta -0x800 @ 0x800..=0x8ff => [];
    0x48 "RTCCNTL" (rtc) => [SRC_RTC_CORE];
    0x49 "IO_MUX" alias (gpio) delta 0x1000 => [];
    0x50 "UART1" (uart[1]) => [SRC_UART1];
    0x53 "I2C0" (i2c[0]) => [SRC_I2C0];
    0x56 "RMT" (rmt) => [SRC_RMT];
    0x59 "LEDC" (ledc) => [SRC_LEDC];
    0x5a "EFUSE" (efuse) => [];
    0x5f "TIMG0" (timg[0]) => [SRC_TG0_T0, SRC_TG0_T1, SRC_TG0_WDT, SRC_TG0_LACT, SRC_TG0_T0_EDGE, SRC_TG0_T0_EDGE + 1, SRC_TG0_T0_EDGE + 2, SRC_TG0_T0_EDGE + 3];
    0x60 "TIMG1" (timg[1]) => [SRC_TG1_T0, SRC_TG1_T1, SRC_TG1_WDT, SRC_TG1_LACT, SRC_TG1_T0_EDGE, SRC_TG1_T0_EDGE + 1, SRC_TG1_T0_EDGE + 2, SRC_TG1_T0_EDGE + 3];
    0x64 "SPI2" (spi[0]) => [SRC_SPI2, SRC_SPI2_DMA];
    0x65 "SPI3" (spi[1]) => [SRC_SPI3, SRC_SPI3_DMA];
    0x67 "I2C1" (i2c[1]) => [SRC_I2C1];
    0x6e "UART2" (uart[2]) => [SRC_UART2];
}

impl DeviceSet for Peripherals {
    const BASE: u32 = PERIPH_BASE;
    fn block_name(block: u32) -> &'static str {
        Self::block_name(block)
    }
    fn misc(&self) -> &Misc {
        &self.misc
    }
    fn misc_mut(&mut self) -> &mut Misc {
        &mut self.misc
    }
}

impl Peripherals {
    pub fn new(mac: [u8; 6]) -> Self {
        let uart = UartLayout {
            thrhd_mask: 0x7f,
            rxfifo_rst: 1 << 17,
            rxfifo_cnt_mask: 0xff,
        };
        let mut gpio = ClassicGpio::new();
        gpio.gpio.strap = 0x13; // normal SPI-fast-flash boot, with ROM messages enabled
        let mut p = Self {
            dport: Dport::new(),
            uart: [
                ClassicUart::new(uart),
                ClassicUart::new(uart),
                ClassicUart::new(uart),
            ],
            spi0: ClassicSpi::new(false),
            spi1: ClassicSpi::new(true),
            spi: [ClassicGpSpi::new(), ClassicGpSpi::new()],
            gpio,
            ledc: ClassicLedc::new(),
            rmt: ClassicRmt::new(),
            rtc: ClassicRtc::new(),
            efuse: ClassicEfuse::new(mac),
            sha: ClassicSha::new(),
            timg: [ClassicTimer::new(0), ClassicTimer::new(1)],
            frc2: crate::frc::Frc2::default(),
            i2c: [esp_periph::i2c::I2c::new_classic(), esp_periph::i2c::I2c::new_classic()],
            misc: Misc::new(),
            spi_exec: false,
            clock: Self::new_clock(),
            apb_phase: 0,
            aes: ClassicAes::new(),
            rsa: ClassicRsa::new(),
            crypto_enabled: [false; 3],
            wifi: crate::wifi::WifiMac::default(),
            analog: crate::wifi::Analog::default(),
            fe: crate::wifi::FrontEnd::default(),
            adc: ClassicAdc::new(),
            i2s: [crate::i2s::ClassicI2s::for_port(0), crate::i2s::ClassicI2s::for_port(1)],
        };
        for uart in &mut p.uart { uart.write(0x20, 1 << 27); }
        p.sync_crypto();
        p
    }
    pub fn uart_route(&self, port: usize) -> esp_soc::uart::UartRoute {
        esp_soc::uart::UartPins::ESP32.route(port, &self.gpio.gpio, &self.gpio.route_mux, self.uart[port].classic_baud())
    }
    pub fn uart_pin_input(&mut self, input: &esp_soc::uart::UartInput) {
        let routes = std::array::from_fn::<_, 3, _>(|port| self.uart_route(port));
        // The shared helper takes plain UARTs; hand it each wrapped port with its own route.
        for (uart, route) in self.uart.iter_mut().zip(&routes) {
            esp_soc::uart::uart_pin_input(std::slice::from_mut(&mut uart.0), input, std::slice::from_ref(route));
        }
    }
    pub fn block_name(block: u32) -> &'static str {
        match block {
            0x01 => "AES",
            0x02 => "RSA",
            0x03 => "SHA",
            0x00..=0x13 => "DPORT",
            0x40 => "UART0",
            0x42 => "SPI1",
            0x43 => "SPI0",
            0x44 => "GPIO",
            0x46 => "FE",
            0x4e => "I2C_MST",
            0x73..=0x75 => "WIFI_MAC",
            0x48 => "RTCCNTL",
            0x49 => "IO_MUX",
            0x4f => "I2S0",
            0x6d => "I2S1",
            0x50 => "UART1",
            0x5a => "EFUSE",
            0x5f => "TIMG0",
            0x60 => "TIMG1",
            0x6e => "UART2",
            _ => "?",
        }
    }
    /// SENS and RTC_IO sit in the RTC domain: like RTC_CNTL's register RAM they keep their
    /// values, including touch-timer and DAC power state, across a guest reboot.
    pub(crate) fn restore_rtc_analog(&mut self, adc: ClassicAdc) {
        self.adc = adc;
        self.gpio.rtc_pads = self.adc.rtc_pad_mask();
        self.gpio.refresh_level_status();
    }
    pub fn read32(&mut self, addr: u32) -> u32 {
        if (0x3ff0_00ec..=0x3ff0_0100).contains(&addr) {
            let (core, word) = if addr < 0x3ff0_00f8 {
                (0, ((addr - 0x3ff0_00ec) / 4) as usize)
            } else {
                (1, ((addr - 0x3ff0_00f8) / 4) as usize)
            };
            return self.source_status(core)[word];
        }
        mmio::read32(self, addr)
    }
    pub fn write32(&mut self, addr: u32, mut v: u32) {
        let crypto = match addr {
            0x3ff0_1000 => Some(0),
            0x3ff0_3080..=0x3ff0_30b8 => Some(1),
            0x3ff0_2808 | 0x3ff0_2810 => Some(2),
            _ => None,
        };
        if crypto.is_some_and(|engine| !self.crypto_enabled[engine]) { return; }
        match addr {
            0x3ff4_8400..=0x3ff4_84ff => {
                let old = self.gpio.driven_levels();
                mmio::write32(self, addr, v);
                self.gpio.rtc_pads = self.adc.rtc_pad_mask();
                self.gpio.refresh_level_status();
                self.gpio.note_driven(old);
                return;
            }
            // IDF v5.5.4 register/soc/rtc_cntl_reg.h:246,277-279: RTC_CNTL_STATE0_REG bit 23 is
            // RTC_CNTL_TOUCH_SLP_TIMER_EN.
            0x3ff4_8018 => self.adc.touch_timer = v & (1 << 23) != 0,
            _ => {}
        }
        let i2c = match (addr - PERIPH_BASE) >> 12 {
            0x53 => Some(0),
            0x67 => Some(1),
            _ => None,
        };
        if let Some(bus) = i2c.filter(|_| addr & 0xfff == 0x04 && v & (1 << 5) != 0) {
            let (scl, sda) = I2C_SIGNALS[bus];
            // Reuse the shared physical route decoder before the synchronous classic
            // command list. IDF v5.5.4 esp32/include/soc/gpio_sig_map.h:73-76,200-203
            // gives I2CEXT0/1 SCL/SDA input/output signals; no new addressing path.
            let routes = esp_soc::pins::ChipPins::ESP32.routes(&self.gpio.gpio, &self.gpio.route_mux);
            let pins = routes.i2c_pin(sda).zip(routes.i2c_pin(scl));
            self.i2c[bus].set_pins(pins);
            if self.gpio.signal_input(scl) != Some(true) || self.gpio.signal_input(sda) != Some(true) {
                self.i2c[bus].int_raw |= esp_periph::i2c::INT_TIMEOUT;
                v &= !(1 << 5);
            }
        }
        // IDF v5.5.4 dport_reg.h:1064,1069: DPORT_CORE_RST_EN_REG bit 2 is DPORT_WIFIMAC_RST.
        if addr == 0x3ff0_00d0 && v & (1 << 2) != 0 { self.wifi.reset(); }
        if addr == 0x3ff0_00c4 && v & (1 << 11) != 0 {
            self.ledc = ClassicLedc::new();
        }
        if addr == 0x3ff0_00c4 && v & (1 << 9) != 0 {
            self.rmt = ClassicRmt::new();
        }
        if mmio::write32(self, addr, v).contains(WriteEffect::SPI_EXEC) {
            self.spi_exec = true;
        }
        if matches!(addr, 0x3ff0_001c | 0x3ff0_0020 | 0x3ff0_0490) { self.sync_crypto(); }
        if let Some(bus) = i2c {
            let (scl, sda) = I2C_SIGNALS[bus];
            self.gpio.set_output_signal(scl, true, false);
            self.gpio.set_output_signal(sda, true, false);
        }
        if matches!(addr, 0x3ff0_00c0 | 0x3ff0_00c4) {
            // IDF v5.5.4 register/soc/dport_reg.h:947,965 (I2S clocks),
            // 982,1000 (resets). Cache on writes, not in the per-tick DMA loop.
            for (port, bit) in [4, 21].into_iter().enumerate() {
                // Camera DMA must not survive a peripheral reset. Keep the
                // established PCM reset behavior unchanged in this slice.
                if self.dport.ram.read(0xc4) & (1 << bit) != 0 && self.i2s[port].camera_rx().is_some() {
                    self.i2s[port] = crate::i2s::ClassicI2s::for_port(port);
                }
                self.i2s[port].clock_enabled = self.dport.ram.read(0xc0) & (1 << bit) != 0
                    && self.dport.ram.read(0xc4) & (1 << bit) == 0;
            }
            self.ledc.clock_enabled = self.dport.ram.read(0xc0) & (1 << 11) != 0
                && self.dport.ram.read(0xc4) & (1 << 11) == 0;
            self.rmt.clock_enabled = self.dport.ram.read(0xc0) & (1 << 9) != 0
                && self.dport.ram.read(0xc4) & (1 << 9) == 0;
        }
        if matches!(addr >> 12, 0x3ff00 | 0x3ff59) { self.sync_ledc_outputs(); }
        if matches!(addr, 0x3ff0_00cc | 0x3ff0_00d0) {
            // IDF v5.5.4 dport_reg.h:1040-1043,1064,1069.
            self.wifi.clock_enabled = self.dport.ram.read(0xcc) & 0x406 == 0x406
                && self.dport.ram.read(0xd0) & (1 << 2) == 0;
        }
        if matches!(addr >> 12, 0x3ff00 | 0x3ff56) { self.sync_rmt_outputs(); }
    }
    /// Light sleep gates the digital clocks. Preserve fractional RTC phase across
    /// entry/exit, and respect WDT_PAUSE_IN_SLP (IDF v3.2.3 rtc_cntl_reg.h bit 7).
    /// The watchdog remains enabled and resumes its count after wake.
    pub(crate) fn tick_light_sleep(&mut self, cycles: u64) {
        let rtc = &mut self.rtc.0;
        self.clock.advance(&[(ClockDomain::RtcSlow, 1600)], cycles, |_, ticks| {
            rtc.slow_ticks += ticks;
            if rtc.ram.read(0x8c) & (1 << 7) == 0 { rtc.wdt_tick(ticks); }
        });
    }
    pub(crate) fn sleep_timer_cycles(&self, ticks: u64) -> u64 {
        ticks.saturating_mul(1600).saturating_sub(self.clock.cycles() % 1600).max(1)
    }
    pub fn tick(&mut self, cycles: u64) -> bool {
        let gpio_before = self.gpio.raw_status();
        let mut irq_changed = Dispatch::tick(self, cycles);
        let phase = self.apb_phase + cycles * (6 / self.apb_divider());
        self.apb_phase = phase % 6;
        let ticks = phase / 6;
        // These adapters use the manually scaled APB clock, not Dispatch's clock.
        // Compare only the devices being advanced; never scan all sources here.
        for timer in &mut self.timg {
            let before = timer.irq_sources();
            timer.tick(ticks);
            irq_changed |= before != timer.irq_sources();
        }
        let frc_before = self.frc2.irq_sources();
        self.frc2.tick(ticks);
        irq_changed |= frc_before != self.frc2.irq_sources();
        let ledc_before = self.ledc.irq_sources();
        let rmt_before = self.rmt.irq_sources();
        if self.ledc.clock() == Some(ClockDomain::Apb) { self.ledc.tick(ticks); }
        if self.rmt.clock() == Some(ClockDomain::Apb) { self.rmt.tick(ticks); }
        irq_changed |= ledc_before != self.ledc.irq_sources();
        irq_changed |= rmt_before != self.rmt.irq_sources();
        self.sync_ledc_outputs();
        self.sync_rmt_outputs();
        // Routed PWM/RMT outputs can also change GPIO edge/level interrupts.
        irq_changed | (gpio_before != self.gpio.raw_status())
    }
    /// Reference ticks per APB tick. IDF v5.5.4
    /// components/esp_hw_support/port/esp32/rtc_clk.c:373-389 sets APB=CPU
    /// on 40 MHz XTAL; components/esp_hw_support/include/esp_private/systimer.h:26
    /// uses LACT prescaler 20 there (40 on 80 MHz PLL APB).
    /// rtc_clk.c:391-404 also sets APB=CPU for RC_FAST. RC_FAST/APLL
    /// frequency remains approximated as XTAL (40 MHz); only PLL uses 80 MHz APB.
    fn apb_divider(&self) -> u64 {
        if (self.rtc.0.ram.read(0x70) >> 27) & 3 != 1 { 6 } else { 3 }
    }
    /// Half reference ticks per CPU clock. IDF v5.5.4 clk_tree_ll.h:543-626,
    /// dport_reg.h:154-174 and rtc_cntl_reg.h:863-881: XTAL=40 MHz;
    /// PLL CPUPERIOD_SEL 0/1/2 selects 80/160/240 MHz. Unsupported clock
    /// sources/reserved PLL selections use XTAL; APLL/RC/dividers are not modeled.
    pub(crate) fn cpu_scale_half(&self) -> u64 {
        if (self.rtc.0.ram.read(0x70) >> 27) & 3 == 1 {
            match self.dport.ram.read(0x3c) & 3 { 0 => 6, 1 => 3, 2 => 2, _ => 12 }
        } else { 12 }
    }
    pub fn source_status(&self, core: usize) -> [u32; 3] {
        let all = Dispatch::source_status(self);
        self.with_gpio_sources(core, &all)
    }
    fn with_gpio_sources(&self, core: usize, all: &[u32; 4]) -> [u32; 3] {
        let mut status = [all[0], all[1], all[2]];
        if self.gpio.irq(core, false) {
            status[SRC_GPIO / 32] |= 1 << (SRC_GPIO % 32);
        }
        if self.gpio.irq(core, true) {
            status[SRC_GPIO_NMI / 32] |= 1 << (SRC_GPIO_NMI % 32);
        }
        status
    }
    pub fn cpu_lines(&self, core: usize) -> u32 {
        self.dport.cpu_lines(core, &self.source_status(core))
    }
    pub(crate) fn all_cpu_lines(&self, out: &mut [u32]) {
        // Only GPIO has per-core source enables. Sample the other live devices
        // once per presentation, rather than once per CPU.
        let all = Dispatch::source_status(self);
        for (core, lines) in out.iter_mut().enumerate() {
            *lines = self.dport.cpu_lines(core, &self.with_gpio_sources(core, &all));
        }
    }
    pub fn cycles_until_timer(&self) -> u32 {
        let mut best = self.rtc.next_deadline().map_or(u64::MAX, |t| t.saturating_sub(1) * 1600);
        for timer in &self.timg {
            if let Some(t) = timer.next_deadline() {
                best = best.min(t.saturating_sub(1).saturating_mul(self.apb_divider()));
            }
        }
        for device in [&self.ledc as &dyn Device, &self.rmt as &dyn Device, &self.frc2 as &dyn Device] {
            if let Some(t) = device.next_deadline() {
                best = best.min(t.saturating_sub(1).saturating_mul(self.apb_divider()));
            }
        }
        best.min(u64::from(u32::MAX)) as u32
    }
    #[inline]
    fn sync_ledc_outputs(&mut self) {
        let mut dirty = self.ledc.take_signal_updates();
        while dirty != 0 {
            let channel = dirty.trailing_zeros() as usize;
            dirty &= dirty - 1;
            let (level, enable) = self.ledc.signal_level(channel);
            self.gpio.set_output_signal(71 + channel, level, enable);
        }
    }
    #[inline]
    fn sync_rmt_outputs(&mut self) {
        for (channel, level) in std::mem::take(&mut self.rmt.outputs) {
            self.gpio.set_output_signal(crate::rmt::SIGNAL0 + channel, level, true);
        }
    }
    pub fn pwm_output(&self, pin: u32) -> Option<(f64, u32)> {
        let pin = pin as usize;
        if pin >= 40 || !self.gpio.matrix_pad(pin) || self.gpio.driven(pin).is_none() {
            return None;
        }
        let matrix = self.gpio.gpio.func_out_sel[pin];
        let mut pwm = self.ledc.pwm((matrix & 0x1ff) as usize)?;
        if matrix & (1 << 9) != 0 {
            pwm.1 = 65535 - pwm.1;
        }
        Some(pwm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_factory_mac_includes_rom_crc8() {
        let mut p = Peripherals::new([0x24, 0x6f, 0x28, 0, 0x11, 0x22]);
        assert_eq!(p.read32(0x3ff5_a004), 0x2800_1122);
        assert_eq!(p.read32(0x3ff5_a008), 0x00cf_246f);
    }

    #[test]
    fn rtc_mux_disconnects_digital_gpio_and_exposes_nominal_adc_vref() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff4_9024, (2 << 12) | (1 << 9));
        p.write32(0x3ff4_4594, 256);
        p.write32(0x3ff4_4024, 1 << 25);
        p.write32(0x3ff4_4008, 1 << 25);
        assert_eq!(p.read32(0x3ff4_403c) & (1 << 25), 1 << 25);

        p.write32(0x3ff4_8484, (128 << 19) | (1 << 18) | (1 << 17) | (1 << 10));
        assert_eq!(p.gpio.driven(25), None);
        assert_eq!(p.read32(0x3ff4_403c) & (1 << 25), 0);
        assert_eq!(p.adc.dac_outputs(), [(25, 1656)]);
        p.write32(0x3ff4_40ec, (2 << 7) | (1 << 15));
        p.gpio.set_input(25, true);
        assert!(!p.gpio.set_input(25, false));
        assert_eq!(p.read32(0x3ff4_4044) & (1 << 25), 0);
        p.write32(0x3ff4_8484, 0);
        assert_eq!(p.gpio.driven(25), Some(true));
        assert!(p.adc.dac_outputs().is_empty());
        assert_eq!((p.read32(0x3ff5_a010) >> 8) & 31, 0x10);
    }

    #[test]
    fn sens_and_rtc_io_accesses_reach_mmio_probes() {
        let mut p = Peripherals::new([0; 6]);
        p.misc.mmio_log = Some(Vec::new());
        p.write32(0x3ff4_8834, 3 << 12);
        let sens = p.read32(0x3ff4_8834);
        p.write32(0x3ff4_8484, 1 << 10);
        let rtc_io = p.read32(0x3ff4_8484);
        assert_eq!((sens, rtc_io), (3 << 12, 1 << 10));
        assert_eq!(p.misc.mmio_log.take().unwrap(), [
            (0, 0x3ff4_8834, 3 << 12, true), (0, 0x3ff4_8834, 3 << 12, false),
            (0, 0x3ff4_8484, 1 << 10, true), (0, 0x3ff4_8484, 1 << 10, false),
        ]);
        // The block-0x48 entries go to the analog model, not RTC_CNTL's register RAM.
        assert_eq!(p.rtc.0.ram.read(0x834), 0);
        assert!(p.adc.dac_outputs().is_empty());
    }

    fn configure_i2c0_pins(p: &mut Peripherals) {
        for (pin, mux, signal) in [(21u32, 0x7c, 30u32), (22, 0x80, 29)] {
            p.write32(0x3ff4_9000 + mux, (2 << 12) | (1 << 9) | (1 << 8));
            p.write32(0x3ff4_4130 + 4 * signal, (1 << 7) | pin);
            p.write32(0x3ff4_4530 + 4 * pin, signal);
        }
    }

    fn command(op: u32, bytes: u32) -> u32 { (op << 11) | (1 << 8) | bytes }

    #[test]
    fn classic_i2c_commands_devices_matrix_timeout_and_interrupts() {
        use esp_periph::i2c::{Reg8Device, INT_END_DETECT, INT_NACK, INT_TIMEOUT, INT_TRANS_COMPLETE};

        const BASE: u32 = 0x3ff5_3000;
        let mut p = Peripherals::new([0; 6]);
        p.i2c[0].attach(0x6b, Box::new(Reg8Device::new("qmi8658", &[(0, 5)])));
        configure_i2c0_pins(&mut p);
        p.write32(0x3ff0_0104 + 4 * SRC_I2C0 as u32, 7);
        p.write32(BASE + 0x28, INT_END_DETECT | INT_NACK | INT_TIMEOUT | INT_TRANS_COMPLETE);

        p.write32(BASE + 0x1c, 0x6b << 1);
        p.write32(BASE + 0x1c, 0);
        p.write32(BASE + 0x58, command(0, 0));
        p.write32(BASE + 0x5c, command(1, 2));
        p.write32(BASE + 0x60, command(4, 0));
        p.write32(BASE + 0x04, 1 << 5);
        assert_ne!(p.read32(BASE + 0x20) & INT_END_DETECT, 0);

        p.write32(BASE + 0x24, u32::MAX);
        p.write32(BASE + 0x1c, (0x6b << 1) | 1);
        p.write32(BASE + 0x58, command(0, 0));
        p.write32(BASE + 0x5c, command(1, 1));
        p.write32(BASE + 0x60, command(2, 1));
        p.write32(BASE + 0x64, command(3, 0));
        p.write32(BASE + 0x04, 1 << 5);
        assert_eq!(p.read32(BASE + 0x1c), 5);
        assert_ne!(p.read32(BASE + 0x20) & INT_TRANS_COMPLETE, 0);
        assert_ne!(p.cpu_lines(0) & (1 << 7), 0);
        assert_eq!(p.gpio.signal_input(29), Some(true));
        assert_eq!(p.gpio.signal_input(30), Some(true));

        p.write32(BASE + 0x24, u32::MAX);
        p.write32(BASE + 0x1c, 0x7e << 1);
        p.write32(BASE + 0x58, command(0, 0));
        p.write32(BASE + 0x5c, command(1, 1));
        p.write32(BASE + 0x60, command(3, 0));
        p.write32(BASE + 0x04, 1 << 5);
        assert_ne!(p.read32(BASE + 0x20) & INT_NACK, 0);
        assert_ne!(p.read32(BASE + 0x08) & 1, 0);

        p.write32(BASE + 0x24, u32::MAX);
        p.gpio.set_input(22, false);
        p.write32(BASE + 0x04, 1 << 5);
        assert_ne!(p.read32(BASE + 0x20) & INT_TIMEOUT, 0);
        assert_ne!(p.read32(BASE + 0x08) & (1 << 2), 0);
        p.write32(BASE + 0x94, command(4, 0));
        assert_eq!(p.read32(BASE + 0x94), command(4, 0));
    }

    #[test]
    fn classic_i2c1_uses_its_own_dport_source() {
        use esp_periph::i2c::INT_NACK;

        const BASE: u32 = 0x3ff6_7000;
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff4_4130 + 4 * 95, (1 << 7) | 0x38);
        p.write32(0x3ff4_4130 + 4 * 96, (1 << 7) | 0x38);
        p.write32(0x3ff0_0104 + 4 * SRC_I2C1 as u32, 9);
        p.write32(BASE + 0x28, INT_NACK);
        p.write32(BASE + 0x1c, 0x7e << 1);
        p.write32(BASE + 0x58, command(0, 0));
        p.write32(BASE + 0x5c, command(1, 1));
        p.write32(BASE + 0x60, command(3, 0));
        p.write32(BASE + 0x04, 1 << 5);
        assert_ne!(p.source_status(0)[SRC_I2C1 / 32] & (1 << (SRC_I2C1 % 32)), 0);
        assert_ne!(p.cpu_lines(0) & (1 << 9), 0);
    }

    #[test]
    fn legacy_frc2_uses_apb_and_routes_source57_to_both_cores() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff0_0104 + 57 * 4, 9);
        p.write32(0x3ff0_0218 + 57 * 4, 13);
        p.write32(0x3ff4_7030, 10);
        p.write32(0x3ff4_7028, 0x81);
        assert_eq!(p.cycles_until_timer(), 54);
        p.tick(30); assert_eq!(p.read32(0x3ff4_7024), 5);
        assert_eq!(p.read32(0x3ff4_7024), 5, "reads alone never advance clock");
        p.write32(0x3ff4_8070, 1 << 27);
        assert_eq!(p.cycles_until_timer(), 12);
        assert!(p.tick(15));
        assert_ne!(p.cpu_lines(0) & (1 << 9), 0);
        assert_ne!(p.cpu_lines(1) & (1 << 13), 0);
        p.write32(0x3ff4_702c, 1);
        assert_eq!(p.cpu_lines(0) & (1 << 9), 0);
        assert_eq!(p.cpu_lines(1) & (1 << 13), 0);
    }

    #[test]
    fn rc_fast_keeps_apb_and_cpu_one_to_one() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff4_8070, 2 << 27);
        assert_eq!(p.cpu_scale_half(), 12);
        assert_eq!(p.apb_divider(), 6);
        p.write32(0x3ff5_f000, (1 << 31) | (1 << 30) | (1 << 11) | (1 << 10) | (2 << 13));
        p.tick(60); // ten modeled CPU/APB clocks
        p.write32(0x3ff5_f00c, 1);
        assert_eq!(p.read32(0x3ff5_f004), 5);
    }

    #[test]
    fn apb_phase_and_deadlines_follow_xtal_pll_switches() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff5_f000, (1 << 31) | (1 << 30) | (1 << 11) | (1 << 10) | (2 << 13));
        p.write32(0x3ff5_f010, 10);
        p.write32(0x3ff5_f098, 1);
        assert_eq!(p.cycles_until_timer(), 114, "XTAL: (20 APB ticks - 1) * 6");
        p.write32(0x3ff4_8070, 1 << 27);
        assert_eq!(p.cycles_until_timer(), 57, "PLL: (20 APB ticks - 1) * 3");
        p.tick(1);
        p.write32(0x3ff4_8070, 0);
        p.tick(3);
        assert_eq!(p.apb_phase, 5);
        p.tick(1);
        assert_eq!(p.apb_phase, 0);
        p.tick(6);
        p.write32(0x3ff5_f00c, 1);
        assert_eq!(p.read32(0x3ff5_f004), 1, "two APB ticks across a source switch");
    }

    #[test]
    fn classic_spi_and_gpio_offsets_reach_shared_models() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff4_2080, 0x1122_3344);
        assert_eq!(p.read32(0x3ff4_2080), 0x1122_3344);
        p.write32(0x3ff4_9040, (2 << 12) | (1 << 9));
        p.write32(0x3ff4_4024, 1 << 2);
        p.write32(0x3ff4_4008, 1 << 2);
        assert_eq!(p.read32(0x3ff4_403c) & (1 << 2), 1 << 2);
    }

    #[test]
    fn classic_gpio_matrix_routes_simple_io_constants_and_inversion() {
        let mut p = Peripherals::new([0; 6]);
        let pin = 4usize;
        p.write32(0x3ff4_9048, (2 << 12) | (1 << 9) | (1 << 7));
        assert_eq!(p.read32(0x3ff4_403c) & (1 << pin), 0, "pull-down drives an undriven input low");
        p.write32(0x3ff4_9048, (2 << 12) | (1 << 9) | (1 << 8));
        assert_eq!(p.read32(0x3ff4_403c) & (1 << pin), 1 << pin);
        p.gpio.set_input(pin as u8, false);
        assert_eq!(p.read32(0x3ff4_403c) & (1 << pin), 0);

        p.write32(0x3ff4_4530 + 4 * pin as u32, 256);
        p.write32(0x3ff4_4024, 1 << pin);
        p.write32(0x3ff4_4008, 1 << pin);
        assert_eq!(p.gpio.driven(pin), Some(true));
        p.write32(0x3ff4_4530 + 4 * pin as u32, 256 | (1 << 9));
        assert_eq!(p.gpio.driven(pin), Some(false));
        p.write32(0x3ff4_4530 + 4 * pin as u32, 256 | (1 << 11));
        assert_eq!(p.gpio.driven(pin), None);

        let signal = 17usize;
        p.write32(0x3ff4_4130 + 4 * signal as u32, (1 << 7) | 0x30);
        assert_eq!(p.gpio.signal_input(signal), Some(false));
        p.write32(0x3ff4_4130 + 4 * signal as u32, (1 << 7) | 0x38);
        assert_eq!(p.gpio.signal_input(signal), Some(true));
        p.write32(
            0x3ff4_4130 + 4 * signal as u32,
            (1 << 7) | (1 << 6) | pin as u32,
        );
        assert_eq!(p.gpio.signal_input(signal), Some(true));
        p.write32(0x3ff4_9048, 1 << 9);
        assert_eq!(
            p.gpio.signal_input(signal),
            None,
            "direct IO_MUX mode bypasses the matrix"
        );

        p.write32(0x3ff4_9014, (2 << 12) | (1 << 9));
        p.write32(0x3ff4_45b8, 256);
        p.write32(0x3ff4_4030, 1 << 2);
        assert_eq!(p.gpio.gpio.enable & (1 << 34), 0, "GPIO34 is input-only");
    }

    #[test]
    fn classic_spi_signals_use_direct_iomux_or_gpio_matrix_routes() {
        let mut p = Peripherals::new([0; 6]);

        p.write32(0x3ff4_908c, (1 << 12) | (1 << 9));
        p.gpio.set_output_signal(65, true, true);
        assert_eq!(p.read32(0x3ff4_403c) & (1 << 23), 1 << 23);

        p.write32(0x3ff4_9074, (1 << 12) | (1 << 9));
        p.gpio.set_input(19, true);
        assert_eq!(p.gpio.signal_input(64), Some(true));
        p.write32(0x3ff4_9074, (2 << 12) | (1 << 9));

        p.write32(0x3ff4_9048, (2 << 12) | (1 << 9));
        p.write32(0x3ff4_4130 + 4 * 64, (1 << 7) | 4);
        p.gpio.set_input(4, false);
        assert_eq!(p.gpio.signal_input(64), Some(false));
        p.gpio.set_input(4, true);
        assert_eq!(p.gpio.signal_input(64), Some(true));
    }

    #[test]
    fn classic_gpio_falling_and_level_interrupt_status_clear_correctly() {
        let mut p = Peripherals::new([0; 6]);
        let bit = 1 << 4;
        p.write32(0x3ff4_9048, (2 << 12) | (1 << 9) | (1 << 8));
        p.write32(0x3ff4_4098, (2 << 7) | (1 << 15));
        assert!(p.gpio.set_input(4, false));
        assert_eq!(p.read32(0x3ff4_4044) & bit, bit);
        assert_eq!(p.read32(0x3ff4_4068) & bit, bit);
        p.write32(0x3ff4_404c, bit);
        assert_eq!(p.read32(0x3ff4_4068) & bit, 0);

        p.write32(0x3ff4_4098, (4 << 7) | (1 << 15));
        assert_eq!(p.read32(0x3ff4_4068) & bit, bit);
        p.write32(0x3ff4_404c, bit);
        assert_eq!(
            p.read32(0x3ff4_4068) & bit,
            bit,
            "active level is not cleared"
        );
        p.gpio.set_input(4, true);
        assert_eq!(p.read32(0x3ff4_4068) & bit, 0);
    }

    #[test]
    fn classic_ledc_routes_pwm_and_interrupt_through_gpio_and_dport() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff4_8070, 1 << 27); // PLL: APB 80 MHz
        p.write32(0x3ff0_00c0, 1 << 11);
        p.write32(0x3ff0_0104 + 4 * SRC_LEDC as u32, 12);
        p.write32(0x3ff5_9140, 8 | (16000 << 5) | (1 << 25));
        p.write32(0x3ff5_9008, 64 << 4);
        p.write32(0x3ff5_900c, (1 << 31) | (1 << 30) | (1 << 20) | (1 << 10));
        p.write32(0x3ff5_9000, 4);
        p.write32(0x3ff5_9188, (1 << 8) | 1);
        p.write32(0x3ff4_9048, (2 << 12) | (1 << 9));
        p.write32(0x3ff4_4540, 71);

        p.tick(48_000);
        let (hz, duty) = p.pwm_output(4).unwrap();
        assert!((hz - 5000.0).abs() < 0.001);
        assert_eq!(duty, 16384);
        assert_ne!(p.cpu_lines(0) & (1 << 12), 0);

        p.write32(0x3ff4_4540, 71 | (1 << 9));
        assert_eq!(p.pwm_output(4).unwrap().1, 49151);
    }

    #[test]
    fn app_cpu_follows_dport_control_bits() {
        let mut dport = Dport::new();
        assert_eq!(dport.core1_control(), (false, true, false));
        dport.write(0x30, 1);
        dport.write(0x2c, 0);
        assert_eq!(dport.core1_control(), (true, false, false));
    }

    #[test]
    fn dport_routes_cross_core_interrupts() {
        let mut dport = Dport::new();
        dport.write(0x104 + 24 * 4, 7);
        dport.write(0x218 + 24 * 4, 9);
        dport.write(0xdc, 1);
        let mut status = [0; 3];
        status[0] = 1 << SRC_FROM_CPU0;
        assert_eq!(dport.cpu_lines(0, &status), 1 << 7);
        assert_eq!(dport.cpu_lines(1, &status), 1 << 9);
        dport.write(0xdc, 0);
        status[0] = 0;
        assert_eq!(dport.cpu_lines(0, &status), 0);
    }

    #[test]
    fn dport_routes_gpio_uart_timers_and_watchdogs() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff4_8070, 1 << 27);

        p.write32(0x3ff0_0104 + 4 * SRC_GPIO as u32, 7);
        p.write32(0x3ff4_9048, (2 << 12) | (1 << 9) | (1 << 8));
        p.write32(0x3ff4_4098, (2 << 7) | (1 << 15));
        p.gpio.set_input(4, false);
        assert_ne!(p.cpu_lines(0) & (1 << 7), 0);
        assert_ne!(p.read32(0x3ff0_00ec) & (1 << SRC_GPIO), 0);

        p.write32(0x3ff0_0104 + 4 * SRC_UART0 as u32, 8);
        p.write32(0x3ff4_000c, 1 << 8);
        p.uart[0].host_input(b"x");
        assert_ne!(p.cpu_lines(0) & (1 << 8), 0);

        p.write32(0x3ff0_0104 + 4 * SRC_TG0_T0 as u32, 9);
        p.write32(
            0x3ff5_f000,
            (1 << 31) | (1 << 30) | (1 << 11) | (1 << 10) | (2 << 13),
        );
        p.write32(0x3ff5_f010, 10);
        p.write32(0x3ff5_f098, 1);
        p.tick(60);
        assert_ne!(p.cpu_lines(0) & (1 << 9), 0);

        p.write32(0x3ff0_0104 + 4 * SRC_RTC_CORE as u32, 11);
        p.rtc.0.ram.write(0x3c, 1 << 3);
        p.rtc.0.ram.write(0x44, 1 << 3);
        assert_ne!(p.source_status(0)[1] & (1 << (46 - 32)), 0);
        assert_eq!(p.source_status(0)[1] & (1 << (47 - 32)), 0);
        assert_ne!(p.cpu_lines(0) & (1 << 11), 0);
    }

    #[test]
    fn classic_lact_counts_and_raises_its_interrupt() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x3ff4_8070, 1 << 27);
        let base = 0x3ff5_f000;
        p.write32(base + 0x84, 8);
        p.write32(base + 0x98, 1 << 3);
        p.write32(
            base + 0x70,
            (1 << 31) | (1 << 30) | (40 << 13) | (1 << 11) | (1 << 10),
        );

        p.tick(1_200); // 400 APB ticks, ten LACT ticks at the IDF's divider 40.
        p.write32(base + 0x80, 1);
        assert_eq!(p.read32(base + 0x78), 10);
        assert_eq!(p.read32(base + 0x9c) & (1 << 3), 1 << 3);
        assert_ne!(Device::irq_sources(&p.timg[0]) & (1 << 3), 0);
        p.write32(base + 0xa4, 1 << 3);
        assert_eq!(p.read32(base + 0x9c) & (1 << 3), 0);
    }

    #[test]
    fn classic_rtc_watchdog_requests_reset() {
        let mut p = Peripherals::new([0; 6]);


        p.write32(0x3ff4_80a4, 0x50d8_3aa1);
        p.write32(0x3ff4_8090, 2);
        p.write32(0x3ff4_808c, (1 << 31) | (3 << 28));
        p.tick(3_200);
        assert!(p.rtc.0.sw_reset);
        assert_eq!(p.rtc.0.reset_cause, esp_periph::RST_RTCWDT_SYS);
    }

    #[test]
    fn classic_uart_reports_receive_fifo_pointers() {
        let mut p = Peripherals::new([0; 6]);
        p.uart[0].host_input(b"ok");
        assert_eq!((p.read32(0x3ff4_0060) >> 13) & 0x7ff, 2);
        assert_eq!(p.read32(0x3ff4_0000), b'o' as u32);
        assert_eq!((p.read32(0x3ff4_0060) >> 13) & 0x7ff, 1);
        assert_eq!(p.read32(0x3ff4_0060) & !(0x7ff << 13), 0);
        // A full 128-byte FIFO wraps the write pointer back to zero.
        p.uart[1].host_input(&[0x55; 128]);
        assert_eq!(p.read32(0x3ff5_0060), 0);
        assert_eq!(p.read32(0x3ff5_001c) & 0xff, 128);
    }

    #[test]
    fn classic_sha256_layout_hashes_one_block() {
        let mut sha = ClassicSha::new();
        let mut block = [0u8; 64];
        block[..3].copy_from_slice(b"abc");
        block[3] = 0x80;
        block[63] = 24;
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            sha.write((i * 4) as u32, u32::from_be_bytes(*word));
        }
        sha.write(0x90, 1);
        sha.write(0x98, 1);
        let got: Vec<u8> = (0..8).flat_map(|i| sha.read(i * 4).to_be_bytes()).collect();
        assert_eq!(got, [0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea,
                         0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
                         0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c,
                         0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad]);
    }
    #[test]
    fn timer_edges_use_idf_sources_58_and_62_on_both_cores() {
        for (group, source, base) in [(0, 58, 0x3ff5_f000), (1, 62, 0x3ff6_0000)] {
            let mut p = Peripherals::new([0; 6]);
            p.write32(0x3ff4_8070, 1 << 27);
            for (core, map) in [(0, 0x3ff0_0104), (1, 0x3ff0_0218)] {
                p.write32(map + source * 4, 10);
                p.write32(map + (source - 1) * 4, 11);
                assert_eq!(p.cpu_lines(core), 0);
            }
            p.write32(base, (1 << 31) | (1 << 30) | (2 << 13) | (1 << 12) | (1 << 10));
            p.write32(base + 0x10, 2);
            p.write32(base + 0x98, 1);
            p.tick(12);
            assert_ne!(Device::irq_sources(&p.timg[group]) & (1 << 4), 0);
            for core in 0..2 { assert_eq!(p.cpu_lines(core), 1 << 10); }
            p.write32(base + 0xa4, 1);
            for core in 0..2 { assert_eq!(p.cpu_lines(core), 0); }
        }
    }

}
