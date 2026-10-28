//! Official MicroPython v1.29.0 ESP32_GENERIC-SPIRAM image, unmodified.
//! Download and SHA-256: docs/evidence/classic-psram-dclk/README.md.
use esp_soc::SocBus;
use std::{env, fs, path::PathBuf};

fn free_heap(mb: usize) -> usize {
    let rom = PathBuf::from(env::var_os("ESP32SIM_ROM_DIR").expect("ESP32SIM_ROM_DIR"))
        .join("esp32_rev300_rom.elf");
    let image = env::var_os("MICROPYTHON_SPIRAM_IMAGE").expect("MICROPYTHON_SPIRAM_IMAGE");
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.bus.set_psram_size(mb << 20).unwrap();
    m.console.capture = true;
    m.load_rom(&fs::read(rom).unwrap()).unwrap();
    m.write_flash(0x1000, &fs::read(image).unwrap()).unwrap();
    m.boot_rom();
    let mut injected = false;
    for _ in 0..3000 {
        let stop = m.run(100_000);
        let serial = String::from_utf8_lossy(&m.console.uart0);
        if serial.contains("\r\nPSRAM_FREE ") { break; }
        if !injected && serial.contains(">>> ") {
            m.bus.serial_input(b"import gc; print('PSRAM_FREE', gc.mem_free())\r\n");
            m.sync_irq();
            injected = true;
        }
        assert!(matches!(stop, esp_soc::Stop::MaxInsns), "{stop:?}");
    }
    let serial = String::from_utf8_lossy(&m.console.uart0);
    println!("MicroPython psram_mb={mb}:\n{serial}");
    assert!(serial.contains("MicroPython v1.29.0 on 2026-08-24; Generic ESP32 module with SPIRAM"));
    assert_eq!(serial.contains("PSRAM ID read error: 0xffffffff"), mb == 0);
    assert!(injected, "REPL never reached");
    assert!(m.stubs.is_empty());
    assert_eq!(m.stub_hits, 0);
    assert_eq!(m.reboots, 0);
    serial.lines().find_map(|line| line.strip_prefix("PSRAM_FREE "))
        .expect("gc.mem_free() result").trim().parse().unwrap()
}

#[test]
#[ignore = "needs MICROPYTHON_SPIRAM_IMAGE official v1.29.0 SPIRAM image and ESP32SIM_ROM_DIR"]
fn external_micropython_spiram_free_heap() {
    let present = free_heap(4);
    let absent = free_heap(0);
    println!("MicroPython gc.mem_free(): 4 MiB={present}, absent={absent}");
    assert!((3_900_000..4_200_000).contains(&present));
    assert!((50_000..250_000).contains(&absent));
    assert!(present > absent + 3_500_000);
}
