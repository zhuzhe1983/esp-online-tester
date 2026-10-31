use std::{fs, path::Path, process::Command};

#[test]
fn classic_board_usage_lists_camera() {
    let output = Command::new(env!("CARGO_BIN_EXE_esp32sim"))
        .args(["--chip", "esp32", "--board", "unknown"])
        .output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr).unwrap().contains("ai-thinker-cam"));
}

#[test]
#[ignore = "needs esp32_rev300_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn classic_camera_image_captures_nonzero_frame() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixture = root.join("esp32/tests/fixtures/classic-camera");
    let rom = esp_soc::rom::require("esp32_rev300_rom.elf");
    let image = std::env::temp_dir().join(format!("esp32sim-camera-{}.ppm", std::process::id()));
    let mut ppm = b"P6\n160 120\n255\n".to_vec();
    for p in 0..160 * 120 {
        ppm.extend_from_slice(&[(p % 256) as u8, ((p / 160) % 256) as u8, ((p * 7) % 256) as u8]);
    }
    fs::write(&image, ppm).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_esp32sim"))
        .args(["--chip", "esp32", "--board", "ai-thinker-cam", "--boot", "rom", "--rom"])
        .arg(&rom)
        .arg("--bootloader").arg(fixture.join("bootloader/bootloader.bin"))
        .arg("--ptable").arg(fixture.join("partition_table/partition-table.bin"))
        .arg("--app").arg(fixture.join("classic_camera.bin"))
        .arg("--cam-image").arg(&image)
        .args(["--max-seconds", "3", "--max-insns", "300000000", "--no-dump"])
        .output().unwrap();
    fs::remove_file(image).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stdout}\n{stderr}");
    // Independent Python RGB565 oracle: examples/classic-camera/expected-frame.py.
    let hash = "c67913e0afba1703e8c9c47e02b1ae2ea62e47596a8440d83f1ebba14aed6cf1";
    for n in 0..2 {
        assert!(stdout.contains(&format!("C8 frame={n} width=160 height=120 len=38400 RGB565 sha256={hash}")), "{stdout}\n{stderr}");
    }
    assert!(stdout.contains("C8 PASS"), "{stdout}\n{stderr}");
}

#[test]
#[ignore = "ESP32SIM_ROM_DIR must contain esp32_rev300_rom.elf"]
fn classic_camera_jpeg_file_reaches_framebuffer_verbatim() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixture = root.join("esp32/tests/fixtures/classic-camera");
    let roms = std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR");
    let run = |image: &Path| Command::new(env!("CARGO_BIN_EXE_esp32sim"))
        .args(["--chip", "esp32", "--board", "ai-thinker-cam", "--boot", "rom", "--rom"])
        .arg(Path::new(&roms).join("esp32_rev300_rom.elf"))
        .arg("--bootloader").arg(fixture.join("bootloader/bootloader.bin"))
        .arg("--ptable").arg(fixture.join("partition_table/partition-table.bin"))
        .arg("--app").arg(fixture.join("jpeg.bin"))
        .arg("--cam-image").arg(image)
        .args(["--max-seconds", "2", "--no-dump"])
        .output().unwrap();
    // The fixture's first phase only: two VGA frames before its first host marker.
    let output = run(&fixture.join("jpeg/vga-a.jpg"));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stderr.contains("camera JPEG") && stderr.contains("(640x480, 6218 bytes)"), "{stderr}");
    // Independent oracle: examples/classic-camera/make-jpeg.py.
    let hash = "f602a49340beba9cbf3c7b8ee37d34db066baf7b7a9f545e52742eeaca576998";
    for n in 0..2 {
        assert!(stdout.contains(&format!("C10 vga-a frame={n} width=640 height=480 len=6218 JPEG DRAM sha256={hash}")), "{stdout}\n{stderr}");
    }
    // A truncated file is a usage error before the machine starts.
    let cut = std::env::temp_dir().join(format!("esp32sim-camera-{}.jpg", std::process::id()));
    let data = fs::read(fixture.join("jpeg/vga-a.jpg")).unwrap();
    fs::write(&cut, &data[..data.len() - 1]).unwrap();
    let output = run(&cut);
    fs::remove_file(&cut).unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!("[emu] {}: jpeg: malformed", cut.display())), "{stderr}");
    assert!(!String::from_utf8(output.stdout).unwrap().contains("C10"));
}
