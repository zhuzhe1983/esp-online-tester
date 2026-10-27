//! Real source-built SRAM fixture; no generated ELF/BIN is published.
#[test]
#[ignore = "set S31_ROM_ELF to Espressif 20260528 esp32s31_rev0_rom.elf"]
fn external_s31_real_rom_unpacks_elf_initializers() {
    use emu_core::Bus;
    let rom = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to Espressif 20260528 esp32s31_rev0_rom.elf");
    let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&std::fs::read(rom).expect("read S31_ROM_ELF")).unwrap();
    assert_eq!(m.bus.read32(0x2f07fff4), Ok(0)); // Host did not initialize guest RAM.
    let mut steps = 0;
    while m.cpu.pc != 0x2f801852 && steps < 10_000 {
        assert_eq!(m.run(1).stop, esp32s31::Stop::Budget); steps += 1;
    }
    assert_eq!(m.cpu.pc, 0x2f801852); assert_eq!(m.bus.read32(0x2f07fff4), Ok(0x2f84882c));
    assert!(m.bus.first_fault.is_none()); eprintln!("S31 real ROM unpack loop completed in {steps} instructions");
}
#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_CLOCKS_ELF and S31_ROM_ELF (20260528 S31 ROM)"]
fn external_s31_rom_clock_and_uart_reset_fixture() {
    let path = std::env::var("S31_CLOCKS_ELF").expect("build firmware/s31-uart-probe and set S31_CLOCKS_ELF");
    let rom = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to Espressif 20260528 esp32s31_rev0_rom.elf");
    let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&std::fs::read(rom).expect("read S31_ROM_ELF")).unwrap();
    m.load_elf(&std::fs::read(path).expect("read S31_CLOCKS_ELF")).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget);
    assert_eq!(m.bus.uart0.tx_out, b"S31 ROM CLOCK20 READY\n"); assert_eq!(m.bus.clocks.rates().cpu_hz, 20_000_000);
    assert_eq!(m.bus.uart_baud(), Some(115273)); m.bus.uart_input(b"A").unwrap();
    let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 ROM CLOCK20 READY\nS31 UART RESET PASS\n");
    assert_eq!(m.bus.uart_baud(), Some(57636)); assert!(m.bus.first_fault.is_none());
    eprintln!("S31 ROM clock/UART reset fixture second-stage steps={} pc={:#010x}", run.steps, m.cpu.pc);
}
#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_EFUSE_ELF to efuse-probe.elf"]
fn external_s31_efuse_fixture() {
    let path = std::env::var("S31_EFUSE_ELF").expect("build firmware/s31-uart-probe and set S31_EFUSE_ELF");
    let elf = std::fs::read(path).expect("read S31_EFUSE_ELF");
    let mut image = esp32s31::efuse::ReadImage::blank_development();
    image.words[2] = 1 << 19; image.words[23] = 0x5a17f00d; image.block0_valid_bits = 7;
    let mut m = esp32s31::Bringup::new(); m.bus.efuse.set_read_image(image).unwrap(); m.load_elf(&elf).unwrap();
    let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 EFUSE READ PASS\n"); assert_eq!(m.bus.rtc_wdt.delay_sel, 1);
    assert!(m.bus.first_fault.is_none()); eprintln!("S31 eFuse fixture steps={} pc={:#010x}", run.steps, m.cpu.pc);
    let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
    let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!(f.address, 0x207151d8); assert!(f.write);
    assert!(m.bus.uart0.tx_out.is_empty());
}
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
    m.bus.pads.set_input(2, Some(true)).unwrap(); m.bus.uart_input(b"A").unwrap();
    let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 PAD HOLD READY\nS31 PAD HOLD INPUT PASS\n");
    assert_eq!(m.bus.pads.drive(10), Ok(Drive::High)); assert!(m.bus.first_fault.is_none());
    eprintln!("S31 pad release steps={} pc={:#010x}", run.steps, m.cpu.pc);
    // Without the actual injected input, the guest takes its failure branch.
    let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget); m.bus.uart_input(b"A").unwrap();
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
    m.bus.uart_input(b"A").unwrap(); assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
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
    m.bus.uart_input(b"A").unwrap();
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

