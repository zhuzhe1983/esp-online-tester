//! Board-only qualification. No I2S, DMA or guest capture claim.
use emu_core::Bus;
use esp32::bus::SocBus;
use esp_periph::i2c::{INT_NACK, INT_TIMEOUT};
use esp_soc::picture::{Jpeg, JpegError, Picture};
use std::sync::Arc;

fn put(bus: &mut SocBus, addr: u32, value: u32) { bus.write32(addr, value).unwrap(); }
fn board() -> SocBus {
    let mut bus = SocBus::new(4 << 20, [0; 6]);
    bus.board = esp32::board::make_board("ai-thinker-cam").unwrap();
    bus.attach_board_devices();
    // IDF v5.5.4 io_mux_reg.h: GPIO32 offset 0x1c; gpio_reg.h:
    // ENABLE1_W1TS at 0x30 and OUT1 at 0x10. PWDN is active high.
    put(&mut bus, 0x3ff4_901c, 2 << 12);
    put(&mut bus, 0x3ff4_4030, 1);
    route(&mut bus, 26, 27);
    bus
}
fn route(bus: &mut SocBus, sda: u8, scl: u8) { route_on(bus, 0, sda, scl) }
fn route_on(bus: &mut SocBus, port: usize, sda: u8, scl: u8) {
    // IDF v5.5.4 gpio_sig_map.h I2CEXT0 SCL/SDA 29/30, I2CEXT1 95/96;
    // io_mux_reg.h pad offsets.
    let (scl_signal, sda_signal) = [(29, 30), (95, 96)][port];
    for (pin, signal) in [(sda, sda_signal), (scl, scl_signal)] {
        let mux = match pin { 26 => 0x28, 27 => 0x2c, _ => unreachable!() };
        put(bus, 0x3ff4_9000 + mux, (2 << 12) | (1 << 9) | (1 << 8));
        put(bus, 0x3ff4_4130 + 4 * signal, (1 << 7) | u32::from(pin));
        put(bus, 0x3ff4_4530 + 4 * u32::from(pin), signal);
    }
}
fn transfer(bus: &mut SocBus, bytes: &[u8], read: bool) -> (u32, u8) { transfer_on(bus, 0, bytes, read) }
fn transfer_on(bus: &mut SocBus, port: usize, bytes: &[u8], read: bool) -> (u32, u8) {
    // IDF v5.5.4 reg_base.h: DR_REG_I2C_EXT_BASE 0x3ff53000, DR_REG_I2C1_EXT_BASE 0x3ff67000.
    let base = [0x3ff5_3000, 0x3ff6_7000][port];
    put(bus, base + 0x24, u32::MAX);
    put(bus, base + 0x18, 3 << 12);
    put(bus, base + 0x1c, if read { 0x61 } else { 0x60 });
    for &b in bytes { put(bus, base + 0x1c, u32::from(b)); }
    let mut commands = vec![0, (1 << 11) | (1 << 8) | (1 + bytes.len() as u32)];
    if read { commands.push((2 << 11) | (1 << 10) | 1); }
    commands.push(3 << 11);
    for (i, cmd) in commands.into_iter().enumerate() { put(bus, base + 0x58 + 4 * i as u32, cmd); }
    put(bus, base + 4, 1 << 5);
    (bus.read32(base + 0x20).unwrap(), bus.read32(base + 0x1c).unwrap() as u8)
}
fn write(bus: &mut SocBus, reg: u8, value: u8) { assert_eq!(transfer(bus, &[reg, value], false).0 & INT_NACK, 0); }
fn read(bus: &mut SocBus, reg: u8) -> u8 {
    assert_eq!(transfer(bus, &[reg], false).0 & INT_NACK, 0);
    let (status, byte) = transfer(bus, &[], true);
    assert_eq!(status & INT_NACK, 0); byte
}
fn configure(bus: &mut SocBus) {
    // ov2640.c:set_window; geometry is in units of four pixels.
    write(bus, 0xff, 0);
    for (reg, value) in [(0x5a, 1), (0x5b, 1), (0x5c, 0), (0xda, 8), (0xe0, 0)] { write(bus, reg, value); }
}
fn clock(bus: &mut SocBus) {
    // IDF v5.5.4 ledc_reg.h / gpio_sig_map.h: HS channel 0 signal 71.
    // 80MHz / (256/256) / 2^3 = 10MHz; duty 4/8.
    put(bus, 0x3ff0_00c0, 1 << 11);
    put(bus, 0x3ff5_9140, 3 | (256 << 5) | (1 << 25));
    put(bus, 0x3ff5_9008, 4 << 4);
    put(bus, 0x3ff5_900c, (1 << 31) | (1 << 30) | (1 << 20) | (1 << 10));
    put(bus, 0x3ff5_9000, 4);
    put(bus, 0x3ff4_9044, (2 << 12) | (1 << 9));
    put(bus, 0x3ff4_4530, 71);
    // Duty becomes active at a timer boundary; no per-PCLK board stepping.
    bus.periph.tick(64);
}
fn picture(rgb: [u8; 3]) -> Picture { Picture { w: 1, h: 1, rgb: rgb.to_vec() } }
/// Structurally valid baseline JPEG (SOI, SOF0, SOS, entropy byte, EOI); never decoded.
fn jpeg(w: u16, h: u16, tag: u8) -> Vec<u8> {
    let mut d = vec![0xff, 0xd8, 0xff, 0xc0, 0, 11, 8];
    d.extend(h.to_be_bytes()); d.extend(w.to_be_bytes());
    d.extend([1, 1, 0x11, 0, 0xff, 0xda, 0, 8, 1, 1, 0, 0, 63, 0, tag, 0xff, 0xd9]);
    d
}
fn set_jpeg(bus: &mut SocBus, bytes: &[u8]) -> Result<(), JpegError> { bus.board.set_camera_jpeg(Jpeg::parse(bytes.to_vec()).unwrap()) }

