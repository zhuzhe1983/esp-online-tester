//! Individually sourced Waveshare AMOLED watch boards. This file owns their
//! physical pad, reset and supply wiring; protocol devices stay opt-in.
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, atomic::Ordering}};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardEdge, BoardModel, SpiQspiPins, VirtualCycle}, pins::PinRoutes,
    devices::{axp2101::{Axp2101, Axp2101Power, PowerRail}, es7210::Es7210,
        es8311::Es8311, ft3168::{Ft3168, Ft3168State}, i2c_bitbang::I2cBitBang,
        imu_motion::ImuMotion, pcf85063::{Pcf85063, Pcf85063State},
        qmi8658::{Qmi8658, Qmi8658Port}, qspi_dcs::QspiDcsPanel}};

const HZ: u64 = 240_000_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmoledWatchVariant { TouchAmoled206 }
impl AmoledWatchVariant {
    pub fn name(self) -> &'static str { "waveshare-s3-touch-amoled206" }
    pub fn sku(self) -> &'static str { "ESP32-S3-Touch-AMOLED-2.06" }
    pub fn flash_mib(self) -> usize { 32 }
    pub fn psram_mib(self) -> usize { 8 }
}

struct Rails { main: bool, adc: bool, main_generation: u64, adc_generation: u64 }
impl Default for Rails {
    fn default() -> Self { Self { main: true, adc: true, main_generation: 0, adc_generation: 0 } }
}
struct RailPort {
    rails: Arc<Mutex<Rails>>, adc: bool, generation: u64,
    device: Box<dyn I2cDevice>, create: Option<fn() -> Box<dyn I2cDevice>>,
}
impl RailPort {
    fn powered(&mut self) -> bool {
        let r = self.rails.lock().expect("AMOLED rails");
        let (on, generation) = if self.adc { (r.adc, r.adc_generation) } else { (r.main, r.main_generation) };
        if generation != self.generation {
            if let Some(create) = self.create { self.device = create(); }
            else { self.device.stop(); }
            self.generation = generation;
        }
        on
    }
}
impl I2cDevice for RailPort {
    fn start(&mut self, read: bool) -> bool { self.powered() && self.device.start(read) }
    fn write(&mut self, b: u8) -> bool { self.powered() && self.device.write(b) }
    fn read(&mut self) -> u8 { if self.powered() { self.device.read() } else { 255 } }
    fn stop(&mut self) { self.device.stop(); }
}

