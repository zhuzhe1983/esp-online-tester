//! X-Powers AXP2101 digital power/control subset. Sources, variant conflicts and
//! explicit USB 5 V / no-cell-battery scenario: docs/platform/axp2101-model.md.
//! No charging, analog settling, watchdog, sleep or fuel-gauge engine is faked.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerRail {
    Dcdc1, Dcdc2, Dcdc3, Dcdc4, Dcdc5,
    Aldo1, Aldo2, Aldo3, Aldo4, Bldo1, Bldo2, CpusLdo, Dldo1, Dldo2,
}

pub struct Axp2101Power {
    registers: [u8; 256],
    known: [bool; 256],
    system_on: bool,
}

impl Default for Axp2101Power {
    fn default() -> Self {
        let mut p = Self { registers: [0; 256], known: [false; 256], system_on: true };
        // Published reset values, not a captured board EFUSE dump. The rail
        // preset is the standard datasheet Table 6-1 reference startup state.
        // EFUSE-only fields with no such reference stay unknown until written.
        for (r, v) in [
            (0x00, 0x20), (0x01, 0x15), (0x03, 0x4a),
            (4, 0), (5, 0), (6, 0), (7, 0), (0x10, 0x30),
            (0x12, 3), (0x13, 3), (0x14, 0x60), (0x15, 6), (0x16, 4),
            (0x17, 0), (0x18, 0x0a), (0x19, 6), (0x1a, 0xa1),
            (0x20, 4), (0x21, 0), (0x23, 0x3f), (0x24, 0), (0x26, 8),
            (0x28, 0), (0x29, 0), (0x2a, 0), (0x2b, 0), (0x30, 3),
            (0x34, 0), (0x35, 0), (0x36, 0x80), (0x37, 0),
            (0x38, 0), (0x39, 0), (0x3a, 0), (0x3b, 0), (0x3c, 0), (0x3d, 0),
            (0x40, 0xff), (0x41, 0xfc), (0x42, 0x5f),
            (0x48, 0), (0x49, 0), (0x4a, 0),
            (0x52, 2), (0x53, 1), (0x54, 0x29), (0x55, 0x58),
            (0x56, 0x3e), (0x57, 0x4c), (0x59, 0x14), (0x5a, 0x37),
            (0x5b, 0x1e), (0x5c, 2), (0x5d, 0x58),
            (0x61, 5), (0x62, 9), (0x63, 0x15), (0x64, 3),
            (0x65, 2), (0x67, 0xe6), (0x68, 1), (0x6a, 3),
            (0x80, 0x0f), (0x81, 0), (0x82, 18), (0x83, 40),
            (0x84, 40), (0x85, 60), (0x90, 0x45), (0x91, 0),
            (0x92, 13), (0x93, 23), (0x94, 28), (0x95, 24),
            (0x96, 13), (0x97, 23), (0x98, 8), (0x99, 28), (0x9a, 14),
            (0xa2, 0), (0xa4, 0),
        ] { p.registers[r] = v; p.known[r] = true; }
        p
    }
}

