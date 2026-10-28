//! P1 command-only qualification: no CPU mapping, allocator or guest success claim.
//! Captured A2 2.0.17/A30 3.0.7 probe values: docs/fixtures/classic-psram/README.md (repository root).
//! Entry/reset controls are source-derived (IDF v4.4.7/v5.1.4), not captured
//! successful P0 transactions. Eight-byte ID tail is a deterministic model choice.
use super::ClassicSpi;
use esp_periph::{Device, DirtyMem, WriteEffect};

struct Rig { spi: ClassicSpi, flash: Vec<u8>, ram: Vec<u8> }
impl Rig {
    fn new(size: usize) -> Self {
        Self { spi: ClassicSpi::new(true), flash: vec![0xff; 4096], ram: vec![0; size] }
    }
    fn w(&mut self, off: u32, value: u32) { self.spi.write(off, value); }
    fn go(&mut self) {
        assert_eq!(self.spi.write(0, 1 << 18), WriteEffect::SPI_EXEC);
        self.spi.0.execute(&mut self.flash, &mut self.ram);
        assert_eq!(self.spi.read(0), 0, "synchronous command completion");
    }
    fn id(&mut self) -> [u32; 2] {
        self.w(0x1c, 0xf0000000);
        self.w(0x20, 0x5c000001);
        self.w(0x24, 0x7000009f);
        self.w(4, 0);
        self.w(0x2c, 0x3f);
        self.w(0x80, 0xa5a5a5a5);
        self.w(0x84, 0xa5a5a5a5);
        self.go();
        [self.spi.read(0x80), self.spi.read(0x84)]
    }
    fn enter(&mut self) {
        self.w(0x1c, 1 << 30);
        self.w(0x20, 7 << 26);
        self.w(4, 0x35000000);
        self.w(0x24, 0x7000009f); // stale command must not win
        self.go();
    }
    fn exit(&mut self) {
        self.w(0x1c, 0x08008000);
        self.w(0x28, 7);
        self.w(0x80, 0xf5);
        self.w(0x24, 0x70000035); // stale entry must not win
        self.go();
    }
    fn control(&mut self, opcode: u32) {
        self.w(0x1c, 1 << 31);
        self.w(0x24, 0x70000000 | opcode);
        self.go();
    }
    fn payload(&mut self, opcode: u32, write: bool, value: u32) -> u32 {
        self.w(0x1c, (1 << 31) | (1 << 30) | (1 << if write { 27 } else { 28 }));
        self.w(0x20, 23 << 26);
        self.w(0x24, 0x70000000 | opcode);
        self.w(4, 0x1200); // address 0x12, classic MSB-aligned
        self.w(0x28, 31);
        self.w(0x2c, 31);
        self.w(0x80, value);
        self.go();
        self.spi.read(0x80)
    }
}
// Source-derived DCLK shapes, IDF v5.5.1 esp_psram_impl_quad.c:421-519.
impl Rig {
    fn delay_id(&mut self) -> [u32; 2] {
        self.w(0x1c, (1 << 31) | (1 << 30) | (1 << 28));
        self.w(0x20, 31 << 26);
        self.w(0x24, 1 << 28); // two zero command bits
        self.w(4, 0x9f000000);
        self.w(0x2c, 63);
        self.go();
        [self.spi.read(0x80), self.spi.read(0x84)]
    }
}

#[test]
fn psram_spi_delay_clock_id_returns_device_id_and_rejects_wrong_phases() {
    let mut r = Rig::new(4 << 20);
    r.w(0x34, 5);
    assert_eq!(r.delay_id(), [0x00225d0d, 0]);
    for (reg, wrong) in [(0x24, (1 << 28) | 1), (0x24, 2 << 28), (0x20, 30 << 26), (0x2c, 31)] {
        r.delay_id();
        r.w(reg, wrong);
        r.go();
        assert_eq!(r.spi.read(0x80), u32::MAX, "malformed DCLK ID reg {reg:#x}");
    }
    r.enter();
    assert_eq!(r.delay_id(), [u32::MAX; 2], "QPI still gates DCLK ID");
    let mut absent = Rig::new(0);
    absent.w(0x34, 5);
    assert_eq!(absent.delay_id(), [u32::MAX; 2]);
}

#[test]
fn psram_spi_delay_clock_mosi_exit_restores_spi() {
    let mut r = Rig::new(4 << 20);
    r.w(0x34, 5);
    r.enter();
    assert!(!valid_id(r.id()));
    r.w(0x1c, 1 << 27);
    r.w(0x28, 15);
    r.w(0x80, 0xf500);
    r.go();
    assert!(valid_id(r.id()), "16-bit MOSI f500 exits QPI");
}

#[test]
fn psram_spi_delay_clock_entry_enters_qpi() {
    let mut r = Rig::new(4 << 20);
    r.w(0x34, 5);
    r.w(0x1c, (1 << 31) | (1 << 30));
    r.w(0x20, 7 << 26);
    r.w(0x24, 1 << 28);
    r.w(4, 0x35000000);
    r.go();
    assert!(!valid_id(r.id()), "two zero bits plus address 35 enters QPI");
    r.exit();
    assert!(valid_id(r.id()));
}

fn valid_id(id: [u32; 2]) -> bool { (id[0] >> 8) & 0xff == 0x5d }

