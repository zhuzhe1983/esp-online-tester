//! External stock dual-core Arduino-ESP32 2.0.17 qualification.
//! Build docs/fixtures/classic-psram-arduino2/recipe.sh; never substitute unicore.
use emu_core::Bus;
use esp_soc::SocBus;
use std::{env, fs, path::PathBuf};

fn boot(present: bool) -> String {
    let firmware = PathBuf::from(env::var_os("ARDUINO2_FIRMWARE_DIR")
        .expect("set ARDUINO2_FIRMWARE_DIR to the public P5 fw-A2 build"));
    let rom = PathBuf::from(env::var_os("ESP32SIM_ROM_DIR")
        .expect("set ESP32SIM_ROM_DIR to the fetched mask ROM directory"));
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.bus.set_psram_size(if present { 4 << 20 } else { 0 }).unwrap();
    m.console.capture = true;
    m.load_rom(&fs::read(rom.join("esp32_rev300_rom.elf")).unwrap()).unwrap();
    m.add_symbols(&fs::read(firmware.join("p5_probe.ino.elf")).unwrap()).unwrap();
    for (offset, suffix) in [(0x1000, ".bootloader.bin"), (0x8000, ".partitions.bin"), (0x10000, ".bin")] {
        m.write_flash(offset, &fs::read(firmware.join(format!("p5_probe.ino{suffix}"))).unwrap()).unwrap();
    }
    let mut milestones = 0;
    if present {
        let pc = m.sym_addr("esp_spiram_test").expect("real SDK memory test symbol");
        m.add_observer(Box::new(esp_soc::observers::Breakpoints { pcs: vec![pc] }));
    }
    m.boot_rom();
    let mut stop = esp_soc::Stop::MaxInsns;
    for _ in 0..5000 {
        stop = m.run(100_000);
        if matches!(stop, esp_soc::Stop::Breakpoint(_)) {
            m.observers.clear();
            if milestones == 0 {
                // IDF 4.4.7 dport_reg.h: PRO/APP low/high controls and SRAM MMU.
                assert_eq!(m.bus.read32(0x3ff00040).unwrap() & (1 << 16), 1 << 16, "PRO low/high mode");
                assert_eq!(m.bus.read32(0x3ff00058).unwrap() & (1 << 14), 1 << 14, "APP low/high mode");
                for page in 0..128 {
                    assert_eq!(m.bus.read32(0x3ff11200 + page * 4).unwrap(), page, "guest PRO map");
                    assert_eq!(m.bus.read32(0x3ff13200 + page * 4).unwrap(), page, "guest APP map");
                }
                let pc = m.sym_addr("esp_spiram_add_to_heapalloc").expect("real SDK heap-add symbol");
                m.add_observer(Box::new(esp_soc::observers::Breakpoints { pcs: vec![pc] }));
            } else {
                // IDF spiram.c:97-118 stride-32 test, before allocator metadata.
                for offset in (0..(4 << 20)).step_by(32) {
                    assert_eq!(m.bus.read32(0x3f800000 + offset), Ok((offset / 4) ^ 0xaaaaaaaa),
                               "real SDK memory-test words across both low/high banks");
                }
            }
            milestones += 1;
            continue;
        }
        let serial = String::from_utf8_lossy(&m.console.uart0);
        if serial.contains("P5 result=") || serial.contains("Rebooting...")
            || !matches!(stop, esp_soc::Stop::MaxInsns) { break; }
    }
    let serial = String::from_utf8_lossy(&m.console.uart0).into_owned();
    println!("{serial}");
    println!("P5 host present={present} stop={stop:?} milestones={milestones} insns={} exceptions={} interrupts={} irq={:?}",
        m.insns(), m.exceptions, m.interrupts, m.irq_hist);
    assert!(m.stubs.is_empty(), "no firmware stubs");
    assert_eq!(m.stub_hits, 0, "no intercepted guest calls");
    assert_eq!(m.reboots, 0, "do not mask startup failure with reboot");
    if present { assert_eq!(milestones, 2, "real SDK init/test/heap-add path must execute"); }
    serial
}

#[test]
#[ignore = "needs ARDUINO2_FIRMWARE_DIR public P5 stock A2 build and ESP32SIM_ROM_DIR"]
fn external_arduino2_psram_startup_and_readback() {
    let serial = boot(true);
    // Weak linked himem reserve is zero; IDF registers [start,start+4MiB-1).
    // Esp.cpp getPsramSize sums multi_heap free+allocated, not the raw range:
    // subtract heap_t=20, TLSF=36+16*4+16*32*4=2148, poison head/tail=12.
    // IDF v4.4.7 multi_heap.c:80-86,109-132,390-408;
    // heap_tlsf.c:473-489; multi_heap_poisoning.c:369-379.
    assert!(serial.contains("found=1 size=4192123 heap=4194303"), "Arduino getters and exact SDK heap range");
    for (pass, hash) in [(1, "20c19dc5"), (2, "9dce9dc5")] {
        let prefix = format!("P5 pass={pass} allocation=1048576 address=");
        let line = serial.lines().find(|line| line.starts_with(&prefix)).expect("real guest 1 MiB allocation/reallocation");
        let address = u32::from_str_radix(&line[prefix.len()..prefix.len()+8], 16).unwrap();
        assert!((0x3f800000..=0x3fb00000).contains(&address), "allocation must reside in mapped PSRAM");
        assert!(line.ends_with(&format!("errors=0 fnv1a={hash}")), "volatile bytewise readback and independent hash");
    }
    let line = serial.lines().find(|line| line.starts_with("P5 free-before=")).expect("free/reallocate accounting");
    let values: Vec<u32> = line.split_whitespace().skip(1)
        .map(|field| field.split_once('=').unwrap().1.parse().unwrap()).collect();
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], 4192123, "available heap excludes metadata and poisoning overhead");
    assert!(values.iter().all(|value| *value == values[0]), "both guest frees must recover available heap");
    assert!(serial.contains("P5 result=pass"), "full Arduino qualification must complete");
    assert_eq!(serial, boot(true), "repeat boot must produce identical guest console");
}

#[test]
#[ignore = "needs ARDUINO2_FIRMWARE_DIR public P5 stock A2 build and ESP32SIM_ROM_DIR"]
fn external_arduino2_no_device_is_rejected() {
    let serial = boot(false);
    assert!(serial.contains("PSRAM ID read error: 0xffffffff"), "absent device command response");
    assert!(serial.contains("found=0 size=0 heap=0 free=0"), "absent Arduino getters and heap");
    assert!(serial.contains("P5 absent-allocation=unavailable"), "no internal-RAM fallback");
    assert!(serial.contains("P5 result=no-psram"));
    assert!(!serial.contains("P5 pass=1"));
}
