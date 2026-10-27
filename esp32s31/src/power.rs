//! S31 bootloader MPLL shutdown controls; no analog PLL output or PMU sleep transitions.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643, soc/esp32s31/register/soc/
//! {pmu_reg,hp_alive_sys_reg}.h and esp_hal_clock/esp32s31/include/hal/clk_tree_ll.h.
use emu_core::Fault;
const ACTIVE_CK: u32 = 0x2070_401c;
const IMMEDIATE_CK: u32 = 0x2070_40f4;
const ALIVE_CK: u32 = 0x2058_9000;
const MPLL_LOW: u32 = (1 << 3) | (1 << 7) | (1 << 11);
pub struct Power { active: u32, alive: u32, mpll_low: u32 }
impl Default for Power {
    fn default() -> Self { Self { active: 0, alive: 0xfffe_0000, mpll_low: 0 } }
}

impl Power {
    pub fn is_mmio(address: u32) -> bool { matches!(address & !3, ACTIVE_CK | IMMEDIATE_CK | ALIVE_CK) }
    /// Explicit software shutdown requests, not a measured oscillator state.
    pub fn mpll_disabled(&self) -> bool { self.mpll_low == MPLL_LOW && self.alive & (1 << 31) == 0 && self.active & 0x4400_0000 == 0 }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        match address { ACTIVE_CK => Ok(self.active), ALIVE_CK => Ok(self.alive), IMMEDIATE_CK => Ok(0), _ => Err(Fault::Unmapped) }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        match address {
            IMMEDIATE_CK => {
                // Only the documented WT shutdown commands used by clk_ll_mpll_disable.
                // The header disagrees about WT/RW on some high controls; powering PLLs
                // up is deliberately unsupported until their protocol is implemented.
                if value & !MPLL_LOW != 0 { return Err(Fault::Prohibited); }
                self.mpll_low |= value;
            }
            ALIVE_CK => {
                // Preserve other domain enables. Turning off CPU/root clocks cannot be
                // represented by continuing execution, so such writes fail explicitly.
                if (value ^ self.alive) & !(1 << 31) != 0 { return Err(Fault::Prohibited); }
                self.alive = value;
            }
            ACTIVE_CK => {
                if value & !0x4400_0000 != 0 { return Err(Fault::Prohibited); }
                self.active = value;
            }
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mpll_shutdown_requires_the_actual_sequence_and_does_not_enable_unknown_plls() {
        let mut p = Power::default(); assert!(!p.mpll_disabled());
        p.write(IMMEDIATE_CK, 1 << 11).unwrap(); assert_eq!(p.read(IMMEDIATE_CK), Ok(0));
        p.write(ALIVE_CK, 0x7ffe_0000).unwrap(); assert!(!p.mpll_disabled());
        p.write(IMMEDIATE_CK, 0x88).unwrap(); assert!(p.mpll_disabled());
        p.write(ACTIVE_CK, 0x4400_0000).unwrap(); assert!(!p.mpll_disabled());
        p.write(ACTIVE_CK, 0).unwrap(); assert!(p.mpll_disabled());
        assert_eq!(p.write(IMMEDIATE_CK, 1 << 30), Err(Fault::Prohibited));
        assert_eq!(p.write(ALIVE_CK, 0), Err(Fault::Prohibited));
        assert_eq!(p.write(ACTIVE_CK, 1 << 27), Err(Fault::Prohibited));
        assert!(p.mpll_disabled()); assert_eq!(p.read(ALIVE_CK), Ok(0x7ffe_0000));
        assert_eq!(p.read(IMMEDIATE_CK + 4), Err(Fault::Unmapped));
    }
}
