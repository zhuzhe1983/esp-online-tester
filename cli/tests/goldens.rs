//! Golden-output tests: the emulator must produce byte-identical console text, audio and
//! instruction counts for the committed demo firmware. This is the regression bar every
//! refactor and every speed change is held to (`docs/decisions.md`, "Performance").
//!
//! They need the ESP32-S3 mask ROM ELF, which ships with ESP-IDF and is not checked in, so they
//! are `#[ignore]`d: run `cargo test --release --workspace -- --include-ignored` (CI does; see
//! `tests/README.md`). Regenerate after an intentional change with `UPDATE_GOLDENS=1`.
#[path = "../../tests/common.rs"]
mod common;
#[path = "../../tests/sd_image.rs"]
mod sd_image;
use common::*;

const BIN: &str = env!("CARGO_BIN_EXE_esp32sim");
const BIN_C3: &str = env!("CARGO_BIN_EXE_esp32sim-c3");
const BIN_C6: &str = env!("CARGO_BIN_EXE_esp32sim-c6");
const BIN_P4: &str = env!("CARGO_BIN_EXE_esp32sim-p4");
const FW: &str = "web/wasm/fw/public";

fn atech(extra: &[&str]) -> (Run, Vec<u8>) {
    let wav = tmp("atech.wav"); let rom = rom("esp32s3_rev0");
    let mut args: Vec<String> = ["--rom", rom.to_str().unwrap(), "--board", "atech14", "--boot", "rom", "--no-dump",
        "--bootloader", &format!("{FW}/atech-bootloader.bin"), "--ptable", &format!("{FW}/atech-ptable.bin"), "--app", &format!("{FW}/atech-firmware.bin"),
        "--script", &format!("{FW}/atech-script1.txt"), "--wav", wav.to_str().unwrap(), "--max-seconds", "5"].iter().map(|s| s.to_string()).collect();
    args.extend(extra.iter().map(|s| s.to_string()));
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r = run(BIN, &args);
    let data = std::fs::read(&wav).expect("wav written");
    (r, data)
}

/// The Pocket Synth scenario: buttons, encoder, a serial command, the SID voice on I2S.
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn atech_script1() {
    let (r, wav) = atech(&[]);
    expect_text("atech-script1.console.txt", &r.stdout);
    expect_sha("atech-script1.wav.sha256", &wav);
    expect_u64("atech-script1.insns", r.insns);
}

/// `--no-jit` is the oracle: the block interpreter and the JIT must agree bit for bit.
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn atech_script1_no_jit() {
    let (r, wav) = atech(&["--no-jit"]);
    expect_text("atech-script1.console.txt", &r.stdout);
    expect_sha("atech-script1.wav.sha256", &wav);
    expect_u64("atech-script1.insns", r.insns);
}

/// The C64 SID jukebox (cRSID): a 6502 + SID emulated inside the emulated S3, 3 s of Commando.
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn atech_sid_jukebox() {
    let wav = tmp("sid.wav"); let rom = rom("esp32s3_rev0");
    let r = run(BIN, &["--rom", rom.to_str().unwrap(), "--board", "atech14", "--boot", "rom", "--no-dump",
        "--bootloader", &format!("{FW}/atech-bootloader.bin"), "--ptable", &format!("{FW}/atech-ptable.bin"), "--app", &format!("{FW}/atech-firmware.bin"),
        "--script", &format!("{FW}/atech-sid.txt"), "--wav", wav.to_str().unwrap(), "--max-seconds", "6"]);
    expect_text("atech-sid.console.txt", &r.stdout);
    expect_sha("atech-sid.wav.sha256", &std::fs::read(&wav).unwrap());
    expect_u64("atech-sid.insns", r.insns);
}

/// The Touch-LCD-4B energy panel in demo mode: PSRAM, LCD_CAM RGB frames, GT911 touch over I2C,
/// the ES8311 codec on I2S, swipes and a play tap from the script.
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn panel_sid() {
    let wav = tmp("panel.wav"); let rom = rom("esp32s3_rev0");
    let r = run(BIN, &["--rom", rom.to_str().unwrap(), "--board", "waveshare-lcd4b", "--boot", "rom", "--no-dump", "--flash-mb", "16", "--psram-mb", "8", "--console", "usb",
        "--bootloader", &format!("{FW}/panel-bootloader.bin"), "--ptable", &format!("{FW}/panel-ptable.bin"), "--app", &format!("{FW}/panel-demo.bin"),
        "--flash-at", &format!("0x610000={FW}/energydata.json"),
        "--script", &format!("{FW}/panel-sid.txt"), "--wav", wav.to_str().unwrap(), "--max-seconds", "7"]);
    expect_text("panel-sid.console.txt", &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("  core")))
        .map(|l| format!("{l}\n")).collect();
    expect_text("panel-sid.report.txt", &report);
    expect_sha("panel-sid.wav.sha256", &std::fs::read(&wav).unwrap());
    expect_u64("panel-sid.insns", r.insns);
}

/// Observers attached must not change the run (the block-path ones run at full speed), and
/// each must produce its report.
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn atech_script1_with_observers() {
    let (cov, vcd) = (tmp("cov.txt"), tmp("atech.vcd"));
    let (r, wav) = atech(&["--profile-blocks", "--coverage-file", cov.to_str().unwrap(), "--irq-latency", "--vcd", vcd.to_str().unwrap()]);
    expect_text("atech-script1.console.txt", &r.stdout);
    expect_sha("atech-script1.wav.sha256", &wav);
    expect_u64("atech-script1.insns", r.insns);
    for tag in ["[profile-blocks] top", "[coverage] ", "[irq-latency] per core", "[vcd] wrote"] { assert!(r.stderr.contains(tag), "missing {} in:\n{}", tag, r.stderr); }
    assert!(r.stderr.contains("core0 int9"), "the systimer line should show up in the latency table");
    let v = std::fs::read_to_string(&vcd).unwrap();
    assert!(v.starts_with("$timescale 1ps $end") && v.contains("gpio2 $end") && v.contains("core0_int9 $end"), "vcd header");
    let c = std::fs::read_to_string(&cov).unwrap();
    assert!(c.lines().count() > 5000, "coverage rows: {}", c.lines().count());
}

/// Stock ESP-IDF hello_world from the mask ROM through the bootloader into app_main, on UART0.
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn hello_world_s3() {
    let rom = rom("esp32s3_rev0");
    let r = run(BIN, &["--rom", rom.to_str().unwrap(), "--board", "none", "--boot", "rom", "--no-dump", "--console", "uart0",
        "--bootloader", &format!("{FW}/hello-bootloader.bin"), "--ptable", &format!("{FW}/hello-ptable.bin"), "--app", &format!("{FW}/hello_world.bin"), "--max-seconds", "3"]);
    assert!(r.stdout.contains("Hello world!"), "app_main never printed:\n{}", r.stdout);
    expect_text("hello-s3.console.txt", &r.stdout);
    expect_u64("hello-s3.insns", r.insns);
}

