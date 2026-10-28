//! S31 PSRAM MSPI2/3 register configuration, distinct from Flash MSPI0/1.
//! Fixed ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! spi_mem_s_reg.h,spi1_mem_s_reg.h and psram_ctrlr_ll.h.
//! AP Memory APS256XXN-OBRx Rev1.00 (2020-07-24),sections6.1/6.2/6.7 and tables20/21.
//! Unknown serial commands and absent memory devices do not return manufactured data.
use emu_core::Fault;
pub const BASE: u32 = 0x2050_2000;
const SYSTEM_CONFIG: &[(u32, u32, u32)] = &[
    (0x8, 0x802c200c, 0xc1bc637f),
    (0xc, 0x28c00000, 0x27c00003),
    (0x10, 0x2c21, 0x7f007fff),
    (0x14, 0x30103, 0x80ffffff),
    (0x18, 0x0, 0x240002c0),
    (0x1c, 0x5c000047, 0xfc00003f),
    (0x20, 0x70000000, 0xf000ffff),
    (0x2c, 0x0, 0xfff0000),
    (0x34, 0x70, 0x670),
    (0x3c, 0xc0000000, 0xc00fffff),
    (0x40, 0x55c070, 0xfffcfff),
    (0x44, 0x80c00000, 0xcfffdfff),
    (0x48, 0x0, 0xf000ffff),
    (0x4c, 0x0, 0xf000ffff),
    (0x50, 0x30103, 0x80ffffff),
    (0x54, 0x200, 0x1fff80),
    (0xd4, 0x3020, 0x7d3fffff),
    (0xd8, 0x3020, 0x7d3fffff),
    (0xdc, 0x0, 0x3fff),
    (0x174, 0x80000, 0x1e0000),
    (0x17c, 0x0, 0xfff),
    (0x180, 0x1, 0x3f),
    (0x184, 0x0, 0x7ffffff),
    (0x188, 0x0, 0x3ffff),
    (0x18c, 0x0, 0x1ff),
    (0x190, 0x2a81, 0x7fbf),
    (0x194, 0x0, 0x7ffffff),
    (0x198, 0x0, 0x3ffff),
    (0x19c, 0x0, 0x1ff),
    (0x1a0, 0xb084, 0xfe01ffff),
    (0x1a4, 0x0, 0x7ffffff),
    (0x1a8, 0x0, 0x3ffff),
    (0x1ac, 0x0, 0x1ff),
    (0x200, 0x1, 0x3),
];
const PERIPHERAL_CONFIG: &[(u32, u32, u32)] = &[
    (0x4, 0x0, 0xffffffff),
    (0x8, 0x2ca00c, 0x1fcef7c),
    (0xc, 0x3ffffc, 0x3fffff),
    (0x10, 0x0, 0x0),
    (0x14, 0x30103, 0x80ffffff),
    (0x18, 0x80000000, 0xff00f200),
    (0x1c, 0x5c000007, 0xfc00003f),
    (0x20, 0x70000000, 0xf000ffff),
    (0x24, 0x0, 0x3ff),
    (0x28, 0x0, 0x3ff),
    (0x2c, 0x0, 0xfff0000),
    (0x34, 0x2, 0x603),
    (0x3c, 0x0, 0x1fa),
    (0xd4, 0x20, 0x7d3fcfff),
    (0x180, 0x0, 0x1e),
    (0x200, 0x1, 0x1),
];

