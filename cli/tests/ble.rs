#[path = "../../tests/common.rs"]
mod common;
use std::{path::PathBuf, process::Command};

#[test]
#[ignore = "set ESP32SIM_BLE_FIRMWARE_DIR to the Arduino builds; needs the S3/C3/C6 ROM ELFs (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_ble_arduino_examples() {
    let firmware = PathBuf::from(std::env::var_os("ESP32SIM_BLE_FIRMWARE_DIR")
        .expect("set ESP32SIM_BLE_FIRMWARE_DIR to the Arduino builds: CHIP/EXAMPLE/.pio/build/BOARD/firmware.{elf,factory.bin}"));
    // The evidence runner takes one directory: the one the S3 ROM is found in must hold all three.
    let roms = esp_soc::rom::require("esp32s3_rev0_rom.elf").parent().unwrap().to_path_buf();
    for (chip, board, rom) in [("s3", "esp32-s3-devkitc-1", "esp32s3_rev0_rom.elf"),
                              ("c3", "esp32-c3-devkitm-1", "esp32c3_rev3_rom.elf"),
                              ("c6", "esp32-c6-devkitc-1", "esp32c6_rev0_rom.elf")] {
        assert!(roms.join(rom).is_file(), "missing ROM input {}", roms.join(rom).display());
        for example in ["Server", "Notify", "Write", "Scan"] {
            for name in ["firmware.elf", "firmware.factory.bin"] {
                let file = firmware.join(chip).join(example).join(".pio/build").join(board).join(name);
                assert!(file.is_file(), "missing Arduino BLE input {}", file.display());
            }
        }
    }
    let output = std::env::temp_dir().join(format!("esp32sim-ble-{}", std::process::id()));
    let result = Command::new("python3").arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/evidence/ble-s3-c3-c6-2026-10-02/run.py"))
        .args(["--emulator", env!("CARGO_BIN_EXE_esp32sim"), "--firmware"]).arg(firmware)
        .arg("--roms").arg(roms).arg("--output").arg(&output).output().expect("python3 BLE evidence runner");
    assert!(result.status.success(), "Arduino BLE checks failed; logs in {}\n{}\n{}", output.display(),
        String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
}

#[test]
fn ble_script_without_enable_exits_unsuccessfully() {
    let script = std::env::temp_dir().join(format!("esp32sim-ble-invalid-{}.txt", std::process::id()));
    for line in ["0 ble conect", "0 ble connect"] {
        std::fs::write(&script, line).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_esp32sim")).args(["--script", script.to_str().unwrap(), "--max-insns", "1", "--no-dump"])
            .output().unwrap();
        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains(if line.ends_with("conect") { "line 1: expected connect" } else { "line 1: BLE requires --ble" }), "{stderr}");
        assert!(!stderr.contains("ROM loaded"), "{stderr}");
    }
    std::fs::remove_file(script).unwrap();
}

#[test]
fn c6_ble_without_elf_names_the_missing_symbol() {
    let result = Command::new(env!("CARGO_BIN_EXE_esp32sim")).args(["--chip", "c6", "--boot", "app", "--ble", "--max-insns", "1", "--no-dump"])
        .output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("requires ELF symbol esp_bt_controller_init"));
}

#[test]
fn full_ble_mode_is_c3_only_and_excludes_hci() {
    for args in [vec!["--chip", "s3", "--ble", "full"], vec!["--chip", "c6", "--ble", "full"], vec!["--chip", "c3", "--ble", "--ble", "full"], vec!["--chip", "c3", "--ble", "full", "--ble"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_esp32sim")).args(&args).output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(if args.iter().filter(|&&a| a == "--ble").count() == 2 { "mutually exclusive" } else { "requires C3" }));
    }
}

#[test]
fn passive_observer_requires_full_ble() {
    let result = Command::new(env!("CARGO_BIN_EXE_esp32sim-c3")).arg("--ble-observe").output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("requires --ble full"));
}