/// Through the RTC watchdog reset esp_restart() arms at the end of the countdown and back up
/// from the ROM: the chip-reset path (what survives, what the ROM reports).
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn hello_world_s3_reboot() {
    let rom = rom("esp32s3_rev0");
    let r = run(BIN, &["--rom", rom.to_str().unwrap(), "--board", "none", "--boot", "rom", "--no-dump", "--console", "uart0",
        "--bootloader", &format!("{FW}/hello-bootloader.bin"), "--ptable", &format!("{FW}/hello-ptable.bin"), "--app", &format!("{FW}/hello_world.bin"), "--max-seconds", "12"]);
    assert!(r.stderr.contains("[emu] chip reset at t="), "no reset seen:\n{}", r.stderr);
    assert!(r.stdout.matches("Hello world!").count() >= 2, "the app did not come back after the reset:\n{}", r.stdout);
    expect_text("hello-s3-reboot.console.txt", &r.stdout);
    expect_u64("hello-s3-reboot.insns", r.insns);
}

/// The block profile attributes time to symbols (the ROM's here), and the per-instruction
/// `--profile` (slow path, idle cores stepping) still agrees on the hottest function.
#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn hello_world_s3_profiles() {
    let rom = rom("esp32s3_rev0");
    let base = ["--rom", rom.to_str().unwrap(), "--board", "none", "--boot", "rom", "--no-dump", "--console", "none",
        "--bootloader", &format!("{FW}/hello-bootloader.bin"), "--ptable", &format!("{FW}/hello-ptable.bin"), "--app", &format!("{FW}/hello_world.bin"), "--max-seconds", "1"];
    let mut a = base.to_vec(); a.push("--profile-blocks");
    let r = run(BIN, &a);
    let line = r.stderr.lines().skip_while(|l| !l.starts_with("[profile-blocks]")).nth(1).unwrap_or("");
    assert!(line.contains("ets_delay_us"), "hottest function should be the ROM's delay loop, got: {:?}", line);
    let mut b = base.to_vec(); b.push("--profile");
    let r = run(BIN, &b);
    assert!(r.stderr.contains("[profile] top 12 pcs"), "{}", r.stderr);
}

/// hello_world from the C3 mask ROM, with the MAC / reset cause / straps of the real module the
/// boot log was compared against (`hw/c3-hello-world-real.txt`, `docs/esp32c3.md`).
#[test] #[ignore = "needs the ESP32-C3 mask ROM ELF"]
fn hello_world_c3() {
    let rom = rom("esp32c3_rev3");
    let r = run(BIN_C3, &["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4",
        "--mac", "3c:84:27:b6:a7:1c", "--reset-cause", "0x15", "--strap", "0xd",
        "--bootloader", &format!("{FW}/c3-hello-bootloader.bin"), "--ptable", &format!("{FW}/c3-hello-ptable.bin"), "--app", &format!("{FW}/c3-hello_world.bin"), "--max-seconds", "3"]);
    assert!(r.stdout.contains("Hello world!"), "app_main never printed:\n{}", r.stdout);
    expect_text("hello-c3.console.txt", &r.stdout);
    expect_u64("hello-c3.insns", r.insns);
    // the same run through the one binary
    let r2 = run(BIN, &["--chip", "c3", "--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4",
        "--mac", "3c:84:27:b6:a7:1c", "--reset-cause", "0x15", "--strap", "0xd",
        "--bootloader", &format!("{FW}/c3-hello-bootloader.bin"), "--ptable", &format!("{FW}/c3-hello-ptable.bin"), "--app", &format!("{FW}/c3-hello_world.bin"), "--max-seconds", "3"]);
    assert_eq!(r.stdout, r2.stdout); assert_eq!(r.insns, r2.insns);
}

/// The C3's esp_restart() path: a software CPU reset, back through the ROM with the right cause.
/// In this unmodeled RV32 path the reported count includes idle cycles and survives reset.
/// A reset charges only its executed partial round, so the 12 s count is 12 * 160 MHz;
/// charging a full reset round previously added device time without matching core work.
#[test] #[ignore = "needs the ESP32-C3 mask ROM ELF"]
fn hello_world_c3_reboot() {
    let rom = rom("esp32c3_rev3");
    let r = run(BIN_C3, &["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4",
        "--mac", "3c:84:27:b6:a7:1c", "--reset-cause", "0x15", "--strap", "0xd",
        "--bootloader", &format!("{FW}/c3-hello-bootloader.bin"), "--ptable", &format!("{FW}/c3-hello-ptable.bin"), "--app", &format!("{FW}/c3-hello_world.bin"), "--max-seconds", "12"]);
    assert!(r.stderr.contains("[emu] chip reset at t="), "no reset seen:\n{}", r.stderr);
    assert!(r.stdout.matches("Hello world!").count() >= 2, "the app did not come back after the reset:\n{}", r.stdout);
    expect_text("hello-c3-reboot.console.txt", &r.stdout);
    expect_u64("hello-c3-reboot.insns", r.insns);
}

/// hello_world from the C6 mask ROM, with the MAC / reset cause / straps of the Waveshare
/// ESP32-C6-LCD-1.47 the boot log was compared against (`hw/c6-hello-world-real.txt`,
/// `docs/esp32c6.md`).
#[test] #[ignore = "needs the ESP32-C6 mask ROM ELF"]
fn hello_world_c6() {
    let rom = rom("esp32c6_rev0");
    let r = run(BIN_C6, &["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4",
        "--mac", "dc:1e:d5:6e:8c:dc", "--reset-cause", "0x15", "--strap", "0x6e",
        "--bootloader", &format!("{FW}/c6-hello-bootloader.bin"), "--ptable", &format!("{FW}/c6-hello-ptable.bin"), "--app", &format!("{FW}/c6-hello_world.bin"), "--max-seconds", "3"]);
    assert!(r.stdout.contains("Hello world!"), "app_main never printed:\n{}", r.stdout);
    expect_text("hello-c6.console.txt", &r.stdout);
    expect_u64("hello-c6.insns", r.insns);
    // the same run through the one binary
    let r2 = run(BIN, &["--chip", "c6", "--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4",
        "--mac", "dc:1e:d5:6e:8c:dc", "--reset-cause", "0x15", "--strap", "0x6e",
        "--bootloader", &format!("{FW}/c6-hello-bootloader.bin"), "--ptable", &format!("{FW}/c6-hello-ptable.bin"), "--app", &format!("{FW}/c6-hello_world.bin"), "--max-seconds", "3"]);
    assert_eq!(r.stdout, r2.stdout); assert_eq!(r.insns, r2.insns);
}

/// The C6's esp_restart() path: a software CPU reset through LP_AON, back through the ROM with
/// the right cause and the ROM's `Saved PC` line.
/// As on the C3, partial-reset accounting preserves the 12 * 160 MHz reported count.
#[test] #[ignore = "needs the ESP32-C6 mask ROM ELF"]
fn hello_world_c6_reboot() {
    let rom = rom("esp32c6_rev0");
    let r = run(BIN_C6, &["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4",
        "--mac", "dc:1e:d5:6e:8c:dc", "--reset-cause", "0x15", "--strap", "0x6e",
        "--bootloader", &format!("{FW}/c6-hello-bootloader.bin"), "--ptable", &format!("{FW}/c6-hello-ptable.bin"), "--app", &format!("{FW}/c6-hello_world.bin"), "--max-seconds", "12"]);
    assert!(r.stderr.contains("[emu] chip reset at t="), "no reset seen:\n{}", r.stderr);
    assert!(r.stdout.matches("Hello world!").count() >= 2, "the app did not come back after the reset:\n{}", r.stdout);
    assert!(r.stdout.contains("rst:0xc (SW_CPU)") && r.stdout.contains("Saved PC:0x4001975a"), "the second boot banner differs from silicon:\n{}", r.stdout);
    expect_text("hello-c6-reboot.console.txt", &r.stdout);
    expect_u64("hello-c6-reboot.insns", r.insns);
}

