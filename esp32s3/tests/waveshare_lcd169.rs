//! Original LX7 diagnostics exercise real GPIO, GP-SPI and I2C MMIO.
//! These are not manufacturer firmware or physical hardware acceptance.
use emu_core::{Bus, Core};
use esp_soc::{SocBus, Stop};

const PC: u32 = 0x4037_0000;
const GPIO: u32 = 0x6000_4000;
const SPI: u32 = 0x6002_4000;
struct Guest { m: esp32s3::Machine }
impl Guest {
    fn new(name: &str) -> Self {
        let mut m = esp32s3::machine([0; 6]);
        m.bus.board = esp32s3::board::make_board(name).unwrap();
        m.bus.attach_board_devices();
        m.bus.load_bytes(PC, &[0x32, 0x64, 0, 0x52, 0x24, 0]).unwrap();
        m.cores[0].ps = 0; m.cores[0].set_jit(false);
        Self { m }
    }
    fn put(&mut self, address: u32, value: u32) {
        self.m.cores[0].pc = PC; self.m.cores[0].set_ar(4, address);
        self.m.cores[0].set_ar(3, value);
        assert!(matches!(self.m.run(1), Stop::MaxInsns));
    }
    fn get(&mut self, address: u32) -> u32 {
        self.m.cores[0].pc = PC + 3; self.m.cores[0].set_ar(4, address);
        assert!(matches!(self.m.run(1), Stop::MaxInsns)); self.m.cores[0].get_ar(5)
    }
    fn route(&mut self, pin: u8, signal: u32) {
        self.put(0x6000_9004 + 4 * u32::from(pin), 1 << 12 | 1 << 9);
        self.put(GPIO + 0x554 + 4 * u32::from(pin), signal);
        if signal == 256 { self.put(GPIO + if pin < 32 { 0x24 } else { 0x30 }, 1 << (pin % 32)); }
    }
    fn high(&mut self, pin: u8, value: bool) {
        self.put(GPIO + if pin < 32 { if value { 8 } else { 12 } } else if value { 20 } else { 24 }, 1 << (pin % 32));
    }
    fn i2c_route(&mut self, sda: u8, scl: u8) {
        for (pin, signal) in [(sda, 90), (scl, 89)] {
            self.route(pin, signal); self.put(GPIO + 0x154 + 4 * signal, 0x80 | u32::from(pin));
        }
    }
    fn i2c(&mut self, address: u8, reg: u8, bytes: &[u8], count: u8) -> Option<Vec<u8>> {
        let base = 0x6001_3000;
        self.put(base + 0x24, u32::MAX); self.put(base + 0x18, 3 << 12);
        for v in [address << 1, reg].into_iter().chain(bytes.iter().copied()) { self.put(base + 0x1c, u32::from(v)); }
        let mut commands = vec![6 << 11, 1 << 11 | 1 << 8 | (bytes.len() + 2) as u32];
        if count > 0 {
            self.put(base + 0x1c, u32::from(address << 1 | 1));
            commands.extend([6 << 11, 1 << 11 | 1 << 8 | 1, 3 << 11 | u32::from(count)]);
        }
        commands.push(2 << 11);
        for (i, command) in commands.into_iter().enumerate() { self.put(base + 0x58 + 4 * i as u32, command); }
        self.put(base + 4, 1 << 5); self.m.bus.tick(100_000);
        if self.get(base + 0x20) & (1 << 10) != 0 { return None; }
        assert_ne!(self.get(base + 0x20) & (1 << 7), 0);
        Some((0..count).map(|_| self.get(base + 0x1c) as u8).collect())
    }
    fn spi(&mut self, dc: bool, bytes: &[u8], duplex: bool, mode3: bool) {
        self.high(4, dc);
        self.put(SPI + 0x20, 0x3e | if mode3 { 1 << 29 } else { 0 });
        self.put(SPI + 0x10, 1 << 27 | if duplex { 1 << 28 | 1 } else { 0 });
        self.put(SPI + 0x1c, (bytes.len() * 8 - 1) as u32);
        for (i, chunk) in bytes.chunks(4).enumerate() {
            self.put(SPI + 0x98 + 4 * i as u32, chunk.iter().enumerate().fold(0, |v, (j, b)| v | u32::from(*b) << (8 * j)));
        }
        self.put(SPI, 1 << 24); self.m.bus.flush_ticks();
    }
    fn cmd(&mut self, c: u8, data: &[u8], duplex: bool, mode3: bool) {
        self.spi(false, &[c], duplex, mode3);
        if !data.is_empty() { self.spi(true, data, duplex, mode3); }
    }
}

