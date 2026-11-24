//! Exact Waveshare TCA9554 / ST7701S RGB board profiles. Sources and limits are
//! retained in docs/platform/waveshare-tca-rgb-runtime.md. No legacy LCD alias.
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, atomic::Ordering}};
use esp_periph::{i2c::{I2cDevice, gt911::{Gt911, Gt911State}},
    sdmmc::{SdCardConfig, SdStore, card::spi::SpiSdCard}};
use esp_soc::{board::{BoardModel, BoardEdge, LcdRgbPins, SpiQspiPins, VirtualCycle},
    pins::PinRoutes, devices::{i2c_bitbang::I2cBitBang, imu_motion::ImuMotion,
        qmi8658::{Qmi8658, Qmi8658Port}, tca9554::{Tca9554, Tca9554State},
        pcf85063::{Pcf85063, Pcf85063State}}};

const HZ: u64 = 240_000_000;
const I2C_PADS: (u8, u8) = (15, 7);
const RGB_PADS: [u8; 16] = [5, 45, 48, 47, 21, 14, 13, 12, 11, 10, 9, 46, 3, 8, 18, 17];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcaRgbVariant { TouchLcd21, TouchLcd21B, Lcd28B, TouchLcd28B, Lcd28C, TouchLcd28C }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TouchKind { None, Cst820, Gt911 }
#[derive(Clone, Copy)]
struct Profile { variant: TcaRgbVariant, name: &'static str, sku: &'static str, height: u16, round: bool, touch: TouchKind }
impl TcaRgbVariant {
    fn profile(self) -> Profile {
        let (name, sku, height, round, touch) = match self {
            Self::TouchLcd21 => ("waveshare-s3-touch-lcd21", "ESP32-S3-Touch-LCD-2.1", 480, true, TouchKind::Cst820),
            Self::TouchLcd21B => ("waveshare-s3-touch-lcd21b", "ESP32-S3-Touch-LCD-2.1B", 480, true, TouchKind::Cst820),
            Self::Lcd28B => ("waveshare-s3-lcd28b", "ESP32-S3-LCD-2.8B", 640, false, TouchKind::None),
            Self::TouchLcd28B => ("waveshare-s3-touch-lcd28b", "ESP32-S3-Touch-LCD-2.8B", 640, false, TouchKind::Gt911),
            Self::Lcd28C => ("waveshare-s3-lcd28c", "ESP32-S3-LCD-2.8C", 480, true, TouchKind::None),
            Self::TouchLcd28C => ("waveshare-s3-touch-lcd28c", "ESP32-S3-Touch-LCD-2.8C", 480, true, TouchKind::Gt911),
        };
        Profile { variant: self, name, sku, height, round, touch }
    }
    pub fn name(self) -> &'static str { self.profile().name }
    pub fn sku(self) -> &'static str { self.profile().sku }
    pub fn dimensions(self) -> (u32, u32) { (480, u32::from(self.profile().height)) }
    pub fn flash_mib(self) -> usize { 16 }
    pub fn psram_mib(self) -> usize { 8 }
}

