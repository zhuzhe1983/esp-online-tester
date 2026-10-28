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
#[ignore = "build examples/s31-bringup and set S31_CLOCKS_ELF and S31_ROM_ELF (20260528 S31 ROM)"]
fn external_s31_rom_clock_and_uart_reset_fixture() {
    let path = std::env::var("S31_CLOCKS_ELF").expect("build examples/s31-bringup and set S31_CLOCKS_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_EFUSE_ELF to efuse-probe.elf"]
fn external_s31_efuse_fixture() {
    let path = std::env::var("S31_EFUSE_ELF").expect("build examples/s31-bringup and set S31_EFUSE_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_PADS_ELF to pads-probe.elf"]
fn external_s31_pads_fixture() {
    use esp32s31::pads::Drive;
    let path = std::env::var("S31_PADS_ELF").expect("build examples/s31-bringup and set S31_PADS_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_MMU_ELF and S31_FLASH_WORKER_BIN"]
fn external_s31_mmu_fixture() {
    let path = std::env::var("S31_MMU_ELF").expect("build examples/s31-bringup and set S31_MMU_ELF");
    let flash = std::env::var("S31_FLASH_WORKER_BIN").expect("build examples/s31-bringup and set S31_FLASH_WORKER_BIN");
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
#[ignore = "build examples/s31-bringup and set S31_UART_ELF to its ELF"]
fn external_s31_uart_fixture() {
    let path = std::env::var("S31_UART_ELF").expect("build examples/s31-bringup and set S31_UART_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).expect("read S31_UART_ELF")).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Budget);
    assert_eq!(m.bus.uart0.tx_out, b"S31 UART READY\n");
    m.bus.uart_input(b"A").unwrap(); assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 UART READY\nA\nS31 SRAM FPU PASS\n");
    assert!(m.bus.first_fault.is_none());
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_UART_IRQ_ELF to irq-probe.elf"]
fn external_s31_uart_irq_fixture() {
    let path = std::env::var("S31_UART_IRQ_ELF").expect("build examples/s31-bringup and set S31_UART_IRQ_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_TRNG_ELF to trng-probe.elf"]
fn external_s31_trng_gates_fixture() {
    let path = std::env::var("S31_TRNG_ELF").expect("build examples/s31-bringup and set S31_TRNG_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).expect("read S31_TRNG_ELF")).unwrap();
    assert_eq!(m.run(10_000).stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 TRNG GATES PASS\n"); assert!(m.bus.first_fault.is_none());
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_WDT_ELF to wdt-probe.elf"]
fn external_s31_wdt_irq_and_reset_fixture() {
    let path = std::env::var("S31_WDT_ELF").expect("build examples/s31-bringup and set S31_WDT_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_USB_ELF to usb-probe.elf"]
fn external_s31_usb_packets_and_irq_fixture() {
    let path = std::env::var("S31_USB_ELF").expect("build examples/s31-bringup and set S31_USB_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_MONITOR_ELF to monitor-probe.elf"]
fn external_s31_monitor_capture_irq_and_actual_faults() {
    let path = std::env::var("S31_MONITOR_ELF").expect("build examples/s31-bringup and set S31_MONITOR_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_MSPI_ELF to mspi-probe.elf"]
fn external_s31_mspi_real_array_and_missing_input_controls() {
    let path = std::env::var("S31_MSPI_ELF").expect("build examples/s31-bringup and set S31_MSPI_ELF");
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
#[ignore = "build examples/s31-bringup and set S31_CACHE_ELF to cache-probe.elf"]
fn external_s31_cache_power_sync_freeze_and_missing_input() {
    let path = std::env::var("S31_CACHE_ELF").expect("build examples/s31-bringup and set S31_CACHE_ELF");
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
#[ignore = "build examples/s31-bringup with S31_BUILD_ROM_IMAGE=1; set S31_ROM_ELF and S31_BOOT_IMAGE"]
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

#[test]
#[ignore = "build examples/s31-bringup and set S31_ANALOG_ELF to analog-probe.elf"]
fn external_s31_analog_data_route_reset_and_missing_seed() {
    let path = std::env::var("S31_ANALOG_ELF").expect("build examples/s31-bringup and set S31_ANALOG_ELF");
    let elf = std::fs::read(path).expect("read S31_ANALOG_ELF");
    for control in ["valid", "wrong-data", "no-seed", "clock-off"] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        if control != "no-seed" { m.bus.analog.set_bias([if control == "wrong-data" { 0 } else { 0x17 }, 0x29]); }
        if control == "clock-off" { m.bus.clocks.write(0x40, 0).unwrap(); }
        let run = m.run(10_000);
        if control == "valid" {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert!(m.bus.first_fault.is_none());
            assert_eq!(m.bus.uart0.tx_out, b"S31 ANALOG ROUTE RESET MPLL PASS\n");
            assert_eq!(m.bus.analog.bias(), Some([0xa7, 0x2a])); assert_eq!(m.bus.analog.transactions, 8);
            assert!(m.bus.power.mpll_disabled());
        } else if control == "wrong-data" {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
            assert_eq!(m.bus.uart0.tx_out, b"S31 ANALOG FAIL\n"); assert_eq!(m.bus.analog.transactions, 1);
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
            let fault = m.bus.first_fault.as_ref().unwrap(); assert_eq!(fault.reason, emu_core::Fault::Prohibited);
            assert_eq!(fault.address, if control == "no-seed" { 0x2010f804 } else { 0x2010f018 });
            assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.bus.analog.transactions, 0);
        }
        eprintln!("S31 analog control={control} steps={} stop={:?} transactions={}", run.steps, run.stop, m.bus.analog.transactions);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_POWER_ELF to power-probe.elf"]
fn external_s31_perif_reset_and_qualified_brownout_input() {
    let path = std::env::var("S31_POWER_ELF").expect("build examples/s31-bringup and set S31_POWER_ELF");
    let elf = std::fs::read(path).expect("read S31_POWER_ELF");
    for control in ["enabled", "disabled", "wrong-data", "no-seed"] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        let enable = if control == "disabled" { 0u32 } else { 1 << 31 };
        m.bus.load(0x2f070000, &enable.to_le_bytes(), 4).unwrap();
        if control != "no-seed" { m.bus.analog.set_perif_glitch(if control == "wrong-data" { 0 } else { 0xf7 }); }
        let run = m.run(10_000);
        if control == "enabled" || control == "disabled" {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert!(m.bus.first_fault.is_none());
            assert_eq!(m.bus.uart0.tx_out, b"S31 PERIF RESET BOD ARMED\n");
            assert_eq!(m.bus.analog.perif_glitch(), Some(0xf7)); assert_eq!(m.bus.analog.transactions, 4);
            m.bus.power.brownout_input(false).unwrap(); assert_eq!(m.run(10).stop, esp32s31::Stop::Waiting);
            let pc = m.cpu.pc; m.bus.power.brownout_input(true).unwrap(); let after = m.run(10);
            assert_eq!(after.steps, 0); assert_eq!(m.cpu.pc, pc);
            assert_eq!(after.stop, if control == "enabled" { esp32s31::Stop::PowerReset(esp32s31::power::ResetCause::Brownout) } else { esp32s31::Stop::Waiting });
        } else if control == "wrong-data" {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
            assert_eq!(m.bus.uart0.tx_out, b"S31 POWER FAIL\n"); assert_eq!(m.bus.analog.transactions, 1);
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
            let fault = m.bus.first_fault.as_ref().unwrap(); assert_eq!(fault.address, 0x2010f804);
            assert_eq!(fault.reason, emu_core::Fault::Prohibited); assert_eq!(m.bus.analog.transactions, 0);
        }
        eprintln!("S31 power control={control} steps={} stop={:?} transactions={} reset={:?}", run.steps, run.stop, m.bus.analog.transactions, m.bus.power.reset_request);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_PMS_ELF to pms-probe.elf"]
fn external_s31_permissions_control_actual_uart_io() {
    let path = std::env::var("S31_PMS_ELF").expect("build examples/s31-bringup and set S31_PMS_ELF");
    let elf = std::fs::read(path).expect("read S31_PMS_ELF");
    for (control, bits, error, exception) in [("allow", 0x111u32, 1u32, 0), ("write-denied", 0x101, 1, 7), ("read-denied", 0x110, 1, 5), ("quiet-denial-unsupported", 0x101, 0, 7)] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        m.bus.load(0x2f070000, &bits.to_le_bytes(), 4).unwrap(); m.bus.load(0x2f070004, &error.to_le_bytes(), 4).unwrap();
        let run = m.run(1000);
        if exception == 0 { assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"AB"); }
        else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(exception)));
            assert_eq!(m.bus.uart0.tx_out, b"A");
            let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!(f.reason, emu_core::Fault::Prohibited);
            assert_eq!(f.address, if error == 0 { 0x2058a028 } else if exception == 5 { 0x2038a01c } else { 0x2038a000 });
        }
        eprintln!("S31 PMS control={control} steps={} stop={:?} writes={}", run.steps, run.stop, m.bus.permissions.writes);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_CPLL_ELF and S31_ROM_ELF (20260528 S31 ROM)"]
fn external_s31_cpll_dig_data_and_real_rom_frequency() {
    let path = std::env::var("S31_CPLL_ELF").expect("build examples/s31-bringup and set S31_CPLL_ELF");
    let rom = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to the fixed 20260528 S31 ROM");
    let elf = std::fs::read(path).expect("read S31_CPLL_ELF"); let rom = std::fs::read(rom).expect("read S31_ROM_ELF");
    for control in 0u32..=6 {
        let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&rom).unwrap(); m.load_elf(&elf).unwrap();
        m.bus.load(0x2f070000, &control.to_le_bytes(), 4).unwrap();
        if control != 6 { m.bus.analog.set_dig_reg([0x11, 0x22, if control == 5 { 0 } else { 0xff }, 0x44]); }
        let run = m.run(10_000);
        match control {
            0 => {
                assert_eq!(run.stop, esp32s31::Stop::Waiting); assert!(m.bus.first_fault.is_none());
                assert_eq!(m.bus.uart0.tx_out, b"S31 ROM CPLL80 DIG PASS\n");
                assert_eq!(m.bus.clocks.rates(), esp32s31::clocks::Rates { cpu_hz: 80_000_000, mem_hz: 80_000_000, sys_hz: 80_000_000, apb_hz: 40_000_000 });
                assert_eq!(m.bus.analog.dig_reg(), Some([0x11, 0x22, 0xf3, 0x44])); assert_eq!(m.bus.analog.transactions, 3);
                assert_eq!(m.bus.clocks.rtc_fast_hz(), 0); assert!(!m.bus.power.fosc_enabled());
            }
            1..=3 => {
                assert_eq!(run.stop, esp32s31::Stop::Waiting); assert!(m.bus.first_fault.is_none());
                assert_eq!(m.bus.uart0.tx_out, b"S31 CPLL NOT READY\n"); assert_eq!(m.bus.clocks.rates().cpu_hz, 40_000_000);
            }
            5 => {
                assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
                assert_eq!(m.bus.uart0.tx_out, b"S31 CPLL FAIL\n"); assert_eq!(m.bus.analog.transactions, 1);
            }
            _ => {
                assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
                let fault = m.bus.first_fault.as_ref().unwrap(); assert_eq!(fault.reason, emu_core::Fault::Prohibited);
                assert_eq!(fault.address, if control == 4 { 0x207040f4 } else { 0x2010f804 });
                assert!(m.bus.uart0.tx_out.is_empty());
                if control == 4 { assert!(m.bus.power.cpll_enabled()); assert_eq!(m.bus.clocks.rates().cpu_hz, 80_000_000); }
            }
        }
        eprintln!("S31 CPLL control={control} steps={} stop={:?} rates={:?} transactions={}", run.steps, run.stop, m.bus.clocks.rates(), m.bus.analog.transactions);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_FLASH_PARAMS_ELF to flash-params-probe.elf"]
fn external_s31_flash_parameters_clock_and_idle_resume() {
    let path = std::env::var("S31_FLASH_PARAMS_ELF").expect("build examples/s31-bringup and set S31_FLASH_PARAMS_ELF");
    let elf = std::fs::read(path).expect("read S31_FLASH_PARAMS_ELF");
    for control in ["valid", "wrong-data", "no-sfdp", "no-bbpll", "clock-gated", "other-manufacturer"] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        m.bus.cache.set_flash(&[0xa5; 128]).unwrap();
        m.bus.mspi.attach_flash([if control == "other-manufacturer" { 0x20 } else { 0xef }, 0x40, 0x18], 2).unwrap();
        if control != "no-sfdp" { m.bus.mspi.attach_sfdp(0x10, &[if control == "wrong-data" { 0 } else { 0x12 }, 0x34, 0x56, 0x78]).unwrap(); }
        if control != "no-bbpll" { m.bus.mspi.set_bbpll_input(Some(480_000_000)).unwrap(); }
        m.bus.load(0x2f070000, &u32::from(control == "clock-gated").to_le_bytes(), 4).unwrap();
        let run = m.run(10_000);
        match control {
            "valid" => {
                assert_eq!(run.stop, esp32s31::Stop::Waiting); assert!(m.bus.first_fault.is_none());
                assert_eq!(m.bus.uart0.tx_out, b"S31 SFDP FLASH80 IDLE PASS\n");
                assert_eq!(m.bus.mspi.commands, 3); assert_eq!(m.bus.mspi.core_hz(), Some(80_000_000));
                assert_eq!(m.bus.mspi.read(1, 0x58), Ok(0x7856));
            }
            "wrong-data" => {
                assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
                assert_eq!(m.bus.uart0.tx_out, b"S31 FLASH PARAMS FAIL\n"); assert_eq!(m.bus.mspi.commands, 1);
            }
            _ => {
                assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
                let fault = m.bus.first_fault.as_ref().unwrap(); assert_eq!(fault.reason, emu_core::Fault::Prohibited);
                assert_eq!(fault.address, if control == "no-bbpll" { 0x20587064 } else { 0x20501000 });
                assert_eq!(m.bus.mspi.commands, if control == "no-sfdp" { 0 } else { 2 });
                assert!(m.bus.uart0.tx_out.is_empty());
                if control == "clock-gated" { assert_eq!(m.bus.mspi.core_hz(), None); assert!(!m.bus.power.bbpll_output_enabled()); }
            }
        }
        eprintln!("S31 flash-params control={control} steps={} stop={:?} commands={} core-hz={:?}", run.steps, run.stop, m.bus.mspi.commands, m.bus.mspi.core_hz());
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_RESET_ELF to reset-probe.elf"]
fn external_s31_reset_gates_and_explicit_lockup_event() {
    let path = std::env::var("S31_RESET_ELF").expect("build examples/s31-bringup and set S31_RESET_ELF");
    let elf = std::fs::read(path).expect("read S31_RESET_ELF");
    for mode in 0u32..8 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        let run = m.run(10_000); assert_eq!(m.bus.uart0.tx_out, b"S31 RESET READY\n"); assert!(m.bus.first_fault.is_none());
        if mode == 4 || mode == 5 {
            let mut cores = [None, None]; cores[(mode & 1) as usize] = Some(esp32s31::reset::Cause::Software);
            assert_eq!(run.stop, esp32s31::Stop::CpuReset(esp32s31::reset::CpuReset { cores }));
            assert_eq!(m.bus.reset.read(0x2070103c + (mode & 1) * 4).unwrap() & (1 << 20), 0);
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Waiting);
            if mode < 4 {
                m.advance_rtc_ticks(1); assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting);
                m.advance_rtc_ticks(1);
                if mode == 0 {
                    assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting); m.advance_rtc_ticks(3);
                    assert_eq!(m.run(100).stop, esp32s31::Stop::Reset(esp32s31::rtc_wdt::ResetRequest::System));
                } else { assert_eq!(m.run(100).stop, esp32s31::Stop::Reset(esp32s31::rtc_wdt::ResetRequest::Cpu { core0: mode & 1 != 0, core1: mode & 2 != 0 })); }
            } else {
                let core = (mode & 1) as usize; m.bus.reset.lockup_input(core, false).unwrap(); m.bus.reset.lockup_input(core ^ 1, true).unwrap();
                assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting);
                m.bus.reset.lockup_input(core, true).unwrap(); let mut cores = [None, None]; cores[core] = Some(esp32s31::reset::Cause::Lockup);
                assert_eq!(m.run(100).stop, esp32s31::Stop::CpuReset(esp32s31::reset::CpuReset { cores }));
            }
        }
        assert_eq!(m.bus.uart0.tx_out, b"S31 RESET READY\n");
        eprintln!("S31 reset mode={mode} initial-steps={} initial-stop={:?}", run.steps, run.stop);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_MWDT_ELF to mwdt-probe.elf"]
fn external_s31_mwdt_actual_guest_isr_and_clock_gates() {
    let path = std::env::var("S31_MWDT_ELF").expect("build examples/s31-bringup and set S31_MWDT_ELF");
    let elf = std::fs::read(path).expect("read S31_MWDT_ELF");
    for group in 0u32..2 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.load(0x2f070000, &group.to_le_bytes(), 4).unwrap();
        let run = m.run(10_000); assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 MWDT READY\n");
        m.bus.mwdt[group as usize].clock_write(0x121).unwrap(); m.advance_mwdt_xtal_ticks(group as usize, 100).unwrap();
        assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting); assert!(!m.bus.mwdt[group as usize].irq());
        m.bus.mwdt[group as usize].clock_write(0x921).unwrap();
        m.advance_mwdt_xtal_ticks(group as usize, 7).unwrap(); assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting);
        assert_eq!(m.bus.uart0.tx_out, b"S31 MWDT READY\n");
        m.advance_mwdt_xtal_ticks(group as usize, 1).unwrap(); let irq = m.run(10_000); assert_eq!(irq.stop, esp32s31::Stop::Waiting);
        assert_eq!(m.bus.uart0.tx_out, b"S31 MWDT READY\nS31 MWDT IRQ PASS\n");
        assert!(!m.bus.mwdt[group as usize].irq()); assert_eq!(m.bus.mwdt[(group ^ 1) as usize].read(0x74), Ok(0));
        m.advance_mwdt_xtal_ticks(group as usize, 5).unwrap(); assert_eq!(m.run(100).stop, esp32s31::Stop::Waiting);
        m.advance_mwdt_xtal_ticks(group as usize, 1).unwrap(); assert_eq!(m.run(100).stop, esp32s31::Stop::MwdtReset { group: group as usize, action: esp32s31::mwdt::Reset::System });
        assert!(m.bus.first_fault.is_none()); assert_eq!(m.advance_mwdt_xtal_ticks(usize::MAX, 1), Err(emu_core::Fault::Unmapped));
        eprintln!("S31 MWDT group={group} initial-steps={} ISR-steps={}", run.steps, irq.steps);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_RTC_TIMER_ELF to rtc-timer-probe.elf"]
fn external_s31_rtc_counter_snapshot_manual_and_interpreter_clock() {
    let path = std::env::var("S31_RTC_TIMER_ELF").expect("build examples/s31-bringup and set S31_RTC_TIMER_ELF");
    let elf = std::fs::read(path).expect("read S31_RTC_TIMER_ELF");
    for mode in 0u32..4 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        m.bus.load(0x2f070000, &u32::from(mode != 0).to_le_bytes(), 4).unwrap();
        if mode == 0 { m.advance_rtc_ticks(0x1_ffff_fffe); }
        m.set_rtc_timer_from_cpu(mode == 1);
        if mode == 3 { m.bus.permissions.write(0x207067f0, 1).unwrap(); m.bus.permissions.write(0x20706024, 0).unwrap(); }
        let run = m.run(10_000);
        match mode {
            0 | 1 => { assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 RTC SNAPSHOT PASS\n"); assert_eq!(m.bus.rtc_timer.counter(), 0); }
            2 => { assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_)))); assert_eq!(m.bus.uart0.tx_out, b"S31 RTC SNAPSHOT FAIL\n"); }
            _ => { assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(5))); let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.address, f.reason), (0x20800028, emu_core::Fault::Prohibited)); assert!(m.bus.uart0.tx_out.is_empty()); }
        }
        if mode != 3 { assert!(m.bus.first_fault.is_none()); }
        eprintln!("S31 RTC snapshot mode={mode} steps={} stop={:?}", run.steps, run.stop);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_NOR_WRITE_ELF to nor-write-probe.elf"]
fn external_s31_nor_erase_program_read_and_retire_flash_code() {
    let path = std::env::var("S31_NOR_WRITE_ELF").expect("build examples/s31-bringup and set S31_NOR_WRITE_ELF");
    let elf = std::fs::read(path).expect("read S31_NOR_WRITE_ELF");
    for mode in 0u32..4 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.cache.set_flash(&[0xa5; 0x4000]).unwrap();
        m.bus.mspi.attach_flash([if mode == 3 { 0x20 } else { 0xef }, 0x40, 0x18], 0).unwrap();
        m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        m.pc_watch = Some(esp32s31::PcWatch { address: 0x40001080, ..Default::default() });
        let run = m.run(10_000);
        if mode == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 NOR ERASE PROGRAM EXECUTE PASS\n");
            assert_eq!(m.pc_watch.as_ref().unwrap().retired, 1); assert!(m.pc_watch.as_ref().unwrap().first_cycle.is_some());
            assert_eq!(m.bus.cache.read_byte(0x40001080, false), Ok(0x13)); assert_eq!(m.bus.cache.read_byte(0x40000000, false), Ok(0xa5));
        } else if mode < 3 {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_)))); assert_eq!(m.bus.uart0.tx_out, b"S31 NOR WRITE FAIL\n"); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 0);
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7))); let f = m.bus.first_fault.as_ref().unwrap();
            assert_eq!((f.address, f.reason), (0x20501000, emu_core::Fault::Prohibited)); assert!(m.bus.uart0.tx_out.is_empty());
        }
        eprintln!("S31 NOR mode={mode} steps={} stop={:?} command-count={} watch={:?}", run.steps, run.stop, m.bus.mspi.commands, m.pc_watch);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_GPIO_INPUT_ELF to gpio-input-probe.elf"]
