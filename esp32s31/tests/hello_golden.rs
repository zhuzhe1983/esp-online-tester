//! IDF v6.1 hello_world for esp32s31, booted from the real mask ROM on the Machine path.
//! Ignored by default: needs the ROM ELF and images from tools/fetch-s31-firmware.sh.
//!   S31_ROM_ELF=…/esp32s31_rev0_rom.elf cargo test -p esp32s31 --release --test hello_golden -- --ignored
use esp_soc::SocBus as _;

const GOLDEN: &str = "I (63) main_task: Started on CPU0\r\nI (63) main_task: Calling app_main()\r\nHello world!\r\n\
This is esp32s31 chip with 2 CPU core(s), WiFi/BLE, 802.15.4 (Zigbee/Thread), silicon revision v0.0, 2MB external flash\r\n\
Minimum free heap size: 478208 bytes\r\nRestarting in 10 seconds...\r\nRestarting in 9 seconds...\r\n";

#[test]
#[ignore = "needs S31_ROM_ELF and tools/fetch-s31-firmware.sh images"]
fn idf61_hello_world_prints_from_mask_rom_spi_boot() {
    let rom = std::fs::read(std::env::var("S31_ROM_ELF").expect("S31_ROM_ELF")).unwrap();
    let dir = std::env::var("S31_FIRMWARE_DIR").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/firmware").into());
    let mut m = esp32s31::machine([0x10, 0x20, 0x30, 0x40, 0x50, 0x60], 16 << 20);
    m.bus.load_mask_rom(&rom).unwrap();
    for (off, f) in [(0x2000, "s31-hello-bootloader.bin"), (0x8000, "s31-hello-ptable.bin"), (0x10000, "s31-hello_world.bin")] {
        m.bus.write_flash(off, &std::fs::read(format!("{dir}/{f}")).unwrap()).unwrap();
    }
    m.bus.set_board_inputs(esp32s31::soc::BoardInputs::devkit_w25q128()).unwrap();
    m.bus.set_strap(0x10); // SPI_FAST_FLASH_BOOT
    m.boot_rom(); m.cores[0].x[2] = esp32s31::ROM_STACK;
    m.console.mask = 2; m.console.capture = true; // UART0 is the IDF console (GPIO 59/58)
    let text = |m: &esp_soc::Machine<_>| format!("{}{}", String::from_utf8_lossy(&m.console.usb), String::from_utf8_lossy(&m.console.uart0));
    let mut n = 0;
    while n < 1_500_000_000u64 && !text(&m).contains("Restarting in 9 seconds") {
        let s = m.run(10_000_000); assert!(matches!(s, esp_soc::Stop::MaxInsns), "{s:?}"); n += 10_000_000;
    }
    let out = text(&m);
    assert!(out.contains("ESP-ROM:esp32s31-20251218\r\n"), "{out}");
    assert!(out.contains("rst:0x1 (POWERON),boot:0x10 (SPI_FAST_FLASH_BOOT)"), "{out}");
    assert!(out.contains("I (58) app_init: App version:      s31-hello-1"), "{out}");
    assert!(out.contains(GOLDEN), "{out}");
}
