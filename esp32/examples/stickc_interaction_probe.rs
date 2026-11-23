//! Exercise the unchanged K016-C factory image through the ESP32 ROM and LCD.
use esp_soc::SocBus;
use std::path::Path;

const SECOND: u64 = 240_000_000;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "stickc_interaction_probe <merged.bin> <rom.elf> <capture-directory>");
    let output = Path::new(&args[3]);
    std::fs::create_dir_all(output).unwrap();

    let mut machine = esp32::machine([0; 6], 4 << 20);
    machine.bus.board = esp32::board::make_board("stickc").unwrap();
    machine.bus.attach_board_devices();
    machine.console.capture = true;
    machine.console.mask = 0;
    machine.load_rom(&std::fs::read(&args[2]).unwrap()).unwrap();
    machine.write_flash(0, &std::fs::read(&args[1]).unwrap()).unwrap();
    machine.boot_rom();

    let mut captures = Vec::new();
    for (name, elapsed_ms, input) in [
        ("idle", 12_000, None),
        ("a-down", 12_180, Some((37, false))),
        ("rtc", 12_700, Some((37, true))),
        ("rtc-tick", 14_000, None),
        ("b-down", 14_180, Some((39, false))),
        ("mic", 14_700, Some((39, true))),
    ] {
        if let Some((pin, high)) = input {
            machine.bus.gpio_set_input(pin, high);
            assert_eq!((machine.bus.gpio_input() >> pin) & 1, u64::from(high));
        }
        machine.max_cycles = elapsed_ms * (SECOND / 1000);
        let max_steps = (machine.max_cycles - machine.bus.cycles).div_ceil(2_000_000);
        for _ in 0..max_steps {
            let stop = machine.run(2_000_000);
            assert!(matches!(stop, esp32::Stop::MaxInsns | esp32::Stop::Halted), "{name}: {stop:?}");
            if machine.bus.cycles >= machine.max_cycles { break; }
        }
        machine.drain_console();
        assert!(machine.bus.cycles >= machine.max_cycles,
            "{name}: stopped at {} of {} cycles\n{}",
            machine.bus.cycles, machine.max_cycles, String::from_utf8_lossy(&machine.console.all));
        let (width, height, pixels, _) = machine.bus.board.display().unwrap();
        assert_eq!((width, height), (80, 160));
        let nonzero = pixels.iter().filter(|&&pixel| pixel != 0).count();
        let fingerprint = pixels.iter().flat_map(|pixel| pixel.to_le_bytes())
            .fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        println!("{name}: cycles={} frames={} nonzero={nonzero} fnv64={fingerprint:016x}",
            machine.bus.cycles, machine.bus.board.display_frames());
        machine.write_tft_png(output.join(format!("{name}.png")).to_str().unwrap(), 1).unwrap();
        captures.push(pixels);
        assert!(nonzero > 0, "{name}: LCD stayed blank");
    }

    let uart = String::from_utf8_lossy(&machine.console.all);
    std::fs::write(output.join("guest-uart.txt"), uart.as_bytes()).unwrap();
    assert!(uart.contains("SPI_FAST_FLASH_BOOT"), "ROM boot receipt missing");
    assert!(uart.contains("M5StickC initializing...OK"), "factory startup receipt missing");
    for fault in ["Guru Meditation", "StoreProhibited", "Brownout detector"] {
        assert!(!uart.contains(fault), "{fault}");
    }
    assert_ne!(captures[0], captures[2], "button A did not open the RTC page");
    assert_ne!(captures[2], captures[3], "RTC guest pixels did not advance");
    assert_ne!(captures[3], captures[5], "button B did not open the microphone page");
    println!("K016-C official ROM boot and A/B LCD transitions passed.");
}
