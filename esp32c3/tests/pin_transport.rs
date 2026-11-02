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

fn route(bus: &mut esp32c3::bus::SocBus, sda: u8, scl: u8) {
    for (pin, sig) in [(sda, 54), (scl, 53)] {
        bus.write32(0x6000_9004 + 4 * pin as u32, 1 << 12 | 1 << 9)
            .unwrap();
        bus.write32(0x6000_4554 + 4 * pin as u32, sig as u32)
            .unwrap();
        bus.write32(0x6000_4154 + 4 * sig as u32, 0x40 | pin as u32)
            .unwrap();
    }
}

fn read_i2c(bus: &mut esp32c3::bus::SocBus) -> (bool, u8) {
    let base = 0x6001_3000;
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
    let mut m = esp32c3::machine([0; 6], 4 * 1024 * 1024);
    let bus = &mut m.bus;
    bus.periph
        .i2c
        .attach(0x42, Box::new(Device(Some((8, 9)), 11)));
    bus.periph
        .i2c
        .attach(0x42, Box::new(Device(Some((6, 7)), 22)));
    route(bus, 8, 9);
    assert_eq!(read_i2c(bus), (true, 11));
    route(bus, 6, 7);
    assert_eq!(read_i2c(bus), (true, 22));
    route(bus, 4, 5);
    assert!(!read_i2c(bus).0);
    route(bus, 8, 9);
    bus.write32(0x6000_9024, 1 << 12).unwrap(); // disable SDA IO_MUX input
    assert!(!read_i2c(bus).0);
    route(bus, 8, 9);
    bus.periph
        .i2c
        .attach(0x42, Box::new(Device(Some((8, 9)), 33)));
    assert_eq!(read_i2c(bus), (true, 33));
    bus.periph.i2c.attach(0x43, Box::new(Device(None, 44)));
    // Fixed devices retain their original controller-only contract.
    let i2c = &mut bus.periph.i2c;
    i2c.attach(0x42, Box::new(Device(None, 55)));
    route(bus, 4, 5);
    assert_eq!(read_i2c(bus), (true, 55));
}

#[derive(Default)]
struct State {
    routes: Vec<SpiPins>,
    transfers: Vec<Vec<u8>>,
    outputs: Vec<(u64, u64, u64)>,
    edges: Vec<BoardEdge>,
    frames: Vec<(u8, Vec<[u8; 3]>)>,
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
    fn spi_transfer_pins(&mut self, _: u8, pins: SpiPins, tx: &[u8], n: usize) -> Vec<u8> {
        self.0.lock().unwrap().routes.push(pins);
        self.0.lock().unwrap().transfers.push(tx.to_vec());
        let selected = pins.cs & (1 << 10) != 0
            && pins.sclk & (1 << 6) != 0
            && pins.mosi & (1 << 7) != 0
            && (n == 0 || pins.miso == Some(2));
        vec![if selected { 0xa5 } else { 0xff }; n]
    }
    fn rmt_frame(&mut self, pin: u8, bits: &[bool]) {
        let mut chain = esp_soc::devices::Ws2812Chain::new(bits.len() / 24);
        chain.from_bits(bits);
        if chain.updates != 0 {
            self.0.lock().unwrap().frames.push((pin, chain.leds));
        }
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
fn spi(bus: &mut esp32c3::bus::SocBus) -> u8 {
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
    let mut m = esp32c3::machine([0; 6], 4 * 1024 * 1024);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.attach_board_devices();
    let bus = &mut m.bus;
    for (pin, sig) in [(6, 63), (7, 65), (10, 68)] {
        bus.write32(0x6000_9004 + 4 * pin, 1 << 12 | 1 << 9)
            .unwrap();
        bus.write32(0x6000_4554 + 4 * pin, sig).unwrap();
    }
    bus.write32(0x6000_900c, 1 << 12 | 1 << 9).unwrap();
    bus.write32(0x6000_4254, 0x40 | 2).unwrap();
    bus.write32(0x6002_4020, 0x3e).unwrap();
    assert_eq!(spi(bus), 0xa5);
    assert_eq!(
        state.lock().unwrap().routes.last().copied().unwrap(),
        SpiPins {
            sclk: 1 << 6,
            mosi: 1 << 7,
            miso: Some(2),
            cs: 1 << 10,
        }
    );
    bus.write32(0x6002_4020, 0x3e | 1 << 7).unwrap(); // active-high CS
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6002_4020, 0x3e).unwrap();
    bus.write32(0x6000_9020, 0).unwrap(); // MOSI mux no longer selects SPI
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6000_9020, 1 << 12 | 1 << 9).unwrap();
    bus.write32(0x6000_4254, 0x60 | 2).unwrap(); // inverted MISO is unsupported
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6000_4254, 0x40 | 2).unwrap();
    bus.write32(0x6002_4020, 0x3f).unwrap();
    assert_eq!(spi(bus), 0xff);
    // Software CS: low only, with GPIO output enabled.
    bus.write32(0x6000_457c, 128).unwrap();
    bus.write32(0x6000_4024, 1 << 10).unwrap();
    assert_eq!(spi(bus), 0xa5);
    bus.write32(0x6000_4008, 1 << 10).unwrap();
    assert_eq!(spi(bus), 0xff);
    // Native IO_MUX, no matrix routes.
    for pin in [6, 7, 2, 10] {
        bus.write32(0x6000_9004 + 4 * pin, 2 << 12 | 1 << 9)
            .unwrap();
    }
    bus.write32(0x6000_4254, 0).unwrap();
    bus.write32(0x6002_4020, 0x3e).unwrap();
    assert_eq!(spi(bus), 0xa5);
    assert_eq!(state.lock().unwrap().routes.last().unwrap().miso, Some(2));
}

