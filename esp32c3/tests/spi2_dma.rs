//! GP-SPI2 MOSI data supplied by GDMA, as esp-hal's `SpiDma` drives it.
use emu_core::Bus;
use esp32c3::bus::{SocBus, DRAM_LOW as RAM};
use esp_soc::board::BoardModel;
use std::sync::{Arc, Mutex};

const SPI2: u32 = 0x6002_4000;
const CONF0: u32 = 0x6003_f0d0;
const LINK: u32 = 0x6003_f0e0;
const CONF1: u32 = 0x6003_f0d4;
const OUTFIFO: u32 = 0x6003_f0d8;
const SELECT: u32 = 0x6003_f100;

struct Capture(Arc<Mutex<Vec<u8>>>);
impl BoardModel for Capture {
    fn name(&self) -> &'static str { "capture" }
    fn spi_tx(&mut self, host: u8, data: &[u8]) { assert_eq!(host, 2); self.0.lock().unwrap().extend_from_slice(data); }
    fn uses_gpio_edges(&self) -> bool { false }
}

/// Two descriptors (3 + 2 bytes, EOF on the second), SPI2 set for a 5-byte DMA MOSI phase.
fn fixture() -> (SocBus, Arc<Mutex<Vec<u8>>>) {
    let mut bus = SocBus::new(4 << 20, [0; 6]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    bus.board = Box::new(Capture(seen.clone()));
    for (i, b) in [0x2c, 0xf8, 0x00, 0x07, 0xe0].into_iter().enumerate() { bus.write8(RAM + 0x100 + i as u32, b).unwrap(); }
    for (desc, len, buf, next, eof) in [(RAM, 3, RAM + 0x100, RAM + 16, 0), (RAM + 16, 2, RAM + 0x103, 0, 1)] {
        bus.write32(desc, (1 << 31) | (eof << 30) | (len << 12) | len).unwrap();
        bus.write32(desc + 4, buf).unwrap();
        bus.write32(desc + 8, next).unwrap();
    }
    bus.write32(SELECT, 0).unwrap();                     // PERI_OUT_SEL: SPI2
    bus.write32(CONF0, 1 << 2).unwrap();                 // OUT_AUTO_WRBACK
    bus.write32(SPI2 + 0x10, 1 << 27).unwrap();          // USER: USR_MOSI only
    bus.write32(SPI2 + 0x1c, 5 * 8 - 1).unwrap();        // MS_DLEN
    bus.write32(SPI2 + 0x30, 1 << 28).unwrap();          // DMA_CONF: DMA_TX_ENA
    (bus, seen)
}

fn check(bus: &mut SocBus, seen: &Arc<Mutex<Vec<u8>>>) {
    assert_eq!(*seen.lock().unwrap(), [0x2c, 0xf8, 0x00, 0x07, 0xe0]);
    assert_eq!(bus.read32(SPI2 + 0x3c).unwrap() & (1 << 12), 1 << 12, "TRANS_DONE");
    assert_eq!(bus.read32(SPI2).unwrap() & (1 << 24), 0, "USR clear");
    let ch = bus.periph.gdma.state.out[0];
    assert_eq!(ch.int_raw & 0b1011, 0b1011, "OUT_DONE, OUT_EOF, OUT_TOTAL_EOF");
    assert_eq!((ch.running, ch.eof_desc), (false, RAM + 16));
    assert_eq!(bus.read32(RAM).unwrap() >> 31, 0, "owner handed back");
    assert_eq!(bus.read32(RAM + 16).unwrap() >> 31, 0);
}

#[test]
fn link_started_before_usr() {
    let (mut bus, seen) = fixture();
    // OUTFIFO_STATUS_CH0 (0xd8): idle reads empty (bit 1); a running channel holding a descriptor
    // is neither empty nor full (bit 0). Reading both pins the C3 offset mapping.
    assert_ne!(bus.read32(OUTFIFO).unwrap() & 0b10, 0, "FIFO empty while idle");
    bus.write32(LINK, (1 << 21) | (RAM & 0xfffff)).unwrap();
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(bus.read32(OUTFIFO).unwrap() & 0b11, 0, "FIFO not empty while the channel runs");
    bus.write32(SPI2, 1 << 24).unwrap();                 // CMD.USR
    check(&mut bus, &seen);
}

#[test]
fn usr_before_link_waits_for_the_channel() {
    let (mut bus, seen) = fixture();
    bus.write32(SPI2, 1 << 24).unwrap();
    assert!(seen.lock().unwrap().is_empty(), "no data without a running channel");
    assert_ne!(bus.read32(SPI2).unwrap() & (1 << 24), 0, "USR busy");
    bus.write32(LINK, (1 << 21) | (RAM & 0xfffff)).unwrap();
    check(&mut bus, &seen);
}

#[test]
fn a_descriptor_the_cpu_owns_faults_the_transfer() {
    let (mut bus, seen) = fixture();
    bus.write32(CONF1, 1 << 12).unwrap();                // OUT_CHECK_OWNER
    bus.write32(RAM, (3 << 12) | 3).unwrap();            // owner bit clear
    bus.write32(LINK, (1 << 21) | (RAM & 0xfffff)).unwrap();
    bus.write32(SPI2, 1 << 24).unwrap();
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(bus.periph.gdma.state.out[0].int_raw & (1 << 2), 1 << 2, "OUT_DSCR_ERR");
    assert_eq!(bus.read32(SPI2 + 0x3c).unwrap() & (1 << 12), 1 << 12, "the command still completes");
}

/// Silicon (C3 rev 0.4) uses a CPU-owned descriptor without error while OUT_CHECK_OWNER is clear, its reset value.
#[test]
fn the_owner_bit_is_ignored_unless_checked() {
    let (mut bus, seen) = fixture();
    bus.write32(RAM, (3 << 12) | 3).unwrap();            // owner bit clear, OUT_CHECK_OWNER clear
    bus.write32(LINK, (1 << 21) | (RAM & 0xfffff)).unwrap();
    bus.write32(SPI2, 1 << 24).unwrap();
    assert_eq!(*seen.lock().unwrap(), [0x2c, 0xf8, 0x00, 0x07, 0xe0]);
    assert_eq!(bus.periph.gdma.state.out[0].int_raw & (1 << 2), 0, "no OUT_DSCR_ERR");
}