#[test]
#[ignore = "set ESP32SIM_BLE_SERVER_DIR to the unchanged Arduino 3.3.11 C3 Server build; needs the C3 rev3 ROM (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_full_ble_server_responds_to_active_scan() {
    let build = PathBuf::from(std::env::var_os("ESP32SIM_BLE_SERVER_DIR").expect("set ESP32SIM_BLE_SERVER_DIR to the Arduino C3 Server build"));
    let rom = esp_soc::rom::require("esp32c3_rev3_rom.elf");
    let mut command = Command::new(env!("CARGO_BIN_EXE_esp32sim-c3"));
    command.args(["--boot", "rom", "--rom"]).arg(rom).args(["--ble", "full", "--ble-scan", "--ble-observe",
        "--flash-mb", "4", "--max-seconds", "2", "--no-dump", "--trace-fn", "r_lld_rxdesc_free"]);
    for (flag, suffix) in [("--bootloader", "bootloader.bin"), ("--ptable", "partitions.bin"), ("--app", "bin"), ("--elf", "elf")] {
        let file = build.join(format!("Server.ino.{suffix}"));
        assert!(file.is_file(), "missing Arduino Server input {}", file.display());
        command.arg(flag).arg(file);
    }
    let result = command.output().expect("run C3 Server specimen");
    let console = String::from_utf8_lossy(&result.stdout);
    let trace = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{trace}");
    assert!(console.contains("Characteristic defined! Now you can read it in your phone!"), "{console}");
    assert!(!console.contains("assert") && !trace.contains("[ble-error]"), "{console}\n{trace}");
    assert!(trace.contains("0 exceptions"), "{trace}");
    let packets: Vec<_> = trace.lines().filter(|line| line.starts_with("[ble-air]") || line.starts_with("[ble-central]")).collect();
    assert!(packets.len() >= 45 && packets.len().is_multiple_of(9), "{trace}");
    let time = |line: &str| -> u64 { line.split_whitespace().find_map(|v| v.strip_prefix("hus=")).unwrap().parse().unwrap() };
    let mut previous = None;
    for (index, exchange) in packets.as_chunks::<3>().0.iter().enumerate() {
        for line in exchange { assert!(line.contains(&format!("channel={}", 37 + index % 3)), "{line}"); }
        assert!(exchange[0].contains("type=ADV_IND AdvA=60:55:f9:00:11:24"), "{}", exchange[0]);
        assert!(exchange[1].contains("type=SCAN_REQ"), "{}", exchange[1]);
        assert!(exchange[2].contains("type=SCAN_RSP AdvA=60:55:f9:00:11:24 name=\"BLE Server Example\""), "{}", exchange[2]);
        assert_eq!(time(exchange[1]) - time(exchange[0]), 2 * (8 * (35 + 8) + 150));
        assert_eq!(time(exchange[2]) - time(exchange[1]), 2 * (8 * (14 + 8) + 150));
        if index.is_multiple_of(3) {
            let current = time(exchange[0]);
            if let Some(prior) = previous { assert!((120_000..=140_000).contains(&(current - prior))); }
            previous = Some(current);
        }
    }
    // Both the ROM veneer and its body can be traced. At least one free per request
    // proves repeated guest consumption, beyond simply printing configured SCAN_RSP.
    assert!(trace.matches("r_lld_rxdesc_free(a0=").count() >= packets.len() / 3, "{trace}");
}

