//! External stock dual-core Arduino-ESP32 3.0.7 qualification.
//! Build docs/fixtures/classic-psram-arduino3/recipe.sh; never substitute unicore.
use emu_core::Bus;
use esp_soc::SocBus;
use std::{env, fs, path::PathBuf};

// Passive entry counts distinguish cached failure from a second physical probe.
struct Calls { pcs: [u32; 5], last: [u32; 2], hits: std::rc::Rc<std::cell::Cell<[u32; 5]>> }
impl<S: esp_soc::Soc> esp_soc::observe::Observer<S> for Calls {
    fn name(&self) -> &'static str { "psram-call-counts" }
    fn wants(&self) -> esp_soc::observe::Wants { esp_soc::observe::Wants::INSN }
    fn on_insn(&mut self, _: &esp_soc::observe::Ctx, core: usize, _: &S::Core, _: &mut S::Bus, pc: u32) -> Option<esp_soc::Stop> {
        self.last[core] = pc;
        None
    }
    fn after_insn(&mut self, _: &esp_soc::observe::Ctx, core: usize, _: &S::Core, _: &mut S::Bus) -> Option<esp_soc::Stop> {
        // A breakpoint sees an entry before execution; count only retired entries.
        let mut hits = self.hits.get();
        for (i, addr) in self.pcs.iter().enumerate() { if self.last[core] == *addr { hits[i] += 1; } }
        self.hits.set(hits);
        None
    }
}

fn boot(present: bool) -> String {
    let firmware = PathBuf::from(env::var_os("ARDUINO3_FIRMWARE_DIR")
        .expect("set ARDUINO3_FIRMWARE_DIR to the public P6 fw-p6 build"));
    let rom = PathBuf::from(env::var_os("ESP32SIM_ROM_DIR")
        .expect("set ESP32SIM_ROM_DIR to the fetched mask ROM directory"));
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.bus.set_psram_size(if present { 4 << 20 } else { 0 }).unwrap();
    m.console.capture = true;
    m.load_rom(&fs::read(rom.join("esp32_rev300_rom.elf")).unwrap()).unwrap();
    m.add_symbols(&fs::read(firmware.join("p6_probe.ino.elf")).unwrap()).unwrap();
    for (offset, suffix) in [(0x1000, ".bootloader.bin"), (0x8000, ".partitions.bin"), (0x10000, ".bin")] {
        m.write_flash(offset, &fs::read(firmware.join(format!("p6_probe.ino{suffix}"))).unwrap()).unwrap();
    }
    let hits = std::rc::Rc::new(std::cell::Cell::new([0; 5]));
    let pcs = ["psramInit", "esp_psram_init", "esp_psram_extram_test", "psramAddToHeap", "esp_psram_extram_add_to_heap_allocator"].map(|name| m.sym_addr(name).expect(name));
    m.add_observer(Box::new(Calls { pcs, last: [0; 2], hits: hits.clone() }));
    let mut milestones = 0;
    if present {
        let pc = m.sym_addr("esp_psram_extram_test").expect("real SDK memory test symbol");
        m.add_observer(Box::new(esp_soc::observers::Breakpoints { pcs: vec![pc] }));
    }
    m.boot_rom();
    let mut stop = esp_soc::Stop::MaxInsns;
    for _ in 0..5000 {
        stop = m.run(100_000);
        if matches!(stop, esp_soc::Stop::Breakpoint(_)) {
            m.observers.pop(); // Preserve the passive call counter.
            if milestones == 0 {
                // IDF 5.1.4 dport_reg.h: PRO/APP low/high controls and SRAM MMU.
                assert_eq!(m.bus.read32(0x3ff00040).unwrap() & (1 << 16), 1 << 16, "PRO low/high mode");
                assert_eq!(m.bus.read32(0x3ff00058).unwrap() & (1 << 14), 1 << 14, "APP low/high mode");
                for page in 0..128 {
                    assert_eq!(m.bus.read32(0x3ff11200 + page * 4).unwrap(), page, "guest PRO map");
                    assert_eq!(m.bus.read32(0x3ff13200 + page * 4).unwrap(), page, "guest APP map");
                }
                let pc = m.sym_addr("psramAddToHeap").expect("separate Arduino heap-add symbol");
                m.add_observer(Box::new(esp_soc::observers::Breakpoints { pcs: vec![pc] }));
            } else if milestones == 1 {
                let pc = m.sym_addr("esp_psram_extram_add_to_heap_allocator").expect("real SDK heap-add symbol");
                m.add_observer(Box::new(esp_soc::observers::Breakpoints { pcs: vec![pc] }));
            } else {
                // IDF esp_psram.c extram_test stride-32 test, before allocator metadata.
                for offset in (0..(4 << 20)).step_by(32) {
                    assert_eq!(m.bus.read32(0x3f800000 + offset), Ok((offset / 4) ^ 0xaaaaaaaa),
                               "real SDK memory-test words across both low/high banks");
                }
            }
            milestones += 1;
            continue;
        }
        let serial = String::from_utf8_lossy(&m.console.uart0);
        if serial.contains("P6 result=") || serial.contains("Rebooting...")
            || !matches!(stop, esp_soc::Stop::MaxInsns) { break; }
    }
    let serial = String::from_utf8_lossy(&m.console.uart0).into_owned();
    println!("{serial}");
    println!("P6 host present={present} stop={stop:?} milestones={milestones} insns={} exceptions={} interrupts={} irq={:?}",
        m.insns(), m.exceptions, m.interrupts, m.irq_hist);
    assert!(m.stubs.is_empty(), "no firmware stubs");
    assert_eq!(m.stub_hits, 0, "no intercepted guest calls");
    assert_eq!(m.reboots, 0, "do not mask startup failure with reboot");
    if present { assert_eq!(milestones, 3, "real SDK init/test/heap-add path must execute"); }
    println!("P6 guest calls={:?}", hits.get());
    assert_eq!(hits.get(), if present { [2, 2, 1, 1, 1] } else { [2, 1, 0, 2, 0] }, "real split startup, cached Arduino retry and SDK duplicate entry counts");
    serial
}