fn p4_hello(seconds: &str) -> (Vec<String>, Run) { p4_run("p4-hello", seconds) }

/// A P4 example image the page does not run, which tools/fetch-p4-firmware.sh pins and fetches.
fn fetched(name: &str) -> String {
    assert!(root().join(FW).join(name).exists(), "{FW}/{name} is missing: tools/fetch-p4-firmware.sh fetches it");
    format!("{FW}/{name}")
}

fn p4_run(images: &str, seconds: &str) -> (Vec<String>, Run) { p4_run_on("esp32p4_rev0", images, seconds) }

fn p4_run_on(rom_name: &str, images: &str, seconds: &str) -> (Vec<String>, Run) {
    let rom = rom(rom_name);
    let (bl, app) = if images == "p4-hello" { (format!("{FW}/p4-hello-bootloader.bin"), format!("{FW}/p4-hello_world.bin")) }
        else { (fetched(&format!("{images}-bootloader.bin")), fetched(&format!("{images}_world.bin"))) };
    let args: Vec<String> = ["--rom", rom.to_str().unwrap(), "--flash-mb", "2",
        "--bootloader", &bl, "--ptable", &format!("{FW}/p4-hello-ptable.bin"), "--app", &app, "--max-seconds", seconds].iter().map(|s| s.to_string()).collect();
    let r = run(BIN_P4, &args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    (args, r)
}

/// hello_world for the ESP32-P4 rev < 3 from the ECO0 mask ROM (docs/esp32p4.md), single-core,
/// with the default boot mode and console: through the bootloader into FreeRTOS, `app_main` and
/// the countdown, on the CLIC's systimer tick and yield interrupts.
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn hello_world_p4() {
    let (args, r) = p4_hello("3");
    assert!(r.stdout.contains("Hello world!") && r.stdout.contains("Restarting in 9 seconds..."), "app_main never printed:\n{}", r.stdout);
    expect_text("hello-p4.console.txt", &r.stdout);
    expect_u64("hello-p4.insns", r.insns);
    let mut chip = vec!["--chip", "p4"];
    chip.extend(args.iter().map(|s| s.as_str()));
    let r2 = run(BIN, &chip);
    assert_eq!(r.stdout, r2.stdout); assert_eq!(r.insns, r2.insns);
}

/// The P4's esp_restart(): a software CPU reset through LP_CLKRST, back through the ROM with
/// the reset cause and core 0's saved PC, and into app_main again.
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn hello_world_p4_reboot() {
    let (_, r) = p4_hello("12");
    assert!(r.stderr.contains("[emu] chip reset at t="), "no reset seen:\n{}", r.stderr);
    assert!(r.stdout.matches("Hello world!").count() >= 2, "the app did not come back after the reset:\n{}", r.stdout);
    assert!(r.stdout.contains("rst:0xc (SW_CPU_RESET)"), "the second boot banner lacks the reset cause:\n{}", r.stdout);
    // the store that requested the reset, as a v1.3 board prints it; core 1 never ran
    assert!(r.stdout.contains("Core0 Saved PC:0x4ff02328") && !r.stdout.contains("Core1 Saved PC"), "{}", r.stdout);
    expect_text("hello-p4-reboot.console.txt", &r.stdout);
    expect_u64("hello-p4-reboot.insns", r.insns);
}

/// The same hello_world built for both cores (`examples/hello_world-p4/sdkconfig.defaults.smp`):
/// the ROM starts core 1 at the boot address the app gives it, FreeRTOS schedules both cores,
/// each on its own systimer alarm and cross-core interrupt, and esp_restart() brings both back.
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn hello_world_p4_smp() {
    let (_, r) = p4_run("p4-hello-smp", "12");
    assert!(r.stdout.contains("cpu_start: Multicore app") && r.stdout.matches("Hello world!").count() >= 2, "both boots should reach app_main:\n{}", r.stdout);
    let core1: u64 = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.split("core1 ").nth(1)).and_then(|t| t.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(core1 > 1_000_000_000, "core 1 did not run:\n{}", r.stderr);
    // core 0 at its reset store as on a v1.3 board (0x4ff0289a), and core 1 recorded too
    assert!(r.stdout.contains("Core0 Saved PC:0x4ff0289a") && r.stdout.contains("Core1 Saved PC:0x"), "{}", r.stdout);
    expect_text("hello-p4-smp.console.txt", &r.stdout);
    expect_u64("hello-p4-smp.insns", r.insns);
}

/// hello_world as `idf.py set-target esp32p4` builds it, for rev >= 3 (`examples/hello_world-p4`,
/// `sdkconfig.defaults.rev3`), on the rev >= 3 model, which esp32sim-p4 chooses from the
/// bootloader's header: the ECO5 ROM, the standard CLIC's `mintthresh`, the bootloader at the top
/// of L2MEM and the app above the L2 cache, the countdown and esp_restart() back through the ROM.
#[test] #[ignore = "needs the ESP32-P4 rev 3 mask ROM ELF"]
fn hello_world_p4_rev3() {
    let (args, r) = p4_run_on("esp32p4_rev300", "p4-hello-rev3", "12");
    assert!(r.stdout.starts_with("ESP-ROM:esp32p4-eco5-20250430") && r.stdout.contains("boot: chip revision: v3.1"), "not the rev 3 ROM and model:\n{}", r.stdout);
    assert!(r.stdout.matches("Hello world!").count() >= 2 && r.stdout.contains("rst:0xc (SW_CPU_RESET)"), "the app did not come back after the reset:\n{}", r.stdout);
    assert!(r.stdout.contains("Core0 Saved PC:0x4ff2237a") && !r.stdout.contains("Core1 Saved PC"), "the v3.2 board's saved PC:\n{}", r.stdout);
    expect_text("hello-p4-rev3.console.txt", &r.stdout);
    expect_u64("hello-p4-rev3.insns", r.insns);
    let mut chip = vec!["--chip", "p4"];
    chip.extend(args.iter().map(|s| s.as_str()));
    let r2 = run(BIN, &chip);
    assert_eq!(r.stdout, r2.stdout); assert_eq!(r.insns, r2.insns);
}

/// The same for both cores (`sdkconfig.defaults.rev3-smp`).
#[test] #[ignore = "needs the ESP32-P4 rev 3 mask ROM ELF"]
fn hello_world_p4_rev3_smp() {
    let (_, r) = p4_run_on("esp32p4_rev300", "p4-hello-rev3-smp", "12");
    assert!(r.stdout.contains("cpu_start: Multicore app") && r.stdout.matches("Hello world!").count() >= 2, "both boots should reach app_main:\n{}", r.stdout);
    let core1: u64 = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.split("core1 ").nth(1)).and_then(|t| t.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(core1 > 1_000_000_000, "core 1 did not run:\n{}", r.stderr);
    assert!(r.stdout.contains("Core0 Saved PC:0x4ff228f0") && r.stdout.contains("Core1 Saved PC:0x"), "the v3.2 board's saved PCs:\n{}", r.stdout);
    expect_text("hello-p4-rev3-smp.console.txt", &r.stdout);
    expect_u64("hello-p4-rev3-smp.insns", r.insns);
}

/// ESP-IDF's sdmmc example on the P4 (`examples/sd_card-p4`) with a card in the SD socket: the
/// driver brings the host up, identifies the card (slot 1 through the GPIO matrix, four lines,
/// 20 MHz), FatFs mounts its FAT32 volume, and the example writes, renames and reads back
/// files through the IDMAC. The card is a 64 MiB image the test lays out, written through, so
/// the files the firmware left are read back from it afterwards.
fn p4_sdcard(card: Option<&std::path::Path>, rw: bool, seconds: &str) -> Run { p4_sd_firmware("p4-sdcard", card, rw, seconds, &[]) }

fn p4_sd_firmware(images: &str, card: Option<&std::path::Path>, rw: bool, seconds: &str, extra: &[&str]) -> Run {
    p4_sd_firmware_on("esp32p4_rev0", images, card, rw, seconds, extra)
}

fn p4_sd_firmware_on(rom_name: &str, images: &str, card: Option<&std::path::Path>, rw: bool, seconds: &str, extra: &[&str]) -> Run {
    let rom = rom(rom_name);
    let (bl, app) = (fetched(&format!("{images}-bootloader.bin")), fetched(&format!("{images}.bin")));
    let mut args = vec!["--rom", rom.to_str().unwrap(), "--flash-mb", "2", "--bootloader", &bl,
        "--ptable", "web/wasm/fw/public/p4-hello-ptable.bin", "--app", &app, "--max-seconds", seconds];
    if let Some(c) = card { args.extend([if rw { "--sd-rw" } else { "--sd" }, c.to_str().unwrap()]); }
    args.extend(extra);
    run(BIN_P4, &args)
}

/// Long raw sector transfers through IDF's driver (`examples/sd_card-p4-sectors`): 64 and 128
/// sectors with CMD25 and CMD18, where the driver refills its four DMA descriptors from its
/// interrupt handler and the host sends the auto-stop; a misaligned buffer split into CMD17s; a
/// read past the end refused by the driver; and a card that programs for 2 ms after each write,
/// which the driver waits out polling CMD13.
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn sd_card_sectors_p4() {
    let card = tmp("sdsectors-p4.img");
    sd_image::fat32(&card, 64 << 20, 512, "").unwrap();
    let r = p4_sd_firmware("p4-sdsectors", Some(&card), true, "1", &["--sd-prg-us", "2000"]);
    for line in ["sectors 64: write 0x0 in ", "read 0x0 in ", "sectors 128: write 0x0, read 0x0, same 1", "sectors 64 misaligned: read 0x0, same 1",
                 "sectors past the end: read 0x104", "status: 0x0", "Card unmounted"] {
        assert!(r.stdout.contains(line), "missing {line:?} in:\n{}", r.stdout);
    }
    let write_us: u64 = r.stdout.split("sectors 64: write 0x0 in ").nth(1).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap();
    assert!(write_us >= 2000, "the write took {write_us} us, less than the card's programming time");
    assert_eq!(r.stdout.matches(", same 1").count(), 3, "every transfer read back what was written:\n{}", r.stdout);
    expect_text("sdcard-sectors-p4.console.txt", &r.stdout);
    expect_u64("sdcard-sectors-p4.insns", r.insns);
    let files = sd_image::Volume::open(&card).unwrap().root_files();
    assert_eq!(files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["NIHAO.TXT", "FOO.TXT"]);
}

#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn sd_card_p4() {
    let card = tmp("sdcard-p4.img");
    sd_image::fat32(&card, 64 << 20, 512, "").unwrap();
    let r = p4_sdcard(Some(&card), true, "1");
    assert!(r.stdout.contains("Read from file: 'Nihao SIMSD!'") && r.stdout.contains("Card unmounted"), "the example did not finish:\n{}", r.stdout);
    expect_text("sdcard-p4.console.txt", &r.stdout);
    expect_u64("sdcard-p4.insns", r.insns);
    let files = sd_image::Volume::open(&card).unwrap().root_files();
    let want: Vec<(String, Vec<u8>)> = vec![("NIHAO.TXT".into(), b"Nihao SIMSD!\n".to_vec()), ("FOO.TXT".into(), b"Hello SIMSD!\n".to_vec())];
    assert_eq!(files, want, "the files the example left on the card");
}

