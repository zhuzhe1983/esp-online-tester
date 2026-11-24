//! Explicit C5 Quad die, controller, fixed pad and MMU boundaries.
use emu_core::{Bus, Fault};
use esp32c5::C5Bus;
use esp_soc::SocBus;

const SPI: u32 = 0x6000_3000;
fn put(b: &mut C5Bus, a: u32, v: u32) { b.write32(a, v).unwrap(); }
fn get(b: &mut C5Bus, a: u32) -> u32 { b.read32(a).unwrap() }
fn pads(b: &mut C5Bus) { for p in [15, 17, 18, 20, 21, 22] { put(b, 0x6009_0000 + 4 * p, 0x800); } }
fn command(b: &mut C5Bus, opcode: u8, qpi: bool, addr: Option<u32>, tx: &[u8], rx: usize, dummy: u32) -> Vec<u8> {
    put(b, SPI + 8, if qpi { (1 << 8) | (1 << 13) | (1 << 24) } else { 0 });
    put(b, SPI + 4, addr.unwrap_or(0));
    put(b, SPI + 0x18, (1 << 31) | if addr.is_some() { 1 << 30 } else { 0 }
        | if rx != 0 { 1 << 28 } else { 0 } | if !tx.is_empty() { 1 << 27 } else { 0 }
        | if dummy != 0 { 1 << 29 } else { 0 } | if qpi { 1 << 15 } else { 0 });
    put(b, SPI + 0x1c, (23 << 26) | dummy.saturating_sub(1));
    put(b, SPI + 0x20, (7 << 28) | u32::from(opcode));
    put(b, SPI + 0x24, (tx.len() as u32 * 8).saturating_sub(1));
    put(b, SPI + 0x28, (rx as u32 * 8).saturating_sub(1));
    for (i, bytes) in tx.chunks(4).enumerate() {
        let mut w = [0; 4]; w[..bytes.len()].copy_from_slice(bytes);
        put(b, SPI + 0x58 + 4 * i as u32, u32::from_le_bytes(w));
    }
    put(b, SPI, 1 << 18);
    (0..rx).map(|i| (get(b, SPI + 0x58 + 4 * (i / 4) as u32) >> ((i & 3) * 8)) as u8).collect()
}
fn configured() -> C5Bus {
    let mut b = C5Bus::new(); b.set_xtal_mhz(48).unwrap(); b.configure_psram(8 << 20).unwrap();
    pads(&mut b); put(&mut b, SPI + 0x34, 1); b
}
fn cache(b: &mut C5Bus) {
    command(b, 0x35, false, None, &[], 0, 0);
    put(b, 0x6000_2040, (1 << 20) | (23 << 14) | (5 << 6) | 0x34);
    put(b, 0x6000_2048, (7 << 28) | 0xeb); put(b, 0x6000_204c, (7 << 28) | 0x38);
    put(b, 0x6000_203c, 0); // Open the AXI interface; POR is closed.
    put(b, 0x6000_2384, 4); put(b, 0x6000_2380, 0); put(b, 0x6000_237c, 0x600);
}

#[test]
fn cpu_cache_data_mask_blocks_psram_stores_without_destroying_die_contents(){
    let mut b=configured();cache(&mut b);put(&mut b,0x4200_0000,0x0000_0013);
    put(&mut b,0x600c_8004,2);assert!(b.write32(0x4200_0000,0xdead_beef).is_err());
    assert_eq!(&b.psram[..4],&0x0000_0013u32.to_le_bytes());assert_eq!(b.fetch(0x4200_0000).unwrap(),[0x13,0,0,0]);
    put(&mut b,0x600c_8004,1);put(&mut b,0x4200_0000,0x0250_0513);assert_eq!(get(&mut b,0x4200_0000),0x0250_0513);assert!(b.fetch(0x4200_0000).is_err());
    put(&mut b,0x600c_8004,0);assert_eq!(b.fetch(0x4200_0000).unwrap(),0x0250_0513u32.to_le_bytes());
}

