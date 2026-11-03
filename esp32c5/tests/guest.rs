use emu_core::Bus;
use esp32c5::{Bringup, Stop};

#[test]
fn real_rv32_guest_uart_clic_controls_stamp_blue_led() {
    for dip in [false, true] {
        let mut m = Bringup::new(); m.bus.attach_board(Box::new(esp32c5::board::StampC5::new(dip)));
        m.load_elf(include_bytes!("../../firmware/c5-uart-probe/probe.elf")).unwrap();
        let boot = m.run(10_000); assert_eq!(boot.stop, Stop::Waiting);
        assert_eq!(m.bus.uart[0].tx_out, b"C5 UART IRQ READY\n");
        assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]]);
        // Standard C5 mintthresh masks the incoming UART source, without dropping data.
        m.cpu.mintthresh = 0xff; m.bus.uart[0].host_input(b"1");
        assert_eq!(m.run(100).stop, Stop::Waiting);
        assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]]);
        m.cpu.mintthresh = 0;
        let on = m.run(10_000); assert_eq!(on.stop, Stop::Waiting);
        assert_eq!(m.bus.board.leds().unwrap().0, [[0, 0, 255]]);
        m.bus.uart[0].host_input(b"0");
        let off = m.run(10_000); assert_eq!(off.stop, Stop::Waiting);
        assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]]);
        assert_eq!(m.bus.uart[0].tx_out, b"C5 UART IRQ READY\n10");
        assert_eq!(m.cpu.mil, 0); assert!(m.bus.first_fault.is_none());
        eprintln!("C5 dip={dip} boot={} on={} off={} UART={}B retired={} insns={} cycles={}", boot.steps, on.steps, off.steps, m.bus.uart[0].tx_out.len(), m.cpu.retired_count, m.cpu.insn_count, m.cpu.cycle_count);
    }
}

#[test]
fn actual_guest_rmt_symbols_follow_c5_matrix_and_complete() {
    use std::{cell::RefCell, rc::Rc};
    type Frames = Rc<RefCell<Vec<(u8, Vec<bool>)>>>;
    struct Capture(Frames);
    impl esp_soc::BoardModel for Capture {
        fn name(&self) -> &'static str { "explicit-rmt-fixture" }
        fn rmt_frame(&mut self, pin: u8, bits: &[bool]) { self.0.borrow_mut().push((pin, bits.to_vec())); }
    }
    let frames = Rc::new(RefCell::new(Vec::new())); let mut m = Bringup::new();
    m.bus.attach_board(Box::new(Capture(frames.clone())));
    m.load_elf(include_bytes!("../../firmware/c5-uart-probe/probe.elf")).unwrap(); assert_eq!(m.run(10_000).stop, Stop::Waiting);
    m.bus.uart[0].host_input(b"R"); let run = m.run(10_000); assert_eq!(run.stop, Stop::Waiting);
    assert_eq!(*frames.borrow(), vec![(5, vec![true, false])]); assert!(m.bus.first_fault.is_none());
    assert_eq!(m.bus.uart[0].tx_out, b"C5 UART IRQ READY\nR");
    eprintln!("C5 RMT steps={} retired={} insns={} cycles={} symbols=2 pin=5", run.steps, m.cpu.retired_count, m.cpu.insn_count, m.cpu.cycle_count);
}

#[test]
fn loader_validates_all_segments_and_entry_before_mutating() {
    let original = include_bytes!("../../firmware/c5-uart-probe/probe.elf"); let mut bad = original.to_vec();
    let mut m = Bringup::new(); m.bus.sram.fill(0xa5);
    bad[24..28].copy_from_slice(&0x4085_0000u32.to_le_bytes());
    assert!(m.load_elf(&bad).unwrap_err().contains("outside its loaded")); assert!(m.bus.sram.iter().all(|&b| b == 0xa5));
    bad = original.to_vec(); bad[18] = 0x5e; assert!(m.load_elf(&bad).is_err());
    assert!(m.bus.load_ram(0xffff_fff0, &[0; 32], 32).is_err());
}

#[test]
fn c5_map_rejects_c6_aliases_bounds_and_wrong_gpio_routes() {
    let mut m = Bringup::new(); m.bus.attach_board(Box::new(esp32c5::board::StampC5::new(false)));
    assert!(m.bus.read32(0x4086_0000).is_err(), "C5 has 384KiB, not C6 512KiB");
    assert!(m.bus.write32(0x2000_1000, 1).is_err(), "C6 PLIC is absent");
    assert!(m.bus.write32(0x6009_1554, 128).is_err(), "C6 output table is absent");
    assert!(m.bus.write32(0x6009_1024, 1 << 28).is_err(), "C6 enable register is absent");
    for (a, v) in [(0x6009_0074, 1 << 12), (0x6009_1b44, 256), (0x6009_1038, 1 << 28)] { m.bus.write32(a, v).unwrap(); }
    assert_eq!(m.bus.board.leds().unwrap().0, [[0, 0, 255]]);
    m.bus.write32(0x6009_0074, 0).unwrap(); assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]]);
    m.bus.write32(0x6009_0074, 1 << 12).unwrap();
    m.bus.write32(0x6009_103c, 1 << 28).unwrap(); assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]]);
    assert!(m.bus.read32(u32::MAX).is_err()); assert!(m.bus.write16(0x4085_ffff, 0).is_err());
}

#[test]
#[ignore = "set C5_ROM_ELF to public esp-rom-elfs20260528 esp32c5_rev100_rom.elf"]
fn external_c5_actual_rom_first_unsupported_access() {
    let p = std::env::var("C5_ROM_ELF").expect("set C5_ROM_ELF to public rev100 ROM ELF");
    let mut m = Bringup::new(); m.load_rom_elf(&std::fs::read(p).expect("read C5_ROM_ELF")).unwrap();
    let run = m.run(1_000_000);
    eprintln!("C5 ROM steps={} stop={:?} pc={:#x} fault={:?} UART={:?}", run.steps, run.stop, m.cpu.pc, m.bus.first_fault, String::from_utf8_lossy(&m.bus.uart[0].tx_out));
    assert_eq!(run.steps, 6575); assert_eq!(run.stop, Stop::Trap(riscv_rv32::Trap::Exception(5)));
    let f = m.bus.first_fault.unwrap(); assert_eq!((f.pc, f.address, f.write, f.width), (0x4002_69b6, 0x600b_2804, false, 4));
    assert!(m.bus.uart[0].tx_out.is_empty(), "No ROM boot banner or firmware execution is claimed");
}