#[test]
#[ignore = "set ESP32SIM_BLE_SERVER_DIR to the unchanged Arduino 3.3.11 C3 Server build; needs the C3 rev3 ROM (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_full_ble_server_connects_and_times_out() {
    let build = PathBuf::from(std::env::var_os("ESP32SIM_BLE_SERVER_DIR").expect("set ESP32SIM_BLE_SERVER_DIR to the Arduino C3 Server build"));
    let rom = esp_soc::rom::require("esp32c3_rev3_rom.elf");
    let mut command = Command::new(env!("CARGO_BIN_EXE_esp32sim-c3"));
    command.args(["--boot", "rom", "--rom"]).arg(rom).args(["--ble", "full", "--ble-observe", "--flash-mb", "4", "--max-seconds", "6", "--no-dump",
        "--trace-fn", "r_llc_disconnect_end", "--trace-fn", "r_lld_con_tx_isr"]);
    for (flag, suffix) in [("--bootloader", "bootloader.bin"), ("--ptable", "partitions.bin"), ("--app", "bin"), ("--elf", "elf")] {
        let file = build.join(format!("Server.ino.{suffix}"));
        assert!(file.is_file(), "missing Arduino Server input {}", file.display());
        command.arg(flag).arg(file);
    }
    let script = common::tmp("ble-timeout.script");
    std::fs::write(&script, "0 ble connect\n2.6 ble central-stop\n").unwrap();
    command.arg("--script").arg(&script);
    let result = command.output().expect("run C3 Server specimen");
    std::fs::remove_file(script).unwrap();
    let console = String::from_utf8_lossy(&result.stdout);
    let trace = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{trace}");
    assert!(!console.contains("assert") && !trace.contains("[ble-error]"), "{console}\n{trace}");
    assert!(trace.contains("0 exceptions"), "{trace}");
    assert!(trace.contains("r_lld_con_tx_isr(a0=0x1"), "{trace}");
    let number = |line: &str, key: &str| -> u64 {
        line.split_whitespace().find_map(|s| s.strip_prefix(key)).unwrap().parse().unwrap()
    };
    let connect = trace.lines().find(|l| l.contains("type=CONNECT_IND")).unwrap();
    let events: Vec<_> = trace.lines().filter(|l| l.starts_with("[ble-connection]")).collect();
    let anchor = number(events[0], "anchor_hus=");
    // CONNECT_IND airtime 352 us, then 1.25 ms + WinOffset(6)*1.25 ms.
    assert_eq!(anchor - number(connect, "hus="), 2 * (352 + 8750));
    for (i, event) in events.iter().enumerate() {
        assert_eq!(number(event, "channel="), ((i as u64 + 1) * 5) % 37);
        assert_eq!(number(event, "anchor_hus="), anchor + i as u64 * 60_000);
    }
    let data: Vec<_> = trace.lines().filter(|l| l.contains("type=DATA")).collect();
    assert!(data.len() >= 140);
    let mut central_sn = 0;
    let mut peripheral_sn = 0;
    let mut last_tx = 0;
    for pair in data.as_chunks::<2>().0 {
        assert!(pair[0].starts_with("[ble-central]") && pair[1].starts_with("[ble-air]"));
        assert_eq!(number(pair[0], "channel="), number(pair[1], "channel="));
        assert_eq!(number(pair[1], "hus=") - number(pair[0], "hus="), 2 * (80 + 150 + 8 * u8::from_str_radix(&pair[0].split("pdu=").nth(1).unwrap()[2..4], 16).unwrap() as u64));
        let header = |line: &str| u8::from_str_radix(&line.split("pdu=").nth(1).unwrap()[..2], 16).unwrap();
        let a = header(pair[0]);
        let b = header(pair[1]);
        assert_eq!((a >> 3) & 1, central_sn);
        assert_eq!((a >> 2) & 1, peripheral_sn);
        assert_eq!((b >> 3) & 1, peripheral_sn);
        assert_eq!((b >> 2) & 1, central_sn ^ 1);
        central_sn ^= 1;
        peripheral_sn ^= 1;
        last_tx = number(pair[0], "hus=");
    }
    assert!(last_tx - anchor >= 4_000_000);
    assert!(trace.lines().any(|l| l.contains("r_llc_disconnect_end(a0=0x1") && l.contains("a2=0x8")), "{trace}");
    let resumed = trace.lines().find(|l| l.contains("[ble-state] disconnected advertising_resumed")).unwrap();
    assert!((3_900_000..=4_200_000).contains(&(number(resumed, "hus=") - last_tx)));
    assert!(trace[trace.find(resumed).unwrap()..].contains("type=ADV_IND"));
}

#[test]
#[ignore = "set ESP32SIM_BLE_SERVER_DIR to the unchanged Arduino 3.3.11 C3 Server build; needs the C3 rev3 ROM (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_full_ble_server_reads_gatt_from_script() {
    let build = PathBuf::from(std::env::var_os("ESP32SIM_BLE_SERVER_DIR").expect("set ESP32SIM_BLE_SERVER_DIR to the Arduino C3 Server build"));
    let rom = esp_soc::rom::require("esp32c3_rev3_rom.elf");
    let mut command = Command::new(env!("CARGO_BIN_EXE_esp32sim-c3"));
    command.args(["--boot", "rom", "--rom"]).arg(rom).args(["--ble", "full",
 "--ble-observe", "--flash-mb", "4", "--max-seconds", "3", "--no-dump",
        "--trace-fn", "r_llc_disconnect_end", "--trace-fn", "r_lld_con_tx_isr"]);
    for (flag, suffix) in [("--bootloader", "bootloader.bin"), ("--ptable", "partitions.bin"), ("--app", "bin"), ("--elf", "elf")] {
        let file = build.join(format!("Server.ino.{suffix}"));
        assert!(file.is_file(), "missing Arduino Server input {}", file.display());
        command.arg(flag).arg(file);
    }
    let script = std::env::temp_dir().join(format!("esp32sim-full-ble-script-{}.txt", std::process::id()));
    std::fs::write(&script, "0.5 ble connect\n0.7 ble read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8\n").unwrap();
    command.arg("--script").arg(&script);
    let result = command.output().expect("run C3 Server specimen");
    std::fs::remove_file(script).unwrap();
    let console = String::from_utf8_lossy(&result.stdout);
    let trace = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{trace}");
    assert!(!console.contains("assert") && !trace.contains("[ble-error]"), "{console}\n{trace}");
    assert!(trace.contains("0 exceptions"), "{trace}");
    assert!(trace.contains("[ble-att] value=48656c6c6f20576f726c642073617973204e65696c text=\"Hello World says Neil\""), "{trace}");
    assert!(!trace.contains("[ble-state] disconnected"), "{trace}");
    // Assert the packets came from the guest's ATT service and declaration responses.
    assert!(trace.contains("05000400070e001000"), "{trace}");
    assert!(trace.contains("1700040009150f000a1000a8261b3607eaf5b78846e1363e48b5be"), "{trace}");
}

