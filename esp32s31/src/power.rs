//! S31 functional CPLL/FOSC controls, MPLL shutdown and separate BOD mode0/mode1 inputs.
//! No analog PLL output, voltage/threshold waveform, glitch detection or PMU sleep transitions.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643, soc/esp32s31/register/soc/
//! {pmu_reg,hp_alive_sys_reg,lp_analog_peri_reg}.h, soc/esp32s31/include/soc/reset_reasons.h,
//! esp_hal_clock/esp32s31/include/hal/clk_tree_ll.h and esp_hal_pmu/esp32s31/include/hal/brownout_ll.h.
use emu_core::Fault;
use crate::pmu_policy::PmuPolicy;
const ACTIVE_CK: u32 = 0x2070_401c;
const IMMEDIATE_CK: u32 = 0x2070_40f4;
const SLEEP_LP_CK: u32 = 0x2070_40d0;
const HP_INT_ENA: u32 = 0x2070_4194;
const HP_INT_CLR: u32 = 0x2070_4198;
const ALIVE_CK: u32 = 0x2058_9000;
pub const BOD_BASE: u32 = 0x2070_2000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetCause { Brownout } // reset_reasons.h: RESET_REASON_SYS_BROWN_OUT=15.
const CPLL_LOW: u32 = 0x111;
const CPLL_HIGH: u32 = (1 << 19) | (1 << 23) | (1 << 27);
const MPLL_HIGH: u32 = (1 << 22) | (1 << 26) | (1 << 30);
const MPLL_LOW: u32 = (1 << 3) | (1 << 7) | (1 << 11);
pub struct Power { pub policy: PmuPolicy, pub mode0: crate::brownout::Brownout, cpll: u32, sleep_lp: u32, active: u32, alive: u32, mpll_high: u32, mpll_low: u32, fib: u32, bod1: u32, glitch: u32, pub reset_request: Option<ResetCause> }
impl Default for Power {
    fn default() -> Self { Self { policy: PmuPolicy::default(), mode0: crate::brownout::Brownout::default(), cpll: 0, sleep_lp: 1 << 30, active: 0, alive: 0xfffe_0000, mpll_high: 0, mpll_low: 0, fib: u32::MAX, bod1: 0, glitch: 0, reset_request: None } }
}

