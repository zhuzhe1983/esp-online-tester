//! S31 interrupt-matrix routing for UART0 and the documented power-on reset-cause registers.
use esp_periph::clic::{ClicCore, CLIC_IDS, CLIC_EXT_OFFSET};

// ESP-IDF v6.1 soc/esp32s31/include/soc/{interrupts,clic_reg}.h and
// register/soc/{reg_base,interrupt_core0_reg,lp_clkrst_reg}.h. The other core's
// matrix is +0x800. Only hart 0 executes in this bring-up stage.
pub const MATRIX_BASE: u32 = 0x2058_5000;
pub const CLIC_BASE: u32 = 0x1080_0000;
pub const RESET_CAUSE0: u32 = 0x2070_1030;
pub const RESET_CAUSE1: u32 = 0x2070_1034;
pub const UART0_SOURCE: usize = 9;
const SOURCES: usize = 169;

pub struct Interrupts {
    map: [[u32; SOURCES]; 2], clock: [u32; 2], date: [u32; 2],
    pub clic: [ClicCore; 2], reset_cause: [u32; 2],
}
impl Default for Interrupts {
    fn default() -> Self {
        // HPCOREn_RESET_CAUSE[6:1] = 1 for POR. RESET_FLAG retains its documented reset 0;
        // no software-reset, watchdog or power-domain state machine is implied here.
        Self { map: [[0; SOURCES]; 2], clock: [0; 2], date: [38_834_448; 2],
            clic: [ClicCore::default(), ClicCore::default()], reset_cause: [1 << 1; 2] }
    }
}
impl Interrupts {
    pub fn reset_read(&self, core: usize) -> u32 { self.reset_cause[core] }
    pub fn reset_write(&mut self, core: usize, value: u32) {
        // Cause and flag are RO; only the two write-triggered clear bits affect them.
        if value & (1 << 30) != 0 { self.reset_cause[core] &= !(0x3f << 1); }
        if value & (1 << 31) != 0 { self.reset_cause[core] &= !1; }
    }
    pub fn matrix_read(&self, core: usize, off: u32, uart: bool) -> Option<u32> {
        match off {
            0x000..=0x2a0 => Some(self.map[core][off as usize / 4]),
            0x2a8 => Some(if uart { 1 << UART0_SOURCE } else { 0 }),
            0x2ac..=0x2bc => Some(0), // Other, unimplemented sources are not asserted.
            0x2fc => Some(self.clock[core]), 0x7fc => Some(self.date[core]),
            _ => None,
        }
    }
    pub fn matrix_write(&mut self, core: usize, off: u32, value: u32) -> bool {
        match off {
            // Unlike the old P4, S31 also has PASS_LEVEL bits 9:8. Zero disables
            // remapping, one is S-mode (not implemented), two/three select M-mode.
            0x000..=0x2a0 => self.map[core][off as usize / 4] = value & 0x33f,
            0x2a8..=0x2bc => {}, // source status is read-only
            0x2fc => self.clock[core] = value & 1, 0x7fc => self.date[core] = value & 0x0fff_ffff,
            _ => return false,
        }
        true
    }
    pub fn refresh(&mut self, uart: bool, threshold: u32) -> Option<u32> {
        for core in 0..2 {
            let route = self.map[core][UART0_SOURCE]; let id = (route & 0x3f) as usize;
            let m_mode = (route >> 8) & 3 >= 2;
            let input = if uart && m_mode && (CLIC_EXT_OFFSET as usize..CLIC_IDS).contains(&id) { 1u64 << id } else { 0 };
            self.clic[core].set_input(input);
        }
        // Standard S31 threshold is the executing hart's CSR, not P4's MMIO register.
        self.clic[0].thresh = (threshold & 0xff) << 24;
        self.clic[0].candidate().and_then(|(id, level)| {
            if !(CLIC_EXT_OFFSET as usize..CLIC_IDS).contains(&id) { return None; }
            let line = id as u32 - CLIC_EXT_OFFSET;
            Some(line | level << riscv_rv32::exec::CLIC_IRQ_LEVEL_SHIFT | if self.clic[0].vectored(id) { riscv_rv32::exec::CLIC_IRQ_SHV } else { 0 })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s31_requires_machine_pass_level_and_strictly_higher_priority() {
        let mut i = Interrupts::default();
        i.clic[0].write(0x1000 + 4 * 17, 1 << 8 | 1 << 29, 0xf);
        for pass in 0..=3 {
            i.matrix_write(0, 4 * UART0_SOURCE as u32, pass << 8 | 17);
            assert_eq!(i.refresh(true, 0x1f).is_some(), pass >= 2);
        }
        assert_eq!(i.refresh(true, 0x3f), None);
        assert!(i.refresh(true, 0x1f).is_some()); assert_eq!(i.refresh(false, 0x1f), None);
        for id in [0, 15, 48, 63] { i.matrix_write(0, 4 * UART0_SOURCE as u32, 0x200 | id); assert_eq!(i.refresh(true, 0x1f), None); }
    }
    #[test]
    fn reset_cause_is_read_only_except_for_documented_clear_triggers() {
        let mut i = Interrupts::default(); assert_eq!(i.reset_read(0), 2);
        i.reset_write(0, !(3 << 30)); assert_eq!(i.reset_read(0), 2);
        i.reset_write(0, 1 << 30); assert_eq!(i.reset_read(0), 0); assert_eq!(i.reset_read(1), 2);
    }
}
