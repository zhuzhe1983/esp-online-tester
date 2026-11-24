//! Exact plain 1.43-inch and 1.8-inch AMOLED boards, distinct from 1.43C and
//! the earlier host-level AMOLED18 stub. Sources: eighth-batch source manifest.
//! Unknown silicon/OTP fields remain unavailable; neither model fakes a PMU
//! watchdog, battery, audio engine or complete manufacturer application.
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, atomic::Ordering}};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardEdge, BoardModel, SpiQspiPins}, pins::PinRoutes,
    devices::{axp2101::{Axp2101, Axp2101Power, PowerRail}, es8311::Es8311,
        ft3168::{Ft3168, Ft3168State}, i2c_bitbang::I2cBitBang, imu_motion::ImuMotion,
        pcf85063::{Pcf85063, Pcf85063State}, qmi8658::{Qmi8658, Qmi8658Port},
        qspi_dcs::QspiDcsPanel, tca9554::{Tca9554, Tca9554State}}};

const HZ: u64 = 240_000_000;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompactVariant { Amoled143Sh8601, Amoled143Co5300, Amoled18V1, Amoled18V2 }
impl CompactVariant {
    fn watch(self) -> bool { matches!(self, Self::Amoled18V1 | Self::Amoled18V2) }
    fn cst(self) -> bool { self == Self::Amoled18V2 }
    fn sh(self) -> bool { matches!(self, Self::Amoled143Sh8601 | Self::Amoled18V1) }
    pub fn name(self) -> &'static str { match self {
        Self::Amoled143Sh8601 => "waveshare-s3-touch-amoled143-sh8601",
        Self::Amoled143Co5300 => "waveshare-s3-touch-amoled143-co5300",
        Self::Amoled18V1 => "waveshare-s3-touch-amoled18-v1",
        Self::Amoled18V2 => "waveshare-s3-touch-amoled18-v2",
    } }
    fn dimensions(self) -> (u16, u16, u16) { match self {
        Self::Amoled143Sh8601 => (466, 466, 0), Self::Amoled143Co5300 => (466, 466, 6),
        Self::Amoled18V1 => (368, 448, 0), Self::Amoled18V2 => (368, 448, 16),
    } }
    fn i2c(self) -> (u8, u8) { if self.watch() { (15, 14) } else { (47, 48) } }
    fn spi(self) -> (u8, [u8; 4], u8) { if self.watch() { (11, [4, 5, 6, 7], 12) } else { (10, [11, 12, 13, 14], 9) } }
}

// The plain143 factory performs the one normal-mode write before FT reports.
// This does not broaden writes to other read-only report registers.
struct FtNormal { inner: Ft3168, first: bool, register: Option<u8>, written: bool }
impl I2cDevice for FtNormal {
    fn start(&mut self, read: bool) -> bool { if !read { self.first = true; self.register = None; self.written = false; } self.inner.start(read) }
    fn write(&mut self, value: u8) -> bool {
        if self.first { self.first = false; self.register = Some(value); return self.inner.write(value); }
        if self.register == Some(0) && value == 0 && !self.written { self.written = true; return true; }
        self.inner.write(value)
    }
    fn read(&mut self) -> u8 { self.inner.read() }
    fn stop(&mut self) { self.inner.stop(); }
}

