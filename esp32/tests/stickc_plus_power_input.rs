//! Native queued-host/board/real AXP-register tests; no ROM, sockets or hardware.
use esp32::board::stickc_plus::StickCPlus;
use esp_periph::i2c::{axp192::Axp192State, I2cDevice};
use esp_soc::{web::WebServer, BoardModel};
use std::sync::{Arc, Mutex};
const HZ: u64 = 240_000_000;
fn fixture() -> (
    esp32::Machine,
    Arc<Mutex<Axp192State>>,
    WebServer,
    Box<dyn I2cDevice>,
) {
    let mut m = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    let mut board = StickCPlus::new();
    let pmic = board.pmic.clone();
    let device = board
        .i2c_devices()
        .into_iter()
        .find(|(bus, a, _)| *bus == 0 && *a == 0x34)
        .unwrap()
        .2;
    m.bus.board = Box::new(board);
    m.bus.attach_board_devices();
    let web = WebServer::queued();
    m.web = Some(web.clone());
    (m, pmic, web, device)
}
fn send(m: &mut esp32::Machine, w: &WebServer, json: &str) {
    let cycles = m.bus.cycles;
    let insns = m.insns();
    w.push_incoming(json.into());
    m.run(0);
    assert_eq!(m.bus.cycles, cycles, "input must not run guest time");
    assert_eq!(m.insns(), insns);
}
fn input(battery: u32, vbus: u32) -> String {
    format!(
        r#"{{"t":"power-environment","batteryVoltageUv":{battery},"batteryPresent":true,"vbusVoltageUv":{vbus},"vbusPresent":true,"vbusValid":true,"charging":true}}"#
    )
}
fn read(d: &mut dyn I2cDevice, r: u8) -> u8 {
    d.start(false);
    d.write(r);
    d.start(true);
    let v = d.read();
    d.stop();
    v
}
fn write(d: &mut dyn I2cDevice, r: u8, v: u8) {
    d.start(false);
    d.write(r);
    d.write(v);
    d.stop();
}
fn raw(d: &mut dyn I2cDevice, r: u8) -> u16 {
    (u16::from(read(d, r)) << 4) | u16::from(read(d, r + 1) & 15)
}
fn at(m: &mut esp32::Machine, us: u64) {
    m.bus.cycles = us * (HZ / 1_000_000);
    m.bus.board.advance_to(m.bus.cycles);
}
#[test]
fn queued_host_zero_low_threshold_high_and_adc_saturation_follow_guest_sample_clock() {
    for (uv, expected) in [
        (0, 0),
        (3_000_000, 2727),
        (3_200_000, 2909),
        (3_201_000, 2910),
        (4_100_000, 3727),
        (4_504_499, 4094),
        (4_504_500, 4095),
        (5_000_000, 4095),
    ] {
        let (mut m, s, w, mut d) = fixture();
        let pixels = m.bus.board.display().unwrap().2.to_vec();
        send(&mut m, &w, &input(uv, 5_000_000));
        assert_eq!(s.lock().unwrap().power_input().battery_voltage_uv, uv);
        assert_eq!(raw(&mut *d, 0x78), 0);
        at(&mut m, 39_999);
        assert_eq!(raw(&mut *d, 0x78), 0);
        at(&mut m, 40_000);
        assert_eq!(raw(&mut *d, 0x78), expected, "{uv}");
        assert_eq!(
            m.bus.board.display().unwrap().2,
            pixels,
            "environment input never draws a screen"
        );
    }
}
#[test]
fn host_environment_preserves_adc_disable_reset_and_writes_cannot_forge_readonly_results() {
    let (mut m, s, w, mut d) = fixture();
    at(&mut m, 40_000);
    assert_eq!(raw(&mut *d, 0x78), 3727);
    write(&mut *d, 0x82, 0);
    send(&mut m, &w, &input(3_000_000, 5_000_000));
    assert_eq!(raw(&mut *d, 0x78), 3727);
    at(&mut m, 120_000);
    assert_eq!(raw(&mut *d, 0x78), 3727);
    write(&mut *d, 0x82, 0x88);
    at(&mut m, 160_000);
    assert_eq!(raw(&mut *d, 0x78), 2727);
    assert_eq!(raw(&mut *d, 0x5a), 2941);
    for r in [0, 1, 0x78, 0x79, 0x5a, 0x5b] {
        let expected = read(&mut *d, r);
        write(&mut *d, r, 0xff);
        assert_eq!(read(&mut *d, r), expected);
    }
    s.lock().unwrap().reset_powered_fixture();
    assert_eq!(raw(&mut *d, 0x78), 0);
    assert_eq!(read(&mut *d, 0x82), 0x83);
    at(&mut m, 199_999);
    assert_eq!(raw(&mut *d, 0x78), 0);
    at(&mut m, 200_000);
    assert_eq!(raw(&mut *d, 0x78), 2727);
    assert_eq!(
        raw(&mut *d, 0x5a),
        0,
        "VBUS ADC resets disabled, not a forced USB reading"
    );
    assert_eq!(read(&mut *d, 0), 0x34);
    assert_eq!(read(&mut *d, 1), 0x60);
    write(&mut *d, 0x33, 0);
    assert_eq!(read(&mut *d, 0), 0x30);
    assert_eq!(read(&mut *d, 1), 0x20);
}
#[test]
fn malformed_or_out_of_contract_host_stimuli_leave_the_environment_unchanged() {
    let (mut m, s, w, _) = fixture();
    let before = s.lock().unwrap().power_input();
    let version = s.lock().unwrap().version;
    let valid = input(3_000_000, 5_000_000);
    for bad in [
        valid.replace("3000000", "-1"),
        valid.replace("3000000", "5000001"),
        valid.replace("5000000", "7000001"),
        valid.replace("3000000", "3000000.5"),
        valid.replace("3000000", "\"3000000\""),
        valid.replace("true", "1"),
        valid.replace("true", "\"true\""),
        valid.replace("\"charging\":true", "\"extra\":true"),
        valid.replace("\"t\":", "\"extra\":0,\"t\":"),
    ] {
        send(&mut m, &w, &bad);
        assert_eq!(s.lock().unwrap().power_input(), before, "{bad}");
        assert_eq!(s.lock().unwrap().version, version);
    }
}
#[test]
fn unrelated_classic_board_cannot_accept_plus_power_environment() {
    let (mut m, s, w, _) = fixture();
    let before = s.lock().unwrap().power_input();
    m.bus.board = esp32::board::make_board("atom-lite").unwrap();
    send(&mut m, &w, &input(0, 0));
    assert_eq!(s.lock().unwrap().power_input(), before);
}

