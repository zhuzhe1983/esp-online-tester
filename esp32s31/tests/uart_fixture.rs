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

#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_UART_IRQ_ELF to irq-probe.elf"]
fn external_s31_uart_irq_fixture() {
    let path = std::env::var("S31_UART_IRQ_ELF").expect("build firmware/s31-uart-probe and set S31_UART_IRQ_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).expect("read S31_UART_IRQ_ELF")).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 IRQ READY\n");
    // Debugger-style threshold change: a pending UART source must not execute the ISR while
    // masked. Unmasking resumes the same guest, which validates mintstatus inside its handler.
    m.cpu.write_csr(riscv_rv32::state::csr::MINTTHRESH, 0xff);
    m.bus.uart0.host_input(b"A");
    assert_eq!(m.run(50).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 IRQ READY\n");
    m.cpu.write_csr(riscv_rv32::state::csr::MINTTHRESH, 0x1f);
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 IRQ READY\nA\nS31 IRQ MRET PASS\n");
    assert_eq!(m.cpu.mil, 0); assert!(m.bus.first_fault.is_none());
}

#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_TRNG_ELF to trng-probe.elf"]
fn external_s31_trng_gates_fixture() {
    let path = std::env::var("S31_TRNG_ELF").expect("build firmware/s31-uart-probe and set S31_TRNG_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).expect("read S31_TRNG_ELF")).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 TRNG GATES PASS\n"); assert!(m.bus.first_fault.is_none());
}