/// The same sdmmc example built for rev >= 3 (`examples/sd_card-p4/sdkconfig.defaults.rev3`), on
/// the rev >= 3 model: the same card, files and SD bus; as on a Waveshare ESP32-P4-WIFI6 with a v3.2
/// chip.
#[test] #[ignore = "needs the ESP32-P4 rev 3 mask ROM ELF"]
fn sd_card_p4_rev3() {
    let card = tmp("sdcard-p4-rev3.img");
    sd_image::fat32(&card, 64 << 20, 512, "").unwrap();
    let r = p4_sd_firmware_on("esp32p4_rev300", "p4-sdcard-rev3", Some(&card), true, "1", &[]);
    assert!(r.stdout.contains("boot: chip revision: v3.1") && r.stdout.contains("Read from file: 'Nihao SIMSD!'") && r.stdout.contains("Card unmounted"), "the example did not finish:\n{}", r.stdout);
    expect_text("sdcard-p4-rev3.console.txt", &r.stdout);
    expect_u64("sdcard-p4-rev3.insns", r.insns);
    let files = sd_image::Volume::open(&card).unwrap().root_files();
    let want: Vec<(String, Vec<u8>)> = vec![("NIHAO.TXT".into(), b"Nihao SIMSD!\n".to_vec()), ("FOO.TXT".into(), b"Hello SIMSD!\n".to_vec())];
    assert_eq!(files, want, "the files the example left on the card");
}

/// IDF's USB mass-storage example on the P4 (`examples/usb_msc-p4`), the card in its socket and a
/// host on its high-speed USB port (`host.txt`): enumeration, the card's capacity, raw blocks
/// written and read back, a FAT32 file written in 64 KiB chunks and read back, an eject that
/// hands the card to the example's application (TEST UNIT READY: MEDIUM NOT PRESENT), the
/// application writing README.MD (`console.txt`) and exposing the card again, the host reading
/// that file, suspend, resume and unplug. Pinned: the console, the host's log with its emulated
/// times, the instruction count, and what is on the card afterwards.
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn usb_msc_p4() {
    let card = tmp("usbmsc-p4.img");
    sd_image::fat32(&card, 64 << 20, 512, "").unwrap();
    let rom = rom("esp32p4_rev0");
    let r = run(BIN_P4, &["--rom", rom.to_str().unwrap(), "--flash-mb", "4", "--bootloader", &fetched("p4-usbmsc-bootloader.bin"),
        "--ptable", &fetched("p4-usbmsc-ptable.bin"), "--app", &fetched("p4-usbmsc.bin"), "--sd-rw", card.to_str().unwrap(),
        "--script", "examples/usb_msc-p4/console.txt", "--usb-host", "examples/usb_msc-p4/host.txt", "--max-seconds", "2"]);
    let host: String = r.stderr.lines().filter(|l| l.starts_with("[usb-host]")).map(|l| format!("{l}\n")).collect();
    for line in ["port reset: High speed", "device 303a:4002", "interface 0: class 08/06/50, endpoints 01 (512 bytes) 81 (512 bytes)",
                 "READ CAPACITY: 131072 blocks of 512 bytes", "verify 16 blocks at 131056: all a5", "fat-read FILE0001.BIN: 1048576 bytes",
                 "matches what was written", "TEST UNIT READY: sense 2/3a/00 (MEDIUM NOT PRESENT)", "root: FILE0001.BIN (1048576), README.MD (379)", "script done"] {
        assert!(host.contains(line), "missing {line:?} in the host's log:\n{host}");
    }
    assert!(r.stdout.contains("TinyUSB Driver installed on port 1") && r.stdout.contains("README.MD doesn't exist yet, creating"), "{}", r.stdout);
    expect_text("usbmsc-p4.console.txt", &r.stdout);
    expect_text("usbmsc-p4.host.txt", &host);
    expect_u64("usbmsc-p4.insns", r.insns);
    let files = sd_image::Volume::open(&card).unwrap().root_files();
    assert_eq!(files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["FILE0001.BIN", "README.MD"]);
    assert!(files[0].1 == esp_soc::usb_host::fat32::file_bytes("FILE0001.BIN", 1 << 20), "the file on the card is the one the host wrote");
    assert!(String::from_utf8_lossy(&files[1].1).starts_with("Mass Storage Devices are one of the most common USB devices."));
    let image = std::fs::read(&card).unwrap();
    assert!(image[131056 * 512..131072 * 512].iter().all(|&b| b == 0xa5), "the raw blocks the host wrote");
}

