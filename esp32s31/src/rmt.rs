//! S31 RMT TX: shared symbol engine with S31 HP_SYS_CLKRST clock selection.
//! IDF v6.1 fff9895c soc/esp32s31/{register/soc/rmt_reg.h,ld/esp32s31.peripherals.ld},
//! esp_hal_rmt/esp32s31/include/hal/rmt_ll.h: TX map and 48-word channels match shared RMT.
pub const BASE: u32 = 0x2035_5000;
pub const CLOCK: u32 = 0x2058_70bc;
pub const IRQ_SOURCE: usize = 22;
pub const SIGNAL0: u32 = 246; // soc/esp32s31/include/soc/gpio_sig_map.h
pub struct Rmt { pub inner: esp_periph::Rmt, clock: u32, fraction: u128 }
impl Default for Rmt { fn default() -> Self { Self { inner: esp_periph::Rmt::new(80_000_000), clock: 0, fraction: 0 } } }
impl Rmt {
    pub fn is_mmio(a: u32) -> bool { (BASE..BASE + 0xe00).contains(&a) || a & !3 == CLOCK }
    pub fn read(&self, a: u32) -> u32 { if a == CLOCK { self.clock } else { self.inner.read(a.wrapping_sub(BASE)) } }
    pub fn write(&mut self, a: u32, v: u32) {
        if a == CLOCK { self.clock = v; if v & 2 != 0 { self.inner = esp_periph::Rmt::new(80_000_000); self.fraction = 0; } }
        else { self.inner.write(a.wrapping_sub(BASE), v); }
    }
    pub fn running(&self) -> bool { self.clock & 0x23 == 0x21 && self.inner.is_running() }
    pub fn irq(&self) -> bool { self.inner.irq() }
    fn rate(&self, pll80: Option<u64>, rc_fast: u64) -> Option<(u64, u64)> {
        if self.clock & 0x23 != 0x21 { return None; }
        let source = match self.clock >> 3 & 3 { 0 => 40_000_000, 1 => rc_fast, 2 => pll80?, _ => return None };
        let a = u64::from(self.clock >> 22 & 255).max(1); let b = u64::from(self.clock >> 14 & 255);
        (source != 0).then_some((source * a, (u64::from(self.clock >> 6 & 255) + 1) * a + b))
    }
    pub fn tick(&mut self, cycles: u64, pll80: Option<u64>, rc_fast: u64) {
        let Some((num, den)) = self.rate(pll80, rc_fast) else { return };
        let denominator = u128::from(den) * u128::from(crate::periph::CPU_HZ);
        let total = self.fraction + u128::from(cycles) * u128::from(num);
        self.fraction = total % denominator; self.inner.tick((total / denominator).min(u128::from(u64::MAX)) as u64);
    }
    /// Nanoseconds per complete pulse and remaining pulses, for the board's nanosecond clock.
    pub fn finite_clock(&self, channel: usize, pll80: Option<u64>, rc_fast: u64) -> Option<(u64, u32)> {
        let (num, den) = self.rate(pll80, rc_fast)?; let (ticks, remaining) = self.inner.finite_clock(channel)?;
        let ns = (u128::from(ticks) * u128::from(den) * 1_000_000_000).div_ceil(u128::from(num));
        Some((ns.min(u128::from(u64::MAX)) as u64, remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_ckv_uses_s31_group_clock_and_nanoseconds() {
        let mut r = Rmt::default();
        // Official INK4 CKV: channel1, 10 MHz resolution, 15.9 us high + 1.1 us low.
        r.write(CLOCK, 0x21 | (2 << 3));
        r.write(BASE + 0x800 + 48 * 4, 0x8000 | 159 | (11 << 16));
        r.write(BASE + 0xa4, (6 << 9) | (1 << 19) | (1 << 21));
        r.write(BASE + 0x24, (8 << 8) | (2 << 16) | (1 << 3) | 1);
        r.write(BASE + 0x78, (1 << 1) | (1 << 13));
        assert_eq!(r.finite_clock(1, Some(80_000_000), 0), Some((17_000, 6)));
        r.tick(5440, Some(80_000_000), 0);
        let (_, remaining) = r.finite_clock(1, Some(80_000_000), 0).unwrap(); assert!(remaining < 6);
        r.tick(5440 * 6, Some(80_000_000), 0); assert!(!r.running()); assert!(r.irq());
        r.write(BASE + 0x7c, u32::MAX); assert!(!r.irq());
        r.write(BASE + 0x24, (8 << 8) | (2 << 16) | (1 << 3) | 1);
        r.write(CLOCK, 0); assert!(!r.running()); assert_eq!(r.finite_clock(1, Some(80_000_000), 0), None);
        r.write(CLOCK, 0x23); assert!(!r.inner.is_running());
    }
}
