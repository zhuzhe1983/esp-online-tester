//! Opt-in physical phase transport, exercised through actual S3 MMIO writes.
use super::*;
use esp_soc::board::{BoardModel, SpiPins, SpiQspiPins};
use std::sync::{Arc, Mutex};

const SPI2: u32 = 0x6002_4000;
const SPI3: u32 = 0x6002_5000;
const DESC: u32 = 0x3fc9_0100;
const DATA: u32 = 0x3fc9_0200;
type Observed = Arc<Mutex<Vec<(u8, SpiQspiPins, Vec<u8>)>>>;

struct PhaseProbe(Observed);
impl BoardModel for PhaseProbe {
    fn name(&self) -> &'static str { "phase-probe" }
    fn uses_qspi_pins(&self) -> bool { true }
    fn spi_transfer(&mut self, _: u8, _: &[u8], _: usize) -> Vec<u8> { panic!("phase board fell back to legacy transport"); }
    fn spi_transfer_qspi_pins(&mut self, host: u8, pins: SpiQspiPins, tx: &[u8], rx: usize) -> Vec<u8> {
        self.0.lock().unwrap().push((host, pins, tx.to_vec()));
        vec![0xa6; rx]
    }
}
fn bus() -> (SocBus, Observed) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut b = SocBus::new(1024, 1024, [0; 6]);
    b.board = Box::new(PhaseProbe(seen.clone()));
    (b, seen)
}
fn route(b: &mut SocBus, pin: u32, signal: u32) {
    b.write32(0x6000_4554 + 4 * pin, signal).unwrap();
    b.write32(0x6000_9004 + 4 * pin, (1 << 12) | (1 << 9)).unwrap();
}
fn matrix(b: &mut SocBus, host: u8) {
    let signals = if host == 2 { [101, 103, 102, 105, 104, 110] } else { [66, 68, 67, 70, 69, 71] };
    for (pin, signal) in [6, 5, 4, 3, 2, 7].into_iter().zip(signals) { route(b, pin, signal); }
}
fn quad_write(b: &mut SocBus, base: u32) {
    b.write32(base + 4, 0x2c00_0000).unwrap();
    b.write32(base + 0x10, (1 << 31) | (1 << 30) | (1 << 27) | (1 << 13)).unwrap();
    b.write32(base + 0x14, 23 << 27).unwrap();
    b.write32(base + 0x18, (7 << 28) | 0x32).unwrap();
    b.write32(base + 0x1c, 31).unwrap();
    b.write32(base + 0x20, (1 << 30) | 0x3e).unwrap();
    b.write32(base + 0x98, 0x4433_2211).unwrap();
}
fn stopped_dma(b: &mut SocBus, host: u8) {
    b.write32(DATA, 0x4433_2211).unwrap();
    b.write32(DESC, (1 << 31) | (1 << 30) | (4 << 12) | 4).unwrap();
    b.write32(DESC + 4, DATA).unwrap();
    b.write32(DESC + 8, 0).unwrap();
    b.periph.gdma.out[0].peri_sel = u32::from(host - 2);
    b.periph.gdma.out[0].desc = DESC;
    b.periph.gdma.out[0].running = false;
    b.write32(if host == 2 { SPI2 } else { SPI3 } + 0x30, 1 << 28).unwrap();
}

