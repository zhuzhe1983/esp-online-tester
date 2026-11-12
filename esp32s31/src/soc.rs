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
pub const CPU_HZ: u64 = crate::periph::CPU_HZ;

pub struct S31;
pub type Machine = esp_soc::Machine<S31>;

pub fn machine(mac: [u8; 6], flash_size: usize) -> Machine {
    let mut m = Machine::new(mac, SocBus::new(flash_size, mac));
    m.set_debug(&esp_soc::DebugFlags::from_env());
    m
}

/// The machine's time base: one cycle is one tick of `periph::CPU_HZ` (320 MHz), whatever CPU
/// clock the firmware selects. Timers, the board and the peripherals advance by emulated time, so a
/// firmware still in the 40 MHz ROM/bootloader clock runs faster than on silicon, not with a faster clock: its delays,
/// timeouts and FreeRTOS ticks keep their length.
const TIME_HZ: u64 = crate::periph::CPU_HZ;
/// Modem TSF timer latch request (bits 0-3); the latched value follows at +0xC/+0x10.
const TSF_LATCH: u32 = 0x2010_d814;

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
    /// emulated nanoseconds the board has been advanced to, and the sub-nanosecond remainder
    board_ns: u64, board_ns_frac: u64,
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
    /// The pad levels last given to the board (`Pads::board_levels`).
    pin_levels: u64,
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
    /// In-package PSRAM behind the MSPI (0 = none).
    pub psram_bytes: usize,
    /// NOR status registers 1-2 at power-up (SR2 bit 1 = QE is non-volatile on GigaDevice/Winbond).
    pub flash_status: u16,
    /// eFuse read-register values (register offset, value) the board was programmed with at the
    /// factory, e.g. USER_DATA word 0 at 0x88 where a board records its revision. Empty = blank chip.
    pub efuse: Vec<(u32, u32)>,
}

impl BoardInputs {
    /// Winbond W25Q128JV (0xef4018) with its SFDP header, zero regi2c seeds, a 480 MHz
    /// SPLL input, idle-low pads and GPIO36 high. pmu_init.c reads GPIO36 as the flash
    /// LDO strap; high selects the internal flash supply path a 3.3 V devkit uses.
    pub fn devkit_w25q128() -> Self {
        let mut sfdp = vec![0x53, 0x46, 0x44, 0x50, 0x05, 0x01, 0x00, 0xff, 0x00, 0x05, 0x01, 0x10, 0x80, 0x00, 0x00, 0xff];
        sfdp.extend([0xff; 0x70]);
        Self { flash_id: [0xef, 0x40, 0x18], sfdp, bias: [0; 2], perif_glitch: 0, dig_reg: [0; 4], bod: 0,
            bbpll_hz: 480_000_000, idle_level: Some(false), inputs: vec![(36, true)], psram_bytes: 0, flash_status: 0, efuse: Vec::new() }
    }

