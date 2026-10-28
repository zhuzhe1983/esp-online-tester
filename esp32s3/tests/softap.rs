//! Opt-in unmodified firmware check for the virtual SoftAP station.
use esp_soc::{LoadKind, SocBus, Stop};

#[test]
#[ignore = "requires ESP32SIM_SOFTAP_FIRMWARE (StopWatch Xiaozhi merged image) and ESP32SIM_ROM_DIR"]
fn external_stopwatch_xiaozhi_softap_association_and_arp() {
    let path = std::env::var("ESP32SIM_SOFTAP_FIRMWARE").expect("set ESP32SIM_SOFTAP_FIRMWARE to the unmodified StopWatch Xiaozhi merged BIN");
    let rom = std::env::var("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the ESP32-S3 ROM directory");
    let mut m = esp32s3::machine([0x44, 0x1b, 0xf6, 0x75, 0xdc, 0xe0]);
    m.bus.board = esp32s3::board::make_board("stopwatch").unwrap();
    esp32s3::board::configure_chip_identity(m.bus.board.name(), &mut m.bus.periph.efuse);
    m.bus.attach_board_devices(); m.bus.set_flash_size(16 << 20); m.bus.set_psram_size(8 << 20).unwrap();
    m.console.capture = true; m.console.mask = 2;
    m.load_input(LoadKind::Rom, &std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
    m.load_input(LoadKind::Flash, &std::fs::read(path).unwrap()).unwrap();
    m.bus.attach_wifi(esp_soc::wifi::ApConfig::parse("ssid=esp32sim,psk=esp32sim-pass").unwrap(), None).unwrap();
    m.bus.configure_softap_client(Some(Default::default())).unwrap();
    m.boot_rom();
    for c in &mut m.cores { emu_core::Core::set_jit(c, false); }
    let mut arp_sent = false; let mut arp_reply = false; let mut last = String::new();
    for _ in 0..2000 {
        m.bus.poll_softap_client();
        m.max_cycles = m.bus.cycles + 2_400_000;
        let stop = m.run(u64::MAX); assert!(matches!(stop, Stop::Halted), "unexpected stop: {stop:?}");
        let status = m.bus.softap_client_status();
        if status != last { eprintln!("{} {}", m.bus.cycles / 240_000, status); last = status.clone(); }
        if status.contains("\"state\":\"connected\"") && !arp_sent {
            let mac = esp_soc::wifi::softap::CLIENT_MAC;
            let mut frame = vec![255; 6]; frame.extend_from_slice(&mac); frame.extend_from_slice(&[8, 6, 0, 1, 8, 0, 6, 4, 0, 1]);
            frame.extend_from_slice(&mac); frame.extend_from_slice(&[192, 168, 4, 2]); frame.extend_from_slice(&[0; 6]); frame.extend_from_slice(&[192, 168, 4, 1]);
            m.bus.receive_softap_frame(&frame).unwrap(); arp_sent = true;
        }
        for frame in m.bus.take_softap_frames() {
            eprintln!("SoftAP Ethernet {} bytes {:02x?}", frame.len(), &frame[..frame.len().min(42)]);
            if frame.len() >= 42 && frame[12..14] == [8, 6] && frame[20..22] == [0, 2] && frame[28..32] == [192, 168, 4, 1] { arp_reply = true; }
        }
        if arp_reply { break; }
    }
    eprintln!("{}", String::from_utf8_lossy(&m.console.uart0));
    assert!(arp_sent, "SoftAP association failed: {last}");
    assert!(arp_reply, "guest did not answer ARP after association: {last}");
}
