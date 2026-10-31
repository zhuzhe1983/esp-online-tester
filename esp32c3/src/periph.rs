//! ESP32-C3 peripherals.
//!
//! The C3 and the S3 share most of their peripheral IP — UART, USB-Serial/JTAG, systimer, timer
//! groups, GPIO, the SPI flash controller, GDMA, SHA/AES/RSA are the same blocks with the same
//! register layouts apart from C3 GDMA. The models come from `esp-periph`; the C3 GDMA adapter,
//! address map, cache controller and interrupt controller live here.

use esp_periph::{i2c::I2c, rmt_compact::RmtCompact, GpSpi};
use emu_core::{ClockDomain, ClockTree};
use esp_periph::{device_set, mmio, Device, DeviceSet, Dispatch, Misc, WriteEffect, NO_SOURCE};
use esp_periph::{Aes, Efuse, Gpio, Ledc, LedcLayout, RegRam, Rsa, RtcCntl, Sha, SpiMem, SystemRegs, Systimer, TimerGroup, Uart, UartLayout, UsbSerialJtag};

pub const CPU_HZ: u64 = 160_000_000;
pub const PERIPH_BASE: u32 = 0x6000_0000;
pub const PERIPH_END: u32 = 0x6010_0000;

/// Interrupt sources, numbered as the hardware numbers them — that is, by the order of the
/// `INTERRUPT_CORE0_*_MAP_REG` registers. Do **not** take these from `soc/interrupts.h`: that
/// enum omits the NMI entries, so its indices are shifted and every source lands on the wrong
/// line. Only the sources we can assert are listed.
pub mod src {
    // ESP-IDF v5.5.5 components/soc/esp32c3/include/soc/interrupts.h:38 (zero-based 20).
    pub const I2S: usize = 20;
    pub const APB_CTRL: usize = 14; pub const GPIO: usize = 16; pub const SPI2: usize = 19;
    pub const UART0: usize = 21; pub const UART1: usize = 22; pub const LEDC: usize = 23;
    pub const EFUSE: usize = 24; pub const USB_SERIAL_JTAG: usize = 26; pub const RTC_CORE: usize = 27;
    pub const RMT: usize = 28; pub const I2C_EXT0: usize = 29;
    pub const TG0_T0: usize = 32; pub const TG0_WDT: usize = 33;
    pub const TG1_T0: usize = 34; pub const TG1_WDT: usize = 35;
    pub const SYSTIMER_T0: usize = 37; pub const SYSTIMER_T1: usize = 38; pub const SYSTIMER_T2: usize = 39;
    pub const DMA_CH0: usize = 44; pub const DMA_CH1: usize = 45; pub const DMA_CH2: usize = 46;
    pub const RSA: usize = 47; pub const AES: usize = 48; pub const SHA: usize = 49;
    /// software interrupts, raised by writing `SYSTEM_CPU_INTR_FROM_CPU_n` — this is how the
    /// FreeRTOS port yields, so without them `xPortStartScheduler` just returns
    pub const FROM_CPU0: usize = 50;
    pub const COUNT: usize = 62;
}

/// The C3's interrupt matrix (`INTERRUPT_CORE0`, 0x600C2000).
///
/// 62 peripheral sources are each mapped to one of 31 CPU interrupt lines. A line is taken when
/// it is enabled, its priority is **at or above** the threshold, and `mstatus.MIE` is set; the
/// CPU then vectors to `mtvec + 4*line`. Level lines follow the source; edge lines latch and are
/// cleared by writing CPU_INT_CLEAR.
pub struct Intc {
    pub map: [u32; src::COUNT],
    pub lines: esp_periph::intmtx::Lines,
    /// asserted sources at the last refresh, for INTR_STATUS_0/1
    pub status: [u32; 2],
    ram: RegRam,
}

impl Default for Intc { fn default() -> Self { Self::new() } }

impl Intc {
    pub fn new() -> Self {
        Intc { map: [0; src::COUNT], lines: Default::default(), status: [0; 2], ram: RegRam::new() }
    }