#[derive(Default)]
struct Cst820State {
    active: bool, generation: u64, x: u16, y: u16, down: bool, irq_until: u64,
    reads: u64, contacts: u64, updates: u64, auto_sleep_disabled: bool,
}
impl Cst820State {
    fn hardware(&mut self, active: bool, reset: bool) {
        if active != self.active || reset {
            self.active = active; self.generation += 1; self.down = false;
            self.x = 0; self.y = 0; self.irq_until = 0; self.auto_sleep_disabled = false;
        }
    }
}
struct Cst820Port { state: Arc<Mutex<Cst820State>>, first: bool, pointer: Option<u8>, generation: u64 }
impl Cst820Port {
    fn sync(&mut self) -> bool {
        let s = self.state.lock().expect("compact CST820");
        if self.generation != s.generation { self.generation = s.generation; self.pointer = None; self.first = true; }
        s.active
    }
}
impl I2cDevice for Cst820Port {
    fn start(&mut self, read: bool) -> bool {
        if !self.sync() { return false; }
        if !read { self.first = true; self.pointer = None; true } else { self.pointer.is_some() }
    }
    fn write(&mut self, v: u8) -> bool {
        if !self.sync() { return false; }
        if self.first { self.first = false; self.pointer = matches!(v, 2..=6 | 0xfe).then_some(v); return self.pointer.is_some(); }
        if self.pointer == Some(0xfe) && v == 255 {
            self.state.lock().expect("compact CST820").auto_sleep_disabled = true;
            self.pointer = None; return true;
        }
        false
    }
    fn read(&mut self) -> u8 {
        if !self.sync() { return 255; }
        let Some(r) = self.pointer else { return 255; };
        let mut s = self.state.lock().expect("compact CST820");
        self.pointer = (r < 6).then_some(r.wrapping_add(1));
        match r {
            2 => { s.reads += 1; s.contacts += u64::from(s.down); u8::from(s.down) },
            3 => (s.x >> 8) as u8 & 15, 4 => s.x as u8,
            5 => (s.y >> 8) as u8 & 15, 6 => s.y as u8,
            0xfe if s.auto_sleep_disabled => 255,
            _ => 255,
        }
    }
}

struct Rail { on: bool, generation: u64 }
impl Default for Rail { fn default() -> Self { Self { on: true, generation: 0 } } }
struct RailPort { rail: Arc<Mutex<Rail>>, device: Box<dyn I2cDevice>, generation: u64, create: Option<fn() -> Box<dyn I2cDevice>> }
impl RailPort {
    fn on(&mut self) -> bool {
        let r = self.rail.lock().expect("compact VCC3V3");
        if r.generation != self.generation {
            if let Some(create) = self.create { self.device = create(); } else { self.device.stop(); }
            self.generation = r.generation;
        }
        r.on
    }
}
impl I2cDevice for RailPort {
    fn start(&mut self, read: bool) -> bool { self.on() && self.device.start(read) }
    fn write(&mut self, b: u8) -> bool { self.on() && self.device.write(b) }
    fn read(&mut self) -> u8 { if self.on() { self.device.read() } else { 255 } }
    fn stop(&mut self) { self.device.stop(); }
}

/// Only the source-used 03/00/DA/00 software-SPI read. SH8601's DA is an
/// OTP module maker code, not a universal silicon ID: 86 is the explicit
/// manufacturer branch profile. CO5300 OTP is unknown and stays floating FF.
#[derive(Default)]
struct IdRead { header: u32, bits: u8, read_bits: u8, clock: bool, selected: bool, sampled: bool, reads: u64 }
impl IdRead {
    fn drive(&mut self, selected: bool, reset: bool, clock: Option<bool>, data: Option<bool>, sh: bool) -> bool {
        if !selected || !reset || clock.is_none() {
            let reads = self.reads; *self = Self { reads, ..Default::default() }; return true;
        }
        let clk = clock.unwrap_or(false);
        if !self.selected { self.clock = clk; self.selected = true; }
        let rising = !self.clock && clk;
        if self.bits < 32 && rising {
            if let Some(d) = data { self.header = (self.header << 1) | u32::from(d); self.bits += 1; }
            else { self.bits = 33; }
        } else if self.bits == 32 && rising && self.sampled {
            self.read_bits = self.read_bits.saturating_add(1); self.sampled = false;
            if self.read_bits == 8 { self.reads += 1; }
        }
        self.clock = clk;
        let valid = sh && self.bits == 32 && self.header == 0x0300_da00 && self.read_bits < 8 && !clk && data.is_none();
        if valid { self.sampled = true; (0x86u8 & (0x80 >> self.read_bits)) != 0 } else { true }
    }
}

