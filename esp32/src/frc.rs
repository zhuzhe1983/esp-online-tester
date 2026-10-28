//! ESP32 legacy FRC2, used by ESP-IDF v3.2.3 esp_timer_esp32.c.
//! Registers: that tag's components/soc/esp32/include/soc/frc_timer_reg.h:23-46;
//! APB, up-counting and software reload: esp_timer_esp32.c:34-80,161-177,347-371.
//! This slice implements free-running FRC2 with level IRQ. FRC1, edge pulses and
//! autoload are not implemented; reserved prescalers/autoload do not advance.
use esp_periph::{Device, WriteEffect};

#[derive(Default)]
pub struct Frc2 {
    load: u32,
    count: u32,
    control: u32,
    alarm: u32,
    phase: u64,
    pending: bool,
}

impl Frc2 {
    fn divider(&self) -> Option<u64> {
        if self.control & 0xc0 != 0x80 { return None; }
        match (self.control >> 1) & 7 { 0 => Some(1), 2 => Some(16), 4 => Some(256), _ => None }
    }
    fn distance(&self) -> u64 {
        // A compare already equal after a LOAD is observed on the next APB edge.
        u64::from(self.alarm.wrapping_sub(self.count)).max(1)
    }
}

impl Device for Frc2 {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x20 => self.load,
            0x24 => self.count,
            0x28 => self.control | (u32::from(self.pending) << 8),
            0x30 => self.alarm,
            _ => 0,
        }
    }
    fn write(&mut self, off: u32, value: u32) -> WriteEffect {
        match off {
            0x20 => { self.load = value; self.count = value; self.phase = 0; }
            0x28 => {
                let control = value & 0xcf;
                if control != self.control { self.phase = 0; }
                self.control = control;
            }
            0x2c => { if value & 1 != 0 { self.pending = false; } }
            0x30 => self.alarm = value,
            _ => (),
        }
        WriteEffect::NONE
    }
    fn tick(&mut self, ticks: u64) {
        let Some(divider) = self.divider() else { return };
        // Split before addition so even a caller's u64::MAX cannot overflow.
        let remainder = self.phase + ticks % divider;
        let steps = ticks / divider + remainder / divider;
        self.phase = remainder % divider;
        if steps >= self.distance() { self.pending = true; }
        self.count = self.count.wrapping_add(steps as u32);
    }
    fn irq_sources(&self) -> u64 { u64::from(self.pending && self.control & 1 != 0) }
    fn has_deadline(&self) -> bool { true }
    fn next_deadline(&self) -> Option<u64> {
        if self.pending || self.control & 1 == 0 { return None; }
        Some(self.distance() * self.divider()? - self.phase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idf32_free_running_compare_clear_reload_and_wrap() {
        let mut t = Frc2::default();
        t.write(0x30, 5); t.write(0x28, 0x81);
        assert_eq!(t.next_deadline(), Some(5));
        t.tick(4); assert_eq!(t.read(0x24), 4); assert_eq!(t.irq_sources(), 0);
        t.tick(1); assert_eq!(t.irq_sources(), 1); assert_eq!(t.read(0x28), 0x181);
        t.tick(3); assert_eq!(t.read(0x24), 8); // alarm does not reset/stop FRC2
        t.write(0x28, 0x81); assert_eq!(t.irq_sources(), 1); // status is read-only
        t.write(0x2c, 0); assert_eq!(t.irq_sources(), 1);
        t.write(0x2c, 1); assert_eq!(t.irq_sources(), 0);
        t.write(0x20, u32::MAX - 1); t.write(0x30, 1);
        assert_eq!(t.next_deadline(), Some(3));
        t.tick(3); assert_eq!(t.read(0x24), 1); assert_eq!(t.irq_sources(), 1);
        t.write(0x2c, 1); t.write(0x20, 0); t.write(0x30, 0);
        t.tick(1); assert_eq!(t.irq_sources(), 1); // esp_timer_impl_advance
    }
    #[test]
    fn divider_phase_reads_disabled_and_reserved_modes() {
        for (bits, divider) in [(0, 1), (4, 16), (8, 256)] {
            let mut t = Frc2::default();
            t.write(0x30, 2); t.write(0x28, 0x81 | bits);
            t.tick(divider - 1);
            assert_eq!(t.read(0x24), 0); assert_eq!(t.read(0x24), 0);
            assert_eq!(t.next_deadline(), Some(divider + 1));
            t.tick(1); assert_eq!(t.read(0x24), 1);
            t.tick(divider); assert_eq!(t.irq_sources(), 1);
        }
        for control in [0, 0x83, 0x87, 0x8b, 0xc1] {
            let mut t = Frc2::default(); t.write(0x28, control); t.tick(u64::MAX);
            assert_eq!(t.read(0x24), 0); assert_eq!(t.next_deadline(), None);
        }
        let mut t = Frc2::default(); t.write(0x28, 0x89); t.tick(u64::MAX);
        assert_eq!(t.read(0x24), u32::MAX); assert_eq!(t.read(0x28), 0x189);
        t.write(0x24, 7); assert_eq!(t.read(0x24), u32::MAX); // COUNT is read-only
    }
}