#[test]
fn unsupported_sensor_output_warns_once() {
    const CHILD: &str = "ESP32SIM_CAMERA_WARNING_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let mut bus = board(); configure(&mut bus); clock(&mut bus);
        // RAW10 and JPEG+RGB565 are unsupported; JPEG without host input is no frame.
        for mode in [0x18, 0x04, 8, 0x18, 0x10, 0x12, 0x10, 0x05] {
            write(&mut bus, 0xda, mode);
            assert_eq!(bus.camera_frame().is_some(), mode == 8);
        }
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "unsupported_sensor_output_warns_once", "--nocapture"])
        .env(CHILD, "1").output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.matches("[camera] unsupported sensor output").count(), 1, "{stderr}");
    assert!(stderr.contains("IMAGE_MODE 0x18; classic ESP32-CAM supports raw RGB565/YUV422 and host-encoded JPEG"), "{stderr}");
    assert_eq!(stderr.matches("[camera] JPEG output 4x4 has no host JPEG of that size").count(), 1, "{stderr}");
}

#[test]
fn aithinker_selection_and_only_sensor_attachment() {
    for alias in ["ai-thinker-cam", "esp32cam"] {
        let mut board = esp32::board::make_board(alias).unwrap();
        assert_eq!(board.name(), "ai-thinker-cam");
        let devices = board.i2c_devices();
        let attached: Vec<_> = devices.iter().map(|(port, address, device)| (*port, *address, device.pins())).collect();
        assert_eq!(attached, [(0, 0x30, Some((26, 27))), (1, 0x30, Some((26, 27)))]);
    }
    assert_eq!(esp32::board::aithinker::PIN_RESET, None);
    assert!(esp32::board::make_board("waveshare-cam").is_none());
}
#[test]
fn aithinker_sccb_requires_both_physical_wires() {
    let mut bus = board();
    write(&mut bus, 0xff, 1);
    assert_eq!(read(&mut bus, 0x0a), 0x26);
    route(&mut bus, 27, 26);
    assert_ne!(transfer(&mut bus, &[0x0a], false).0 & INT_NACK, 0);
    route(&mut bus, 26, 27);
    put(&mut bus, 0x3ff4_4598, 256); // disconnect SDA output only
    assert_ne!(transfer(&mut bus, &[0x0a], false).0 & INT_NACK, 0);
    route(&mut bus, 26, 27);
    put(&mut bus, 0x3ff4_459c, 256); // disconnect SCL output only
    assert_ne!(transfer(&mut bus, &[0x0a], false).0 & INT_NACK, 0);
    route(&mut bus, 26, 27);
    assert_eq!(read(&mut bus, 0x0a), 0x26);
}
#[test]
fn aithinker_sccb_is_one_sensor_on_either_controller() {
    // SCCB_Init uses CONFIG_SCCB_HARDWARE_I2C_PORT (default I2C1); C8
    // sets PORT0. Either controller routed to GPIO26/27 reaches the sensor.
    let read_on = |bus: &mut SocBus, port, reg| {
        assert_eq!(transfer_on(bus, port, &[reg], false).0 & INT_NACK, 0);
        let (status, byte) = transfer_on(bus, port, &[], true);
        assert_eq!(status & INT_NACK, 0);
        byte
    };
    let mut bus = board();
    // I2C1 not routed (its SDA/SCL inputs are not high): it times out.
    assert_ne!(transfer_on(&mut bus, 1, &[0x0a], false).0 & INT_TIMEOUT, 0);
    route_on(&mut bus, 1, 26, 27);
    assert_eq!(transfer_on(&mut bus, 1, &[0xff, 1], false).0 & INT_NACK, 0);
    assert_eq!(read_on(&mut bus, 1, 0x0a), 0x26);
    // The bank selected and a register written through I2C1 are what I2C0 sees.
    assert_eq!(transfer_on(&mut bus, 1, &[0xff, 0], false).0 & INT_NACK, 0);
    assert_eq!(transfer_on(&mut bus, 1, &[0xda, 0x10], false).0 & INT_NACK, 0);
    route(&mut bus, 26, 27);
    assert_eq!(read_on(&mut bus, 0, 0xda), 0x10);
    // A powered-down sensor answers neither controller.
    put(&mut bus, 0x3ff4_4014, 1); // OUT1_W1TS: PWDN high
    route_on(&mut bus, 1, 26, 27);
    assert_ne!(transfer_on(&mut bus, 1, &[0x0a], false).0 & INT_NACK, 0);
}
#[test]
fn aithinker_host_and_register_updates_invalidate_shared_cache() {
    let mut bus = board(); configure(&mut bus); clock(&mut bus);
    assert_eq!(*bus.camera_frame().unwrap().2, [0; 32]);
    bus.board.set_camera_picture(picture([255, 0, 0])).unwrap();
    assert_eq!(bus.board.camera_preview(2, 1).unwrap(), [255, 0, 0, 255, 0, 0]);
    let first = bus.camera_frame().unwrap().2;
    assert_eq!(*first, [248, 0].repeat(16));
    assert!(Arc::ptr_eq(&first, &bus.camera_frame().unwrap().2));
    assert!(bus.board.set_camera_picture(Picture { w: 1, h: 1, rgb: vec![] }).is_err());
    assert!(Arc::ptr_eq(&first, &bus.camera_frame().unwrap().2));
    bus.board.set_camera_picture(picture([0, 255, 0])).unwrap();
    assert_eq!(*bus.camera_frame().unwrap().2, [7, 224].repeat(16));
    write(&mut bus, 0x5a, 2);
    let (w, h, frame) = bus.camera_frame().unwrap();
    assert_eq!((w, h, frame.len()), (8, 4, 64));
    write(&mut bus, 0xda, 0x10);
    assert!(bus.camera_frame().is_none(), "JPEG is not fabricated as raw bytes");
    write(&mut bus, 0xda, 8);
    assert_eq!(*bus.camera_frame().unwrap().2, [7, 224].repeat(32));
}
#[test]
fn aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size() {
    let mut bus = board(); configure(&mut bus); clock(&mut bus);
    bus.board.set_camera_picture(picture([255, 0, 0])).unwrap();
    let a = jpeg(4, 4, 1);
    assert_eq!(set_jpeg(&mut bus, &a), Ok(()));
    assert!(!bus.board.camera_frame_is_jpeg());
    assert_eq!(*bus.camera_frame().unwrap().2, [248, 0].repeat(16), "raw output ignores host JPEG");
    // ov2640_settings_jpeg3: IMAGE_MODE_JPEG_EN | IMAGE_MODE_HREF_VSYNC.
    for mode in [0x12, 0x10] {
        write(&mut bus, 0xda, mode);
        assert!(bus.board.camera_frame_is_jpeg());
        let (w, h, frame) = bus.camera_frame().unwrap();
        assert_eq!((w, h, &*frame), (4, 4, &a));
    }
    let first = bus.camera_frame().unwrap().2;
    assert_eq!(set_jpeg(&mut bus, &jpeg(8, 4, 2)), Err(JpegError::SizeMismatch { jpeg: (8, 4), sensor: (4, 4) }));
    assert!(Arc::ptr_eq(&first, &bus.camera_frame().unwrap().2), "refusal keeps the previous JPEG");
    let b = jpeg(4, 4, 3);
    assert_eq!(set_jpeg(&mut bus, &b), Ok(()));
    assert_eq!(*bus.camera_frame().unwrap().2, b);
    // A new geometry needs a matching JPEG; nothing is scaled or fabricated.
    write(&mut bus, 0x5a, 2);
    assert!(bus.camera_frame().is_none());
    assert_eq!(set_jpeg(&mut bus, &b), Err(JpegError::SizeMismatch { jpeg: (4, 4), sensor: (8, 4) }));
    let c = jpeg(8, 4, 4);
    assert_eq!(set_jpeg(&mut bus, &c), Ok(()));
    assert_eq!(*bus.camera_frame().unwrap().2, c);
    write(&mut bus, 0xda, 8);
    assert!(!bus.board.camera_frame_is_jpeg());
    assert_eq!(*bus.camera_frame().unwrap().2, [248, 0].repeat(32));
}
#[test]
fn aithinker_unconfigured_sensor_accepts_jpeg_checked_at_delivery() {
    for (size, delivered) in [((4, 4), true), ((8, 4), false), ((4, 8), false)] {
        let mut bus = board(); clock(&mut bus);
        let bytes = jpeg(size.0, size.1, 5);
        assert_eq!(set_jpeg(&mut bus, &bytes), Ok(()), "no configured size yet");
        configure(&mut bus);
        write(&mut bus, 0xda, 0x12);
        assert_eq!(bus.camera_frame().map(|f| (f.0, f.1, f.2.to_vec())), delivered.then_some((4, 4, bytes)));
    }
}
#[test]
fn aithinker_grayscale_requests_keep_yuv_wire_bytes() {
    let mut bus = board(); configure(&mut bus); clock(&mut bus);
    bus.board.set_camera_picture(Picture { w: 2, h: 1, rgb: vec![255, 0, 0, 0, 255, 0] }).unwrap();
    // Public grayscale and YUV422 register sequences both select IMAGE_MODE=0.
    write(&mut bus, 0xda, 0);
    assert_eq!(*bus.camera_frame().unwrap().2, [76, 84, 76, 255, 149, 43, 149, 21].repeat(4));
}
#[test]
fn aithinker_pwdn_stop_and_reset_withhold_frames() {
    let mut bus = board(); configure(&mut bus); clock(&mut bus);
    assert!(bus.camera_frame().is_some());
    put(&mut bus, 0x3ff4_4010, 1); // PWDN high
    assert!(bus.camera_frame().is_none());
    assert_ne!(transfer(&mut bus, &[0xff, 0], false).0 & INT_NACK, 0);
    put(&mut bus, 0x3ff4_4010, 0);
    assert!(bus.camera_frame().is_some());
    put(&mut bus, 0x3ff4_4034, 1); // release low PWDN
    assert!(bus.camera_frame().is_none());
    put(&mut bus, 0x3ff4_4030, 1);
    assert!(bus.camera_frame().is_some());
    for (bank, reg, value) in [(1, 9, 0x10), (0, 0xe0, 4), (0, 5, 1)] {
        write(&mut bus, 0xff, bank); write(&mut bus, reg, value);
        assert!(bus.camera_frame().is_none());
        write(&mut bus, reg, 0); assert!(bus.camera_frame().is_some());
    }
    write(&mut bus, 0xff, 1); write(&mut bus, 0x12, 0x80);
    assert!(bus.camera_frame().is_none());
    assert_eq!(read(&mut bus, 0x0a), 0x26);
}
#[test]
fn aithinker_xclk_rejects_constant_or_invalid_clock() {
    let mut bus = board(); configure(&mut bus);
    assert!(bus.board.camera_frame().is_none(), "clockless board hook cannot bypass routing");
    for hz in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(bus.board.camera_frame_with_clock(&|_| Some((hz, 32768))).is_none());
    }
    for duty in [0, 65535, 65536] {
        assert!(bus.board.camera_frame_with_clock(&|_| Some((10_000_000.0, duty))).is_none());
    }
    assert!(bus.board.camera_frame_with_clock(&|pin| {
        assert_eq!(pin, 0); Some((10_000_000.0, 32768))
    }).is_some());
}
#[test]
fn aithinker_xclk_requires_live_routed_pwm_not_gpio_level() {
    let mut bus = board(); configure(&mut bus);
    assert!(bus.camera_frame().is_none());
    clock(&mut bus);
    let (hz, duty) = bus.periph.pwm_output(0).unwrap();
    assert_eq!((hz, duty), (10_000_000.0, 32768));
    assert!(bus.camera_frame().is_some());
    put(&mut bus, 0x3ff4_4530, 256); // XCLK disconnected, GPIO high is not a clock
    put(&mut bus, 0x3ff4_4024, 1); put(&mut bus, 0x3ff4_4008, 1);
    assert!(bus.camera_frame().is_none());
    put(&mut bus, 0x3ff4_4530, 71 | (1 << 9));
    assert!(bus.camera_frame().is_some(), "inverted PWM still clocks; GPIO0 is not a reset wire");
    put(&mut bus, 0x3ff5_9140, 3 | (256 << 5) | (1 << 25) | (1 << 23)); // timer pause
    assert!(bus.camera_frame().is_none());
    clock(&mut bus);
    put(&mut bus, 0x3ff0_00c0, 0);
    assert!(bus.camera_frame().is_none());
}