fn reconnect_server(build: &std::path::Path, arduino: bool, read_first: bool, normal: bool) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let rom = esp_soc::rom::find("esp32c3_rev3_rom.elf").unwrap_or_else(|| root.join("web/wasm/fw/esp32c3_rev3_rom.elf"));
    assert!(rom.is_file(), "fetch the C3 ROM, set ESP32SIM_ROM_DIR or install ESP-IDF");
    let mut command = Command::new(env!("CARGO_BIN_EXE_esp32sim-c3"));
    command.args(["--boot", "rom", "--rom"]).arg(rom).args(["--ble", "full", "--ble-scan",
        "--ble-observe", "--flash-mb", "4", "--max-seconds", "8", "--no-dump", "--trace-fn", "r_llc_disconnect_end"]);
    let files = if arduino {
        vec![("--bootloader", "Server.ino.bootloader.bin"), ("--ptable", "Server.ino.partitions.bin"),
            ("--app", "Server.ino.bin"), ("--elf", "Server.ino.elf")]
    } else {
        vec![("--bootloader", "c3-ble-bootloader.bin"), ("--ptable", "c3-ble-ptable.bin"), ("--app", "c3-ble-server.bin")]
    };
    for (flag, file) in files {
        assert!(build.join(file).is_file(), "missing BLE firmware {}", build.join(file).display());
        command.arg(flag).arg(build.join(file));
    }
    let read = "ble read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8";
    let mut script = "0.5 ble connect\n".to_string();
    if read_first { script += &format!("0.7 {read}\n"); }
    let stop = if normal { "disconnect" } else { "central-stop" };
    script += &format!("3.1 ble {stop}\n5.7 ble connect\n5.9 {read}\n");
    if normal { script += "7.7 ble disconnect\n"; }
    let path = std::env::temp_dir().join(format!("ble-reconnect-{}-{arduino}-{read_first}-{normal}.txt", std::process::id()));
    std::fs::write(&path, script).unwrap();
    let result = command.arg("--script").arg(&path).output().unwrap();
    std::fs::remove_file(path).unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{stdout}\n{stderr}");
    assert!(!stdout.contains("assert") && !stdout.lines().any(|line| line.starts_with("E (")) && !stdout.contains("panic")
        && !stderr.contains("[ble-error]") && !stderr.contains("[ble-observer] dropped"), "{stdout}\n{stderr}");
    assert_eq!(stderr.matches("[ble-att] value=48656c6c6f20576f726c642073617973204e65696c").count(), if read_first { 2 } else { 1 }, "{stderr}");
    assert_eq!(stderr.matches("[ble-state] connected").count(), 2, "{stderr}");
    assert_eq!(stderr.matches("[ble-state] disconnected advertising_resumed").count(), if normal { 2 } else { 1 }, "{stderr}");
    let (before, after) = stderr.split_once("[ble-state] disconnected advertising_resumed").expect("guest resumes advertising");
    for kind in ["type=ADV_IND", "type=SCAN_RSP"] {
        let packets = |text: &str| -> Vec<String> {
            text.lines().filter(|line| line.contains(kind)).map(|line| line.split("pdu=").nth(1).unwrap().to_string()).collect()
        };
        let first = packets(before); let second = packets(after);
        assert!(!first.is_empty() && !second.is_empty(), "missing {kind}\n{stderr}");
        assert!(first.iter().chain(&second).all(|p| p == &first[0]), "{kind} changed across timeout");
    }
    assert!(stderr.contains("name=\"BLE Server Example\""));
    let reason = if normal { "a2=0x13" } else { "a2=0x8" };
    assert!(stderr.lines().any(|l| l.contains("r_llc_disconnect_end(a0=0x1") && l.contains(reason)), "{stderr}");
    assert!(!stderr.contains("still pending"), "{stderr}");
    if !arduino {
        let name = format!("ble-server-c3-{read_first}-{normal}");
        common::expect_text(&format!("{name}.console.txt"), &stdout);
        let events = stderr.lines().find(|l| l.starts_with("[emu] stop:")).unwrap().rsplit_once("); ").unwrap().1;
        let report: String = std::iter::once(events).chain(stderr.lines().filter(|l| l.starts_with("  core")))
            .map(|l| format!("{l}\n")).collect();
        common::expect_text(&format!("{name}.report.txt"), &report);
    }
}

