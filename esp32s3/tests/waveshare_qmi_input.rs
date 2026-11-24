//! Host stimulus -> shared QMI -> actual S3 I2C command/FIFO -> Xtensa load.
//! Wiring/source scope is in the two board runtime documents and QST Rev A.
//! This is a register transport test, not a claim that the factory UI handles
//! motion or that sensor axes are calibrated to the physical PCB or 3D model.
use emu_core::{Bus, Core};
use esp_soc::{BoardModel, Stop};
use esp32s3::board::{
    waveshare_spi_legacy::{LegacySpiVariant, WaveshareLegacySpi},
    waveshare_tca_rgb::{TcaRgbVariant, WaveshareTcaRgb},
};

const HZ: u32 = 240_000_000;
const PC: u32 = 0x4037_1800;

fn route(m: &mut esp32s3::Machine, controller: u32, sda: u8, scl: u8) {
    for (pin, signal) in [(sda, 90 + controller * 2), (scl, 89 + controller * 2)] {
        m.bus.write32(0x6000_9004 + 4 * u32::from(pin), 1 << 12 | 1 << 9).unwrap();
        m.bus.write32(0x6000_4554 + 4 * u32::from(pin), signal).unwrap();
        m.bus.write32(0x6000_4154 + 4 * signal, 0x80 | u32::from(pin)).unwrap();
    }
}

fn transaction(m: &mut esp32s3::Machine, controller: u32, address: u8, register: u8,
               data: &[u8], read_len: u8) -> Option<Vec<u8>> {
    let base = [0x6001_3000, 0x6002_7000][controller as usize];
    m.bus.write32(base + 0x24, u32::MAX).unwrap();
    m.bus.write32(base + 0x18, 3 << 12).unwrap();
    for byte in [address << 1, register].into_iter().chain(data.iter().copied()) {
        m.bus.write32(base + 0x1c, u32::from(byte)).unwrap();
    }
    let mut commands = vec![6 << 11, 1 << 11 | 1 << 8 | (data.len() + 2) as u32];
    if read_len != 0 {
        m.bus.write32(base + 0x1c, u32::from(address << 1 | 1)).unwrap();
        commands.extend([6 << 11, 1 << 11 | 1 << 8 | 1, 3 << 11 | u32::from(read_len)]);
    }
    commands.push(2 << 11);
    for (i, command) in commands.into_iter().enumerate() {
        m.bus.write32(base + 0x58 + 4 * i as u32, command).unwrap();
    }
    m.bus.write32(base + 4, 1 << 5).unwrap(); m.bus.tick(100_000);
    if m.bus.read32(base + 0x20).unwrap() & (1 << 10) != 0 { return None; }
    assert_ne!(m.bus.read32(base + 0x20).unwrap() & (1 << 7), 0, "transfer must finish");
    let mut bytes = Vec::new();
    for _ in 0..read_len {
        // The same L32I.N encoding is exercised by the existing SRAM cost
        // model fixture: l32i.n a10, a2, 4. Read the actual MMIO FIFO in a CPU.
        m.cores[0].set_pc(PC); m.cores[0].set_ar(2, base + 0x18);
        assert!(matches!(m.run(1), Stop::MaxInsns));
        bytes.push(m.cores[0].get_ar(10) as u8);
    }
    Some(bytes)
}

fn values(m: &mut esp32s3::Machine, controller: u32) -> Vec<i16> {
    transaction(m, controller, 0x6b, 0x35, &[], 12).unwrap().as_chunks::<2>().0.iter()
        .map(|b| i16::from_le_bytes(*b)).collect()
}

fn host_sample(m: &mut esp32s3::Machine, a: [f64; 3], g: [f64; 3]) -> bool {
    // Same cold input boundary as the browser ABI: deliver work belonging to
    // the previous stimulus, then publish current time and replace the hold.
    let before = (m.bus.cycles, m.insns());
    m.bus.flush_ticks(); m.bus.board.advance_to(m.bus.cycles);
    let accepted = m.bus.board.set_imu_sample(a, g);
    assert_eq!((m.bus.cycles, m.insns()), before);
    accepted
}

