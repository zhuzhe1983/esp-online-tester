//! S31 timer-group watchdog subset with explicit XTAL ticks, not general timers.
//! ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643: timer_group_reg.h,
//! hp_sys_clkrst_reg.h, timg_ll.h and mwdt_ll.h. Flashboot's automatic timing,
//! CPU-cycle conversion and physical reset-domain/pulse behavior are unsupported.
use emu_core::Fault;
pub const BASES: [u32; 2] = [0x2058_0000, 0x2058_1000];
pub const CLOCKS: [u32; 2] = [0x2058_7114, 0x2058_711c];
const KEY: u32 = 0x50d8_3aa1;
const EN: u32 = 1 << 31;
const UPDATE: u32 = 1 << 22;
const CONFIG_MASK: u32 = 0xff9f_f000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reset { Cpu { core0: bool, core1: bool }, System }
#[derive(Clone)]
struct Config { control: u32, divisor: u32, hold: [u32; 4] }
impl Default for Config {
    fn default() -> Self { Self { control: 0x4c000, divisor: 1, hold: [26_000_000, 134_217_727, 1_048_575, 1_048_575] } }
}
pub struct Mwdt { clock: u32, pending: Config, active: Config, key: u32, raw: u32, ena: u32, stage: usize, elapsed: u64, fraction: u64, pub reset: Option<Reset> }
impl Default for Mwdt {
    fn default() -> Self { Self { clock: 0x921, pending: Config::default(), active: Config::default(), key: KEY, raw: 0, ena: 0, stage: 0, elapsed: 0, fraction: 0, reset: None } }
}
impl Mwdt {
    pub fn clock_read(&self) -> u32 { self.clock }
    pub fn clock_write(&mut self, value: u32) -> Result<(), Fault> {
        // Only documented default XTAL selection; force-no-reset and alternate
        // sources remain unsupported. T0/T1 do not acquire fake timer behavior.
        if value & 0x6dc != 0 { return Err(Fault::Prohibited); }
        let clock = value & 0xfff;
        if clock & 2 != 0 { *self = Self::default(); }
        self.clock = clock; Ok(())
    }
    fn bus_on(&self) -> bool { self.clock & 3 == 1 }
    pub fn irq(&self) -> bool { self.raw & self.ena & 4 != 0 }
    fn feed(&mut self) { self.stage = 0; self.elapsed = 0; self.fraction = 0; }
    pub fn read(&self, off: u32) -> Result<u32, Fault> {
        if !self.bus_on() { return Err(Fault::Prohibited); }
        match off {
            0x48 => Ok(self.pending.control), 0x4c => Ok(self.pending.divisor << 16),
            0x50 | 0x54 | 0x58 | 0x5c => Ok(self.pending.hold[((off - 0x50) / 4) as usize]),
            0x60 | 0x7c => Ok(0), 0x64 => Ok(self.key), 0x70 => Ok(self.ena), 0x74 => Ok(self.raw), 0x78 => Ok(self.raw & self.ena),
            _ => Err(Fault::Unmapped), // GP timers/calibration are not RegRam-ready placeholders.
        }
    }
    pub fn write(&mut self, off: u32, value: u32) -> Result<(), Fault> {
        if !self.bus_on() { return Err(Fault::Prohibited); }
        match off {
            0x48 if self.key == KEY => {
                let control = value & CONFIG_MASK;
                if (self.active.control ^ control) & EN != 0 { self.feed(); }
                self.pending.control = control;
                // EN is directly written by the LL; staged fields are latched by
                // CONF_UPDATE_EN. The functional update is atomic, with WT read0.
                self.active.control = (self.active.control & !EN) | (control & EN);
                if value & UPDATE != 0 { self.active = self.pending.clone(); }
            }
            0x4c if self.key == KEY => {
                if value >> 16 == 0 { return Err(Fault::Prohibited); }
                self.pending.divisor = value >> 16; if value & 1 != 0 { self.fraction = 0; }
            }
            0x50 | 0x54 | 0x58 | 0x5c if self.key == KEY => self.pending.hold[((off - 0x50) / 4) as usize] = value,
            0x60 if self.key == KEY => self.feed(),
            0x48..=0x60 if off & 3 == 0 => {}, // protected writes do not change active state
            0x64 => self.key = value,
            0x70 => { if value & 3 != 0 { return Err(Fault::Prohibited); } self.ena = value & 4; }
            0x74 | 0x7c => self.raw &= !(value & 4), 0x78 => {},
            _ => return Err(Fault::Unmapped),
        }
        Ok(())
    }
    /// Explicit source-domain ticks; no guessed mapping from guest instructions.
    /// Flashboot automatic timeout behavior is not implemented and cannot report
    /// successful ticking while selected. Reset requests stop this finite stage run.
    pub fn advance_xtal(&mut self, ticks: u64) -> Result<(), Fault> {
        if ticks == 0 || self.reset.is_some() || self.clock & 0x802 != 0x800 { return Ok(()); }
        if self.active.control & (1 << 14) != 0 { return Err(Fault::Prohibited); }
        if self.active.control & EN == 0 { return Ok(()); }
        let total = u128::from(self.fraction) + u128::from(ticks); let div = u128::from(self.active.divisor);
        self.fraction = (total % div) as u64; self.elapsed = self.elapsed.saturating_add((total / div) as u64);
        for _ in 0..4 {
            if self.stage >= 4 { break; }
            let timeout = u64::from(self.active.hold[self.stage]); if self.elapsed < timeout { break; }
            self.elapsed -= timeout; let action = (self.active.control >> (29 - 2 * self.stage)) & 3; self.stage += 1;
            match action {
                1 => self.raw |= 4,
                2 => { let core0 = self.active.control & (1 << 13) != 0; let core1 = self.active.control & (1 << 12) != 0;
                    if core0 || core1 { self.reset = Some(Reset::Cpu { core0, core1 }); } }
                3 => self.reset = Some(Reset::System), _ => {},
            }
            if self.reset.is_some() { break; }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn write_protection_update_feed_irq_and_reset_obey_real_counter_boundaries() {
        let mut w = Mwdt::default(); assert_eq!(w.read(0x64), Ok(KEY));
        assert_eq!(w.advance_xtal(1), Err(Fault::Prohibited)); // no automatic flashboot timing is claimed
        w.write(0x48, UPDATE).unwrap(); w.write(0x64, 0).unwrap();
        w.write(0x50, 3).unwrap(); assert_eq!(w.read(0x50), Ok(26_000_000));
        w.write(0x64, KEY).unwrap(); w.write(0x50, 3).unwrap(); w.write(0x54, 2).unwrap(); w.write(0x4c, 2 << 16).unwrap();
        w.write(0x48, EN | 1 << 29 | 3 << 27).unwrap();
        w.advance_xtal(10).unwrap(); assert!(!w.irq()); assert_eq!(w.reset, None); // no CONF_UPDATE yet
        w.write(0x48, EN | 1 << 29 | 3 << 27 | UPDATE).unwrap(); w.write(0x60, 0).unwrap();
        assert_eq!(w.read(0x48).unwrap() & UPDATE, 0); w.write(0x70, 4).unwrap();
        w.advance_xtal(5).unwrap(); assert!(!w.irq()); w.advance_xtal(1).unwrap(); assert!(w.irq());
        w.write(0x78, 0).unwrap(); assert!(w.irq()); w.write(0x7c, 4).unwrap(); assert!(!w.irq());
        w.write(0x64, 0).unwrap(); w.write(0x60, 1).unwrap(); // locked feed cannot postpone stage1
        w.advance_xtal(3).unwrap(); assert_eq!(w.reset, None); w.advance_xtal(1).unwrap(); assert_eq!(w.reset, Some(Reset::System));
    }
    #[test]
    fn clock_gates_module_reset_and_target_masks_are_independent() {
        for targets in 0..4 {
            let mut w = Mwdt::default(); w.write(0x50, 1).unwrap(); w.write(0x54, 2).unwrap();
            w.write(0x48, EN | 2 << 29 | 3 << 27 | targets << 12 | UPDATE).unwrap();
            w.clock_write(0x121).unwrap(); w.advance_xtal(100).unwrap(); assert_eq!(w.reset, None);
            w.clock_write(0x920).unwrap(); assert_eq!(w.read(0x48), Err(Fault::Prohibited));
            w.advance_xtal(1).unwrap(); // APB clock gates access, not the independent WDT clock
            if targets == 0 { assert_eq!(w.reset, None); w.advance_xtal(2).unwrap(); assert_eq!(w.reset, Some(Reset::System)); }
            else { assert_eq!(w.reset, Some(Reset::Cpu { core0: targets & 2 != 0, core1: targets & 1 != 0 })); }
            w.clock_write(0x923).unwrap(); assert_eq!(w.reset, None); assert_eq!(w.read(0x48), Err(Fault::Prohibited));
            w.clock_write(0x921).unwrap(); assert_eq!(w.read(0x48), Ok(0x4c000)); assert_eq!(w.read(0x64), Ok(KEY));
            assert_eq!(w.clock_write(0xb21), Err(Fault::Prohibited)); assert_eq!(w.clock_read(), 0x921);
            assert_eq!(w.write(0x4c, 0), Err(Fault::Prohibited)); assert_eq!(w.read(0x4c), Ok(1 << 16));
            assert_eq!(w.write(0x70, 1), Err(Fault::Prohibited)); assert_eq!(w.read(0), Err(Fault::Unmapped));
        }
    }
}
