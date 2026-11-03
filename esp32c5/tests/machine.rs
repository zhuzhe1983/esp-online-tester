use esp_soc::{LoadKind, Soc, SocBus};
use emu_core::Bus;
const APP: &[u8] = include_bytes!("../../firmware/c5-uart-probe/probe.bin");
fn slice(m: &mut esp32c5::Machine) {
    m.max_cycles = m.bus.cycles() + 2000;
    let stop = m.run(u64::MAX); assert!(matches!(stop, esp_soc::Stop::MaxInsns | esp_soc::Stop::Halted), "{stop:?}");
    assert_eq!(m.exceptions, 0); assert!(m.bus.first_fault.is_none());
}
#[test]
fn espressif_app_image_uses_real_shared_machine_irq_gpio_and_restart() {
    for name in ["stamp-c5", "stamp-c5-dip"] {
        let mut m = esp32c5::machine([0; 6], 4 << 20); m.console.capture = true; m.console.mask = 2;
        m.bus.attach_board(esp32c5::board::make_board(name).unwrap());
        m.load_input(LoadKind::App, APP).unwrap(); assert_eq!(m.boot_app(0x10000).unwrap(), 0x4080_0000);
        slice(&mut m); assert_eq!(m.console.uart0, b"C5 UART IRQ READY\n");
        for (input, expected) in [(b"1", [0, 0, 255]), (b"0", [0; 3]), (b"W", [0; 3])] {
            m.bus.uart_input(0, input); slice(&mut m); assert_eq!(m.bus.board_ref().leds().unwrap().0, [expected]);
        }
        assert_eq!(m.console.uart0, b"C5 UART IRQ READY\n10W"); assert_eq!(m.interrupts, 3);
        assert!(!m.bus.gpio_state(27).unwrap().output); assert!(m.bus.gpio_state(27).unwrap().output_enable);
        m.bus.uart_input(0, b"1"); slice(&mut m); assert_eq!(m.bus.board_ref().leds().unwrap().0, [[0, 0, 255]]);
        // Host reset from an explicit app-direct session clears the digital state,
        // turns the lamp off and recopies this app; it never pretends to execute ROM.
        m.bus.request_reset(3); m.reboot(); assert_eq!(m.bus.board_ref().leds().unwrap().0, [[0; 3]]);
        assert_eq!(m.cores[0].pc, 0x4080_0000); slice(&mut m);
        assert!(m.console.uart0.ends_with(b"C5 UART IRQ READY\n")); assert_eq!(m.bus.board_ref().leds().unwrap().0, [[0; 3]]);
        m.bus.uart_input(0, b"1"); slice(&mut m); assert_eq!(m.bus.board_ref().leds().unwrap().0, [[0, 0, 255]]);
        eprintln!("C5 Machine {name}: cycles={} slots={} retired={} IRQ={} resets={} UART={}B", m.bus.cycles(), m.insns(), m.cores[0].retired_count, m.interrupts, m.reboots, m.console.uart0.len());
    }
}
#[test]
fn sram_app_contract_rejects_wrong_chip_flash_segments_integrity_and_revision() {
    assert!(esp32c5::C5::check_image(APP).is_ok());
    for chip in [0x05, 0x0d, 0x12] { let mut bad = APP.to_vec(); bad[12] = chip; assert!(esp32c5::C5::check_image(&bad).unwrap_err().contains("chip ID")); }
    let mut flash = APP.to_vec(); flash[24..28].copy_from_slice(&0x4200_0020u32.to_le_bytes());
    assert!(esp32c5::C5::check_image(&flash).unwrap_err().contains("SRAM segments"));
    let mut bad = APP.to_vec(); bad[15..17].copy_from_slice(&200u16.to_le_bytes()); assert!(esp32c5::C5::check_image(&bad).unwrap_err().contains("revision100"));
    let mut bad = APP.to_vec(); bad[32] ^= 1; assert!(esp32c5::C5::check_image(&bad).unwrap_err().contains("checksum"));
    let mut bad = APP.to_vec(); let last = bad.len() - 1; bad[last] ^= 1; assert!(esp32c5::C5::check_image(&bad).unwrap_err().contains("SHA-256"));
    let mut m = esp32c5::machine([0; 6], 4 << 20); m.bus.sram.fill(0xa5);
    m.bus.write_flash(0x10000, &flash).unwrap(); assert!(m.boot_app(0x10000).is_err()); assert!(m.bus.sram.iter().all(|&b| b == 0xa5));
    assert!(m.bus.set_psram_size(4 << 20).is_err()); assert!(m.bus.write_flash(usize::MAX, &[1]).is_err());
    assert!(m.bus.read32(0x4200_0000).is_err());
}

#[test]
fn app_header_padding_alignment_and_trailing_data_are_bounded() {
    let parsed = esp_soc::image::parse(APP).unwrap();
    let end = parsed.segments.last().map(|s| (s.file_off + s.len) as usize).unwrap(); let check = end / 16 * 16 + 15;
    let mut unsigned = APP[..=check].to_vec(); unsigned[23] = 0;
    assert!(esp32c5::image::parse(&unsigned).is_ok());
    unsigned[14] = 255; assert!(esp32c5::image::parse(&unsigned).is_ok(), "C5 ignores the deprecated legacy revision byte");
    unsigned.extend_from_slice(&[0xff; 16]); assert!(esp32c5::image::parse(&unsigned).is_ok());
    let last = unsigned.len() - 1; unsigned[last] = 0; assert!(esp32c5::image::parse(&unsigned).err().unwrap().contains("after its image footer"));
    for count in [0, 17, 255] { let mut bad = APP.to_vec(); bad[1] = count; assert!(esp32c5::image::parse(&bad).is_err()); }
    let mut bad = APP.to_vec(); bad[24] |= 2; assert!(esp32c5::image::parse(&bad).err().unwrap().contains("aligned"));
    let mut bad = APP.to_vec(); bad[4] |= 1; assert!(esp32c5::image::parse(&bad).err().unwrap().contains("entry"));
    let mut bad = APP.to_vec(); bad[end] = 1; assert!(esp32c5::image::parse(&bad).err().unwrap().contains("padding"));
    for len in [0, 23, 24, end, check] { assert!(esp32c5::image::parse(&APP[..len]).is_err()); }
}
