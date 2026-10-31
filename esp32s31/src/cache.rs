//! Functional S31 external-memory translation, without cache lines or timing.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/{register/soc/{cache,spi_mem_c,spi_mem_s}_reg.h,include/soc/ext_mem_defs.h}
//! and hal/esp32s31/include/hal/{mmu,cache}_ll.h. Each hart has an I-cache
//! input/clock; both use D-cache bus0, as cache_ll_l1_{enable,disable}_bus specifies.
use emu_core::Fault;

pub const CACHE_BASE: u32 = 0x2c00_0000;
pub const FLASH_MMU: u32 = 0x2050_0000;
pub const PSRAM_MMU: u32 = 0x2050_2000;
pub const FLASH_VADDR: u32 = 0x4000_0000;
pub const PSRAM_VADDR: u32 = 0x5000_0000;
pub const CLOCK_CTRL: u32 = 0x2058_7038;
pub const MMU_LP_CTRL: u32 = 0x2058_902c;
const ENTRIES: usize = 1024;

struct Mmu { entries: [u32; ENTRIES], index: u32, power: u32, lp_ctrl: u32 }
impl Default for Mmu {
    fn default() -> Self {
        // Deterministic invalid table, not a claim about uninitialized physical MMU SRAM.
        // The header's CONTENT port reset value 892 does not specify all SRAM entries.
        Self { entries: [0; ENTRIES], index: 0, power: 4896 << 16, lp_ctrl: 0 }
    }
}

