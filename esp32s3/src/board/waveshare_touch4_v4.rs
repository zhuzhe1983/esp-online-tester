//! Exact ESP32-S3-(Touch-)LCD-4 V4.0, not the older TCA9554 or 4B board.
//! Pinned V4 schematic / BSP3.0.0 / CH32 driver1.0.1 are retained in
//! docs/platform/research/waveshare-touch4-v4-runtime-sources/manifest.json.
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex}};
use esp_periph::i2c::{I2cDevice, gt911::{Gt911, Gt911State}};
use esp_soc::{board::{Board, BoardEdge, BoardModel, LcdRgbPins, SpiQspiPins}, pins::PinRoutes,
    devices::{i2c_bitbang::I2cBitBang, pcf85063::{Pcf85063, Pcf85063State},
        waveshare_touch_parts::{Ch32Port, Ch32V003}}};
const HZ: u64 = 240_000_000;
const RGB: [u8; 16] = [5, 45, 48, 47, 21, 14, 13, 12, 11, 10, 9, 46, 3, 8, 18, 17];
const SIZE: usize = 480 * 480;

pub fn make_board(name: &str) -> Option<Board> {
    match name {
        "waveshare-s3-touch-lcd4-v4" => Some(Box::new(Touch4V4::new(true))),
        "waveshare-s3-lcd4-v4" => Some(Box::new(Touch4V4::new(false))),
        _ => None,
    }
}

struct Touch {
    populated: bool, io: Arc<Mutex<Ch32Port>>, state: Arc<Mutex<Gt911State>>, chip: Gt911,
    enabled: bool, falls: u64, address: u8, cycle: u64, next_scan: u64,
    pointer: u16, phase: u8, valid: bool, reads: u64, contacts: u64, updates: u64,
}
impl Touch {
    fn new(populated: bool, io: Arc<Mutex<Ch32Port>>) -> Self {
        let state = Arc::new(Mutex::new(Gt911State::new(480, 480, HZ / 100)));
        state.lock().expect("V4 GT911").powered = false;
        Self { populated, io, chip: Gt911::new(state.clone()), state, enabled: false,
            falls: 0, address: 0x5d, cycle: 0, next_scan: 0, pointer: 0, phase: 0,
            valid: false, reads: 0, contacts: 0, updates: 0 }
    }
    fn sync_reset(&mut self) {
        let (falls, high, strap_low) = {
            let io = self.io.lock().expect("V4 CH32");
            (io.falling_edges(1), io.high(1), io.direction & 4 != 0 && !io.high(2))
        };
        if falls != self.falls {
            self.falls = falls; self.enabled = false; self.valid = false; self.phase = 0;
            *self.state.lock().expect("V4 GT911") = Gt911State::new(480, 480, HZ / 100);
            self.chip = Gt911::new(self.state.clone()); self.next_scan = self.cycle;
        }
        let enabled = high && self.populated;
        if enabled && !self.enabled {
            // Goodix Rev09: INT driven low at reset release selects 5Dh;
            // high/released selects 14h. V4 routes INT through CH32 EX2.
            self.address = if strap_low { 0x5d } else { 0x14 };
        }
        self.enabled = enabled;
        self.state.lock().expect("V4 GT911").powered = enabled;
    }
    fn sync_inputs(&self) {
        let irq_high = !self.enabled || !self.state.lock().expect("V4 GT911").ready;
        let mut io = self.io.lock().expect("V4 CH32");
        // EX7 is inactive for the modeled calendar-only RTC. Alarm/timer IRQ
        // is not implemented. EX2 is read as an input only when dir2=0;
        // an output left low by the BSP is not silently changed to an input.
        io.inputs = (io.inputs & !0x84) | 0x80 | if irq_high { 4 } else { 0 };
    }
    fn advance_to(&mut self, cycle: u64) {
        self.cycle = cycle; self.sync_reset();
        self.state.lock().expect("V4 GT911").advance_to(cycle); self.sync_inputs();
    }
    fn start(&mut self, at: u8, read: bool) -> bool {
        self.sync_reset();
        if !self.enabled || self.address != at || (read && !self.valid) { return false; }
        if !read { self.phase = 0; self.valid = false; }
        self.chip.start(read)
    }
    fn write(&mut self, value: u8) -> bool {
        self.sync_reset(); if !self.enabled { return false; }
        match self.phase {
            0 => { self.pointer = u16::from(value) << 8; self.phase = 1; return self.chip.write(value); }
            1 => {
                self.pointer |= u16::from(value); self.phase = 2;
                self.valid = matches!(self.pointer, 0x8047..=0x8100 | 0x8140..=0x8143 | 0x8146..=0x8149 | 0x814e..=0x8157);
                return self.valid && self.chip.write(value);
            }
            _ => {}
        }
        if !self.valid || (!(0x8047..=0x8100).contains(&self.pointer) && !(self.pointer == 0x814e && value == 0)) { return false; }
        if self.pointer == 0x814e { self.next_scan = self.cycle.saturating_add(HZ / 100); }
        self.pointer = self.pointer.wrapping_add(1);
        let okay = self.chip.write(value); self.sync_inputs(); okay
    }
    fn read(&mut self) -> u8 {
        self.sync_reset(); if !self.enabled || !self.valid { return 255; }
        let at = self.pointer; self.pointer = self.pointer.wrapping_add(1);
        let value = self.chip.read();
        match at {
            0x8146 | 0x8148 => 0xe0, 0x8147 | 0x8149 => 1,
            0x814e => { self.reads += 1; self.contacts += u64::from(value & 0x8f == 0x81); value }
            0x8047..=0x8100 | 0x8140..=0x8143 | 0x814f..=0x8157 => value,
            _ => 255,
        }
    }
}
struct TouchPort { touch: Rc<RefCell<Touch>>, address: u8 }
impl I2cDevice for TouchPort {
    fn start(&mut self, read: bool) -> bool { self.touch.borrow_mut().start(self.address, read) }
    fn write(&mut self, value: u8) -> bool { self.touch.borrow_mut().write(value) }
    fn read(&mut self) -> u8 { self.touch.borrow_mut().read() }
    fn stop(&mut self) { self.touch.borrow_mut().chip.stop(); }
}
struct ExpanderPort { inner: Ch32V003, touch: Rc<RefCell<Touch>> }
impl I2cDevice for ExpanderPort {
    fn start(&mut self, read: bool) -> bool { self.inner.start(read) }
    fn write(&mut self, value: u8) -> bool {
        let okay = self.inner.write(value);
        if okay { let mut touch = self.touch.borrow_mut(); touch.sync_reset(); touch.sync_inputs(); }
        okay
    }
    fn read(&mut self) -> u8 { self.touch.borrow().sync_inputs(); self.inner.read() }
    fn stop(&mut self) { self.inner.stop(); }
}