fn external_s31_gpio_alias_and_explicit_pad_voltage_guest_readback() {
    let path = std::env::var("S31_GPIO_INPUT_ELF").expect("build examples/s31-bringup and set S31_GPIO_INPUT_ELF");
    let elf = std::fs::read(path).expect("read S31_GPIO_INPUT_ELF");
    for mode in 0..4 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        if mode != 1 { m.bus.pads.set_unresolved_sample(32, Some(mode == 2)).unwrap(); }
        if mode == 3 { m.bus.pads.set_input(32, Some(true)).unwrap(); }
        let run = m.run(2000);
        if mode == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Budget); assert_eq!(m.bus.uart0.tx_out, b"S31 GPIO SAMPLE READY\n");
            assert_eq!(m.bus.pads.drive(36), Ok(esp32s31::pads::Drive::High));
            assert_eq!(m.bus.pads.drive(32), Ok(esp32s31::pads::Drive::Unsupported));
            m.bus.pads.set_unresolved_sample(32, Some(true)).unwrap(); m.bus.uart_input(b"A").unwrap();
            assert_eq!(m.run(2000).stop, esp32s31::Stop::Waiting);
            assert_eq!(m.bus.uart0.tx_out, b"S31 GPIO SAMPLE READY\nS31 GPIO SAMPLE CHANGED PASS\n");
        } else if mode == 2 {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_)))); assert_eq!(m.bus.uart0.tx_out, b"S31 GPIO SAMPLE FAIL\n");
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(5))); assert!(m.bus.uart0.tx_out.is_empty());
            let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.address, f.reason), (0x20583068, emu_core::Fault::Prohibited));
        }
        eprintln!("S31 GPIO sample mode={mode} steps={} stop={:?} uart={:?}", run.steps, run.stop, String::from_utf8_lossy(&m.bus.uart0.tx_out));
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_PMU_POLICY_ELF to pmu-policy-probe.elf"]
fn external_s31_pmu_policy_storage_command_and_active_power_guards() {
    let path = std::env::var("S31_PMU_POLICY_ELF").expect("build examples/s31-bringup and set S31_PMU_POLICY_ELF");
    let elf = std::fs::read(path).expect("read S31_PMU_POLICY_ELF");
    for mode in 0u32..4 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        let run = m.run(2000);
        if mode == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 PMU POLICY PASS\n");
            assert_eq!(m.bus.power.policy.dbias_requests, 1); assert_eq!(m.bus.power.policy.readonly_write_attempts, 1);
            assert_eq!(m.bus.power.policy.applied_clocks().1, [u32::MAX; 2]);
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7))); assert!(m.bus.uart0.tx_out.is_empty());
            let f = m.bus.first_fault.as_ref().unwrap(); let address = match mode { 1 => 0x20704000, 2 => 0x20704100, _ => 0x20704120 };
            assert_eq!((f.address, f.reason), (address, emu_core::Fault::Prohibited));
        }
        eprintln!("S31 PMU policy mode={mode} steps={} stop={:?} updates={} dbias-requests={}", run.steps, run.stop, m.bus.power.policy.updates, m.bus.power.policy.dbias_requests);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_MPLL_ELF to mpll-probe.elf"]
