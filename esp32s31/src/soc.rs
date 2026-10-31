//! ESP32-S31 as an `esp_soc::Soc`: dual-hart Machine path wrapping the bring-up bus.
//!
//! `Bringup` remains the fixture runner. This module exposes `Machine` / `SocBus` so the chip
//! follows the same review surface as C3/C6/P4. Flash-MMU app boot is not ready yet; SRAM/ROM
//! loads and interpreter steps work today. Peripheral models stay in `S31Bus` (Systimer, Mwdt,
//! Efuse, UsbSerialJtag, Gpio pads, interrupt matrix / CLIC) rather than a one-commit rewrite.

use crate::bus::{self, S31Bus};
use crate::interrupts;
use emu_core::{Bus, Fault};
use esp_periph::clic::CLIC_EXT_OFFSET;
use esp_periph::Misc;
use esp_soc::{BoardModel, CoreState, GpioState, NoBoard, Soc};
use riscv_rv32::exec::{CLIC_IRQ_LEVEL_SHIFT, CLIC_IRQ_SHV};
use riscv_rv32::Cpu;

/// Mask ROM reset / cold-boot vector (IDF v6.1 `SOC_IROM_MASK_LOW` for S31).
pub const RESET_VECTOR: u32 = bus::ROM_BASE;
/// Stack the ROM leaves the bootloader (IDF v6.1 `SOC_ROM_STACK_START`).
pub const ROM_STACK: u32 = 0x2f07_cfb0;
/// Default XTAL-rooted CPU rate before guest clock configuration (`Clocks::default`).
pub const CPU_HZ: u64 = 40_000_000;

pub struct S31;
pub type Machine = esp_soc::Machine<S31>;

pub fn machine(mac: [u8; 6], flash_size: usize) -> Machine {
    let mut m = Machine::new(mac, SocBus::new(flash_size, mac));
    m.set_debug(&esp_soc::DebugFlags::from_env());
    m
}

impl Soc for S31 {
    type Core = Cpu;
    type Bus = SocBus;
    const NAME: &'static str = "esp32s31";
    /// IDF v6.1 bootloader Kconfig default for this target family.
    const BOOTLOADER_OFFSET: usize = 0x2000;
    const ROM_ELF: &'static str = "esp32s31_rev0_rom.elf";
    const CPU_HZ: u64 = CPU_HZ;
    const CORES: usize = 2;
    const IDLE_CHUNK: u64 = 64;
    const ROM_BOOT_DEFAULT: bool = true;
    /// 12-byte (dst_start, dst_end, rom_src) entries, as reconstructed by `rom::image`.
    const ROM_DATA_TABLE: &'static [&'static str] = &["_data_table_start"];
    const ROM_DATA_TABLE_END: &'static [&'static str] = &["_data_table_end"];
    const ROM_DATA_TABLE_STRIDE: u32 = 12;

    fn new_core(i: usize) -> Cpu {
        let mut c = Cpu::new_esp32s31();
        c.hartid = i as u32;
        c.pc = RESET_VECTOR;
        c
    }

    fn reset_core(c: &mut Cpu, _i: usize) {
        Cpu::reset(c);
        c.pc = RESET_VECTOR;
    }

    fn reset_core_with_bus(c: &mut Cpu, bus: &mut SocBus, i: usize) {
        Self::reset_core(c, i);
        bus.inner.reset_hart(i);
        bus.irq_dirty = true;
    }

    fn boot_core(c: &mut Cpu, entry: u32) {
        Self::reset_core(c, 0);
        c.pc = entry;
        c.x[2] = ROM_STACK;
    }

    /// Each core's CLIC candidate in `clic_std` packing (`riscv_rv32::exec::CLIC_IRQ_*`).
    fn irqs(bus: &SocBus, out: &mut [Option<u32>]) {
        for (core, o) in out.iter_mut().enumerate() {
            if !bus.inner.cores.clic_on(core) {
                *o = None;
                continue;
            }
            let c = &bus.inner.interrupts.clic[core];
            *o = c.candidate().filter(|&(id, _)| id >= CLIC_EXT_OFFSET as usize).map(|(id, level)| {
                (id as u32 - CLIC_EXT_OFFSET)
                    | level << CLIC_IRQ_LEVEL_SHIFT
                    | if c.vectored(id) { CLIC_IRQ_SHV } else { 0 }
            });
        }
    }

    fn interrupt_taken(bus: &mut SocBus, core: usize, line: u32) {
        bus.inner.set_hart(core);
        bus.inner.interrupt_taken(line);
        bus.irq_dirty = true;
    }

    fn select_core(bus: &mut SocBus, core: usize) {
        bus.inner.set_hart(core);
    }

    fn core_state(bus: &SocBus, core: usize) -> CoreState {
        if bus.inner.cores.running(core) {
            CoreState::Running
        } else {
            CoreState::Reset
        }
    }
}

