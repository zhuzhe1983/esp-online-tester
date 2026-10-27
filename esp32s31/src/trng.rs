//! S31 gated CRC random-data path, with reproducible simulated noise rather than physical entropy.
use esp_periph::{device::Device, rng::Rng};

// ESP-IDF v6.1 fff9895c: soc/esp32s31/register/soc/{reg_base,trng_reg,lp_peri_clkrst_reg}.h
// and hal/esp32s31/include/hal/{rng_ll,trng_ll}.h. ROM 20260528 reads CRC_DATA;
// the current IDF LL reads CRC_SYNC_DATA. Both are functional sample points here:
// no oscillator, CRC polynomial, synchronizer delay or cryptographic quality is modeled.
pub const TRNG_BASE: u32 = 0x2081_4000;
pub const RNG_CTRL: u32 = 0x2071_003c;
const SAMPLE: u32 = 1 << 31;
const NOISE_CRC: u32 = 1 << 30;
const MODULE_CLOCK: u32 = 1 << 28;
const BUS_CLOCK: u32 = 1 << 30;
const RESET: u32 = 1 << 31;

pub struct Trng { conf: u32, date: u32, ctrl: u32, data: u32, noise: Rng }
impl Default for Trng {
    fn default() -> Self { Self { conf: NOISE_CRC, date: 2_426_672, ctrl: BUS_CLOCK, data: 0, noise: Rng::new() } }
}
impl Trng {
    pub fn ctrl_read(&self) -> u32 { self.ctrl }
    pub fn ctrl_write(&mut self, value: u32) {
        self.ctrl = value & (BUS_CLOCK | RESET);
        if self.ctrl & RESET != 0 {
            // Reset the sampling logic and pending output. Keep the register clock control:
            // the real ROM enables TRNG_DATE.CLK_EN before pulsing this reset.
            self.conf = NOISE_CRC; self.data = 0;
        }
    }
    fn active(&self) -> bool {
        self.ctrl & (BUS_CLOCK | RESET) == BUS_CLOCK && self.date & MODULE_CLOCK != 0 && self.conf & (SAMPLE | NOISE_CRC) == SAMPLE | NOISE_CRC
    }
    pub fn read(&mut self, off: u32) -> Option<u32> {
        match off {
            0 => Some(self.conf),
            0x44 | 0x48 => { if self.active() { self.data = self.noise.read(0); } Some(self.data) }
            0xfc => Some(self.date),
            // Health tests, conditioning keys, arbitration and request/FIFO output have not
            // been implemented. Do not invent ready/success values for those consumers.
            _ => None,
        }
    }
    pub fn write(&mut self, off: u32, value: u32) -> bool {
        match off {
            0 => {
                if self.ctrl & RESET == 0 { self.conf = value & !(1 << 28); }
                // FIFO reset is write-triggered, not a stored enable. This subset has only
                // the sampled data latch; the FIFO consumer remains explicitly unmapped.
            }
            0x44 | 0x48 => {}, // read-only data
            0xfc => self.date = value & 0x1fff_ffff,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_output_requires_all_gates_and_reset_discards_the_sample() {
        let mut r = Trng::default(); assert_eq!(r.read(0x44), Some(0));
        r.write(0, SAMPLE | NOISE_CRC); assert_eq!(r.read(0x44), Some(0));
        r.write(0xfc, MODULE_CLOCK); let first = r.read(0x44).unwrap(); let second = r.read(0x48).unwrap();
        assert_ne!(first, 0); assert_ne!(first, second);
        r.ctrl_write(0); assert_eq!(r.read(0x44), Some(second));
        r.ctrl_write(BUS_CLOCK); r.write(0, SAMPLE); assert_eq!(r.read(0x44), Some(second));
        r.write(0, SAMPLE | NOISE_CRC); r.write(0xfc, 0); assert_eq!(r.read(0x48), Some(second));
        r.ctrl_write(BUS_CLOCK | RESET); assert_eq!(r.read(0x44), Some(0));
        r.write(0xfc, MODULE_CLOCK); r.write(0, SAMPLE | NOISE_CRC); assert_eq!(r.read(0), Some(NOISE_CRC));
        r.ctrl_write(BUS_CLOCK); assert_eq!(r.read(0x44), Some(0));
        r.write(0, SAMPLE | NOISE_CRC); assert_ne!(r.read(0x44), Some(0));
    }
    #[test]
    fn unsupported_health_and_conditioning_remain_explicit() {
        let mut r = Trng::default();
        for off in [4, 8, 0x28, 0x38, 0x3c, 0x40] { assert_eq!(r.read(off), None); assert!(!r.write(off, u32::MAX)); }
        assert_eq!(r.read(0xfc), Some(2_426_672)); r.write(0x44, 123); assert_eq!(r.read(0x44), Some(0));
    }
}
