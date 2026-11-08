use emu_core::Bus;
use esp_soc::{
    uart::{UartInput, UartRoute},
    BoardModel, SocBus,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct State {
    tx: Vec<(UartRoute, u8)>,
    rx: Vec<UartInput>,
    tx_cycles: Vec<u64>,
    rx_cycles: Vec<u64>,
}
struct Endpoint(Rc<RefCell<State>>);
impl BoardModel for Endpoint {
    fn name(&self) -> &'static str {
        "uart-test"
    }
    fn uses_gpio_edges(&self) -> bool { false }
    fn uses_uart_pins(&self) -> bool { true }
    fn uart_tx(&mut self, cycle: u64, route: UartRoute, byte: u8) {
        let mut state = self.0.borrow_mut();
        state.tx_cycles.push(cycle);
        state.tx.push((route, byte));
        if route.transmits_on(5) && route.matches_baud(9600) {
            state.rx.push(UartInput::new(4, 9600, vec![byte]));
        }
    }
    fn uart_rx(&mut self, cycle: u64) -> Vec<UartInput> {
        let mut state = self.0.borrow_mut();
        state.rx_cycles.push(cycle);
        std::mem::take(&mut state.rx)
    }
}

macro_rules! check_chip {
    ($name:ident, $machine:expr, $uart:expr, $gpio:expr, $mux:expr, $clock:expr, $signal:expr, $input_select:expr, $native_tx:expr, $native_rx:expr, $native_func:expr, $rc_div:expr, $rc_baud:expr) => {
        #[test]
        fn $name() {
            let mut machine = $machine;
            let state = Rc::new(RefCell::new(State::default()));
            machine.bus.board = Box::new(Endpoint(state.clone()));
            machine.bus.attach_board_devices();
            let bus = &mut machine.bus;
            for reset in 0..2 {
                // Serial1 at 9600: XTAL / 2 / (2083 + 5/16).
                bus.write32($clock, (3 << 20) | (1 << 12)).unwrap();
                bus.write32($uart + 0x14, 2083 | (5 << 20)).unwrap();
                bus.write32($gpio + 0x24, 1 << 5).unwrap();
                bus.write32($mux + 4 + 5 * 4, 1 << 12).unwrap();
                bus.write32($mux + 4 + 4 * 4, 1 << 9).unwrap();
                bus.write32($gpio + 0x554 + 5 * 4, $signal).unwrap();
                bus.write32($gpio + 0x154 + $signal * 4, $input_select | 4).unwrap();
                bus.write32($uart + 0x24, 1).unwrap();
                bus.write32($uart + 0xc, 1).unwrap();
                let cycle = bus.cycles();
                bus.write32($uart, (b'A' + reset) as u32).unwrap();
                // Changing pins before the tick must not relabel an earlier byte.
                bus.write32($gpio + 0x554 + 5 * 4, 0x100).unwrap();
                bus.tick(256);
                assert_eq!(bus.read32($uart + 0x1c).unwrap() & 0xff, 1, "one byte received (a register read also flushes the S3's deferred ticks)");
                assert!(bus.periph.uart[1].irq());
                assert_eq!(bus.read32($uart).unwrap(), (b'A' + reset) as u32);
                assert!(!bus.periph.uart[1].irq(), "RXFIFO_FULL follows the FIFO level: the read that drains it lowers it");
                bus.write32($uart + 0x10, u32::MAX).unwrap();
                assert!(!bus.periph.uart[1].irq());
                assert_eq!(*state.borrow().tx_cycles.last().unwrap(), cycle);
                assert_eq!(*state.borrow().rx_cycles.last().unwrap(), bus.cycles());
                let route = state.borrow().tx.last().unwrap().0;
                assert_eq!(route.port, 1);
                assert_eq!(route.rx_pin, Some(4));
                assert!(route.transmits_on(5) && route.matches_baud(9600));
                assert!(!route.transmits_on(7));
                assert!(!route.transmits_on(255));
                assert!(!route.matches_baud(0) && !route.matches_baud(19200));
                assert_eq!(bus.console_take()[2], vec![b'A' + reset]);
                bus.reboot([0; 6]);
                assert_eq!(bus.periph.uart_route(1).tx_pins, 0);
            }
            assert_eq!(state.borrow().tx.len(), 2);
            // Matrix gates must fail independently of output function selection.
            bus.write32($clock, 3 << 20 | 1 << 12).unwrap();
            bus.write32($uart + 0x14, 2083 | 5 << 20).unwrap();
            bus.write32($mux + 4 + 4 * 4, 1 << 12 | 1 << 9).unwrap();
            bus.write32($gpio + 0x154 + $signal * 4, $input_select | 4).unwrap();
            assert_eq!(bus.periph.uart_route(1).rx_pin, Some(4));
            bus.write32($mux + 4 + 4 * 4, 1 << 12).unwrap();
            assert_eq!(bus.periph.uart_route(1).rx_pin, None);
            bus.write32($mux + 4 + 5 * 4, 1 << 12).unwrap();
            let oen = if $signal == 15 { 1 << 10 } else { 1 << 9 };
            bus.write32($gpio + 0x554 + 5 * 4, $signal | oen).unwrap();
            bus.write32($gpio + 0x24, 1 << 5).unwrap();
            assert!(bus.periph.uart_route(1).transmits_on(5));
            bus.write32($gpio + 0x28, 1 << 5).unwrap();
            assert!(!bus.periph.uart_route(1).transmits_on(5));
            // GPIO_ENABLE also gates a matrix route using the peripheral OEN source.
            bus.write32($gpio + 0x554 + 5 * 4, $signal).unwrap();
            assert!(!bus.periph.uart_route(1).transmits_on(5));
            let route = bus.periph.uart_route(1);
            assert!(route.matches_baud(9600));
            assert!(route.matches_baud(9800));
            assert!(!route.matches_baud(10000));
            bus.write32($mux + 4 + 4 * 4, 1 << 9).unwrap();
            bus.periph.uart_pin_input(&UartInput::new(4, 10000, vec![42]));
            assert_ne!(bus.periph.uart[1].int_raw & esp_periph::uart::INT_FRM_ERR, 0);
            assert_eq!(bus.periph.uart[1].rx_pending(), 0);
            bus.write32($uart + 0x10, u32::MAX).unwrap();
            // RC_FAST / fractional divider gives approximately 115200 baud.
            bus.write32($clock, 2 << 20).unwrap();
            bus.write32($uart + 0x14, $rc_div).unwrap();
            assert_eq!(bus.periph.uart_route(1).baud, Some($rc_baud));
            bus.periph.uart_pin_input(&UartInput::new(4, 115200, vec![42]));
            assert_eq!(bus.periph.uart[1].int_raw & esp_periph::uart::INT_FRM_ERR, 0);
            assert_eq!(bus.read32($uart).unwrap(), 42);
            // IO_MUX direct routing on UART0, independent of matrix output selection.
            bus.write32($gpio + 0x24, 1u32.wrapping_shl($native_tx))
                .unwrap();
            if $native_tx >= 32 {
                bus.write32($gpio + 0x30, 1 << ($native_tx.wrapping_sub(32)))
                    .unwrap();
            }
            bus.write32($mux + 4 + $native_tx * 4, $native_func << 12)
                .unwrap();
            bus.write32($mux + 4 + $native_rx * 4, ($native_func << 12) | (1 << 9))
                .unwrap();
            let route = bus.periph.uart_route(0);
            assert!(route.transmits_on($native_tx as u8));
            assert_eq!(route.rx_pin, Some($native_rx as u8));
            // A direct RX route cannot override a selected matrix constant or inversion.
            bus.write32($gpio + 0x154 + ($signal - 3) * 4, $input_select | ($input_select / 2 - 1))
                .unwrap();
            assert_eq!(bus.periph.uart_route(0).rx_pin, None);

            // Mismatched device input raises FRM_ERR; unrelated pins and invalid pins do not.
            bus.write32($clock, 3 << 20).unwrap();
            bus.write32($uart + 0x14, 2083 | (5 << 20)).unwrap();
            bus.write32($mux + 4 + 4 * 4, 1 << 9).unwrap();
            bus.write32($gpio + 0x154 + $signal * 4, $input_select | 4).unwrap();
            for pin in [7, 255] {
                bus.periph.uart_pin_input(&UartInput::new(pin, 9600, vec![1]));
            }
            assert_eq!(bus.periph.uart[1].int_raw & (1 << 3), 0);
            bus.periph.uart_pin_input(&UartInput::new(4, 9600, vec![1]));
            assert_ne!(bus.periph.uart[1].int_raw & (1 << 3), 0);
            assert_eq!(bus.periph.uart[1].rx_pending(), 0);
            bus.periph.uart_pin_input(&UartInput::new(4, 19200, vec![2; 129]));
            assert_eq!(bus.periph.uart[1].rx_pending(), 128);
            assert_ne!(bus.periph.uart[1].int_raw & (1 << 4), 0);
        }
    };
}
check_chip!(
    s3,
    esp32s3::machine([0; 6]),
    0x6001_0000,
    0x6000_4000,
    0x6000_9000,
    0x6001_0078,
    15,
    0x80,
    43u32,
    44u32,
    0,
    151 | 14 << 20,
    115226
);
check_chip!(
    c3,
    esp32c3::machine([0; 6], 4 * 1024 * 1024),
    0x6001_0000,
    0x6000_4000,
    0x6000_9000,
    0x6001_0078,
    9,
    0x40,
    21u32,
    20u32,
    0,
    151 | 14 << 20,
    115226
);
check_chip!(
    c6,
    esp32c6::machine([0; 6], 4 * 1024 * 1024),
    0x6000_1000,
    0x6009_1000,
    0x6009_0000,
    0x6009_6010,
    9,
    0x80,
    16u32,
    17u32,
    0,
    173 | 10 << 20,
    115190
);

