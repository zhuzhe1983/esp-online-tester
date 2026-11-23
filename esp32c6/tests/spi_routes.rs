use emu_core::Bus;
use esp_soc::board::{BoardModel, SpiPins};
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct State {
    routes: Vec<SpiPins>,
}
struct Board(Arc<Mutex<State>>);
impl BoardModel for Board {
    fn name(&self) -> &'static str {
        "spi-route-test"
    }
    fn uses_spi_pins(&self) -> bool {
        true
    }
    fn spi_transfer_pins(&mut self, _: u8, pins: SpiPins, tx: &[u8], n: usize) -> Vec<u8> {
        assert_eq!(tx, [0x5a]);
        self.0.lock().unwrap().routes.push(pins);
        let selected = pins.cs & (1 << 16) != 0
            && pins.sclk & (1 << 6) != 0
            && pins.mosi & (1 << 7) != 0
            && pins.miso == Some(2);
        vec![if selected { 0xa5 } else { 0xff }; n]
    }
}
fn spi(bus: &mut esp32c6::bus::SocBus) -> u8 {
    for (off, value) in [
        (0x10, 1 << 27 | 1 << 28),
        (0x1c, 7),
        (0x98, 0x5a),
        (0, 1 << 24),
    ] {
        bus.write32(0x6008_1000 + off, value).unwrap();
    }
    bus.read32(0x6008_1098).unwrap() as u8
}

#[test]
fn spi_native_matrix_and_software_chip_select_routes() {
    let mut m = esp32c6::machine([0; 6], 4 * 1024 * 1024);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    let bus = &mut m.bus;
    for (pin, sig) in [(6, 63), (7, 65), (16, 68)] {
        bus.write32(0x6009_0004 + 4 * pin, 1 << 12 | 1 << 9)
            .unwrap();
        bus.write32(0x6009_1554 + 4 * pin, sig).unwrap();
    }
    bus.write32(0x6009_000c, 1 << 12 | 1 << 9).unwrap();
    bus.write32(0x6009_1254, 0x80 | 2).unwrap();
    bus.write32(0x6008_1020, 0x3e).unwrap();
    assert_eq!(spi(bus), 0xa5);
    assert_eq!(
        state.lock().unwrap().routes.last().copied().unwrap(),
        SpiPins {
            sclk: 1 << 6,
            mosi: 1 << 7,
            miso: Some(2),
            cs: 1 << 16,
            keep_cs: false,
        }
    );
    bus.write32(0x6008_1020, 0x3e | 1 << 7).unwrap(); // active-high CS
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6008_1020, 0x3e).unwrap();
    bus.write32(0x6009_0020, 0).unwrap(); // MOSI mux no longer selects SPI
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6009_0020, 1 << 12 | 1 << 9).unwrap();
    bus.write32(0x6009_1254, 0xc0 | 2).unwrap(); // inverted MISO is unsupported
    assert_eq!(spi(bus), 0xff);
    bus.write32(0x6009_1254, 0x80 | 2).unwrap();
    bus.write32(0x6008_1020, 0x3f).unwrap();
    assert_eq!(spi(bus), 0xff);
    // Software CS: low only, with GPIO output enabled.
    bus.write32(0x6009_1594, 128).unwrap();
    bus.write32(0x6009_1024, 1 << 16).unwrap();
    assert_eq!(spi(bus), 0xa5);
    bus.write32(0x6009_1008, 1 << 16).unwrap();
    assert_eq!(spi(bus), 0xff);
    // Native IO_MUX, no matrix routes.
    for pin in [6, 7, 2, 16] {
        bus.write32(0x6009_0004 + 4 * pin, 2 << 12 | 1 << 9)
            .unwrap();
    }
    bus.write32(0x6009_1254, 0).unwrap();
    bus.write32(0x6008_1020, 0x3e).unwrap();
    assert_eq!(spi(bus), 0xa5);
    assert_eq!(state.lock().unwrap().routes.last().unwrap().miso, Some(2));
}