impl Mmu {
    // v6.1 hp_alive_sys_reg.h: LP_EN bit2, MODE[1:0], FORCE_CTRL bit3.
    // No PMU transition is synthesized. Explicit software power-down blocks table
    // access/translation; retention after deep sleep is outside this active-run subset.
    fn lp_write(&mut self, value: u32) -> Result<(), Fault> {
        // Only active/light-retention/disabled-low-power states are specified here.
        // Deep sleep and shutdown retention/content are not invented.
        if matches!(value & 7, 4 | 6) { return Err(Fault::Prohibited); }
        self.lp_ctrl = value & 15; Ok(())
    }
    fn powered(&self) -> bool { self.lp_ctrl & 4 == 0 || self.lp_ctrl & 3 == 3 }
    fn page_size(&self, psram: bool) -> usize { (if psram { 65536 } else { 262144 }) >> ((self.power >> 3) & 3) }
    fn read(&self, off: u32) -> Result<u32, Fault> {
        match off {
            0x37c if !self.powered() => Err(Fault::Prohibited),
            0x37c => self.entries.get(self.index as usize).copied().ok_or(Fault::Unmapped),
            0x380 => Ok(self.index), 0x384 => Ok(self.power), _ => Err(Fault::Unmapped),
        }
    }
    fn write(&mut self, off: u32, value: u32, psram: bool) -> Result<(), Fault> {
        match off {
            0x37c if !self.powered() => return Err(Fault::Prohibited),
            0x37c => *self.entries.get_mut(self.index as usize).ok_or(Fault::Unmapped)? = value,
            0x380 => self.index = value,
            0x384 => {
                // FLASH AUX is HRO, PSRAM AUX is RW; ECO result is RO. No ECO test implemented.
                let mask = if psram { 0x7fff_0019 } else { 0x4000_0019 };
                self.power = (self.power & !mask) | (value & mask);
            }
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    fn translate(&self, relative: usize, psram: bool, size: usize) -> Result<usize, Fault> {
        if !self.powered() { return Err(Fault::Prohibited); }
        let page = self.page_size(psram);
        let entry = *self.entries.get(relative / page).ok_or(Fault::Unmapped)?;
        let (valid, sensitive, mask) = if psram { (1 << 11, 1 << 12, 0x3ff) } else { (1 << 12, 1 << 13, 0x7ff) };
        if entry & valid == 0 { return Err(Fault::Unmapped); }
        // Encrypted mappings and alternate PSRAM target settings are not modeled.
        if entry & sensitive != 0 || (psram && entry & (1 << 10) == 0) { return Err(Fault::Prohibited); }
        let physical = (entry as usize & mask) * page + relative % page;
        if physical >= size { return Err(Fault::Unmapped); }
        Ok(physical)
    }
}

pub struct Cache {
    icache: u32, dcache: u32, bypass: u32, atomic: u32,
    clock: u32, sync: [u32; 4], fail_check: u32, trace: u32, error_raw: u32, error_ena: u32, freeze: u32, pub maintenance_operations: u64,
    flash_mmu: Mmu, psram_mmu: Mmu, flash: Vec<u8>, psram: Vec<u8>,
}
impl Default for Cache {
    fn default() -> Self { Self { icache: 0, dcache: 0, bypass: 0, atomic: 1,
        clock: 0x15249, sync: [16, 31, 0, 0], fail_check: 0, trace: 0, error_raw: 0, error_ena: 0, freeze: 0, maintenance_operations: 1,
        flash_mmu: Mmu::default(), psram_mmu: Mmu::default(), flash: Vec::new(), psram: Vec::new() } }
}
impl Cache {
    // Fixed v6.1 HP_SYS_CLKRST_CACHE_CTRL0. The functional memory path is
    // write-through with no retained cache lines; invalidation/clean/writeback
    // therefore finish synchronously once the selected domains are usable.
    // Reset's INVALIDATE_ENA=1 completes over the initially empty cache at time0.
    fn domain_on(&self, clock_bit: u32, reset_bit: u32) -> bool { self.clock & (1 << clock_bit | 1 << reset_bit) == 1 << clock_bit }
    /// Fixed S31 cache_err_int.c and cache_ll.h: source69 reflects latched
    /// L1 failure bits masked by ENA. Turning the tracer off does not clear them.
    pub fn irq(&self) -> bool { self.error_raw & self.error_ena != 0 }
    fn failed_access(&mut self, address: u32, fetch: bool, hart: usize) {
        // Bounded, documented event: an actual request through a closed cache bus.
        // Missing host backing, freeze, power, encryption and unmodeled MMU errors
        // remain diagnostic faults; they are not mislabeled as hardware events.
        let shut = if fetch { self.icache >> hart } else { self.dcache } & 1 != 0;
        let clock = if fetch { 14 + hart as u32 * 2 } else { 12 };
        let frozen = self.freeze & (if fetch { 1 << (hart * 4) } else { 1 << 16 }) != 0;
        if (FLASH_VADDR..0x54000000).contains(&address) && self.trace & 1 != 0 && shut && !frozen
            && self.domain_on(9, 11) && self.domain_on(6, 8) && self.domain_on(clock, clock + 1) {
            self.error_raw |= if fetch { 1 << hart } else { 16 };
        }
    }
    pub fn clock_read(&self) -> u32 { self.clock }
    pub fn clock_write(&mut self, value: u32) {
        self.clock = value & 0x3ffff;
        if value & (1 << 15) != 0 { self.icache = 0; self.freeze &= !7; }
        if value & (1 << 17) != 0 { self.freeze &= !(7 << 4); }
        if value & (1 << 13) != 0 { self.dcache = 0; self.freeze &= !(7 << 16); }
    }
    fn freeze_write(&mut self, value: u32) -> Result<(), Fault> {
        let mut next = value & 0x30033;
        for (shift, clock_bit, reset_bit) in [(0, 14, 15), (4, 16, 17), (16, 12, 13)] {
            if next & (1 << shift) != 0 {
                // No retained cache lines exist. Mode0 freezes all further misses;
                // mode1's forced cache hit requires unavailable cache SRAM contents.
                if next & (2 << shift) != 0 || !self.domain_on(clock_bit, reset_bit) { return Err(Fault::Prohibited); }
                next |= 4 << shift; // No pending memory transactions in this synchronous model.
            }
        }
        self.freeze = next; Ok(())
    }
    fn sync_write(&mut self, value: u32) -> Result<(), Fault> {
        let operation = value & 15;
        if operation.count_ones() > 1 { return Err(Fault::Prohibited); }
        if operation != 0 {
            // Only the two actual instruction caches and data cache exist on S31.
            if self.sync[1] & !0x13 != 0 || self.sync[1] == 0 { return Err(Fault::Prohibited); }
            for (map_bit, clock_bit, reset_bit) in [(0, 14, 15), (1, 16, 17), (4, 12, 13)] {
                if self.sync[1] & (1 << map_bit) != 0 && !self.domain_on(clock_bit, reset_bit) { return Err(Fault::Prohibited); }
            }
            self.maintenance_operations = self.maintenance_operations.wrapping_add(1);
            self.sync[0] = (value & 0x1e0) | 16;
        } else { self.sync[0] = (self.sync[0] & 16) | (value & 0x1e0); }
        Ok(())
    }
    /// Explicit development input, not a ROM boot shortcut or an assertion of product capacity.
    /// Allocation limits are host software bounds. The SPI1 command subset uses this same backing array.
    pub fn set_flash(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 256 * 1024 * 1024 { return Err("S31 development flash input exceeds 256 MiB".into()); }
        let mut data = Vec::new(); data.try_reserve_exact(bytes.len()).map_err(|e| e.to_string())?;
        data.extend_from_slice(bytes); self.flash = data; Ok(())
    }
    /// Hand the NOR array (including SPI1 program/erase results) across a chip reset.
    pub(crate) fn take_flash(&mut self) -> Vec<u8> { std::mem::take(&mut self.flash) }
    pub(crate) fn flash_bytes_mut(&mut self) -> &mut [u8] { &mut self.flash }
    pub(crate) fn psram_bytes_mut(&mut self) -> &mut [u8] { &mut self.psram }
    /// Distinguish an invalid MMU address from absent development backing or an
    /// unsupported encrypted/alternate target. Only the former is AXI_ADDR_ERR.
    pub(crate) fn invalid_psram_mapping(&self, address: u32, bytes: usize) -> bool {
        if !self.psram_mmu.powered() { return false; }
        for i in 0..bytes.min(4) {
            let Some(a) = address.checked_add(i as u32) else { return false; };
            if !(PSRAM_VADDR..0x54000000).contains(&a) { return false; }
            let relative = (a - PSRAM_VADDR) as usize;
            let index = relative / self.psram_mmu.page_size(true);
            if self.psram_mmu.entries.get(index).is_none_or(|v| v & (1 << 11) == 0) { return true; }
            // Preserve first-failure ordering across page boundaries: a prior
            // absent backing/encrypted target is not the later invalid entry.
            if self.psram_mmu.translate(relative, true, self.psram.len()).is_err() { return false; }
        }
        false
    }
    pub fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> {
        if bytes > 64 * 1024 * 1024 { return Err("S31 development PSRAM input exceeds 64 MiB".into()); }
        let mut data = Vec::new(); data.try_reserve_exact(bytes).map_err(|e| e.to_string())?;
        data.resize(bytes, 0); self.psram = data; Ok(())
    }
    pub fn is_memory(address: u32) -> bool { (FLASH_VADDR..0x5400_0000).contains(&address) }
    pub fn is_mmio(address: u32) -> bool {
        (MMU_LP_CTRL..MMU_LP_CTRL + 8).contains(&address)
            || (CACHE_BASE..CACHE_BASE + 0x1000).contains(&address)
            || (FLASH_MMU + 0x37c..FLASH_MMU + 0x388).contains(&address)
            || (PSRAM_MMU + 0x37c..PSRAM_MMU + 0x388).contains(&address)
    }
    pub fn read_register(&self, address: u32) -> Result<u32, Fault> {
        if address == MMU_LP_CTRL { return Ok(self.flash_mmu.lp_ctrl); }
        if address == MMU_LP_CTRL + 4 { return Ok(self.psram_mmu.lp_ctrl); }
        match address.checked_sub(CACHE_BASE) {
            Some(0) => return Ok(self.icache), Some(4) => return Ok(self.dcache),
            Some(8) => return Ok(self.bypass), Some(12) => return Ok(self.atomic),
            Some(0x170) => return Ok(self.error_ena), Some(0x174) => return Ok(0),
            Some(0x178) => return Ok(self.error_raw), Some(0x17c) => return Ok(self.error_raw & self.error_ena),
            Some(0x3d8) => return Ok(self.trace),
            // Autoload is disabled at reset; no speculative fills are modeled.
            Some(0xac | 0xb8 | 0xdc | 0xe8 | 0xfc | 0x138) => return Ok(2),
            Some(0x8c) => return Ok(4), Some(0x30) => return Ok(self.freeze), _ => {}
        }
        if (CACHE_BASE + 0x9c..=CACHE_BASE + 0xa8).contains(&address) { return Ok(self.sync[((address - CACHE_BASE - 0x9c) / 4) as usize]); }
        if address == CACHE_BASE + 0x16c { return Ok(self.fail_check); }
        if (FLASH_MMU..FLASH_MMU + 0x1000).contains(&address) { return self.flash_mmu.read(address - FLASH_MMU); }
        if (PSRAM_MMU..PSRAM_MMU + 0x1000).contains(&address) { return self.psram_mmu.read(address - PSRAM_MMU); }
        Err(Fault::Unmapped)
    }
    pub fn write_register(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address == MMU_LP_CTRL { return self.flash_mmu.lp_write(value); }
        if address == MMU_LP_CTRL + 4 { return self.psram_mmu.lp_write(value); }
        match address.checked_sub(CACHE_BASE) {
            Some(0) => { self.icache = value & 0xff0f; return Ok(()); }
            Some(4) => { self.dcache = value & 0xff1f; return Ok(()); }
            Some(8) => { self.bypass = value & 0x13; return Ok(()); }
            Some(12) => { self.atomic = value & 1; return Ok(()); }
            Some(0x170) => { self.error_ena = value & 0x1f; return Ok(()); }
            Some(0x174) => { self.error_raw &= !(value & 0x1f); return Ok(()); } // WT clears the corresponding WTC field.
            Some(0x178) => return Ok(()), // R/WTC/SS is not directly writable; TRM access-type glossary.
            Some(0x17c) => return Ok(()), // RO, cannot create an interrupt.
            Some(0x3d8) => { self.trace = value & 1; return Ok(()); } // L2 bit1 is HRO; no L2 events.

            Some(0x30) => return self.freeze_write(value),
            Some(0x8c) => return if value & !4 == 0 { Ok(()) } else { Err(Fault::Prohibited) },
            Some(0xac | 0xb8 | 0xdc | 0xe8 | 0xfc | 0x138) => return if value & !2 == 0 { Ok(()) } else { Err(Fault::Prohibited) },
            Some(0x9c) => return self.sync_write(value),
            Some(0xa0) => { self.sync[1] = value & 63; return Ok(()); }
            Some(0xa4) => { self.sync[2] = value; return Ok(()); }
            Some(0xa8) => { self.sync[3] = value & 0x0fffffff; return Ok(()); }
            // ROM enables propagation on its two I-caches and D-cache. This diagnostic
            // runner still stops on unsupported accesses even before propagation is
            // enabled. Trace/IRQ reporting never supplies fabricated data on failure.
            Some(0x16c) => { self.fail_check = value & 0x13; return Ok(()); }
            _ => {}
        }
        if (FLASH_MMU..FLASH_MMU + 0x1000).contains(&address) { return self.flash_mmu.write(address - FLASH_MMU, value, false); }
        if (PSRAM_MMU..PSRAM_MMU + 0x1000).contains(&address) { return self.psram_mmu.write(address - PSRAM_MMU, value, true); }
        Err(Fault::Unmapped)
    }
    fn physical(&self, address: u32, fetch: bool, write: bool, hart: usize) -> Result<(bool, usize), Fault> {
        if hart >= 2 { return Err(Fault::Prohibited); }
        let iclock = 14 + hart as u32 * 2;
        if !self.domain_on(9, 11) || !self.domain_on(6, 8) || !self.domain_on(if fetch { iclock } else { 12 }, if fetch { iclock + 1 } else { 13 }) { return Err(Fault::Prohibited); }
        if self.freeze & (if fetch { 1 << (hart * 4) } else { 1 << 16 }) != 0 { return Err(Fault::Prohibited); }
        if (if fetch { self.icache >> hart } else { self.dcache }) & 1 != 0 { return Err(Fault::Prohibited); }
        if (FLASH_VADDR..PSRAM_VADDR).contains(&address) {
            if write { return Err(Fault::Prohibited); }
            self.flash_mmu.translate((address - FLASH_VADDR) as usize, false, self.flash.len()).map(|o| (false, o))
        } else if (PSRAM_VADDR..0x5400_0000).contains(&address) {
            self.psram_mmu.translate((address - PSRAM_VADDR) as usize, true, self.psram.len()).map(|o| (true, o))
        } else { Err(Fault::Unmapped) }
    }
    pub fn read_byte(&mut self, address: u32, fetch: bool) -> Result<u8, Fault> { self.read_byte_core(address, fetch, 0) }
    pub fn read_byte_core(&mut self, address: u32, fetch: bool, hart: usize) -> Result<u8, Fault> {
        let (psram, off) = match self.physical(address, fetch, false, hart) {
            Ok(v) => v, Err(e) => { if hart < 2 { self.failed_access(address, fetch, hart); } return Err(e); }
        };
        Ok(if psram { self.psram[off] } else { self.flash[off] })
    }
    pub fn write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if bytes > 4 { return Err(Fault::Unmapped); }
        // Validate the entire access before changing any bytes, including discontiguous pages.
        let mut offsets = [0; 4];
        for (i, off) in offsets.iter_mut().take(bytes).enumerate() {
            *off = match self.physical(address.checked_add(i as u32).ok_or(Fault::Unmapped)?, false, true, 0) {
                Ok(v) => v.1, Err(e) => { self.failed_access(address + i as u32, false, 0); return Err(e); }
            };
        }
        for (i, &off) in offsets.iter().take(bytes).enumerate() { self.psram[off] = value.to_le_bytes()[i]; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn map(c: &mut Cache, psram: bool, index: u32, value: u32) {
        let base = if psram { PSRAM_MMU } else { FLASH_MMU };
        c.write_register(base + 0x380, index).unwrap(); c.write_register(base + 0x37c, value).unwrap();
    }
    #[test]
    fn each_page_size_uses_its_own_table_and_physical_bounds() {
        for psram in [false, true] {
            for code in 0..4 {
                let mut c = Cache::default(); let base = if psram { PSRAM_MMU } else { FLASH_MMU };
                let vaddr = if psram { PSRAM_VADDR } else { FLASH_VADDR };
                let page = (if psram { 65536 } else { 262144 }) >> code;
                let valid = if psram { 0xc00 } else { 0x1000 };
                if psram { c.set_psram_size(2 * page).unwrap(); c.psram[page] = 0xa7; }
                else { let mut flash = vec![0; 2 * page]; flash[page] = 0xa7; c.set_flash(&flash).unwrap(); }
                c.write_register(base + 0x384, code << 3).unwrap();
                assert_eq!(c.read_byte(vaddr, false), Err(Fault::Unmapped));
                map(&mut c, psram, 0, valid | 1);
                assert_eq!(c.read_byte(vaddr, false), Ok(0xa7));
                assert_eq!(c.read_byte(vaddr + page as u32, false), Err(Fault::Unmapped));
                map(&mut c, psram, 1, valid); assert_eq!(c.read_byte(vaddr + page as u32, false), Ok(0));
                map(&mut c, psram, 0, valid | 2); assert_eq!(c.read_byte(vaddr, false), Err(Fault::Unmapped));
                map(&mut c, psram, 0, valid | if psram { 0x1000 } else { 0x2000 });
                assert_eq!(c.read_byte(vaddr, false), Err(Fault::Prohibited));
                c.write_register(base + 0x380, u32::MAX).unwrap();
                assert_eq!(c.read_register(base + 0x37c), Err(Fault::Unmapped));
                assert_eq!(c.write_register(base + 0x37c, 0), Err(Fault::Unmapped));
            }
        }
    }
    #[test]
    fn data_and_instruction_gates_do_not_alias_or_allow_flash_writes() {
        let mut c = Cache::default(); c.set_flash(&[1, 2, 3, 4]).unwrap(); map(&mut c, false, 0, 0x1000);
        c.write_register(CACHE_BASE, 1).unwrap();
        assert_eq!(c.read_byte(FLASH_VADDR, true), Err(Fault::Prohibited)); assert_eq!(c.read_byte(FLASH_VADDR, false), Ok(1));
        c.write_register(CACHE_BASE, 0).unwrap(); c.write_register(CACHE_BASE + 4, 1).unwrap();
        assert_eq!(c.read_byte(FLASH_VADDR, true), Ok(1)); assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited));
        c.write_register(CACHE_BASE + 4, 0).unwrap();
        assert_eq!(c.write(FLASH_VADDR, 0, 4), Err(Fault::Prohibited)); assert_eq!(c.read_byte(FLASH_VADDR, false), Ok(1));
        assert_eq!(c.read_register(CACHE_BASE + 0x10), Err(Fault::Unmapped));
    }
    #[test]
    fn functional_maintenance_obeys_domains_freeze_and_mmu_power() {
        let mut c = Cache::default(); c.set_flash(&[0xa7]).unwrap(); map(&mut c, false, 0, 0x1000);
        c.write_register(CACHE_BASE + 0xa0, 0x13).unwrap();
        c.write_register(CACHE_BASE + 0xa4, FLASH_VADDR).unwrap(); c.write_register(CACHE_BASE + 0xa8, 1).unwrap();
        for op in [1, 2, 4, 8] {
            c.write_register(CACHE_BASE + 0x9c, op).unwrap();
            assert_eq!(c.read_register(CACHE_BASE + 0x9c), Ok(16)); assert_eq!(c.read_byte(FLASH_VADDR, false), Ok(0xa7));
        }
        assert_eq!(c.maintenance_operations, 5);
        assert_eq!(c.write_register(CACHE_BASE + 0x9c, 3), Err(Fault::Prohibited));
        c.clock_write(c.clock & !(1 << 12));
        assert_eq!(c.write_register(CACHE_BASE + 0x9c, 1), Err(Fault::Prohibited));
        assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited)); assert_eq!(c.read_byte(FLASH_VADDR, true), Ok(0xa7));
        c.clock_write(0x15249); c.write_register(CACHE_BASE + 0x30, 1 << 16).unwrap();
        assert_eq!(c.read_register(CACHE_BASE + 0x30), Ok(5 << 16));
        assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited)); assert_eq!(c.read_byte(FLASH_VADDR, true), Ok(0xa7));
        assert_eq!(c.write_register(CACHE_BASE + 0x30, 3 << 16), Err(Fault::Prohibited));
        assert_eq!(c.read_register(CACHE_BASE + 0x30), Ok(5 << 16));
        c.write_register(CACHE_BASE + 0x30, 4 << 16).unwrap(); // RO DONE cannot keep freeze enabled.
        assert_eq!(c.read_register(CACHE_BASE + 0x30), Ok(0)); assert_eq!(c.read_byte(FLASH_VADDR, false), Ok(0xa7));
        c.write_register(MMU_LP_CTRL, 0xd).unwrap(); // Explicit light sleep, retain table but deny accesses.
        assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited));
        assert_eq!(c.read_register(FLASH_MMU + 0x37c), Err(Fault::Prohibited));
        c.write_register(MMU_LP_CTRL, 9).unwrap(); assert_eq!(c.read_byte(FLASH_VADDR, false), Ok(0xa7));
        for off in [0xac, 0xb8, 0xdc, 0xe8, 0xfc, 0x138] {
            assert_eq!(c.read_register(CACHE_BASE + off), Ok(2));
            assert_eq!(c.write_register(CACHE_BASE + off, 1), Err(Fault::Prohibited));
        }
        assert_eq!(c.write_register(CACHE_BASE + 0x8c, 1), Err(Fault::Prohibited));
    }
    #[test]
    fn a_cross_page_write_is_atomic_at_bus_boundary_and_can_be_discontiguous() {
        let mut c = Cache::default(); c.set_psram_size(3 * 8192).unwrap();
        c.write_register(PSRAM_MMU + 0x384, 3 << 3).unwrap(); map(&mut c, true, 0, 0xc02);
        assert_eq!(c.write(PSRAM_VADDR + 8190, 0x44332211, 4), Err(Fault::Unmapped));
        assert_eq!(c.psram[3 * 8192 - 2], 0);
        map(&mut c, true, 1, 0xc00); c.write(PSRAM_VADDR + 8190, 0x44332211, 4).unwrap();
        assert_eq!(&c.psram[3 * 8192 - 2..], &[0x11, 0x22]); assert_eq!(&c.psram[..2], &[0x33, 0x44]);
        map(&mut c, true, 0, 0x800); assert_eq!(c.read_byte(PSRAM_VADDR, false), Err(Fault::Prohibited));
        assert_eq!(c.set_psram_size(usize::MAX), Err("S31 development PSRAM input exceeds 64 MiB".into()));
    }
    #[test]
    fn per_core_instruction_inputs_and_clocks_share_one_data_path() {
        let mut c = Cache::default(); c.set_flash(&[0x6f, 0, 0, 0]).unwrap(); map(&mut c, false, 0, 0x1000);
        c.write_register(CACHE_BASE, 1).unwrap();
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 0), Err(Fault::Prohibited));
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 1), Ok(0x6f));
        c.write_register(CACHE_BASE, 2).unwrap();
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 0), Ok(0x6f));
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 1), Err(Fault::Prohibited));
        for core in 0..2 { assert_eq!(c.read_byte_core(FLASH_VADDR, false, core), Ok(0x6f)); }
        c.write_register(CACHE_BASE, 0).unwrap(); c.clock_write(c.clock_read() & !(1 << 16));
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 0), Ok(0x6f));
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 1), Err(Fault::Prohibited));
        c.clock_write(c.clock_read() | 1 << 16); c.write_register(CACHE_BASE + 0x30, 1 << 4).unwrap();
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 0), Ok(0x6f));
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 1), Err(Fault::Prohibited));
        c.write_register(CACHE_BASE + 4, 1).unwrap();
        for core in 0..2 { assert_eq!(c.read_byte_core(FLASH_VADDR, false, core), Err(Fault::Prohibited)); }
    }

    #[test]
    fn fail_tracer_latches_real_closed_bus_requests_and_wtc_cannot_forge_events() {
        let mut c = Cache::default(); c.set_flash(&[1, 0, 0, 0]).unwrap(); map(&mut c, false, 0, 0x1000);
        c.write_register(CACHE_BASE + 0x170, 31).unwrap(); c.write_register(CACHE_BASE, 3).unwrap();
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 1), Err(Fault::Prohibited)); assert!(!c.irq());
        c.write_register(CACHE_BASE + 0x3d8, 3).unwrap(); assert_eq!(c.read_register(CACHE_BASE + 0x3d8), Ok(1));
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 1), Err(Fault::Prohibited));
        assert_eq!(c.read_register(CACHE_BASE + 0x178), Ok(2)); assert!(c.irq());
        assert_eq!(c.read_byte_core(FLASH_VADDR, true, 0), Err(Fault::Prohibited));
        c.write_register(CACHE_BASE + 4, 1).unwrap(); assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited));
        assert_eq!(c.read_register(CACHE_BASE + 0x17c), Ok(19));
        c.write_register(CACHE_BASE + 0x170, 0).unwrap(); assert!(!c.irq());
        assert_eq!(c.read_register(CACHE_BASE + 0x178), Ok(19));
        c.write_register(CACHE_BASE + 0x170, 31).unwrap(); assert!(c.irq());
        c.write_register(CACHE_BASE + 0x3d8, 0).unwrap(); assert!(c.irq());
        c.write_register(CACHE_BASE + 0x174, 2).unwrap(); assert_eq!(c.read_register(CACHE_BASE + 0x178), Ok(17));
        c.write_register(CACHE_BASE + 0x178, 17).unwrap(); assert_eq!(c.read_register(CACHE_BASE + 0x178), Ok(17));
        c.write_register(CACHE_BASE + 0x174, 17).unwrap(); assert!(!c.irq());
        c.write_register(CACHE_BASE + 0x17c, 31).unwrap(); assert_eq!(c.read_register(CACHE_BASE + 0x178), Ok(0));
        assert_eq!(c.read_register(CACHE_BASE + 0x214), Err(Fault::Unmapped)); // Requester ID/ATTR not guessed.
    }
    #[test]
    fn missing_backing_or_frozen_cache_does_not_invent_closed_bus_error() {
        let mut c = Cache::default(); c.write_register(CACHE_BASE + 0x3d8, 1).unwrap();
        c.write_register(CACHE_BASE + 0x170, 31).unwrap(); map(&mut c, false, 0, 0x1000);
        assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Unmapped)); assert!(!c.irq());
        c.set_flash(&[1]).unwrap(); c.write_register(CACHE_BASE + 0x30, 1 << 16).unwrap();
        assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited)); assert!(!c.irq());
        c.write_register(CACHE_BASE + 4, 1).unwrap();
        assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited)); assert!(!c.irq());
        c.write_register(CACHE_BASE + 0x30, 0).unwrap(); c.clock_write(c.clock_read() & !(1 << 12));
        assert_eq!(c.read_byte(FLASH_VADDR, false), Err(Fault::Prohibited)); assert!(!c.irq());
    }

}