#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_USB_ELF to usb-probe.elf"]
fn external_s31_usb_packets_and_irq_fixture() {
    let path = std::env::var("S31_USB_ELF").expect("build firmware/s31-uart-probe and set S31_USB_ELF");
    let elf = std::fs::read(path).expect("read S31_USB_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget);
    assert_eq!(m.bus.uart0.tx_out, b"S31 USB FULL WAIT\n");
    assert_eq!(m.bus.usb_console.host_receive(), None);
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget); // No host: no fake progress.
    assert_eq!(m.bus.uart0.tx_out, b"S31 USB FULL WAIT\n");
    m.bus.usb_console.attach_host(true);
    assert_eq!(m.bus.usb_console.host_receive(), Some((0..64).collect()));
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget);
    assert_eq!(m.bus.usb_console.host_receive(), Some(vec![])); // Guest WR_DONE ZLP.
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 USB FULL WAIT\nS31 USB RX READY\n");
    m.cpu.write_csr(riscv_rv32::state::csr::MINTTHRESH, 0xff);
    m.bus.usb_console.host_send(b"A").unwrap();
    assert_eq!(m.run(50).stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.usb_console.host_receive(), None);
    m.cpu.write_csr(riscv_rv32::state::csr::MINTTHRESH, 0x1f);
    let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.usb_console.host_receive(), Some(b"A".to_vec()));
    assert_eq!(m.bus.uart0.tx_out, b"S31 USB FULL WAIT\nS31 USB RX READY\nS31 USB IRQ ECHO PASS\n");
    assert_eq!(m.cpu.mil, 0); assert!(m.bus.first_fault.is_none());
    eprintln!("S31 USB IRQ guest steps={} pc={:#010x}", run.steps, m.cpu.pc);
}

#[test]
#[ignore = "set S31_ROM_ELF to the fixed public 20260528 esp32s31_rev0_rom.elf"]
fn external_s31_real_rom_usb_and_flash_boot_banner() {
    let path = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to the fixed public 20260528 ROM ELF");
    let elf = std::fs::read(path).expect("read S31_ROM_ELF");
    for (strap, mode) in [(0, "USB_BOOT"), (0x12, "SPI_FAST_FLASH_BOOT")] {
        let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&elf).unwrap();
        m.bus.pads.strap = strap; m.bus.usb_console.attach_host(true);
        let mut steps = 0; let mut packets = Vec::new();
        let stop = loop {
            let run = m.run(1000); steps += run.steps;
            if let Some(packet) = m.bus.usb_console.host_receive() { packets.push(packet); }
            if run.stop != esp32s31::Stop::Budget { break run.stop; }
            assert!(steps < 100_000, "real ROM failed to reach the documented first unsupported access");
        };
        let uart = format!("ESP-ROM:esp32s31-20251218\r\nBuild:Dec 18 2025\r\nrst:0x1 (POWERON),boot:{strap:#x} ({mode})\r\n");
        assert_eq!(m.bus.uart0.tx_out, uart.as_bytes());
        assert_eq!(packets.len(), 3); assert_eq!(packets.concat(), uart.as_bytes());
        assert_eq!(stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(5)));
        let f = m.bus.first_fault.as_ref().unwrap();
        let expected = if strap == 0 { (0x2f8152d4, 0x205870ac) } else { (0x2f80ebf6, 0x20500028) };
        assert_eq!((f.pc, f.address), expected);
        eprintln!("S31 real ROM strap={strap:#x} steps={steps} UART+USB={}B", uart.len());
    }
}

