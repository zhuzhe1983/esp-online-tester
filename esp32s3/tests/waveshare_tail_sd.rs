//! Actual SDMMC MMIO/IDMAC uses the LCD2.8 native one-bit source pins.
//! Real512-byte card write/read roundtrip; incorrect route, width and eject time out.
use emu_core::Bus;

use esp_periph::sdmmc::{self, reg, des0, int, CMD_START, CMD_RESP, CMD_DATA,
    CMD_WRITE, CMD_RESP_LONG, CTRL_USE_INTERNAL_DMA, CTRL_INT_ENABLE, BMOD_DE};
use esp_soc::SocBus as _;

const SD: u32 = 0x6002_8000;
const RAM: u32 = esp32s3::bus::DRAM_LOW + 0x1000;
fn write(bus: &mut esp32s3::bus::SocBus, reg: u32, value: u32) { bus.write32(SD + reg, value).unwrap(); }
fn command(bus: &mut esp32s3::bus::SocBus, number: u8, argument: u32, flags: u32) -> u32 {
    write(bus, reg::RINTSTS, u32::MAX); write(bus, reg::CMDARG, argument);
    write(bus, reg::CMD, CMD_START | 1 << 16 | u32::from(number) | flags);
    bus.read32(SD + reg::RESP0).unwrap()
}
fn wiring(bus: &mut esp32s3::bus::SocBus) {
    // SDMMC slot 1, from the already retained exact ESP32-S3 gpio_sig_map.h.
    for (pin, signal) in [(14, 173), (17, 179), (16, 213)] {
        bus.write32(0x6000_9004 + 4 * pin, 1 << 12 | 1 << 9).unwrap();
        bus.write32(0x6000_4554 + 4 * pin, signal).unwrap();
        bus.write32(0x6000_4154 + 4 * signal, 0x80 | pin).unwrap();
    }
    let enabled = bus.read32(0x600c_001c).unwrap(); bus.write32(0x600c_001c, enabled | 1 << 7).unwrap();
    let reset = bus.read32(0x600c_0024).unwrap(); bus.write32(0x600c_0024, reset & !(1 << 7)).unwrap();
}
fn ready(bus: &mut esp32s3::bus::SocBus) {
    command(bus, 0, 0, 0); assert_eq!(command(bus, 8, 0x1aa, CMD_RESP), 0x1aa);
    for _ in 0..2 { command(bus, 55, 0, CMD_RESP); command(bus, 41, 0x40ff8000, CMD_RESP); }
    command(bus, 2, 0, CMD_RESP | CMD_RESP_LONG);
    let rca = command(bus, 3, 0, CMD_RESP) & 0xffff0000; command(bus, 7, rca, CMD_RESP);
    write(bus, reg::CTYPE, 0); // Exactly one data line; no invented D1/D2 GPIOs.
}
fn transfer(bus: &mut esp32s3::bus::SocBus, number: u8, buffer: u32) {
    for (i, value) in [des0::OWN | des0::FS | des0::LD, 512, buffer, 0].into_iter().enumerate() {
        bus.write32(RAM + i as u32 * 4, value).unwrap();
    }
    write(bus, reg::CTRL, CTRL_USE_INTERNAL_DMA | CTRL_INT_ENABLE); write(bus, reg::BMOD, BMOD_DE);
    write(bus, reg::DBADDR, RAM); write(bus, reg::BLKSIZ, 512); write(bus, reg::BYTCNT, 512);
    command(bus, number, 77, CMD_RESP | CMD_DATA | if number == 24 { CMD_WRITE } else { 0 });
    assert_ne!(bus.read32(SD + reg::RINTSTS).unwrap() & int::DATA_OVER, 0);
    assert_eq!(bus.read32(RAM).unwrap() & des0::OWN, 0);
}

#[test]
fn three_lcd28_slots_obey_exact_native_routes_and_idmac_read_write() {
    for name in ["waveshare-s3-lcd28", "waveshare-s3-touch-lcd28-v1", "waveshare-s3-touch-lcd28-v2"] {
        let mut bus = esp32s3::bus::SocBus::new(16 << 20, 8 << 20, [0; 6]);
        bus.board = esp32s3::board::make_board(name).unwrap(); bus.attach_board_devices();
        assert_eq!(bus.board.sd_card_slot(), Some(([14, 17, 16, 255, 255, 255], false)));
        bus.attach_sd_card(Box::new(sdmmc::MemStore(vec![0; 1 << 20])), Default::default()).unwrap();
        assert_eq!(bus.periph.sdmmc.devices.len(), 1, "native host owns the inserted image");
        wiring(&mut bus); ready(&mut bus);
        for i in 0..512 { bus.write8(RAM + 512 + i, (i % 251) as u8).unwrap(); }
        transfer(&mut bus, 24, RAM + 512); transfer(&mut bus, 17, RAM + 1024);
        for i in 0..512 { assert_eq!(bus.read8(RAM + 1024 + i).unwrap(), (i % 251) as u8); }
        assert_eq!(bus.periph.sdmmc.stats.bytes_written, 512);
        assert_eq!(bus.periph.sdmmc.stats.bytes_read, 512);
        for (signal, wrong_pin) in [(179, 3), (213, 41)] {
            bus.write32(0x6000_4154 + 4 * signal, 0x80 | wrong_pin).unwrap();
            command(&mut bus, 8, 0x1aa, CMD_RESP);
            assert_ne!(bus.read32(SD + reg::RINTSTS).unwrap() & int::RTO, 0, "wrong CMD/D0 input must time out");
            wiring(&mut bus);
        }
        write(&mut bus, reg::CTYPE, 2); command(&mut bus, 8, 0x1aa, CMD_RESP);
        assert_ne!(bus.read32(SD + reg::RINTSTS).unwrap() & int::RTO, 0, "unwired four-bit mode must time out");
        write(&mut bus, reg::CTYPE, 0); bus.board.set_sd_card_present(false);
        command(&mut bus, 8, 0x1aa, CMD_RESP);
        assert_ne!(bus.read32(SD + reg::RINTSTS).unwrap() & int::RTO, 0, "removed media must not reply");
    }
}