#[test]
fn spi_additional_selects_mirrors_and_disabled_routes() {
    let mut m = esp32c6::machine([0; 6], 4 << 20);
    let state = Arc::new(Mutex::new(State::default()));
    m.bus.board = Box::new(Board(state.clone()));
    let b = &mut m.bus;
    for (cs, signal) in [68, 101, 102, 103, 104, 105].into_iter().enumerate() {
        let pin = 16 + cs as u32;
        b.write32(0x6009_0004 + 4 * pin, 1 << 12).unwrap();
        b.write32(0x6009_1554 + 4 * pin, signal).unwrap();
        b.write32(0x6008_1020, 0x3f & !(1 << cs)).unwrap();
        spi(b);
        assert_eq!(state.lock().unwrap().routes.last().unwrap().cs, 1 << pin);
        b.write32(0x6008_1020, (0x3f & !(1 << cs)) | (1 << (cs + 7)))
            .unwrap();
        spi(b);
        assert_eq!(state.lock().unwrap().routes.last().unwrap().cs, 0);
        b.write32(0x6009_0004 + 4 * pin, 2 << 12).unwrap();
        b.write32(0x6008_1020, 0x3f & !(1 << cs)).unwrap();
        spi(b);
        assert_eq!(state.lock().unwrap().routes.last().unwrap().cs, 1 << pin);
        b.write32(0x6009_0004 + 4 * pin, 0).unwrap();
    }
    for pin in [6, 8] {
        b.write32(0x6009_0004 + 4 * pin, 1 << 12).unwrap();
        b.write32(0x6009_1554 + 4 * pin, 63).unwrap();
    }
    spi(b);
    assert_eq!(
        state.lock().unwrap().routes.last().unwrap().sclk,
        1 << 6 | 1 << 8
    );
    for invalid in [63 | 1 << 8, 63 | 1 << 10, 63 | 1 << 9] {
        b.write32(0x6009_1574, invalid).unwrap();
        spi(b);
        assert_eq!(state.lock().unwrap().routes.last().unwrap().sclk, 1 << 6);
    }
    b.write32(0x6009_1024, 1 << 8).unwrap();
    spi(b);
    assert_eq!(
        state.lock().unwrap().routes.last().unwrap().sclk,
        1 << 6 | 1 << 8
    );
    for invalid in [0x80 | 31, 0x80 | 63, 0xc0 | 2] {
        b.write32(0x6009_1254, invalid).unwrap();
        spi(b);
        assert_eq!(state.lock().unwrap().routes.last().unwrap().miso, None);
    }
}

#[test]
fn spi_legacy_callback_remains_compatible() {
    struct Legacy;
    impl BoardModel for Legacy {
        fn name(&self) -> &'static str {
            "legacy-spi"
        }
        fn spi_transfer(&mut self, host: u8, tx: &[u8], n: usize) -> Vec<u8> {
            assert_eq!((host, tx), (2, &[0x5a][..]));
            vec![0x33; n]
        }
    }
    let mut m = esp32c6::machine([0; 6], 4 << 20);
    m.bus.board = Box::new(Legacy);
    assert_eq!(spi(&mut m.bus), 0x33);
}

#[test]
fn gpio_matrix_reset_uses_c6_selector_width() {
    let mut m = esp32c6::machine([0; 6], 4 << 20);
    for pin in 0..31 {
        let addr = 0x6009_1554 + pin * 4;
        assert_eq!(m.bus.read32(addr).unwrap(), 128);
        m.bus.write32(addr, 0x180).unwrap();
    }
    esp_soc::SocBus::reboot(&mut m.bus, [0; 6]);
    for pin in 0..31 {
        assert_eq!(m.bus.read32(0x6009_1554 + pin * 4).unwrap(), 128);
    }
}