    /// Winbond W25Q16 (0xef4015) — 2 MiB, matching the IDF hello_world image header
    /// (`flash_size = 2MB`). Prefer this for BoardInputs + Machine flash sizing so the
    /// image-header / modelled-flash mismatch warning does not fire.
    pub fn hello_w25q16() -> Self {
        let mut sfdp = vec![0x53, 0x46, 0x44, 0x50, 0x05, 0x01, 0x00, 0xff, 0x00, 0x05, 0x01, 0x10, 0x80, 0x00, 0x00, 0xff];
        sfdp.extend([0xff; 0x70]);
        Self { flash_id: [0xef, 0x40, 0x15], sfdp, bias: [0; 2], perif_glitch: 0, dig_reg: [0; 4], bod: 0,
            bbpll_hz: 480_000_000, idle_level: Some(false), inputs: vec![(36, true)], psram_bytes: 0, flash_status: 0, efuse: Vec::new() }
    }
    /// ESP-Mosaico CoreBoard: GigaDevice GD25Q128E (JEDEC 0xc84018, read from the board with
    /// esptool 5.6.0 `flash-id`), 16 MiB, and the ESP32-S31NRV16's 16 MiB in-package PSRAM. The
    /// SFDP header and the regi2c, SPLL and GPIO36 choices are the devkit's until the board's
    /// own are measured.
    pub fn mosaico_gd25q128() -> Self {
        // Read from the user's board (2026-11-11, esptool 5.6.0): flash status 0x0200 (QE set),
        // eFuse USER_DATA word 0 = 0x0102.
        // No GPIO36 strap: on the Mosaico GPIO36 is the CO5300's QSPI D1 (driven by GP-SPI2), and a
        // board driver on it would contend with the panel bus.
        Self { flash_id: [0xc8, 0x40, 0x18], psram_bytes: 16 << 20, flash_status: 0x0200, inputs: Vec::new(),
            efuse: vec![
                // USER_DATA (0x88) = 0x0102: board v1.2 as esp-mosaico-bsp reads it (onboard/esp_mosaico.c).
                (0x88, 0x0102),
                // RD_DIS (0x30 bits 0-6) bit 1: key block 5 (KEY1) is read-protected, and
                // KEY_PURPOSE_1 (0x38 bits 5-9) = 8, HMAC_UP. The factory app's
                // trial_provider_validate_hmac_efuse_key() requires both (HMAC key ID 1).
                (0x30, 1 << 1), (0x38, 8 << 5),
            ], ..Self::devkit_w25q128() }
    }
    pub fn apply(&self, b: &mut S31Bus, mac: [u8; 6]) -> Result<(), String> {
        let a = &mut b.analog; a.set_bias(self.bias); a.set_perif_glitch(self.perif_glitch); a.set_dig_reg(self.dig_reg); a.set_bod_register(self.bod);
        b.mspi.attach_flash(self.flash_id, self.flash_status).map_err(|e| format!("{e:?}"))?;
        b.mspi.attach_sfdp(0, &self.sfdp).map_err(|e| format!("{e:?}"))?;
        if let Some(level) = self.idle_level { for pin in 0..62 { let _ = b.pads.set_unresolved_sample(pin, Some(level)); } }
        for &(pin, level) in &self.inputs { b.pads.set_input(pin, Some(level)).map_err(|e| format!("{e:?}"))?; }
        b.mspi.set_bbpll_input(Some(self.bbpll_hz)).map_err(|e| format!("{e:?}"))?;
        if self.psram_bytes != 0 { b.set_psram_size(self.psram_bytes)?; }
        if !self.efuse.is_empty() {
            // The shadow words start at RD_WR_DIS0 (0x2c): register `off` is word (off - 0x2c) / 4.
            let mut image = crate::efuse::ReadImage::blank_development();
            for &(off, value) in &self.efuse {
                let word = off.checked_sub(0x2c).map(|d| d as usize / 4).filter(|&w| w < image.words.len() && off % 4 == 0)
                    .ok_or_else(|| format!("eFuse register {off:#x} is not a read register"))?;
                image.words[word] |= value;
            }
            // BLOCK1 MAC (efuse_reg.h RD_MAC_SYS0/1, 0x50/0x54): bytes 2-5 big-endian, then bytes 0-1
            // and MAC_EXT ff:fe, as the C6 model and esp_efuse_mac_get_default read them. The address
            // is the run's (--mac), never the board's own.
            image.words[(0x50 - 0x2c) / 4] = u32::from_be_bytes([mac[2], mac[3], mac[4], mac[5]]);
            image.words[(0x54 - 0x2c) / 4] = u32::from(mac[0]) << 8 | u32::from(mac[1]) | 0xfffe << 16;
            b.periph.efuse.set_read_image(image)?;
        }
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
            cycles: 0, board_ns: 0, board_ns_frac: 0,
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
            pin_levels: !0,
        }
    }

    pub fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let target = self
            .flash
            .get_mut(offset..)
            .and_then(|tail| tail.get_mut(..data.len()))
            .ok_or("flash image too large")?;
        target.copy_from_slice(data);
        self.inner.invalidate_code();
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
    /// The board's I2C devices: bus 0 on I2C0, bus 1 on I2C1.
    pub fn attach_board_devices(&mut self) {
        for (pin, level) in self.board.input_levels() { let _ = self.inner.pads.set_input(usize::from(pin), Some(level)); }
        for i in &mut self.inner.i2c { i.clear_devices(); }
        for (bus, address, device) in self.board.i2c_devices() {
            if let Some(i) = self.inner.i2c.get_mut(bus as usize) { i.attach(address, device); }
        }
    }
    pub fn set_board_inputs(&mut self, inputs: BoardInputs) -> Result<(), String> {
        inputs.apply(&mut self.inner, self.mac)?;
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

    /// After a store: pad level changes, then a finished GP-SPI2 transaction, to the board. Only
    /// GPIO stores and a started transfer do any work here.
    fn deliver_board_events(&mut self, address: u32) {
        if address >> 24 != 0x20 { return; }
        if address & !3 == TSF_LATCH { self.latch_tsf(); }
        if crate::pads::Pads::is_mmio(address) { self.deliver_pin_levels(); self.irq_dirty = true; }
        self.deliver_spi();
    }

    /// GP-SPI2 and GP-SPI3 transactions: the DMA data phase first (when the channel runs), then the board.
    fn deliver_spi(&mut self) {
        if self.inner.spi2.dma_tx_pending.is_some() && self.inner.spi_dma_tx(2) { self.irq_dirty = true; }
        if self.inner.spi3.dma_tx_pending.is_some() && self.inner.spi_dma_tx(3) { self.irq_dirty = true; }
        if let Some(transfer) = self.inner.spi2.take_transfer() {
            let rx = self.board.spi_transfer(2, &transfer.tx, transfer.rx_len);
            self.inner.spi2.finish_transfer(transfer, &rx);
            self.irq_dirty = true;
        }
        if let Some(transfer) = self.inner.spi3.take_transfer() {
            let rx = self.board.spi_transfer(3, &transfer.tx, transfer.rx_len);
            self.inner.spi3.finish_transfer(transfer, &rx);
            self.irq_dirty = true;
        }
        for host in [2u8, 3] {
            let spi = if host == 2 { &mut self.inner.spi2 } else { &mut self.inner.spi3 };
            if let Some(data) = spi.dma_rx.take() { self.spi_dma_rx(host, &data); }
        }
    }

    /// MISO bytes of a DMA-receiving GP-SPI transfer into its AXI-GDMA IN channel (SPI2 trigger 1,
    /// SPI3 trigger 2), ended with SUC_EOF as the SPI's RX EOF does.
    fn spi_dma_rx(&mut self, host: u8, data: &[u8]) {
        let trigger = u32::from(host - 1);
        let Some(ch) = self.inner.axi_dma.gdma.in_channel_for(trigger) else {
            let running: Vec<(usize, u32)> = (0..3).filter(|&i| self.inner.axi_dma.gdma.inp[i].running).map(|i| (i, self.inner.axi_dma.gdma.inp[i].peri_sel)).collect();
            eprintln!("[spi{host}] {} MISO bytes by DMA, but no AXI-GDMA IN channel on trigger {trigger} runs (running: {running:?})", data.len());
            return;
        };
        let mut channel = self.inner.axi_dma.gdma.inp[ch];
        let mut irq_changed = false;
        let _ = channel.receive(self, data, Some(data.len() as u32), true, |a| a >> 24 == 0x20, &mut irq_changed);
        self.inner.axi_dma.gdma.inp[ch] = channel;
        self.irq_dirty = true;
    }

    /// The modem's TSF timers (802.11 timing synchronization, microseconds). The Wi-Fi library
    /// reads timer n (0-3) by setting bit n of 0x2010D814, reading the latched value from
    /// 0x2010D820 (low) and 0x2010D824 (high), then clearing the bit (`hal_get_tsf_time`). With the block as plain storage the latch never moved, so every
    /// caller waiting for time to pass spun on it (half the host time of a Wi-Fi session). A
    /// request bit latches emulated time; the four timers share one count (no per-timer offset or
    /// TSF adjustment is modelled).
    fn latch_tsf(&mut self) {
        let req = self.inner.unmodelled.get(&TSF_LATCH).copied().unwrap_or(0);
        if req & 0xf == 0 { return; }
        let us = self.now_us();
        self.inner.unmodelled.insert(TSF_LATCH + 0xc, us as u32);
        self.inner.unmodelled.insert(TSF_LATCH + 0x10, (us >> 32) as u32);
    }

    fn deliver_pin_levels(&mut self) {
        let levels = self.inner.pads.board_levels();
        let diff = levels ^ self.pin_levels;
        if diff == 0 { return; }
        self.pin_levels = levels;
        let changes: Vec<(u8, bool)> = (0..64u8).filter(|&p| diff >> p & 1 != 0).map(|p| (p, levels >> p & 1 != 0)).collect();
        if let Some(ev) = &mut self.gpio_events { for &(pin, level) in &changes { ev.push((self.cycles, pin, level)); } }
        self.board.gpio_changes(&changes);
    }

    /// Emulated microseconds, on the run's time base (`cycles` at `periph::CPU_HZ`).
    fn now_us(&self) -> u64 { self.cycles / (crate::periph::CPU_HZ / 1_000_000) }
    /// The WiFi MAC's queued transmissions, then the virtual air and the network (`wifi.rs`).
    fn wifi_step(&mut self) {
        let now_us = self.now_us();
        if !self.inner.wifi_mac.tx_pending.is_empty() {
            let frames = self.debug.has("wifi-frames");
            self.inner.wifi_tx_step(now_us, frames);
            self.irq_dirty = true;
        }
        if self.inner.wifi_mac.link.ap().is_some() {
            if self.inner.wifi_air_step(now_us) { self.irq_dirty = true; }
            self.inner.wifi_mac.link.net_step(now_us);
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
    fn i2s_input(&mut self, port: usize) -> Option<&mut esp_periph::i2s::PcmInput> { self.inner.i2s.get_mut(port).map(|i| i.rx_input()) }
    fn cycles(&self) -> u64 {
        self.cycles
    }

    fn next_deadline(&self) -> Option<u64> {
        // Convert SYSTIMER ticks-until-IRQ into CPU cycles so Machine idle_advance
        // can skip FreeRTOS vTaskDelay (hello_world's 10 s countdown) instead of
        // burning one cycle per idle round when next_deadline is None.
        let stalled = self.inner.cores.stalled();
        let hz = TIME_HZ;
        // The board's next input edge (the Mosaico's TE), in emulated nanoseconds.
        let board = self.board.next_deadline().map(|ns| {
            let dt = u128::from(ns.saturating_sub(self.board_ns)) * u128::from(hz);
            (dt.div_ceil(1_000_000_000).min(u128::from(u64::MAX)) as u64).max(1)
        });
        let Some(ticks) = self.inner.periph.systimer.ticks_until_irq(stalled) else { return board };
        if ticks == 0 { return Some(1); }
        let source = if self.inner.periph.systimer.source_rc_fast() {
            self.inner.clocks.rc_fast_hz().max(1)
        } else {
            40_000_000
        };
        // Inverse of Systimer::advance_cpu: ticks ≈ cycles * source * 2 / (cpu_hz * 5)
        // => cycles ≈ ticks * cpu_hz * 5 / (source * 2), ceil.
        let cycles = (u128::from(ticks) * u128::from(hz) * 5).div_ceil(u128::from(source) * 2);
        let timer = cycles.min(u128::from(u64::MAX)).max(1) as u64;
        Some(board.map_or(timer, |b| b.min(timer)))
    }

    fn irq_dirty(&mut self) -> &mut bool {
        &mut self.irq_dirty
    }
    fn set_net_tap(&mut self, tap: esp_soc::netcap::NetTap) -> Result<(), String> {
        self.inner.wifi_mac.link.set_tap(tap);
        Ok(())
    }
    fn add_wifi_ap(&mut self, cfg: esp_soc::wifi::ApConfig) -> Result<(), String> {
        self.inner.wifi_mac.link.add_ap(cfg, &self.debug)
    }
    fn attach_wifi(&mut self, cfg: esp_soc::wifi::ApConfig, nat: Option<esp_soc::nat::Nat>) -> Result<(), String> {
        self.inner.wifi_mac.link.attach_new(cfg, nat, &self.debug);
        Ok(())
    }
    /// The host on the other end of the high-speed USB-OTG port (DR_REG_USB_OTGHS_BASE).
    fn attach_usb_host(&mut self, timing: esp_periph::usb_otg::UsbTiming) -> Result<(), String> {
        let now = self.cycles;
        let mut link = esp_periph::usb_otg::UsbLink::new(timing, crate::periph::CPU_HZ, now);
        link.log = self.inner.usb_otg.logging();
        self.inner.usb_otg.attach(now, link);
        self.irq_dirty = true;
        Ok(())
    }
    fn usb_link(&mut self) -> Option<&mut esp_periph::usb_otg::UsbLink> { self.inner.usb_otg.link.as_deref_mut() }

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
        inner.spi2.log = self.inner.spi2.log;
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
        if let Some(b) = &self.board_inputs { let _ = b.apply(&mut inner, mac); }
        // The virtual AP is outside the chip: it survives the reset, as on the C6 (surviving_reboot).
        inner.wifi_mac.link = std::mem::take(&mut self.inner.wifi_mac.link).surviving_reboot();
        self.inner = inner;
        self.attach_board_devices();                     // the board's I2C devices, on the new controllers
        self.deliver_pin_levels();                       // the pads are back to their reset state
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

    /// The I2S controller that has sent the most frames (the Mosaico's speaker codec is on I2S0).
    fn audio(&self) -> (&[i16], u32) {
        let i = &self.inner.i2s;
        let a = if i[1].frames_out > i[0].frames_out { &i[1] } else { &i[0] };
        (&a.pcm, a.sample_rate)
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
        self.inner.spi2.log = f.has("spi"); self.inner.spi3.log = f.has("spi");
        esp_periph::Device::debug(&mut self.inner.axi_dma, f.has("dma"));
        esp_periph::Device::debug(&mut self.inner.dma2d, f.has("dma2d")); esp_periph::Device::debug(&mut self.inner.ppa, f.has("ppa"));
        for i in &mut self.inner.i2c { i.log = f.has("i2c"); }
        esp_periph::Device::debug(&mut self.inner.wifi_mac, f.has("wifi"));
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
        let board = self.board.report();
        let u = &self.inner.unmodelled;
        let list: Vec<String> = u.iter().take(48).map(|(a, v)| format!("{a:#010x}={v:#x}")).collect();
        format!(
            "esp32s31: cycles={} flash={}KiB CLI Machine path (ROM boot + --boot app MMU map)\nesp32s31: {} unmodelled peripheral registers used as storage{}{}\nesp32s31: {} peripheral registers whose model refused an access (dropped; last value written): {}",
            self.cycles,
            self.flash.len() / 1024,
            u.len(),
            if u.is_empty() { "" } else { ": " },
            list.join(" "),
            self.inner.refused.len(),
            self.inner.refused.iter().take(48).map(|(a, (n, v))| format!("{a:#010x}x{n}(last write {v:#x})")).collect::<Vec<_>>().join(" ")
        ) + &format!("\nesp32s31: GP-SPI2 {} transfers", self.inner.spi2.transfers) + &self.inner.i2s.iter().enumerate().filter(|(_, i)| i.frames_out != 0 || i.bytes_in != 0 || i.rx_running())
            .map(|(n, i)| format!("\nesp32s31: I2S{n} TX {} frames at {} Hz, RX {} bytes to DMA (RX rate {:?}, RX_CONF {:#x}, RX_CONF1 {:#x})", i.frames_out, i.sample_rate, i.bytes_in, i.rx_rate(), i.read(0x20), i.read(0x28))).collect::<String>() + if board.is_empty() { "" } else { "\n" } + &board
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
    #[inline(always)]
    fn page_versions(&self) -> &[u32] {
        self.inner.page_versions()
    }
    #[inline(always)]
    fn code_page(&mut self, pc: u32) -> u32 {
        self.inner.code_page(pc)
    }
    fn fetch_ahead(&mut self, pc: u32) -> Option<[u8; 4]> {
        self.inner.fetch_ahead(pc)
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
                self.deliver_board_events(a);
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
                self.deliver_board_events(a);
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
                self.deliver_board_events(a);
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
        // The lines the models advanced here can raise without flagging `irq_dirty` themselves.
        let lines = |b: &S31Bus| (b.usb_otg.irq(), b.rtc_wdt.irq(), b.periph.timg[0].irq_wdt(), b.periph.timg[1].irq_wdt());
        let lines_before = lines(&self.inner);
        let hz = TIME_HZ;
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
        if self.inner.i2c.iter().any(|i| i.is_active()) {
            // The controllers count their source clock (XTAL, 40 MHz) in CLK_CONF's divider.
            let t = u64::from(cycles) * 40_000_000 + self.inner.i2c_frac;
            let (ticks, frac) = (t / hz, t % hz);
            self.inner.i2c_frac = frac;
            for i in &mut self.inner.i2c { if i.is_active() { esp_periph::Device::tick(i, ticks); } }
            self.irq_dirty = true;
        }
        if self.inner.spi2.dma_tx_pending.is_some() || self.inner.spi3.dma_tx_pending.is_some() { self.deliver_spi(); }
        for n in 0..2 {
            if self.inner.i2s[n].poll_active() {
                let mut changed = self.inner.i2s_tx(n, u64::from(cycles));
                if self.inner.i2s[n].rx_running() { changed |= self.inner.i2s_rx(n, u64::from(cycles)); }
                if changed { self.irq_dirty = true; }
            }
        }
        // Emulated time for the board (TE), in nanoseconds.
        let t = u128::from(cycles) * 1_000_000_000 + u128::from(self.board_ns_frac);
        self.board_ns += (t / u128::from(hz)) as u64; self.board_ns_frac = (t % u128::from(hz)) as u64;
        self.board.advance_to(self.board_ns);
        let edges = self.board.take_edges();
        if !edges.is_empty() {
            for e in edges { let _ = self.inner.pads.set_input(usize::from(e.pin), Some(e.level)); }
            self.irq_dirty = true;
        }
        esp_periph::Device::tick(&mut self.inner.usb_otg, u64::from(cycles));
        if self.inner.usb_otg.pending() { self.inner.run_usb_otg(); }
        // Machine time drives the RTC domains (timer + calibration) that were formerly opt-in:
        // TIMG0 RTC_CALI counter both count from the modeled source rates.
        self.inner.rtc_timer.advance_cpu_cycles(cycles as u64, hz, self.inner.clocks.rtc_slow_hz());
        self.inner.check_psram_fetch();
        let source = self.inner.clocks.calibration_hz(self.inner.rtc_calibration.source());
        let group = self.inner.periph.timg[0].wdt.clock_read();
        self.inner.rtc_calibration.advance_cpu(cycles as u64, hz, source, group);
        self.wifi_step();
        self.poll_chip_reset();
        // Non-zero only when an interrupt line may have moved (emu_core Bus::tick): every other
        // change sets `irq_dirty`, which the machine checks too. Returning 1 unconditionally made
        // the machine re-derive every line after every round (about 60 % of a Mosaico run).
        u32::from(lines(&self.inner) != lines_before || self.sw_reset)
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
        assert_eq!(S31::CPU_HZ, 320_000_000);
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
        let vpage = 0; // the app maps at the flash window base, virtual page 0
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

    /// GP-SPI2 at DR_REG_GPSPI2_BASE reaches the Mosaico's CO5300 the way esp_lcd_co5300 talks
    /// to it in QSPI mode: a 32-bit address phase (0x02 command / 0x32 pixel write, then the DCS
    /// command byte) and data from the W buffer, the panel powered by GPIO 60 low and out of reset.
    #[test]
    fn gpspi2_qspi_transfers_reach_the_mosaico_panel() {
        use crate::pads::{GPIO_BASE, IO_MUX_BASE};
        use emu_core::Bus;
        let mut bus = SocBus::new(1 << 20, [0; 6]);
        bus.board = crate::board::make_board("mosaico").unwrap();
        for pin in [44u32, 60] { bus.write32(IO_MUX_BASE + 4 * pin, 0x1200).unwrap(); }
        bus.write32(GPIO_BASE + 0x14, 1 << (44 - 32)).unwrap();           // RESET high, power enable low
        bus.write32(GPIO_BASE + 0x44, 1 << (44 - 32) | 1 << (60 - 32)).unwrap();
        let spi = bus::GPSPI2;
        let send = |bus: &mut SocBus, lead: u32, cmd: u32, data: &[u8]| {
            bus.write32(spi + 0x10, 1 << 30 | if data.is_empty() { 0 } else { 1 << 27 | 1 << 13 }).unwrap();
            bus.write32(spi + 0x14, 31 << 27).unwrap();
            bus.write32(spi + 0x04, lead << 24 | cmd << 8).unwrap();
            bus.write32(spi + 0x1c, (data.len() as u32 * 8).max(1) - 1).unwrap();
            for (i, w) in data.chunks(4).enumerate() {
                let mut b = [0u8; 4]; b[..w.len()].copy_from_slice(w);
                bus.write32(spi + 0x98 + 4 * i as u32, u32::from_le_bytes(b)).unwrap();
            }
            bus.write32(spi, 1 << 24).unwrap();
            assert_eq!(bus.read32(spi).unwrap() & 1 << 24, 0, "USR done");
        };
        send(&mut bus, 0x02, 0x2a, &[0, 10, 0, 11]);
        send(&mut bus, 0x02, 0x2b, &[0, 20, 0, 20]);
        send(&mut bus, 0x32, 0x2c, &[0xf8, 0x00, 0x07, 0xe0]);
        let (w, h, frame, written) = bus.board.display().unwrap();
        assert_eq!((w, h, written), (480, 480, 2));
        assert_eq!(&frame[20 * 480 + 10..20 * 480 + 12], [0xf800, 0x07e0]);
        assert_ne!(bus.inner.spi2.int_raw & 1 << 12, 0, "TRANS_DONE");
        // GPIO 60 high cuts the panel's supply: the next window is not taken
        bus.write32(GPIO_BASE + 0x14, 1 << (60 - 32)).unwrap();
        send(&mut bus, 0x32, 0x2c, &[0x12, 0x34]);
        assert_eq!(bus.board.display().unwrap().3, 0, "an unpowered panel lost its RAM");
    }

    /// hal_get_tsf_time: setting bit n of 0x2010D814 latches emulated microseconds into
    /// 0x2010D820/0x2010D824; with no request bit set the latched value stays put.
    #[test]
    fn modem_tsf_request_latches_emulated_microseconds() {
        use emu_core::Bus;
        let mut bus = SocBus::new(1 << 20, [0; 6]);
        bus.cycles = 5 * TIME_HZ + 7 * (TIME_HZ / 1_000_000);
        bus.write32(TSF_LATCH, 1 << 2).unwrap();
        assert_eq!(bus.read32(TSF_LATCH + 0xc).unwrap(), 5_000_007);
        assert_eq!(bus.read32(TSF_LATCH + 0x10).unwrap(), 0);
        bus.write32(TSF_LATCH, 0).unwrap();
        bus.cycles += 10 * TIME_HZ;
        bus.write32(TSF_LATCH, 1 << 4).unwrap();
        assert_eq!(bus.read32(TSF_LATCH + 0xc).unwrap(), 5_000_007, "no request, no latch");
        bus.cycles = 5000 * TIME_HZ;
        bus.write32(TSF_LATCH, 1).unwrap();
        assert_eq!(bus.read32(TSF_LATCH + 0xc).unwrap(), 5_000_000_000u64 as u32);
        assert_eq!(bus.read32(TSF_LATCH + 0x10).unwrap(), 1, "the high word carries");
    }
}
