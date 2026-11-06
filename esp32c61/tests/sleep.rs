//! Original UIFlow sleep acceptance; no firmware patch or sleep-function stub.
use esp_soc::{Soc, SocBus};

fn advance(m: &mut esp32c61::Machine, cycles: u64) {
    let result = m.run_until_cycle(m.bus.cycles + cycles);
    assert!(matches!(result, esp_soc::RunUntil::Reached), "{result:?}, PC={:#x}, console={}", m.cores[0].pc, String::from_utf8_lossy(&m.console.all));
}
fn raw_program(m: &mut esp32c61::Machine, source: &[u8]) {
    m.bus.serial_input(&[1]); m.run(2_000_000);
    for chunk in source.chunks(64) { m.bus.serial_input(chunk); m.run(1_000_000); }
    m.bus.serial_input(&[4]);
}
fn await_sleep(m: &mut esp32c61::Machine, deep: bool) {
    for _ in 0..4000 {
        advance(m, 40000);
        if m.bus.periph.lpsys.sleeping() {
            assert_eq!(m.bus.periph.lpsys.deep_sleeping(), deep);
            assert_eq!(esp32c61::C61::core_state(&m.bus, 0), esp_soc::CoreState::Held);
            return;
        }
    }
    panic!("firmware did not enter PMU sleep; PC={:#x}, console={}", m.cores[0].pc, String::from_utf8_lossy(&m.console.all));
}

#[test]
#[ignore = "requires ESP32SIM_COREMATRIX_FIRMWARE (official UIFlow 2.5.4 image) and ESP32SIM_ROM_DIR"]
fn external_official_uiflow_timed_light_and_deep_sleep() {
    let roms = std::path::PathBuf::from(std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the fetched C61 ROM directory"));
    let firmware = std::env::var_os("ESP32SIM_COREMATRIX_FIRMWARE").expect("set ESP32SIM_COREMATRIX_FIRMWARE to the original CoreMatrix UIFlow 2.5.4 merged image");
    let mut m = esp32c61::machine([2,0,0,0,0,0x61], 16 << 20);
    m.console.capture = true; m.console.mask = 0; m.dbg.stop_after_exceptions = 1;
    m.bus.set_psram_size(8 << 20).unwrap(); m.bus.periph.usb.connected = true;
    m.bus.board = esp32c61::board::make_board("corematrix").unwrap(); m.bus.attach_board_devices();
    m.load_rom(&std::fs::read(roms.join("esp32c61_rev100_rom.elf")).expect("read esp32c61_rev100_rom.elf")).unwrap();
    m.write_flash(0, &std::fs::read(firmware).expect("read original CoreMatrix merged image")).unwrap(); m.boot_rom(); m.run(100_000_000);
    assert!(String::from_utf8_lossy(&m.console.all).contains("M5Stack CoreMatrix with ESP32-C61HR8"));
    assert!(String::from_utf8_lossy(&m.console.all).contains(">>>"));
    raw_program(&mut m, b"import machine,time\nt=time.ticks_ms()\nmachine.lightsleep(200)\nprint('C61_LIGHT_DONE',time.ticks_diff(time.ticks_ms(),t))\n");
    await_sleep(&mut m, false);
    let state = (m.cores[0].pc, m.cores[0].insn_count, m.cores[0].x);
    let systime = m.bus.periph.systimer.unit[0];
    advance(&mut m, 16_000_000);
    assert!(m.bus.periph.lpsys.sleeping());
    assert_eq!((m.cores[0].pc, m.cores[0].insn_count, m.cores[0].x), state);
    assert_eq!(m.bus.periph.systimer.unit[0], systime, "digital timer advanced during sleep");
    let remaining = m.bus.periph.lpsys.sleep_deadline_cycles().expect("armed LP alarm");
    advance(&mut m, remaining + 1); assert!(!m.bus.periph.lpsys.sleeping(), "remaining={remaining}, ticks={}, wake_en={:08x}, raw={:08x}, target={:08x}:{:08x}, lp_target={:08x}:{:08x}", m.bus.periph.lpsys.slow_ticks, m.bus.periph.lpsys.read(0x112c), m.bus.periph.lpsys.read(0xc28), m.bus.periph.lpsys.read(0xc04), m.bus.periph.lpsys.read(0xc00), m.bus.periph.lpsys.read(0xc0c), m.bus.periph.lpsys.read(0xc08)); assert!(!m.bus.sw_reset());
    m.run(20_000_000);
    let text = String::from_utf8_lossy(&m.console.all);
    let elapsed: i32 = text.lines().find_map(|line| line.split_once("C61_LIGHT_DONE ").and_then(|(_, v)| v.trim().parse().ok())).unwrap_or_else(|| panic!("light sleep must return and print elapsed milliseconds, PC={:#x}; {text}", m.cores[0].pc));
    assert!((180..=220).contains(&elapsed), "elapsed={elapsed}; {text}");
    let before = m.console.all.len();
    raw_program(&mut m, b"import machine\nprint('C61_DEEP_START')\nmachine.deepsleep(200)\nprint('C61_DEEP_MUST_NOT_RETURN')\n");
    await_sleep(&mut m, true);
    let state = (m.cores[0].pc, m.cores[0].insn_count);
    let remaining = m.bus.periph.lpsys.sleep_deadline_cycles().expect("armed deep LP alarm");
    advance(&mut m, remaining - 1); assert!(m.bus.periph.lpsys.sleeping());
    assert_eq!((m.cores[0].pc, m.cores[0].insn_count), state);
    let result = m.run_until_cycle(m.bus.cycles + 2);
    assert!(matches!(result, esp_soc::RunUntil::Stop(esp_soc::Stop::SwReset)), "{result:?}");
    assert_eq!(m.bus.reset_cause(), 5); assert_eq!(m.reboot(), 5);
    m.run(100_000_000);
    let reboot = String::from_utf8_lossy(&m.console.all[before..]);
    assert!(reboot.contains("ESP-ROM:esp32c61"), "{reboot}");
    assert!(reboot.contains(">>>"), "{reboot}");
    assert!(!reboot.contains("C61_DEEP_MUST_NOT_RETURN"), "{reboot}");
    raw_program(&mut m, b"import machine\nprint('C61_DEEP_RESET_OK',machine.reset_cause()==machine.DEEPSLEEP_RESET)\n");
    m.run(20_000_000);
    assert!(String::from_utf8_lossy(&m.console.all).contains("C61_DEEP_RESET_OK True"));
    assert_eq!(m.exceptions, 0); assert_eq!(m.reboots, 1);
}