    /// Re-derive the CPU lines from the asserted sources.
    fn update(&mut self, st: &[u32; 4]) {
        self.status = [st[0], st[1]];
        self.lines.update(&self.map, st);
    }

    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x000..=0x0f4 => self.map.get((off / 4) as usize).copied().unwrap_or(0),
            // INTERRUPT_CORE0_INTR_STATUS_0/1_REG after the 62 map registers: IDF v5.5.4
            // components/soc/esp32c3/register/soc/interrupt_core0_reg.h (offsets checked against
            // the esp32c3 PAC 0.33.0 SVD). esp-hal's trap handler reads them to pick the
            // peripheral handlers to run; without them it returns with the line still asserted.
            0x0f8 => self.status[0], 0x0fc => self.status[1],
            0x104 => self.lines.enable,
            0x108 => self.lines.int_type,
            0x110 => self.lines.level | self.lines.edge_pending,     // EIP_STATUS: raw source state
            0x114..=0x190 => self.lines.pri[((off - 0x114) / 4) as usize],
            0x194 => self.lines.thresh,
            _ => self.ram.read(off),
        }
    }

    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x000..=0x0f8 => { if let Some(m) = self.map.get_mut((off / 4) as usize) { *m = v & 0x1f; } }
            0x104 => self.lines.enable = v,
            0x108 => self.lines.int_type = v,
            0x10c => self.lines.edge_pending &= !v,            // CPU_INT_CLEAR
            0x114..=0x190 => self.lines.pri[((off - 0x114) / 4) as usize] = v & 0xf,
            0x194 => self.lines.thresh = v & 0xf,
            _ => self.ram.write(off, v),
        }
    }
}


/// The C3's cache controller. Only the "operation finished" bits matter to us: the ROM and the
/// bootloader kick a sync/preload/lock and then poll for done, so a model that never completes
/// hangs the boot. Register offsets differ from the S3's, which is why this is not shared.
pub struct Extmem { ram: RegRam }

impl Default for Extmem { fn default() -> Self { Self::new() } }

impl Extmem {
    pub fn new() -> Self { Extmem { ram: RegRam::new() } }
    pub fn read(&self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x01c => v | (1 << 2),                 // ICACHE_LOCK_CTRL: LOCK_DONE
            0x028 => v | (1 << 1),                 // ICACHE_SYNC_CTRL: SYNC_DONE
            0x034 => v | (1 << 1),                 // ICACHE_PRELOAD_CTRL: PRELOAD_DONE
            0x040 => v | (1 << 3),                 // ICACHE_AUTOLOAD_CTRL: AUTOLOAD_DONE
            0x0b0 => 0x001,                        // CACHE_STATE: icache idle
            0x0cc => if v & 1 != 0 { v | (1 << 2) } else { v & !(1 << 2) },   // ICACHE_FREEZE: DONE follows ENA
            0x3fc => 0x2007_0000,                  // DATE
            _ => v,
        }
    }
    pub fn write(&mut self, off: u32, v: u32) { self.ram.write(off, v); }
}


/// Seed the efuse block the way real C3 silicon reads back. The S3's `Efuse::new` lays the
/// wafer-version fields out for its own chip; on the C3 they live in BLK1 bits 114 (minor low),
/// 183 (minor high) and 184 (major), and the bootloader refuses to start an app whose
/// `min_chip_rev` is above what it finds — so getting these wrong stops the boot with
/// "chip revision check failed".
/// Values verified against real silicon (a C3 module, MAC 3c:84:27:b6:a7:1c, 2026-08-29):
/// wafer v0.4, package 0, block revision v1.3. The bootloader prints both and refuses to start
/// an app whose `min_chip_rev` is above the wafer version.
pub fn efuse_c3(mac: [u8; 6], rev_major: u32, rev_minor: u32, blk_minor: u32) -> Efuse {
    let mut e = Efuse::new(mac);
    e.write(0x48, (mac[0] as u32) << 8 | mac[1] as u32);       // BLK1 word 1: MAC high, nothing else
    // BLK1 word 3 holds WAFER_VERSION_MINOR_LO (bit 114), PKG_VERSION (117) and BLK_VERSION_MINOR (120)
    e.write(0x50, (rev_minor & 7) << 18 | (blk_minor & 7) << 24);
    e.write(0x58, ((rev_minor >> 3) & 1) << 23 | (rev_major & 3) << 24);
    e                                                          // BLK_VERSION_MAJOR = 1 comes from Efuse::new (0x6c)
}
impl Device for Intc {
    fn read(&mut self, off: u32) -> u32 { Intc::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Intc::write(self, off, v); WriteEffect::NONE }
}
impl Device for Extmem {
    fn read(&mut self, off: u32) -> u32 { Extmem::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Extmem::write(self, off, v); WriteEffect::NONE }
}