fn external_s31_mpll_real_psram_code_with_power_clock_and_calibration_negatives() {
    let path = std::env::var("S31_MPLL_ELF").expect("build examples/s31-bringup and set S31_MPLL_ELF");
    let elf = std::fs::read(path).expect("read S31_MPLL_ELF");
    for mode in 0u32..6 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.cache.set_psram_size(65536).unwrap();
        m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        m.pc_watch = Some(esp32s31::PcWatch { address: 0x50000000, ..Default::default() });
        let run = m.run(10_000);
        if mode == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 MPLL PSRAM EXECUTE PASS\n");
            assert_eq!(m.cpu.x[10], 42); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 1);
            assert_eq!(m.bus.clocks.mpll.output_hz(), Some(400_000_000)); assert_eq!(m.bus.clocks.psram_core_hz(), Some(200_000_000));
        } else if mode == 1 {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_))));
            assert_eq!(m.bus.uart0.tx_out, b"S31 MPLL FAIL\n"); assert_eq!(m.bus.clocks.mpll.output_hz(), None);
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(if mode == 3 || mode == 4 { 1 } else { 7 })));
            let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.address, f.reason), (0x50000000, emu_core::Fault::Prohibited));
            assert!(m.bus.uart0.tx_out.is_empty());
        }
        if mode != 0 { assert_eq!(m.pc_watch.as_ref().unwrap().retired, 0); }
        eprintln!("S31 MPLL mode={mode} steps={} stop={:?} mpll={:?} psram={:?} retired={}", run.steps, run.stop, m.bus.clocks.mpll.output_hz(), m.bus.clocks.psram_core_hz(), m.pc_watch.as_ref().unwrap().retired);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_MSPI_PADS_ELF to mspi-pads-probe.elf"]