#[test]
fn output_cycles_include_low_release_and_input_edges_keep_their_timestamp() {
    let mut m = esp32c3::machine([0; 6], 4 * 1024 * 1024);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.attach_board_devices();
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
    m.bus.tick(9);
    assert_eq!(m.bus.periph.gpio.input & (1 << 5), 0);
    assert!(m
        .bus
        .gpio_events
        .as_ref()
        .unwrap()
        .contains(&(24110, 5, false)));
}

/// Build docs/evidence/c3-peripherals-2026-10-02/main.cpp with the adjacent
/// PlatformIO configuration, then supply its build directory and a ROM ELF.
#[test]
#[ignore = "set ESP32SIM_TRANSPORT_BUILD to the PlatformIO build directory; needs esp32c3_rev3_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_c3_arduino_pin_transport() {
    let build = std::path::PathBuf::from(std::env::var_os("ESP32SIM_TRANSPORT_BUILD")
        .expect("ESP32SIM_TRANSPORT_BUILD must name the PlatformIO build directory containing firmware.factory.bin"));
    let rom = esp_soc::rom::require("esp32c3_rev3_rom.elf");
    let state = Arc::new(Mutex::new(State::default()));
    let mut m = esp32c3::machine([2, 0, 0, 0, 0, 1], 4 * 1024 * 1024);
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.attach_board_devices();
    m.console.capture = true;
    m.console.mask = 2;
    m.load_rom(&std::fs::read(rom).expect("readable esp32c3_rev3_rom.elf")).unwrap();
    m.write_flash(
        0,
        &std::fs::read(build.join("firmware.factory.bin"))
            .expect("ESP32SIM_TRANSPORT_BUILD must contain a readable firmware.factory.bin"),
    )
    .unwrap();
    m.boot_rom();
    m.max_cycles = 160_000_000;
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
    assert_eq!(
        state.frames,
        [
            (5, vec![[0x12, 0x34, 0x56]]),
            (1, vec![[0xab, 0xcd, 0xef], [0x21, 0x43, 0x65]])
        ]
    );
    println!("WS2812 frames={:?}", state.frames);
    assert_eq!(state.routes.len(), 2);
    assert_eq!(state.transfers, [vec![0xa5], vec![0x5a]]);
    for pins in &state.routes {
        assert_eq!(pins.sclk, 1 << 6);
        assert_eq!(pins.mosi, 1 << 7);
        assert_eq!(pins.miso, Some(2));
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
        width as f64 / 160.0
    );
    // delayMicroseconds plus digitalWrite overhead; one native scheduler round is 64 cycles.
    assert!(width.abs_diff(16_000) <= 64, "width={width}");
}