/// APB_CTRL + 0xB0 is the hardware RNG (`WDEV_RND_REG`); everything else in the block is plain
/// configuration. The model is the shared xorshift32 in `esp_periph::rng`; re-exported here so
/// `esp32c3::periph::Rng` keeps working.
pub use esp_periph::Rng;

pub struct Peripherals {
    pub ble_lc: crate::ble_lc::BleLc,
    pub adc: esp_periph::sar_adc::SarAdc,
    pub uart: [Uart; 2],
    pub usb: UsbSerialJtag,
    pub systimer: Systimer,
    pub timg: [TimerGroup; 2],
    pub gpio: Gpio,
    pub rtc: RtcCntl,
    pub efuse: Efuse,
    pub system: SystemRegs,
    pub extmem: Extmem,
    pub intc: Intc,
    pub spi0: SpiMem,
    pub spi1: SpiMem,
    pub gdma: esp_periph::gdma::GdmaAhb,
    pub sha: Sha,
    pub aes: Aes,
    pub rsa: Rsa,
    pub rng: Rng,
    /// register RAM behind unmodelled blocks, first-touch logging, pc attribution
    pub misc: Misc,
    pub spi_exec: bool,
    pub work_pending: bool,
    wifi_irq: bool, // cached source 0; updated with feature work, not polled by each interrupt scan
    clock: ClockTree<4>,
    last_status: [u32; 4],
    pin_irqs_enabled: bool,
    pub i2c: Box<I2c>,
    pub spi2: Box<GpSpi>,
    pub rmt: Box<RmtCompact>,
    pub io_mux: RegRam,
    pub wifi: Box<crate::wifi::WifiMac>,
    pub fe_iq: crate::wifi::FeIq,
    pub i2c_mst: crate::wifi::I2cMst,
    pub ledc: Ledc,
    /// Boxed: the controller is cold unless firmware uses I2S.
    pub i2s0: Box<esp_periph::I2s>,
    /// I2S0 RX running, cached from RX_CONF writes for the per-write work refresh.
    pub i2s_rx: bool,
}

// Every peripheral, where it sits, and its interrupt source numbers (`src`).
device_set! { Peripherals; inline always; clock: (clock) CPU_HZ, [(ClockDomain::Systimer, 10), (ClockDomain::Apb, 2), (ClockDomain::RtcSlow, 1067), (ClockDomain::Cpu, 1)];
    // IDF v5.5.5 components/soc/esp32c3/include/soc/interrupts.h: ETS_RWBLE_INTR_SOURCE = 8.
    0x31 "BLE_LC" optional (ble_lc) => [8];
    0x40 "APB_SARADC" (adc) => [];
    0x06 "FE_IQ" (fe_iq) @ 0x140..=0x177 => [];
    0x33 "WIFI_MAC" (wifi) => [];
    0x34 "WIFI_MAC2" (wifi) delta 0x1000 => [];
    0x35 "WDEV" (wifi) delta 0x2000 => [];
    0x0e "I2C_MST" (i2c_mst) => [];
    0x00 "UART0" (uart[0]) => [src::UART0];
    0x10 "UART1" (uart[1]) => [src::UART1];
    0x02 "SPI1" (spi1) => [];
    0x03 "SPI0" (spi0) => [];
    0x04 "GPIO" (gpio) => [src::GPIO];
    0x09 "IO_MUX" (io_mux) => [];
    // Pin clocks and sources participate only when active.
    // I2C interrupts already participate in the gated pin-source scan.
    0x13 "I2C0" optional (i2c) => [];
    0x16 "RMT" alias (rmt) => [src::RMT];
    0x24 "SPI2" alias (spi2) => [src::SPI2];
    0x19 "LEDC" optional (ledc) => [src::LEDC];
    // the efuse controller shares the RTC block on the C3, at +0x800
    0x08 "EFUSE" (efuse) delta -0x800 @ 0x800..=0xfff => [];
    0x08 "RTCCNTL" (rtc) => [];
    0x1f "TIMG0" (timg[0]) => [src::TG0_T0];
    0x20 "TIMG1" (timg[1]) => [src::TG1_T0];
    0x23 "SYSTIMER" (systimer) => [src::SYSTIMER_T0, src::SYSTIMER_T1, src::SYSTIMER_T2];
    0x26 "APB_CTRL" (rng) @ 0xb0..=0xb3 => [];
    0x3a "AES" (aes) => [src::AES];
    0x3b "SHA" (sha) => [];
    0x3c "RSA" (rsa) => [src::RSA];
    // three channels; out and in interrupts of a channel share one source
    0x3f "GDMA" (gdma) => [src::DMA_CH0, src::DMA_CH1, src::DMA_CH2, NO_SOURCE, NO_SOURCE, src::DMA_CH0, src::DMA_CH1, src::DMA_CH2, NO_SOURCE, NO_SOURCE];
    0x2d "I2S" optional (i2s0) => [src::I2S];
    0x43 "USB_SERIAL_JTAG" (usb) => [src::USB_SERIAL_JTAG];
    0xc0 "SYSTEM" (system) => [src::FROM_CPU0, src::FROM_CPU0 + 1, src::FROM_CPU0 + 2, src::FROM_CPU0 + 3];
    0xc2 "INTERRUPT" (intc) => [];
    0xc4 "EXTMEM" (extmem) => [];
}

