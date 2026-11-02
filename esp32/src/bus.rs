//! Classic ESP32 memory map and flash-cache MMUs.
//! ESP-IDF v5.5.4 components/soc/esp32/include/soc/soc.h:170-199 gives memory bounds.
//! Flash MMU behavior and the ROM integrity payload mapping are inferred from ROM execution.
use crate::periph::{Peripherals, PERIPH_BASE, PERIPH_END};
use esp_periph::{Device, Rng};
use esp_periph::gdma::{DescriptorWalk, DmaDescriptorFault};
use xtensa_lx7::bus::{Bus, Fault};

pub const DRAM_LOW: u32 = 0x3ffa_e000;
pub const DRAM_HIGH: u32 = 0x4000_0000;
/// Classic DMA engines use the DRAM data window, not CPU-visible memory aliases.
/// ESP-IDF v5.5.4 components/soc/esp32/include/soc/soc.h:203-205 (SOC_DMA_LOW/HIGH);
/// v4.4.8 same file:266-268. IRAM, RTC, ROM and external memory are excluded.
/// Return backing offsets (the ROM-only integrity prefix is not DMA-capable).
#[inline]
pub(crate) fn dma_range(addr: u32, len: usize) -> Option<std::ops::Range<usize>> {
    if !(DRAM_LOW..DRAM_HIGH).contains(&addr) { return None; }
    let off = (addr - ROM_DRAM_LOW) as usize;
    let end = off.checked_add(len)?;
    (end <= (DRAM_HIGH - ROM_DRAM_LOW) as usize).then_some(off..end)
}
// The ECO3 ELF carries one reset-integrity payload 0x504 bytes below documented DRAM.
pub(crate) const ROM_DRAM_LOW: u32 = 0x3ffa_dafc;
pub const IRAM_LOW: u32 = 0x4008_0000;
pub const IRAM_HIGH: u32 = 0x400c_0000;
pub const IROM_MASK_LOW: u32 = 0x4000_0000;
pub const IROM_MASK_HIGH: u32 = 0x4007_0000;
pub const DROM_MASK_LOW: u32 = 0x3ff9_0000;
pub const DROM_MASK_HIGH: u32 = 0x3ffa_0000;
pub const DBUS_LOW: u32 = 0x3f40_0000;
pub const DBUS_HIGH: u32 = 0x3f80_0000;
pub const IBUS_LOW: u32 = 0x400d_0000;
pub const IBUS_HIGH: u32 = 0x4040_0000;
const RTC_FAST_D: u32 = 0x3ff8_0000;
const RTC_FAST_I: u32 = 0x400c_0000;
const RTC_SLOW: u32 = 0x5000_0000;
// DPORT cache-MMU tables: 64 KiB flash pages, bit 8 marks an invalid entry.
const MMU_PRO: u32 = 0x3ff1_0000;
const MMU_APP: u32 = 0x3ff1_2000;
const MMU_INVALID: u32 = 1 << 8;
const PAGE: usize = 0x1_0000;
const WDEV_RND: u32 = 0x3ff7_5144;

fn merge(old: u32, value: u32, addr: u32, size: u32) -> u32 {
    let (shift, mask) = match size { 1 => ((addr & 3) * 8, 0xff), 2 => ((addr & 2) * 8, 0xffff), _ => return value };
    (old & !(mask << shift)) | ((value & mask) << shift)
}

pub struct SocBus {
    pub dram: Vec<u8>,
    pub iram: Vec<u8>,
    pub cache_ram: Vec<u8>,
    pub irom: Vec<u8>,
    pub drom: Vec<u8>,
    pub rtc_fast: Vec<u8>,
    pub rtc_slow: Vec<u8>,
    pub flash: Vec<u8>,
    pub rng: Rng,
    pub mmu: [[u32; 2048]; 2],
    pub periph: Peripherals,
    pub board: esp_soc::Board,
    pub cycles: u64,
    pub last_fault: Option<(u32, bool)>,
    pub irq_dirty: bool,
    pub gpio_events: Option<Vec<(u64, u8, bool)>>,
    pub debug: esp_soc::DebugFlags,
    page_ver: Vec<u32>,
    // Preserve the half reference tick at 160 MHz across batches/clock changes.
    pub(crate) clock_half: u64,
    pub(crate) pins_active: bool,
    pub ble: esp_soc::ble::vhci::Ble,
    pub(crate) deep_sleep: bool,
    // One allocation shared by CPU translation and the command adapter (no flat CPU window).
    pub(crate) psram: Vec<u8>,
}

