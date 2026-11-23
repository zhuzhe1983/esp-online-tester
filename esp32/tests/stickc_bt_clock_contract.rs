//! Audit the exact ROM-side contract without inventing the missing peripheral.
use emu_core::Bus;
#[test]
#[ignore = "needs esp32_rev300_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_classic_rom_clock_request_and_result_contract() {
    let bytes=std::fs::read(esp_soc::rom::require("esp32_rev300_rom.elf")).unwrap();
    let elf=esp_soc::elf::parse(&bytes).unwrap();
    for (name,pc) in [("r_ld_read_clock",0x4003c9e4),("r_ea_time_get_halfslot_rounded",0x40015894),("r_ea_time_get_slot_rounded",0x400158d4)] {assert_eq!(elf.by_name[name],pc);}
    let mut m=esp32::machine([0;6],4<<20);m.load_rom(&bytes).unwrap();
    // Literal pools referenced by the three unmodified getters. In particular,
    // native-clock +0x1c must not be confused with BLEBASETIMECNT +0x21c.
    for (addr,value) in [(0x40015888,0x3ff7101c),(0x4001588c,0x7fffffff),(0x40011830,0x80000000),(0x40015890,0x0fffffff),(0x40014e90,0x07ffffff),(0x400158d0,0x3ff71020)] {assert_eq!(m.bus.read32_unpriced(addr).unwrap(),value);}
    // memw; l32i.n a2,a8,0; bltz a2,<loop>. Checks the actual request-busy
    // contract; it does NOT assert that a future real sampler must stay busy.
    for pc in [0x4003ca00,0x400158b0] {
        let actual:Vec<_>=(0..8).map(|i|m.bus.read8_unpriced(pc+i).unwrap()).collect();
        assert_eq!(actual,[0xc0,0x20,0,0x28,0x08,0x96,0x72,0xff]);
    }
}