/// The example against the faults a host meets (examples/usb_msc-p4/faults.txt): a command that
/// times out mid-transfer and the port reset Linux's usb-storage recovers with, a port reset after
/// an eject (tud_mount_cb takes the card back from the application), and VBUS lost mid-write
/// (tud_umount_cb gives it to the application).
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn usb_msc_p4_faults() {
    let card = tmp("usbmsc-p4-faults.img");
    sd_image::fat32(&card, 64 << 20, 512, "").unwrap();
    let rom = rom("esp32p4_rev0");
    let r = run(BIN_P4, &["--rom", rom.to_str().unwrap(), "--flash-mb", "4", "--bootloader", &fetched("p4-usbmsc-bootloader.bin"),
        "--ptable", &fetched("p4-usbmsc-ptable.bin"), "--app", &fetched("p4-usbmsc.bin"), "--sd-rw", card.to_str().unwrap(),
        "--usb-host", "examples/usb_msc-p4/faults.txt", "--max-seconds", "3"]);
    let host: String = r.stderr.lines().filter(|l| l.starts_with("[usb-host]")).map(|l| format!("{l}\n")).collect();
    for line in ["command aborted after 5 ms", "port reset", "failed, going on: timeout", "verify 240 blocks at 130000: all 5a", "ejected",
                 "device reset, configuration 1 restored", "no device on the port", "failed, going on: device disconnected",
                 "root: FILE0002.BIN (0)", "script done"] {
        assert!(host.contains(line), "missing {line:?} in the host's log:\n{host}");
    }
    // boot, enumeration, eject, the port reset, VBUS lost, attached again, the final unplug
    let owner: Vec<&str> = r.stdout.lines().filter_map(|l| l.split("Storage mounted to application: ").nth(1)).collect();
    assert_eq!(owner, ["Yes", "No", "Yes", "No", "Yes", "No", "Yes"], "{}", r.stdout);
    assert!(r.stdout.contains("storage exposed over USB: Yes"), "{}", r.stdout);
    // the cut file: an entry of size 0 and no cluster, its clusters allocated in the FAT
    let image = std::fs::read(&card).unwrap();
    let v = &image[2048 * 512..];
    let (reserved, fat_sectors) = (u16::from_le_bytes([v[14], v[15]]) as usize, u32::from_le_bytes(v[36..40].try_into().unwrap()) as usize);
    let fat = &v[reserved * 512..(reserved + fat_sectors) * 512];
    let used = fat.as_chunks::<4>().0.iter().skip(3).filter(|&&e| u32::from_le_bytes(e) & 0x0fff_ffff != 0).count();
    let files = sd_image::Volume::open(&card).unwrap().root_files();
    assert!(files.iter().any(|f| f.0 == "FILE0002.BIN" && f.1.is_empty()), "{:?}", files.iter().map(|f| (&f.0, f.1.len())).collect::<Vec<_>>());
    assert!(used > 0, "the clusters the cut file was given are still allocated");
    expect_text("usbmsc-p4-faults.console.txt", &r.stdout);
    expect_text("usbmsc-p4-faults.host.txt", &host);
    expect_u64("usbmsc-p4-faults.insns", r.insns);
}

/// `--max-insns` bounds the host's script as it bounds a run: the script stops at the limit and
/// the run ends there.
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn usb_host_script_counts_against_max_insns() {
    let card = tmp("usbmsc-p4-insns.img");
    sd_image::fat32(&card, 64 << 20, 512, "").unwrap();
    let rom = rom("esp32p4_rev0");
    let r = run(BIN_P4, &["--rom", rom.to_str().unwrap(), "--flash-mb", "4", "--bootloader", &fetched("p4-usbmsc-bootloader.bin"),
        "--ptable", &fetched("p4-usbmsc-ptable.bin"), "--app", &fetched("p4-usbmsc.bin"), "--sd-rw", card.to_str().unwrap(),
        "--usb-host", "examples/usb_msc-p4/host.txt", "--max-insns", "20000000", "--max-seconds", "2"]);
    assert!(r.stderr.contains("failed: the run's instruction limit"), "{}", r.stderr);
    assert!(r.stderr.contains("[emu] stop: MaxInsns — core0 20000000 +"), "{}", r.stderr);
}

/// The same firmware with an unformatted card (FatFs finds no volume and the example does not
/// format) and with an empty socket (every command times out).
#[test] #[ignore = "needs the ESP32-P4 mask ROM ELF"]
fn sd_card_p4_unformatted_and_absent() {
    let card = tmp("sdcard-blank.img");
    std::fs::File::create(&card).unwrap().set_len(64 << 20).unwrap();
    let r = p4_sdcard(Some(&card), false, "1");
    assert!(r.stdout.contains("vfs_fat_sdmmc: failed to mount card (13)") && r.stdout.contains("example: Failed to mount filesystem."), "{}", r.stdout);
    assert!(std::fs::read(&card).unwrap().iter().all(|&b| b == 0), "copy-on-write leaves the image alone");
    let r = p4_sdcard(None, false, "1");
    assert!(r.stdout.contains("example: Failed to initialize the card (ESP_ERR_TIMEOUT)"), "{}", r.stdout);
}

