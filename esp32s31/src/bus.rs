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
const UART_SIZE: usize = 0xa0;
// S31 v6.1 lp_system_reg.h: sixteen R/W LP_SCRATCH registers, POR reset0.
// rtc.h names STORE4 bit0 ROM_LOG_CTRL. No reset/sleep domain sequencing is
// implemented yet; these retain software writes for the lifetime of this bus.
const LP_STORE0: u32 = 0x2070_002c;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessFault { pub hart: usize, pub pc: u32, pub address: u32, pub bytes: usize, pub write: bool, pub fetch: bool, pub reason: Fault }
pub struct S31Bus {
    sram: Vec<u8>, lp: Vec<u8>, rom: Vec<u8>, lp_store: [u32; 16],
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
    pub power: Power,
    pub permissions: Permissions,
    pub apm: Apm,
    pub reset: ResetControl,
    pub cores: CoreControl,
    hart: usize, reservations: [Option<u32>; 2],
    pub rtc_calibration: crate::rtc_calibration::Calibration,
    pub rtc_timer: RtcTimer,
    pc: u32, pub first_fault: Option<AccessFault>,
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
        Self { sram: vec![0; SRAM_SIZE], lp: vec![0; LP_SIZE], rom: vec![0; ROM_SIZE], lp_store: [0; 16],
            periph: Peripherals::new(), interrupts: Interrupts::default(), trng: Trng::default(), rtc_wdt: RtcWdt::default(), cache: Cache::default(), pads: Pads::default(), clocks: Clocks::default(), startup_clocks: StartupClocks::default(), bus_monitor: BusMonitor::default(), mspi: Mspi::default(), mspi_pads: MspiPads::default(), psram_controller: PsramController::default(), sha: ShaEngine::default(), key_manager: crate::key_manager::KeyManager::default(), analog: Analog::default(), power: Power::default(), permissions: Permissions::default(), apm: Apm::default(), reset: ResetControl::default(), cores: CoreControl::default(), hart: 0, reservations: [None; 2], rtc_timer: RtcTimer::default(), rtc_calibration: crate::rtc_calibration::Calibration::default(), pc: 0, first_fault: None }
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
    pub fn load(&mut self, address: u32, data: &[u8], memsz: usize) -> Result<(), String> {
        if data.len() > memsz { return Err("S31 ELF data exceeds segment size".into()); }
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
    pub(crate) fn peripheral_irqs(&self) -> crate::interrupts::Irqs { crate::interrupts::Irqs { uart: self.uart_irq(), wdt: self.rtc_wdt.irq(), efuse: self.periph.efuse.irq(), usb: self.periph.usb.irq(), monitor: self.bus_monitor.irq(), cache: self.cache.irq(), mwdt: [self.periph.timg[0].irq_wdt(), self.periph.timg[1].irq_wdt()], systimer: self.periph.systimer.irqs(), psram: self.psram_controller.irq(), brownout: self.power.mode0.irq(), from_cpu: self.cores.from_cpu_irqs() } }
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
    fn read(&mut self, address: u32, bytes: usize) -> Result<u32, Fault> {
        if let Err(e) = self.apm.check(address, bytes, 2) { return self.fault(address, bytes, false, false, e); }
        if let Err(e) = self.permissions.check(address, false) { return self.fault(address, bytes, false, false, e); }
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
        if matches!(address & !3, crate::clocks::UART0_CTRL | crate::clocks::UART0_MEM) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.clocks.uart_read(address));
        }
        if Power::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.power.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
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
        if Cache::is_memory(address) {
            if address >= crate::cache::PSRAM_VADDR && (!self.clocks.psram_on() || !self.power.policy.psram_enabled() || self.mspi_pads.check_transfer(self.psram_controller.mode().is_some()).is_err() || self.psram_controller.memory_ready(false).is_err()) { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            if address < crate::cache::PSRAM_VADDR && !self.mspi.flash_accessible() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            let mut value = [0; 4];
            for (i, byte) in value.iter_mut().take(bytes).enumerate() {
                match address.checked_add(i as u32).ok_or(Fault::Unmapped).and_then(|a| self.cache.read_byte_core(a, false, self.hart)) {
                    Ok(v) => *byte = v, Err(e) => { self.psram_address_error(address, bytes, false, e); return self.fault(address, bytes, false, false, e); },
                }
            }
            return Ok(u32::from_le_bytes(value));
        }
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
        if let Some(o) = offset(address, UART0_BASE, bytes, UART_SIZE) {
            if bytes == 4 && o.is_multiple_of(4) {
                if !self.clocks.uart_bus_on() || o == 0 && !self.clocks.uart_core_on() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
                return Ok(self.periph.uart0.read(o as u32));
            }
            return self.fault(address, bytes, false, false, Fault::Misaligned);
        }
        self.fault(address, bytes, false, false, Fault::Unmapped)
    }
    fn write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if let Err(e) = self.apm.check(address, bytes, 1) { return self.fault(address, bytes, true, false, e); }
        if let Err(e) = self.permissions.check(address, true) { return self.fault(address, bytes, true, false, e); }
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
            if self.clocks.using_cpll() && (address == 0x2070_40f4 && value & 0x111 != 0 || address == 0x2058_9000 && value & (1 << 29) == 0) {
                return self.fault(address, bytes, true, false, Fault::Prohibited); // CPU-clock loss/suspend not modeled.
            }
            return match self.power.write(address, value) {
                Ok(()) => { self.clocks.set_cpll_power(self.power.cpll_enabled()); self.clocks.mpll.set_power(self.power.mpll_enabled()); self.clocks.set_fosc_power(self.power.fosc_enabled()); self.mspi.set_bbpll_gate(self.power.bbpll_output_enabled()); Ok(()) }
                Err(e) => self.fault(address, bytes, true, false, e),
            };
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
            return match self.clocks.write(address - crate::clocks::BASE, value) { Ok(()) => { if address == crate::clocks::BASE + 0x68 && value & 4 != 0 { self.psram_controller.reset(); } Ok(()) }, Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if StartupClocks::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
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
        if Cache::is_memory(address) {
            if address >= crate::cache::PSRAM_VADDR && (!self.clocks.psram_on() || !self.power.policy.psram_enabled() || self.mspi_pads.check_transfer(self.psram_controller.mode().is_some()).is_err() || self.psram_controller.memory_ready(true).is_err()) { return self.fault(address, bytes, true, false, Fault::Prohibited); }
            return match self.cache.write(address, value, bytes) { Ok(()) => Ok(()), Err(e) => { self.psram_address_error(address, bytes, true, e); self.fault(address, bytes, true, false, e) } };
        }
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
    fn note_code_page(&mut self, _vidx: u32) {} // Only the interpreter is used; no decode cache.
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
    fn crypto_roots_preserve_real_sha_data_and_obey_source_gates() {
        use emu_core::Bus;
        let mut b = super::S31Bus::new(); let ctrl = crate::sha::CLOCK; let sha = crate::sha::BASE;
        assert_eq!(b.read32(0x2058717c), Ok(0x101));
        b.write32(ctrl, 0x200045).unwrap(); assert_eq!(b.crypto_clock_hz(), None);
        assert_eq!(b.write32(sha + 0x10, 1), Err(emu_core::Fault::Prohibited)); assert_eq!(b.sha.blocks(), 0);
        b.mspi.set_bbpll_input(Some(480_000_000)).unwrap(); assert_eq!(b.crypto_clock_hz(), Some(240_000_000));
        for (source, hz) in [(0, 40_000_000), (1, 17_500_000), (2, 240_000_000)] {
            b.write32(ctrl, source << 20 | 0x45 | 0x80).unwrap(); b.write32(ctrl, source << 20 | 0x45).unwrap();
            assert_eq!(b.crypto_clock_hz(), Some(hz));
            b.write32(sha + 0x80, 0x80636261).unwrap(); b.write32(sha + 0xbc, 0x18000000).unwrap();
            b.write32(sha + 0x10, 1).unwrap(); assert_eq!(b.read32(sha + 0x40), Ok(0xbf1678ba)); assert_eq!(b.sha.blocks(), 1);
        }
        b.write32(0x2058717c, 7).unwrap(); assert_eq!(b.crypto_clock_hz(), None);
        assert_eq!(b.read32(sha + 0x40), Err(emu_core::Fault::Prohibited));
        b.write32(0x2058717c, 0x107).unwrap(); assert_eq!(b.crypto_clock_hz(), Some(60_000_000));
        assert_eq!(b.read32(sha + 0x40), Ok(0xbf1678ba)); // Gating never invents a reset or a digest.
        b.mspi.set_bbpll_gate(false); assert_eq!(b.crypto_clock_hz(), None);
        assert_eq!(b.write32(sha + 0x10, 1), Err(emu_core::Fault::Prohibited)); assert_eq!(b.sha.blocks(), 1);
        b.write32(ctrl, 0x100045).unwrap(); b.clocks.set_fosc_power(false);
        assert_eq!(b.crypto_clock_hz(), None); assert_eq!(b.read32(sha), Err(emu_core::Fault::Prohibited));
        let before = b.read32(ctrl).unwrap(); assert_eq!(b.write32(ctrl, 0x300045), Err(emu_core::Fault::Prohibited));
        assert_eq!(b.read32(ctrl), Ok(before));
        b.write32(0x2058705c, 7).unwrap(); assert_eq!(b.read32(0x2058705c), Ok(7));
        b.write32(0x2058705c, 5).unwrap(); assert_eq!(b.read32(0x2058705c), Ok(5));
        assert_eq!(b.write32(0x20506024, 1), Err(emu_core::Fault::Prohibited)); // No key-manager engine claimed.
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
        assert_eq!(b.write32(LP_STORE0 - 4, 0), Err(Fault::Unmapped));
        assert_eq!(b.read32(LP_STORE0 + 64), Err(Fault::Unmapped));
    }
    #[test]
    fn uart_clocks_and_reset_control_real_fifo_output_and_irq() {
        use crate::clocks::{UART0_CTRL, UART0_MEM};
        let mut b = S31Bus::new(); b.write32(UART0_BASE + 0x24, 1).unwrap(); b.write32(UART0_BASE + 0xc, 1).unwrap();
        b.uart_input(b"A").unwrap(); assert!(b.uart_irq());
        b.write32(UART0_CTRL, 0x80).unwrap(); assert!(!b.uart_irq());
        assert_eq!(b.uart_input(b"B"), Err(Fault::Prohibited)); assert_eq!(b.write32(UART0_BASE, 88), Err(Fault::Prohibited)); assert!(b.periph.uart0.tx_out.is_empty());
        b.write32(UART0_CTRL, 0x83).unwrap(); assert!(b.uart_irq()); assert_eq!(b.read32(UART0_BASE), Ok(65));
        b.write32(UART0_CTRL, 0x87).unwrap(); assert!(!b.uart_irq()); b.write32(UART0_CTRL, 0x83).unwrap(); assert_eq!(b.periph.uart0.rx_pending(), 0);
        b.write32(UART0_BASE, 67).unwrap(); b.write32(UART0_CTRL, 0x8b).unwrap(); b.write32(UART0_CTRL, 0x83).unwrap();
        assert_eq!(b.periph.uart0.tx_out, b"C"); assert_eq!(b.read32(UART0_BASE + 0xc), Ok(0)); assert_eq!(b.uart_baud(), Some(57636));
        b.write32(UART0_CTRL, 0x183).unwrap(); assert_eq!(b.uart_baud(), Some(28818));
        b.write32(UART0_BASE + 0x88, 1 << 25).unwrap(); assert_eq!(b.write32(UART0_BASE, 68), Err(Fault::Prohibited));
        assert_eq!(b.write32(UART0_CTRL, 0xa3), Err(Fault::Prohibited)); assert_eq!(b.write32(UART0_MEM, 12), Err(Fault::Prohibited));
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
        assert_eq!(b.read8(MATRIX_BASE), Err(Fault::Misaligned)); assert_eq!(b.read32(MATRIX_BASE + 0x2a4), Err(Fault::Unmapped));
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

}