#[test]
fn uart_source_and_fractional_dividers() {
    let mut uart = esp_periph::Uart::new(esp_periph::UartLayout::S3);
    assert_eq!(uart.baud(3 << 20, 8_000_000), None);
    uart.write(0x14, 100 | (8 << 20));
    assert_eq!(uart.baud(0, 8_000_000), None);
    assert_eq!(uart.baud(1 << 20, 8_000_000), Some(796019));
    assert_eq!(uart.baud(2 << 20, 8_000_000), Some(79601));
    assert_eq!(uart.baud(2 << 20, 20_000_000), Some(199004));
    // XTAL / (2 + 1/3) / (100 + 8/16), preserving the fractional source divider.
    assert_eq!(
        uart.baud((3 << 20) | (1 << 12) | (1 << 6) | 3, 8_000_000),
        Some(170575)
    );
}

#[test]
fn s3_uart2_receive_has_an_interrupt_source() {
    let mut machine = esp32s3::machine([0; 6]);
    let p = &mut machine.bus.periph;
    p.uart[2].write(0x78, (3 << 20) | (1 << 12));
    p.uart[2].write(0x14, 2083 | (5 << 20));
    p.uart[2].write(0xc, 1);
    p.io_mux.write(4 + 4 * 4, 1 << 9);
    p.gpio.write(0x154 + 4 * 18, 0x80 | 4);
    p.uart_pin_input(&UartInput::new(4, 9600, vec![42]));
    assert_ne!(p.source_status()[0] & (1 << 29), 0);
    assert_eq!(p.uart[2].read(0), 42);
}

