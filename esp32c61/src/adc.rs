//! ESP32-C61 SAR one-shot ADC, including the internal-ground calibration path.
//! ESP-IDF v5.5.4 hal/esp32c61/include/hal/adc_ll.h; soc/esp32c61/adc_periph.c,
//! register/soc/apb_saradc_reg.h and include/soc/regi2c_saradc.h.
use esp_periph::{AnalogInputs, Device, RegRam, WriteEffect};
pub const PINS: [u8; 4] = [1, 3, 4, 5];
/// Nominal zero-difference eFuse transfer. Coefficients from ESP-IDF v5.5.4
/// esp_adc/esp32c61/curve_fitting_coefficients.c (Espressif 2025, Apache-2.0;
/// license in esp-periph/LICENSE-ADC), calibration voltages from esp_efuse_rtc_calib.c.
fn millivolts(raw: u32, atten: usize) -> i64 {
    const COEFF: [[(u64, u64, i64); 3]; 4] = [
        [(8668885650149671, 10_000_000_000_000_000, -1), (15630376830615, 10_000_000_000_000_000, 1), (0, 1, 1)],
        [(1090569589734153, 1_000_000_000_000_000, -1), (13859487941542, 10_000_000_000_000_000, 1), (0, 1, 1)],
        [(14231790752153335, 10_000_000_000_000_000, -1), (122016745867, 100_000_000_000_000, 1), (0, 1, 1)],
        [(13204544579940347, 10_000_000_000_000_000, -1), (11762579610906, 10_000_000_000_000_000, -1), (7639928529, 10_000_000_000_000_000, 1)],
    ];
    let voltage = u64::from(raw) * (65536 * [600u64,800,1200,2300][atten] / [2250u64,2250,2300,2300][atten]) / 65536;
    if voltage == 0 { return 0; }
    let mut power = 1u64; let mut error = 0i64;
    for (coefficient, divisor, sign) in COEFF[atten] { error += (power * coefficient / divisor) as i64 * sign; power *= voltage; }
    voltage as i64 - error
}
pub struct SarAdc { pub analog: AnalogInputs, pub now_cycles: u64, pub ground: bool, ram: RegRam, raw: u32 }
impl SarAdc { pub fn new(cpu_hz: u64) -> Self { Self { analog: AnalogInputs::new(cpu_hz), now_cycles: 0, ground: false, ram: RegRam::new(), raw: 0 } } }
impl Device for SarAdc {
    fn read(&mut self, off: u32) -> u32 { match off { 0x44 => self.raw, 0x48 => self.raw & self.ram.read(0x40), 0x4c => 0, _ => self.ram.read(off) } }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        if off == 0x4c { self.raw &= !value; return WriteEffect::NONE; }
        let old = self.ram.read(off); self.ram.write(off, value);
        if off == 0x20 && value & !old & (1 << 29) != 0 && value & (1 << 31) != 0 {
            let channel = ((value >> 25) & 15) as usize;
            let raw = if self.ground { Some(0) } else { PINS.get(channel).map(|&pin| {
                let atten = ((value >> 23) & 3) as usize;
                self.analog.convert(pin, self.now_cycles, |v| {
                    let want = (v.max(0.0) * 1000.0) as i64;
                    (0..4096).min_by_key(|&raw| (millivolts(raw, atten) - want).abs()).unwrap_or(0)
                })
            }) };
            if let Some(raw) = raw { self.ram.write(0x2c, raw); self.raw |= 1 << 31; }
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { u64::from(self.raw & self.ram.read(0x40) != 0) }
}