#[test]
fn explicit_capacity_fixed_pads_chip_select_and_id_are_independent() {
    let mut b = C5Bus::new(); assert!(b.set_psram_size(4 << 20).is_err()); assert!(b.psram.is_empty());
    b.set_psram_size(8 << 20).unwrap(); put(&mut b, SPI + 0x34, 1);
    assert_eq!(command(&mut b, 0x9f, false, Some(0), &[], 3, 0), [255; 3], "GPIO mux has not selected MSPI");
    pads(&mut b);
    assert_eq!(command(&mut b, 0x9f, false, Some(0), &[], 6, 0), [0x0d, 0x5d, 0x42, 0, 0, 0]);
    put(&mut b, 0x6009_003c, 0x1800);
    assert_eq!(command(&mut b, 0x9f, false, Some(0), &[], 3, 0), [255; 3], "stolen CS1 pad");
    pads(&mut b); put(&mut b, SPI + 0x34, 3);
    assert_eq!(command(&mut b, 0x9f, false, Some(0), &[], 3, 0), [255; 3], "both devices unselected");
    put(&mut b, SPI + 0x34, 1); b.set_psram_size(0).unwrap(); assert!(b.psram.is_empty());
    assert_eq!(command(&mut b, 0x9f, false, Some(0), &[], 3, 0), [255; 3], "absent die");
}

#[test]
fn serial_qpi_reset_sequence_and_wrap_have_real_state() {
    let mut b = configured(); let bytes: Vec<u8> = (0..12).collect();
    command(&mut b, 0x02, false, Some(1020), &bytes, 0, 0);
    assert_eq!(command(&mut b, 0x03, false, Some(1020), &[], 12, 0), bytes, "linear burst crosses 1K page");
    command(&mut b, 0x35, false, None, &[], 0, 0);
    assert_eq!(command(&mut b, 0x03, false, Some(1020), &[], 4, 0), [255; 4], "SPI command after QPI entry");
    assert_eq!(command(&mut b, 0xeb, true, Some(1020), &[], 12, 6), bytes);
    assert_eq!(command(&mut b, 0xeb, true, Some(1020), &[], 4, 5), [255; 4], "wrong dummy count");
    command(&mut b, 0xc0, true, None, &[], 0, 0);
    command(&mut b, 0x38, true, Some(28), &bytes, 0, 0);
    assert_eq!(&b.psram[28..32], &[0, 1, 2, 3]); assert_eq!(&b.psram[..8], &[4, 5, 6, 7, 8, 9, 10, 11]);
    command(&mut b, 0x66, true, None, &[], 0, 0);
    command(&mut b, 0x00, true, None, &[], 0, 0); // cancels reset enable
    command(&mut b, 0x99, true, None, &[], 0, 0);
    assert_eq!(command(&mut b, 0x03, false, Some(0), &[], 4, 0), [255; 4]);
    command(&mut b, 0x66, true, None, &[], 0, 0); command(&mut b, 0x99, true, None, &[], 0, 0);
    assert_eq!(command(&mut b, 0x03, false, Some(0), &[], 4, 0), [4, 5, 6, 7], "reset preserves volatile cells while clearing mode");
    command(&mut b, 0x02, false, Some(28), &bytes, 0, 0);
    assert_eq!(&b.psram[28..40], &bytes, "reset restores linear burst");
}