#[test]
fn reset_reattaches_pinned_board_devices_and_clears_routes() {
    let mut m = esp32c3::machine([0; 6], 4 * 1024 * 1024);
    m.bus.board = Box::new(Board(Arc::new(Mutex::new(State::default()))));
    m.bus.attach_board_devices();
    route(&mut m.bus, 8, 9);
    assert_eq!(read_i2c(&mut m.bus), (true, 0x33));
    esp_soc::SocBus::reboot(&mut m.bus, [0; 6]);
    assert!(!read_i2c(&mut m.bus).0);
    route(&mut m.bus, 8, 9);
    assert_eq!(read_i2c(&mut m.bus), (true, 0x33));
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
    let mut m = esp32c3::machine([0; 6], 4 * 1024 * 1024);
    m.bus.board = Box::new(Legacy(changes.clone()));
    m.bus.attach_board_devices();
    m.bus.write32(0x6000_4024, 16).unwrap();
    m.bus.write32(0x6000_4008, 16).unwrap();
    m.bus.write32(0x6000_400c, 16).unwrap();
    m.bus.write32(0x6000_4028, 16).unwrap();
    assert_eq!(*changes.lock().unwrap(), [(4, true), (4, false)]);
}

#[test]
fn rmt_channels_deliver_colours_and_raise_c3_interrupts() {
    let mut m = esp32c3::machine([0; 6], 4 * 1024 * 1024);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.attach_board_devices();
    m.bus.periph.intc.map[esp32c3::periph::src::RMT] = 5;
    for ch in 0..2u32 {
        let pin = 5 + ch;
        m.bus.write32(0x6000_4554 + pin * 4, 51 + ch).unwrap();
        m.bus.write32(0x6000_9004 + pin * 4, 1 << 12).unwrap();
        let colour = 0x341256u32;
        for bit in 0..24 {
            let (high, low) = if colour & (1 << (23 - bit)) != 0 {
                (64, 32)
            } else {
                (32, 64)
            };
            m.bus
                .write32(
                    0x6001_6400 + ch * 48 * 4 + bit * 4,
                    0x8000 | high | low << 16,
                )
                .unwrap();
        }
        m.bus
            .write32(0x6001_6400 + ch * 48 * 4 + 24 * 4, 0)
            .unwrap();
        m.bus.write32(0x6001_6040, 1 << ch).unwrap();
        m.bus
            .write32(0x6001_6010 + ch * 4, 1 | 1 << 8 | 1 << 16)
            .unwrap();
        assert!(esp_soc::SocBus::next_deadline(&m.bus).is_some_and(|n| n <= 32));
        m.bus.tick(24 * 96 * 2 + 1);
        assert_ne!(
            m.bus.periph.source_status()[0] & (1 << esp32c3::periph::src::RMT),
            0
        );
        m.bus.periph.refresh_lines();
        assert_ne!(m.bus.periph.intc.lines.level & (1 << 5), 0);
        m.bus.write32(0x6001_6044, 1 << ch).unwrap();
        m.bus.periph.refresh_lines();
        assert_eq!(m.bus.periph.intc.lines.level & (1 << 5), 0);
        assert_eq!(
            m.bus.periph.source_status()[0] & (1 << esp32c3::periph::src::RMT),
            0
        );
    }
    assert_eq!(
        state.lock().unwrap().frames,
        [(5, vec![[0x12, 0x34, 0x56]]), (6, vec![[0x12, 0x34, 0x56]])]
    );
}

