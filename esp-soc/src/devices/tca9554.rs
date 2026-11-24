//! TI TCA9554 eight-pin expander, SCPS233E sections 8.3 and 8.6.
//! Four non-incrementing registers, actual input/output directions and INT.
//! Pin levels and edge counts are digital; no voltage, contention or ACK-edge
//! interrupt timing is inferred. Board wiring supplies the I2C address and pads.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc, Mutex};

pub struct Tca9554State {
    output: u8,
    polarity: u8,
    configuration: u8,
    inputs: u8,
    sampled_inputs: u8,
    changes: u64,
    rising: [u64; 8],
    falling: [u64; 8],
}

impl Default for Tca9554State {
    fn default() -> Self {
        // Unconnected inputs use the documented weak pull-ups. A board must
        // provide any external low drivers/pulls instead of changing POR values.
        Self { output: 0xff, polarity: 0, configuration: 0xff, inputs: 0xff,
            sampled_inputs: 0xff, changes: 0, rising: [0; 8], falling: [0; 8] }
    }
}

impl Tca9554State {
    pub fn levels(&self) -> u8 {
        (self.inputs & self.configuration) | (self.output & !self.configuration)
    }
    pub fn driven(&self, pin: u8) -> Option<bool> {
        if pin >= 8 || self.configuration & (1 << pin) != 0 { None }
        else { Some(self.output & (1 << pin) != 0) }
    }
    pub fn high(&self, pin: u8) -> bool { pin < 8 && self.levels() & (1 << pin) != 0 }
    pub fn changes(&self) -> u64 { self.changes }
    pub fn falling_edges(&self, pin: u8) -> u64 { self.falling.get(pin as usize).copied().unwrap_or(0) }
    pub fn rising_edges(&self, pin: u8) -> u64 { self.rising.get(pin as usize).copied().unwrap_or(0) }
    pub fn irq_high(&self) -> bool {
        (self.levels() ^ self.sampled_inputs) & self.configuration == 0
    }
    pub fn set_inputs(&mut self, mask: u8, levels: u8) {
        let old = self.levels();
        self.inputs = (self.inputs & !mask) | (levels & mask);
        self.record_edges(old);
    }
    fn record_edges(&mut self, old: u8) {
        let now = self.levels();
        if old == now { return; }
        self.changes = self.changes.wrapping_add(1);
        for pin in 0..8 {
            let bit = 1 << pin;
            if (old ^ now) & bit != 0 {
                if now & bit != 0 { self.rising[pin] = self.rising[pin].wrapping_add(1); }
                else { self.falling[pin] = self.falling[pin].wrapping_add(1); }
            }
        }
    }
    fn write(&mut self, register: u8, value: u8) {
        let old = self.levels();
        match register {
            // The data sheet explicitly says writes to Input Port have no effect.
            // The bus still acknowledges the byte; no state is fabricated.
            0 => {},
            1 => self.output = value,
            2 => self.polarity = value,
            3 => self.configuration = value,
            _ => unreachable!("TCA9554 register validated by device"),
        }
        self.record_edges(old);
    }
    fn read(&mut self, register: u8) -> u8 {
        match register {
            0 => {
                let levels = self.levels();
                // Clearing is observed at the byte read boundary. The analog
                // t_ir delay and a concurrent edge inside ACK are not modeled.
                self.sampled_inputs = levels;
                levels ^ (self.polarity & self.configuration)
            },
            1 => self.output,
            2 => self.polarity,
            3 => self.configuration,
            _ => 0xff,
        }
    }
}