#[test]
fn cache_requires_qpi_real_phase_configuration_and_valid_capacity_mapping() {
    let mut b = configured(); put(&mut b, 0x6000_2380, 0); put(&mut b, 0x6000_237c, 0x600);
    assert_ne!(get(&mut b, 0x6000_203c) & (1 << 31), 0, "AXI is closed at controller reset");
    assert!(b.write32(0x4200_0000, 0).is_err(), "MMU mapping alone is not initialization");
    cache(&mut b); put(&mut b, 0x4200_0000, 0x12345678); assert_eq!(&b.psram[..4], &[0x78, 0x56, 0x34, 0x12]);
    put(&mut b, 0x6000_203c, 1 << 31); assert!(b.read32(0x4200_0000).is_err(), "closing AXI must stop mapped reads");
    assert!(b.write32(0x4200_0000, 0).is_err()); put(&mut b, 0x6000_203c, 0);
    b.write8(0x4200_0001, 0xaa).unwrap(); b.write16(0x4200_0002, 0xbbcc).unwrap(); assert_eq!(get(&mut b, 0x4200_0000), 0xbbccaa78);
    put(&mut b, 0x6000_2048, (7 << 28) | 0x03); assert!(b.read32(0x4200_0000).is_err()); put(&mut b, 0x6000_2048, (7 << 28) | 0xeb);
    put(&mut b, 0x6009_0048, 0x1800); assert!(b.read8(0x4200_0000).is_err(), "stolen WP quad lane"); pads(&mut b);
    put(&mut b, 0x6000_2380, 127); put(&mut b, 0x6000_237c, 0x67f); put(&mut b, 0x427f_fffc, 0xaabbccdd);
    assert_eq!(&b.psram[(8 << 20) - 4..], &[0xdd, 0xcc, 0xbb, 0xaa]);
    put(&mut b, 0x6000_2380, 128); put(&mut b, 0x6000_237c, 0x680); assert_eq!(b.read8(0x4280_0000), Err(Fault::Unmapped));
    put(&mut b, 0x6000_2380, 0); put(&mut b, 0x6000_237c, 0xe00); assert!(b.read32(0x4200_0000).is_err(), "encrypted entry is not plaintext");
    put(&mut b, 0x6000_237c, 0x600); put(&mut b, 0x6009_601c, 0x500); assert!(b.read32(0x4200_0000).is_err(), "RC_FAST source is not assumed to be XTAL");
}

#[test]
fn all_mmu_page_sizes_and_axi_power_clock_encryption_boundaries_are_checked() {
    let mut b = configured(); cache(&mut b);
    for selector in 0..4 {
        let page = 65536usize >> selector;
        put(&mut b, 0x6000_2384, 4 | selector << 3);
        put(&mut b, 0x6000_2380, 511); put(&mut b, 0x6000_237c, 0x603);
        let addr = 0x4200_0000 + (512 * page - 4) as u32;
        put(&mut b, addr, 0x11223344 + selector);
        assert_eq!(&b.psram[4 * page - 4..4 * page], &(0x11223344 + selector).to_le_bytes());
        assert!(b.read32(addr + 4).is_err(), "MMU table has 512 entries even with smaller pages");
        put(&mut b, 0x6000_2384, 6 | selector << 3); assert!(b.read32(addr).is_err(), "MMU forced power-down");
    }
    cache(&mut b);
    put(&mut b, 0x6000_2380, 512); assert!(b.write32(0x6000_237c, 0x600).is_err(), "out-of-range MMU index does not alias page0");
    assert!(b.read32(0x6000_237c).is_err());
    for source in [0x500, 0x600, 0x700, 0x403, 0, 0xc00] {
        put(&mut b, 0x6009_601c, source); assert!(b.read32(0x4200_0000).is_err(), "unsupported/gated MSPI clock {source:#x}");
    }
    put(&mut b, 0x6009_601c, 0x400);
    put(&mut b, 0x6000_2380, 0); put(&mut b, 0x6000_237c, 0xc00);
    b.flash = vec![0x5a; 1 << 20]; assert!(b.read32(0x4200_0000).is_err(), "encrypted flash does not silently read plaintext either");
}