fn external_s31_dedicated_mspi_pad_hold_and_release_control_real_memory_execution() {
    let path = std::env::var("S31_MSPI_PADS_ELF").expect("build examples/s31-bringup and set S31_MSPI_PADS_ELF");
    let elf = std::fs::read(path).expect("read S31_MSPI_PADS_ELF");
    for mode in 0u32..4 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.cache.set_psram_size(65536).unwrap();
        m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        m.pc_watch = Some(esp32s31::PcWatch { address: 0x50000000, ..Default::default() });
        let run = m.run(2000);
        if mode == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 MSPI PADS EXECUTE PASS\n");
            assert_eq!(m.cpu.x[10], 42); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 1);
            assert!(!m.bus.mspi_pads.held()); assert!(m.bus.mspi_pads.dqs_requested());
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(if mode == 1 { 7 } else if mode == 2 { 1 } else { 5 })));
            let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.address, f.reason), (0x50000000, emu_core::Fault::Prohibited));
            assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 0);
        }
        eprintln!("S31 MSPI pads mode={mode} steps={} stop={:?} held={} retired={}", run.steps, run.stop, m.bus.mspi_pads.held(), m.pc_watch.as_ref().unwrap().retired);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_PSRAM_CONTROLLER_ELF to psram-controller-probe.elf"]