/// The IEEE 802.15.4 energy scanner on the Waveshare ESP32-C6-LCD-1.47 (its owner's firmware,
/// not in this repository): `ENERGY_SCAN_DIR` points at the built project. Pins the console and
/// the counts of energy scans, display writes and LED updates the board model reports.
#[test] #[ignore = "set ENERGY_SCAN_DIR=/path/to/energy_scan (built for esp32c6); needs the ESP32-C6 mask ROM ELF"]
fn external_energy_scan_c6() {
    let dir = std::env::var("ENERGY_SCAN_DIR").expect("ENERGY_SCAN_DIR=/path/to/energy_scan is required for this test");
    let b = format!("{}/build", dir); let rom = rom("esp32c6_rev0");
    let r = run(BIN_C6, &["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4", "--board", "waveshare-c6-lcd147", "--no-dump",
        "--bootloader", &format!("{b}/bootloader/bootloader.bin"), "--ptable", &format!("{b}/partition_table/partition-table.bin"),
        "--app", &format!("{b}/energy_scan.bin"), "--elf", &format!("{b}/energy_scan.elf"), "--max-seconds", "4"]);
    assert!(r.stdout.contains("libbtbb version"), "the PHY did not come up:\n{}", r.stdout);
    assert!(!r.stdout.contains("Guru Meditation"), "the app panicked:\n{}", r.stdout);
    let scans: u64 = r.stderr.lines().find(|l| l.starts_with("[emu] 802.15.4:")).and_then(|l| l.split_whitespace().nth(2)).and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(scans >= 50, "too few energy scans: {}\n{}", scans, r.stderr);
    let lcd = r.stderr.lines().find(|l| l.starts_with("[emu] lcd147:")).unwrap_or("");
    assert!(lcd.contains("RAMWR") && lcd.contains("updates"), "no board report:\n{}", r.stderr);
    expect_text("energy-scan-c6.console.txt", &r.stdout);
    expect_u64("energy-scan-c6.insns", r.insns);
}

/// The WiFi station (`examples/c6-wifi-station`, built with ESP-IDF 5.5.4 for each chip with the
/// emulator's default network and no display, `sdkconfig.ci.defaults`): the unmodified WiFi
/// library scans, finds the virtual access point, joins it through the WPA2 four-way handshake,
/// takes a DHCP lease and gets five of five gateway pings answered. The station's own lines are
/// the same on every chip and are what the board prints against a real network; the console and
/// the instruction count pin the run bit for bit, and the end-of-run counts pin the radio traffic
/// and the interrupts (on the C3 and C6 the instruction count is the cycle count, so a change in
/// the WiFi model's timing shows only in the interrupt count). This is the bar for changes to the
/// WiFi model.
fn wifi_station(chip: &str, bin: &str, rom_name: &str, extra: &[&str]) {
    let rom = rom(rom_name);
    let mut args = vec!["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4", "--console", "usb", "--no-dump"];
    let (bl, pt, app) = (format!("{FW}/{chip}-wifi-bootloader.bin"), format!("{FW}/{chip}-wifi-ptable.bin"), format!("{FW}/{chip}-wifi_station.bin"));
    args.extend(["--bootloader", &bl, "--ptable", &pt, "--app", &app, "--wifi", "ssid=esp32sim,psk=esp32sim-pass", "--net", "none", "--max-seconds", "14"]);
    args.extend(extra);
    let r = run(bin, &args);
    assert!(!r.stdout.contains("Guru Meditation") && !r.stdout.contains("assert failed"), "the app panicked:\n{}", r.stdout);
    let station: String = r.stdout.lines().filter_map(|l| l.split_once("station: ").map(|(_, rest)| rest.trim_end_matches("\u{1b}[0m"))).map(|l| format!("{l}\n")).collect();
    assert!(station.contains("GOT_IP ip=10.0.2.15") && station.contains("PING done sent=5 received=5"), "the station did not get through:\n{}\n{}", station, r.stderr);
    expect_text(&format!("wifi-station-{chip}.station.txt"), &station);
    expect_text(&format!("wifi-station-{chip}.console.txt"), &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("[emu] wifi:") || l.starts_with("[emu] net:"))).map(|l| format!("{l}\n")).collect();
    expect_text(&format!("wifi-station-{chip}.report.txt"), &report);
    expect_u64(&format!("wifi-station-{chip}.insns"), r.insns);
}

#[test] #[ignore = "needs the ESP32-S3 mask ROM ELF"]
fn wifi_station_s3() { wifi_station("s3", BIN, "esp32s3_rev0", &["--board", "none"]); }

#[test] #[ignore = "needs the ESP32-C3 mask ROM ELF"]
fn wifi_station_c3() { wifi_station("c3", BIN_C3, "esp32c3_rev3", &[]); }

/// The committed image, no ELF and no stub: the PHY blob's baseband calibration (`bb_init`) runs
/// to completion against the modelled baseband status fields (docs/esp32c6.md, #203).
#[test] #[ignore = "needs the ESP32-C6 mask ROM ELF"]
fn wifi_station_c6() { wifi_station("c6", BIN_C6, "esp32c6_rev0", &[]); }

/// The P4 with an ESP32-C6 co-processor on SDIO (`examples/esp-hosted-p4`): IDF's WiFi station
/// example on the P4, its WiFi through esp_wifi_remote and ESP-Hosted 2.12.13 to the unmodified
/// ESP-Hosted slave on the C6, whose radio joins the virtual access point. The P4 resets the C6
/// on GPIO 54, brings up the SDIO transport (CMD5/3/7/52/53, the INIT event and capabilities),
/// sends the WiFi RPCs, and takes a DHCP lease over the link. Pinned: both consoles in the order
/// the slices produced them, both chips' instruction counts, the C6's SDIO, WiFi and network
/// counts.
#[test] #[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn hosted_p4() {
    let (rom_p4, rom_c6) = (rom("esp32p4_rev0"), rom("esp32c6_rev0"));
    let coproc = format!("c6:bootloader={},ptable={},app={},otadata={},rom={},stub=0x420bac5c=0",
        fetched("c6-hosted-bootloader.bin"), fetched("c6-hosted-ptable.bin"), fetched("c6-hosted-slave.bin"), fetched("c6-hosted-otadata.bin"), rom_c6.display());
    let r = run(BIN_P4, &["--rom", rom_p4.to_str().unwrap(), "--flash-mb", "2", "--no-dump",
        "--bootloader", &fetched("p4-hosted-bootloader.bin"), "--ptable", &fetched("p4-hosted-ptable.bin"), "--app", &fetched("p4-hosted-station.bin"),
        "--coproc", &coproc, "--wifi", "ssid=esp32sim,psk=esp32sim-pass", "--net", "none,epoch=1767225600", "--max-seconds", "4"]);
    assert!(r.stdout.contains("[c6] I (115) co-pro-main:                 ESP-Hosted-MCU Slave FW version :: 2.12.13"), "{}", r.stdout);
    assert!(r.stdout.contains("transport: Identified slave [esp32c6]") && r.stdout.contains("wifi station: got ip:10.0.2.15"), "{}\n{}", r.stdout, r.stderr);
    assert!(!r.stdout.contains("Guru Meditation") && !r.stdout.contains("Version mismatch"), "{}", r.stdout);
    expect_text("hosted-p4.console.txt", &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("[c6]") || l.starts_with("[cosim]") || l.starts_with("[sdmmc]")))
        .map(|l| format!("{l}\n")).collect();
    expect_text("hosted-p4.report.txt", &report);
    expect_u64("hosted-p4.insns", r.insns);
}