impl DeviceSet for Peripherals {
    const BASE: u32 = PERIPH_BASE;
    fn block_name(block: u32) -> &'static str { Peripherals::block_name(block) }
    fn misc(&self) -> &Misc { &self.misc }
    fn misc_mut(&mut self) -> &mut Misc { &mut self.misc }
    fn pre_access(&mut self, block: u32, _off: u32, _write: bool) {
        if block == 0x31 && self.ble_lc.enabled() {
            // IDF v5.5.5 soc/esp32c3/register/soc/{syscon,rtc_cntl}_reg.h.
            // Checked on C3 rev v0.3: pre-init BT is powered down and isolated,
            // even though SYSCON already enables its clocks and releases reset.
            let syscon = self.misc.generic.get(&0x26).expect("C3 SYSCON reset registers");
            self.ble_lc.accessible = syscon.read(0x14) & 0x30800 == 0x30800
                && syscon.read(0x18) & 0x3e08 == 0
                && self.rtc.ram.read(0x88) & (1 << 11) == 0
                && self.rtc.ram.read(0x8c) & (1 << 22) == 0;
        }
        if block == 0x40 { self.adc.now_cycles = self.clock.cycles(); }
        if (0x33..=0x35).contains(&block) { self.wifi.now_cycles = self.clock.cycles(); }
        if block == 0x06 { self.fe_iq.now_cycles = self.clock.cycles(); }
        if block == 0x26 { self.rng.now = self.clock.cycles() as u32; }
    }
}

impl Peripherals {
    pub fn enable_ble_full(&mut self, observe: bool) {
        if !self.ble_lc.enabled() { self.ble_lc.enable(); }
        self.ble_lc.observe(observe);
        self.refresh_work();
        self.refresh_optional(0x31);
    }

    pub fn new(mac: [u8; 6]) -> Self {
        Peripherals {
            ble_lc: Default::default(),
            i2c: Box::new(I2c::new()), spi2: Box::new(GpSpi::new()), rmt: Box::new(RmtCompact::new(CPU_HZ)), io_mux: RegRam::new(),
            adc: esp_periph::sar_adc::SarAdc::new(false, CPU_HZ),
            wifi: Default::default(), fe_iq: Default::default(), i2c_mst: Default::default(),
            uart: [Uart::new(UartLayout::C3), Uart::new(UartLayout::C3)], usb: UsbSerialJtag::new(CPU_HZ), systimer: Systimer::new(),
            timg: [TimerGroup::new(), TimerGroup::new()], gpio: { let mut g = Gpio::new(); g.func_out_sel.fill(128); g.input_select = esp_soc::pins::ChipPins::C3.input_select; g }, rtc: RtcCntl::new_c3(), ledc: Ledc::new(LedcLayout::C3),
            efuse: efuse_c3(mac, 0, 4, 3), system: {
                let mut s = SystemRegs::new(0x28);
                // IDF v5.5.5 soc/esp32c3/register/soc/system_reg.h:
                // BT_LPCK_DIV_FRAC reset: SEL_8M=1, DIV_A=1, DIV_B=1.
                // Guest read/modify/write snapshots checked on C3 rev v0.3.
                s.ram.write(0x24, 0x0200_1001);
                s
            }, extmem: Extmem::new(), intc: Intc::new(),
            spi0: { let mut s = SpiMem::new(false); s.has_psram = false; s },
            spi1: { let mut s = SpiMem::new(true); s.has_psram = false; s },   // the C3 has no PSRAM
            gdma: esp_periph::gdma::GdmaAhb::new(esp_periph::gdma::AhbLayout::C3, esp_periph::DMA_ADDR_BASE),
            sha: Sha::new(), aes: Aes::new(), rsa: Rsa::new(), rng: Rng::new(),
            misc: {
                let mut m = Misc::new();
                // IDF v5.5.5 soc/esp32c3/register/soc/syscon_reg.h reset.
                // Guest read/modify/write yields ffffffdf, checked on C3 rev v0.3.
                m.generic.entry(0x26).or_default().write(0x14, 0xfffc_e030);
                m
            }, spi_exec: false, work_pending: false, wifi_irq: false, clock: Self::new_clock(),
            i2s0: Box::new(esp_periph::I2s::new(CPU_HZ)), i2s_rx: false,
            last_status: [0; 4], pin_irqs_enabled: false,
        }
    }

