//! S31 memory windows and UART0; every other MMIO access remains an explicit fault.
use emu_core::{Bus, Fault};
use esp_periph::uart::{Uart, UartLayout};

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
            uart0: Uart::new(layout), pc: 0, first_fault: None }
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
    fn read(&mut self, address: u32, bytes: usize) -> Result<u32, Fault> {
        if let Some(data) = self.memory(address, bytes) {
            let mut value = [0; 4]; value[..bytes].copy_from_slice(data); return Ok(u32::from_le_bytes(value));
        }
        if let Some(o) = offset(address, UART0_BASE, bytes, UART_SIZE) {
            if bytes == 4 && o.is_multiple_of(4) { return Ok(self.uart0.read(o as u32)); }
            return self.fault(address, bytes, false, false, Fault::Misaligned);
        }
        self.fault(address, bytes, false, false, Fault::Unmapped)
    }
    fn write(&mut self, address: u32, value: u32, bytes: usize) -> Result<(), Fault> {
        if let Some(dest) = self.memory_mut(address, bytes, false) { dest.copy_from_slice(&value.to_le_bytes()[..bytes]); return Ok(()); }
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
}