#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_MONITOR_ELF to monitor-probe.elf"]
fn external_s31_monitor_capture_irq_and_actual_faults() {
    let path = std::env::var("S31_MONITOR_ELF").expect("build firmware/s31-uart-probe and set S31_MONITOR_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).expect("read S31_MONITOR_ELF")).unwrap();
    for cause in [5, 7] {
        let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(cause)));
        assert_eq!(m.cpu.mtval, 0); assert!(m.bus.uart0.tx_out.is_empty());
        eprintln!("S31 monitor actual trap cause={cause} steps={} mepc={:#010x}", run.steps, m.cpu.mepc);
        // Resume at the architectural vector the guest itself installed; never set pc,
        // mepc, cause, capture registers or ISR flags from the host.
    }
    let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 MONITOR CAPTURE IRQ TRAPS PASS\n");
    assert_eq!(m.cpu.x[18], 1); assert_eq!(m.cpu.x[19], 2); assert_eq!(m.cpu.mil, 0);
    eprintln!("S31 monitor final steps={} pc={:#010x}", run.steps, m.cpu.pc);
}

#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_MSPI_ELF to mspi-probe.elf"]
fn external_s31_mspi_real_array_and_missing_input_controls() {
    let path = std::env::var("S31_MSPI_ELF").expect("build firmware/s31-uart-probe and set S31_MSPI_ELF");
    let elf = std::fs::read(path).expect("read S31_MSPI_ELF");
    for control in ["valid", "wrong-data", "no-identity", "no-array", "clock-off"] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        if control != "no-identity" { m.bus.mspi.attach_flash([0xef, 0x40, 0x18], 0).unwrap(); }
        let mut bytes = vec![0xff; 0x100]; bytes[0x20..0x28].copy_from_slice(&[0x5a, 0x3c, 0x29, 0x17, 0x21, 0x43, 0x65, 0x87]);
        if control == "wrong-data" { bytes[0x20] ^= 1; }
        if control != "no-array" { m.bus.cache.set_flash(&bytes).unwrap(); }
        if control == "clock-off" { m.bus.mspi.clock_write(0x680).unwrap(); }
        let run = m.run(10_000);
        if control == "valid" {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 MSPI ID STATUS ARRAY PASS\n");
            assert_eq!(m.bus.mspi.commands, 7); assert!(m.bus.first_fault.is_none());
        } else if control == "wrong-data" {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
            assert_eq!(m.bus.uart0.tx_out, b"S31 MSPI FAIL\n"); assert_eq!(m.bus.mspi.commands, 6);
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
            assert_eq!(m.bus.first_fault.as_ref().unwrap().address, 0x20501000);
            assert_eq!(m.bus.first_fault.as_ref().unwrap().reason, emu_core::Fault::Prohibited);
            assert_eq!(m.bus.mspi.commands, 0); assert!(m.bus.uart0.tx_out.is_empty());
        }
        eprintln!("S31 MSPI control={control} steps={} stop={:?}", run.steps, run.stop);
    }
}

#[test]
#[ignore = "build firmware/s31-uart-probe and set S31_CACHE_ELF to cache-probe.elf"]
fn external_s31_cache_power_sync_freeze_and_missing_input() {
    let path = std::env::var("S31_CACHE_ELF").expect("build firmware/s31-uart-probe and set S31_CACHE_ELF");
    let elf = std::fs::read(path).expect("read S31_CACHE_ELF");
    for control in ["valid", "wrong-data", "no-array", "clock-off"] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        if control != "no-array" { m.bus.cache.set_flash(&[if control == "wrong-data" { 0 } else { 0x5a }, 0x3c, 0x29, 0x17]).unwrap(); }
        if control == "clock-off" { m.bus.cache.clock_write(0x14249); }
        let run = m.run(10_000);
        if control == "valid" {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 CACHE POWER SYNC FREEZE PASS\n");
            assert_eq!(m.bus.cache.maintenance_operations, 2); assert!(m.bus.first_fault.is_none());
        } else if control == "wrong-data" {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
            assert_eq!(m.bus.uart0.tx_out, b"S31 CACHE FAIL\n");
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(5)));
            assert_eq!(m.bus.first_fault.as_ref().unwrap().address, 0x40000000); assert!(m.bus.uart0.tx_out.is_empty());
        }
        eprintln!("S31 CACHE control={control} steps={} stop={:?}", run.steps, run.stop);
    }
}

