//! IDF v6.1 hello_world for esp32s31, booted from the real mask ROM on the Machine path.
//! Ignored by default: needs the ROM ELF and images from `tools/fetch-s31-firmware.sh`.
//!   S31_ROM_ELF=…/esp32s31_rev0_rom.elf cargo test -p esp32s31 --release --test hello_golden -- --ignored
//! Optional: `S31_FIRMWARE_DIR` overrides the default `esp32s31/firmware` image dir.
use esp_soc::SocBus as _;

fn console(m: &esp_soc::Machine<esp32s31::S31>) -> String {
    // Prefer UART0 (IDF console); fall back to USB if UART is empty.
    let u = String::from_utf8_lossy(&m.console.uart0);
    if !u.is_empty() {
        u.into_owned()
    } else {
        String::from_utf8_lossy(&m.console.usb).into_owned()
    }
}

fn clear_console(m: &mut esp_soc::Machine<esp32s31::S31>) {
    m.console.usb.clear();
    m.console.uart0.clear();
    m.console.all.clear();
}

fn boot_hello(rom: &[u8], dir: &str) -> esp_soc::Machine<esp32s31::S31> {
    // 2 MiB NOR + W25Q16 JEDEC matches the IDF image header (avoids flash-size warning).
    let mut m = esp32s31::machine([0x10, 0x20, 0x30, 0x40, 0x50, 0x60], 2 << 20);
    m.bus.load_mask_rom(rom).unwrap();
    for (off, f) in [
        (0x2000, "s31-hello-bootloader.bin"),
        (0x8000, "s31-hello-ptable.bin"),
        (0x10000, "s31-hello_world.bin"),
    ] {
        m.bus
            .write_flash(off, &std::fs::read(format!("{dir}/{f}")).unwrap())
            .unwrap();
    }
    m.bus
        .set_board_inputs(esp32s31::soc::BoardInputs::hello_w25q16())
        .unwrap();
    m.bus.set_strap(0x10); // SPI_FAST_FLASH_BOOT
    m.boot_rom();
    m.cores[0].x[2] = esp32s31::ROM_STACK;
    m.console.mask = 2; // UART0 is the IDF console
    m.console.capture = true;
    m
}

#[test]
#[ignore = "needs S31_ROM_ELF and tools/fetch-s31-firmware.sh images"]
fn idf61_hello_world_prints_from_mask_rom_spi_boot() {
    let rom = std::fs::read(std::env::var("S31_ROM_ELF").expect("S31_ROM_ELF")).unwrap();
    let dir = std::env::var("S31_FIRMWARE_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/firmware").into()
    });
    let mut m = boot_hello(&rom, &dir);
    let mut n = 0u64;
    // Drive through the 10 s countdown; SYSTIMER next_deadline lets idle skip FreeRTOS delays.
    loop {
        match m.run(10_000_000) {
            esp_soc::Stop::MaxInsns => {}
            esp_soc::Stop::SwReset => break,
            other => panic!("unexpected stop before restart: {other:?}"),
        }
        n += 10_000_000;
        assert!(
            n < 8_000_000_000,
            "timed out waiting for esp_restart; console:\n{}",
            console(&m)
        );
    }
    let out = console(&m);
    assert!(
        out.contains("ESP-ROM:esp32s31-20251218"),
        "missing ROM banner:\n{out}"
    );
    assert!(
        out.contains("rst:0x1 (POWERON),boot:0x10 (SPI_FAST_FLASH_BOOT)"),
        "missing first POWERON banner:\n{out}"
    );
    assert!(
        out.contains("App version:      s31-hello-1") || out.contains("App version: s31-hello-1"),
        "missing pinned app version:\n{out}"
    );
    assert!(out.contains("Hello world!"), "missing Hello world:\n{out}");
    assert!(
        out.contains("2MB external flash"),
        "flash size should match 2 MiB header/board:\n{out}"
    );
    assert!(
        out.contains("Restarting now"),
        "missing end-of-countdown:\n{out}"
    );
    assert_eq!(
        m.bus.reset_cause(),
        esp_periph::RST_SW_CPU,
        "esp_restart must latch RST_SW_CPU before Machine::reboot"
    );

    let cause = m.reboot();
    assert_eq!(cause, esp_periph::RST_SW_CPU);
    // USB + UART0 both capture the same guest stream; clear so second-boot
    // assertions are not confused with first-boot banners / Hello counts.
    clear_console(&mut m);

    n = 0;
    while !console(&m).contains("Hello world!") {
        match m.run(10_000_000) {
            esp_soc::Stop::MaxInsns => {}
            esp_soc::Stop::SwReset => {
                // Second countdown can request another restart; keep going until Hello.
                let _ = m.reboot();
                clear_console(&mut m);
            }
            other => panic!("unexpected stop on second boot: {other:?}"),
        }
        n += 10_000_000;
        assert!(
            n < 4_000_000_000,
            "timed out on second boot; console:\n{}",
            console(&m)
        );
    }
    let out = console(&m);
    assert!(
        out.contains("rst:0xc (SW_CPU_RESET),boot:0x10 (SPI_FAST_FLASH_BOOT)"),
        "second boot must report software reset cause:\n{out}"
    );
}

#[test]
#[ignore = "needs tools/fetch-s31-firmware.sh app image only (no ROM ELF)"]
fn boot_app_skips_rom_and_reaches_hello_entry() {
    let dir = std::env::var("S31_FIRMWARE_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/firmware").into()
    });
    let mut m = esp32s31::machine([0x10, 0x20, 0x30, 0x40, 0x50, 0x60], 2 << 20);
    m.bus
        .write_flash(
            0x10000,
            &std::fs::read(format!("{dir}/s31-hello_world.bin")).unwrap(),
        )
        .unwrap();
    m.bus
        .set_board_inputs(esp32s31::soc::BoardInputs::hello_w25q16())
        .unwrap();
    let entry = m.boot_app(0x10000).expect("boot_app");
    assert!(
        (0x2f00_0000..0x2f08_0000).contains(&entry),
        "unexpected entry {entry:#x}"
    );
    // Entry is typically in SRAM after the image loader copies .iram/.dram; a few
    // million instructions should not immediately trap on an unmapped fetch.
    match m.run(100_000) {
        esp_soc::Stop::MaxInsns => {}
        other => panic!("boot_app entry trapped early: {other:?} pc={:#x}", m.cores[0].pc),
    }
}
