use emu_core::Bus;
use esp_periph::{rtc_cntl::RtcCntl, sar_adc::{voltage_code, Calibration}, AnalogSource};
use esp_soc::SocBus;

fn check(bus: &mut impl SocBus, pin: u8, (control, select): (u32, u32), result: u32, done: u32, clear: Option<u32>, expected_voltage: u32) {
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 0);
    assert!(bus.adc_observation(99).is_none());
    assert!(!bus.adc_set_raw(99, 1));
    assert!(bus.adc_set_raw(pin, 1234));
    assert!(!bus.adc_set_raw(pin, 4096));
    let start = if clear.is_some() { 1 << 29 } else { 1 << 17 };
    bus.write32(control, select).unwrap();
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 0);
    bus.write32(control, select | start).unwrap();
    assert_eq!(bus.read32(result).unwrap() & 4095, 1234);
    assert_ne!(bus.read32(done).unwrap(), 0);
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 1);
    assert!(bus.adc_set_raw(pin, 3000));
    bus.write32(control, select | start).unwrap();
    assert_eq!(bus.read32(result).unwrap() & 4095, 1234);
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 1);
    if let Some(clear) = clear {
        bus.write32(result, 0).unwrap();
        assert_eq!(bus.read32(result).unwrap() & 4095, 1234);
        bus.write32(clear, u32::MAX).unwrap();
        assert_eq!(bus.read32(done).unwrap(), 0);
    }
    bus.write32(control, select).unwrap();
    bus.write32(control, select | start).unwrap();
    assert_eq!(bus.adc_observation(pin).unwrap().raw, 3000);
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 2);
    bus.analog_set(pin, AnalogSource::Const(0.5));
    bus.write32(control, select).unwrap();
    bus.write32(control, select | start).unwrap();
    assert_eq!(bus.read32(result).unwrap() & 4095, expected_voltage);
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 3);
    bus.reboot([0; 6]);
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 3);
    bus.write32(control, select).unwrap();
    bus.write32(control, select | start).unwrap();
    assert_eq!(bus.adc_observation(pin).unwrap().raw, expected_voltage as u16);
    assert_eq!(bus.adc_observation(pin).unwrap().generation, 4);
}

#[test]
fn s3_adc1_and_adc2_host_contract() {
    for (pin, off, expected) in [
        (1, 0x0c, RtcCntl::s3_adc_code(0.5, 0)),
        (20, 0x30, voltage_code(0.5, 0, Calibration::S3Adc2)),
    ] {
        let mut m = esp32s3::machine([0; 6]);
        check(&mut m.bus, pin, (0x60008800 + off, 1 << (19 + (pin - 1) % 10)), 0x60008800 + off, 0x60008800 + off, None, expected);
    }
}

#[test]
fn c3_adc1_adc2_and_c6_adc1_host_contract() {
    for (pin, channel) in [(4, 4), (5, 8)] {
        let mut m = esp32c3::machine([0; 6], 4 << 20);
        let unit = channel >> 3;
        check(&mut m.bus, pin, (0x60040020, (channel << 25) | (1 << (31 - unit))), 0x6004002c + unit * 4, 0x60040044, Some(0x6004004c), voltage_code(0.5, 0, Calibration::C3));
    }
    let mut m = esp32c6::machine([0; 6], 4 << 20);
    check(&mut m.bus, 6, (0x6000e020, (6 << 25) | (1 << 31)), 0x6000e02c, 0x6000e044, Some(0x6000e04c), voltage_code(0.5, 0, Calibration::C6));
    assert!(!m.bus.adc_set_raw(7, 1));
    let before = m.bus.adc_observation(6);
    for selection in [(6 << 25), (7 << 25) | (1 << 31), (8 << 25) | (1 << 30)] {
        m.bus.write32(0x6000e020, selection).unwrap();
        m.bus.write32(0x6000e020, selection | (1 << 29)).unwrap();
        assert_eq!(m.bus.adc_observation(6), before);
    }
}

#[test]
fn voltage_inverse_covers_attenuation_and_input_bounds() {
    use esp_periph::sar_adc::millivolts;
    for c in [Calibration::S3Adc1, Calibration::S3Adc2, Calibration::C3, Calibration::C6] {
        for atten in 0..4 {
            for mv in [100, 300, 500] {
                let raw = voltage_code(mv as f32 / 1000.0, atten, c);
                assert!(millivolts(raw, atten, c).abs_diff(mv) <= 1);
            }
            assert_eq!(voltage_code(-1.0, atten, c), 0);
            assert_eq!(voltage_code(f32::NAN, atten, c), 0);
            assert!(voltage_code(f32::INFINITY, atten, c) <= 4095);
        }
    }
}