impl Axp2101Power {
    /// Requested digital rail voltage, only for enabled and supported codes.
    /// `None` also covers unimplemented / conflicting codes; it is not a claim
    /// that a physical regulator has settled or that an unknown rail is 0 V.
    pub fn output_mv(&self, rail: PowerRail) -> Option<u16> {
        if !self.system_on { return None; }
        use PowerRail::*;
        let (enable_reg, bit, voltage_reg) = match rail {
            Dcdc1 => (0x80, 0, 0x82), Dcdc2 => (0x80, 1, 0x83),
            Dcdc3 => (0x80, 2, 0x84), Dcdc4 => (0x80, 3, 0x85),
            Dcdc5 => return None, // standard / SWcharge variants disagree
            Aldo1 => (0x90, 0, 0x92), Aldo2 => (0x90, 1, 0x93),
            Aldo3 => (0x90, 2, 0x94), Aldo4 => (0x90, 3, 0x95),
            Bldo1 => (0x90, 4, 0x96), Bldo2 => (0x90, 5, 0x97),
            CpusLdo => (0x90, 6, 0x98), Dldo1 => (0x90, 7, 0x99),
            Dldo2 => (0x91, 0, 0x9a),
        };
        if self.registers[enable_reg] & (1 << bit) == 0 { return None; }
        let code = u16::from(self.registers[voltage_reg] & if voltage_reg <= 0x85 { 0x7f } else { 0x1f });
        match rail {
            Dcdc1 if code <= 19 => Some(1500 + code * 100),
            Dcdc2 | Dcdc3 | Dcdc4 if code <= 70 => Some(500 + code * 10),
            Dcdc2 | Dcdc3 if code <= 87 => Some(1220 + (code - 71) * 20),
            Dcdc4 if code <= 102 => Some(1220 + (code - 71) * 20),
            Aldo1 | Aldo2 | Aldo3 | Aldo4 | Bldo1 | Bldo2 if code <= 30 => Some(500 + code * 100),
            Dldo1 if code <= 28 => Some(500 + code * 100),
            CpusLdo | Dldo2 if code <= 18 => Some(500 + code * 50),
            _ => None, // conflicting endpoints and unsupported higher ranges
        }
    }
    pub fn system_on(&self) -> bool { self.system_on }
    pub fn irq_asserted(&self) -> bool {
        (0..3).any(|i| self.registers[0x40 + i] & self.registers[0x48 + i] != 0)
    }
    fn system_reset(&mut self) {
        // Respect the per-bit System Reset vs POR column. Buffers, ADC control,
        // charge limits and POR-only interrupt latches survive a system reset.
        let reference = Self::default();
        for r in [0x20, 0x40, 0x41, 0x42, 0x65, 0x80, 0x82, 0x83, 0x84,
            0x85, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a] {
            self.registers[r] = reference.registers[r]; self.known[r] = true;
        }
        self.registers[0x20] = 0; // no new external wakeup source is asserted
        self.registers[0x18] = (self.registers[0x18] & 8) | 2;
        self.registers[0x26] &= !3;
        self.registers[0x48] &= !3; self.registers[0x49] &= 0xf0; self.registers[0x4a] &= !0x40;
        self.registers[0x63] |= 0x10; self.registers[0x69] &= !0x30;
        self.registers[0x81] &= 3;
        self.system_on = true;
    }
    fn read(&self, r: u8) -> Option<u8> {
        if !self.known[usize::from(r)] { return None; }
        match r {
            0x00 => Some(0x20), // declared ideal VBUS good, battery absent
            0x01 => Some(if self.system_on { 0x15 } else { 5 }),
            // No battery/TS/system-voltage/temperature converter is invented.
            // Disabled channels expose only their published reset latch.
            0x34..=0x37 | 0x3a..=0x3d => {
                let bit = match r { 0x34 | 0x35 => 0, 0x36 | 0x37 => 1, 0x3a | 0x3b => 3, _ => 4 };
                (self.registers[0x30] & (1 << bit) == 0).then_some(self.registers[usize::from(r)])
            }
            _ => Some(self.registers[usize::from(r)]),
        }
    }
    fn write(&mut self, r: u8, value: u8) -> bool {
        let Some(mask) = writable_mask(r) else { return false; };
        if mask == 0 { return false; } // strict unsupported write, no fake RO mutation
        let v = value & mask;
        // Unknown engines / reserved encodings are not completed by storing a
        // convenient success flag. Charger controls are retained but, with no
        // cell battery, charging status never advances or reports completion.
        if match r {
            0x12 | 0x13 => v & 6 == 6,
            0x16 => v > 5,
            0x17 => v != 0,
            0x18 => v & 5 != 0, // no watchdog or button-cell charger
            0x19 => v & 8 != 0,
            0x26 => v & 3 != 0,
            0x2b => v & 0xc0 != 0,
            0x34 => v != 0, 0x36 => v & 0x40 != 0, // no ADC debug mux
            0x58 => v != 0, // no JEITA engine
            0x59 => v & 3 == 3 || v & 0x0c == 0x0c,
            0x61 | 0x63 => v & 15 > 8,
            0x62 => v > 16,
            0x64 => !(1..=5).contains(&v),
            0x69 => v & 6 == 6,
            0x80 => v & 0x10 != 0, // DCDC5 variant remains unspecified
            0xa2 => v != 0, // no battery-ROM/SRAM loader
            _ => false,
        } { return false; }
        if (0x48..=0x4a).contains(&r) {
            self.registers[usize::from(r)] &= !v;
            return true;
        }
        let i = usize::from(r);
        self.registers[i] = (self.registers[i] & !mask) | v;
        self.known[i] = true;
        if r == 0x10 {
            // Commands self-clear. This digital subset has no analog startup
            // delay or host-SoC reset callback; rail state changes immediately.
            self.registers[i] &= !3;
            if v & 2 != 0 { self.system_reset(); }
            if v & 1 != 0 { self.system_on = false; self.registers[0x21] |= 2; }
        }
        true
    }
}