    pub fn block_name(block: u32) -> &'static str {
        match block {
            0x00 => "UART0", 0x02 => "SPI1", 0x03 => "SPI0", 0x04 => "GPIO", 0x05 => "FE2", 0x06 => "FE",
            0x08 => "RTCCNTL/EFUSE", 0x09 => "IO_MUX", 0x0e => "RTC_I2C", 0x10 => "UART1",
            0x13 => "I2C0", 0x14 => "UHCI0", 0x16 => "RMT", 0x19 => "LEDC", 0x1c => "NRX", 0x1d => "BB",
            0x1f => "TIMG0", 0x20 => "TIMG1", 0x23 => "SYSTIMER", 0x24 => "SPI2", 0x26 => "APB_CTRL",
            0x31 => "BLE_LC", 0x2b => "TWAI", 0x2d => "I2S", 0x3a => "AES", 0x3b => "SHA", 0x3c => "RSA", 0x3d => "DS",
            0x3e => "HMAC", 0x3f => "GDMA", 0x40 => "APB_SARADC", 0x43 => "USB_SERIAL_JTAG",
            0xc0 => "SYSTEM", 0xc1 => "SENSITIVE", 0xc2 => "INTERRUPT", 0xc4 => "EXTMEM",
            0xc5 => "MMU", 0xcc => "XTS_AES", 0xce => "ASSIST_DEBUG", 0xcf => "DEDICATED_GPIO",
            _ => "?",
        }
    }

    pub fn uart_route(&self, port: usize) -> esp_soc::uart::UartRoute {
        // ESP-IDF v5.5.4 components/soc/esp32c3/include/soc/clk_tree_defs.h:
        // UART_SCLK_RTC = SOC_MOD_CLK_RC_FAST, SOC_CLK_RC_FAST_FREQ_APPROX = 17.5 MHz.
        // IDF 4.4 used 20 MHz (RTC_CLK_FREQ).
        let clock = self.uart[port].clock_config();
        esp_soc::uart::UartPins::C3.route(port, &self.gpio, &self.io_mux,
            self.uart[port].baud(clock, 17_500_000))
    }

    pub fn uart_pin_input(&mut self, input: &esp_soc::uart::UartInput) {
        let routes: [_; 2] = std::array::from_fn(|port| self.uart_route(port));
        esp_soc::uart::uart_pin_input(&mut self.uart, input, &routes);
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        mmio::read32(self, addr)
    }

    /// Returns true for pin-service blocks (I2C, RMT, SPI2), whose writes can change pin activity.
    pub fn write32(&mut self, addr: u32, v: u32) -> bool {
        // IDF v5.5.4 components/soc/esp32c3/register/soc/io_mux_reg.h:42-50 (FUN_PD/PU).
        if (0x60009004..=0x60009058).contains(&addr) {
            self.gpio.set_pad(((addr - 0x60009004) / 4) as u8, v);
        }
        if addr == 0x6001_3004 && v & (1 << 5) != 0 && self.i2c.has_pinned_devices() {
            let pins = self.i2c_pin(54).zip(self.i2c_pin(53));
            self.i2c.set_pins(pins);
        }
        let fx = mmio::write32(self, addr, v);
        // One test covers both effects, as the SPI test alone did before RX existed.
        if fx.contains(WriteEffect::SPI_EXEC | WriteEffect::RX_CONF) {
            self.spi_exec |= fx.contains(WriteEffect::SPI_EXEC);
            if fx.contains(WriteEffect::RX_CONF) { self.i2s_rx = self.i2s0.rx_running(); }
        }
        let pin_block = matches!(addr & !0xfff, 0x6001_3000 | 0x6001_6000 | 0x6002_4000);
        if pin_block {
            self.pin_irqs_enabled = self.i2c.int_ena | self.spi2.int_ena | self.rmt.rmt.int_ena != 0;
        }
        if addr == PERIPH_BASE + 0xc0018 && v & (1 << 11) != 0 { self.ledc = Ledc::new(LedcLayout::C3); }
        if addr == PERIPH_BASE + 0xc0010 || addr == PERIPH_BASE + 0xc0018 {
            self.ledc.clock_enabled = self.system.read(0x10) & (1 << 11) != 0 && self.system.read(0x18) & (1 << 11) == 0;
            self.refresh_optional(0x19);
        }
        // IDF v5.5.5 syscon_reg.h: BTBB, RW_BTMAC/BTLP and their register resets.
        // Reset cancellation/register clearing are inferred, not hardware-probed.
        if addr == 0x6002_6018 && v & 0x3e08 != 0 && self.ble_lc.enabled() {
            self.ble_lc.reset_controller();
            self.refresh_optional(0x31);
        }
        self.refresh_work();
        pin_block
    }

    /// Recompute after MMIO or host AP configuration, not on idle scheduler rounds.
    pub fn refresh_work(&mut self) {
        self.wifi_irq = self.wifi.irq();
        self.work_pending = self.ble_lc.enabled() || self.spi_exec || self.aes.dma_pending || !self.wifi.tx_pending.is_empty() || self.wifi.link.ap().is_some() || self.i2s_rx;
    }

    /// Advance the fixed clock-tree devices by `cycles` CPU cycles (16 MHz systimer, 80 MHz APB, ~150 kHz
    /// RTC slow clock), with delivered-tick accounting so a slow clock never drifts.
    #[inline(always)]
    pub fn tick(&mut self, cycles: u64) { Dispatch::tick(self, cycles); }

    #[inline(always)]
    /// CPU cycles since this peripheral set was built.
    pub(crate) fn clock_cycles(&self) -> u64 { self.clock.cycles() }
    pub fn cycles_until_timer(&self) -> u32 {
        Dispatch::cycles_until_deadline(self)
    }

    /// Which interrupt sources are asserted right now.
    pub fn source_status(&self) -> [u32; 4] {
        let mut st = Dispatch::source_status(self);
        if self.i2c.int_ena | self.spi2.int_ena | self.rmt.rmt.int_ena != 0 {
            if self.i2c.irq() { st[0] |= 1 << src::I2C_EXT0; }
            if self.spi2.irq() { st[0] |= 1 << src::SPI2; }
            if self.rmt.rmt.irq() { st[0] |= 1 << src::RMT; }
        }
        st[0] |= self.wifi_irq as u32;
        st
    }

    /// Refresh the interrupt matrix; returns true if any source changed.
    pub fn refresh_lines(&mut self) -> bool {
        if self.pin_irqs_enabled { return self.refresh_pin_lines(); }
        let mut st = Dispatch::source_status(self);
        st[0] |= self.wifi_irq as u32;
        let changed = st != self.last_status;
        self.last_status = st;
        self.intc.update(&st);
        changed
    }

    #[inline(never)]
    fn refresh_pin_lines(&mut self) -> bool {
        let st = self.source_status();
        let changed = st != self.last_status;
        self.last_status = st;
        self.intc.update(&st);
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_and_pin_sources_survive_interrupt_refresh() {
        let mut p = Peripherals::new([0; 6]);
        p.wifi.tx_done(0);
        p.refresh_work();
        assert!(p.refresh_lines());
        assert_eq!(p.last_status[0], 1);
        assert!(!p.refresh_lines());

        p.i2c.int_raw = 1 << 7;
        p.spi2.int_raw = 1 << 12;
        p.rmt.rmt.int_raw = 1;
        for (addr, bit) in [(0x6001_3028, 1 << 7), (0x6002_4034, 1 << 12), (0x6001_6040, 1)] {
            p.write32(addr, bit);
        }
        let pins = (1 << src::I2C_EXT0) | (1 << src::SPI2) | (1 << src::RMT);
        assert_eq!(p.misc.optional_sources[0] & pins, 0, "pin IRQs have one owner");
        assert_eq!(p.source_status()[0], pins | 1);
        assert!(p.refresh_lines());
        assert_eq!(p.last_status[0], pins | 1);
        p.write32(0x6003_3c40, u32::MAX);
        assert!(p.refresh_lines());
        assert_eq!(p.last_status[0], pins);
        for addr in [0x6001_3028, 0x6002_4034, 0x6001_6040] { p.write32(addr, 0); }
        assert!(p.refresh_lines());
        assert_eq!(p.last_status[0], 0);
    }

    /// INTR_STATUS_0/1 report the asserted sources; the map registers end before them.
    #[test]
    fn interrupt_status_registers_report_asserted_sources() {
        let mut p = Peripherals::new([0; 6]);
        p.write32(0x600c_2000 + 4 * src::TG0_T0 as u32, 7);          // TG0_T0 -> CPU line 7
        p.write32(0x6001_f070, 1);                                    // TIMG0 INT_ENA T0
        p.timg[0].int_raw = 1;
        p.refresh_lines();
        assert_eq!(p.read32(0x600c_20f8), 0, "no source below 32 asserted");
        assert_eq!(p.read32(0x600c_20fc), 1 << (src::TG0_T0 - 32));
        assert_eq!(p.read32(0x600c_2000 + 4 * src::TG0_T0 as u32), 7, "the map register is unchanged");
        p.write32(0x6001_f07c, 1);                                    // TIMG0 INT_CLR T0
        p.refresh_lines();
        assert_eq!(p.read32(0x600c_20fc), 0);
    }

    /// What an ESP32-C3 (rev v0.4) returned: a source shows in INTR_STATUS_0/1 while its peripheral
    /// asserts it with the peripheral's own enable set, whether or not it is mapped to a CPU line.
    #[test]
    fn interrupt_status_follows_the_peripheral_enable_not_the_map() {
        let mut p = Peripherals::new([0; 6]);
        let status1 = |p: &mut Peripherals| { p.refresh_lines(); p.read32(0x600c_20fc) };
        // TIMG0 T0 (source 32): fired, INT_ENA clear -> not shown; set -> shown, mapped or not.
        p.timg[0].int_raw = 1;
        assert_eq!(status1(&mut p), 0, "raw alone is not enough");
        p.write32(0x6001_f070, 1);
        assert_eq!(status1(&mut p), 1 << (src::TG0_T0 - 32), "unmapped");
        p.write32(0x600c_2000 + 4 * src::TG0_T0 as u32, 8);
        assert_eq!(status1(&mut p), 1 << (src::TG0_T0 - 32), "mapped to line 8");
        p.write32(0x6001_f070, 0);
        assert_eq!(status1(&mut p), 0, "INT_ENA clear again");
        // FROM_CPU0 (source 50): shown as soon as SYSTEM_CPU_INTR_FROM_CPU_0 is set, mapped or not.
        p.write32(0x600c_0028, 1);
        assert_eq!(status1(&mut p), 1 << (src::FROM_CPU0 - 32), "map register at reset");
        p.write32(0x600c_2000 + 4 * src::FROM_CPU0 as u32, 7);
        assert_eq!(status1(&mut p), 1 << (src::FROM_CPU0 - 32), "mapped to line 7");
        p.write32(0x600c_0028, 0);
        assert_eq!(status1(&mut p), 0, "cleared");
        assert_eq!(p.read32(0x600c_20f8), 0);
    }

    #[test]
    fn pin_interrupt_participation_tracks_enable_registers() {
        let mut p = Peripherals::new([0; 6]);
        assert!(!p.pin_irqs_enabled);
        for (addr, bit) in [(0x6001_3028, 1 << 7), (0x6002_4034, 1 << 12), (0x6001_6040, 1)] {
            p.write32(addr, bit);
            assert!(p.pin_irqs_enabled);
            p.write32(addr, 0);
            assert!(!p.pin_irqs_enabled);
        }
    }
}