#[test]
#[ignore = "needs ARDUINO3_FIRMWARE_DIR public P6 stock A2 build and ESP32SIM_ROM_DIR"]
fn external_arduino3_psram_startup_and_readback() {
    let serial = boot(true);
    assert!(serial.contains("P6 boot-init=0 duplicate-arduino=1"), "pinned BOOT_INIT=n and successful Arduino duplicate");
    assert!(serial.contains("P6 duplicate-sdk=259 expected=259"), "SDK duplicate must return ESP_ERR_INVALID_STATE");
    // Stock IDF5 registers all 4 MiB, and Arduino returns the total range.
    // Free payload subtracts heap header 20 + TLSF 2148 + poisoning 12.
    assert!(serial.contains("found=1 size=4194304 heap=4194304"), "Arduino getters and exact SDK heap range");
    for (pass, hash) in [(1, "20c19dc5"), (2, "9dce9dc5")] {
        let prefix = format!("P6 pass={pass} allocation=1048576 address=");
        let line = serial.lines().find(|line| line.starts_with(&prefix)).expect("real guest 1 MiB allocation/reallocation");
        let address = u32::from_str_radix(&line[prefix.len()..prefix.len()+8], 16).unwrap();
        assert!((0x3f800000..=0x3fb00000).contains(&address), "allocation must reside in mapped PSRAM");
        assert!(line.ends_with(&format!("errors=0 fnv1a={hash}")), "volatile bytewise readback and independent hash");
    }
    let line = serial.lines().find(|line| line.starts_with("P6 free-before=")).expect("free/reallocate accounting");
    let values: Vec<u32> = line.split_whitespace().skip(1)
        .map(|field| field.split_once('=').unwrap().1.parse().unwrap()).collect();
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], 4192124, "available heap excludes metadata and poisoning overhead");
    assert!(values.iter().all(|value| *value == values[0]), "both guest frees must recover available heap");
    assert!(serial.contains("P6 result=pass"), "full Arduino qualification must complete");
    assert_eq!(serial, boot(true), "repeat boot must produce identical guest console");
}

#[test]
#[ignore = "needs ARDUINO3_FIRMWARE_DIR public P6 stock A2 build and ESP32SIM_ROM_DIR"]
fn external_arduino3_no_device_is_rejected() {
    let serial = boot(false);
    assert!(serial.contains("P6 boot-init=0 duplicate-arduino=0"), "pinned BOOT_INIT=n preserves failed state on retry");
    assert!(serial.contains("P6 failed-add=0"), "heap-add must reject undetected PSRAM");
    assert!(serial.contains("PSRAM ID read error: 0xffffffff"), "absent device command response");
    assert!(serial.contains("found=0 size=0 heap=0 free=0"), "absent Arduino getters and heap");
    assert!(serial.contains("P6 absent-allocation=unavailable"), "no internal-RAM fallback");
    assert!(serial.contains("P6 result=no-psram"));
    assert!(!serial.contains("P6 pass=1"));
}
