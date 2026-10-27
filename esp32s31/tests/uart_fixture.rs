//! Real source-built SRAM fixture; no generated ELF/BIN is published.
#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_UART_ELF to its ELF"]
fn external_s31_uart_fixture() {
    let path = std::env::var("S31_UART_ELF").expect("build firmware/s31-uart-probe and set S31_UART_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).expect("read S31_UART_ELF")).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget);
    assert_eq!(m.bus.uart0.tx_out, b"S31 UART READY\n");
    m.bus.uart0.host_input(b"A"); assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 UART READY\nA\nS31 SRAM FPU PASS\n");
    assert!(m.bus.first_fault.is_none());
}
