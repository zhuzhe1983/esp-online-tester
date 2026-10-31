use emu_core::Bus;
use esp_periph::i2c::I2cDevice;
use esp_soc::board::{BoardEdge, BoardModel, SpiPins};
use std::sync::{Arc, Mutex};

struct Device(Option<(u8, u8)>, u8);
impl I2cDevice for Device {
    fn pins(&self) -> Option<(u8, u8)> {
        self.0
    }
    fn write(&mut self, _: u8) -> bool {
        true
    }
    fn read(&mut self) -> u8 {
        self.1
    }
}

fn route(bus: &mut esp32s3::bus::SocBus, sda: u8, scl: u8, controller: usize) {
    for (pin, sig) in [(sda, 90 + 2 * controller), (scl, 89 + 2 * controller)] {
        bus.write32(0x6000_9004 + 4 * pin as u32, 1 << 12 | 1 << 9)
            .unwrap();
        bus.write32(0x6000_4554 + 4 * pin as u32, sig as u32)
            .unwrap();
        bus.write32(0x6000_4154 + 4 * sig as u32, 0x80 | pin as u32)
            .unwrap();
    }
}

fn read_i2c(bus: &mut esp32s3::bus::SocBus, controller: usize) -> (bool, u8) {
    let base = [0x6001_3000, 0x6002_7000][controller];
    for (off, value) in [
        (0x24, u32::MAX),
        (0x18, 3 << 12),
        (0x1c, 0x85),
        (0x58, 6 << 11),
        (0x5c, 1 << 11 | 1 << 8 | 1),
        (0x60, 3 << 11 | 1),
        (0x64, 2 << 11),
        (0x04, 1 << 5),
    ] {
        bus.write32(base + off, value).unwrap();
    }
    bus.tick(100_000);
    (
        bus.read32(base + 0x20).unwrap() & (1 << 10) == 0,
        bus.read32(base + 0x1c).unwrap() as u8,
    )
}

#[test]
fn i2c_matches_pins_and_preserves_replacement_and_fixed_devices() {
    for controller in 0..2 {
        let mut m = esp32s3::machine([0; 6]);
        let bus = &mut m.bus;
        bus.periph.i2c[controller].attach(0x42, Box::new(Device(Some((8, 9)), 11)));
        bus.periph.i2c[controller].attach(0x42, Box::new(Device(Some((6, 7)), 22)));
        route(bus, 8, 9, controller);
        assert_eq!(read_i2c(bus, controller), (true, 11));
        route(bus, 6, 7, controller);
        assert_eq!(read_i2c(bus, controller), (true, 22));
        route(bus, 4, 5, controller);
        assert!(!read_i2c(bus, controller).0);
        route(bus, 8, 9, controller);
        bus.write32(0x6000_9024, 1 << 12).unwrap(); // disable SDA IO_MUX input
        assert!(!read_i2c(bus, controller).0);
        route(bus, 8, 9, controller);
        bus.periph.i2c[controller].attach(0x42, Box::new(Device(Some((8, 9)), 33)));
        assert_eq!(read_i2c(bus, controller), (true, 33));
        bus.periph.i2c[controller].attach(0x43, Box::new(Device(None, 44)));
        // Fixed devices retain their original controller-only contract.
        let i2c = &mut bus.periph.i2c[controller];
        i2c.attach(0x42, Box::new(Device(None, 55)));
        route(bus, 4, 5, controller);
        assert_eq!(read_i2c(bus, controller), (true, 55));
    }
}

#[derive(Default)]
struct State {
    routes: Vec<SpiPins>,
    outputs: Vec<(u64, u64, u64)>,
    edges: Vec<BoardEdge>,
}
struct Board(Arc<Mutex<State>>);
impl BoardModel for Board {
    fn name(&self) -> &'static str {
        "pin-test"
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        vec![(0, 0x42, Box::new(Device(Some((8, 9)), 0x33)))]
    }
    fn uses_spi_pins(&self) -> bool {
        true
    }
    fn spi_transfer_pins(&mut self, _: u8, pins: SpiPins, _: &[u8], n: usize) -> Vec<u8> {
        self.0.lock().unwrap().routes.push(pins);
        let selected = pins.cs & (1 << 10) != 0
            && pins.sclk & (1 << 12) != 0
            && pins.mosi & (1 << 11) != 0
            && (n == 0 || pins.miso == Some(13));
        vec![if selected { 0xa5 } else { 0xff }; n]
    }
    fn gpio_output_at(&mut self, cycle: u64, _: &[(u8, bool)], enabled: u64, output: u64) {
        self.0
            .lock()
            .unwrap()
            .outputs
            .push((cycle, enabled, output));
    }
    fn next_deadline(&self) -> Option<u64> {
        self.0.lock().unwrap().edges.first().map(|e| e.cycle)
    }
    fn advance_to(&mut self, _: u64) {}
    fn take_edges(&mut self) -> Vec<BoardEdge> {
        std::mem::take(&mut self.0.lock().unwrap().edges)
    }
}
fn spi(bus: &mut esp32s3::bus::SocBus) -> u8 {
    for (off, value) in [
        (0x10, 1 << 27 | 1 << 28),
        (0x1c, 7),
        (0x98, 0x5a),
        (0, 1 << 24),
    ] {
        bus.write32(0x6002_4000 + off, value).unwrap();
    }
    bus.read32(0x6002_4098).unwrap() as u8
}

