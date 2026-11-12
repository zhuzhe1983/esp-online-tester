use esp_periph::Device;
use esp_periph::i2c::{I2c, I2cDevice, Reg8Device, INT_NACK};
use esp_soc::{BoardModel, GpioState, SocBus};

fn snapshot(bus: &mut dyn SocBus, gpio: u32, mux: u32, last: u8, holes: &[u8]) {
    for pin in 0..=last {
        if holes.contains(&pin) {
            assert_eq!(bus.gpio_state(pin), None);
            continue;
        }
        assert_eq!(bus.gpio_state(pin), Some(GpioState::default()));
        let bank = if pin >= 32 { 12 } else { 0 };
        let mask = 1 << (pin % 32);
        // Register effects of pinMode(INPUT_PULLUP), INPUT_PULLDOWN and OUTPUT.
        for (pulls, enable) in [(1 << 8, false), (1 << 7, false), (0, true), (3 << 7, false)] {
            bus.write32(mux + 4 + u32::from(pin) * 4, pulls | (1 << 9))
                .unwrap();
            bus.write32(gpio + bank + if enable { 0x24 } else { 0x28 }, mask)
                .unwrap();
            for output in [true, false] {
                bus.write32(gpio + bank + if output { 8 } else { 12 }, mask)
                    .unwrap();
                bus.gpio_set_input(pin, !output);
                let state = bus.gpio_state(pin).unwrap();
                assert_eq!(state.output, output);
                assert_eq!(state.output_enable, enable);
                assert_eq!(state.pull_up, pulls & (1 << 8) != 0);
                assert_eq!(state.pull_down, pulls & (1 << 7) != 0);
                assert_eq!(state, GpioState::new(output, enable, pulls & (1 << 8) != 0, pulls & (1 << 7) != 0));
            }
        }
    }
    for pin in [last + 1, 63, 64, 255] {
        assert_eq!(bus.gpio_state(pin), None);
    }
}

#[test]
fn s3_gpio_snapshot() {
    snapshot(
        &mut esp32s3::machine([0; 6]).bus,
        0x60004000,
        0x60009000,
        48,
        &[22, 23, 24, 25],
    );
}
#[test]
fn c3_gpio_snapshot() {
    let mut m = esp32c3::machine([0; 6], 4096);
    snapshot(&mut m.bus, 0x60004000, 0x60009000, 21, &[]);
    assert!(!m.bus.periph.misc.generic.contains_key(&0x09));
    m.bus.periph.io_mux.write(4, 1 << 8);
    assert!(m.bus.gpio_state(0).unwrap().pull_up);
    assert_eq!(m.bus.periph.read32(0x60009000 + 4), 1 << 8);
}
#[test]
fn c6_gpio_snapshot() {
    let mut m = esp32c6::machine([0; 6], 4096);
    snapshot(&mut m.bus, 0x60091000, 0x60090000, 30, &[]);
    assert!(!m.bus.periph.misc.generic.contains_key(&0x90));
    m.bus.periph.io_mux.write(4, 1 << 8);
    assert!(m.bus.gpio_state(0).unwrap().pull_up);
    assert_eq!(m.bus.periph.read32(0x60090000 + 4), 1 << 8);
}

fn device(value: u8) -> Box<dyn I2cDevice> {
    Box::new(Reg8Device::new("test", &[(0, value)]))
}
fn address(i2c: &mut I2c, addr: u8) {
    i2c.write(0x24, u32::MAX);
    i2c.write(0x1c, u32::from(addr) * 2 + 1);
    i2c.write(0x58, 6 << 11); // RSTART
    i2c.write(0x5c, (1 << 11) | (1 << 8) | 1);
    i2c.write(0x60, 4 << 11); // END, retain the selected device
    i2c.write(0x04, 1 << 5);
    i2c.tick(100_000);
}
fn read(i2c: &mut I2c) -> u32 {
    i2c.write(0x58, (3 << 11) | 1);
    i2c.write(0x5c, 4 << 11);
    i2c.write(0x04, 1 << 5);
    i2c.tick(100_000);
    i2c.read(0x1c)
}
#[test]
fn detach_preserves_other_selection_and_allows_move() {
    let mut i2c = I2c::new();
    i2c.attach(0x20, device(20));
    i2c.attach(0x21, device(21));
    i2c.attach(0x22, device(22));
    address(&mut i2c, 0x21);
    assert!(i2c.detach(0x7f).is_none());
    assert!(i2c.detach(0x22).is_some());
    let moved = i2c.detach(0x20).unwrap();
    assert_eq!(read(&mut i2c), 21);
    assert!(i2c.detach(0x21).is_some());
    assert_eq!(read(&mut i2c), 0xff);
    i2c.write(0x1c, 0x55);
    i2c.write(0x58, (1 << 11) | (1 << 8) | 1);
    i2c.write(0x04, 1 << 5);
    i2c.tick(100_000);
    assert_ne!(i2c.int_raw & INT_NACK, 0);
    address(&mut i2c, 0x21);
    assert_ne!(i2c.int_raw & INT_NACK, 0);
    i2c.attach(0x30, moved);
    address(&mut i2c, 0x30);
    assert_eq!(i2c.int_raw & INT_NACK, 0);
    assert_eq!(read(&mut i2c), 20);
    i2c.write(0x28, INT_NACK);
    let before = (i2c.transactions, i2c.int_raw);
    i2c.clear_devices();
    i2c.clear_devices();
    assert_eq!((i2c.transactions, i2c.int_raw), before);
    assert_eq!(i2c.int_ena, INT_NACK);
    assert!(!i2c.has_device(0x30));
    assert_eq!(read(&mut i2c), 0xff);
    address(&mut i2c, 0x30);
    assert_ne!(i2c.int_raw & INT_NACK, 0);
}

