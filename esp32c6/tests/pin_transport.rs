use emu_core::Bus;
use esp_periph::i2c::I2cDevice;
use esp_soc::board::{BoardEdge, BoardModel};
use esp_soc::SocBus as _;
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

fn route(bus: &mut esp32c6::bus::SocBus, sda: u8, scl: u8) {
    for (pin, sig) in [(sda, 46), (scl, 45)] {
        bus.write32(0x6009_0004 + 4 * pin as u32, 1 << 12 | 1 << 9)
            .unwrap();
        bus.write32(0x6009_1554 + 4 * pin as u32, sig as u32)
            .unwrap();
        bus.write32(0x6009_1154 + 4 * sig as u32, 0x80 | pin as u32)
            .unwrap();
    }
}

fn read_i2c(bus: &mut esp32c6::bus::SocBus) -> (bool, u8) {
    let base = 0x6000_4000;
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
    let mut m = esp32c6::machine([0; 6], 4 << 20);
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
    bus.write32(0x6009_0024, 1 << 12).unwrap(); // disable SDA IO_MUX input
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
    outputs: Vec<(u64, u64, u64)>,
    pending: Vec<BoardEdge>,
    due: Vec<BoardEdge>,
    delivered: Vec<BoardEdge>,
}
struct Board(Arc<Mutex<State>>);
impl BoardModel for Board {
    fn name(&self) -> &'static str {
        "c6-pin-test"
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        vec![(0, 0x42, Box::new(Device(Some((6, 7)), 0xa5)))]
    }
    fn input_levels(&self) -> Vec<(u8, bool)> {
        vec![(5, false)]
    }
    fn gpio_output_at(&mut self, cycle: u64, _: &[(u8, bool)], enabled: u64, out: u64) {
        let mut s = self.0.lock().unwrap();
        let was_high = s.outputs.last().is_some_and(|&(_, en, v)| en & v & 16 != 0);
        s.outputs.push((cycle, enabled, out));
        if was_high && enabled & 16 != 0 && out & 16 == 0 {
            s.pending.extend([
                BoardEdge {
                    cycle: cycle + 32_000,
                    pin: 5,
                    level: true,
                },
                BoardEdge {
                    cycle: cycle + 192_000,
                    pin: 5,
                    level: false,
                },
            ]);
        }
    }
    fn next_deadline(&self) -> Option<u64> {
        self.0.lock().unwrap().pending.first().map(|e| e.cycle)
    }
    fn advance_to(&mut self, cycle: u64) {
        let mut s = self.0.lock().unwrap();
        while s.pending.first().is_some_and(|e| e.cycle <= cycle) {
            let edge = s.pending.remove(0);
            s.due.push(edge);
            s.delivered.push(edge);
        }
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> {
        std::mem::take(&mut self.0.lock().unwrap().due)
    }
}

#[test]
fn board_reset_deadlines_interrupts_and_output_enable() {
    let state = Arc::new(Mutex::new(State::default()));
    let mut m = esp32c6::machine([0; 6], 4 << 20);
    let b = &mut m.bus;
    b.board = Box::new(Board(state.clone()));
    b.attach_board_devices();
    route(b, 6, 7);
    assert_eq!(read_i2c(b), (true, 0xa5));
    b.periph.intmtx.map[esp32c6::periph::src::I2C_EXT0] = 4;
    b.periph.intc.lines.enable = 1 << 4;
    b.periph.intc.lines.pri[4] = 1;
    b.write32(0x6000_4028, 1 << 7).unwrap();
    b.refresh_irq();
    assert_eq!(b.periph.intc.lines.pending(), Some(4));
    b.write32(0x6000_4024, u32::MAX).unwrap();
    b.refresh_irq();
    assert_eq!(b.periph.intc.lines.pending(), None);
    b.reboot([0; 6]);
    assert!(!read_i2c(b).0);
    route(b, 6, 7);
    assert_eq!(read_i2c(b), (true, 0xa5));
    assert_eq!(b.gpio_input() & (1 << 5), 0);
    let stimulus_cycle = b.cycles();
    b.observe_gpio(true);
    b.write32(0x6009_1024, 16).unwrap(); // enable GPIO4 low
    b.write32(0x6009_1028, 16).unwrap(); // release low, no level change
    assert_eq!(state.lock().unwrap().outputs.len(), 2);
    b.write32(0x6009_1024, 16).unwrap();
    b.write32(0x6009_1008, 16).unwrap();
    b.tick(16000);
    b.write32(0x6009_100c, 16).unwrap();
    b.write32(0x6009_1074 + 4 * 5, 3 << 7 | 1 << 13).unwrap();
    b.periph.intmtx.map[esp32c6::periph::src::GPIO] = 3;
    b.periph.intc.lines.enable = 1 << 3;
    b.periph.intc.lines.pri[3] = 1;
    b.irq_dirty = false;
    assert!(b.next_deadline().unwrap() <= 32000);
    b.tick(31999);
    assert_eq!(b.gpio_input() & 32, 0);
    b.tick(1);
    assert_ne!(b.gpio_input() & 32, 0);
    assert!(b.irq_dirty);
    b.refresh_irq();
    assert_eq!(b.periph.intc.lines.pending(), Some(3));
    b.write32(0x6009_104c, 32).unwrap();
    b.tick(160000);
    assert_eq!(b.gpio_input() & 32, 0);
    assert!(b.take_gpio_events().contains(&(stimulus_cycle + 48000, 5, true)));
    assert_eq!(
        state.lock().unwrap().delivered.last().unwrap().cycle,
        stimulus_cycle + 208000
    );
}

