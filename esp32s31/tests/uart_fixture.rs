//! Real source-built SRAM fixture; no generated ELF/BIN is published.
#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_PADS_ELF to pads-probe.elf"]
fn external_s31_pads_fixture() {
    use esp32s31::pads::Drive;
    let path = std::env::var("S31_PADS_ELF").expect("build firmware/s31-uart-probe and set S31_PADS_ELF");
    let elf = std::fs::read(path).expect("read S31_PADS_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget);
    assert_eq!(m.bus.uart0.tx_out, b"S31 PAD HOLD READY\n");
    assert_eq!(m.bus.pads.drive(10), Ok(Drive::Low));
    m.bus.pads.set_input(2, Some(true)).unwrap(); m.bus.uart0.host_input(b"A");
    let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 PAD HOLD READY\nS31 PAD HOLD INPUT PASS\n");
    assert_eq!(m.bus.pads.drive(10), Ok(Drive::High)); assert!(m.bus.first_fault.is_none());
    eprintln!("S31 pad release steps={} pc={:#010x}", run.steps, m.cpu.pc);
    // Without the actual injected input, the guest takes its failure branch.
    let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget); m.bus.uart0.host_input(b"A");
    assert!(matches!(m.run(10_000).stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
    assert_eq!(m.bus.uart0.tx_out, b"S31 PAD HOLD READY\nS31 PAD FAIL\n");
}
#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_MMU_ELF and S31_FLASH_WORKER_BIN"]
fn external_s31_mmu_fixture() {
    let path = std::env::var("S31_MMU_ELF").expect("build firmware/s31-uart-probe and set S31_MMU_ELF");
    let flash = std::env::var("S31_FLASH_WORKER_BIN").expect("build firmware/s31-uart-probe and set S31_FLASH_WORKER_BIN");
    let elf = std::fs::read(path).expect("read S31_MMU_ELF");
    let flash = std::fs::read(flash).expect("read S31_FLASH_WORKER_BIN");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
    m.bus.cache.set_flash(&flash).unwrap();
    m.bus.cache.set_psram_size(16384).unwrap();
    let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 MMU FLASH PSRAM PASS\n"); assert!(m.bus.first_fault.is_none());
    eprintln!("S31 MMU fixture steps={} pc={:#010x}", run.steps, m.cpu.pc);
    // The same guest must fail at the real Flash fetch or PSRAM store when backing memory
    // is absent/too small; mapped addresses alone must not produce a successful fixture.
    for missing_flash in [false, true] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        if !missing_flash { m.bus.cache.set_flash(&flash).unwrap(); }
        m.bus.cache.set_psram_size(8192).unwrap();
        assert_eq!(m.run(10_000).stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(if missing_flash { 1 } else { 7 })));
        let f = m.bus.first_fault.as_ref().unwrap();
        assert_eq!(f.address, if missing_flash { 0x40000000 } else { 0x50000000 });
        assert_eq!(f.fetch, missing_flash); assert_eq!(f.write, !missing_flash);
        assert_eq!(f.reason, emu_core::Fault::Unmapped); assert!(m.bus.uart0.tx_out.is_empty());
    }
}
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

#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_WDT_ELF to wdt-probe.elf"]
fn external_s31_wdt_irq_and_reset_fixture() {
    let path = std::env::var("S31_WDT_ELF").expect("build firmware/s31-uart-probe and set S31_WDT_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).expect("read S31_WDT_ELF")).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 WDT READY\n");
    m.advance_rtc_ticks(7); assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 WDT READY\n");
    m.advance_rtc_ticks(1); assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 WDT READY\nS31 WDT IRQ PASS\n");
    m.advance_rtc_ticks(2); assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting);
    m.advance_rtc_ticks(1); assert_eq!(m.run(100).stop, esp32s31::Stop::Reset(esp32s31::rtc_wdt::ResetRequest::System));
    assert!(m.bus.first_fault.is_none());
}