#[test]
fn reset_gpio_select_drives_software_cs_and_keeps_the_change_buffer() {
    let mut m = esp32c3::machine([0; 6], 4 << 20);
    for pin in 0..22 { assert_eq!(m.bus.read32(0x6000_4554 + pin * 4).unwrap(), 128); }
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.attach_board_devices();
    for (pin, sig) in [(6, 63), (7, 65)] {
        m.bus.write32(0x6000_9004 + pin * 4, 1 << 12).unwrap();
        m.bus.write32(0x6000_4554 + pin * 4, sig).unwrap();
    }
    m.bus.write32(0x6000_902c, 1 << 12).unwrap();
    m.bus.write32(0x6000_900c, 1 << 12 | 1 << 9).unwrap();
    m.bus.write32(0x6000_4254, 0x40 | 2).unwrap();
    m.bus.write32(0x6000_4024, 1 << 10).unwrap();
    m.bus.write32(0x6002_4020, 0x3f).unwrap();
    assert_eq!(spi(&mut m.bus), 0xa5);
    m.bus.write32(0x6000_4008, 1 << 10).unwrap();
    let capacity = m.bus.periph.gpio.changes.capacity();
    assert!(capacity > 0);
    m.bus.write32(0x6000_400c, 1 << 10).unwrap();
    assert_eq!(m.bus.periph.gpio.changes.capacity(), capacity);
    assert!(m.bus.periph.gpio.changes.is_empty());
}

#[test]
fn idle_board_does_not_receive_clock_callbacks() {
    struct Idle;
    impl BoardModel for Idle {
        fn name(&self) -> &'static str { "idle" }
        fn uses_gpio_edges(&self) -> bool { false }
        fn next_deadline(&self) -> Option<u64> { panic!("idle deadline queried"); }
        fn advance_to(&mut self, _: u64) { panic!("idle board advanced"); }
        fn take_edges(&mut self) -> Vec<BoardEdge> { panic!("idle board polled"); }
    }
    let mut m = esp32c3::machine([0; 6], 4 << 20);
    m.bus.board = Box::new(Idle);
    m.bus.attach_board_devices();
    let _ = esp_soc::SocBus::next_deadline(&m.bus);
    assert_eq!(m.bus.tick(64), 1);
    assert!(!m.bus.periph.rmt.rmt.ch.iter().any(|c| c.running));
    assert!(m.bus.periph.rmt.rmt.done.is_empty());
}

#[test]
fn pin_interrupts_follow_enable_clear_and_spi_completion() {
    use esp32c3::periph::src;
    let mut m = esp32c3::machine([0; 6], 4 << 20);
    let bus = &mut m.bus;
    bus.periph.i2c.attach(0x42, Box::new(Device(None, 11)));
    for (base, ena, clr, bit, source) in [
        (0x6001_3000, 0x28, 0x24, 1 << 7, src::I2C_EXT0),
        (0x6002_4000, 0x34, 0x38, 1 << 12, src::SPI2),
    ] {
        bus.periph.intc.map[source] = 5;
        bus.write32(base + ena, bit).unwrap();
        if source == src::I2C_EXT0 { assert_eq!(read_i2c(bus), (true, 11)); }
        else { spi(bus); }
        assert_ne!(bus.periph.source_status()[0] & (1 << source), 0);
        bus.periph.refresh_lines();
        assert_ne!(bus.periph.intc.lines.level & (1 << 5), 0);
        bus.write32(base + ena, 0).unwrap();
        assert_eq!(bus.periph.source_status()[0] & (1 << source), 0);
        bus.periph.refresh_lines();
        assert_eq!(bus.periph.intc.lines.level & (1 << 5), 0);
        bus.write32(base + ena, bit).unwrap();
        assert_ne!(bus.periph.source_status()[0] & (1 << source), 0);
        bus.periph.refresh_lines();
        assert_ne!(bus.periph.intc.lines.level & (1 << 5), 0);
        bus.write32(base + clr, bit).unwrap();
        assert_eq!(bus.periph.source_status()[0] & (1 << source), 0);
        bus.periph.refresh_lines();
        assert_eq!(bus.periph.intc.lines.level & (1 << 5), 0);
    }
}
