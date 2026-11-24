//! The Waveshare CH32V006 board firmware protocol, not a CH32 silicon peripheral.
//! Fixed C5 Touch3.5 `06_exio` source writes/reads 16-bit values low byte first.
//! The same repository's factory component writes only the low byte. That leaves
//! the upper byte unchanged/unknown; it does not turn this into the V003 device.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc, Mutex};

pub struct Ch32V006Port {
    direction: u16,
    output: u16,
    direction_known: u16,
    output_known: u16,
    pub inputs: u16,
    pub pwm: Option<u8>,
    powered: bool,
    pub power_epoch: u64,
    falling: [u64; 16],
    pub writes: u64,
}
impl Default for Ch32V006Port { fn default()->Self { Self::new() } }
impl Ch32V006Port {
    pub fn new() -> Self {
        Self { direction: 0, output: 0, direction_known: 0, output_known: 0,
            inputs: 0, pwm: None, powered: true, power_epoch: 0,
            falling: [0; 16], writes: 0 }
    }
    pub fn set_powered(&mut self, on: bool) {
        if self.powered && !on {
            self.direction_known = 0;
            self.output_known = 0;
            self.pwm = None;
            self.power_epoch = self.power_epoch.wrapping_add(1);
        }
        self.powered = on;
    }
    /// Unknown startup direction/output is not guessed from the V003 library.
    /// Once configured as input, the board supplies the actual modeled input net.
    pub fn level(&self, pin: u8) -> Option<bool> {
        let mask = 1u16 << pin;
        if !self.powered || self.direction_known & mask == 0 { return None; }
        if self.direction & mask == 0 { return Some(self.inputs & mask != 0); }
        (self.output_known & mask != 0).then_some(self.output & mask != 0)
    }
    pub fn levels(&self) -> u16 {
        (0..16).fold(0, |word, pin| word | (u16::from(self.level(pin) == Some(true)) << pin))
    }
    pub fn falling_edges(&self, pin: usize) -> u64 { self.falling[pin] }
    fn write_byte(&mut self, reg: u8, byte: usize, value: u8) {
        let before = self.levels();
        let shift = byte * 8;
        let mask = 0xffu16 << shift;
        match reg {
            2 => { self.direction = (self.direction & !mask) | (u16::from(value) << shift); self.direction_known |= mask; }
            3 => { self.output = (self.output & !mask) | (u16::from(value) << shift); self.output_known |= mask; }
            5 => self.pwm = Some(value),
            _ => return,
        }
        let falling = before & !self.levels();
        for (pin, count) in self.falling.iter_mut().enumerate() {
            if falling & (1 << pin) != 0 { *count = count.wrapping_add(1); }
        }
        self.writes += 1;
    }
}

pub struct Ch32V006 {
    state: Arc<Mutex<Ch32V006Port>>, reg: Option<u8>, first: bool, byte: usize,
    input_snapshot: u16, epoch: u64,
}
impl Ch32V006 {
    pub fn new(state: Arc<Mutex<Ch32V006Port>>) -> Self {
        Self { state, reg: None, first: true, byte: 0, input_snapshot: 0, epoch: 0 }
    }
}
impl I2cDevice for Ch32V006 {
    fn start(&mut self, read: bool) -> bool {
        let state = self.state.lock().expect("CH32V006 state");
        if self.epoch != state.power_epoch { self.epoch = state.power_epoch; self.reg = None; }
        if !state.powered { return false; }
        self.byte = 0;
        if read {
            self.input_snapshot = state.levels();
            self.reg == Some(4)
        } else { self.first = true; self.reg = None; true }
    }
    fn write(&mut self, value: u8) -> bool {
        if self.first { self.first = false; self.reg = matches!(value, 2..=5).then_some(value); return self.reg.is_some(); }
        let Some(reg) = self.reg else { return false; };
        let limit = if reg == 5 { 1 } else { 2 };
        if reg == 4 || self.byte >= limit { return false; }
        self.state.lock().expect("CH32V006 state").write_byte(reg, self.byte, value);
        self.byte += 1;
        true
    }
    fn read(&mut self) -> u8 {
        if self.reg != Some(4) || self.byte >= 2 { return 0xff; }
        let byte = (self.input_snapshot >> (self.byte * 8)) as u8;
        self.byte += 1;
        byte
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn write(d: &mut dyn I2cDevice, bytes: &[u8]) -> bool { d.start(false) && bytes.iter().all(|b| d.write(*b)) }
    #[test] fn v006_uses_sixteen_real_pins_and_retains_unknown_upper_startup_state() {
        let s = Arc::new(Mutex::new(Ch32V006Port::new())); let mut d = Ch32V006::new(s.clone());
        assert_eq!(s.lock().unwrap().level(0), None);
        assert!(write(&mut d, &[2, 3])); assert!(write(&mut d, &[3, 3]));
        assert_eq!(s.lock().unwrap().level(0), Some(true));
        assert_eq!(s.lock().unwrap().level(8), None, "the 8-bit factory access does not initialize EX8");
        assert!(write(&mut d, &[2, 3, 0x81])); assert!(write(&mut d, &[3, 3, 0x80]));
        assert_eq!(s.lock().unwrap().level(15), Some(true)); assert_eq!(s.lock().unwrap().level(8), Some(false));
        s.lock().unwrap().inputs = 0x4000;
        assert!(write(&mut d, &[4])); assert!(d.start(true)); assert_eq!([d.read(), d.read(), d.read()], [3, 0xc0, 0xff]);
        assert!(write(&mut d, &[3, 2])); assert_eq!(s.lock().unwrap().falling_edges(0), 1);
        assert_eq!(s.lock().unwrap().level(15), Some(true), "low-byte writes preserve configured EX15");
        for reg in [0, 1, 6, 7, 255] { assert!(!write(&mut d, &[reg])); }
        assert!(!write(&mut d, &[5, 10, 20])); assert!(write(&mut d, &[2])); assert!(!d.start(true));
        s.lock().unwrap().set_powered(false); assert!(!d.start(false));
        s.lock().unwrap().set_powered(true); assert!(!d.start(true)); assert_eq!(s.lock().unwrap().level(15), None);
    }
}
