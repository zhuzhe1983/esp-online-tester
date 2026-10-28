use super::*;
use esp_soc::Soc;
use emu_core::Core;

fn lowhigh() -> SocBus {
    let mut b = configured();
    b.write32(0x3ff00040, 8 | (1 << 16)).unwrap();
    b.write32(0x3ff00058, 8 | (1 << 14)).unwrap();
    // Deliberately divergent/invalid UNUSED halves: catches falling back to PRO,
    // demanding matched tables, or folding APP slot 64 down to slot 0.
    for page in 0..128 {
        b.write32(0x3ff11200 + page * 4, if page < 64 { 63 - page } else { 256 }).unwrap();
        b.write32(0x3ff13200 + page * 4, if page >= 64 { 191 - page } else { 256 }).unwrap();
    }
    b
}

#[test]
fn lowhigh_both_lx6_cpus_load_store_all_widths_both_halves() {
    let mut b = lowhigh();
    for core in 0..2 {
        let mut cpu = crate::Esp32::new_core(core);
        for (addr, physical) in [(0x3f800000, 63 * 32768), (0x3fa00000, 127 * 32768),
            (0x3f9f8000, 0), (0x3fbf8000, 64 * 32768)] {
            for (kind, mask, value) in [(0, 0xff, 0xa5), (1, 0xffff, 0xb6a5), (2, u32::MAX, 0xd8c7b6a5)] {
                // s8i/s16i/s32i a3,a4,0; l8ui/l16ui/l32i a5,a4,0.
                let program = [0x32, 0x44 + (kind << 4), 0, 0x52, 0x04 + (kind << 4), 0];
                b.load_bytes(0x40080000, &program).unwrap();
                cpu.pc = 0x40080000;
                cpu.set_ar(4, addr);
                cpu.set_ar(3, value);
                assert!(cpu.step(&mut b).trap().is_none(), "core {core} store width {kind}");
                assert_eq!(b.read32(addr).unwrap() & mask, value, "store data core {core}");
                assert_eq!(u32::from_le_bytes(b.psram[physical..physical + 4].try_into().unwrap()) & mask, value);
                assert!(cpu.step(&mut b).trap().is_none(), "core {core} load width {kind}");
                assert_eq!(cpu.get_ar(5), value, "load data core {core}");
            }
        }
    }
}

#[test]
fn lowhigh_cross_half_atomic_validation_and_table_selection() {
    let mut b = lowhigh();
    for a in [0x3f9ffffd, 0x3f9ffffe, 0x3f9fffff] {
        b.write32(a, 0x76543210).unwrap();
        assert_eq!(b.read32(a), Ok(0x76543210));
    }
    b.write16(0x3f9fffff, 0xabcd).unwrap();
    assert_eq!(b.read16(0x3f9fffff), Ok(0xabcd));
    assert_eq!(b.psram[32767], 0xcd);
    assert_eq!(b.psram[127 * 32768], 0xab);
    for (entry, valid) in [(0x3ff112fc, 0), (0x3ff13300, 127)] {
        b.write32(entry, 256).unwrap();
        let before = b.psram.clone();
        assert_eq!(b.read32(0x3f9fffff), Err(Fault::Unmapped));
        assert_eq!(b.write32(0x3f9fffff, 0), Err(Fault::Unmapped));
        assert_eq!(b.psram, before, "failed crossing cannot partly write");
        b.write32(entry, valid).unwrap();
    }
}

#[test]
fn lowhigh_cache_disable_masks_and_page_mode_are_per_half() {
    let mut b = lowhigh();
    for (ctrl, mode, blocked, allowed) in [(0x3ff00040, 16, 0x3f800000, 0x3fa00000),
        (0x3ff00058, 14, 0x3fa00000, 0x3f800000)] {
        for (reg, value, restore) in [(ctrl, 1 << mode, 8 | (1 << mode)),
            (ctrl + 4, 8, 0), (ctrl + 4, 32, 0), (ctrl + 4, 64, 0),
            (ctrl + 4, 128, 0), (ctrl + 4, 256, 0)] {
            b.write32(reg, value).unwrap();
            let before = b.psram.clone();
            assert_eq!(b.read32(blocked), Err(Fault::Prohibited));
            assert_eq!(b.write32(blocked, 3), Err(Fault::Prohibited));
            assert_eq!(b.write32(0x3f9fffff, 3), Err(Fault::Prohibited));
            assert_eq!(b.psram, before);
            b.write32(allowed, 0x12345678).unwrap();
            assert_eq!(b.read32(allowed), Ok(0x12345678));
            b.write32(reg, restore).unwrap();
        }
    }
    // Cache ownership is independent of whether the APP processor is held.
    b.write32(0x3ff0002c, 1).unwrap();
    b.write32(0x3ff00058, 1 << 14).unwrap();
    assert!(b.read32(0x3f800000).is_ok());
    assert_eq!(b.read32(0x3fa00000), Err(Fault::Prohibited));
}

#[test]
fn lowhigh_split_combined_and_live_mixed_modes_rejected() {
    let mut b = lowhigh();
    for (pro, app) in [(8 | (1 << 11), 8 | (1 << 11)),
        (8 | (1 << 11), 8), (8 | (1 << 16) | (1 << 11), 8 | (1 << 14)),
        (8 | (1 << 16) | (1 << 11), 8 | (1 << 14) | (1 << 11)),
        (8 | (1 << 16), 8), (8, 8 | (1 << 14)),
        (8 | (1 << 16), 8 | (1 << 14) | (1 << 11))] {
        b.write32(0x3ff00040, pro).unwrap();
        b.write32(0x3ff00058, app).unwrap();
        for addr in [0x3f800000, 0x3fa00000] {
            assert_eq!(b.read32(addr), Err(Fault::Prohibited));
            assert_eq!(b.write32(addr, 0), Err(Fault::Prohibited));
        }
    }
}
