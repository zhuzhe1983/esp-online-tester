//! S31 software GPIO pads and PMU global hold; peripheral routing/IRQs are not modeled.
//! ESP-IDF v6.1 fff9895c82d744c7237be8847347bdd1b07c6643: soc/esp32s31
//! register/soc/{gpio,io_mux,pmu,lp_system}_reg.h, include/soc/soc_caps.h, esp_hal_gpio gpio_ll.h;
//! esp_driver_gpio/include/driver/gpio.h documents held configuration and live input.
use emu_core::Fault;

pub const GPIO_BASE: u32 = 0x2058_3000;
pub const IO_MUX_BASE: u32 = 0x2058_2000;
pub const PMU_PAD_HOLD: u32 = 0x2070_410c;
pub const HP_PIN_HOLD_HIGH: u32 = 0x2070_0270;
pub const HP_PIN_HOLD_LOW: u32 = 0x2070_0274;
const PINS: usize = 62;
const VALID: u64 = ((1u64 << PINS) - 1) & !(1 << 29) & !(1 << 41);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive { HighZ, Low, High, Unsupported }
#[derive(Clone, Copy)]
struct Electrical { drive: Drive, input: bool, pull_up: bool, pull_down: bool }
pub struct Pads {
    out: u64, enable: u64, iomux: [u32; PINS], pin: [u32; PINS], out_sel: [u32; PINS],
    held: [Option<Electrical>; PINS], host: [Option<bool>; PINS], unresolved_sample: [Option<bool>; PINS], hold_lp: bool, hold_hp: bool, hold_pin: u64,
    /// Explicit sampled boot strap input; zero is the development model initial condition.
    pub strap: u16,
    gpio_date: u32, iomux_date: u32,
}