fn probe(board: Box<dyn BoardModel>, pins: (u8, u8)) {
    let name = board.name(); let mut m = esp32s3::machine([0; 6]);
    m.bus.board = board; m.bus.attach_board_devices();
    m.bus.load_bytes(PC, &[0xa8, 0x12]).unwrap();
    m.bus.tick(16 * HZ / 1000); route(&mut m, 0, pins.0, pins.1);
    for (reg, value) in [(2, 0x40), (3, 0x13), (4, 0x43), (8, 3)] {
        assert!(transaction(&mut m, 0, 0x6b, reg, &[value], 0).is_some(), "{name}");
    }
    let at = m.bus.cycles;
    assert!(host_sample(&mut m, [0.5, -0.25, 1.0], [5.0, -10.0, 2.5]));
    assert_eq!(m.bus.cycles, at, "host input does not run or retime the guest");
    m.bus.tick(2 * HZ / 1000);
    let first = values(&mut m, 0); assert_eq!(first, [4096, -2048, 8192, 640, -1280, 320], "{name}");
    route(&mut m, 1, pins.0, pins.1);
    assert!(transaction(&mut m, 0, 0x6b, 0, &[], 1).is_none(), "controller 0 lost its physical pad route");
    assert_eq!(values(&mut m, 1), first, "controller 1 sees the same sensor state");
    let at = m.bus.cycles;
    assert!(host_sample(&mut m, [-1.0, 0.25, 0.5], [0.0, 1.0, 2.0]));
    assert_eq!(m.bus.cycles, at); m.bus.tick(2 * HZ / 1000);
    let second = values(&mut m, 1); assert_eq!(second, [-8192, 2048, 4096, 0, 128, 256], "{name}");
    assert!(!host_sample(&mut m, [f64::NAN, 0.0, 0.0], [0.0; 3]));
    assert_eq!(values(&mut m, 1), second, "invalid input must preserve the held sample");
    route(&mut m, 1, pins.0, pins.1 + 1);
    assert!(transaction(&mut m, 1, 0x6b, 0, &[], 1).is_none(), "wrong SCL must NACK");
    route(&mut m, 1, pins.0, pins.1);
    assert!(transaction(&mut m, 1, 0x6a, 0, &[], 1).is_none(), "grounded SA0 has no 6A alias");
    assert!(m.bus.board.set_imu_motion(0)); m.bus.tick(2 * HZ / 1000);
    assert_eq!(values(&mut m, 1), [0, 0, 8192, 0, 0, 0]);
    eprintln!("{name}: guest I2C0/I2C1 + Xtensa FIFO loads: {first:?} -> {second:?}; wrong pins/address NACK; invalid sample rejected; static mode restored");
}

#[test]
fn both_legacy_products_expose_host_samples_through_the_guest_i2c_fifo() {
    for variant in [LegacySpiVariant::Lcd128, LegacySpiVariant::TouchLcd128] {
        probe(Box::new(WaveshareLegacySpi::new(variant)), (6, 7));
    }
}

#[test]
fn all_six_rgb_products_expose_host_samples_through_the_guest_i2c_fifo() {
    for variant in [TcaRgbVariant::TouchLcd21, TcaRgbVariant::TouchLcd21B,
        TcaRgbVariant::Lcd28B, TcaRgbVariant::TouchLcd28B,
        TcaRgbVariant::Lcd28C, TcaRgbVariant::TouchLcd28C] {
        probe(Box::new(WaveshareTcaRgb::new(variant)), (15, 7));
    }
}

#[test]
fn exact_tail_spi_and_matrix_products_deliver_samples_over_actual_cpu_fifo() {
    for name in ["waveshare-s3-lcd28", "waveshare-s3-touch-lcd28-v1",
        "waveshare-s3-touch-lcd28-v2", "waveshare-s3-touch-lcd169", "waveshare-s3-matrix8"] {
        let pins = if name == "waveshare-s3-matrix8" { (11, 12) } else { (11, 10) };
        probe(esp32s3::board::make_board(name).unwrap(), pins);
    }
}
