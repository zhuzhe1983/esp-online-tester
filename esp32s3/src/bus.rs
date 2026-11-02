//! ESP32-S3 memory map: internal SRAM (512 KiB, IRAM/DRAM aliases), mask ROM, RTC
//! memories, external flash + PSRAM through the 512-entry cache MMU, peripherals.
use crate::periph::{Peripherals, PERIPH_BASE, PERIPH_END};
use crate::board::Board;
mod dma;
pub use dma::{DmaDescriptorFault, DmaDescriptorWord};
use dma::Spi2DmaCompletion;
#[cfg(test)]
use dma::SPI2_DMA_DESCRIPTOR_STEP_BUDGET;
use xtensa_lx7::bus::{Bus, Fault};

pub const SRAM_SIZE: usize = 512 * 1024;
pub const IRAM_LOW: u32 = 0x4037_0000;
pub const IRAM_HIGH: u32 = 0x403E_0000;
pub const DRAM_LOW: u32 = 0x3FC8_8000;
pub const DRAM_HIGH: u32 = 0x3FD0_0000;
pub const IROM_MASK_LOW: u32 = 0x4000_0000;
pub const IROM_MASK_HIGH: u32 = 0x4006_0000;
pub const DROM_MASK_LOW: u32 = 0x3FF0_0000;
pub const DROM_MASK_HIGH: u32 = 0x3FF2_0000;
pub const RTC_FAST_LOW: u32 = 0x600F_E000;
pub const RTC_FAST_HIGH: u32 = 0x6010_0000;
pub const RTC_SLOW_LOW: u32 = 0x5000_0000;
pub const RTC_SLOW_HIGH: u32 = 0x5000_2000;
pub const DBUS_LOW: u32 = 0x3C00_0000;
pub const DBUS_HIGH: u32 = 0x3E00_0000;
pub const IBUS_LOW: u32 = 0x4200_0000;
pub const IBUS_HIGH: u32 = 0x4400_0000;
/// GPIO matrix output signal of RMT TX channel 0 (soc/gpio_sig_map.h); channel n is this + n.
pub const RMT_SIG_OUT0: u32 = 81;
pub const MMU_TABLE: u32 = 0x600C_5000;
pub const MMU_ENTRIES: usize = 512;
pub const MMU_INVALID: u32 = 1 << 14;
pub const MMU_SPIRAM: u32 = 1 << 15;
pub const PAGE: u32 = 0x1_0000;
const _: () = {
    // EX173: a generated access derives its alignment from `addr - entry.lo`, so every mapping
    // this bus can build must start at least 16-byte aligned (the widest access is a PIE vector).
    assert!(PAGE.is_multiple_of(16) && DRAM_LOW.is_multiple_of(16) && IRAM_LOW.is_multiple_of(16) && IROM_MASK_LOW.is_multiple_of(16));
    assert!(DROM_MASK_LOW.is_multiple_of(16) && RTC_FAST_LOW.is_multiple_of(16) && RTC_SLOW_LOW.is_multiple_of(16));
    assert!(DBUS_LOW.is_multiple_of(16) && IBUS_LOW.is_multiple_of(16));
};

pub struct SocBus {
    pub ble: esp_soc::ble::vhci::Ble,
    pub sram: Vec<u8>,
    pub irom: Vec<u8>,
    pub drom: Vec<u8>,
    pub rtc_fast: Vec<u8>,
    pub rtc_slow: Vec<u8>,
    pub flash: Vec<u8>,
    pub psram: Vec<u8>,
    pub mmu: [u32; MMU_ENTRIES],
    pub periph: Peripherals,
    pub board: Board,
    uart_pins: bool,
    pub cycles: u64,
    pub last_fault: Option<(u32, bool)>,
    pub spi2_dma_fault: Option<DmaDescriptorFault>,
    /// Experimental register-derived SPI2 wire timing; disabled for baseline runs.
    pub spi2_timing: bool,
    spi2_scheduled: Option<(u64, Spi2DmaCompletion)>,
    spi2_pins: Option<esp_soc::board::SpiPins>,
    /// SPI3's DMA descriptor fault and routes, as SPI2's above (no wire timing).
    pub spi3_dma_fault: Option<DmaDescriptorFault>,
    spi3_pins: Option<esp_soc::board::SpiPins>,
    /// set by any peripheral write: interrupt lines must be re-evaluated before the next instruction
    pub irq_dirty: bool,
    /// GPIO edges for observers, while one wants them: (cycle, pin, level)
    pub gpio_events: Option<Vec<(u64, u8, bool)>>,
    pub debug: esp_soc::DebugFlags,
    /// Software TLB: the last resolved mapping per 64 KiB page, so loads, stores and fetches skip
    /// the address-range walk and the flash MMU. Cleared whenever the MMU changes.
    tlb: Vec<TlbEntry>,
    /// One version counter per `VPAGE`-byte page of every buffer, bumped by each write there. The
    /// decode and block caches store the versions they were built under, so a stale decode can
    /// never run. 256 bytes rather than 4 KiB because IRAM and DRAM are one SRAM: on the S3 the
    /// app's `.dram0.data` begins in the same 4 KiB as the end of IRAM text, and every global
    /// write was invalidating `_xt_context_save`.
    page_ver: Vec<u32>,
    /// first `page_ver` index of each buffer, by `SRC_*`
    ver_base: [u32; 7],
    /// shell-s2: moves with every change to a version `stable_pages` covers (flash pages). Starts at a
    /// per-bus base `BUS_EPOCHS` hands out, so equal epochs also mean the same bus (review B1).
    flash_epoch: u64,
    /// EX110: one flag per 64 KiB block of the `page_ver` index space (256 pages, the span of one
    /// TLB entry): some decode cache, block or region has recorded the version of a page in it, or
    /// of a page next to it. Never cleared while the buffers stand. `TlbEntry.code` copies it, so a
    /// write through a mapping whose whole block is unwatched can skip the version bump entirely.
    code_blk: Vec<u8>,
    /// Device time is advanced lazily: cycles accumulate here and the devices see them in one
    /// batch when a timer is due, a peripheral register is accessed, or the active-device
    /// backstop expires. Quiet devices allow a longer backstop.
    tick_pending: u32, tick_budget: u32,
    /// EX133 virtual quanta: stop in front of device-register accesses / one was just refused.
    pub(crate) defer_mmio: bool, pub(crate) mmio_deferred: bool, pub vq_violations: u64,
    approximate_cache: Option<crate::approximate_cache::CacheTiming>,
    approximate_cache_pending: u32,
    approximate_cache_fast_internal: bool,
    approximate_cache_inline: bool,
    approximate_cache_yield_miss: bool,
    cache_resource: CacheResource,
    pub(crate) fetch_cache: xtensa_lx7::state::SharedFetchCache,
    board_edges: bool,
}

/// One shared external resource, occupied only by priced fills/writebacks.
/// Requests within a compiled batch are serialized at its supplied start time.
#[derive(Default)]
struct CacheResource {
    enabled: bool,
    busy_until: u64,
    cursor: u64,
    core: usize,
    fill_cycles: u32,
    fill_service_cycles: u32,
    flash_timing: Option<(u32, u32)>, // demand readiness and shared-bus occupancy
    writeback_cycles: u32,
    wait_cycles: [u64; 2],
}

/// Preserve the original cadence whenever an active device lacks a deadline.
const MAX_TICK_DEFER: u32 = 256;
/// EX134: only quiet devices may use this longer backstop. The default stays below
/// one USB SOF period (60000 cycles); overrides are for explicit experiments.
const QUIET_TICK_DEFER: u32 = match option_env!("ESP32SIM_DEFER_BUILD") {
    Some(s) => { let b = s.as_bytes(); let (mut i, mut v) = (0, 0u32); while i < b.len() { v = v * 10 + (b[i] - b'0') as u32; i += 1; } v }
    None => 32768,
};

