//! FT3168 single-contact report and the explicitly used FT5x06-compatible
//! configuration subset. No fabricated firmware/chip identity or calibration.
//! Sources and boundaries: docs/platform/ft3168-report-model.md.
use std::sync::{Arc, Mutex};
use esp_periph::i2c::I2cDevice;

const CONFIG_REGISTERS: [u8; 9] = [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x87, 0x88, 0x89];

pub struct Ft3168State {
    cycle: u64,
    hz: u64,
    active: bool,
    generation: u64,
    ready_at: u64,
    irq_until: u64,
    x: u16,
    y: u16,
    down: bool,
    event: u8,
    configuration: [Option<u8>; 9],
    pub host_updates: u64,
    pub reports: u64,
    pub contact_reports: u64,
    pub configuration_writes: u64,
}
impl Ft3168State {
    pub fn new(cpu_hz: u64) -> Self {
        Self { cycle: 0, hz: cpu_hz.max(1), active: false, generation: 0, ready_at: 0,
            irq_until: 0, x: 0, y: 0, down: false, event: 1,
            configuration: [None; 9], host_updates: 0, reports: 0,
            contact_reports: 0, configuration_writes: 0 }
    }
    pub fn advance_to(&mut self, cycle: u64) {
        assert!(cycle >= self.cycle, "FT3168 time moved backwards"); self.cycle = cycle;
    }
    pub fn hardware_at(&mut self, cycle: u64, powered: bool, reset_released: bool) {
        self.advance_to(cycle);
        let active = powered && reset_released;
        if active != self.active {
            self.active = active; self.generation = self.generation.wrapping_add(1);
            self.configuration = [None; 9];
            self.down = false; self.x = 0; self.y = 0; self.event = 1; self.irq_until = 0;
            // FT3168 Table 3-5: at least 70 ms before reporting a point after
            // reset. Configuration transactions are separate from point-ready.
            let delay = (u128::from(self.hz) * 70).div_ceil(1000).min(u128::from(u64::MAX)) as u64;
            self.ready_at = cycle.saturating_add(delay);
        }
    }
    pub fn active(&self) -> bool { self.active }
    pub fn report_ready(&self) -> bool { self.active && self.cycle >= self.ready_at }
    pub fn touch_at(&mut self, cycle: u64, x: u16, y: u16, down: bool) -> bool {
        self.advance_to(cycle);
        if !self.active { return false; }
        let (x, y) = (x.min(0x0fff), y.min(0x0fff));
        if (x, y, down) != (self.x, self.y, self.down) {
            self.event = if !down { 1 } else if self.down { 2 } else { 0 };
            // Falling IRQ pulse is a declared digital host-input policy, not a
            // measured physical scan cadence or analog threshold calculation.
            self.irq_until = cycle.max(self.ready_at).saturating_add(self.hz.div_ceil(1000));
        }
        self.x = x; self.y = y; self.down = down; self.host_updates += 1; true
    }
    pub fn irq_high(&self) -> bool {
        !self.report_ready() || self.cycle >= self.irq_until
    }
    pub fn next_deadline(&self) -> Option<u64> {
        if !self.active || self.cycle >= self.irq_until { None }
        else if self.cycle < self.ready_at { Some(self.ready_at) }
        else { Some(self.irq_until) }
    }
    fn report(&self) -> [u8; 9] {
        let down = self.down && self.report_ready();
        [0, 0, u8::from(down), (self.event << 6) | (self.x >> 8) as u8,
            self.x as u8, (self.y >> 8) as u8, self.y as u8, 0, 0]
    }
    fn config_slot(r: u8) -> Option<usize> { CONFIG_REGISTERS.iter().position(|v| *v == r) }
}