pub struct WaveshareAmoledWatch {
    variant: AmoledWatchVariant, panel: QspiDcsPanel,
    power: Arc<Mutex<Axp2101Power>>, rails: Arc<Mutex<Rails>>,
    touch: Arc<Mutex<Ft3168State>>, rtc: Arc<Mutex<Pcf85063State>>,
    imu: Arc<ImuMotion>, qmi: Rc<RefCell<Qmi8658>>,
    i2c: I2cBitBang, attached: bool, cycle: u64,
    lcd_reset: bool, touch_reset: bool, software_selected: bool,
    sd_present: bool, sd_dat3_high: bool, gpio_events: u64,
    levels: [bool; 3], edges: Vec<BoardEdge>, last_reject: Option<String>, trace: bool,
}
impl WaveshareAmoledWatch {
    pub fn new(variant: AmoledWatchVariant) -> Self {
        let imu = Arc::new(ImuMotion::default());
        let qmi = Rc::new(RefCell::new(Qmi8658::new_c(imu.clone(), HZ)));
        Self { variant, panel: QspiDcsPanel::co5300(410, 502, 22, 0),
            power: Default::default(), rails: Default::default(),
            touch: Arc::new(Mutex::new(Ft3168State::new(HZ))),
            rtc: Arc::new(Mutex::new(Pcf85063State::with_averaged_offset(HZ))), imu, qmi,
            i2c: I2cBitBang::new(), attached: false, cycle: 0,
            lcd_reset: false, touch_reset: false, software_selected: false,
            sd_present: false, sd_dat3_high: true, gpio_events: 0,
            levels: [true; 3], edges: Vec::new(), last_reject: None,
            trace: std::env::var_os("ESP32SIM_WAVESHARE_QSPI_TRACE").is_some() }
    }
    fn main_power(&self) -> bool {
        self.power.lock().expect("AMOLED power").output_mv(PowerRail::Dcdc1) == Some(3300)
    }
    fn sync_power(&mut self) {
        let p = self.power.lock().expect("AMOLED power");
        // The schematic powers display, touch, IMU, SD and ES8311 digital I/O
        // from DCDC1/VCC3V3. ALDO2 is DSI_PWR_EN, not QSPI glass supply.
        let main = p.output_mv(PowerRail::Dcdc1) == Some(3300);
        // ES7210 VDDD/VDDP/VDDA/VDDM are A3V3 (ALDO1). Its Rev21 p9
        // recommended supply range is 1.6..3.6 V. This is digital availability;
        // I/O voltage compatibility, PCM sample rate and analog audio are not
        // validated by accepting an in-range programmed supply.
        let adc = p.output_mv(PowerRail::Aldo1).is_some_and(|mv| (1600..=3600).contains(&mv));
        drop(p);
        let mut r = self.rails.lock().expect("AMOLED rails");
        if main != r.main {
            r.main = main; r.main_generation += 1; self.panel.reset();
            *self.qmi.borrow_mut() = Qmi8658::new_c(self.imu.clone(), HZ);
        }
        if adc != r.adc { r.adc = adc; r.adc_generation += 1; }
        drop(r);
        self.touch.lock().expect("AMOLED touch").hardware_at(self.cycle, main, self.touch_reset);
    }
    fn sync_irq(&mut self) {
        let high = self.touch.lock().expect("AMOLED touch").irq_high();
        if high != self.levels[2] {
            self.levels[2] = high; self.edges.push(BoardEdge { cycle: self.cycle, pin: 38, level: high });
        }
    }
    fn gated(&self, adc: bool, device: Box<dyn I2cDevice>, create: Option<fn() -> Box<dyn I2cDevice>>) -> Box<dyn I2cDevice> {
        Box::new(RailPort { rails: self.rails.clone(), adc, generation: 0, device, create })
    }
}
impl BoardModel for WaveshareAmoledWatch {
    fn name(&self) -> &'static str { self.variant.name() }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) { self.gpio_events += changes.len() as u64; }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn gpio_waveform_at(&mut self, cycle: VirtualCycle, r: PinRoutes<'_>) {
        self.advance_to(cycle);
        let reset = r.software_output(8) == Some(true);
        if reset != self.lcd_reset { if !reset { self.panel.reset(); } self.lcd_reset = reset; self.panel.changes += 1; }
        self.touch_reset = r.software_output(9) == Some(true);
        let selected = r.software_output(12) == Some(false);
        if self.software_selected && !selected { self.panel.deselect(); }
        self.software_selected = selected;
        // R3..R6 slot pull-ups: native SD leaves DAT3/GPIO17 high. This is a
        // mode gate, not a fourth data lane or a physical card-power switch.
        self.sd_dat3_high = r.software_output(17).unwrap_or(true);
        self.sync_power();
        let scl = r.software_output(14).unwrap_or(true);
        let sda = self.i2c.drive(scl, r.software_output(15).unwrap_or(true));
        for (i, (pin, level)) in [(15, sda), (14, scl)].into_iter().enumerate() {
            if level != self.levels[i] { self.levels[i] = level; self.edges.push(BoardEdge { cycle, pin, level }); }
        }
        self.sync_irq();
    }
    fn uses_qspi_pins(&self) -> bool { true }
    fn spi_transfer_qspi_pins(&mut self, _host: u8, p: SpiQspiPins, bytes: &[u8], rx_len: usize) -> Vec<u8> {
        if p.cs & (1 << 12) == 0 { return vec![255; rx_len]; }
        self.sync_power();
        let quad = [p.command_width, p.address_width, p.data_width].contains(&4);
        let routed = p.sclk & (1 << 11) != 0 && p.data[0] & (1 << 4) != 0
            && (!quad || [5, 6, 7].into_iter().enumerate().all(|(i, pin)| p.data[i + 1] & (1 << pin) != 0));
        let accepted = if !routed || !self.main_power() || !self.lcd_reset || rx_len != 0 {
            self.panel.reject_until_deselect(); false
        } else { self.panel.transfer(p, bytes) };
        if !accepted {
            let detail = format!("cycle={} routed={routed} reset={} main={} phases={}/{}/{} bits={}/{}/{} keep={} first={:02x?}",
                self.cycle, self.lcd_reset, self.main_power(), p.command_width, p.address_width,
                p.data_width, p.command_bits, p.address_bits, p.data_bits, p.keep_cs, &bytes[..bytes.len().min(8)]);
            if self.trace && self.panel.rejected <= 8 { eprintln!("AMOLED watch QSPI reject {detail}"); }
            self.last_reject = Some(detail);
        }
        if !p.keep_cs && p.software_cs & (1 << 12) == 0 { self.panel.deselect(); }
        vec![255; rx_len]
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if !self.attached {
            self.i2c.attach(0x34, Box::new(Axp2101::new(self.power.clone())));
            self.i2c.attach(0x38, Box::new(Ft3168::new(self.touch.clone())));
            self.i2c.attach(Qmi8658::i2c_address(false), self.gated(false, Box::new(Qmi8658Port::new(self.qmi.clone())), None));
            // AXP RTCLDO is the RTC supply. In this ideal USB / no battery
            // scenario it remains available; no host wall clock is injected.
            self.i2c.attach(0x51, Box::new(Pcf85063::new(self.rtc.clone())));
            self.i2c.attach(0x18, self.gated(false, Box::new(Es8311::new()), Some(|| Box::new(Es8311::new()))));
            self.i2c.attach(0x40, self.gated(true, Box::new(Es7210::new()), Some(|| Box::new(Es7210::new()))));
            self.attached = true;
        }
        (0..2).flat_map(|bus| self.i2c.controller_devices(Some((15, 14))).into_iter().map(move |(a, d)| (bus, a, d))).collect()
    }
    fn advance_to(&mut self, cycle: VirtualCycle) {
        assert!(cycle >= self.cycle, "AMOLED watch time moved backwards");
        self.cycle = cycle; self.imu.cycle.store(cycle, Ordering::Relaxed);
        self.rtc.lock().expect("AMOLED RTC").advance_to(cycle);
        self.sync_power(); self.sync_irq();
    }
    fn next_deadline(&self) -> Option<VirtualCycle> { self.touch.lock().expect("AMOLED touch").next_deadline() }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        self.touch.lock().expect("AMOLED touch").touch_at(self.cycle, x.min(409), y.min(501), down);
        self.sync_irq();
    }
    fn touch_at(&mut self, cycle: VirtualCycle, x: u16, y: u16, down: bool) { self.advance_to(cycle); self.touch(x, y, down); }
    fn set_imu_motion(&mut self, mode: u32) -> bool { self.qmi.borrow_mut().set_motion_mode(mode) }
    fn set_imu_sample(&mut self, a: [f64; 3], g: [f64; 3]) -> bool { self.qmi.borrow_mut().set_host_sample(a, g) }
    fn imu_motion(&self) -> u32 { self.imu.mode.load(Ordering::Relaxed) }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(0, true), (10, false), (15, self.levels[0]), (14, self.levels[1]), (38, self.levels[2])] }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn named_pin(&self, name: &str) -> Option<u8> { match name { "boot" | "button" => Some(0), "power" | "pwr" => Some(10), _ => None } }
    fn sd_card_slot(&self) -> Option<([u8; 6], bool)> {
        Some(([2, 1, 3, 255, 255, 255], self.sd_present && self.main_power() && self.sd_dat3_high))
    }
    fn set_sd_card_present(&mut self, present: bool) { self.sd_present = present; }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        Some((410, 502, if self.main_power() && self.lcd_reset { self.panel.frame() } else { vec![0; 410 * 502] }, self.display_version()))
    }
    fn display_version(&self) -> u64 { self.panel.version() }
    fn display_frames(&self) -> u64 { self.panel.frames() }
    fn report(&self) -> String {
        let t = self.touch.lock().expect("AMOLED touch");
        let p = self.power.lock().expect("AMOLED power");
        format!("{}: physical32F8P CO5300 QSPI410x502 gap22/0 CLK11 D4/Q5/WP6/HD7 CS12 RESET8={} RAMWR={} pixels={} commands={} opaqueCommands={} opaqueBytes={} rejected={} window={:?} madctl={:02x} state={:?}; FT3168@38 I2C15/14 RESET9={} IRQ38 reports={} contacts={} hostUpdates={} configurationWrites={}; AXP2101@34 DCDC1={:?} ALDO1={:?} ALDO2={:?}; QMI8658C@6b SA0=GND hostSample={} RTC=PCF85063A@51 averagedOffset={:?}; ES8311@18 VCC3V3 ES7210@40 A3V3; SDMMC1bit CLK2/CMD1/D0=3 inserted={} DAT3GPIO17={} BOOT0-low/PWR10-high. lastReject={:?}. Physical board EFUSE unknown: AXP Table6-1 reference startup; ideal USB5V/no battery. Digital supply availability only, no analog I/O voltage compatibility, power ramps/CPU poweroff, charging, PCM/RF, QMI IRQ/calibration, RTC correction pulses/alarm/clockout, TE waveform, FT identity/gestures or calibrated glass/IMU pose. PWR10 is the MCU sense input, not an AXP poweroff event.",
            self.variant.sku(), self.lcd_reset, self.panel.frames(), self.panel.pixels_written(), self.panel.commands,
            self.panel.opaque_commands, self.panel.opaque_bytes, self.panel.rejected, self.panel.window(), self.panel.madctl, self.panel.state(),
            self.touch_reset, t.reports, t.contact_reports, t.host_updates, t.configuration_writes,
            p.output_mv(PowerRail::Dcdc1), p.output_mv(PowerRail::Aldo1), p.output_mv(PowerRail::Aldo2),
            self.qmi.borrow().host_sample_active(), self.rtc.lock().expect("AMOLED RTC").programmed_offset(), self.sd_present, self.sd_dat3_high, self.last_reject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn route(b: &mut WaveshareAmoledWatch, lcd: bool, touch: bool, dat3: bool) {
        let mut gpio = esp_periph::Gpio::new(); let mut mux = esp_periph::RegRam::new();
        for (pin, high) in [(8u8, lcd), (9, touch), (17, dat3)] {
            gpio.enable |= 1 << pin; if high { gpio.out |= 1 << pin; }
            gpio.func_out_sel[pin as usize] = 256; mux.write(4 + 4 * u32::from(pin), 1 << 12);
        }
        b.gpio_waveform_at(b.cycle, esp_soc::pins::ChipPins::S3.routes(&gpio, &mux));
    }
    fn write(d: &mut dyn I2cDevice, bytes: &[u8]) {
        assert!(d.start(false)); for b in bytes { assert!(d.write(*b), "I2C write {bytes:02x?}"); } d.stop();
    }
    fn read(d: &mut dyn I2cDevice, reg: u8, n: usize) -> Vec<u8> {
        assert!(d.start(false)); assert!(d.write(reg)); assert!(d.start(true));
        let out = (0..n).map(|_| d.read()).collect(); d.stop(); out
    }
    fn pins(n: usize, width: u8, keep: bool) -> SpiQspiPins {
        SpiQspiPins { sclk: 1 << 11, data: [1 << 4, 1 << 5, 1 << 6, 1 << 7], cs: 1 << 12, hardware_cs: 1 << 12,
            data_width: width, data_bits: n as u32 * 8, keep_cs: keep, ..Default::default() }
    }
    fn command(b: &mut WaveshareAmoledWatch, cmd: u8, data: &[u8]) {
        b.spi_transfer_qspi_pins(2, pins(4, 1, !data.is_empty()), &[2, 0, cmd, 0], 0);
        if !data.is_empty() { b.spi_transfer_qspi_pins(2, pins(data.len(), 1, false), data, 0); }
    }
    #[test]
    fn exact_qspi_window_lanes_and_dcdc1_gate_do_not_depend_on_dsi_aldo2() {
        let mut b = WaveshareAmoledWatch::new(AmoledWatchVariant::TouchAmoled206);
        route(&mut b, true, true, true);
        command(&mut b, 0x3a, &[0x55]); command(&mut b, 0x11, &[]); command(&mut b, 0x29, &[]);
        command(&mut b, 0x53, &[0x20]); command(&mut b, 0x51, &[255]);
        command(&mut b, 0x2a, &[0, 22, 0, 22]); command(&mut b, 0x2b, &[0, 0, 0, 0]);
        b.spi_transfer_qspi_pins(2, pins(4, 1, true), &[0x32, 0, 0x2c, 0], 0);
        b.spi_transfer_qspi_pins(2, pins(2, 4, false), &[0xf8, 0], 0);
        assert_eq!(b.display().unwrap().2[0], 0xf800);
        assert_eq!(b.power.lock().unwrap().output_mv(PowerRail::Aldo2), None);
        let before = b.panel.pixels_written();
        for i in 0..4 {
            b.spi_transfer_qspi_pins(2, pins(4, 1, true), &[0x32, 0, 0x2c, 0], 0);
            let mut bad = pins(2, 4, true); bad.data[i] = 1 << 20;
            b.spi_transfer_qspi_pins(2, bad, &[0, 0], 0);
            b.spi_transfer_qspi_pins(2, pins(2, 4, false), &[0, 0], 0);
            assert_eq!(b.panel.pixels_written(), before);
        }
        let mut axp = Axp2101::new(b.power.clone()); write(&mut axp, &[0x80, 0x0e]); b.advance_to(1);
        assert!(b.display().unwrap().2.iter().all(|v| *v == 0));
        write(&mut axp, &[0x80, 0x0f]); b.advance_to(2); assert!(!b.panel.visible());
    }
    #[test]
    fn shared_i2c_registers_touch_reset_and_native_sd_mode_follow_actual_pins() {
        let mut b = WaveshareAmoledWatch::new(AmoledWatchVariant::TouchAmoled206); route(&mut b, true, true, true);
        let mut devices = b.i2c_devices();
        let pick = |bus, addr| devices.iter().position(|(c, a, _)| *c == bus && *a == addr).unwrap();
        let (pmu, t0, t1, qmi) = (pick(0, 0x34), pick(0, 0x38), pick(1, 0x38), pick(1, 0x6b));
        assert_eq!(devices[t0].2.pins(), Some((15, 14)));
        assert!(!devices.iter().any(|(_, a, _)| *a == 0x6a));
        write(devices[t0].2.as_mut(), &[0x80, 70]); assert_eq!(read(devices[t1].2.as_mut(), 0x80, 1), [70]);
        b.touch_at(80 * HZ / 1000, 123, 456, true);
        assert_eq!(read(devices[t1].2.as_mut(), 2, 5), [1, 0, 123, 1, 200]);
        assert_eq!(read(devices[qmi].2.as_mut(), 0, 1), [5]);
        write(devices[pmu].2.as_mut(), &[0x80, 0x0e]); b.advance_to(b.cycle + 1);
        assert!(!devices[t0].2.start(false)); assert!(!devices[qmi].2.start(false));
        b.set_sd_card_present(true); assert_eq!(b.sd_card_slot(), Some(([2, 1, 3, 255, 255, 255], false)));
        write(devices[pmu].2.as_mut(), &[0x80, 0x0f]); b.advance_to(b.cycle + 1);
        assert_eq!(b.sd_card_slot(), Some(([2, 1, 3, 255, 255, 255], true)));
        route(&mut b, true, true, false); assert!(!b.sd_card_slot().unwrap().1);
        route(&mut b, true, false, true); assert!(!devices[t0].2.start(false));
        assert_eq!((b.named_pin("boot"), b.named_pin("power")), (Some(0), Some(10)));
    }

    #[test]
    #[ignore = "fixed original 20261016 factory, S3 mask ROM and 64MiB FAT32 image required"]
    fn external_amoled206_factory() {
        use esp_soc::{LoadKind, SocBus, Stop};
        let source = std::env::var("ESP32SIM_WAVESHARE_AMOLED_SOURCES").expect("fixed original source cache");
        let rom = std::env::var("ESP32SIM_ROM_DIR").expect("S3 mask ROM directory");
        let v = AmoledWatchVariant::TouchAmoled206;
        let mut m = crate::machine([2, 0, 0, 0, 0, 1]); m.bus.board = Box::new(WaveshareAmoledWatch::new(v)); m.bus.attach_board_devices();
        m.bus.set_flash_size(v.flash_mib() << 20); m.bus.set_psram_size(v.psram_mib() << 20).unwrap();
        m.console.capture = true; m.console.mask = 2;
        m.load_input(LoadKind::Rom, &std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
        m.load_input(LoadKind::Flash, &std::fs::read(format!("{source}/{}/firmware/ESP32-S3-Touch-AMOLED-2.06_FactoryOnly_20261016.bin", v.sku())).unwrap()).unwrap();
        if let Ok(image) = std::env::var("ESP32SIM_WAVESHARE_SD_IMAGE") { m.load_input(LoadKind::SdCard, &std::fs::read(image).unwrap()).unwrap(); }
        m.boot_rom();
        for ms in [1000, 2000, 4000, 8000] {
            m.max_cycles = ms * HZ / 1000; let stop = m.run(u64::MAX); m.bus.refresh_board_outputs();
            eprintln!("{ms}ms stop={stop:?} reboots={} {}", m.reboots, m.bus.board.report());
            if let Ok(root) = std::env::var("ESP32SIM_WAVESHARE_AMOLED_EVIDENCE") {
                let p = std::path::Path::new(&root).join(v.sku()); std::fs::create_dir_all(&p).unwrap();
                m.write_tft_png(p.join(format!("boot-{ms}.png")).to_str().unwrap(), 1).unwrap();
                let (w, h, pixels, version) = m.bus.board.display().unwrap();
                std::fs::write(p.join(format!("boot-{ms}.rgb565")), pixels.iter().flat_map(|p| p.to_le_bytes()).collect::<Vec<_>>()).unwrap();
                std::fs::write(p.join(format!("boot-{ms}.uart.txt")), &m.console.all).unwrap();
                std::fs::write(p.join(format!("boot-{ms}.board.txt")), m.bus.board.report()).unwrap();
                let s = &m.bus.periph.sdmmc.stats;
                std::fs::write(p.join(format!("boot-{ms}.state.json")), format!("{{\"guestCycles\":{},\"guestMilliseconds\":{},\"reboots\":{},\"width\":{w},\"height\":{h},\"version\":{version},\"ramwrCommands\":{},\"pc\":[{},{}],\"sdmmcCommands\":{},\"sdmmcTimeouts\":{},\"sdBytesRead\":{},\"sdBytesWritten\":{}}}\n", m.bus.cycles, m.bus.cycles / (HZ / 1000), m.reboots, m.bus.board.display_frames(), m.cores[0].pc, m.cores[1].pc, s.commands, s.timeouts, s.bytes_read, s.bytes_written)).unwrap();
            }
            assert!(matches!(stop, Stop::Halted));
        }
        assert_eq!(m.reboots, 0); assert!(m.bus.board.display_frames() > 0);
        assert!(m.bus.board.display().unwrap().2.iter().any(|p| *p != 0), "factory has not exposed a visible image");
    }
}