/// Buffer identifiers for resolved addresses.
pub const SRC_SRAM: u8 = 0; pub const SRC_IROM: u8 = 1; pub const SRC_FLASH: u8 = 2; pub const SRC_PSRAM: u8 = 3;
pub const SRC_DROM: u8 = 4; pub const SRC_RTC_FAST: u8 = 5; pub const SRC_RTC_SLOW: u8 = 6;
const TLB_SIZE: usize = xtensa_lx7::bus::TLB_ENTRIES;
/// Granularity of the write-version counters. Must exceed the longest block (`block::MAX_LEN` × 3 bytes).
const VPAGE_SHIFT: usize = xtensa_lx7::bus::VPAGE_SHIFT as usize;
const VPAGE_MASK: usize = (1 << VPAGE_SHIFT) - 1;
use xtensa_lx7::bus::{FastMem, TlbEntry};
#[inline(always)]
fn tlb_idx(addr: u32) -> usize { xtensa_lx7::bus::tlb_index(addr) }
/// Review B1: the next bus's `flash_epoch` base. A CPU whose caches outlive one bus must not match
/// another bus's epoch; 2^32 flash-version changes per bus before bases could meet.
static BUS_EPOCHS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl SocBus {
    pub(crate) fn cancel_spi2_timing(&mut self) { self.spi2_scheduled = None; self.spi2_pins = None; self.spi3_pins = None; }

    pub fn new(flash_size: usize, psram_size: usize, mac: [u8; 6]) -> Self { Self::with_sizes(flash_size, psram_size, mac) }
    pub fn with_sizes(flash_size: usize, psram_size: usize, mac: [u8; 6]) -> Self {
        let bus_uninit = SocBus {
            ble: Default::default(),
            sram: vec![0; SRAM_SIZE], irom: vec![0; (IROM_MASK_HIGH - IROM_MASK_LOW) as usize], drom: vec![0; (DROM_MASK_HIGH - DROM_MASK_LOW) as usize],
            rtc_fast: vec![0; 8192], rtc_slow: vec![0; 8192], flash: vec![0xff; flash_size], psram: vec![0; psram_size],
            mmu: [MMU_INVALID; MMU_ENTRIES], periph: Peripherals::new(mac), board: Box::new(crate::board::Atech14::new()), uart_pins: false, cycles: 0, last_fault: None, spi2_dma_fault: None, irq_dirty: false, gpio_events: None, debug: Default::default(), board_edges: true,
            spi2_timing: false, spi2_scheduled: None, spi2_pins: None, spi3_dma_fault: None, spi3_pins: None,
            tlb: vec![TlbEntry::EMPTY; TLB_SIZE], page_ver: Vec::new(), ver_base: [0; 7], flash_epoch: BUS_EPOCHS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) << 32, code_blk: Vec::new(), tick_pending: 0, tick_budget: 0, defer_mmio: false, mmio_deferred: false, vq_violations: 0,
            approximate_cache: None, approximate_cache_pending: 0, approximate_cache_fast_internal: false, approximate_cache_inline: false,
            approximate_cache_yield_miss: false,
            cache_resource: CacheResource::default(),
            fetch_cache: xtensa_lx7::state::SharedFetchCache::default(),
        };
        let mut b = bus_uninit;
        b.periph.spi0.set_psram_size(psram_size); b.periph.spi1.set_psram_size(psram_size);   // MR2 says how big `psram` is
        b.rebuild_page_table();
        b
    }

    /// An octal SPI flash (ESP32-S3-WROOM-2) instead of the quad one. The flash answers the
    /// Macronix octal commands and its JEDEC ID is C2 80 (log2(size) + 0x20); efuse FLASH_TYPE,
    /// EFUSE_RD_REPEAT_DATA3 (0x3c) bit 9, says octal (IDF v5.5.5
    /// components/soc/esp32s3/register/soc/efuse_reg.h:540, :602-605; v4.4.8
    /// components/soc/esp32s3/include/soc/efuse_reg.h:433, :472-475 the same). The bootloader
    /// reads it (components/bootloader_support/bootloader_flash/src/bootloader_flash.c:1024-1030
    /// `bootloader_flash_is_octal_mode_enabled`, v5.5.5), as does the ROM's own OPI bring-up.
    /// Call before boot.
    pub fn set_flash_octal(&mut self, on: bool) {
        let id = esp_periph::flash_jedec(on, self.flash.len());
        for s in [&mut self.periph.spi0, &mut self.periph.spi1] { s.octal_flash = on; s.flash_cr2 = 0; s.jedec = id; }
        let r = self.periph.efuse.ram.read(0x3c);
        self.periph.efuse.ram.write(0x3c, if on { r | 1 << 9 } else { r & !(1 << 9) });
    }

    /// Rough helper-path experiment. Disables direct memory access so reads and
    /// writes visit cache state. Enable before execution. No instruction costs.
    pub fn enable_approximate_cache(&mut self, config: crate::approximate_cache::CacheConfig) {
        self.cache_resource.fill_cycles = config.fill_cycles;
        self.cache_resource.fill_service_cycles = config.fill_cycles;
        self.cache_resource.flash_timing = None;
        self.cache_resource.writeback_cycles = config.writeback_cycles;
        self.approximate_cache = Some(crate::approximate_cache::CacheTiming::new(config));
        self.approximate_cache_pending = 0;
        self.approximate_cache_inline = false;
        self.approximate_cache_fast_internal = false;
        self.invalidate_tlb();
    }

    /// Chip reset drops shared cache contents and timing debt, not experiment settings.
    /// Reset in place so an inline cache view retains its backing allocation.
    pub(crate) fn reset_approximate_cache(&mut self) {
        if let Some(cache) = &mut self.approximate_cache { cache.reset(); }
        self.approximate_cache_pending = 0;
        self.cache_resource.busy_until = self.cycles;
        self.cache_resource.cursor = self.cycles;
        self.cache_resource.core = 0;
        self.cache_resource.wait_cycles = [0; 2];
    }

    pub fn approximate_cache_stats(&self) -> Option<crate::approximate_cache::CacheAccess> {
        self.approximate_cache.as_ref().map(|cache| cache.stats())
    }
    /// Keep internal SRAM on the generated direct path while external data uses priced helpers.
    pub fn set_approximate_cache_fast_internal(&mut self, enabled: bool) {
        self.approximate_cache_fast_internal = enabled;
        self.invalidate_tlb();
    }
    /// Reuse the existing compiled helper exit to schedule the first cache miss promptly.
    pub fn set_approximate_cache_yield_miss(&mut self, enabled: bool) {
        self.approximate_cache_yield_miss = enabled;
    }

    pub fn set_approximate_cache_inline(&mut self) -> bool {
        if !cfg!(all(target_arch = "wasm32", feature = "cache-inline")) { return false; }
        if self.approximate_cache.as_mut().and_then(|c| c.inline_view()).is_none() { return false; }
        self.approximate_cache_inline = true;
        self.approximate_cache_fast_internal = true;
        self.invalidate_tlb();
        true
    }

    pub fn take_approximate_cache_penalty(&mut self) -> u32 {
        std::mem::take(&mut self.approximate_cache_pending)
    }

    /// Enable before execution, together with the earliest-ready compiled scheduler.
    pub fn set_approximate_cache_contention(&mut self, enabled: bool) {
        self.cache_resource.enabled = enabled;
        self.cache_resource.busy_until = self.cycles;
        self.cache_resource.cursor = self.cycles;
        self.cache_resource.wait_cycles = [0; 2];
    }
    pub fn approximate_cache_wait_cycles(&self) -> [u64; 2] { self.cache_resource.wait_cycles }
    /// Hypothesis: requested data can be ready before the external line burst finishes.
    /// CPU readiness remains CacheConfig.fill_cycles; default service equals readiness.
    pub fn set_approximate_cache_fill_service(&mut self, cycles: u32) -> bool {
        if cycles < self.cache_resource.fill_cycles { return false; }
        self.cache_resource.fill_service_cycles = cycles;
        true
    }

    /// Optional flash-only timing experiment. Configure the common cache first;
    /// absent this override, flash and PSRAM retain the common readiness/service.
    pub fn set_approximate_flash_timing(&mut self, ready: u32, service: u32) -> bool {
        if self.approximate_cache.is_none() || service < ready { return false; }
        self.cache_resource.flash_timing = Some((ready, service));
        true
    }

    #[cfg_attr(not(target_arch = "wasm32"), inline(always))]
    #[cfg_attr(target_arch = "wasm32", inline)]
    fn price_cached_data(&mut self, entry: TlbEntry, address: u32, width: u32, write: bool) {
        if !matches!(entry.src as u8, SRC_FLASH | SRC_PSRAM) { return; }
        if let Some(cache) = &mut self.approximate_cache {
            // Physical offset plus resource distinguishes flash from PSRAM and
            // recognizes virtual aliases. Both cores share this bus/cache.
            let key = (entry.src << 28) | (entry.off + address - entry.lo);
            let resource = &mut self.cache_resource;
            let common = (resource.fill_cycles, resource.fill_service_cycles);
            let (ready, service) = if entry.src as u8 == SRC_FLASH {
                resource.flash_timing.unwrap_or(common)
            } else { common };
            let result = cache.access_with_fill_cycles(key, width, write, ready);
            if resource.enabled {
                let mut wait = 0u64;
                // Dirty victims finish before refill. Writes do not have an early-ready split.
                for _ in 0..result.dirty_writebacks {
                    wait = wait.saturating_add(resource.reserve(resource.writeback_cycles, resource.writeback_cycles));
                }
                for _ in 0..result.line_fills {
                    wait = wait.saturating_add(resource.reserve(ready, service));
                }
                resource.wait_cycles[resource.core] = resource.wait_cycles[resource.core].saturating_add(wait);
                // Requested-data readiness is already included in extra_cycles below.
                self.approximate_cache_pending = self.approximate_cache_pending
                    .saturating_add(wait.min(u32::MAX as u64) as u32);
            }
            self.approximate_cache_pending = self.approximate_cache_pending
                .saturating_add(result.extra_cycles.min(u32::MAX as u64) as u32);
        }
    }

    /// Clear both I2C buses, including directly attached devices. Call before
    /// `attach_board_devices` when replacing a board or rebuilding its device list.
    pub fn clear_i2c_devices(&mut self) {
        for bus in &mut self.periph.i2c { bus.clear_devices(); }
    }

    /// Attach fresh peripheral-side devices and restore the levels driven by the persistent board.
    pub fn attach_board_devices(&mut self) {
        if self.board.uses_gpio_waveform() {
            self.board.gpio_waveform_at(self.cycles, esp_soc::pins::ChipPins::S3.routes(&self.periph.gpio, &self.periph.io_mux));
        }
        self.uart_pins = self.board.uses_uart_pins();
        self.board_edges = self.board.uses_gpio_edges();
        for (bus, address, device) in self.board.i2c_devices() {
            self.periph.i2c[bus as usize].attach(address, device);
        }
        for (pin, level) in self.board.input_levels() {
            self.irq_dirty |= self.periph.gpio.set_input(pin, level);
        }
        // Restored input edges and the board's own deadline can activate device work.
        self.refresh_tick_budget();
        for pin in self.board.released_inputs() { esp_soc::SocBus::gpio_release_input(self, pin); }
    }

    /// Time until deferred device work must run. The bounded fallback covers devices without
    /// an explicit timer deadline.
    pub fn next_deadline(&self) -> u64 { self.tick_budget.saturating_sub(self.tick_pending).max(1) as u64 }

    /// Size the per-page version table to the buffers. Call after replacing `flash` or `psram`.
    pub fn rebuild_page_table(&mut self) {
        let sizes = [self.sram.len(), self.irom.len(), self.flash.len(), self.psram.len(), self.drom.len(), self.rtc_fast.len(), self.rtc_slow.len()];
        let mut base = 0u32;
        for (i, n) in sizes.iter().enumerate() { self.ver_base[i] = base; base += ((n + VPAGE_MASK) >> VPAGE_SHIFT) as u32; }
        self.page_ver = vec![0; base as usize + 1];
        // Small buffers can share a version block; marking it conservatively watches both.
        // An entry covers at most 64 KiB, hence at most two blocks. Keep marks across a resize: forgetting one could let a write to watched code skip its version bump.
        self.code_blk.resize((self.page_ver.len() >> 8) + 2, 0);
        self.invalidate_tlb();
    }

    /// EX110: is any page of the 64 KiB block holding version page `p` watched by decoded code?
    /// Unknown indices answer yes, so a mapping outside the table never skips bookkeeping.
    #[inline(always)]
    fn blk_watched(&self, p: u32) -> bool { self.code_blk.get((p >> 8) as usize).copied().unwrap_or(1) != 0 }

    /// Forget every cached mapping. Anything that re-points the flash MMU must call this.
    /// A remap changes which bytes a cache-window pc refers to without any write happening, so
    /// the flash and PSRAM page versions are bumped too: that is what invalidates decoded
    /// instructions and blocks that were built through the old mapping. Shared fetch tags
    /// are virtual, so remapping also makes the shared instruction cache cold.
    pub fn invalidate_tlb(&mut self) {
        self.fetch_cache.reset();
        for e in self.tlb.iter_mut() { *e = TlbEntry::EMPTY; }
        let (a, b) = (self.ver_base[SRC_FLASH as usize] as usize, self.ver_base[SRC_DROM as usize] as usize);
        for v in &mut self.page_ver[a..b] { *v = v.wrapping_add(1); }          // flash then psram
        self.flash_epoch += 1;
    }

    /// shell-s2: versions `first..=last` changed; move the epoch if `stable_pages` covers one.
    #[inline(always)]
    pub(crate) fn touched(&mut self, first: usize, last: usize) {
        let (lo, hi, _) = Bus::stable_pages(self);
        if first < hi as usize && last >= lo as usize { self.flash_epoch += 1; }
    }

    #[inline(always)]
    fn buf(&self, src: u8) -> &Vec<u8> {
        match src { SRC_SRAM => &self.sram, SRC_IROM => &self.irom, SRC_FLASH => &self.flash, SRC_PSRAM => &self.psram,
                    SRC_DROM => &self.drom, SRC_RTC_FAST => &self.rtc_fast, _ => &self.rtc_slow }
    }
    #[inline(always)]
    fn buf_mut(&mut self, src: u8) -> &mut Vec<u8> {
        match src { SRC_SRAM => &mut self.sram, SRC_IROM => &mut self.irom, SRC_FLASH => &mut self.flash, SRC_PSRAM => &mut self.psram,
                    SRC_DROM => &mut self.drom, SRC_RTC_FAST => &mut self.rtc_fast, _ => &mut self.rtc_slow }
    }

    /// The mapping covering `addr`, from the TLB or by walking the address map.
    #[inline(always)]
    fn lookup(&mut self, addr: u32) -> Option<TlbEntry> {
        let e = self.tlb[tlb_idx(addr)];
        if addr >= e.lo && addr < e.hi { Some(e) } else { self.tlb_fill(addr) }
    }

    /// Walk the address map for the 64 KiB page holding `addr` and remember it.
    fn tlb_fill(&mut self, addr: u32) -> Option<TlbEntry> {
        let page = addr & !0xffff;
        let region = |lo: u32, hi: u32, src: u8, w: bool| -> TlbEntry {
            let lo_ = page.max(lo); let hi_ = (page + 0x10000).min(hi);
            TlbEntry { lo: lo_, hi: hi_, base: std::ptr::null_mut(), off: lo_ - lo, vbase: 0, src: src as u32, writable: w as u16, code: 1, #[cfg(target_arch = "wasm32")] span: 0 }
        };
        let mut e = match addr {
            DRAM_LOW..=0x3FCF_FFFF => { let mut e = region(DRAM_LOW, 0x3FD0_0000, SRC_SRAM, true); e.off += 0x8000; e }
            IRAM_LOW..=0x403D_FFFF => region(IRAM_LOW, 0x403E_0000, SRC_SRAM, true),
            IROM_MASK_LOW..=0x4005_FFFF => region(IROM_MASK_LOW, 0x4006_0000, SRC_IROM, false),
            DROM_MASK_LOW..=0x3FF1_FFFF => region(DROM_MASK_LOW, 0x3FF2_0000, SRC_DROM, false),
            RTC_FAST_LOW..=0x600F_FFFF => region(RTC_FAST_LOW, 0x6010_0000, SRC_RTC_FAST, true),
            RTC_SLOW_LOW..=0x5000_1FFF => region(RTC_SLOW_LOW, 0x5000_2000, SRC_RTC_SLOW, true),
            DBUS_LOW..=0x3DFF_FFFF | IBUS_LOW..=0x43FF_FFFF => {
                let linear = addr & 0x1FF_FFFF;
                let entry = self.mmu[(linear >> 16) as usize];
                if entry & MMU_INVALID != 0 { return None; }
                let off = (entry & 0x3fff) as usize * PAGE as usize;
                let (src, w) = if entry & MMU_SPIRAM != 0 { (SRC_PSRAM, true) } else { (SRC_FLASH, false) };
                if off + PAGE as usize > self.buf(src).len() { return None; }
                TlbEntry { lo: page, hi: page + 0x10000, base: std::ptr::null_mut(), off: off as u32, vbase: 0, src: src as u32, writable: w as u16, code: 1, #[cfg(target_arch = "wasm32")] span: 0 }
            }
            _ => return None,
        };
        e.vbase = self.ver_base[e.src as usize] + (e.off as usize >> VPAGE_SHIFT) as u32;
        // EX110: an entry spans at most 64 KiB, so its pages lie in at most two blocks.
        e.code = (self.blk_watched(e.vbase) || self.blk_watched(e.vbase + ((e.hi - e.lo - 1) >> VPAGE_SHIFT))) as u16;
        let off = e.off as usize;
        e.base = self.buf_mut(e.src as u8).as_mut_ptr().wrapping_add(off);
        // EX173: generated probes test `addr - lo` against this span, and derive an access's
        // alignment from the same difference, so every mapping starts 16-byte aligned.
        debug_assert_eq!(e.lo % 16, 0);
        let e = e.with_span();
        // Do not publish external mappings to generated loads/stores while pricing cache accesses.
        // Returning the mapping still lets the slow accessor perform this one access.
        if !(self.approximate_cache.is_some() && self.approximate_cache_fast_internal && !self.approximate_cache_inline
            && matches!(e.src as u8, SRC_FLASH | SRC_PSRAM)) {
            self.tlb[tlb_idx(addr)] = e;
        }
        Some(e)
    }

    /// Record that `len` bytes at `off` of the page group starting at `vbase` changed. An
    /// instruction can begin up to two bytes before a page boundary, so the previous page is
    /// bumped too when the write touches the first bytes of one. EX110: guest stores call this
    /// only through a mapping with `code != 0`; every other caller bumps unconditionally.
    #[inline(always)]
    fn bump(&mut self, vbase: u32, off: usize, len: usize) {
        let p = vbase as usize + (off >> VPAGE_SHIFT);
        self.page_ver[p] = self.page_ver[p].wrapping_add(1);
        let last = vbase as usize + ((off + len - 1) >> VPAGE_SHIFT);
        if last != p { self.page_ver[last] = self.page_ver[last].wrapping_add(1); }
        if off & VPAGE_MASK < emu_core::bus::PREV_PAGE_BYTES as usize && p > 0 { self.page_ver[p - 1] = self.page_ver[p - 1].wrapping_add(1); }
        self.touched(p.saturating_sub(1), last);
    }

    /// EX110: watch page `vidx` from now on. A code page also marks its neighbors' blocks,
    /// because `bump` moves the page before a write in the first three bytes of a page and the
    /// page after a write that spans one: if the written page's own block is unwatched, then no
    /// consumer depends on any of the three, and the whole bump can go. Published entries are
    /// dropped so that generated code reloads the new flag; no byte changed, so no version moves.
    fn watch_code_page(&mut self, vidx: u32) {
        let mut changed = false;
        for p in [vidx.saturating_sub(1), vidx, vidx.saturating_add(1)] {
            if let Some(f) = self.code_blk.get_mut((p >> 8) as usize) { if *f == 0 { *f = 1; changed = true; } }
        }
        if changed { for e in self.tlb.iter_mut() { *e = TlbEntry::EMPTY; } }
    }

    /// Record a write done behind the bus's back (image loaders, the SPI flash controller).
    pub fn note_written(&mut self, src: u8, off: usize, len: usize) {
        if len == 0 { return; }
        let vbase = self.ver_base[src as usize];
        let (first, last) = (off >> VPAGE_SHIFT, (off + len - 1) >> VPAGE_SHIFT);
        for p in first..=last { let i = vbase as usize + p; if i < self.page_ver.len() { self.page_ver[i] = self.page_ver[i].wrapping_add(1); } }
        if off & VPAGE_MASK < emu_core::bus::PREV_PAGE_BYTES as usize && first > 0 { let i = vbase as usize + first - 1; self.page_ver[i] = self.page_ver[i].wrapping_add(1); }
        self.touched((vbase as usize + first).saturating_sub(1), vbase as usize + last);
    }

    #[inline]
    fn is_periph(addr: u32) -> bool { (PERIPH_BASE..PERIPH_END).contains(&addr) }

    fn periph_read(&mut self, addr: u32) -> u32 {
        if (MMU_TABLE..MMU_TABLE + (MMU_ENTRIES as u32) * 4).contains(&addr) {
            return self.mmu[((addr - MMU_TABLE) >> 2) as usize];
        }
        self.vq_backstop(addr);
        self.flush_ticks();                                         // registers must show exact time
        // IDF v5.5.4 components/soc/esp32s3/register/soc/gpio_reg.h: GPIO_IN/IN1.
        if self.board_edges && matches!(addr & !3, 0x6000_403c | 0x6000_4040) {
            self.irq_dirty |= esp_soc::gpio::deliver_board_inputs(&mut *self.board, &mut self.periph.gpio, &mut self.gpio_events, self.cycles);
            self.refresh_tick_budget();
        }
        self.periph.read32(addr)
    }
    /// EX133: every device-register access must have been deferred out of a multi-quantum run.
    /// One that was not (a PIE or MAC16 word access; nothing real does this) saw early time.
    #[inline]
    fn vq_backstop(&mut self, addr: u32) {
        if self.defer_mmio { assert!(option_env!("ESP32SIM_VQ_STRICT").is_none(), "EX133: undeferred device access at {addr:#x}"); self.vq_violations += 1; if self.vq_violations == 1 { eprintln!("[emu] EX133: undeferred device access at {addr:#x}, pc {:#x}", self.periph.misc.cur_pc); } }
    }
    fn periph_write(&mut self, addr: u32, v: u32) {
        self.vq_backstop(addr);
        self.periph_write_inner(addr, v);
        // RMT TX3 DMA is staged at MMIO boundaries once DMA_ACCESS_EN is set; one bit
        // test keeps every other write on main's path.
        if self.periph.rmt.ch[3].conf0 & (1 << 25) != 0 { self.rmt_dma_write(addr & !3, v); }
        self.refresh_tick_budget();   // the write may have armed something
    }
    fn periph_write_inner(&mut self, addr: u32, v: u32) {
        if (MMU_TABLE..MMU_TABLE + (MMU_ENTRIES as u32) * 4).contains(&addr) {
            let i = ((addr - MMU_TABLE) >> 2) as usize;
            if self.mmu[i] != v & 0xffff { self.mmu[i] = v & 0xffff; self.invalidate_tlb(); }
            return;
        }
        // EX254: `periph_write` re-derives the tick budget after this write, and nothing in
        // between reads it, so the flush skips its own refresh.
        self.flush_ticks_unbudgeted();
        let a = addr & !3;
        // mmio-s1: SPI2 and GDMA writes reach only those two devices' sources (and RAM, the board),
        // so they re-derive interrupt lines only when one of those sources moved; SPI3 (0x60025)
        // likewise.
        let spi = matches!(a >> 12, 0x60024 | 0x60025 | 0x6003f);
        let sources = |p: &Peripherals| (p.spi2.irq(), p.spi3.irq(), esp_periph::Device::irq_sources(&p.gdma));
        let before = if spi { sources(&self.periph) } else { (false, false, 0) };
        if a == PERIPH_BASE + 0x24_000 && v & (1 << 24) != 0 {
            self.spi2_dma_fault = None;
            self.spi2_scheduled = None;
        }
        if a == PERIPH_BASE + 0x25_000 && v & (1 << 24) != 0 {
            self.spi3_dma_fault = None;
        }
        // GDMA block: OUT_RST (0x60) or an OUT_LINK START/RESTART (0x80) on the
        // scheduled SPI2 source channel discards the transaction: the payload was
        // snapshotted for a chain that no longer exists, so hardware-model semantics
        // are abort without TRANS_DONE. OUT_LINK STOP only stops later fetches and
        // plain W1C interrupt clears or configuration writes never cancel.
        if let Some((_, completion)) = &self.spi2_scheduled {
            if a >= PERIPH_BASE + 0x3f_000 && a < PERIPH_BASE + 0x3f_000 + crate::periph::GDMA_CH_STRIDE * crate::periph::GDMA_CHANNELS as u32 {
                let offset = a & 0xfff;
                let (channel, register) = ((offset / crate::periph::GDMA_CH_STRIDE) as usize, offset % crate::periph::GDMA_CH_STRIDE);
                if channel == completion.channel
                    && ((register == 0x60 && v & 1 != 0) || (register == 0x80 && v & ((1 << 21) | (1 << 22)) != 0))
                {
                    if self.periph.spi2.log { eprintln!("[spi2] DMA source reset/rebound: aborting scheduled transfer"); }
                    self.spi2_scheduled = None;
                    self.periph.spi2.fail_dma_tx();
                }
            }
        }
        let old_input = self.periph.gpio.input;
        let old_gpio_out = self.periph.gpio.out;
        let old_gpio_enable = self.periph.gpio.enable;
        if a == PERIPH_BASE + 0x24_000 && v & (1 << 24) != 0 && !self.periph.spi2.has_pending_transfer() {
            self.spi2_pins = self.board.uses_spi_pins().then(|| self.periph.spi2_pins());
        }
        if a == PERIPH_BASE + 0x25_000 && v & (1 << 24) != 0 && !self.periph.spi3.has_pending_transfer() {
            self.spi3_pins = self.board.uses_spi_pins().then(|| self.periph.spi3_pins());
        }
        self.periph.write32(a, v);
        if matches!(a & !0xfff, 0x6000_4000 | 0x6000_9000) && self.board.uses_gpio_waveform() {
            self.board.gpio_waveform_at(self.cycles, esp_soc::pins::ChipPins::S3.routes(&self.periph.gpio, &self.periph.io_mux));
        }
        if old_gpio_out != self.periph.gpio.out || old_gpio_enable != self.periph.gpio.enable {
            let changes = &self.periph.gpio.changes;
            if let Some(events) = &mut self.gpio_events {
                for &(pin, level) in changes { events.push((self.cycles, pin, level)); }
            }
            self.board.gpio_output_at(self.cycles, changes, self.periph.gpio.enable, self.periph.gpio.out);
            self.periph.gpio.changes.clear();
        }
        if let Some(port) = match a { 0x60000000 => Some(0), 0x60010000 => Some(1), 0x6002e000 => Some(2), _ => None } {
            if self.uart_pins { self.board.uart_tx(self.cycles, self.periph.uart_route(port), v as u8); }
        }
        self.complete_spi2_dma();
        self.deliver_spi2_transfer();
        self.complete_spi3_dma();
        self.deliver_spi3_transfer();
        // Resolved output changes can raise edge or level input interrupts.
        if spi {
            self.irq_dirty |= before != sources(&self.periph);
        } else if !(0x6000_4004..=0x6000_4018).contains(&a) {
            self.irq_dirty = true;
        } else {
            self.irq_dirty |= (old_input ^ self.periph.gpio.input) & self.periph.gpio.int_ena_pins != 0;
        }
        if self.periph.spi_exec {
            self.periph.spi_exec = false;
            self.periph.spi1.execute(&mut self.flash, &mut self.psram);
            for (m, off, len) in std::mem::take(&mut self.periph.spi1.dirty) { self.note_written(match m { crate::periph::DirtyMem::Flash => SRC_FLASH, crate::periph::DirtyMem::Psram => SRC_PSRAM }, off, len); }
        }
    }

    fn now_us(&self) -> u64 { self.cycles / (crate::periph::CPU_HZ / 1_000_000) }

    /// WiFi MAC transmit: fetch the queued frames from their DMA descriptors and complete them.
    fn wifi_tx_step(&mut self) {
        let pending = std::mem::take(&mut self.periph.wifi.tx_pending);
        for (slot, desc) in pending {
            let dw0 = self.read32_unpriced(desc).unwrap_or(0); let pkt = self.read32_unpriced(desc + 4).unwrap_or(0);
            let len = ((dw0 >> 12) & 0xfff) as usize;
            let mut frame = Vec::with_capacity(len);
            for i in 0..len { frame.push(self.read8_unpriced(pkt + i as u32).unwrap_or(0)); }
            if self.periph.wifi.log || self.debug.has("wifi-frames") { eprintln!("[wifi] TX slot {} desc {:#010x} pkt {:#010x} {}", slot, desc, pkt, crate::wifi::describe(&frame)); }
            self.periph.wifi.tx_done(slot);
            self.irq_dirty = true;
            let now_us = self.now_us();
            self.periph.wifi.link.station_tx(&frame, now_us);
        }
    }

    /// The virtual air: beacons/responses from the AP and frames from the network backend land in the RX ring.
    fn wifi_air_step(&mut self) {
        let now_us = self.now_us();
        if self.periph.wifi.link.nothing_due(now_us) { return; }
        let busy = { let d = self.periph.wifi.link.last_rx_desc(); d != 0 && self.read32_unpriced(d).unwrap_or(0) & esp_soc::wifi::RX_DESC_HAS_DATA != 0 };
        if let Some(frame) = self.periph.wifi.link.next_rx(now_us, busy) { self.wifi_rx_deliver(&frame, now_us); }
    }

    /// The receive descriptor at `desc` (a masked pointer from `rx_next`) as the DMA would use it for a
    /// `total`-byte frame, if software armed it: owned, with a buffer, and with room for the frame.
    /// `None` means the receive DMA is stopped there.
    fn wifi_rx_armed(&mut self, desc: u32, total: usize) -> Option<(u32, u32, u32, u32)> {
        if desc == 0 { return None; }
        let d = desc | crate::periph::DMA_ADDR_BASE;
        let dw0 = self.read32_unpriced(d).unwrap_or(0);
        let buf = self.read32_unpriced(d + 4).unwrap_or(0);
        let next = self.read32_unpriced(d + 8).unwrap_or(0);
        (dw0 & (1 << 31) != 0 && buf != 0 && (dw0 & 0xfff) as usize >= total).then_some((d, dw0, buf, next))
    }

    /// Write one received frame into the next RX descriptor (rx_ctrl header + frame + FCS) and raise the RX event.
    #[allow(clippy::identity_op, reason = "rx_state zero remains visible in the packed descriptor layout")]
    fn wifi_rx_deliver(&mut self, frame: &[u8], now_us: u64) {
        let total = 48 + frame.len() + 4;
        // The DMA can only advance while the descriptor at `rx_next` is one software armed: owned, with a
        // buffer, and with room for the whole frame. A driver that hands the hardware one short list at a
        // time (Arduino-ESP32 does: four descriptors, the last one's `next` null) leaves the next word of
        // the list empty by the time the next frame is due, and writes the base of the *new* list before
        // that frame arrives. So a descriptor that is not usable means the receive DMA has stopped: take
        // the pending base as the new head and try once more. Frames that still find nothing are lost, as
        // they are when a radio's RX FIFO is full.
        let mut armed = self.wifi_rx_armed(self.periph.wifi.rx_next, total);
        if armed.is_none() && self.periph.wifi.rx_restart_at_base() { armed = self.wifi_rx_armed(self.periph.wifi.rx_next, total); }
        // Neither a head nor a pending base: the driver has not handed the radio a receive ring yet, which
        // is not a lost frame. Counted apart, and only after the hand-over above has had its chance (#276).
        if self.periph.wifi.rx_next == 0 { self.periph.wifi.link.drop_rx_unarmed(); return; }
        let Some((desc, dw0, buf, next)) = armed else {
            if self.periph.wifi.log { eprintln!("[wifi] RX dropped: rx_next {:#x} is not a usable descriptor (base {:#x}, last {:#x})", self.periph.wifi.rx_next, self.periph.wifi.rx_base & 0xf_ffff, self.periph.wifi.rx_last); }
            self.periph.wifi.link.drop_rx(); return;
        };
        // A network around's frames carry its own channel and signal (`VirtualAp::rx_meta`)
        let (chan, rssi, log) = { let ap = self.periph.wifi.link.ap().unwrap(); let (c, r) = ap.rx_meta(frame); (c as u32, r, ap.log) };
        let mut b = Vec::with_capacity(total);
        // rx_ctrl word 0 (silicon: a real broadcast beacon reads 0x111b20ad — bit 28 set, signed rssi in the low
        // byte). The MAC has already address-filtered, so every delivered frame is "for us"; use the same flags
        // for unicast and broadcast (an invented "filter_match" nibble made the blob discard unicast frames).
        // filter-match nibble (silicon: broadcast beacon reads bit 28). A frame the hardware accepted because
        // addr1 is our unicast MAC must carry the unicast-match bit (29), not the broadcast bit (28), or
        // wDev_IndicateFrame drops it as "not for me".
        let bcast = frame.len() >= 5 && frame[4] & 1 == 1;
        // filter-match nibble: bit 28 is the "accepted by the address filter" bit the blob's RX path
        // requires (silicon: a broadcast beacon reads 0x111b20ad); unicast frames add bit 29.
        let fm = if bcast { 1u32 << 28 } else { (1u32 << 28) | (1u32 << 29) };
        let w0: u32 = fm | (rssi as u8 as u32);   // rssi as the network reports it (`rx_meta`), 1 Mbps, legacy
        let w2: u32 = (chan << 16) | (chan << 20);                                        // channel, secondary
        let w5: u32 = 0xa6;                                                                // noise floor -90
        let w11: u32 = ((frame.len() + 4) as u32 & 0xfff) | (0 << 24);                    // sig_len (incl. FCS), rx_state OK
        for w in [w0, 0, w2, now_us as u32, 0, w5, 0, 0, 0, 0, 0, w11] { b.extend_from_slice(&w.to_le_bytes()); }
        b.extend_from_slice(frame); b.extend_from_slice(&crate::wifi::fcs(frame).to_le_bytes());
        let mut i = 0usize;
        while i + 4 <= b.len() { let v = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]); let _ = self.write32_unpriced(buf + i as u32, v); i += 4; }
        while i < b.len() { let _ = self.write8_unpriced(buf + i as u32, b[i]); i += 1; }
        let ndw0 = (dw0 & !(0xfff << 12)) | ((total as u32) << 12) | (1 << 30) | (1 << 31);   // length; owner AND has_data set (verified on silicon 2026-08-25: dw0=0xc0..)
        let _ = self.write32_unpriced(desc, ndw0);
        self.periph.wifi.rx_filled(desc, next);
        if log { let d = crate::wifi::describe(frame); if d.contains("auth")||d.contains("assoc") { eprintln!("[wifi] RX AUTH/ASSOC -> desc {:#010x} buf {:#010x} {}", desc, buf, d); } else { eprintln!("[wifi] RX -> desc {:#010x} {}", desc, d); } }
        self.irq_dirty = true;
    }

    pub(crate) fn flash_off(&mut self, addr: u32) -> Option<usize> {
        let e = self.lookup(addr)?;
        (e.src as u8 == SRC_FLASH).then_some(e.off as usize + (addr - e.lo) as usize)
    }

    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        for (i, b) in data.iter().enumerate() {
            let a = addr.wrapping_add(i as u32);
            let Some(e) = self.lookup(a) else { return Err(format!("load: address {:#010x} not mapped", a)) };
            let o = e.off as usize + (a - e.lo) as usize;
            self.buf_mut(e.src as u8)[o] = *b;
            self.bump(e.vbase, o - e.off as usize, 1);
        }
        Ok(())
    }
}

