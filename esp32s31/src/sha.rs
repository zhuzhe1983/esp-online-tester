//! S31 SHA register/block engine. ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/{sha,hp_sys_clkrst}_reg.h and
//! esp_hal_security/esp32s31/include/hal/sha_ll.h. Shared compression only; S31 gates,
//! 16-bit DMA block count and algorithm limits are explicit. No DMA or SHA3/SM3.
//! Crypto root encoding follows S31 sec_ll.h at IDF6.1
//! 274d7ebeaf15cf397347877c34816ea9470d153f (absent at the older header revision).
use emu_core::Fault;
use esp_periph::sha::Sha;
pub const BASE: u32 = 0x2050_9000;
pub const CLOCK: u32 = 0x2058_7058;
pub struct ShaEngine { engine: Sha, clock: u32, dma_blocks: u32, date: u32, free: u32 }
impl Default for ShaEngine {
    fn default() -> Self { Self { engine: Sha::new(), clock: 0x415555, dma_blocks: 0, date: 539297315, free: 1 } }
}
impl ShaEngine {
    pub fn clock_read(&self) -> u32 { self.clock }
    pub fn clock_source(&self) -> u32 { (self.clock >> 20) & 3 }
    pub fn clock_write(&mut self, value: u32) -> Result<(), Fault> {
        if (value >> 20) & 3 == 3 { return Err(Fault::Prohibited); } // Reserved by the S31 LL.
        self.clock = value & 0xf3ffff;
        // The official LL explicitly clears parent/DS/HMAC/ECDSA resets because they
        // hold SHA in reset. Include the enclosing security-domain reset as well.
        if value & 0x2888a != 0 { self.engine = Sha::new(); self.dma_blocks = 0; self.date = 539297315; self.free = 1; }
        Ok(())
    }
    fn enabled(&self) -> bool { self.clock & (0x45 | 0x2888a) == 0x45 }
    pub fn blocks(&self) -> u64 { self.engine.blocks }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.enabled() { return Err(Fault::Prohibited); }
        match off {
            0 | 0x18 | 0x40..=0xfc => Ok(self.engine.read(off)),
            0xc => Ok(self.dma_blocks), 0x2c => Ok(self.date), 0x38 => Ok(self.free),
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.enabled() { return Err(Fault::Prohibited); }
        match off {
            0 if value & 15 > 4 => return Err(Fault::Prohibited),
            0 | 0x40..=0xfc => self.engine.write(off, value),
            0x10 | 0x14 => { if value & 1 != 0 { self.engine.write(off, 1); } }
            0xc => self.dma_blocks = value & 0xffff,
            0x2c => self.date = value & 0x3fffffff,
            0x38 => self.free = value & 1,
            // Empty DMA state can be reset/cleared, but never reports a fabricated transfer.
            0x1c | 0x20 | 0x28 if value & 1 != 0 => return Err(Fault::Prohibited),
            0x1c | 0x20 | 0x24 | 0x28 | 0x30 | 0x34 => {}
            0x18 => {} // RO busy. All supported compression completes synchronously.
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_known_answer_and_parent_reset_gates() {
        let mut s = ShaEngine::default();
        s.write(0x80, 0x80636261).unwrap(); s.write(0xbc, 0x18000000).unwrap();
        s.write(0x10, 0).unwrap(); assert_eq!(s.blocks(), 0);
        s.write(0x10, 1).unwrap(); assert_eq!(s.blocks(), 1); assert_eq!(s.read(0x18), Ok(0));
        let digest: Vec<_> = (0..8).flat_map(|i| s.read(0x40 + 4 * i).unwrap().to_le_bytes()).collect();
        assert_eq!(digest, [0xba,0x78,0x16,0xbf,0x8f,0x01,0xcf,0xea,0x41,0x41,0x40,0xde,0x5d,0xae,0x22,0x23,0xb0,0x03,0x61,0xa3,0x96,0x17,0x7a,0x9c,0xb4,0x10,0xff,0x61,0xf2,0x00,0x15,0xad]);
        for bit in [0, 2, 6] {
            s.clock_write(0x415555 & !(1 << bit)).unwrap(); assert_eq!(s.read(0x40), Err(Fault::Prohibited));
            s.clock_write(0x415555).unwrap(); assert_eq!(s.read(0x40), Ok(0xbf1678ba));
        }
        for bit in [1, 3, 7, 11, 15, 17] {
            s.clock_write(0x415555 | (1 << bit)).unwrap(); assert_eq!(s.write(0x10, 1), Err(Fault::Prohibited));
            s.clock_write(0x415555).unwrap(); assert_eq!(s.read(0x40), Ok(0));
        }
    }
    #[test]
    fn s31_modes_and_dma_count_do_not_alias_the_older_shared_defaults() {
        let mut s = ShaEngine::default();
        s.write(0xc, 0x12345678).unwrap(); assert_eq!(s.read(0xc), Ok(0x5678));
        for mode in 5..16 { assert_eq!(s.write(0, mode), Err(Fault::Prohibited)); }
        assert_eq!(s.read(0), Ok(2)); assert_eq!(s.write(0x1c, 1), Err(Fault::Prohibited));
        assert_eq!(s.blocks(), 0); assert_eq!(s.read(0x100), Err(Fault::Unmapped));
    }
}
