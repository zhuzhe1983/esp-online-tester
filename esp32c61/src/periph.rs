//! ESP32-C61 peripheral map. Register offsets cite ESP-IDF v5.5.4 components/soc/esp32c61/register/soc/*.h.
use emu_core::{ClockDomain, ClockTree};
use esp_periph::{device_set, mmio, Device, DeviceSet, Dispatch, Misc, RegRam, WriteEffect, NO_SOURCE};
use esp_periph::{Efuse, Gdma, Gpio, GpSpi, Ledc, LedcLayout, Sha, SpiMem, Systimer, TimerGroup, Uart, UartLayout, UsbSerialJtag};
use esp_periph::{RST_POWERON, RST_SW_CPU, RST_SW_SYS};

pub const CPU_HZ: u64 = 160_000_000;
pub const PERIPH_BASE: u32 = 0x6000_0000;
pub const PERIPH_END: u32 = 0x6010_0000;

pub const CPU_SUB_BASE: u32 = 0x2080_0000;
pub const CPU_SUB_END: u32 = 0x2080_2000;

pub mod src {

    pub const FROM_CPU0: usize = 19; pub const GPIO: usize = 27;
    pub const I2S: usize = 39; pub const UART0: usize = 40; pub const UART1: usize = 41; pub const UART2: usize = 42;
    pub const LEDC: usize = 43; pub const USB_SERIAL_JTAG: usize = 44; pub const I2C_EXT0: usize = 45;
    pub const TG0_T0: usize = 46; pub const TG1_T0: usize = 49;
    pub const SYSTIMER_T0: usize = 52; pub const SYSTIMER_T1: usize = 53; pub const SYSTIMER_T2: usize = 54;
    pub const DMA_IN_CH0: usize = 58; pub const DMA_OUT_CH0: usize = 60; pub const GPSPI2: usize = 62;
    pub const SHA: usize = 63; pub const ECC: usize = 64; pub const COUNT: usize = 66;
}

pub struct IntMatrix {
    pub map: [u32; src::COUNT],

    pub status: [u32; 4],
    ram: RegRam,
}
impl Default for IntMatrix { fn default() -> Self { Self::new() } }
impl IntMatrix {
    pub fn new() -> Self { IntMatrix { map: [0; src::COUNT], status: [0; 4], ram: RegRam::new() } }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            off if off < src::COUNT as u32 * 4 => self.map[(off / 4) as usize],

            0x108..=0x110 => self.status[((off - 0x108) / 4) as usize],
            _ => self.ram.read(off),
        }
    }

    pub fn write(&mut self, off: u32, v: u32) {
        match off { off if off < src::COUNT as u32 * 4 => self.map[(off / 4) as usize] = v & 0x3f, _ => self.ram.write(off, v) }
    }
}
impl Device for IntMatrix {
    fn read(&mut self, off: u32) -> u32 { IntMatrix::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { IntMatrix::write(self, off, v); WriteEffect::NONE }
}

pub struct Intc { pub clic: esp_periph::clic::ClicCore, pub config: u32, pub sw_int: u32 }
impl Default for Intc { fn default() -> Self { Self::new() } }
impl Intc {
    pub fn new() -> Self { Self { clic: Default::default(), config: 0, sw_int: 0 } }
    pub fn cpu_read(&self, off: u32) -> u32 { if off == 0 { self.config } else { self.clic.read(off) } }
    pub fn cpu_write(&mut self, off: u32, v: u32, lanes: u32) {
        if off == 0 { self.config = v & 0xf; self.clic.mnlbits = (v & 0xf) << 1; }
        else { self.clic.write(off, v, lanes); }
    }
}
impl Device for Intc {
    fn read(&mut self, off: u32) -> u32 { match off { 0x90..=0x9c => (self.sw_int >> ((off - 0x90) / 4)) & 1, _ => 0 } }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if (0x90..=0x9c).contains(&off) { let b = (off - 0x90) / 4; self.sw_int = (self.sw_int & !(1 << b)) | ((v & 1) << b); }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { (self.sw_int & 0xf) as u64 }
}

pub struct Cache { ram: RegRam }
impl Default for Cache { fn default() -> Self { Self::new() } }
impl Cache {
    pub fn new() -> Self { Cache { ram: RegRam::new() } }
    pub fn read(&self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x028 => if v & (1 << 16) != 0 { v | (1 << 18) } else { v & !(1 << 18) },
            0x084 => v | (1 << 2),
            0x094 => v | (1 << 4),
            0x0d4 => v | (1 << 1),
            0x130 => v | (1 << 1),
            0x3fc => 0x2207_0400,
            _ => v,
        }
    }
    pub fn write(&mut self, off: u32, v: u32) { self.ram.write(off, v); }
}
impl Device for Cache {
    fn read(&mut self, off: u32) -> u32 { Cache::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Cache::write(self, off, v); WriteEffect::NONE }
}

