//! NXP PCF85063A calendar subset, data sheet Rev. 7 §§8.2, 8.3, 8.4.
//! Guest-cycle clock, BCD calendar/12h, STOP, OS and one pending burst tick.
//! No host time, calibration, alarms, timer, interrupt or CLKOUT waveforms.
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc, Mutex};

pub struct Pcf85063State {
    registers: [u8; 18],
    cycle: u64,
    hz: u64,
    next_tick: u64,
    access: bool,
    pending: bool,
}
impl Pcf85063State {
    pub fn new(hz: u64) -> Self {
        let hz = hz.max(1);
        let mut state = Self { registers: [0; 18], cycle: 0, hz,
            next_tick: hz, access: false, pending: false };
        state.reset(); state
    }
    fn reset(&mut self) {
        self.registers = [0; 18];
        self.registers[4] = 0x80; // OS: date integrity not yet established.
        self.registers[7] = 1; self.registers[8] = 6; self.registers[9] = 1;
        self.registers[11..16].fill(0x80);
        self.registers[17] = 0x18;
        // Table 8: Saturday, 2000-01-01 00:00:00. Crystal startup and POR
        // subsecond phase are not modeled; choose one guest second to first tick.
        self.next_tick = self.cycle.saturating_add(self.hz);
        self.access = false; self.pending = false;
    }
    pub fn advance_to(&mut self, cycle: u64) {
        if cycle <= self.cycle { return; }
        self.cycle = cycle;
        if self.registers[0] & 0x20 != 0 || cycle < self.next_tick { return; }
        let elapsed = cycle - self.next_tick;
        let seconds = elapsed / self.hz + 1;
        self.next_tick = cycle.saturating_add(self.hz - elapsed % self.hz);
        if self.access { self.pending = true; } else { self.add_seconds(seconds); }
    }
    fn add_seconds(&mut self, seconds: u64) {
        let r = &mut self.registers;
        let (Some(sec), Some(min), Some(mut day), Some(mut month), Some(mut year)) =
            (decode(r[4] & 0x7f, 59), decode(r[5], 59), decode(r[7], 31),
             decode(r[9], 12), decode(r[10], 99)) else { return; };
        let twelve = r[0] & 2 != 0;
        let hour = if twelve {
            let Some(h) = decode(r[6] & 0x1f, 12).filter(|h| *h != 0) else { return; };
            h % 12 + if r[6] & 0x20 != 0 { 12 } else { 0 }
        } else { let Some(h) = decode(r[6], 23) else { return; }; h };
        // Undefined guest calendar values remain visible. Never normalize them
        // into an invented valid timestamp or use them as an unchecked index.
        if day == 0 || month == 0 || day > month_days(month, year) || r[8] > 6 { return; }
        let sum = u64::from(hour) * 3600 + u64::from(min) * 60 + u64::from(sec) + seconds % 86400;
        let days = seconds / 86400 + sum / 86400; let time = sum % 86400;
        let h = (time / 3600) as u8;
        r[4] = (r[4] & 0x80) | encode((time % 60) as u8);
        r[5] = encode((time / 60 % 60) as u8);
        r[6] = if twelve { encode(if h.is_multiple_of(12) { 12 } else { h % 12 }) | if h >= 12 { 0x20 } else { 0 } } else { encode(h) };
        r[8] = ((u64::from(r[8]) + days % 7) % 7) as u8;
        // The two-digit silicon year treats 00 as leap. The entire calendar
        // repeats every 36525 days; weekday above retains the full elapsed time.
        let mut left = days % 36525;
        while left > 0 {
            let remaining = u64::from(month_days(month, year) - day);
            if left <= remaining { day += left as u8; break; }
            left -= remaining + 1; day = 1; month += 1;
            if month == 13 { month = 1; year = (year + 1) % 100; }
        }
        r[7] = encode(day); r[9] = encode(month); r[10] = encode(year);
    }
    fn write(&mut self, register: u8, value: u8) -> bool {
        let i = usize::from(register);
        match register {
            0 if value == 0x58 => { self.reset(); },
            0 => {
                // EXT_TEST, calibration interrupt and every other software
                // reset sequence are unsupported, including the vendor's 11h.
                if value & !0x23 != 0 { return false; }
                let was_stopped = self.registers[0] & 0x20 != 0;
                let stopped = value & 0x20 != 0;
                if stopped != was_stopped {
                    self.pending = false;
                    // §8.2.1.2: first increment 0.507813..0.507935 seconds
                    // after STOP release. Choose the 65/128 s lower phase,
                    // rounded up to a guest cycle; no crystal jitter is implied.
                    self.next_tick = self.cycle.saturating_add(
                        ((u128::from(self.hz) * 65).div_ceil(128)).max(1) as u64);
                }
                self.registers[0] = value;
            },
            1 => {
                if value & 0xb0 != 0 { return false; } // no AIE / MI / HMI engine
                self.registers[1] = (value & 7) | (self.registers[1] & value & 0x48);
            },
            2 => { if value != 0 { return false; } self.registers[2] = 0; },
            3 | 10 | 16 => self.registers[i] = value,
            // OS is set by oscillator failure/reset, and can be cleared by a
            // write. Writing a 1 does not invent another oscillator failure.
            4 => self.registers[4] = (value & 0x7f) | (self.registers[4] & value & 0x80),
            5 => self.registers[5] = value & 0x7f,
            6 | 7 => self.registers[i] = value & 0x3f,
            8 => self.registers[8] = value & 7,
            9 => self.registers[9] = value & 0x1f,
            11..=15 => {
                if value & 0x80 == 0 { return false; } // disabled alarm storage only
                let mask = match register { 11 | 12 => 0xff, 13 | 14 => 0xbf, _ => 0x87 };
                self.registers[i] = value & mask;
            },
            17 => {
                if value & !0x19 != 0 { return false; } // no TE / TIE or reserved bits
                self.registers[17] = value;
            },
            _ => return false,
        }
        true
    }
    fn stop(&mut self) {
        self.access = false;
        if self.pending {
            self.pending = false;
            if self.registers[0] & 0x20 == 0 { self.add_seconds(1); }
        }
    }
}
fn encode(value: u8) -> u8 { ((value / 10) << 4) | (value % 10) }
fn decode(value: u8, max: u8) -> Option<u8> {
    let n = (value >> 4) * 10 + (value & 15);
    ((value & 15) < 10 && (value >> 4) < 10 && n <= max).then_some(n)
}
fn month_days(month: u8, year: u8) -> u8 {
    match month { 2 => if year.is_multiple_of(4) { 29 } else { 28 }, 4 | 6 | 9 | 11 => 30, _ => 31 }
}

