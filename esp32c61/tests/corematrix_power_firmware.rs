//! External original-firmware acceptance. Inputs are never patched; Python is
//! transient RAM input through the same USB raw REPL used by the platform.
use emu_core::Bus;
use esp_soc::{RunUntil, SocBus, Stop};
use esp32c61::Machine;

const MS: u64 = esp32c61::periph::CPU_HZ / 1000;
const BANNER: &str = "M5Stack CoreMatrix with ESP32-C61HR8";

fn advance(m: &mut Machine, ms: u64) {
    let target = m.bus.cycles + ms * MS;
    loop {
        match m.run_until_cycle(target) {
            RunUntil::Reached => break,
            RunUntil::Yield => {},
            stop => panic!("unexpected stop {stop:?}: {}", String::from_utf8_lossy(&m.console.all)),
        }
    }
    m.drain_console();
    assert_eq!(m.exceptions, 0);
}

fn wait_text(m: &mut Machine, start: usize, needle: &str, max_ms: u64) {
    for _ in 0..max_ms.div_ceil(25) {
        if String::from_utf8_lossy(&m.console.all[start..]).contains(needle) { return; }
        advance(m, 25);
    }
    panic!("missing {needle:?}: {}", String::from_utf8_lossy(&m.console.all[start..]));
}

fn submit(m: &mut Machine, source: &str) -> usize {
    m.bus.serial_input(&[1]); advance(m, 20);
    for chunk in source.as_bytes().chunks(64) { m.bus.serial_input(chunk); advance(m, 10); }
    let start = m.console.all.len();
    m.bus.serial_input(&[4]);
    start
}

fn raw(m: &mut Machine, source: &str, marker: &str) -> String {
    let start = submit(m, source);
    wait_text(m, start, marker, 6000);
    wait_text(m, start, "\u{4}\u{4}>", 1000);
    let output = String::from_utf8_lossy(&m.console.all[start..]).into_owned();
    assert!(!output.contains("Traceback"), "{output}");
    m.bus.serial_input(&[2]); advance(m, 20);
    output
}

fn power(m: &mut Machine, pressed: bool) {
    assert!(m.bus.board.button_at(m.bus.cycles, "power", pressed));
    m.bus.tick(1);
}

fn reboot_to_repl(m: &mut Machine) {
    assert!(m.bus.sw_reset());
    let start = m.console.all.len();
    m.reboot();
    wait_text(m, start, BANNER, 6000);
    wait_text(m, start, ">>>", 1000);
    assert!(m.bus.controller_powered);
    assert!(String::from_utf8_lossy(&m.console.all[start..]).contains("ESP-ROM:esp32c61"));
}

fn verify_held(m: &mut Machine, ms: u64) -> u64 {
    assert!(!m.bus.controller_powered);
    let retired = m.insns(); let pc = m.cores[0].pc;
    advance(m, ms);
    assert_eq!(m.insns(), retired, "powered-off CPU retired instructions");
    assert_eq!(m.cores[0].pc, pc, "powered-off CPU changed PC");
    assert!(!m.bus.sw_reset());
    retired
}

