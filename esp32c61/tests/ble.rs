//! Caller-supplied original UIFlow; radio observations originate in guest descriptors.
use esp_soc::SocBus;
fn boot()->esp32c61::Machine {
    let rom=std::path::PathBuf::from(std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the fetched public ROM directory")).join("esp32c61_rev100_rom.elf");
    let image=std::env::var_os("ESP32SIM_COREMATRIX_FIRMWARE").expect("set ESP32SIM_COREMATRIX_FIRMWARE to original official CoreMatrix UIFlow 2.5.4");
    let mut m=esp32c61::machine([2,0,0,0,0,0x61],16<<20);m.console.capture=true;m.console.mask=0;m.dbg.stop_after_exceptions=1;m.bus.set_psram_size(8<<20).unwrap();m.bus.periph.usb.connected=true;
    m.bus.board=esp32c61::board::make_board("corematrix").unwrap();m.bus.attach_board_devices();m.load_rom(&std::fs::read(rom).unwrap()).unwrap();m.write_flash(0,&std::fs::read(image).unwrap()).unwrap();m.boot_rom();m.run(100_000_000);m
}
fn run_python(m:&mut esp32c61::Machine,program:&[u8],budget:u64) {
    m.bus.serial_input(&[1]);m.run(2_000_000);for bytes in program.chunks(64) {m.bus.serial_input(bytes);m.run(1_000_000);}m.bus.serial_input(&[4]);m.run(budget);
}
#[test]
#[ignore="requires ESP32SIM_COREMATRIX_FIRMWARE original UIFlow 2.5.4 and ESP32SIM_ROM_DIR"]
fn external_corematrix_ble_legacy_advertisements_are_real_dma_pdus() {
    let mut m=boot();run_python(&mut m,b"import bluetooth,time\nb=bluetooth.BLE();b.active(True)\nb.gap_advertise(100000,b'\\x02\\x01\\x06\\x04\\x09C61')\ntime.sleep_ms(250)\nb.gap_advertise(None);b.active(False);print('C61_BLE_ADV_DONE')\n",200_000_000);
    let text=String::from_utf8_lossy(&m.console.all);assert!(text.contains("C61_BLE_ADV_DONE"),"{text}");assert_eq!(m.exceptions,0,"{text}");
    let mut frames=Vec::new();while let Some(frame)=m.bus.periph.ble_mac.take_tx() {frames.push(frame);}
    assert!(frames.len()>=6,"at least two physical three-channel advertising events, got {frames:?}");
    for (i,frame) in frames.iter().enumerate() {assert_eq!(frame.channel,37+(i%3) as u8);assert_eq!(frame.pdu,b"\x20\x0e\x63\0\0\0\0\x02\x02\x01\x06\x04\x09C61");}
    for pair in frames.windows(4).step_by(3) {assert!((100_000..=110_000).contains(&pair[3].timestamp_us.wrapping_sub(pair[0].timestamp_us)),"interval includes the controller's random advertising delay: {pair:?}");}
}

#[test]
#[ignore="requires ESP32SIM_COREMATRIX_FIRMWARE original UIFlow 2.5.4 and ESP32SIM_ROM_DIR"]
fn external_corematrix_ble_nearby_reaches_original_scan_callback() {
    let mut m=boot();m.bus.periph.ble_mac.trace=std::env::var_os("COREMATRIX_BLE_TRACE").is_some();
    m.bus.ble_nearby(vec![esp_soc::ble::peer::Nearby::parse("name=Nearby61,addr=12:34:56:78:9a:bc,type=public,rssi=-42").unwrap(),esp_soc::ble::peer::Nearby::parse("name=Other,addr=ca:fe:00:00:00:42,type=random,rssi=-71").unwrap()]).unwrap();
    run_python(&mut m,b"import bluetooth,time,ubinascii\nb=bluetooth.BLE();b.active(True)\ndef irq(e,d):\n if e==5: print('BLE_RX',d[0],ubinascii.hexlify(bytes(d[1])),d[2],d[3],ubinascii.hexlify(bytes(d[4])))\n elif e==6: print('BLE_SCAN_DONE')\nb.irq(irq)\nb.gap_scan(500,30000,30000,False)\ntime.sleep_ms(600)\nb.active(False);print('C61_BLE_RX_DONE')\n",250_000_000);
    let text=String::from_utf8_lossy(&m.console.all);assert_eq!(m.exceptions,0,"{text}");assert!(text.contains("C61_BLE_RX_DONE"),"{text}");assert!(text.contains("BLE_RX 0 b'123456789abc' 2 -42 b'02010609094e65617262793631'"),"{text}");assert!(text.contains("BLE_RX 1 b'cafe00000042' 2 -71 b'02010606094f74686572'"),"{text}");assert!(text.contains("BLE_SCAN_DONE"),"{text}");assert!(m.bus.periph.ble_mac.received>=8);
}