impl SocBus {
    pub fn new(flash_size: usize, mac: [u8; 6]) -> Self {
        Self {
            dram: vec![0; (DRAM_HIGH - ROM_DRAM_LOW) as usize],
            iram: vec![0; (IRAM_HIGH - IRAM_LOW) as usize],
            cache_ram: vec![0; 0x1_0000],
            irom: vec![0; (IROM_MASK_HIGH - IROM_MASK_LOW) as usize],
            drom: vec![0; (DROM_MASK_HIGH - DROM_MASK_LOW) as usize],
            rtc_fast: vec![0; 0x2000],
            rtc_slow: vec![0; 0x2000],
            flash: vec![0xff; flash_size],
            rng: Rng::new(),
            mmu: [[MMU_INVALID; 2048]; 2],
            periph: Peripherals::new(mac),
            board: Box::new(esp_soc::NoBoard),
            cycles: 0,
            last_fault: None,
            irq_dirty: true,
            gpio_events: None,
            debug: Default::default(),
            page_ver: vec![0; 0x10021],
            clock_half: 0,
            pins_active: false,
            ble: Default::default(),
            deep_sleep: false,
            psram: Vec::new(),
        }
    }
    fn load<const N: usize>(&mut self, a: u32) -> Result<[u8; N], Fault> {
        if Self::is_periph(a) {
            return Ok(self.periph_read(a, N as u32).to_le_bytes()[..N].try_into().unwrap());
        }
        match self.memory(a) {
            Some((b, o, _)) if b.len().saturating_sub(o) >= N => Ok(b[o..o + N].try_into().unwrap()),
            _ => self.load_miss(a),
        }
    }
    fn store(&mut self, a: u32, bytes: &[u8]) -> Result<(), Fault> {
        if Self::is_periph(a) {
            let mut value = [0; 4];
            value[..bytes.len()].copy_from_slice(bytes);
            self.periph_write(a, u32::from_le_bytes(value), bytes.len() as u32);
            return Ok(());
        }
        match self.memory(a) {
            Some((b, o, true)) if b.len().saturating_sub(o) >= bytes.len() => {
                b[o..o + bytes.len()].copy_from_slice(bytes);
                self.written(a, bytes.len());
                Ok(())
            }
            _ => self.store_miss(a, bytes),
        }
    }
    // External RAM is disjoint from memory()'s ordinary windows. Dispatch only
    // after a miss, leaving the mapped-memory and peripheral paths unchanged.
    #[cold]
    fn load_miss<const N: usize>(&mut self, a: u32) -> Result<[u8; N], Fault> {
        if (crate::psram::LOW..crate::psram::HIGH).contains(&a) { return self.psram_load(a); }
        self.last_fault = Some((a, false));
        Err(Fault::Unmapped)
    }
    #[cold]
    fn store_miss(&mut self, a: u32, bytes: &[u8]) -> Result<(), Fault> {
        if (crate::psram::LOW..crate::psram::HIGH).contains(&a) { return self.psram_store(a, bytes); }
        self.last_fault = Some((a, true));
        Err(Fault::Prohibited)
    }
    #[cold]
    fn fetch_miss(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        self.last_fault = Some((pc, false));
        if (crate::psram::LOW..crate::psram::HIGH).contains(&pc) {
            Err(Fault::Prohibited)
        } else {
            Err(Fault::Unmapped)
        }
    }
    // 256-byte decode pages. RTC FAST's data and instruction windows share backing bytes.
    fn version_page(addr: u32) -> usize {
        let addr = if (RTC_FAST_D..RTC_FAST_D + 0x2000).contains(&addr) { RTC_FAST_I + addr - RTC_FAST_D } else { addr };
        match addr {
            0x3f40_0000..=0x403f_ffff => ((addr - 0x3f40_0000) >> 8) as usize,
            RTC_SLOW..=0x5000_1fff => 0x10000 + ((addr - RTC_SLOW) >> 8) as usize,
            _ => 0x10020,
        }
    }
    pub(crate) fn written(&mut self, addr: u32, len: usize) {
        if len == 0 { return; }
        let first = Self::version_page(addr);
        let last = Self::version_page(addr + len as u32 - 1);
        let first = if addr & 255 < emu_core::bus::PREV_PAGE_BYTES { first.saturating_sub(1) } else { first };
        for version in &mut self.page_ver[first..=last] { *version = version.wrapping_add(1); }
    }
    fn flash_written(&mut self, off: usize, len: usize) {
        if len == 0 { return; }
        for slot in 0..128 {
            let entry = self.mmu[0][slot];
            if entry & MMU_INVALID != 0 { continue; }
            let start = (entry as usize & 255) * PAGE;
            if off < start + PAGE && start < off + len {
                if let Some(addr) = Self::mmu_address(slot) { self.written(addr, PAGE); }
            }
        }
    }
    fn mmu_address(slot: usize) -> Option<u32> {
        match slot {
            0..=63 => Some(DBUS_LOW + slot as u32 * PAGE as u32),
            77..=127 => Some(0x4000_0000 + (slot as u32 - 64) * PAGE as u32),
            _ => None,
        }
    }
    pub(crate) fn flash_off(&self, addr: u32) -> Option<usize> {
        let (table, index) = if (DBUS_LOW..DBUS_HIGH).contains(&addr) {
            (0, ((addr - DBUS_LOW) >> 16) as usize)
        } else if (IBUS_LOW..IBUS_HIGH).contains(&addr) {
            (64, ((addr - 0x4000_0000) >> 16) as usize)
        } else {
            return None;
        };
        let entry = self.mmu[0][table + index];
        if entry & MMU_INVALID != 0 {
            return None;
        }
        let off = (entry as usize & 0xff) * PAGE + (addr as usize & 0xffff);
        (off < self.flash.len()).then_some(off)
    }
    fn memory(&mut self, addr: u32) -> Option<(&mut Vec<u8>, usize, bool)> {
        match addr {
            IROM_MASK_LOW..=0x4006_ffff => {
                Some((&mut self.irom, (addr - IROM_MASK_LOW) as usize, false))
            }
            DROM_MASK_LOW..=0x3ff9_ffff => {
                Some((&mut self.drom, (addr - DROM_MASK_LOW) as usize, false))
            }
            ROM_DRAM_LOW..=0x3fff_ffff => {
                Some((&mut self.dram, (addr - ROM_DRAM_LOW) as usize, true))
            }
            0x4007_0000..=0x4007_ffff => {
                Some((&mut self.cache_ram, (addr - 0x4007_0000) as usize, true))
            }
            IRAM_LOW..=0x400b_ffff => Some((&mut self.iram, (addr - IRAM_LOW) as usize, true)),
            0x3ff8_0000..=0x3ff8_1fff => {
                Some((&mut self.rtc_fast, (addr - RTC_FAST_D) as usize, true))
            }
            0x400c_0000..=0x400c_1fff => {
                Some((&mut self.rtc_fast, (addr - RTC_FAST_I) as usize, true))
            }
            0x5000_0000..=0x5000_1fff => {
                Some((&mut self.rtc_slow, (addr - RTC_SLOW) as usize, true))
            }
            _ => self
                .flash_off(addr)
                .map(|off| (&mut self.flash, off, false)),
        }
    }
    fn ahb_to_apb(addr: u32) -> Option<u32> {
        (0x6000_0000..0x6004_0000).contains(&addr)
            .then(|| addr - 0x6000_0000 + 0x3ff4_0000)
    }
    fn is_periph(addr: u32) -> bool {
        (PERIPH_BASE..PERIPH_END).contains(&addr) || Self::ahb_to_apb(addr).is_some()
    }
    fn mmu_slot(addr: u32) -> Option<(usize, usize)> {
        for (cpu, base) in [(0, MMU_PRO), (1, MMU_APP)] {
            if (base..base + 0x2000).contains(&addr) {
                return Some((cpu, ((addr - base) >> 2) as usize));
            }
        }
        None
    }
    fn periph_read(&mut self, addr: u32, size: u32) -> u32 {
        if self.pins_active { self.deliver_board_inputs(); }
        if let Some((cpu, n)) = Self::mmu_slot(addr) {
            return self.mmu[cpu][n];
        }
        let a = Self::ahb_to_apb(addr & !3).unwrap_or(addr & !3);
        let w = if a == WDEV_RND {
            self.rng.now = self.cycles as u32;
            self.rng.read(0)
        } else {
            self.periph.read32(a)
        };
        match size {
            1 => (w >> ((addr & 3) * 8)) & 0xff,
            2 => (w >> ((addr & 2) * 8)) & 0xffff,
            _ => w,
        }
    }
    fn periph_write(&mut self, addr: u32, value: u32, size: u32) {
        if let Some((cpu, n)) = Self::mmu_slot(addr) {
            self.mmu[cpu][n] = value & 0x1ff;
            if cpu == 0 { if let Some(addr) = Self::mmu_address(n) { self.written(addr, PAGE); } }
            return;
        }
        let a = Self::ahb_to_apb(addr & !3).unwrap_or(addr & !3);
        if a == WDEV_RND {
            return;
        }
        let v = if size == 4 { value } else { merge(self.periph.read32(a), value, addr, size) };
        let old_enable = self.periph.gpio.gpio.enable;
        // Configuration-only bookkeeping; no new work in ordinary ticks.
        // Only physical DVP routing bits matter (IDF v4.4.7 gpio_reg.h,
        // io_mux_reg.h and rtc_io_reg.h). Unrelated pad writes cannot close
        // a valid window. Loss/restoration without elapsed time still does.
        let camera_route_mask = match a {
            a if ((140..148).chain([28, 190, 191, 192])).any(|s| a == 0x3ff4_4130 + 4 * s) => 0xff,
            0x3ff4_906c | 0x3ff4_9070 | 0x3ff4_9074 | 0x3ff4_907c |
            0x3ff4_9004 | 0x3ff4_9010 | 0x3ff4_9014 | 0x3ff4_9018 |
            0x3ff4_9080 | 0x3ff4_908c | 0x3ff4_9024 => (7 << 12) | (1 << 9),
            0x3ff4_847c => (1 << 27) | (1 << 24), // GPIO36/39
            0x3ff4_8480 => (1 << 29) | (1 << 28), // GPIO34/35
            0x3ff4_8484 => 1 << 17, // GPIO25
            _ => 0,
        };
        if camera_route_mask != 0 && (self.periph.read32(a) ^ v) & camera_route_mask != 0 {
            self.board.invalidate_camera_frame_start();
        }
        if (0x3ff4_8800..=0x3ff4_88ff).contains(&a) { self.periph.adc.now_cycles = self.cycles; }
        self.periph.write32(a, v);
        // IDF v5.5.4 esp32/rtc_cntl_reg.h:2017-2039. Only a CRC START write
        // touches RTC memory; no new state or work in reads, ticks or instruction execution.
        if a == 0x3ff4_8100 && v & (1 << 8) != 0 {
            self.periph.rtc.memory_crc(&self.rtc_fast);
        }
        if matches!(a & !0xfff, 0x3ff4_4000 | 0x3ff4_9000) && self.board.uses_gpio_waveform() {
            self.board.gpio_waveform_at(self.cycles, self.periph.gpio.routes());
        }
        // IDF v5.5.4 rtc_cntl_reg.h:247-252,1281-1287: SLEEP_EN and
        // DIG_PWC.DG_WRAP_PD_EN distinguish deep sleep from light sleep.
        if a == 0x3ff4_8018 && v & (1 << 31) != 0 {
            self.enter_deep_sleep();
        }
        if self.pins_active {
            if let Some(port) = [0x3ff4_0000, 0x3ff5_0000, 0x3ff6_e000].iter().position(|&base| a == base) {
                self.board.uart_tx(self.cycles, self.periph.uart_route(port), v as u8);
            }
            if old_enable != self.periph.gpio.gpio.enable && self.periph.gpio.gpio.changes.is_empty() {
                self.board.gpio_output_at(self.cycles, &[], self.periph.gpio.gpio.enable, self.periph.gpio.gpio.out);
            }
        }
        if self.periph.spi_exec {
            self.run_spi();
        }
        self.run_gp_spi();
        self.flush_rmt();
        self.flush_gpio();
        // Clock, matrix and SCCB writes can start/stop autonomous sensor inputs
        // without changing an output GPIO. Reconcile at this same bus horizon.
        if self.pins_active { self.deliver_board_inputs(); }
        if (0x3ff4_f000..0x3ff4_f100).contains(&a) { self.camera_dma_written(); }
        // Configuration horizon, not an idle-tick poll: a qualified receiver
        // must observe even a loss restored before the next DMA step (matrix,
        // IO_MUX/RTC, PWDN, SCCB streaming, XCLK or peripheral clock gating).
        // An initially unavailable source still waits for its first valid route.
        if self.periph.i2s[0].camera.as_ref().is_some_and(|c| c.qualified())
            && self.camera_routed_frame().is_none() {
            self.periph.i2s[0].abort_camera_dma();
        }
        self.irq_dirty = true;
    }
    #[inline]
    fn flush_gpio(&mut self) {
        if self.periph.gpio.gpio.changes.is_empty() { return; }
        let changes = std::mem::take(&mut self.periph.gpio.gpio.changes);
        if let Some(events) = &mut self.gpio_events {
            for &(pin, level) in &changes { events.push((self.cycles, pin, level)); }
        }
        if self.pins_active {
            self.board.gpio_output_at(self.cycles, &changes, self.periph.gpio.gpio.enable, self.periph.gpio.gpio.out);
            self.deliver_board_inputs();
        }
    }
    fn deliver_board_inputs(&mut self) {
        let periph = &self.periph;
        self.board.advance_inputs_with_clock(self.cycles, &|pin| periph.pwm_output(u32::from(pin)));
        for edge in self.board.take_edges() {
            self.periph.gpio.set_input(edge.pin, edge.level);
            self.ext0_wake();
            self.irq_dirty = true;
            if let Some(events) = &mut self.gpio_events { events.push((edge.cycle, edge.pin, edge.level)); }
        }
        for pin in self.board.released_inputs() { esp_soc::SocBus::gpio_release_input(self, pin); }
    }
    #[inline]
    fn flush_rmt(&mut self) {
        for (channel, mut bits) in std::mem::take(&mut self.periph.rmt.done) {
            if let Some((pin, inverted)) = self.periph.gpio.output_pin(crate::rmt::SIGNAL0 + channel) {
                if inverted { for bit in &mut bits { *bit = !*bit; } }
                if self.pins_active { self.board.rmt_frame(pin, &bits); }
            }
        }
    }
    fn run_spi(&mut self) {
        self.periph.spi_exec = false;
        self.periph.spi1.0.execute(&mut self.flash, &mut self.psram);
        for (memory, off, len) in std::mem::take(&mut self.periph.spi1.0.dirty) {
            if memory == esp_periph::DirtyMem::Flash { self.flash_written(off, len); }
        }
    }
    /// Request sensor bytes using current physical XCLK routing, without polling
    /// the camera from the ordinary bus tick. I2S transport is a separate adapter.
    pub fn camera_frame(&mut self) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)> {
        let periph = &self.periph;
        self.board.camera_frame_with_clock(&|pin| periph.pwm_output(u32::from(pin)))
    }
    /// Qualify a camera frame for I2S0's parallel receiver. No FIFO or DMA work
    /// occurs here. esp32-camera v2.1.4 ll_cam.c:350-395 and IDF v4.4.7
    /// gpio_sig_map.h define D0=140, WS/PCLK=28, H/V_SYNC=190/191,
    /// H_ENABLE=192 (constant high). A fixed sensor's inverted data lines
    /// invert the actual bytes; inverted control polarities are unsupported.
    pub fn camera_routed_frame(&mut self) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)> {
        if !self.periph.i2s[0].clock_enabled { return None; }
        let rx = self.periph.i2s[0].camera_rx()?;
        if !rx.started || rx.reset || !rx.slave || !rx.descriptor_enabled || !rx.fifo_mode_forced || ![1, 3].contains(&rx.fifo_mode) { return None; }
        let pins = self.board.camera_dvp_pins()?;
        let gpio = &self.periph.gpio;
        for (signal, pin) in [(28, pins[8]), (190, pins[9]), (191, pins[10])] {
            if gpio.matrix_input_route(signal) != Some((pin, false)) { return None; }
        }
        let enable = gpio.gpio.func_in_sel[192];
        if ![0x30, 0x38].contains(&(enable & 0x3f)) || gpio.signal_input(192) != Some(true) { return None; }
        let mut invert = 0u8;
        for (bit, pin) in pins[..8].iter().enumerate() {
            let (actual, inverted) = gpio.matrix_input_route(140 + bit)?;
            if actual != *pin { return None; }
            if inverted { invert |= 1 << bit; }
        }
        let (w, h, mut frame) = self.camera_frame()?;
        if invert != 0 {
            if let Some(capture) = &mut self.periph.i2s[0].camera {
                frame = capture.inverted_frame(frame, invert);
            } else {
                for byte in std::sync::Arc::make_mut(&mut frame) { *byte ^= invert; }
            }
        }
        Some((w, h, frame))
    }
    pub fn attach_board_devices(&mut self) {
        self.irq_dirty = true;
        if self.board.uses_gpio_waveform() {
            self.board.gpio_waveform_at(self.cycles, self.periph.gpio.routes());
        }
        self.pins_active = self.board.uses_gpio_edges() || self.board.uses_uart_pins();
        for (pin, level) in self.board.input_levels() { self.periph.gpio.set_input(pin, level); }
        if self.pins_active { self.deliver_board_inputs(); }
        for (bus, address, device) in self.board.i2c_devices() {
            if let Some(i2c) = self.periph.i2c.get_mut(bus as usize) {
                i2c.attach(address, device);
            }
        }
    }
    pub(crate) fn dma_read_word(&mut self, addr: u32) -> Option<u32> {
        let range = dma_range(addr, 4)?;
        Some(u32::from_le_bytes(self.dram.get(range)?.try_into().ok()?))
    }
    pub(crate) fn dma_write_word(&mut self, addr: u32, value: u32) -> bool {
        if dma_range(addr, 4).is_none() { return false; }
        value.to_le_bytes().into_iter().enumerate().all(|(i, byte)| self.dma_write_byte(addr + i as u32, byte))
    }
    pub(crate) fn dma_write_byte(&mut self, addr: u32, value: u8) -> bool {
        let Some(range) = dma_range(addr, 1) else { return false; };
        let Some(dst) = self.dram.get_mut(range.start) else { return false; };
        *dst = value;
        self.written(addr, 1);
        true
    }
    fn dma_desc_word(&mut self, addr: u32) -> Result<u32, Fault> { self.dma_read_word(addr).ok_or(Fault::Unmapped) }
    /// Classic DMA descriptors come from memory, never MMIO, through the shared GDMA walker.
    pub(crate) fn read_dma_descriptor(&mut self, addr: u32) -> Result<(u32, esp_periph::DmaDesc), DmaDescriptorFault> {
        DescriptorWalk::new(1).read_with(addr, |a| self.dma_desc_word(a))
    }
    /// Walk a finite descriptor transaction, rejecting cycles and bounding visits. The visitor
    /// returns false once it has consumed the requested payload.
    fn walk_chain(
        &mut self,
        mut desc: u32,
        mut visit: impl FnMut(&mut Self, u32, esp_periph::DmaDesc) -> Result<bool, DmaDescriptorFault>,
    ) -> Result<(), DmaDescriptorFault> {
        let mut seen = std::collections::HashSet::new();
        let mut walk = DescriptorWalk::new(1024);
        while desc != 0 {
            if !seen.insert(desc) { return Err(DmaDescriptorFault::Cycle { descriptor: desc }); }
            let (control, d) = walk.read_with(desc, |a| self.dma_desc_word(a))?;
            let (next, eof) = (d.next, d.eof);
            if !visit(self, control, d)? || eof { break; }
            desc = next;
        }
        Ok(())
    }
    fn dma_read_chain(&mut self, desc: u32, wanted: usize) -> Option<(Vec<u8>, u32)> {
        if wanted == 0 { return Some((Vec::new(), 0)); }
        let mut data = Vec::with_capacity(wanted);
        let mut last = 0;
        self.walk_chain(desc, |bus, _, d| {
            if !d.owner_dma { return Err(DmaDescriptorFault::NotOwned { descriptor: d.addr }); }
            let count = (d.length as usize).min(wanted - data.len());
            if count != 0 {
                let range = dma_range(d.buf, count).ok_or(DmaDescriptorFault::BufferRead {
                    descriptor: d.addr, address: d.buf, fault: Fault::Prohibited,
                })?;
                data.extend_from_slice(&bus.dram[range]);
            }
            last = d.addr;
            Ok(data.len() < wanted)
        }).ok()?;
        (data.len() == wanted).then_some((data, last))
    }
    fn dma_write_chain(&mut self, desc: u32, data: &[u8]) -> Option<u32> {
        if data.is_empty() { return Some(0); }
        let mut pos = 0;
        let mut last = 0;
        self.walk_chain(desc, |bus, control, d| {
            let fault = DmaDescriptorFault::Writeback { descriptor: d.addr, fault: Fault::Prohibited };
            if !d.owner_dma { return Err(DmaDescriptorFault::NotOwned { descriptor: d.addr }); }
            let count = (d.size as usize).min(data.len() - pos);
            if count != 0 {
                let range = dma_range(d.buf, count).ok_or(fault)?;
                bus.dram[range].copy_from_slice(&data[pos..pos + count]);
                for i in 0..count { bus.written(d.buf + i as u32, 1); }
            }
            let updated = (control & !(0xfff << 12) & !(1 << 31)) | (count as u32) << 12;
            if !bus.dma_write_word(d.addr, updated) { return Err(fault); }
            pos += count;
            last = d.addr;
            Ok(pos < data.len())
        }).ok()?;
        (pos == data.len()).then_some(last)
    }
    fn run_gp_spi(&mut self) {
        for index in 0..2 {
            let Some((dma_len, rx_len, dma_rx)) = self.periph.spi[index]
                .pending()
                .map(|t| (t.dma_tx_len, t.rx_len, t.dma_rx))
            else {
                continue;
            };
            let (out_link, in_link) = self.periph.spi[index].dma_links();
            let mut out_eof = 0;
            if dma_len != 0 {
                match self.dma_read_chain(out_link, dma_len) {
                    Some((bytes, last)) => {
                        out_eof = last;
                        self.periph.spi[index].supply_dma_tx(&bytes);
                    }
                    None => self.periph.spi[index].dma_fault(true),
                }
            }
            let Some(transfer) = self.periph.spi[index].take_transfer() else {
                continue;
            };
            let host = index + 2;
            let (clock_signal, mosi_signal, miso_signal, cs_signals) = crate::spi::signals(host);
            let (clock_idle, mosi_idle) = self.periph.spi[index].idle_levels(&transfer);
            self.periph
                .gpio
                .set_output_signal(clock_signal, clock_idle, true);
            self.periph
                .gpio
                .set_output_signal(mosi_signal, mosi_idle, true);
            if let Some((cs, polarity)) = transfer.cs {
                self.periph
                    .gpio
                    .set_output_signal(cs_signals[cs], polarity, true);
            }
            self.flush_gpio();
            let rx = if self.board.uses_spi_pins() {
            let gpio = &self.periph.gpio;
            let pins = esp_soc::board::SpiPins {
                sclk: gpio.output_pins(clock_signal), mosi: gpio.output_pins(mosi_signal),
                miso: gpio.input_pin(miso_signal),
                cs: gpio.low_outputs(),
            };
            self.board.spi_transfer_pins(host as u8, pins, &transfer.tx, transfer.rx_len)
            } else { self.board.spi_transfer(host as u8, &transfer.tx, transfer.rx_len) };
            let mut in_eof = 0;
            if dma_rx && rx_len != 0 {
                match self.dma_write_chain(in_link, &rx[..rx.len().min(rx_len)]) {
                    Some(last) => in_eof = last,
                    None => self.periph.spi[index].dma_fault(false),
                }
            }
            let cs = transfer.cs;
            let keep_cs = transfer.keep_cs;
            self.periph.spi[index].finish(transfer, &rx, out_eof, in_eof);
            if !keep_cs {
                if let Some((cs, polarity)) = cs {
                    self.periph
                        .gpio
                        .set_output_signal(cs_signals[cs], !polarity, true);
                }
            }
        }
    }
    pub fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let target = self
            .flash
            .get_mut(offset..)
            .and_then(|d| d.get_mut(..data.len()))
            .ok_or("flash image too large")?;
        target.copy_from_slice(data);
        self.flash_written(offset, data.len());
        Ok(())
    }
    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        for (i, &v) in data.iter().enumerate() {
            let a = addr.wrapping_add(i as u32);
            if (crate::psram::LOW..crate::psram::HIGH).contains(&a) {
                self.psram_store(a, &[v]).map_err(|f| format!("load: address {a:#010x}: {f:?}"))?;
                continue;
            }
            match self.memory(a) {
                Some((b, o, _)) if o < b.len() => b[o] = v,
                _ => return Err(format!("load: address {a:#010x} not mapped")),
            }
        }
        self.written(addr, data.len());
        Ok(())
    }
}

