//! Classic external data RAM translation and synchronous write-through cache contract.
//! ESP-IDF v4.4.7 and v5.1.4 components/esp_hw_support/port/esp32/cache_sram_mmu.c:
//! PID 0/1 use table index 1152, 32 KiB pages (shift 15), at 0x3f800000.
//! Matched live PRO/APP tables in normal mode are an emulator qualification
//! restriction, not silicon behavior. Low/high mode shares PRO-low/APP-high
//! mappings between CPUs (ESP32 TRM v5.8 §4.3.2.2, tables 4.3-16/18).
use crate::bus::SocBus;
use xtensa_lx7::bus::Fault;

pub(crate) const LOW: u32 = 0x3f80_0000;
pub(crate) const HIGH: u32 = 0x3fc0_0000;
const SRAM_SLOT: usize = 1152;
const SRAM_SHIFT: u32 = 15;
const SRAM_PAGE: usize = 1 << SRAM_SHIFT;

impl SocBus {
    // Resolve the whole access before touching bytes, including noncontiguous page crossings.
    fn psram_offsets<const N: usize>(&self, addr: u32) -> Result<[usize; N], Fault> {
        let mut out = [0; N];
        for (i, off) in out.iter_mut().enumerate() {
            let a = addr.checked_add(i as u32).filter(|a| (LOW..HIGH).contains(a)).ok_or(Fault::Unmapped)?;
            let slot = SRAM_SLOT + ((a - LOW) >> SRAM_SHIFT) as usize;
            let lowhigh = self.psram_lowhigh()?;
            // ESP32 TRM v5.8 §4.3.2.2, tables 4.3-16/18: both CPUs use
            // PRO slots 0..63 below 2 MiB and APP slots 64..127 above it.
            // The unused entries need not match in low/high mode.
            let core = usize::from(lowhigh && a >= LOW + (2 << 20));
            let entry = self.mmu[core][slot];
            let (clock, reset, stall) = self.periph.dport.core1_control();
            let app_live = clock && !reset && !stall;
            if entry & 0x100 != 0 || (!lowhigh && app_live && entry != self.mmu[1][slot]) { return Err(Fault::Unmapped); }
            if lowhigh { self.psram_half_gate(core)?; }
            *off = (entry as usize & 255) * SRAM_PAGE + (a as usize & (SRAM_PAGE - 1));
            if *off >= self.psram.len() { return Err(Fault::Unmapped); }
        }
        Ok(out)
    }
    fn psram_lowhigh(&self) -> Result<bool, Fault> {
        let pro = self.periph.dport.ram.read(0x40);
        let app = self.periph.dport.ram.read(0x58);
        let lowhigh = pro & (1 << 16) != 0;
        let (clock, reset, stall) = self.periph.dport.core1_control();
        // IDF v4.4.7 spiram_psram.c:1090-1105 programs both modes together.
        // Mixed modes are not qualified; SPLIT (even/odd) remains unsupported.
        if pro & (1 << 11) != 0 || ((lowhigh || (clock && !reset && !stall))
            && (app & (1 << 11) != 0 || lowhigh != (app & (1 << 14) != 0))) {
            return Err(Fault::Prohibited);
        }
        Ok(lowhigh)
    }
    fn psram_half_gate(&self, core: usize) -> Result<(), Fault> {
        // TRM v5.8 §4.3.2.2: disabling APP cache removes only the upper
        // half in HL mode, for either CPU. IDF v4.4.7 dport_reg.h and
        // spiram_psram.c:1100-1105: enable=3, DRAM1/OPSDRAM masks=3/5,
        // 32 KiB page mode=0. Requiring BOTH masks clear on the selected
        // cache is a conservative qualification restriction (no mask routing claim).
        let ctrl = self.periph.dport.ram.read(if core == 0 { 0x40 } else { 0x58 });
        let ctrl1 = self.periph.dport.ram.read(if core == 0 { 0x44 } else { 0x5c });
        if ctrl & 8 == 0 || ctrl1 & ((7 << 6) | 0x28) != 0 { return Err(Fault::Prohibited); }
        Ok(())
    }
    fn psram_cache_gate(&self, write: bool) -> Result<(), Fault> {
        // Normal mode keeps the P4 matched-live-cache qualification unchanged.
        let (clock, reset, stall) = self.periph.dport.core1_control();
        let cores = if self.psram_lowhigh()? { 0 } else if clock && !reset && !stall { 2 } else { 1 };
        for core in 0..cores { self.psram_half_gate(core)?; }
        if !self.periph.spi1.0.classic_qpi_enabled() || !self.periph.spi0.sram_cache_ready(write) {
            return Err(Fault::Prohibited);
        }
        Ok(())
    }
    #[cold]
    pub(crate) fn psram_load<const N: usize>(&mut self, addr: u32) -> Result<[u8; N], Fault> {
        match self.psram_offsets::<N>(addr).and_then(|o| self.psram_cache_gate(false).map(|()| o)) {
            Ok(offsets) => Ok(offsets.map(|o| self.psram[o])),
            Err(f) => { self.last_fault = Some((addr, false)); Err(f) }
        }
    }
    #[cold]
    pub(crate) fn psram_store(&mut self, addr: u32, bytes: &[u8]) -> Result<(), Fault> {
        match bytes.len() {
            1 => self.psram_store_width::<1>(addr, bytes.try_into().unwrap()),
            2 => self.psram_store_width::<2>(addr, bytes.try_into().unwrap()),
            4 => self.psram_store_width::<4>(addr, bytes.try_into().unwrap()),
            _ => unreachable!("bus store width"),
        }
    }
    fn psram_store_width<const N: usize>(&mut self, addr: u32, bytes: &[u8; N]) -> Result<(), Fault> {
        match self.psram_offsets::<N>(addr).and_then(|o| self.psram_cache_gate(true).map(|()| o)) {
            Ok(offsets) => {
                for (off, byte) in offsets.into_iter().zip(bytes) { self.psram[off] = *byte; }
                Ok(())
            }
            Err(f) => { self.last_fault = Some((addr, true)); Err(f) }
        }
    }
    pub(crate) fn reset_psram(&mut self) {
        self.psram.fill(0);
        for table in &mut self.mmu { table[SRAM_SLOT..SRAM_SLOT + 128].fill(0x100); }
    }
    pub(crate) fn configure_psram(&mut self, bytes: usize) -> Result<(), String> {
        if bytes != 0 && bytes != (4 << 20) { return Err("classic PSRAM supports 0 or 4194304 bytes".into()); }
        // Stopped-machine boundary only. Classic has no fast_mem or published backing pointer.
        self.reset_psram();
        self.psram = vec![0; bytes];
        Ok(())
    }
}

#[cfg(test)]
#[path = "psram_tests.rs"]
mod tests;