#[test]
fn spi_native_matrix_and_software_chip_select_routes() {
    let mut m = esp32s3::machine([0; 6]);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    let bus = &mut m.bus;
    for (pin, sig) in [(12, 101), (11, 103), (10, 110)] {
        bus.write32(0x6000_9004 + 4 * pin, 1 << 12 | 1 << 9)
            .unwrap();
        bus.write32(0x6000_4554 + 4 * pin, sig).unwrap();
    }
    bus.write32(0x6000_9038, 1 << 12 | 1 << 9).unwrap();
    bus.write32(0x6000_42ec, 0x80 | 13).unwrap();
    bus.write32(0x6002_4020, 0x3e).unwrap();
    assert_eq!(spi(bus), 0xa5);
    assert_eq!(
        state.lock().unwrap().routes.last().copied().unwrap(),
        SpiPins {
            sclk: 1 << 12,
            mosi: 1 << 11,
            miso: Some(13),
            cs: 1 << 10,
        }
    );
    bus.write32(0x6002_4020, 0x3e | 1 << 7).unwrap(); // active-high CS
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6002_4020, 0x3e).unwrap();
    bus.write32(0x6000_9030, 0).unwrap(); // MOSI mux no longer selects SPI
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6000_9030, 1 << 12 | 1 << 9).unwrap();
    bus.write32(0x6000_42ec, 0xc0 | 13).unwrap(); // inverted MISO is unsupported
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6000_42ec, 0x80 | 13).unwrap();
    bus.write32(0x6002_4020, 0x3f).unwrap();
    assert_eq!(spi(bus), 0xff);
    // Software CS: low only, with GPIO output enabled.
    bus.write32(0x6000_457c, 256).unwrap();
    bus.write32(0x6000_4024, 1 << 10).unwrap();
    assert_eq!(spi(bus), 0xa5);
    bus.write32(0x6000_4008, 1 << 10).unwrap();
    assert_eq!(spi(bus), 0xff);
    // Native IO_MUX, no matrix routes.
    for pin in 10..=13 {
        bus.write32(0x6000_9004 + 4 * pin, 4 << 12 | 1 << 9)
            .unwrap();
    }
    bus.write32(0x6000_42ec, 0).unwrap();
    bus.write32(0x6002_4020, 0x3e).unwrap();
    assert_eq!(spi(bus), 0xa5);
    assert_eq!(state.lock().unwrap().routes.last().unwrap().miso, Some(13));
}

#[test]
fn output_cycles_include_low_release_and_input_edges_keep_their_timestamp() {
    let mut m = esp32s3::machine([0; 6]);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.gpio_events = Some(Vec::new());
    m.bus.tick(100);
    m.bus.write32(0x6000_4024, 1 << 4).unwrap();
    m.bus.write32(0x6000_4008, 1 << 4).unwrap();
    m.bus.tick(24000);
    m.bus.write32(0x6000_400c, 1 << 4).unwrap();
    m.bus.tick(1);
    m.bus.write32(0x6000_4028, 1 << 4).unwrap();
    assert_eq!(
        state.lock().unwrap().outputs,
        [(100, 16, 0), (100, 16, 16), (24100, 16, 0), (24101, 0, 0)]
    );
    state.lock().unwrap().edges.push(BoardEdge {
        cycle: 24110,
        pin: 5,
        level: false,
    });
    m.bus.refresh_tick_budget();
    m.bus.tick(9);
    m.bus.flush_ticks();
    assert_eq!(m.bus.periph.gpio.input & (1 << 5), 0);
    assert!(m
        .bus
        .gpio_events
        .as_ref()
        .unwrap()
        .contains(&(24110, 5, false)));
}