/// Machine-facing bus: flash + Misc + cycle/IRQ bookkeeping around the existing `S31Bus`.
pub struct SocBus {
    pub inner: S31Bus,
    pub flash: Vec<u8>,
    pub misc: Misc,
    pub board: esp_soc::Board,
    pub cycles: u64,
    pub last_fault: Option<(u32, bool)>,
    pub irq_dirty: bool,
    pub gpio_events: Option<Vec<(u64, u8, bool)>>,
    pub debug: esp_soc::DebugFlags,
    pub reset_cause: u32,
    pub sw_reset: bool,
    pub mac: [u8; 6],
}

impl SocBus {
    pub fn new(flash_size: usize, mac: [u8; 6]) -> Self {
        let mut inner = S31Bus::new();
        // USB console peer so Machine can drain EP1 IN packets like a connected host.
        inner.usb_console.attach_host(true);
        SocBus {
            inner,
            flash: vec![0xff; flash_size],
            misc: Misc::new(),
            board: Box::new(NoBoard),
            cycles: 0,
            last_fault: None,
            irq_dirty: true,
            gpio_events: None,
            debug: Default::default(),
            // POWERON (reset_reasons.h); mirrors the POR latch in `Interrupts::default`.
            reset_cause: 1,
            sw_reset: false,
            mac,
        }
    }

    pub fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let target = self
            .flash
            .get_mut(offset..)
            .and_then(|tail| tail.get_mut(..data.len()))
            .ok_or("flash image too large")?;
        target.copy_from_slice(data);
        Ok(())
    }

    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        self.inner.load(addr, data, data.len())
    }

    fn note_access_fault(&mut self, write: bool) {
        if let Some(f) = &self.inner.first_fault {
            self.last_fault = Some((f.address, write || f.write));
        }
    }

    fn refresh_clic_inputs(&mut self) {
        let irq = self.inner.peripheral_irqs();
        // Threshold 0: `clic_std` CPUs apply `mintthresh` themselves (same as P4 rev ≥ 3).
        let _ = self.inner.interrupts.refresh_core(0, irq, 0);
        self.inner.interrupts.clic[1].thresh = 0;
    }

    fn poll_chip_reset(&mut self) {
        if self.inner.reset.request.is_some()
            || self.inner.power.reset_request.is_some()
            || self.inner.rtc_wdt.reset_request.is_some()
            || self.inner.mwdt.iter().any(|w| w.reset.is_some())
        {
            self.sw_reset = true;
        }
        for (core, reset) in self.inner.cores.take_resets().into_iter().enumerate() {
            if reset {
                self.inner.reset_hart(core);
                self.irq_dirty = true;
            }
        }
    }
}

impl esp_soc::SocBus for SocBus {
    fn cycles(&self) -> u64 {
        self.cycles
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    fn irq_dirty(&mut self) -> &mut bool {
        &mut self.irq_dirty
    }

    fn refresh_irq(&mut self) -> bool {
        self.refresh_clic_inputs();
        true
    }

    fn misc(&mut self) -> &mut Misc {
        &mut self.misc
    }

    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        SocBus::load_bytes(self, addr, data)
    }

    fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        SocBus::write_flash(self, offset, data)
    }

    fn boot_app(&mut self, _app_off: usize) -> Result<u32, String> {
        Err(
            "--boot app is not supported on ESP32-S31 yet: flash MMU / 2nd-stage bootloader \
             mapping is incomplete. Load an SRAM/ROM-linked ELF, or boot from the mask ROM once \
             a flash image and IDF v6.1 hello_world (--preview set-target esp32s31) are available."
                .into(),
        )
    }

    fn reboot(&mut self, mac: [u8; 6]) -> u32 {
        let cause = self.reset_cause;
        let efuse = self.inner.efuse.clone();
        let strap = self.inner.pads.strap;
        let usb_attached = true;
        let log_unknown = self.misc.log_unknown;
        let log_all = self.misc.log_all;
        let mut inner = S31Bus::new();
        inner.efuse = efuse;
        inner.pads.strap = strap;
        inner.usb_console.attach_host(usb_attached);
        // Publish the cause the ROM reads (HPCOREn_RESET_CAUSE[6:1]).
        inner.interrupts.reset_cause = [cause << 1; 2];
        self.inner = inner;
        self.mac = mac;
        self.misc.log_unknown = log_unknown;
        self.misc.log_all = log_all;
        self.sw_reset = false;
        self.irq_dirty = true;
        self.last_fault = None;
        cause
    }

    fn sw_reset(&self) -> bool {
        self.sw_reset
    }

    fn request_reset(&mut self, cause: u32) {
        self.sw_reset = true;
        self.reset_cause = cause;
    }

    fn reset_cause(&self) -> u32 {
        self.reset_cause
    }

    fn last_fault(&self) -> Option<(u32, bool)> {
        self.last_fault
    }

    fn console_take(&mut self) -> [Vec<u8>; 4] {
        let mut usb = Vec::new();
        while let Some(p) = self.inner.usb_console.host_receive() {
            usb.extend(p);
        }
        [
            usb,
            std::mem::take(&mut self.inner.uart0.tx_out),
            Vec::new(),
            Vec::new(),
        ]
    }

    fn serial_input(&mut self, data: &[u8]) {
        self.inner.usb_console.attach_host(true);
        for chunk in data.chunks(64) {
            if chunk.is_empty() {
                continue;
            }
            if self.inner.usb_console.host_send(chunk).is_ok() {
                self.irq_dirty = true;
            }
        }
    }

    fn uart_input(&mut self, n: usize, data: &[u8]) {
        if n != 0 {
            return;
        }
        let before = self.inner.uart0.irq();
        let _ = self.inner.uart_input(data);
        self.irq_dirty |= before != self.inner.uart0.irq();
    }

    fn gpio_set_input(&mut self, pin: u8, level: bool) {
        let _ = self.inner.pads.set_input(pin as usize, Some(level));
        if let Some(ev) = &mut self.gpio_events {
            ev.push((self.cycles, pin, level));
        }
        self.irq_dirty = true;
    }

    fn gpio_release_input(&mut self, pin: u8) {
        let _ = self.inner.pads.set_input(pin as usize, None);
        self.irq_dirty = true;
    }

    fn gpio_input(&self) -> u64 {
        // Functional snapshot; unresolved/gated pins read as zero in the pad model.
        self.inner.pads.input_word(0).unwrap_or(0) as u64
            | (self.inner.pads.input_word(1).unwrap_or(0) as u64) << 32
    }

    fn gpio_state(&self, pin: u8) -> Option<GpioState> {
        let _ = pin;
        None
    }

    fn observe_gpio(&mut self, on: bool) {
        self.gpio_events = if on { Some(Vec::new()) } else { None };
    }

    fn take_gpio_events(&mut self) -> Vec<(u64, u8, bool)> {
        self.gpio_events.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn board(&mut self) -> &mut dyn BoardModel {
        &mut *self.board
    }

    fn board_ref(&self) -> &dyn BoardModel {
        &*self.board
    }

    fn audio(&self) -> (&[i16], u32) {
        (&[], 44100)
    }

    fn irq_sources_of(&self, core: usize, line: u32) -> Vec<usize> {
        let id = line + CLIC_EXT_OFFSET;
        (0..interrupts::SOURCE_COUNT)
            .filter(|&s| self.inner.interrupts.map_id(core, s) == id)
            .collect()
    }

    fn set_debug(&mut self, f: &esp_soc::DebugFlags) {
        self.debug = f.clone();
        self.misc.log_all = f.has("mmio");
        self.misc.log_unknown = f.has("mmio");
    }

    fn set_flash_size(&mut self, bytes: usize) {
        self.flash = vec![0xff; bytes];
    }

    fn set_strap(&mut self, v: u32) {
        self.inner.pads.strap = v as u16;
    }

    fn set_reset_cause(&mut self, cause: u32) {
        self.reset_cause = cause;
        self.inner.interrupts.reset_cause = [cause << 1; 2];
    }

    fn report(&self) -> String {
        format!(
            "esp32s31: cycles={} flash={}KiB bringup-bus (Machine path; flash MMU boot pending)",
            self.cycles,
            self.flash.len() / 1024
        )
    }
}