// None: no documented supported register. Zero: supported read-only register.
fn writable_mask(r: u8) -> Option<u8> {
    Some(match r {
        0x00 | 0x01 | 0x03 | 0x20 | 0x21 | 0x35 | 0x37..=0x3d | 0x5d | 0xa4 => 0,
        4..=7 | 0x1a | 0x28..=0x2a | 0x40..=0x42 | 0x48..=0x4a | 0x52..=0x57 | 0x5a..=0x5b |
        0x81 | 0x83 | 0x84 | 0x90 => 0xff,
        0x10 => 0x2f,
        0x12 | 0x13 | 0x16 | 0x22 | 0x24 | 0x64 | 0x6a => 7,
        0x14 => 0x70, 0x15 | 0x18 | 0x61 => 15,
        0x17 => 12, 0x19 => 0x3f, 0x69 => 0x37,
        0x23 | 0x27 | 0x30 | 0x5c => 0x3f,
        0x25 | 0x26 | 0x50 | 0x62 | 0x63 | 0x82 | 0x92..=0x9a => 0x1f,
        0x2b => 0xcf, 0x34 | 0x36 => 0xc0,
        0x58 | 0x68 | 0x91 => 1,
        0x59 => 0x5f, 0x65 => 3, 0x67 => 0xf7,
        0x80 | 0x85 => 0x7f, 0xa2 => 0x31,
        _ => return None,
    })
}

