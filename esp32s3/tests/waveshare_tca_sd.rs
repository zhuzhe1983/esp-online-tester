//! Actual SDMMC MMIO/IDMAC and original factory media checks for the six TCA RGB
//! boards. The official SD_Card.cpp uses native one-bit SD, not GP-SPI.
use emu_core::Bus;
use esp32s3::board::waveshare_tca_rgb::{TcaRgbVariant, WaveshareTcaRgb};
use esp_periph::sdmmc::{self, reg, des0, int, CMD_START, CMD_RESP, CMD_DATA,
    CMD_WRITE, CMD_RESP_LONG, CTRL_USE_INTERNAL_DMA, CTRL_INT_ENABLE, BMOD_DE};
use esp_soc::{LoadKind, SocBus as _, Stop};

const HZ: u64 = 240_000_000;
const SD: u32 = 0x6002_8000;
const RAM: u32 = esp32s3::bus::DRAM_LOW + 0x1000;
const VARIANTS: [TcaRgbVariant; 6] = [TcaRgbVariant::TouchLcd21, TcaRgbVariant::TouchLcd21B,
    TcaRgbVariant::Lcd28B, TcaRgbVariant::TouchLcd28B, TcaRgbVariant::Lcd28C, TcaRgbVariant::TouchLcd28C];

