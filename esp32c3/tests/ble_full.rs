use esp_soc::{SocBus, Stop};

#[test]
fn machine_api_excludes_hci_and_full_ble_in_both_orders() {
    let mut m = esp32c3::machine([0; 6], 4 << 20);
    m.bus.enable_ble_full(false).unwrap();
    assert!(m.bus.enable_ble(&Default::default()).unwrap_err().contains("mutually exclusive"));
    let mut m = esp32c3::machine([0; 6], 4 << 20);
    m.bus.ble.hooks.push(0x4200_0000);
    assert!(m.bus.enable_ble_full(false).unwrap_err().contains("mutually exclusive"));
    assert!(!m.bus.periph.ble_lc.enabled());
}

#[test]
#[ignore = "needs the ESP32-C3 mask ROM ELF (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn guest_esp_restart_preserves_full_ble_and_advertises_again() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut m = esp32c3::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    m.load_rom(&std::fs::read(esp_soc::rom::require("esp32c3_rev3_rom.elf")).unwrap()).unwrap();
    for (off, name) in [(0, "c3-ble-bootloader.bin"), (0x8000, "c3-ble-ptable.bin"), (0x10000, "c3-ble-advertiser.bin")] {
        m.write_flash(off, &std::fs::read(root.join("web/wasm/fw/public").join(name)).unwrap()).unwrap();
    }
    m.bus.enable_ble_full(true).unwrap();
    m.boot_rom();
    m.run(320_000_000);
    let mut first = 0;
    while let Some(line) = m.bus.periph.ble_lc.take_observation() { first += usize::from(line.starts_with("[ble-air]")); }
    assert_eq!(first, 57);
    // esp_restart in the pinned IDF 5.5.5 advertiser ELF, SHA-256 b7eb3e20…
    // No stub: execute the guest restart function and its RTC watchdog path.
    m.cores[0].pc = 0x4200_0722;
    m.cores[0].waiting = false;
    assert!(matches!(m.run(160_000_000), Stop::SwReset));
    m.reboot();
    m.run(320_000_000);
    let mut second = 0;
    while let Some(line) = m.bus.periph.ble_lc.take_observation() { second += usize::from(line.starts_with("[ble-air]")); }
    assert_eq!(second, 57);
}

#[test]
#[ignore = "needs the ESP32-C3 mask ROM ELF (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn bluetooth_rom_initializer_has_its_rom_source_copy() {
    use emu_core::Bus;
    let bytes = std::fs::read(esp_soc::rom::require("esp32c3_rev3_rom.elf")).unwrap();
    let elf = esp_soc::elf::parse(&bytes).unwrap();
    let data = elf.sections.iter().find(|s| s.name == ".data_btdm").unwrap();
    assert!(data.data.iter().any(|&b| b != 0));
    let mut m = esp32c3::machine([0;6], 4 << 20);
    m.load_rom(&bytes).unwrap();
    let source = m.bus.read32_unpriced(elf.by_name["_data_start_btdm_rom"]).unwrap();
    let actual: Vec<_> = (0..data.data.len()).map(|i| m.bus.read8_unpriced(source + i as u32).unwrap()).collect();
    assert_eq!(actual, data.data);
}
