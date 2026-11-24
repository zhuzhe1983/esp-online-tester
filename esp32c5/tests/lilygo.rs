//! Exercise the documented C5 GPIO register map and real LCD protocol, including
//! inactive chip-select, disabled output, reset and the cropped ST7735 glass.
use emu_core::Bus;
use esp_soc::SocBus;

fn bus(name: &str) -> esp32c5::C5Bus {
    let mut b = esp32c5::C5Bus::new();
    b.attach_board(esp32c5::board::make_board(name).unwrap()); b
}
fn output(b: &mut esp32c5::C5Bus, pin: u8, level: bool) {
    b.write32(0x6009_0000 + 4 * pin as u32, 1 << 12).unwrap();
    b.write32(0x6009_1ad4 + 4 * pin as u32, 256).unwrap();
    level_pin(b, pin, level);
    b.write32(0x6009_1038, 1 << pin).unwrap();
}
fn level_pin(b: &mut esp32c5::C5Bus, pin: u8, level: bool) {
    b.write32(if level { 0x6009_1008 } else { 0x6009_100c }, 1 << pin).unwrap();
}
fn byte(b: &mut esp32c5::C5Bus, value: u8) {
    for shift in (0..8).rev() {
        level_pin(b, 6, false); level_pin(b, 2, value & (1 << shift) != 0); level_pin(b, 6, true);
    }
    level_pin(b, 6, false);
}
fn command(b: &mut esp32c5::C5Bus, value: u8, data: &[u8]) {
    level_pin(b, 3, false); byte(b, value); level_pin(b, 3, true);
    for &v in data { byte(b, v); }
}
fn init(b: &mut esp32c5::C5Bus) {
    for (pin, level) in [(0, false), (1, true), (2, false), (3, false), (6, false), (10, false)] { output(b, pin, level); }
    command(b, 0x11, &[]); command(b, 0x29, &[]); command(b, 0x3a, &[0x05]); command(b, 0x36, &[0x08]);
    command(b, 0x2a, &[0, 26, 0, 26]); command(b, 0x2b, &[0, 1, 0, 1]);
}
fn first(b: &esp32c5::C5Bus) -> u16 { let (w,h,p,_) = b.board.display().unwrap(); assert_eq!((w,h),(80,160)); p[0] }

#[test]
fn t7_c5_boot_is_a_real_active_low_input_and_has_no_screen_or_led() {
    let mut b = bus("lilygo-t7-c5");
    assert_eq!(b.board.named_pin("boot"), Some(28));
    b.write32(0x6009_0070, (1 << 12) | (1 << 9)).unwrap();
    assert_ne!(b.read32(0x6009_1064).unwrap() & (1 << 28), 0);
    b.gpio_set_input(28, false); assert_eq!(b.read32(0x6009_1064).unwrap() & (1 << 28), 0);
    b.gpio_set_input(28, true); assert_ne!(b.read32(0x6009_1064).unwrap() & (1 << 28), 0);
    assert!(b.board.display().is_none()); assert!(b.board.leds().is_none());
}

#[test]
fn dongle_software_spi_obeys_select_routes_backlight_sleep_and_reset() {
    let mut b = bus("lilygo-t-dongle-c5"); init(&mut b);
    command(&mut b, 0x2c, &[0xf8, 0x00]); assert_eq!(first(&b), 0xf800);
    let generation = b.board.display_version();
    level_pin(&mut b, 10, true); command(&mut b, 0x2c, &[0x07, 0xe0]); assert_eq!(first(&b), 0xf800, "unselected LCD cannot consume another device's traffic");
    level_pin(&mut b, 10, false); b.write32(0x6009_1aec, 0).unwrap(); // G6 routed away from software GPIO.
    command(&mut b, 0x2c, &[0x07, 0xe0]); assert_eq!(first(&b), 0xf800);
    output(&mut b, 6, false); command(&mut b, 0x2c, &[0x07, 0xe0]); assert_eq!(first(&b), 0x07e0);
    level_pin(&mut b, 0, true); assert_eq!(first(&b), 0, "G0 is active-low backlight");
    level_pin(&mut b, 0, false); assert_eq!(first(&b), 0x07e0);
    command(&mut b, 0x10, &[]); assert_eq!(first(&b), 0); command(&mut b, 0x11, &[]); assert_eq!(first(&b), 0x07e0);
    command(&mut b, 0x28, &[]); assert_eq!(first(&b), 0); command(&mut b, 0x29, &[]); assert_eq!(first(&b), 0x07e0);
    level_pin(&mut b, 1, false); command(&mut b, 0x2c, &[0, 0x1f]); assert_eq!(first(&b), 0);
    level_pin(&mut b, 1, true); assert_eq!(first(&b), 0, "reset stays asleep/off until guest commands");
    command(&mut b, 0x11, &[]); command(&mut b, 0x29, &[]); command(&mut b, 0x36, &[0x08]); assert_eq!(first(&b), 0x07e0, "hardware reset retains GRAM");
    assert!(b.board.display_version() > generation); assert!(b.first_fault.is_none());
}

#[test]
fn dongle_rotation_and_full_visible_window_match_the_official_offsets() {
    let mut b = bus("lilygo-t-dongle-c5"); init(&mut b);
    command(&mut b, 0x36, &[0x68]); // official landscape rotation1 (MX|MV|BGR)
    command(&mut b, 0x2a, &[0, 1, 0, 1]); command(&mut b, 0x2b, &[0, 26, 0, 26]); command(&mut b, 0x2c, &[0, 0x1f]);
    let (_,_,pixels,_) = b.board.display().unwrap(); assert_eq!(pixels[79], 0x001f); assert_eq!(pixels.iter().filter(|&&x|x!=0).count(), 1);
    // Off-screen writes remain in controller RAM, not wrapped onto the glass.
    command(&mut b, 0x36, &[0x08]); command(&mut b, 0x2a, &[0,0,0,0]); command(&mut b, 0x2b, &[0,0,0,0]); command(&mut b, 0x2c, &[0xff,0xff]);
    assert_eq!(b.board.display().unwrap().2.iter().filter(|&&x|x!=0).count(), 1);
}
