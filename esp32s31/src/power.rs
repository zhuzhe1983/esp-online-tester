//! S31 MPLL shutdown and BOD mode1 configuration with an explicit qualified comparator input.
//! No analog PLL output, voltage/threshold waveform, glitch detection or PMU sleep transitions.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643, soc/esp32s31/register/soc/
//! {pmu_reg,hp_alive_sys_reg,lp_analog_peri_reg}.h, soc/esp32s31/include/soc/reset_reasons.h,
//! esp_hal_clock/esp32s31/include/hal/clk_tree_ll.h and esp_hal_pmu/esp32s31/include/hal/brownout_ll.h.
use emu_core::Fault;
const ACTIVE_CK: u32 = 0x2070_401c;
const IMMEDIATE_CK: u32 = 0x2070_40f4;
const ALIVE_CK: u32 = 0x2058_9000;
pub const BOD_BASE: u32 = 0x2070_2000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetCause { Brownout } // reset_reasons.h: RESET_REASON_SYS_BROWN_OUT=15.
const MPLL_LOW: u32 = (1 << 3) | (1 << 7) | (1 << 11);
pub struct Power { active: u32, alive: u32, mpll_low: u32, fib: u32, bod1: u32, glitch: u32, pub reset_request: Option<ResetCause> }
impl Default for Power {
    fn default() -> Self { Self { active: 0, alive: 0xfffe_0000, mpll_low: 0, fib: u32::MAX, bod1: 0, glitch: 0, reset_request: None } }
}

impl Power {
    pub fn is_mmio(address: u32) -> bool { matches!(address & !3, ACTIVE_CK | IMMEDIATE_CK | ALIVE_CK) || matches!((address & !3).wrapping_sub(BOD_BASE), 4 | 0x18 | 0x1c) }
    /// Explicit software shutdown requests, not a measured oscillator state.
    pub fn mpll_disabled(&self) -> bool { self.mpll_low == MPLL_LOW && self.alive & (1 << 31) == 0 && self.active & 0x4400_0000 == 0 }
    /// A caller-supplied already-qualified BOD comparator event, not a measured voltage
    /// or an analog threshold model. FIB/eFuse-controlled policy is still unknown.
    pub fn brownout_input(&mut self, tripped: bool) -> Result<(), Fault> {
        if !tripped { return Ok(()); }
        if self.fib & 2 != 0 { return Err(Fault::Prohibited); }
        if self.bod1 != 0 { self.reset_request = Some(ResetCause::Brownout); }
        Ok(())
    }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        match address { ACTIVE_CK => Ok(self.active), ALIVE_CK => Ok(self.alive), IMMEDIATE_CK => Ok(0),
            a if a == BOD_BASE + 4 => Ok(self.bod1), a if a == BOD_BASE + 0x18 => Ok(self.glitch),
            a if a == BOD_BASE + 0x1c => Ok(self.fib), _ => Err(Fault::Unmapped) }
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
            a if a == BOD_BASE + 4 => self.bod1 = value & (1 << 31),
            a if a == BOD_BASE + 0x18 => self.glitch = value & 0xfc00_0000,
            a if a == BOD_BASE + 0x1c => self.fib = value,
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brownout_requires_known_software_policy_and_actual_comparator_event() {
        let mut p = Power::default();
        assert_eq!(p.brownout_input(true), Err(Fault::Prohibited));
        p.write(BOD_BASE + 4, u32::MAX).unwrap(); assert_eq!(p.read(BOD_BASE + 4), Ok(1 << 31));
        assert_eq!(p.brownout_input(true), Err(Fault::Prohibited)); assert_eq!(p.reset_request, None);
        p.write(BOD_BASE + 0x1c, !2u32).unwrap();
        p.brownout_input(false).unwrap(); assert_eq!(p.reset_request, None);
        p.write(BOD_BASE + 4, 0).unwrap(); p.brownout_input(true).unwrap(); assert_eq!(p.reset_request, None);
        p.write(BOD_BASE + 4, 1 << 31).unwrap(); p.brownout_input(true).unwrap();
        assert_eq!(p.reset_request, Some(ResetCause::Brownout));
        p.brownout_input(false).unwrap(); assert_eq!(p.reset_request, Some(ResetCause::Brownout));
        p.write(BOD_BASE + 0x18, u32::MAX).unwrap(); assert_eq!(p.read(BOD_BASE + 0x18), Ok(0xfc00_0000));
        // Mode0 IRQs and comparator qualification are not silently represented by mode1.
        assert_eq!(p.write(BOD_BASE, 1 << 29), Err(Fault::Unmapped));
    }
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
