//! Native execution check for caller-supplied original CoreMatrix firmware.
//! Build against the integrated C61 crate; see system-audit.md for commands.
use esp_soc::{SocBus, Stop};

fn run(m: &mut esp32c61::Machine, count: u64) -> Stop {
    let stop = m.run(count);
    m.drain_console();
    assert_eq!(m.exceptions, 0, "unexpected guest exception: {stop:?}");
    assert!(matches!(stop, Stop::MaxInsns | Stop::SwReset), "unexpected stop: {stop:?}");
    stop
}

fn script(m: &mut esp32c61::Machine, source: &[u8]) -> (Stop, String) {
    let start = m.console.all.len();
    m.bus.serial_input(&[3, 3, 1]);
    run(m, 2_000_000);
    for chunk in source.chunks(64) {
        m.bus.serial_input(chunk);
        run(m, 1_000_000);
    }
    m.bus.serial_input(&[4]);
    let stop = run(m, 500_000_000);
    let output = String::from_utf8_lossy(&m.console.all[start..]).into_owned();
    print!("{output}");
    (stop, output)
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 5, "system-audit ROM_ELF ORIGINAL_BIN WRITE_PY READ_PY");
    let mut m = esp32c61::machine([2, 0, 0, 0, 0, 0x61], 16 << 20);
    m.bus.set_flash_size(16 << 20);
    m.bus.set_psram_size(8 << 20).unwrap();
    m.bus.periph.usb.connected = true;
    m.bus.board = esp32c61::board::make_board("corematrix").unwrap();
    m.bus.attach_board_devices();
    m.console.capture = true;
    m.console.mask = 0;
    m.load_rom(&std::fs::read(&args[1]).unwrap()).unwrap();
    m.write_flash(0, &std::fs::read(&args[2]).unwrap()).unwrap();
    m.boot_rom();
    m.dbg.stop_after_exceptions = 1;
    run(&mut m, 100_000_000);
    let boot = String::from_utf8_lossy(&m.console.all);
    assert!(boot.contains("MicroPython"), "official firmware did not boot");
    println!("CM_SYSTEM_BOOT insns={} exceptions={}", m.insns(), m.exceptions);
    let (stop, first) = script(&mut m, &std::fs::read(&args[3]).unwrap());
    assert!(matches!(stop, Stop::SwReset), "guest did not request hardware reset");
    for marker in ["CM_SYSTEM_FLASH_WRITTEN", "CM_SYSTEM_RTC_ACTIVE", "CM_SYSTEM_I2C", "CM_SYSTEM_RESET_REQUEST"] {
        assert!(first.contains(marker), "missing guest result: {marker}");
    }
    let cause = m.reboot();
    println!("CM_SYSTEM_HOST_REBOOT cause={cause}");
    run(&mut m, 100_000_000);
    let (_, second) = script(&mut m, &std::fs::read(&args[4]).unwrap());
    for marker in ["CM_SYSTEM_FLASH_RETAINED", "CM_SYSTEM_RTC_AFTER_RESET", "CM_SYSTEM_RESET_CAUSE", "CM_SYSTEM_DONE"] {
        assert!(second.contains(marker), "missing guest result: {marker}");
    }
    assert!(!second.contains("Traceback"), "post-reset guest check failed");
    assert_eq!(m.reboots, 1);
    println!("CM_SYSTEM_PASS reboots={} insns={} exceptions={}", m.reboots, m.insns(), m.exceptions);
}