#[test]
fn phase_snapshot_carries_real_four_lanes_native_routes_and_select_kind() {
    let (mut b, seen) = bus();
    matrix(&mut b, 2);
    route(&mut b, 8, 256);
    b.write32(0x6000_4020, 1 << 8).unwrap(); // GPIO8 enabled and low: software CS
    quad_write(&mut b, SPI2);
    b.write32(SPI2, 1 << 24).unwrap();
    let (host, p, bytes) = seen.lock().unwrap()[0].clone();
    assert_eq!(host, 2);
    assert_eq!(bytes, [0x32, 0x2c, 0, 0, 0x11, 0x22, 0x33, 0x44]);
    assert_eq!((p.sclk, p.data), (1 << 6, [1 << 5, 1 << 4, 1 << 3, 1 << 2]));
    assert_eq!((p.cs, p.hardware_cs, p.software_cs, p.keep_cs), ((1 << 7) | (1 << 8), 1 << 7, 1 << 8, true));
    assert_eq!((p.command_width, p.address_width, p.data_width, p.read_width), (1, 1, 4, 0));
    assert_eq!((p.command_bits, p.address_bits, p.data_bits, p.read_bits, p.dummy_cycles), (8, 24, 32, 0, 0));
    assert!(!p.unsupported_mode);
    // Native mux group 4: IO2/WP=14, IO3/HD=9. Disable all matrix routes.
    for pin in 0..49 { b.write32(0x6000_9004 + 4 * pin, 0).unwrap(); }
    for pin in [12, 11, 13, 14, 9, 10] { b.write32(0x6000_9004 + 4 * pin, (4 << 12) | (1 << 9)).unwrap(); }
    let p = b.periph.spi_qspi_pins(2);
    assert_eq!((p.sclk, p.data, p.hardware_cs), (1 << 12, [1 << 11, 1 << 13, 1 << 14, 1 << 9], 1 << 10));
    b.write32(0x6000_9004 + 4 * 14, 0).unwrap();
    assert_eq!(b.periph.spi_qspi_pins(2).data[2], 0, "wrong mux cannot count as an attached quad lane");
    assert_eq!(b.periph.spi_qspi_pins(3).data, [0; 4], "SPI3 has no native S3 pad group");
}

#[test]
fn phase_snapshot_retains_read_pins_disabled_phases_and_unsupported_modes() {
    let (mut b, _) = bus();
    matrix(&mut b, 3);
    for (pin, signal) in [(5, 68), (4, 67), (3, 70), (2, 69)] {
        b.write32(0x6000_4154 + 4 * signal, 0x80 | pin).unwrap();
    }
    b.write32(SPI3 + 0x10, (1 << 28) | (1 << 29) | (1 << 17)).unwrap();
    b.write32(SPI3 + 0x14, 7).unwrap();
    b.write32(SPI3 + 0x1c, 15).unwrap();
    b.write32(SPI3 + 0x08, 1 << 15).unwrap();
    let p = b.periph.spi_qspi_pins(3);
    assert_eq!((p.command_width, p.address_width, p.data_width, p.read_width), (0, 0, 0, 4));
    assert_eq!((p.command_bits, p.address_bits, p.data_bits, p.read_bits, p.dummy_cycles), (0, 0, 0, 16, 8));
    assert!(p.sio);
    assert_eq!(p.input, [Some(5), Some(4), Some(3), Some(2)]);
    b.write32(0x6000_4154 + 4 * 70, 0xc0 | 3).unwrap();
    assert_eq!(b.periph.spi_qspi_pins(3).input[2], None, "inverted input is not a valid uninverted data line");
    for value in [1 << 16, (1 << 15) | (1 << 14), 1 << 25] {
        b.write32(SPI3 + 8, value).unwrap();
        assert!(b.periph.spi_qspi_pins(3).unsupported_mode);
    }
    b.write32(SPI3 + 8, 0).unwrap();
    b.write32(SPI3 + 0x20, 1 << 17).unwrap();
    assert!(b.periph.spi_qspi_pins(3).unsupported_mode, "DTR must not become an SDR device write");
}

#[test]
fn phase_snapshot_survives_pending_dma_register_edits_and_busy_usr() {
    for (host, base) in [(2, SPI2), (3, SPI3)] {
        let (mut b, seen) = bus();
        matrix(&mut b, host);
        quad_write(&mut b, base);
        stopped_dma(&mut b, host);
        b.write32(base, 1 << 24).unwrap();
        assert!(b.periph.gpspi(host).has_pending_transfer());
        assert!(seen.lock().unwrap().is_empty());
        b.write32(base + 0x10, 1 << 27).unwrap();
        b.write32(base + 0x14, 0).unwrap();
        b.write32(base + 0x20, 0x3f).unwrap();
        route(&mut b, 3, 256);
        b.write32(base, 1 << 24).unwrap();
        b.write32(0x6003_f080, (1 << 21) | (DESC & 0xfffff)).unwrap();
        let got = seen.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].1.data_width, got[0].1.address_bits, got[0].1.keep_cs), (4, 24, true));
        assert_eq!(got[0].1.data[2], 1 << 3);
        assert_eq!(got[0].2, [0x32, 0x2c, 0, 0, 0x11, 0x22, 0x33, 0x44]);
    }
}