pub struct Ft3168 {
    state: Arc<Mutex<Ft3168State>>,
    pointer: Option<u8>,
    first: bool,
    wrote_parameter: bool,
    report: [u8; 9],
    generation: u64,
}
impl Ft3168 {
    pub fn new(state: Arc<Mutex<Ft3168State>>) -> Self {
        Self { state, pointer: None, first: true, wrote_parameter: false, report: [0; 9], generation: 0 }
    }
    fn synchronize(&mut self) -> bool {
        let s = self.state.lock().expect("FT3168 state");
        if s.generation != self.generation {
            self.pointer = None; self.first = true; self.wrote_parameter = false;
            self.report = [0; 9]; self.generation = s.generation;
        }
        s.active
    }
}
impl I2cDevice for Ft3168 {
    fn start(&mut self, read: bool) -> bool {
        if !self.synchronize() { return false; }
        let s = self.state.lock().expect("FT3168 state");
        if !s.active { return false; }
        if !read { self.first = true; self.wrote_parameter = false; self.pointer = None; return true; }
        let Some(r) = self.pointer else { return false; };
        if r <= 8 { self.report = s.report(); true }
        else { Ft3168State::config_slot(r).is_some_and(|i| s.configuration[i].is_some()) }
    }
    fn write(&mut self, value: u8) -> bool {
        if !self.synchronize() { return false; }
        let mut s = self.state.lock().expect("FT3168 state");
        if !s.active { return false; }
        if self.first {
            self.first = false;
            self.pointer = (value <= 8 || Ft3168State::config_slot(value).is_some()).then_some(value);
            return self.pointer.is_some();
        }
        if self.wrote_parameter { return false; }
        let Some(r) = self.pointer else { return false; };
        let Some(i) = Ft3168State::config_slot(r) else { return false; };
        s.configuration[i] = Some(value); s.configuration_writes += 1;
        self.wrote_parameter = true; true
    }
    fn read(&mut self) -> u8 {
        if !self.synchronize() { return 255; }
        let mut s = self.state.lock().expect("FT3168 state");
        if !s.active { return 255; }
        let Some(r) = self.pointer else { return 255; };
        if r <= 8 {
            if r == 2 { s.reports += 1; s.contact_reports += u64::from(self.report[2] != 0); }
            self.pointer = (r < 8).then_some(r + 1); self.report[usize::from(r)]
        } else {
            self.pointer = None;
            Ft3168State::config_slot(r).and_then(|i| s.configuration[i]).unwrap_or(255)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make() -> (Arc<Mutex<Ft3168State>>, Ft3168) {
        let state = Arc::new(Mutex::new(Ft3168State::new(1_000_000)));
        let device = Ft3168::new(state.clone()); (state, device)
    }
    fn read(d: &mut Ft3168, reg: u8, n: usize) -> Vec<u8> {
        assert!(d.start(false)); assert!(d.write(reg)); assert!(d.start(true));
        let out = (0..n).map(|_| d.read()).collect(); d.stop(); out
    }
    #[test]
    fn initialization_values_are_stored_but_unknown_identity_and_calibration_are_not_faked() {
        let (s, mut d) = make(); assert!(!d.start(false));
        s.lock().unwrap().hardware_at(0, true, true);
        for (reg, value) in CONFIG_REGISTERS.into_iter().zip([70,60,16,60,10,20,2,12,40]) {
            assert!(d.start(false)); assert!(d.write(reg)); assert!(!d.start(true), "unknown factory default");
            assert!(d.start(false)); assert!(d.write(reg)); assert!(d.write(value));
            assert!(!d.write(255), "only the evidenced single-register write is implemented");
            assert_eq!(read(&mut d, reg, 1), [value]);
        }
        for unknown in [0x86,0xa0,0xa3,0xa4,0xa5,0xa6,0xa7,0xa8] {
            assert!(d.start(false)); assert!(!d.write(unknown)); assert!(!d.start(true));
        }
        assert!(d.start(false)); assert!(d.write(2)); assert!(!d.write(1), "report is read-only");
        assert_eq!(s.lock().unwrap().configuration_writes, 9);
    }
    #[test]
    fn report_burst_is_latched_and_packs_single_contact_coordinates_and_events() {
        let (s, mut d) = make(); s.lock().unwrap().hardware_at(0, true, true);
        assert!(s.lock().unwrap().touch_at(80_000, 0x123, 0x456, true));
        assert!(d.start(false)); assert!(d.write(2)); assert!(d.start(true));
        assert_eq!(d.read(), 1);
        s.lock().unwrap().touch_at(81_000, 0x234, 0x567, true);
        assert_eq!((0..6).map(|_| d.read()).collect::<Vec<_>>(), [1,0x23,4,0x56,0,0]);
        d.stop(); assert_eq!(read(&mut d, 2, 7), [1,0x82,0x34,5,0x67,0,0]);
        s.lock().unwrap().touch_at(82_000, 0x234, 0x567, false);
        assert_eq!(read(&mut d, 2, 5), [0,0x42,0x34,5,0x67]);
        assert_eq!(s.lock().unwrap().contact_reports, 2);
    }
    #[test]
    fn point_ready_deadline_reset_and_power_are_guest_cycle_ordered() {
        let (s, mut d) = make(); s.lock().unwrap().hardware_at(10_000, true, true);
        s.lock().unwrap().touch_at(20_000, 20, 30, true);
        assert_eq!(read(&mut d, 2, 1), [0]); assert!(s.lock().unwrap().irq_high());
        assert_eq!(s.lock().unwrap().next_deadline(), Some(80_000));
        s.lock().unwrap().advance_to(79_999); assert_eq!(read(&mut d, 2, 1), [0]);
        s.lock().unwrap().advance_to(80_000); assert_eq!(read(&mut d, 2, 1), [1]);
        assert!(!s.lock().unwrap().irq_high()); s.lock().unwrap().advance_to(81_000);
        assert!(s.lock().unwrap().irq_high());
        assert!(d.start(false)); assert!(d.write(0x80)); assert!(d.write(70));
        s.lock().unwrap().hardware_at(82_000, true, false); assert!(!d.start(false));
        s.lock().unwrap().hardware_at(92_000, true, true);
        assert!(!d.start(true), "reset invalidates the old I2C pointer");
        assert!(d.start(false)); assert!(d.write(0x80)); assert!(!d.start(true));
        assert_eq!(read(&mut d, 2, 1), [0]);
        s.lock().unwrap().hardware_at(93_000, false, true); assert!(!d.start(false));
        assert!(!s.lock().unwrap().touch_at(94_000, 1, 1, true));
    }
}