impl Default for Pads {
    fn default() -> Self {
        // IO_MUX initial zero is a deterministic unconfigured development state, not a
        // claim about package-specific reset pull-ups/functions. Firmware must configure it.
        Self { out: 0, enable: 0, iomux: [0; PINS], pin: [0; PINS], out_sel: [256; PINS],
            held: [None; PINS], host: [None; PINS], unresolved_sample: [None; PINS], hold_lp: false, hold_hp: false, hold_pin: 0, strap: 0,
            gpio_date: 38830416, iomux_date: 0x2502070 }
    }
}
impl Pads {
    fn valid(pin: usize) -> bool { pin < PINS && VALID & (1u64 << pin) != 0 }
    pub fn set_input(&mut self, pin: usize, level: Option<bool>) -> Result<(), Fault> {
        if !Self::valid(pin) { return Err(Fault::Unmapped); }
        self.host[pin] = level; Ok(())
    }
    /// Explicit already-resolved voltage input for an unmodeled peripheral pad.
    /// This is not an external driver, a pin-function implementation or a hardware
    /// reset value. It never replaces known GPIO drive/contention or input gating.
    pub fn set_unresolved_sample(&mut self, pin: usize, level: Option<bool>) -> Result<(), Fault> {
        if !Self::valid(pin) { return Err(Fault::Unmapped); }
        self.unresolved_sample[pin] = level; Ok(())
    }
    fn gpio_function(pin: usize, function: u32) -> bool {
        // Fixed S31 io_mux_reg.h FUNC_*_GPIO<n>_0 aliases, not peripheral
        // function0 on SPI2,Flash,JTAG or UART pins. GPIO29/41 remain unbonded.
        function == 1 || function == 0 && matches!(pin, 0..=8 | 20..=25 | 33..=40 | 42..=53 | 60..=61)
    }
    fn live(&self, pin: usize) -> Electrical {
        let mux = self.iomux[pin]; let sel = self.out_sel[pin];
        let enable = (self.enable & (1 << pin) != 0) ^ (sel & (1 << 11) != 0);
        let high = (self.out & (1 << pin) != 0) ^ (sel & (1 << 9) != 0);
        let drive = if !Self::gpio_function(pin, (mux >> 12) & 7) { Drive::Unsupported }
            else if sel & 0x1ff != 256 { if sel & (1 << 10) != 0 && !enable { Drive::HighZ } else { Drive::Unsupported } }
            else if !enable || (high && self.pin[pin] & 4 != 0) { Drive::HighZ }
            else if high { Drive::High } else { Drive::Low };
        Electrical { drive, input: mux & (1 << 9) != 0, pull_up: mux & (1 << 8) != 0, pull_down: mux & (1 << 7) != 0 }
    }
    fn electrical(&self, pin: usize) -> Electrical { self.held[pin].unwrap_or_else(|| self.live(pin)) }
    pub fn drive(&self, pin: usize) -> Result<Drive, Fault> {
        if !Self::valid(pin) { return Err(Fault::Unmapped); }
        Ok(self.electrical(pin).drive)
    }
    fn sample(&self, pin: usize) -> Result<bool, Fault> {
        let e = self.electrical(pin);
        if !e.input { return Ok(false); }
        let output = match e.drive { Drive::High => Some(true), Drive::Low => Some(false), Drive::HighZ => None,
            Drive::Unsupported => self.unresolved_sample[pin].ok_or(Fault::Prohibited).map(Some)? };
        if let (Some(a), Some(b)) = (self.host[pin], output) { if a != b { return Err(Fault::Prohibited); } }
        if let Some(value) = output.or(self.host[pin]) { return Ok(value); }
        if e.pull_up && e.pull_down { return Err(Fault::Prohibited); }
        // Floating unconnected inputs resolve low only as a deterministic model convention.
        Ok(e.pull_up)
    }
    pub(crate) fn input_word(&self, bank: usize) -> Result<u32, Fault> {
        let mut word = 0;
        for bit in 0..32 { let pin = bank * 32 + bit; if Self::valid(pin) && self.sample(pin)? { word |= 1 << bit; } }
        Ok(word)
    }
    fn refresh_hold(&mut self) {
        let mask = self.hold_pin | if self.hold_lp { 255 } else { 0 } | if self.hold_hp { VALID & !255 } else { 0 };
        for pin in 0..PINS {
            if !Self::valid(pin) { continue; }
            if mask & (1 << pin) == 0 { self.held[pin] = None; }
            else if self.held[pin].is_none() { self.held[pin] = Some(self.live(pin)); }
        }
    }
    fn set_global_hold(&mut self, lp: bool, held: bool) {
        if lp { self.hold_lp = held; } else { self.hold_hp = held; }
        self.refresh_hold();
    }
    fn hold_write(&mut self, value: u32) -> Result<(), Fault> {
        // Simultaneous contradictory strobes have no documented priority; reject instead of guess.
        if value & 0x3000_0000 == 0x3000_0000 || value & 0xc000_0000 == 0xc000_0000 { return Err(Fault::Prohibited); }
        if value & (1 << 28) != 0 { self.set_global_hold(true, true); }
        if value & (1 << 29) != 0 { self.set_global_hold(true, false); }
        if value & (1 << 30) != 0 { self.set_global_hold(false, true); }
        if value & (1 << 31) != 0 { self.set_global_hold(false, false); }
        Ok(())
    }
    pub fn is_mmio(address: u32) -> bool {
        (GPIO_BASE..GPIO_BASE + 0xe00).contains(&address)
            || (IO_MUX_BASE..IO_MUX_BASE + 0x200).contains(&address) || matches!(address & !3, PMU_PAD_HOLD | HP_PIN_HOLD_HIGH | HP_PIN_HOLD_LOW)
    }
    fn pin_at(address: u32, base: u32) -> Option<usize> {
        let n = address.checked_sub(base)? as usize / 4; if n < PINS { Some(n) } else { None }
    }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        if address & 3 != 0 { return Err(Fault::Misaligned); }
        if matches!(address, HP_PIN_HOLD_HIGH | HP_PIN_HOLD_LOW) {
            return Ok((self.hold_pin >> if address == HP_PIN_HOLD_HIGH { 40 } else { 8 }) as u32);
        }
        if address == PMU_PAD_HOLD { return Ok(0); } // WT commands, not state bits.
        if address == IO_MUX_BASE + 0x1fc { return Ok(self.iomux_date); }
        if let Some(pin) = Self::pin_at(address, IO_MUX_BASE) { return Ok(self.iomux[pin]); }
        if let Some(pin) = Self::pin_at(address, GPIO_BASE + 0xf4) { return Ok(self.pin[pin]); }
        if let Some(pin) = Self::pin_at(address, GPIO_BASE + 0xaf4) { return Ok(self.out_sel[pin]); }
        match address.checked_sub(GPIO_BASE) {
            Some(0) => Ok(self.strap as u32), Some(4) => Ok(self.out as u32), Some(0x10) => Ok((self.out >> 32) as u32),
            Some(0x34) => Ok(self.enable as u32), Some(0x40) => Ok((self.enable >> 32) as u32),
            Some(8 | 12 | 0x14 | 0x18 | 0x38 | 0x3c | 0x44 | 0x48) => Ok(0),
            Some(0x64) => self.input_word(0), Some(0x68) => self.input_word(1), Some(0xdfc) => Ok(self.gpio_date),
            _ => Err(Fault::Unmapped),
        }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        if address & 3 != 0 { return Err(Fault::Misaligned); }
        if matches!(address, HP_PIN_HOLD_HIGH | HP_PIN_HOLD_LOW) {
            // v6.1 lp_system_reg.h: high bits map GPIO40..62, low GPIO8..39.
            // soc_caps.h excludes GPIO29/41 and exposes only GPIO0..61.
            let shift = if address == HP_PIN_HOLD_HIGH { 40 } else { 8 };
            let mask = if shift == 40 { 0x7fffff } else { 0xffffffff };
            self.hold_pin = (self.hold_pin & !(mask << shift) | (u64::from(value) & mask) << shift) & VALID;
            self.refresh_hold(); return Ok(());
        }
        if address == PMU_PAD_HOLD { return self.hold_write(value); }
        if address == IO_MUX_BASE + 0x1fc { self.iomux_date = value & 0x0fff_ffff; return Ok(()); }
        if let Some(pin) = Self::pin_at(address, IO_MUX_BASE) { self.iomux[pin] = value & 0x7fff; return Ok(()); }
        if let Some(pin) = Self::pin_at(address, GPIO_BASE + 0xf4) {
            if value & 0x3ff80 != 0 { return Err(Fault::Prohibited); } // IRQ/wakeup not implemented.
            self.pin[pin] = value & 0x1f; return Ok(());
        }
        if let Some(pin) = Self::pin_at(address, GPIO_BASE + 0xaf4) { self.out_sel[pin] = value & 0xfff; return Ok(()); }
        let off = address.checked_sub(GPIO_BASE).ok_or(Fault::Unmapped)?;
        let (data, base) = match off {
            4..=0x18 => (&mut self.out, 4), 0x34..=0x48 => (&mut self.enable, 0x34),
            0 | 0x64 | 0x68 => return Ok(()),
            0xdfc => { self.gpio_date = value & 0x0fff_ffff; return Ok(()); }
            _ => return Err(Fault::Unmapped),
        };
        let bank = (off - base) / 12; let shift = bank * 32; let bits = (value as u64) << shift;
        match (off - base) % 12 { 0 => *data = (*data & !(0xffff_ffffu64 << shift)) | bits, 4 => *data |= bits, 8 => *data &= !bits, _ => return Err(Fault::Unmapped) }
        *data &= VALID; Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configure(p: &mut Pads, pin: usize, output: bool) {
        p.write(IO_MUX_BASE + pin as u32 * 4, 0x1200).unwrap();
        if output { p.write(GPIO_BASE + if pin < 32 { 0x38 } else { 0x44 }, 1 << (pin % 32)).unwrap(); }
    }
    #[test]
    fn function_zero_gpio_aliases_route_output_but_spi_and_uart_remain_unknown() {
        let mut p = Pads::default();
        p.write(IO_MUX_BASE + 36 * 4, 0x200).unwrap(); // GPIO36 function0 alias
        p.write(GPIO_BASE + 0x44, 1 << 4).unwrap(); p.write(GPIO_BASE + 0x14, 1 << 4).unwrap();
        assert_eq!(p.drive(36), Ok(Drive::High)); assert_eq!(p.read(GPIO_BASE + 0x68), Ok(1 << 4));
        p.write(IO_MUX_BASE + 32 * 4, 0x200).unwrap(); // Flash D function0 is not GPIO
        assert_eq!(p.drive(32), Ok(Drive::Unsupported)); assert_eq!(p.read(GPIO_BASE + 0x68), Err(Fault::Prohibited));
        p.set_unresolved_sample(32, Some(false)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x68), Ok(1 << 4));
        p.write(IO_MUX_BASE + 59 * 4, 0x300).unwrap();
        assert_eq!(p.drive(59), Ok(Drive::Unsupported)); assert_eq!(p.read(GPIO_BASE + 0x68), Err(Fault::Prohibited));
    }
    #[test]
    fn explicit_sample_is_live_but_never_overrides_gpio_contention_or_disabled_input() {
        let mut p = Pads::default(); p.write(IO_MUX_BASE + 32 * 4, 0x200).unwrap();
        p.set_unresolved_sample(32, Some(true)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x68), Ok(1));
        p.write(PMU_PAD_HOLD, 1 << 30).unwrap(); p.write(IO_MUX_BASE + 32 * 4, 0).unwrap();
        p.set_unresolved_sample(32, Some(false)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x68), Ok(0));
        p.set_input(32, Some(true)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x68), Err(Fault::Prohibited));
        p.set_input(32, None).unwrap(); p.set_unresolved_sample(32, None).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x68), Err(Fault::Prohibited));
        p.write(PMU_PAD_HOLD, 1 << 31).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x68), Ok(0));
        configure(&mut p, 36, true); p.set_unresolved_sample(36, Some(true)).unwrap();
        assert_eq!(p.read(GPIO_BASE + 0x68), Ok(0)); // known low output wins
        p.set_input(36, Some(true)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x68), Err(Fault::Prohibited));
        for pin in [29, 41, 62, usize::MAX] { assert_eq!(p.set_unresolved_sample(pin, Some(false)), Err(Fault::Unmapped)); }
    }
    #[test]
    fn individual_and_global_hold_preserve_each_others_snapshot() {
        let mut p = Pads::default(); configure(&mut p, 59, true);
        p.write(HP_PIN_HOLD_HIGH, 1 << 19).unwrap(); p.write(GPIO_BASE + 0x14, 1 << 27).unwrap();
        assert_eq!(p.drive(59), Ok(Drive::Low)); p.write(PMU_PAD_HOLD, 1 << 30).unwrap();
        p.write(HP_PIN_HOLD_HIGH, 0).unwrap(); assert_eq!(p.drive(59), Ok(Drive::Low));
        p.write(PMU_PAD_HOLD, 1 << 31).unwrap(); assert_eq!(p.drive(59), Ok(Drive::High));
        p.write(HP_PIN_HOLD_LOW, 1 << 2).unwrap(); assert_eq!(p.read(HP_PIN_HOLD_LOW), Ok(4));
        p.write(HP_PIN_HOLD_HIGH, u32::MAX).unwrap(); assert_eq!(p.read(HP_PIN_HOLD_HIGH), Ok(0x3ffffd));
    }
    #[test]
    fn bank_layout_and_bonded_pins_match_s31_capabilities() {
        let mut p = Pads::default(); configure(&mut p, 61, true);
        p.write(GPIO_BASE + 0x14, 1 << 29).unwrap(); assert_eq!(p.drive(61), Ok(Drive::High));
        assert_eq!(p.read(GPIO_BASE + 0x68), Ok(1 << 29));
        p.write(GPIO_BASE + 0x18, 1 << 29).unwrap(); assert_eq!(p.drive(61), Ok(Drive::Low));
        p.write(GPIO_BASE + 0x48, 1 << 29).unwrap(); assert_eq!(p.drive(61), Ok(Drive::HighZ));
        p.write(GPIO_BASE + 4, u32::MAX).unwrap(); p.write(GPIO_BASE + 0x10, u32::MAX).unwrap();
        assert_eq!(p.read(GPIO_BASE + 4), Ok(!(1 << 29)));
        assert_eq!(p.read(GPIO_BASE + 0x10), Ok(0x3fff_ffff & !(1 << 9)));
        for pin in [29, 41, 62, usize::MAX] { assert_eq!(p.drive(pin), Err(Fault::Unmapped)); assert_eq!(p.set_input(pin, None), Err(Fault::Unmapped)); }
        assert_eq!(p.read(GPIO_BASE + 0x1c), Err(Fault::Unmapped));
        assert_eq!(p.write(GPIO_BASE + 5, 1), Err(Fault::Misaligned));
    }
    #[test]
    fn hold_keeps_output_and_input_enable_but_not_input_signal() {
        let mut p = Pads::default(); configure(&mut p, 10, true); configure(&mut p, 2, false);
        p.write(PMU_PAD_HOLD, 0x5000_0000).unwrap();
        p.write(GPIO_BASE + 8, 1 << 10).unwrap(); p.write(IO_MUX_BASE + 8, 0x1000).unwrap();
        assert_eq!(p.drive(10), Ok(Drive::Low));
        p.set_input(2, Some(true)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x64), Ok(4));
        p.set_input(2, Some(false)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x64), Ok(0));
        p.write(PMU_PAD_HOLD, 1 << 31).unwrap(); assert_eq!(p.drive(10), Ok(Drive::High));
        p.set_input(2, Some(true)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x64), Ok((1 << 10) | 4));
        p.write(PMU_PAD_HOLD, 1 << 29).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x64), Ok(1 << 10));
        assert_eq!(p.read(PMU_PAD_HOLD), Ok(0));
        assert_eq!(p.write(PMU_PAD_HOLD, 0xf000_0000), Err(Fault::Prohibited));
    }
    #[test]
    fn open_drain_pulls_inversion_and_unimplemented_routing_stay_distinct() {
        let mut p = Pads::default(); configure(&mut p, 10, true);
        p.write(GPIO_BASE + 0xf4 + 40, 4).unwrap(); p.write(GPIO_BASE + 8, 1 << 10).unwrap();
        assert_eq!(p.drive(10), Ok(Drive::HighZ));
        p.write(IO_MUX_BASE + 40, 0x1300).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x64), Ok(1 << 10));
        p.set_input(10, Some(false)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x64), Ok(0));
        p.set_input(10, None).unwrap(); p.write(GPIO_BASE + 0xaf4 + 40, 0x300).unwrap();
        assert_eq!(p.drive(10), Ok(Drive::Low));
        p.set_input(10, Some(true)).unwrap(); assert_eq!(p.read(GPIO_BASE + 0x64), Err(Fault::Prohibited));
        p.set_input(10, None).unwrap(); p.write(GPIO_BASE + 0xaf4 + 40, 1).unwrap();
        assert_eq!(p.drive(10), Ok(Drive::Unsupported)); assert_eq!(p.read(GPIO_BASE + 0x64), Err(Fault::Prohibited));
        assert_eq!(p.write(GPIO_BASE + 0xf4 + 40, 1 << 7), Err(Fault::Prohibited));
    }
}
