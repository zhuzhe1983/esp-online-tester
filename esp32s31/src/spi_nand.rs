//! A Winbond W25N01GV SPI NAND (1 Gbit: 1024 blocks of 64 pages of 2048 + 64 bytes, JEDEC EF AA 21)
//! at the command level the ESP-Mosaico's storage driver uses (espressif/spi_nand_flash:
//! spi_nand_oper.c, nand_winbond.c; command set from the W25N01GV datasheet, rev. N): READ ID 9F
//! (8 dummy bits as an address byte), GET/SET FEATURE 0F/1F (A0 protection, B0 configuration,
//! C0 status), WRITE ENABLE/DISABLE 06/04, PAGE DATA READ 13 into the data buffer, buffer reads
//! 03/0B/3B/6B/BB/EB (a 16-bit column; dummy cycles are not bytes on this bus), PROGRAM LOAD
//! 02/32 (the buffer reset to FF first) and RANDOM PROGRAM LOAD 84/34, PROGRAM EXECUTE 10 (NAND
//! programming ANDs into the page), BLOCK ERASE D8 and RESET FF. The array starts erased; pages
//! are allocated when first programmed. Operations complete at once: status OIP never reads 1.
use std::collections::HashMap;

pub const PAGE: usize = 2048;
pub const SPARE: usize = 64;
pub const PAGES_PER_BLOCK: u32 = 64;
pub const BLOCKS: u32 = 1024;
const RAW: usize = PAGE + SPARE;

pub struct SpiNand {
    pages: HashMap<u32, Box<[u8]>>,
    buffer: Box<[u8]>,
    protection: u8,
    config: u8,
    wel: bool,
    pub stats: NandStats,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct NandStats { pub reads: u64, pub programs: u64, pub erases: u64, pub refused: u64 }

impl Default for SpiNand { fn default() -> Self { Self::new() } }

impl SpiNand {
    pub fn new() -> Self {
        // Power-up: all blocks locked (A0 = 0x7c), BUF = 1 and ECC-E = 1 (B0 = 0x18).
        SpiNand { pages: HashMap::new(), buffer: vec![0xff; RAW].into_boxed_slice(), protection: 0x7c, config: 0x18, wel: false, stats: NandStats::default() }
    }
    fn row(tx: &[u8]) -> Option<u32> {
        (tx.len() >= 4).then(|| u32::from(tx[1]) << 16 | u32::from(tx[2]) << 8 | u32::from(tx[3])).filter(|&r| r < BLOCKS * PAGES_PER_BLOCK)
    }
    fn column(tx: &[u8]) -> usize { if tx.len() >= 3 { (usize::from(tx[1]) << 8 | usize::from(tx[2])) & 0xfff } else { 0 } }
    fn locked(&self) -> bool { self.protection & 0x7c != 0 }
    fn status(&self) -> u8 { u8::from(self.wel) << 1 }

    /// One SPI transaction: command, address and MOSI bytes as the bus sent them; the MISO bytes.
    pub fn transaction(&mut self, tx: &[u8], rx_len: usize) -> Vec<u8> {
        let mut rx = vec![0xff; rx_len];
        let Some(&cmd) = tx.first() else { return rx };
        match cmd {
            0x9f => {
                // The bytes after the command: the dummy byte then EF AA 21; the driver reads the
                // manufacturer after one address byte and the device ID after two.
                let id = [0x00, 0xef, 0xaa, 0x21];
                let skip = (tx.len() - 1).min(id.len());
                for (i, b) in rx.iter_mut().enumerate() { *b = id.get(skip + i).copied().unwrap_or(0); }
            }
            0x0f => {
                let v = match tx.get(1) { Some(0xa0 | 0x01) => self.protection, Some(0xb0 | 0x02) => self.config, Some(0xc0 | 0x03) => self.status(), _ => 0 };
                rx.iter_mut().for_each(|b| *b = v);
            }
            0x1f => if let (Some(&reg), Some(&v)) = (tx.get(1), tx.get(2)) {
                match reg { 0xa0 | 0x01 => self.protection = v, 0xb0 | 0x02 => self.config = v, _ => {} }
            },
            0x06 => self.wel = true,
            0x04 => self.wel = false,
            0xff => { self.wel = false; self.buffer.fill(0xff); }
            0x13 => if let Some(row) = Self::row(tx) {
                self.stats.reads += 1;
                match self.pages.get(&row) { Some(p) => self.buffer.copy_from_slice(p), None => self.buffer.fill(0xff) }
            },
            0x03 | 0x0b | 0x3b | 0x6b | 0xbb | 0xeb => {
                let col = Self::column(tx);
                for (i, b) in rx.iter_mut().enumerate() { *b = self.buffer.get(col + i).copied().unwrap_or(0xff); }
            }
            0x02 | 0x32 | 0x84 | 0x34 => {
                if self.wel {
                    if matches!(cmd, 0x02 | 0x32) { self.buffer.fill(0xff); }
                    let col = Self::column(tx);
                    for (i, &b) in tx.iter().skip(3).enumerate() { if let Some(d) = self.buffer.get_mut(col + i) { *d = b; } }
                } else { self.stats.refused += 1; }
            }
            0x10 => {
                match Self::row(tx) {
                    Some(row) if self.wel && !self.locked() => {
                        let page = self.pages.entry(row).or_insert_with(|| vec![0xff; RAW].into_boxed_slice());
                        for (d, s) in page.iter_mut().zip(self.buffer.iter()) { *d &= *s; }
                        self.stats.programs += 1;
                    }
                    _ => self.stats.refused += 1,
                }
                self.wel = false;
            }
            0xd8 => {
                match Self::row(tx) {
                    Some(row) if self.wel && !self.locked() => {
                        let first = row & !(PAGES_PER_BLOCK - 1);
                        for p in first..first + PAGES_PER_BLOCK { self.pages.remove(&p); }
                        self.stats.erases += 1;
                    }
                    _ => self.stats.refused += 1,
                }
                self.wel = false;
            }
            _ => {}
        }
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifies_unlocks_programs_reads_and_erases_as_the_driver_drives_it() {
        let mut n = SpiNand::new();
        assert_eq!(n.transaction(&[0x9f, 0], 1), [0xef]);
        assert_eq!(n.transaction(&[0x9f, 0, 0], 2), [0xaa, 0x21]);
        // locked at power-up: a program is refused until A0 is cleared
        n.transaction(&[0x06], 0); n.transaction(&[0x02, 0, 0, 0x12, 0x34], 0); n.transaction(&[0x10, 0, 0, 65], 0);
        assert_eq!(n.stats.refused, 1);
        n.transaction(&[0x1f, 0xa0, 0], 0);
        n.transaction(&[0x06], 0); n.transaction(&[0x02, 0, 4, 0x12, 0x34], 0); n.transaction(&[0x10, 0, 0, 65], 0);
        n.transaction(&[0x13, 0, 0, 65], 0);
        assert_eq!(n.transaction(&[0x03, 0, 3], 4), [0xff, 0x12, 0x34, 0xff]);
        n.transaction(&[0x06], 0); n.transaction(&[0xd8, 0, 0, 64], 0);
        n.transaction(&[0x13, 0, 0, 65], 0);
        assert_eq!(n.transaction(&[0x03, 0, 4], 2), [0xff, 0xff]);
        assert_eq!(n.transaction(&[0x0f, 0xc0], 1), [0]);
    }
}