/// The same pair with the P4 at rev >= 3 (`examples/esp-hosted-p4/station`,
/// `sdkconfig.defaults.rev3`): esp32sim-p4 takes the rev >= 3 model from the bootloader, and the
/// co-simulation runs it with the same C6. As on a Waveshare ESP32-P4-WIFI6 with a v3.2 chip: the
/// SDIO transport, the slave identified, WiFi started; here the virtual access point gives the lease.
#[test] #[ignore = "needs the ESP32-P4 rev 3 and ESP32-C6 mask ROM ELFs"]
fn hosted_p4_rev3() {
    let (rom_p4, rom_c6) = (rom("esp32p4_rev300"), rom("esp32c6_rev0"));
    let coproc = format!("c6:bootloader={},ptable={},app={},otadata={},rom={},stub=0x420bac5c=0",
        fetched("c6-hosted-bootloader.bin"), fetched("c6-hosted-ptable.bin"), fetched("c6-hosted-slave.bin"), fetched("c6-hosted-otadata.bin"), rom_c6.display());
    let r = run(BIN_P4, &["--rom", rom_p4.to_str().unwrap(), "--flash-mb", "2", "--no-dump",
        "--bootloader", &fetched("p4-hosted-rev3-bootloader.bin"), "--ptable", &fetched("p4-hosted-ptable.bin"), "--app", &fetched("p4-hosted-rev3-station.bin"),
        "--coproc", &coproc, "--wifi", "ssid=esp32sim,psk=esp32sim-pass", "--net", "none,epoch=1767225600", "--max-seconds", "4"]);
    assert!(r.stdout.contains("boot: chip revision: v3.1") && r.stdout.contains("transport: Identified slave [esp32c6]") && r.stdout.contains("wifi station: got ip:10.0.2.15"), "{}\n{}", r.stdout, r.stderr);
    assert!(!r.stdout.contains("Guru Meditation") && !r.stdout.contains("Version mismatch"), "{}", r.stdout);
    expect_text("hosted-p4-rev3.console.txt", &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("[c6]") || l.starts_with("[cosim]") || l.starts_with("[sdmmc]")))
        .map(|l| format!("{l}\n")).collect();
    expect_text("hosted-p4-rev3.report.txt", &report);
    expect_u64("hosted-p4-rev3.insns", r.insns);
}

/// Scintilla (the Elka Synthex in integer C, GPL-3.0, joakimeriksson/esp32 `eed6180`, directory
/// `scintilla-p4-lcd35`; licences in web/wasm/fw/public/p4-scintilla-NOTICE.txt) on
/// the model of Waveshare's ESP32-P4-WIFI6-Touch-LCD-3.5, with no C6 on the SDIO bus: Waveshare's
/// BSP 2.0.2 and LVGL 9 bring up the ST7796 over GPSPI2 and the AXI-GDMA, the FT5x06 on I2C1 and
/// the ES8311 on I2S1 and the AHB-GDMA. A4 (the thirteenth white key, at the panel's (290, 160)
/// under the firmware's landscape touch mapping) held for 0.6 s sounds at 440 Hz; a tap on the
/// program display opens the patch picker. Pinned: the audio, the last frame, the instruction count.
#[test] #[ignore = "needs the ESP32-P4 rev 3 mask ROM ELF"]
fn scintilla_p4_lcd35() {
    let rom = rom("esp32p4_rev300");
    let (script, wav, png) = (tmp("scintilla.txt"), tmp("scintilla.wav"), tmp("scintilla.png"));
    std::fs::write(&script, "4.5 touch 290 160 1\n5.1 touch 290 160 0\n6.0 touch 23 179 1\n6.0 touch 23 179 0\n").unwrap();
    let r = run(BIN_P4, &["--rom", rom.to_str().unwrap(), "--board", "waveshare-p4-lcd35", "--flash-mb", "16", "--psram-mb", "32", "--no-dump",
        "--bootloader", &fetched("p4-scintilla-lcd35-bootloader.bin"), "--ptable", &fetched("p4-scintilla-lcd35-ptable.bin"), "--app", &fetched("p4-scintilla-lcd35.bin"),
        "--script", script.to_str().unwrap(), "--wav", wav.to_str().unwrap(), "--tft-png", png.to_str().unwrap(), "--max-seconds", "7"]);
    for line in ["boot: chip revision: v3.1", "audio: preset 0 Chunky Synth", "ble-midi: no co-processor answers, BLE MIDI off", "scintilla: ready"] {
        assert!(r.stdout.contains(line), "missing {line:?}:\n{}", r.stdout);
    }
    assert!(!r.stdout.contains("Guru Meditation") && !r.stdout.contains("abort()"), "{}", r.stdout);
    assert!(r.stderr.contains("[emu] p4-lcd35:") && r.stderr.contains("madctl=0xe8") && r.stderr.contains("amplifier on"), "{}", r.stderr);
    let data = std::fs::read(&wav).unwrap();
    let pcm: Vec<i16> = data[44..].as_chunks::<2>().0.iter().map(|&b| i16::from_le_bytes(b)).collect();
    let onset = pcm.iter().position(|&v| v != 0).expect("the key sounds");
    assert!(onset > 3 * 44100, "sound before the key was touched, at sample {onset}");
    // the fundamental, by autocorrelation over 40 ms from 50 ms after the onset
    let w = &pcm[onset + 2205..onset + 2205 + 1764];
    let lag = (44100 / 1000..44100 / 100).max_by_key(|&l| w[..w.len() - l].iter().zip(&w[l..]).map(|(&a, &b)| a as i64 * b as i64).sum::<i64>()).unwrap();
    let hz = 44100.0 / lag as f64;
    assert!((hz - 440.0).abs() < 440.0 * 0.03, "A4 sounds at {hz:.1} Hz");
    expect_sha("scintilla-p4-lcd35.wav.sha256", &data);
    expect_sha("scintilla-p4-lcd35.png.sha256", &std::fs::read(&png).unwrap());
    expect_u64("scintilla-p4-lcd35.insns", r.insns);
}

/// ESP-Hosted's raw throughput test on the same pair (`examples/esp-hosted-p4`, "Raw
/// throughput"), the SD bus timed at the slot's 4 lines and 40 MHz: `rx` the C6 sends, `tx` the P4
/// does. Traffic starts near 7.7 s (both sides wait 5 s); the per-second reports pin the steady
/// rate. The receive run carries about 40 MB, so PKT_LEN wraps at 2^20 dozens of times.
fn hosted_rawtp(dir: &str, steady: &str) {
    let (rom_p4, rom_c6) = (rom("esp32p4_rev0"), rom("esp32c6_rev0"));
    let coproc = format!("c6:bootloader={},ptable={},app={},otadata={},rom={},stub=0x420bb020=0",
        fetched("c6-hosted-bootloader.bin"), fetched("c6-hosted-ptable.bin"), fetched("c6-hosted-rawtp-slave.bin"), fetched("c6-hosted-otadata.bin"), rom_c6.display());
    let r = run(BIN_P4, &["--rom", rom_p4.to_str().unwrap(), "--flash-mb", "2", "--no-dump", "--sd-timing",
        "--bootloader", &fetched("p4-hosted-bootloader.bin"), "--ptable", &fetched("p4-hosted-ptable.bin"), "--app", &fetched(&format!("p4-hosted-rawtp-{dir}.bin")),
        "--coproc", &coproc, "--wifi", "ssid=esp32sim,psk=esp32sim-pass", "--net", "none,epoch=1767225600", "--max-seconds", "10"]);
    assert!(r.stdout.contains(steady), "{}\n{}", r.stdout, r.stderr);
    assert!(!r.stdout.contains("Guru Meditation") && !r.stdout.lines().any(|l| l.trim_start_matches("[c6] ").starts_with("E (")), "{}", r.stdout);
    expect_text(&format!("hosted-rawtp-{dir}.console.txt"), &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("[c6]") || l.starts_with("[cosim]") || l.starts_with("[sdmmc]")))
        .map(|l| format!("{l}\n")).collect();
    expect_text(&format!("hosted-rawtp-{dir}.report.txt"), &report);
    expect_u64(&format!("hosted-rawtp-{dir}.insns"), r.insns);
}

