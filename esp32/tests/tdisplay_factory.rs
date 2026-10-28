//! Unmodified TTGO T-Display factory image. Download and SHA-256 instructions:
//! docs/evidence/classic-light-sleep-tdisplay/README.md. No image is mirrored here.
use std::{fs, path::Path};
use esp_soc::SocBus as _;

fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| (hash ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}
fn run_to(m: &mut esp32::Machine, seconds: f64) {
    m.max_cycles = (seconds * 240_000_000.0) as u64;
    while m.bus.cycles < m.max_cycles {
        let stop = m.run(2_000_000);
        assert!(matches!(stop, esp32::Stop::MaxInsns | esp32::Stop::Halted), "unexpected stop at {}: {stop:?}", m.bus.cycles);
    }
    m.drain_console();
}
fn checkpoint(m: &mut esp32::Machine, name: &str) -> u64 {
    let (w, h, pixels, version) = m.bus.board.display().expect("factory display");
    assert_eq!((w, h), (135, 240));
    let hash = fnv(pixels.iter().flat_map(|p| p.to_le_bytes()));
    println!("FACTORY {name} cycles={} frame-fnv64={hash:016x} display-version={version} instructions={}", m.bus.cycles, m.insns());
    if let Some(dir) = std::env::var_os("TDISPLAY_CAPTURE_DIR") {
        m.write_tft_png(Path::new(&dir).join(format!("{name}.png")).to_str().unwrap(), 3).unwrap();
    }
    hash
}

#[test]
#[ignore = "set TDISPLAY_FACTORY_BIN to the pinned official firmware.bin and ESP32SIM_ROM_DIR to the ROM directory"]
fn external_factory_boot_buttons_and_deep_sleep_wake() {
    let firmware = fs::read(std::env::var_os("TDISPLAY_FACTORY_BIN").expect("set TDISPLAY_FACTORY_BIN to the pinned official firmware.bin")).unwrap();
    assert_eq!(firmware.len(), 807728);
    // Secondary fixture identity check; use the documented SHA-256 before running.
    assert_eq!(fnv(firmware.iter().copied()), 0x5fb0_a5d3_5d59_af20);
    let roms = std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the ROM directory");
    let mut m = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    m.console.capture = true;
    m.bus.board = esp32::board::make_board("ttgo-tdisplay").unwrap();
    m.bus.attach_board_devices();
    m.load_rom(&fs::read(Path::new(&roms).join("esp32_rev300_rom.elf")).unwrap()).unwrap();
    m.write_flash(0, &firmware).unwrap();
    m.boot_rom();
    run_to(&mut m, 8.0);
    assert!(String::from_utf8_lossy(&m.console.all).contains("eFuse Vref:1100 mV"));
    let menu = checkpoint(&mut m, "menu");
    assert_eq!(menu, 0x95c5_ea57_a02c_b645);
    m.bus.gpio_set_input(35, false);
    run_to(&mut m, 8.1);
    m.bus.gpio_set_input(35, true);
    run_to(&mut m, 9.5);
    let voltage = checkpoint(&mut m, "voltage");
    assert_eq!(voltage, 0x4006_054a_b84c_daa8);
    let serial = String::from_utf8_lossy(&m.console.all);
    assert!(serial.contains("Detect Voltage..") && serial.contains("Voltage :0.00V"));
    run_to(&mut m, 10.0);
    m.bus.gpio_set_input(0, false);
    run_to(&mut m, 10.1);
    m.bus.gpio_set_input(0, true);
    run_to(&mut m, 15.0);
    let scan = checkpoint(&mut m, "scan");
    assert_eq!(scan, 0xe84d_73cc_7407_87bd);
    assert!(String::from_utf8_lossy(&m.console.all).contains("btn press wifi scan"));
    // Long right-button press uses the firmware's own light-sleep delay, then
    // EXT0 deep sleep. Release before it finishes the delay, then press to wake.
    m.bus.gpio_set_input(35, false);
    run_to(&mut m, 16.5);
    m.bus.gpio_set_input(35, true);
    run_to(&mut m, 23.0);
    let asleep = m.insns();
    assert_eq!(checkpoint(&mut m, "deep-sleep"), 0x2699_3a53_c3c4_39a5);
    run_to(&mut m, 24.0);
    assert_eq!(m.insns(), asleep, "deep sleep must park both cores");
    assert!(!m.bus.sw_reset());
    m.bus.gpio_set_input(35, false);
    assert!(matches!(m.run(1), esp32::Stop::SwReset));
    assert_eq!(m.bus.reset_cause(), 5, "only the physical wake should reset");
    m.reboot();
    run_to(&mut m, 24.1);
    m.bus.gpio_set_input(35, true);
    run_to(&mut m, 33.0);
    assert_eq!(checkpoint(&mut m, "wake-menu"), menu);
    let serial = String::from_utf8_lossy(&m.console.all);
    assert_eq!(serial.matches("Start\r\n").count(), 2);
    assert!(!serial.contains("RTCWDT"));
    println!("FACTORY PASS boot, GPIO35 voltage, GPIO0 scan, GPIO35 long-press sleep/wake");
}
