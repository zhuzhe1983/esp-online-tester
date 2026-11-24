//! The non-touch LCD-1.69 PCB, including its documented pin revisions.
//! Original schematics and demo 09 distinguish SYS_OUT36/SYS_EN35/Buzz33
//! from V2 SYS_OUT40/SYS_EN41/Buzz42. Neither has a populated touch IC or TF.
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, atomic::Ordering}};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board, BoardEdge, BoardModel, SpiQspiPins}, pins::PinRoutes,
    devices::{DcsPanel, i2c_bitbang::I2cBitBang, imu_motion::ImuMotion,
        pcf85063::{Pcf85063, Pcf85063State}, qmi8658::{Qmi8658, Qmi8658Port}}};

const HZ: u64 = 240_000_000;

pub fn make_board(name: &str) -> Option<Board> {
    let v2 = match name {
        "waveshare-s3-lcd169-v1" => false,
        "waveshare-s3-lcd169-v2" => true,
        _ => return None,
    };
    Some(Box::new(WaveshareLcd169::new(v2)))
}

struct WaveshareLcd169 {
    v2: bool, panel: DcsPanel, command: u8, dc: Option<bool>, reset: bool,
    light: u32, gpio_light: u32, pwm: Option<u32>, version: u64, cycle: u64,
    rejected: u64, i2c: I2cBitBang, attached: bool, levels: [bool; 2],
    edges: Vec<BoardEdge>, rtc: Arc<Mutex<Pcf85063State>>, motion: Arc<ImuMotion>,
    qmi: Rc<RefCell<Qmi8658>>,
}
impl WaveshareLcd169 {
    fn new(v2: bool) -> Self {
        let motion = Arc::new(ImuMotion::default());
        let qmi = Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(), HZ)));
        Self { v2, panel: DcsPanel::st7789(), command: 0, dc: None, reset: false,
            light: 0, gpio_light: 0, pwm: None, version: 0, cycle: 0, rejected: 0,
            i2c: I2cBitBang::new(), attached: false, levels: [true; 2], edges: vec![],
            rtc: Arc::new(Mutex::new(Pcf85063State::new(HZ))), motion, qmi }
    }
    fn power_key(&self) -> u8 { if self.v2 { 40 } else { 36 } }
    fn sync_light(&mut self) {
        let light = self.pwm.unwrap_or(self.gpio_light);
        if self.light != light { self.light = light; self.version += 1; }
    }
}
impl BoardModel for WaveshareLcd169 {
    fn name(&self) -> &'static str { if self.v2 { "waveshare-s3-lcd169-v2" } else { "waveshare-s3-lcd169-v1" } }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_waveform_at(&mut self, cycle: u64, routes: PinRoutes<'_>) {
        self.advance_to(cycle);
        let reset = routes.software_output(8) == Some(true);
        if self.reset && !reset { self.panel.reset(); self.command = 0; }
        if reset != self.reset { self.version += 1; }
        self.reset = reset;
        self.dc = routes.software_output(4);
        self.gpio_light = if routes.software_output(15) == Some(true) { 65535 } else { 0 };
        let scl = routes.software_output(10).unwrap_or(true);
        let sda = self.i2c.drive(scl, routes.software_output(11).unwrap_or(true));
        for (i, (pin, level)) in [(11, sda), (10, scl)].into_iter().enumerate() {
            if self.levels[i] != level {
                self.levels[i] = level;
                self.edges.push(BoardEdge { cycle, pin, level });
            }
        }
        self.sync_light();
    }
    fn uses_gpio_pwm(&self) -> bool { true }
    fn gpio_pwm_at(&mut self, cycle: u64, pwm: &dyn Fn(u8) -> Option<(f64, u32)>) {
        self.advance_to(cycle);
        self.pwm = pwm(15).map(|(_, duty)| duty.min(65535));
        self.sync_light();
    }
    fn uses_qspi_pins(&self) -> bool { true }
    fn spi_transfer_qspi_pins(&mut self, _host: u8, p: SpiQspiPins, tx: &[u8], rx: usize) -> Vec<u8> {
        if p.cs & (1 << 5) == 0 { return vec![255; rx]; }
        // ST7789 accepts the MOSI stream with or without simultaneous host RX.
        // With no physical MISO, readback stays floating-high. ESP32's output
        // edge bit is zero for both supported SPI modes 0 and 3, not CPHA.
        let valid = self.reset && self.dc.is_some()
            && !p.sio && !p.unsupported_mode && !p.clock_output_edge
            && p.dummy_cycles == 0 && p.address_bits == 0 && p.sclk & (1 << 6) != 0
            && p.data[0] & (1 << 7) != 0
            && [p.command_width, p.address_width, p.data_width, p.read_width].iter().all(|w| *w <= 1)
            && p.command_bits.is_multiple_of(8) && p.data_bits.is_multiple_of(8);
        if !valid { self.rejected += 1; return vec![255; rx]; }
        self.panel.dc = self.dc == Some(true);
        if self.panel.dc && matches!(self.command, 0x2c | 0x3c) && self.panel.colmod & 7 != 5 {
            self.rejected += 1; return vec![255; rx];
        }
        for &value in tx {
            if !self.panel.dc { self.command = value; }
            self.panel.byte(value);
        }
        self.version += 1;
        vec![255; rx]
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if !self.attached {
            self.i2c.attach(0x51, Box::new(Pcf85063::new(self.rtc.clone())));
            self.i2c.attach(0x6b, Box::new(Qmi8658Port::new(self.qmi.clone())));
            self.attached = true;
        }
        (0..2).flat_map(|bus| self.i2c.controller_devices(Some((11, 10)))
            .into_iter().map(move |(address, device)| (bus, address, device))).collect()
    }
    fn advance_to(&mut self, cycle: u64) {
        self.cycle = cycle;
        self.motion.cycle.store(cycle, Ordering::Relaxed);
        self.rtc.lock().expect("LCD169 RTC").advance_to(cycle);
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn input_levels(&self) -> Vec<(u8, bool)> {
        vec![(0, true), (self.power_key(), true), (11, self.levels[0]), (10, self.levels[1])]
    }
    fn named_pin(&self, name: &str) -> Option<u8> {
        match name { "boot" => Some(0), "power" => Some(self.power_key()), _ => None }
    }
    fn set_imu_motion(&mut self, mode: u32) -> bool { self.qmi.borrow_mut().set_motion_mode(mode) }
    fn set_imu_sample(&mut self, a: [f64; 3], g: [f64; 3]) -> bool { self.qmi.borrow_mut().set_host_sample(a, g) }
    fn imu_motion(&self) -> u32 { self.motion.mode.load(Ordering::Relaxed) }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let mut frame = vec![0; 240 * 280];
        if self.reset && self.light > 0 && self.panel.on && !self.panel.sleeping {
            for y in 0..280 {
                for x in 0..240 {
                    // Physical 240x280 glass crops rows20..299 of 240x320 RAM.
                    // MADCTL address remapping is handled once by DcsPanel.
                    let mut pixel = self.panel.gram[(y + 20) * 240 + x];
                    if self.panel.madctl & 8 != 0 { pixel = (pixel & 0x7e0) | (pixel >> 11) | ((pixel & 31) << 11); }
                    let p = u32::from(pixel); let d = self.light;
                    frame[y * 240 + x] = ((((p >> 11) * d / 65535) << 11)
                        | ((((p >> 5) & 63) * d / 65535) << 5) | ((p & 31) * d / 65535)) as u16;
                }
            }
        }
        Some((240, 280, frame, self.display_version()))
    }
    fn display_version(&self) -> u64 { self.version }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn report(&self) -> String {
        format!("{} ST7789V2 RGB565 RAM240x320 glass240x280 rows20..299; SPI CLK6/MOSI7/CS5/DC4/RST8 BL15high; SDA11/SCL10 PCF85063@51 QMI8658C@6b; BOOT0 SYS_OUT{}active-low digital key. SYS_EN{}, buzzer{} and RTC_INT{} identified but analog power latch, buzzer audio and RTC IRQ are not implemented. No touch, TF, RGB LED or AXP2101 endpoint; ETA6098 is an analog charger, not an I2C PMIC. No microphone capture, analog battery, optical pose, RF or complete factory/hardware acceptance. pixels={} rejectedSPI={}", self.name(), self.power_key(), if self.v2 {41} else {35}, if self.v2 {42} else {33}, if self.v2 {39} else {41}, self.panel.pixels_written, self.rejected)
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn route(b: &mut WaveshareLcd169, dc: bool, on: bool) {
        let mut gpio = esp_periph::Gpio::new(); let mut mux = esp_periph::RegRam::new();
        for (pin, high) in [(8, on), (15, on), (4, dc)] {
            gpio.enable |= 1 << pin; if high { gpio.out |= 1 << pin; }
            mux.write(4 + 4 * pin, 1 << 12);
        }
        b.gpio_waveform_at(b.cycle, esp_soc::pins::ChipPins::S3.routes(&gpio, &mux));
        b.gpio_pwm_at(b.cycle, &|_| None);
    }
    fn pins(n: usize) -> SpiQspiPins {
        SpiQspiPins { sclk: 1 << 6, data: [1 << 7, 0, 0, 0], cs: 1 << 5,
            data_width: 1, data_bits: n as u32 * 8, ..Default::default() }
    }
    fn command(b: &mut WaveshareLcd169, c: u8, args: &[u8]) {
        route(b, false, true); b.spi_transfer_qspi_pins(3, pins(1), &[c], 0);
        route(b, true, true); b.spi_transfer_qspi_pins(3, pins(args.len()), args, 0);
    }
    #[test] fn exact_gap_reset_pwm_wire_routes_and_non_touch_topology() {
        for v2 in [false, true] {
            let mut b = WaveshareLcd169::new(v2);
            command(&mut b, 0x11, &[]); command(&mut b, 0x29, &[]); command(&mut b, 0x3a, &[0x55]);
            command(&mut b, 0x2a, &[0, 0, 0, 0]); command(&mut b, 0x2b, &[0, 20, 0, 20]);
            command(&mut b, 0x2c, &[0xf8, 0]); assert_eq!(b.display().unwrap().2[0], 0xf800);
            command(&mut b, 0x2b, &[0, 0, 0, 0]); command(&mut b, 0x2c, &[0x07, 0xe0]);
            assert_eq!(b.display().unwrap().2[0], 0xf800, "hidden RAM rows do not move the optical window");
            let before = b.panel.pixels_written;
            b.spi_transfer_qspi_pins(3, SpiQspiPins { data: [1 << 8, 0, 0, 0], ..pins(2) }, &[0, 0], 0);
            assert_eq!(b.panel.pixels_written, before);
            b.gpio_pwm_at(1, &|pin| (pin == 15).then_some((10_000.0, 32768)));
            assert_eq!(b.display().unwrap().2[0], 15 << 11, "duty is the second PWM tuple field");
            b.gpio_pwm_at(2, &|pin| (pin == 15).then_some((10_000.0, 0)));
            assert!(b.display().unwrap().2.iter().all(|p| *p == 0));
            let ds = b.i2c_devices();
            assert_eq!(ds.len(), 4); assert!(ds.iter().all(|(_, a, d)| [0x51, 0x6b].contains(a) && d.pins() == Some((11, 10))));
            assert!(b.sd_card_slot().is_none()); assert!(b.leds().is_none());
            assert_eq!(b.named_pin("power"), Some(if v2 {40} else {36}));
            route(&mut b, true, false); assert!(b.display().unwrap().2.iter().all(|p| *p == 0));
            assert!(b.panel.sleeping && !b.panel.on);
        }
    }
}
