//! ESP32-S31 as an `esp_soc::Soc`: dual-hart Machine path (same review surface as C3/C6/P4).
//!
//! Mask-ROM and Flash-MMU `--boot app` work on this path. Shared DeviceSet IP covers UART /
//! Systimer / USJ / TimerGroup(+local MWDT stages) / Efuse. GPIO pads and the interrupt matrix
//! stay local after map check (not shared `Gpio` / not `intmtx::Lines`); see `pads` /
//! `interrupts` / `periph` module notes for the irreducible remainder.

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
        // Match the ROM stack the IDF hello path / load_mask_rom leave for core 0.
        c.x[2] = ROM_STACK;
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
    /// Reconstructed mask-ROM image; ROM is not reset-volatile, so `reboot` reloads it.
    rom_image: Option<Vec<u8>>,
    /// Board-level inputs replayed after every chip reset (they model the PCB, not the SoC).
    pub board_inputs: Option<BoardInputs>,
}

/// Explicit board-level inputs needed to run an IDF v6.1 image from the mask ROM. None of
/// these are SoC reset values: they describe the attached flash part, the analog/regi2c
/// readback seeds a host must choose, the SPLL/BBPLL source rate, and the pad levels a
/// board presents. `devkit_w25q128` is one such documented choice, not silicon truth.
#[derive(Clone, Debug)]
pub struct BoardInputs {
    /// JEDEC ID (manufacturer, type, capacity) answered by the SPI NOR model.
    pub flash_id: [u8; 3],
    /// SFDP bytes returned by READ_SFDP (0x5a) from offset 0.
    pub sfdp: Vec<u8>,
    /// regi2c readback seeds: BIAS[2], PERIF glitch, DIG_REG[4], BOD. Host choices.
    pub bias: [u8; 2], pub perif_glitch: u8, pub dig_reg: [u8; 4], pub bod: u8,
    /// Rate fed to the MSPI SPLL/BBPLL consumer (Hz).
    pub bbpll_hz: u64,
    /// Level sampled for every GPIO that has no driver (`None` keeps them unresolved).
    pub idle_level: Option<bool>,
    /// Explicit per-pin input levels applied after `idle_level`.
    pub inputs: Vec<(usize, bool)>,
}

impl BoardInputs {
    /// Winbond W25Q128JV (0xef4018) with its SFDP header, zero regi2c seeds, a 480 MHz
    /// SPLL input, idle-low pads and GPIO36 high. pmu_init.c reads GPIO36 as the flash
    /// LDO strap; high selects the internal flash supply path a 3.3 V devkit uses.
    pub fn devkit_w25q128() -> Self {
        let mut sfdp = vec![0x53, 0x46, 0x44, 0x50, 0x05, 0x01, 0x00, 0xff, 0x00, 0x05, 0x01, 0x10, 0x80, 0x00, 0x00, 0xff];
        sfdp.extend([0xff; 0x70]);
        Self { flash_id: [0xef, 0x40, 0x18], sfdp, bias: [0; 2], perif_glitch: 0, dig_reg: [0; 4], bod: 0,
            bbpll_hz: 480_000_000, idle_level: Some(false), inputs: vec![(36, true)] }
    }

    /// Winbond W25Q16 (0xef4015) — 2 MiB, matching the IDF hello_world image header
    /// (`flash_size = 2MB`). Prefer this for BoardInputs + Machine flash sizing so the
    /// image-header / modelled-flash mismatch warning does not fire.
    pub fn hello_w25q16() -> Self {
        let mut sfdp = vec![0x53, 0x46, 0x44, 0x50, 0x05, 0x01, 0x00, 0xff, 0x00, 0x05, 0x01, 0x10, 0x80, 0x00, 0x00, 0xff];
        sfdp.extend([0xff; 0x70]);
        Self { flash_id: [0xef, 0x40, 0x15], sfdp, bias: [0; 2], perif_glitch: 0, dig_reg: [0; 4], bod: 0,
            bbpll_hz: 480_000_000, idle_level: Some(false), inputs: vec![(36, true)] }
    }
    pub fn apply(&self, b: &mut S31Bus) -> Result<(), String> {
        let a = &mut b.analog; a.set_bias(self.bias); a.set_perif_glitch(self.perif_glitch); a.set_dig_reg(self.dig_reg); a.set_bod_register(self.bod);
        b.mspi.attach_flash(self.flash_id, 0).map_err(|e| format!("{e:?}"))?;
        b.mspi.attach_sfdp(0, &self.sfdp).map_err(|e| format!("{e:?}"))?;
        if let Some(level) = self.idle_level { for pin in 0..62 { let _ = b.pads.set_unresolved_sample(pin, Some(level)); } }
        for &(pin, level) in &self.inputs { b.pads.set_input(pin, Some(level)).map_err(|e| format!("{e:?}"))?; }
        b.mspi.set_bbpll_input(Some(self.bbpll_hz)).map_err(|e| format!("{e:?}"))?;
        Ok(())
    }
}

