use std::{path::PathBuf, process::Command};

#[test]
#[ignore = "needs the classic ECO3 mask ROM ELF (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn classic_psram_cli_reaches_guest_allocator() {
    let rom = esp_soc::rom::require("esp32_rev300_rom.elf");
    let fw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/wasm/fw/public");
    let run = |size: Option<&str>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_esp32sim"));
        cmd.args(["--chip", "esp32", "--boot", "rom", "--no-dump", "--max-insns", "30000000"])
            .arg("--rom").arg(&rom);
        for (flag, name) in [("--bootloader", "bootloader"), ("--ptable", "ptable"), ("--app", "app")] {
            cmd.arg(flag).arg(fw.join(format!("classic-psram-idf-{name}.bin")));
        }
        if let Some(size) = size { cmd.args(["--psram-mb", size]); }
        let output = cmd.output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
    };
    let present = run(Some("4"));
    assert!(present.contains("P4 result=pass"), "{present}");
    for size in [None, Some("0")] {
        let absent = run(size);
        assert!(absent.contains("PSRAM ID read error: 0xffffffff"), "{absent}");
        assert!(!absent.contains("P4 result=pass"));
    }
}

#[test]
fn classic_psram_cli_rejects_unsupported_sizes_without_panicking() {
    for size in ["1", "2", "8", "18446744073709551615"] {
        let output = Command::new(env!("CARGO_BIN_EXE_esp32sim"))
            .args(["--chip", "esp32", "--psram-mb", size]).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("classic ESP32 supports only 0 or 4 MiB"));
    }
}