#[test]
fn phase_snapshot_scheduled_dma_busy_usr_fault_and_reboot_do_not_replay() {
    use esp_soc::soc::SocBus as _;
    let (mut b, seen) = bus();
    matrix(&mut b, 2);
    quad_write(&mut b, SPI2);
    stopped_dma(&mut b, 2);
    b.spi2_timing = true;
    b.periph.gdma.out[0].running = true;
    b.write32(SPI2, 1 << 24).unwrap();
    let deadline = b.spi2_scheduled.as_ref().unwrap().0;
    b.write32(SPI2, 1 << 24).unwrap();
    assert_eq!(b.spi2_scheduled.as_ref().unwrap().0, deadline, "busy USR does not restart wire timing");
    b.write32(0x6003_f060, 1).unwrap();
    assert!(b.spi2_scheduled.is_none());
    assert!(b.spi2_pins.is_none());
    assert!(!b.periph.spi2.has_pending_transfer());
    b.cycles = deadline + 1;
    b.complete_spi2_dma(); b.deliver_spi2_transfer();
    assert!(seen.lock().unwrap().is_empty());
    stopped_dma(&mut b, 2);
    b.write32(SPI2, 1 << 24).unwrap();
    b.reboot([0; 6]);
    assert!(b.spi2_pins.is_none());
    assert!(!b.periph.spi2.has_pending_transfer());
    assert!(seen.lock().unwrap().is_empty());
    // Unowned descriptor must drop the snapshot and allow a new CPU transfer.
    quad_write(&mut b, SPI2); stopped_dma(&mut b, 2);
    b.write32(DESC, (1 << 30) | (4 << 12) | 4).unwrap();
    b.periph.gdma.out[0].running = true;
    b.write32(SPI2, 1 << 24).unwrap();
    assert!(b.spi2_dma_fault.is_some()); assert!(b.spi2_pins.is_none());
    b.write32(SPI2 + 0x30, 0).unwrap();
    b.write32(SPI2, 1 << 24).unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn phase_transport_is_opt_in_and_empty_hook_does_not_fall_back() {
    struct Empty;
    impl BoardModel for Empty {
        fn name(&self) -> &'static str { "empty-phase-hook" }
        fn uses_qspi_pins(&self) -> bool { true }
        fn spi_transfer(&mut self, _: u8, _: &[u8], _: usize) -> Vec<u8> { panic!("legacy fallback"); }
    }
    struct Single(Arc<Mutex<Option<SpiPins>>>);
    impl BoardModel for Single {
        fn name(&self) -> &'static str { "legacy-single" }
        fn uses_spi_pins(&self) -> bool { true }
        fn spi_transfer_pins(&mut self, _: u8, p: SpiPins, _: &[u8], n: usize) -> Vec<u8> { *self.0.lock().unwrap() = Some(p); vec![0x3c; n] }
        fn spi_transfer_qspi_pins(&mut self, _: u8, _: SpiQspiPins, _: &[u8], _: usize) -> Vec<u8> { panic!("legacy board entered new callback"); }
    }
    let (mut b, _) = bus(); b.board = Box::new(Empty);
    b.write32(SPI2 + 0x10, 1 << 28).unwrap(); b.write32(SPI2 + 0x1c, 7).unwrap(); b.write32(SPI2, 1 << 24).unwrap();
    assert_eq!(b.periph.spi2.w[0], u32::MAX);
    let p = Arc::new(Mutex::new(None)); b.board = Box::new(Single(p.clone()));
    matrix(&mut b, 2); b.write32(SPI2 + 0x20, (1 << 30) | 0x3e).unwrap();
    b.write32(SPI2, 1 << 24).unwrap();
    assert_eq!(b.periph.spi2.w[0] & 255, 0x3c);
    assert!(p.lock().unwrap().unwrap().keep_cs, "the existing SpiPins contract is preserved");
}
