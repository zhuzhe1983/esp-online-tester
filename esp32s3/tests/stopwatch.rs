//! GPIO38 interrupt register addresses and fields follow the ESP-IDF v6.1 headers
//! cited in `tests/co5300.rs`; the fields also match ESP-IDF v5.5.4.
use emu_core::Bus;
use esp_periph::i2c::I2cDevice;
use esp32s3::board::{make_board, BoardEdge, BoardModel, StopWatch};

type Devices = Vec<(u8, u8, Box<dyn I2cDevice>)>;

fn device(devices: &mut Devices, address: u8) -> &mut dyn I2cDevice {
    &mut *devices.iter_mut().find(|(bus, addr, _)| *bus == 0 && *addr == address).unwrap().2
}

fn read(devices: &mut Devices, address: u8, reg: u8, count: usize) -> Vec<u8> {
    let dev = device(devices, address);
    assert!(dev.start(false));
    assert!(dev.write(reg));
    assert!(dev.start(true));
    let bytes = (0..count).map(|_| dev.read()).collect();
    dev.stop();
    bytes
}

fn write(devices: &mut Devices, address: u8, reg: u8, value: u8) {
    let dev = device(devices, address);
    assert!(dev.start(false));
    assert!(dev.write(reg));
    assert!(dev.write(value));
    dev.stop();
}

fn update(devices: &mut Devices, address: u8, reg: u8, set: u8, clear: u8) {
    let value = read(devices, address, reg, 1)[0];
    write(devices, address, reg, (value & !clear) | set);
}

fn power_up(devices: &mut Devices) {
    assert_eq!(read(devices, 0x6e, 0, 2), [0x50, 0x20]);
    assert_eq!(read(devices, 0x4f, 0, 2), [0, 0]);
    update(devices, 0x6e, 0x06, 0, 0x10);
    write(devices, 0x6e, 0x09, 0);
    write(devices, 0x6e, 0x0a, 0);
    update(devices, 0x6e, 0x06, 7, 0x10);
    update(devices, 0x6e, 0x16, 0, 0x30);
    update(devices, 0x6e, 0x10, 0, 4);
    update(devices, 0x6e, 0x49, 0x80, 0);
    write(devices, 0x4f, 0x23, 0);
    update(devices, 0x4f, 0x13, 0, 0x9d);
    update(devices, 0x4f, 0x03, 0x9d, 0);
    update(devices, 0x4f, 0x05, 0x99, 0);
    update(devices, 0x4f, 0x05, 0, 0x18);
    update(devices, 0x4f, 0x05, 0x18, 0);
    assert_eq!(read(devices, 0x6e, 0x06, 1), [7]);
    assert_eq!(read(devices, 0x6e, 0x49, 1), [0xaa]);
}

fn command(board: &mut dyn BoardModel, command: u8, data: &[u8]) {
    let mut tx = vec![0x02, 0, command, 0];
    tx.extend_from_slice(data);
    assert_eq!(board.spi_transfer(2, &tx, 1), [0xff]);
}

fn pixel(board: &mut dyn BoardModel, value: u16) {
    command(board, 0x2a, &[0, 6, 0, 6]);
    command(board, 0x2b, &[0, 0, 0, 0]);
    command(board, 0x2c, &value.to_be_bytes());
}

#[test]
fn factory_devices_and_active_low_button_inputs() {
    for name in ["stopwatch", "m5stack-stopwatch"] {
        let mut board = make_board(name).unwrap();
        assert_eq!(board.name(), "stopwatch");
        let devices = board.i2c_devices();
        assert_eq!(devices.iter().map(|(bus, addr, _)| (*bus, *addr)).collect::<Vec<_>>(),
            [(0, 0x6e), (0, 0x4f), (0, 0x15), (0, 0x68), (0, 0x32),
             (1, 0x6e), (1, 0x4f), (1, 0x15), (1, 0x68), (1, 0x32)]);
        assert!(devices.iter().all(|(_,_,device)|device.pins()==Some((47,48))));
        assert_eq!(board.named_pin("a"), Some(2));
        assert_eq!(board.named_pin("btn_b"), Some(1));
        assert_eq!(board.named_pin("missing"), None);
        assert_eq!(board.input_levels(), [(38, false), (2, true), (1, true), (47,true), (48,true)]);
        assert_eq!(board.display().map(|(w, h, _, _)| (w, h)), Some((468, 466)));
    }
}