impl SocBus {
    pub fn new(flash_size: usize, mac: [u8; 6]) -> Self {
        let mut inner = S31Bus::new();
        // USB console peer so Machine can drain EP1 IN packets like a connected host.
        inner.periph.usb.attach_host(true);
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
            rom_image: None,
            board_inputs: None,
        }
    }

    pub fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let target = self
            .flash
            .get_mut(offset..)
            .and_then(|tail| tail.get_mut(..data.len()))
            .ok_or("flash image too large")?;
        target.copy_from_slice(data);
        // SPI0 cache reads and SPI1 commands see the same NOR array.
        self.inner.cache.set_flash(&self.flash)
    }

    /// Load the real S31 mask ROM, reconstructing its ROM-side initializer payloads via
    /// `rom::image` (same result as `Machine::load_rom` after eh_frame skip + ROM snapshot).
    pub fn load_mask_rom(&mut self, bytes: &[u8]) -> Result<(), String> {
        let elf = esp_soc::elf::parse(bytes)?;
        if elf.entry != bus::ROM_BASE { return Err("S31 ROM reset entry must be 0x2f800000".into()); }
        let image = crate::rom::image(&elf)?;
        // The 20251218 mask ROM's SPI_init read-modify-writes SPI0 +0x24/+0x28 (legacy
        // MISO/MOSI length words, reserved in the v6.1 headers); keep them as plain storage.
        self.inner.mspi.enable_legacy_rom_lengths();
        self.inner.load(bus::ROM_BASE, &image, image.len())?;
        self.rom_image = Some(image);
        Ok(())
    }

    /// Install board inputs now and after every reset.
    pub fn set_board_inputs(&mut self, inputs: BoardInputs) -> Result<(), String> {
        inputs.apply(&mut self.inner)?;
        self.board_inputs = Some(inputs);
        Ok(())
    }

    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        // Debugger-only .eh_frame at vaddr 0 (Machine::load_rom); nothing reads it (P4 pattern).
        if addr == 0 {
            return Ok(());
        }
        // Keep a ROM snapshot for reboot() when Machine::load_rom writes the mask ROM window.
        if addr >= bus::ROM_BASE {
            let off = (addr - bus::ROM_BASE) as usize;
            let end = off
                .checked_add(data.len())
                .filter(|&e| e <= bus::ROM_SIZE)
                .ok_or_else(|| format!("load: ROM address {:#010x} out of range", addr))?;
            self.inner.mspi.enable_legacy_rom_lengths();
            let image = self.rom_image.get_or_insert_with(|| vec![0u8; bus::ROM_SIZE]);
            image[off..end].copy_from_slice(data);
        }
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
        // Latch the reset *cause* the ROM reads from HPCORE0_RESET_CAUSE before the
        // Machine consumes `sw_reset`. Soft reboot (esp_restart → esp_cpu_reset →
        // LP_AONCLKRST_HPCOREn_SW_RESET) must report RESET_REASON_CPU_SW (0x0c),
        // matching silicon — not POWERON.
        if self.inner.reset.request.is_some() {
            self.reset_cause = esp_periph::RST_SW_CPU;
            self.sw_reset = true;
        } else if self.inner.power.reset_request.is_some() {
            self.reset_cause = 0x0f; // RESET_REASON_SYS_BROWN_OUT
            self.sw_reset = true;
        } else if self.inner.rtc_wdt.reset_request.is_some() {
            self.reset_cause = esp_periph::RST_RTCWDT_SYS;
            self.sw_reset = true;
        } else if self.inner.periph.timg.iter().any(|t| t.wdt.reset.is_some()) {
            self.reset_cause = 0x07; // RESET_REASON_CORE_MWDT0
            self.sw_reset = true;
        }
        for (core, reset) in self.inner.cores.take_resets().into_iter().enumerate() {
            if reset {
                // Core-local lockup / exception reset: keep chip up, report Lockup.
                self.reset_cause = 0x1a; // RESET_REASON_CPU_LOCKUP
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
        // Convert SYSTIMER ticks-until-IRQ into CPU cycles so Machine idle_advance
        // can skip FreeRTOS vTaskDelay (hello_world's 10 s countdown) instead of
        // burning one cycle per idle round when next_deadline is None.
        let stalled = self.inner.cores.stalled();
        let ticks = self.inner.periph.systimer.ticks_until_irq(stalled)?;
        if ticks == 0 { return Some(1); }
        let hz = self.inner.clocks.rates().cpu_hz.max(1);
        let source = if self.inner.periph.systimer.source_rc_fast() {
            self.inner.clocks.rc_fast_hz().max(1)
        } else {
            40_000_000
        };
        // Inverse of Systimer::advance_cpu: ticks ≈ cycles * source * 2 / (cpu_hz * 5)
        // => cycles ≈ ticks * cpu_hz * 5 / (source * 2), ceil.
        let cycles = (u128::from(ticks) * u128::from(hz) * 5).div_ceil(u128::from(source) * 2);
        Some(cycles.min(u128::from(u64::MAX)).max(1) as u64)
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

    fn boot_app(&mut self, app_off: usize) -> Result<u32, String> {
        // Skip mask ROM: parse the IDF image, program Flash MMU (32 KiB pages, same as
        // the 2nd-stage bootloader), open SPI0 AXI, map flash-resident segments and
        // copy SRAM/RTC ones. Entry is ESP_IMAGE_HEADER.entry (often in SRAM).
        use crate::cache::{FLASH_MMU, FLASH_VADDR};
        const PAGE_SHIFT: u32 = 15; // 32 KiB
        let image = self.flash.get(app_off..).ok_or("app offset beyond flash")?;
        let img = esp_soc::image::parse(image)?;
        // Page size + AXI enable (bootloader writes SPI_MEM_AXI_CTRL / MMU page_size).
        self.write32(FLASH_MMU + 0x384, 3 << 3)
            .map_err(|e| format!("boot_app page_size: {e:?}"))?;
        self.write32(FLASH_MMU + 0x3c, 0x4000_0001)
            .map_err(|e| format!("boot_app axi: {e:?}"))?;
        // Clear a 4 MiB DROM/IROM window (128 × 32 KiB).
        for idx in 0..0x80u32 {
            self.write32(FLASH_MMU + 0x380, idx)
                .map_err(|e| format!("boot_app mmu idx: {e:?}"))?;
            self.write32(FLASH_MMU + 0x37c, 0)
                .map_err(|e| format!("boot_app mmu clr: {e:?}"))?;
        }
        let mask = (1u32 << PAGE_SHIFT) - 1;
        for s in &img.segments {
            let start = app_off + s.file_off as usize;
            let end = start + s.len as usize;
            if end > self.flash.len() {
                return Err("segment beyond flash".into());
            }
            if (FLASH_VADDR..FLASH_VADDR + 0x1000_0000).contains(&s.load_addr) {
                if (s.load_addr & mask) != (start as u32 & mask) {
                    return Err(format!(
                        "segment {:#x} not page-aligned with flash offset {:#x}",
                        s.load_addr, start
                    ));
                }
                let first_page = (start as u32) >> PAGE_SHIFT;
                let npages = ((s.load_addr & mask) + s.len + mask) >> PAGE_SHIFT;
                for i in 0..npages {
                    let idx = ((s.load_addr - FLASH_VADDR) >> PAGE_SHIFT) + i;
                    self.write32(FLASH_MMU + 0x380, idx)
                        .map_err(|e| format!("boot_app mmu idx: {e:?}"))?;
                    // valid (bit12) | physical page — page_size lives in MMU power, not the entry
                    self.write32(FLASH_MMU + 0x37c, (1 << 12) | (first_page + i))
                        .map_err(|e| format!("boot_app mmu map: {e:?}"))?;
                }
            } else {
                let data = self.flash[start..end].to_vec();
                SocBus::load_bytes(self, s.load_addr, &data)?;
            }
        }
        Ok(img.entry)
    }

    fn reboot(&mut self, mac: [u8; 6]) -> u32 {
        let cause = self.reset_cause;
        let efuse = self.inner.periph.efuse.clone();
        let strap = self.inner.pads.strap;
        let usb_attached = true;
        let log_unknown = self.misc.log_unknown;
        let log_all = self.misc.log_all;
        let mut inner = S31Bus::new();
        inner.periph.efuse = efuse;
        inner.pads.strap = strap;
        inner.periph.usb.attach_host(usb_attached);
        // Publish the cause the ROM reads (HPCOREn_RESET_CAUSE[6:1]).
        inner.interrupts.reset_cause = [cause << 1; 2];
        // Non-volatile across a chip reset: NOR contents (with guest writes), mask ROM, PCB.
        let nor = self.inner.cache.take_flash();
        if !nor.is_empty() { self.flash = nor; }
        let _ = inner.cache.set_flash(&self.flash);
        if let Some(image) = &self.rom_image {
            inner.mspi.enable_legacy_rom_lengths();
            let _ = inner.load(bus::ROM_BASE, image, image.len());
        }
        if let Some(b) = &self.board_inputs { let _ = b.apply(&mut inner); }
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
        while let Some(p) = self.inner.periph.usb.host_receive() {
            usb.extend(p);
        }
        [
            usb,
            std::mem::take(&mut self.inner.periph.uart0.tx_out),
            Vec::new(),
            Vec::new(),
        ]
    }

    fn serial_input(&mut self, data: &[u8]) {
        self.inner.periph.usb.attach_host(true);
        for chunk in data.chunks(64) {
            if chunk.is_empty() {
                continue;
            }
            if self.inner.periph.usb.host_send(chunk).is_ok() {
                self.irq_dirty = true;
            }
        }
    }

    fn uart_input(&mut self, n: usize, data: &[u8]) {
        if n != 0 {
            return;
        }
        let before = self.inner.periph.uart0.irq();
        let _ = self.inner.uart_input(data);
        self.irq_dirty |= before != self.inner.periph.uart0.irq();
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
            "esp32s31: cycles={} flash={}KiB CLI Machine path (ROM boot + --boot app MMU map)",
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
        let before = self.inner.periph.systimer.irqs();
        self.inner.periph.systimer.advance_cpu(
            cycles as u64,
            hz,
            self.inner.clocks.rc_fast_hz(),
            self.inner.cores.stalled(),
        );
        if self.inner.periph.systimer.irqs() != before {
            self.irq_dirty = true;
        }
        self.inner.clocks.mpll.advance(
            cycles,
            self.inner.analog.master_enabled() && self.inner.clocks.modem_on(),
        );
        self.inner.clocks.advance_calibration(cycles);
        // Machine time drives the RTC domains (timer + calibration) that were formerly opt-in:
        // TIMG0 RTC_CALI counter both count from the modeled source rates.
        self.inner.rtc_timer.advance_cpu_cycles(cycles as u64, hz, self.inner.clocks.rtc_slow_hz());
        let source = self.inner.clocks.calibration_hz(self.inner.rtc_calibration.source());
        let group = self.inner.periph.timg[0].wdt.clock_read();
        self.inner.rtc_calibration.advance_cpu(cycles as u64, hz, source, group);
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
    fn boot_app_maps_flash_image_and_returns_entry() {
        use crate::cache::FLASH_MMU;
        use emu_core::Bus;
        // Minimal ESP image: magic, 1 flash segment at 0x4000_0000, entry = that vaddr.
        // Payload must share the 32 KiB page offset with the load address (C6/P4 rule).
        // Layout: header+meta immediately before a 32 KiB-aligned payload at 0x8000.
        let mut flash = vec![0xffu8; 0x10000];
        let page = 0x8000usize;
        let payload_off = page;
        let meta_off = payload_off - 8;
        let hdr_off = meta_off - 24;
        flash[hdr_off] = 0xE9;
        flash[hdr_off + 1] = 1; // n_seg
        flash[hdr_off + 4..hdr_off + 8].copy_from_slice(&0x4000_0000u32.to_le_bytes());
        flash[meta_off..meta_off + 4].copy_from_slice(&0x4000_0000u32.to_le_bytes());
        flash[meta_off + 4..meta_off + 8].copy_from_slice(&0x20u32.to_le_bytes());
        for i in 0..0x20 {
            flash[payload_off + i] = i as u8;
        }
        let mut bus = SocBus::new(1 << 20, [0; 6]);
        bus.write_flash(0, &flash).unwrap();
        let entry = bus.boot_app(hdr_off).unwrap();
        assert_eq!(entry, 0x4000_0000);
        let vpage = (0x4000_0000u32 - 0x4000_0000) / 0x8000;
        bus.write32(FLASH_MMU + 0x380, vpage).unwrap();
        let got = bus.read32(FLASH_MMU + 0x37c).unwrap();
        assert_eq!(got, (1 << 12) | (payload_off as u32 / 0x8000), "{got:#x}");
    }

    #[test]
    fn software_reset_latches_cause_0x0c() {
        use emu_core::Bus;
        let mut bus = SocBus::new(1 << 20, [0; 6]);
        assert_eq!(bus.reset_cause, esp_periph::RST_POWERON);
        // esp_cpu_reset → LP_AONCLKRST_HPCORE0_SW_RESET (bit 20) via ResetControl.
        bus.write32(crate::reset::BASE, 0x4001_0000 | (1 << 20)).unwrap();
        assert!(bus.inner.reset.request.is_some());
        bus.poll_chip_reset();
        assert!(bus.sw_reset);
        assert_eq!(bus.reset_cause, esp_periph::RST_SW_CPU);
        let cause = esp_soc::SocBus::reboot(&mut bus, [0; 6]);
        assert_eq!(cause, esp_periph::RST_SW_CPU);
        assert_eq!(bus.reset_cause, esp_periph::RST_SW_CPU);
        let raw = bus.read32(crate::interrupts::RESET_CAUSE0).unwrap();
        assert_eq!((raw >> 1) & 0x3f, esp_periph::RST_SW_CPU);
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