// The GT911 wrapper adds only the actual read-only resolution at 8146h..8149h,
// reset/address strap and strict access boundaries. Other boards' existing
// shared GT911 behavior remains unchanged. CST820 has its own documented
// one-byte register protocol; it is not a differently named GT911/CST816S.
struct TouchChip {
    kind: TouchKind, io: Arc<Mutex<Tca9554State>>, height: u16, cycle: u64,
    reset_falls: u64, enabled: bool, irq_drive: Option<bool>, address: u8,
    gt_state: Arc<Mutex<Gt911State>>, gt: Gt911, next_scan: u64,
    pointer: u16, phase: u8, pointer_valid: bool, x: u16, y: u16, down: bool, irq_until: u64,
    disable_auto_sleep: Option<u8>, reads: u64, contact_reads: u64, updates: u64,
}
impl TouchChip {
    fn new(kind: TouchKind, height: u16, io: Arc<Mutex<Tca9554State>>) -> Self {
        let gt_state = Arc::new(Mutex::new(Gt911State::new(480, height, HZ / 100)));
        let gt = Gt911::new(gt_state.clone());
        Self { kind, io, height, cycle: 0, reset_falls: 0, enabled: true,
            irq_drive: None, address: if kind == TouchKind::Gt911 { 0x14 } else { 0x15 },
            gt_state, gt, next_scan: 0, pointer: 0, phase: 0, pointer_valid: false, x: 0, y: 0,
            down: false, irq_until: 0, disable_auto_sleep: None, reads: 0,
            contact_reads: 0, updates: 0 }
    }
    fn sync_reset(&mut self) {
        let (falls, high) = { let io = self.io.lock().expect("RGB expander"); (io.falling_edges(1), io.high(1)) };
        if falls != self.reset_falls {
            self.reset_falls = falls; self.enabled = false;
            self.pointer = 0; self.phase = 0; self.pointer_valid = false; self.down = false; self.irq_until = 0;
            self.disable_auto_sleep = None;
            *self.gt_state.lock().expect("RGB GT911 state") = Gt911State::new(480, self.height, HZ / 100);
            self.gt = Gt911::new(self.gt_state.clone()); self.next_scan = self.cycle;
        }
        if high && !self.enabled && self.kind == TouchKind::Gt911 {
            // Goodix Rev09 address selection: INT low when RST rises => 5Dh;
            // high => 14h. The board drawing supplies an external INT pull-up.
            self.address = if self.irq_drive == Some(false) { 0x5d } else { 0x14 };
        }
        self.enabled = high;
        self.gt_state.lock().expect("RGB GT911 state").powered = high;
    }
    fn advance_to(&mut self, cycle: u64) {
        self.cycle = cycle; self.sync_reset();
        if self.kind == TouchKind::Gt911 { self.gt_state.lock().expect("RGB GT911 state").advance_to(cycle); }
    }
    fn irq_high(&self) -> bool {
        if !self.enabled { return true; }
        match self.kind {
            TouchKind::None => true,
            TouchKind::Gt911 => !self.gt_state.lock().expect("RGB GT911 state").ready,
            TouchKind::Cst820 => self.cycle >= self.irq_until,
        }
    }
    fn update(&mut self, x: u16, y: u16, down: bool) {
        self.sync_reset(); if !self.enabled { return; }
        self.updates += 1; let (x, y) = (x.min(479), y.min(self.height - 1));
        if self.kind == TouchKind::Gt911 { self.gt_state.lock().expect("RGB GT911 state").update(x, y, down); }
        else {
            if (x, y, down) != (self.x, self.y, self.down) {
                // Driver documents falling-edge reports but no pulse duration.
                // One guest millisecond is an explicit digital injection policy.
                self.irq_until = self.cycle.saturating_add(HZ / 1000);
            }
            self.x = x; self.y = y; self.down = down;
        }
    }
    fn start(&mut self, address: u8, read: bool) -> bool {
        self.advance_to(self.cycle);
        if !self.enabled || self.address != address || (read && !self.pointer_valid) { return false; }
        if !read { self.phase = 0; }
        self.kind != TouchKind::Gt911 || self.gt.start(read)
    }
    fn write(&mut self, byte: u8) -> bool {
        self.sync_reset(); if !self.enabled { return false; }
        if self.kind == TouchKind::Gt911 {
            match self.phase {
                0 => { self.pointer = u16::from(byte) << 8; self.phase = 1; self.pointer_valid = false; return self.gt.write(byte); },
                1 => {
                    self.pointer |= u16::from(byte); self.phase = 2;
                    self.pointer_valid = matches!(self.pointer, 0x8047..=0x8100 | 0x8140..=0x8149 | 0x814e..=0x8157);
                    if !self.pointer_valid { return false; }
                    return self.gt.write(byte);
                },
                _ => {}
            }
            if !self.pointer_valid || (!(0x8047..=0x8100).contains(&self.pointer) && !(self.pointer == 0x814e && byte == 0)) { return false; }
            if self.pointer == 0x814e { self.next_scan = self.cycle.saturating_add(HZ / 100); }
            self.pointer = self.pointer.wrapping_add(1); self.gt.write(byte)
        } else {
            if self.phase == 0 {
                self.phase = 1; self.pointer = u16::from(byte);
                self.pointer_valid = matches!(byte, 0..=8 | 0x15 | 0xa7..=0xa9 | 0xfe);
                return self.pointer_valid;
            }
            if self.pointer != 0xfe || byte != 0xff { return false; }
            self.disable_auto_sleep = Some(byte); self.pointer += 1; true
        }
    }
    fn read(&mut self) -> u8 {
        self.sync_reset(); if !self.enabled || !self.pointer_valid { return 255; }
        let address = self.pointer; self.pointer = self.pointer.wrapping_add(1);
        if self.kind == TouchKind::Gt911 {
            let value = self.gt.read();
            if address == 0x814e { self.reads += 1; self.contact_reads += u64::from(value & 0x8f == 0x81); }
            match address {
                0x8146 => 480u16 as u8, 0x8147 => (480u16 >> 8) as u8,
                0x8148 => self.height as u8, 0x8149 => (self.height >> 8) as u8,
                _ => value,
            }
        } else {
            match address {
                0 | 1 | 7 | 8 => 0, // normal mode, no gesture, unused report tail
                2 => { self.reads += 1; self.contact_reads += u64::from(self.down); u8::from(self.down) },
                3 => (self.x >> 8) as u8 & 15, 4 => self.x as u8,
                5 => (self.y >> 8) as u8 & 15, 6 => self.y as u8,
                0x15 | 0xa7..=0xa9 => 0, // IDs not supplied by the official source
                0xfe => self.disable_auto_sleep.unwrap_or(0),
                _ => 255,
            }
        }
    }
}
struct TouchPort { chip: Rc<RefCell<TouchChip>>, address: u8 }
impl I2cDevice for TouchPort {
    fn start(&mut self, read: bool) -> bool { self.chip.borrow_mut().start(self.address, read) }
    fn write(&mut self, byte: u8) -> bool { self.chip.borrow_mut().write(byte) }
    fn read(&mut self) -> u8 { self.chip.borrow_mut().read() }
    fn stop(&mut self) { self.chip.borrow_mut().gt.stop(); }
}

