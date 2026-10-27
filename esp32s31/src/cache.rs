//! Functional S31 external-memory translation, without cache lines or timing.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/{register/soc/{cache,spi_mem_c,spi_mem_s}_reg.h,include/soc/ext_mem_defs.h}
//! and hal/esp32s31/include/hal/mmu_ll.h. Only hart0 buses execute.
use emu_core::Fault;

pub const CACHE_BASE: u32 = 0x2c00_0000;
pub const FLASH_MMU: u32 = 0x2050_0000;
pub const PSRAM_MMU: u32 = 0x2050_2000;
pub const FLASH_VADDR: u32 = 0x4000_0000;
pub const PSRAM_VADDR: u32 = 0x5000_0000;
const ENTRIES: usize = 1024;

struct Mmu { entries: [u32; ENTRIES], index: u32, power: u32 }
impl Default for Mmu {
    fn default() -> Self {
        // Deterministic invalid table, not a claim about uninitialized physical MMU SRAM.
        // The header's CONTENT port reset value 892 does not specify all SRAM entries.
        Self { entries: [0; ENTRIES], index: 0, power: 4896 << 16 }
    }
}

impl Mmu {
    fn page_size(&self, psram: bool) -> usize { (if psram { 65536 } else { 262144 }) >> ((self.power >> 3) & 3) }
    fn read(&self, off: u32) -> Result<u32, Fault> {
        match off {
            0x37c => self.entries.get(self.index as usize).copied().ok_or(Fault::Unmapped),
            0x380 => Ok(self.index), 0x384 => Ok(self.power), _ => Err(Fault::Unmapped),
        }
    }
    fn write(&mut self, off: u32, value: u32, psram: bool) -> Result<(), Fault> {
        match off {
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
    flash_mmu: Mmu, psram_mmu: Mmu, flash: Vec<u8>, psram: Vec<u8>,
}
impl Default for Cache {
    fn default() -> Self { Self { icache: 0, dcache: 0, bypass: 0, atomic: 1,
        flash_mmu: Mmu::default(), psram_mmu: Mmu::default(), flash: Vec::new(), psram: Vec::new() } }
}
impl Cache {
    /// Explicit development input, not a ROM boot shortcut or an assertion of product capacity.
    /// Allocation limits are host software bounds. The SPI1 command subset uses this same backing array.
    pub fn set_flash(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 256 * 1024 * 1024 { return Err("S31 development flash input exceeds 256 MiB".into()); }
        let mut data = Vec::new(); data.try_reserve_exact(bytes.len()).map_err(|e| e.to_string())?;
        data.extend_from_slice(bytes); self.flash = data; Ok(())
    }
    pub(crate) fn flash_bytes_mut(&mut self) -> &mut [u8] { &mut self.flash }
    pub fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> {
        if bytes > 64 * 1024 * 1024 { return Err("S31 development PSRAM input exceeds 64 MiB".into()); }
        let mut data = Vec::new(); data.try_reserve_exact(bytes).map_err(|e| e.to_string())?;
        data.resize(bytes, 0); self.psram = data; Ok(())
    }
    pub fn is_memory(address: u32) -> bool { (FLASH_VADDR..0x5400_0000).contains(&address) }
    pub fn is_mmio(address: u32) -> bool {
        (CACHE_BASE..CACHE_BASE + 0x1000).contains(&address)
            || (FLASH_MMU + 0x37c..FLASH_MMU + 0x388).contains(&address)
            || (PSRAM_MMU + 0x37c..PSRAM_MMU + 0x388).contains(&address)
    }
    pub fn read_register(&self, address: u32) -> Result<u32, Fault> {
        match address.checked_sub(CACHE_BASE) {
            Some(0) => return Ok(self.icache), Some(4) => return Ok(self.dcache),
            Some(8) => return Ok(self.bypass), Some(12) => return Ok(self.atomic), _ => {}
        }
        if (FLASH_MMU..FLASH_MMU + 0x1000).contains(&address) { return self.flash_mmu.read(address - FLASH_MMU); }
        if (PSRAM_MMU..PSRAM_MMU + 0x1000).contains(&address) { return self.psram_mmu.read(address - PSRAM_MMU); }
        Err(Fault::Unmapped)
    }
    pub fn write_register(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        match address.checked_sub(CACHE_BASE) {
            Some(0) => { self.icache = value & 0xff0f; return Ok(()); }
            Some(4) => { self.dcache = value & 0xff1f; return Ok(()); }
            Some(8) => { self.bypass = value & 0x13; return Ok(()); }
            Some(12) => { self.atomic = value & 1; return Ok(()); }
            _ => {}
        }
        if (FLASH_MMU..FLASH_MMU + 0x1000).contains(&address) { return self.flash_mmu.write(address - FLASH_MMU, value, false); }
        if (PSRAM_MMU..PSRAM_MMU + 0x1000).contains(&address) { return self.psram_mmu.write(address - PSRAM_MMU, value, true); }
        Err(Fault::Unmapped)
    }
    fn physical(&self, address: u32, fetch: bool, write: bool) -> Result<(bool, usize), Fault> {
        if (if fetch { self.icache } else { self.dcache }) & 1 != 0 { return Err(Fault::Prohibited); }
        if (FLASH_VADDR..PSRAM_VADDR).contains(&address) {
            if write { return Err(Fault::Prohibited); }
            self.flash_mmu.translate((address - FLASH_VADDR) as usize, false, self.flash.len()).map(|o| (false, o))
        } else if (PSRAM_VADDR..0x5400_0000).contains(&address) {
            self.psram_mmu.translate((address - PSRAM_VADDR) as usize, true, self.psram.len()).map(|o| (true, o))
        } else { Err(Fault::Unmapped) }
    }
    pub fn read_byte(&self, address: u32, fetch: bool) -> Result<u8, Fault> {
        let (psram, off) = self.physical(address, fetch, false)?;
        Ok(if psram { self.psram[off] } else { self.flash[off] })
    }
    pub fn write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if bytes > 4 { return Err(Fault::Unmapped); }
        // Validate the entire access before changing any bytes, including discontiguous pages.
        let mut offsets = [0; 4];
        for (i, off) in offsets.iter_mut().take(bytes).enumerate() {
            *off = self.physical(address.checked_add(i as u32).ok_or(Fault::Unmapped)?, false, true)?.1;
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
}