pub struct Pcf85063 {
    state: Arc<Mutex<Pcf85063State>>,
    pointer: Option<u8>,
    first: bool,
}
impl Pcf85063 {
    pub fn new(state: Arc<Mutex<Pcf85063State>>) -> Self {
        Self { state, pointer: Some(0), first: true }
    }
}
impl I2cDevice for Pcf85063 {
    fn start(&mut self, read: bool) -> bool {
        self.state.lock().expect("PCF85063 state poisoned").access = true;
        if !read { self.first = true; }
        !read || self.pointer.is_some()
    }
    fn write(&mut self, value: u8) -> bool {
        if self.first {
            self.first = false;
            self.pointer = (value < 18).then_some(value);
            return self.pointer.is_some();
        }
        let Some(register) = self.pointer else { return false; };
        if !self.state.lock().expect("PCF85063 state poisoned").write(register, value) { return false; }
        self.pointer = Some(if register == 0 && value == 0x58 { 0 } else { (register + 1) % 18 });
        true
    }
    fn read(&mut self) -> u8 {
        let Some(register) = self.pointer else { return 255; };
        let value = self.state.lock().expect("PCF85063 state poisoned").registers[usize::from(register)];
        self.pointer = Some((register + 1) % 18); value
    }
    fn stop(&mut self) { self.state.lock().expect("PCF85063 state poisoned").stop(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup(hz: u64) -> (Arc<Mutex<Pcf85063State>>, Pcf85063) {
        let s = Arc::new(Mutex::new(Pcf85063State::new(hz))); let d = Pcf85063::new(s.clone()); (s, d)
    }
    fn write(d: &mut Pcf85063, register: u8, bytes: &[u8]) {
        assert!(d.start(false)); assert!(d.write(register));
        for &b in bytes { assert!(d.write(b), "register {register:02x}, byte {b:02x}"); } d.stop();
    }
    fn read(d: &mut Pcf85063, register: u8, n: usize) -> Vec<u8> {
        assert!(d.start(false)); assert!(d.write(register)); assert!(d.start(true));
        let bytes = (0..n).map(|_| d.read()).collect(); d.stop(); bytes
    }
    #[test]
    fn table8_reset_os_flag_and_guest_time() {
        let (s, mut d) = setup(1000);
        assert_eq!(read(&mut d, 0, 18), [0,0,0,0,0x80,0,0,1,6,1,0,0x80,0x80,0x80,0x80,0x80,0,0x18]);
        s.lock().unwrap().advance_to(2000); assert_eq!(read(&mut d, 4, 3), [0x82,0,0]);
        write(&mut d, 4, &[0]); write(&mut d, 4, &[0x80]);
        assert_eq!(read(&mut d, 4, 1), [0], "software cannot manufacture OS");
        write(&mut d, 3, &[0x5a]); write(&mut d, 0, &[0x58]);
        assert!(d.start(true)); assert_eq!(d.read(), 0, "reset returns pointer to 00h"); d.stop();
        assert_eq!(read(&mut d, 3, 8), [0,0x80,0,0,1,6,1,0]);
        s.lock().unwrap().advance_to(2999); assert_eq!(read(&mut d, 4, 1), [0x80]);
        s.lock().unwrap().advance_to(3000); assert_eq!(read(&mut d, 4, 1), [0x81]);
    }
    #[test]
    fn stop_preserves_date_and_uses_documented_half_second_restart_window() {
        let (s, mut d) = setup(128000); write(&mut d, 0, &[0x21]);
        s.lock().unwrap().advance_to(1_280_000); assert_eq!(read(&mut d, 4, 1), [0x80]);
        write(&mut d, 4, &[0x59,0x59,0x23,0x31,6,0x12,0x99]); write(&mut d, 0, &[1]);
        s.lock().unwrap().advance_to(1_344_999); assert_eq!(read(&mut d, 4, 3), [0x59,0x59,0x23]);
        s.lock().unwrap().advance_to(1_345_000); assert_eq!(read(&mut d, 4, 7), [0,0,0,1,0,1,0]);
        s.lock().unwrap().advance_to(1_473_000); assert_eq!(read(&mut d, 4, 1), [1]);
    }
    #[test]
    fn calendar_leap_days_weekdays_and_twelve_hour_rollovers() {
        for (year, day, want) in [(0x24,0x28,[0x29,0x02]),(0x23,0x28,[1,3]),(0,0x28,[0x29,2])] {
            let (s, mut d) = setup(1000); write(&mut d, 4, &[0x59,0x59,0x23,day,6,2,year]);
            s.lock().unwrap().advance_to(1000);
            assert_eq!(read(&mut d, 4, 7), [0,0,0,want[0],0,want[1],year]);
        }
        for (hour, want, day) in [(0x11,0x32,1),(0x31,0x12,2),(0x32,0x21,1)] {
            let (s, mut d) = setup(1000); write(&mut d, 0, &[2]);
            write(&mut d, 4, &[0x59,0x59,hour,1,6,1,0]); s.lock().unwrap().advance_to(1000);
            let value = read(&mut d, 4, 7); assert_eq!(value[2], want); assert_eq!(value[3], day);
        }
    }
    #[test]
    fn repeated_start_burst_defers_only_one_tick_and_stop_releases_it() {
        let (s, mut d) = setup(1000); write(&mut d, 4, &[0x59,0x59,0x23,0x31,2,0x12,0x26]);
        assert!(d.start(false)); assert!(d.write(4)); s.lock().unwrap().advance_to(1000);
        assert!(d.start(true)); assert_eq!(d.read(), 0x59); s.lock().unwrap().advance_to(3000);
        assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(), [0x59,0x23,0x31,2,0x12,0x26]); d.stop();
        assert_eq!(read(&mut d, 4, 7), [0,0,0,1,3,1,0x27]);
    }
    #[test]
    fn exact_pointer_wrap_and_unsupported_operations_do_not_fake_completion() {
        let (_, mut d) = setup(1000); write(&mut d, 17, &[0x19,1]); assert_eq!(read(&mut d, 17, 2), [0x19,1]);
        for (register,value) in [(0,0x11),(0,0x80),(0,4),(1,0x80),(1,0x10),(2,1),(11,0),(17,4),(17,2)] {
            assert!(d.start(false)); assert!(d.write(register)); assert!(!d.write(value)); d.stop();
        }
        assert_eq!(read(&mut d, 0, 3), [1,0,0]);
        assert!(d.start(false)); assert!(!d.write(18)); assert!(!d.start(true)); assert_eq!(d.read(),255); d.stop();
    }
    #[test]
    fn invalid_dates_are_retained_and_large_time_jumps_are_bounded() {
        for register in 4..=10 { for value in 0..=255 {
            let (s, mut d) = setup(1); write(&mut d, register, &[value]);
            s.lock().unwrap().advance_to(u64::MAX); let _ = read(&mut d, 4, 7);
        }}
        let (s, mut d) = setup(1); write(&mut d, 4, &[0x6a,0,0,1,0,0x13,0]);
        s.lock().unwrap().advance_to(1000); assert_eq!(read(&mut d, 4, 7), [0x6a,0,0,1,0,0x13,0]);
        let (s, mut d) = setup(1); s.lock().unwrap().advance_to(36525*86400);
        assert_eq!(read(&mut d, 4, 7), [0x80,0,0,1,5,1,0]);
    }
}
