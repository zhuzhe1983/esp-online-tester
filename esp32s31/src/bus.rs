//! S31 memory/MMU, UART0, reset causes and CLIC/matrix windows; other MMIO faults explicitly.
use emu_core::{Bus, Fault};
use esp_periph::uart::{Uart, UartLayout};
use crate::interrupts::{Interrupts, MATRIX_BASE, CLIC_BASE, RESET_CAUSE0, RESET_CAUSE1};
use crate::trng::{Trng, TRNG_BASE, RNG_CTRL};
use crate::rtc_wdt::{RtcWdt, BASE as RTC_WDT_BASE};
use crate::cache::Cache;
use crate::pads::Pads;
use crate::clocks::Clocks;
use crate::usb_console::UsbConsole;
use crate::bus_monitor::BusMonitor;
use crate::mspi::Mspi;
use crate::sha::ShaEngine;
use crate::efuse::{Efuse, BASE as EFUSE_BASE};

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
fn new_uart() -> Uart {
    let mut uart = Uart::new(UartLayout { thrhd_mask: 0xff, rxfifo_rst: 1 << 22, rxfifo_cnt_mask: 0xff });
    // S31 v6.1 uart_reg.h. Remaining serial framing/flow-control features are inherited
    // from the existing functional FIFO, not a physical line timing model.
    for (off, value) in [(0x14, 694), (0x20, 0x1c), (0x24, 0x6060), (0x88, 3 << 24), (0x8c, 36774432), (0x9c, 1280)] { uart.write(off, value); }
    uart
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessFault { pub pc: u32, pub address: u32, pub bytes: usize, pub write: bool, pub fetch: bool, pub reason: Fault }
pub struct S31Bus {
    sram: Vec<u8>, lp: Vec<u8>, rom: Vec<u8>, lp_store: [u32; 16], pub uart0: Uart,
    pub interrupts: Interrupts,
    pub trng: Trng,
    pub rtc_wdt: RtcWdt,
    pub cache: Cache,
    pub pads: Pads,
    pub efuse: Efuse,
    pub clocks: Clocks,
    pub usb_console: UsbConsole,
    pub bus_monitor: BusMonitor,
    pub mspi: Mspi,
    pub sha: ShaEngine,
    pc: u32, pub first_fault: Option<AccessFault>,
}
fn offset(address: u32, base: u32, bytes: usize, size: usize) -> Option<usize> {
    let off = address.checked_sub(base)? as usize;
    off.checked_add(bytes).filter(|&end| end <= size).map(|_| off)
}
impl Default for S31Bus { fn default() -> Self { Self::new() } }
impl S31Bus {
    pub fn new() -> Self {
        // S31 v6.1 uart_reg.h: RX threshold/count 8 bits, RXFIFO_RST bit 22 at 0x20;
        // FIFO=0, STATUS=0x1c, REG_UPDATE=0x98. Same functional layout as C6, checked
        // against S31 headers. This reuses FIFO behavior, not C6's address map/clock tree.
        Self { sram: vec![0; SRAM_SIZE], lp: vec![0; LP_SIZE], rom: vec![0; ROM_SIZE], lp_store: [0; 16],
            uart0: new_uart(), interrupts: Interrupts::default(), trng: Trng::default(), rtc_wdt: RtcWdt::default(), cache: Cache::default(), pads: Pads::default(), efuse: Efuse::default(), clocks: Clocks::default(), usb_console: UsbConsole::default(), bus_monitor: BusMonitor::default(), mspi: Mspi::default(), sha: ShaEngine::default(), pc: 0, first_fault: None }
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
        if self.first_fault.is_none() { self.first_fault = Some(AccessFault { pc: self.pc, address, bytes, write, fetch, reason }); }
        Err(reason)
    }
    fn uart_irq(&self) -> bool { self.clocks.uart_bus_on() && self.clocks.uart_core_on() && self.uart0.irq() }
    pub fn uart_input(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        if !self.clocks.uart_bus_on() || !self.clocks.uart_core_on() || self.uart0.read(0x88) & (1 << 25 | 1 << 27) != 1 << 25 { return Err(Fault::Prohibited); }
        self.uart0.host_input(bytes); Ok(())
    }
    pub fn uart_baud(&mut self) -> Option<u64> {
        let div = self.uart0.read(0x14); let divisor = u64::from(div & 0xfff) * 16 + u64::from((div >> 20) & 15);
        (divisor != 0 && self.clocks.uart_sclk_hz() != 0).then(|| self.clocks.uart_sclk_hz() * 16 / divisor)
    }
    fn uart_core_reset(&mut self) {
        let conf = self.uart0.read(0x20); self.uart0.write(0x20, conf | 1 << 22); self.uart0.write(0x20, conf);
        self.uart0.int_raw = (1 << 14) | 2;
    }
    fn irqs(&self) -> crate::interrupts::Irqs { crate::interrupts::Irqs { uart: self.uart_irq(), wdt: self.rtc_wdt.irq(), efuse: self.efuse.irq(), usb: self.usb_console.irq(), monitor: self.bus_monitor.irq() } }
    pub fn refresh_irq(&mut self, threshold: u32) -> Option<u32> { self.interrupts.refresh(self.irqs(), threshold) }
    pub fn interrupt_taken(&mut self, line: u32) { if line < 32 { self.interrupts.clic[0].taken(line as usize + 16); } }
    fn clic_offset(address: u32, bytes: usize) -> Option<(usize, u32)> {
        let off = address.checked_sub(CLIC_BASE)?;
        let core = (off >> 16) as usize;
        let local = off & 0xffff;
        if core < 2 && local.checked_add(bytes as u32).is_some_and(|end| end <= 0x2000) { Some((core, local)) } else { None }
    }
    fn clic_register(off: u32) -> bool { matches!(off & !3, 0 | 4 | 0x1000..=0x10bc) }
    fn read(&mut self, address: u32, bytes: usize) -> Result<u32, Fault> {
        if let Some(data) = self.memory(address, bytes) {
            let mut value = [0; 4]; value[..bytes].copy_from_slice(data); return Ok(u32::from_le_bytes(value));
        }
        if address & !3 == crate::mspi::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.mspi.clock_read());
        }
        if address & !3 == crate::sha::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.sha.clock_read());
        }
        if let Some(off) = offset(address, crate::sha::BASE, bytes, 0x300) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
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
        if address & !3 == crate::usb_console::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.usb_console.clock_read());
        }
        if let Some(off) = offset(address, crate::usb_console::BASE, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.usb_console.read(off as u32) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if let Some(off) = offset(address, EFUSE_BASE, bytes, 0x200) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.efuse.read(off as u32) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if matches!(address & !3, crate::clocks::UART0_CTRL | crate::clocks::UART0_MEM) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.clocks.uart_read(address));
        }
        if Clocks::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.clocks.read(address - crate::clocks::BASE) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Pads::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return match self.pads.read(address) { Ok(v) => Ok(v), Err(e) => self.fault(address, bytes, false, false, e) };
        }
        if Cache::is_memory(address) {
            if address < crate::cache::PSRAM_VADDR && !self.mspi.flash_accessible() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
            let mut value = [0; 4];
            for (i, byte) in value.iter_mut().take(bytes).enumerate() {
                match address.checked_add(i as u32).ok_or(Fault::Unmapped).and_then(|a| self.cache.read_byte(a, false)) {
                    Ok(v) => *byte = v, Err(e) => return self.fault(address, bytes, false, false, e),
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
            if let Some(value) = self.interrupts.matrix_read(off >> 11, (off & 0x7ff) as u32, self.irqs()) { return Ok(value); }
            return self.fault(address, bytes, false, false, Fault::Unmapped);
        }
        if let Some((core, off)) = Self::clic_offset(address, bytes) {
            if !address.is_multiple_of(bytes as u32) { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if !Self::clic_register(off) { return self.fault(address, bytes, false, false, Fault::Unmapped); }
            let word = self.interrupts.clic[core].read(off & !3);
            return Ok((word >> ((off & 3) * 8)) & ((1u64 << (bytes * 8)) - 1) as u32);
        }
        if let Some(o) = offset(address, UART0_BASE, bytes, UART_SIZE) {
            if bytes == 4 && o.is_multiple_of(4) {
                if !self.clocks.uart_bus_on() || o == 0 && !self.clocks.uart_core_on() { return self.fault(address, bytes, false, false, Fault::Prohibited); }
                return Ok(self.uart0.read(o as u32));
            }
            return self.fault(address, bytes, false, false, Fault::Misaligned);
        }
        self.fault(address, bytes, false, false, Fault::Unmapped)
    }
    fn write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if let Some(dest) = self.memory_mut(address, bytes, false) { dest.copy_from_slice(&value.to_le_bytes()[..bytes]); return Ok(()); }
        if address & !3 == crate::mspi::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.mspi.clock_write(value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if address & !3 == crate::sha::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.sha.clock_write(value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, crate::sha::BASE, bytes, 0x300) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
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
        if address & !3 == crate::usb_console::CLOCK {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            self.usb_console.clock_write(value); return Ok(());
        }
        if let Some(off) = offset(address, crate::usb_console::BASE, bytes, 0x100) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.usb_console.write(off as u32, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, EFUSE_BASE, bytes, 0x200) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.efuse.write(off as u32, value) {
                Ok(()) => { self.rtc_wdt.delay_sel = self.efuse.wdt_delay_selector(); Ok(()) }
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
                    let output = std::mem::take(&mut self.uart0.tx_out); self.uart0 = new_uart(); self.uart0.tx_out = output;
                } else if value & 4 != 0 { self.uart_core_reset(); }
            }
            return Ok(());
        }
        if Clocks::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.clocks.write(address - crate::clocks::BASE, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Pads::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.pads.write(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Cache::is_memory(address) {
            return match self.cache.write(address, value, bytes) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if Cache::is_mmio(address) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.cache.write_register(address, value) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
        }
        if let Some(off) = offset(address, crate::mspi::BASE, bytes, 0x1400) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            return match self.mspi.write(off / 0x1000, (off & 0xfff) as u32, value, self.cache.flash_bytes_mut()) { Ok(()) => Ok(()), Err(e) => self.fault(address, bytes, true, false, e) };
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
        if let Some((core, off)) = Self::clic_offset(address, bytes) {
            if !address.is_multiple_of(bytes as u32) { return self.fault(address, bytes, true, false, Fault::Misaligned); }
            if !Self::clic_register(off) { return self.fault(address, bytes, true, false, Fault::Unmapped); }
            let lane = off & 3;
            self.interrupts.clic[core].write(off & !3, value << (lane * 8), ((1u32 << bytes) - 1) << lane); return Ok(());
        }
        if let Some(o) = offset(address, UART0_BASE, bytes, UART_SIZE) {
            if bytes == 4 && o.is_multiple_of(4) {
                if !self.clocks.uart_bus_on() || o == 0 && (!self.clocks.uart_core_on() || self.uart0.read(0x88) & (1 << 24 | 1 << 26) != 1 << 24) { return self.fault(address, bytes, true, false, Fault::Prohibited); }
                if o == 0x88 && value & (1 << 27) != 0 { self.uart_core_reset(); }
                self.uart0.write(o as u32, value); return Ok(());
            }
            return self.fault(address, bytes, true, false, Fault::Misaligned);
        }
        let reason = if offset(address, ROM_BASE, bytes, ROM_SIZE).is_some() { Fault::Prohibited } else { Fault::Unmapped };
        self.fault(address, bytes, true, false, reason)
    }
}
impl Bus for S31Bus {
    fn note_pc(&mut self, pc: u32) { self.pc = pc; }
    fn note_code_page(&mut self, _vidx: u32) {} // Only the interpreter is used; no decode cache.
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.read(a, 1).map(|v| v as u8) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.read(a, 2).map(|v| v as u16) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.read(a, 4) }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.write(a, v as u32, 1) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.write(a, v as u32, 2) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.write(a, v, 4) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        let mut data = [0; 4];
        for i in 0..4 {
            if i == 2 && data[0] & 3 != 3 { break; } // A compressed instruction needs only two bytes.
            let Some(address) = pc.checked_add(i as u32) else { return self.fault(pc, 4, false, true, Fault::Unmapped); };
            if let Some(src) = self.memory(address, 1) { data[i] = src[0]; }
            else if Cache::is_memory(address) {
                if address < crate::cache::PSRAM_VADDR && !self.mspi.flash_accessible() { return self.fault(pc, 4, false, true, Fault::Prohibited); }
                match self.cache.read_byte(address, true) {
                    Ok(v) => data[i] = v, Err(e) => return self.fault(pc, 4, false, true, e),
                }
            } else { return self.fault(pc, 4, false, true, Fault::Unmapped); }
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(b.uart_input(b"B"), Err(Fault::Prohibited)); assert_eq!(b.write32(UART0_BASE, 88), Err(Fault::Prohibited)); assert!(b.uart0.tx_out.is_empty());
        b.write32(UART0_CTRL, 0x83).unwrap(); assert!(b.uart_irq()); assert_eq!(b.read32(UART0_BASE), Ok(65));
        b.write32(UART0_CTRL, 0x87).unwrap(); assert!(!b.uart_irq()); b.write32(UART0_CTRL, 0x83).unwrap(); assert_eq!(b.uart0.rx_pending(), 0);
        b.write32(UART0_BASE, 67).unwrap(); b.write32(UART0_CTRL, 0x8b).unwrap(); b.write32(UART0_CTRL, 0x83).unwrap();
        assert_eq!(b.uart0.tx_out, b"C"); assert_eq!(b.read32(UART0_BASE + 0xc), Ok(0)); assert_eq!(b.uart_baud(), Some(57636));
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
        for address in [0, u32::MAX, 0x500c_a000] { assert_eq!(b.read32(address), Err(Fault::Unmapped)); }
        for address in [0x4000_0000, 0x4ff0_0000, 0x4fc0_0000] { assert_eq!(b.read32(address), Err(Fault::Prohibited)); } // POR MSPI AXI interface is closed.
        assert!(b.check_load(SRAM_BASE, usize::MAX).is_err());
    }
    #[test]
    fn uart_uses_s31_fifo_and_reset_fields() {
        let mut b = S31Bus::new(); b.uart0.host_input(b"RX");
        assert_eq!(b.read32(UART0_BASE + 0x1c).unwrap() & 0xff, 2);
        assert_eq!(b.read32(UART0_BASE), Ok(b'R' as u32));
        b.write32(UART0_BASE + 0x20, 1 << 17).unwrap();
        assert_eq!(b.read32(UART0_BASE + 0x1c).unwrap() & 0xff, 1);
        b.write32(UART0_BASE + 0x20, 1 << 22).unwrap();
        assert_eq!(b.read32(UART0_BASE + 0x1c).unwrap() & 0xff, 0);
        assert_eq!(b.fetch(UART0_BASE), Err(Fault::Unmapped));
    }
    #[test]
    fn clic_byte_lanes_and_core_windows_are_independent() {
        let mut b = S31Bus::new(); let ctrl = CLIC_BASE + 0x1044;
        b.write8(ctrl + 1, 1).unwrap(); b.write8(ctrl + 2, 1).unwrap(); b.write8(ctrl + 3, 0x3f).unwrap();
        assert_eq!(b.read32(ctrl), Ok(0x3fc1_0100));
        assert_eq!(b.read8(ctrl + 1), Ok(1)); assert_eq!(b.read16(ctrl + 2), Ok(0x3fc1));
        assert_eq!(b.read32(ctrl + 0x1_0000), Ok(0x1fc0_0000));
        assert_eq!(b.write16(ctrl + 1, 0), Err(Fault::Misaligned));
        assert_eq!(b.read32(CLIC_BASE + 8), Err(Fault::Unmapped)); // S31 threshold lives in CSR 0x347.
        assert_eq!(b.read32(CLIC_BASE + 0x10c0), Err(Fault::Unmapped));
        assert_eq!(b.read32(CLIC_BASE + 0x2_0000), Err(Fault::Unmapped));
    }
    #[test]
    fn matrix_source_status_and_reset_cause_use_documented_addresses() {
        let mut b = S31Bus::new();
        b.write32(UART0_BASE + 0x24, 1).unwrap(); b.write32(UART0_BASE + 0xc, 1).unwrap(); b.uart0.host_input(b"A");
        assert_eq!(b.read32(MATRIX_BASE + 0x2a8), Ok(1 << 9));
        b.write32(MATRIX_BASE + 0x24, 0x211).unwrap();
        assert_eq!(b.read32(MATRIX_BASE + 0x24), Ok(0x211)); assert_eq!(b.read32(MATRIX_BASE + 0x824), Ok(0));
        assert_eq!(b.read8(MATRIX_BASE), Err(Fault::Misaligned)); assert_eq!(b.read32(MATRIX_BASE + 0x2a4), Err(Fault::Unmapped));
        assert_eq!(b.read32(RESET_CAUSE0), Ok(2)); b.write32(RESET_CAUSE0, 1 << 30).unwrap();
        assert_eq!(b.read32(RESET_CAUSE0), Ok(0)); assert_eq!(b.read32(RESET_CAUSE1), Ok(2));
    }
}