#[test]
#[ignore = "set S31_ROM_ELF to the fixed public 20260528 ROM ELF"]
fn external_s31_rom_legacy_opt_in_still_requires_flash_input() {
    let path = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to the fixed public 20260528 ROM ELF");
    let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&std::fs::read(path).expect("read S31_ROM_ELF")).unwrap();
    m.bus.pads.strap = 0x12; m.bus.usb_console.attach_host(true); m.bus.mspi.enable_legacy_rom_lengths();
    let mut steps = 0;
    loop {
        let run = m.run(1000); steps += run.steps; m.bus.usb_console.host_receive();
        if run.stop != esp32s31::Stop::Budget {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7))); break;
        }
        assert!(steps < 100_000);
    }
    let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.pc, f.address), (0x2f80e43a, 0x20501000));
    assert_eq!(f.reason, emu_core::Fault::Prohibited); assert_eq!(m.bus.mspi.commands, 0);
    assert_eq!(m.bus.cache.maintenance_operations, 2);
    eprintln!("S31 ROM explicit shadow without NOR/array steps={steps} commands=0");
}

#[test]
#[ignore = "build firmware/s31-uart-probe with S31_BUILD_ROM_IMAGE=1; set S31_ROM_ELF and S31_BOOT_IMAGE"]
fn external_s31_real_rom_sha_boot_and_corruption_controls() {
    let path = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to public20260528 ROM ELF");
    let rom = std::fs::read(path).expect("read S31_ROM_ELF");
    let path = std::env::var("S31_BOOT_IMAGE").expect("set S31_BOOT_IMAGE to source-built spi0-observation.bin");
    let image = std::fs::read(path).expect("read S31_BOOT_IMAGE");
    assert!(image.len() > 64 && image.len() < 0xe000 && image[0] == 0xe9 && image[23] == 1);
    let digest = image[image.len() - 32..].iter().map(|b| format!("{b:02x}")).collect::<String>();
    for control in ["valid", "digest-flip", "payload-flip"] {
        let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&rom).unwrap();
        m.bus.pads.strap = 0x12; m.bus.usb_console.attach_host(true); m.bus.mspi.enable_legacy_rom_lengths();
        m.bus.mspi.attach_flash([0xef, 0x40, 0x18], 0).unwrap();
        let mut flash = vec![0xff; 65536]; flash[0x2000..0x2000 + image.len()].copy_from_slice(&image);
        if control == "digest-flip" { flash[0x2000 + image.len() - 1] ^= 1; }
        if control == "payload-flip" { flash[0x2020] ^= 1; }
        m.bus.cache.set_flash(&flash).unwrap(); let mut steps = 0;
        let stop = loop {
            let run = m.run(1000); steps += run.steps; m.bus.usb_console.host_receive();
            if run.stop != esp32s31::Stop::Budget || steps >= 200_000 { break run.stop; }
        };
        let uart = String::from_utf8(m.bus.uart0.tx_out.clone()).unwrap();
        assert!(m.bus.first_fault.is_none());
        if control == "payload-flip" {
            assert_eq!(stop, esp32s31::Stop::Budget); assert!(uart.contains("Checksum failure."));
            assert!(!uart.contains("entry 0x2f000000")); assert!(!uart.contains("SPI0 OBSERVATION DONE"));
        } else {
            assert_eq!(stop, esp32s31::Stop::Waiting); assert!(uart.contains("entry 0x2f000000"));
            assert!(uart.contains("SPI0 AFTER\na55a03ff\na55a03ff\nSPI0 OBSERVATION DONE\n"));
            assert_eq!(m.bus.sha.blocks(), 5);
            if control == "digest-flip" {
                assert!(uart.contains("SHA-256 comparison failed:")); assert!(uart.contains(&format!("Calculated: {digest}")));
                assert!(uart.contains("Attempting to boot anyway...")); // Actual unsecured ROM behavior, not a host override.
            } else { assert!(!uart.contains("failed")); }
        }
        eprintln!("S31 ROM SHA control={control} steps={steps} stop={stop:?} blocks={}", m.bus.sha.blocks());
    }
}