impl Bus for SocBus {
    fn reserve(&mut self, address: u32) {
        self.inner.reserve(address);
    }
    fn reservation_held(&mut self, address: u32) -> bool {
        self.inner.reservation_held(address)
    }
    fn note_pc(&mut self, pc: u32) {
        self.misc.cur_pc = pc;
        self.inner.note_pc(pc);
    }
    fn note_code_page(&mut self, vidx: u32) {
        self.inner.note_code_page(vidx);
    }
    fn read8(&mut self, a: u32) -> Result<u8, Fault> {
        match self.inner.read8(a) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.note_access_fault(false);
                Err(e)
            }
        }
    }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> {
        match self.inner.read16(a) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.note_access_fault(false);
                Err(e)
            }
        }
    }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> {
        match self.inner.read32(a) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.note_access_fault(false);
                Err(e)
            }
        }
    }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> {
        match self.inner.write8(a, v) {
            Ok(()) => {
                self.irq_dirty = true;
                Ok(())
            }
            Err(e) => {
                self.note_access_fault(true);
                Err(e)
            }
        }
    }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> {
        match self.inner.write16(a, v) {
            Ok(()) => {
                self.irq_dirty = true;
                Ok(())
            }
            Err(e) => {
                self.note_access_fault(true);
                Err(e)
            }
        }
    }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> {
        match self.inner.write32(a, v) {
            Ok(()) => {
                self.irq_dirty = true;
                Ok(())
            }
            Err(e) => {
                self.note_access_fault(true);
                Err(e)
            }
        }
    }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        match self.inner.fetch(pc) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.note_access_fault(false);
                Err(e)
            }
        }
    }
    fn tick(&mut self, cycles: u32) -> u32 {
        self.cycles += cycles as u64;
        let hz = self.inner.clocks.rates().cpu_hz.max(1);
        let before = self.inner.systimer.irqs();
        self.inner.systimer.advance_cpu(
            cycles as u64,
            hz,
            self.inner.clocks.rc_fast_hz(),
            self.inner.cores.stalled(),
        );
        if self.inner.systimer.irqs() != before {
            self.irq_dirty = true;
        }
        self.inner.clocks.mpll.advance(
            cycles,
            self.inner.analog.master_enabled() && self.inner.clocks.modem_on(),
        );
        self.inner.clocks.advance_calibration(cycles);
        self.poll_chip_reset();
        1
    }
    fn block_break(&self) -> bool {
        self.irq_dirty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_soc::SocBus as _;

    #[test]
    fn machine_constructs_loads_sram_and_steps() {
        let mut m = machine([0; 6], 1 << 20);
        assert_eq!(S31::NAME, "esp32s31");
        assert_eq!(S31::CORES, 2);
        assert_eq!(S31::CPU_HZ, 40_000_000);
        assert_eq!(S31::ROM_ELF, "esp32s31_rev0_rom.elf");
        assert_eq!(m.cores[0].pc, RESET_VECTOR);
        assert_eq!(m.cores[0].hartid, 0);
        assert_eq!(m.cores[1].hartid, 1);
        // jal x0, 0 — self-loop in HP SRAM.
        m.bus
            .load_bytes(bus::SRAM_BASE, &0x0000_006fu32.to_le_bytes())
            .unwrap();
        m.cores[0].pc = bus::SRAM_BASE;
        let stop = m.run(8);
        assert!(matches!(stop, esp_soc::Stop::MaxInsns), "{stop:?}");
        assert!(m.cores[0].insn_count >= 8, "core0 retired {}", m.cores[0].insn_count);
        assert!(m.bus.cycles() >= 8);
        assert!(m.cores[1].insn_count == 0, "core1 still held in reset");
    }

    #[test]
    fn boot_app_reports_flash_mmu_gap() {
        let mut bus = SocBus::new(1 << 20, [0; 6]);
        let err = bus.boot_app(0x10000).unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn uart_store_reaches_console_through_machine_bus() {
        use emu_core::Bus;
        let mut bus = SocBus::new(1 << 20, [0; 6]);
        // Write 'A' to UART0 FIFO (THR at offset 0) the same way guest stores do.
        bus.write32(bus::UART0_BASE, b'A' as u32).unwrap();
        let cons = esp_soc::SocBus::console_take(&mut bus);
        assert_eq!(cons[1], b"A");
    }
}