fn serial_until<S: esp_soc::Soc>(m: &mut esp_soc::Machine<S>, marker: &str) -> String {
    for _ in 0..500 {
        m.run(1_000_000);
        let text = String::from_utf8_lossy(&m.console.uart0);
        if text.contains(marker) { return text.into_owned(); }
    }
    panic!("serial marker {marker:?} missing; {}", String::from_utf8_lossy(&m.console.uart0));
}

fn firmware<S: esp_soc::Soc>(mut m: esp_soc::Machine<S>, chip: &str, pins: &[u8], expected: &[[(u16, i32); 4]]) {
    use std::{fs, path::PathBuf};
    let build = PathBuf::from(std::env::var("ADC_FIRMWARE_DIR").expect(
        "set ADC_FIRMWARE_DIR to Arduino 3.3.8 builds with s3/c3/c6 subdirectories; see EX207 receipt",
    )).join(chip);
    // ADC_ROM_DIR is the name the EX207 receipt uses; without it the ROM is found like everywhere else.
    let rom = |file: String| std::env::var_os("ADC_ROM_DIR").map(|d| PathBuf::from(d).join(&file)).unwrap_or_else(|| esp_soc::rom::require(&file));
    let read = |path: PathBuf| fs::read(&path).unwrap_or_else(|e| panic!("required ADC test input {}: {e}", path.display()));
    m.console.capture = true;
    m.console.mask = 2;
    let rev = if chip == "c3" { 3 } else { 0 };
    m.load_rom(&read(rom(format!("esp32{chip}_rev{rev}_rom.elf")))).unwrap();
    m.write_flash(0, &read(build.join("bootloader.bin"))).unwrap();
    m.write_flash(0x8000, &read(build.join("partitions.bin"))).unwrap();
    m.write_flash(0x10000, &read(build.join("firmware.bin"))).unwrap();
    m.boot_rom();
    serial_until(&mut m, "ADC READY");
    for (phase, command) in (b'A'..=b'D').enumerate() {
        let before: Vec<_> = pins.iter().map(|&pin| m.bus.adc_observation(pin).unwrap().generation).collect();
        for &pin in pins {
            match phase {
                0 => m.bus.analog_set(pin, AnalogSource::Const(0.5)),
                1 => assert!(m.bus.adc_set_raw(pin, 1024)),
                2 => m.bus.analog_set(pin, AnalogSource::Const(1.0)),
                _ => assert!(m.bus.adc_set_raw(pin, 3072)),
            }
        }
        m.console.uart0.clear();
        m.bus.uart_input(0, &[command]);
        let output = serial_until(&mut m, "ADC DONE");
        println!("{output}");
        for (i, &pin) in pins.iter().enumerate() {
            let observed = m.bus.adc_observation(pin).unwrap();
            println!("{chip} phase={phase} pin={pin} generation={} raw={}", observed.generation, observed.raw);
            let (raw, mv) = expected[i][phase];
            let line = format!("ADC {} pin={pin} raw={raw} mv={mv}", command as char);
            assert!(output.contains(&line), "expected {line}");
            assert_eq!(observed.raw, raw);
            assert_eq!(observed.generation - before[i], 2, "one analogRead and one analogReadMilliVolts");
        }
    }
    println!("{chip} work insns={} cycles={}", m.insns(), m.bus.cycles());
}

#[test]
#[ignore = "set ADC_FIRMWARE_DIR to the Arduino build directory and ESP32SIM_ROM_DIR (or ADC_ROM_DIR) to the S3 rev0, C3 rev3 and C6 rev0 ROM ELFs"]
fn external_arduino_s3() {
    firmware(esp32s3::machine([0; 6]), "s3", &[1, 11], &[[(521, 500), (1024, 960), (1068, 1000), (3072, 2765)], [(528, 500), (1024, 944), (1087, 1000), (3072, 2718)]]);
}
#[test]
#[ignore = "set ADC_FIRMWARE_DIR to the Arduino build directory and ESP32SIM_ROM_DIR (or ADC_ROM_DIR) to the S3 rev0, C3 rev3 and C6 rev0 ROM ELFs"]
fn external_arduino_c3() {
    firmware(esp32c3::machine([0; 6], 4 << 20), "c3", &[0], &[[(722, 500), (1024, 706), (1459, 1000), (3072, 2081)]]);
}
#[test]
#[ignore = "set ADC_FIRMWARE_DIR to the Arduino build directory and ESP32SIM_ROM_DIR (or ADC_ROM_DIR) to the S3 rev0, C3 rev3 and C6 rev0 ROM ELFs"]
fn external_arduino_c6() {
    firmware(esp32c6::machine([0; 6], 4 << 20), "c6", &[0], &[[(508, 500), (1024, 1008), (1016, 1000), (3072, 3016)]]);
}
