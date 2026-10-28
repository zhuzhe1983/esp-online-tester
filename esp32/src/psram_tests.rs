use super::*;
use xtensa_lx7::bus::Bus;
use esp_soc::SocBus as _;
#[path = "psram_cache_tests.rs"]
mod cache_tests;
use cache_tests::enable_cache;
#[path = "psram_lowhigh_tests.rs"]
mod lowhigh_tests;

#[test]
fn sram_miss_dispatch_preserves_window_and_absent_faults() {
    let mut b = configured();
    map(&mut b, 0, 0);
    map(&mut b, 127, 127);
    for a in [0x3f800000, 0x3fbffffc] {
        assert_eq!(b.write32(a, 0x12345678), Ok(()), "PSRAM store miss dispatch");
        assert_eq!(b.read32(a), Ok(0x12345678), "PSRAM load miss dispatch");
        assert_eq!(b.fetch(a), Err(Fault::Prohibited), "PSRAM is data only");
    }
    for a in [0x3f7fffff, 0x3fc00000] {
        assert_eq!(b.read8(a), Err(Fault::Unmapped));
        assert_eq!(b.write8(a, 1), Err(Fault::Prohibited));
        assert_eq!(b.fetch(a), Err(Fault::Unmapped), "outside PSRAM window");
        assert_eq!(b.last_fault, Some((a, false)));
    }
    b.set_psram_size(0).unwrap();
    assert!(b.psram.is_empty(), "absent device has no backing");
    enable_cache(&mut b);
    map(&mut b, 0, 0);
    for a in [0x3f800000, 0x3fbffffc] {
        assert_eq!(b.read32(a), Err(Fault::Unmapped));
        assert_eq!(b.write32(a, 1), Err(Fault::Unmapped));
        assert_eq!(b.fetch(a), Err(Fault::Prohibited));
    }
}