#[test]
fn firmware_power_sequence_gates_spi_and_preserves_full_window_stride() {
    let mut board = StopWatch::new();
    let mut devices = board.i2c_devices();
    pixel(&mut board, 0xffff);
    assert_eq!(board.display_version(), 0);
    assert!(board.display().unwrap().2.iter().all(|&p| p == 0));
    power_up(&mut devices);
    command(&mut board, 0x2a, &[0, 6, 1, 0xd9]);
    command(&mut board, 0x2b, &[0, 0, 1, 0xd1]);
    command(&mut board, 0x2c, &[0xf8]);
    board.spi_transfer(2, &[0], 0);
    let mut strip = [0x07, 0xe0].repeat(467);
    strip.extend_from_slice(&[0, 0x1f]);
    command(&mut board, 0x3c, &strip);
    let (_, _, frame, version) = board.display().unwrap();
    assert_eq!(frame[0], 0xf800);
    assert_eq!(frame[467], 0x07e0);
    assert_eq!(frame[468], 0x001f);
    assert_eq!(frame[469], 0);
    assert!(version > 0);
    assert_eq!(board.display_frames(), 1);
    board.spi_transfer(3, &[0x02, 0, 0x2c, 0, 0xff, 0xff], 0);
    assert_eq!(board.display_version(), version);
    command(&mut board, 0x2a, &[1, 0xd9, 1, 0xd9]);
    command(&mut board, 0x2b, &[1, 0xd1, 1, 0xd1]);
    command(&mut board, 0x2c, &[0xff, 0xff]);
    assert_eq!(board.display().unwrap().2[468 * 466 - 1], 0xffff);
}

#[test]
fn reset_pulse_between_board_callbacks_clears_pixels_and_tear_state() {
    let mut board = StopWatch::new();
    let mut devices = board.i2c_devices();
    power_up(&mut devices);
    pixel(&mut board, 0xffff);
    command(&mut board, 0x35, &[0]);
    let us = esp32s3::periph::CPU_HZ / 1_000_000;
    board.advance_to(15_000 * us);
    assert_eq!(board.take_edges(), [BoardEdge { cycle: 15_000 * us, pin: 38, level: true }]);
    let before = board.display_version();
    update(&mut devices, 0x4f, 0x05, 0, 0x18);
    update(&mut devices, 0x4f, 0x05, 0x18, 0);
    assert_eq!(board.take_edges(), [BoardEdge { cycle: 15_000 * us, pin: 38, level: false }]);
    assert_eq!(board.next_deadline(), None);
    assert!(board.display().unwrap().2.iter().all(|&p| p == 0));
    assert!(board.display_version() > before);
    board.spi_transfer(2, &[0xff, 0xff], 0);
    assert!(board.display().unwrap().2.iter().all(|&p| p == 0));
    pixel(&mut board, 0x1234);
    assert_eq!(board.display().unwrap().2[0], 0x1234);
}

#[test]
fn rail_off_and_latched_pmic_shutdown_disable_panel_without_halting_cpu() {
    let mut board = StopWatch::new();
    let mut devices = board.i2c_devices();
    power_up(&mut devices);
    pixel(&mut board, 0xffff);
    update(&mut devices, 0x4f, 0x05, 0, 0x80);
    pixel(&mut board, 0xffff);
    assert_eq!(board.display().unwrap().2[0], 0);
    update(&mut devices, 0x4f, 0x05, 0x80, 0);
    pixel(&mut board, 0x1234);
    update(&mut devices, 0x6e, 0x06, 0, 0x10);
    write(&mut devices, 0x6e, 0x07, 0);
    assert_eq!(board.display().unwrap().2[0], 0x1234);
    write(&mut devices, 0x6e, 0x0c, 0xa1);
    pixel(&mut board, 0xffff);
    command(&mut board, 0x35, &[0]);
    board.advance_to(10_000_000);
    assert_eq!(board.next_deadline(), None);
    assert!(board.display().unwrap().2.iter().all(|&p| p == 0));
    assert_eq!(board.report(), "[stopwatch] power_off_requested=true");
    assert_eq!(read(&mut devices, 0x6e, 0, 2), [0x50, 0x20]);
}