/// Build docs/evidence/s3-pin-transport-2026-10-02/main.cpp with the adjacent
/// PlatformIO configuration, then supply its build directory and a ROM ELF.
#[test]
#[ignore = "set ESP32SIM_TRANSPORT_BUILD to the PlatformIO build directory; needs esp32s3_rev0_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_s3_arduino_pin_transport() {
    let build = std::path::PathBuf::from(std::env::var_os("ESP32SIM_TRANSPORT_BUILD")
        .expect("ESP32SIM_TRANSPORT_BUILD must name the PlatformIO build directory containing firmware.factory.bin"));
    let rom = esp_soc::rom::require("esp32s3_rev0_rom.elf");
    let state = Arc::new(Mutex::new(State::default()));
    let mut m = esp32s3::machine([2, 0, 0, 0, 0, 1]);
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.attach_board_devices();
    m.console.capture = true;
    m.console.mask = 2;
    m.load_rom(&std::fs::read(rom).expect("readable esp32s3_rev0_rom.elf")).unwrap();
    m.write_flash(
        0,
        &std::fs::read(build.join("firmware.factory.bin"))
            .expect("ESP32SIM_TRANSPORT_BUILD must contain a readable firmware.factory.bin"),
    )
    .unwrap();
    m.boot_rom();
    m.max_cycles = 240_000_000;
    let stop = m.run(u64::MAX);
    let console = String::from_utf8_lossy(&m.console.all);
    println!("{console}");
    println!(
        "stop={stop:?} cycles={} instructions={}",
        m.bus.cycles,
        m.insns()
    );
    assert!(console.contains("I2C right=0"));
    assert!(console.contains("I2C wrong=2"));
    assert!(console.contains("SPI right=a5 wrong=ff"));
    assert!(console.contains("TRANSPORT DONE"));
    let state = state.lock().unwrap();
    assert_eq!(state.routes.len(), 2);
    for pins in &state.routes {
        assert_eq!(pins.sclk, 1 << 12);
        assert_eq!(pins.mosi, 1 << 11);
        assert_eq!(pins.miso, Some(13));
    }
    let high = state
        .outputs
        .iter()
        .find(|&&(_, enabled, out)| enabled & out & 16 != 0)
        .unwrap()
        .0;
    let low = state
        .outputs
        .iter()
        .find(|&&(cycle, enabled, out)| cycle > high && enabled & 16 != 0 && out & 16 == 0)
        .unwrap()
        .0;
    let width = low - high;
    println!(
        "pulse high_cycle={high} low_cycle={low} width_cycles={width} width_us={:.3}",
        width as f64 / 240.0
    );
    // delayMicroseconds plus digitalWrite overhead; one native scheduler round is 64 cycles.
    assert!(width.abs_diff(24_000) <= 64, "width={width}");
}

#[test]
fn delayed_spi_keeps_submission_routes() {
    let mut m = esp32s3::machine([0; 6]);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    let bus = &mut m.bus;
    bus.spi2_timing = true;
    for (pin, sig) in [(12, 101), (11, 103), (10, 110)] {
        bus.write32(0x6000_9004 + 4 * pin, 1 << 12).unwrap();
        bus.write32(0x6000_4554 + 4 * pin, sig).unwrap();
    }
    let desc = 0x3fc9_0100;
    bus.periph.gdma.out[0].peri_sel = 0;
    bus.periph.gdma.out[0].desc = desc;
    bus.periph.gdma.out[0].running = true;
    bus.write32(desc, 4 | 4 << 12 | 1 << 30 | 1 << 31).unwrap();
    bus.write32(desc + 4, desc + 16).unwrap();
    bus.write32(desc + 8, 0).unwrap();
    for (off, value) in [(0x30, 1 << 28), (0x10, 1 << 27), (0x1c, 31), (0, 1 << 24)] {
        bus.write32(0x6002_4000 + off, value).unwrap();
    }
    assert!(state.lock().unwrap().routes.is_empty());
    bus.write32(0x6000_9030, 0).unwrap(); // unroute MOSI before completion
    bus.write32(0x6000_902c, 0).unwrap(); // unroute CS before completion
    bus.tick(1000);
    bus.flush_ticks();
    let state = state.lock().unwrap();
    assert_eq!(state.routes.len(), 1);
    assert_eq!(state.routes[0].mosi, 1 << 11);
    assert_eq!(state.routes[0].cs, 1 << 10);
}

#[test]
fn reset_reattaches_pinned_board_devices_and_clears_routes() {
    let mut m = esp32s3::machine([0; 6]);
    m.bus.board = Box::new(Board(Arc::new(Mutex::new(State::default()))));
    m.bus.attach_board_devices();
    route(&mut m.bus, 8, 9, 0);
    assert_eq!(read_i2c(&mut m.bus, 0), (true, 0x33));
    esp_soc::SocBus::reboot(&mut m.bus, [0; 6]);
    assert!(!read_i2c(&mut m.bus, 0).0);
    route(&mut m.bus, 8, 9, 0);
    assert_eq!(read_i2c(&mut m.bus, 0), (true, 0x33));
}

#[test]
fn legacy_gpio_callback_still_receives_only_level_changes() {
    struct Legacy(Arc<Mutex<Vec<(u8, bool)>>>);
    impl BoardModel for Legacy {
        fn name(&self) -> &'static str {
            "legacy"
        }
        fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
            assert!(!changes.is_empty());
            self.0.lock().unwrap().extend_from_slice(changes);
        }
    }
    let changes = Arc::new(Mutex::new(Vec::new()));
    let mut m = esp32s3::machine([0; 6]);
    m.bus.board = Box::new(Legacy(changes.clone()));
    m.bus.write32(0x6000_4024, 16).unwrap();
    m.bus.write32(0x6000_4008, 16).unwrap();
    m.bus.write32(0x6000_400c, 16).unwrap();
    m.bus.write32(0x6000_4028, 16).unwrap();
    assert_eq!(*changes.lock().unwrap(), [(4, true), (4, false)]);
}