// Literal addresses/stride intentionally independent of translator constants.
fn map(b: &mut SocBus, slot: u32, page: u32) {
    for base in [0x3ff11200, 0x3ff13200] {
        b.write32(base + slot * 4, page).unwrap();
        assert_eq!(b.read32(base + slot * 4), Ok(page));
    }
}
fn configured() -> SocBus {
    let mut b = SocBus::new(4 << 20, [0; 6]);
    b.set_psram_size(4 << 20).unwrap();
    b.write32(0x3ff00030, 1).unwrap();
    b.write32(0x3ff0002c, 0).unwrap();
    enable_cache(&mut b);
    b
}
#[test]
fn sram_all_128_pages_widths_edges_and_physical_aliases() {
    let mut b = configured();
    for slot in 0..128 {
        // Reverse physical order distinguishes 32K SRAM from 64K flash indexing.
        let page = 127 - slot;
        map(&mut b, slot, page);
        let a = 0x3f800000 + slot * 32768;
        for delta in [0, 32767] {
            b.write8(a + delta, slot as u8 ^ 0x5a).unwrap();
            assert_eq!(b.read8(a + delta), Ok(slot as u8 ^ 0x5a));
            assert_eq!(b.psram[(page * 32768 + delta) as usize], slot as u8 ^ 0x5a);
        }
        for delta in [0, 32766] {
            b.write16(a + delta, slot as u16 ^ 0xa55a).unwrap();
            assert_eq!(b.read16(a + delta), Ok(slot as u16 ^ 0xa55a));
        }
        for delta in [0, 32764] {
            b.write32(a + delta, slot ^ 0x12345678).unwrap();
            assert_eq!(b.read32(a + delta), Ok(slot ^ 0x12345678));
            let off = (page * 32768 + delta) as usize;
            assert_eq!(&b.psram[off..off + 4], &(slot ^ 0x12345678).to_le_bytes());
        }
    }
    // Two virtual pages refer to the same physical bytes, not two allocations.
    map(&mut b, 1, 127);
    b.write32(0x3f800020, 0xfeedbeef).unwrap();
    assert_eq!(b.read32(0x3f808020), Ok(0xfeedbeef));
    b.psram[127 * 32768 + 32] = 7;
    assert_eq!(b.read8(0x3f808020), Ok(7));
}
#[test]
fn sram_cross_page_access_validates_all_bytes_before_write() {
    let mut b = configured();
    map(&mut b, 0, 7);
    map(&mut b, 1, 3);
    for a in [0x3f807ffd, 0x3f807ffe, 0x3f807fff] {
        // Bus already permits unaligned data; CPU alignment checks remain unchanged.
        b.write32(a, 0x76543210).unwrap();
        assert_eq!(b.read32(a), Ok(0x76543210));
    }
    b.write16(0x3f807fff, 0xa55a).unwrap();
    assert_eq!(b.read16(0x3f807fff), Ok(0xa55a));
    assert_eq!(b.psram[7 * 32768 + 32767], 0x5a);
    assert_eq!(b.psram[3 * 32768], 0xa5);
    map(&mut b, 1, 0x100);
    let before = b.psram.clone();
    assert_eq!(b.write32(0x3f807fff, 0), Err(Fault::Unmapped));
    assert_eq!(b.write16(0x3f807fff, 0), Err(Fault::Unmapped));
    assert_eq!(b.read32(0x3f807fff), Err(Fault::Unmapped));
    assert_eq!(b.psram, before);
    assert_eq!(b.write16(0x3fbfffff, 1), Err(Fault::Unmapped));
    assert_eq!(b.read32(0x3fbfffff), Err(Fault::Unmapped));
}
#[test]
fn sram_remap_absent_invalid_bounds_and_both_banks() {
    let mut b = SocBus::new(0, [0; 6]);
    map(&mut b, 0, 0);
    assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
    assert_eq!(b.write8(LOW, 1), Err(Fault::Unmapped));
    b.set_psram_size(4 << 20).unwrap();
    assert_eq!(b.read8(LOW), Err(Fault::Unmapped), "configuration invalidates tables");
    enable_cache(&mut b);
    map(&mut b, 0, 0);
    b.write8(LOW, 42).unwrap();
    b.write32(0x3ff00030, 1).unwrap();
    b.write32(0x3ff0002c, 0).unwrap();
    // Either table changes translation; divergent entries never silently use PRO.
    for base in [0x3ff11200, 0x3ff13200] {
        b.write32(base, 1).unwrap();
        assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
        assert_eq!(b.write8(LOW, 99), Err(Fault::Unmapped));
        b.write32(base, 0).unwrap();
    }
    map(&mut b, 0, 1);
    assert_eq!(b.read8(LOW), Ok(0));
    map(&mut b, 0, 0);
    assert_eq!(b.read8(LOW), Ok(42));
    for entry in [128, 255, 256] {
        map(&mut b, 0, entry);
        assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
        assert_eq!(b.write32(LOW, 0), Err(Fault::Unmapped));
    }
    assert_eq!(b.last_fault, Some((LOW, true)));
    assert_eq!(b.fetch(LOW), Err(Fault::Prohibited));
    assert_eq!(b.read8(HIGH), Err(Fault::Unmapped));
}
#[test]
fn sram_pro_only_startup_then_live_app_requires_match() {
    let mut b = SocBus::new(0, [0; 6]);
    b.set_psram_size(4 << 20).unwrap();
    enable_cache(&mut b);
    b.write32(0x3ff11200, 0).unwrap();
    b.write8(LOW, 77).unwrap();
    assert_eq!(b.read8(LOW), Ok(77), "PRO-only before APP release");
    b.write32(0x3ff00030, 1).unwrap();
    b.write32(0x3ff0002c, 0).unwrap();
    assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
    b.write32(0x3ff13200, 0).unwrap();
    assert_eq!(b.read8(LOW), Ok(77));
    assert_eq!(b.dma_read_word(LOW), None);
    assert!(!b.dma_write_word(LOW, 0));
    assert_eq!(b.read8(LOW), Ok(77));
}
#[test]
fn sram_configuration_reset_loader_and_flash_separation() {
    let mut b = configured();
    map(&mut b, 0, 0);
    b.load_bytes(LOW, &[1, 2, 3, 4]).unwrap();
    assert_eq!(b.read32(LOW), Ok(0x04030201));
    b.write32(0x3ff10000, 0).unwrap();
    b.write_flash(0, &[9, 8, 7, 6]).unwrap();
    assert_eq!(b.read32(0x3f400000), Ok(0x06070809));
    assert_eq!(b.read32(LOW), Ok(0x04030201));
    let ptr = b.psram.as_ptr();
    for size in [1, 2 << 20, 8 << 20] {
        assert!(b.set_psram_size(size).is_err());
        assert_eq!(b.psram.as_ptr(), ptr);
        assert_eq!(b.read32(LOW), Ok(0x04030201));
    }
    b.reboot([0; 6]);
    assert_eq!(b.psram.as_ptr(), ptr);
    assert_eq!(b.psram.len(), 4 << 20);
    assert!(b.psram.iter().all(|&v| v == 0));
    assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
    assert!(b.load_bytes(LOW, &[1]).is_err());
    assert!(b.mmu.iter().all(|t| t[1152..1280].iter().all(|&v| v == 256)));
    b.set_psram_size(0).unwrap();
    map(&mut b, 0, 0);
    assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
    assert!(b.psram.is_empty());
    let fresh = configured();
    assert!(fresh.psram.iter().all(|&v| v == 0));
}
