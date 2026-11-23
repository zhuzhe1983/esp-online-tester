//! Functional noise-floor measurement handshake, NOT a radio or RF calibration model.
//! Exact SDK85c43024c -> esp32-wifi-lib c85f2d18: ram_check_noise_floor,
//! and rev300 ROM rom_check_noise_floor: C018 stop23, pulse reset25,
//! start23, wait done24, read C050 low10 minus1024. See associated platform note.
//! Chosen uncalibrated policy: 100us enabled-domain conversion, raw input fixture640.
//! C038/C080 retain observed mode words only: the real guest clears C080.0
//! before requesting measurement, disproving its use as an enable gate.
const START: u32 = 1 << 23;
const DONE: u32 = 1 << 24;
const RESET: u32 = 1 << 25;
#[derive(Clone)]
pub struct PhyNoise {
    control: u32,
    setup38: u32,
    setup80: u32,
    result: u16,
    input: u16,
    captured: u16,
    remaining: Option<u64>,
    latency: u64,
    clock: bool,
    reset_held: bool,
    done: bool,
    pub requests: u64,
    pub completed: u64,
    pub log: bool,
}
impl Default for PhyNoise {
    fn default() -> Self {
        Self::with_fixture(640, 24000)
    }
}
impl PhyNoise {
    /// Digital environment fixture, not a calibrated dBm measurement. The driver
    /// interprets low10 as raw-1024; keep that independently testable conversion.
    pub fn with_fixture(raw: u16, latency_ticks: u64) -> Self {
        Self {
            control: 0,
            setup38: 0,
            setup80: 0,
            result: 0,
            input: raw & 1023,
            captured: 0,
            remaining: None,
            latency: latency_ticks.max(1),
            clock: false,
            reset_held: false,
            done: false,
            requests: 0,
            completed: 0,
            log: false,
        }
    }
    pub fn set_input_raw(&mut self, raw: u16) {
        self.input = raw & 1023;
    }
    pub fn reset_for_reboot(&self) -> Self {
        let mut n = Self::with_fixture(self.input, self.latency);
        n.log = self.log;
        n
    }
    pub fn set_control(&mut self, clock: bool, reset: bool) {
        if self.log && (clock != self.clock || reset != self.reset_held) {
            eprintln!("NOISE_CLOCK enabled={clock} reset={reset}");
        }
        if reset {
            *self = self.reset_for_reboot();
        }
        self.reset_held = reset;
        self.clock = clock && !reset;
    }
    pub fn busy(&self) -> bool {
        self.remaining.is_some()
    }
    pub fn advancing(&self) -> bool {
        self.busy() && self.clock
    }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x18 => self.control | if self.done { DONE } else { 0 },
            0x38 => self.setup38,
            0x50 => u32::from(self.result),
            0x80 => self.setup80,
            _ => 0,
        }
    }
    pub fn write(&mut self, off: u32, value: u32) {
        if self.log {
            eprintln!(
                "NOISE_WRITE +{off:03x}={value:08x} previous={:08x}",
                self.read(off)
            );
        }
        if self.reset_held {
            return;
        }
        match off {
            0x18 => {
                let old = self.control;
                self.control = value & !DONE;
                if value & RESET != 0 {
                    self.remaining = None;
                    self.done = false;
                    self.result = 0;
                    return;
                }
                if value & START == 0 {
                    self.remaining = None;
                    self.done = false;
                } else if old & START == 0 {
                    self.done = false;
                    self.captured = self.input;
                    self.remaining = Some(self.latency);
                    self.requests = self.requests.wrapping_add(1);
                }
            }
            0x38 => self.setup38 = value,
            0x80 => self.setup80 = value,
            _ => (), // Result is RO; no guest write can manufacture completion/data.
        }
    }
    pub fn advance(&mut self, ticks: u64) {
        if !self.advancing() {
            return;
        }
        let left = self.remaining.unwrap();
        if ticks < left {
            self.remaining = Some(left - ticks);
            return;
        }
        self.result = self.captured;
        self.remaining = None;
        self.done = true;
        self.completed = self.completed.wrapping_add(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn configured() -> PhyNoise {
        let mut n = PhyNoise::with_fixture(640, 240);
        n.set_control(true, false);
        n.write(0x38, 1 << 26);
        n.write(0x80, 1);
        n
    }
    fn start(n: &mut PhyNoise) {
        n.write(0x18, 0);
        n.write(0x18, RESET);
        n.write(0x18, 0);
        n.write(0x18, START);
    }
    #[test]
    fn real_trigger_time_result_and_read_only_status() {
        let mut n = configured();
        n.advance(9999);
        assert_eq!(n.read(0x18) & DONE, 0);
        n.write(0x18, DONE);
        n.write(0x50, 640);
        assert_eq!(n.read(0x50), 0);
        assert_eq!(n.read(0x18) & DONE, 0);
        start(&mut n);
        for _ in 0..100 {
            assert_eq!(n.read(0x18) & DONE, 0);
        }
        n.advance(239);
        assert!(n.busy());
        n.advance(1);
        assert!(!n.busy());
        assert_eq!(n.read(0x18) & DONE, DONE);
        assert_eq!(n.read(0x50) as i32 - 1024, -384);
        assert_eq!((n.requests, n.completed), (1, 1));
    }
    #[test]
    fn fixture_captured_once_and_next_measurement_changes_driver_result() {
        let mut n = configured();
        start(&mut n);
        n.set_input_raw(650);
        n.advance(240);
        assert_eq!(n.read(0x50), 640);
        start(&mut n);
        n.advance(240);
        assert_eq!(n.read(0x50) as i32 - 1024, -374);
        assert_eq!((n.requests, n.completed), (2, 2));
    }
    #[test]
    fn gates_freeze_deadline_and_reset_held_ignores_requests() {
        let mut n = configured();
        start(&mut n);
        n.advance(120);
        n.write(0x80, 0);
        n.set_control(false, false);
        n.advance(u64::MAX);
        assert!(n.busy());
        n.set_control(true, false);
        n.advance(119);
        assert!(n.busy());
        n.advance(1);
        assert_eq!(n.completed, 1);
        start(&mut n);
        n.set_control(true, true);
        n.write(0x18, START);
        n.advance(1000);
        assert_eq!(n.read(0x18), 0);
        assert_eq!(n.read(0x50), 0);
        assert!(!n.busy());
    }
    #[test]
    fn stop_reset_and_busy_rewrites_cannot_fabricate_or_delay_completion() {
        let mut n = configured();
        start(&mut n);
        n.advance(120);
        n.write(0x18, START | DONE);
        n.advance(120);
        assert_eq!(n.completed, 1);
        start(&mut n);
        n.advance(120);
        n.write(0x18, 0);
        n.advance(1000);
        assert_eq!(n.completed, 1);
        assert!(!n.done);
        start(&mut n);
        n.write(0x18, RESET);
        assert!(!n.busy());
        assert_eq!(n.read(0x50), 0);
    }
    #[test]
    fn soc_ahb_alias_controls_clock_and_reboot_environment() {
        use emu_core::Bus;
        let mut b = crate::bus::SocBus::new(4 << 20, [0; 6]);
        b.periph.phy_noise.set_input_raw(650);
        b.write32(0x3ff0_00cc, 1 << 11).unwrap();
        b.write32(0x6001_c038, 1 << 26).unwrap();
        b.write32(0x6001_c080, 1).unwrap();
        b.write32(0x6001_c018, START).unwrap();
        b.periph.tick(24000);
        assert_eq!(b.read32(0x3ff5_c018).unwrap() & DONE, DONE);
        assert_eq!(b.read32(0x6001_c050).unwrap(), 650);
        esp_soc::SocBus::reboot(&mut b, [0; 6]);
        assert_eq!(b.read32(0x6001_c050).unwrap(), 0);
        assert_eq!(b.periph.phy_noise.input, 650);
        assert!(!b.periph.phy_noise.busy());
    }
}
