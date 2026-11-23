//! The classic ROM ELF has BT RAM data but omits its pointer-backed ROM copy.
use emu_core::Bus;
#[test]
#[ignore = "needs esp32_rev300_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_classic_bluetooth_rom_initializer_has_its_rom_source_copy() {
    let bytes=std::fs::read(esp_soc::rom::require("esp32_rev300_rom.elf")).unwrap();
    let elf=esp_soc::elf::parse(&bytes).unwrap();let data=elf.sections.iter().find(|s|s.name==".data_btdm").unwrap();
    assert_eq!(data.addr,0x3ffae6e0);assert_eq!(data.data.len(),0x1830);
    let mut m=esp32::machine([0;6],4<<20);m.load_rom(&bytes).unwrap();
    let source=m.bus.read32_unpriced(elf.by_name["_data_start_btdm_rom"]).unwrap();assert_eq!(source,0x4000d890);
    let actual:Vec<_>=(0..data.data.len()).map(|i|m.bus.read8_unpriced(source+i as u32).unwrap()).collect();assert_eq!(actual,data.data);
    let offset=(elf.by_name["r_modules_funcs_p"]-data.addr) as usize;
    let expected=u32::from_le_bytes(actual[offset..offset+4].try_into().unwrap());assert_eq!(expected,elf.by_name["r_modules_funcs"]);
    // This is the pointer used by the unmodified guest getter, not a patched
    // return value or Bluetooth-controller emulation.
    assert_eq!(m.bus.read32_unpriced(source+offset as u32).unwrap(),0x3ffafd6c);
}
