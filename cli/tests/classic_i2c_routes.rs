use emu_core::Bus;

#[test]
#[ignore = "needs the classic mask ROM ELF (ESP32SIM_ROM_DIR, ESP-IDF or CLASSIC_I2C_ROM); set CLASSIC_I2C_FIRMWARE to the PlatformIO classic build directory"]
fn external_arduino_classic_i2c_routes() {
    use std::{env, fs, path::PathBuf};
    let firmware = PathBuf::from(env::var_os("CLASSIC_I2C_FIRMWARE")
        .expect("set CLASSIC_I2C_FIRMWARE to the build directory for docs/evidence/classic-i2c-routes/firmware"));
    let mut m = esp32::machine([0; 6], 4 * 1024 * 1024);
    m.console.capture = true;
    // CLASSIC_I2C_ROM (a file) is the name the evidence README uses; otherwise the shared lookup.
    let rom = env::var_os("CLASSIC_I2C_ROM").map(PathBuf::from).unwrap_or_else(|| esp_soc::rom::require("esp32_rev300_rom.elf"));
    m.load_rom(&fs::read(rom).unwrap()).unwrap();
    for (offset, name) in [(0x1000, "bootloader.bin"), (0x8000, "partitions.bin"), (0x10000, "firmware.bin")] {
        m.write_flash(offset, &fs::read(firmware.join(name)).unwrap()).unwrap();
    }
    for port in 0..2 {
        for pins in [(21, 22), (18, 19)] {
            m.bus.periph.i2c[port].attach(0x29, Box::new(Pinned { pins, seen: Arc::new(Mutex::new(Vec::new())) }));
        }
    }
    m.boot_rom();
    while m.seconds() < 10.0 && !String::from_utf8_lossy(&m.console.uart0).contains("ROUTE:done") {
        assert!(matches!(m.run(1_000_000), esp_soc::Stop::MaxInsns));
    }
    let console = String::from_utf8_lossy(&m.console.uart0);
    let lines: Vec<_> = console.lines().filter(|line| line.starts_with("ROUTE:")).collect();
    println!("{console}");
    assert_eq!(lines, ["ROUTE:0:21:22:0:21", "ROUTE:0:18:19:0:18", "ROUTE:0:21:19:4:-1",
        "ROUTE:1:21:22:0:21", "ROUTE:1:18:19:0:18", "ROUTE:1:21:19:4:-1", "ROUTE:done"]);
    assert_eq!(m.reboots, 0);
}
use esp_periph::i2c::{I2c, I2cDevice, INT_NACK, INT_TRANS_COMPLETE};
use esp_periph::Device;
use std::sync::{Arc, Mutex};

struct Pinned { pins: (u8, u8), seen: Arc<Mutex<Vec<u8>>> }
impl I2cDevice for Pinned {
    fn pins(&self) -> Option<(u8, u8)> { Some(self.pins) }
    fn write(&mut self, byte: u8) -> bool { self.seen.lock().unwrap().push(byte); true }
    fn read(&mut self) -> u8 { self.pins.0 }
}

fn route(bus: &mut esp32::bus::SocBus, port: usize, pins: (u8, u8)) {
    // IDF v5.5.4 esp32 gpio_sig_map.h: I2CEXT0 SCL/SDA 29/30, EXT1 95/96.
    // io_mux_reg.h: GPIO21/22 offsets 0x7c/0x80, GPIO18/19 0x70/0x74.
    let (scl, sda) = [(29, 30), (95, 96)][port];
    for (pin, signal) in [(pins.0, sda), (pins.1, scl)] {
        let mux = match pin { 18 => 0x70, 19 => 0x74, 21 => 0x7c, 22 => 0x80, _ => unreachable!() };
        bus.write32(0x3ff4_9000 + mux, (2 << 12) | (1 << 9) | (1 << 8)).unwrap();
        bus.write32(0x3ff4_4130 + 4 * signal, (1 << 7) | u32::from(pin)).unwrap();
        bus.write32(0x3ff4_4530 + 4 * u32::from(pin), signal).unwrap();
    }
}
fn send(bus: &mut esp32::bus::SocBus, base: u32, byte: u8, end: bool) -> u32 {
    bus.write32(base + 0x24, u32::MAX).unwrap();
    bus.write32(base + 0x18, 3 << 12).unwrap();
    for b in [0x52, byte] { bus.write32(base + 0x1c, u32::from(b)).unwrap(); }
    for (i, cmd) in [0, (1 << 11) | (1 << 8) | 2, (if end { 4 } else { 3 }) << 11].into_iter().enumerate() {
        bus.write32(base + 0x58 + 4 * i as u32, cmd).unwrap();
    }
    bus.write32(base + 4, 1 << 5).unwrap();
    bus.read32(base + 0x20).unwrap()
}

