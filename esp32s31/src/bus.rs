//! S31 memory/MMU, UART0, reset causes and CLIC/matrix windows; other MMIO faults explicitly.
use emu_core::{Bus, Fault};
use crate::interrupts::{Interrupts, MATRIX_BASE, CLIC_BASE, RESET_CAUSE0, RESET_CAUSE1};
use crate::trng::{Trng, TRNG_BASE, RNG_CTRL};
use crate::rtc_wdt::{RtcWdt, BASE as RTC_WDT_BASE};
use crate::cache::Cache;
use crate::pads::Pads;
use crate::periph::{Peripherals, SYSTIMER_CLOCK, USB_CLOCK, TIMG0_CLOCK, TIMG1_CLOCK};
use crate::clocks::Clocks;
use crate::startup_clocks::StartupClocks;
use crate::bus_monitor::BusMonitor;
use crate::mspi::Mspi;
use crate::mspi_pads::MspiPads;
use crate::psram_controller::PsramController;
use crate::sha::ShaEngine;
/// DR_REG_AES_BASE (IDF v6.1 soc/esp32s31/register/soc/reg_base.h)
const AES_BASE: u32 = 0x2050_8000;
use crate::analog::Analog;
use crate::power::Power;
use crate::pms::Permissions;
use crate::apm::Apm;
use crate::reset::ResetControl;
use crate::core_control::CoreControl;
use crate::rtc_timer::RtcTimer;

// ESP-IDF v6.1 (fff9895c82d744c7237be8847347bdd1b07c6643),
// components/soc/esp32s31/include/soc/soc.h and register/soc/reg_base.h.
pub const SRAM_BASE: u32 = 0x2f00_0000;
pub const SRAM_SIZE: usize = 0x80000;
pub const LP_BASE: u32 = 0x2e00_0000;
pub const LP_SIZE: usize = 0x8000;
pub const ROM_BASE: u32 = 0x2f80_0000;
pub const ROM_SIZE: usize = 0x50000;
pub const UART0_BASE: u32 = 0x2038_a000;
/// IDF v6.1 reg_base.h/interrupts.h: four HP UARTs, IRQ9..12. LP UART is NUM4.
pub const AUX_UART_BASE: [u32; 3] = [0x2038_b000, 0x2038_c000, 0x2038_d000];
const UART_SIZE: usize = 0xa0;
// S31 v6.1 lp_system_reg.h: sixteen R/W LP_SCRATCH registers, POR reset0.
// rtc.h names STORE4 bit0 ROM_LOG_CTRL. No reset/sleep domain sequencing is
// implemented yet; these retain software writes for the lifetime of this bus.
const LP_STORE0: u32 = 0x2070_002c;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessFault { pub hart: usize, pub pc: u32, pub address: u32, pub bytes: usize, pub write: bool, pub fetch: bool, pub reason: Fault }
/// reg_base.h DR_REG_USB_OTGHS_BASE.
pub const USB_OTG: u32 = 0x2030_0000;
/// reg_base.h DR_REG_GPSPI2_BASE. IDF master spi_reg.h for the S31 has the P4/S3 GP-SPI register
/// offsets (one extra field, CTRL.CLK_EDGE_SEL bit 30, kept as storage), so the shared `GpSpi` serves it.
pub const GPSPI2: u32 = 0x2038_f000;
/// reg_base.h DR_REG_I2C0_BASE / DR_REG_I2C1_BASE: the S3/P4 I2C master register layout.
/// reg_base.h DR_REG_GPSPI3_BASE: a second instance of the same GP-SPI block (IRQ source 5).
pub const GPSPI3: u32 = 0x2039_0000;
pub const I2C0: u32 = 0x2038_5000;
pub const I2C1: u32 = 0x2038_6000;
/// reg_base.h DR_REG_AXI_DMA_BASE: the P4's AXI-GDMA layout (three channel pairs, SPI2 trigger 1).
pub const AXI_DMA: u32 = 0x2034_8000;
/// reg_base.h DR_REG_PPA_BASE / DR_REG_DMA2D_BASE: the P4's PPA and 2D-DMA (esp_periph::ppa / dma2d).
pub const PPA: u32 = 0x2034_5000;
pub const DMA2D: u32 = 0x2034_6000;
/// reg_base.h DR_REG_AHB_DMA_BASE: the AHB-GDMA, `esp_periph::gdma::AhbDmaV2` (five channel pairs; I2S0 triggers 1-4, I2S1 5-8).
pub const AHB_DMA: u32 = 0x2034_3000;
/// reg_base.h DR_REG_I2S0_BASE / DR_REG_I2S1_BASE: the P4's I2S layout (`esp_periph::I2s::new_v2`).
pub const I2S: [u32; 2] = [0x2038_7000, 0x2038_8000];
/// HP_SYS_CLKRST I2Sn_TX_CTRL0 (clock enable, source, integer divider) and I2Sn_TX_DIV_CTRL0
/// (fractional divider) of I2S0 and I2S1 (IDF master hp_sys_clkrst_reg.h 0xf0/0xf4, 0x104/0x108).
const I2S_TX_CLOCK: [(u32, u32); 2] = [(0x2058_70f0, 0x2058_70f4), (0x2058_7104, 0x2058_7108)];
/// HP_SYS_CLKRST I2Sn_RX_CTRL0 / I2Sn_RX_DIV_CTRL0 (IDF master hp_sys_clkrst_reg.h 0xe8/0xec,
/// 0xfc/0x100; the TX words' field layout).
const I2S_RX_CLOCK: [(u32, u32); 2] = [(0x2058_70e8, 0x2058_70ec), (0x2058_70fc, 0x2058_7100)];

struct UsbDma<'a> { sram: &'a mut [u8], lp: &'a mut [u8], ver: &'a mut [u32] }
impl UsbDma<'_> {
    fn span(&mut self, addr: u32, n: usize) -> Result<&mut [u8], Fault> {
        for (base, data) in [(SRAM_BASE, &mut *self.sram), (LP_BASE, &mut *self.lp)] {
            if let Some(o) = offset(addr, base, n, data.len()) { return Ok(&mut data[o..o + n]); }
        }
        Err(Fault::Unmapped)
    }
}
impl esp_periph::sdmmc::SdmmcMemory for UsbDma<'_> {
    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Fault> { let n = buf.len(); buf.copy_from_slice(self.span(addr, n)?); Ok(()) }
    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Fault> { self.span(addr, data.len())?.copy_from_slice(data); bump_code(self.ver, addr, data.len()); Ok(()) }
}

/// 2D-DMA's AXI view: internal SRAM and LP RAM directly, flash and PSRAM through the MMU.
struct Dma2dMem<'a> { mem: UsbDma<'a>, cache: &'a mut Cache }
impl esp_periph::sdmmc::SdmmcMemory for Dma2dMem<'_> {
    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Fault> {
        if Cache::is_memory(addr) { return self.cache.dma_access(addr, None, buf); }
        esp_periph::sdmmc::SdmmcMemory::read(&mut self.mem, addr, buf)
    }
    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Fault> {
        if Cache::is_memory(addr) {
            // A write that faults part-way has still changed the pages before the fault.
            let r = self.cache.dma_access(addr, Some(data), &mut []);
            bump_psram_range(self.mem.ver, self.cache, addr, data.len());
            return r;
        }
        esp_periph::sdmmc::SdmmcMemory::write(&mut self.mem, addr, data)
    }
}

/// The DMA memory view as a `Bus`, for the shared IN-descriptor scatter (`GdmaInCh::receive`).
struct DmaBus<'a>(Dma2dMem<'a>);
impl DmaBus<'_> {
    fn get<const N: usize>(&mut self, a: u32) -> Result<[u8; N], Fault> { let mut b = [0u8; N]; esp_periph::sdmmc::SdmmcMemory::read(&mut self.0, a, &mut b).map(|_| b) }
    fn put(&mut self, a: u32, b: &[u8]) -> Result<(), Fault> { esp_periph::sdmmc::SdmmcMemory::write(&mut self.0, a, b) }
}
impl Bus for DmaBus<'_> {
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.get::<1>(a).map(|b| b[0]) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.get(a).map(u16::from_le_bytes) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.get(a).map(u32::from_le_bytes) }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.put(a, &[v]) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.put(a, &v.to_le_bytes()) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.put(a, &v.to_le_bytes()) }
    fn fetch(&mut self, _pc: u32) -> Result<[u8; 4], Fault> { Err(Fault::Unmapped) }
    fn note_code_page(&mut self, _vidx: u32) {}
}

struct AxiOut<'a> { mem: Dma2dMem<'a>, channel: &'a mut esp_periph::gdma::GdmaOutCh }
impl esp_periph::gdma::OutMemory for AxiOut<'_> {
    fn out_channel(&mut self) -> &mut esp_periph::gdma::GdmaOutCh { self.channel }
    fn word(&mut self, address: u32) -> Result<u32, Fault> {
        let mut b = [0u8; 4];
        esp_periph::sdmmc::SdmmcMemory::read(&mut self.mem, address, &mut b)?;
        Ok(u32::from_le_bytes(b))
    }
    fn append(&mut self, address: u32, count: usize, out: &mut Vec<u8>) -> Result<(), (u32, Fault)> {
        let start = out.len();
        out.resize(start + count, 0);
        esp_periph::sdmmc::SdmmcMemory::read(&mut self.mem, address, &mut out[start..]).map_err(|f| { out.truncate(start); (address, f) })
    }
    fn writeback(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        esp_periph::sdmmc::SdmmcMemory::write(&mut self.mem, address, &value.to_le_bytes())
    }
}