const SYSTEM_IRQ_MASK: u32 = 0xfc0023f8; // Fixed spi_mem_s_reg.h:3..9,13,26..31.
pub struct PsramController { regs: [[u32; 256]; 2], mode: Option<[u8; 6]>, raw: u32, system_raw: u32, system_ena: u32, pub commands: u64, pub command_counts: [u64; 4], pub last_command: Option<u32> }
impl Default for PsramController {
    fn default() -> Self {
        let mut p = Self { regs: [[0; 256]; 2], mode: None, raw: 0, system_raw: 0, system_ena: 0, commands: 0, command_counts: [0; 4], last_command: None }; p.reset(); p
    }
}
impl PsramController {
    fn config(port: usize) -> &'static [(u32, u32, u32)] { if port == 0 { SYSTEM_CONFIG } else { PERIPHERAL_CONFIG } }
    pub fn reset(&mut self) {
        self.regs = [[0; 256]; 2]; self.raw = 0; self.system_raw = 0; self.system_ena = 0; self.last_command = None; // controller reset leaves the external die intact
        for p in 0..2 { for &(off, reset, _) in Self::config(p) { self.regs[p][off as usize / 4] = reset; } }
    }
    /// Fixed S31 mspi_periph.c assigns source112 only to system MSPI2. MSPI3
    /// has irq=-1; do not invent an additional line or OR in its local raw bank.
    pub fn irq(&self) -> bool { self.system_raw & self.system_ena != 0 }
    /// Only a real rejected AXI translation raises these address-error events.
    /// Cache/power gating, absent backing or unsupported encryption do not.
    pub(crate) fn invalid_axi_mapping(&mut self, write: bool) { self.system_raw |= if write { 1 << 9 } else { 1 << 7 }; }
    pub fn read(&self, port: usize, off: u32) -> Result<u32, Fault> {
        if port > 1 { return Err(Fault::Unmapped); }
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if Self::config(port).iter().any(|x| x.0 == off) || port == 1 && (0x58..=0x94).contains(&off) { return Ok(self.regs[port][off as usize / 4]); }
        match (port, off) {
            (_, 0) => Ok(0), // no accepted pending transaction
            (0, 0x178) => Ok(0xfc000000), // queues are empty in the synchronous subset
            (0, 0xc0) => Ok(self.system_ena), (0, 0xc8) => Ok(self.system_raw), (0, 0xcc) => Ok(self.system_raw & self.system_ena),
            (1, 0xc8) => Ok(self.raw), (1, 0xcc | 0xc0) | (_, 0xc4) => Ok(0),
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, port: usize, off: u32, value: u32, memory: &mut [u8]) -> Result<(), Fault> {
        if port > 1 { return Err(Fault::Unmapped); }
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if let Some(&(_, _, mask)) = Self::config(port).iter().find(|x| x.0 == off) {
            // ECC encoding and instruction cache-path encryption are not modeled.
            if port == 0 && (off == 0x1a0 && value & (1 << 16) != 0 || off == 0x174 && value & (1 << 20) != 0) { return Err(Fault::Prohibited); }
            let r = &mut self.regs[port][off as usize / 4]; *r = (*r & !mask) | (value & mask); return Ok(());
        }
        if port == 1 && (0x58..=0x94).contains(&off) { self.regs[port][off as usize / 4] = value; return Ok(()); }
        match off {
            0 if value == 0 => Ok(()),
            0 => { self.last_command = Some(value); self.execute(port, value, memory) }
            0xc0 if port == 0 => { self.system_ena = value & SYSTEM_IRQ_MASK; Ok(()) }
            0xc0 if value == 0 => Ok(()),
            0xc0 => Err(Fault::Prohibited), // no demonstrated MSPI3 interrupt route
            0xc4 if port == 1 => { self.raw &= !value; Ok(()) }
            0xc4 if port == 0 => { self.system_raw &= !(value & SYSTEM_IRQ_MASK); Ok(()) }
            0xc8 | 0xcc => Ok(()), // RAW is SS/WTC through CLR; ST is read-only
            _ => Err(Fault::Unmapped),
        }
    }
    /// Explicit AP-compatible MR0,MR1,MR2,MR3,MR4,MR8 responses. No identity or
    /// capacity is inferred from the separately supplied backing memory.
    pub fn attach_mode(&mut self, mode: [u8; 6]) -> Result<(), Fault> {
        if mode[0] & 0xc0 != 0 || mode[5] & 0xb0 != 0 { return Err(Fault::Prohibited); }
        self.mode = Some(mode); Ok(())
    }
    pub fn mode(&self) -> Option<[u8; 6]> { self.mode }
    pub fn memory_ready(&self, write: bool) -> Result<(), Fault> {
        // The older isolated MMU fixture can still use explicit raw backing
        // without a serial die. An attached die must use configured MSPI2.
        let Some(mode) = self.mode else { return Ok(()); };
        let r = &self.regs[0]; let command = if write { 0x8080 } else { 0 }; let phase = if write { 1 << 20 } else { 1 << 5 };
        let hex = if write { 1 << 27 } else { 1 << 26 }; let oct = if write { 1 << 19 } else { 1 << 18 };
        if r[0x3c / 4] & 0x80000001 != 1 || r[0x40 / 4] & (phase | 1 | 1 << 21) != phase | 1 | 1 << 21
            || (r[0x40 / 4] >> 14) & 63 != 31 || r[if write { 0x4c / 4 } else { 0x48 / 4 }] != 0xf0000000 | command
            || r[0xd8 / 4] & 0x6000001d != 1 || r[0x44 / 4] & (oct | 1 << 20 | 1 << 21) != oct | 1 << 20 | 1 << 21
            || (r[0x44 / 4] & hex != 0) != (mode[5] & 0x40 != 0) { return Err(Fault::Prohibited); }
        Ok(())
    }
    /// AP Memory APS256XXN-OBRx Rev1.00,2020-07-24,section6.7/figure12:
    /// reads repeat a pair; notably MA4 is MR4/MR8 and MA8 is MR8/MR0.
    fn mode_index(address: u32) -> Result<usize, Fault> {
        match address { 0..=4 => Ok(address as usize), 8 => Ok(5), _ => Err(Fault::Prohibited) }
    }
    fn execute(&mut self, port: usize, value: u32, memory: &mut [u8]) -> Result<(), Fault> {
        if port != 1 || value != 1 << 18 { return Err(Fault::Prohibited); }
        let r = &self.regs[1]; let ctrl = r[8 / 4]; let user = r[0x18 / 4]; let user1 = r[0x1c / 4]; let user2 = r[0x20 / 4];
        let ddr = r[0xd4 / 4]; let read = user & (1 << 28) != 0; let write = user & (1 << 27) != 0;
        // 16-bit OPI command,32-bit address,CS1,DDR on eight command/address lines.
        // Alternate address packing,byte swapping,CRC,HyperBus and full-duplex are absent.
        if user & 0xc0000000 != 0xc0000000 || user2 >> 28 != 15 || user1 >> 26 != 31
            || ctrl & 0x3f0 != 0x270 || ctrl & 0xc00 != 0 || ddr & 0x6000001d != 1
            || r[0x34 / 4] & 0x403 != 1 || read == write { return Err(Fault::Prohibited); }
        let mode = self.mode.ok_or(Fault::Prohibited)?;
        let bits = r[if read { 0x28 / 4 } else { 0x24 / 4 }] + 1;
        let high = user & (1 << if read { 24 } else { 25 }) != 0;
        let start = if high { 32 } else { 0 }; let count = (bits / 8) as usize;
        if !bits.is_multiple_of(8) || count == 0 || start + count > 64 { return Err(Fault::Prohibited); }
        let command = user2 as u16; let address = r[1];
        let mut data = [0u8; 64];
        for (i, byte) in data.iter_mut().take(count).enumerate() { let n = start + i; *byte = (r[0x58 / 4 + n / 4] >> ((n % 4) * 8)) as u8; }
        match command {
            0x4040 if read => {
                if user & (1 << 29) == 0 { return Err(Fault::Prohibited); }
                let index = Self::mode_index(address)?;
                for (i, byte) in data.iter_mut().take(count).enumerate() { *byte = mode[(index + i % 2) % 6]; }
            }
            0xc0c0 if write => {
                if user & (1 << 29) != 0 { return Err(Fault::Prohibited); }
                // Figure13 writes one addressed MR byte. Subsequent bus data is
                // don't-care,not sequential writes to the read-only paired register.
                let index = Self::mode_index(address)?; let mask = match address { 0 => 0x3f, 4 => 0xff, 8 => 0x4f, _ => return Err(Fault::Prohibited) };
                if data[0] & !mask != 0 || count > 2 { return Err(Fault::Prohibited); }
                let mut next = mode; next[index] = data[0]; self.mode = Some(next);
            }
            0x0000 | 0x2020 if read => {
                if user & (1 << 29) == 0 { return Err(Fault::Prohibited); }
                let offsets = Self::memory_offsets(mode, address, count, command == 0x2020, true, memory.len())?;
                for (i, byte) in data.iter_mut().take(count).enumerate() { *byte = memory[offsets[i]]; }
            }
            0x8080 | 0xa0a0 if write => {
                if user & (1 << 29) == 0 { return Err(Fault::Prohibited); }
                let offsets = Self::memory_offsets(mode, address, count, command == 0xa0a0, false, memory.len())?;
                for (i, &byte) in data.iter().take(count).enumerate() { memory[offsets[i]] = byte; }
            }
            _ => return Err(Fault::Prohibited),
        }
        if read { for (i, &byte) in data.iter().take(count).enumerate() {
            let n = start + i; let word = &mut self.regs[1][0x58 / 4 + n / 4]; let shift = (n % 4) * 8;
            *word = (*word & !(255 << shift)) | (u32::from(byte) << shift);
        } }
        self.raw |= 0x18; self.commands = self.commands.wrapping_add(1);
        let kind = match command { 0x4040 => 0, 0xc0c0 => 1, 0x0000 | 0x2020 => 2, _ => 3 };
        self.command_counts[kind] = self.command_counts[kind].wrapping_add(1); Ok(())
    }
    // AP section6.1/6.2/table20. X16 addresses have ten column bits (CA10 is
    // ignored); rows retain their address position. Array bounds stay explicit.
    fn memory_offsets(mode: [u8; 6], address: u32, count: usize, linear: bool, read: bool, size: usize) -> Result<[usize; 64], Fault> {
        let wide = mode[5] & 0x40 != 0;
        if address & 1 != 0 || count > 64 || !read && count < if wide { 4 } else { 2 } { return Err(Fault::Prohibited); }
        let physical = if wide { (address & !2047) | ((address & 1023) << 1) } else { address } as usize;
        let page = physical & !2047; let column = physical & 2047;
        let span = if linear || mode[5] & 3 == 3 { 2048 } else { (16 << (mode[5] & 3)) * if wide { 2 } else { 1 } };
        let block = column & !(span - 1); let hybrid = mode[5] & 4 != 0 && !linear;
        let cross = linear && read && mode[3] & 0x80 != 0 && mode[5] & 8 != 0;
        let mut result = [0; 64];
        for (i, offset) in result.iter_mut().take(count).enumerate() {
            *offset = if cross { physical.checked_add(i).ok_or(Fault::Unmapped)? }
                else if hybrid && i >= span { page + ((block + i) % 2048) }
                else { page + block + ((column % span + i) % span) };
            if *offset >= size { return Err(Fault::Unmapped); }
        }
        Ok(result)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    fn controller() -> PsramController {
        let mut p = PsramController::default(); p.attach_mode([0x0c, 0x0d, 0xdf, 0x80, 0x20, 5]).unwrap(); p
    }
    fn command(p: &mut PsramController, memory: &mut [u8], code: u16, address: u32, count: u32, read: bool) -> Result<(), Fault> {
        for (off, value) in [(8, 0x2ca27c), (4, address), (0x18, if read { 0xf0000000 } else if matches!(code, 0x8080 | 0xa0a0) { 0xe8000000 } else { 0xc8000000 }),
            (0x1c, 0x7c00000b), (0x20, 0xf0000000 | u32::from(code)), (0xd4, 0x21), (0x34, 1),
            (if read { 0x28 } else { 0x24 }, count * 8 - 1)] { p.write(1, off, value, memory).unwrap(); }
        p.write(1, 0, 1 << 18, memory)
    }
    #[test]
    fn mode_reads_repeat_documented_pairs_and_writes_do_not_touch_the_second_register() {
        let mut p = controller(); let mut memory = vec![0; 4096];
        command(&mut p, &mut memory, 0x4040, 4, 4, true).unwrap(); assert_eq!(p.read(1, 0x58), Ok(0x05200520));
        command(&mut p, &mut memory, 0x4040, 8, 4, true).unwrap(); assert_eq!(p.read(1, 0x58), Ok(0x0c050c05));
        p.write(1, 0x58, 0xff40, &mut memory).unwrap(); command(&mut p, &mut memory, 0xc0c0, 4, 2, false).unwrap();
        assert_eq!(p.mode().unwrap(), [0x0c, 0x0d, 0xdf, 0x80, 0x40, 5]);
        let before = p.mode(); let count = p.commands;
        assert_eq!(command(&mut p, &mut memory, 0xc0c0, 2, 2, false), Err(Fault::Prohibited));
        assert_eq!(p.mode(), before); assert_eq!(p.commands, count);
        assert_eq!(p.read(1, 0xc8), Ok(0x18)); p.write(1, 0xc4, 8, &mut memory).unwrap(); assert_eq!(p.read(1, 0xc8), Ok(0x10));
        p.reset(); assert_eq!(p.mode(), before); assert_eq!(p.read(1, 0xc8), Ok(0)); assert_eq!(p.read(0, 0x1a0), Ok(0xb084));
    }
    #[test]
    fn serial_memory_uses_actual_backing_with_x16_addressing_and_burst_wrap() {
        let mut p = controller(); let mut memory = vec![0; 4096];
        p.write(1, 0x58, 0x1234abcd, &mut memory).unwrap(); command(&mut p, &mut memory, 0x8080, 4, 4, false).unwrap();
        assert_eq!(&memory[4..8], &[0xcd, 0xab, 0x34, 0x12]);
        p.write(1, 0x58, 0, &mut memory).unwrap(); command(&mut p, &mut memory, 0x0000, 4, 4, true).unwrap(); assert_eq!(p.read(1, 0x58), Ok(0x1234abcd));
        p.write(1, 0x58, 0x4b, &mut memory).unwrap(); command(&mut p, &mut memory, 0xc0c0, 8, 2, false).unwrap();
        p.write(1, 0x58, 0xdeadbeef, &mut memory).unwrap(); command(&mut p, &mut memory, 0x8080, 2, 4, false).unwrap();
        assert_eq!(&memory[4..8], &[0xef, 0xbe, 0xad, 0xde]);
        assert_eq!(command(&mut p, &mut memory, 0x8080, 2, 2, false), Err(Fault::Prohibited));
        let mode = p.mode().unwrap(); let offsets = PsramController::memory_offsets(mode, 1022, 8, false, false, 4096).unwrap();
        assert_eq!(&offsets[..8], &[2044, 2045, 2046, 2047, 0, 1, 2, 3]);
        let offsets = PsramController::memory_offsets(mode, 1022, 8, true, true, 4096).unwrap();
        assert_eq!(&offsets[..8], &[2044, 2045, 2046, 2047, 2048, 2049, 2050, 2051]);
        assert_eq!(PsramController::memory_offsets(mode, 1022, 8, true, true, 2048), Err(Fault::Unmapped));
    }
    #[test]
    fn malformed_or_absent_device_commands_do_not_invent_status_or_modify_data() {
        let mut p = PsramController::default(); let mut memory = vec![0x55; 16];
        assert_eq!(command(&mut p, &mut memory, 0x4040, 0, 2, true), Err(Fault::Prohibited)); assert_eq!(p.commands, 0);
        p.attach_mode([0x0c, 0x0d, 0xdf, 0x80, 0x20, 5]).unwrap();
        assert_eq!(command(&mut p, &mut memory, 0x8080, 14, 4, false), Err(Fault::Unmapped));
        assert_eq!(memory, vec![0x55; 16]); assert_eq!(p.read(1, 0xc8), Ok(0));
        assert_eq!(p.write(0, 0x1a0, 1 << 16, &mut memory), Err(Fault::Prohibited));
        assert_eq!(p.read(0, 0x1a0), Ok(0xb084)); assert_eq!(p.read(1, u32::MAX), Err(Fault::Misaligned));
        assert_eq!(p.write(0, 0x174, 0x1c0000, &mut memory), Err(Fault::Prohibited)); assert_eq!(p.read(0, 0x174), Ok(0x80000));
        assert_eq!(p.read(0, 0xc8), Ok(0));
        assert_eq!(command(&mut p, &mut memory, 0x4040, 6, 2, true), Err(Fault::Prohibited));
        assert_eq!(command(&mut p, &mut memory, 0x4040, 0, 65, true), Err(Fault::Prohibited));
    }
    #[test]
    fn system_irq_masks_clear_reset_and_peripheral_raw_are_independent() {
        let mut p = controller(); let mut memory = vec![0; 16];
        command(&mut p, &mut memory, 0x4040, 0, 2, true).unwrap(); assert_eq!(p.read(1, 0xc8), Ok(0x18)); assert_eq!(p.read(0, 0xc8), Ok(0));
        p.write(0, 0xc0, u32::MAX, &mut memory).unwrap(); assert_eq!(p.read(0, 0xc0), Ok(SYSTEM_IRQ_MASK)); assert!(!p.irq());
        p.write(0, 0xc8, u32::MAX, &mut memory).unwrap(); assert_eq!(p.read(0, 0xc8), Ok(0));
        p.invalid_axi_mapping(false); p.invalid_axi_mapping(true); assert!(p.irq()); assert_eq!(p.read(0, 0xcc), Ok(0x280));
        p.write(0, 0xc8, 0x280, &mut memory).unwrap(); assert_eq!(p.read(0, 0xcc), Ok(0x280));
        p.write(0, 0xc4, 0x80, &mut memory).unwrap(); assert_eq!(p.read(0, 0xc8), Ok(0x200)); assert_eq!(p.read(0, 0xc4), Ok(0));
        p.write(1, 0xc8, 0x18, &mut memory).unwrap(); assert_eq!(p.read(1, 0xc8), Ok(0x18));
        p.write(1, 0xc4, 0x18, &mut memory).unwrap(); assert_eq!(p.read(1, 0xc8), Ok(0)); assert_eq!(p.read(0, 0xc8), Ok(0x200));
        p.write(0, 0xc0, 0, &mut memory).unwrap(); assert!(!p.irq()); assert_eq!(p.read(0, 0xc8), Ok(0x200));
        assert_eq!(p.write(1, 0xc0, 0x18, &mut memory), Err(Fault::Prohibited)); let mode = p.mode(); p.reset(); assert_eq!(p.mode(), mode); assert_eq!(p.read(0, 0xc8), Ok(0));
    }
    #[test]
    fn attached_device_requires_its_real_axi_configuration() {
        let mut p = controller(); let mut memory = vec![0; 16]; assert_eq!(p.memory_ready(false), Err(Fault::Prohibited));
        for (off, value) in [(0x3c, 1), (0x40, 0x37c021), (0x44, 0x3c0000), (0x48, 0xf0000000), (0x4c, 0xf0008080), (0xd8, 0x3021)] { p.write(0, off, value, &mut memory).unwrap(); }
        assert_eq!(p.memory_ready(false), Ok(())); assert_eq!(p.memory_ready(true), Ok(()));
        p.write(0, 0x3c, 0x80000001, &mut memory).unwrap(); assert_eq!(p.memory_ready(false), Err(Fault::Prohibited));
    }
}