impl Bus for SocBus {
    fn note_code_page(&mut self, _vidx: u32) {}
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.load(a).map(u8::from_le_bytes) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.load(a).map(u16::from_le_bytes) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.load(a).map(u32::from_le_bytes) }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.store(a, &v.to_le_bytes()) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.store(a, &v.to_le_bytes()) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.store(a, &v.to_le_bytes()) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        match self.memory(pc) {
            Some((b, o, _)) if o < b.len() => {
                let mut out = [0; 4];
                for (i, v) in out.iter_mut().enumerate() {
                    if let Some(x) = b.get(o + i) {
                        *v = *x;
                    }
                }
                Ok(out)
            }
            _ => self.fetch_miss(pc),
        }
    }
    fn page_versions(&self) -> &[u32] {
        &self.page_ver
    }
    fn code_page(&mut self, pc: u32) -> u32 {
        Self::version_page(pc) as u32
    }
    fn note_pc(&mut self, pc: u32) {
        self.periph.misc.cur_pc = pc;
    }
    fn block_break(&self) -> bool {
        self.irq_dirty
    }
    fn tick(&mut self, cycles: u32) -> u32 {
        let half = u64::from(cycles) * self.periph.cpu_scale_half() + self.clock_half;
        self.clock_half = half & 1;
        let elapsed = half >> 1;
        self.cycles += elapsed;
        if self.pins_active {
            self.deliver_board_inputs();
            for input in self.board.uart_rx(self.cycles) { self.periph.uart_pin_input(&input); self.irq_dirty = true; }
        }
        let irq_changed = self.periph.tick(elapsed);
        self.irq_dirty |= irq_changed;
        self.i2s_step(elapsed);
        self.flush_rmt();
        self.flush_gpio();

        if self.periph.wifi.clock_enabled { self.wifi_step(); }
        u32::from(self.irq_dirty)
    }
}