fn write(bus: &mut esp32s3::bus::SocBus, reg: u32, value: u32) { bus.write32(SD + reg, value).unwrap(); }
fn command(bus: &mut esp32s3::bus::SocBus, number: u8, argument: u32, flags: u32) -> u32 {
    write(bus, reg::RINTSTS, u32::MAX); write(bus, reg::CMDARG, argument);
    write(bus, reg::CMD, CMD_START | 1 << 16 | u32::from(number) | flags);
    bus.read32(SD + reg::RESP0).unwrap()
}
fn tca(bus: &mut esp32s3::bus::SocBus, register: u8, value: u8) {
    let mut devices = bus.board.i2c_devices();
    let (_, _, device) = devices.iter_mut().find(|(b, a, _)| *b == 0 && *a == 0x20).unwrap();
    assert!(device.start(false)); assert!(device.write(register)); assert!(device.write(value)); device.stop();
}
fn wiring(bus: &mut esp32s3::bus::SocBus) {
    // SDMMC slot 1, from the already retained exact ESP32-S3 gpio_sig_map.h.
    for (pin, signal) in [(2, 173), (1, 179), (42, 213)] {
        bus.write32(0x6000_9004 + 4 * pin, 1 << 12 | 1 << 9).unwrap();
        bus.write32(0x6000_4554 + 4 * pin, signal).unwrap();
        bus.write32(0x6000_4154 + 4 * signal, 0x80 | pin).unwrap();
    }
    let enabled = bus.read32(0x600c_001c).unwrap(); bus.write32(0x600c_001c, enabled | 1 << 7).unwrap();
    let reset = bus.read32(0x600c_0024).unwrap(); bus.write32(0x600c_0024, reset & !(1 << 7)).unwrap();
    tca(bus, 3, 0xf7); tca(bus, 1, 0xff); // P3 output high: DAT3 native-mode selection.
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
fn all_six_native_sd_slots_obey_routes_dat3_and_idmac_read_write() {
    for variant in VARIANTS {
        let mut bus = esp32s3::bus::SocBus::new(16 << 20, 8 << 20, [0; 6]);
        bus.board = Box::new(WaveshareTcaRgb::new(variant)); bus.attach_board_devices();
        assert_eq!(bus.board.sd_card_slot(), Some(([2, 1, 42, 255, 255, 255], false)));
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
        write(&mut bus, reg::CTYPE, 0); tca(&mut bus, 1, 0xf7);
        assert!(!bus.board.sd_card_slot().unwrap().1);
        command(&mut bus, 8, 0x1aa, CMD_RESP);
        assert_ne!(bus.read32(SD + reg::RINTSTS).unwrap() & int::RTO, 0, "DAT3 low is outside the supported native-mode selection");
        tca(&mut bus, 1, 0xff); bus.board.set_sd_card_present(false);
        command(&mut bus, 8, 0x1aa, CMD_RESP);
        assert_ne!(bus.read32(SD + reg::RINTSTS).unwrap() & int::RTO, 0, "removed media must not reply");
    }
}

fn factory(variant: TcaRgbVariant) {
    let (sku, repo, file) = match variant {
        TcaRgbVariant::TouchLcd21 => ("ESP32-S3-Touch-LCD-2.1", "ESP32-S3-Touch-LCD-2.1", "ESP32-S3-Touch-LCD-2.1_FactoryOnly_20250224.bin"),
        TcaRgbVariant::TouchLcd21B => ("ESP32-S3-Touch-LCD-2.1B", "ESP32-S3-Touch-LCD-2.1", "ESP32-S3-Touch-LCD-2.1_FactoryOnly_20250224.bin"),
        TcaRgbVariant::Lcd28B => ("ESP32-S3-LCD-2.8B", "ESP32-S3-Touch-LCD-2.8B", "ESP32-S3-LCD-2.8B_FactoryOnly_20250124.bin"),
        TcaRgbVariant::TouchLcd28B => ("ESP32-S3-Touch-LCD-2.8B", "ESP32-S3-Touch-LCD-2.8B", "ESP32-S3-Touch-LCD-2.8B_FactoryOnly_20241025.bin"),
        TcaRgbVariant::Lcd28C => ("ESP32-S3-LCD-2.8C", "ESP32-S3-Touch-LCD-2.8C", "ESP32-S3-LCD-2.8C_FactoryOnly_20250124.bin"),
        TcaRgbVariant::TouchLcd28C => ("ESP32-S3-Touch-LCD-2.8C", "ESP32-S3-Touch-LCD-2.8C", "ESP32-S3-Touch-LCD-2.8C_FactoryOnly_20250224.bin"),
        TcaRgbVariant::Driver | TcaRgbVariant::Driver21 | TcaRgbVariant::Driver28 | TcaRgbVariant::Driver4 => panic!("Separate LCD Driver board has no complete factory image in this six-board SD fixture"),
    };
    let source = std::env::var("ESP32SIM_WAVESHARE_RGB_SOURCES").expect("pinned official image cache");
    let rom = std::env::var("ESP32SIM_ROM_DIR").expect("original S3 ROM");
    let image = std::env::var("ESP32SIM_WAVESHARE_SD_IMAGE").expect("deterministic 64 MiB default FAT32 image");
    let mut m = esp32s3::machine([2, 0, 0, 0, 0, 1]);
    m.bus.board = Box::new(WaveshareTcaRgb::new(variant)); m.bus.attach_board_devices();
    m.bus.set_flash_size(16 << 20); m.bus.set_psram_size(8 << 20).unwrap();
    m.console.capture = true; m.console.mask = 2;
    m.load_input(LoadKind::Rom, &std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).unwrap()).unwrap();
    m.load_input(LoadKind::Flash, &std::fs::read(format!("{source}/{repo}/firmware/{file}")).unwrap()).unwrap();
    m.load_input(LoadKind::SdCard, &std::fs::read(image).unwrap()).unwrap(); m.boot_rom();
    let ms: u64 = std::env::var("ESP32SIM_WAVESHARE_SD_MS").map_or(8000, |v| v.parse().unwrap());
    m.max_cycles = ms * HZ / 1000; assert!(matches!(m.run(u64::MAX), Stop::Halted)); m.bus.refresh_board_outputs();
    let uart = String::from_utf8_lossy(&m.console.all);
    let mounted = uart.contains("SD card initialization successful!") && !uart.contains("SD card initialization failed!");
    let s = &m.bus.periph.sdmmc.stats;
    let bytes_read = s.bytes_read;
    let result = format!("{{\"product\":\"{sku}\",\"guestCycles\":{},\"guestMilliseconds\":{},\"reboots\":{},\"factoryFilesystemMounted\":{mounted},\"sdmmcCommands\":{},\"sdmmcTimeouts\":{},\"bytesRead\":{},\"bytesWritten\":{},\"rgbFrames\":{}}}\n",
        m.bus.cycles, m.bus.cycles / (HZ / 1000), m.reboots, s.commands, s.timeouts, s.bytes_read, s.bytes_written, m.bus.board.display_frames());
    eprintln!("{result}");
    if let Ok(root) = std::env::var("ESP32SIM_WAVESHARE_SD_EVIDENCE") {
        let path = std::path::Path::new(&root).join(sku); std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("result.json"), result).unwrap();
        std::fs::write(path.join("uart.txt"), &m.console.all).unwrap();
        std::fs::write(path.join("board.txt"), m.bus.board.report()).unwrap();
        if let Some((_, _, pixels, _)) = m.bus.board.display() {
            std::fs::write(path.join("frame.rgb565"), pixels.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>()).unwrap();
            m.write_tft_png(path.join("frame.png").to_str().unwrap(), 1).unwrap();
        }
    }
    assert_eq!(m.reboots, 0);
    if std::env::var_os("ESP32SIM_WAVESHARE_SD_EXPECT_FAILURE").is_some() {
        assert!(!mounted, "explicit pre-fix diagnostic must reproduce the mount failure");
    } else {
        assert!(mounted, "official factory did not mount the inserted image; preserve its actual UART failure");
        assert!(bytes_read > 0, "success text alone cannot prove storage transport");
    }
}

#[test] #[ignore = "official firmware, mask ROM and exact default FAT32 image required"] fn external_sd_touch21() { factory(TcaRgbVariant::TouchLcd21); }
#[test] #[ignore = "official firmware, mask ROM and exact default FAT32 image required"] fn external_sd_touch21b() { factory(TcaRgbVariant::TouchLcd21B); }
#[test] #[ignore = "official firmware, mask ROM and exact default FAT32 image required"] fn external_sd_lcd28b() { factory(TcaRgbVariant::Lcd28B); }
#[test] #[ignore = "official firmware, mask ROM and exact default FAT32 image required"] fn external_sd_touch28b() { factory(TcaRgbVariant::TouchLcd28B); }
#[test] #[ignore = "official firmware, mask ROM and exact default FAT32 image required"] fn external_sd_lcd28c() { factory(TcaRgbVariant::Lcd28C); }
#[test] #[ignore = "official firmware, mask ROM and exact default FAT32 image required"] fn external_sd_touch28c() { factory(TcaRgbVariant::TouchLcd28C); }
