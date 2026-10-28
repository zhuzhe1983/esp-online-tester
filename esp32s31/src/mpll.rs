//! S31 functional MPLL control, not an analog lock or silicon settling model.
//! ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643:
//! lp_clkrst_reg.h MSPI_DIV(+0x54),hp_sys_clkrst_reg.h ANA_PLL_CTRL0(+0x174),
//! esp_hal_clock/esp32s31/include/hal/clk_tree_ll.h and rtc_clk.c.
use emu_core::Fault;
pub struct Mpll { div: u32, powered: bool, stop: bool, done: bool, left: Option<u8> }
impl Default for Mpll {
    fn default() -> Self { Self { div: 0xc1, powered: false, stop: false, done: false, left: None } }
}
impl Mpll {
    pub fn divider(&self) -> u32 { self.div }
    pub fn write_divider(&mut self, value: u32) -> Result<(), Fault> {
        // LL uses reference1 (divide2); charge-pump/filter tuning is unsupported.
        if value & 0x307 != 1 { return Err(Fault::Prohibited); }
        self.div = value & 0x3ff; self.done = false;
        if self.left.is_some() { self.left = Some(16); }
        Ok(())
    }
    pub fn set_power(&mut self, on: bool) { self.powered = on; if !on { self.done = false; self.left = None; } }
    pub fn control(&self) -> u32 { u32::from(self.done) << 8 | u32::from(self.stop) << 9 }
    pub fn write_control(&mut self, value: u32) {
        self.stop = value & (1 << 9) != 0;
        if self.stop { self.left = None; } else { self.done = false; self.left = Some(16); }
    }
    /// Sixteen private runner quanta, not measured PLL settling time. Completion
    /// requires power, an explicit start, supported dividers and analog-master clock.
    pub fn advance(&mut self, quanta: u32, analog_clock: bool) {
        if !self.powered || self.stop || !analog_clock { return; }
        if let Some(left) = self.left {
            if quanta >= u32::from(left) { self.left = None; self.done = true; }
            else { self.left = Some(left - quanta as u8); }
        }
    }
    pub fn output_hz(&self) -> Option<u64> {
        if self.powered && self.done { Some(40_000_000 * u64::from(((self.div >> 3) & 31) + 1) / 2) } else { None }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mpll_needs_start_power_analog_clock_and_elapsed_quanta_not_a_written_done_bit() {
        let mut m = Mpll::default(); assert_eq!(m.divider(), 0xc1);
        m.set_power(true); m.advance(100, true); assert_eq!(m.output_hz(), None);
        m.write_control(0x300); m.advance(100, true); assert_eq!(m.control(), 0x200);
        m.write_control(0x100); m.advance(100, false); assert_eq!(m.control(), 0);
        m.advance(15, true); assert_eq!(m.output_hz(), None); m.advance(1, true); assert_eq!(m.output_hz(), Some(500_000_000));
        m.write_control(0x300); assert_eq!(m.control(), 0x300);
        m.set_power(false); assert_eq!(m.control(), 0x200); m.set_power(true); m.advance(100, true); assert_eq!(m.output_hz(), None);
        m.write_control(0); m.advance(4, true); m.write_control(0x200); m.advance(100, true); assert_eq!(m.output_hz(), None);
    }
    #[test]
    fn mpll_divider_is_real_state_and_invalid_tuning_does_not_mutate_it() {
        let mut m = Mpll::default(); m.set_power(true); m.write_control(0);
        m.advance(15, true); m.write_divider((19 << 3) | 1).unwrap();
        m.advance(1, true); assert_eq!(m.output_hz(), None); m.advance(15, true); assert_eq!(m.output_hz(), Some(400_000_000));
        for bad in [0, 2, 0x1c1, 0x2c1] { assert_eq!(m.write_divider(bad), Err(Fault::Prohibited)); assert_eq!(m.output_hz(), Some(400_000_000)); }
        m.set_power(false); assert_eq!(m.output_hz(), None);
    }
}