pub struct S31Bus {
    sram: Vec<u8>, lp: Vec<u8>, rom: Vec<u8>, lp_store: [u32; 16],
    /// `Bus::page_versions`: see `CODE_PAGES`
    code_ver: Vec<u32>,
    /// whether the PSRAM window can be fetched from (`psram_fetch_ok`), as last checked
    psram_fetch: bool,
    pub periph: Peripherals,
    pub interrupts: Interrupts,
    pub trng: Trng,
    pub rtc_wdt: RtcWdt,
    pub cache: Cache,
    pub pads: Pads,
    pub clocks: Clocks,
    pub startup_clocks: StartupClocks,
    pub bus_monitor: BusMonitor,
    pub mspi: Mspi,
    pub mspi_pads: MspiPads,
    pub psram_controller: PsramController,
    pub sha: ShaEngine,
    pub key_manager: crate::key_manager::KeyManager,
    pub analog: Analog,
    /// the modem baseband (`wifi.rs`)
    pub modem_bb: crate::wifi::ModemBb,
    /// the WiFi MAC and the virtual access point behind it (`wifi.rs`)
    pub wifi_mac: crate::wifi::WifiMac,
    pub power: Power,
    pub permissions: Permissions,
    pub apm: Apm,
    pub reset: ResetControl,
    pub cores: CoreControl,
    hart: usize, reservations: [Option<u32>; 2],
    pub rtc_calibration: crate::rtc_calibration::Calibration,
    pub rtc_timer: RtcTimer,
    pc: u32, pub first_fault: Option<AccessFault>,
    /// Peripheral registers no model claims: plain storage, as silicon accepts them. Word-addressed.
    pub unmodelled: std::collections::BTreeMap<u32, u32>,
    /// Peripheral accesses a model refused (`Prohibited`) outside APM/PMS: word address -> count.
    pub refused: std::collections::BTreeMap<u32, (u64, u32)>,
    /// The high-speed USB-OTG core (DWC2) at DR_REG_USB_OTGHS_BASE, IRQ source 99.
    pub usb_otg: esp_periph::usb_otg::Dwc2,
    /// GP-SPI2 (FSPI) at DR_REG_GPSPI2_BASE, IRQ source 4; the chip bus hands its transfers to the board.
    pub spi2: esp_periph::GpSpi,
    /// GP-SPI3 at DR_REG_GPSPI3_BASE; its transfers go to the board as host 3.
    pub spi3: esp_periph::GpSpi,
    /// I2C0 and I2C1 (IRQ sources 23 and 24); the board attaches its bus devices.
    pub i2c: [esp_periph::i2c::I2c; 2],
    /// AXI-GDMA: GP-SPI2's DMA (IRQ sources 48-50 IN, 51-53 OUT).
    pub axi_dma: esp_periph::gdma::AxiDma,
    pub lcd: crate::lcd::Lcd, pub rmt: crate::rmt::Rmt, pub sdmmc: crate::sdmmc::SdmmcS31,
    /// 2D-DMA (IRQ sources 105-107 IN, 108-111 OUT) and the PPA (source 102) it feeds.
    pub dma2d: esp_periph::dma2d::Dma2d,
    pub ppa: esp_periph::ppa::Ppa,
    /// AHB-GDMA (IRQ sources 36-40 IN, 41-45 OUT): the I2S controllers' DMA.
    pub ahb_dma: esp_periph::gdma::AhbDmaV2,
    /// I2S0 and I2S1 (IRQ sources 6 and 7).
    pub i2s: [esp_periph::I2s; 2],
    /// XTAL-domain remainder carried between ticks for the I2C controllers.
    pub i2c_frac: u64,
}
/// Decoded-code versions (`Bus::page_versions`): one counter per 256-byte page of HP SRAM and LP
/// RAM, one for the mask ROM, one for everything the flash window of the cache shows, then one
/// per 256-byte physical page of PSRAM (a page seen through any virtual alias has one version).
const CODE_PAGE_SHIFT: u32 = emu_core::bus::VPAGE_SHIFT;
const SRAM_PAGES: usize = SRAM_SIZE >> CODE_PAGE_SHIFT;
const LP_PAGES: usize = LP_SIZE >> CODE_PAGE_SHIFT;
const ROM_CODE: usize = SRAM_PAGES + LP_PAGES;
const FLASH_CODE: usize = ROM_CODE + 1;
const PSRAM_CODE: usize = FLASH_CODE + 1;
const CODE_PAGES: usize = PSRAM_CODE;
/// Every bus starts its versions where no earlier bus's (before a chip reset) can still be.
static CODE_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// A write of `n` bytes at `addr` may have changed code: move the versions of the SRAM or LP RAM
/// pages it touches, and of the page before when it reaches into the first three bytes (a
/// four-byte instruction there starts on that page).
#[inline]
fn bump_code(ver: &mut [u32], addr: u32, n: usize) {
    let (index, size) = if (SRAM_BASE..SRAM_BASE + SRAM_SIZE as u32).contains(&addr) { (addr - SRAM_BASE, 0) }
        else if (LP_BASE..LP_BASE + LP_SIZE as u32).contains(&addr) { (addr - LP_BASE, SRAM_PAGES) }
        else { return; };
    let pages = if size == 0 { SRAM_PAGES } else { LP_PAGES };
    let first = (index.saturating_sub(emu_core::bus::PREV_PAGE_BYTES) >> CODE_PAGE_SHIFT) as usize;
    let last = ((index as usize + n.max(1) - 1) >> CODE_PAGE_SHIFT).min(pages - 1);
    for v in &mut ver[size + first..=size + last] { *v = v.wrapping_add(1); }
}
/// A write at physical PSRAM offset `phys`: its page, and the page before when it reaches into
/// the first three bytes.
#[inline]
fn bump_psram(ver: &mut [u32], phys: usize) {
    for p in [phys.saturating_sub(emu_core::bus::PREV_PAGE_BYTES as usize) >> CODE_PAGE_SHIFT, phys >> CODE_PAGE_SHIFT] {
        if let Some(v) = ver.get_mut(PSRAM_CODE + p) { *v = v.wrapping_add(1); }
    }
}
/// A write of `n` bytes at PSRAM-window address `addr`, page by page through the MMU.
fn bump_psram_range(ver: &mut [u32], cache: &Cache, addr: u32, n: usize) {
    let page = 1usize << CODE_PAGE_SHIFT;
    let mut a = addr as usize;
    let end = addr as usize + n.max(1);
    while a < end {
        if let Some(phys) = cache.psram_phys(a as u32) { bump_psram(ver, phys); }
        a = (a & !(page - 1)) + page;
    }
    if let Some(phys) = cache.psram_phys((end - 1) as u32) { bump_psram(ver, phys); }
}
fn offset(address: u32, base: u32, bytes: usize, size: usize) -> Option<usize> {
    let off = address.checked_sub(base)? as usize;
    off.checked_add(bytes).filter(|&end| end <= size).map(|_| off)
}
impl Default for S31Bus { fn default() -> Self { Self::new() } }
impl S31Bus {
    /// S31 sec_ll.h: XTAL, RC_FAST, BBPLL/2; the root must actually be supplied
    /// and powered. SHA compression is functional/synchronous, not timed silicon.
    pub fn crypto_clock_hz(&self) -> Option<u64> {
        match self.sha.clock_source() {
            0 => Some(40_000_000),
            1 => { let hz = self.clocks.rc_fast_hz(); if hz != 0 { Some(hz) } else { None } }
            2 => self.clocks.ref_240m_hz(self.mspi.bbpll_root_hz()),
            _ => None,
        }
    }
    pub fn new() -> Self {
        // S31 v6.1 uart_reg.h: RX threshold/count 8 bits, RXFIFO_RST bit 22 at 0x20;
        // FIFO=0, STATUS=0x1c, REG_UPDATE=0x98. Same functional layout as C6, checked
        // against S31 headers. This reuses FIFO behavior, not C6's address map/clock tree.
        let epoch = CODE_EPOCH.fetch_add(1 << 20, std::sync::atomic::Ordering::Relaxed);
        Self { sram: vec![0; SRAM_SIZE], lp: vec![0; LP_SIZE], rom: vec![0; ROM_SIZE], lp_store: [0; 16], code_ver: vec![epoch; CODE_PAGES], psram_fetch: false,
            periph: Peripherals::new(), interrupts: Interrupts::default(), trng: Trng::default(), rtc_wdt: RtcWdt::default(), cache: Cache::default(), pads: Pads::default(), clocks: Clocks::default(), startup_clocks: StartupClocks::default(), bus_monitor: BusMonitor::default(), mspi: Mspi::default(), mspi_pads: MspiPads::default(), psram_controller: PsramController::default(), sha: ShaEngine::default(), key_manager: crate::key_manager::KeyManager::default(), analog: Analog::default(), modem_bb: Default::default(), wifi_mac: Default::default(), power: Power::default(), permissions: Permissions::default(), apm: Apm::default(), reset: ResetControl::default(), cores: CoreControl::default(), hart: 0, reservations: [None; 2], rtc_timer: RtcTimer::default(), rtc_calibration: crate::rtc_calibration::Calibration::default(), pc: 0, first_fault: None, unmodelled: Default::default(), refused: Default::default(),
            // Same HS UTMI DWC2 instance as the P4 (esp_hal_usb/esp32s31/usb_dwc_periph.c); its
            // GHWCFG words are assumed equal to the P4's until read from an S31.
            usb_otg: esp_periph::usb_otg::Dwc2::new(esp_periph::usb_otg::Dwc2Config::esp32p4_hs(crate::periph::CPU_HZ)),
            spi2: esp_periph::GpSpi::new().diverting_dma_rx(), spi3: esp_periph::GpSpi::with_host(3).diverting_dma_rx(), i2c: [esp_periph::i2c::I2c::new(), esp_periph::i2c::I2c::new()], i2c_frac: 0, axi_dma: esp_periph::gdma::AxiDma::new(), lcd: Default::default(), rmt: Default::default(), sdmmc: Default::default(), dma2d: esp_periph::dma2d::Dma2d::new(), ppa: esp_periph::ppa::Ppa::new(),
            ahb_dma: esp_periph::gdma::AhbDmaV2::new(), i2s: [esp_periph::I2s::new_v2(crate::periph::CPU_HZ), esp_periph::I2s::new_v2(crate::periph::CPU_HZ)] }
    }
    fn memory(&self, address: u32, bytes: usize) -> Option<&[u8]> {
        for (base, data) in [(SRAM_BASE, &self.sram), (LP_BASE, &self.lp), (ROM_BASE, &self.rom)] {
            if let Some(o) = offset(address, base, bytes, data.len()) { return Some(&data[o..o + bytes]); }
        }
        None
    }
    fn memory_mut(&mut self, address: u32, bytes: usize, loader: bool) -> Option<&mut [u8]> {
        for (base, data) in [(SRAM_BASE, &mut self.sram), (LP_BASE, &mut self.lp), (ROM_BASE, &mut self.rom)] {
            if base == ROM_BASE && !loader { continue; }
            if let Some(o) = offset(address, base, bytes, data.len()) { return Some(&mut data[o..o + bytes]); }
        }
        None
    }
    pub fn check_load(&self, address: u32, bytes: usize) -> Result<(), String> {
        self.memory(address, bytes).map(|_| ()).ok_or_else(|| format!("S31 ELF segment outside SRAM/ROM: {address:#010x}, {bytes} bytes"))
    }
    /// HP SRAM, LP RAM or the mask ROM.
    #[inline(always)]
    pub fn is_ram_or_rom(address: u32) -> bool {
        (SRAM_BASE..SRAM_BASE + SRAM_SIZE as u32).contains(&address) || (LP_BASE..LP_BASE + LP_SIZE as u32).contains(&address) || (ROM_BASE..ROM_BASE + ROM_SIZE as u32).contains(&address)
    }
    /// Decoded code may no longer match anything: after loading or changing memory or the
    /// configuration an instruction fetch depends on from outside the guest's own accesses.
    pub fn invalidate_code(&mut self) { for v in &mut self.code_ver { *v = v.wrapping_add(1); } }
    fn invalidate_psram_code(&mut self) { for v in &mut self.code_ver[PSRAM_CODE..] { *v = v.wrapping_add(1); } }
    /// In-package PSRAM of `bytes` (0 = none), with a code version per page.
    pub fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> {
        self.cache.set_psram_size(bytes)?;
        let epoch = self.code_ver[0];
        self.code_ver.resize(PSRAM_CODE + bytes.div_ceil(1 << CODE_PAGE_SHIFT), epoch);
        self.invalidate_code();
        Ok(())
    }
    /// What `fetch` requires of the PSRAM side before it reads the window through the cache.
    fn psram_fetch_ok(&self) -> bool {
        self.clocks.psram_on() && self.power.policy.psram_enabled() && self.mspi_pads.check_transfer(self.psram_controller.mode().is_some()).is_ok() && self.psram_controller.memory_ready(false).is_ok()
    }
    /// Move every PSRAM code version when `psram_fetch_ok` changes. After every device write,
    /// and from `SocBus::tick`, because the PLL behind the PSRAM clock settles with time.
    pub fn check_psram_fetch(&mut self) {
        let ok = self.psram_fetch_ok();
        if ok != self.psram_fetch { self.psram_fetch = ok; self.invalidate_psram_code(); }
    }
    pub fn load(&mut self, address: u32, data: &[u8], memsz: usize) -> Result<(), String> {
        if data.len() > memsz { return Err("S31 ELF data exceeds segment size".into()); }
        self.invalidate_code();
        let dest = self.memory_mut(address, memsz, true).ok_or("S31 ELF segment outside SRAM/ROM")?;
        dest.fill(0); dest[..data.len()].copy_from_slice(data); Ok(())
    }
    fn fault<T>(&mut self, address: u32, bytes: usize, write: bool, fetch: bool, reason: Fault) -> Result<T, Fault> {
        if self.first_fault.is_none() { self.first_fault = Some(AccessFault { hart: self.hart, pc: self.pc, address, bytes, write, fetch, reason }); }
        Err(reason)
    }
    fn uart_irq(&self) -> bool { self.clocks.uart_bus_on() && self.clocks.uart_core_on() && self.periph.uart0.irq() }
    fn psram_address_error(&mut self, address: u32, bytes: usize, write: bool, reason: Fault) {
        if reason == Fault::Unmapped && self.cache.invalid_psram_mapping(address, bytes) { self.psram_controller.invalid_axi_mapping(write); }
    }
    pub fn uart_input(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        if !self.clocks.uart_bus_on() || !self.clocks.uart_core_on() || self.periph.uart0.read(0x88) & (1 << 25 | 1 << 27) != 1 << 25 { return Err(Fault::Prohibited); }
        self.periph.uart0.host_input(bytes); Ok(())
    }
    pub fn uart_baud(&mut self) -> Option<u64> {
        let div = self.periph.uart0.read(0x14); let divisor = u64::from(div & 0xfff) * 16 + u64::from((div >> 20) & 15);
        (divisor != 0 && self.clocks.uart_sclk_hz() != 0).then(|| self.clocks.uart_sclk_hz() * 16 / divisor)
    }
    fn uart_core_reset(&mut self) {
        let conf = self.periph.uart0.read(0x20); self.periph.uart0.write(0x20, conf | 1 << 22); self.periph.uart0.write(0x20, conf);
        self.periph.uart0.int_raw = (1 << 14) | 2;
    }
    fn aux_uart_ready(&self, index: usize) -> (bool, bool, u64) {
        let (bus, core, hz) = self.clocks.aux_uart_clock_with_pll(index, self.mspi.bbpll_root_hz().map(|hz| hz / 6)).unwrap_or((false, false, 0));
        (bus, bus && core && hz != 0 && self.clocks.aux_uart_mem_on(index), hz)
    }
    fn aux_uart_reset(&mut self, index: usize, apb: bool) {
        let uart = &mut self.periph.uart_aux[index];
        if apb { let output = std::mem::take(&mut uart.tx_out); *uart = crate::periph::new_uart(); uart.tx_out = output; self.periph.uart_aux_update[index] = false; }
        else { let conf = uart.read(0x20); uart.write(0x20, conf | 1 << 22); uart.write(0x20, conf); uart.int_raw = (1 << 14) | 2; }
    }
    /// Cold host endpoint only. No board peer or module response is synthesized.
    pub fn uart_port_input(&mut self, port: u8, bytes: &[u8]) -> Result<(), Fault> {
        if port == 0 { return self.uart_input(bytes); }
        let i = usize::from(port - 1); if i >= 3 { return Err(Fault::Unmapped); }
        if !self.aux_uart_ready(i).1 || self.periph.uart_aux[i].read(0x88) & (1 << 25 | 1 << 27) != 1 << 25 { return Err(Fault::Prohibited); }
        self.periph.uart_aux[i].host_input(bytes); Ok(())
    }
    pub fn uart_port_baud(&mut self, port: u8) -> Option<u64> {
        if port == 0 { return self.uart_baud(); }
        let i = usize::from(port - 1); if i >= 3 { return None; }
        let (bus, ready, hz) = self.aux_uart_ready(i); let div = self.periph.uart_aux[i].read(0x14);
        let divisor = u64::from(div & 0xfff) * 16 + u64::from(div >> 20 & 15);
        (bus && ready && divisor != 0).then(|| hz * 16 / divisor)
    }
    /// A GP-SPI's DMA data phase: the AXI-GDMA OUT channel on its trigger (SPI2 1, SPI3 2) gathers the bytes from
    /// internal SRAM / LP RAM, then the transfer goes on as on the P4 (esp32p4 bus.rs spi2_dma_tx).
    /// A channel not started yet is retried at the next tick.
    pub(crate) fn spi_dma_tx(&mut self, host: u8) -> bool {
        let trigger = u32::from(host - 1);
        let spi = if host == 2 { &mut self.spi2 } else { &mut self.spi3 };
        let Some(bits) = spi.dma_tx_pending else { return false };
        let Some(ch) = self.axi_dma.gdma.out_channel_for(trigger) else { return false };
        let mut data = Vec::with_capacity((bits as usize).div_ceil(8));
        let mut memory = AxiOut { mem: Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache }, channel: &mut self.axi_dma.gdma.out[ch] };
        let result = esp_periph::gdma::gather_out(&mut memory, (bits as usize).div_ceil(8), &mut data);
        let c = &mut self.axi_dma.gdma.out[ch];
        c.running = false; c.buf_pos = 0;
        match result { Ok(next) => { c.desc = next; c.int_raw |= 1 << 3; } Err(_) => c.int_raw |= 1 << 2 }
        let spi = if host == 2 { &mut self.spi2 } else { &mut self.spi3 };
        spi.complete_dma_tx(&data);
        true
    }
    /// AES DMA mode (IDF v6.1 esp_crypto_shared_gdma.c): the AXI-GDMA OUT channel on trigger 4
    /// (esp_hal_dma esp32s31 gdma_channel.h SOC_GDMA_TRIG_PERIPH_AES0) gathers the input, the engine
    /// transforms it in its block mode, the IN channel on trigger 4 scatters the result (SUC_EOF at
    /// the end). Done in one step, as on the C3 (esp32c3 bus.rs aes_dma_step); while a channel is not
    /// started yet the operation waits for the next AES or AXI-GDMA write.
    fn aes_dma_step(&mut self) {
        const TRIGGER: u32 = 4;
        let (Some(out_ch), Some(in_ch)) = (self.axi_dma.gdma.out_channel_for(TRIGGER), self.axi_dma.gdma.in_channel_for(TRIGGER)) else { return };
        self.periph.aes.dma_pending = false;
        let mut input = Vec::new();
        let mut memory = AxiOut { mem: Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache }, channel: &mut self.axi_dma.gdma.out[out_ch] };
        let result = esp_periph::gdma::gather_out(&mut memory, usize::MAX, &mut input);
        let c = &mut self.axi_dma.gdma.out[out_ch];
        c.running = false; c.buf_pos = 0;
        match result { Ok(next) => { c.desc = next; c.int_raw |= 1 << 3; } Err(_) => c.int_raw |= 1 << 2 }   // OUT_TOTAL_EOF / OUT_DSCR_ERR
        let output = self.periph.aes.transform_blocks(&input);
        let mut channel = self.axi_dma.gdma.inp[in_ch];
        let mut changed = false;
        let mut mem = DmaBus(Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache });
        if channel.receive(&mut mem, &output, Some(output.len() as u32), false, Self::peripheral, &mut changed) { channel.running = false; channel.desc = 0; }
        self.axi_dma.gdma.inp[in_ch] = channel;
        self.periph.aes.state = 2;                                       // DONE until DMA_EXIT
        self.periph.aes.int_raw |= 1;
    }

    /// One LCD descriptor (or its currently due prefix) per scheduler turn. Even a circular
    /// bounce chain yields at EOF, and a pending enabled EOF waits for the guest ISR. This
    /// provides refill backpressure; it does not claim a cycle-exact LCD FIFO underrun model.
    /// v6.1 axi_dma_reg.h: CHECK_OWNER bit12, AUTO_WRBACK bit2; OUT IRQ bits0..3.
    pub(crate) fn lcd_dma_step(&mut self, cycles: u64) -> bool {
        use esp_periph::gdma::OutMemory;
        self.lcd.advance(cycles);
        let due = self.lcd.due();
        if due == 0 { return false; }
        let Some(ch) = self.axi_dma.gdma.out_channel_for(crate::lcd::DMA_TRIGGER) else { return false };
        let c = self.axi_dma.gdma.out[ch];
        if c.int_raw & c.int_ena & 2 != 0 { return false; }
        let mut memory = AxiOut { mem: Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache }, channel: &mut self.axi_dma.gdma.out[ch] };
        let result = (|| -> Result<(), Fault> {
            if c.desc == 0 || c.desc & 3 != 0 { return Err(Fault::Misaligned); }
            let (control, d) = esp_periph::gdma::DescriptorWalk::new(1).read_with(c.desc, |a| memory.word(a)).map_err(|_| Fault::Prohibited)?;
            if d.length == 0 || d.length > d.size || c.buf_pos > d.length || c.conf1 & (1 << 12) != 0 && !d.owner_dma { return Err(Fault::Prohibited); }
            let take = due.min((d.length - c.buf_pos) as usize);
            let address = d.buf.checked_add(c.buf_pos).ok_or(Fault::Unmapped)?;
            memory.append(address, take, &mut self.lcd.data).map_err(|(_, fault)| fault)?;
            memory.channel.buf_pos += take as u32;
            if memory.channel.buf_pos == d.length {
                if c.conf0 & (1 << 2) != 0 { memory.writeback(c.desc, control & !(1 << 31))?; }
                let c = &mut memory.channel;
                c.int_raw |= 1;
                if d.eof { c.int_raw |= 2; c.eof_desc = c.desc; }
                c.desc = d.next; c.buf_pos = 0;
                if d.next == 0 { c.running = false; if d.eof { c.int_raw |= 8; } }
            }
            Ok(())
        })();
        if result.is_err() { memory.channel.running = false; memory.channel.int_raw |= 4; self.lcd.fail(); }
        memory.channel.int_raw != c.int_raw || result.is_err()
    }
    /// I2Sn's transmit DMA through the AHB-GDMA, paced by the frame rate, as the P4 runs its
    /// I2S1 (esp32p4 bus.rs `i2s1_tx`): the frames due since the last call are taken from the OUT
    /// channel bound to one of the controller's four triggers (I2S0 1-4, I2S1 5-8; IDF's driver
    /// uses the first), a descriptor at a time, each written back to the CPU when AUTO_WRBACK is
    /// set, with OUT_DONE, OUT_EOF on a SUC_EOF one and OUT_TOTAL_EOF when it ends the chain. The
    /// buffers may be in PSRAM: the memory view is the 2D-DMA's. The first slot of each frame
    /// goes to the audio sink. True when a channel interrupt may have changed.
    pub(crate) fn i2s_tx(&mut self, n: usize, cycles: u64) -> bool {
        let (frames, bpf, sample_bytes) = { let i = &mut self.i2s[n]; (i.frames_due(cycles), i.bytes_per_frame as usize, i.sample_bytes()) };
        if frames == 0 || bpf == 0 { return false; }
        let first = 1 + 4 * n as u32;
        let Some(ch) = (first..first + 4).find_map(|t| self.ahb_dma.gdma.out_channel_for(t)) else { return false };
        let mut need = frames as usize * bpf;
        let mut stream: Vec<u8> = Vec::with_capacity(need);
        let mut mem = Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache };
        let mut walk = esp_periph::gdma::DescriptorWalk::new(esp_periph::gdma::GDMA_DESCRIPTOR_STEP_BUDGET);
        let gdma = &mut self.ahb_dma.gdma;
        while need != 0 {
            let c = gdma.out[ch];
            if !c.running || c.desc == 0 { break; }
            let read = walk.read_with(c.desc, |a| { let mut b = [0u8; 4]; esp_periph::sdmmc::SdmmcMemory::read(&mut mem, a, &mut b).map(|_| u32::from_le_bytes(b)) });
            let (dw0, d) = match read {
                Ok(read) => read,
                Err(esp_periph::gdma::DmaDescriptorFault::StepBudgetExceeded { .. }) => break,
                Err(_) => { let c = &mut gdma.out[ch]; c.running = false; c.int_raw |= 1 << 2; break; }   // OUT_DSCR_ERR
            };
            let remaining = d.length.saturating_sub(c.buf_pos) as usize;
            if remaining == 0 {
                if c.conf0 & (1 << 2) != 0 { let _ = esp_periph::sdmmc::SdmmcMemory::write(&mut mem, c.desc, &(dw0 & !(1 << 31)).to_le_bytes()); }   // AUTO_WRBACK: owner -> CPU
                let c = &mut gdma.out[ch];
                c.int_raw |= 1 << 0;                                                    // OUT_DONE
                if dw0 & (1 << 30) != 0 { c.int_raw |= 1 << 1; c.eof_desc = c.desc; }   // OUT_EOF
                if d.next == 0 { c.running = false; c.desc = 0; if d.eof { c.int_raw |= 1 << 3; } break; }   // OUT_TOTAL_EOF
                c.desc = d.next; c.buf_pos = 0;
                continue;
            }
            let take = remaining.min(need);
            let start = stream.len();
            stream.resize(start + take, 0);
            if esp_periph::sdmmc::SdmmcMemory::read(&mut mem, d.buf.wrapping_add(c.buf_pos), &mut stream[start..]).is_err() {
                stream.truncate(start);
                let c = &mut gdma.out[ch]; c.running = false; c.int_raw |= 1 << 2; break;
            }
            gdma.out[ch].buf_pos += take as u32;
            need -= take;
        }
        let samples: Vec<i16> = stream.chunks_exact(bpf)
            .map(|f| if sample_bytes == 1 { (f[0] as i8 as i16) << 8 } else { i16::from_le_bytes([f[sample_bytes - 2], f[sample_bytes - 1]]) })
            .collect();
        let i = &mut self.i2s[n];
        i.frames_out += samples.len() as u64; i.pcm.extend_from_slice(&samples);
        true
    }
    /// I2Sn's receive DMA through the AHB-GDMA: the frames due since the last call (the RX clock,
    /// or the TX clock for a full-duplex slave) from the controller's input (`rx_input`: silence,
    /// `--i2s-tone`, `--mic-wav`) into the IN channel bound to one of its triggers, through the
    /// shared scatter: IN_DONE per filled descriptor, IN_SUC_EOF every RX_EOF_NUM bytes. Input
    /// pauses while no IN channel runs. True when a channel interrupt may have changed.
    pub(crate) fn i2s_rx(&mut self, n: usize, cycles: u64) -> bool {
        let first = 1 + 4 * n as u32;
        let Some(ch) = (first..first + 4).find_map(|t| self.ahb_dma.gdma.in_channel_for(t)) else { return false };
        let bytes = self.i2s[n].receive_input(cycles, false);
        if bytes.is_empty() { self.i2s[n].recycle_rx_buffer(bytes); return false; }
        let eof = self.i2s[n].read(0x64) & 0xfff;
        let mut channel = self.ahb_dma.gdma.inp[ch];
        let mut changed = false;
        let mut mem = DmaBus(Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache });
        let _ = channel.receive(&mut mem, &bytes, Some(eof), false, Self::peripheral, &mut changed);
        self.ahb_dma.gdma.inp[ch] = channel;
        self.i2s[n].bytes_in += bytes.len() as u64;
        self.i2s[n].recycle_rx_buffer(bytes);
        changed
    }
    /// Run the 2D-DMA transfers (M2M, PPA) whose channels are all started; count refusals.
    fn run_dma2d(&mut self) {
        let mut mem = Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache };
        for r in self.dma2d.service(&mut self.ppa, &mut mem) {
            self.refused.entry(if r.ppa { PPA } else { DMA2D } + r.off).or_insert((0, 0)).0 += 1;
        }
    }
    /// What the WiFi MAC's DMA sees: internal SRAM and LP RAM, PSRAM and flash through the MMU.
    fn wifi_mem(&mut self) -> Dma2dMem<'_> { Dma2dMem { mem: UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver }, cache: &mut self.cache } }
    /// SDMMC uses the same AXI SRAM/PSRAM DMA view as Wi-Fi and LCD, including code-page
    /// invalidation on IDMAC writes. Called only for a pending command or resumed transfer.
    pub(crate) fn run_sdmmc(&mut self, now_ns: u64, slot: Option<([u8; 6], bool)>) {
        let mut host = std::mem::take(&mut self.sdmmc); let mpll = self.clocks.mpll.output_hz();
        host.run(&mut self.wifi_mem(), now_ns, slot, mpll); self.sdmmc = host;
    }
    fn wifi_word(&mut self, addr: u32) -> u32 {
        let mut b = [0u8; 4];
        let _ = esp_periph::sdmmc::SdmmcMemory::read(&mut self.wifi_mem(), addr, &mut b);
        u32::from_le_bytes(b)
    }
    /// WiFi transmit, as on the C6 (`esp32c6::bus`): the frames the library queued come out of
    /// their descriptors (word 0 bits 27:14 the packet's length, word 1 the packet: an 8-byte
    /// header, the frame's length in its first word's bits 13:0, then the frame), complete at
    /// once, and go to the access point.
    pub(crate) fn wifi_tx_step(&mut self, now_us: u64, frames_log: bool) {
        for (queue, low) in std::mem::take(&mut self.wifi_mac.tx_pending) {
            let desc = self.wifi_mac.addr(low);
            let (dw0, pkt) = (self.wifi_word(desc), self.wifi_word(desc.wrapping_add(4)));
            const TX_HEADER: usize = 8;
            let mut packet = vec![0u8; ((dw0 >> 14) & 0x3fff) as usize];
            if esp_periph::sdmmc::SdmmcMemory::read(&mut self.wifi_mem(), pkt, &mut packet).is_err() { packet.clear(); }
            let len = packet.get(..4).map_or(0, |w| (u32::from_le_bytes(w.try_into().unwrap()) & 0x3fff) as usize);
            let frame = packet.get(TX_HEADER..TX_HEADER + len).unwrap_or_default().to_vec();
            if frame.is_empty() && !packet.is_empty() { eprintln!("[wifi] TX queue {queue}: a {}-byte packet whose header says {len} bytes of frame; not sent", packet.len()); }
            if self.wifi_mac.log || frames_log { eprintln!("[wifi] {:.6}s TX queue {queue} desc {desc:#010x} {}", now_us as f64 / 1e6, esp_soc::wifi::describe(&frame)); }
            self.wifi_mac.tx_done(queue);
            self.wifi_mac.link.station_tx(&frame, now_us);
        }
    }
    /// The virtual air: what the access point has due and what the network sends the station, one
    /// frame at a time into the RX ring (`StationLink::next_rx`). True when a frame went in.
    pub(crate) fn wifi_air_step(&mut self, now_us: u64) -> bool {
        if self.wifi_mac.link.nothing_due(now_us) { return false; }
        let last_desc = self.wifi_mac.link.last_rx_desc();
        let busy = last_desc != 0 && self.wifi_word(last_desc) & esp_soc::wifi::RX_DESC_HAS_DATA != 0;
        match self.wifi_mac.link.next_rx(now_us, busy) { Some(frame) => { self.wifi_rx_deliver(&frame, now_us); true } None => false }
    }
    /// One received frame into the next RX descriptor behind the control header the S31's
    /// `wDev_ProcessRxSucData` reads (`rxctrl:64`): 56 fixed bytes (the C6's first 21 at the same
    /// places: byte 0 the RSSI, byte 3 bits 4 and 5 the address match, bytes 12..15 the
    /// timestamp; byte 43 bit 7 announces a channel-estimate dump, none here), then 8 bytes
    /// (`get_sublen_offset`: 56 without a dump) with the frame's length, FCS included, in bits
    /// 13:0 and the receive state in byte 4 (0, good), then the frame and its FCS.
    fn wifi_rx_deliver(&mut self, frame: &[u8], now_us: u64) {
        const RX_CTRL: usize = 64;
        if self.wifi_mac.rx_next == 0 { self.wifi_mac.link.drop_rx_unarmed(); return; }
        let desc = self.wifi_mac.addr(self.wifi_mac.rx_next);
        let log = self.wifi_mac.link.ap().is_some_and(|ap| ap.log);
        let (dw0, buf, next) = (self.wifi_word(desc), self.wifi_word(desc.wrapping_add(4)), self.wifi_word(desc.wrapping_add(8)));
        let total = RX_CTRL + frame.len() + 4;
        if dw0 & (1 << 31) == 0 || dw0 & (1 << 30) != 0 || ((dw0 & 0x3fff) as usize) < total { self.wifi_mac.link.drop_rx(); return; }
        let group = frame.len() >= 5 && frame[4] & 1 == 1;
        let mut words = [0u32; RX_CTRL / 4];
        let rssi = self.wifi_mac.link.rx_meta(frame).1;
        words[0] = (rssi as u8 as u32) | 1 << 28 | if group { 0 } else { 1 << 29 };
        words[3] = now_us as u32;
        words[5] = 0xa6;
        words[14] = (frame.len() as u32 + 4) & 0x3fff;
        words[15] = 0;
        let mut b: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        b.extend_from_slice(frame);
        b.extend_from_slice(&esp_soc::wifi::fcs(frame).to_le_bytes());
        if esp_periph::sdmmc::SdmmcMemory::write(&mut self.wifi_mem(), buf, &b).is_err() { self.wifi_mac.link.drop_rx(); return; }
        let filled = (dw0 & !(0x3fff << 14)) | (total as u32) << 14 | 1 << 30;
        let _ = esp_periph::sdmmc::SdmmcMemory::write(&mut self.wifi_mem(), desc, &filled.to_le_bytes());
        self.wifi_mac.rx_filled(desc, next);
        if log { eprintln!("[wifi] RX -> desc {desc:#010x} (was {dw0:#010x}, next {next:#010x}) buf {buf:#010x} {}", esp_soc::wifi::describe(frame)); }
    }
    /// Let the DWC2 move what its DMA owes, against internal SRAM and LP RAM.
    pub(crate) fn run_usb_otg(&mut self) {
        let mut dma = UsbDma { sram: &mut self.sram, lp: &mut self.lp, ver: &mut self.code_ver };
        self.usb_otg.run(&mut dma);
    }
    pub(crate) fn peripheral_irqs(&self) -> crate::interrupts::Irqs { crate::interrupts::Irqs { uart: self.uart_irq(), uart_aux: std::array::from_fn(|i| self.periph.uart_aux[i].irq() && self.aux_uart_ready(i).1), wdt: self.rtc_wdt.irq(), efuse: self.periph.efuse.irq(), usb: self.periph.usb.irq(), monitor: self.bus_monitor.irq(), cache: self.cache.irq(), mwdt: [self.periph.timg[0].irq_wdt(), self.periph.timg[1].irq_wdt()], systimer: self.periph.systimer.irqs(), psram: self.psram_controller.irq(), brownout: self.power.mode0.irq(), from_cpu: self.cores.from_cpu_irqs(), usb_otg: self.usb_otg.irq(), spi2: self.spi2.irq(), spi3: self.spi3.irq(), i2c: [self.i2c[0].irq(), self.i2c[1].irq()], gpio: self.pads.irq_lines(), axi_dma: esp_periph::Device::irq_sources(&self.axi_dma), aes: self.periph.aes.irq(), dma2d: self.dma2d.irq_bits() as u8, ppa: self.ppa.irq(), wifi_mac: self.wifi_mac.irq(), ahb_dma: esp_periph::Device::irq_sources(&self.ahb_dma), i2s: [self.i2s[0].irq(), self.i2s[1].irq()], lcd: self.lcd.irq(), rmt: self.rmt.irq(), sdmmc: self.sdmmc.irq() } }
    pub(crate) fn set_hart(&mut self, hart: usize) { self.hart = hart; }
    pub(crate) fn reset_hart(&mut self, hart: usize) { self.reservations[hart] = None; self.interrupts.clic[hart] = Default::default(); }
    pub fn refresh_irq(&mut self, threshold: u32) -> Option<u32> {
        if !self.cores.clic_on(self.hart) { return None; }
        self.interrupts.refresh_core(self.hart, self.peripheral_irqs(), threshold)
    }
    pub fn interrupt_taken(&mut self, line: u32) { if line < 32 { self.interrupts.clic[self.hart].taken(line as usize + 16); } }
    fn clic_offset(&self, address: u32, bytes: usize) -> Option<(usize, u32)> {
        // Fixed S31 clic_reg.h names only 0x10800000. The actual ROM uses this
        // same address on both harts. No public evidence for a +64KiB remote alias.
        let off = address.checked_sub(CLIC_BASE)?;
        off.checked_add(bytes as u32).filter(|&end| end <= 0x2000).map(|_| (self.hart, off))
    }
    fn clic_register(off: u32) -> bool { matches!(off & !3, 0 | 4 | 0x1000..=0x10bc) }
    /// HP and LP peripheral space (soc/reg_base.h: DR_REG_*_BASE between 0x2000_0000 and 0x20ff_ffff).
    fn peripheral(address: u32) -> bool { (0x2000_0000..0x2100_0000).contains(&address) }
    /// An access no model claims in peripheral space reads what was last written there (0 at
    /// first) instead of faulting: silicon answers every decoded register, and IDF touches many
    /// we do not model. `unmodelled` lists them for `--debug` reports.
    /// A model's `Prohibited` in peripheral space that the APM and PMS would let through: a value
    /// or clock state the model does not cover, which silicon accepts. Real permission faults stay.
    fn refusal_only(&mut self, address: u32, bytes: usize, write: bool) -> bool {
        Self::peripheral(address) && self.apm.check(address, bytes, if write { 1 } else { 2 }).is_ok() && self.permissions.check(address, write).is_ok()
    }
    fn read(&mut self, address: u32, bytes: usize) -> Result<u32, Fault> {
        // SRAM, LP RAM and ROM first: with no APM route, `read_modelled` passes the APM, the PMS
        // has no target there and no device before `memory` claims these addresses, so it would
        // return the same bytes after a dozen range tests.
        // The cache windows likewise: no device before `Cache::is_memory` in the chain claims
        // them either, and a fault there is not a peripheral's, so `read` returns it unchanged.
        if self.apm.unrouted() {
            if let Some(data) = self.memory(address, bytes) {
                let mut value = [0; 4]; value[..bytes].copy_from_slice(data); return Ok(u32::from_le_bytes(value));
            }
            if Cache::is_memory(address) { return self.cache_window_read(address, bytes); }
        }
        let had = self.first_fault.is_some();
        match self.read_modelled(address, bytes) {
            Err(Fault::Prohibited) if self.refusal_only(address, bytes, false) => {
                if !had { self.first_fault = None; }
                self.refused.entry(address & !3).or_insert((0, 0)).0 += 1;
                Ok(0)
            }
            Err(Fault::Unmapped) if Self::peripheral(address) => {
                if !had { self.first_fault = None; }
                let word = *self.unmodelled.entry(address & !3).or_insert(0);
                let shift = (address & 3) * 8;
                Ok(if bytes == 4 { word } else { (word >> shift) & ((1u32 << (bytes * 8)) - 1) })
            }
            r => r,
        }
    }
    fn write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        // SRAM, LP RAM and the cache windows first, as in `read`; no I2S clock word lives there.
        if self.apm.unrouted() {
            if let Some(dest) = self.memory_mut(address, bytes, false) {
                dest.copy_from_slice(&value.to_le_bytes()[..bytes]);
                bump_code(&mut self.code_ver, address, bytes);
                return Ok(());
            }
            if Cache::is_memory(address) {
                self.cache_window_write(address, value, bytes)?;
                for a in [address, address + bytes as u32 - 1] { if let Some(phys) = self.cache.psram_phys(a) { bump_psram(&mut self.code_ver, phys); } }
                return Ok(());
            }
        }
        let flash = (self.mspi.flash_accessible(), self.mspi.flash_generation);
        let r = self.write_unhooked(address, value, bytes);
        if r.is_ok() { self.note_code_write(address, bytes, flash); }
        if let Some(n) = I2S_TX_CLOCK.iter().position(|&(c, d)| address & !3 == c || address & !3 == d) {
            let (c, d) = I2S_TX_CLOCK[n];
            let (ctrl0, div) = (self.read(c, 4).unwrap_or(0), self.read(d, 4).unwrap_or(0));
            self.i2s[n].set_v2_clock(ctrl0, div);
        }
        if let Some(n) = I2S_RX_CLOCK.iter().position(|&(c, d)| address & !3 == c || address & !3 == d) {
            let (c, d) = I2S_RX_CLOCK[n];
            let (ctrl0, div) = (self.read(c, 4).unwrap_or(0), self.read(d, 4).unwrap_or(0));
            self.i2s[n].set_v2_rx_clock(ctrl0, div);
        }
        r
    }
    /// After a successful guest write: move the versions of what it may have changed of the
    /// code `fetch` would read. Memory: its pages. The cache windows: the MMU, L1-enable and
    /// cache clock registers (`Cache::affects_fetch`). The flash window also: an SPI1 command that may have changed the array
    /// (`Mspi::flash_generation`), and any write that changes whether the flash is readable. Everything: the APM and PMS, which gate fetches
    /// everywhere. PSRAM is versioned per physical page through the MMU (`bump_psram_range`).
    #[inline]
    fn note_code_write(&mut self, address: u32, bytes: usize, (flash_fetch, flash_generation): (bool, u64)) {
        if (SRAM_BASE..ROM_BASE).contains(&address) || (LP_BASE..LP_BASE + LP_SIZE as u32).contains(&address) { bump_code(&mut self.code_ver, address, bytes); return; }
        if (crate::cache::PSRAM_VADDR..0x5400_0000).contains(&address) {
            for a in [address, address + bytes as u32 - 1] { if let Some(phys) = self.cache.psram_phys(a) { bump_psram(&mut self.code_ver, phys); } }
            return;
        }
        if !Self::peripheral(address) && !Cache::is_mmio(address) { return; }
        if Apm::is_mmio(address) || Permissions::is_mmio(address) { self.invalidate_code(); return; }
        if Cache::affects_fetch(address) || address & !3 == crate::cache::CLOCK_CTRL {
            self.code_ver[FLASH_CODE] = self.code_ver[FLASH_CODE].wrapping_add(1);
            self.invalidate_psram_code();
        } else if flash_generation != self.mspi.flash_generation || flash_fetch != self.mspi.flash_accessible() {
            self.code_ver[FLASH_CODE] = self.code_ver[FLASH_CODE].wrapping_add(1);
        }
        // An MSPI2 command may move PSRAM contents.
        if address & !3 == crate::psram_controller::BASE || address & !3 == crate::psram_controller::BASE + 0x1000 { self.invalidate_psram_code(); }
        self.check_psram_fetch();
    }
    fn write_unhooked(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        let had = self.first_fault.is_some();
        match self.write_modelled(address, value, bytes) {
            Err(Fault::Prohibited) if self.refusal_only(address, bytes, true) => {
                if !had { self.first_fault = None; }
                let e = self.refused.entry(address & !3).or_insert((0, 0)); e.0 += 1; e.1 = value;
                Ok(())
            }
            Err(Fault::Unmapped) if Self::peripheral(address) => {
                if !had { self.first_fault = None; }
                let word = self.unmodelled.entry(address & !3).or_insert(0);
                if bytes == 4 { *word = value; } else {
                    let shift = (address & 3) * 8; let mask = ((1u32 << (bytes * 8)) - 1) << shift;
                    *word = (*word & !mask) | ((value << shift) & mask);
                }
                Ok(())
            }
            r => r,
        }
    }
    /// A CPU load from the flash or PSRAM window of the cache.
    fn cache_window_read(&mut self, address: u32, bytes: usize) -> Result<u32, Fault> {
        if address >= crate::cache::PSRAM_VADDR && (!self.clocks.psram_on() || !self.power.policy.psram_enabled() || self.mspi_pads.check_transfer(self.psram_controller.mode().is_some()).is_err() || self.psram_controller.memory_ready(false).is_err()) { return self.fault(address, bytes, false, false, Fault::Prohibited); }
        if address < crate::cache::PSRAM_VADDR && !self.mspi.flash_accessible() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
        let mut value = [0; 4];
        for (i, byte) in value.iter_mut().take(bytes).enumerate() {
            match address.checked_add(i as u32).ok_or(Fault::Unmapped).and_then(|a| self.cache.read_byte_core(a, false, self.hart)) {
                Ok(v) => *byte = v, Err(e) => { self.psram_address_error(address, bytes, false, e); return self.fault(address, bytes, false, false, e); },
            }
        }
        Ok(u32::from_le_bytes(value))
    }
    /// A CPU store to the cache windows (the flash window refuses it).
    fn cache_window_write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if address >= crate::cache::PSRAM_VADDR && (!self.clocks.psram_on() || !self.power.policy.psram_enabled() || self.mspi_pads.check_transfer(self.psram_controller.mode().is_some()).is_err() || self.psram_controller.memory_ready(true).is_err()) { return self.fault(address, bytes, true, false, Fault::Prohibited); }
        match self.cache.write(address, value, bytes) { Ok(()) => Ok(()), Err(e) => { self.psram_address_error(address, bytes, true, e); self.fault(address, bytes, true, false, e) } }
    }
    fn read_modelled(&mut self, address: u32, bytes: usize) -> Result<u32, Fault> {
        if let Err(e) = self.apm.check(address, bytes, 2) { return self.fault(address, bytes, false, false, e); }
        if let Err(e) = self.permissions.check(address, false) { return self.fault(address, bytes, false, false, e); }
        if crate::sdmmc::SdmmcS31::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.sdmmc.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if crate::lcd::Lcd::is_mmio(address) || crate::rmt::Rmt::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(if crate::lcd::Lcd::is_mmio(address) { self.lcd.read(address) } else { self.rmt.read(address) });
        }
        if (USB_OTG..USB_OTG + 0x4_0000).contains(&address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(esp_periph::Device::read(&mut self.usb_otg, address - USB_OTG));
        }
        if let Some(off) = offset(address, GPSPI2, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.spi2.read(off as u32));
        }
        if let Some(off) = offset(address, GPSPI3, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.spi3.read(off as u32));
        }
        if let Some(off) = offset(address, AXI_DMA, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(esp_periph::Device::read(&mut self.axi_dma, off as u32));
        }
        if let Some(off) = offset(address, DMA2D, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(esp_periph::Device::read(&mut self.dma2d, off as u32));
        }
        if let Some(off) = offset(address, AHB_DMA, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(esp_periph::Device::read(&mut self.ahb_dma, off as u32));
        }
        for (n, base) in I2S.into_iter().enumerate() {
            if let Some(off) = offset(address, base, bytes, 0x100) {
                if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
                return Ok(self.i2s[n].read(off as u32));
            }
        }
        if let Some(off) = offset(address, PPA, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(esp_periph::Device::read(&mut self.ppa, off as u32));
        }
        for (n, base) in [I2C0, I2C1].into_iter().enumerate() {
            if let Some(off) = offset(address, base, bytes, 0x100) {
                if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
                return Ok(self.i2c[n].read(off as u32));
            }
        }
        if address & !3 == crate::rtc_timer::RESET || address & !0xfff == crate::rtc_timer::BASE {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if address == crate::rtc_timer::RESET { return Ok(self.rtc_timer.reset_read()); }
            return match self.rtc_timer.read(address & 0xfff) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if address & !3 == SYSTIMER_CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.periph.systimer.clock_read());
        }
        if address >> 12 == 0x20399 {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if !self.periph.systimer.clock_on() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            return Ok(self.periph.read32(address));
        }
        if crate::rtc_calibration::Calibration::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.rtc_calibration.read(address, self.periph.timg[0].wdt.clock_read()) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(id) = [TIMG0_CLOCK, TIMG1_CLOCK].iter().position(|&c| address & !3 == c) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.periph.timg[id].wdt.clock_read());
        }
        if matches!(address >> 12, 0x20580 | 0x20581) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            let id = if address >> 12 == 0x20580 { 0 } else { 1 };
            // Preserve MWDT gate Faults for the WDT half; timer regs go through DeviceSet.
            if (address & 0xfff) >= 0x48 {
                return match self.periph.timg[id].wdt.read(address & 0xfff) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
            }
            return Ok(self.periph.read32(address));
        }
        if CoreControl::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.cores.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if ResetControl::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.reset.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Apm::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.apm.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Permissions::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.permissions.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(data) = self.memory(address, bytes) {
            let mut value = [0; 4]; value[..bytes].copy_from_slice(data); return Ok(u32::from_le_bytes(value));
        }
        if address & !3 == crate::mspi::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.mspi.clock_read());
        }
        if address & !3 == crate::key_manager::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.key_manager.clock_read());
        }
        if let Some(off) = offset(address, crate::key_manager::BASE, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.key_manager.read(off as u32) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if address & !3 == crate::sha::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.sha.clock_read());
        }
        if let Some(off) = offset(address, crate::sha::BASE, bytes, 0x300) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if self.crypto_clock_hz().is_none() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            return match self.sha.read(off as u32) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        // AES shares the crypto clock with SHA (block mode, and DMA mode through the AXI-GDMA).
        // Its per-engine aes_clk_en / aes_rst_en (HP_SYS_CLKRST) are not modelled, as for SHA.
        if let Some(off) = offset(address, AES_BASE, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if self.crypto_clock_hz().is_none() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            return Ok(self.periph.aes.read(off as u32));
        }
        if address & !3 == crate::cache::CLOCK_CTRL {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.cache.clock_read());
        }
        if address & !3 == crate::bus_monitor::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.bus_monitor.clock_read());
        }
        if let Some(off) = offset(address, crate::bus_monitor::BASE, bytes, 0x400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.bus_monitor.read(off as u32) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(off) = offset(address, LP_STORE0, bytes, 64) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.lp_store[off / 4]);
        }
        if address & !3 == USB_CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.periph.usb.clock_read());
        }
        if address >> 12 == 0x20391 {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if !self.periph.usb.clock_on() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            return Ok(self.periph.read32(address));
        }
        if address >> 12 == 0x20715 {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.periph.efuse.read(address & 0xfff) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(i) = Clocks::aux_uart_mem_index(address & !3) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.clocks.aux_uart_mem_read(i));
        }
        if matches!(address & !3, crate::clocks::UART0_CTRL | crate::clocks::UART0_MEM) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.clocks.uart_read(address));
        }
        if Power::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.power.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if (crate::wifi::BB_BASE..crate::wifi::BB_END).contains(&address) {
            return Ok(crate::wifi::read_sized(&mut self.modem_bb, address - crate::wifi::BB_BASE, bytes));
        }
        if (crate::wifi::MAC_BASE..crate::wifi::MAC_END).contains(&address) {
            return Ok(crate::wifi::read_sized(&mut self.wifi_mac, address - crate::wifi::MAC_BASE, bytes));
        }
        if Analog::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.analog.read(address, self.clocks.modem_on()) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Clocks::is_lp_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.clocks.lp_read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Clocks::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.clocks.read(address - crate::clocks::BASE) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if StartupClocks::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.startup_clocks.read(address) { Ok(v)=>Ok(v),Err(e)=>self.fault(address,bytes,false,false,e) };
        }
        if MspiPads::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.mspi_pads.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Pads::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.pads.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Cache::is_memory(address) { return self.cache_window_read(address, bytes); }
        if Cache::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.cache.read_register(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(off) = offset(address, crate::mspi::BASE, bytes, 0x1400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.mspi.read(off / 0x1000, (off & 0xfff) as u32) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(off) = offset(address, crate::psram_controller::BASE, bytes, 0x1400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if !self.clocks.psram_bus_on() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            return match self.psram_controller.read(off / 0x1000, (off & 0xfff) as u32) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(off) = offset(address, RTC_WDT_BASE, bytes, 0x400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if let Some(value) = self.rtc_wdt.read(off as u32) { return Ok(value); }
            return self.fault(address, bytes, false, false, Fault::Unmapped);
        }
        if address & !3 == RNG_CTRL {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.trng.ctrl_read());
        }
        if let Some(off) = offset(address, TRNG_BASE, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if let Some(value) = self.trng.read(off as u32) { return Ok(value); }
            return self.fault(address, bytes, false, false, Fault::Unmapped);
        }
        if matches!(address & !3, RESET_CAUSE0 | RESET_CAUSE1) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.interrupts.reset_read(((address - RESET_CAUSE0) / 4) as usize));
        }
        if let Some(off) = offset(address, MATRIX_BASE, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if let Some(value) = self.interrupts.matrix_read(off >> 11, (off & 0x7ff) as u32, self.peripheral_irqs()) { return Ok(value); }
            return self.fault(address, bytes, false, false, Fault::Unmapped);
        }
        if let Some((core, off)) = self.clic_offset(address, bytes) {
            if !self.cores.clic_on(core) { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            if !address.is_multiple_of(bytes as u32) { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if !Self::clic_register(off) { return self.fault(address, bytes, false, false, Fault::Unmapped); }
            let word = self.interrupts.clic[core].read(off & !3);
            return Ok((word >> ((off & 3) * 8)) & ((1u64 << (bytes * 8)) - 1) as u32);
        }
        if let Some((i, o)) = AUX_UART_BASE.iter().enumerate().find_map(|(i, &base)| offset(address, base, bytes, UART_SIZE).map(|o| (i, o))) {
            if bytes != 4 || o & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            let (bus, ready, _) = self.aux_uart_ready(i);
            if !bus || o == 0 && !ready { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            if o == 0x98 { if ready { self.periph.uart_aux_update[i] = false; } return Ok(u32::from(self.periph.uart_aux_update[i])); }
            return Ok(self.periph.uart_aux[i].read(o as u32));
        }
        if let Some(o) = offset(address, UART0_BASE, bytes, UART_SIZE) {
            if bytes == 4 && o.is_multiple_of(4) {
                if !self.clocks.uart_bus_on() || o == 0 && !self.clocks.uart_core_on() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
                return Ok(self.periph.uart0.read(o as u32));
            }
            return self.fault(address, bytes, false, false, Fault::Misaligned);
        }
        self.fault(address, bytes, false, false, Fault::Unmapped)
    }
    fn write_modelled(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if let Err(e) = self.apm.check(address, bytes, 1) { return self.fault(address, bytes, true, false, e); }
        if let Err(e) = self.permissions.check(address, true) { return self.fault(address, bytes, true, false, e); }
        if crate::sdmmc::SdmmcS31::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.sdmmc.write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if crate::lcd::Lcd::is_mmio(address) || crate::rmt::Rmt::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if crate::lcd::Lcd::is_mmio(address) { self.lcd.write(address, value); } else { self.rmt.write(address, value); }
            return Ok(());
        }
        if (USB_OTG..USB_OTG + 0x4_0000).contains(&address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            esp_periph::Device::write(&mut self.usb_otg, address - USB_OTG, value);
            if self.usb_otg.pending() { self.run_usb_otg(); }
            return Ok(());
        }
        if let Some(off) = offset(address, GPSPI2, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.spi2.write(off as u32, value);
            return Ok(());
        }
        if let Some(off) = offset(address, GPSPI3, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.spi3.write(off as u32, value);
            return Ok(());
        }
        if let Some(off) = offset(address, AXI_DMA, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            esp_periph::Device::write(&mut self.axi_dma, off as u32, value);
            if self.periph.aes.dma_pending { self.aes_dma_step(); }
            return Ok(());
        }
        if let Some(off) = offset(address, DMA2D, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            esp_periph::Device::write(&mut self.dma2d, off as u32, value);
            self.run_dma2d();
            return Ok(());
        }
        if let Some(off) = offset(address, AHB_DMA, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            esp_periph::Device::write(&mut self.ahb_dma, off as u32, value);
            return Ok(());
        }
        for (n, base) in I2S.into_iter().enumerate() {
            if let Some(off) = offset(address, base, bytes, 0x100) {
                if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
                self.i2s[n].write(off as u32, value);
                return Ok(());
            }
        }
        if let Some(off) = offset(address, PPA, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            esp_periph::Device::write(&mut self.ppa, off as u32, value);
            self.run_dma2d();
            return Ok(());
        }
        for (n, base) in [I2C0, I2C1].into_iter().enumerate() {
            if let Some(off) = offset(address, base, bytes, 0x100) {
                if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
                if self.i2c[n].has_pinned_devices() { self.i2c[n].set_pins(self.pads.i2c_pins(n)); }
                self.i2c[n].write(off as u32, value);
                return Ok(());
            }
        }
        if address & !3 == crate::rtc_timer::RESET || address & !0xfff == crate::rtc_timer::BASE {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if address == crate::rtc_timer::RESET { self.rtc_timer.reset_write(value); return Ok(()); }
            return match self.rtc_timer.write(address & 0xfff, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if address & !3 == SYSTIMER_CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.periph.systimer.clock_write(value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if address >> 12 == 0x20399 {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if !self.periph.systimer.clock_on() { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            self.periph.write32(address, value); return Ok(());
        }
        if crate::rtc_calibration::Calibration::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.rtc_calibration.write(address, value, self.periph.timg[0].wdt.clock_read()) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(id) = [TIMG0_CLOCK, TIMG1_CLOCK].iter().position(|&c| address & !3 == c) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.periph.timg[id].wdt.clock_write(value) { Ok(()) => { if id == 0 && value & 2 != 0 { self.rtc_calibration.reset(); } Ok(()) }, Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if matches!(address >> 12, 0x20580 | 0x20581) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            let id = if address >> 12 == 0x20580 { 0 } else { 1 };
            if (address & 0xfff) >= 0x48 {
                return match self.periph.timg[id].wdt.write(address & 0xfff, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
            }
            self.periph.write32(address, value); return Ok(());
        }
        if CoreControl::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.cores.write(address, value) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if ResetControl::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.reset.write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Apm::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.apm.write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Permissions::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.permissions.write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(dest) = self.memory_mut(address, bytes, false) { dest.copy_from_slice(&value.to_le_bytes()[..bytes]); return Ok(()); }
        if address & !3 == crate::mspi::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.mspi.set_cpll_input(self.clocks.cpll_rate());
            return match self.mspi.clock_write(value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if address & !3 == crate::key_manager::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.key_manager.clock_write(value); return Ok(());
        }
        if let Some(off) = offset(address, crate::key_manager::BASE, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.key_manager.write(off as u32, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if address & !3 == crate::sha::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.sha.clock_write(value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, crate::sha::BASE, bytes, 0x300) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if self.crypto_clock_hz().is_none() { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            return match self.sha.write(off as u32, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, AES_BASE, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if self.crypto_clock_hz().is_none() { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            self.periph.aes.write(off as u32, value);
            if self.periph.aes.dma_pending { self.aes_dma_step(); }
            return Ok(());
        }
        if address & !3 == crate::cache::CLOCK_CTRL {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.cache.clock_write(value); return Ok(());
        }
        if address & !3 == crate::bus_monitor::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.bus_monitor.clock_write(value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, crate::bus_monitor::BASE, bytes, 0x400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.bus_monitor.write(off as u32, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, LP_STORE0, bytes, 64) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.lp_store[off / 4] = value; return Ok(());
        }
        if address & !3 == USB_CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.periph.usb.clock_write(value); return Ok(());
        }
        if address >> 12 == 0x20391 {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if !self.periph.usb.clock_on() { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            self.periph.write32(address, value); return Ok(());
        }
        if address >> 12 == 0x20715 {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.periph.efuse.write(address & 0xfff, value) {
                Ok(()) => { self.rtc_wdt.delay_sel = self.periph.efuse.wdt_delay_selector(); Ok(()) }
                Err(e) => self.fault(address, bytes, true, false, e),
            };
        }
        if let Some(i) = Clocks::aux_uart_mem_index(address & !3) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.clocks.aux_uart_mem_write(i, value); return Ok(());
        }
        if matches!(address & !3, crate::clocks::UART0_CTRL | crate::clocks::UART0_MEM) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if let Err(e) = self.clocks.uart_write(address, value) { return self.fault(address, bytes, true, false, e); }
            if address == crate::clocks::UART0_CTRL {
                if value & 8 != 0 {
                    // uart_ll_reset_register pulses APB_RST to reset the module. Captured
                    // bytes already transmitted belong to host evidence, not UART memory.
                    let output = std::mem::take(&mut self.periph.uart0.tx_out); self.periph.uart0 = Peripherals::new().uart0; self.periph.uart0.tx_out = output;
                } else if value & 4 != 0 { self.uart_core_reset(); }
            }
            return Ok(());
        }
        if Power::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            // IDF sequences PLL power around the CPU clock switch itself; a write that would stop
            // the running CPU clock is not refused here (silicon would stall; IDF never does it).
            return match self.power.write(address, value) {
                Ok(()) => { self.clocks.set_cpll_power(self.power.cpll_enabled()); self.clocks.mpll.set_power(self.power.mpll_enabled()); self.clocks.set_fosc_power(self.power.fosc_enabled()); self.mspi.set_bbpll_gate(self.power.bbpll_output_enabled()); Ok(()) }
                Err(e) => self.fault(address, bytes, true, false, e),
            };
        }
        if (crate::wifi::BB_BASE..crate::wifi::BB_END).contains(&address) {
            crate::wifi::write_sized(&mut self.modem_bb, address - crate::wifi::BB_BASE, value, bytes); return Ok(());
        }
        if (crate::wifi::MAC_BASE..crate::wifi::MAC_END).contains(&address) {
            crate::wifi::write_sized(&mut self.wifi_mac, address - crate::wifi::MAC_BASE, value, bytes); return Ok(());
        }
        if Analog::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.analog.write(address, value, self.clocks.modem_on()) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Clocks::is_lp_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.clocks.lp_write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Clocks::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.clocks.write(address - crate::clocks::BASE, value) { Ok(()) => {
                if address == crate::clocks::BASE + 0x68 && value & 4 != 0 { self.psram_controller.reset(); }
                if (crate::clocks::UART0_CTRL + 4..=crate::clocks::UART0_CTRL + 12).contains(&address) && value & 12 != 0 {
                    self.aux_uart_reset(((address - crate::clocks::UART0_CTRL) / 4 - 1) as usize, value & 8 != 0);
                }
                Ok(()) }, Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if StartupClocks::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if address == StartupClocks::GPSPI2_CTRL0 && value & 1 << 2 != 0 { let log = self.spi2.log; self.spi2 = esp_periph::GpSpi::new().diverting_dma_rx(); self.spi2.log = log; }   // RST_EN held: registers to reset
            return match self.startup_clocks.write(address,value) { Ok(())=>Ok(()),Err(e)=>self.fault(address,bytes,true,false,e) };
        }
        if MspiPads::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.mspi_pads.write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Pads::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.pads.write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Cache::is_memory(address) { return self.cache_window_write(address, value, bytes); }
        if Cache::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.cache.write_register(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, crate::mspi::BASE, bytes, 0x1400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.mspi.write(off / 0x1000, (off & 0xfff) as u32, value, self.cache.flash_bytes_mut()) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, crate::psram_controller::BASE, bytes, 0x1400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if !self.clocks.psram_bus_on() { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            if off & 0xfff == 0 && value != 0 && (!self.clocks.psram_on() || !self.power.policy.psram_enabled() || self.mspi_pads.check_transfer(true).is_err()) { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            return match self.psram_controller.write(off / 0x1000, (off & 0xfff) as u32, value, self.cache.psram_bytes_mut()) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, RTC_WDT_BASE, bytes, 0x400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if self.rtc_wdt.write(off as u32, value) { return Ok(()); }
            return self.fault(address, bytes, true, false, Fault::Unmapped);
        }
        if address & !3 == RNG_CTRL {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.trng.ctrl_write(value); return Ok(());
        }
        if let Some(off) = offset(address, TRNG_BASE, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if self.trng.write(off as u32, value) { return Ok(()); }
            return self.fault(address, bytes, true, false, Fault::Unmapped);
        }
        if matches!(address & !3, RESET_CAUSE0 | RESET_CAUSE1) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.interrupts.reset_write(((address - RESET_CAUSE0) / 4) as usize, value); return Ok(());
        }
        if let Some(off) = offset(address, MATRIX_BASE, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if self.interrupts.matrix_write(off >> 11, (off & 0x7ff) as u32, value) { return Ok(()); }
            return self.fault(address, bytes, true, false, Fault::Unmapped);
        }
        if let Some((core, off)) = self.clic_offset(address, bytes) {
            if !self.cores.clic_on(core) { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            if !address.is_multiple_of(bytes as u32) { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if !Self::clic_register(off) { return self.fault(address, bytes, true, false, Fault::Unmapped); }
            let lane = off & 3;
            self.interrupts.clic[core].write(off & !3, value << (lane * 8), ((1u32 << bytes) - 1) << lane); return Ok(());
        }
        if let Some((i, o)) = AUX_UART_BASE.iter().enumerate().find_map(|(i, &base)| offset(address, base, bytes, UART_SIZE).map(|o| (i, o))) {
            if bytes != 4 || o & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            let (bus, ready, _) = self.aux_uart_ready(i);
            if !bus || o == 0 && (!ready || self.periph.uart_aux[i].read(0x88) & (1 << 24 | 1 << 26) != 1 << 24) { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            if o == 0x88 && value & (1 << 27) != 0 { self.aux_uart_reset(i, false); }
            if o == 0x98 { self.periph.uart_aux_update[i] = value & 1 != 0 && !ready; }
            self.periph.uart_aux[i].write(o as u32, value); return Ok(());
        }
        if let Some(o) = offset(address, UART0_BASE, bytes, UART_SIZE) {
            if bytes == 4 && o.is_multiple_of(4) {
                if !self.clocks.uart_bus_on() || o == 0 && (!self.clocks.uart_core_on() || self.periph.uart0.read(0x88) & (1 << 24 | 1 << 26) != 1 << 24) { return self.fault(address, bytes, true, false, Fault::Prohibited); }
                if o == 0x88 && value & (1 << 27) != 0 { self.uart_core_reset(); }
                self.periph.uart0.write(o as u32, value); return Ok(());
            }
            return self.fault(address, bytes, true, false, Fault::Misaligned);
        }
        let reason = if offset(address, ROM_BASE, bytes, ROM_SIZE).is_some() { Fault::Prohibited } else { Fault::Unmapped };
        self.fault(address, bytes, true, false, reason)
    }
}
impl Bus for S31Bus {
    // A whole-address-space reservation set conservatively permits SC failure on
    // unrelated stores, but never misses an intervening write through a virtual alias.
    fn reserve(&mut self, address: u32) { self.reservations[self.hart] = Some(address); }
    fn reservation_held(&mut self, address: u32) -> bool { self.reservations[self.hart].take() == Some(address) }
    fn note_pc(&mut self, pc: u32) { self.pc = pc; }
    fn note_code_page(&mut self, _vidx: u32) {} // Every write that can change code moves its version (`note_code_write`).
    #[inline(always)]
    fn page_versions(&self) -> &[u32] { &self.code_ver }
    /// `CODE_PAGES`; `u32::MAX` (no version) outside SRAM, LP RAM, ROM, the flash window and
    /// mapped PSRAM. A PSRAM pc has its physical page's version under the current mapping; any
    /// change to the PSRAM MMU moves them all.
    #[inline(always)]
    fn code_page(&mut self, pc: u32) -> u32 {
        if (SRAM_BASE..SRAM_BASE + SRAM_SIZE as u32).contains(&pc) { (pc - SRAM_BASE) >> CODE_PAGE_SHIFT }
        else if (LP_BASE..LP_BASE + LP_SIZE as u32).contains(&pc) { SRAM_PAGES as u32 + ((pc - LP_BASE) >> CODE_PAGE_SHIFT) }
        else if (ROM_BASE..ROM_BASE + ROM_SIZE as u32).contains(&pc) { ROM_CODE as u32 }
        else if (crate::cache::FLASH_VADDR..crate::cache::PSRAM_VADDR).contains(&pc) { FLASH_CODE as u32 }
        else if let Some(phys) = self.cache.psram_phys(pc) { let i = PSRAM_CODE + (phys >> CODE_PAGE_SHIFT); if i < self.code_ver.len() { i as u32 } else { u32::MAX } }
        else { u32::MAX }
    }
    /// `fetch` without its fault records: the same APM gate, memories and cache paths.
    fn fetch_ahead(&mut self, pc: u32) -> Option<[u8; 4]> {
        let mut data = [0; 4];
        for i in 0..4 {
            if i == 2 && data[0] & 3 != 3 { break; }
            let address = pc.checked_add(i as u32)?;
            if !self.apm.allows(address, 0) { return None; }
            if let Some(src) = self.memory(address, 1) { data[i] = src[0]; }
            else if Cache::is_memory(address) {
                if address >= crate::cache::PSRAM_VADDR { if !self.psram_fetch_ok() { return None; } }
                else if !self.mspi.flash_accessible() { return None; }
                data[i] = self.cache.fetch_byte_quietly(address, self.hart)?;
            } else { return None; }
        }
        Some(data)
    }
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.read(a, 1).map(|v| v as u8) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.read(a, 2).map(|v| v as u16) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.read(a, 4) }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.write(a, v as u32, 1)?; self.reservations = [None; 2]; Ok(()) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.write(a, v as u32, 2)?; self.reservations = [None; 2]; Ok(()) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.write(a, v, 4)?; self.reservations = [None; 2]; Ok(()) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        let mut data = [0; 4];
        for i in 0..4 {
            if i == 2 && data[0] & 3 != 3 { break; } // A compressed instruction needs only two bytes.
            let Some(address) = pc.checked_add(i as u32) else { return self.fault(pc, 4, false, true, Fault::Unmapped); };
            if let Err(e) = self.apm.check(address, 1, 0) { return self.fault(pc, 4, false, true, e); }
            if let Some(src) = self.memory(address, 1) { data[i] = src[0]; }
            else if Cache::is_memory(address) {
                if address >= crate::cache::PSRAM_VADDR && (!self.clocks.psram_on() || !self.power.policy.psram_enabled() || self.mspi_pads.check_transfer(self.psram_controller.mode().is_some()).is_err() || self.psram_controller.memory_ready(false).is_err()) { return self.fault(pc, 4, false, true, Fault::Prohibited); }
                if address < crate::cache::PSRAM_VADDR && !self.mspi.flash_accessible() { return self.fault(pc, 4, false, true, Fault::Prohibited); }
                match self.cache.read_byte_core(address, true, self.hart) {
                    Ok(v) => data[i] = v, Err(e) => { self.psram_address_error(address, 1, false, e); return self.fault(pc, 4, false, true, e); },
                }
            } else { return self.fault(pc, 4, false, true, Fault::Unmapped); }
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hp_uart_ports_apply_update_route_irqs_and_never_invent_peer_responses() {
        let mut b = S31Bus::new(); b.mspi.set_bbpll_input(Some(480_000_000)).unwrap();
        b.write32(UART0_BASE, b'X' as u32).unwrap();
        for (i, base) in AUX_UART_BASE.into_iter().enumerate() {
            let clock = crate::clocks::UART0_CTRL + 4 * (i as u32 + 1);
            b.write32(clock, 0xc3).unwrap(); // PLL_F80M, bus/core enabled
            b.write32(base + 0x88, 3 << 24).unwrap();
            b.write32(base + 0x14, 694 | (7 << 20)).unwrap();
            b.write32(base + 0x98, 1).unwrap(); assert_eq!(b.read32(base + 0x98), Ok(0));
            assert_eq!(b.uart_port_baud(i as u8 + 1), Some(115201));
            for byte in b"AT\r\n" { b.write32(base, u32::from(*byte)).unwrap(); }
            assert_eq!(b.periph.uart_aux[i].tx_out, b"AT\r\n");
            assert_eq!(b.read32(base + 0x1c).unwrap() & 255, 0, "no attached module, no invented AT response");
            b.write32(base + 0x24, 1).unwrap(); b.write32(base + 0xc, 1).unwrap();
            b.uart_port_input(i as u8 + 1, &[0x40 + i as u8]).unwrap();
            assert!(b.peripheral_irqs().sources().contains(&(10 + i, true)));
            assert_eq!(b.read32(base), Ok(0x40 + i as u32));
            assert!(!b.peripheral_irqs().sources().contains(&(10 + i, true)));
        }
        assert_eq!(b.periph.uart0.tx_out, b"X"); assert!(b.periph.usb.usj.tx_out.is_empty());
        assert_eq!(b.uart_port_input(4, b"bad"), Err(Fault::Unmapped)); assert_eq!(b.uart_port_baud(255), None);
        assert!(b.unmodelled.is_empty() && b.refused.is_empty());
    }
    #[test]
    fn hp_uart_clock_memory_and_resets_gate_fifo_and_pending_update() {
        let mut b = S31Bus::new(); let base = AUX_UART_BASE[0]; let clock = crate::clocks::UART0_CTRL + 4;
        let mem = crate::clocks::AUX_UART_MEM[0]; assert_eq!(b.read32(mem), Ok(6));
        b.write32(base + 0x24, 1).unwrap(); b.write32(base + 0xc, 1).unwrap(); b.uart_port_input(1, b"A").unwrap();
        b.write32(clock, 0x3).unwrap(); // bus alive, core clock disabled
        assert!(!b.peripheral_irqs().uart_aux[0]); assert_eq!(b.uart_port_input(1, b"B"), Err(Fault::Prohibited));
        b.write32(base + 0x98, 1).unwrap(); assert_eq!(b.read32(base + 0x98), Ok(1));
        b.write32(clock, 0x83).unwrap(); assert_eq!(b.read32(base + 0x98), Ok(0)); assert_eq!(b.read32(base), Ok(65));
        b.uart_port_input(1, b"RX").unwrap(); b.write32(clock, 0x87).unwrap(); b.write32(clock, 0x83).unwrap();
        assert_eq!(b.periph.uart_aux[0].rx_pending(), 0); assert_eq!(b.read32(base + 0xc), Ok(1), "core reset retains APB config");
        b.write32(base, 67).unwrap(); b.write32(clock, 0x8b).unwrap(); b.write32(clock, 0x83).unwrap();
        assert_eq!(b.read32(base + 0xc), Ok(0)); assert_eq!(b.periph.uart_aux[0].tx_out, b"C", "transmitted capture survives APB reset");
        b.write32(mem, 14).unwrap(); assert_eq!(b.uart_port_input(1, b"D"), Err(Fault::Prohibited));
        b.write32(base + 0x98, 1).unwrap(); assert_eq!(b.read32(base + 0x98), Ok(1));
        b.write32(mem, 10).unwrap(); assert_eq!(b.read32(base + 0x98), Ok(0));
        b.write32(clock, 0xc3).unwrap(); // PLL source absent: sync really waits
        b.write32(base + 0x98, 1).unwrap(); assert_eq!(b.read32(base + 0x98), Ok(1));
        b.mspi.set_bbpll_input(Some(480_000_000)).unwrap(); assert_eq!(b.read32(base + 0x98), Ok(0));
        b.write32(clock, 0xa3).unwrap(); assert_eq!(b.clocks.aux_uart_clock(0), Ok((true, true, 17_500_000)));
        b.clocks.set_fosc_power(false); assert_eq!(b.uart_port_input(1, b"E"), Err(Fault::Prohibited));
        assert_eq!(b.periph.uart_aux[1].rx_pending(), 0); assert_eq!(b.clocks.aux_uart_mem_read(1), 6);
        assert!(b.refused.is_empty() && b.unmodelled.is_empty());
    }
    #[test]
    fn lcd_circular_bounces_yield_at_eof_and_require_refill_before_reuse() {
        use crate::lcd::{BASE, BUS_CLOCK, PIXEL_CLOCK, SHARED_CLOCK, USER};
        let mut b = S31Bus::new();
        let (d0, d1, buf0, buf1) = (SRAM_BASE + 0x1000, SRAM_BASE + 0x1010, SRAM_BASE + 0x2000, SRAM_BASE + 0x2010);
        let control = (1 << 31) | (1 << 30) | (4 << 12) | 4;
        for (desc, buf, next, value) in [(d0, buf0, d1, 0x04030201), (d1, buf1, d0, 0x08070605)] {
            b.write32(desc, control).unwrap(); b.write32(desc + 4, buf).unwrap(); b.write32(desc + 8, next).unwrap(); b.write32(buf, value).unwrap();
        }
        let out = AXI_DMA + 0x138;
        for (address, value) in [(out + 8, 2), (out + 0x10, 4), (out + 0x14, 1 << 12), (out + 0x44, 0), (out + 0x24, d0), (out + 0x20, 2),
            (BUS_CLOCK, 5), (PIXEL_CLOCK, 4), (SHARED_CLOCK, 4), (BASE + 0x18, 8), (BASE + 0x20, 11 | (11 << 16)), (BASE + 0x1c, 0), (BASE + 0x64, 3), (USER, (1 << 27) | (1 << 24))] { b.write32(address, value).unwrap(); }
        assert_eq!(b.lcd.begin(None), Some((12, 1, 1)));
        assert!(b.lcd_dma_step(96)); assert_eq!(b.lcd.data, [1, 2, 3, 4]); assert_eq!(b.read32(d0).unwrap() >> 31, 0);
        assert!(!b.lcd_dma_step(96)); assert_eq!(b.lcd.data.len(), 4); assert!(!b.lcd.irq());
        assert!(b.peripheral_irqs().sources().contains(&(51, true)));
        b.write32(out + 12, 2).unwrap(); assert!(b.lcd_dma_step(0)); assert_eq!(b.lcd.data, [1, 2, 3, 4, 5, 6, 7, 8]);
        // The guest refills buffer0 before returning its owner bit to DMA.
        b.write32(buf0, 0x0c0b0a09).unwrap(); b.write32(d0, control).unwrap(); b.write32(out + 12, 2).unwrap();
        assert!(b.lcd_dma_step(0)); let (_, _, frame) = b.lcd.finish().unwrap(); assert_eq!(frame, (1u8..=12).collect::<Vec<_>>());
        assert!(b.peripheral_irqs().sources().contains(&(13, true))); assert_eq!(b.lcd.frames, 1);
        assert!(b.unmodelled.is_empty() && b.refused.is_empty());
        // A new scan of a descriptor still owned by the CPU faults, never republishes old data.
        b.write32(USER, (1 << 27) | (1 << 24)).unwrap(); assert!(b.lcd.begin(None).is_some()); b.write32(out + 12, 2).unwrap();
        assert!(b.lcd_dma_step(96)); assert_eq!(b.lcd.faults, 1); assert!(b.lcd.data.is_empty()); assert!(b.lcd.finish().is_none());
    }
    /// I2S0 TX through the AHB-GDMA (OUT ch1 on trigger 1) from a descriptor in SRAM: the frames
    /// due go to the audio sink, the channel ends with OUT_EOF/OUT_TOTAL_EOF on source 42, and the
    /// rate comes from the HP_SYS_CLKRST divider words.
    #[test]
    fn i2s0_transmits_through_the_ahb_gdma() {
        let mut b = S31Bus::new();
        let (desc, buf) = (SRAM_BASE + 0x1000, SRAM_BASE + 0x2000);
        for (k, s) in [0x1111u16, 0x2222, 0x3333, 0x4444].iter().enumerate() { b.write(buf + 2 * k as u32, u32::from(*s), 2).unwrap(); }
        b.write32(desc, (1 << 31) | (1 << 30) | (8 << 12) | 8).unwrap();   // owner DMA, SUC_EOF, 8 bytes
        b.write32(desc + 4, buf).unwrap(); b.write32(desc + 8, 0).unwrap();
        let out1 = AHB_DMA + 0x200 + 0x80;
        b.write32(AHB_DMA + 0x68, 0xf).unwrap();                            // OUT_INT_ENA_CH1
        b.write32(out1 + 0x38, 1).unwrap();                                 // OUT_PERI_SEL_CH1: I2S0CH0
        b.write32(out1 + 0x14, desc).unwrap(); b.write32(out1 + 0x10, 1 << 1).unwrap();   // OUT_LINK_ADDR, START
        b.write32(I2S_TX_CLOCK[0].0, 1 | (9 << 3)).unwrap();                // XTAL, DIV_N 9
        b.write32(I2S_TX_CLOCK[0].1, 3 | (4 << 9) | (15 << 18) | (1 << 27)).unwrap();
        b.write32(I2S[0] + 0x2c, (15 << 14) | (15 << 19)).unwrap();         // 16-bit, 32-bit frames
        b.write32(I2S[0] + 0x54, (1 << 16) | 3).unwrap();                   // two slots
        b.write32(I2S[0] + 0x24, (7 << 21) | (1 << 2)).unwrap();            // BCK /8, TX_START
        assert_eq!(b.i2s[0].sample_rate, 16000);
        assert!(b.i2s_tx(0, crate::periph::CPU_HZ));
        assert_eq!(b.i2s[0].pcm, [0x1111, 0x3333]);
        assert_eq!(b.read32(AHB_DMA + 0x60).unwrap() & 0b1010, 0b1010, "OUT_EOF, OUT_TOTAL_EOF");
        assert_eq!(b.read32(desc).unwrap() >> 31, 1, "no AUTO_WRBACK: the owner stays");
        let irq = b.peripheral_irqs();
        assert!(irq.ahb_dma & (1 << 1) != 0);
        assert!(irq.sources().contains(&(crate::interrupts::AHB_DMA_IN_SOURCE + 6, true)));
    }
    /// I2S0 RX into the AHB-GDMA (IN ch2 on trigger 1) as the Mosaico's duplex capture runs it:
    /// the RX a full-duplex slave on the TX clock, 16-bit mono at 16 kHz. Host input fills two
    /// 4-byte descriptors in SRAM (IN_DONE each, owner and length written back) and RX_EOF_NUM
    /// = 8 bytes ends the second with IN_SUC_EOF on IN source 36 + 2; with no IN channel the
    /// input waits.
    #[test]
    fn i2s0_receives_through_the_ahb_gdma() {
        let mut b = S31Bus::new();
        let (d0, d1, buf) = (SRAM_BASE + 0x1000, SRAM_BASE + 0x1010, SRAM_BASE + 0x2000);
        for (d, next, off) in [(d0, d1, 0), (d1, 0, 4)] {
            b.write32(d, (1 << 31) | 4).unwrap();                           // owner DMA, size 4
            b.write32(d + 4, buf + off).unwrap(); b.write32(d + 8, next).unwrap();
        }
        b.write32(I2S_TX_CLOCK[0].0, 1 | (9 << 3)).unwrap();
        b.write32(I2S_TX_CLOCK[0].1, 3 | (4 << 9) | (15 << 18) | (1 << 27)).unwrap();
        b.write32(I2S[0] + 0x2c, (15 << 14) | (15 << 19)).unwrap();
        b.write32(I2S[0] + 0x24, (1 << 30) | (7 << 21)).unwrap();           // TX: SIG_LOOPBACK, BCK /8 (not started)
        b.write32(I2S[0] + 0x28, (15 << 14) | (15 << 19)).unwrap();         // RX_CONF1: 16-bit, 32-bit frames
        b.write32(I2S[0] + 0x50, (1 << 16) | 1).unwrap();                   // RX_TDM_CTRL: left slot
        b.write32(I2S[0] + 0x64, 8).unwrap();                               // RX_EOF_NUM: 8 bytes
        b.write32(I2S[0] + 0x20, (7 << 21) | (1 << 6) | (1 << 3) | (1 << 2)).unwrap();   // slave, MONO, RX_START
        b.i2s[0].rx_input().push(&[[0x1111, 0], [0x2222, 0], [0x3333, 0], [0x4444, 0], [0x5555, 0]]);
        let frame = crate::periph::CPU_HZ / 16000;
        assert!(!b.i2s_rx(0, 4 * frame), "no IN channel: nothing taken");
        let in2 = AHB_DMA + 0x100 + 0x100 * 2;
        b.write32(AHB_DMA + 0x28, 0xf).unwrap();                            // IN_INT_ENA_CH2
        b.write32(in2 + 0x38, 1).unwrap();                                  // IN_PERI_SEL_CH2: I2S0CH0
        b.write32(in2 + 0x14, d0).unwrap(); b.write32(in2 + 0x10, 1 << 2).unwrap();   // IN_LINK_ADDR, START
        assert!(b.i2s_rx(0, 4 * frame));
        assert_eq!([b.read32(buf).unwrap(), b.read32(buf + 4).unwrap()], [0x2222_1111, 0x4444_3333]);
        assert_eq!(b.read32(d0).unwrap() >> 12 & 0xfff, 4, "length written back");
        assert_eq!(b.read32(d1).unwrap() >> 30, 1, "SUC_EOF, owner back to the CPU");
        assert_eq!(b.read32(AHB_DMA + 0x20).unwrap() & 3, 3, "IN_DONE, IN_SUC_EOF");
        assert_eq!(b.i2s[0].bytes_in, 8);
        let irq = b.peripheral_irqs();
        assert!(irq.sources().contains(&(crate::interrupts::AHB_DMA_IN_SOURCE + 2, true)));
    }
    #[test]
    fn aes_block_mode_on_the_crypto_clock() {
        // FIPS-197 appendix C.1: AES-128 key 000102..0f, plaintext 00112233..ff.
        use emu_core::Bus;
        let mut b = super::S31Bus::new();
        b.write32(crate::sha::CLOCK, 0x45).unwrap(); assert_eq!(b.crypto_clock_hz(), Some(40_000_000));
        let words = |bytes: [u8; 16]| -> [u32; 4] { core::array::from_fn(|i| u32::from_le_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap())) };
        let key = words(core::array::from_fn(|i| i as u8));
        let plain = words(core::array::from_fn(|i| (i as u8) * 0x11));
        let cipher = words([0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4, 0xc5, 0x5a]);
        for (i, w) in key.iter().enumerate() { b.write32(super::AES_BASE + 4 * i as u32, *w).unwrap(); }
        for (mode, input, expect) in [(0, plain, cipher), (4, cipher, plain)] {
            b.write32(super::AES_BASE + 0x40, mode).unwrap();
            for (i, w) in input.iter().enumerate() { b.write32(super::AES_BASE + 0x20 + 4 * i as u32, *w).unwrap(); }
            b.write32(super::AES_BASE + 0x48, 1).unwrap();
            assert_eq!(b.read32(super::AES_BASE + 0x4c), Ok(0));
            for (i, w) in expect.iter().enumerate() { assert_eq!(b.read32(super::AES_BASE + 0x30 + 4 * i as u32), Ok(*w)); }
        }
        assert_eq!(b.periph.aes.blocks, 2);
    }

    #[test]
    fn aes_dma_mode_through_the_axi_gdma() {
        // NIST SP 800-38A F.2.1/F.2.2 AES-128-CBC, four blocks, in two OUT and two IN descriptors in SRAM;
        // ten rounds of encrypt (320 bytes) with the chained IV keep the last result and the IV update honest.
        use emu_core::Bus;
        use super::{AES_BASE, AXI_DMA, SRAM_BASE};
        let hex = |s: &str| -> Vec<u8> { (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect() };
        let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
        let iv = hex("000102030405060708090a0b0c0d0e0f");
        let plain = hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e5130c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710");
        let cipher = hex("7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b273bed6b8e3c1743b7116e69e222295163ff1caa1681fac09120eca307586e1a7");
        let mut b = super::S31Bus::new();
        b.write32(crate::sha::CLOCK, 0x45).unwrap();
        let word = |v: &[u8], i: usize| u32::from_le_bytes(v[4 * i..4 * i + 4].try_into().unwrap());
        let (src, dst, desc) = (SRAM_BASE + 0x1000, SRAM_BASE + 0x2000, SRAM_BASE + 0x3000);
        let run = |b: &mut super::S31Bus, decrypt: bool, input: &[u8], iv: &[u8]| -> Vec<u8> {
            let n = input.len();
            for (i, x) in input.iter().enumerate() { b.write8(src + i as u32, *x).unwrap(); }
            for i in 0..n as u32 { b.write8(dst + i, 0).unwrap(); }
            let half = (n / 2) as u32;
            // OUT: owner + length + size, SUC_EOF on the last; IN: owner + size.
            for (k, (buf, eof)) in [(src, false), (src + half, true)].into_iter().enumerate() {
                let d = desc + 12 * k as u32;
                b.write32(d, 1 << 31 | (eof as u32) << 30 | half << 12 | half).unwrap();
                b.write32(d + 4, buf).unwrap(); b.write32(d + 8, if eof { 0 } else { d + 12 }).unwrap();
            }
            for (k, buf) in [dst, dst + half].into_iter().enumerate() {
                let d = desc + 0x40 + 12 * k as u32;
                b.write32(d, 1 << 31 | half).unwrap();
                b.write32(d + 4, buf).unwrap(); b.write32(d + 8, if k == 0 { d + 12 } else { 0 }).unwrap();
            }
            for (i, w) in (0..4).map(|i| word(&key, i)).enumerate() { b.write32(AES_BASE + 4 * i as u32, w).unwrap(); }
            for (i, w) in (0..4).map(|i| word(iv, i)).enumerate() { b.write32(AES_BASE + 0x50 + 4 * i as u32, w).unwrap(); }
            b.write32(AES_BASE + 0x40, if decrypt { 4 } else { 0 }).unwrap();
            b.write32(AES_BASE + 0x90, 1).unwrap(); b.write32(AES_BASE + 0x94, 1).unwrap();       // DMA, CBC
            b.write32(AES_BASE + 0x98, (n / 16) as u32).unwrap(); b.write32(AES_BASE + 0xb0, 1).unwrap();
            // As esp_crypto_shared_gdma_start_axi_ahb: OUT ch0 and IN ch0 on PERI_SEL 4, links started, then TRIGGER.
            let (inb, outb) = (AXI_DMA, AXI_DMA + 0x138);
            b.write32(outb + 0x44, 4).unwrap(); b.write32(inb + 0x44, 4).unwrap();
            b.write32(outb + 0x24, desc).unwrap(); b.write32(outb + 0x20, 1 << 1).unwrap();
            b.write32(inb + 0x24, desc + 0x40).unwrap(); b.write32(inb + 0x20, 1 << 2).unwrap();
            b.write32(AES_BASE + 0x48, 1).unwrap();
            assert_eq!(b.read32(AES_BASE + 0x4c), Ok(2), "STATE done");
            assert!(b.peripheral_irqs().sources().contains(&(crate::interrupts::AES_SOURCE, true)));
            assert_ne!(b.read32(inb).unwrap() & 2, 0, "IN_SUC_EOF");
            b.write32(AES_BASE + 0xac, 1).unwrap(); b.write32(AES_BASE + 0xb8, 1).unwrap();     // INT_CLEAR, DMA_EXIT
            assert_eq!(b.read32(AES_BASE + 0x4c), Ok(0), "idle after DMA_EXIT");
            assert!(!b.peripheral_irqs().sources().contains(&(crate::interrupts::AES_SOURCE, true)));
            (0..n as u32).map(|i| b.read8(dst + i).unwrap()).collect()
        };
        assert_eq!(run(&mut b, false, &plain, &iv), cipher);
        assert_eq!(run(&mut b, true, &cipher, &iv), plain);
        // > 128 bytes (the IDF DMA threshold): 20 blocks; blocks 0-3 are F.2.1's, the rest are checked by decrypting back.
        let long: Vec<u8> = plain.iter().cycle().take(320).copied().collect();
        let enc = run(&mut b, false, &long, &iv);
        assert_eq!(enc[..64], cipher[..]);
        assert_eq!(run(&mut b, true, &enc, &iv), long);
        assert_eq!(b.periph.aes.blocks, 48);
    }

    #[test]
    fn crypto_roots_preserve_real_sha_data_and_obey_source_gates() {
        use emu_core::Bus;
        let mut b = super::S31Bus::new(); let ctrl = crate::sha::CLOCK; let sha = crate::sha::BASE;
        assert_eq!(b.read32(0x2058717c), Ok(0x101));
        b.write32(ctrl, 0x200045).unwrap(); assert_eq!(b.crypto_clock_hz(), None);
        // A write to the gated SHA is dropped (silicon: no clock, no effect) and counted, not a bus fault.
        assert_eq!(b.write32(sha + 0x10, 1), Ok(())); assert_eq!(b.sha.blocks(), 0); assert_eq!(b.refused.get(&(sha + 0x10)), Some(&(1, 1)));
        b.mspi.set_bbpll_input(Some(480_000_000)).unwrap(); assert_eq!(b.crypto_clock_hz(), Some(240_000_000));
        for (source, hz) in [(0, 40_000_000), (1, 17_500_000), (2, 240_000_000)] {
            b.write32(ctrl, source << 20 | 0x45 | 0x80).unwrap(); b.write32(ctrl, source << 20 | 0x45).unwrap();
            assert_eq!(b.crypto_clock_hz(), Some(hz));
            b.write32(sha + 0x80, 0x80636261).unwrap(); b.write32(sha + 0xbc, 0x18000000).unwrap();
            b.write32(sha + 0x10, 1).unwrap(); assert_eq!(b.read32(sha + 0x40), Ok(0xbf1678ba)); assert_eq!(b.sha.blocks(), 1);
        }
        b.write32(0x2058717c, 7).unwrap(); assert_eq!(b.crypto_clock_hz(), None);
        assert_eq!(b.read32(sha + 0x40), Ok(0)); // gated: reads 0, counted
        b.write32(0x2058717c, 0x107).unwrap(); assert_eq!(b.crypto_clock_hz(), Some(60_000_000));
        assert_eq!(b.read32(sha + 0x40), Ok(0xbf1678ba)); // Gating never invents a reset or a digest.
        b.mspi.set_bbpll_gate(false); assert_eq!(b.crypto_clock_hz(), None);
        assert_eq!(b.write32(sha + 0x10, 1), Ok(())); assert_eq!(b.sha.blocks(), 1); assert_eq!(b.refused.get(&(sha + 0x10)).map(|r| r.0), Some(2));
        b.write32(ctrl, 0x100045).unwrap(); b.clocks.set_fosc_power(false);
        assert_eq!(b.crypto_clock_hz(), None); assert_eq!(b.read32(sha), Ok(0));
        let before = b.read32(ctrl).unwrap(); assert_eq!(b.write32(ctrl, 0x300045), Ok(())); // invalid source: dropped
        assert_eq!(b.read32(ctrl), Ok(before));
        b.write32(0x2058705c, 7).unwrap(); assert_eq!(b.read32(0x2058705c), Ok(7));
        b.write32(0x2058705c, 5).unwrap(); assert_eq!(b.read32(0x2058705c), Ok(5));
        assert_eq!(b.write32(0x20506024, 1), Ok(())); assert_eq!(b.refused.get(&0x20506024), Some(&(1, 1))); // No key-manager engine: the start is dropped.
    }
    #[test]
    fn unmodelled_peripheral_registers_are_storage_and_listed() {
        let mut b = S31Bus::new(); let reg = 0x20ff_0010;
        assert_eq!(b.read32(reg), Ok(0)); b.write32(reg, 0x1234_5678).unwrap(); assert_eq!(b.read32(reg), Ok(0x1234_5678));
        b.write8(reg + 2, 0xab).unwrap(); assert_eq!(b.read32(reg), Ok(0x12ab_5678)); assert_eq!(b.read8(reg + 1), Ok(0x56));
        b.write16(reg + 2, 0xbeef).unwrap(); assert_eq!(b.read16(reg + 2), Ok(0xbeef)); assert_eq!(b.read32(reg + 4), Ok(0));
        assert_eq!(b.unmodelled.get(&reg), Some(&0xbeef_5678)); assert!(b.unmodelled.contains_key(&(reg + 4)));
        assert!(b.first_fault.is_none()); assert!(b.refused.is_empty());
        // Outside peripheral space an unclaimed address still faults.
        assert_eq!(b.read32(0x1000_0000), Err(Fault::Unmapped));
    }
    #[test]
    fn refused_peripheral_writes_are_dropped_and_counted_but_permission_faults_stay() {
        use crate::clocks::UART0_CTRL;
        let mut b = S31Bus::new(); b.write32(UART0_CTRL, 0x80).unwrap(); // UART0 clock off
        b.write32(UART0_BASE, 1).unwrap(); b.write32(UART0_BASE, 2).unwrap(); assert!(b.periph.uart0.tx_out.is_empty());
        assert_eq!(b.refused.get(&UART0_BASE), Some(&(2, 2))); assert!(b.first_fault.is_none());
        b.permissions.write(crate::pms::HP0 + 0xff0, 1).unwrap(); b.permissions.write(crate::pms::HP0 + 0x6c, 1).unwrap();
        assert_eq!(b.write32(0x20506018, 31), Err(Fault::Prohibited)); assert!(!b.refused.contains_key(&0x20506018));
    }
    #[test]
    fn usb_otg_core_identifies_itself() {
        let mut b = S31Bus::new(); assert_eq!(b.read32(USB_OTG + 0x40), Ok(0x4f54_430a));
        assert_eq!(b.read8(USB_OTG + 0x40), Err(Fault::Misaligned));
    }
    #[test]
    fn key_manager_permission_denial_cannot_change_the_locked_policy() {
        use emu_core::Bus;
        let mut b = super::S31Bus::new();
        b.write32(0x20506018, 0x1e2).unwrap(); b.write32(0x2050601c, 2).unwrap();
        b.permissions.write(crate::pms::HP0 + 0xff0, 1).unwrap();
        b.permissions.write(crate::pms::HP0 + 0x6c, 1).unwrap(); // Read permitted, write denied.
        assert_eq!(b.write32(0x20506018, 31), Err(emu_core::Fault::Prohibited));
        assert_eq!(b.read32(0x20506018), Ok(0x1e2));
        b.permissions.write(crate::pms::HP0 + 0x6c, 0x10).unwrap();
        assert_eq!(b.read32(0x20506028), Err(emu_core::Fault::Prohibited));
        assert_eq!(b.key_manager.read(0x18), Ok(0x1e2)); assert_eq!(b.key_manager.read(0x1c), Ok(2));
    }
    use super::*;
    #[test]
    fn peripheral_denial_does_not_mutate_real_fifo_scratch_or_sha_control() {
        use crate::pms::{LP, HP0, HP1};
        let mut b = S31Bus::new(); b.write32(UART0_BASE, 65).unwrap();
        b.write32(HP1 + 0xff0, 1).unwrap(); b.write32(HP1 + 0x28, 1).unwrap();
        assert_eq!(b.write32(UART0_BASE, 66), Err(Fault::Prohibited)); assert_eq!(b.periph.uart0.tx_out, b"A");
        b.write32(LP + 0x7f0, 1).unwrap(); b.write32(LP_STORE0, 0x1234).unwrap(); b.write32(LP, 1).unwrap();
        assert_eq!(b.write32(LP_STORE0, 0), Err(Fault::Prohibited)); assert_eq!(b.read32(LP_STORE0), Ok(0x1234));
        b.write32(LP, 0x10).unwrap(); assert_eq!(b.read32(LP_STORE0), Err(Fault::Prohibited));
        b.write32(LP, 0x11).unwrap(); assert_eq!(b.read32(LP_STORE0), Ok(0x1234));
        b.write32(HP0 + 0xff0, 1).unwrap(); b.write32(HP0 + 0x70, 1).unwrap();
        assert_eq!(b.write32(crate::sha::BASE, 2), Err(Fault::Prohibited)); assert_eq!(b.sha.blocks(), 0);
        // Self-protection applies to PMS words too: no privileged bypass via the bus.
        b.write32(HP1 + 0x98, 1).unwrap(); assert_eq!(b.write32(HP1 + 0x28, 0x11), Err(Fault::Prohibited));
    }
    #[test]
    fn peripheral_permissions_gate_apm_configuration_without_changing_filters() {
        let mut b = S31Bus::new();
        b.write32(0x207067f0, 1).unwrap(); b.write32(0x20706020, 1).unwrap();
        assert_eq!(b.write32(0x20706cbc, 0), Err(Fault::Prohibited));
        assert_eq!(b.apm.function_controls()[0], 15);
        b.write32(0x20505ff0, 1).unwrap();
        for (pms, apm, index, reset) in [(0x2050505c, 0x205044c4, 1, 127), (0x20505060, 0x205048c4, 2, 63), (0x20505064, 0x20504cc4, 3, 15)] {
            b.write32(pms, 1).unwrap(); assert_eq!(b.write32(apm, 0), Err(Fault::Prohibited));
            assert_eq!(b.apm.function_controls()[index], reset);
            b.write32(pms, 0x11).unwrap(); b.write32(apm, 0).unwrap(); assert_eq!(b.apm.function_controls()[index], 0);
        }
    }
    #[test]
    fn psram_phy_power_and_clock_gate_real_access_without_losing_backing_storage() {
        use crate::cache::{PSRAM_MMU, PSRAM_VADDR};
        let mut b = S31Bus::new(); b.cache.set_psram_size(65536).unwrap();
        b.write32(PSRAM_MMU + 0x37c, 0xc00).unwrap();
        assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Prohibited));
        assert_eq!(b.write32(PSRAM_VADDR, 0), Err(Fault::Prohibited)); assert_eq!(b.fetch(PSRAM_VADDR), Err(Fault::Prohibited));
        b.write32(0x207041e8, 1 << 31).unwrap(); b.write32(PSRAM_VADDR, 0x00500013).unwrap();
        assert_eq!(b.fetch(PSRAM_VADDR), Ok([0x13, 0, 0x50, 0]));
        for clock in [0x180, 0x81, 0x183, 0x185] {
            b.write32(crate::clocks::BASE + 0x68, clock).unwrap();
            assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Prohibited));
            assert_eq!(b.write32(PSRAM_VADDR, 0), Err(Fault::Prohibited));
            assert_eq!(b.fetch(PSRAM_VADDR), Err(Fault::Prohibited));
        }
        b.write32(crate::clocks::BASE + 0x68, 0x181).unwrap(); assert_eq!(b.read32(PSRAM_VADDR), Ok(0x00500013));
        b.write32(crate::clocks::BASE + 0x68, 0x1a1).unwrap(); // No powered/calibrated MPLL input.
        assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Prohibited)); assert_eq!(b.fetch(PSRAM_VADDR), Err(Fault::Prohibited));
        assert_eq!(b.write32(PSRAM_VADDR, 0), Err(Fault::Prohibited));
        b.write32(crate::clocks::BASE + 0x68, 0x181).unwrap();
        b.write32(0x207041e8, 0).unwrap();
        assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Prohibited)); assert_eq!(b.fetch(PSRAM_VADDR), Err(Fault::Prohibited));
        assert_eq!(b.write32(PSRAM_VADDR, 0), Err(Fault::Prohibited));
        b.write32(0x207041e8, 1 << 31).unwrap(); assert_eq!(b.read32(PSRAM_VADDR), Ok(0x00500013));
    }
    #[test]
    fn dma2d_m2m_reads_psram_through_the_mmu_and_raises_its_sources() {
        use crate::cache::{PSRAM_MMU, PSRAM_VADDR};
        let mut b = S31Bus::new(); b.cache.set_psram_size(2 * 65536).unwrap();
        b.cache.write_register(PSRAM_MMU + 0x380, 0).unwrap(); b.cache.write_register(PSRAM_MMU + 0x37c, 0xc01).unwrap(); // vpage 0 -> ppage 1
        b.cache.psram_bytes_mut()[65536 + 16..65536 + 24].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let (tx, rx, dst) = (SRAM_BASE + 0x100, SRAM_BASE + 0x200, SRAM_BASE + 0x300);
        for (i, w) in [8 << 14 | 3 << 30, 0, 0, PSRAM_VADDR + 16, 0].into_iter().enumerate() { b.write32(tx + 4 * i as u32, w).unwrap(); }
        for (i, w) in [16 | 1 << 31, 0, 0, dst, 0].into_iter().enumerate() { b.write32(rx + 4 * i as u32, w).unwrap(); }
        b.write32(DMA2D + 0x608, 2).unwrap(); b.write32(DMA2D + 0x600, 1).unwrap(); b.write32(DMA2D + 0x63c, 3).unwrap();
        b.write32(DMA2D + 0x620, rx).unwrap(); b.write32(DMA2D + 0x61c, 1 << 22).unwrap();
        b.write32(DMA2D + 0x138, 4).unwrap(); b.write32(DMA2D + 0x120, tx).unwrap(); b.write32(DMA2D + 0x11c, 1 << 21).unwrap();
        assert_eq!((b.read32(dst), b.read32(dst + 4), b.read32(dst + 8)), (Ok(0x04030201), Ok(0x08070605), Ok(0)));
        assert!(b.peripheral_irqs().dma2d == 1 << 1 && b.refused.is_empty());
        assert_eq!(b.read32(DMA2D + 0x124).map(|v| v >> 24 & 1), Ok(1), "RESET_AVAIL while idle");
    }
    #[test]
    fn actual_invalid_psram_translation_routes_error_but_gating_and_missing_input_do_not() {
        use crate::cache::{PSRAM_MMU, PSRAM_VADDR};
        let mut b = S31Bus::new(); b.cache.set_psram_size(65536).unwrap(); b.write32(0x207041e8, 1 << 31).unwrap();
        assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Unmapped)); assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0x80)); assert!(!b.psram_controller.irq());
        b.write32(PSRAM_MMU + 0xc0, 0x280).unwrap(); assert!(b.psram_controller.irq());
        b.interrupts.matrix_write(0, 112 * 4, 0x211); b.interrupts.clic[0].write(0x1044, 1 << 29 | 1 << 8, 15); assert!(b.refresh_irq(0).is_some());
        b.write32(PSRAM_MMU + 0xc4, 0x80).unwrap(); assert_eq!(b.refresh_irq(0), None);
        assert_eq!(b.write32(PSRAM_VADDR, 1), Err(Fault::Unmapped)); assert_eq!(b.read32(PSRAM_MMU + 0xcc), Ok(0x200));
        b.write32(PSRAM_MMU + 0xc4, 0x200).unwrap();
        b.write32(0x207041e8, 0).unwrap(); assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Prohibited)); assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0)); b.write32(0x207041e8, 1 << 31).unwrap();
        for entry in [0x1c00, 0x800] { b.write32(PSRAM_MMU + 0x37c, entry).unwrap(); assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Prohibited)); assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0)); }
        b.write32(PSRAM_MMU + 0x37c, 0xc01).unwrap(); // valid mapping beyond explicitly supplied array is not an invalid MMU entry
        assert_eq!(b.read32(PSRAM_VADDR), Err(Fault::Unmapped)); assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0));
        b.write32(PSRAM_MMU + 0x37c, 0xc00).unwrap(); b.write32(PSRAM_VADDR, 0x12345678).unwrap(); assert_eq!(b.read32(PSRAM_VADDR), Ok(0x12345678)); assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0));
    }
    #[test]
    fn cross_page_rejected_write_is_atomic_and_fetch_invalid_mapping_is_read_error() {
        use crate::cache::{PSRAM_MMU, PSRAM_VADDR};
        let mut b = S31Bus::new(); b.cache.set_psram_size(65536).unwrap(); b.write32(0x207041e8, 1 << 31).unwrap(); b.write32(PSRAM_MMU + 0x37c, 0xc00).unwrap();
        b.write16(PSRAM_VADDR + 65534, 0x1234).unwrap(); assert_eq!(b.write32(PSRAM_VADDR + 65534, 0xdeadbeef), Err(Fault::Unmapped)); assert_eq!(b.read16(PSRAM_VADDR + 65534), Ok(0x1234));
        assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0x200)); b.write32(PSRAM_MMU + 0xc4, 0x200).unwrap();
        assert_eq!(b.fetch(PSRAM_VADDR + 65536), Err(Fault::Unmapped)); assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0x80));
        b.write32(crate::clocks::BASE + 0x68, 0x185).unwrap(); assert!(!b.psram_controller.irq()); b.write32(crate::clocks::BASE + 0x68, 0x181).unwrap();
        assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0)); assert_eq!(b.read16(PSRAM_VADDR + 65534), Ok(0x1234));
        b.cache.set_psram_size(0).unwrap(); // first byte lacks supplied backing, before the later invalid page
        assert_eq!(b.read32(PSRAM_VADDR + 65534), Err(Fault::Unmapped)); assert_eq!(b.write32(PSRAM_VADDR + 65534, 1), Err(Fault::Unmapped));
        assert_eq!(b.read32(PSRAM_MMU + 0xc8), Ok(0));
    }
    #[test]
    fn lp_scratch_words_preserve_full_values_without_aliasing_other_registers() {
        let mut b = S31Bus::new();
        for n in 0..16 { assert_eq!(b.read32(LP_STORE0 + n * 4), Ok(0)); b.write32(LP_STORE0 + n * 4, 0x8123_4567 ^ n).unwrap(); }
        for n in 0..16 { assert_eq!(b.read32(LP_STORE0 + n * 4), Ok(0x8123_4567 ^ n)); }
        assert_eq!(b.read8(LP_STORE0), Err(Fault::Misaligned));
        // Neighbours are not scratch words: they are separate (unmodelled) storage, no aliasing.
        b.write32(LP_STORE0 - 4, 0xdead_beef).unwrap(); assert_eq!(b.read32(LP_STORE0 + 64), Ok(0));
        assert_eq!(b.read32(LP_STORE0 - 4), Ok(0xdead_beef)); assert_eq!(b.read32(LP_STORE0), Ok(0x8123_4567));
        assert!(b.unmodelled.contains_key(&(LP_STORE0 - 4)) && b.unmodelled.contains_key(&(LP_STORE0 + 64)));
    }
    #[test]
    fn uart_clocks_and_reset_control_real_fifo_output_and_irq() {
        use crate::clocks::{UART0_CTRL, UART0_MEM};
        let mut b = S31Bus::new(); b.write32(UART0_BASE + 0x24, 1).unwrap(); b.write32(UART0_BASE + 0xc, 1).unwrap();
        b.uart_input(b"A").unwrap(); assert!(b.uart_irq());
        b.write32(UART0_CTRL, 0x80).unwrap(); assert!(!b.uart_irq());
        assert_eq!(b.uart_input(b"B"), Err(Fault::Prohibited)); assert_eq!(b.write32(UART0_BASE, 88), Ok(())); assert!(b.periph.uart0.tx_out.is_empty()); assert_eq!(b.refused.get(&UART0_BASE), Some(&(1, 88)));
        b.write32(UART0_CTRL, 0x83).unwrap(); assert!(b.uart_irq()); assert_eq!(b.read32(UART0_BASE), Ok(65));
        b.write32(UART0_CTRL, 0x87).unwrap(); assert!(!b.uart_irq()); b.write32(UART0_CTRL, 0x83).unwrap(); assert_eq!(b.periph.uart0.rx_pending(), 0);
        b.write32(UART0_BASE, 67).unwrap(); b.write32(UART0_CTRL, 0x8b).unwrap(); b.write32(UART0_CTRL, 0x83).unwrap();
        assert_eq!(b.periph.uart0.tx_out, b"C"); assert_eq!(b.read32(UART0_BASE + 0xc), Ok(0)); assert_eq!(b.uart_baud(), Some(57636));
        b.write32(UART0_CTRL, 0x183).unwrap(); assert_eq!(b.uart_baud(), Some(28818));
        b.write32(UART0_BASE + 0x88, 1 << 25).unwrap(); assert_eq!(b.write32(UART0_BASE, 68), Ok(())); assert_eq!(b.periph.uart0.tx_out, b"C");
        assert_eq!(b.write32(UART0_CTRL, 0xa3), Ok(())); assert_eq!(b.uart_baud(), Some(28818)); // unsupported value dropped
        assert_eq!(b.write32(UART0_MEM, 12), Ok(())); assert!(b.refused.contains_key(&UART0_CTRL) && b.refused.contains_key(&UART0_MEM));
    }
    #[test]
    fn instruction_fetch_requires_the_real_length_across_mmu_boundaries() {
        use crate::cache::{FLASH_MMU, FLASH_VADDR};
        let mut b = S31Bus::new(); b.write32(FLASH_MMU + 0x3c, 0x40000001).unwrap(); let page = 32768;
        let mut flash = vec![0; page]; flash[page - 2..].copy_from_slice(&[1, 0]);
        b.cache.set_flash(&flash).unwrap(); b.write32(FLASH_MMU + 0x384, 3 << 3).unwrap();
        b.write32(FLASH_MMU + 0x37c, 0x1000).unwrap();
        assert_eq!(b.fetch(FLASH_VADDR + page as u32 - 2), Ok([1, 0, 0, 0]));
        flash[page - 2] = 0x13; b.cache.set_flash(&flash).unwrap();
        assert_eq!(b.fetch(FLASH_VADDR + page as u32 - 2), Err(Fault::Unmapped));
        assert!(b.first_fault.as_ref().unwrap().fetch);
        flash.resize(page * 3, 0); flash[page * 2] = 0x50;
        b.cache.set_flash(&flash).unwrap(); b.write32(FLASH_MMU + 0x380, 1).unwrap(); b.write32(FLASH_MMU + 0x37c, 0x1002).unwrap();
        assert_eq!(b.fetch(FLASH_VADDR + page as u32 - 2), Ok([0x13, 0, 0x50, 0]));
        b.write16(SRAM_BASE + SRAM_SIZE as u32 - 2, 0x13).unwrap();
        assert_eq!(b.fetch(SRAM_BASE + SRAM_SIZE as u32 - 2), Err(Fault::Unmapped));
        b.write16(SRAM_BASE + SRAM_SIZE as u32 - 2, 1).unwrap();
        assert_eq!(b.fetch(SRAM_BASE + SRAM_SIZE as u32 - 2), Ok([1, 0, 0, 0]));
    }
    #[test]
    fn s31_windows_have_exact_bounds_and_rom_is_read_only() {
        let mut b = S31Bus::new();
        for (base, size) in [(SRAM_BASE, SRAM_SIZE), (LP_BASE, LP_SIZE)] {
            let end = base + size as u32;
            assert_eq!(b.write8(end - 1, 0xa5), Ok(()));
            assert_eq!(b.write32(end - 2, 0), Err(Fault::Unmapped));
            assert_eq!(b.read8(end - 1), Ok(0xa5)); assert_eq!(b.read8(end), Err(Fault::Unmapped));
        }
        b.load(ROM_BASE, &[1, 2], 2).unwrap();
        assert_eq!(b.read16(ROM_BASE), Ok(0x0201)); assert_eq!(b.write8(ROM_BASE, 0), Err(Fault::Prohibited));
        for address in [0, u32::MAX] { assert_eq!(b.read32(address), Err(Fault::Unmapped)); }
        for address in [0x4000_0000, 0x4ff0_0000, 0x4fc0_0000] { assert_eq!(b.read32(address), Err(Fault::Prohibited)); } // POR MSPI AXI interface is closed.
        assert_eq!(b.read32(0x500c_a000), Err(Fault::Prohibited)); // POR PSRAM PHY is powered off.
        b.write32(0x207041e8, 1 << 31).unwrap();
        assert_eq!(b.read32(0x500c_a000), Err(Fault::Unmapped)); // Power alone does not create a backing chip or map.
        assert!(b.check_load(SRAM_BASE, usize::MAX).is_err());
    }
    /// EX265: `fetch_ahead` returns exactly what `fetch` returns where it succeeds, `None` where
    /// it faults, and records no fault itself.
    #[test]
    fn fetch_ahead_is_fetch_without_its_fault_records() {
        use crate::cache::{FLASH_MMU, FLASH_VADDR, PSRAM_VADDR};
        let mut b = S31Bus::new(); b.write32(FLASH_MMU + 0x3c, 0x40000001).unwrap(); let page = 32768;
        let mut flash = vec![0x13; page]; flash[page - 2..].copy_from_slice(&[0x13, 0]);
        b.cache.set_flash(&flash).unwrap(); b.write32(FLASH_MMU + 0x384, 3 << 3).unwrap(); b.write32(FLASH_MMU + 0x37c, 0x1000).unwrap();
        b.write16(SRAM_BASE + SRAM_SIZE as u32 - 2, 0x13).unwrap();
        b.write32(LP_BASE + 8, 0x0050_0513).unwrap();
        for pc in [SRAM_BASE, SRAM_BASE + 6, SRAM_BASE + SRAM_SIZE as u32 - 2, LP_BASE + 8, LP_BASE + LP_SIZE as u32 - 2, ROM_BASE + 0x40,
                   FLASH_VADDR, FLASH_VADDR + page as u32 - 2, FLASH_VADDR + page as u32, PSRAM_VADDR, 0x1000_0000, 0x2038_a000, u32::MAX - 1] {
            b.first_fault = None;
            let ahead = b.fetch_ahead(pc);
            assert!(b.first_fault.is_none(), "{pc:#x}: fetch_ahead recorded a fault");
            assert_eq!(ahead, b.fetch(pc).ok(), "{pc:#x}");
        }
    }
    /// EX265: what moves the code versions (`Bus::page_versions`) and what leaves them alone.
    #[test]
    fn code_versions_move_with_every_write_that_can_change_fetched_code() {
        use crate::cache::{CACHE_BASE, FLASH_MMU, FLASH_VADDR};
        let mut b = S31Bus::new();
        let ver = |b: &mut S31Bus, pc: u32| { let i = b.code_page(pc); b.page_versions()[i as usize] };
        let pc = SRAM_BASE + 0x1000;
        let moves = |b: &mut S31Bus, pc: u32, f: &mut dyn FnMut(&mut S31Bus)| { let v = ver(b, pc); f(b); ver(b, pc) != v };
        assert!(moves(&mut b, pc, &mut |b| b.write32(pc + 8, 1).unwrap()), "a store on the page");
        assert!(moves(&mut b, pc, &mut |b| b.write8(pc + 0x100, 1).unwrap()), "a store in the first bytes of the next page");
        assert!(!moves(&mut b, pc, &mut |b| b.write32(pc + 0x400, 1).unwrap()), "a store elsewhere");
        assert!(moves(&mut b, pc, &mut |b| { let mut d = UsbDma { sram: &mut b.sram, lp: &mut b.lp, ver: &mut b.code_ver }; esp_periph::sdmmc::SdmmcMemory::write(&mut d, pc + 4, &[1, 2]).unwrap(); }), "a DMA write");
        assert!(moves(&mut b, pc, &mut |b| b.invalidate_code()), "loading");
        assert!(moves(&mut b, pc, &mut |b| b.write32(0x20706020, 1).unwrap()), "an APM write");
        assert!(moves(&mut b, FLASH_VADDR, &mut |b| b.write32(FLASH_MMU + 0x380, 1).unwrap()), "a flash MMU write");
        assert!(!moves(&mut b, FLASH_VADDR, &mut |b| b.write32(CACHE_BASE + 0xa0, 1).unwrap()), "cache maintenance");
        assert!(!moves(&mut b, FLASH_VADDR, &mut |b| b.write32(pc, 1).unwrap()), "an SRAM store");
        assert_eq!(b.code_page(0x1000_0000), u32::MAX, "no version outside code memory");
    }
    #[test]
    fn uart_uses_s31_fifo_and_reset_fields() {
        let mut b = S31Bus::new(); b.periph.uart0.host_input(b"RX");
        assert_eq!(b.read32(UART0_BASE + 0x1c).unwrap() & 0xff, 2);
        assert_eq!(b.read32(UART0_BASE), Ok(b'R' as u32));
        b.write32(UART0_BASE + 0x20, 1 << 17).unwrap();
        assert_eq!(b.read32(UART0_BASE + 0x1c).unwrap() & 0xff, 1);
        b.write32(UART0_BASE + 0x20, 1 << 22).unwrap();
        assert_eq!(b.read32(UART0_BASE + 0x1c).unwrap() & 0xff, 0);
        assert_eq!(b.fetch(UART0_BASE), Err(Fault::Unmapped));
    }
    #[test]
    fn clic_byte_lanes_are_hart_local_and_undocumented_remote_alias_faults() {
        let mut b = S31Bus::new(); let ctrl = CLIC_BASE + 0x1044;
        b.write8(ctrl + 1, 1).unwrap(); b.write8(ctrl + 2, 1).unwrap(); b.write8(ctrl + 3, 0x3f).unwrap();
        assert_eq!(b.read32(ctrl), Ok(0x3fc1_0100));
        assert_eq!(b.read8(ctrl + 1), Ok(1)); assert_eq!(b.read16(ctrl + 2), Ok(0x3fc1));
        assert_eq!(b.read32(ctrl + 0x1_0000), Err(Fault::Unmapped));
        b.cores.write(crate::core_control::CTRL1, 3).unwrap(); b.set_hart(1);
        assert_eq!(b.read32(ctrl), Ok(0x1fc0_0000)); b.write8(ctrl + 3, 0x7f).unwrap();
        assert_eq!(b.read8(ctrl + 3), Ok(0x7f)); b.set_hart(0); assert_eq!(b.read8(ctrl + 3), Ok(0x3f));
        assert_eq!(b.write16(ctrl + 1, 0), Err(Fault::Misaligned));
        assert_eq!(b.read32(CLIC_BASE + 8), Err(Fault::Unmapped)); // S31 threshold lives in CSR 0x347.
        assert_eq!(b.read32(CLIC_BASE + 0x10c0), Err(Fault::Unmapped));
        assert_eq!(b.read32(CLIC_BASE + 0x2_0000), Err(Fault::Unmapped));
    }
    #[test]
    fn matrix_source_status_and_reset_cause_use_documented_addresses() {
        let mut b = S31Bus::new();
        b.write32(UART0_BASE + 0x24, 1).unwrap(); b.write32(UART0_BASE + 0xc, 1).unwrap(); b.periph.uart0.host_input(b"A");
        assert_eq!(b.read32(MATRIX_BASE + 0x2a8), Ok(1 << 9));
        b.write32(MATRIX_BASE + 0x24, 0x211).unwrap();
        assert_eq!(b.read32(MATRIX_BASE + 0x24), Ok(0x211)); assert_eq!(b.read32(MATRIX_BASE + 0x824), Ok(0));
        assert_eq!(b.read8(MATRIX_BASE), Err(Fault::Misaligned)); assert_eq!(b.read32(MATRIX_BASE + 0x2a4), Ok(0)); assert!(b.unmodelled.contains_key(&(MATRIX_BASE + 0x2a4)));
        assert_eq!(b.read32(RESET_CAUSE0), Ok(2)); b.write32(RESET_CAUSE0, 1 << 30).unwrap();
        assert_eq!(b.read32(RESET_CAUSE0), Ok(0)); assert_eq!(b.read32(RESET_CAUSE1), Ok(2));
    }
    #[test]
    fn successful_cross_hart_stores_break_the_conservative_reservation_set() {
        let mut b = S31Bus::new(); let addr = SRAM_BASE + 64;
        b.write32(addr, 0x12345678).unwrap(); b.reserve(addr);
        b.set_hart(1); b.write8(addr + 1, 0x56).unwrap(); b.set_hart(0);
        assert!(!b.reservation_held(addr)); assert_eq!(b.read32(addr), Ok(0x12345678)); // Same-value write still breaks it.
        b.reserve(addr); b.set_hart(1); b.reserve(addr); b.set_hart(0);
        assert!(b.reservation_held(addr)); assert!(!b.reservation_held(addr));
        b.set_hart(1); assert!(b.reservation_held(addr));
        b.reserve(addr); b.set_hart(0); b.write32(addr + 16, 0).unwrap(); b.set_hart(1);
        assert!(!b.reservation_held(addr)); // Legal larger set: aliases cannot spuriously succeed.
        b.reserve(addr); assert_eq!(b.write32(0, 0), Err(Fault::Unmapped)); assert!(b.reservation_held(addr));
    }

    #[test]
    fn a_partial_dma_write_to_psram_still_moves_code_versions() {
        use crate::cache::{PSRAM_MMU, PSRAM_VADDR};
        let mut b = S31Bus::new(); b.set_psram_size(2 * 65536).unwrap();
        b.cache.write_register(PSRAM_MMU + 0x380, 0).unwrap(); b.cache.write_register(PSRAM_MMU + 0x37c, 0xc00).unwrap(); // vpage 0 -> ppage 0; vpage 1 unmapped
        let a = PSRAM_VADDR + 65536 - 4;
        let phys = b.cache.psram_phys(a).unwrap();
        let before = b.code_ver[PSRAM_CODE + (phys >> CODE_PAGE_SHIFT)];
        let r = { let mut m = Dma2dMem { mem: UsbDma { sram: &mut b.sram, lp: &mut b.lp, ver: &mut b.code_ver }, cache: &mut b.cache };
            esp_periph::sdmmc::SdmmcMemory::write(&mut m, a, &[0x13; 8]) };
        assert!(r.is_err());
        assert_eq!(b.cache.psram_bytes_mut()[phys], 0x13, "the first page was written");
        assert_ne!(b.code_ver[PSRAM_CODE + (phys >> CODE_PAGE_SHIFT)], before, "a written page must move its version");
    }
}