#[test]
fn c6_route_rejects_inversion_invalid_pins_and_disabled_output() {
    let mut m = esp32c6::machine([0; 6], 4 << 20);
    let b = &mut m.bus;
    b.periph
        .i2c
        .attach(0x42, Box::new(Device(Some((6, 7)), 11)));
    for invalid in [46 | 1 << 8, 46 | 1 << 10, 46 | 1 << 9] {
        route(b, 6, 7);
        b.write32(0x6009_1554 + 4 * 6, invalid).unwrap();
        assert!(!read_i2c(b).0);
    }
    b.write32(0x6009_1024, 1 << 6).unwrap();
    assert_eq!(read_i2c(b), (true, 11));
    for invalid in [0x80 | 63, 0x80 | 31, 0xc0 | 6, 6] {
        route(b, 6, 7);
        b.write32(0x6009_1154 + 4 * 46, invalid).unwrap();
        assert!(!read_i2c(b).0);
    }
}

#[test]
#[ignore = "set ESP32SIM_TRANSPORT_BUILD to the PlatformIO build directory; needs esp32c6_rev0_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_c6_arduino_pin_transport() {
    let build = std::path::PathBuf::from(std::env::var_os("ESP32SIM_TRANSPORT_BUILD")
        .expect("ESP32SIM_TRANSPORT_BUILD must name the PlatformIO build directory containing firmware.factory.bin"));
    let rom = esp_soc::rom::require("esp32c6_rev0_rom.elf");
    let state = Arc::new(Mutex::new(State::default()));
    let mut m = esp32c6::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    m.bus.board = Box::new(Board(state.clone()));
    m.bus.attach_board_devices();
    m.console.capture = true;
    m.console.mask = 2;
    m.load_rom(&std::fs::read(rom).expect("readable esp32c6_rev0_rom.elf")).unwrap();
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
    println!(
        "{console}\nstop={stop:?} cycles={} instructions={}",
        m.bus.cycles,
        m.insns()
    );
    assert!(console.contains("I2C right=0"));
    assert!(console.contains("I2C count=1 value=a5"));
    assert!(console.contains("I2C wrong=2"));
    assert!(console.contains("C6 TRANSPORT DONE"));
    let echo = console
        .lines()
        .find(|l| l.starts_with("ECHO width_us="))
        .unwrap();
    let width: u64 = echo
        .split_whitespace()
        .nth(1)
        .unwrap()
        .strip_prefix("width_us=")
        .unwrap()
        .parse()
        .unwrap();
    assert!(width.abs_diff(1000) <= 2, "{echo}");
    assert!(echo.ends_with("edges=2"));
    let s = state.lock().unwrap();
    let high = s
        .outputs
        .iter()
        .find(|&&(_, en, out)| en & out & 16 != 0)
        .unwrap()
        .0;
    let low = s
        .outputs
        .iter()
        .find(|&&(at, en, out)| at > high && en & 16 != 0 && out & 16 == 0)
        .unwrap()
        .0;
    println!(
        "pulse high_cycle={high} low_cycle={low} width_cycles={} input_edges={:?}",
        low - high,
        s.delivered
    );
    // Arduino polls a microsecond timer; allow 2 us of call/poll overhead plus a round.
    assert!((low - high).abs_diff(16000) <= 2 * 160 + 64);
    assert_eq!(s.delivered.len(), 2);
    assert_eq!(s.delivered[0].cycle, low + 32000);
    assert_eq!(s.delivered[1].cycle, low + 192000);
}