struct Board(u8);
impl BoardModel for Board {
    fn name(&self) -> &'static str {
        "test"
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        vec![(self.0, 0x20, device(42))]
    }
}
#[test]
fn board_devices_can_be_removed_or_moved_without_reset() {
    let mut m = esp32s3::machine([0; 6]);
    m.bus.board = Box::new(Board(0));
    m.bus.attach_board_devices();
    address(&mut m.bus.periph.i2c[0], 0x20);
    assert_eq!(read(&mut m.bus.periph.i2c[0]), 42);
    m.bus.clear_i2c_devices();
    assert_eq!(read(&mut m.bus.periph.i2c[0]), 0xff);
    m.bus.board = Box::new(Board(1));
    m.bus.attach_board_devices();
    address(&mut m.bus.periph.i2c[0], 0x20);
    assert_ne!(m.bus.periph.i2c[0].int_raw & INT_NACK, 0);
    address(&mut m.bus.periph.i2c[1], 0x20);
    assert_eq!(read(&mut m.bus.periph.i2c[1]), 42);
    m.bus.clear_i2c_devices();
    m.bus.board = Box::new(esp_soc::NoBoard);
    m.bus.attach_board_devices();
    assert!(!m.bus.periph.i2c[1].has_device(0x20));
}

/// Build the sketch in docs/evidence/gpio-i2c-hooks-2026-10-02, then set
/// HOOKS_FIRMWARE (the PlatformIO build directory). The S3 ROM comes from ESP32SIM_ROM_DIR or
/// ESP-IDF, or from HOOKS_ROM (a file), the name the evidence README uses.
#[test]
#[ignore = "needs the S3 mask ROM ELF (ESP32SIM_ROM_DIR, ESP-IDF or HOOKS_ROM); set HOOKS_FIRMWARE to the GPIO/I2C sketch PlatformIO build directory"]
fn external_arduino_s3_gpio_and_i2c_detach() {
    use std::{env, fs, path::PathBuf};
    let firmware = PathBuf::from(env::var_os("HOOKS_FIRMWARE").expect("set HOOKS_FIRMWARE to the PlatformIO build directory for docs/evidence/gpio-i2c-hooks-2026-10-02"));
    let mut m = esp32s3::machine([0; 6]);
    m.bus.board = Box::new(esp_soc::NoBoard);
    m.console.capture = true;
    let rom = env::var_os("HOOKS_ROM").map(PathBuf::from).unwrap_or_else(|| esp_soc::rom::require("esp32s3_rev0_rom.elf"));
    m.load_rom(&fs::read(rom).unwrap()).unwrap();
    for (offset, name) in [
        (0, "bootloader.bin"),
        (0x8000, "partitions.bin"),
        (0x10000, "firmware.bin"),
    ] {
        m.write_flash(offset, &fs::read(firmware.join(name)).unwrap())
            .unwrap();
    }
    m.bus.periph.i2c[0].attach(0x42, device(42));
    m.boot_rom();
    let expected = [
        "HOOK pullup",
        "HOOK pulldown",
        "HOOK output",
        "HOOK i2c 0 0",
        "HOOK i2c 1 2",
        "HOOK i2c 2 0",
        "HOOK i2c 3 2",
        "HOOK done",
    ];
    let mut phase = 0;
    let mut consumed = 0;
    let mut moved = None;
    while m.seconds() < 10.0 && phase < expected.len() {
        assert!(matches!(m.run(1_000_000), esp_soc::Stop::MaxInsns));
        let console = String::from_utf8_lossy(&m.console.uart0);
        let Some(end) = console[consumed..].rfind('\n').map(|n| consumed + n + 1) else {
            continue;
        };
        let lines = console[consumed..end].to_owned();
        consumed = end;
        for line in lines.lines().filter(|line| line.starts_with("HOOK ")) {
            println!("{line}");
            assert_eq!(line, expected[phase]);
            match phase {
                0..=2 => {
                    let state = m.bus.gpio_state(4).unwrap();
                    assert_eq!(state, GpioState::new(phase == 2, phase == 2, phase == 0, phase == 1));
                    println!("snapshot {state:?}");
                }
                3 => moved = m.bus.periph.i2c[0].detach(0x42),
                4 => m.bus.periph.i2c[0].attach(0x42, moved.take().unwrap()),
                5 => m.bus.clear_i2c_devices(),
                _ => {}
            }
            phase += 1;
            m.bus.uart_input(0, b"\n");
        }
    }
    assert_eq!(
        phase,
        expected.len(),
        "serial: {}",
        String::from_utf8_lossy(&m.console.uart0)
    );
    println!(
        "cycles={} instructions={} reboots={}",
        m.bus.cycles(),
        m.insns(),
        m.reboots
    );
    assert_eq!(m.reboots, 0);
}