#[test]
fn classic_i2c_routes_both_ports_and_aliases() {
    for (port, apb) in [(0, 0x3ff5_3000), (1, 0x3ff6_7000)] {
        let mut bus = esp32::bus::SocBus::new(4 * 1024 * 1024, [0; 6]);
        let a = Arc::new(Mutex::new(Vec::new()));
        let b = Arc::new(Mutex::new(Vec::new()));
        bus.periph.i2c[port].attach(0x29, Box::new(Pinned { pins: (21, 22), seen: a.clone() }));
        bus.periph.i2c[port].attach(0x29, Box::new(Pinned { pins: (18, 19), seen: b.clone() }));
        for (base, pins, byte) in [(apb, (21, 22), 7), (apb - 0x3ff4_0000 + 0x6000_0000, (18, 19), 9)] {
            route(&mut bus, port, pins);
            assert_eq!(send(&mut bus, base, byte, false) & INT_NACK, 0);
            assert_ne!(bus.read32(base + 0x20).unwrap() & INT_TRANS_COMPLETE, 0);
        }
        assert_eq!(*a.lock().unwrap(), [7]);
        assert_eq!(*b.lock().unwrap(), [9]);
        // Each individual wire must match, not just SDA or SCL.
        for pins in [(21, 19), (18, 22)] {
            route(&mut bus, port, pins);
            assert_ne!(send(&mut bus, apb, 11, false) & INT_NACK, 0);
        }
        // An input route alone is insufficient, even when the sampled line is high.
        route(&mut bus, port, (21, 22));
        bus.write32(0x3ff4_4530 + 4 * 21, 256).unwrap();
        assert_ne!(send(&mut bus, apb, 13, false) & INT_NACK, 0);
        assert_eq!(*a.lock().unwrap(), [7]);
        assert_eq!(*b.lock().unwrap(), [9]);
    }
}

#[test]
fn classic_i2c_bit10_retains_end_recipient() {
    let mut i2c = I2c::new_classic();
    let seen = Arc::new(Mutex::new(Vec::new()));
    i2c.attach(0x29, Box::new(Pinned { pins: (21, 22), seen: seen.clone() }));
    i2c.set_pins(Some((21, 22)));
    i2c.write(0x1c, 0x52);
    i2c.write(0x58, 0);
    i2c.write(0x5c, (1 << 11) | (1 << 8) | 1);
    i2c.write(0x60, 4 << 11); // END, not STOP: retain addressed recipient.
    i2c.write(4, 1 << 5);
    i2c.write(4, (1 << 10) | (1 << 4));
    // Classic bit 10 must not reset the recipient; reserved-bit readback is not a contract.
    i2c.write(0x1c, 0x77);
    i2c.write(0x58, (1 << 11) | (1 << 8) | 1);
    i2c.write(0x5c, 3 << 11);
    i2c.write(4, (1 << 10) | (1 << 5));
    assert_eq!(*seen.lock().unwrap(), [0x77]);
    assert_eq!(i2c.int_raw & INT_NACK, 0);
}

#[test]
fn modern_i2c_bit10_cancels_pending_transfer() {
    let mut i2c = I2c::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    i2c.attach(0x29, Box::new(Pinned { pins: (21, 22), seen: seen.clone() }));
    i2c.set_pins(Some((21, 22)));
    i2c.write(0x1c, 0x52);
    i2c.write(0x1c, 0x77);
    i2c.write(0x58, 6 << 11);
    i2c.write(0x5c, (1 << 11) | 2);
    i2c.write(0x60, 2 << 11);
    i2c.write(4, 1 << 5);
    assert!(i2c.is_active());
    assert_eq!(i2c.int_raw & INT_TRANS_COMPLETE, 0);
    i2c.write(4, (1 << 10) | (1 << 4));
    assert!(!i2c.is_active());
    assert_eq!(i2c.read(4), 1 << 4);
    i2c.tick(100_000);
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(i2c.int_raw & INT_TRANS_COMPLETE, 0);
}
