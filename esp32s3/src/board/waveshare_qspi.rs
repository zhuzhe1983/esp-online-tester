//! Exact, opt-in Waveshare QSPI boards. Each profile is tied to its own pinned
//! schematic/BSP and original image; sources and limits accompany each board.
use std::sync::{Arc, Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardEdge, BoardModel, SpiQspiPins, VirtualCycle}, pins::PinRoutes,
    devices::{i2c_bitbang::I2cBitBang, qspi_dcs::QspiDcsPanel,
        es8311::Es8311, es7210::Es7210}};

const HZ: u64 = 240_000_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QspiVariant { TouchAmoled143C }
impl QspiVariant {
    pub fn name(self) -> &'static str { match self { Self::TouchAmoled143C => "waveshare-s3-touch-amoled143c" } }
    pub fn sku(self) -> &'static str { match self { Self::TouchAmoled143C => "ESP32-S3-Touch-AMOLED-1.43C" } }
    pub fn flash_mib(self) -> usize { 8 }
    pub fn psram_mib(self) -> usize { 8 }
}

#[derive(Default)]
struct Cst820State {
    reset_high: bool, down: bool, x: u16, y: u16,
    pointer: u8, first: bool, valid: bool, irq_until: u64,
    reads: u64, contact_reads: u64, updates: u64,
}
impl Cst820State {
    fn reset(&mut self, high: bool) {
        let (reads, contact_reads, updates) = (self.reads, self.contact_reads, self.updates);
        *self = Self { reset_high: high, reads, contact_reads, updates, ..Default::default() };
    }
}
struct Cst820(Arc<Mutex<Cst820State>>);
impl I2cDevice for Cst820 {
    fn start(&mut self, read: bool) -> bool {
        let mut s = self.0.lock().expect("1.43C CST820");
        if !s.reset_high { return false; }
        if !read { s.first = true; s.valid = false; true } else { s.valid }
    }
    fn write(&mut self, b: u8) -> bool {
        let mut s = self.0.lock().expect("1.43C CST820");
        if !s.reset_high || !s.first { return false; }
        s.first = false; s.pointer = b; s.valid = (2..=6).contains(&b); s.valid
    }
    fn read(&mut self) -> u8 {
        let mut s = self.0.lock().expect("1.43C CST820");
        if !s.reset_high || !s.valid { return 255; }
        let value = match s.pointer {
            2 => { s.reads += 1; s.contact_reads += u64::from(s.down); u8::from(s.down) },
            3 => (s.x >> 8) as u8 & 15, 4 => s.x as u8,
            5 => (s.y >> 8) as u8 & 15, 6 => s.y as u8,
            _ => 255,
        };
        s.pointer = s.pointer.wrapping_add(1); s.valid = (2..=6).contains(&s.pointer); value
    }
}

struct CodecPower { enabled: bool, generation: u64 }
impl Default for CodecPower {
    fn default() -> Self {
        // Schematic R18 pulls U4 RT9193 EN / CODEC_EN to VCC3V3. A floating
        // GPIO18 therefore leaves the codec rail on, rather than off.
        Self { enabled: true, generation: 0 }
    }
}
struct GatedCodec {
    power: Arc<Mutex<CodecPower>>, generation: u64,
    device: Box<dyn I2cDevice>, create: fn() -> Box<dyn I2cDevice>,
}
impl GatedCodec {
    fn new(power: Arc<Mutex<CodecPower>>, create: fn() -> Box<dyn I2cDevice>) -> Self {
        Self { power, generation: 0, device: create(), create }
    }
    fn powered(&mut self) -> bool {
        let p = self.power.lock().expect("1.43C audio power");
        if p.generation != self.generation { self.device = (self.create)(); self.generation = p.generation; }
        p.enabled
    }
}
impl I2cDevice for GatedCodec {
    fn start(&mut self, read: bool) -> bool { self.powered() && self.device.start(read) }
    fn write(&mut self, b: u8) -> bool { self.powered() && self.device.write(b) }
    fn read(&mut self) -> u8 { if self.powered() { self.device.read() } else { 255 } }
    fn stop(&mut self) { self.device.stop(); }
}