struct Touch4V4 {
    populated: bool, io: Arc<Mutex<Ch32Port>>, touch: Rc<RefCell<Touch>>, rtc: Arc<Mutex<Pcf85063State>>,
    i2c: I2cBitBang, attached: bool, cycle: u64, levels: [bool; 2], edges: Vec<BoardEdge>, gpio_events: u64,
    frame: Vec<u16>, frames: u64, version: u64, map: Option<[u8; 16]>,
    reset_falls: u64, reset_high: bool, light: u8, clock: Option<bool>, shift: u16, bits: u8,
    page: u8, command: u8, args: [u8; 5], count: usize,
    sleeping: bool, on: bool, inverted: bool, madctl: u8, colmod: u8,
    words: u64, vendor_bytes: u64, rejected_spi: u64, partial_words: u64, rejected_rgb: u64, card: bool,
}
impl Touch4V4 {
    fn new(populated: bool) -> Self {
        let io = Arc::new(Mutex::new(Ch32Port::new()));
        Self { populated, touch: Rc::new(RefCell::new(Touch::new(populated, io.clone()))), io,
            rtc: Arc::new(Mutex::new(Pcf85063State::new(HZ))), i2c: I2cBitBang::new(), attached: false,
            cycle: 0, levels: [true; 2], edges: Vec::new(), gpio_events: 0,
            frame: vec![0; SIZE], frames: 0, version: 0, map: None, reset_falls: 0, reset_high: false,
            light: 0, clock: None, shift: 0, bits: 0, page: 0, command: 0, args: [0; 5], count: 0,
            sleeping: true, on: false, inverted: false, madctl: 0, colmod: 0x70,
            words: 0, vendor_bytes: 0, rejected_spi: 0, partial_words: 0, rejected_rgb: 0, card: false }
    }
    fn panel_reset(&mut self) {
        self.frame.fill(0); self.page = 0; self.command = 0; self.count = 0; self.sleeping = true;
        self.on = false; self.inverted = false; self.madctl = 0; self.colmod = 0x70;
        self.clock = None; self.shift = 0; self.bits = 0; self.version += 1;
    }
    fn sync_io(&mut self) {
        let (falls, high, light) = {
            let io = self.io.lock().expect("V4 CH32");
            // BSP3.0.0 brightness_set inverts the 8-bit PWM byte. V4's R40 /
            // R36 feed the AP3032 FB node, not its CTRL enable. Linear display
            // scaling is an explicit digital policy, not measured LED current.
            (io.falling_edges(3), io.high(3), io.pwm.map_or(0, |d| 255 - d))
        };
        if falls != self.reset_falls { self.reset_falls = falls; self.panel_reset(); }
        if (self.reset_high, self.light) != (high, light) { self.reset_high = high; self.light = light; self.version += 1; }
    }
    fn word(&mut self, data: bool, value: u8) {
        self.words += 1;
        if !data {
            self.command = value; self.count = 0;
            if self.page == 0 {
                match value { 1 => self.panel_reset(), 0x10 => self.sleeping = true,
                    0x11 => self.sleeping = false, 0x20 => self.inverted = false,
                    0x21 => self.inverted = true, 0x28 => self.on = false, 0x29 => self.on = true, _ => {} }
            }
        } else if self.command == 0xff {
            if self.count < 5 { self.args[self.count] = value; }
            self.count = self.count.saturating_add(1);
            if self.count == 5 && self.args[..4] == [0x77, 1, 0, 0] { self.page = self.args[4]; }
        } else {
            if self.page == 0 && self.count == 0 {
                match self.command { 0x36 => self.madctl = value, 0x3a => self.colmod = value & 0x70, _ => {} }
            } else if self.page != 0 { self.vendor_bytes += 1; }
            self.count = self.count.saturating_add(1);
        }
        self.version += 1;
    }
    fn wire(&mut self, cs: Option<bool>, clock: Option<bool>, data: Option<bool>) {
        if !self.reset_high || cs != Some(false) || clock.is_none() || data.is_none() {
            if self.bits != 0 { self.partial_words += 1; }
            self.bits = 0; self.shift = 0;
        } else if self.clock == Some(false) && clock == Some(true) {
            self.shift = (self.shift << 1) | u16::from(data == Some(true)); self.bits += 1;
            if self.bits == 9 {
                let word = self.shift; self.bits = 0; self.shift = 0; self.word(word & 0x100 != 0, word as u8);
            }
        }
        self.clock = clock;
    }
    fn mapping(&mut self, controls: bool, bits: [Option<u8>; 16]) {
        let map = (controls && bits.iter().all(Option::is_some)).then(|| bits.map(|b| b.unwrap_or(0)));
        if map != self.map { self.map = map; self.version += 1; }
    }
    fn visible(&self) -> bool {
        let io = self.io.lock().expect("V4 CH32");
        io.high(3) && io.falling_edges(3) == self.reset_falls && io.pwm.is_some_and(|p| p < 255)
            && self.map.is_some() && self.on && !self.sleeping && matches!(self.colmod, 0x50 | 0x60)
            && self.madctl & !0x18 == 0
    }
}
impl BoardModel for Touch4V4 {
    fn name(&self) -> &'static str { if self.populated { "waveshare-s3-touch-lcd4-v4" } else { "waveshare-s3-lcd4-v4" } }
    fn uses_gpio_routes(&self) -> bool { true }
    fn uses_gpio_waveform(&self) -> bool { true }
    fn gpio_changes(&mut self, c: &[(u8, bool)]) { self.gpio_events += c.len() as u64; }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn gpio_waveform_at(&mut self, cycle: u64, routes: PinRoutes<'_>) {
        self.advance_to(cycle);
        // ESP-IDF5.4 gpio_sig_map.h: LCD data133..148, DE150, HS151,
        // VS152, PCLK154. S3 delivers this route callback, not lcd_rgb_pins.
        let controls = routes.matrix_output(41, 154) && routes.matrix_output(38, 151)
            && routes.matrix_output(39, 152) && routes.matrix_output(40, 150);
        self.mapping(controls, RGB.map(|p| (0..16).find(|i| routes.matrix_output(usize::from(p), 133 + u32::from(*i)))));
        self.wire(routes.software_output(42), routes.software_output(2), routes.software_output(1));
        let scl = routes.software_output(7).unwrap_or(true);
        let sda = self.i2c.drive(scl, routes.software_output(15).unwrap_or(true));
        for (i, (pin, level)) in [(15, sda), (7, scl)].into_iter().enumerate() {
            if self.levels[i] != level { self.levels[i] = level; self.edges.push(BoardEdge { cycle, pin, level }); }
        }
        self.sync_io(); self.touch.borrow_mut().advance_to(cycle);
    }
    fn lcd_rgb_pins(&mut self, p: LcdRgbPins) {
        let controls = p.width == 16 && p.clock & (1 << 41) != 0 && p.hsync & (1 << 38) != 0
            && p.vsync & (1 << 39) != 0 && p.enable & (1 << 40) != 0;
        self.mapping(controls, RGB.map(|pin| (0..16).find(|i| p.data[usize::from(*i)] & (1 << pin) != 0)));
    }
    fn uses_qspi_pins(&self) -> bool { true }
    fn spi_transfer_qspi_pins(&mut self, _host: u8, pins: SpiQspiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        self.sync_io();
        let valid = self.reset_high && pins.cs & (1 << 42) != 0 && pins.sclk & (1 << 2) != 0
            && pins.data[0] & (1 << 1) != 0 && !pins.unsupported_mode && !pins.sio
            && !pins.clock_idle_high && !pins.clock_output_edge && pins.dummy_cycles == 0
            && pins.command_width == 1 && pins.command_bits == 1 && pins.address_width == 1 && pins.address_bits == 8
            && pins.data_bits == 0 && (pins.read_bits == 0 || pins.full_duplex)
            && pins.data_width <= 1 && pins.read_width <= 1 && tx.len() == 2;
        if valid {
            // IDF spi_ll.h: one-bit COMMAND_VALUE is emitted from bit7.
            // Concurrent unconnected MISO sampling does not suppress TX.
            self.word(tx[0] & 0x80 != 0, tx[1]);
        } else { self.rejected_spi += 1; }
        vec![255; rx_len]
    }
    fn lcd_frame(&mut self, w: u32, h: u32, data: &[u8]) {
        self.sync_io();
        let Some(map) = self.map.filter(|_| self.reset_high && (w, h) == (480, 480) && data.len() == SIZE * 2) else {
            self.rejected_rgb += 1; return;
        };
        for (out, raw) in self.frame.iter_mut().zip(data.as_chunks::<2>().0) {
            let value = u16::from_le_bytes(*raw);
            *out = map.iter().enumerate().fold(0, |v, (bit, input)| v | (((value >> input) & 1) << bit));
        }
        self.frames += 1; self.version += 1;
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if !self.attached {
            self.i2c.attach(0x24, Box::new(ExpanderPort { inner: Ch32V003::new(self.io.clone()), touch: self.touch.clone() }));
            self.i2c.attach(0x51, Box::new(Pcf85063::new(self.rtc.clone())));
            if self.populated { for address in [0x5d, 0x14] {
                self.i2c.attach(address, Box::new(TouchPort { touch: self.touch.clone(), address }));
            }}
            self.attached = true;
        }
        (0..2).flat_map(|bus| self.i2c.controller_devices(Some((15, 7))).into_iter().map(move |(a, d)| (bus, a, d))).collect()
    }
    fn advance_to(&mut self, c: u64) {
        self.cycle = c; self.sync_io(); self.rtc.lock().expect("V4 RTC").advance_to(c);
        self.touch.borrow_mut().advance_to(c);
    }
    fn next_deadline(&self) -> Option<u64> {
        let t = self.touch.borrow();
        (t.enabled && !t.state.lock().expect("V4 GT911").ready && t.next_scan > self.cycle).then_some(t.next_scan)
    }
    fn touch_at(&mut self, c: u64, x: u16, y: u16, down: bool) { self.advance_to(c); self.touch(x, y, down); }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        let mut t = self.touch.borrow_mut(); t.sync_reset();
        if t.enabled { t.updates += 1; t.state.lock().expect("V4 GT911").update(x, y, down); }
        t.sync_inputs();
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(0, true), (15, self.levels[0]), (7, self.levels[1])] }
    fn named_pin(&self, name: &str) -> Option<u8> { (name == "boot" || name == "button").then_some(0) }
    fn sd_card_slot(&self) -> Option<([u8; 6], bool)> {
        // DAT3/SDCS is CH32 EX4, not a fourth routed MCU data lane. Only the
        // explicitly selected native one-bit path is supported. EX4 low is
        // not interpreted as removal of card power or as an SPI implementation.
        Some(([2, 1, 4, 255, 255, 255], self.card && self.io.lock().expect("V4 CH32").high(4)))
    }
    fn set_sd_card_present(&mut self, present: bool) { self.card = present; }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let mut out = vec![0; SIZE];
        if self.visible() {
            let light = u32::from(self.io.lock().expect("V4 CH32").pwm.map_or(0, |d| 255 - d));
            for y in 0..480 { for x in 0..480 {
                let row = if self.madctl & 0x10 != 0 { 479 - y } else { y };
                let mut value = self.frame[row * 480 + x];
                if self.madctl & 8 != 0 { value = (value & 0x07e0) | ((value & 0xf800) >> 11) | ((value & 31) << 11); }
                if self.inverted { value ^= 65535; }
                let p = u32::from(value);
                out[y * 480 + x] = ((((p >> 11) & 31) * light / 255) << 11
                    | (((p >> 5) & 63) * light / 255) << 5 | ((p & 31) * light / 255)) as u16;
            }}
        }
        Some((480, 480, out, self.version))
    }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { Some((self.frame.clone(), 480, 480)) }
    fn display_frames(&self) -> u64 { self.frames }
    fn display_version(&self) -> u64 { self.version }
    fn report(&self) -> String {
        let t = self.touch.borrow(); let io = self.io.lock().expect("V4 CH32");
        format!("{}: exact V4 16MiB Flash/8MiB PSRAM; ST7701S square480x480 RGB16 actual GPIO matrix mapping={:?}, LCD CS42/SCK2/MOSI1 nine-bit control, CH32@24 SDA15/SCL7 dir={:02x} out={:02x} levels={:02x} pwm={:?} light={}/255; LCD EX3 reset={} bank={:02x} COLMOD={:02x} MADCTL={:02x} on={} sleep={} words={} vendorBytes={} frames={} rejectedRGB={} rejectedSPI={} partialWords={}; GT911 populated={} EX1reset={} EX2IRQ/address={:02x} reports={} contacts={} injections={}; PCF85063@51 guest-cycle calendar, RTC EX7 inactive; TF native1bit2/1/4 inserted={} EX4select={}. Digital brightness follows BSP3 inverse PWM and V4 FB topology; contradictory ioexpander README retained. COLMOD50/60 display is normalized wired RGB565, not calibrated18-bit glass. BOOT0 only; GPIO16 is SYS_OUT, not IRQ/key. No analog supply/SYS_EN latch/battery ADC, RTC alarm/timer IRQ, buzzer audio, RS485/CAN/USB devices, IMU/codec models, serial readback or full factory/hardware acceptance.",
            self.name(), self.map, io.direction, io.output, io.levels(), io.pwm, self.light,
            self.reset_high, self.page, self.colmod, self.madctl, self.on, self.sleeping, self.words, self.vendor_bytes,
            self.frames, self.rejected_rgb, self.rejected_spi, self.partial_words,
            self.populated, t.enabled, t.address, t.reads, t.contacts, t.updates, self.card, io.high(4))
    }
}
