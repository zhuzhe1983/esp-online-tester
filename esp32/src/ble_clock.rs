//! Separate BLE sampler (+21c/+220), NOT an alias of the native +1c/+20 bank.
//! ROM r_rwble_reset and r_rwip_check_wakeup_boundary request/poll this bank.
//! Tarlogic esp-pacs284111a6 BLEBASETIMECNT:27-bit625us counter; FINECNT:10bits.
//! Functional fine convention: descending624..0 at nominal1MHz, uncalibrated.
//! Reuse the tested sampler mechanics at half its reference rate, own state/phase.
use crate::bt_clock::BtNativeClock;
pub struct BleClock {
    inner: BtNativeClock,
    half_phase: u64,
}
impl Default for BleClock {
    fn default() -> Self {
        Self {
            inner: BtNativeClock::with_latency_ticks(120),
            half_phase: 0,
        }
    }
}
impl BleClock {
    pub fn set_control(&mut self, on: bool, reset: bool) {
        if reset {
            self.half_phase = 0;
        }
        self.inner.set_control(on, reset);
    }
    pub fn clock_enabled(&self) -> bool {
        self.inner.clock_enabled()
    }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x21c => self.inner.read(0x1c) & 0x87ff_ffff,
            0x220 => self.inner.read(0x20),
            _ => 0,
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x21c => self.inner.write(0x1c, v),
            0x220 => self.inner.write(0x20, v),
            _ => (),
        }
    }
    pub fn completed(&self) -> u64 {
        self.inner.completed
    }
    pub fn advance(&mut self, cycles: u64) {
        if !self.clock_enabled() {
            return;
        }
        let rem = cycles % 2 + self.half_phase;
        self.inner.advance(cycles / 2 + rem / 2);
        self.half_phase = rem % 2;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ble_slot_units_phase_busy_gate_and_wrap() {
        let mut c = BleClock::default();
        c.set_control(true, false);
        c.advance(150000 * 0x07ff_ffff);
        c.write(0x21c, 1 << 31);
        c.advance(239);
        assert_ne!(c.read(0x21c) >> 31, 0);
        c.advance(1);
        assert_eq!(c.read(0x21c), 0x07ff_ffff);
        c.advance(150000);
        c.write(0x21c, 1 << 31);
        c.set_control(false, false);
        c.advance(u64::MAX);
        assert_ne!(c.read(0x21c) >> 31, 0);
        c.set_control(true, false);
        c.advance(240);
        assert_eq!(c.read(0x21c), 0);
        assert!(c.read(0x220) <= 624);
    }
    #[test]
    fn soc_native_and_ble_samples_have_distinct_state_and_units() {
        let mut p = crate::periph::Peripherals::new([0; 6]);
        p.write32(0x3ff000cc, 0x30800);
        p.tick(240000);
        p.write32(0x3ff7101c, 1 << 31);
        p.write32(0x3ff7121c, 1 << 31);
        p.tick(240);
        assert_eq!(p.read32(0x3ff7101c), 3);
        assert_eq!(p.read32(0x3ff7121c), 1);
        assert_eq!(p.bt_clock.completed, 1);
        assert_eq!(p.ble_clock.completed(), 1);
        p.write32(0x3ff000d0, 1 << 9);
        assert_eq!(p.read32(0x3ff7121c), 0);
    }
}
