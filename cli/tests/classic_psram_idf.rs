use emu_core::Bus;
use esp_soc::SocBus;
use std::{env, fs, path::PathBuf};

// Genuine ROM -> stock IDF startup -> guest allocator. No register preconfiguration,
// symbol stubs, host-side allocator, or direct PSRAM writes in this harness.
fn boot(present: bool) -> String {
    let firmware = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/wasm/fw/public");
    let rom = PathBuf::from(env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the fetched ROM directory"))
        .join("esp32_rev300_rom.elf");
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.bus.set_psram_size(if present { 4 << 20 } else { 0 }).unwrap();
    m.console.capture = true;
    m.load_rom(&fs::read(rom).unwrap()).unwrap();
    for (offset, name) in [(0x1000, "bootloader"), (0x8000, "ptable"), (0x10000, "app")] {
        m.write_flash(offset, &fs::read(firmware.join(format!("classic-psram-idf-{name}.bin"))).unwrap()).unwrap();
    }
    m.boot_rom();
    for _ in 0..3000 {
        let stop = m.run(100_000);
        let serial = String::from_utf8_lossy(&m.console.uart0);
        if serial.contains("P4 result=pass\r\n") || serial.contains("P4 result=failed\r\n")
            || serial.contains("Rebooting...") || !matches!(stop, esp_soc::Stop::MaxInsns) { break; }
    }
    let serial = String::from_utf8_lossy(&m.console.uart0).into_owned();
    println!("{serial}");
    println!("P4 host present={present} insns={} exceptions={} interrupts={} irq={:?}", m.insns(), m.exceptions, m.interrupts, m.irq_hist);
    assert!(m.stubs.is_empty());
    assert_eq!(m.stub_hits, 0);
    assert_eq!(m.reboots, 0, "harness must not hide failed startup behind a reboot");
    if present && serial.contains("P4 result=pass") {
        assert_eq!((m.insns(), m.exceptions, m.interrupts), (26_944_062, 399, 15));
        let mut irq = [[0; 32]; 2];
        irq[0][6] = 13;
        irq[0][9] = 2;
        assert_eq!(m.irq_hist.as_slice(), irq);
        // IDF v4.4.7 spiram.c:128-139 calls the ROM/IRAM mapper for 128 32-KiB pages.
        // Unicore deliberately leaves APP table invalid; no dual-core claim.
        for page in 0..128 {
            assert_eq!(m.bus.read32(0x3ff1_1200 + page * 4).unwrap(), page, "guest PRO mapping");
            assert_eq!(m.bus.read32(0x3ff1_3200 + page * 4).unwrap(), 0x100, "held APP mapping");
        }
    }
    if !present && serial.contains("Failed to init external RAM!") {
        assert_eq!((m.insns(), m.exceptions, m.interrupts), (2_431_527, 22, 0));
        assert_eq!(m.irq_hist.as_slice(), [[0; 32]; 2]);
    }
    serial
}

#[test]
#[ignore = "needs ESP32SIM_ROM_DIR containing the classic ECO3 mask ROM ELF"]
fn classic_psram_idf_boot_allocator() {
    let serial = boot(true);
    for milestone in [
        "PSRAM initialized, cache is in normal (1-core) mode.",
        "SPI SRAM memory test OK",
        "Adding pool of 4095K of external SPI memory to heap allocator",
        "P4 profile=boot-init idf=v4.4.7",
        "P4 physical=4194304 heap=4194303 free=4192135",
        "P4 allocation=1048576 address=3f80087c",
        "P4 readback=1048576 errors=0 fnv1a=678f9dc5 pages=33",
        "P4 spiram_dma=rejected plain_dma=internal",
        "P4 exhaustion=rejected",
        "P4 result=pass",
    ] { assert!(serial.contains(milestone), "missing guest milestone {milestone}: {serial}"); }
    assert!(!serial.contains("panic'ed") && !serial.contains("abort()"));
    // Repeat creation boots the exact same firmware with a fresh backing, not retained bytes.
    assert_eq!(serial, boot(true));
}

#[test]
#[ignore = "needs ESP32SIM_ROM_DIR containing the classic ECO3 mask ROM ELF"]
fn classic_psram_idf_absent_fails_before_allocator() {
    let serial = boot(false);
    assert!(serial.contains("PSRAM ID read error: 0xffffffff"));
    assert!(serial.contains("Failed to init external RAM!"));
    assert!(!serial.contains("SPI SRAM memory test OK"));
    assert!(!serial.contains("Adding pool of"));
    assert!(!serial.contains("P4 profile=") && !serial.contains("P4 result=pass"));
}
