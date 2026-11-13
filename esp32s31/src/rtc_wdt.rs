//! S31 RTC watchdog registers and a four-stage counter driven by explicit RTC slow-clock ticks.
// ESP-IDF v6.1 fff9895c: soc/esp32s31/register/soc/rtc_wdt_reg.h and
// esp_hal_wdt/esp32s31/include/hal/rwdt_ll.h. No silicon timing or reset-domain
// implementation is implied: this block emits a typed reset request for the caller.
pub const BASE: u32 = 0x2080_1000;
const KEY: u32 = 0x50d8_3aa1;
const LP_IRQ: u32 = 1 << 31;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetRequest { Cpu { core0: bool, core1: bool }, System, Rtc }
pub struct RtcWdt {
    conf: u32, hold: [u32; 4], key: u32, swd: u32, swd_key: u32, raw: u32, ena: u32,
    pause: u32, date: u32, stage: usize, elapsed: u64,
    /// eFuse WDT_DELAY_SEL is not provisioned by the bring-up loader yet. Zero is the
    /// unprogrammed model input; it is not a claim about a product's physical eFuses.
    pub delay_sel: u8,
    /// Boot-mode logic must explicitly enter/leave flashboot; CONFIG0 alone does not
    /// assert that a flash boot occurred.
    pub flashboot: bool,
    pub reset_request: Option<ResetRequest>,
}
impl Default for RtcWdt {
    fn default() -> Self { Self { conf: 0x13200, hold: [200_000, 80_000, 4095, 4095], key: 0,
        swd: 300 << 20, swd_key: 0, raw: 0, ena: 0, pause: 1 << 31, date: 38_818_560,
        stage: 0, elapsed: 0, delay_sel: 0, flashboot: false, reset_request: None } }
}
impl RtcWdt {
    pub fn irq(&self) -> bool { self.raw & self.ena != 0 }
    fn feed(&mut self) { self.stage = 0; self.elapsed = 0; }
    /// Explicit clock-domain input until the S31 clock tree is installed. No host wall time
    /// or assumed CPU/RTC conversion is used. A reset request stops the counter.
    pub fn advance(&mut self, ticks: u64, sleeping: bool, debug_halted: bool) {
        self.advance_with_cpu_gates(ticks, sleeping, debug_halted, [true, true]);
    }
    /// The standalone block emits CPU reset outputs. The S31 bus additionally
    /// gates delivery through LP_AONCLKRST_HPCOREn_ENABLE_LP_WDT_RESET.
    pub fn advance_with_cpu_gates(&mut self, ticks: u64, sleeping: bool, debug_halted: bool, gates: [bool; 2]) {
        if ticks == 0 || self.reset_request.is_some() || (self.conf & (1 << 31) == 0 && !(self.flashboot && self.conf & (1 << 12) != 0))
            || sleeping && self.conf & (1 << 9) != 0 || debug_halted && self.pause != 0 { return; }
        self.elapsed = self.elapsed.saturating_add(ticks);
        // One finite stage sequence. Disabled actions emit no interrupt/reset; no fabricated
        // source is asserted. Feeding or enabling starts stage zero again.
        for _ in 0..4 {
            if self.stage >= 4 { break; }
            let timeout = u64::from(self.hold[self.stage]) << if self.stage == 0 { 1 + (self.delay_sel & 3) } else { 0 };
            if self.elapsed < timeout { break; }
            self.elapsed -= timeout;
            let action = (self.conf >> (28 - 3 * self.stage)) & 7; self.stage += 1;
            match action {
                1 => self.raw |= LP_IRQ,
                2 => {
                    let core0 = gates[0] && self.conf & (1 << 11) != 0; let core1 = gates[1] && self.conf & (1 << 10) != 0;
                    if core0 || core1 { self.reset_request = Some(ResetRequest::Cpu { core0, core1 }); }
                }
                3 => self.reset_request = Some(ResetRequest::System),
                4 => self.reset_request = Some(ResetRequest::Rtc),
                _ => {}, // reserved action values never panic or invent a reset cause
            }
            if self.reset_request.is_some() { break; }
        }
    }
    pub fn read(&self, off: u32) -> Option<u32> {
        match off {
            0 => Some(self.conf), 4 | 8 | 0xc | 0x10 => Some(self.hold[(off / 4 - 1) as usize]),
            0x14 | 0x30 => Some(0), 0x18 => Some(self.key), 0x1c => Some(self.swd), 0x20 => Some(self.swd_key),
            0x24 => Some(self.raw), 0x28 => Some(self.raw & self.ena), 0x2c => Some(self.ena),
            0x34 => Some(self.pause), 0x3fc => Some(self.date), _ => None,
        }
    }
    pub fn write(&mut self, off: u32, value: u32) -> bool {
        match off {
            0 => if self.key == KEY {
                if (self.conf ^ value) & (1 << 31) != 0 { self.feed(); }
                self.conf = value & 0xffff_fe00;
            }
            4 | 8 | 0xc | 0x10 => if self.key == KEY { self.hold[(off / 4 - 1) as usize] = value; }
            0x14 => if self.key == KEY && value & (1 << 31) != 0 { self.feed(); }
            0x18 => self.key = value,
            0x1c => {
                // The ROM clears this flag before unlocking SWD configuration. The reset
                // latch/clear path is modeled; missing-clock super-WDT timing is not.
                if value & (1 << 19) != 0 { self.swd &= !1; }
                if self.swd_key == KEY { self.swd = (self.swd & 1) | (value & 0x7ff4_0000); }
            }
            0x20 => self.swd_key = value,
            0x24 | 0x30 => self.raw &= !(value & 0xc000_0000),
            0x28 => {}, 0x2c => self.ena = value & 0xc000_0000,
            0x34 => self.pause = value & (1 << 31), 0x3fc => self.date = value,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_configuration_stage_zero_multiplier_and_feed() {
        let mut w = RtcWdt::default(); w.write(4, 4); assert_eq!(w.read(4), Some(200_000));
        w.write(0x18, KEY); w.write(4, 4); w.write(8, 3); w.write(0, 1 << 31 | 1 << 28 | 3 << 25); w.write(0x2c, LP_IRQ);
        w.advance(7, false, false); assert!(!w.irq()); w.write(0x14, 1 << 31);
        w.advance(7, false, false); assert!(!w.irq()); w.advance(1, false, false); assert!(w.irq());
        w.write(0x30, LP_IRQ); assert!(!w.irq()); w.advance(2, false, false); assert_eq!(w.reset_request, None);
        w.advance(1, false, false); assert_eq!(w.reset_request, Some(ResetRequest::System));
    }
    #[test]
    fn flashboot_and_pause_inputs_do_not_depend_on_host_time() {
        let mut w = RtcWdt::default(); w.write(0x18, KEY); w.write(4, 1); w.write(0, 1 << 28 | 1 << 12 | 1 << 9); w.write(0x2c, LP_IRQ);
        w.delay_sel = 3; w.advance(100, false, false); assert!(!w.irq());
        w.flashboot = true; w.advance(100, true, false); w.advance(100, false, true); assert!(!w.irq());
        w.advance(15, false, false); assert!(!w.irq()); w.advance(1, false, false); assert!(w.irq());
    }
    #[test]
    fn super_reset_flag_is_read_only_and_clears_without_configuration_unlock() {
        let mut w = RtcWdt::default(); w.swd |= 1;
        w.write(0x1c, 0); assert_eq!(w.read(0x1c), Some(300 << 20 | 1));
        w.write(0x1c, 1 << 19); assert_eq!(w.read(0x1c), Some(300 << 20));
        w.write(0x1c, 1); assert_eq!(w.read(0x1c), Some(300 << 20));
        w.write(0x18, KEY); w.write(0, u32::MAX); w.write(4, 0); w.write(8, 0); w.write(0xc, 0); w.write(0x10, 0);
        w.advance(u64::MAX, false, false); assert_eq!(w.reset_request, None);
    }
    #[test]
    fn cpu_reset_selects_only_enabled_targets_and_zero_ticks_do_nothing() {
        for targets in 0..4 {
            let mut w = RtcWdt::default(); w.write(0x18, KEY); w.write(4, 0); w.write(0, 1 << 31 | 2 << 28 | targets << 10);
            w.advance(0, false, false); assert_eq!(w.reset_request, None);
            w.advance(1, false, false);
            assert_eq!(w.reset_request, if targets == 0 { None } else { Some(ResetRequest::Cpu { core0: targets & 2 != 0, core1: targets & 1 != 0 }) });
        }
    }
    #[test]
    fn disabled_receiver_gates_allow_later_watchdog_stages_to_run() {
        for gates in [[false, false], [true, false], [false, true], [true, true]] {
            let mut w = RtcWdt::default(); w.write(0x18, KEY); w.write(4, 1); w.write(8, 3);
            w.write(0, 1 << 31 | 2 << 28 | 3 << 25 | 3 << 10);
            w.advance_with_cpu_gates(2, false, false, gates);
            if gates == [false, false] {
                assert_eq!(w.reset_request, None); w.advance_with_cpu_gates(3, false, false, gates);
                assert_eq!(w.reset_request, Some(ResetRequest::System));
            } else { assert_eq!(w.reset_request, Some(ResetRequest::Cpu { core0: gates[0], core1: gates[1] })); }
        }
    }
}
