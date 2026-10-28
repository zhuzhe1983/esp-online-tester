// The classic ESP32 has a station link (#209); `--wifi` must attach it as on the other chips instead of
// being refused as unavailable. The run attaches the access point and then exits on the missing firmware
// (with a ROM dir it executes one instruction); either way the AP line has been printed.
#[test]
fn classic_esp32_accepts_the_wifi_option() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esp32sim"))
        .args(["--chip", "esp32", "--wifi", "ssid=demo,chan=6", "--net", "none", "--max-insns", "1"]).output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("not available on the classic ESP32"), "{stderr}");
    assert!(stderr.contains("virtual AP 'demo'"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    // The other gates stay.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esp32sim")).args(["--chip", "esp32", "--psram-mb", "2"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr).unwrap().contains("--psram-mb: classic ESP32 supports only 0 or 4 MiB"));
}