#[test] #[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn hosted_rawtp_rx() { hosted_rawtp("rx", "stats: 7-8 sec Tx:0 Rx:139518 Kbps"); }

#[test] #[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn hosted_rawtp_tx() { hosted_rawtp("tx", "stats: 8-9 sec       Rx: 106283.44 Tx: 0.00 kbps"); }

/// The guest controller owns the advertising schedule and handles every END IRQ.
#[test] #[ignore = "needs the ESP32-C3 mask ROM ELF fetched by CI"]
fn ble_advertiser_c3() {
    let rom = rom("esp32c3_rev3");
    let r = run(BIN_C3, &["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4", "--no-dump",
        "--bootloader", &format!("{FW}/c3-ble-bootloader.bin"), "--ptable", &format!("{FW}/c3-ble-ptable.bin"),
        "--app", &format!("{FW}/c3-ble-advertiser.bin"), "--ble", "full", "--ble-observe", "--max-seconds", "2"]);
    assert!(r.stdout.contains("advertiser: STARTED"), "{}", r.stdout);
    assert!(!r.stdout.contains("assert") && !r.stderr.contains("[ble-error]"), "{}\n{}", r.stdout, r.stderr);
    assert!(r.stderr.contains("0 exceptions"), "{}", r.stderr);
    let packets: Vec<_> = r.stderr.lines().filter(|line| line.starts_with("[ble-air]")).collect();
    assert!(packets.len() >= 51 && packets.len().is_multiple_of(3), "must wrap the 16-entry event table");
    for event in packets.as_chunks::<3>().0 {
        for (line, channel) in event.iter().zip([37, 38, 39]) {
            assert!(line.contains(&format!("channel={channel} type=ADV_SCAN_IND")), "{line}");
            assert!(line.contains("name=\"esp32sim\"") && line.contains("service=180f"), "{line}");
        }
    }
    expect_text("ble-advertiser-c3.console.txt", &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("  core") || l.starts_with("[ble-config]")))
        .map(|l| format!("{l}\n")).collect();
    expect_text("ble-advertiser-c3.report.txt", &report);
    expect_text("ble-advertiser-c3.observer.txt", &(packets.join("\n") + "\n"));
    expect_u64("ble-advertiser-c3.insns", r.insns);
}


fn crypto_tls(chip: &str, bin: &str, rom_name: &str) {
    let rom = rom(rom_name);
    let r = run(bin, &["--rom", rom.to_str().unwrap(), "--boot", "rom", "--flash-mb", "4", "--no-dump",
        "--bootloader", &format!("{FW}/{chip}-crypto-tls-bootloader.bin"),
        "--ptable", &format!("{FW}/{chip}-crypto-tls-ptable.bin"), "--app", &format!("{FW}/{chip}-crypto-tls.bin"),
        "--wifi", "ssid=esp32sim,psk=esp32sim-pass", "--net", "none", "--max-seconds", "10"]);
    assert!(r.stdout.contains("TLS GOT_IP 10.0.2.15"), "{}", r.stdout);
    assert!(r.stdout.contains("TLS PASS TLSv1.2 TLS-ECDHE-PSK-WITH-AES-128-CBC-SHA256 bidirectional=256"), "{}", r.stdout);
    assert!(!r.stdout.contains("FAIL") && !r.stdout.contains("assert") && r.stderr.contains("0 exceptions"), "{}\n{}", r.stdout, r.stderr);
    expect_text(&format!("crypto-tls-{chip}.console.txt"), &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("  core") || l.starts_with("[emu] wifi:") || l.starts_with("[emu] net:")))
        .map(|l| format!("{l}\n")).collect();
    expect_text(&format!("crypto-tls-{chip}.report.txt"), &report);
}

#[test] #[ignore = "needs the ESP32-C3 mask ROM ELF fetched by CI"]
fn crypto_tls_c3() { crypto_tls("c3", BIN_C3, "esp32c3_rev3"); }

#[test] #[ignore = "needs the ESP32-C6 mask ROM ELF fetched by CI"]
fn crypto_tls_c6() { crypto_tls("c6", BIN_C6, "esp32c6_rev0"); }
/// The stock countdown includes 10 through 0: eleven one-second delays per reset.
#[test]
#[ignore = "needs the ESP32 ECO3 mask ROM ELF"]
fn classic_reset_clock_and_flash_size() {
    let rom = rom("esp32_rev300");
    let r = run(BIN, &["--chip", "esp32", "--rom", rom.to_str().unwrap(), "--board", "esp32dev", "--boot", "rom", "--no-dump", "--console", "uart0", "--flash-mb", "16",
        "--bootloader", &format!("{FW}/classic-hello-bootloader.bin"), "--ptable", &format!("{FW}/classic-hello-ptable.bin"), "--app", &format!("{FW}/classic-hello_world.bin"), "--max-seconds", "25"]);
    let resets: Vec<f64> = r.stderr.lines().filter_map(|l| l.strip_prefix("[emu] chip reset at t="))
        .map(|s| s.split_once('s').unwrap().0.parse().unwrap()).collect();
    assert_eq!(resets.len(), 2, "{}", r.stderr);
    for interval in [resets[0], resets[1] - resets[0]] { assert!((11.0..11.1).contains(&interval), "{resets:?}"); }
    assert_eq!(r.stdout.matches("Detected size(16384k)").count(), 3, "{}", r.stdout);
    assert!(!r.stdout.contains("Detected size(8192k)"));
    assert_eq!(r.stdout.matches("Restarting now.").count(), 2);
    assert!(r.stderr.contains("[emu] stop: Halted"), "{}", r.stderr);
    assert!(!r.stdout.contains("ESP_ERR_FLASH_SIZE_NOT_MATCH"), "{}", r.stdout);
}

/// Classic ECO3 ROM, stock IDF bootloader and hello_world application.
#[test]
#[ignore = "needs the ESP32 ECO3 mask ROM ELF"]
fn hello_world_classic() {
    let rom = rom("esp32_rev300");
    let r = run(BIN, &["--chip", "esp32", "--rom", rom.to_str().unwrap(), "--board", "esp32dev", "--boot", "rom", "--no-dump", "--console", "uart0",
        "--bootloader", &format!("{FW}/classic-hello-bootloader.bin"), "--ptable", &format!("{FW}/classic-hello-ptable.bin"), "--app", &format!("{FW}/classic-hello_world.bin"), "--max-seconds", "3"]);
    for text in ["ets Jul 29 2019", "2nd stage bootloader", "Hello world!"] { assert!(r.stdout.contains(text), "missing {text}: {}", r.stdout); }
    expect_text("hello-classic.console.txt", &r.stdout);
    let events = r.stderr.lines().find(|l| l.starts_with("[emu] stop:")).and_then(|l| l.rsplit_once("); ")).map_or("", |(_, e)| e);
    let report: String = std::iter::once(events).chain(r.stderr.lines().filter(|l| l.starts_with("  core")))
        .map(|l| format!("{l}\n")).collect();
    expect_text("hello-classic.report.txt", &report);
}