impl Power {
    pub fn is_mmio(address: u32) -> bool { crate::brownout::Brownout::is_mmio(address & !3) || PmuPolicy::is_mmio(address) || matches!(address & !3, ACTIVE_CK | IMMEDIATE_CK | ALIVE_CK | SLEEP_LP_CK | HP_INT_ENA | HP_INT_CLR) || matches!((address & !3).wrapping_sub(BOD_BASE), 4 | 0x18 | 0x1c) }
    /// Explicit software shutdown requests, not a measured oscillator state.
    pub fn bbpll_output_enabled(&self) -> bool { self.alive & (1 << 30) != 0 }
    pub fn fosc_enabled(&self) -> bool { self.sleep_lp & (1 << 30) != 0 }
    pub fn cpll_enabled(&self) -> bool { self.cpll == CPLL_LOW && self.alive & (1 << 29) != 0 }
    pub fn mpll_enabled(&self) -> bool { self.mpll_high == MPLL_LOW && self.alive & (1 << 31) != 0 && self.active & 0x4400_0000 == 0x4400_0000 }
    pub fn mpll_disabled(&self) -> bool { self.mpll_low == MPLL_LOW && self.alive & (1 << 31) == 0 && self.active & 0x4400_0000 == 0 }
    /// A caller-supplied already-qualified BOD comparator event, not a measured voltage
    /// or an analog threshold model. FIB/eFuse-controlled policy is still unknown.
    pub fn brownout_input(&mut self, tripped: bool) -> Result<(), Fault> {
        if !tripped { return Ok(()); }
        if self.fib & 2 != 0 { return Err(Fault::Prohibited); }
        if self.bod1 != 0 { self.reset_request = Some(ResetCause::Brownout); }
        Ok(())
    }
    /// Separate detector-mode0 input. Its source/threshold are already qualified
    /// by the caller; unknown FIB or active higher-priority mode1 cannot be bypassed.
    pub fn brownout_mode0_ticks(&mut self, tripped: bool, ticks: u64) -> Result<(), Fault> {
        if tripped && ticks != 0 && (self.fib & 2 != 0 || self.bod1 != 0) { return Err(Fault::Prohibited); }
        if self.mode0.advance(tripped, ticks)? { self.reset_request = Some(ResetCause::Brownout); }
        Ok(())
    }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        if crate::brownout::Brownout::is_mmio(address) { return self.mode0.read(address); }
        if PmuPolicy::is_mmio(address) { return self.policy.read(address); }
        match address { HP_INT_ENA | HP_INT_CLR => Ok(0), SLEEP_LP_CK => Ok(self.sleep_lp), ACTIVE_CK => Ok(self.active), ALIVE_CK => Ok(self.alive), IMMEDIATE_CK => Ok(0),
            a if a == BOD_BASE + 4 => Ok(self.bod1), a if a == BOD_BASE + 0x18 => Ok(self.glitch),
            a if a == BOD_BASE + 0x1c => Ok(self.fib), _ => Err(Fault::Unmapped) }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if crate::brownout::Brownout::is_mmio(address) { return self.mode0.write(address, value); }
        if PmuPolicy::is_mmio(address) { return self.policy.write(address, value); }
        match address {
            // PMU sleep transitions remain unsupported. Mode0 has its own
            // explicit comparator/edge input and separate interrupt banks.
            HP_INT_ENA => { if value & 0xf800_0000 != 0 { return Err(Fault::Prohibited); } }
            HP_INT_CLR => {},
            SLEEP_LP_CK => {
                // v6.1 clk_ll_rc_fast_enable/disable uses this bit even while awake.
                // XTAL32K/RC32K and sleep oscillator shutdown are not implemented.
                if value & 0xb000_0000 != 0 { return Err(Fault::Prohibited); }
                self.sleep_lp = value & (1 << 30);
            }
            IMMEDIATE_CK => {
                // Group field definitions and the driver use tie-high/low commands.
                // Some generated per-CPLL aliases say RW instead of WT; model the group
                // WT protocol, not latched command bits or an analog PLL waveform.
                if value & !(MPLL_LOW | MPLL_HIGH | CPLL_LOW | CPLL_HIGH) != 0 { return Err(Fault::Prohibited); }
                let high = ((value >> 23) & 0xf) | ((value >> 23) & 0xf0) | ((value >> 11) & 0xf00);
                let low = value & (CPLL_LOW | MPLL_LOW);
                if high & low != 0 { return Err(Fault::Prohibited); }
                self.cpll = (self.cpll | (high & CPLL_LOW)) & !low;
                self.mpll_high = (self.mpll_high | (high & MPLL_LOW)) & !low;
                self.mpll_low = (self.mpll_low | (low & MPLL_LOW)) & !high;
            }
            ALIVE_CK => {
                // Preserve other domain enables. Turning off CPU/root clocks cannot be
                // represented by continuing execution, so such writes fail explicitly.
                if (value ^ self.alive) & !((1 << 31) | (1 << 30) | (1 << 29)) != 0 { return Err(Fault::Prohibited); }
                self.alive = value;
            }
            ACTIVE_CK => {
                // Store documented mode policy; this does not replace immediate PLL tie controls.
                self.active = value & 0x7ff8_0000;
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
    fn cpll_power_commands_require_all_gates_and_irq_cleanup_cannot_enable_missing_sources() {
        let mut p = Power::default(); assert!(!p.cpll_enabled());
        p.write(IMMEDIATE_CK, 1 << 23 | 1 << 27).unwrap(); assert!(!p.cpll_enabled());
        p.write(IMMEDIATE_CK, 1 << 19).unwrap(); assert!(p.cpll_enabled());
        assert_eq!(p.read(IMMEDIATE_CK), Ok(0));
        assert_eq!(p.write(IMMEDIATE_CK, 1 << 19 | 1 << 8), Err(Fault::Prohibited)); assert!(p.cpll_enabled());
        p.write(ALIVE_CK, 0xdffe_0000).unwrap(); assert!(!p.cpll_enabled());
        p.write(ALIVE_CK, 0xfffe_0000).unwrap(); assert!(p.cpll_enabled());
        p.write(IMMEDIATE_CK, 0x111).unwrap(); assert!(!p.cpll_enabled());
        assert!(p.fosc_enabled()); p.write(SLEEP_LP_CK, 0).unwrap(); assert!(!p.fosc_enabled());
        assert_eq!(p.write(SLEEP_LP_CK, 1 << 28), Err(Fault::Prohibited));
        p.write(SLEEP_LP_CK, 1 << 30).unwrap(); assert!(p.fosc_enabled());
        {
            let address = HP_INT_ENA;
            assert_eq!(p.read(address), Ok(0)); p.write(address, 0).unwrap();
            assert_eq!(p.write(address, 1 << 31), Err(Fault::Prohibited)); assert_eq!(p.read(address), Ok(0));
        }
        p.write(HP_INT_CLR, u32::MAX).unwrap(); p.write(BOD_BASE + 0x3c, u32::MAX).unwrap();
        assert_eq!(p.read(HP_INT_CLR), Ok(0)); assert_eq!(p.read(BOD_BASE + 0x3c), Ok(0)); assert_eq!(p.reset_request, None);
    }
    #[test]
    fn mpll_tie_commands_and_active_gate_are_independent_from_cpll() {
        let mut p = Power::default(); p.write(IMMEDIATE_CK, CPLL_HIGH).unwrap(); assert!(p.cpll_enabled());
        p.write(IMMEDIATE_CK, 1 << 22).unwrap(); assert!(!p.mpll_enabled());
        p.write(IMMEDIATE_CK, (1 << 26) | (1 << 30)).unwrap(); assert!(!p.mpll_enabled());
        p.write(ACTIVE_CK, 0x4400_0000).unwrap(); assert!(p.mpll_enabled()); assert!(p.cpll_enabled());
        assert_eq!(p.write(IMMEDIATE_CK, (1 << 22) | (1 << 11)), Err(Fault::Prohibited)); assert!(p.mpll_enabled());
        p.write(ALIVE_CK, 0x7ffe_0000).unwrap(); assert!(!p.mpll_enabled());
        p.write(ALIVE_CK, 0xfffe_0000).unwrap(); assert!(p.mpll_enabled());
        p.write(IMMEDIATE_CK, MPLL_LOW).unwrap(); assert!(!p.mpll_enabled()); assert!(p.cpll_enabled());
        p.write(ALIVE_CK, 0x7ffe_0000).unwrap(); p.write(ACTIVE_CK, 0).unwrap(); assert!(p.mpll_disabled());
    }
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
        // Mode0 cannot silently bypass the higher-priority analog reset path.
        p.write(BOD_BASE, 1 << 29).unwrap();
        assert_eq!(p.brownout_mode0_ticks(true, 1), Err(Fault::Prohibited));
        assert_eq!(p.mode0.read(BOD_BASE + 0x20), Ok(0));
    }
    #[test]
    fn mpll_shutdown_requires_the_actual_sequence_and_does_not_enable_unknown_plls() {
        let mut p = Power::default(); assert!(!p.mpll_disabled());
        p.write(IMMEDIATE_CK, 1 << 11).unwrap(); assert_eq!(p.read(IMMEDIATE_CK), Ok(0));
        p.write(ALIVE_CK, 0x7ffe_0000).unwrap(); assert!(!p.mpll_disabled());
        p.write(IMMEDIATE_CK, 0x88).unwrap(); assert!(p.mpll_disabled());
        p.write(ACTIVE_CK, 0x4400_0000).unwrap(); assert!(!p.mpll_disabled());
        p.write(ACTIVE_CK, 0).unwrap(); assert!(p.mpll_disabled());
        assert_eq!(p.write(IMMEDIATE_CK, 1 << 29), Err(Fault::Prohibited)); // APLL remains absent.
        assert_eq!(p.write(ALIVE_CK, 0), Err(Fault::Prohibited));
        p.write(ACTIVE_CK, 1 << 27).unwrap(); assert_eq!(p.read(ACTIVE_CK), Ok(1 << 27));
        assert!(p.mpll_disabled()); assert_eq!(p.read(ALIVE_CK), Ok(0x7ffe_0000));
        assert_eq!(p.read(IMMEDIATE_CK + 4), Ok(0)); // PMU update commands are WT.
    }
}