#[test]
fn psram_spi_pinned_probe_eight_bytes_and_absence() {
    for profile in ["A2 2.0.17", "A30 3.0.7"] {
        for size in [0, 0x400000] {
            let mut r = Rig::new(size);
            r.w(0x34, 5);
            r.exit();
            let id = r.id();
            assert_eq!(valid_id(id), size != 0, "{profile}: KGD acceptance");
            assert_eq!(id, if size == 0 { [u32::MAX; 2] } else { [0x00225d0d, 0] }, "{profile}: all eight ID bytes");
            assert_eq!(r.id(), id, "retry is deterministic");
        }
    }
}

#[test]
fn psram_spi_address_entry_mosi_exit_and_reentry() {
    let mut r = Rig::new(0x400000);
    r.w(0x34, 5);
    for _ in 0..2 {
        r.enter();
        assert_eq!(r.id(), [u32::MAX; 2], "address-only entry blocks SPI ID in QPI");
        r.exit();
        assert!(valid_id(r.id()), "MOSI-only exit restores SPI ID despite stale USER2");
    }
}

#[test]
fn psram_spi_reset_requires_enable_and_is_consumed() {
    let mut r = Rig::new(0x400000);
    r.w(0x34, 5);
    r.enter();
    r.control(0x99);
    assert!(!valid_id(r.id()), "unarmed reset must retain QPI");
    r.control(0x66);
    r.control(0x99);
    assert!(valid_id(r.id()), "armed reset restores SPI mode");
    r.enter();
    r.control(0x66);
    r.control(0x00);
    r.control(0x99);
    assert!(!valid_id(r.id()), "intervening command consumes reset enable");
    r.exit();
    r.enter();
    assert!(!valid_id(r.id()), "entry works again after reset/exit");
}

#[test]
fn psram_spi_cs_selection_flash_isolation_and_disabled_cs() {
    let mut r = Rig::new(0x400000);
    r.w(0x34, 6);
    assert_eq!(r.id(), [0x00174020, 0], "CS0 keeps flash ID");
    r.enter(); // CS0 must not change PSRAM state
    r.w(0x34, 5);
    assert!(valid_id(r.id()), "CS0 address opcode must not enter PSRAM QPI");
    r.w(0x34, 7);
    assert_eq!(r.id(), [u32::MAX; 2], "disabled CS1 must not answer");
    r.payload(0x02, true, 0x12345678);
    assert_eq!(&r.flash[0x12..0x16], &[0xff; 4], "disabled CS cannot write flash");
    r.w(0x34, 5);
    r.payload(0x02, true, 0x12345678);
    assert_eq!(r.spi.0.dirty.pop(), Some((DirtyMem::Psram, 0x12, 4)));
    assert_eq!(&r.ram[0x12..0x16], &0x12345678u32.to_le_bytes());
    assert_eq!(&r.flash[0x12..0x16], &[0xff; 4], "CS1 write must not change flash");
    r.w(0x34, 6);
    r.payload(0x02, true, 0x11223344);
    assert_eq!(r.spi.0.dirty.pop(), Some((DirtyMem::Flash, 0x12, 4)));
    assert_eq!(&r.flash[0x12..0x16], &0x11223344u32.to_le_bytes());
    assert_eq!(&r.ram[0x12..0x16], &0x12345678u32.to_le_bytes());
    let mut cache = ClassicSpi::new(false);
    assert_eq!(cache.write(0, 1 << 18), WriteEffect::NONE, "SPI0 is not the user command engine");
}

#[test]
fn psram_spi_phase_lengths_and_payload_modes() {
    let mut r = Rig::new(0x400000);
    r.w(0x34, 5);
    assert!(valid_id(r.id()));
    // A stale 9f with its command phase disabled must not answer ID.
    r.w(0x1c, 0x70000000);
    r.go();
    assert_eq!(r.spi.read(0x80), u32::MAX, "disabled command phase is not an opcode");
    r.id();
    r.w(0x24, 0x6000009f);
    r.go();
    assert_eq!(r.spi.read(0x80), u32::MAX, "ID requires eight command bits");
    r.id();
    r.w(0x20, 7 << 26);
    r.go();
    assert_eq!(r.spi.read(0x80), u32::MAX, "ID requires 24 address bits");
    r.control(0x00);
    r.w(0x1c, (1 << 31) | (1 << 28));
    r.w(0x24, 0x70000035);
    r.go();
    assert!(valid_id(r.id()), "control with MISO must not enter QPI");
    r.w(0x1c, 1 << 30);
    r.w(0x20, 6 << 26);
    r.w(4, 0x35000000);
    r.go();
    assert!(valid_id(r.id()), "address-only entry requires eight bits");
    r.enter();
    r.w(0x1c, 0x08008000);
    r.w(0x28, 6);
    r.w(0x80, 0xf5);
    r.go();
    assert!(!valid_id(r.id()), "MOSI-only exit requires eight bits");
    r.exit();
    r.payload(0x38, true, 0xdeadbeef);
    assert!(r.ram.iter().all(|&b| b == 0), "QPI write rejected in SPI mode");
    r.payload(0x02, true, 0x12345678);
    assert_eq!(r.payload(0x03, false, 0), 0x12345678, "SPI roundtrip");
    r.enter();
    r.payload(0x02, true, 0xdeadbeef);
    assert_eq!(r.payload(0xeb, false, 0), 0x12345678, "SPI write rejected in QPI mode");
    r.payload(0x38, true, 0x87654321);
    assert_eq!(r.payload(0xeb, false, 0), 0x87654321, "QPI roundtrip shares existing backing");
    r.control(0x66);
    r.control(0x99);
    assert_eq!(r.payload(0x03, false, 0), 0x87654321, "device reset retains payload (not machine reboot)");
}
