//! S31 memory, UART0, reset causes and CLIC/matrix windows; other MMIO faults explicitly.
use emu_core::{Bus, Fault};
use esp_periph::uart::{Uart, UartLayout};
use crate::interrupts::{Interrupts, MATRIX_BASE, CLIC_BASE, RESET_CAUSE0, RESET_CAUSE1};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessFault { pub pc: u32, pub address: u32, pub bytes: usize, pub write: bool, pub fetch: bool, pub reason: Fault }
pub struct S31Bus {
    sram: Vec<u8>, lp: Vec<u8>, rom: Vec<u8>, pub uart0: Uart,
    pub interrupts: Interrupts,
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
        let layout = UartLayout { thrhd_mask: 0xff, rxfifo_rst: 1 << 22, rxfifo_cnt_mask: 0xff };
        Self { sram: vec![0; SRAM_SIZE], lp: vec![0; LP_SIZE], rom: vec![0; ROM_SIZE],
            uart0: Uart::new(layout), interrupts: Interrupts::default(), pc: 0, first_fault: None }
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
    pub fn refresh_irq(&mut self, threshold: u32) -> Option<u32> { self.interrupts.refresh(self.uart0.irq(), threshold) }
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
        if matches!(address & !3, RESET_CAUSE0 | RESET_CAUSE1) {
            if bytes != 4 || address & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            return Ok(self.interrupts.reset_read(((address - RESET_CAUSE0) / 4) as usize));
        }
        if let Some(off) = offset(address, MATRIX_BASE, bytes, 0x1000) {
            if bytes != 4 || off & 3 != 0 { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if let Some(value) = self.interrupts.matrix_read(off >> 11, (off & 0x7ff) as u32, self.uart0.irq()) { return Ok(value); }
            return self.fault(address, bytes, false, false, Fault::Unmapped);
        }
        if let Some((core, off)) = Self::clic_offset(address, bytes) {
            if !address.is_multiple_of(bytes as u32) { return self.fault(address, bytes, false, false, Fault::Misaligned); }
            if !Self::clic_register(off) { return self.fault(address, bytes, false, false, Fault::Unmapped); }
            let word = self.interrupts.clic[core].read(off & !3);
            return Ok((word >> ((off & 3) * 8)) & ((1u64 << (bytes * 8)) - 1) as u32);
        }
        if let Some(o) = offset(address, UART0_BASE, bytes, UART_SIZE) {
            if bytes == 4 && o.is_multiple_of(4) { return Ok(self.uart0.read(o as u32)); }
            return self.fault(address, bytes, false, false, Fault::Misaligned);
        }
        self.fault(address, bytes, false, false, Fault::Unmapped)
    }
    fn write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if let Some(dest) = self.memory_mut(address, bytes, false) { dest.copy_from_slice(&value.to_le_bytes()[..bytes]); return Ok(()); }
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
            if bytes == 4 && o.is_multiple_of(4) { self.uart0.write(o as u32, value); return Ok(()); }
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
        if self.memory(pc, 2).is_none() { return self.fault(pc, 4, false, true, Fault::Unmapped); }
        let mut data = [0; 4];
        for (i, b) in data.iter_mut().enumerate() { if let Some(src) = self.memory(pc.wrapping_add(i as u32), 1) { *b = src[0]; } }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        for address in [0, u32::MAX, 0x4ff0_0000, 0x4fc0_0000, 0x500c_a000, 0x4000_0000] { assert_eq!(b.read32(address), Err(Fault::Unmapped)); }
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