fn external_s31_psram_serial_reads_writes_and_executes_with_device_power_clock_and_axi_negatives() {
    let path = std::env::var("S31_PSRAM_CONTROLLER_ELF").expect("build examples/s31-bringup and set S31_PSRAM_CONTROLLER_ELF");
    let elf = std::fs::read(path).expect("read S31_PSRAM_CONTROLLER_ELF");
    for mode in 0u32..10 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        if mode != 9 { m.bus.cache.set_psram_size(65536).unwrap(); }
        if mode != 1 { m.bus.psram_controller.attach_mode([0x0c, 0x0d, 0xdf, 0x80, 0x20, 5]).unwrap(); }
        m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        m.pc_watch = Some(esp32s31::PcWatch { address: 0x50000000, ..Default::default() });
        let run = m.run(3000);
        if mode == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 PSRAM SERIAL EXECUTE PASS\n");
            assert_eq!(m.cpu.x[10], 42); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 1);
            assert_eq!(m.bus.psram_controller.command_counts, [2, 1, 1, 1]);
        } else {
            let fetch = matches!(mode, 5 | 6);
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(if fetch { 1 } else { 7 })));
            let f = m.bus.first_fault.as_ref().unwrap();
            assert_eq!(f.address, if fetch { 0x50000000 } else { 0x20503000 });
            assert_eq!(f.reason, if mode == 9 { emu_core::Fault::Unmapped } else { emu_core::Fault::Prohibited });
            assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 0);
        }
        eprintln!("S31 PSRAM serial mode={mode} steps={} stop={:?} commands={:?} retired={}", run.steps, run.stop, m.bus.psram_controller.command_counts, m.pc_watch.as_ref().unwrap().retired);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_APM_ELF to apm-probe.elf"]
fn external_s31_apm_declared_path_filters_real_memory_and_instruction_fetch() {
    use emu_core::Bus;
    let path = std::env::var("S31_APM_ELF").expect("build examples/s31-bringup and set S31_APM_ELF");
    let elf = std::fs::read(path).expect("read S31_APM_ELF");
    for (label, attr, error, func, route, exception) in [
        ("allow", 0x7000u32, 1u32, 0x3fu32, true, 0),
        ("write-denied", 0x5000, 1, 0x3f, true, 7),
        ("read-denied", 0x3000, 1, 0x3f, true, 5),
        ("execute-denied", 0x6000, 1, 0x3f, true, 1),
        ("unknown-route", 0x6000, 1, 0x3f, false, 7),
        ("quiet-denial-unsupported", 0x6000, 0, 0x3f, true, 7),
        ("filter-disabled", 0x6000, 1, 0x3b, true, 0),
    ] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        for (i, word) in [attr, error, func].iter().enumerate() { m.bus.load(0x2f070000 + i as u32 * 4, &word.to_le_bytes(), 4).unwrap(); }
        if route { m.bus.apm.set_test_route(2, 2, 0x2f060000, 0x2f06007f).unwrap(); }
        m.pc_watch = Some(esp32s31::PcWatch { address: 0x2f060000, ..Default::default() });
        let run = m.run(1000);
        if exception == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 APM RWX PASS\n");
            assert_eq!(m.pc_watch.as_ref().unwrap().retired, 1); assert_eq!(m.bus.read32(0x2f060008), Ok(0xdeadbeef));
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(exception)));
            assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 0);
            let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!(f.reason, emu_core::Fault::Prohibited);
            assert_eq!(f.address, if !route || error == 0 { 0x2050480c } else if exception == 1 { 0x2f060000 } else { 0x2f060008 });
            assert_eq!(f.fetch, exception == 1);
            if route && error != 0 {
                assert_eq!(m.bus.apm.read(0x205048e8), Ok(1));
                assert_eq!(m.bus.apm.read(0x205048f0), Err(emu_core::Fault::Unmapped));
                m.bus.apm.write(0x2050480c, 0x7000).unwrap();
                assert_eq!(m.bus.read32(0x2f060008), Ok(if exception == 7 { 0x12345678 } else { 0xdeadbeef }));
                m.bus.apm.write(0x205048ec, 1).unwrap(); assert_eq!(m.bus.apm.read(0x205048e8), Ok(0));
            }
        }
        eprintln!("S31 APM control={label} steps={} stop={:?} retired={} writes={} denial={:?}", run.steps, run.stop, m.pc_watch.as_ref().unwrap().retired, m.bus.apm.writes, m.bus.apm.last_denial);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_MULTICORE_ELF and S31_ROM_ELF"]