#[cfg(test)]
#[path = "irq_event_tests.rs"]
mod irq_event_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irq_events_idle_polling_is_clean_but_writes_and_inputs_are_dirty() {
        let mut b = SocBus::new(0, [0; 6]);
        assert_eq!(b.tick(1), 1, "initial presentation");
        b.irq_dirty = false;
        for _ in 0..100 {
            assert_eq!(b.read32(0x3ff4_403c), Ok(0));
            assert_eq!(b.tick(64), 0, "idle/polling must not present IRQs");
            assert!(!b.irq_dirty);
        }
        b.write32(0x6000_000c, 1 << 8).unwrap();
        assert!(b.irq_dirty, "AHB write invalidates source/matrix state");
        b.irq_dirty = false;
        esp_soc::SocBus::serial_input(&mut b, b"x");
        assert!(b.irq_dirty);
        b.irq_dirty = false;
        esp_soc::SocBus::gpio_set_input(&mut b, 4, true);
        assert!(b.irq_dirty);
    }

    #[test]
    fn irq_events_timer_alarm_and_clear_keep_both_core_lines_live() {
        let mut b = SocBus::new(0, [0; 6]);
        for base in [0x104, 0x218] {
            b.write32(0x3ff0_0000 + base + 4 * 14, 9).unwrap();
        }
        b.write32(0x3ff5_f000, (1 << 31) | (1 << 30) | (1 << 11) | (1 << 10) | (2 << 13)).unwrap();
        b.write32(0x3ff5_f010, 1000).unwrap();
        b.write32(0x3ff5_f098, 1).unwrap();
        b.irq_dirty = false;
        assert_eq!(b.tick(1), 0, "timer before alarm");
        assert_ne!(b.tick(6000), 0, "clocked alarm must invalidate presentation");
        let mut lines = [0; 2];
        b.periph.all_cpu_lines(&mut lines);
        assert_eq!(lines, [1 << 9; 2]);
        b.irq_dirty = false;
        assert_eq!(b.tick(1), 0, "unchanged asserted source is clean");
        b.write32(0x3ff5_f0a4, 1).unwrap();
        assert!(b.irq_dirty, "clearing source must present its falling edge");
        b.periph.all_cpu_lines(&mut lines);
        assert_eq!(lines, [0; 2]);
    }

    #[test]
    fn irq_events_uart_fifo_reads_stay_sticky_until_clear_on_all_aliases() {
        for (base, alias, port) in [(0x3ff4_0000, 0x6000_0000, 0), (0x3ff5_0000, 0x6001_0000, 1), (0x3ff6_e000, 0x6002_e000, 2)] {
            for addr in [base, alias] {
                let mut b = SocBus::new(0, [0; 6]);
                b.write32(base + 0x24, 1).unwrap();
                b.write32(base + 0xc, 1).unwrap();
                esp_soc::SocBus::uart_input(&mut b, port, b"x");
                assert_ne!(b.periph.uart[port].irq_sources(), 0);
                b.irq_dirty = false;
                assert_eq!(b.read8(addr), Ok(b'x'));
                assert_ne!(b.periph.uart[port].irq_sources(), 0, "FIFO interrupt is sticky");
                assert!(!b.irq_dirty, "FIFO read does not change source level");
                b.write32(base + 0x10, 1).unwrap();
                assert_eq!(b.periph.uart[port].irq_sources(), 0);
                assert!(b.irq_dirty, "clear write must invalidate presentation");
            }
        }
    }

    struct I2cBoard;
    impl esp_soc::BoardModel for I2cBoard {
        fn name(&self) -> &'static str { "i2c-test" }
        fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn esp_periph::i2c::I2cDevice>)> {
            vec![(1, 0x6b, Box::new(esp_periph::i2c::Reg8Device::new("qmi8658", &[(0, 5)])))]
        }
    }

    const SPI2: u32 = 0x3ff6_4000;

    #[test]
    fn classic_idle_gates_skip_adapter_calls() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        for _ in 0..100 { b.tick(64); }
        assert_eq!(b.periph.ledc.tick_calls, 0);
        assert_eq!(b.periph.rmt.tick_calls, 0);
        assert_eq!(b.periph.wifi.step_calls, 0);
        // Clock-on but no work must not enter the LEDC/RMT engines either.
        b.write32(0x3ff0_00c0, (1 << 11) | (1 << 9)).unwrap();
        for _ in 0..100 { b.tick(64); }
        assert_eq!(b.periph.ledc.tick_calls, 0);
        assert_eq!(b.periph.rmt.tick_calls, 0);
    }

    #[test]
    fn classic_wifi_clock_and_reset_skip_step() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.tick(64);
        assert_eq!(b.periph.wifi.step_calls, 0);
        b.write32(0x3ff0_00cc, 0x406).unwrap();
        b.tick(64);
        assert_eq!(b.periph.wifi.step_calls, 1);
        b.write32(0x3ff0_00cc, 0x402).unwrap();
        b.tick(64);
        assert_eq!(b.periph.wifi.step_calls, 1);
        b.write32(0x3ff0_00cc, 0x406).unwrap();
        b.write32(0x3ff0_00d0, 4).unwrap();
        b.tick(64);
        assert_eq!(b.periph.wifi.step_calls, 0); // MAC reset clears counters too.
        b.write32(0x3ff0_00d0, 0).unwrap();
        b.tick(64);
        assert_eq!(b.periph.wifi.step_calls, 1);
    }

    #[test]
    fn pro_mmu_maps_classic_drom_and_irom() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.flash[0x12345] = 0x5a;
        b.mmu[0][0] = 1;
        b.mmu[0][77] = 1;
        assert_eq!(b.read8(DBUS_LOW + 0x2345), Ok(0x5a));
        assert_eq!(b.read8(0x4000_0000 + 0x2345), Ok(b.irom[0x2345])); // mask ROM wins below app IROM
        assert_eq!(b.read8(IBUS_LOW + 0x2345), Ok(0x5a));
    }

    #[test]
    fn analog_config_register_is_read_write() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(0x6000_e044, 0x1234).unwrap();
        assert_eq!(b.read32(0x6000_e044), Ok(0x1234));
        assert_eq!(b.read32(0x3ff4_e044), Ok(0x1234));
        b.write32(0x6003_3d24, 2).unwrap();
        assert_eq!(b.read32(0x3ff7_3d24), Ok(3));
        b.write32(0x6000_607c, 3).unwrap();
        assert_eq!(b.read32(0x3ff4_607c), Ok(0x8000_0003));
    }

    #[test]
    fn rng_ahb_and_apb_aliases_share_one_generator() {
        let mut b = SocBus::new(0, [0; 6]);
        let mut rng = Rng::new();
        b.cycles = 23;
        rng.now = 23;
        for addr in [0x6003_5144, 0x3ff7_5144, 0x6003_5144] {
            assert_eq!(b.read32(addr), Ok(rng.read(0)));
            b.write32(addr, 0xdeadbeef).unwrap();
        }
    }

    #[test]
    fn uart_ahb_alias_reaches_fifo() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(0x6000_0000, b'X' as u32).unwrap();
        assert_eq!(b.periph.uart[0].tx_out, b"X");
    }


    #[test]
    fn reboot_preserves_rtc_reset_hint_and_publishes_cause() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        let hint = 0x8006_0006;
        b.periph.rtc.0.ram.write(0xc8, hint);
        b.periph.rtc.0.reset_cause = 12;
        assert_eq!(esp_soc::SocBus::reboot(&mut b, [0; 6]), 12);
        assert_eq!(b.read32(0x3ff4_80b8), Ok(hint));
        assert_eq!(b.read32(0x3ff4_8034), Ok(12 | (12 << 6)));
    }
    #[test]
    fn reboot_reapplies_debug_areas() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        let mut flags = esp_soc::DebugFlags::default();
        flags.add("mmio");
        esp_soc::SocBus::set_debug(&mut b, &flags);
        assert!(b.periph.misc.log_all);
        esp_soc::SocBus::reboot(&mut b, [0; 6]);
        assert!(b.periph.misc.log_all, "--debug mmio still logs after esp_restart()");
    }
    #[test]
    fn stores_invalidate_only_the_written_code_pages_and_rtc_alias() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        let rom = b.code_page(IROM_MASK_LOW) as usize;
        let code = b.code_page(IRAM_LOW + 0x100) as usize;
        b.write32(DRAM_LOW + 0x100, 42).unwrap();
        assert_eq!(b.page_versions()[rom], 0);
        assert_eq!(b.page_versions()[code], 0);
        b.write16(IRAM_LOW + 0x1ff, 42).unwrap();
        assert_eq!(b.page_versions()[code], 1);
        assert_eq!(b.page_versions()[code + 1], 1);
        b.write8(IRAM_LOW + 0x200, 42).unwrap();
        assert_eq!(b.page_versions()[code], 2, "straddling instruction on previous page");
        let rtc = b.code_page(RTC_FAST_I) as usize;
        b.write32(RTC_FAST_D, 42).unwrap();
        assert_eq!(b.code_page(RTC_FAST_D) as usize, rtc);
        assert_eq!(b.page_versions()[rtc], 1);
    }
    #[test]
    fn flash_remapping_and_programming_invalidate_aliases_without_touching_rom() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        let rom = b.code_page(IROM_MASK_LOW) as usize;
        b.write32(MMU_PRO + 77 * 4, 1).unwrap();
        b.write32(MMU_PRO + 78 * 4, 1).unwrap();
        let code = b.code_page(IBUS_LOW + 0x100) as usize;
        let alias = b.code_page(IBUS_LOW + PAGE as u32 + 0x100) as usize;
        let before = b.page_versions()[code];
        b.write_flash(PAGE + 0x100, &[42]).unwrap();
        assert!(b.page_versions()[code] > before);
        assert_eq!(b.page_versions()[code], b.page_versions()[alias]);
        let before = b.page_versions()[code];
        b.write32(MMU_PRO + 77 * 4, 2).unwrap();
        assert!(b.page_versions()[code] > before);
        assert_eq!(b.page_versions()[rom], 0);
    }

    #[test]
    fn i2c_ahb_alias_and_board_devices_reach_both_controllers() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.board = Box::new(I2cBoard);
        b.attach_board_devices();
        assert!(b.periph.i2c[1].has_device(0x6b));
        b.irq_dirty = false;
        b.write32(0x6001_301c, 0xd6).unwrap();
        assert!(b.irq_dirty, "AHB FIFO writes use the generic IRQ update");
        assert_eq!(b.read32(0x3ff5_3008).unwrap() >> 18 & 0x3f, 1);

        <SocBus as esp_soc::SocBus>::reboot(&mut b, [0; 6]);
        assert!(b.periph.i2c[1].has_device(0x6b));
    }


    #[test]
    fn classic_spi_dma_fault_does_not_publish_success_or_send_a_transfer() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(SPI2 + 0x104, (1 << 29) | 0xb0100).unwrap();
        b.write32(SPI2 + 0x1c, 1 << 27).unwrap();
        b.write32(SPI2 + 0x28, 23).unwrap();
        b.write32(SPI2, 1 << 18).unwrap();
        assert_eq!(b.read32(SPI2 + 0x114), Ok(1 << 1));
        assert_eq!(b.periph.spi[0].transfers, 0);
        assert_eq!(b.read32(SPI2).unwrap() & (1 << 18), 0);
    }

    #[test]
    fn classic_spi_dma_reach_descriptors_and_payloads() {
        for address in [DRAM_LOW, DRAM_HIGH - 12, ROM_DRAM_LOW, DRAM_LOW - 12,
            IRAM_LOW, IROM_MASK_LOW, DROM_MASK_LOW, RTC_FAST_D, RTC_SLOW] {
            let allowed = address == DRAM_LOW || address == DRAM_HIGH - 12;
            let mut b = SocBus::new(0, [0; 6]);
            let descriptor = [0x01, 0x10, 0x00, 0xc0, 0x00, 0x10, 0xfb, 0x3f, 0, 0, 0, 0];
            b.load_bytes(address, &descriptor).unwrap();
            b.write8(0x3ffb_1000, 42).unwrap();
            assert_eq!(b.read_dma_descriptor(address).is_ok(), allowed, "descriptor {address:#x}");
            assert_eq!(b.dma_read_chain(address, 1).is_some(), allowed, "TX descriptor {address:#x}");
            assert_eq!(b.dma_write_chain(address, &[42]).is_some(), allowed, "RX descriptor {address:#x}");
        }
        for address in [DRAM_LOW, DRAM_HIGH - 1, ROM_DRAM_LOW, DRAM_LOW - 1,
            IRAM_LOW, IROM_MASK_LOW, DROM_MASK_LOW, RTC_FAST_D, RTC_SLOW] {
            let allowed = address == DRAM_LOW || address == DRAM_HIGH - 1;
            let mut b = SocBus::new(0, [0; 6]);
            let desc = 0x3ffb_0000;
            b.write32(desc, 0xc000_1001).unwrap();
            b.write32(desc + 4, address).unwrap();
            b.write32(desc + 8, 0).unwrap();
            b.load_bytes(address, &[17]).unwrap();
            assert_eq!(b.dma_read_chain(desc, 1).is_some(), allowed, "TX buffer {address:#x}");
            assert_eq!(b.dma_write_chain(desc, &[42]).is_some(), allowed, "RX buffer {address:#x}");
            assert_eq!(b.read8(address).unwrap(), if allowed {42} else {17});
        }
        let mut b = SocBus::new(0, [0; 6]);
        b.write8(DRAM_HIGH - 1, 17).unwrap();
        assert!(!b.dma_write_word(DRAM_HIGH - 1, 42), "reject entire crossing word");
        assert_eq!(b.read8(DRAM_HIGH - 1), Ok(17));
        assert!(b.dma_read_word(u32::MAX).is_none());
    }

    #[test]
    fn classic_dma_reach_rejects_sram1_iram_alias() {
        // ESP-IDF v5.5.4 soc/esp32/include/soc/soc.h:196-205:
        // SRAM1's instruction window is CPU-accessible but outside SOC_DMA_LOW..SOC_DMA_HIGH.
        let alias = 0x400a_0000;
        let mut b = SocBus::new(0, [0; 6]);
        b.load_bytes(alias, &[0x01, 0x10, 0x00, 0xc0, 0x00, 0x10, 0xfb, 0x3f, 0, 0, 0, 0]).unwrap();
        assert_eq!(b.read32(alias), Ok(0xc000_1001));
        assert!(dma_range(alias, 12).is_none());
        assert!(b.read_dma_descriptor(alias).is_err());
        assert!(b.dma_read_chain(alias, 1).is_none());
        assert!(b.dma_write_chain(alias, &[42]).is_none());
        assert!(b.dma_read_word(alias).is_none());
        assert!(!b.dma_write_word(alias, 42));
        let desc = DRAM_LOW;
        b.write32(desc, 0xc000_1001).unwrap();
        b.write32(desc + 4, alias).unwrap();
        b.write32(desc + 8, 0).unwrap();
        let dram = b.dram.clone();
        let iram = b.iram.clone();
        assert!(b.dma_read_chain(desc, 1).is_none());
        assert!(b.dma_write_chain(desc, &[42]).is_none());
        assert_eq!(b.dram, dram, "rejected DMA must not write back the descriptor");
        assert_eq!(b.iram, iram, "CPU-accessible IRAM must remain untouched");
    }

    #[test]
    fn classic_spi_dma_empty_nodes_do_not_access_a_buffer() {
        let mut b = SocBus::new(0, [0; 6]);
        let first = 0x3ffb_0000;
        let second = first + 16;
        b.write32(first, 1 << 31).unwrap();
        b.write32(first + 4, 0).unwrap();
        b.write32(first + 8, second).unwrap();
        b.write32(second, 0xc000_1001).unwrap();
        b.write32(second + 4, 0x3ffb_1000).unwrap();
        b.write32(second + 8, 0).unwrap();
        b.write8(0x3ffb_1000, 17).unwrap();
        assert_eq!(b.dma_read_chain(first, 1), Some((vec![17], second)));
        assert_eq!(b.dma_write_chain(first, &[42]), Some(second));
        assert_eq!(b.read8(0x3ffb_1000), Ok(42));
    }

    #[test]
    fn classic_dma_bounds_empty_rings_and_rejects_mmio_buffers() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        let first = 0x3ffb_0100;
        let second = first + 16;
        for (desc, next) in [(first, second), (second, first)] {
            b.write32(desc, 1 << 31).unwrap();
            b.write32(desc + 4, 0x3ffb_0200).unwrap();
            b.write32(desc + 8, next).unwrap();
        }
        assert!(b.dma_read_chain(first, 1).is_none());
        assert!(b.dma_write_chain(first, &[42]).is_none());
        b.write32(first, 0xc000_1001).unwrap();
        b.write32(first + 4, 0x3ff4_0000).unwrap();
        assert!(b.dma_read_chain(first, 1).is_none());
        assert!(b.dma_write_chain(first, &[42]).is_none());
        assert!(b.periph.uart[0].tx_out.is_empty());
    }

    #[test]
    fn classic_raw_adc_reports_only_completed_conversions() {
        use esp_soc::SocBus as _;
        let mut b = SocBus::new(0, [0; 6]);
        assert!(!b.adc_set_raw(1, 42));
        assert!(b.adc_observation(1).is_none());
        assert!(b.adc_set_raw(34, 1234));
        assert!(!b.adc_set_raw(34, 4096));
        b.write32(0x3ff4_8800, 1 << 28).unwrap();
        assert_eq!(b.adc_observation(34).unwrap().generation, 0);
        let command = (1 << 31) | (1 << 25) | (1 << 18) | (1 << 17);
        b.write32(0x3ff4_8854, command).unwrap();
        assert_eq!(b.read32(0x3ff4_8854).unwrap() & 0xffff, 1234);
        assert_eq!(b.adc_observation(34).unwrap(), esp_periph::AdcObservation { generation: 1, raw: 1234 });
        b.write32(0x3ff4_8854, command).unwrap();
        assert_eq!(b.adc_observation(34).unwrap().generation, 1);
    }

    #[test]
    fn classic_adc_samples_host_waveform_at_bus_time() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(0x3ff4_8070, 1 << 27).unwrap();
        b.write32(0x3ff0_003c, 2).unwrap(); // PLL 240 MHz: CPU and bus reference agree
        esp_soc::SocBus::analog_set(&mut b, 34, esp_periph::AnalogSource::Wave {
            samples: std::sync::Arc::new(vec![1.650, 0.800]),
            rate_hz: 10.0,
            start_cycles: 24_000_000,
        });
        b.write32(0x3ff4_8800, 1 << 28).unwrap();
        b.write32(0x3ff4_8834, 3 << 12).unwrap();
        for (cycles, expected) in [(0, 1872), (47_999_999, 1872), (48_000_000, 817), (240_000_000, 817)] {
            b.tick((cycles - b.cycles) as u32);
            let start = (1 << 31) | (1 << 25) | (1 << 18);
            b.write32(0x3ff4_8854, start).unwrap();
            b.write32(0x3ff4_8854, start | (1 << 17)).unwrap();
            assert_eq!(b.read32(0x3ff4_8854).unwrap() & 0xffff, expected);
        }
        esp_soc::SocBus::reboot(&mut b, [0; 6]);
        b.write32(0x3ff4_8800, 1 << 28).unwrap();
        b.write32(0x3ff4_8834, 3 << 12).unwrap();
        b.write32(0x3ff4_8854, (1 << 31) | (1 << 25) | (1 << 18) | (1 << 17)).unwrap();
        assert_eq!(b.read32(0x3ff4_8854).unwrap() & 0xffff, 817);
    }

    #[test]
    fn reboot_keeps_rtc_domain_analog_registers_and_host_inputs() {
        use esp_soc::SocBus as _;
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.analog_set(34, esp_periph::AnalogSource::Const(1.650));
        b.set_touch_input(4, true);
        b.write32(0x3ff4_8484, (128 << 19) | (1 << 18) | (1 << 17) | (1 << 10)).unwrap();
        b.write32(0x3ff4_8494, 1 << 19).unwrap(); // GPIO4 RTC mux
        b.write32(0x3ff4_8018, 1 << 23).unwrap(); // RTC_CNTL_TOUCH_SLP_TIMER_EN
        assert!(b.report().contains("[dac] GPIO25: 1656 mV"));
        b.reboot([0; 6]);
        // SENS/RTC_IO and the STATE0 copy survive like RTC_CNTL's register RAM.
        assert_eq!(b.periph.adc.dac_outputs(), [(25, 1656)]);
        assert!(b.periph.adc.touch_timer);
        assert_ne!(b.read32(0x3ff4_8018).unwrap() & (1 << 23), 0);
        b.write32(0x3ff4_9048, (2 << 12) | (1 << 9)).unwrap();
        b.gpio_set_input(4, true);
        assert_eq!(b.read32(0x3ff4_403c).unwrap() & (1 << 4), 0, "GPIO4 stays on the RTC mux");

        b.write32(0x3ff4_8480, 1 << 29).unwrap();
        b.write32(0x3ff4_8800, 1 << 28).unwrap();
        b.write32(0x3ff4_8834, 3 << 12).unwrap();
        b.write32(0x3ff4_8854, (1 << 31) | (1 << 25) | (1 << 18) | (1 << 17)).unwrap();
        assert_eq!(b.read32(0x3ff4_8854).unwrap() & 0xffff, 1872);
        b.write32(0x3ff4_8494, (1 << 19) | (4 << 23)).unwrap();
        b.write32(0x3ff4_8884, (1 << 11) | (1 << 12) | (1 << 13)).unwrap();
        assert_eq!(b.read32(0x3ff4_8870).unwrap() >> 16, 300);
    }

    #[test]
    fn classic_spi_dma_walks_native_descriptors_and_stores_loopback_rx() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.board = crate::spi::make_board("loopback").unwrap();
        let (tx_desc, rx_desc, tx_buf, rx_buf) =
            (0x3ffb_0100, 0x3ffb_0140, 0x3ffb_0200, 0x3ffb_0240);
        let descriptor = 0xc000_3003; // owner, eof, length=3, size=3
        for (desc, buf) in [(tx_desc, tx_buf), (rx_desc, rx_buf)] {
            b.write32(desc, descriptor).unwrap();
            b.write32(desc + 4, buf).unwrap();
            b.write32(desc + 8, 0).unwrap();
        }
        b.write32(tx_buf, 0x0033_2211).unwrap();
        b.write32(SPI2 + 0x104, (1 << 29) | (tx_desc & 0xfffff))
            .unwrap();
        b.write32(SPI2 + 0x108, (1 << 29) | (rx_desc & 0xfffff))
            .unwrap();
        b.write32(SPI2 + 0x1c, (1 << 28) | (1 << 27) | 1).unwrap();
        b.write32(SPI2 + 0x28, 23).unwrap();
        b.write32(SPI2 + 0x2c, 23).unwrap();
        b.write32(SPI2, 1 << 18).unwrap();

        assert_eq!(b.read32(rx_buf), Ok(0x0033_2211));
        assert_eq!((b.read32(rx_desc).unwrap() >> 12) & 0xfff, 3);
        assert_eq!(b.read32(rx_desc).unwrap() >> 31, 0);
        assert_eq!(b.read32(SPI2).unwrap() & (1 << 18), 0);
        assert_eq!(b.read32(SPI2 + 0x114).unwrap() & 0x1e8, 0x1e8);
        assert_eq!(b.periph.spi[0].transfers, 1);
    }

    #[test]
    fn classic_rmt_routes_inverted_gpio_frames_to_shared_board_and_report() {
        struct LedBoard(esp_soc::devices::Ws2812Chain);
        impl esp_soc::BoardModel for LedBoard {
            fn name(&self) -> &'static str { "rmt-test" }
            fn rmt_frame(&mut self, pin: u8, bits: &[bool]) {
                assert_eq!(pin, 4);
                self.0.from_bits(bits);
            }
            fn leds(&self) -> Option<(&[[u8; 3]], u64)> { Some((&self.0.leds, self.0.updates)) }
        }
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.board = Box::new(LedBoard(esp_soc::devices::Ws2812Chain::new(1)));
        b.attach_board_devices();
        b.write32(0x3ff4_9048, 2 << 12).unwrap();
        b.write32(0x3ff4_4540, crate::rmt::SIGNAL0 as u32 | (1 << 9)).unwrap();
        b.periph.gpio.set_output_signal(crate::rmt::SIGNAL0, false, true);
        let bits = [0u8, 255, 64].into_iter().flat_map(|byte| (0..8).rev().map(move |shift| byte & (1 << shift) == 0)).collect();
        b.periph.rmt.done.push((0, bits));
        b.periph.rmt.tx_count = 1;
        b.tick(0);
        assert_eq!(b.board.leds(), Some((&[[255, 0, 64]][..], 1)));
        assert!(b.periph.rmt.done.is_empty());
    }

    #[test]
    fn classic_rmt_clock_gate_deadline_reset_and_dport_source_47() {
        let mut b = SocBus::new(4 << 20, [0; 6]);
        b.write32(0x3ff4_8070, 1 << 27).unwrap();
        b.write32(0x3ff0_003c, 2).unwrap(); // PLL 240 MHz: three CPU clocks/APB tick
        let rmt = 0x3ff5_6000;
        b.write32(0x3ff4_9048, (2 << 12) | (1 << 9)).unwrap();
        b.write32(0x3ff4_4540, 87).unwrap();
        b.write32(0x3ff0_0104 + 47 * 4, 5).unwrap();
        b.write32(0x3ff0_0218 + 47 * 4, 6).unwrap();
        b.write32(0x3ff0_0104 + 46 * 4, 7).unwrap();
        b.write32(0x3ff0_00c0, 1 << 9).unwrap();
        b.write32(rmt + 0x20, (1 << 24) | 1).unwrap();
        b.write32(rmt + 0x800, 4 | (1 << 15) | (6 << 16)).unwrap();
        b.write32(rmt + 0xa8, 1).unwrap();
        b.write32(rmt + 0x24, (1 << 17) | (1 << 18) | (1 << 19) | 1).unwrap();
        assert_eq!(esp_soc::SocBus::next_deadline(&b), Some(9));
        b.tick(9);
        b.write32(0x3ff0_00c0, 0).unwrap();
        assert_eq!(esp_soc::SocBus::next_deadline(&b), None);
        b.tick(300);
        assert_eq!(b.read32(rmt + 0xa0), Ok(0));
        b.write32(0x3ff0_00c0, 1 << 9).unwrap();
        b.tick(3);
        assert_eq!(esp_soc::SocBus::next_deadline(&b), Some(15));
        b.tick(18);
        assert_ne!(b.read32(0x3ff4_403c).unwrap() & (1 << 4), 0, "idle level is high");
        assert_eq!(b.periph.cpu_lines(0), 1 << 5);
        assert_eq!(b.periph.cpu_lines(1), 1 << 6);
        assert_eq!(b.periph.source_status(0)[1] & ((1 << 14) | (1 << 15)), 1 << 15);
        b.write32(rmt + 0xac, 1).unwrap();
        assert_eq!(b.periph.cpu_lines(0), 0);

        b.periph.rtc.0.ram.write(0x44, 1 << 3);
        b.periph.rtc.0.ram.write(0x3c, 1 << 3);
        assert_eq!(b.periph.cpu_lines(0), 1 << 7, "RTC remains source 46");
        b.periph.rtc.0.ram.write(0x44, 0);
        b.write32(0x3ff0_00c4, 1 << 9).unwrap();
        assert_eq!(b.read32(0x3ff4_403c).unwrap() & (1 << 4), 0, "reset returns the signal low");
        assert_eq!(b.read32(rmt + 0x800), Ok(0));
        assert_eq!(b.read32(rmt + 0xa8), Ok(0));
        assert!(!b.periph.rmt.clock_enabled);
        assert_eq!(b.periph.cpu_lines(0), 0);
        b.write32(0x3ff0_00c4, 0).unwrap();
        assert!(b.periph.rmt.clock_enabled);
    }
}
