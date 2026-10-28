//! esp32sim-p4 runs the revision the bootloader was built for, and refuses an image built for the
//! other one before it runs, naming the file.
#[path = "../../tests/common.rs"]
mod common;
use common::*;

/// `name` from web/wasm/fw/public with its header's revision range set as a rev >= 3 build sets it.
fn rev3_copy(name: &str) -> std::path::PathBuf {
    let mut d = std::fs::read(root().join("web/wasm/fw/public").join(name)).unwrap();
    d[15..17].copy_from_slice(&300u16.to_le_bytes());
    d[17..19].copy_from_slice(&399u16.to_le_bytes());
    let p = tmp(name);
    std::fs::write(&p, d).unwrap();
    p
}

/// A rev >= 3 app behind a rev < 3 bootloader is refused by the rev < 3 model; a rev >= 3
/// bootloader brings up the rev >= 3 model, which refuses the rev < 3 app.
#[test]
fn a_bootloader_and_app_for_different_revisions_stop_the_run_with_the_file_named() {
    let fw = "web/wasm/fw/public";
    let (bl, app) = (format!("{fw}/p4-hello-bootloader.bin"), format!("{fw}/p4-hello_world.bin"));
    let lt3 = "was built for ESP32-P4 rev ≥ v3.0 (min_chip_rev_full=300); this model is rev < 3 (v1.0). \
        Rebuild with CONFIG_ESP32P4_SELECTS_REV_LESS_V3=y, or run a bootloader and app both built for rev ≥ 3.";
    let ge3 = "was built for ESP32-P4 rev < v3.0 (max_chip_rev_full=199); this model is rev ≥ 3 (v3.1). \
        Rebuild without CONFIG_ESP32P4_SELECTS_REV_LESS_V3, or run a bootloader and app both built for rev < 3.";
    for (flag, name) in [("--app", "p4-hello_world.bin"), ("--bootloader", "p4-hello-bootloader.bin")] {
        let patched = rev3_copy(name);
        let mut args = vec!["--boot", "app", "--flash-mb", "2", "--bootloader", &bl, "--ptable", "web/wasm/fw/public/p4-hello-ptable.bin", "--app", &app, "--max-seconds", "1"];
        let at = args.iter().position(|a| *a == flag).unwrap() + 1;
        args[at] = patched.to_str().unwrap();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_esp32sim-p4")).args(&args).current_dir(root()).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{err}");
        let want = if flag == "--app" { format!("[emu] {} {lt3}", patched.display()) } else { format!("[emu] {app} {ge3}") };
        assert!(err.contains(&want), "{err}");
        assert!(!err.contains("[emu] stop:"), "the run started:\n{err}");
    }
}

/// A rev >= 3 bootloader placed with `--flash-at 0x2000=` picks the rev >= 3 model as `--bootloader`
/// does, which then refuses the rev < 3 app.
#[test]
fn a_rev3_bootloader_placed_with_flash_at_picks_rev3() {
    let app = "web/wasm/fw/public/p4-hello_world.bin";
    let bl = rev3_copy("p4-hello-bootloader.bin");
    let at = format!("0x2000={}", bl.display());
    let args = ["--boot", "app", "--flash-mb", "2", "--flash-at", &at, "--ptable", "web/wasm/fw/public/p4-hello-ptable.bin", "--app", app, "--max-seconds", "1"];
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_esp32sim-p4")).args(args).current_dir(root()).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains(&format!("[emu] {app} was built for ESP32-P4 rev < v3.0")) && err.contains("this model is rev ≥ 3"), "{err}");
}