fn external_s31_two_real_rom_harts_share_atomics_and_honor_stall_reset() {
    use emu_core::Bus;
    let path = std::env::var("S31_MULTICORE_ELF").expect("build examples/s31-bringup and set S31_MULTICORE_ELF");
    let rom = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to Espressif 20260528 S31 ROM");
    let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&std::fs::read(rom).unwrap()).unwrap();
    while m.cpu.pc != 0x2f801852 { assert_eq!(m.run(1).stop, esp32s31::Stop::Budget); assert!(m.cpu.insn_count < 10000); }
    m.load_elf(&std::fs::read(path).unwrap()).unwrap();
    m.pc_watch1 = Some(esp32s31::PcWatch { address: esp32s31::bus::ROM_BASE, ..Default::default() });
    let run = m.run(50000);
    let counts = [m.bus.read32(0x2f060008), m.bus.read32(0x2f060014)];
    eprintln!("multicore steps={} stop={:?} fault={:?} pcs={:x}/{:x} counters={:?}/{:?}", run.steps, run.stop, m.bus.first_fault, m.cpu.pc, m.cpu1.pc, counts[0], counts[1]);
    assert_eq!(run.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 MULTICORE ROM ATOMIC RESET PASS\n");
    assert_eq!(m.bus.read32(0x2f060008), Ok(400));
    assert!(m.bus.read32(0x2f060010).unwrap() + m.bus.read32(0x2f060014).unwrap() > 0);
    assert_eq!(m.bus.read32(0x2f060018), Ok(0x5a31));
    assert_eq!(m.pc_watch1.as_ref().unwrap().retired, 2);
    assert_eq!(m.bus.cores.resets, [0, 1]); assert!(m.cpu1.retired_count > 0); assert!(m.bus.first_fault.is_none());
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_CACHE_ERROR_ELF and S31_ROM_ELF"]
fn external_s31_real_guest_cache_error_isr_and_wtc_clear() {
    use emu_core::Bus;
    let path = std::env::var("S31_CACHE_ERROR_ELF").expect("build examples/s31-bringup and set S31_CACHE_ERROR_ELF");
    let rom = std::env::var("S31_ROM_ELF").expect("set S31_ROM_ELF to Espressif 20260528 S31 ROM");
    let mut m = esp32s31::Bringup::new(); m.load_rom_elf(&std::fs::read(rom).unwrap()).unwrap();
    while m.cpu.pc != 0x2f801852 { assert_eq!(m.run(1).stop, esp32s31::Stop::Budget); assert!(m.cpu.insn_count < 10000); }
    m.load_elf(&std::fs::read(path).unwrap()).unwrap(); m.bus.cache.set_flash(&[0x5a, 0, 0, 0]).unwrap();
    let first = m.run(10000); assert_eq!(first.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(5)));
    let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.hart, f.address, f.write), (1, 0x40000000, false));
    assert_eq!(m.bus.cache.read_register(0x2c000178), Ok(16)); assert!(m.bus.cache.irq());
    // No state/register repair: resume execution at the real guest exception vector.
    let second = m.run(10000);
    eprintln!("cache-error steps={}/{} stop={:?} pcs={:x}/{:x} uart={:?}", first.steps, second.steps, second.stop, m.cpu.pc, m.cpu1.pc, String::from_utf8_lossy(&m.bus.uart0.tx_out));
    assert_eq!(second.stop, esp32s31::Stop::Waiting);
    assert_eq!(m.bus.uart0.tx_out, b"S31 CACHE ERROR REAL ISR PASS\n");
    assert_eq!(m.bus.read32(0x2f060000), Ok(1)); assert!(!m.bus.cache.irq());
    assert_eq!(m.bus.cache.read_register(0x2c000178), Ok(0)); assert_eq!(m.bus.read32(0x40000000), Ok(0x5a));
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_BROWNOUT_ELF"]
fn external_s31_brownout_qualified_input_irq_system_reset_and_missing_analog_seed() {
    use emu_core::Bus;
    let path = std::env::var("S31_BROWNOUT_ELF").expect("build examples/s31-bringup and set S31_BROWNOUT_ELF");
    let elf = std::fs::read(path).unwrap();
    for mode in 0u32..4 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        if mode != 3 { m.bus.analog.set_bod_register(0xff); }
        let first = m.run(10000);
        if mode == 3 {
            assert_eq!(first.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7))); assert_eq!(m.bus.analog.transactions, 0);
            assert_eq!(m.bus.first_fault.as_ref().unwrap().address, 0x2010f804); assert!(m.bus.uart0.tx_out.is_empty()); continue;
        }
        assert_eq!(first.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.analog.bod_register(), Some(0xfd)); assert_eq!(m.bus.analog.transactions, 3);
        assert_eq!(m.bus.uart0.tx_out, b"S31 BROWNOUT MODE0 ARMED\n"); assert_eq!(m.run(1000).steps, 0); assert_eq!(m.bus.read32(0x20702020), Ok(0));
        if mode == 0 {
            m.bus.power.brownout_mode0_ticks(true, 1).unwrap(); assert_eq!(m.run(1000).steps, 0); assert_eq!(m.bus.read32(0x20702020), Ok(0));
            m.bus.power.brownout_mode0_ticks(true, 1).unwrap(); let second = m.run(10000);
            eprintln!("brownout mode0 armed={} after-input={} stop={:?}", first.steps, second.steps, second.stop);
            assert_eq!(second.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.read32(0x2f060000), Ok(1));
            assert_eq!(m.bus.uart0.tx_out, b"S31 BROWNOUT MODE0 ARMED\nS31 BROWNOUT REAL IRQ MRET PASS\n");
            assert_eq!(m.bus.read32(0x20702020), Ok(0)); assert_eq!(m.bus.read32(0x20702030), Ok(0));
        } else if mode == 1 {
            m.bus.power.brownout_mode0_ticks(true, 3).unwrap(); assert_eq!(m.run(1000).stop, esp32s31::Stop::Waiting);
            m.bus.power.brownout_mode0_ticks(true, 1).unwrap(); assert_eq!(m.run(1000).stop, esp32s31::Stop::PowerReset(esp32s31::power::ResetCause::Brownout));
            assert_eq!(m.bus.read32(0x2f060000), Ok(0));
        } else {
            assert_eq!(m.bus.power.brownout_mode0_ticks(true, 1), Err(emu_core::Fault::Prohibited));
            assert_eq!(m.bus.read32(0x20702020), Ok(0)); assert_eq!(m.run(1000).stop, esp32s31::Stop::Waiting);
        }
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_PSRAM_IRQ_ELF"]
fn external_s31_psram_actual_read_write_errors_and_guest_exception_irq_handlers() {
    use emu_core::Bus;
    let path = std::env::var("S31_PSRAM_IRQ_ELF").expect("build examples/s31-bringup and set S31_PSRAM_IRQ_ELF");
    let mut m = esp32s31::Bringup::new(); m.load_elf(&std::fs::read(path).unwrap()).unwrap(); m.bus.cache.set_psram_size(65536).unwrap();
    let first = m.run(10000); assert_eq!(first.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(5)));
    assert_eq!(m.bus.read32(0x205020c8), Ok(0x80)); assert!(m.bus.psram_controller.irq());
    let fault = m.bus.first_fault.as_ref().unwrap(); assert_eq!((fault.hart, fault.address, fault.reason), (0, 0x50000000, emu_core::Fault::Unmapped));
    // No register/PC repair: execute the guest's own exception handler, mret,
    // source112 ISR and next real store. That store must independently fault.
    let second = m.run(10000); assert_eq!(second.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
    assert_eq!(m.bus.read32(0x205020c8), Ok(0x200)); assert!(m.bus.psram_controller.irq());
    let third = m.run(10000);
    eprintln!("psram-irq steps={}/{}/{} stop={:?} uart={:?}", first.steps, second.steps, third.steps, third.stop, String::from_utf8_lossy(&m.bus.uart0.tx_out));
    assert_eq!(third.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 PSRAM REAL FAULT IRQ MRET PASS\n");
    assert_eq!(m.bus.read32(0x2f060000), Ok(2)); assert_eq!(m.bus.read32(0x50000000), Ok(0x12345678));
    assert_eq!(m.bus.read32(0x205020c8), Ok(0)); assert!(!m.bus.psram_controller.irq());
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_RTC_CALIBRATION_ELF"]
fn external_s31_guest_measures_clock_divider_and_timeout_without_read_side_completion() {
    use emu_core::Bus;
    let path = std::env::var("S31_RTC_CALIBRATION_ELF").expect("build examples/s31-bringup and set S31_RTC_CALIBRATION_ELF");
    let bytes = std::fs::read(path).unwrap();
    let mut m = esp32s31::Bringup::new(); m.load_elf(&bytes).unwrap();
    assert_eq!(m.run(20000).stop, esp32s31::Stop::Budget); assert_eq!(m.bus.rtc_calibration.measurements, 0);
    assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.bus.read32(0x2f060000), Ok(0));
    // Continue the same guest without repairing registers or flags. Only supply
    // the explicitly selected nominal source-clock relationship.
    m.set_rtc_calibration_from_cpu(true); let r = m.run(30000);
    eprintln!("rtc-calibration steps={} stop={:?} measurements={} uart={:?}", r.steps, r.stop, m.bus.rtc_calibration.measurements, String::from_utf8_lossy(&m.bus.uart0.tx_out));
    assert_eq!(r.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.rtc_calibration.measurements, 2);
    assert_eq!(m.bus.uart0.tx_out, b"S31 RTC CALIBRATION COUNT TIMEOUT PASS\n");
    assert_eq!(m.bus.read32(0x2f060000), Ok(1)); assert!(m.bus.first_fault.is_none());
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_SYSTIMER_ELF"]
fn external_s31_systimer_clock_snapshot_alarm_wfi_and_real_guest_irq() {
    use emu_core::Bus;
    let path = std::env::var("S31_SYSTIMER_ELF").expect("build examples/s31-bringup and set S31_SYSTIMER_ELF");
    let bytes = std::fs::read(path).unwrap(); let mut m = esp32s31::Bringup::new(); m.load_elf(&bytes).unwrap();
    assert_eq!(m.run(20000).stop, esp32s31::Stop::Budget); assert_eq!(m.bus.systimer.counters(), [0; 2]); assert!(m.bus.uart0.tx_out.is_empty());
    // Continue the unchanged guest, now with the documented explicit nominal
    // clock relationship. It programs a future alarm and genuinely reaches WFI.
    m.set_systimer_from_cpu(true); let first = m.run(2000);
    assert_eq!(first.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.systimer.events, [0; 3]); assert_eq!(m.bus.read32(0x2f060000), Ok(0));
    let counters = m.bus.systimer.counters(); assert_eq!(m.run(1000).steps, 0); assert_eq!(m.bus.systimer.counters(), counters);
    // A halted hart does not manufacture elapsed time. Supply512 actual timer
    // edges; the routed interrupt wakes it and its own ISR clears the source.
    m.advance_systimer_ticks(512); let second = m.run(2000);
    eprintln!("systimer strict=20000 pre-wfi={} post-input={} stop={:?} counters={:?} events={:?} uart={:?}", first.steps, second.steps, second.stop, m.bus.systimer.counters(), m.bus.systimer.events, String::from_utf8_lossy(&m.bus.uart0.tx_out));
    assert_eq!(second.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.systimer.events, [1, 0, 0]);
    assert_eq!(m.bus.uart0.tx_out, b"S31 SYSTIMER CLOCK SNAPSHOT IRQ MRET PASS\n");
    assert_eq!(m.bus.read32(0x2f060000), Ok(1)); assert!(m.bus.first_fault.is_none());
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_MODEM_CLOCKS_ELF"]
fn external_s31_modem_clocks_share_actual_i2c_gate_and_reset_with_uart_configuration() {
    use emu_core::Bus;
    let path = std::env::var("S31_MODEM_CLOCKS_ELF").expect("build examples/s31-bringup and set S31_MODEM_CLOCKS_ELF");
    let elf = std::fs::read(path).unwrap();
    for mode in ["enabled", "master-disabled", "parent-disabled", "no-bias-input"] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        let enables: u32 = if mode == "master-disabled" { 11 } else { 15 };
        m.bus.load(0x2f070000, &enables.to_le_bytes(), 4).unwrap();
        if mode != "no-bias-input" { m.bus.analog.set_bias([0x17,0x29]); }
        if mode == "parent-disabled" { m.bus.clocks.write(0x40,0).unwrap(); }
        let r = m.run(10000);
        if mode == "enabled" {
            assert_eq!(r.stop, esp32s31::Stop::Waiting); assert!(m.bus.first_fault.is_none());
            assert_eq!(m.bus.uart0.tx_out,b"S31 MODEM CLOCKS I2C RESET PASS\n");
            assert_eq!(m.bus.read32(0x2f060000),Ok(1)); assert_eq!(m.bus.analog.transactions,8);
            assert_eq!(m.bus.analog.bias(),Some([0xa7,0x2a]));
            for (i,hz) in [1000000,34000,2000000].into_iter().enumerate() { assert_eq!(m.bus.analog.lpcon.lp_hz(i,m.bus.clocks.modem_on(),m.bus.clocks.rtc_slow_hz()),Ok(hz)); }
            assert_eq!(m.bus.clocks.aux_uart_clock(0),Ok((false,true,16000000)));
            assert_eq!(m.bus.clocks.aux_uart_clock(1),Ok((true,false,0)));
            assert_eq!(m.bus.clocks.aux_uart_clock(2),Ok((true,false,0)));
            assert_eq!(m.bus.read32(0x20589010),Ok(0)); assert_eq!(m.bus.read32(0x20589014),Ok(0x80000000));
        } else {
            assert_eq!(r.stop,esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
            let f=m.bus.first_fault.as_ref().unwrap(); assert_eq!(f.reason,emu_core::Fault::Prohibited);
            assert_eq!(f.address,if mode=="parent-disabled" { 0x2010f018 } else { 0x2010f804 });
            assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.bus.analog.transactions,0);
            assert_eq!(m.bus.read32(0x2f060000),Ok(0));
        }
        eprintln!("modem-clocks mode={mode} steps={} stop={:?} transactions={}",r.steps,r.stop,m.bus.analog.transactions);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_STARTUP_CLOCKS_ELF"]
fn external_s31_startup_clocks_preserve_real_sha_and_apply_permissions_to_new_domains() {
    use emu_core::Bus;
    let path=std::env::var("S31_STARTUP_CLOCKS_ELF").expect("build examples/s31-bringup and set S31_STARTUP_CLOCKS_ELF");
    let elf=std::fs::read(path).unwrap();
    for mode in 0u32..7 {
        let mut m=esp32s31::Bringup::new();m.load_elf(&elf).unwrap();m.bus.load(0x2f070000,&mode.to_le_bytes(),4).unwrap();
        let r=m.run(10000);
        if mode==0 {
            assert_eq!(r.stop,esp32s31::Stop::Waiting);assert!(m.bus.first_fault.is_none());
            assert_eq!(m.bus.uart0.tx_out,b"S31 STARTUP CLOCKS SHA PRESERVE PASS\n");
            assert_eq!(m.bus.read32(0x2f060000),Ok(1));assert_eq!(m.bus.sha.blocks(),1);
            assert_eq!(m.bus.sha.read(0x40),Ok(0xbf1678ba));
            assert_eq!(m.bus.startup_clocks.read(0x20359030),Ok(0xe0000000));
        } else {
            assert_eq!(r.stop,esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(if mode==3 {7} else {5})));
            let f=m.bus.first_fault.as_ref().unwrap();
            assert_eq!(f.address,[0,0x20509040,0x20393000,0x20587154,0x205043f8,0x20710018,0x20359030][mode as usize]);
            assert_eq!(f.reason,if mode==2 {emu_core::Fault::Unmapped} else {emu_core::Fault::Prohibited});
            assert!(m.bus.uart0.tx_out.is_empty());assert_eq!(m.bus.read32(0x2f060000),Ok(0));
        }
        eprintln!("startup-clocks mode={mode} steps={} stop={:?} sha-blocks={}",r.steps,r.stop,m.bus.sha.blocks());
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_WAITI_ELF"]
fn external_s31_auto_wait_after_actual_nor_writes_and_invalid_commands() {
    let path = std::env::var("S31_WAITI_ELF").expect("build examples/s31-bringup and set S31_WAITI_ELF");
    let elf = std::fs::read(path).unwrap();
    for mode in 0u32..6 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.cache.set_flash(&[0xa5; 0x4000]).unwrap();
        m.bus.mspi.attach_flash([0xef, 0x40, 0x18], 0x200).unwrap(); m.bus.load(0x2f070000, &mode.to_le_bytes(), 4).unwrap();
        m.pc_watch = Some(esp32s31::PcWatch { address: 0x40001080, ..Default::default() }); let run = m.run(10000);
        if mode < 2 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 FLASH WAITI ERASE PROGRAM EXECUTE PASS\n");
            assert_eq!(m.pc_watch.as_ref().unwrap().retired, 1); assert_eq!(m.bus.cache.read_byte(0x40001080, false), Ok(0x13));
            assert_eq!(m.bus.mspi.waiti_polls, if mode == 0 { 3 } else { 0 }); assert_eq!(m.bus.mspi.last_waiti_status, if mode == 0 { Some(0) } else { None });
        } else {
            assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7))); assert!(m.bus.uart0.tx_out.is_empty());
            let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.address, f.reason), (if mode == 5 { 0x2050109c } else { 0x20501000 }, emu_core::Fault::Prohibited));
            assert_eq!(m.bus.cache.read_byte(0x40001080, false), Ok(0xa5)); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 0);
            assert_eq!(m.bus.mspi.waiti_polls, 0); assert_eq!(m.bus.mspi.last_waiti_status, None);
        }
        assert_eq!(m.bus.cache.read_byte(0x40000000, false), Ok(0xa5));
        eprintln!("S31 WAITI mode={mode} steps={} stop={:?} polls={} sample={:?}", run.steps, run.stop, m.bus.mspi.waiti_polls, m.bus.mspi.last_waiti_status);
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_LAZY_FPU_ELF"]
fn external_s31_guest_lazy_fpu_handler_retires_original_load_without_host_repair() {
    use emu_core::Bus;
    let path = std::env::var("S31_LAZY_FPU_ELF").expect("build examples/s31-bringup and set S31_LAZY_FPU_ELF");
    let elf = std::fs::read(path).unwrap(); let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
    let first = m.run(10000); assert_eq!(first.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(2)));
    assert!(m.bus.first_fault.is_none()); assert_eq!(m.cpu.mtval & 0x7f, 7); assert!(m.cpu.fs_off()); assert!(m.bus.uart0.tx_out.is_empty());
    let faulting_pc = m.cpu.mepc; m.pc_watch = Some(esp32s31::PcWatch { address: faulting_pc, ..Default::default() });
    let second = m.run(10000); assert_eq!(second.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 GUEST LAZY FPU MRET PASS\n");
    assert_eq!(m.bus.read32(0x2f060000), Ok(1)); assert_eq!(m.bus.read32(0x2f060004), Ok(0x40400000)); assert_eq!(m.pc_watch.as_ref().unwrap().retired, 1);
    eprintln!("S31 lazyFPU first={} second={} original-pc={faulting_pc:#x}", first.steps, second.steps);
    let mut repeat = esp32s31::Bringup::new(); repeat.load_elf(&elf).unwrap(); repeat.bus.uart_input(b"L").unwrap();
    for _ in 0..3 { assert_eq!(repeat.run(10000).stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(2))); }
    assert!(repeat.cpu.fs_off()); assert!(repeat.bus.uart0.tx_out.is_empty()); assert_eq!(repeat.bus.read32(0x2f060000), Ok(2));
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_FPU_REASON_ELF"]
fn external_s31_explicit_fp_memory_reason_policy_and_guest_handler() {
    use emu_core::Bus;
    let path = std::env::var("S31_FPU_REASON_ELF").expect("build examples/s31-bringup and set S31_FPU_REASON_ELF");
    let elf = std::fs::read(path).unwrap();
    for compatibility in [false, true] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.set_upstream_fpu_memory_reason(compatibility);
        let mut steps = 0; let mut exceptions = 0;
        loop {
            let run = m.run(10000); steps += run.steps;
            if run.stop == esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(2)) {
                exceptions += 1; assert!(exceptions <= 6); assert!(m.bus.first_fault.is_none()); assert!(m.cpu.fs_off());
                assert_eq!(m.cpu.ext_ill & 1, u32::from(compatibility)); continue;
            }
            if compatibility {
                assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 FPU MEMORY REASONS PASS\n");
                assert_eq!(exceptions, 6); assert_eq!(m.bus.read32(0x2f060000), Ok(6)); assert_eq!(m.bus.read32(0x2f07fff4), Ok(0x3fc00000));
                assert_eq!(m.compatibility_fpu_memory_traps, [6, 0]);
            } else {
                assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Ebreak(_)))); assert_eq!(m.bus.uart0.tx_out, b"S31 FPU MEMORY REASON FAIL\n");
                assert_eq!(exceptions, 1); assert_eq!(m.compatibility_fpu_memory_traps, [0, 0]);
            }
            break;
        }
        eprintln!("S31 FP memory policy compatibility={compatibility} steps={steps} actual-exceptions={exceptions}");
    }
}

