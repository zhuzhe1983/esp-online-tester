//! S31 key-manager cold configuration, not a key-generation engine.
//! IDF6.1 274d7ebeaf15cf397347877c34816ea9470d153f: keymng_reg.h,
//! hp_sys_clkrst_reg.h and key_mgr_ll.h. Reset restores documented register
//! defaults. START/CONTINUE fail; no key, valid flag, completion or success is made.
use emu_core::Fault;
pub const BASE: u32 = 0x2050_6000;
pub const CLOCK: u32 = 0x2058_705c;
pub struct KeyManager { clock: u32, force_clock: u32, irq_enable: u32, config: u32, static_config: u32, locks: u32 }
impl Default for KeyManager {
    fn default() -> Self { Self { clock: 5, force_clock: 0, irq_enable: 0, config: 0, static_config: 0x1e0, locks: 0 } }
}
impl KeyManager {
    pub fn clock_read(&self) -> u32 { self.clock }
    pub fn clock_write(&mut self, value: u32) {
        if value & 2 != 0 { *self = Self::default(); }
        self.clock = value & 7;
    }
    fn enabled(&self) -> bool { self.clock == 5 }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.enabled() { return Err(Fault::Prohibited); }
        match off {
            4 => Ok(self.force_clock), 0x10 => Ok(self.irq_enable), 0x18 => Ok(self.static_config),
            0x1c => Ok(self.locks), 0x20 => Ok(self.config),
            // No command is accepted. IDLE/result0/key-invalid/raw0 are the
            // actual reset state, not completion of an unsupported operation.
            8 | 0xc | 0x14 | 0x24 | 0x28 | 0x2c | 0x30 => Ok(0),
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if off & 3 != 0 { return Err(Fault::Misaligned); }
        if !self.enabled() { return Err(Fault::Prohibited); }
        match off {
            4 => self.force_clock = value & 3,
            0x10 => self.irq_enable = value & 7,
            0x18 => {
                let protected = self.locks & 31 | if self.locks & 32 != 0 { 0x3e0 } else { 0 }
                    | if self.locks & 64 != 0 { 0x400 } else { 0 }
                    | if self.locks & 128 != 0 { 0x800 } else { 0 }
                    | if self.locks & 256 != 0 { 0x1000 } else { 0 };
                self.static_config = (self.static_config & protected) | (value & 0x1fff & !protected);
            }
            0x1c => self.locks |= value & 0x1ff,
            0x20 => self.config = value & 0x7f,
            0x24 if value & 3 != 0 => return Err(Fault::Prohibited),
            8 | 0xc | 0x14 | 0x24 | 0x28 | 0x2c | 0x30 => {}, // RO/WTC/WT while no operation has started.
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cold_policy_locks_are_one_way_and_reset_does_not_create_keys() {
        let mut k = KeyManager::default(); assert_eq!(k.read(0x18), Ok(0x1e0));
        k.write(0x18, 0x1802).unwrap(); k.write(0x1c, 0x182).unwrap(); k.write(0x1c, 0).unwrap();
        k.write(0x18, 0x7fd).unwrap(); assert_eq!(k.read(0x18), Ok(0x1fff)); assert_eq!(k.read(0x1c), Ok(0x182));
        k.write(0x10, 7).unwrap();
        for command in [1, 2, 3] { assert_eq!(k.write(0x24, command), Err(Fault::Prohibited)); }
        for off in [8, 0xc, 0x28, 0x2c, 0x30] { k.write(off, u32::MAX).unwrap(); assert_eq!(k.read(off), Ok(0)); }
        assert_eq!(k.read(0x34), Err(Fault::Unmapped)); // No fabricated key material.
        for gate in [0, 1, 4, 7] { k.clock_write(gate); assert_eq!(k.read(0x28), Err(Fault::Prohibited)); }
        k.clock_write(5); assert_eq!(k.read(0x18), Ok(0x1e0)); assert_eq!(k.read(0x1c), Ok(0)); assert_eq!(k.read(0x10), Ok(0));
        assert_eq!(k.read(0x28), Ok(0)); assert_eq!(k.read(0x2c), Ok(0)); assert_eq!(k.read(0x30), Ok(0));
    }
}
