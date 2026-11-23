//! Bounded classic ESP32 ROM300 BT control-register surface, not a controller.
//! ROM r_ld_reset @4003c714 compares +004 to08000b00 and requests/polls
//! control+000 bits31/30. r_lld_core_reset @40048a9c compares +204 to08000900.
//! Revision constants model that specific IP profile; guest cannot change them.
//! Reset strobe completion latency is chosen/configurable, NOT silicon calibrated.
const STROBES: u32 = 3 << 30;
pub struct BtInit {
    control: [u32; 2],
    pending: [u32; 2],
    left: [u64; 2],
    latency: u64,
    enabled: bool,
    reset: bool,
    pub requests: u64,
    pub completed: u64,
}
impl Default for BtInit {
    fn default() -> Self {
        Self::with_latency(240)
    }
}
impl BtInit {
    pub fn with_latency(ticks: u64) -> Self {
        Self {
            control: [0; 2],
            pending: [0; 2],
            left: [0; 2],
            latency: ticks.max(1),
            enabled: false,
            reset: false,
            requests: 0,
            completed: 0,
        }
    }
    pub fn set_control(&mut self, enabled: bool, reset: bool) {
        if reset {
            *self = Self::with_latency(self.latency);
        }
        self.enabled = enabled && !reset;
        self.reset = reset;
    }
    pub fn active(&self) -> bool {
        self.enabled && self.pending != [0; 2]
    }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            4 => 0x0800_0b00,
            0x204 => 0x0800_0900,
            0 => self.control[0] | self.pending[0],
            0x200 => self.control[1] | self.pending[1],
            _ => 0,
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        if self.reset || !matches!(off, 0 | 0x200) {
            return;
        }
        let i = (off >> 9) as usize;
        self.control[i] = v & !STROBES;
        if v & STROBES != 0 {
            if self.pending[i] == 0 {
                self.left[i] = self.latency;
                self.requests = self.requests.wrapping_add(1);
            }
            self.pending[i] |= v & STROBES;
        }
    }
    pub fn advance(&mut self, ticks: u64) {
        if !self.enabled {
            return;
        }
        for i in 0..2 {
            if self.pending[i] == 0 {
                continue;
            }
            if ticks < self.left[i] {
                self.left[i] -= ticks;
                continue;
            }
            if self.pending[i] & (1 << 31) != 0 {
                self.control[i] = 0;
            }
            self.pending[i] = 0;
            self.left[i] = 0;
            self.completed = self.completed.wrapping_add(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revisions_read_only_and_banks_not_aliases() {
        let mut d = BtInit::default();
        d.write(4, 0);
        d.write(0x204, 0);
        assert_eq!(d.read(4), 0x08000b00);
        assert_eq!(d.read(0x204), 0x08000900);
        d.write(0, 0x123);
        assert_eq!(d.read(0x200), 0);
        assert_eq!(d.read(0), 0x123);
    }
    #[test]
    fn soft_reset_is_request_time_clock_and_domain_dependent() {
        let mut d = BtInit::with_latency(10);
        d.write(0, 1 << 31);
        d.advance(999);
        assert_eq!(d.read(0), 1 << 31);
        d.set_control(true, false);
        d.advance(9);
        for _ in 0..100 {
            assert_ne!(d.read(0) & STROBES, 0);
        }
        d.write(0, 0);
        d.advance(1);
        assert_eq!(d.read(0), 0);
        assert_eq!(d.completed, 1);
        d.write(0x200, 1 << 30);
        d.set_control(true, true);
        d.write(0, 1 << 31);
        assert_eq!(d.read(0x200), 0);
        assert_eq!(d.read(0), 0);
    }
}
