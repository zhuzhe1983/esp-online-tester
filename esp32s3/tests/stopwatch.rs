//! GPIO38 interrupt register addresses and fields follow the ESP-IDF v6.1 headers
//! cited in `tests/co5300.rs`; the fields also match ESP-IDF v5.5.4.
use emu_core::Bus;
use esp_periph::i2c::I2cDevice;
use esp32s3::board::{make_board, BoardEdge, BoardModel, StopWatch};

type Devices = Vec<(u8, u8, Box<dyn I2cDevice>)>;

fn fill_window(board: &mut StopWatch, window: [u16; 4], color: u16) {
    let [x0, x1, y0, y1] = window;
    command(board, 0x2a, &[x0.to_be_bytes(), x1.to_be_bytes()].concat());
    command(board, 0x2b, &[y0.to_be_bytes(), y1.to_be_bytes()].concat());
    command(board, 0x2c, &color.to_be_bytes().repeat(usize::from(x1 - x0 + 1) * usize::from(y1 - y0 + 1)));
}

fn display_board() -> StopWatch {
    let mut board = StopWatch::new();
    power_up(&mut board.i2c_devices());
    fill_window(&mut board, [6, 473, 0, 465], 0x001f);
    board
}

#[test]
fn ui_commit_waits_for_contiguous_strips_while_gram_remains_live() {
    let mut board = display_board();
    let previous = board.display_for_ui().unwrap();
    for (index, y0) in (0..466u16).step_by(60).enumerate() {
        board.advance_to((index as u64 + 1) * 240_000);
        fill_window(&mut board, [6, 473, y0, (y0 + 59).min(465)], 0xf800);
        assert_eq!(board.display().unwrap().2[usize::from(y0) * 468], 0xf800, "guest GRAM changes immediately");
        if y0 < 420 { assert_eq!(board.display_for_ui().unwrap(), previous, "no partial strip sequence may become a UI frame"); }
    }
    let completed = board.display_for_ui().unwrap();
    assert_eq!(completed, board.display().unwrap());
    assert!(completed.2.iter().all(|&p| p == 0xf800));
    assert!(completed.3 > previous.3);
    fill_window(&mut board, [106, 107, 12, 13], 0x07e0);
    assert_eq!(board.display_for_ui().unwrap(), board.display().unwrap(), "small local update must not wait for a full screen");
}

#[test]
fn abandoned_top_strip_has_bounded_idle_fallback_including_sleep_commands() {
    for command_after in [None, Some(0x10), Some(0x28)] {
        let mut board = display_board();
        let previous = board.display_for_ui().unwrap();
        let start = 240_000;
        board.advance_to(start);
        fill_window(&mut board, [6, 473, 0, 59], 0xf800);
        if let Some(cmd) = command_after { command(&mut board, cmd, &[]); }
        // CO5300 sleep/display-off optical behavior is outside the existing model;
        // these commands must not leave its last real GRAM writes hidden forever.
        board.advance_to(start + esp32s3::periph::CPU_HZ / 50 - 1);
        assert_eq!(board.display_for_ui().unwrap(), previous);
        board.advance_to(start + esp32s3::periph::CPU_HZ / 50);
        assert_eq!(board.display_for_ui().unwrap(), board.display().unwrap());
        assert_eq!(board.display_for_ui().unwrap().2[60 * 468], 0x001f);
    }
}

#[test]
fn interrupted_reordered_and_restarted_strip_sequences_keep_latest_pixels() {
    for next in [[6, 473, 120, 179], [6, 473, 0, 59], [106, 107, 12, 13]] {
        let mut board = display_board();
        let previous = board.display_for_ui().unwrap();
        board.advance_to(240_000);
        fill_window(&mut board, [6, 473, 0, 59], 0xf800);
        board.advance_to(480_000);
        fill_window(&mut board, next, 0x07e0);
        if next[2] == 0 {
            assert_eq!(board.display_for_ui().unwrap(), previous, "new top strip replaces the abandoned pass without exposing it");
            for y0 in (60..466u16).step_by(60) { fill_window(&mut board, [6, 473, y0, (y0 + 59).min(465)], 0x07e0); }
            assert!(board.display_for_ui().unwrap().2.iter().all(|&p| p == 0x07e0));
        } else {
            assert_eq!(board.display_for_ui().unwrap(), board.display().unwrap(), "non-adjacent/local windows end strip grouping");
        }
    }
}

#[test]
fn reset_discards_a_retained_ui_frame_and_incomplete_strip_sequence() {
    let mut board = display_board();
    board.advance_to(240_000);
    fill_window(&mut board, [6, 473, 0, 59], 0xf800);
    command(&mut board, 0x01, &[]);
    assert_eq!(board.display_for_ui().unwrap(), board.display().unwrap());
    assert!(board.display_for_ui().unwrap().2.iter().all(|&p| p == 0));
    fill_window(&mut board, [6, 473, 0, 59], 0x07e0);
    let mut devices = board.i2c_devices();
    update(&mut devices, 0x4f, 0x05, 0, 0x08);
    assert_eq!(board.display_for_ui().unwrap(), board.display().unwrap());
    assert!(board.display_for_ui().unwrap().2.iter().all(|&p| p == 0));
}