#[test]
fn cst820_reports_clamped_touch_and_clears_it_on_expander_reset() {
    let mut board = StopWatch::new();
    let mut devices = board.i2c_devices();
    board.touch(100, 200, true);
    assert_eq!(read(&mut devices, 0x15, 0x02, 1), [0]);
    power_up(&mut devices);
    assert_eq!(read(&mut devices, 0x15, 0xa7, 3), [0xb7, 0x41, 2]);
    board.touch(999, 999, true);
    assert_eq!(read(&mut devices, 0x15, 0x02, 5), [1, 1, 0xd3, 1, 0xd1]);
    board.touch(467, 465, false);
    assert_eq!(read(&mut devices, 0x15, 0x02, 1), [0]);
    board.touch(100, 200, true);
    update(&mut devices, 0x4f, 0x05, 0, 8);
    update(&mut devices, 0x4f, 0x05, 8, 0);
    assert_eq!(read(&mut devices, 0x15, 0x02, 1), [0]);
}

#[test]
fn synthetic_scan_uses_cpu_cycles_and_drives_gpio38_interrupts() {
    let mut board = StopWatch::new();
    let mut devices = board.i2c_devices();
    power_up(&mut devices);
    let mut bus = esp32s3::bus::SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(board);
    bus.attach_board_devices();
    power_up(&mut devices);
    bus.gpio_events = Some(Vec::new());
    bus.write32(0x6000_410c, (3 << 7) | (1 << 13)).unwrap();
    bus.write32(0x600c_2040, 9).unwrap();
    command(&mut *bus.board, 0x44, &320u16.to_be_bytes());
    command(&mut *bus.board, 0x35, &[0]);
    let us = esp32s3::periph::CPU_HZ / 1_000_000;
    bus.tick((9_600 * us - 1) as u32);
    assert_eq!(bus.read32(0x6000_4040).unwrap() & (1 << 6), 0);
    bus.tick(1);
    assert_eq!(bus.read32(0x6000_4068).unwrap(), 1 << 6);
    assert_eq!(bus.periph.cpu_lines(0), 1 << 9);
    bus.write32(0x6000_4058, 1 << 6).unwrap();
    bus.tick((120 * us) as u32);
    assert_eq!(bus.read32(0x6000_4040).unwrap() & (1 << 6), 0);
    assert_eq!(bus.read32(0x6000_4068).unwrap(), 1 << 6);
    assert_eq!(bus.gpio_events.as_deref(), Some(&[(9_600 * us, 38, true), (9_720 * us, 38, false)][..]));
}

#[test]
fn software_cs_delimits_split_m5gfx_headers_and_command_like_pixel_data() {
    // M5GFX 0.2.19 Panel_AMOLED::write_cmd sends four individual bytes under
    // one software CS assertion; framebuffer DMA then sends one row per transfer.
    let mut board = StopWatch::new();
    let mut devices = board.i2c_devices();
    power_up(&mut devices);
    let cs_command = |board: &mut StopWatch, cmd: u8, args: &[u8]| {
        board.gpio_changes(&[(39, true), (39, false)]);
        for byte in [0x02, 0, cmd, 0] { board.spi_transfer(2, &[byte], 0); }
        for byte in args { board.spi_transfer(2, &[*byte], 0); }
        board.gpio_changes(&[(39, true)]);
    };
    cs_command(&mut board, 0x2a, &[0, 6, 0, 9]);
    cs_command(&mut board, 0x2b, &[0, 0, 0, 1]);
    cs_command(&mut board, 0x44, &[1, 0xd2]);
    cs_command(&mut board, 0x35, &[0x80]);
    assert!(board.next_deadline().is_some(), "split TE enable was not decoded");
    board.gpio_changes(&[(39, false)]);
    board.spi_transfer(2, &[0x32, 0, 0x2c, 0], 0);
    board.spi_transfer(2, &[0x02, 0, 0x01, 0, 0x32, 0, 0x2c, 0], 0);
    board.spi_transfer(2, &[0xf8, 0, 0x07, 0xe0, 0, 0x1f, 0xff, 0xff], 0);
    board.gpio_changes(&[(39, true)]);
    let (_, _, frame, version) = board.display().unwrap();
    assert_eq!(&frame[..4], &[0x0200, 0x0100, 0x3200, 0x2c00]);
    assert_eq!(&frame[468..472], &[0xf800, 0x07e0, 0x001f, 0xffff]);
    board.spi_transfer(2, &[0, 0, 0, 0], 0);
    assert_eq!(board.display_version(), version, "deasserted CS must ignore data");
    board.gpio_changes(&[(39, false)]);
    board.spi_transfer(2, &[0x02, 0], 0);
    board.gpio_changes(&[(39, true)]);
    cs_command(&mut board, 0x34, &[]);
    assert!(board.next_deadline().is_none(), "an abandoned header must not corrupt the next command");
}
