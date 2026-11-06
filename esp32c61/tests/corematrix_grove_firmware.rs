//! Original firmware executes every I2C transaction; no app/ROM patches or
//! direct register readback is substituted for MicroPython's hardware driver.
use esp_soc::SocBus;
use esp_periph::i2c::env4::Environment;
fn python(m:&mut esp32c61::Machine,source:&str,marker:&str) {
    m.console.all.clear();m.bus.serial_input(&[1]);m.run(2_000_000);
    for chunk in source.as_bytes().chunks(64) {m.bus.serial_input(chunk);m.run(1_000_000);}
    m.bus.serial_input(&[4]);
    for _ in 0..100 {m.run(10_000_000);if m.console.all.windows(marker.len()).any(|x|x==marker.as_bytes()){break;}}
    let text=String::from_utf8_lossy(&m.console.all);assert!(text.contains(marker),"missing {marker}: {text}");assert!(!text.contains("Traceback"),"{text}");assert_eq!(m.exceptions,0,"{text}");println!("{text}");
    m.run(1_000_000);m.bus.serial_input(&[2]);m.run(1_000_000);
}
fn boot(m:&mut esp32c61::Machine) {
    m.run(100_000_000);let text=String::from_utf8_lossy(&m.console.all);assert!(text.contains("M5Stack CoreMatrix with ESP32-C61HR8"),"{text}");assert!(text.contains(">>>"),"{text}");
}
#[test]
#[ignore="requires ESP32SIM_COREMATRIX_FIRMWARE (official UIFlow2 CoreMatrix image) and ESP32SIM_ROM_DIR"]
fn external_corematrix_original_firmware_grove_i2c_power_detach_and_reset() {
    let path=std::env::var_os("ESP32SIM_COREMATRIX_FIRMWARE").expect("set ESP32SIM_COREMATRIX_FIRMWARE to the original official CoreMatrix image");
    let roms=std::path::PathBuf::from(std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to fetched mask ROMs"));
    let mut m=esp32c61::machine([2,0,0,0,0,0x61],16<<20);m.console.capture=true;m.console.mask=0;m.dbg.stop_after_exceptions=1;
    m.bus.set_psram_size(8<<20).unwrap();m.bus.periph.usb.connected=true;m.bus.board=esp32c61::board::make_board("corematrix").unwrap();m.bus.attach_board_devices();
    let ambient=Environment{temperature_c:22.0,humidity_percent:60.0,pressure_hpa:990.0};
    assert!(m.bus.board.accessory_environment_at(0,"sht40-bmp280-u001d","grove",true,ambient));
    assert!(m.bus.board.accessory_illuminance_at(0,"bh1750fvi-u136","grove",true,1000.0));
    m.load_rom(&std::fs::read(roms.join("esp32c61_rev100_rom.elf")).expect("read esp32c61_rev100_rom.elf")).unwrap();m.write_flash(0,&std::fs::read(path).expect("read original official CoreMatrix firmware")).unwrap();m.boot_rom();boot(&mut m);
    let source=include_str!("../../tools/fixtures/corematrix-uiflow/grove.py");python(&mut m,source,"CM_GROVE_POWER_RESET_PASS");
    let cycle=m.bus.cycles();m.bus.board.accessory_environment_at(cycle,"sht40-bmp280-u001d","grove",false,ambient);
    python(&mut m,"nack(lambda:bus.readfrom_mem(0x76,0xd0,1))\nassert len(bus.readfrom(0x23,2))==2\nprint('CM_GROVE_LEAF_DETACHED')\n","CM_GROVE_LEAF_DETACHED");
    let cycle=m.bus.cycles();m.bus.board.accessory_illuminance_at(cycle,"bh1750fvi-u136","grove",false,1000.0);
    python(&mut m,"nack(lambda:bus.readfrom(0x23,2))\nassert len(bus.readfrom_mem(0x6e,0,1))==1\nprint('CM_GROVE_ALL_DETACHED')\n","CM_GROVE_ALL_DETACHED");
    let cycle=m.bus.cycles();m.bus.board.accessory_environment_at(cycle,"sht40-bmp280-u001d","grove",true,ambient);m.bus.board.accessory_illuminance_at(cycle,"bh1750fvi-u136","grove",true,1000.0);
    m.console.all.clear();m.reboot();boot(&mut m);python(&mut m,source,"CM_GROVE_POWER_RESET_PASS");
    println!("CM_GROVE_REBOOT_REATTACH_PASS");
}
