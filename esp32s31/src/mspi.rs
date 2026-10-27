//! S31 SPI0 cache interface and SPI1 command access to explicitly described quad NOR input.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643: spi_mem_c_reg.h,
//! spi1_mem_c_reg.h, hp_sys_clkrst_reg.h and spimem_flash_ll.h. The validated subset
//! reuses SpiMem's array/standard command implementation, not its S3 register map,
//! implicit identity, PSRAM branch, permissive unknown commands or automatic QE default.
use emu_core::Fault;
use esp_periph::spi_mem::SpiMem;
pub const BASE: u32 = 0x2050_0000;
pub const CLOCK: u32 = 0x2058_7064;
const SPI0_CONFIG: &[(u32, u32, u32)] = &[
    (8, 0x802c200c, 0x81bc610e), (0xc, 0x28c00000, 0x27c00003), (0x10, 0x2c21, 0x7f0003ff),
    (0x14, 0x30103, 0x80ffffff), (0x18, 0, 0x240002c0), (0x1c, 0x5c000047, 0xfc00003f),
    (0x20, 0x70000000, 0xf000ffff), (0x34, 0x70, 0x600), (0x3c, 0xc0000000, 0xc00001ff), (0xd4, 0x3020, 0x3011),
];
const SPI1_CONFIG: &[(u32, u32, u32)] = &[
    (4, 0, u32::MAX), (8, 0x2ca00c, 0x01fce10c), (0xc, 0x3ffffc, 0x3fffff), (0x10, 0, 0),
    (0x14, 0x30103, 0x80ffffff), (0x18, 0x80000000, 0xfc00f200), (0x1c, 0x5c000007, 0xfc00003f),
    (0x20, 0x70000000, 0xf000ffff), (0x24, 0, 0x3ff), (0x28, 0, 0x3ff), (0x34, 2, 0x603),
    (0x3c, 0, 0x1fa), (0x9c, 0x08002000, u32::MAX), (0xa0, 0x57575, u32::MAX), (0xa4, 0x7a7a0000, 0xffff80ff), (0xd4, 0x20, 0),
];
pub struct Mspi { spi0: [u32; 64], spi1: SpiMem, clock: u32, descriptor: Option<([u8; 3], u16)>, read_status: u32, pub commands: u64 }
impl Default for Mspi {
    fn default() -> Self {
        let mut m = Self { spi0: [0; 64], spi1: SpiMem::new(true), clock: 0x781, descriptor: None, read_status: 0, commands: 0 };
        m.reset_registers(); m
    }
}
impl Mspi {
    fn reset_registers(&mut self) {
        self.spi0.fill(0); for &(off, reset, _) in SPI0_CONFIG { self.spi0[off as usize / 4] = reset; }
        self.spi1 = SpiMem::new(true); self.spi1.has_psram = false; self.read_status = 0;
        let (id, status) = self.descriptor.unwrap_or(([0; 3], 0)); self.spi1.jedec = id; self.spi1.status = status as u32;
        for &(off, reset, _) in SPI1_CONFIG { self.spi1.regs.write(off, reset); }
    }
    /// Caller-supplied decoded device response, never a product-specific default.
    /// This subset supports ordinary 8-bit quad-NOR commands only; no OPI/security/SFDP.
    pub fn attach_flash(&mut self, jedec: [u8; 3], status: u16) -> Result<(), Fault> {
        if status & !0x202 != 0 { return Err(Fault::Prohibited); } // No BUSY/protection/security state synthesized.
        self.descriptor = Some((jedec, status)); self.spi1.jedec = jedec; self.spi1.status = status as u32; Ok(())
    }
    pub fn clock_read(&self) -> u32 { self.clock }
    pub fn clock_write(&mut self, value: u32) -> Result<(), Fault> {
        // XTAL root0 is the only modeled source. PLL/RC and force-no-reset are explicit gaps.
        if value & 0x78 != 0 { return Err(Fault::Prohibited); }
        self.clock = value & 0x1ffff;
        if value & 4 != 0 {
            let status = self.spi1.status; self.reset_registers(); self.spi1.status = status; // APB reset does not reset the external NOR die.
        }
        Ok(())
    }
    fn bus_on(&self) -> bool { self.clock & 5 == 1 }
    pub fn flash_accessible(&self) -> bool { self.clock & 0x107 == 0x101 && self.spi0[0x3c / 4] & 0x80000001 == 1 }
    pub fn read(&self, port: usize, off: u32) -> Result<u32, Fault> {
        if port > 1 { return Err(Fault::Unmapped); }
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.bus_on() { return Err(Fault::Prohibited); }
        if port == 0 {
            if SPI0_CONFIG.iter().any(|x| x.0 == off) { return Ok(self.spi0[off as usize / 4]); }
            return match off {
                0 | 0x54 => Ok(0), // Every supported access completes synchronously; no outstanding transaction.
                0x178 => Ok(0xfc000000), // All documented FIFO-empty bits: this subset has no queued AXI requests.
                _ => Err(Fault::Unmapped),
            };
        }
        if SPI1_CONFIG.iter().any(|x| x.0 == off) { return Ok(self.spi1.regs.read(off)); }
        match off { 0x2c => Ok(self.read_status), 0 | 0x54 | 0x58..=0x94 => Ok(self.spi1.read(off)), _ => Err(Fault::Unmapped) }
    }
    pub fn write(&mut self, port: usize, off: u32, value: u32, flash: &mut [u8]) -> Result<(), Fault> {
        if port > 1 { return Err(Fault::Unmapped); }
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.bus_on() { return Err(Fault::Prohibited); }
        if port == 0 {
            if let Some(&(_, _, mask)) = SPI0_CONFIG.iter().find(|x| x.0 == off) {
                let r = &mut self.spi0[off as usize / 4]; *r = (*r & !mask) | (value & mask); return Ok(());
            }
            return match off { 0 if value == 0 => Ok(()), 0x54 | 0x178 => Ok(()), _ => Err(Fault::Unmapped) };
        }
        if let Some(&(_, _, mask)) = SPI1_CONFIG.iter().find(|x| x.0 == off) {
            // Enabling suspend/resume machinery is not silently treated as completed.
            if off == 0x9c && value & 0x3f != 0 { return Err(Fault::Prohibited); }
            let old = self.spi1.regs.read(off); self.spi1.regs.write(off, (old & !mask) | (value & mask)); return Ok(());
        }
        match off {
            0 => self.execute(value, flash),
            0x58..=0x94 => { self.spi1.write(off, value); Ok(()) }
            0x2c | 0x54 => Err(Fault::Prohibited),
            _ => Err(Fault::Unmapped),
        }
    }
    fn execute(&mut self, value: u32, flash: &mut [u8]) -> Result<(), Fault> {
        if value == 0 { return Ok(()); }
        if self.clock & 0x105 != 0x101 || self.descriptor.is_none() || flash.is_empty() { return Err(Fault::Prohibited); }
        if self.spi1.regs.read(0x34) & 3 != 2 { return Err(Fault::Prohibited); } // Exactly the explicit CS0 device.
        // No multiple commands, writes/erase, auto-suspend, power-down or unknown commands
        // are sent into the older shared model's permissive fallbacks.
        let allowed = (1 << 31) | (1 << 30) | (1 << 29) | (1 << 28) | (1 << 27) | (1 << 18);
        if value & !allowed != 0 || value.count_ones() != 1 { return Err(Fault::Prohibited); }
        let user = self.spi1.regs.read(0x18); let user1 = self.spi1.regs.read(0x1c); let user2 = self.spi1.regs.read(0x20);
        let miso_bits = (self.spi1.regs.read(0x28) & 0x3ff) + 1;
        if miso_bits > 512 || (self.spi1.regs.read(0x24) & 0x3ff) > 511 { return Err(Fault::Prohibited); }
        let mut read_bytes = None;
        if value == 1 << 18 {
            let command = user2 as u8;
            if user & (1 << 31) == 0 || user2 >> 28 != 7 || user & (1 << 27) != 0 { return Err(Fault::Prohibited); }
            if !matches!(command, 0x03 | 0x0b | 0x3b | 0x6b | 0xbb | 0xeb | 0x9f | 0x05 | 0x35 | 0x06 | 0x04) { return Err(Fault::Prohibited); }
            let array_read = matches!(command, 0x03 | 0x0b | 0x3b | 0x6b | 0xbb | 0xeb);
            let returns_data = !matches!(command, 0x06 | 0x04);
            if (user & (1 << 30) != 0) != array_read || (user & (1 << 28) != 0) != returns_data { return Err(Fault::Prohibited); }
            // Only byte-aligned reads and the ordinary 24-bit quad-NOR address protocol
            // are covered. Do not silently accept 4-byte commands or partial data phases.
            if array_read && user1 >> 26 != 23 || returns_data && !miso_bits.is_multiple_of(8) { return Err(Fault::Prohibited); }
            if matches!(command, 0x6b | 0xeb) && self.spi1.status & 0x200 == 0 { return Err(Fault::Prohibited); }
            if returns_data { read_bytes = Some((miso_bits / 8) as usize); }
        } else if value == 1 << 31 {
            if user1 >> 26 != 23 || !miso_bits.is_multiple_of(8) { return Err(Fault::Prohibited); }
            read_bytes = Some((miso_bits / 8) as usize);
        }
        self.spi1.pending_cmd = value; self.spi1.execute(flash, &mut []);
        if value == 1 << 27 { self.read_status = self.spi1.status; }
        if let Some(n) = read_bytes {
            // The reused RDID/status helpers fill whole values; only bytes actually
            // clocked by this controller may become visible in its receive buffer.
            for i in n..64 { self.spi1.w[i / 4] &= !(0xff << ((i % 4) * 8)); }
        }
        self.commands += 1; Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controller_gates_reset_and_external_die_state_are_separate() {
        let mut m = Mspi::default(); let mut flash = vec![0xaa; 128];
        assert!(!m.flash_accessible()); m.write(0, 0x3c, 0x40000001, &mut flash).unwrap(); assert!(m.flash_accessible());
        m.clock_write(0x681).unwrap(); assert!(!m.flash_accessible());
        assert_eq!(m.write(1, 0, 1 << 28, &mut flash), Err(Fault::Prohibited));
        m.clock_write(0x781).unwrap(); assert_eq!(m.write(1, 0, 1 << 28, &mut flash), Err(Fault::Prohibited));
        m.attach_flash([0xef, 0x40, 0x18], 0).unwrap(); m.write(1, 0, 1 << 30, &mut flash).unwrap();
        assert_eq!(m.read(1, 0x2c), Ok(0)); // RDSR is a transfer, not a live alias of the external die.
        m.write(1, 0, 1 << 27, &mut flash).unwrap(); assert_eq!(m.read(1, 0x2c), Ok(2));
        m.clock_write(0x785).unwrap(); assert!(!m.flash_accessible()); assert_eq!(m.read(1, 0x2c), Err(Fault::Prohibited));
        m.clock_write(0x781).unwrap(); assert_eq!(m.read(1, 0x2c), Ok(0));
        m.write(1, 0, 1 << 27, &mut flash).unwrap(); assert_eq!(m.read(1, 0x2c), Ok(2)); // APB reset did not reset NOR.
        assert_eq!(m.clock_write(0x7a1), Err(Fault::Prohibited));
        assert_eq!(m.read(0, 0x24), Err(Fault::Unmapped)); assert_eq!(m.read(0, 0x28), Err(Fault::Unmapped));
    }
    #[test]
    fn command_subset_rejects_unsupported_transfers_without_changing_data() {
        let mut m = Mspi::default(); let mut flash = vec![0xa5; 128]; m.attach_flash([1, 2, 3], 0).unwrap();
        for value in [1 << 25, 1 << 23, (1 << 31) | (1 << 28)] { assert_eq!(m.write(1, 0, value, &mut flash), Err(Fault::Prohibited)); }
        m.write(1, 0x18, 0x90000000, &mut flash).unwrap(); m.write(1, 0x20, 0x7000009f, &mut flash).unwrap();
        m.write(1, 0x28, 7, &mut flash).unwrap(); m.write(1, 0, 1 << 18, &mut flash).unwrap(); assert_eq!(m.read(1, 0x58), Ok(1));
        m.write(1, 0x20, 0x7000005a, &mut flash).unwrap(); assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
        m.write(1, 0x20, 0x70000003, &mut flash).unwrap(); assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
        m.write(1, 0x18, 0xd0000000, &mut flash).unwrap(); m.write(1, 0x1c, 31 << 26, &mut flash).unwrap();
        assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
        m.write(1, 0x1c, 23 << 26, &mut flash).unwrap(); m.write(1, 0x28, 512, &mut flash).unwrap();
        assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
        m.write(1, 0x28, 31, &mut flash).unwrap(); m.write(1, 0x20, 0x7000006b, &mut flash).unwrap();
        assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited)); // No invented QE.
        assert_eq!(m.commands, 1); assert_eq!(flash, vec![0xa5; 128]);
    }
}
