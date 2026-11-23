//! Opt-in probe for the original StickT2 v1.0 image. See docs/platform/stickt2-runtime.md.
#[path = "../../tests/common.rs"]
mod common;

use std::{fs, path::Path};
use esp32::board::stickt2::StickT2;
use esp_soc::BoardModel;

const FLASH_BYTES: usize = 4 << 20;
const FACTORY_SHA: &str = "bb33ffd41f9c6f9235b2fb44eac893b7ce6f70879f48eed9cb1ff75ef9419cd7";
const ROM_SHA: &str = "920b70635440517866aab2230964a570d2cf2b676658d93c52fbac108c1cca31";
const EIGHT_SECONDS: u64 = 1_920_000_000;

#[test]
#[ignore = "requires STICKT2_FACTORY_BIN original v1.0 image and ESP32SIM_ROM_DIR public ESP32 ROM"]
fn external_stickt2_original_factory_boot_probe() {
    let firmware = fs::read(std::env::var_os("STICKT2_FACTORY_BIN").expect("set STICKT2_FACTORY_BIN to the pinned original v1.0 image")).unwrap();
    assert_eq!(firmware.len(), FLASH_BYTES);
    assert_eq!(common::sha256_hex(&firmware), FACTORY_SHA);
    let roms = std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the public ROM directory");
    let rom = fs::read(Path::new(&roms).join("esp32_rev300_rom.elf")).unwrap();
    assert_eq!(common::sha256_hex(&rom), ROM_SHA);

    let bootloader = &firmware[0x1000..0x4680];
    let stored_digest = &firmware[0x4680..0x46a0];
    assert_eq!(bootloader[2], 3);
    assert_eq!(bootloader[23], 1);
    assert_ne!(common::sha256(bootloader), stored_digest);
    // Diagnostic only. The ESP32 ROM still receives the original merged image.
    let mut historical_header = bootloader.to_vec();
    historical_header[2] = 2;
    assert_eq!(common::sha256(&historical_header), stored_digest);

    let center = run_scene(&firmware, &rom, [80, 60]);
    let offset = run_scene(&firmware, &rom, [40, 30]);
    let delta = [offset[0] - center[0], offset[1] - center[1]];
    assert!((delta[0] - 30.0).abs() < 2.0 && (delta[1] + 40.0).abs() < 2.0, "unexpected guest hotspot movement {delta:?}");
}

fn run_scene(firmware: &[u8], rom: &[u8], [x, y]: [u16; 2]) -> [f64; 2] {
    let mut board = StickT2::new();
    assert!(board.thermal_environment_at(0, [8000, 9000, x, y, 12]));
    let thermal = board.thermal.clone();
    let mut machine = esp32::machine([2, 0, 0, 0, 0, 1], FLASH_BYTES);
    machine.bus.board = Box::new(board);
    machine.bus.attach_board_devices();
    machine.console.capture = true;
    machine.load_rom(rom).unwrap();
    machine.write_flash(0, firmware).unwrap();
    machine.boot_rom();
    machine.max_cycles = EIGHT_SECONDS;

    // Bound execution even if a peripheral stalls or boot fails.
    for _ in 0..256 {
        let stop = machine.run(10_000_000);
        if machine.bus.cycles >= machine.max_cycles || !matches!(stop, esp32::Stop::MaxInsns | esp32::Stop::Halted) { break; }
    }
    assert_eq!(machine.bus.cycles, EIGHT_SECONDS);
    assert!(machine.insns() > 0);
    machine.drain_console();
    let serial = String::from_utf8_lossy(&machine.console.all);
    for error in ["error=2", "MPU ERROR", "StoreProhibited"] {
        assert!(!serial.contains(error), "guest reported {error}");
    }

    let thermal = thermal.lock().unwrap();
    for command in ["W0004=4855", "W0004=4859"] { assert!(thermal.trace.iter().any(|entry| entry == command)); }
    assert!(thermal.commands >= 2 && thermal.packets >= 240, "factory firmware must acquire a complete VoSPI frame");
    drop(thermal);

    let (width, height, pixels, _) = machine.bus.board.display().unwrap();
    assert_eq!((width, height), (135, 240));
    let (mut count, mut sum_x, mut sum_y) = (0, 0, 0);
    for (index, &rgb565) in pixels.iter().enumerate() {
        if rgb565 == 0xf800 {
            count += 1;
            sum_x += index % width as usize;
            sum_y += index / width as usize;
        }
    }
    assert!(count > 100, "guest palette must show a visible hotspot");
    let centroid = [sum_x as f64 / count as f64, sum_y as f64 / count as f64];
    println!("RAW14 center=[{x},{y}] cycles={} instructions={} redPixels={count} centroid={centroid:?}", machine.bus.cycles, machine.insns());
    println!("{}", machine.bus.board.report());
    centroid
}