#[test]
#[ignore = "build examples/s31-bringup and set S31_CRYPTO_CLOCK_ELF"]
fn external_s31_crypto_roots_hash_guest_data_and_reject_absent_clocks() {
    use emu_core::Bus;
    let path = std::env::var("S31_CRYPTO_CLOCK_ELF").expect("build examples/s31-bringup and set S31_CRYPTO_CLOCK_ELF");
    let elf = std::fs::read(path).unwrap();
    for mode in 0..6 {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap();
        m.bus.write32(0x2f070000, match mode { 0 => 0, 1 => 1, 3 => 3, _ => 2 }).unwrap();
        if mode != 2 { m.bus.mspi.set_bbpll_input(Some(480_000_000)).unwrap(); }
        if mode == 1 { m.bus.clocks.set_fosc_power(false); }
        if mode == 4 { m.bus.write32(0x2058717c, 1).unwrap(); }
        if mode == 5 { m.bus.mspi.set_bbpll_gate(false); }
        let run = m.run(10000);
        if mode == 0 {
            assert_eq!(run.stop, esp32s31::Stop::Waiting); assert_eq!(m.bus.uart0.tx_out, b"S31 CRYPTO ROOTS REAL SHA PASS\n");
            assert_eq!(m.bus.read32(0x2f060000), Ok(3)); assert_eq!(m.bus.sha.blocks(), 1);
            assert_eq!(m.bus.crypto_clock_hz(), Some(240_000_000));
        } else {
            assert!(matches!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7))));
            let fault = m.bus.first_fault.as_ref().unwrap();
            assert_eq!(fault.address, if mode == 3 { 0x20587058 } else { 0x20509000 });
            assert_eq!(fault.reason, emu_core::Fault::Prohibited); assert_eq!(m.bus.sha.blocks(), 0);
            assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.bus.read32(0x2f060000), Ok(0));
        }
        eprintln!("S31 crypto root mode={mode} steps={} stop={:?}", run.steps, run.stop);
    }
}