pub struct WaveshareQspi {
    variant: QspiVariant, panel: QspiDcsPanel,
    i2c: I2cBitBang, attached: bool, touch: Arc<Mutex<Cst820State>>, audio: Arc<Mutex<CodecPower>>,
    cycle: u64, reset_high: bool, software_selected: bool, gpio_events: u64,
    levels: [bool; 3], edges: Vec<BoardEdge>, last_reject: Option<String>, trace: bool,
}
impl WaveshareQspi {
    pub fn new(variant: QspiVariant) -> Self {
        Self { variant, panel: QspiDcsPanel::co5300(466, 466, 8, 0xc0),
            i2c: I2cBitBang::new(), attached: false, touch: Default::default(), audio: Default::default(),
            cycle: 0, reset_high: false, software_selected: false, gpio_events: 0,
            levels: [true; 3], edges: Vec::new(), last_reject: None,
            trace: std::env::var_os("ESP32SIM_WAVESHARE_QSPI_TRACE").is_some() }
    }
    fn sync_irq(&mut self) {
        let s = self.touch.lock().expect("1.43C CST820");
        let high = !s.reset_high || self.cycle >= s.irq_until;
        if high != self.levels[2] { self.levels[2] = high; self.edges.push(BoardEdge { cycle: self.cycle, pin: 17, level: high }); }
    }
}
impl BoardModel for WaveshareQspi {
    fn name(&self) -> &'static str { self.variant.name() }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) { self.gpio_events += changes.len() as u64; }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn gpio_waveform_at(&mut self, cycle: VirtualCycle, r: PinRoutes<'_>) {
        self.advance_to(cycle);
        let reset = r.software_output(13) == Some(true);
        if reset != self.reset_high {
            if !reset { self.panel.reset(); }
            self.reset_high = reset; self.panel.changes += 1;
        }
        let selected = r.software_output(15) == Some(false);
        if self.software_selected && !selected { self.panel.deselect(); }
        self.software_selected = selected;
        let touch_reset = r.software_output(16) == Some(true);
        { let mut s = self.touch.lock().expect("1.43C CST820"); if touch_reset != s.reset_high { s.reset(touch_reset); } }
        { let mut a = self.audio.lock().expect("1.43C audio power");
            let enabled = r.software_output(18).unwrap_or(true);
            if enabled != a.enabled { a.enabled = enabled; a.generation += 1; }
        }
        let scl = r.software_output(48).unwrap_or(true);
        let sda = self.i2c.drive(scl, r.software_output(47).unwrap_or(true));
        for (i, (pin, level)) in [(47, sda), (48, scl)].into_iter().enumerate() {
            if level != self.levels[i] { self.levels[i] = level; self.edges.push(BoardEdge { cycle, pin, level }); }
        }
        self.sync_irq();
    }
    fn uses_qspi_pins(&self) -> bool { true }
    fn spi_transfer_qspi_pins(&mut self, _host: u8, p: SpiQspiPins, bytes: &[u8], rx_len: usize) -> Vec<u8> {
        if p.cs & (1 << 15) == 0 { return vec![255; rx_len]; }
        let quad = [p.command_width, p.address_width, p.data_width].contains(&4);
        let routed = p.sclk & (1 << 14) != 0 && p.data[0] & (1 << 9) != 0
            && (!quad || [10, 11, 12].into_iter().enumerate().all(|(i, pin)| p.data[i + 1] & (1 << pin) != 0));
        let accepted = if !routed || !self.reset_high || rx_len != 0 {
            self.panel.reject_until_deselect(); false
        } else { self.panel.transfer(p, bytes) };
        if !accepted {
            let detail = format!("cycle={} routed={routed} reset={} phases={}/{}/{} bits={}/{}/{} keep={} softwareCS={} mode={p:?} first={:02x?}",
                self.cycle, self.reset_high, p.command_width, p.address_width, p.data_width,
                p.command_bits, p.address_bits, p.data_bits, p.keep_cs, p.software_cs & (1 << 15) != 0, &bytes[..bytes.len().min(8)]);
            if self.trace && self.panel.rejected <= 8 { eprintln!("QSPI reject: {detail}"); }
            self.last_reject = Some(detail);
        }
        if !p.keep_cs && p.software_cs & (1 << 15) == 0 { self.panel.deselect(); }
        vec![255; rx_len]
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if !self.attached {
            self.i2c.attach(0x15, Box::new(Cst820(self.touch.clone())));
            self.i2c.attach(0x18, Box::new(GatedCodec::new(self.audio.clone(), || Box::new(Es8311::new()))));
            self.i2c.attach(0x40, Box::new(GatedCodec::new(self.audio.clone(), || Box::new(Es7210::new()))));
            self.attached = true;
        }
        (0..2).flat_map(|bus| self.i2c.controller_devices(Some((47, 48))).into_iter().map(move |(a, d)| (bus, a, d))).collect()
    }
    fn advance_to(&mut self, cycle: VirtualCycle) { assert!(cycle >= self.cycle, "QSPI board time moved backwards"); self.cycle = cycle; self.sync_irq(); }
    fn next_deadline(&self) -> Option<VirtualCycle> {
        let s = self.touch.lock().expect("1.43C CST820");
        (s.reset_high && self.cycle < s.irq_until).then_some(s.irq_until)
    }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        let mut s = self.touch.lock().expect("1.43C CST820");
        if !s.reset_high { return; }
        // Fixed BSP process_coordinates inverts both raw CST820 axes. This is
        // the corresponding physical report for our declared display reference.
        let (x, y) = (465 - x.min(465), 465 - y.min(465));
        if (s.x, s.y, s.down) != (x, y, down) {
            // Source specifies a falling edge, not pulse width. One guest ms is
            // an explicit digital input policy, not a measured sensor cadence.
            s.irq_until = self.cycle.saturating_add(HZ / 1000);
        }
        s.x = x; s.y = y; s.down = down; s.updates += 1; drop(s); self.sync_irq();
    }
    fn touch_at(&mut self, cycle: VirtualCycle, x: u16, y: u16, down: bool) { self.advance_to(cycle); self.touch(x, y, down); }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(0, true), (47, self.levels[0]), (48, self.levels[1]), (17, self.levels[2])] }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn named_pin(&self, name: &str) -> Option<u8> { match name { "boot" | "button" => Some(0), _ => None } }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let mut frame = if self.reset_high { self.panel.frame() } else { vec![0; 466 * 466] };
        // Nominal circular active aperture, not a metrology claim about edge pixels.
        for y in 0..466 { for x in 0..466 {
            let dx = 2 * x as i64 + 1 - 466; let dy = 2 * y as i64 + 1 - 466;
            if dx * dx + dy * dy > 466 * 466 { frame[y * 466 + x] = 0; }
        } }
        Some((466, 466, frame, self.display_version()))
    }
    fn display_version(&self) -> u64 { self.panel.version() }
    fn display_frames(&self) -> u64 { self.panel.frames() }
    fn report(&self) -> String {
        let s = self.touch.lock().expect("1.43C CST820");
        let a = self.audio.lock().expect("1.43C audio power");
        format!("{}: CO5300 QSPI466x466 gap8/0 CLK14 D9/Q10/WP11/HD12 CS15 RESET13={} RAMWR={} pixels={} commands={} opaqueCommands={} opaqueBytes={} rejected={} window={:?} madctl={:02x} state={:?}; CST820@15 I2C47/48 RESET16={} IRQ17 reports={} contacts={} hostUpdates={}; ES8311@18/ES7210@40 LDO18={} powerGeneration={} BOOT0 physicalTF=false; lastReject={:?}. RGB565/declared BSP coordinate reference only; no physical glass/PCB calibration, analog brightness/reset settling, TE waveform, private panel voltage/gamma, SPI reads, CST gestures/identity/scan cadence, I2S PCM/codecs analog or RF simulation.",
            self.variant.sku(), self.reset_high, self.panel.frames(), self.panel.pixels_written(), self.panel.commands,
            self.panel.opaque_commands, self.panel.opaque_bytes, self.panel.rejected, self.panel.window(), self.panel.madctl,
            self.panel.state(), s.reset_high, s.reads, s.contact_reads, s.updates, a.enabled, a.generation, self.last_reject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn route(b: &mut WaveshareQspi, lcd: bool, touch: bool, audio: bool, cs: Option<bool>) {
        let mut gpio = esp_periph::Gpio::new(); let mut mux = esp_periph::RegRam::new();
        for (pin, value) in [(13u8, Some(lcd)), (16, Some(touch)), (18, Some(audio)), (15, cs)] {
            if let Some(high) = value {
                gpio.enable |= 1 << pin; if high { gpio.out |= 1 << pin; }
                gpio.func_out_sel[pin as usize] = 256; mux.write(4 + 4 * u32::from(pin), 1 << 12);
            }
        }
        b.gpio_waveform_at(b.cycle, esp_soc::pins::ChipPins::S3.routes(&gpio, &mux));
    }
    fn pins(n: usize, width: u8, keep: bool) -> SpiQspiPins {
        SpiQspiPins { sclk: 1 << 14, data: [1 << 9, 1 << 10, 1 << 11, 1 << 12],
            cs: 1 << 15, hardware_cs: 1 << 15, data_width: width, data_bits: n as u32 * 8, keep_cs: keep, ..Default::default() }
    }
    fn command(b: &mut WaveshareQspi, cmd: u8, data: &[u8]) {
        b.spi_transfer_qspi_pins(2, pins(4, 1, !data.is_empty()), &[2, 0, cmd, 0], 0);
        if !data.is_empty() { b.spi_transfer_qspi_pins(2, pins(data.len(), 1, false), data, 0); }
    }
    fn ready() -> WaveshareQspi {
        let mut b = WaveshareQspi::new(QspiVariant::TouchAmoled143C); route(&mut b, true, true, true, None);
        command(&mut b, 0x36, &[0xc0]); command(&mut b, 0x3a, &[0x55]);
        command(&mut b, 0x11, &[]); command(&mut b, 0x29, &[]);
        command(&mut b, 0x53, &[0x20]); command(&mut b, 0x51, &[255]);
        command(&mut b, 0x2a, &[0, 108, 0, 108]); command(&mut b, 0x2b, &[0, 200, 0, 200]); b
    }
    #[test]
    fn exact_four_lanes_keep_cs_and_reset_are_required_for_pixels() {
        let mut b = ready();
        b.spi_transfer_qspi_pins(2, pins(4, 1, true), &[0x32, 0, 0x2c, 0], 0);
        b.spi_transfer_qspi_pins(2, pins(2, 4, false), &[0xf8, 0], 0);
        assert_eq!(b.display().unwrap().2[200 * 466 + 100], 0xf800);
        let before = b.panel.pixels_written();
        for lane in 0..4 {
            b.spi_transfer_qspi_pins(2, pins(4, 1, true), &[0x32, 0, 0x2c, 0], 0);
            let mut bad = pins(2, 4, true); bad.data[lane] = 1 << 20;
            b.spi_transfer_qspi_pins(2, bad, &[0, 0], 0);
            b.spi_transfer_qspi_pins(2, pins(2, 4, false), &[0, 0], 0);
            assert_eq!(b.panel.pixels_written(), before, "discard until CS actually releases");
        }
        route(&mut b, false, true, true, None); assert!(b.display().unwrap().2.iter().all(|p| *p == 0));
        route(&mut b, true, true, true, None); assert!(!b.panel.visible());
        assert_eq!(b.panel.pixels_written(), before, "reset retained GRAM and the write counter");
        assert_eq!(b.named_pin("boot"), Some(0));
    }
    #[test]
    fn software_cs_release_resets_rejection_and_does_not_invent_a_new_header() {
        let mut b = ready(); route(&mut b, true, true, true, Some(false));
        let software = |n, width| SpiQspiPins { hardware_cs: 0, software_cs: 1 << 15, ..pins(n, width, false) };
        b.spi_transfer_qspi_pins(2, software(4, 1), &[0x32, 0, 0x2c, 0], 0);
        b.spi_transfer_qspi_pins(2, software(2, 4), &[0xf8, 0], 0);
        b.spi_transfer_qspi_pins(2, software(4, 4), &[2, 0, 0x28, 0], 0);
        assert!(b.panel.visible()); assert_eq!(b.panel.pixels_written(), 3);
        route(&mut b, true, true, true, Some(true)); route(&mut b, true, true, true, Some(false));
        b.spi_transfer_qspi_pins(2, software(4, 1), &[2, 0, 0x28, 0], 0); assert!(!b.panel.visible());
    }
    #[test]
    fn cst820_real_report_transform_and_codec_power_are_shared_across_buses() {
        let mut b = WaveshareQspi::new(QspiVariant::TouchAmoled143C); let mut devs = b.i2c_devices();
        assert_eq!(devs.len(), 6); assert!(devs.iter().all(|(_, _, d)| d.pins() == Some((47, 48))));
        for (_, address, d) in &mut devs {
            assert_eq!(d.start(false), *address != 0x15, "R18 powers codecs before GPIO18 is driven");
        }
        route(&mut b, true, true, true, None); b.touch_at(100, 100, 200, true);
        let read = |d: &mut dyn I2cDevice, r: u8, n: usize| {
            assert!(d.start(false)); assert!(d.write(r)); assert!(d.start(true));
            let bytes = (0..n).map(|_| d.read()).collect::<Vec<_>>(); d.stop(); bytes
        };
        for (bus, address, d) in &mut devs {
            match *address {
                0x15 => { assert_eq!(read(d.as_mut(), 2, 5), [1, 1, 109, 1, 9]);
                    assert!(d.start(false)); assert!(!d.write(0xa7)); assert!(!d.start(true)); },
                0x18 => { assert_eq!(read(d.as_mut(), 0xfd, 1), [0x83]);
                    if *bus == 0 { assert!(d.start(false)); assert!(d.write(0x32)); assert!(d.write(0xbf)); }
                    else { assert_eq!(read(d.as_mut(), 0x32, 1), [0xbf]); } },
                0x40 => assert_eq!(read(d.as_mut(), 0x3d, 1), [0x72]),
                _ => unreachable!(),
            }
        }
        assert!(b.input_levels().contains(&(17, false))); b.advance_to(100 + HZ / 1000);
        assert!(b.input_levels().contains(&(17, true)));
        route(&mut b, true, true, false, None);
        assert!(!devs.iter_mut().find(|(_, a, _)| *a == 0x18).unwrap().2.start(false));
        let gpio = esp_periph::Gpio::new(); let mux = esp_periph::RegRam::new();
        b.gpio_waveform_at(b.cycle, esp_soc::pins::ChipPins::S3.routes(&gpio, &mux));
        assert_eq!(read(devs.iter_mut().find(|(_, a, _)| *a == 0x18).unwrap().2.as_mut(), 0x32, 1), [0]);
        route(&mut b, true, false, true, None);
        assert!(!devs.iter_mut().find(|(_, a, _)| *a == 0x15).unwrap().2.start(false));
    }

    #[test]
    #[ignore = "fixed original 20261027 factory and S3 mask ROM required"]
    fn external_amoled143c_factory() {
        use esp_soc::{LoadKind, SocBus, Stop};
        let source = std::env::var("ESP32SIM_WAVESHARE_AMOLED_SOURCES").expect("fixed original source cache");
        let rom = std::env::var("ESP32SIM_ROM_DIR").expect("S3 mask ROM directory");
        let v = QspiVariant::TouchAmoled143C;
        let mut m = crate::machine([2, 0, 0, 0, 0, 1]); m.bus.board = Box::new(WaveshareQspi::new(v)); m.bus.attach_board_devices();
        m.bus.set_flash_size(v.flash_mib() << 20); m.bus.set_psram_size(v.psram_mib() << 20).unwrap();
        m.console.capture = true; m.console.mask = 2;
        m.load_input(LoadKind::Rom, &std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
        m.load_input(LoadKind::Flash, &std::fs::read(format!("{source}/{}/firmware/ESP32-S3-Touch-AMOLED-1.43C_FactoryOnly_20261027.bin", v.sku())).unwrap()).unwrap(); m.boot_rom();
        for ms in [1000, 2000, 4000, 8000] {
            m.max_cycles = ms * HZ / 1000;
            let stop = m.run(u64::MAX); m.bus.refresh_board_outputs();
            eprintln!("{ms}ms stop={stop:?} reboots={} {}", m.reboots, m.bus.board.report());
            if let Ok(root) = std::env::var("ESP32SIM_WAVESHARE_AMOLED_EVIDENCE") {
                let p = std::path::Path::new(&root).join(v.sku()); std::fs::create_dir_all(&p).unwrap();
                m.write_tft_png(p.join(format!("boot-{ms}.png")).to_str().unwrap(), 1).unwrap();
                let (w, h, pixels, version) = m.bus.board.display().unwrap();
                std::fs::write(p.join(format!("boot-{ms}.rgb565")), pixels.iter().flat_map(|p| p.to_le_bytes()).collect::<Vec<_>>()).unwrap();
                std::fs::write(p.join(format!("boot-{ms}.uart.txt")), &m.console.all).unwrap();
                std::fs::write(p.join(format!("boot-{ms}.board.txt")), m.bus.board.report()).unwrap();
                std::fs::write(p.join(format!("boot-{ms}.state.json")), format!("{{\"guestCycles\":{},\"guestMilliseconds\":{},\"reboots\":{},\"width\":{w},\"height\":{h},\"version\":{version},\"ramwrCommands\":{},\"pc\":[{},{}]}}\n", m.bus.cycles, m.bus.cycles / (HZ / 1000), m.reboots, m.bus.board.display_frames(), m.cores[0].pc, m.cores[1].pc)).unwrap();
            }
            assert!(matches!(stop, Stop::Halted));
        }
        assert_eq!(m.reboots, 0); assert!(m.bus.board.display_frames() > 0);
        assert!(m.bus.board.display().unwrap().2.iter().any(|p| *p != 0), "factory has not exposed a visible image");
    }

    #[test]
    #[ignore = "fixed original 20261027 factory and S3 mask ROM required"]
    fn external_amoled143c_factory_inputs() {
        use esp_soc::{LoadKind, SocBus, Stop};
        let source = std::env::var("ESP32SIM_WAVESHARE_AMOLED_SOURCES").expect("fixed original source cache");
        let rom = std::env::var("ESP32SIM_ROM_DIR").expect("S3 mask ROM directory");
        let v = QspiVariant::TouchAmoled143C;
        let make = || {
            let mut m = crate::machine([2, 0, 0, 0, 0, 1]); m.bus.board = Box::new(WaveshareQspi::new(v)); m.bus.attach_board_devices();
            m.bus.set_flash_size(v.flash_mib() << 20); m.bus.set_psram_size(v.psram_mib() << 20).unwrap();
            m.console.capture = true; m.console.mask = 2;
            m.load_input(LoadKind::Rom, &std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
            m.load_input(LoadKind::Flash, &std::fs::read(format!("{source}/{}/firmware/ESP32-S3-Touch-AMOLED-1.43C_FactoryOnly_20261027.bin", v.sku())).unwrap()).unwrap(); m.boot_rom(); m
        };
        let run = |m: &mut crate::Machine, ms: u64| {
            m.max_cycles = m.bus.cycles + ms * HZ / 1000;
            assert!(matches!(m.run(u64::MAX), Stop::Halted)); m.bus.refresh_board_outputs();
        };
        let save = |m: &mut crate::Machine, label: &str| {
            let ms = m.bus.cycles / (HZ / 1000);
            eprintln!("{label} {ms}ms reboots={} {}", m.reboots, m.bus.board.report());
            if let Ok(root) = std::env::var("ESP32SIM_WAVESHARE_AMOLED_EVIDENCE") {
                let p = std::path::Path::new(&root).join(v.sku()); std::fs::create_dir_all(&p).unwrap();
                m.write_tft_png(p.join(format!("{label}.png")).to_str().unwrap(), 1).unwrap();
                let (w, h, pixels, version) = m.bus.board.display().unwrap();
                std::fs::write(p.join(format!("{label}.rgb565")), pixels.iter().flat_map(|p| p.to_le_bytes()).collect::<Vec<_>>()).unwrap();
                std::fs::write(p.join(format!("{label}.uart.txt")), &m.console.all).unwrap();
                std::fs::write(p.join(format!("{label}.board.txt")), m.bus.board.report()).unwrap();
                std::fs::write(p.join(format!("{label}.state.json")), format!("{{\"guestCycles\":{},\"guestMilliseconds\":{ms},\"reboots\":{},\"width\":{w},\"height\":{h},\"version\":{version},\"ramwrCommands\":{},\"pc\":[{},{}]}}\n", m.bus.cycles, m.reboots, m.bus.board.display_frames(), m.cores[0].pc, m.cores[1].pc)).unwrap();
            }
        };
        let tap_open = |m: &mut crate::Machine| {
            m.bus.touch_input(305, 295, true); run(m, 120);
            m.bus.touch_input(305, 295, false); run(m, 600);
        };
        let mut control = make(); run(&mut control, 8000); save(&mut control, "control-desktop");
        let desktop = control.bus.board.display().unwrap().2;
        run(&mut control, 500); save(&mut control, "control-desktop-quiet");
        let quiet = control.bus.board.display().unwrap().2;
        tap_open(&mut control); save(&mut control, "control-canvas");
        let canvas = control.bus.board.display().unwrap().2;
        run(&mut control, 1000); save(&mut control, "control-draw-window");
        let draw_control = control.bus.board.display().unwrap().2;
        run(&mut control, 1000); save(&mut control, "control-boot-window");
        let boot_control = control.bus.board.display().unwrap().2;
        assert_eq!(control.reboots, 0); drop(control);

        let mut input = make(); run(&mut input, 8500); tap_open(&mut input); save(&mut input, "input-canvas");
        input.bus.touch_input(150, 180, true); run(&mut input, 40);
        for step in 1..=40 { input.bus.touch_input(150 + step * 4, 180 + step * 2, true); run(&mut input, 20); }
        input.bus.touch_input(310, 260, false); run(&mut input, 160); save(&mut input, "input-drawn");
        let drawn = input.bus.board.display().unwrap().2;
        input.bus.gpio_set_input(0, false); run(&mut input, 150);
        input.bus.gpio_set_input(0, true); run(&mut input, 850); save(&mut input, "input-boot-exit");
        let boot = input.bus.board.display().unwrap().2;
        let different = |a: &[u16], b: &[u16]| a.iter().zip(b).filter(|(a, b)| a != b).count();
        let red = |p: u16| (p >> 11) > 24 && ((p >> 5) & 63) < 12 && (p & 31) < 6;
        let added_red = drawn.iter().zip(&draw_control).filter(|(a, b)| red(**a) && !red(**b)).count();
        let result = format!("{{\"quietDesktopPixels\":{},\"openCanvasPixels\":{},\"drawVsMatchedQuietPixels\":{},\"addedRedPixels\":{added_red},\"bootVsMatchedQuietPixels\":{},\"bootVsOriginalDesktopPixels\":{},\"tapMs\":[8500,9220],\"drawMs\":[9220,10220],\"bootMs\":[10220,11220],\"touchIcon\":[305,295],\"drawStart\":[150,180],\"drawEnd\":[310,260],\"drawSteps\":40,\"bootPressedMs\":150}}\n", different(&desktop, &quiet), different(&quiet, &canvas), different(&drawn, &draw_control), different(&boot, &boot_control), different(&boot, &desktop));
        eprintln!("INPUT {result}");
        if let Ok(root) = std::env::var("ESP32SIM_WAVESHARE_AMOLED_EVIDENCE") { std::fs::write(std::path::Path::new(&root).join(v.sku()).join("input-comparison.json"), result).unwrap(); }
        assert_eq!(input.reboots, 0); assert_eq!(different(&desktop, &quiet), 0, "quiet desktop changed");
        assert!(different(&quiet, &canvas) > 10000, "tap did not open Touch Test");
        assert!(added_red > 500, "original Touch Test did not draw the host trajectory");
        assert!(different(&boot, &boot_control) > 10000, "BOOT did not exit the canvas");
        assert_eq!(different(&boot, &desktop), 0, "BOOT did not restore the original desktop");
    }
}
