//! GPIO matrix: output/enable/input words, per-pin config and interrupt types, the function
//! selection tables, and the edge queues the SoC hands to its board and pulse counters.
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;

// ------------------------------------------------------------------ GPIO
pub struct Gpio {
    pub out: u64, pub enable: u64, pub input: u64, pub status: u64, pub pin: [u32; 49],
    pub func_in_sel: [u32; 256], pub func_out_sel: [u32; 49], ram: RegRam,
    pub input_changes: Vec<(u8, bool)>,
    /// (pin, level) changes of enabled outputs since last drain
    pub changes: Vec<(u8, bool)>,
    pub strap: u32,
    pull_up: u64, pull_down: u64, open_drain: u64,
    external_mask: u64, external_levels: u64,
    pub int_ena_pins: u64,
    matrix_input_pins: u64,
    /// GPIO matrix input-select bit: 6 on C3, 7 on S3/C6.
    pub input_select: u32,
}
impl Gpio {
    pub fn new() -> Self { Gpio { out: 0, enable: 0, input: (1u64 << 49) - 1, status: 0, pin: [0; 49], func_in_sel: [0x3c; 256], func_out_sel: [0x100; 49], ram: RegRam::new(), changes: Vec::new(), strap: 0x0f, input_changes: Vec::new(), pull_up: 0, pull_down: 0, open_drain: 0, external_mask: 0, external_levels: 0, int_ena_pins: 0, matrix_input_pins: 0, input_select: 0x80 } }
    /// Report every pin whose driven level changed: `out & enable` before against after. Both
    /// words matter — a driver that toggles the output *enable* to produce a level (IDF 5.5's
    /// esp_lcd releases the D/C line after each colour transfer and re-enables it before the
    /// next) changes what a board sees just as much as one that toggles `out`.
    fn note_out(&mut self, old_out: u64, old_enable: u64) {
        let vis = self.out & self.enable; let oldvis = old_out & old_enable;
        let mut diff = (vis ^ oldvis) & ((1u64 << 49) - 1);
        while diff != 0 {
            let p = diff.trailing_zeros() as u8;
            self.changes.push((p, vis & (1u64 << p) != 0));
            diff &= diff - 1;
        }
    }
    /// Drive an input; returns whether its resolved level changed.
    pub fn set_input(&mut self, pin: u8, level: bool) -> bool {
        if pin >= 49 { return false; }
        let mask = 1u64 << pin;
        self.external_mask |= mask;
        if level { self.external_levels |= mask; } else { self.external_levels &= !mask; }
        self.resolve(mask, u64::MAX)
    }
    /// Stop driving a pin externally; return whether output/pull resolution changed its level.
    pub fn release_input(&mut self, pin: u8) -> bool {
        if pin >= 49 { return false; }
        let mask = 1u64 << pin;
        self.external_mask &= !mask;
        self.resolve(mask, u64::MAX)
    }
    /// IDF v5.5.4 components/soc/esp32{c3,c6,s3}/register/soc/io_mux_reg.h:42-50.
    pub fn set_pad(&mut self, pin: u8, v: u32) {
        self.set_pulls(pin, v & (1 << 8) != 0, v & (1 << 7) != 0);
    }
    pub fn set_pulls(&mut self, pin: u8, up: bool, down: bool) {
        if pin >= 49 { return; }
        let mask = 1u64 << pin;
        self.pull_up = (self.pull_up & !mask) | if up { mask } else { 0 };
        self.pull_down = (self.pull_down & !mask) | if down { mask } else { 0 };
        self.resolve(mask, u64::MAX);
    }
    /// Preserve host drives across a chip reset, without retaining pad configuration or edges.
    pub fn restore_external(&mut self, old: &Self) {
        self.external_mask = old.external_mask;
        self.external_levels = old.external_levels;
        self.resolve(self.external_mask, u64::MAX);
        self.input_changes.clear();
    }
    // INT_TYPE/INT_ENA: IDF v5.5.4 components/soc/esp32s3/register/soc/gpio_reg.h:278-301.
    // C3/C6 use the same fields; see docs/evidence/gpio-input-feedback/README.md.
    fn resolve(&mut self, mask: u64, record: u64) -> bool {
        let mask = mask & ((1u64 << 49) - 1);
        // Host-driven levels are ideal digital sources; opposing output drivers do not
        // model electrical contention. Floating and simultaneous pulls retain HIGH.
        let passive = self.pull_up | !self.pull_down;
        // IDF v5.4.2 (Cardputer-Adv) and v5.5.1 components/soc/esp32s3/register/
        // soc/gpio_reg.h: GPIO_PINn_PAD_DRIVER bit 2. An enabled open-drain HIGH
        // releases the pad, so the pulls apply just as when the output is disabled.
        let enabled = self.enable & !(self.open_drain & self.out);
        let driven = (self.out & enabled) | (passive & !enabled);
        let levels = (self.external_levels & self.external_mask) | (driven & !self.external_mask);
        let mut changed = (self.input ^ levels) & mask;
        self.input = (self.input & !mask) | (levels & mask);
        let input_changed = changed != 0;
        while changed != 0 {
            let pin = changed.trailing_zeros() as u8;
            let bit = 1u64 << pin;
            let level = self.input & bit != 0;
            if record & bit != 0 { self.input_changes.push((pin, level)); }
            let typ = (self.pin[pin as usize] >> 7) & 7;
            if (level && typ == 1) || (!level && typ == 2) || typ == 3 {
                self.status |= bit;
            }
            changed &= changed - 1;
        }
        input_changed
    }
    pub fn level(&self, pin: u8) -> bool {
        if pin as usize >= self.pin.len() { return false; }
        self.input & (1u64 << pin) != 0
    }
    /// `GPIO_STATUS_REG` (0x44) / `GPIO_PCPU_INT_REG` (0x5c, `soc/gpio_reg.h`, IDF v5.5.3): the
    /// latched edge bits, plus every level-type pin (`INT_TYPE` 4: low, 5: high) whose condition
    /// holds right now. A level interrupt's status stays set for as long as the level does, and
    /// `gpio_intr_service` (`esp_driver_gpio/src/gpio.c`) reads `PCPU_INT` through
    /// `gpio_ll_get_intr_status` to find the pins whose handlers to call: without them here it
    /// found none. Masking of `PCPU_INT` by `INT_ENA` is not modelled, as before.
    fn status_now(&self) -> u64 {
        (0..49u8).filter(|&p| { let typ = (self.pin[p as usize] >> 7) & 7; (typ == 4 && !self.level(p)) || (typ == 5 && self.level(p)) }).fold(self.status, |m, p| m | (1u64 << p))
    }
    pub fn irq(&self) -> bool {
        // level-type interrupts on current input, plus latched edge status, gated by INT_ENA (bits 13..17, bit 13 = core0)
        (0..49u8).any(|p| { let cfg = self.pin[p as usize]; let ena = (cfg >> 13) & 1 != 0; let typ = (cfg >> 7) & 7;
            ena && ((self.status & (1u64 << p) != 0) || (typ == 4 && !self.level(p)) || (typ == 5 && self.level(p))) })
    }
    pub fn read(&mut self, off: u32) -> u32 {
        match off {
            0x4 => self.out as u32, 0x10 => (self.out >> 32) as u32,
            0x20 => self.enable as u32, 0x2c => (self.enable >> 32) as u32,
            0x38 => self.strap,
            0x3c => self.input as u32, 0x40 => (self.input >> 32) as u32,
            0x44 | 0x5c => self.status_now() as u32, 0x50 | 0x68 => (self.status_now() >> 32) as u32,   // STATUS, and PCPU_INT: the status seen by core 0
            0x74..=0x134 => self.pin[((off - 0x74) / 4) as usize],
            0x154..=0x550 => self.func_in_sel[((off - 0x154) / 4) as usize],
            0x554..=0x614 => self.func_out_sel[((off - 0x554) / 4) as usize],
            0x6fc => 0x2006130,
            _ => self.ram.read(off),
        }
    }
    // Keep the Device wrapper specialized to its bounded register offset even
    // when another chip adapter also calls this shared implementation.
    #[inline(always)]
    pub fn write(&mut self, off: u32, v: u32) {
        let (old, old_enable) = (self.out, self.enable);
        match off {
            0x4 => self.out = (self.out & !0xffff_ffff) | v as u64,
            0x8 => self.out |= v as u64,
            0xc => self.out &= !(v as u64),
            0x10 => self.out = (self.out & 0xffff_ffff) | ((v as u64 & 0x1ffff) << 32),
            0x14 => self.out |= (v as u64) << 32,
            0x18 => self.out &= !((v as u64) << 32),
            0x20 => { self.enable = (self.enable & !0xffff_ffff) | v as u64; }
            0x24 => self.enable |= v as u64,
            0x28 => self.enable &= !(v as u64),
            0x2c => self.enable = (self.enable & 0xffff_ffff) | ((v as u64 & 0x1ffff) << 32),
            0x30 => self.enable |= (v as u64) << 32,
            0x34 => self.enable &= !((v as u64) << 32),
            0x44 => self.status = (self.status & !0xffff_ffff) | v as u64,
            0x48 => self.status |= v as u64,
            0x4c => self.status &= !(v as u64),
            0x50 => self.status = (self.status & 0xffff_ffff) | ((v as u64) << 32),
            0x54 => self.status |= (v as u64) << 32,
            0x58 => self.status &= !((v as u64) << 32),
            0x74..=0x134 => {
                let pin = ((off - 0x74) / 4) as usize;
                self.pin[pin] = v;
                let mask = 1u64 << pin;
                let old_drain = self.open_drain;
                self.open_drain = (self.open_drain & !mask) | if v & 4 != 0 { mask } else { 0 };
                if old_drain != self.open_drain { self.resolve(mask, u64::MAX); }
                let enabled = v & (1 << 13) != 0 && matches!((v >> 7) & 7, 1..=5);
                self.int_ena_pins = (self.int_ena_pins & !(1 << pin)) | (u64::from(enabled) << pin);
            },
            0x154..=0x550 => {
                self.func_in_sel[((off - 0x154) / 4) as usize] = v;
                self.matrix_input_pins = self.func_in_sel.iter().fold(0, |mask, &sel| {
                    let pin = sel & (self.input_select / 2 - 1);
                    if sel & self.input_select != 0 && pin < 49 { mask | (1 << pin) } else { mask }
                });
            },
            0x554..=0x614 => self.func_out_sel[((off - 0x554) / 4) as usize] = v,
            _ => self.ram.write(off, v),
        }
        // enable changes also change what's visible on pins
        if matches!(off, 0x4 | 0x8 | 0xc | 0x10 | 0x14 | 0x18 | 0x20 | 0x24 | 0x28 | 0x2c | 0x30 | 0x34) { self.note_out(old, old_enable); self.resolve((old ^ self.out) | (old_enable ^ self.enable), self.matrix_input_pins); }
    }
}

impl Default for Gpio { fn default() -> Self { Self::new() } }

impl Device for Gpio {
    fn read(&mut self, off: u32) -> u32 { Gpio::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Gpio::write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
}