#[test]
#[ignore = "requires C3 rev3 ROM; tools/fetch-rom-elfs.sh web/wasm/fw or set ESP32SIM_ROM_DIR"]
fn full_ble_server_reconnects_in_ci() {
    let build = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/wasm/fw/public");
    for read_first in [false, true] { reconnect_server(&build, false, read_first, false); }
    reconnect_server(&build, false, true, true);
}

#[test]
#[ignore = "set ESP32SIM_BLE_SERVER_DIR to the unchanged Arduino 3.3.11 C3 Server build; needs the C3 rev3 ROM (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn external_full_ble_server_reconnects_without_uuid_corruption() {
    let build = PathBuf::from(std::env::var_os("ESP32SIM_BLE_SERVER_DIR").expect("set ESP32SIM_BLE_SERVER_DIR to the unchanged Arduino C3 Server build"));
    for read_first in [false, true] { reconnect_server(&build, true, read_first, false); }
    reconnect_server(&build, true, true, true);
}

#[test]
fn full_ble_accepts_c3_alias_and_rejects_hci_script_without_panic() {
    let script = std::env::temp_dir().join(format!("esp32sim-full-ble-{}.txt", std::process::id()));
    std::fs::write(&script, "0 ble discover\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_esp32sim")).args(["--chip", "esp32c3", "--ble", "full", "--script"])
        .arg(&script).output().unwrap();
    std::fs::remove_file(script).unwrap();
    assert_eq!(result.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("line 1: command requires HCI --ble"), "{stderr}");
    assert!(!stderr.contains("panicked") && !stderr.contains("ROM loaded"), "{stderr}");
}

#[test]
fn removed_central_flags_are_rejected_without_panic() {
    for args in [vec!["--ble-scan"], vec!["--ble-connect"],
        vec!["--ble-read-uuid", "bad", "bad"], vec!["--ble", "full", "--ble-stop-after-ms", "1"],
        vec!["--ble", "full", "--ble-connect", "--ble-stop-after-ms", "-1"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_esp32sim-c3")).args(args).output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("panicked"));
    }
}

#[test]
#[ignore = "needs the ESP32-C3 mask ROM ELF fetched by CI; set ESP32SIM_ROM_DIR"]
fn full_ble_script_connect_read_and_stop() {
    let rom = common::rom("esp32c3_rev3");
    let script = common::tmp("ble-script-stop.txt");
    std::fs::write(&script, "0 ble connect\n0 ble read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8\n0.6 ble central-stop\n").unwrap();
    let r = common::run(env!("CARGO_BIN_EXE_esp32sim-c3"), &["--boot", "rom", "--rom", rom.to_str().unwrap(),
        "--bootloader", "web/wasm/fw/public/c3-ble-bootloader.bin", "--ptable", "web/wasm/fw/public/c3-ble-ptable.bin",
        "--app", "web/wasm/fw/public/c3-ble-server.bin", "--ble", "full", "--ble-observe", "--script", script.to_str().unwrap(), "--max-seconds", "4", "--no-dump"]);
    std::fs::remove_file(script).unwrap();
    assert!(r.stderr.contains("[ble-att] value=48656c6c6f20576f726c642073617973204e65696c"), "{}", r.stderr);
    assert!(r.stdout.contains("server: disconnected reason=520"), "{}", r.stdout);
    assert!(r.stderr.contains("disconnected advertising_resumed"));
    assert!(!r.stderr.contains("[ble-error]") && !r.stderr.contains("still pending"));
    let time = |line: &str, key: &str| -> u64 { line.split_whitespace().find_map(|w| w.strip_prefix(key)).unwrap().parse().unwrap() };
    let stopped = r.stderr.lines().find(|l| l.starts_with("[ble-central] stopped")).unwrap();
    assert!((1_200_000..1_260_000).contains(&time(stopped, "hus=")));
}