pub struct Axp2101 {
    power: Arc<Mutex<Axp2101Power>>,
    pointer: u8,
    first: bool,
    valid: bool,
    vbus_latch: u16,
}
impl Axp2101 {
    pub fn new(power: Arc<Mutex<Axp2101Power>>) -> Self {
        Self { power, pointer: 0, first: true, valid: false, vbus_latch: 0 }
    }
}
impl I2cDevice for Axp2101 {
    fn start(&mut self, read: bool) -> bool {
        if !read { self.first = true; true }
        else { self.valid && self.power.lock().expect("AXP2101 power").read(self.pointer).is_some() }
    }
    fn write(&mut self, value: u8) -> bool {
        if self.first {
            self.first = false; self.pointer = value;
            self.valid = writable_mask(value).is_some();
            return self.valid;
        }
        if !self.valid { return false; }
        let accepted = self.power.lock().expect("AXP2101 power").write(self.pointer, value);
        if accepted { self.pointer = self.pointer.wrapping_add(1); }
        self.valid = accepted && writable_mask(self.pointer).is_some();
        accepted
    }
    fn read(&mut self) -> u8 {
        if !self.valid { return 0xff; }
        let p = self.power.lock().expect("AXP2101 power");
        let Some(mut v) = p.read(self.pointer) else { self.valid = false; return 0xff; };
        if self.pointer == 0x38 {
            self.vbus_latch = if p.registers[0x30] & 4 != 0 { 5000 } else { 0 };
            v = (self.vbus_latch >> 8) as u8;
        } else if self.pointer == 0x39 { v = self.vbus_latch as u8; }
        self.pointer = self.pointer.wrapping_add(1);
        self.valid = writable_mask(self.pointer).is_some();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device() -> (Axp2101, Arc<Mutex<Axp2101Power>>) {
        let p = Arc::new(Mutex::new(Axp2101Power::default())); (Axp2101::new(p.clone()), p)
    }
    fn put(d: &mut Axp2101, r: u8, v: u8) -> bool {
        assert!(d.start(false)); assert!(d.write(r)); d.write(v)
    }
    fn get(d: &mut Axp2101, r: u8, n: usize) -> Vec<u8> {
        assert!(d.start(false)); assert!(d.write(r)); assert!(d.start(true));
        (0..n).map(|_| d.read()).collect()
    }
    #[test]
    fn exact_power_enables_voltages_and_pwm_are_guest_state() {
        let (mut d, p) = device();
        assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc1), Some(3300));
        assert!(put(&mut d, 0x81, 4)); assert_eq!(get(&mut d, 0x81, 1), [4]);
        assert!(put(&mut d, 0x82, 0)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc1), Some(1500));
        assert!(put(&mut d, 0x82, 19)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc1), Some(3400));
        assert!(put(&mut d, 0x82, 20)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc1), None);
        assert!(put(&mut d, 0x82, 18)); assert!(put(&mut d, 0x80, 0x0e));
        assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc1), None);
        assert!(put(&mut d, 0x90, 0xff)); assert!(put(&mut d, 0x96, 28));
        assert_eq!(p.lock().unwrap().output_mv(PowerRail::Bldo1), Some(3300));
        assert_eq!(p.lock().unwrap().output_mv(PowerRail::Aldo1), Some(1800));
        assert!(put(&mut d, 0x91, 1)); assert!(put(&mut d, 0x9a, 18));
        assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dldo2), Some(1400));
        assert!(put(&mut d, 0x9a, 19)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dldo2), None);
        assert!(put(&mut d, 0x83, 70)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc2), Some(1200));
        assert!(put(&mut d, 0x83, 71)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc2), Some(1220));
        assert!(put(&mut d, 0x84, 88)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc3), None);
        assert!(!put(&mut d, 0x80, 0x1f)); assert_eq!(p.lock().unwrap().output_mv(PowerRail::Dcdc5), None);
    }
    #[test]
    fn system_reset_preserves_por_fields_and_soft_off_removes_rail_power() {
        let (mut d, p) = device();
        assert!(put(&mut d, 4, 0xa5)); assert!(put(&mut d, 0x30, 4));
        assert!(put(&mut d, 0x62, 10)); assert!(put(&mut d, 0x81, 7));
        assert!(put(&mut d, 0x90, 0)); assert!(put(&mut d, 0x10, 0x32));
        assert_eq!(get(&mut d, 4, 1), [0xa5]); assert_eq!(get(&mut d, 0x30, 1), [4]);
        assert_eq!(get(&mut d, 0x62, 1), [10]); assert_eq!(get(&mut d, 0x81, 1), [3]);
        assert_eq!(get(&mut d, 0x90, 1), [0x45]); assert_eq!(get(&mut d, 0x10, 1), [0x30]);
        assert!(put(&mut d, 0x10, 0x31)); assert!(!p.lock().unwrap().system_on());
        assert_eq!(get(&mut d, 0x21, 1), [2]); assert_eq!(get(&mut d, 1, 1), [5]);
        for rail in [PowerRail::Dcdc1, PowerRail::Aldo1, PowerRail::CpusLdo] {
            assert_eq!(p.lock().unwrap().output_mv(rail), None);
        }
        let fresh = Axp2101Power::default(); assert_eq!(fresh.registers[4], 0);
    }
    #[test]
    fn irq_write_one_clear_respects_masks_and_reset_domains() {
        let (mut d, p) = device();
        { let mut p = p.lock().unwrap(); p.registers[0x48] = 0x83; p.registers[0x49] = 0x88; p.registers[0x4a] = 0x50; }
        assert!(p.lock().unwrap().irq_asserted());
        assert!(put(&mut d, 0x40, 0)); assert!(d.write(0)); assert!(d.write(0));
        assert!(!p.lock().unwrap().irq_asserted());
        assert_eq!(get(&mut d, 0x48, 3), [0x83, 0x88, 0x50]);
        assert!(put(&mut d, 0x48, 0x80)); assert_eq!(get(&mut d, 0x48, 1), [3]);
        assert!(put(&mut d, 0x10, 0x32));
        assert_eq!(get(&mut d, 0x48, 3), [0, 0x80, 0x10]);
        assert!(put(&mut d, 0x49, 0xff)); assert!(d.write(0xff));
        assert_eq!(get(&mut d, 0x48, 3), [0, 0, 0]);
    }
    #[test]
    fn usb_scenario_never_fabricates_battery_charging_or_unknown_measurements() {
        let (mut d, _) = device();
        assert_eq!(get(&mut d, 0, 2), [0x20, 0x15]); assert_eq!(get(&mut d, 3, 1), [0x4a]);
        assert!(put(&mut d, 0x18, 0x0a)); assert!(put(&mut d, 0x62, 16));
        assert_eq!(get(&mut d, 1, 1), [0x15]); assert_eq!(get(&mut d, 0xa4, 1), [0]);
        assert_eq!(get(&mut d, 0x38, 2), [0, 0]);
        assert!(put(&mut d, 0x30, 4)); assert_eq!(get(&mut d, 0x38, 2), [0x13, 0x88]);
        assert!(put(&mut d, 0x30, 0x1f)); assert!(d.start(false)); assert!(d.write(0x3c));
        assert!(!d.start(true)); assert_eq!(d.read(), 0xff);
        assert!(!put(&mut d, 0x18, 0x0b)); assert!(!put(&mut d, 0x17, 8));
        assert!(!put(&mut d, 0x26, 9)); assert!(!put(&mut d, 0xa2, 1));
    }
    #[test]
    fn transfers_are_bounded_read_only_fields_and_unknown_efuse_are_not_ram() {
        let (mut d, _) = device();
        assert!(put(&mut d, 4, 1)); assert!(d.write(2)); assert!(d.write(3)); assert!(d.write(4));
        assert!(!d.write(5)); assert_eq!(get(&mut d, 4, 4), [1, 2, 3, 4]);
        assert!(!put(&mut d, 3, 0)); assert_eq!(get(&mut d, 3, 1), [0x4a]);
        assert!(put(&mut d, 0x82, 0xf2)); assert_eq!(get(&mut d, 0x82, 1), [18]);
        for r in [2, 8, 0x31, 0x43, 0x51, 0x60, 0x86, 0xa1, 0xff] {
            assert!(d.start(false)); assert!(!d.write(r)); assert!(!d.start(true)); assert_eq!(d.read(), 0xff);
        }
        assert!(d.start(false)); assert!(d.write(0x50)); assert!(!d.start(true));
        assert!(put(&mut d, 0x50, 0x12)); assert_eq!(get(&mut d, 0x50, 1), [0x12]);
    }
}