#[test]
fn both_revisions_receive_guest_spi_full_duplex_mosi_and_keep_the_20_row_glass_gap() {
    for name in ["waveshare-s3-lcd169-v1", "waveshare-s3-lcd169-v2"] {
        for (duplex, mode3) in [(false, false), (true, false), (true, true)] {
            let mut g = Guest::new(name);
            for (pin, signal) in [(6, 101), (7, 103), (5, 110), (4, 256), (8, 256), (15, 256)] { g.route(pin, signal); }
            g.high(8, true); g.high(15, true);
            for c in [0x11, 0x29] { g.cmd(c, &[], duplex, mode3); }
            g.cmd(0x3a, &[0x55], duplex, mode3);
            g.cmd(0x2a, &[0, 0, 0, 1], duplex, mode3);
            g.cmd(0x2b, &[0, 20, 0, 20], duplex, mode3);
            g.cmd(0x2c, &[0xf8, 0, 0x07, 0xe0], duplex, mode3);
            assert_eq!(&g.m.bus.board.display().unwrap().2[..2], &[0xf800, 0x07e0]);
            g.route(7, 102);
            g.cmd(0x2c, &[0, 0, 0, 0], duplex, mode3);
            assert_eq!(&g.m.bus.board.display().unwrap().2[..2], &[0xf800, 0x07e0], "wrong MOSI cannot replace actual pixels");
            g.high(15, false); assert!(g.m.bus.board.display().unwrap().2.iter().all(|v| *v == 0));
            g.high(15, true); assert_eq!(g.m.bus.board.display().unwrap().2[0], 0xf800);
            g.high(8, false); g.high(8, true);
            assert!(g.m.bus.board.display().unwrap().2.iter().all(|v| *v == 0), "reset cannot preserve an illuminated stale framebuffer");
        }
    }
}

#[test]
fn actual_guest_reads_revision_power_keys_qmi8658c_and_rtc_on_exact_pads() {
    for (name, key) in [("waveshare-s3-lcd169-v1", 36), ("waveshare-s3-lcd169-v2", 40)] {
        let mut g = Guest::new(name);
        assert_eq!(g.m.bus.board.named_pin("power"), Some(key));
        g.route(key, 256); g.put(GPIO + 0x34, 1 << (key - 32));
        assert_ne!(g.get(GPIO + 0x40) & (1 << (key - 32)), 0);
        g.m.bus.gpio_set_input(key, false);
        assert_eq!(g.get(GPIO + 0x40) & (1 << (key - 32)), 0);
        g.m.bus.gpio_set_input(key, true);
        assert_ne!(g.get(GPIO + 0x40) & (1 << (key - 32)), 0);
        g.i2c_route(11, 10);
        assert_eq!(g.i2c(0x6b, 0, &[], 1), Some(vec![5]));
        assert_eq!(g.i2c(0x51, 4, &[], 1), Some(vec![0x80]));
        for absent in [0x15, 0x34, 0x38] { assert_eq!(g.i2c(absent, 0, &[], 1), None); }
        g.i2c(0x51, 4, &[0x58], 0).unwrap();
        g.m.bus.tick(240_000_000); g.m.bus.board.advance_to(g.m.bus.cycles);
        assert_eq!(g.i2c(0x51, 4, &[], 1), Some(vec![0x59]));
        g.i2c_route(10, 11); assert_eq!(g.i2c(0x6b, 0, &[], 1), None);
        assert!(g.m.bus.board.sd_card_slot().is_none()); assert!(g.m.bus.board.leds().is_none());
    }
}
