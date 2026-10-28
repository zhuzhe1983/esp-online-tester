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
    (0x20, 0x70000000, 0xf000ffff), (0x2c, 0, 0x0fff0000), (0x34, 0x70, 0x600), (0x3c, 0xc0000000, 0xc00001ff), (0xd4, 0x3020, 0x3011),
];
const SPI1_CONFIG: &[(u32, u32, u32)] = &[
    (4, 0, u32::MAX), (8, 0x2ca00c, 0x01fce10c), (0xc, 0x3ffffc, 0x3fffff), (0x10, 0, 0),
    (0x14, 0x30103, 0x80ffffff), (0x18, 0x80000000, 0xfc00f200), (0x1c, 0x5c000007, 0xfc00003f),
    (0x20, 0x70000000, 0xf000ffff), (0x24, 0, 0x3ff), (0x28, 0, 0x3ff), (0x34, 2, 0x603),
    (0x3c, 0, 0x1fa), (0x98, 0x00050001, 0xfffffe1f), (0x9c, 0x08002000, u32::MAX), (0xa0, 0x57575, u32::MAX), (0xa4, 0x7a7a0000, 0xffff80ff), (0xd4, 0x20, 0),
];
pub struct Mspi { spi0: [u32; 64], spi1: SpiMem, clock: u32, descriptor: Option<([u8; 3], u16)>, read_status: u32, powered_down: bool, legacy_rom_lengths: Option<[u32; 2]>, sfdp: Option<(u32, Vec<u8>)>, bbpll_input: Option<u64>, bbpll_gate: bool, pub commands: u64, pub waiti_polls: u64, pub last_waiti_status: Option<u8>, pub last_command: Option<u32> }
impl Default for Mspi {
    fn default() -> Self {
        let mut m = Self { spi0: [0; 64], spi1: SpiMem::new(true), clock: 0x781, descriptor: None, read_status: 0, powered_down: false, legacy_rom_lengths: None, sfdp: None, bbpll_input: None, bbpll_gate: true, commands: 0, waiti_polls: 0, last_waiti_status: None, last_command: None };
        m.reset_registers(); m
    }
}
impl Mspi {
    fn reset_registers(&mut self) {
        if let Some(words) = &mut self.legacy_rom_lengths { words.fill(0); }
        self.spi0.fill(0); for &(off, reset, _) in SPI0_CONFIG { self.spi0[off as usize / 4] = reset; }
        self.spi1 = SpiMem::new(true); self.spi1.has_psram = false; self.read_status = 0;
        let (id, status) = self.descriptor.unwrap_or(([0; 3], 0)); self.spi1.jedec = id; self.spi1.status = status as u32;
        for &(off, reset, _) in SPI1_CONFIG { self.spi1.regs.write(off, reset); }
    }
    /// Caller-supplied decoded device response, never a product-specific default.
    /// This subset supports ordinary 8-bit quad-NOR commands only; no OPI/security.
    /// Optional SFDP data is an independent explicit input, never derived from JEDEC ID.
    pub fn attach_flash(&mut self, jedec: [u8; 3], status: u16) -> Result<(), Fault> {
        if status & !0x202 != 0 { return Err(Fault::Prohibited); } // No BUSY/protection/security state synthesized.
        self.descriptor = Some((jedec, status)); self.powered_down = false; self.spi1.jedec = jedec; self.spi1.status = status as u32; Ok(())
    }
    /// Caller-supplied bytes for one SFDP address interval. Unknown bytes are not
    /// manufactured from the device ID, array contents or a different chip's table.
    pub fn attach_sfdp(&mut self, start: u32, bytes: &[u8]) -> Result<(), Fault> {
        if bytes.is_empty() || bytes.len() > 65536 || start.checked_add(bytes.len() as u32).is_none_or(|end| end > 0x100_0000) { return Err(Fault::Prohibited); }
        self.sfdp = Some((start, bytes.to_vec())); Ok(())
    }
    /// Explicit development compatibility, not a confirmed S31 hardware register map.
    /// Public ROM20260528 SPI_init accesses SPI0+0x24/+0x28 as legacy length words;
    /// both initial/current IDF headers mark them reserved. Espressif emulator0.51.0
    /// guest read/write accepts full words. Keep two stateful shadows only when the
    /// caller opts in; no ready flag, transfer result or product profile is invented.
    pub fn enable_legacy_rom_lengths(&mut self) { self.legacy_rom_lengths = Some([0; 2]); }
    /// A supplied stable SPLL output for digital bring-up, not simulated analog lock.
    /// The fixed v6.1 MSPI LL explicitly assumes480MHz. No reset-ready source is guessed.
    pub fn set_bbpll_input(&mut self, hz: Option<u64>) -> Result<(), Fault> {
        if hz.is_some_and(|hz| hz != 480_000_000) { return Err(Fault::Prohibited); }
        self.bbpll_input = hz; Ok(())
    }
    pub fn set_bbpll_gate(&mut self, on: bool) { self.bbpll_gate = on; }
    /// The same explicitly supplied BBPLL root feeds documented peripheral taps.
    /// No additional ready/lock state is inferred from a requested divider.
    pub fn bbpll_root_hz(&self) -> Option<u64> { if self.bbpll_gate { self.bbpll_input } else { None } }
    fn core_hz_for(&self, clock: u32) -> Option<u64> {
        if clock & 0x105 != 0x101 { return None; }
        match (clock >> 5) & 3 {
            0 => Some(40_000_000 / (u64::from((clock >> 9) & 255) + 1)),
            1 if clock & 0x80 != 0 && self.bbpll_gate => self.bbpll_input.map(|hz| hz / (u64::from((clock >> 9) & 255) + 1)),
            _ => None,
        }
    }
    pub fn core_hz(&self) -> Option<u64> { self.core_hz_for(self.clock) }
    pub fn clock_read(&self) -> u32 { self.clock }
    pub fn clock_write(&mut self, value: u32) -> Result<(), Fault> {
        // XTAL or explicit BBPLL input only. CPLL mux and force-reset overrides
        // remain unsupported. Selecting an absent root cannot create a ready clock.
        if value & 0x18 != 0 || (value >> 5) & 3 > 1 { return Err(Fault::Prohibited); }
        if value & 0x105 == 0x101 && (value >> 5) & 3 == 1 && self.core_hz_for(value).is_none() { return Err(Fault::Prohibited); }
        self.clock = value & 0x1ffff;
        if value & 4 != 0 {
            let status = self.spi1.status; self.reset_registers(); self.spi1.status = status; // APB reset does not reset the external NOR die.
        }
        Ok(())
    }
    fn bus_on(&self) -> bool { self.clock & 5 == 1 }
    pub fn flash_accessible(&self) -> bool { !self.powered_down && self.core_hz().is_some() && self.clock & 2 == 0 && self.spi0[0x3c / 4] & 0x80000001 == 1 }
    pub fn read(&self, port: usize, off: u32) -> Result<u32, Fault> {
        if port > 1 { return Err(Fault::Unmapped); }
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.bus_on() { return Err(Fault::Prohibited); }
        if port == 0 {
            if matches!(off, 0x24 | 0x28) { return self.legacy_rom_lengths.map(|words| words[(off as usize - 0x24) / 4]).ok_or(Fault::Unmapped); }
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
            if matches!(off, 0x24 | 0x28) {
                let words = self.legacy_rom_lengths.as_mut().ok_or(Fault::Unmapped)?;
                words[(off as usize - 0x24) / 4] = value; return Ok(());
            }
            if let Some(&(_, _, mask)) = SPI0_CONFIG.iter().find(|x| x.0 == off) {
                let r = &mut self.spi0[off as usize / 4]; *r = (*r & !mask) | (value & mask); return Ok(());
            }
            return match off { 0 if value == 0 => Ok(()), 0x54 | 0x178 => Ok(()), _ => Err(Fault::Unmapped) };
        }
        if let Some(&(_, _, mask)) = SPI1_CONFIG.iter().find(|x| x.0 == off) {
            // PER/PES wait delays are cold policy. Actual suspend/resume triggers
            // and automatic engines remain unsupported,not silently completed.
            if off == 0x9c && value & 0x33 != 0 { return Err(Fault::Prohibited); }
            let old = self.spi1.regs.read(off); self.spi1.regs.write(off, (old & !mask) | (value & mask)); return Ok(());
        }
        match off {
            0 => self.execute(value, flash),
            0x58..=0x94 => { self.spi1.write(off, value); Ok(()) }
            0x2c => { self.read_status = value & 0x0fffffff; Ok(()) } // Controller latch only; not a NOR status write.
            0x54 => Err(Fault::Prohibited),
            _ => Err(Fault::Unmapped),
        }
    }
    /// Synchronous functional NOR has no outstanding BUSY interval. Automatic
    /// wait still performs a real status sample after each supported SE/PP;
    /// validate its command before touching the backing or WEL. IDF's dummy
    /// tuning fields are retained without claiming physical sample calibration.
    fn validate_waiti(&self) -> Result<(), Fault> {
        let config = self.spi1.regs.read(0x98);
        if config & 1 != 0 && (config & 0x204 != 0 || config >> 16 & 255 != 5 || self.spi1.status & 1 != 0) { return Err(Fault::Prohibited); }
        Ok(())
    }
    fn finish_program_erase(&mut self) {
        if self.spi1.regs.read(0x98) & 1 != 0 {
            self.last_waiti_status = Some(self.spi1.status as u8);
            self.waiti_polls = self.waiti_polls.wrapping_add(1);
        }
        // RD_STATUS is latched only by explicit RDSR/RES per the header; no
        // invented public status register alias for this internal auto sample.
        self.commands = self.commands.wrapping_add(1);
    }
    fn execute(&mut self, value: u32, flash: &mut [u8]) -> Result<(), Fault> {
        if value == 0 { return Ok(()); }
        self.last_command = Some(value);
        if self.core_hz().is_none() || self.descriptor.is_none() || flash.is_empty() { return Err(Fault::Prohibited); }
        if self.spi1.regs.read(0x34) & 3 != 2 { return Err(Fault::Prohibited); } // Exactly the explicit CS0 device.
        // Standard controller RES (bit20) / DP (bit21), fixed v6.1 spi1_mem_c_reg.h.
        // Functional die state only: wakeup latency/high-performance mode are absent.
        // RESANDRES=1 also asks for electronic ID, which needs a separate descriptor.
        if value == 1 << 20 || value == 1 << 21 {
            if value == 1 << 20 && self.spi1.regs.read(8) & (1 << 15) != 0 { return Err(Fault::Prohibited); }
            self.powered_down = value == 1 << 21; self.commands = self.commands.wrapping_add(1); return Ok(());
        }
        if self.powered_down { return Err(Fault::Prohibited); }
        if value == 1 << 24 || value == (1 << 24 | 1 << 17) {
            // S31 SPI1 FLASH_SE; fixed ROM20260528 also sets FLASH_PE with SE.
            // Header PE prose mentions USER only. This additional combination is
            // observed ROM behavior, not an independent silicon verification.
            // Winbond W25Q128JV RevF2018 §7.1.2/8.2.15: WEL required,4KiB erase,
            // low12 address bits ignored,WEL clears after actual completion.
            if self.descriptor.map(|d| d.0) != Some([0xef, 0x40, 0x18]) || self.spi1.regs.read(0x1c) >> 26 != 23 { return Err(Fault::Prohibited); }
            let address = self.spi1.regs.read(4); if address > 0xff_ffff { return Err(Fault::Prohibited); }
            self.validate_waiti()?;
            if self.spi1.status & 2 != 0 {
                let start = (address & !0xfff) as usize;
                let sector = flash.get_mut(start..start + 0x1000).ok_or(Fault::Unmapped)?;
                sector.fill(0xff); self.spi1.status &= !2;
            }
            self.finish_program_erase(); return Ok(()); // synchronous functional completion,not erase latency
        }
        if (value == 1 << 18 || value == (1 << 18 | 1 << 17)) && self.spi1.regs.read(0x20) & 0xff == 2 {
            // USER+FLASH_PE is documented by the S31 header. ROM SPI_write at
            // 0x2f80e86a sends command02/address24/MOSI bytes through this path.
            // Winbond §8.2.13: program ANDs bits and wraps within the256-byte page.
            let user = self.spi1.regs.read(0x18); let user2 = self.spi1.regs.read(0x20);
            let bits = (self.spi1.regs.read(0x24) & 0x3ff) + 1; let address = self.spi1.regs.read(4);
            if self.descriptor.map(|d| d.0) != Some([0xef, 0x40, 0x18]) || user & 0xff00_0000 != 0xc800_0000 || user2 >> 28 != 7 || self.spi1.regs.read(0x1c) >> 26 != 23 || address > 0xff_ffff || bits > 512 || !bits.is_multiple_of(8) { return Err(Fault::Prohibited); }
            self.validate_waiti()?;
            if self.spi1.status & 2 != 0 {
                let count = (bits / 8) as usize; let page = (address & !255) as usize; let first = (address & 255) as usize;
                for i in 0..count { if flash.get(page + ((first + i) & 255)).is_none() { return Err(Fault::Unmapped); } }
                for i in 0..count { flash[page + ((first + i) & 255)] &= (self.spi1.w[i / 4] >> ((i % 4) * 8)) as u8; }
                self.spi1.status &= !2;
            }
            self.finish_program_erase(); return Ok(());
        }
        // No multiple commands, other writes/erase, auto-suspend or unknown commands
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
            if command == 0x7a {
                // Winbond W25Q128JV RevF2018/RevI2021 §8.2.20: an idle die
                // (SUS=0) ignores Resume. No erase/program/suspend state is accepted
                // by this subset. Never feed this into permissive generic fallbacks.
                if self.descriptor.map(|d| d.0) != Some([0xef, 0x40, 0x18]) || user & 0xfc00_0000 != 0x8000_0000 || self.spi1.status & !0x202 != 0 { return Err(Fault::Prohibited); }
                self.commands += 1; return Ok(());
            }
            if command == 0x5a {
                // Fixed v6.1 bootloader_flash_read_sfdp: command8/address24/dummy8,
                // byte-aligned MISO, no MOSI. SPI1 address is LSB-aligned on S31.
                if user & 0xff00_0000 != 0xf000_0000 || user1 >> 26 != 23 || user1 & 63 != 7 || !miso_bits.is_multiple_of(8) { return Err(Fault::Prohibited); }
                let address = self.spi1.regs.read(4); let count = (miso_bits / 8) as usize;
                let (start, data) = self.sfdp.as_ref().ok_or(Fault::Prohibited)?;
                let offset = address.checked_sub(*start).ok_or(Fault::Unmapped)? as usize;
                let bytes = data.get(offset..offset.checked_add(count).ok_or(Fault::Unmapped)?).ok_or(Fault::Unmapped)?;
                self.spi1.w.fill(0);
                for (i, byte) in bytes.iter().enumerate() { self.spi1.w[i / 4] |= u32::from(*byte) << ((i % 4) * 8); }
                self.commands += 1; return Ok(());
            }
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
        if value == 1 << 27 { self.read_status = (self.read_status & 0x0fff0000) | (self.spi1.status & 0xffff); }
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
    fn auto_wait_samples_completed_nor_operation_and_guards_unsupported_framing() {
        let mut m = Mspi::default(); let mut flash = vec![0xa5; 0x1000]; m.attach_flash([0xef, 0x40, 0x18], 0x200).unwrap();
        assert_eq!(m.read(1, 0x98), Ok(0x50001));
        m.write(1, 0x98, u32::MAX, &mut flash).unwrap(); assert_eq!(m.read(1, 0x98), Ok(0xfffffe1f));
        m.write(1, 0x9c, 0x0800200c, &mut flash).unwrap(); assert_eq!(m.read(1, 0x9c), Ok(0x0800200c));
        for trigger in [1, 2, 16, 32] { assert_eq!(m.write(1, 0x9c, 0x0800200c | trigger, &mut flash), Err(Fault::Prohibited)); }
        assert_eq!(m.read(1, 0x9c), Ok(0x0800200c));
        m.write(1, 0, 1 << 30, &mut flash).unwrap(); m.write(1, 0x2c, 0x123456, &mut flash).unwrap();
        for bad in [0x350001, 0x50201, 0x50005] {
            m.write(1, 0x98, bad, &mut flash).unwrap(); assert_eq!(m.write(1, 0, 1 << 24, &mut flash), Err(Fault::Prohibited));
            assert!(flash.iter().all(|&b| b == 0xa5)); assert_eq!(m.spi1.status, 0x202); assert_eq!(m.waiti_polls, 0);
        }
        // Eight-bit command ignores its stored high byte; disabled address
        // length and dummy tuning fields do not turn it into another command.
        m.write(1, 0x98, 0xab05fc1b, &mut flash).unwrap(); m.write(1, 0, 1 << 24, &mut flash).unwrap();
        assert!(flash.iter().all(|&b| b == 0xff)); assert_eq!(m.spi1.status, 0x200); assert_eq!(m.waiti_polls, 1); assert_eq!(m.last_waiti_status, Some(0));
        assert_eq!(m.read(1, 0x2c), Ok(0x123456));
        m.write(1, 0x98, 0xfffffe1e, &mut flash).unwrap(); m.write(1, 0, 1 << 24, &mut flash).unwrap(); assert_eq!(m.waiti_polls, 1);
        m.clock_write(0x785).unwrap(); m.clock_write(0x781).unwrap(); assert_eq!(m.read(1, 0x98), Ok(0x50001)); assert_eq!(m.read(1, 0x9c), Ok(0x08002000));
    }
    #[test]
    fn user_program_uses_only_clocked_bytes_wraps_page_and_cannot_restore_bits() {
        let mut m = Mspi::default(); let mut flash = vec![0xff; 0x200]; m.attach_flash([0xef, 0x40, 0x18], 0).unwrap();
        m.write(1, 0x18, 0xc8000000, &mut flash).unwrap(); m.write(1, 0x20, 0x70000002, &mut flash).unwrap();
        m.write(1, 4, 0xfe, &mut flash).unwrap(); m.write(1, 0x24, 31, &mut flash).unwrap(); m.write(1, 0x58, 0x78563412, &mut flash).unwrap();
        m.write(1, 0, 1 << 18 | 1 << 17, &mut flash).unwrap(); assert!(flash.iter().all(|&b| b == 0xff));
        m.write(1, 0, 1 << 30, &mut flash).unwrap(); m.write(1, 0, 1 << 18 | 1 << 17, &mut flash).unwrap();
        assert_eq!(&flash[0xfe..0x100], &[0x12, 0x34]); assert_eq!(&flash[..2], &[0x56, 0x78]); assert!(flash[0x100..].iter().all(|&b| b == 0xff));
        m.write(1, 0, 1 << 27, &mut flash).unwrap(); assert_eq!(m.read(1, 0x2c).unwrap() & 2, 0);
        m.write(1, 0x58, u32::MAX, &mut flash).unwrap(); m.write(1, 0, 1 << 30, &mut flash).unwrap();
        m.write(1, 0, 1 << 18, &mut flash).unwrap(); assert_eq!(&flash[0xfe..0x100], &[0x12, 0x34]);
        m.write(1, 0, 1 << 30, &mut flash).unwrap(); let before = flash.clone();
        for (off, bad, good) in [(4, 0x1000000, 0xfe), (0x24, 30, 31), (0x24, 519, 31), (0x18, 0xcc000000, 0xc8000000)] {
            m.write(1, off, bad, &mut flash).unwrap(); assert_eq!(m.write(1, 0, 1 << 18 | 1 << 17, &mut flash), Err(Fault::Prohibited)); assert_eq!(flash, before); m.write(1, off, good, &mut flash).unwrap();
        }
        m.write(1, 4, 0x200, &mut flash).unwrap(); assert_eq!(m.write(1, 0, 1 << 18 | 1 << 17, &mut flash), Err(Fault::Unmapped)); assert_eq!(flash, before);
    }
    #[test]
    fn sector_erase_requires_wel_and_changes_only_the_complete_addressed_sector() {
        let mut m = Mspi::default(); let mut flash = vec![0xa5; 0x3000]; m.attach_flash([0xef, 0x40, 0x18], 0).unwrap();
        m.write(1, 4, 0x1234, &mut flash).unwrap(); m.write(1, 0, 1 << 24, &mut flash).unwrap(); assert!(flash.iter().all(|&b| b == 0xa5));
        m.write(1, 0, 1 << 30, &mut flash).unwrap(); m.write(1, 0, 1 << 24 | 1 << 17, &mut flash).unwrap();
        assert!(flash[0x1000..0x2000].iter().all(|&b| b == 0xff)); assert!(flash[..0x1000].iter().all(|&b| b == 0xa5)); assert!(flash[0x2000..].iter().all(|&b| b == 0xa5));
        m.write(1, 0, 1 << 27, &mut flash).unwrap(); assert_eq!(m.read(1, 0x2c).unwrap() & 2, 0);
        m.write(1, 4, 0x3000, &mut flash).unwrap(); m.write(1, 0, 1 << 30, &mut flash).unwrap(); let before = flash.clone();
        assert_eq!(m.write(1, 0, 1 << 24, &mut flash), Err(Fault::Unmapped)); assert_eq!(flash, before);
        m.write(1, 0, 1 << 27, &mut flash).unwrap(); assert_eq!(m.read(1, 0x2c).unwrap() & 2, 2);
        assert_eq!(m.write(1, 0, 1 << 24 | 1 << 25, &mut flash), Err(Fault::Prohibited)); assert_eq!(flash, before);
    }
    #[test]
    fn sfdp_reads_only_supplied_interval_with_exact_command_framing() {
        let mut m = Mspi::default(); let mut flash = vec![0xa5; 128]; m.attach_flash([0xef, 0x40, 0x18], 0).unwrap();
        m.write(1, 0x18, 0xf0000000, &mut flash).unwrap(); m.write(1, 0x20, 0x7000005a, &mut flash).unwrap();
        m.write(1, 4, 0x10, &mut flash).unwrap(); m.write(1, 0x28, 31, &mut flash).unwrap();
        m.write(1, 0x58, 0xdeadbeef, &mut flash).unwrap();
        assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
        assert_eq!(m.read(1, 0x58), Ok(0xdeadbeef));
        m.attach_sfdp(0x10, &[0x12, 0x34, 0x56, 0x78]).unwrap();
        m.write(1, 0, 1 << 18, &mut flash).unwrap(); assert_eq!(m.read(1, 0x58), Ok(0x78563412));
        m.write(1, 4, 0x12, &mut flash).unwrap(); m.write(1, 0x28, 15, &mut flash).unwrap();
        m.write(1, 0, 1 << 18, &mut flash).unwrap(); assert_eq!(m.read(1, 0x58), Ok(0x7856)); assert_eq!(m.read(1, 0x5c), Ok(0));
        m.write(1, 0x28, 31, &mut flash).unwrap(); assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Unmapped));
        assert_eq!(m.read(1, 0x58), Ok(0x7856)); assert_eq!(m.commands, 2);
        m.write(1, 4, 0x10, &mut flash).unwrap();
        for (off, invalid, valid) in [(0x1c, 0x5c000006, 0x5c000007), (0x28, 30, 31), (0x18, 0xd0000000, 0xf0000000), (0x18, 0xf4000000, 0xf0000000), (0x20, 0x6000005a, 0x7000005a)] {
            m.write(1, off, invalid, &mut flash).unwrap(); assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
            m.write(1, off, valid, &mut flash).unwrap(); assert_eq!(m.commands, 2);
        }
        m.clock_write(0x785).unwrap(); m.clock_write(0x781).unwrap(); // separate NOR data survives controller reset
        m.write(1, 0x18, 0xf0000000, &mut flash).unwrap(); m.write(1, 0x20, 0x7000005a, &mut flash).unwrap();
        m.write(1, 4, 0x10, &mut flash).unwrap(); m.write(1, 0x28, 31, &mut flash).unwrap();
        m.write(1, 0, 1 << 18, &mut flash).unwrap(); assert_eq!(m.read(1, 0x58), Ok(0x78563412));
        assert_eq!(m.attach_sfdp(0, &[]), Err(Fault::Prohibited)); assert_eq!(m.attach_sfdp(0xffffff, &[1,2]), Err(Fault::Prohibited));
        assert_eq!(flash, vec![0xa5; 128]);
    }
    #[test]
    fn bbpll_input_gate_and_idle_resume_do_not_manufacture_data_or_suspended_operations() {
        let mut m = Mspi::default(); let mut flash = vec![0xa5; 128]; m.attach_flash([0xef, 0x40, 0x18], 2).unwrap();
        assert_eq!(m.clock_write(0xba1), Err(Fault::Prohibited)); assert_eq!(m.clock_read(), 0x781);
        assert_eq!(m.set_bbpll_input(Some(320_000_000)), Err(Fault::Prohibited));
        m.set_bbpll_input(Some(480_000_000)).unwrap(); m.clock_write(0xba1).unwrap(); assert_eq!(m.core_hz(), Some(80_000_000));
        m.write(0, 0x3c, 1, &mut flash).unwrap(); assert!(m.flash_accessible());
        m.write(1, 0x20, 0x7000007a, &mut flash).unwrap(); m.write(1, 0x58, 0x12345678, &mut flash).unwrap();
        m.write(1, 0, 1 << 18, &mut flash).unwrap(); assert_eq!(m.spi1.status, 2); assert_eq!(m.read(1, 0x58), Ok(0x12345678));
        m.set_bbpll_gate(false); assert_eq!(m.core_hz(), None); assert!(!m.flash_accessible());
        assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited)); assert_eq!(m.commands, 1);
        m.set_bbpll_gate(true); assert!(m.flash_accessible()); m.set_bbpll_input(None).unwrap(); assert_eq!(m.core_hz(), None); assert!(!m.flash_accessible());
        m.clock_write(0x781).unwrap(); m.write(1, 0, 1 << 18, &mut flash).unwrap();
        m.attach_flash([0x20, 0x40, 0x18], 0).unwrap(); assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
        m.attach_flash([0xef, 0x40, 0x18], 0).unwrap(); m.write(1, 0x18, 0x90000000, &mut flash).unwrap();
        assert_eq!(m.write(1, 0, 1 << 18, &mut flash), Err(Fault::Prohibited));
        assert_eq!(m.attach_flash([0xef, 0x40, 0x18], 0x8000), Err(Fault::Prohibited)); // suspended operations are not installed
        assert_eq!(flash, vec![0xa5; 128]);
    }
    #[test]
    fn controller_gates_reset_and_nor_die_state_are_separate() {
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
    fn explicit_legacy_shadows_do_not_change_strict_default_or_spi1_lengths() {
        let mut m = Mspi::default(); let mut flash = vec![0xa5; 16];
        assert_eq!(m.write(0, 0x24, 0xffffffff, &mut flash), Err(Fault::Unmapped));
        m.enable_legacy_rom_lengths();
        for off in [0x24, 0x28] {
            assert_eq!(m.read(0, off), Ok(0)); m.write(0, off, 0xa55a03ff, &mut flash).unwrap();
            assert_eq!(m.read(0, off), Ok(0xa55a03ff));
            m.write(1, off, 0xa55a03ff, &mut flash).unwrap(); assert_eq!(m.read(1, off), Ok(0x3ff));
        }
        m.clock_write(0x785).unwrap(); m.clock_write(0x781).unwrap(); assert_eq!(m.read(0, 0x24), Ok(0));
    }
    #[test]
    fn nor_sleep_state_survives_controller_reset_and_wake_requires_attached_device() {
        let mut m = Mspi::default(); let mut flash = vec![0xa5; 128];
        assert_eq!(m.write(1, 0, 1 << 20, &mut flash), Err(Fault::Prohibited));
        m.attach_flash([0xef, 0x40, 0x18], 0).unwrap(); m.write(0, 0x3c, 1, &mut flash).unwrap();
        m.write(1, 0, 1 << 21, &mut flash).unwrap(); assert!(!m.flash_accessible());
        assert_eq!(m.write(1, 0, 1 << 28, &mut flash), Err(Fault::Prohibited));
        m.clock_write(0x785).unwrap(); m.clock_write(0x781).unwrap();
        assert_eq!(m.write(1, 0, 1 << 28, &mut flash), Err(Fault::Prohibited));
        m.write(1, 8, 1 << 15, &mut flash).unwrap();
        assert_eq!(m.write(1, 0, 1 << 20, &mut flash), Err(Fault::Prohibited));
        m.write(1, 8, 0, &mut flash).unwrap(); m.write(1, 0, 1 << 20, &mut flash).unwrap();
        m.write(1, 0, 1 << 28, &mut flash).unwrap(); assert_eq!(m.read(1, 0x58), Ok(0x1840ef));
        assert_eq!(flash, vec![0xa5; 128]); assert_eq!(m.commands, 3);
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
