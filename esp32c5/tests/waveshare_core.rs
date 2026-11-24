//! Independent CPU/MMIO behavior, not a catalog-field assertion or host LED injection.
use emu_core::Bus;
use esp32c5::{Bringup, C5Bus, Stop};
use esp_soc::SocBus;

fn li(words: &mut Vec<u32>, register: u32, value: u32) {
    words.push((value.wrapping_add(0x800) & 0xfffff000) | (register << 7) | 0x37);
    words.push(((value & 0xfff) << 20) | (register << 15) | (register << 7) | 0x13);
}
fn write(words: &mut Vec<u32>, address: u32, value: u32) {
    li(words, 5, address); li(words, 6, value); words.push((6 << 20) | (5 << 15) | (2 << 12) | 0x23);
}
fn guest(m: &mut Bringup, words: &[u32]) {
    let mut bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    bytes.extend_from_slice(&0x10500073u32.to_le_bytes());
    m.bus.load_ram(0x40800000, &bytes, bytes.len()).unwrap(); m.cpu.pc = 0x40800000; m.cpu.waiting = false;
    assert_eq!(m.run(20_000).stop, Stop::Waiting, "C5 fault: {:?}", m.bus.first_fault); assert!(m.bus.first_fault.is_none());
}
fn rmt(m: &mut Bringup, pin: u32, bytes: [u8; 3], connected: bool) {
    let mut words = vec![];
    write(&mut words, 0x60090000 + pin * 4, if connected { 1 << 12 } else { 0 });
    write(&mut words, 0x60091ad4 + pin * 4, 64);
    for (i, bit) in bytes.into_iter().flat_map(|v| (0..8).map(move |b| v & (0x80 >> b) != 0)).enumerate() {
        write(&mut words, 0x60006400 + i as u32 * 4, if bit { 0x00148028 } else { 0x00288014 });
    }
    write(&mut words, 0x60006460, 0); write(&mut words, 0x60006044, 1); write(&mut words, 0x60006010, 0x00010101);
    // Poll actual TX completion before WFI; the CPU does not synthesize an LED callback.
    li(&mut words, 5, 0x60006038);
    words.extend([(5 << 15) | (2 << 12) | (6 << 7) | 3, (1 << 20) | (6 << 15) | (7 << 12) | (6 << 7) | 0x13, 0xfe030ce3]);
    guest(m, &words);
}

#[test]
fn real_c5_guest_rmt_and_boot_follow_exact_board_pins_and_color_order() {
    for (name, expected) in [
        ("waveshare-c5-zero", [0x34, 0x12, 0x56]), ("waveshare-c5-pico", [0x12, 0x34, 0x56]),
        ("waveshare-c5-mini-kit", [0x12, 0x34, 0x56]), ("waveshare-c5-wifi6-n16r8", [0x34, 0x12, 0x56]),
        ("waveshare-c5-wifi6-n32r8", [0x34, 0x12, 0x56]),
    ] {
        let mut m = Bringup::new(); m.bus.attach_board(esp32c5::board::make_board(name).unwrap());
        rmt(&mut m, 26, [0x12, 0x34, 0x56], true); assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]], "wrong pad {name}");
        rmt(&mut m, 27, [0x12, 0x34, 0x56], false); assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]], "unrouted IO_MUX {name}");
        rmt(&mut m, 27, [0x12, 0x34, 0x56], true); assert_eq!(m.bus.board.leds().unwrap().0, [expected], "{name}");
        for level in [true, false, true] {
            m.bus.gpio_set_input(28, level); let mut words = vec![];
            write(&mut words, 0x60090070, (1 << 12) | (1 << 9)); li(&mut words, 5, 0x60091064);
            words.push((5 << 15) | (2 << 12) | (10 << 7) | 3); guest(&mut m, &words);
            assert_eq!(m.cpu.x[10] & (1 << 28) != 0, level, "guest BOOT read {name}");
        }
        // MCU reboot keeps an externally powered WS2812 latch; a new platform board is dark.
        m.bus.reboot([0; 6]); assert_eq!(m.bus.board.leds().unwrap().0, [expected]);
        m.bus.attach_board(esp32c5::board::make_board(name).unwrap()); assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]]);
    }
}

