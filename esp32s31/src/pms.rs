//! S31 LP/HP peripheral permissions for the single HP CPU's reset TEE domain.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! soc/esp32s31/register/soc/{reg_base,lp_peri_pms_reg,hp_peri0_pms_reg,
//! hp_peri1_pms_reg,tee_reg}.h and bootloader_support/src/bootloader_mem.c.
//! No REE transition, DMA-master security, APM memory-region filter or PMS IRQ yet.
use emu_core::Fault;
pub const LP: u32 = 0x2070_6000;
pub const HP0: u32 = 0x2050_5000;
pub const HP1: u32 = 0x2058_a000;
const COUNTS: [usize; 3] = [29, 31, 39];
struct Domain { controls: [u16; 39], bus_error: bool, keep_clock: bool, date: u32 }
impl Domain {
    fn new(date: u32) -> Self { Self { controls: [0x11; 39], bus_error: false, keep_clock: true, date } }
}
pub struct Permissions { domains: [Domain; 3], pub writes: u64 }
impl Default for Permissions {
    fn default() -> Self { Self { domains: [Domain::new(38_834_752), Domain::new(38_834_736), Domain::new(38_834_720)], writes: 0 } }
}
impl Permissions {
    fn register(address: u32) -> Option<(usize, u32)> {
        for (id, base, size) in [(0, LP, 0x800), (1, HP0, 0x1000), (2, HP1, 0x1000)] {
            if let Some(off) = address.checked_sub(base).filter(|&o| o < size) { return Some((id, off)); }
        }
        None
    }
    pub fn is_mmio(address: u32) -> bool { Self::register(address).is_some() }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        let (id, off) = Self::register(address).ok_or(Fault::Unmapped)?;
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        let d = &self.domains[id]; let end = if id == 0 { 0x800 } else { 0x1000 };
        if (off / 4) < COUNTS[id] as u32 { return Ok(d.controls[(off / 4) as usize].into()); }
        match off {
            0x300 => Ok(0), // IRQ disabled; enabling it is unsupported, not silently ignored.
            o if o == end - 16 => Ok(u32::from(d.bus_error)),
            o if o == end - 8 => Ok(u32::from(d.keep_clock)),
            o if o == end - 4 => Ok(d.date),
            _ => Err(Fault::Unmapped), // Exception record layout is not fabricated.
        }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        let (id, off) = Self::register(address).ok_or(Fault::Unmapped)?;
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        let d = &mut self.domains[id]; let end = if id == 0 { 0x800 } else { 0x1000 };
        if (off / 4) < COUNTS[id] as u32 {
            let word = &mut d.controls[(off / 4) as usize];
            if *word & 0x100 == 0 {
                // The header disables error replies by default but does not specify the
                // returned read data. Do not invent zero/stall semantics: a restricted
                // TEE profile requires the documented bus-error response in this subset.
                if value & 0x11 != 0x11 && !d.bus_error { return Err(Fault::Prohibited); }
                *word = (value & 0x1ff) as u16;
            }
            self.writes += 1; return Ok(());
        }
        match off {
            0x300 if value == 0 => {},
            o if o == end - 16 => {
                if value & 1 == 0 && d.controls.iter().any(|v| v & 0x11 != 0x11) { return Err(Fault::Prohibited); }
                d.bus_error = value & 1 != 0;
            }
            o if o == end - 8 => d.keep_clock = value & 1 != 0, // automatic gating, not a bus-disable bit
            o if o == end - 4 => d.date = value & 0x0fff_ffff,
            0x300 => return Err(Fault::Prohibited),
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    /// Only peripheral windows that currently contain implemented S31 MMIO are
    /// connected. Reserved offsets still fault in their actual device. No permission
    /// is inferred for the separate MODEM, CLIC, SRAM/ROM or cached data windows.
    fn target(address: u32) -> Option<(usize, usize)> {
        match address {
            0x2070_6000..=0x2070_67ff => Some((0, 6)),
            0x2070_6800..=0x2070_6bff => Some((0, 7)), // LP_TEE (clock policy only)
            0x2070_6c00..=0x2070_6fff => Some((0, 8)),
            0x2050_4000..=0x2050_43ff => Some((1, 22)), // TEE (clock policy only)
            0x2050_4400..=0x2050_47ff => Some((1, 23)),
            0x2050_4800..=0x2050_4bff => Some((1, 24)),
            0x2050_4c00..=0x2050_4fff => Some((1, 25)),
            0x2050_0000..=0x2050_1fff => Some((1, 20)), // FLASH_SPI0/1
            0x2050_2000..=0x2050_3fff => Some((1, 21)), // PSRAM_MSPI0/1
            0x2050_8000..=0x2050_ffff => Some((1, 28)), // CRYPTO includes SHA
            0x2058_2000..=0x2058_2fff => Some((2, 31)), // IOMUX (separate GPIO ownership unconfirmed)
            _ => match address & !0xfff {
                0x2070_0000 => Some((0, 0)), 0x2070_1000 => Some((0, 1)),
                0x2071_0000 => Some((0, 14)), // LP_PERICLKRST
                0x2070_2000 => Some((0, 2)), 0x2070_4000 => Some((0, 4)),
                0x2071_5000 => Some((0, 17)), 0x2080_0000 => Some((0, 9)), 0x2080_1000 => Some((0, 10)), 0x2081_4000 => Some((0, 22)),
                0x2d00_2000 => Some((1, 2)), 0x2c00_0000 => Some((1, 5)), 0x2050_5000 => Some((1, 26)), 0x2050_6000 => Some((1, 27)), // KEY_MANAGER: HP_PERI0_PMS+0x6c
                0x2035_9000 => Some((1, 19)), // CNNT_SYS_REG
                0x2038_a000 => Some((2, 10)), 0x2039_1000 => Some((2, 17)),
                0x2039_9000 => Some((2, 24)), // HP_SYSTIMER_CTRL offset0x60
                0x2058_0000 => Some((2, 29)), 0x2058_1000 => Some((2, 30)),
                0x2058_5000 => Some((2, 33)), 0x2058_6000 => Some((2, 34)),
                0x2058_7000 => Some((2, 35)), 0x2058_9000 => Some((2, 37)), 0x2058_a000 => Some((2, 38)),
                _ => None,
            },
        }
    }
    pub fn check(&self, address: u32, write: bool) -> Result<(), Fault> {
        if let Some((id, target)) = Self::target(address) {
            if self.domains[id].controls[target] & (1 << if write { 4 } else { 0 }) == 0 { return Err(Fault::Prohibited); }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_target_permissions_and_lock_are_independent_across_domains() {
        let mut p = Permissions::default();
        for base in [LP, HP0, HP1] { assert_eq!(p.read(base), Ok(0x11)); }
        assert_eq!(p.write(HP1 + 0x28, 1), Err(Fault::Prohibited)); // unknown quiet-denial read semantics
        assert_eq!(p.read(HP1 + 0x28), Ok(0x11));
        p.write(HP1 + 0xff0, 1).unwrap(); p.write(HP1 + 0x28, 0x101).unwrap();
        assert_eq!(p.check(0x2038a000, false), Ok(())); assert_eq!(p.check(0x2038a000, true), Err(Fault::Prohibited));
        assert_eq!(p.check(0x20391000, true), Ok(())); assert_eq!(p.check(0x2070002c, true), Ok(()));
        p.write(HP1 + 0x28, 0xff).unwrap(); assert_eq!(p.read(HP1 + 0x28), Ok(0x101));
        assert_eq!(p.write(HP1 + 0xff0, 0), Err(Fault::Prohibited));
        assert_eq!(p.write(HP1 + 0x300, 1), Err(Fault::Prohibited));
        assert_eq!(p.read(LP + 0x74), Err(Fault::Unmapped)); assert_eq!(p.read(HP0 + 0x7c), Err(Fault::Unmapped));
        assert_eq!(p.read(HP1 + 0x9c), Err(Fault::Unmapped)); assert_eq!(p.write(HP1 + 1, 0xff), Err(Fault::Misaligned));
    }
}