#[test]
#[ignore="requires ESP32SIM_COREMATRIX_FIRMWARE (official UIFlow 2.5.4 merged image) and ESP32SIM_ROM_DIR"]
fn external_corematrix_uiflow_power_key_voltage_and_motion_wake() {
    let roms = std::path::PathBuf::from(std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the fetched ROM directory"));
    let firmware = std::env::var_os("ESP32SIM_COREMATRIX_FIRMWARE").expect("set ESP32SIM_COREMATRIX_FIRMWARE to the official CoreMatrix UIFlow 2.5.4 merged image");
    let mut m = esp32c61::machine([2,0,0,0,0,0x61], 16 << 20);
    m.console.capture = true; m.console.mask = 0; m.dbg.stop_after_exceptions = 1;
    m.bus.set_psram_size(8 << 20).unwrap(); m.bus.periph.usb.connected = true;
    m.bus.board = esp32c61::board::make_board("corematrix").unwrap(); m.bus.attach_board_devices();
    m.load_rom(&std::fs::read(roms.join("esp32c61_rev100_rom.elf")).expect("read esp32c61_rev100_rom.elf")).unwrap();
    m.write_flash(0, &std::fs::read(firmware).expect("read original official CoreMatrix merged image")).unwrap();
    m.boot_rom(); wait_text(&mut m, 0, BANNER, 6000); wait_text(&mut m, 0, ">>>", 1000);

    // M5Unified b67ea7b requires two stable battery observations >=1250 ms
    // apart. An initial -1 means not yet determined, not a failed voltage ADC.
    let output = raw(&mut m, concat!(
        "import M5, time\nfrom machine import I2C, Pin\nM5.begin()\n",
        "p=I2C(0,sda=Pin(0),scl=Pin(1),freq=400000)\n",
        "for _ in range(3):\n print('CM_POWER',M5.Power.getBatteryVoltage(),M5.Power.getVBUSVoltage(),M5.Power.getExtOutput())\n time.sleep_ms(1300)\n",
        "print('CM_KEY_CONFIG',list(p.readfrom_mem(0x6e,0x49,2)))\nprint('CM_POWER_DONE')\n"
    ), "CM_POWER_DONE");
    assert!(output.contains("CM_POWER 3900 5000 True"), "{output}");
    assert!(output.contains("CM_KEY_CONFIG [42, 0]"), "{output}");
    println!("{}", output.trim_matches(['\u{4}', '>']));

    // Physical double click, with its second release while the CPU is going
    // down. No host register writes or synthetic reset invoke the shutdown.
    power(&mut m, true); advance(&mut m, 20); power(&mut m, false);
    advance(&mut m, 70); power(&mut m, true); advance(&mut m, 20); power(&mut m, false);
    let held = verify_held(&mut m, 500);
    println!("CM_KEY_OFF held_ms=500 retired_before={held} retired_after={}", m.insns());
    power(&mut m, true); assert!(m.bus.controller_powered); assert!(m.bus.sw_reset());
    // Releasing the same wake press cannot queue a second reset. Release it
    // while reset is pending, then verify a complete ROM + original app boot.
    power(&mut m, false); reboot_to_repl(&mut m); advance(&mut m, 500);
    assert_eq!(m.reboots, 1); println!("CM_KEY_WAKE rom_boots=2 reboots=1");

    for hold in [false, true] {
        assert!(m.bus.board.set_imu_sample([0.,0.,1.], [0.;3]));
        let output = raw(&mut m, include_str!("../../tools/fixtures/corematrix-uiflow/power-wake.py"), "CM_MOTION_READY");
        assert!(output.contains("CM_IMU_INIT 1"), "{output}");
        println!("{}", output.trim_matches(['\u{4}', '>']));
        let source = format!("p.writeto_mem(0x6e,7,bytes([{}]))\nprint('CM_SLEEP',{})\np.writeto_mem(0x6e,0x0c,b'\\xa1')\n", if hold {32} else {0}, u8::from(hold));
        let start = submit(&mut m, &source);
        for _ in 0..100 { if !m.bus.controller_powered {break;} advance(&mut m, 1); }
        assert!(!m.bus.controller_powered, "{}", String::from_utf8_lossy(&m.console.all[start..]));
        // Establish a stable baseline while held, then change only the host
        // acceleration stimulus. Guest config decides whether an IRQ can wake.
        let held = verify_held(&mut m, 50);
        assert!(m.bus.board.set_imu_sample([0.5,0.,1.], [0.;3]));
        let changed_at = m.bus.cycles;
        verify_held(&mut m, 80);
        if hold {
            let stop = m.run_until_cycle(changed_at + 125 * MS); m.drain_console();
            assert!(matches!(stop, RunUntil::Stop(Stop::SwReset)), "{stop:?}");
            assert_eq!(m.insns(), held, "motion wake must reset before executing the sleeping PC");
            assert!(m.bus.controller_powered);
            assert!((80 * MS..=100 * MS).contains(&(m.bus.cycles - changed_at)), "five 50 Hz samples, with arbitrary initial phase");
            println!("CM_MOTION_WAKE hold=1 latency_cycles={} retired_before={held} retired_after={}", m.bus.cycles - changed_at, m.insns());
            reboot_to_repl(&mut m);
            // WAKE_SRC GPIO3/4 pair is bit5; the board's actual wire is GPIO4.
            let output = raw(&mut m, "from machine import I2C,Pin\np=I2C(0,sda=Pin(0),scl=Pin(1),freq=400000)\nprint('CM_WAKE_SOURCE',p.readfrom_mem(0x6e,5,1)[0])\nprint('CM_MOTION_DONE')\n", "CM_MOTION_DONE");
            assert!(output.contains("CM_WAKE_SOURCE 32"), "{output}");
            println!("CM_MOTION_REBOOT rom_boots=4 wake_source=32");
        } else {
            verify_held(&mut m, 70);
            println!("CM_MOTION_NO_WAKE hold=0 stimulus_ms=150 retired_before={held} retired_after={}", m.insns());
            power(&mut m, true); power(&mut m, false); reboot_to_repl(&mut m);
        }
    }
    assert_eq!(m.exceptions, 0); assert_eq!(m.reboots, 3);
    println!("CM_POWER_ACCEPTANCE PASS exceptions={} reboots={}", m.exceptions, m.reboots);
}