pub struct WaveshareCompact {
    variant: CompactVariant, panel: QspiDcsPanel, power: Arc<Mutex<Axp2101Power>>,
    rail: Arc<Mutex<Rail>>, io: Arc<Mutex<Tca9554State>>, rtc: Arc<Mutex<Pcf85063State>>,
    ft: Arc<Mutex<Ft3168State>>, cst: Arc<Mutex<Cst820State>>,
    motion: Arc<ImuMotion>, qmi: Rc<RefCell<Qmi8658>>, i2c: I2cBitBang, attached: bool,
    cycle: u64, reset: bool, reset_falls: [u64; 2], selected: bool,
    sd_present: bool, sd_dat3: bool, power_pressed: bool, id: IdRead,
    levels: [bool; 4], edges: Vec<BoardEdge>, rejected: u64,
}
impl WaveshareCompact {
    pub fn new(variant: CompactVariant) -> Self {
        let (w, h, gap) = variant.dimensions();
        let motion = Arc::new(ImuMotion::default());
        let mut b = Self { variant,
            panel: if variant.sh() { QspiDcsPanel::sh8601_bsp_rgb565(usize::from(w), usize::from(h), gap, 0) } else { QspiDcsPanel::co5300(usize::from(w), usize::from(h), gap, 0) },
            power: Default::default(), rail: Default::default(), io: Default::default(),
            rtc: Arc::new(Mutex::new(Pcf85063State::with_averaged_offset(HZ))),
            ft: Arc::new(Mutex::new(Ft3168State::new(HZ))), cst: Default::default(),
            qmi: Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(), HZ))), motion,
            i2c: I2cBitBang::new(), attached: false, cycle: 0, reset: variant.watch(),
            reset_falls: [0; 2], selected: false, sd_present: false, sd_dat3: true,
            power_pressed: false, id: IdRead::default(), levels: [true; 4], edges: Vec::new(), rejected: 0 };
        b.sync(); b
    }
    fn main_power(&self) -> bool { !self.variant.watch() || self.power.lock().expect("compact AXP").output_mv(PowerRail::Dcdc1) == Some(3300) }
    fn gated(&self, device: Box<dyn I2cDevice>, create: Option<fn() -> Box<dyn I2cDevice>>) -> Box<dyn I2cDevice> {
        Box::new(RailPort { rail: self.rail.clone(), device, generation: 0, create })
    }
    fn sync(&mut self) {
        let on = self.main_power();
        let changed = { let mut r = self.rail.lock().expect("compact VCC3V3");
            let changed = r.on != on; if changed { r.on = on; r.generation += 1; } changed };
        if changed {
            self.panel.reset(); *self.qmi.borrow_mut() = Qmi8658::new_c(self.motion.clone(), HZ);
            *self.io.lock().expect("compact TCA") = Tca9554State::default(); self.reset_falls = [0; 2];
        }
        let mut touch_reset = false;
        let touch_high = if self.variant.watch() {
            let mut io = self.io.lock().expect("compact TCA");
            // R11 and the PWRON-controlled N-FET invert the physical key: SYS_OUT
            // on P4 is LOW idle, HIGH pressed. No PMU timer is synthesized.
            let irq_high = !self.power.lock().expect("compact AXP").irq_asserted();
            io.set_inputs(0x30, if self.power_pressed { 0x10 } else { 0 } | if irq_high { 0x20 } else { 0 });
            let falls = [io.falling_edges(0), io.falling_edges(2)];
            if falls[0] != self.reset_falls[0] { self.panel.reset(); }
            touch_reset = falls[1] != self.reset_falls[1]; self.reset_falls = falls;
            let reset = on && io.high(0);
            if self.reset != reset { self.reset = reset; self.panel.changes += 1; }
            self.sd_dat3 = io.high(7); io.high(2)
        } else {
            // J9 exposes no touch reset or touch interrupt. OLED_EN's R9 is NC,
            // its real panel input remains pulled high by R2 independently of G42.
            true
        };
        self.ft.lock().expect("compact FT").hardware_at(self.cycle, on, touch_high && !touch_reset);
        if touch_reset { self.ft.lock().expect("compact FT").hardware_at(self.cycle, on, touch_high); }
        self.cst.lock().expect("compact CST").hardware(on && touch_high, touch_reset);
        if self.variant.watch() {
            let high = if self.variant.cst() { let s = self.cst.lock().expect("compact CST"); !s.active || self.cycle >= s.irq_until }
                else { self.ft.lock().expect("compact FT").irq_high() };
            self.level(2, 21, high);
        }
    }
    fn level(&mut self, i: usize, pin: u8, high: bool) {
        if self.levels[i] != high { self.levels[i] = high; self.edges.push(BoardEdge { cycle: self.cycle, pin, level: high }); }
    }
}
impl BoardModel for WaveshareCompact {
    fn name(&self) -> &'static str { self.variant.name() }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_waveform_at(&mut self, cycle: u64, r: PinRoutes<'_>) {
        self.advance_to(cycle);
        let (_, _, cs) = self.variant.spi();
        let selected = r.software_output(cs) == Some(false);
        if self.selected && !selected { self.panel.deselect(); } self.selected = selected;
        if !self.variant.watch() {
            let reset = r.software_output(21) == Some(true);
            if reset != self.reset { if !reset { self.panel.reset(); } self.reset = reset; self.panel.changes += 1; }
            self.sd_dat3 = r.software_output(38).unwrap_or(true);
            let high = self.id.drive(selected, reset, r.software_output(10), r.software_output(11), self.variant.sh());
            self.level(3, 11, high);
        }
        let (sda_pin, scl_pin) = self.variant.i2c();
        let scl = r.software_output(scl_pin).unwrap_or(true);
        let sda = self.i2c.drive(scl, r.software_output(sda_pin).unwrap_or(true));
        self.level(0, sda_pin, sda); self.level(1, scl_pin, scl); self.sync();
    }
    fn uses_qspi_pins(&self) -> bool { true }
    fn spi_transfer_qspi_pins(&mut self, _host: u8, p: SpiQspiPins, tx: &[u8], rx: usize) -> Vec<u8> {
        self.sync(); let (clock, lanes, cs) = self.variant.spi();
        if p.cs & (1 << cs) == 0 { return vec![255; rx]; }
        let quad = [p.command_width, p.address_width, p.data_width].contains(&4);
        let routed = p.sclk & (1 << clock) != 0 && p.data[0] & (1 << lanes[0]) != 0
            && (!quad || (1..4).all(|i| p.data[i] & (1 << lanes[i]) != 0));
        if !routed || !self.main_power() || !self.reset || rx != 0 { self.panel.reject_until_deselect(); self.rejected += 1; }
        else if !self.panel.transfer(p, tx) { self.rejected += 1; }
        if !p.keep_cs && p.software_cs & (1 << cs) == 0 { self.panel.deselect(); }
        vec![255; rx]
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if !self.attached {
            let touch: Box<dyn I2cDevice> = if self.variant.cst() {
                Box::new(Cst820Port { state: self.cst.clone(), first: true, pointer: None, generation: 0 })
            } else { Box::new(FtNormal { inner: Ft3168::new(self.ft.clone()), first: true, register: None, written: false }) };
            self.i2c.attach(if self.variant.cst() { 0x15 } else { 0x38 }, touch);
            self.i2c.attach(0x6b, self.gated(Box::new(Qmi8658Port::new(self.qmi.clone())), None));
            self.i2c.attach(0x51, Box::new(Pcf85063::new(self.rtc.clone())));
            if self.variant.watch() {
                self.i2c.attach(0x20, self.gated(Box::new(Tca9554::new(self.io.clone())), None));
                self.i2c.attach(0x34, Box::new(Axp2101::new(self.power.clone())));
                self.i2c.attach(0x18, self.gated(Box::new(Es8311::new()), Some(|| Box::new(Es8311::new()))));
            }
            self.attached = true;
        }
        let pins = self.variant.i2c();
        (0..2).flat_map(|bus| self.i2c.controller_devices(Some(pins)).into_iter().map(move |(a, d)| (bus, a, d))).collect()
    }
    fn advance_to(&mut self, c: u64) {
        assert!(c >= self.cycle, "compact board time moved backwards"); self.cycle = c;
        self.motion.cycle.store(c, Ordering::Relaxed); self.rtc.lock().expect("compact RTC").advance_to(c); self.sync();
    }
    fn next_deadline(&self) -> Option<u64> {
        if self.variant.cst() { let s = self.cst.lock().expect("compact CST"); (s.active && self.cycle < s.irq_until).then_some(s.irq_until) }
        else { self.ft.lock().expect("compact FT").next_deadline() }
    }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        self.sync(); let (w, h, _) = self.variant.dimensions(); let (x, y) = (x.min(w - 1), y.min(h - 1));
        if self.variant.cst() {
            let mut s = self.cst.lock().expect("compact CST");
            if s.active {
                if (s.x, s.y, s.down) != (x, y, down) { s.irq_until = self.cycle.saturating_add(HZ / 1000); }
                s.x = x; s.y = y; s.down = down; s.updates += 1;
            }
        } else { self.ft.lock().expect("compact FT").touch_at(self.cycle, x, y, down); }
        self.sync();
    }
    fn touch_at(&mut self, c: u64, x: u16, y: u16, down: bool) { self.advance_to(c); self.touch(x, y, down); }
    fn button_at(&mut self, c: u64, id: &str, pressed: bool) -> bool {
        if !self.variant.watch() || id != "power" { return false; }
        self.advance_to(c); self.power_pressed = pressed; self.sync(); true
    }
    fn input_levels(&self) -> Vec<(u8, bool)> {
        let (sda, scl) = self.variant.i2c(); let mut v = vec![(0, true), (sda, self.levels[0]), (scl, self.levels[1])];
        if self.variant.watch() { v.push((21, self.levels[2])); } else { v.push((11, self.levels[3])); } v
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn named_pin(&self, n: &str) -> Option<u8> { (n == "boot").then_some(0) }
    fn set_imu_motion(&mut self, m: u32) -> bool { self.qmi.borrow_mut().set_motion_mode(m) }
    fn set_imu_sample(&mut self, a: [f64; 3], g: [f64; 3]) -> bool { self.qmi.borrow_mut().set_host_sample(a, g) }
    fn imu_motion(&self) -> u32 { self.motion.mode.load(Ordering::Relaxed) }
    fn sd_card_slot(&self) -> Option<([u8; 6], bool)> {
        let pads = if self.variant.watch() { [2, 1, 3, 255, 255, 255] } else { [41, 39, 40, 255, 255, 255] };
        Some((pads, self.sd_present && self.main_power() && self.sd_dat3))
    }
    fn set_sd_card_present(&mut self, present: bool) { self.sd_present = present; }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let (w, h, _) = self.variant.dimensions();
        let mut frame = if self.main_power() && self.reset { self.panel.frame() } else { vec![0; usize::from(w) * usize::from(h)] };
        if !self.variant.watch() {
            // Nominal circular aperture, not metrology of peripheral glass pixels.
            for y in 0..466 { for x in 0..466 { let dx = 2 * x as i64 + 1 - 466; let dy = 2 * y as i64 + 1 - 466;
                if dx * dx + dy * dy > 466 * 466 { frame[y * 466 + x] = 0; }
            } }
        }
        Some((u32::from(w), u32::from(h), frame, self.display_version()))
    }
    fn display_version(&self) -> u64 { self.panel.version() }
    fn display_frames(&self) -> u64 { self.panel.frames() }
    fn report(&self) -> String {
        let (w, h, gap) = self.variant.dimensions(); let ft = self.ft.lock().expect("compact FT"); let cst = self.cst.lock().expect("compact CST");
        format!("{} {} {}x{} gap{} refMADCTL0 QSPI{:?} reset={} pixels={} frames={} reject={} state={:?}; I2C{:?} {} reports={} contacts={} QMI8658C@6B PCF85063A@51. SD native1bit {:?} present={} DAT3={}; AXP/TCA/ES8311={} VCC3V3={} SYS_OUThighPressed={} bitbangSHmaker86reads={}. {} No physical OTP/EFUSE capture, CO ID, touch firmware IDs/gestures/cadence, radio, analog audio/charging, PMU watchdog/button-cell/DC5 engine, CPU poweroff, QMIIRQ/calibration, RTC alarm, TE or measured glass calibration; simulation remains partial.",
            self.name(), self.panel.controller(), w, h, gap, self.variant.spi(), self.reset,
            self.panel.pixels_written(), self.panel.frames(), self.rejected, self.panel.state(), self.variant.i2c(),
            if self.variant.cst() { "CST820@15" } else { "FT3168@38" },
            if self.variant.cst() { cst.reads } else { ft.reports }, if self.variant.cst() { cst.contacts } else { ft.contact_reports },
            self.sd_card_slot().map(|v| v.0), self.sd_present, self.sd_dat3, self.variant.watch(), self.main_power(), self.power_pressed, self.id.reads,
            if self.variant.watch() { "TCA P0 LCDreset/P2 touchreset/P7 DAT3; P1 DSI power is not glass supply. AXP Table6-1 idealUSB/no-cell startup." }
            else { "OLED_EN42 disconnected by R9NC/R2pullup; touch no reset/IRQ net. SH86 is manufacturer-selected OTP profile, not a generic die ID." })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d: &mut dyn I2cDevice, values: &[u8]) { assert!(d.start(false)); for &v in values { assert!(d.write(v), "write {values:02x?}"); } d.stop(); }
    fn read(d: &mut dyn I2cDevice, r: u8, n: usize) -> Vec<u8> { assert!(d.start(false)); assert!(d.write(r)); assert!(d.start(true)); (0..n).map(|_| d.read()).collect() }
    fn route(b: &mut WaveshareCompact, values: &[(u8, Option<bool>)]) {
        let mut g = esp_periph::Gpio::new(); let mut m = esp_periph::RegRam::new();
        for &(p, v) in values { if let Some(high) = v { g.enable |= 1 << p; if high { g.out |= 1 << p; } } m.write(4 + 4 * u32::from(p), 1 << 12); }
        b.gpio_waveform_at(b.cycle, esp_soc::pins::ChipPins::S3.routes(&g, &m));
    }
    fn packet(b: &WaveshareCompact, c: u8, data: &[u8]) -> (SpiQspiPins, Vec<u8>) {
        let (clock, lanes, cs) = b.variant.spi(); let quad = c == 0x2c;
        let mut v = vec![if quad { 0x32 } else { 2 }, 0, c, 0]; v.extend(data);
        (SpiQspiPins { sclk: 1 << clock, data: lanes.map(|p| 1 << p), cs: 1 << cs,
            command_width: 1, address_width: 1, data_width: if data.is_empty() { 0 } else if quad { 4 } else { 1 },
            command_bits: 8, address_bits: 24, data_bits: (8 * data.len()) as u32, ..Default::default() }, v)
    }
    fn cmd(b: &mut WaveshareCompact, c: u8, data: &[u8]) { let (p, v) = packet(b, c, data); b.spi_transfer_qspi_pins(2, p, &v, 0); }
    fn pixel(b: &mut WaveshareCompact) {
        let (_, _, gap) = b.variant.dimensions(); let x = gap + 200;
        for c in [0x11, 0x29] { cmd(b, c, &[]); } cmd(b, 0x3a, &[0x55]); cmd(b, 0x51, &[255]);
        cmd(b, 0x2a, &[(x >> 8) as u8, x as u8, (x >> 8) as u8, x as u8]);
        cmd(b, 0x2b, &[0, 200, 0, 200]); cmd(b, 0x2c, &[0xf8, 0]);
    }
    #[test]
    fn four_profiles_keep_real_gram_gaps_resets_and_wrong_lanes_distinct() {
        for v in [CompactVariant::Amoled143Sh8601, CompactVariant::Amoled143Co5300, CompactVariant::Amoled18V1, CompactVariant::Amoled18V2] {
            let mut b = WaveshareCompact::new(v); if !v.watch() { route(&mut b, &[(21, Some(true)), (42, Some(false))]); }
            pixel(&mut b); let (w, _, _) = v.dimensions(); assert_eq!(b.display().unwrap().2[200 * usize::from(w) + 200], 0xf800);
            let count = b.panel.pixels_written(); let (mut p, data) = packet(&b, 0x2c, &[0, 0]); p.data[2] = 1 << 19;
            b.spi_transfer_qspi_pins(2, p, &data, 0); assert_eq!(b.panel.pixels_written(), count);
            if v.watch() {
                let mut io = Tca9554::new(b.io.clone()); write(&mut io, &[3, 0xf8]); write(&mut io, &[1, 0xfd]); b.advance_to(1);
                assert_eq!(b.display().unwrap().2[200 * usize::from(w) + 200], 0xf800, "P1 DSI supply is not panel power");
                write(&mut io, &[1, 0xfc]); b.advance_to(2); assert!(b.display().unwrap().2.iter().all(|p| *p == 0));
            } else { route(&mut b, &[(21, Some(false)), (42, Some(true))]); assert!(b.display().unwrap().2.iter().all(|p| *p == 0)); }
        }
    }
    #[test]
    fn watch_button_pmu_rail_touch_address_and_native_sd_follow_schematic() {
        for v in [CompactVariant::Amoled18V1, CompactVariant::Amoled18V2] {
            let mut b = WaveshareCompact::new(v); b.advance_to(HZ / 10); b.set_sd_card_present(true);
            assert_eq!(b.sd_card_slot(), Some(([2, 1, 3, 255, 255, 255], true)));
            let mut devices = b.i2c_devices(); let a = if v.cst() { 0x15 } else { 0x38 };
            assert!(devices.iter().all(|(_, address, _)| *address != if v.cst() { 0x38 } else { 0x15 }));
            let tp = devices.iter_mut().find(|(bus, address, _)| *bus == 0 && *address == a).unwrap().2.as_mut();
            assert_eq!(tp.pins(), Some((15, 14))); b.touch(123, 321, true); assert_eq!(read(tp, 2, 5), [1, 0, 123, 1, 65]);
            assert!(tp.start(false)); assert!(!tp.write(0xa7), "unknown touch identity stays unavailable");
            let mut io = Tca9554::new(b.io.clone()); assert_eq!(read(&mut io, 0, 1)[0] & 0x10, 0);
            assert!(b.button_at(b.cycle, "power", true)); assert_eq!(read(&mut io, 0, 1)[0] & 0x10, 0x10);
            assert!(b.button_at(b.cycle, "power", false)); assert_eq!(read(&mut io, 0, 1)[0] & 0x10, 0);
            write(&mut io, &[3, 0x7f]); write(&mut io, &[1, 0x7f]); b.advance_to(b.cycle + 1); assert!(!b.sd_card_slot().unwrap().1);
            let mut pmu = Axp2101::new(b.power.clone()); write(&mut pmu, &[0x80, 0x0e]); b.advance_to(b.cycle + 1);
            assert!(!tp.start(false)); assert!(!b.sd_card_slot().unwrap().1);
            write(&mut pmu, &[0x80, 0x0f]); b.advance_to(b.cycle + 1);
            assert!(tp.start(false)); assert_eq!(read(&mut io, 3, 1), [255], "rail cycling resets the physical expander");
        }
    }
    fn read_id(b: &mut WaveshareCompact, header: u32, cs: u8) -> u8 {
        route(b, &[(21, Some(true)), (cs, Some(false)), (10, Some(false)), (11, Some(false))]);
        for i in (0..32).rev() { let value = header & (1 << i) != 0;
            route(b, &[(21, Some(true)), (cs, Some(false)), (10, Some(false)), (11, Some(value))]);
            route(b, &[(21, Some(true)), (cs, Some(false)), (10, Some(true)), (11, Some(value))]);
        }
        let mut id = 0;
        for _ in 0..8 {
            route(b, &[(21, Some(true)), (cs, Some(false)), (10, Some(false)), (11, None)]);
            id = (id << 1) | u8::from(b.input_levels().iter().find(|(p, _)| *p == 11).unwrap().1);
            route(b, &[(21, Some(true)), (cs, Some(false)), (10, Some(false)), (11, Some(false))]);
            route(b, &[(21, Some(true)), (cs, Some(false)), (10, Some(true)), (11, Some(false))]);
        }
        route(b, &[(21, Some(true)), (cs, Some(true))]); id
    }
    #[test]
    fn plain143_otp_profile_requires_exact_gpio_read_header_and_preserves_unknown_co_id() {
        let mut sh = WaveshareCompact::new(CompactVariant::Amoled143Sh8601);
        assert_eq!(read_id(&mut sh, 0x0300_da00, 9), 0x86);
        assert_eq!(sh.id.reads, 1); assert_eq!(read_id(&mut sh, 0x0300_db00, 9), 255);
        assert_eq!(read_id(&mut sh, 0x0300_da00, 8), 255);
        let mut co = WaveshareCompact::new(CompactVariant::Amoled143Co5300);
        assert_eq!(read_id(&mut co, 0x0300_da00, 9), 255); assert_eq!(co.id.reads, 0);
        co.set_sd_card_present(true); assert_eq!(co.sd_card_slot(), Some(([41, 39, 40, 255, 255, 255], true)));
        route(&mut co, &[(21, Some(true)), (38, Some(false))]); assert!(!co.sd_card_slot().unwrap().1);
        assert!(!co.button_at(0, "power", true));
    }
}