#[test]
fn c3_matrix_enable_and_inversion_bits_match_silicon() {
    let mut gpio = esp_periph::Gpio::new();
    let mut mux = esp_periph::RegRam::new();
    mux.write(4 + 4 * 4, 1 << 9);
    mux.write(4 + 5 * 4, 1 << 12);
    gpio.enable = 1 << 5;
    gpio.write(0x154 + 4 * 9, 0x44);
    // C3 bit 9 selects GPIO output enable, not signal inversion.
    gpio.func_out_sel[5] = 9 | (1 << 9);
    let route = esp_soc::uart::UartPins::C3.route(1, &gpio, &mux, Some(9600));
    assert_eq!(route.rx_pin, Some(4));
    assert!(route.transmits_on(5));
    gpio.func_in_sel[9] |= 1 << 5;
    gpio.func_out_sel[5] |= 1 << 8;
    let inverted = esp_soc::uart::UartPins::C3.route(1, &gpio, &mux, Some(9600));
    assert_eq!(inverted.rx_pin, None);
    assert!(!inverted.transmits_on(5));
}

#[test]
fn s3_gpio46_is_a_uart_output() {
    let mut m = esp32s3::machine([0; 6]);
    m.bus.write32(0x6000_90bc, 1 << 12).unwrap();
    m.bus.write32(0x6000_460c, 15).unwrap();
    m.bus.write32(0x6000_4030, 1 << 14).unwrap();
    assert!(m.bus.periph.uart_route(1).transmits_on(46));
}

#[test]
fn boards_without_uart_opt_in_keep_console_only() {
    struct ConsoleOnly(Rc<std::cell::Cell<usize>>);
    impl BoardModel for ConsoleOnly {
        fn name(&self) -> &'static str { "console-only" }
        fn uses_uart_pins(&self) -> bool { self.0.set(self.0.get() + 1); false }
        fn uart_tx(&mut self, _: u64, _: UartRoute, _: u8) { panic!("TX without opt-in"); }
        fn uart_rx(&mut self, _: u64) -> Vec<UartInput> { panic!("RX without opt-in"); }
    }
    macro_rules! check {
        ($machine:expr) => {{
            let mut m = $machine;
            let queries = Rc::new(std::cell::Cell::new(0));
            m.bus.board = Box::new(ConsoleOnly(queries.clone()));
            m.bus.attach_board_devices();
            m.bus.write32(0x6000_0000, 42).unwrap();
            m.bus.tick(256);
            assert_eq!(m.bus.console_take()[1], [42]);
            assert_eq!(queries.get(), 1, "capability is cached until reattachment");
        }};
    }
    check!(esp32s3::machine([0; 6]));
    check!(esp32c3::machine([0; 6], 4 << 20));
    check!(esp32c6::machine([0; 6], 4 << 20));
}