pub struct LpSys {
    pub ram: RegRam,
    pub reset_cause: u32,
    pub sw_reset: bool,

    pub slow_ticks: u64,
    pub time_latch: u64,
}
impl Default for LpSys { fn default() -> Self { Self::new() } }
impl LpSys {
    pub const CLKRST: u32 = 0x000; pub const AON: u32 = 0x400; pub const WDT: u32 = 0x800; pub const TIMER: u32 = 0xc00;
    pub fn new() -> Self { LpSys { ram: RegRam::new(), reset_cause: RST_POWERON, sw_reset: false, slow_ticks: 0, time_latch: 0 } }
    fn request_reset(&mut self, cause: u32) { self.reset_cause = cause; self.sw_reset = true; }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x010 => (self.ram.read(off) & !0x1f) | (self.reset_cause & 0x1f),
            0xc14 => self.time_latch as u32, 0xc18 => (self.time_latch >> 32) as u32,
            0xc10 => self.ram.read(off) & !(1 << 27),
            0x3fc | 0x7fc | 0xbfc | 0xffc => 0x2207_0400,
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x434 => { if v & (1 << 31) != 0 { self.request_reset(RST_SW_SYS); } self.ram.write(off, v & !(1 << 31)); }
            0x438 => { if v & (1 << 28) != 0 { self.request_reset(RST_SW_CPU); } self.ram.write(off, v & !(1 << 28)); }
            0xc10 => { if v & (1 << 27) != 0 { self.time_latch = self.slow_ticks; } self.ram.write(off, v); }
            _ => self.ram.write(off, v),
        }
    }
}
impl Device for LpSys {
    fn read(&mut self, off: u32) -> u32 { LpSys::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { LpSys::write(self, off, v); WriteEffect::NONE }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::RtcSlow) }
    fn tick(&mut self, ticks: u64) { self.slow_ticks += ticks; }
}

pub struct SpiMemC61(pub SpiMem);
impl Device for SpiMemC61 {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x178 => 0xfc00_0000,
            _ => Device::read(&mut self.0, off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Device::write(&mut self.0, off, v) }
    fn debug(&mut self, on: bool) { Device::debug(&mut self.0, on) }
}

pub struct AnaMst { ram: RegRam, pub ana: esp_periph::Regi2c }
impl Default for AnaMst { fn default() -> Self { Self::new() } }
impl AnaMst {
    pub fn new() -> Self { AnaMst { ram: RegRam::new(), ana: Default::default() } }
    fn ctrl_read(&self, off: u32) -> u32 {
        let c = self.ram.read(off);

        self.ana.read(0, c, |key| match key { 0x0063 => &0x5b, k if k & 0xff == 0x63 => &0xff, _ => &0 })
    }
}
impl Device for AnaMst {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x00 | 0x04 => self.ctrl_read(off),
            0x18 => self.ram.read(off) | (1 << 24),
            _ => self.ram.read(off),
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if off == 0x00 || off == 0x04 { self.ana.write(0, v); }
        self.ram.write(off, v);
        WriteEffect::NONE
    }
}

pub struct AssistDebug { pub saved_pc: u32, ram: RegRam }
impl Default for AssistDebug { fn default() -> Self { Self::new() } }
impl AssistDebug {
    pub fn new() -> Self { AssistDebug { saved_pc: 0, ram: RegRam::new() } }
}
impl Device for AssistDebug {
    fn read(&mut self, off: u32) -> u32 { match off { 0x48 => self.saved_pc, _ => self.ram.read(off) } }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.ram.write(off, v); WriteEffect::NONE }
}