#[test]
fn initial_queue_survives_boot_entry_setup_and_is_applied_before_any_guest_step() {
    let (mut m, s, w, mut d) = fixture();
    w.push_incoming(input(0, 0));
    m.boot_rom();
    assert_eq!(
        s.lock().unwrap().power_input().battery_voltage_uv,
        4_100_000
    );
    let cycles = m.bus.cycles;
    let insns = m.insns();
    m.run(0);
    assert_eq!(m.bus.cycles, cycles);
    assert_eq!(m.insns(), insns);
    assert_eq!(s.lock().unwrap().power_input().battery_voltage_uv, 0);
    at(&mut m, 40_000);
    assert_eq!(raw(&mut *d, 0x78), 0);
}
#[test]
fn cpu_software_reboot_preserves_external_pmic_input_and_adc_controls() {
    let (mut m, s, w, mut d) = fixture();
    send(&mut m, &w, &input(3_000_000, 0));
    at(&mut m, 40_000);
    assert_eq!(raw(&mut *d, 0x78), 2727);
    write(&mut *d, 0x82, 0);
    let cycles = m.bus.cycles;
    m.reboot();
    assert_eq!(m.bus.cycles, cycles);
    assert_eq!(
        s.lock().unwrap().power_input().battery_voltage_uv,
        3_000_000
    );
    assert_eq!(read(&mut *d, 0x82), 0);
    send(&mut m, &w, &input(4_100_000, 0));
    at(&mut m, 80_000);
    assert_eq!(raw(&mut *d, 0x78), 2727);
    write(&mut *d, 0x82, 0x80);
    at(&mut m, 120_000);
    assert_eq!(raw(&mut *d, 0x78), 3727);
}