#[test]
fn split_cs_headers_and_row_transfers_keep_the_ui_frozen_from_the_first_pixel() {
    let mut board = display_board();
    let previous = board.display_for_ui().unwrap();
    board.advance_to(20 * esp32s3::periph::CPU_HZ);
    let cs_command = |board: &mut StopWatch, cmd: u8, args: &[u8]| {
        board.gpio_changes(&[(39, true), (39, false)]);
        for byte in [0x02, 0, cmd, 0] { board.spi_transfer(2, &[byte], 0); }
        for byte in args { board.spi_transfer(2, &[*byte], 0); }
        board.gpio_changes(&[(39, true)]);
    };
    cs_command(&mut board, 0x2a, &[0, 6, 1, 0xd9]);
    cs_command(&mut board, 0x2b, &[0, 0, 0, 59]);
    board.gpio_changes(&[(39, false)]);
    for byte in [0x32, 0, 0x2c, 0] { board.spi_transfer(2, &[byte], 0); }
    assert_eq!(board.display_for_ui().unwrap(), previous, "a zero-payload header must not look like an old idle timeout");
    for row in 0..60 {
        board.spi_transfer(2, &[0xf8, 0].repeat(468), 0);
        assert_eq!(board.display().unwrap().2[row * 468], 0xf800);
        assert_eq!(board.display_for_ui().unwrap(), previous);
    }
    board.gpio_changes(&[(39, true)]);
    board.advance_to(20 * esp32s3::periph::CPU_HZ + esp32s3::periph::CPU_HZ / 50);
    assert_eq!(board.display_for_ui().unwrap(), board.display().unwrap());
}

#[test]
fn an_incomplete_or_overlapping_strip_cannot_be_mistaken_for_a_complete_pass() {
    for previous_complete in [false, true] {
        let mut board = display_board();
        command(&mut board, 0x2a, &[0, 6, 1, 0xd9]);
        command(&mut board, 0x2b, &[0, 0, 0, 59]);
        command(&mut board, 0x2c, &[0xf8, 0].repeat(if previous_complete { 468 * 60 } else { 468 }));
        fill_window(&mut board, [6, 473, if previous_complete { 30 } else { 60 }, 119], 0x07e0);
        assert_eq!(board.display_for_ui().unwrap(), board.display().unwrap());
    }
}

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
        let mut devices = board.i2c_devices();
        let internal:Vec<_>=devices.iter().filter(|(_,_,d)|d.pins()==Some((47,48))).map(|(bus,addr,_)|(*bus,*addr)).collect();
        assert_eq!(internal,[(0,0x6e),(0,0x4f),(0,0x15),(0,0x68),(0,0x32),(0,0x18),(1,0x6e),(1,0x4f),(1,0x15),(1,0x68),(1,0x32),(1,0x18)]);
        let external:Vec<_>=devices.iter().filter(|(_,_,d)|d.pins()==Some((10,11))).map(|(bus,addr,_)|(*bus,*addr)).collect();
        assert_eq!(external,[(0,0x44),(0,0x76),(0,0x23),(1,0x44),(1,0x76),(1,0x23)]);
        for (_,_,dev) in devices.iter_mut().filter(|(_,_,d)|d.pins()==Some((10,11))) {assert!(!dev.start(false));}
        assert_eq!(board.named_pin("a"), Some(2));
        assert_eq!(board.named_pin("btn_b"), Some(1));
        assert_eq!(board.named_pin("missing"), None);
        assert_eq!(board.input_levels(), [(38, false), (2, true), (1, true), (47,true), (48,true)]);
        assert_eq!(board.display().map(|(w, h, _, _)| (w, h)), Some((468, 466)));
    }
}

#[test]
fn es8311_probe_identity_and_configuration_survive_controller_handoff() {
    let mut board = StopWatch::new();
    let mut devices = board.i2c_devices();
    // ES8311 rev10.0 January 2021, p29: FD/FE/FF identify 83/11/00.
    // This exercises only the control-register subset, not PCM or microphone data.
    assert_eq!(read(&mut devices, 0x18, 0xfd, 3), [0x83, 0x11, 0]);
    write(&mut devices, 0x18, 0x32, 0xbf);
    let dev = devices.iter_mut().find(|(bus, addr, _)| *bus == 1 && *addr == 0x18).unwrap().2.as_mut();
    assert_eq!(dev.pins(), Some((47, 48)));
    assert!(!dev.start_address(0x18, 0x19, false));
    assert!(dev.start(false));
    assert!(dev.write(0x32));
    assert!(dev.start(true));
    assert_eq!(dev.read(), 0xbf);
    dev.stop();
    assert!(dev.start(false));
    assert!(dev.write(0x14));
    assert!(dev.write(0x1a));
    dev.stop();
    assert_eq!(read(&mut devices, 0x18, 0x14, 1), [0x1a]);
    let mut reset = board.i2c_devices();
    assert_eq!(read(&mut reset, 0x18, 0xfd, 3), [0x83, 0x11, 0]);
    assert_eq!(read(&mut reset, 0x18, 0x32, 1), [0]);
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