#[test]
#[ignore = "build examples/s31-bringup and set S31_CRYPTO_CLOCK_ELF"]
fn external_s31_key_manager_guest_reset_policy_never_reports_key_operation_success() {
    use emu_core::Bus;
    let path = std::env::var("S31_CRYPTO_CLOCK_ELF").expect("build examples/s31-bringup and set S31_CRYPTO_CLOCK_ELF");
    let elf = std::fs::read(path).unwrap();
    for mode in [6, 7] {
        let mut m = esp32s31::Bringup::new(); m.load_elf(&elf).unwrap(); m.bus.write32(0x2f070000, mode).unwrap();
        let run = m.run(10000); assert_eq!(run.stop, esp32s31::Stop::Trap(riscv_rv32::Trap::Exception(7)));
        let fault = m.bus.first_fault.as_ref().unwrap(); assert_eq!(fault.address, 0x20506024); assert_eq!(fault.reason, emu_core::Fault::Prohibited);
        assert!(m.bus.uart0.tx_out.is_empty()); assert_eq!(m.bus.read32(0x2f060000), Ok(0));
        for off in [8, 0xc, 0x28, 0x2c, 0x30] { assert_eq!(m.bus.read32(0x20506000 + off), Ok(0)); }
        eprintln!("S31 key manager mode={mode} steps={} unsupported command rejected", run.steps);
    }
}