#[cfg(test)]
#[path = "camera_dma/cases.rs"]
mod c7;

fn dvp(bus: &mut SocBus) {
    // IDF v4.4.7 gpio_sig_map.h, io_mux_reg.h; AI-Thinker fixed wiring.
    for (signal, pin, mux) in [(140,5,0x6c),(141,18,0x70),(142,19,0x74),(143,21,0x7c),
        (144,36,4),(145,39,0x10),(146,34,0x14),(147,35,0x18),
        (28,22,0x80),(190,23,0x8c),(191,25,0x24)] {
        put(bus, 0x3ff4_9000 + mux, (2 << 12) | (1 << 9));
        put(bus, 0x3ff4_4130 + 4 * signal, 0x80 | pin);
    }
    put(bus, 0x3ff4_4130 + 4 * 192, 0xb8);
}
fn receiver(bus: &mut SocBus) {
    // Preserve LEDC's gate while enabling I2S0 (IDF v4.4.7 dport_reg.h).
    put(bus, 0x3ff0_00c0, (1 << 11) | (1 << 4));
    put(bus, 0x3ff4_f0a8, 0x21);
    put(bus, 0x3ff4_f020, (1 << 12) | (1 << 16) | (1 << 20));
    put(bus, 0x3ff4_f008, (1 << 7) | (1 << 5));
}
fn tick_bus(bus: &mut SocBus, cycles: u64) {
    let cpu = esp_soc::SocBus::cpu_cycles_for_bus(bus, cycles);
    bus.tick(cpu.try_into().unwrap());
}
fn running() -> SocBus {
    let mut bus = board();
    put(&mut bus, 0x3ff4_8070, 1 << 27);
    put(&mut bus, 0x3ff0_003c, 2); // 240MHz for exact deadline boundaries
    configure(&mut bus); clock(&mut bus); dvp(&mut bus);
    // Reconcile the first latched PWM duty without requiring an I2S start.
    bus.read32(0x3ff4_403c).unwrap();
    bus
}
#[test]
fn c6_vsync_unarmed_stopped_irq_and_one_us_level_without_dma() {
    use esp32::board::aithinker::{FRAME_CYCLES, VSYNC_CYCLES};
    let mut bus = running();
    let original = bus.dram.clone();
    // GPIO25 rising edge, core0 enable: IDF v4.4.7 gpio_reg.h.
    put(&mut bus, 0x3ff4_4088 + 4 * 25, (1 << 7) | (1 << 15));
    let first = bus.board.next_deadline().unwrap();
    assert_eq!(first, FRAME_CYCLES);
    assert_eq!(esp_soc::SocBus::next_deadline(&bus), Some(esp_soc::SocBus::cpu_cycles_for_bus(&bus, first)));
    tick_bus(&mut bus, first - 1);
    assert_eq!(bus.read32(0x3ff4_403c).unwrap() & (1 << 25), 0);
    tick_bus(&mut bus, 1);
    assert_ne!(bus.read32(0x3ff4_403c).unwrap() & (1 << 25), 0);
    assert_ne!(bus.read32(0x3ff4_4044).unwrap() & (1 << 25), 0, "GPIO edge IRQ latched");
    assert!(bus.periph.gpio.irq(0, false));
    tick_bus(&mut bus, 240); // one microsecond in the stable bus domain
    assert_ne!(bus.read32(0x3ff4_403c).unwrap() & (1 << 25), 0, "delayed ISR check");
    tick_bus(&mut bus, VSYNC_CYCLES - 240);
    assert_eq!(bus.read32(0x3ff4_403c).unwrap() & (1 << 25), 0);
    receiver(&mut bus);
    put(&mut bus, 0x3ff4_f008, 1 << 7); // explicitly stopped
    let next = bus.board.next_deadline().unwrap();
    let delta = next - esp_soc::SocBus::cycles(&bus);
    tick_bus(&mut bus, delta);
    assert_ne!(bus.read32(0x3ff4_403c).unwrap() & (1 << 25), 0, "stopped capture still gets VSYNC");
    assert_eq!(bus.dram, original, "C6 never writes DMA memory");
    assert_eq!(bus.read32(0x3ff4_f00c).unwrap(), 0, "not an I2S/LCD_CAM interrupt");
}
#[test]
fn c6_power_clock_and_sensor_stop_cancel_and_restart_vsync() {
    let mut bus = running();
    for (addr, off, on) in [(0x3ff4_4010,1,0),(0x3ff4_4034,1,0)] {
        let next = bus.board.next_deadline().unwrap();
        let delta = next - esp_soc::SocBus::cycles(&bus); tick_bus(&mut bus, delta);
        assert!(bus.periph.gpio.gpio.level(25));
        put(&mut bus, addr, off);
        assert_eq!(bus.board.next_deadline(), None);
        assert!(!bus.periph.gpio.gpio.level(25));
        if addr == 0x3ff4_4034 { put(&mut bus, 0x3ff4_4030, 1); } else { put(&mut bus, addr, on); }
        assert!(bus.board.next_deadline().is_some());
    }
    put(&mut bus, 0x3ff4_4530, 256);
    assert_eq!(bus.board.next_deadline(), None);
    put(&mut bus, 0x3ff4_4530, 71 | (1 << 9));
    assert!(bus.board.next_deadline().is_some(), "inverted XCLK is still a clock");
    write(&mut bus, 0xe0, 4);
    assert_eq!(bus.board.next_deadline(), None);
    write(&mut bus, 0xe0, 0);
    assert!(bus.board.next_deadline().is_some());
}
#[test]
fn c6_physical_routes_inversion_and_reconfiguration() {
    let mut bus = running(); receiver(&mut bus);
    bus.board.set_camera_picture(picture([255,0,0])).unwrap();
    assert_eq!(*bus.camera_routed_frame().unwrap().2, [248,0].repeat(16));
    // Every data/clock/sync wire is required; restoring it recovers immediately.
    for (signal,pin) in [(140,5),(141,18),(142,19),(143,21),(144,36),(145,39),(146,34),(147,35),(28,22),(190,23),(191,25)] {
        let wrong = if pin == 5 { 18 } else { 5 }; // another enabled physical input, not a floating pad
        put(&mut bus, 0x3ff4_4130 + 4*signal, 0x80 | wrong);
        assert!(bus.camera_routed_frame().is_none(), "wrong signal {signal}");
        put(&mut bus, 0x3ff4_4130 + 4*signal, 0x80 | pin);
        assert!(bus.camera_routed_frame().is_some());
    }
    for (signal,pin) in [(28,22),(190,23),(191,25)] {
        put(&mut bus, 0x3ff4_4130 + 4*signal, 0xc0 | pin);
        assert!(bus.camera_routed_frame().is_none(), "unsupported control polarity");
        put(&mut bus, 0x3ff4_4130 + 4*signal, 0x80 | pin);
    }
    put(&mut bus, 0x3ff4_4130 + 4*140, 0xc0 | 5);
    put(&mut bus, 0x3ff4_4130 + 4*147, 0xc0 | 35);
    assert_eq!(*bus.camera_routed_frame().unwrap().2, [121,129].repeat(16));
    dvp(&mut bus);
    assert_eq!(*bus.camera_routed_frame().unwrap().2, [248,0].repeat(16), "inversion cannot corrupt cached source");
    put(&mut bus, 0x3ff4_4130 + 4*192, 0xb0);
    assert!(bus.camera_routed_frame().is_none());
    dvp(&mut bus);
    put(&mut bus, 0x3ff4_9004, 2 << 12); // disable D4 pad input
    assert!(bus.camera_routed_frame().is_none());
    dvp(&mut bus);
    put(&mut bus, 0x3ff4_9070, 1 << 9); // wrong D1 pad function
    assert!(bus.camera_routed_frame().is_none());
    dvp(&mut bus);
    put(&mut bus, 0x3ff4_4530, 256);
    assert!(bus.camera_routed_frame().is_none());
    clock(&mut bus); receiver(&mut bus);
    assert!(bus.camera_routed_frame().is_some());
    put(&mut bus, 0x3ff4_4010, 1);
    assert!(bus.camera_routed_frame().is_none());
}
#[test]
fn c6_receiver_unarmed_reset_and_modes_withhold_routed_frames() {
    let mut bus = running();
    assert!(bus.camera_routed_frame().is_none());
    receiver(&mut bus); assert!(bus.camera_routed_frame().is_some());
    for conf in [1 << 7, 1 << 5, (1<<7)|(1<<5)|2, (1<<7)|(1<<5)|8] {
        put(&mut bus, 0x3ff4_f008, conf);
        assert!(bus.camera_routed_frame().is_none());
    }
    receiver(&mut bus);
    for fifo in [(1<<16)|(1<<20), (1<<16)|(1<<12), (1<<12)|(1<<20)] {
        put(&mut bus, 0x3ff4_f020, fifo);
        assert!(bus.camera_routed_frame().is_none());
    }
    receiver(&mut bus);
    put(&mut bus, 0x3ff4_f020, (3<<16)|(1<<12)|(1<<20));
    assert!(bus.camera_routed_frame().is_some());
    put(&mut bus, 0x3ff0_00c0, 1 << 11);
    assert!(bus.camera_routed_frame().is_none(), "I2S0 gate");
    receiver(&mut bus);
    put(&mut bus, 0x3ff0_00c4, 1 << 4);
    assert!(bus.camera_routed_frame().is_none(), "I2S0 reset gate");
}
#[test]
fn c6_deadline_edges_preserve_timestamps_and_large_steps() {
    use esp32::board::aithinker::{FRAME_CYCLES, VSYNC_CYCLES};
    use esp_soc::board::BoardEdge;
    let mut bus = running();
    let first = bus.board.next_deadline().unwrap();
    bus.board.advance_to(first + FRAME_CYCLES + VSYNC_CYCLES);
    assert_eq!(bus.board.take_edges(), [
        BoardEdge { cycle: first, pin: 25, level: true },
        BoardEdge { cycle: first + VSYNC_CYCLES, pin: 25, level: false },
        BoardEdge { cycle: first + FRAME_CYCLES, pin: 25, level: true },
        BoardEdge { cycle: first + FRAME_CYCLES + VSYNC_CYCLES, pin: 25, level: false },
    ]);
    let next = bus.board.next_deadline();
    bus.board.advance_to(0);
    assert_eq!(bus.board.next_deadline(), next, "advance is monotonic");
    assert!(bus.board.take_edges().is_empty());
    assert_eq!(next, Some(first + 2 * FRAME_CYCLES));
}
#[test]
fn c6_active_non_camera_board_never_queries_camera_clock() {
    use esp_soc::board::BoardModel;
    struct Button { now: u64 }
    impl BoardModel for Button {
        fn name(&self) -> &'static str { "button" }
        fn advance_to(&mut self, cycle: u64) { self.now = cycle; }
    }
    let mut board = Button { now: 0 };
    board.advance_inputs_with_clock(240, &|_| panic!("camera clock polled"));
    assert_eq!(board.now, 240);
    assert_eq!(board.camera_dvp_pins(), None);
}
#[test]
fn c6_camera_free_has_no_producer_deadlines_or_steps() {
    struct Idle;
    impl esp_soc::board::BoardModel for Idle {
        fn name(&self) -> &'static str { "idle" }
        fn uses_gpio_edges(&self) -> bool { false }
        fn next_deadline(&self) -> Option<u64> { panic!("camera-free deadline poll") }
        fn advance_inputs_with_clock(&mut self, _: u64, _: &dyn Fn(u8)->Option<(f64,u32)>) { panic!("camera-free producer step") }
        fn camera_dvp_pins(&self) -> Option<[u8;11]> { panic!("camera-free route poll") }
    }
    let mut bus = SocBus::new(0, [0;6]); bus.board = Box::new(Idle); bus.attach_board_devices();
    assert_eq!(esp_soc::SocBus::next_deadline(&bus), None);
    for _ in 0..100 { bus.tick(240); bus.read32(0x3ff4_403c).unwrap(); }
    assert_eq!(esp_soc::SocBus::next_deadline(&bus), None);
}