pub struct Tca9554 {
    state: Arc<Mutex<Tca9554State>>,
    register: Option<u8>,
    first: bool,
}
impl Tca9554 {
    pub fn new(state: Arc<Mutex<Tca9554State>>) -> Self {
        Self { state, register: Some(0), first: true }
    }
}
impl I2cDevice for Tca9554 {
    fn start(&mut self, read: bool) -> bool {
        if !read { self.first = true; }
        !read || self.register.is_some()
    }
    fn write(&mut self, value: u8) -> bool {
        if self.first {
            self.first = false;
            self.register = (value < 4).then_some(value);
            return self.register.is_some();
        }
        let Some(register) = self.register else { return false; };
        self.state.lock().expect("TCA9554 state poisoned").write(register, value);
        true
    }
    fn read(&mut self) -> u8 {
        let Some(register) = self.register else { return 0xff; };
        self.state.lock().expect("TCA9554 state poisoned").read(register)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d: &mut Tca9554, register: u8, bytes: &[u8]) {
        assert!(d.start(false)); assert!(d.write(register));
        for &byte in bytes { assert!(d.write(byte)); }
        d.stop();
    }
    fn read(d: &mut Tca9554, register: u8, count: usize) -> Vec<u8> {
        assert!(d.start(false)); assert!(d.write(register)); assert!(d.start(true));
        let result = (0..count).map(|_| d.read()).collect(); d.stop(); result
    }
    #[test]
    fn four_register_por_and_repeated_bytes_do_not_auto_increment() {
        let state = Arc::new(Mutex::new(Tca9554State::default()));
        let mut d = Tca9554::new(state.clone());
        assert_eq!(read(&mut d, 0, 2), [255, 255]);
        assert_eq!(read(&mut d, 1, 2), [255, 255]);
        assert_eq!(read(&mut d, 2, 2), [0, 0]);
        assert_eq!(read(&mut d, 3, 2), [255, 255]);
        write(&mut d, 1, &[0x12, 0x34, 0x56]);
        assert_eq!(read(&mut d, 1, 3), [0x56; 3]);
        assert_eq!(read(&mut d, 2, 1), [0]);
        write(&mut d, 0, &[0]); assert_eq!(read(&mut d, 0, 1), [255]);
        assert!(d.start(false)); assert!(!d.write(4)); assert!(!d.write(1));
        assert!(!d.start(true)); assert_eq!(d.read(), 255);
    }
    #[test]
    fn output_latch_direction_and_input_polarity_remain_distinct() {
        let state = Arc::new(Mutex::new(Tca9554State::default()));
        let mut d = Tca9554::new(state.clone());
        state.lock().unwrap().set_inputs(0xff, 0xaa);
        write(&mut d, 1, &[0x55]); write(&mut d, 3, &[0xf0]);
        assert_eq!(read(&mut d, 0, 1), [0xa5]);
        assert_eq!(read(&mut d, 1, 1), [0x55]);
        write(&mut d, 2, &[0xff]); assert_eq!(read(&mut d, 0, 1), [0x55]);
        let s = state.lock().unwrap();
        assert_eq!(s.driven(0), Some(true)); assert_eq!(s.driven(1), Some(false));
        assert_eq!(s.driven(4), None); assert_eq!(s.driven(8), None);
        assert_eq!(s.levels(), 0xa5, "polarity changes the read value, never the physical pad");
    }
    #[test]
    fn interrupt_tracks_input_changes_and_read_or_return_to_baseline_clears_it() {
        let state = Arc::new(Mutex::new(Tca9554State::default()));
        let mut d = Tca9554::new(state.clone());
        state.lock().unwrap().set_inputs(1, 0); assert!(!state.lock().unwrap().irq_high());
        assert_eq!(read(&mut d, 1, 1), [255]); assert!(!state.lock().unwrap().irq_high());
        state.lock().unwrap().set_inputs(1, 1); assert!(state.lock().unwrap().irq_high());
        state.lock().unwrap().set_inputs(1, 0); assert_eq!(read(&mut d, 0, 1), [254]);
        assert!(state.lock().unwrap().irq_high());
        write(&mut d, 3, &[0xfe]); write(&mut d, 1, &[0xfe]);
        assert!(state.lock().unwrap().irq_high(), "an output change does not raise INT");
        write(&mut d, 1, &[255]);
        assert_eq!(state.lock().unwrap().rising_edges(0), 3, "enabling a high output also produces a physical rising edge");
        assert_eq!(state.lock().unwrap().falling_edges(0), 3);
    }
    #[test]
    fn shared_state_and_edge_counts_survive_controller_reattachment() {
        let state = Arc::new(Mutex::new(Tca9554State::default()));
        let mut a = Tca9554::new(state.clone());
        write(&mut a, 3, &[0xf8]); write(&mut a, 1, &[0xfa]); write(&mut a, 1, &[255]);
        let mut b = Tca9554::new(state.clone());
        assert_eq!(read(&mut b, 3, 1), [0xf8]);
        let s = state.lock().unwrap();
        assert_eq!((s.falling_edges(0), s.rising_edges(0)), (1, 1));
        assert_eq!((s.falling_edges(2), s.rising_edges(2)), (1, 1));
        assert!(s.high(0)); assert!(s.high(2));
    }
}