fn put(bus: &mut C5Bus, address: u32, value: u32) { bus.write32(address, value).unwrap(); }
fn i2c_setup(bus: &mut C5Bus, sda: u32, scl: u32) {
    put(bus, 0x60096020, 3); put(bus, 0x60096020, 1); put(bus, 0x60096024, 1 << 22);
    for (pin, signal) in [(sda, 47), (scl, 46)] {
        put(bus, 0x60090000 + 4 * pin, 0x1a00); put(bus, 0x60091ad4 + 4 * pin, signal);
        put(bus, 0x60091038, 1 << pin); put(bus, 0x600912d4 + 4 * signal, 0x100 | pin);
    }
    put(bus, 0x60004000, 59); put(bus, 0x60004038, 60);
}
fn i2c(bus: &mut C5Bus, address: u8, register: u8, value: Option<u8>) -> Result<u8, ()> {
    const I: u32 = 0x60004000;
    put(bus, I + 0x24, 0x7ffff); put(bus, I + 0x18, 0x408b | (3 << 12));
    put(bus, I + 0x1c, u32::from(address) << 1); put(bus, I + 0x1c, register.into()); put(bus, I + 0x58, 6 << 11);
    if let Some(v) = value {
        put(bus, I + 0x1c, v.into()); put(bus, I + 0x5c, (1 << 11) | (1 << 8) | 3); put(bus, I + 0x60, 2 << 11);
    } else {
        put(bus, I + 0x1c, (u32::from(address) << 1) | 1); put(bus, I + 0x5c, (1 << 11) | (1 << 8) | 2);
        put(bus, I + 0x60, 6 << 11); put(bus, I + 0x64, (1 << 11) | (1 << 8) | 1);
        put(bus, I + 0x68, (3 << 11) | (1 << 10) | 1); put(bus, I + 0x6c, 2 << 11);
    }
    put(bus, I + 4, (1 << 4) | (1 << 5)); bus.advance(100_000);
    if bus.read32(I + 0x20).unwrap() & (1 << 10) != 0 { Err(()) } else { Ok(bus.read32(I + 0x1c).unwrap() as u8) }
}
#[test]
fn pico_expander_requires_real_i2c_pads_and_preserves_state_across_controller_reset() {
    let mut bus = C5Bus::new(); bus.attach_board(esp32c5::board::make_board("waveshare-c5-pico").unwrap());
    i2c_setup(&mut bus, 23, 24); assert_eq!(i2c(&mut bus, 0x20, 3, None), Ok(0xff));
    i2c(&mut bus, 0x20, 3, Some(0xc0)).unwrap(); i2c(&mut bus, 0x20, 1, Some(0x15)).unwrap();
    assert_eq!(i2c(&mut bus, 0x20, 0, None), Ok(0xd5));
    i2c_setup(&mut bus, 23, 24); assert_eq!(i2c(&mut bus, 0x20, 3, None), Ok(0xc0));
    assert!(i2c(&mut bus, 0x21, 0, None).is_err()); assert!(i2c(&mut bus, 0x20, 4, Some(0)).is_err());
    i2c_setup(&mut bus, 8, 9); assert!(i2c(&mut bus, 0x20, 0, None).is_err(), "display-board pins must not alias the Pico");
    for name in ["waveshare-c5-zero", "waveshare-c5-mini-kit", "waveshare-c5-wifi6-n16r8", "waveshare-c5-wifi6-n32r8"] {
        assert!(esp32c5::board::make_board(name).unwrap().i2c_devices().is_empty());
    }
}