pub struct GdmaC61 { pub gdma: Gdma, ram: RegRam }
impl Default for GdmaC61 { fn default() -> Self { Self::new() } }
impl GdmaC61 {
    pub fn new() -> Self { let mut g = Gdma::new(); g.addr_base = 0x4080_0000; GdmaC61 { gdma: g, ram: RegRam::new() } }
    pub(crate) fn map(off: u32) -> Option<u32> {

        const T: [u32; 13] = [0x00, 0x04, 0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x34, 0x38, 0x44, 0x48];
        if off < 0x20 { return Some((off / 0x10) * 0xc0 + 0x08 + off % 0x10); }
        if (0x30..0x50).contains(&off) { let o = off - 0x30; return Some((o / 0x10) * 0xc0 + 0x68 + o % 0x10); }
        if off == 0x64 { return Some(0x3c8); }
        if (0x70..0x70 + 2 * 0xc0).contains(&off) {
            let rel = off - 0x70; let (n, k) = (rel / 0xc0, rel % 0xc0);
            if k <= 0x30 { return Some(n * 0xc0 + T[(k / 4) as usize]); }
            if (0x60..=0x90).contains(&k) { return Some(n * 0xc0 + 0x60 + T[((k - 0x60) / 4) as usize]); }
        }
        None
    }
}
impl Device for GdmaC61 {
    fn read(&mut self, off: u32) -> u32 { match Self::map(off) { Some(o) => self.gdma.read(o), None => self.ram.read(off) } }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { match Self::map(off) { Some(o) => self.gdma.write(o, v), None => self.ram.write(off, v) } WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { Device::irq_sources(&self.gdma) }
    fn debug(&mut self, on: bool) { self.gdma.dbg = on; }
}

pub struct Pcr { ram: RegRam }
impl Default for Pcr { fn default() -> Self { Self::new() } }
impl Pcr {
    pub fn new() -> Self {
        let mut p = Pcr { ram: RegRam::new() };
        p.ram.write(0x11c, 0x300);
        p.ram.write(0x100, 0xff);
        p
    }
    pub fn read(&self, off: u32) -> u32 {
        let v = self.ram.read(off);
        match off {
            0x0 => v | 4,
            0xc => v | 4,
            0x18 => v | 4,
            0x24 => v | 8,
            0x34 => v | 4,
            0x40 => v | 28,
            0x4c => v | 28,
            0x58 => v | 4,
            0x60 => v | 12,
            0x80 => v | 4,
            0x84 => v | 4,
            0x94 => v | 4,
            0x9c => v | 4,
            0xa0 => v | 4,
            0xa4 => v | 4,
            0xac => v | 4,
            0xb4 => v | 4,
            0xb8 => v | 4,
            0xbc => v | 4,
            0xc8 => v | 4,
            0xcc => v | 4,
            0x13c => v | 4,
            0x120 => v & !1,
            0xe8 => (v & 0x00ff_0000) | 40 << 24 | 2 << 8,
            0xec => v,
            0xfc => 480 << 8 | 20,
            0xffc => 0x2207_0400,
            _ => v,
        }
    }
}
impl Device for Pcr {
    fn read(&mut self, off: u32) -> u32 { Pcr::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        self.ram.write(off, v);

        WriteEffect::NONE
    }
}

pub use esp_periph::Rng;

pub fn efuse_c61(mac: [u8; 6], rev_major: u32, rev_minor: u32, pkg: u32, blk_major: u32, blk_minor: u32) -> Efuse {
    let mut e = Efuse::new(mac);
    e.write(0x48, (mac[0] as u32) << 8 | mac[1] as u32);
    e.write(0x4c, (rev_minor & 0xf) | (rev_major & 3) << 4 | (pkg & 7) << 26 | (blk_minor & 7) << 8 | (blk_major & 3) << 11);
    e.write(0x6c, 0);
    e
}

/// GPIO C61 register spacing differs from S3/C6; retain one shared resolved pin state.
/// IDF v5.5.4 gpio_reg.h: IN 0x64, PIN0 0xd4, FUNC_IN 0x2d4, FUNC_OUT 0xad4.
pub struct GpioC61 { pub gpio: Gpio }
impl std::ops::Deref for GpioC61 { type Target = Gpio; fn deref(&self) -> &Gpio { &self.gpio } }
impl std::ops::DerefMut for GpioC61 { fn deref_mut(&mut self) -> &mut Gpio { &mut self.gpio } }
impl GpioC61 {
    fn map(off: u32) -> u32 { match off {
        0x0 => 0x38, 0x34..=0x48 => off - 0x14, 0x64..=0x68 => off - 0x28,
        0x74..=0x88 => off - 0x30, 0xa4 => 0x5c, 0xac => 0x60,
        0xd4..=0x148 => off - 0xd4 + 0x74,
        0x2d4..=0x6d0 => off - 0x2d4 + 0x154,
        0xad4..=0xb48 => off - 0xad4 + 0x554,
        _ => off,
    } }
}
impl Device for GpioC61 {
    fn read(&mut self, off: u32) -> u32 { self.gpio.read(Self::map(off)) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { self.gpio.write(Self::map(off), v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.gpio.irq() as u64 }
}
pub const PINS: esp_soc::pins::ChipPins = esp_soc::pins::ChipPins { valid: (1 << 30) - 1, input_select: 0x100, output_mask: 0x3ff };
pub struct Peripherals {
    pub modem_bb: crate::wifi::ModemBb, pub wifi_mac: crate::wifi::WifiMac,
    pub adc: crate::adc::SarAdc,
    pub uart: [Uart; 3], pub usb: UsbSerialJtag, pub systimer: Systimer, pub timg: [TimerGroup; 2],
    pub gpio: GpioC61, pub io_mux: RegRam, pub i2c: esp_periph::i2c::I2c, pub efuse: Efuse,
    pub spi0: SpiMemC61, pub spi1: SpiMemC61, pub sha: Sha, pub gdma: GdmaC61, pub spi2: GpSpi,
    pub intmtx: IntMatrix, pub intc: Intc, pub cache: Cache, pub lpsys: LpSys, pub pcr: Pcr,
    pub ana_mst: AnaMst, pub rng: Rng, pub misc: Misc, pub spi_exec: bool, clock: ClockTree<4>, pub ledc: Ledc,
}
device_set! { Peripherals; inline always; clock: (clock) CPU_HZ, [(ClockDomain::Systimer, 10), (ClockDomain::Apb, 2), (ClockDomain::RtcSlow, 1067), (ClockDomain::Cpu, 1)];
    0x00 "UART0" (uart[0]) => [src::UART0];
    0x01 "UART1" (uart[1]) => [src::UART1];
    0x02 "SPI0" (spi0) => [];
    0x03 "SPI1" (spi1) => [];
    0x04 "I2C0" optional (i2c) => [src::I2C_EXT0];
    0x06 "UART2" (uart[2]) => [src::UART2];
    0x07 "LEDC" optional (ledc) => [src::LEDC];
    0x08 "TIMG0" (timg[0]) => [src::TG0_T0];
    0x09 "TIMG1" (timg[1]) => [src::TG1_T0];
    0x0a "SYSTIMER" (systimer) => [src::SYSTIMER_T0, src::SYSTIMER_T1, src::SYSTIMER_T2];
    0x0e "SARADC" (adc) => [];
    0x0f "USB_SERIAL_JTAG" (usb) => [src::USB_SERIAL_JTAG];
    0x10 "INTMTX" (intmtx) => [];
    0x80 "GDMA" (gdma) => [src::DMA_OUT_CH0, src::DMA_OUT_CH0 + 1, NO_SOURCE, NO_SOURCE, NO_SOURCE, src::DMA_IN_CH0, src::DMA_IN_CH0 + 1, NO_SOURCE, NO_SOURCE, NO_SOURCE];
    0x81 "SPI2" (spi2) => [src::GPSPI2];
    0x89 "SHA" (sha) => [];
    0x90 "IO_MUX" (io_mux) delta 4 => [];
    0x91 "GPIO" (gpio) => [src::GPIO];
    0x96 "PCR" (pcr) => [];
    0xa0 "MODEM_BB" (modem_bb) => [];
    0xa4 "WIFI_MAC" (wifi_mac) => [0];
    0xa5 "WIFI_RESULTS" alias (wifi_mac) delta 0x1000 => [];
    0xaf "I2C_ANA_MST" (ana_mst) delta -0x800 @ 0x800..=0xfff => [];
    0xb0 "LP_CLKRST" (lpsys) delta -0x400 @ 0x400..=0x7ff => [];
    0xb0 "LP_TIMER" alias (lpsys) @ 0xc00..=0xfff => [];
    0xb1 "LP_AON" alias (lpsys) delta 0x400 @ 0x000..=0x3ff => [];
    0xb1 "LP_WDT" alias (lpsys) delta -0x400 @ 0xc00..=0xfff => [];
    0xb2 "LPPERI_RNG" (rng) @ 0x808..=0x80b => [];
    0xb2 "LPPERI_RNG_SYNC" alias (rng) @ 0x828..=0x82b => [];
    0xb4 "EFUSE" (efuse) delta -0x800 @ 0x800..=0xbff => [];
    0xc5 "INTPRI" (intc) => [src::FROM_CPU0, src::FROM_CPU0 + 1, src::FROM_CPU0 + 2, src::FROM_CPU0 + 3];
    0xc8 "CACHE" (cache) => [];
}
impl DeviceSet for Peripherals {
    const BASE: u32 = PERIPH_BASE;
    fn block_name(block: u32) -> &'static str { Peripherals::block_name(block) }
    fn misc(&self) -> &Misc { &self.misc }
    fn misc_mut(&mut self) -> &mut Misc { &mut self.misc }
    fn pre_access(&mut self, block: u32, _off: u32, _write: bool) {
        if block == 4 { let c = self.pcr.read(0x30); self.i2c.set_external_clock_config(Some(((c >> 12) & 255) | ((c & 63) << 8) | (((c >> 6) & 63) << 14) | (c & (1 << 20)))); }
        if block == 0x0e { self.adc.now_cycles = self.clock.cycles(); self.adc.ground = self.ana_mst.ana.read(0, 0x0769, |_| &0) & (1 << 21) != 0; }
        if block == 0xb2 { self.rng.now = self.clock.cycles() as u32; }
    }
}
impl Peripherals {
    pub fn new(mac: [u8; 6]) -> Self { Self {
        modem_bb: crate::wifi::ModemBb::new(), wifi_mac: crate::wifi::WifiMac::new(), adc: crate::adc::SarAdc::new(CPU_HZ), uart: std::array::from_fn(|_| Uart::new(UartLayout::C6)), usb: UsbSerialJtag::new(CPU_HZ), systimer: Systimer::new(),
        timg: [TimerGroup::new(), TimerGroup::new()], gpio: GpioC61 { gpio: { let mut g = Gpio::with_pins(30); g.input_select = 0x100; g.func_in_sel.fill(0x60); g } }, io_mux: RegRam::new(), i2c: esp_periph::i2c::I2c::new(),
        efuse: efuse_c61(mac, 1, 0, 0, 0, 0), spi0: SpiMemC61(SpiMem::new(false)), spi1: SpiMemC61(SpiMem::new(true)), sha: Sha::new(),
        gdma: GdmaC61::new(), spi2: GpSpi::new(), intmtx: IntMatrix::new(), intc: Intc::new(), cache: Cache::new(), lpsys: LpSys::new(), pcr: Pcr::new(),
        ana_mst: AnaMst::new(), rng: Rng::new(), misc: Misc::new(), spi_exec: false, clock: Self::new_clock(), ledc: Ledc::new(LedcLayout::C6),
    } }
    pub fn block_name(_block: u32) -> &'static str { "C61" }
    pub fn read32(&mut self, addr: u32) -> u32 { mmio::read32(self, addr) }
    pub fn write32(&mut self, addr: u32, v: u32) {
        if (0x60090000..=0x60090074).contains(&addr) { self.gpio.set_pad(((addr - 0x60090000) / 4) as u8, v); }
        if addr == 0x60004004 && v & (1 << 5) != 0 && self.i2c.has_pinned_devices() {
            let route = PINS.routes(&self.gpio, &self.io_mux); self.i2c.set_pins(route.i2c_pin(47).zip(route.i2c_pin(46)));
        }
        let fx = mmio::write32(self, addr, v); self.spi_exec |= fx.contains(WriteEffect::SPI_EXEC);
    }
    pub fn cpu_sub_read(&self, off: u32) -> u32 { self.intc.cpu_read(off) }
    pub fn cpu_sub_write(&mut self, off: u32, v: u32, lanes: u32) { self.intc.cpu_write(off, v, lanes); }
    pub fn cycles_until_timer(&self) -> u32 { Dispatch::cycles_until_deadline(self) }
    pub fn refresh_lines(&mut self) -> bool {
        let status = Dispatch::source_status(self); let changed = status != self.intmtx.status; self.intmtx.status = status;
        let mut input = 0u64;
        for i in 0..src::COUNT { let line = self.intmtx.map[i]; if (16..48).contains(&line) && status[i / 32] & (1 << (i % 32)) != 0 { input |= 1 << line; } }
        self.intc.clic.set_input(input); changed
    }
}