impl CacheResource {
    fn reserve(&mut self, ready: u32, service: u32) -> u64 {
        if service == 0 { return 0; }
        let start = self.cursor.max(self.busy_until);
        let wait = start - self.cursor;
        self.cursor = start.saturating_add(u64::from(ready));
        self.busy_until = start.saturating_add(u64::from(service));
        wait
    }
}

impl SocBus {
    /// The radio's analog blocks, whose registers Espressif's SVD declares 8 bits wide: FE2, FE,
    /// NRX and BB (IDF v5.5.4 components/soc/esp32s3/register/soc/reg_base.h, DR_REG_FE2_BASE
    /// 0x6000_5000, DR_REG_FE_BASE 0x6000_6000, DR_REG_NRX_BASE 0x6001_CC00, DR_REG_BB_BASE
    /// 0x6001_D000). esp-hal 1.2.2's sleep setup, run from `esp_hal::init`, clears bits of
    /// NRXPD_CTRL (NRX + 0xd4, IDF v5.5.4 components/soc/esp32s3/include/soc/nrx_reg.h) with
    /// l8ui/s8i. Sub-word accesses there are served from the containing word.
    fn radio_narrow(addr: u32) -> bool {
        matches!(addr & !0xfff, 0x6000_5000 | 0x6000_6000 | 0x6001_d000) || (0x6001_cc00..0x6001_d000).contains(&addr)
    }
    // Const specialization keeps origin checks and CPU pricing out of DMA/host accessors.
    fn read8_access<const CPU: bool>(&mut self, addr: u32) -> Result<u8, Fault> {
        if Self::radio_narrow(addr) { return Ok((self.periph_read(addr & !3) >> (8 * (addr & 3))) as u8); }
        if Self::is_periph(addr) { self.last_fault = Some((addr, false)); return Err(Fault::Prohibited); }
        let Some(e) = self.lookup(addr) else { self.last_fault = Some((addr, false)); return Err(Fault::Unmapped) };
        if CPU { self.price_cached_data(e, addr, 1, false); }
        Ok(self.buf(e.src as u8)[e.off as usize + (addr - e.lo) as usize])
    }
    fn read16_access<const CPU: bool>(&mut self, addr: u32) -> Result<u16, Fault> {
        if Self::radio_narrow(addr) && addr & 1 == 0 { return Ok((self.periph_read(addr & !3) >> (8 * (addr & 2))) as u16); }
        if Self::is_periph(addr) { self.last_fault = Some((addr, false)); return Err(Fault::Prohibited); }
        match self.lookup(addr) {
            Some(e) if e.hi - addr >= 2 => { if CPU { self.price_cached_data(e, addr, 2, false); } let o = e.off as usize + (addr - e.lo) as usize; Ok(u16::from_le_bytes(self.buf(e.src as u8)[o..o + 2].try_into().unwrap())) }
            Some(_) => Ok(u16::from_le_bytes([self.read8_access::<CPU>(addr)?, self.read8_access::<CPU>(addr + 1)?])),       // straddles a page
            None => { self.last_fault = Some((addr, false)); Err(Fault::Unmapped) }
        }
    }
    fn read32_access<const CPU: bool>(&mut self, addr: u32) -> Result<u32, Fault> {
        if Self::is_periph(addr) {
            if addr & 3 != 0 { self.last_fault = Some((addr, false)); return Err(Fault::Misaligned); }
            return Ok(self.periph_read(addr));
        }
        match self.lookup(addr) {
            Some(e) if e.hi - addr >= 4 => { if CPU { self.price_cached_data(e, addr, 4, false); } let o = e.off as usize + (addr - e.lo) as usize; Ok(u32::from_le_bytes(self.buf(e.src as u8)[o..o + 4].try_into().unwrap())) }
            Some(_) => Ok(u32::from_le_bytes([self.read8_access::<CPU>(addr)?, self.read8_access::<CPU>(addr + 1)?, self.read8_access::<CPU>(addr + 2)?, self.read8_access::<CPU>(addr + 3)?])),
            None => { self.last_fault = Some((addr, false)); Err(Fault::Unmapped) }
        }
    }
    fn write8_access<const CPU: bool>(&mut self, addr: u32, v: u8) -> Result<(), Fault> {
        // S3 register writes are modelled only as aligned words (TRM §15.6.6).
        // Reject unsupported widths before reading a device or advancing its time.
        // This is an explicit emulator policy, not a model of optional PMS IRQs. The radio's
        // analog registers are 8 bits wide (`radio_narrow`).
        if Self::radio_narrow(addr) {
            let (word, shift) = (addr & !3, 8 * (addr & 3));
            let old = self.periph_read(word);
            self.periph_write(word, (old & !(0xff << shift)) | (u32::from(v) << shift));
            return Ok(());
        }
        if Self::is_periph(addr) { self.last_fault = Some((addr, true)); return Err(Fault::Prohibited); }
        match self.lookup(addr) {
            Some(e) if e.writable != 0 => { if CPU { self.price_cached_data(e, addr, 1, true); } let rel = (addr - e.lo) as usize; self.buf_mut(e.src as u8)[e.off as usize + rel] = v; if e.code != 0 { self.bump(e.vbase, rel, 1); } Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn write16_access<const CPU: bool>(&mut self, addr: u32, v: u16) -> Result<(), Fault> {
        if Self::radio_narrow(addr) && addr & 1 == 0 {
            let (word, shift) = (addr & !3, 8 * (addr & 2));
            let old = self.periph_read(word);
            self.periph_write(word, (old & !(0xffff << shift)) | (u32::from(v) << shift));
            return Ok(());
        }
        if Self::is_periph(addr) { self.last_fault = Some((addr, true)); return Err(Fault::Prohibited); }
        match self.lookup(addr) {
            Some(e) if e.writable != 0 && e.hi - addr >= 2 => { if CPU { self.price_cached_data(e, addr, 2, true); } let rel = (addr - e.lo) as usize; let o = e.off as usize + rel; self.buf_mut(e.src as u8)[o..o + 2].copy_from_slice(&v.to_le_bytes()); if e.code != 0 { self.bump(e.vbase, rel, 2); } Ok(()) }
            Some(e) if e.writable != 0 => { let b = v.to_le_bytes(); self.write8_access::<CPU>(addr, b[0])?; self.write8_access::<CPU>(addr + 1, b[1]) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn write32_access<const CPU: bool>(&mut self, addr: u32, v: u32) -> Result<(), Fault> {
        if Self::is_periph(addr) {
            if addr & 3 != 0 { self.last_fault = Some((addr, true)); return Err(Fault::Misaligned); }
            self.periph_write(addr, v); return Ok(());
        }
        match self.lookup(addr) {
            Some(e) if e.writable != 0 && e.hi - addr >= 4 => { if CPU { self.price_cached_data(e, addr, 4, true); } let rel = (addr - e.lo) as usize; let o = e.off as usize + rel; self.buf_mut(e.src as u8)[o..o + 4].copy_from_slice(&v.to_le_bytes()); if e.code != 0 { self.bump(e.vbase, rel, 4); } Ok(()) }
            Some(e) if e.writable != 0 => { let b = v.to_le_bytes(); for i in 0..4 { self.write8_access::<CPU>(addr + i, b[i as usize])?; } Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
}

impl Bus for SocBus {
    #[inline]
    fn read8(&mut self, addr: u32) -> Result<u8, Fault> { self.read8_access::<true>(addr) }
    fn read8_unpriced(&mut self, addr: u32) -> Result<u8, Fault> { self.read8_access::<false>(addr) }
    #[inline]
    fn read16(&mut self, addr: u32) -> Result<u16, Fault> { self.read16_access::<true>(addr) }
    fn read16_unpriced(&mut self, addr: u32) -> Result<u16, Fault> { self.read16_access::<false>(addr) }
    #[inline]
    fn read32(&mut self, addr: u32) -> Result<u32, Fault> { self.read32_access::<true>(addr) }
    fn read32_unpriced(&mut self, addr: u32) -> Result<u32, Fault> { self.read32_access::<false>(addr) }
    #[inline]
    fn write8(&mut self, addr: u32, v: u8) -> Result<(), Fault> { self.write8_access::<true>(addr, v) }
    fn write8_unpriced(&mut self, addr: u32, v: u8) -> Result<(), Fault> { self.write8_access::<false>(addr, v) }
    #[inline]
    fn write16(&mut self, addr: u32, v: u16) -> Result<(), Fault> { self.write16_access::<true>(addr, v) }
    fn write16_unpriced(&mut self, addr: u32, v: u16) -> Result<(), Fault> { self.write16_access::<false>(addr, v) }
    #[inline]
    fn write32(&mut self, addr: u32, v: u32) -> Result<(), Fault> { self.write32_access::<true>(addr, v) }
    fn write32_unpriced(&mut self, addr: u32, v: u32) -> Result<(), Fault> { self.write32_access::<false>(addr, v) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        let Some(e) = self.lookup(pc) else { self.last_fault = Some((pc, false)); return Err(Fault::Unmapped) };
        let o = e.off as usize + (pc - e.lo) as usize;
        let b = self.buf(e.src as u8);
        if let Some(w) = b.get(o..o + 4) { return Ok(w.try_into().unwrap()); }
        // last bytes of a buffer (or of a mapped page): what physical memory has, zero beyond
        let mut r = [0u8; 4];
        for (i, byte) in r.iter_mut().enumerate() { if let Some(x) = b.get(o + i) { *byte = *x; } }
        Ok(r)
    }
    #[inline(always)]
    fn page_versions(&self) -> &[u32] { &self.page_ver }
    /// shell-s2: flash pages change only through `bump`, `note_written`, DMA `bump_run` and
    /// `invalidate_tlb`, which all call `touched` or move the epoch; generated stores need a writable
    /// mapping and flash never has one. The last flash page is left out: the EX180 previous-page rule
    /// lets a generated or DMA store to the first bytes of PSRAM bump it without the bus.
    /// hop-s2b: mask ROM (just below flash) too: it is never mapped writable and changes only through
    /// the same calls; its first page is left out, next to SRAM's last.
    #[inline(always)]
    fn stable_pages(&self) -> (u32, u32, u64) { (self.ver_base[SRC_IROM as usize] + 1, self.ver_base[SRC_PSRAM as usize].saturating_sub(1), self.flash_epoch) }
    fn note_code_page(&mut self, vidx: u32) { self.watch_code_page(vidx); }
    #[inline(always)]
    fn note_pc(&mut self, pc: u32) { self.periph.misc.cur_pc = pc; }
    fn fast_mem(&mut self) -> Option<FastMem> { if self.approximate_cache.is_some() && !self.approximate_cache_fast_internal { None } else { Some(FastMem { tlb: self.tlb.as_ptr(), page_ver: self.page_ver.as_mut_ptr() }) } }
    fn read_bulk(&mut self, addr: u32, out: &mut [u8]) -> bool {
        // Only a range inside one mapped entry with no peripheral behind it: exactly what the
        // per-word reads would return, without their per-word lookups or fault reporting.
        if Self::is_periph(addr) { return false; }
        let Some(e) = self.lookup(addr) else { return false };
        // Packed PIE falls back to its four word reads when external-cache timing
        // is enabled, so fills, hits and queued service are accounted for normally.
        if self.approximate_cache.is_some() && matches!(e.src as u8, SRC_FLASH | SRC_PSRAM) { return false; }
        if u64::from(addr) + out.len() as u64 > u64::from(e.hi) { return false; }
        let o = e.off as usize + (addr - e.lo) as usize;
        match self.buf(e.src as u8).get(o..o + out.len()) { Some(bytes) => { out.copy_from_slice(bytes); true } None => false }
    }
    fn take_timing_penalty(&mut self) -> u32 { self.take_approximate_cache_penalty() }
    fn add_timing_penalty(&mut self, cycles: u32) {
        self.approximate_cache_pending = self.approximate_cache_pending.saturating_add(cycles);
    }
    fn fast_cache(&mut self) -> Option<emu_core::bus::FastCache> {
        if self.approximate_cache_inline { self.approximate_cache.as_mut().and_then(|c| c.inline_view()) } else { None }
    }
    fn begin_timing_batch(&mut self, core: usize, now: u64) {
        self.cache_resource.core = core.min(1);
        self.cache_resource.cursor = now;
    }
    #[inline(always)]
    fn block_break(&self) -> bool { self.irq_dirty || (self.approximate_cache_yield_miss && self.approximate_cache_pending != 0) }
    #[inline(always)]
    fn defer_armed(&self) -> bool { self.defer_mmio }
    #[inline(always)]
    fn defer_access(&mut self, addr: u32) -> bool {
        // PIE ld.qr/st.qr add [-128,112] before accessing memory; MAC16 loads
        // add +/-4. Their conservative AR scan must also catch boundary crossings.
        if self.defer_mmio && (PERIPH_BASE - 128..PERIPH_END + 128).contains(&addr) {
            self.mmio_deferred = true; true
        } else { false }
    }
    #[inline(always)]
    fn deferred(&self) -> bool { self.mmio_deferred }
    fn code_page(&mut self, pc: u32) -> u32 {
        match self.lookup(pc) { Some(e) => e.vbase + ((pc - e.lo) >> VPAGE_SHIFT), None => self.page_ver.len() as u32 - 1 }
    }
    /// Returns 1 when interrupt inputs may have changed. Device time can advance
    /// without requesting a full interrupt-source scan.
    fn tick(&mut self, cycles: u32) -> u32 {
        self.cycles += cycles as u64;
        self.tick_pending += cycles;
        if self.tick_pending < self.tick_budget { return 0; }
        self.flush_ticks();
        u32::from(self.irq_dirty)
    }
}

impl SocBus {
    fn cadence_active(&self) -> bool {
        let p = &self.periph;
        p.i2s0.active() || p.i2s1.active()
            || p.lcd_cam.cam_active() || p.lcd_cam.lcd_running()
            // EX157: a running GDMA IN channel is passive. Its only producers are the camera
            // (capture or VSYNC enabled), AES (dma_pending) and mem-to-mem (needs the OUT side running),
            // all listed here, so an armed-but-idle receive channel does not hold the cadence.
            || p.gdma.out.iter().any(|c| c.running)
            || p.wifi.link.ap().is_some() || p.wifi.link.net().is_some() || !p.wifi.tx_pending.is_empty()
            || p.aes.dma_pending || p.sha.dma_pending
            || p.spi2.has_pending_transfer() || p.spi2.dma_tx_pending.is_some()
            || p.spi3.has_pending_transfer() || p.spi3.dma_tx_pending.is_some()
            || p.rmt.ch.iter().any(|c| c.running) || !p.rmt.done.is_empty()
            || !p.gpio.changes.is_empty() || !p.gpio.input_changes.is_empty()
            || p.rtc.ram.read(0x98) & (1 << 31) != 0
            || p.usb.int_ena & (1 << 1) != 0
    }

    /// Refresh the cached deadline after host-side device configuration changes.
    /// Pending elapsed cycles are retained and count toward the new threshold.
    pub fn refresh_tick_budget(&mut self) {
        let cap = if self.cadence_active() { MAX_TICK_DEFER } else { QUIET_TICK_DEFER };
        let mut budget = self.periph.cycles_until_timer().clamp(1, cap);
        if let Some((deadline, _)) = &self.spi2_scheduled {
            let until = u64::from(self.tick_pending).saturating_add(deadline.saturating_sub(self.cycles));
            budget = budget.min(until.clamp(1, u64::from(MAX_TICK_DEFER)) as u32);
        }
        if let Some(deadline) = self.board.next_deadline() {
            let until_deadline = u64::from(self.tick_pending)
                .saturating_add(deadline.saturating_sub(self.cycles))
                .clamp(1, u64::from(cap));
            budget = budget.min(until_deadline as u32);
        }
        self.tick_budget = budget;
    }

    /// Bus cycles at which the current peripheral clock started (deferred ticks excluded).
    pub(crate) fn peripheral_clock_offset(&self) -> u64 {
        self.cycles - u64::from(self.tick_pending) - self.periph.clock_cycles()
    }

    /// Deliver the deferred cycles to the device models now.
    pub fn flush_ticks(&mut self) {
        let c = std::mem::take(&mut self.tick_pending);
        if c == 0 { return; }
        self.tick_impl(c);
        self.refresh_tick_budget();
    }

    /// `flush_ticks` for a caller that refreshes the tick budget itself before anything reads
    /// it (`tick` and `next_deadline` are its only readers).
    #[inline]
    fn flush_ticks_unbudgeted(&mut self) {
        let c = std::mem::take(&mut self.tick_pending);
        if c != 0 { self.tick_impl(c); }
    }

    fn tick_impl(&mut self, cycles: u32) -> u32 {
        // Board time advances before peripheral callbacks; inputs are delivered after them.
        if self.board_edges { self.board.advance_to(self.cycles); }
        // Reads may flush before the periodic backstop. Refresh for either edge
        // of a clocked source, without breaking every block that polls MMIO.
        self.irq_dirty |= self.periph.tick(cycles as u64);
        if self.uart_pins {
            for input in self.board.uart_rx(self.cycles) {
                self.periph.uart_pin_input(&input);
                self.irq_dirty = true;
            }
        }
        if self.board_edges { self.irq_dirty |= esp_soc::gpio::drain_board_inputs(&mut *self.board, &mut self.periph.gpio, &mut self.gpio_events, self.cycles); }
        self.complete_spi2_dma();
        self.deliver_spi2_transfer();
        self.complete_spi3_dma();
        self.deliver_spi3_transfer();
        self.dma_i2s_step(cycles as u64);
        self.dma_cam_step(cycles as u64);
        self.dma_lcd_step(cycles as u64);
        self.dma_m2m_step();
        if !self.periph.wifi.tx_pending.is_empty() { self.wifi_tx_step(); }
        if self.periph.aes.dma_pending { self.aes_dma_step(); }
        if self.periph.sha.dma_pending { self.sha_dma_step(); }
        if self.periph.wifi.link.ap().is_some() { self.wifi_air_step(); }
        // the S3 runs its network even without an access point (the C3 and C6 only with one); nothing
        // reaches it then, but the host sockets are still read
        if self.periph.wifi.link.net().is_some() { let now_us = self.now_us(); self.periph.wifi.link.net_step(now_us); }
        if !self.periph.gpio.changes.is_empty() {
            let ch = &self.periph.gpio.changes;
            if let Some(ev) = &mut self.gpio_events { for &(pin, level) in ch { ev.push((self.cycles, pin, level)); } }
            self.board.gpio_output_at(self.cycles, ch, self.periph.gpio.enable, self.periph.gpio.out);
            self.periph.gpio.changes.clear();
        }
        self.deliver_spi2_transfer();
        self.deliver_spi3_transfer();
        if !self.periph.rmt.done.is_empty() { self.deliver_rmt_frames(); }
        0
    }
    /// Every routed pin of a completed transmitter receives its frame (mirrored strips).
    #[cold]
    #[inline(never)]
    fn deliver_rmt_frames(&mut self) {
        for (ch, bits) in std::mem::take(&mut self.periph.rmt.done) {
            for pin in esp_soc::pins::ChipPins::S3.routes(&self.periph.gpio, &self.periph.io_mux).output_pins(RMT_SIG_OUT0 + ch as u32) { self.board.rmt_frame(pin, &bits); }
        }
        self.irq_dirty = true;
    }
}
#[cfg(test)]
#[path = "bus/cache_tests.rs"]
mod cache_tests;
#[cfg(test)]
#[path = "bus/tests.rs"]
mod gp_spi_board_tests;
#[cfg(test)]
#[path = "bus/dma_tests.rs"]
mod dma_tests;

#[cfg(test)]
#[path = "bus/memory_origin_tests.rs"]
mod memory_origin_regressions;

#[path = "bus/pins.rs"]
mod pins;