pub struct WaveshareTcaRgb {
    p: Profile, io: Arc<Mutex<Tca9554State>>, rtc: Arc<Mutex<Pcf85063State>>,
    imu: Arc<ImuMotion>, qmi: Rc<RefCell<Qmi8658>>, touch: Rc<RefCell<TouchChip>>, i2c: I2cBitBang, attached: bool,
    cycle: u64, input: [bool; 3], edges: Vec<BoardEdge>, gpio_events: u64,
    frame: Vec<u16>, frames: u64, version: u64, rgb_valid: bool, rgb_map: [u8; 16],
    reset_falls: u64, select_rises: u64, reset_high: bool, backlight_gpio: bool,
    backlight: u32, page: u8, command: u8, args: [u8; 5], count: usize,
    sleeping: bool, on: bool, inverted: bool, madctl: u8, colmod: u8,
    commands: u64, vendor_bytes: u64, rejected_spi: u64, rejected_rgb: u64,
    sd: Option<Box<SpiSdCard>>, sd_present: bool, sd_select_rises: u64,
}
impl WaveshareTcaRgb {
    pub fn new(variant: TcaRgbVariant) -> Self {
        let p = variant.profile(); let io = Arc::new(Mutex::new(Tca9554State::default()));
        let imu = Arc::new(ImuMotion::default());
        let qmi = Rc::new(RefCell::new(Qmi8658::new_c(imu.clone(), HZ)));
        Self { p, touch: Rc::new(RefCell::new(TouchChip::new(p.touch, p.height, io.clone()))),
            io, rtc: Arc::new(Mutex::new(Pcf85063State::new(HZ))), imu, qmi,
            i2c: I2cBitBang::new(), attached: false, cycle: 0, input: [true; 3],
            edges: Vec::new(), gpio_events: 0, frame: vec![0; 480 * usize::from(p.height)],
            frames: 0, version: 0, rgb_valid: false, rgb_map: std::array::from_fn(|i| i as u8),
            reset_falls: 0, select_rises: 0, reset_high: true, backlight_gpio: false,
            backlight: 0, page: 0, command: 0, args: [0; 5], count: 0,
            sleeping: true, on: false, inverted: false, madctl: 0, colmod: 0x70,
            commands: 0, vendor_bytes: 0, rejected_spi: 0, rejected_rgb: 0,
            sd: None, sd_present: false, sd_select_rises: 0 }
    }
    fn panel_reset(&mut self) {
        self.page = 0; self.command = 0; self.count = 0; self.sleeping = true;
        self.on = false; self.inverted = false; self.madctl = 0; self.colmod = 0x70;
        self.frame.fill(0); self.version += 1;
    }
    fn sync_io(&mut self) {
        let (falls, high, rises, sd_rises) = { let io = self.io.lock().expect("RGB expander");
            (io.falling_edges(0), io.high(0), io.rising_edges(2), io.rising_edges(3)) };
        if falls != self.reset_falls { self.reset_falls = falls; self.panel_reset(); }
        if high != self.reset_high { self.reset_high = high; self.version += 1; }
        // Each delivered transfer is a complete 9-bit word. Raising CS between
        // words does not reset the panel's register parameter pointer.
        if rises != self.select_rises { self.select_rises = rises; }
        if sd_rises != self.sd_select_rises {
            self.sd_select_rises = sd_rises; if let Some(sd) = &mut self.sd { sd.deselect(); }
        }
    }
    fn sync_irq(&mut self) {
        if self.p.touch == TouchKind::None { return; }
        let high = self.touch.borrow().irq_high();
        if high != self.input[2] { self.input[2] = high; self.edges.push(BoardEdge { cycle: self.cycle, pin: 16, level: high }); }
    }
    fn serial_word(&mut self, data: bool, value: u8) {
        if !data {
            self.command = value; self.count = 0; self.commands += 1;
            if self.page == 0 {
                match value { 1 => self.panel_reset(), 0x10 => self.sleeping = true,
                    0x11 => self.sleeping = false, 0x20 => self.inverted = false,
                    0x21 => self.inverted = true, 0x28 => self.on = false,
                    0x29 => self.on = true, _ => {} }
            }
        } else if self.command == 0xff {
            if self.count < 5 { self.args[self.count] = value; self.count += 1; }
            if self.count == 5 && self.args[..4] == [0x77, 1, 0, 0] { self.page = self.args[4]; }
        } else {
            if self.page == 0 && self.count == 0 {
                match self.command { 0x36 => self.madctl = value & 0x18, 0x3a => self.colmod = value & 0x70, _ => {} }
            } else if self.page != 0 { self.vendor_bytes += 1; }
            self.count = self.count.saturating_add(1);
        }
        self.version += 1;
    }
    fn set_mapping(&mut self, controls: bool, mapping: [Option<u8>; 16]) {
        let valid = controls && mapping.iter().all(Option::is_some);
        let map = mapping.map(|v| v.unwrap_or(0));
        if (valid, map) != (self.rgb_valid, self.rgb_map) { self.rgb_valid = valid; self.rgb_map = map; self.version += 1; }
    }
    fn visible(&self) -> bool {
        let io = self.io.lock().expect("RGB expander");
        io.high(0) && io.falling_edges(0) == self.reset_falls && self.rgb_valid
            && self.backlight > 0 && self.on && !self.sleeping
    }
}
impl BoardModel for WaveshareTcaRgb {
    fn name(&self) -> &'static str { self.p.name }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) { self.gpio_events += changes.len() as u64; }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn gpio_waveform_at(&mut self, cycle: VirtualCycle, routes: PinRoutes<'_>) {
        self.touch.borrow_mut().irq_drive = routes.software_output(16);
        self.advance_to(cycle);
        let controls = routes.matrix_output(41, 154) && routes.matrix_output(40, 150)
            && routes.matrix_output(39, 152) && routes.matrix_output(38, 151);
        let mapping = RGB_PADS.map(|pin| (0..16).find(|i| routes.matrix_output(usize::from(pin), 133 + u32::from(*i))));
        self.set_mapping(controls, mapping);
        self.backlight_gpio = routes.software_output(6) == Some(true);
        let scl = routes.software_output(I2C_PADS.1).unwrap_or(true);
        let sda = self.i2c.drive(scl, routes.software_output(I2C_PADS.0).unwrap_or(true));
        for (i, (pin, level)) in [(I2C_PADS.0, sda), (I2C_PADS.1, scl)].into_iter().enumerate() {
            if self.input[i] != level { self.input[i] = level; self.edges.push(BoardEdge { cycle, pin, level }); }
        }
        self.sync_io(); self.touch.borrow_mut().advance_to(cycle); self.sync_irq();
    }
    fn lcd_rgb_pins(&mut self, pins: LcdRgbPins) {
        let controls = pins.width == 16 && pins.clock & (1 << 41) != 0 && pins.enable & (1 << 40) != 0
            && pins.vsync & (1 << 39) != 0 && pins.hsync & (1 << 38) != 0;
        self.set_mapping(controls, RGB_PADS.map(|pin| (0..16).find(|i| pins.data[usize::from(*i)] & (1 << pin) != 0)));
    }
    fn uses_gpio_pwm(&self) -> bool { true }
    fn gpio_pwm_at(&mut self, cycle: VirtualCycle, pwm: &dyn Fn(u8) -> Option<(f64, u32)>) {
        self.advance_to(cycle);
        let duty = pwm(6).map_or(if self.backlight_gpio { 65535 } else { 0 }, |(_, d)| d.min(65535));
        if duty != self.backlight { self.backlight = duty; self.version += 1; }
    }
    fn uses_qspi_pins(&self) -> bool { true }
    fn spi_transfer_qspi_pins(&mut self, _host: u8, pins: SpiQspiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        self.sync_io();
        let (lcd_selected, sd_selected) = { let io = self.io.lock().expect("RGB expander"); (!io.high(2), !io.high(3)) };
        let wires = pins.sclk & (1 << 2) != 0 && pins.data[0] & (1 << 1) != 0;
        let common = wires && !pins.unsupported_mode && !pins.sio && !pins.clock_idle_high
            && !pins.clock_output_edge && pins.dummy_cycles == 0
            && [pins.command_width, pins.address_width, pins.data_width, pins.read_width].iter().all(|w| *w <= 1);
        if lcd_selected {
            // TCA P2, not an MCU hardware-CS selector, controls this glass.
            // Official SPI2: 1-bit D/C command followed by an 8-bit address.
            if !common || sd_selected || !self.reset_high || pins.command_width != 1
                || pins.command_bits != 1 || pins.address_width != 1 || pins.address_bits != 8
                || pins.data_bits != 0 || pins.read_bits != 0 || rx_len != 0 || tx.len() != 2 {
                self.rejected_spi += 1; return vec![255; rx_len];
            }
            // IDF spi_ll.h at 73550728: MSB-first command output starts at
            // USER2.COMMAND_VALUE bit7, then bit15. A one-bit command is
            // therefore 00h/80h in the first serialized byte, not 00h/01h.
            self.serial_word(tx[0] & 0x80 != 0, tx[1]);
        } else if common && self.sd_present && pins.command_bits == 0 && pins.address_bits == 0
            && pins.data_bits.is_multiple_of(8) && pins.read_bits.is_multiple_of(8) {
            if let Some(card) = &mut self.sd {
                let reply = card.transfer(sd_selected, tx, rx_len);
                if pins.input[1] == Some(42) { return reply; }
            }
        }
        vec![255; rx_len]
    }
    fn lcd_frame(&mut self, w: u32, h: u32, data: &[u8]) {
        self.sync_io();
        if !self.rgb_valid || !self.reset_high || (w, h) != self.p.variant.dimensions() || data.len() != self.frame.len() * 2 {
            self.rejected_rgb += 1; return;
        }
        let direct = self.rgb_map.iter().enumerate().all(|(i, bit)| usize::from(*bit) == i);
        for (dst, src) in self.frame.iter_mut().zip(data.as_chunks::<2>().0) {
            let value = u16::from_le_bytes(*src);
            *dst = if direct { value } else { self.rgb_map.iter().enumerate().fold(0, |v, (bit, input)| v | (((value >> input) & 1) << bit)) };
        }
        self.frames += 1;
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if !self.attached {
            self.i2c.attach(0x20, Box::new(Tca9554::new(self.io.clone())));
            self.i2c.attach(0x51, Box::new(Pcf85063::new(self.rtc.clone())));
            self.i2c.attach(Qmi8658::i2c_address(false), Box::new(Qmi8658Port::new(self.qmi.clone())));
            let addresses: &[u8] = match self.p.touch { TouchKind::None => &[], TouchKind::Cst820 => &[0x15], TouchKind::Gt911 => &[0x5d, 0x14] };
            for &address in addresses { self.i2c.attach(address, Box::new(TouchPort { chip: self.touch.clone(), address })); }
            self.attached = true;
        }
        (0..2).flat_map(|bus| self.i2c.controller_devices(Some(I2C_PADS)).into_iter().map(move |(a, d)| (bus, a, d))).collect()
    }
    fn advance_to(&mut self, cycle: VirtualCycle) {
        self.cycle = cycle; self.imu.cycle.store(cycle, Ordering::Relaxed);
        self.rtc.lock().expect("RGB RTC state").advance_to(cycle);
        self.sync_io(); self.touch.borrow_mut().advance_to(cycle); self.sync_irq();
    }
    fn next_deadline(&self) -> Option<VirtualCycle> {
        let chip = self.touch.borrow();
        if !chip.enabled { return None; }
        match self.p.touch {
            TouchKind::Gt911 if !chip.gt_state.lock().expect("RGB GT911 state").ready && chip.next_scan > self.cycle => Some(chip.next_scan),
            TouchKind::Cst820 if chip.irq_until > self.cycle => Some(chip.irq_until),
            _ => None,
        }
    }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        if self.p.touch != TouchKind::None { self.touch.borrow_mut().update(x, y, down); self.sync_irq(); }
    }
    fn touch_at(&mut self, cycle: VirtualCycle, x: u16, y: u16, down: bool) { self.advance_to(cycle); self.touch(x, y, down); }
    fn set_imu_motion(&mut self, mode: u32) -> bool { self.qmi.borrow_mut().set_motion_mode(mode) }
    fn set_imu_sample(&mut self, accel_g: [f64; 3], gyro_dps: [f64; 3]) -> bool { self.qmi.borrow_mut().set_host_sample(accel_g, gyro_dps) }
    fn imu_motion(&self) -> u32 { self.imu.mode.load(Ordering::Relaxed) }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn input_levels(&self) -> Vec<(u8, bool)> {
        let mut result = vec![(0, true), (15, self.input[0]), (7, self.input[1])];
        if self.p.touch != TouchKind::None { result.push((16, self.input[2])); } result
    }
    fn named_pin(&self, name: &str) -> Option<u8> { match name { "boot" | "button" => Some(0), _ => None } }
    fn sd_card_slot(&self) -> Option<([u8; 6], bool)> {
        // Every retained SD_Card.cpp uses SD_MMC.setPins(2, 1, 42, -1, -1, -1)
        // and one-bit mode. EXIO_PIN4 is one-based, hence physical TCA P3:
        // the factory holds that DAT3 pin high for native SD selection.
        // D1/D2 have no MCU route; DAT3 is not a fourth SDMMC data lane.
        // Low DAT3/native-mode transitions are outside this supported path;
        // this readiness gate is not a claim that DAT3 removes card power.
        Some(([2, 1, 42, 255, 255, 255],
            self.sd_present && self.io.lock().expect("RGB expander").high(3)))
    }
    fn attach_spi_sd_card(&mut self, image: Box<dyn SdStore>, config: SdCardConfig) -> Result<String, String> {
        let card = SpiSdCard::new(image, config)?; let description = card.describe();
        self.sd = Some(Box::new(card)); self.sd_present = true; Ok(description)
    }
    fn set_sd_card_present(&mut self, present: bool) {
        if present != self.sd_present { if let Some(card) = &mut self.sd { card.power_cycle(); } }
        self.sd_present = present;
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let mut out = vec![0; self.frame.len()];
        if self.visible() {
            let height = usize::from(self.p.height);
            for y in 0..height { for x in 0..480 {
                if self.p.round { let dx = 2 * x as i32 - 479; let dy = 2 * y as i32 - 479; if dx * dx + dy * dy > 480 * 480 { continue; } }
                let row = if self.madctl & 0x10 != 0 { height - 1 - y } else { y };
                let mut pixel = self.frame[row * 480 + x];
                if self.madctl & 8 != 0 { pixel = (pixel & 0x07e0) | ((pixel & 0xf800) >> 11) | ((pixel & 31) << 11); }
                if self.inverted { pixel ^= 0xffff; }
                let p = u32::from(pixel); let d = self.backlight;
                out[y * 480 + x] = ((((p >> 11) & 31) * d / 65535) << 11 | (((p >> 5) & 63) * d / 65535) << 5 | ((p & 31) * d / 65535)) as u16;
            }}
        }
        Some((480, u32::from(self.p.height), out, self.display_version()))
    }
    fn display_frames(&self) -> u64 { self.frames }
    fn display_version(&self) -> u64 { self.frames.wrapping_add(self.version) }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { Some((self.frame.clone(), 480, usize::from(self.p.height))) }
    fn report(&self) -> String {
        let touch = self.touch.borrow(); let io = self.io.lock().expect("RGB expander");
        let mut report = format!("{}: ST7701S RGB565 480x{} frames={} commands={} vendorBytes={} rejectedSPI={} rejectedRGB={} routed={} map={:?} on={} sleeping={} reset={} bank={:02x} madctl={:02x} colmod={:02x} backlight={} TCA9554@20 levels={:02x} touch={:?}@{:02x} reset={} reads={} contactReads={} hostUpdates={} QMI8658C@6b motion={} PCF85063A@51 I2C=15/7 physicalTF=true inserted={} SD={}. Analog panel voltages/gamma, exact reset/power timings, calibrated brightness/optical pose, CST IDs/gestures/scan/auto-sleep, GT config-engine/IRQ-mode/physical scan timing, QMI IRQ/calibration, RTC alarm/clock outputs, buzzer/battery/charger and RF are outside this subset.",
            self.p.sku, self.p.height, self.frames, self.commands, self.vendor_bytes, self.rejected_spi, self.rejected_rgb,
            self.rgb_valid, self.rgb_map, self.on, self.sleeping, io.high(0), self.page, self.madctl, self.colmod,
            self.backlight, io.levels(), self.p.touch, touch.address, touch.enabled, touch.reads, touch.contact_reads,
            touch.updates, self.imu_motion(), self.sd_present, self.sd.as_ref().map_or_else(|| {
                if self.sd_present { format!("native SDMMC one-bit CLK2/CMD1/D0=42 DAT3-high={}", io.high(3)) }
                else { "absent".into() }
            }, |sd| sd.report()));
        if self.qmi.borrow().host_sample_active() { report.push_str(" Host IMU sample held in sensor axes; physical PCB pose uncalibrated."); }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_soc::pins::ChipPins;

    const VARIANTS: [TcaRgbVariant; 6] = [TcaRgbVariant::TouchLcd21, TcaRgbVariant::TouchLcd21B,
        TcaRgbVariant::Lcd28B, TcaRgbVariant::TouchLcd28B, TcaRgbVariant::Lcd28C, TcaRgbVariant::TouchLcd28C];
    fn write8(d: &mut dyn I2cDevice, r: u8, v: &[u8]) {
        assert!(d.start(false)); assert!(d.write(r)); for &b in v { assert!(d.write(b)); } d.stop();
    }
    fn read8(d: &mut dyn I2cDevice, r: u8, n: usize) -> Vec<u8> {
        write8(d, r, &[]); assert!(d.start(true)); let result = (0..n).map(|_| d.read()).collect(); d.stop(); result
    }
    fn read16(d: &mut dyn I2cDevice, r: u16, n: usize) -> Vec<u8> {
        assert!(d.start(false)); for b in r.to_be_bytes() { assert!(d.write(b)); }
        assert!(d.start(true)); let result = (0..n).map(|_| d.read()).collect(); d.stop(); result
    }
    fn output(b: &mut WaveshareTcaRgb, v: u8) {
        let mut io = Tca9554::new(b.io.clone()); write8(&mut io, 3, &[0xf0]); write8(&mut io, 1, &[v]); b.sync_io();
    }
    fn pins() -> SpiQspiPins {
        SpiQspiPins { sclk: 1 << 2, data: [1 << 1, 0, 0, 0], command_width: 1, command_bits: 1,
            address_width: 1, address_bits: 8, ..Default::default() }
    }
    fn command(b: &mut WaveshareTcaRgb, c: u8, data: &[u8]) {
        output(b, 0xfb); b.spi_transfer_qspi_pins(2, pins(), &[0, c], 0);
        for &v in data { b.spi_transfer_qspi_pins(2, pins(), &[0x80, v], 0); }
    }
    fn gpio_route(b: &mut WaveshareTcaRgb, omit: Option<u8>, light: bool, swap: bool) {
        let mut gpio = esp_periph::Gpio::new(); let mut mux = esp_periph::RegRam::new();
        for (pin, signal) in RGB_PADS.into_iter().enumerate().map(|(i, p)| (p, 133 + if swap { i ^ 8 } else { i } as u32))
            .chain([(41, 154), (40, 150), (39, 152), (38, 151), (6, 256)]) {
            if omit == Some(pin) { continue; }
            gpio.enable |= 1 << pin; gpio.func_out_sel[usize::from(pin)] = signal;
            mux.write(4 + u32::from(pin) * 4, 1 << 12);
        }
        if light { gpio.out |= 1 << 6; }
        b.gpio_waveform_at(b.cycle, ChipPins::S3.routes(&gpio, &mux)); b.gpio_pwm_at(b.cycle, &|_| None);
    }
    fn init(b: &mut WaveshareTcaRgb) {
        command(b, 0xff, &[0x77, 1, 0, 0, 0]); command(b, 0x11, &[]); command(b, 0x29, &[]);
        gpio_route(b, None, true, false);
    }
    #[test]
    fn six_exact_profiles_require_expander_reset_display_init_and_every_rgb_lane() {
        for v in VARIANTS {
            let mut b = WaveshareTcaRgb::new(v); assert_eq!(b.name(), v.name()); assert!(!b.visible());
            init(&mut b); let mut frame = vec![0; 480 * usize::from(b.p.height) * 2];
            let i = 240 * 480 + 240; frame[2 * i..2 * i + 2].copy_from_slice(&0xf800u16.to_le_bytes());
            b.lcd_frame(480, u32::from(b.p.height), &frame); assert_eq!(b.display().unwrap().2[i], 0xf800);
            for pin in RGB_PADS.into_iter().chain([41, 40, 39, 38]) {
                gpio_route(&mut b, Some(pin), true, false); b.lcd_frame(480, u32::from(b.p.height), &frame);
                assert_eq!(b.frames, 1); assert!(!b.visible(), "missing pad {pin}");
            }
            gpio_route(&mut b, None, true, false); b.lcd_frame(1, 1, &frame); assert_eq!(b.frames, 1);
            b.lcd_frame(480, u32::from(b.p.height), &frame[..frame.len() - 1]); assert_eq!(b.frames, 1);
            command(&mut b, 0x10, &[]); assert!(!b.visible()); command(&mut b, 0x11, &[]); assert!(b.visible());
            output(&mut b, 0xfa); assert!(!b.visible()); output(&mut b, 0xfb); assert!(!b.visible(), "reset requires fresh panel init");
            assert_eq!(b.named_pin("boot"), Some(0)); assert_eq!(b.p.round, b.p.height == 480);
        }
    }
    #[test]
    fn nine_bit_spi_honors_real_phase_pads_and_vendor_banks_without_legacy_fallback() {
        let mut b = WaveshareTcaRgb::new(TcaRgbVariant::TouchLcd21); output(&mut b, 0xfb);
        command(&mut b, 0xff, &[0x77, 1, 0, 0, 0x11]); command(&mut b, 0x11, &[]); command(&mut b, 0x29, &[]);
        assert!(b.sleeping); assert!(!b.on, "bank11 vendor opcodes are not common DCS");
        command(&mut b, 0xff, &[0x77, 1, 0, 0, 0]);
        for bad in [SpiQspiPins { sclk: 1 << 3, ..pins() }, SpiQspiPins { data: [1 << 2, 0, 0, 0], ..pins() },
            SpiQspiPins { command_bits: 8, ..pins() }, SpiQspiPins { address_width: 4, ..pins() },
            SpiQspiPins { address_bits: 16, ..pins() }, SpiQspiPins { data_bits: 1, ..pins() },
            SpiQspiPins { clock_output_edge: true, ..pins() }, SpiQspiPins { dummy_cycles: 1, ..pins() }] {
            b.spi_transfer_qspi_pins(2, bad, &[0, 0x29], 0);
        }
        assert!(!b.on); assert_eq!(b.rejected_spi, 8);
        output(&mut b, 0xff); b.spi_transfer_qspi_pins(2, pins(), &[0, 0x29], 0); assert!(!b.on, "external TCA CS is high");
        output(&mut b, 0xf3); b.spi_transfer_qspi_pins(2, pins(), &[0, 0x29], 0); assert!(!b.on, "SD/LCD simultaneous select is not valid");
        command(&mut b, 0x29, &[]); assert!(b.on); command(&mut b, 0x36, &[0xff]); assert_eq!(b.madctl, 0x18);
        b.spi_transfer(2, &[0, 0x28], 0); assert!(b.on, "unrouted callback has no panel side effect");
    }
    #[test]
    fn idf_one_bit_command_alignment_survives_actual_spi_mmio_and_external_cs_boundaries() {
        let mut b = WaveshareTcaRgb::new(TcaRgbVariant::TouchLcd21);
        let mut gpio = esp_periph::Gpio::new(); let mut mux = esp_periph::RegRam::new();
        for (pin, signal) in [(2, 101), (1, 103)] {
            gpio.func_out_sel[pin] = signal; gpio.enable |= 1 << pin; mux.write(4 + pin as u32 * 4, 1 << 12);
        }
        let mut spi = esp_periph::GpSpi::new();
        spi.write(0x10, (1 << 31) | (1 << 30)); // command + address; no dummy/data
        spi.write(0x14, 7 << 27); // 8-bit address
        spi.write(0x20, 0x3f); // every hardware CS disabled; TCA owns chip select
        for (dc, value) in [(false, 0xff), (true, 0x77), (true, 1), (true, 0), (true, 0), (true, 0x11), (false, 0x29)] {
            output(&mut b, 0xfb);
            spi.write(0x18, if dc { 0x80 } else { 0 }); // HAL_SPI_SWAP_DATA_TX(cmd, 1)
            spi.write(4, u32::from(value) << 24); spi.write(0, 1 << 24);
            let snapshot = ChipPins::S3.routes(&gpio, &mux).spi_qspi_pins(&spi,
                [101, 103, 102, 105, 104, 110, 111, 112, 113, 114, 115], &[]);
            let transfer = spi.take_transfer().unwrap();
            assert_eq!(transfer.tx, [if dc { 0x80 } else { 0 }, value]);
            b.spi_transfer_qspi_pins(2, snapshot, &transfer.tx, transfer.rx_len);
            output(&mut b, 0xff);
        }
        assert_eq!(b.page, 0x11); assert!(!b.on, "bank11 parameter bytes must not be interpreted as display commands");
        assert_eq!(b.commands, 2); assert_eq!(b.rejected_spi, 0);
        command(&mut b, 0xff, &[0x77, 1, 0, 0, 0]); command(&mut b, 0x3a, &[0x66]);
        assert_eq!(b.colmod, 0x60);
    }
    #[test]
    fn rgb_scan_color_and_matrix_lane_mapping_follow_received_wire_bits() {
        let mut b = WaveshareTcaRgb::new(TcaRgbVariant::Lcd28B); init(&mut b);
        let mut frame = vec![0; 480 * 640 * 2]; for i in 0..16 { frame[2 * i..2 * i + 2].copy_from_slice(&(1u16 << i).to_be_bytes()); }
        gpio_route(&mut b, None, true, true); b.lcd_frame(480, 640, &frame);
        for (i, p) in b.display().unwrap().2.iter().take(16).enumerate() { assert_eq!(*p, 1 << i); }
        command(&mut b, 0x36, &[0x18]); let shown = b.display().unwrap().2;
        assert_eq!(shown[639 * 480], 0x0800, "ML reverses row scan; BGR swaps color channels");
        gpio_route(&mut b, None, false, true); assert!(b.display().unwrap().2.iter().all(|p| *p == 0));
        b.gpio_pwm_at(0, &|p| (p == 6).then_some((20000., 32768))); assert!(b.visible());
    }
    #[test]
    fn non_touch_has_no_touch_endpoint_and_all_buses_share_grounded_qmi_and_rtc() {
        for v in [TcaRgbVariant::Lcd28B, TcaRgbVariant::Lcd28C] {
            let mut b = WaveshareTcaRgb::new(v); let mut devices = b.i2c_devices(); assert_eq!(devices.len(), 6);
            assert!(devices.iter().all(|(_, a, d)| [0x20, 0x51, 0x6b].contains(a) && d.pins() == Some((15, 7))));
            let d = devices.iter_mut().find(|(bus, a, _)| (*bus, *a) == (0, 0x51)).unwrap().2.as_mut();
            write8(d, 4, &[0]); assert_eq!(read8(d, 4, 1), [0]); b.advance_to(HZ);
            let d = devices.iter_mut().find(|(bus, a, _)| (*bus, *a) == (1, 0x51)).unwrap().2.as_mut();
            assert_eq!(read8(d, 4, 1), [1], "controller adapters forward STOP");
            let d = devices.iter_mut().find(|(bus, a, _)| (*bus, *a) == (0, 0x6b)).unwrap().2.as_mut();
            assert_eq!(read8(d, 0, 1), [5]); b.touch(100, 100, true); assert_eq!(b.touch.borrow().updates, 0);
            assert_eq!(b.i2c_devices().len(), 6, "reattach preserves devices instead of duplicating them");
        }
    }
    #[test]
    fn cst820_is_its_own_single_contact_protocol_and_real_expander_reset_gates_it() {
        for v in [TcaRgbVariant::TouchLcd21, TcaRgbVariant::TouchLcd21B] {
            let mut b = WaveshareTcaRgb::new(v); let mut devices = b.i2c_devices();
            assert_eq!(devices.len(), 8); let d = devices.iter_mut().find(|(bus, a, _)| (*bus, *a) == (0, 0x15)).unwrap().2.as_mut();
            output(&mut b, 0xfd); assert!(!d.start(false)); output(&mut b, 0xff);
            write8(d, 0xfe, &[0xff]); b.touch_at(HZ, 123, 345, true);
            assert_eq!(read8(d, 1, 6), [0, 1, 0, 123, 1, 89]); assert_eq!(read8(d, 0xa7, 3), [0; 3]);
            assert!(d.start(false)); assert!(d.write(0xa7)); assert!(!d.write(0xb5)); d.stop();
            assert!(b.take_edges().iter().any(|e| e.pin == 16 && !e.level));
            b.advance_to(HZ + HZ / 1000); assert!(b.take_edges().iter().any(|e| e.pin == 16 && e.level));
            assert_eq!(read8(d, 2, 1), [1], "IRQ pulse end does not erase a held contact");
            b.touch(123, 345, false); assert_eq!(read8(d, 2, 1), [0]);
        }
    }
    #[test]
    fn gt911_actual_resolution_is_read_only_and_reset_strap_selects_one_address() {
        for v in [TcaRgbVariant::TouchLcd28B, TcaRgbVariant::TouchLcd28C] {
            let mut b = WaveshareTcaRgb::new(v); let mut devices = b.i2c_devices();
            b.touch.borrow_mut().irq_drive = Some(false); output(&mut b, 0xfd); b.advance_to(1); output(&mut b, 0xff); b.advance_to(2);
            let high = devices.iter_mut().find(|(bus, a, _)| (*bus, *a) == (0, 0x14)).unwrap().2.as_mut(); assert!(!high.start(false));
            let d = devices.iter_mut().find(|(bus, a, _)| (*bus, *a) == (0, 0x5d)).unwrap().2.as_mut();
            assert_eq!(read16(d, 0x8140, 4), b"911\0");
            let h = b.p.height.to_le_bytes(); assert_eq!(read16(d, 0x8146, 4), [0xe0, 1, h[0], h[1]]);
            assert!(d.start(false)); for byte in [0x81, 0x46] { assert!(d.write(byte)); } assert!(!d.write(0)); d.stop();
            assert!(d.start(false)); for byte in [0x80, 0x48, 1, 0] { assert!(d.write(byte)); } d.stop();
            assert_eq!(read16(d, 0x8048, 2), [1, 0]); assert_eq!(read16(d, 0x8146, 2), [0xe0, 1], "unimplemented config engine does not change actual report geometry");
            assert!(d.start(false)); for byte in [0x81, 0x4e, 0] { assert!(d.write(byte)); } d.stop();
            b.touch_at(3, 200, 300, true); b.advance_to(2 + HZ / 100);
            assert_eq!(read16(d, 0x814e, 8), [0x81, 0, 200, 0, 44, 1, 20, 0]);
            b.touch.borrow_mut().irq_drive = Some(true); output(&mut b, 0xfd); b.advance_to(HZ); output(&mut b, 0xff); b.advance_to(HZ + 1);
            assert!(!d.start(false)); let high = devices.iter_mut().find(|(bus, a, _)| (*bus, *a) == (1, 0x14)).unwrap().2.as_mut();
            assert_eq!(read16(high, 0x8146, 4), [0xe0, 1, h[0], h[1]]);
        }
    }
    #[test]
    fn spi_tf_is_optional_and_uses_expander_select_and_actual_miso_pad() {
        let mut b = WaveshareTcaRgb::new(TcaRgbVariant::Lcd28C); output(&mut b, 0xff);
        b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0; 1 << 20])), Default::default()).unwrap();
        let p = SpiQspiPins { sclk: 1 << 2, data: [1 << 1, 0, 0, 0], input: [None, Some(42), None, None], data_width: 1, data_bits: 80, ..Default::default() };
        b.spi_transfer_qspi_pins(2, p, &[255; 10], 0); output(&mut b, 0xf7);
        let p = SpiQspiPins { data_bits: 48, ..p }; b.spi_transfer_qspi_pins(2, p, &[0x40, 0, 0, 0, 0, 0x95], 0);
        let p = SpiQspiPins { data_bits: 16, read_bits: 16, read_width: 1, ..p };
        let reply = b.spi_transfer_qspi_pins(2, p, &[255; 2], 2); assert_eq!(reply, [255, 1], "NCR byte precedes R1");
        b.spi_transfer_qspi_pins(2, SpiQspiPins { data_bits: 48, read_bits: 0, ..p }, &[0x40, 0, 0, 0, 0, 0x95], 0);
        assert_eq!(b.spi_transfer_qspi_pins(2, SpiQspiPins { input: [None, Some(41), None, None], ..p }, &[255; 2], 2), [255; 2], "card response is lost on an incorrectly routed MISO");
        b.set_sd_card_present(false); assert_eq!(b.spi_transfer_qspi_pins(2, p, &[255; 1], 1), [255]);
    }
    fn factory(v: TcaRgbVariant) {
        use esp_soc::{LoadKind, SocBus, Stop};
        let source = std::env::var("ESP32SIM_WAVESHARE_RGB_SOURCES").expect("original pinned official image cache required");
        let rom = std::env::var("ESP32SIM_ROM_DIR").expect("original ESP32-S3 mask ROM required");
        let (repo, filename) = match v {
            TcaRgbVariant::TouchLcd21 | TcaRgbVariant::TouchLcd21B => ("ESP32-S3-Touch-LCD-2.1", "ESP32-S3-Touch-LCD-2.1_FactoryOnly_20250224.bin"),
            TcaRgbVariant::Lcd28B => ("ESP32-S3-Touch-LCD-2.8B", "ESP32-S3-LCD-2.8B_FactoryOnly_20250124.bin"),
            TcaRgbVariant::TouchLcd28B => ("ESP32-S3-Touch-LCD-2.8B", "ESP32-S3-Touch-LCD-2.8B_FactoryOnly_20241025.bin"),
            TcaRgbVariant::Lcd28C => ("ESP32-S3-Touch-LCD-2.8C", "ESP32-S3-LCD-2.8C_FactoryOnly_20250124.bin"),
            TcaRgbVariant::TouchLcd28C => ("ESP32-S3-Touch-LCD-2.8C", "ESP32-S3-Touch-LCD-2.8C_FactoryOnly_20250224.bin"),
        };
        let make = || {
            let mut m = crate::machine([2, 0, 0, 0, 0, 1]); m.bus.board = Box::new(WaveshareTcaRgb::new(v)); m.bus.attach_board_devices();
            m.bus.set_flash_size(v.flash_mib() << 20); m.bus.set_psram_size(v.psram_mib() << 20).unwrap(); m.console.capture = true; m.console.mask = 2;
            m.load_input(LoadKind::Rom, &std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
            m.load_input(LoadKind::Flash, &std::fs::read(format!("{source}/{repo}/firmware/{filename}")).unwrap()).unwrap(); m.boot_rom(); m
        };
        let run = |m: &mut crate::Machine, ms: u64| {
            m.max_cycles = m.bus.cycles + ms * HZ / 1000;
            assert!(matches!(m.run(u64::MAX), Stop::Halted)); m.bus.refresh_board_outputs();
        };
        let save = |m: &mut crate::Machine, label: &str| {
            let ms = m.bus.cycles / (HZ / 1000);
            eprintln!("{} {label} {ms}ms reboots={} {}", v.sku(), m.reboots, m.bus.board.report());
            if let Ok(root) = std::env::var("ESP32SIM_WAVESHARE_RGB_EVIDENCE") {
                let path = std::path::Path::new(&root).join(v.sku()); std::fs::create_dir_all(&path).unwrap();
                m.write_tft_png(path.join(format!("{label}.png")).to_str().unwrap(), 1).unwrap();
                let (w, h, pixels, _) = m.bus.board.display().unwrap();
                std::fs::write(path.join(format!("{label}.rgb565")), pixels.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>()).unwrap();
                std::fs::write(path.join(format!("{label}.uart.txt")), &m.console.all).unwrap();
                std::fs::write(path.join(format!("{label}.board.txt")), m.bus.board.report()).unwrap();
                std::fs::write(path.join(format!("{label}.state.json")), format!("{{\"guestCycles\":{},\"guestMilliseconds\":{ms},\"reboots\":{},\"width\":{w},\"height\":{h},\"rgbFrames\":{},\"corePc\":[{},{}]}}\n", m.bus.cycles, m.reboots, m.bus.board.display_frames(), m.cores[0].pc, m.cores[1].pc)).unwrap();
            }
        };
        let mut m = make();
        for ms in [2000, 4000, 6000, 8000] { let step = ms - m.bus.cycles / (HZ / 1000); run(&mut m, step); save(&mut m, &format!("initial-{ms}")); }
        let before_late = m.bus.board.display().unwrap().2;
        let touch = v.profile().touch != TouchKind::None;
        if touch { m.bus.touch_input(240, 240, true); } else { m.bus.gpio_set_input(0, false); }
        run(&mut m, 100);
        if touch { m.bus.touch_input(240, 240, false); } else { m.bus.gpio_set_input(0, true); }
        run(&mut m, 300); save(&mut m, "late-input"); let after_late = m.bus.board.display().unwrap().2;
        assert_eq!(m.reboots, 0); assert!(m.bus.board.display_frames() > 0, "no actual LCD RGB frame received"); drop(m);
        let gesture = |m: &mut crate::Machine, input: bool| {
            let height = v.dimensions().1 as u16; let (x, begin, end) = if height == 640 { (440, 530, 130) } else { (420, 360, 120) };
            if input { if touch { m.bus.touch_input(x, begin, true); } else { m.bus.gpio_set_input(0, false); } }
            run(m, 60);
            for i in 1..=20 { if input && touch { m.bus.touch_input(x, begin - (begin - end) * i / 20, true); } run(m, 20); }
            if input { if touch { m.bus.touch_input(x, end, false); } else { m.bus.gpio_set_input(0, true); } }
            run(m, 60); run(m, 340);
        };
        let mut control = make(); run(&mut control, 2000); run(&mut control, 200); gesture(&mut control, false);
        save(&mut control, "early-matched-control"); let reference = control.bus.board.display().unwrap().2; drop(control);
        let mut input = make(); run(&mut input, 2000); save(&mut input, "early-before"); let before = input.bus.board.display().unwrap().2;
        run(&mut input, 200); save(&mut input, "early-quiet"); let quiet = input.bus.board.display().unwrap().2;
        gesture(&mut input, true); save(&mut input, "early-input"); let after = input.bus.board.display().unwrap().2;
        let different = |a: &[u16], b: &[u16]| a.iter().zip(b).filter(|(a, b)| a != b).count();
        let result = format!("{{\"quietRawPixels\":{},\"earlyInputVsMatchedControlPixels\":{},\"lateInputRawPixels\":{},\"inputKind\":\"{}\",\"earlyInputWindowMs\":[2200,3060],\"lateInputWindowMs\":[8000,8400]}}\n", different(&before, &quiet), different(&reference, &after), different(&before_late, &after_late), if touch { "touch-scroll" } else { "boot-button" });
        eprintln!("{} INPUT {result}", v.sku());
        if let Ok(root) = std::env::var("ESP32SIM_WAVESHARE_RGB_EVIDENCE") { std::fs::write(std::path::Path::new(&root).join(v.sku()).join("input-comparison.json"), result).unwrap(); }
        assert_eq!(input.reboots, 0);
    }
    #[test] #[ignore = "original factory image and mask ROM required"] fn external_touch21_factory() { factory(TcaRgbVariant::TouchLcd21); }
    #[test] #[ignore = "official 2.1 series image, not a separately named B factory image"] fn external_touch21b_factory() { factory(TcaRgbVariant::TouchLcd21B); }
    #[test] #[ignore = "original factory image and mask ROM required"] fn external_lcd28b_factory() { factory(TcaRgbVariant::Lcd28B); }
    #[test] #[ignore = "original factory image and mask ROM required"] fn external_touch28b_factory() { factory(TcaRgbVariant::TouchLcd28B); }
    #[test] #[ignore = "original factory image and mask ROM required"] fn external_lcd28c_factory() { factory(TcaRgbVariant::Lcd28C); }
    #[test] #[ignore = "original factory image and mask ROM required"] fn external_touch28c_factory() { factory(TcaRgbVariant::TouchLcd28C); }
}
