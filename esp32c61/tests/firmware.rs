//! Original UIFlow execution; the image is user-supplied and never patched.
use esp_soc::SocBus;
#[test]
#[ignore="requires ESP32SIM_COREMATRIX_FIRMWARE (official UIFlow 2.5.4 merged image) and ESP32SIM_ROM_DIR"]
fn external_corematrix_uiflow_rom_boot_and_guest_matrix_write() {
    let roms=std::path::PathBuf::from(std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the fetched ROM directory"));
    let firmware=std::env::var_os("ESP32SIM_COREMATRIX_FIRMWARE").expect("set ESP32SIM_COREMATRIX_FIRMWARE to the official CoreMatrix UIFlow 2.5.4 merged image");
    let mut m=esp32c61::machine([2,0,0,0,0,0x61],16<<20);m.console.capture=true;m.console.mask=0;m.dbg.stop_after_exceptions=1;
    m.bus.set_psram_size(8<<20).unwrap();m.bus.periph.usb.connected=true;
    m.bus.board=esp32c61::board::make_board("corematrix").unwrap();m.bus.attach_board_devices();
    m.load_rom(&std::fs::read(roms.join("esp32c61_rev100_rom.elf")).expect("read esp32c61_rev100_rom.elf")).unwrap();
    m.write_flash(0,&std::fs::read(firmware).expect("read original official CoreMatrix merged image")).unwrap();m.boot_rom();m.run(100_000_000);
    let text=String::from_utf8_lossy(&m.console.all);assert!(text.contains("ESP-ROM:esp32c61"),"{text}");assert!(text.contains("M5Stack CoreMatrix with ESP32-C61HR8"),"{text}");assert!(text.contains(">>>"),"{text}");assert!(!text.contains("ESP_ERR_FLASH_NOT_INITIALISED"),"{text}");
    let frames=m.bus.board.display_frames();assert!(frames>=1);assert!(m.bus.board.leds().unwrap().0.iter().all(|v|*v==[0;3]));
    m.bus.serial_input(&[1]);m.run(2_000_000);
    let program=b"import M5\nM5.Display.setBrightness(255)\nM5.Display.fillScreen(0xffffff)\nprint('NATIVE_C61_MATRIX',M5.Display.width(),M5.Display.height())\nfrom machine import PWM,Pin\npwm=PWM(Pin(25),freq=1000,duty_u16=16384)\nimport time;time.sleep_ms(3)\nprint('NATIVE_C61_PWM',pwm.freq(),pwm.duty_u16())\n";
    for chunk in program.chunks(64) {m.bus.serial_input(chunk);m.run(1_000_000);}
    m.bus.serial_input(&[4]);m.run(100_000_000);
    let text=String::from_utf8_lossy(&m.console.all);assert!(text.contains("NATIVE_C61_MATRIX 16 16"),"{text}");
    assert!(m.bus.board.display_frames()>frames);assert!(m.bus.board.leds().unwrap().0.iter().all(|v|*v==[255;3]));assert_eq!(m.exceptions,0,"{text}");
    let (hz,duty)=m.bus.pwm_output(25).expect("guest PWM must reach the physical GPIO25 output");assert!((hz-(80_000_000.0*256.0/(65536.0*313.0))).abs()<0.001,"PWM={hz}/{duty}, PCR={:x}, timer={:x}; {text}",m.bus.periph.read32(0x60096038),m.bus.periph.read32(0x600070a0));assert!(duty.abs_diff(16384)<3);assert!(text.contains("NATIVE_C61_PWM 998 16384"),"{text}");
    // The boot.py M5Things error in this distribution is retained in console;
    // reaching a working REPL and executing display code is a separate contract.
}