#[test]
fn serial_host_boundaries_and_the_actual_c5_mspi_interrupt_are_observable() {
    let mut b = configured(); put(&mut b, SPI + 0xc0, 24);
    command(&mut b, 0x02, false, Some(4), &[1, 2, 3, 4], 0, 0);
    put(&mut b, 0x6001_00b0, 17); // C5 MSPI source44, not another chip's source.
    assert_eq!(b.irq_sources_of(0, 1), vec![44]);
    put(&mut b, SPI + 0xc4, 24); assert!(b.irq_sources_of(0, 1).is_empty());
    let saved = b.psram[..8].to_vec();
    put(&mut b, SPI + 0x34, 0);
    assert!(b.write32(SPI, 1 << 18).is_err(), "simultaneous flash/PSRAM selection is not decoded as one die");
    put(&mut b, SPI + 0x34, 1);
    put(&mut b, SPI + 0x24, 1023);
    assert!(b.write32(SPI, 1 << 18).is_err(), "guest cannot send more bytes than the 64-byte FIFO");
    assert_eq!(&b.psram[..8], saved);
    assert!(b.set_psram_size(4 << 20).is_err()); assert_eq!(&b.psram[..8], saved, "invalid capacity is atomic");
    assert_eq!(b.psram.len(), 8 << 20);
    assert!(b.write32(0x6000_2024, 0).is_err(), "reserved SPI0 slot is not SPI1 data length");
    assert!(b.write32(0x6000_234c, 1).is_err(), "unimplemented XTS trigger is not generic register RAM");
}

#[test]
fn controller_and_cpu_resets_preserve_die_cells_and_mode_without_preserving_maps() {
    let mut b = configured(); cache(&mut b); put(&mut b, 0x4200_0000, 0x5a6b7c8d);
    put(&mut b, 0x6009_6018, 7); put(&mut b, 0x6009_6018, 5); assert!(b.read32(0x4200_0000).is_err());
    put(&mut b, SPI + 0x34, 1);
    assert_eq!(command(&mut b, 0xeb, true, Some(0), &[], 4, 6), [0x8d, 0x7c, 0x6b, 0x5a]);
    b.reboot([0; 6]); assert_eq!(b.psram.len(), 8 << 20); assert!(b.read32(0x4200_0000).is_err());
    pads(&mut b); put(&mut b, SPI + 0x34, 1);
    assert_eq!(command(&mut b, 0xeb, true, Some(0), &[], 4, 6), [0x8d, 0x7c, 0x6b, 0x5a]);
    command(&mut b, 0xf5, true, None, &[], 0, 0);
    assert_eq!(command(&mut b, 0x03, false, Some(0), &[], 4, 0), [0x8d, 0x7c, 0x6b, 0x5a]);
}

#[test]
fn original_sram_guest_checks_all_eight_mib_and_executes_through_its_own_mmu_map() {
    let mut m = esp32c5::machine([0; 6], 4 << 20);
    m.bus.set_xtal_mhz(48).unwrap(); m.bus.set_psram_size(8 << 20).unwrap();
    m.console.capture = true; m.console.mask = 2;
    m.load_input(esp_soc::LoadKind::App, include_bytes!("../../firmware/c5-psram-probe/probe.bin")).unwrap();
    m.boot_app(0x10000).unwrap();
    while !m.console.uart0.ends_with(b"C5 PSRAM8 PASS\n") && m.bus.cycles() < 100_000_000 {
        m.max_cycles = m.bus.cycles() + 2_000_000;
        assert!(matches!(m.run(u64::MAX), esp_soc::Stop::MaxInsns | esp_soc::Stop::Halted));
    }
    assert_eq!(m.console.uart0, b"C5 PSRAM8 PASS\n");
    assert_eq!((m.exceptions, m.reboots), (0, 0)); assert!(m.bus.first_fault.is_none());
    assert_eq!(m.bus.psram.len(), 8 << 20);
    assert_eq!(&m.bus.psram[..8], &[0x13, 0x05, 0x50, 0x02, 0x67, 0x80, 0, 0]);
    assert_eq!(m.bus.psram[(8 << 20) - 1], 0xa5);
}
