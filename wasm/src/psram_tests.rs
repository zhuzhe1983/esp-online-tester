//! Qualification-only exports: deliberately absent from the production module.
//! Run identical bus assertions in native tests and real wasm32, not a host mock.
use super::*;
use xtensa_lx7::bus::{Bus, Fault};
const LOW: u32 = 0x3f800000;
fn map(b: &mut esp32::bus::SocBus, slot: u32, page: u32) {
    for base in [0x3ff11200, 0x3ff13200] { b.write32(base + slot * 4, page).unwrap(); }
}
fn enable(b: &mut esp32::bus::SocBus) {
    // IDF v5.1.4 esp_psram_impl_quad.c:1068-1125; v4.4.7 dport_reg.h.
    for (a, v) in [(0x3ff42034, 5), (0x3ff4201c, 1 << 31),
        (0x3ff42024, 0x70000035), (0x3ff42000, 1 << 18),
        (0x3ff43034, 0), (0x3ff43054, (23 << 22) | (1 << 28) | 0x34),
        (0x3ff4305c, 0x700000eb), (0x3ff43060, 0x70000038),
        (0x3ff00040, 8), (0x3ff00044, 0), (0x3ff00058, 8), (0x3ff0005c, 0)] {
        b.write32(a, v).unwrap();
    }
}
fn physical_word(b: &mut esp32::bus::SocBus, addr: u32) -> u32 {
    // Independent SPI1 command path: physical 24-bit byte address, no SRAM table.
    for (a, v) in [(0x3ff42034, 5), (0x3ff4201c, (1 << 31) | (1 << 30) | (1 << 28)),
        (0x3ff42020, 23 << 26), (0x3ff42024, 0x700000eb), (0x3ff42004, addr << 8),
        (0x3ff4202c, 31), (0x3ff42000, 1 << 18)] { b.write32(a, v).unwrap(); }
    b.read32(0x3ff42080).unwrap()
}

/// Same factory, mapping, fault and lifecycle assertions on both compilation targets.
#[no_mangle]
pub extern "C" fn esp32sim_test_classic_psram() -> u32 {
    // Existing manifest defaults (T-Display: 0 || 2), upload defaults and ABI callers.
    for (board, flash, size) in [("esp32", 4, 1), ("esp32", 4, 2),
        ("esp32", 4, 8), ("esp32", 4, 32), ("ttgo-tdisplay", 4, 2),
        ("esp32", 32, 32)] {
        // SAFETY: the board string is readable for its declared length.
        let p = unsafe { esp32sim_new(board.as_ptr(), board.len(), flash, size) };
        assert!(!p.is_null(), "unsupported classic PSRAM size must start without PSRAM");
        {
            // SAFETY: p is checked and exclusively owned for this scope.
            let e = unsafe { &mut *p };
            let MachineKind::Esp32(m) = &mut e.m else { panic!("classic factory") };
            let b = &mut m.bus;
            assert_eq!(b.read32(LOW), Err(Fault::Unmapped));
            enable(b);
            map(b, 0, 0);
            assert_eq!(b.read32(LOW), Err(Fault::Unmapped), "unsupported size leaves PSRAM absent");
            assert_eq!(b.write32(LOW, 0x12345678), Err(Fault::Unmapped));
            assert!(b.set_psram_size((size as usize) << 20).is_err(), "native setter stays strict");
        }
        // SAFETY: p is live, uniquely owned, and not subsequently used.
        unsafe { esp32sim_delete(p) };
    }
    for size in [33, u32::MAX] {
        // SAFETY: the board literal is readable for its declared length.
        assert!(unsafe { esp32sim_new(b"esp32".as_ptr(), 5, 4, size) }.is_null(), "PSRAM cap still rejects oversized requests");
    }
    for size in [4, 0, 4, 0] {
        // SAFETY: factory returns unique ownership, consumed by delete after the borrow ends.
        let p = unsafe { esp32sim_new(b"esp32".as_ptr(), 5, 4, size) };
        assert!(!p.is_null());
        {
            // SAFETY: p is checked and exclusively owned for this scope.
            let e = unsafe { &mut *p };
            let MachineKind::Esp32(m) = &mut e.m else { panic!("classic factory") };
            let b = &mut m.bus;
            assert!(b.fast_mem().is_none(), "classic publishes no raw fast-memory pointer");
            assert_eq!(b.read32(LOW), Err(Fault::Unmapped), "fresh machine has no stale mappings");
            enable(b);
            if size == 0 {
                map(b, 0, 0);
                assert_eq!(b.read32(LOW), Err(Fault::Unmapped), "absent device cannot be mapped");
            } else {
                for slot in 0..128 {
                    let page = 127 - slot;
                    map(b, slot, page);
                    let a = LOW + slot * 32768;
                    assert_eq!(b.read32(a), Ok(0), "fresh physical pages contain no stale bytes");
                    b.write8(a + 32767, slot as u8 ^ 0x5a).unwrap();
                    assert_eq!(b.read8(a + 32767), Ok(slot as u8 ^ 0x5a));
                    b.write16(a + 32766, slot as u16 ^ 0xa55a).unwrap();
                    assert_eq!(b.read16(a + 32766), Ok(slot as u16 ^ 0xa55a));
                    b.write32(a, 0x12345678 ^ slot).unwrap();
                    assert_eq!(b.read32(a), Ok(0x12345678 ^ slot));
                    assert_eq!(physical_word(b, page * 32768), 0x12345678 ^ slot, "physical page/shift oracle");
                }
                map(b, 1, 127);
                assert_eq!(b.read32(LOW + 32768), Ok(0x12345678), "remap sees alias, not stale page");
                map(b, 0, 126);
                assert_eq!(b.read32(LOW), Ok(0x12345679), "remap selects different physical bytes");
                map(b, 1, 3);
                b.write32(LOW + 32766, 0x76543210).unwrap();
                assert_eq!(b.read32(LOW + 32766), Ok(0x76543210), "noncontiguous crossing");
                map(b, 1, 256);
                let before = physical_word(b, 126 * 32768 + 32764);
                assert_eq!(b.write32(LOW + 32766, 0), Err(Fault::Unmapped));
                assert_eq!(physical_word(b, 126 * 32768 + 32764), before, "invalid second page cannot partially write");
                map(b, 0, 128);
                assert_eq!(b.read32(LOW), Err(Fault::Unmapped), "physical bounds");
                map(b, 0, 127);
                b.write32(0x3ff00040, 0).unwrap();
                assert_eq!(b.read32(LOW), Err(Fault::Prohibited), "cache disabled");
                b.write32(0x3ff00040, 8).unwrap();
                b.write32(0x3ff00044, 32).unwrap();
                assert_eq!(b.read32(LOW), Err(Fault::Prohibited), "cache masked");
                b.write32(0x3ff00044, 0).unwrap();
                assert_eq!(b.read32(LOW), Ok(0x12345678), "cache restore retains original bytes");
                m.reboot();
                let b = &mut m.bus;
                assert!(b.fast_mem().is_none(), "reset cannot leave published PSRAM pointers");
                for base in [0x3ff11200, 0x3ff13200] {
                    for slot in 0..128 { assert_eq!(b.read32(base + slot * 4), Ok(256), "reset clears every SRAM map"); }
                }
                assert_eq!(b.read32(LOW), Err(Fault::Unmapped));
                map(b, 0, 0);
                assert_eq!(b.read32(LOW), Err(Fault::Prohibited), "reset clears device/cache controls");
                enable(b);
                assert_eq!(b.read32(LOW), Ok(0), "re-enabled reset mapping reads zero");
                for slot in 0..128 {
                    map(b, slot, slot);
                    for off in (0..32768).step_by(4) {
                        assert_eq!(b.read32(LOW + slot * 32768 + off), Ok(0), "reset clears all 4 MiB");
                    }
                }
                b.set_psram_size(0).unwrap();
                map(b, 0, 0);
                assert_eq!(b.read32(LOW), Err(Fault::Unmapped));
                b.set_psram_size(4 << 20).unwrap();
                assert_eq!(b.read32(LOW), Err(Fault::Unmapped), "resize invalidates mappings");
                enable(b); map(b, 0, 0);
                assert_eq!(b.read32(LOW), Ok(0), "resize does not reuse stale bytes");
            }
        }
        // SAFETY: p is live, uniquely owned, and not subsequently used.
        unsafe { esp32sim_delete(p) };
    }
    4
}

/// Reset the real guest through the same Machine::reboot path as software reset.
/// # Safety
/// e must be a live exclusively owned classic emulator returned by esp32sim_new.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_test_classic_reboot(e: *mut Emu) {
    // SAFETY: caller guarantees a live exclusive emulator.
    let e = unsafe { &mut *e };
    let MachineKind::Esp32(m) = &mut e.m else { panic!("classic only") };
    assert!(m.stubs.is_empty());
    assert_eq!(m.stub_hits, 0, "no guest API interception");
    m.reboot();
    assert!(m.bus.fast_mem().is_none());
    assert_eq!(m.bus.read32(LOW), Err(Fault::Unmapped));
    e.out.clear();
}

#[test]
fn classic_psram_abi_lifecycle() { assert_eq!(esp32sim_test_classic_psram(), 4); }

/// Qualification-only counter, since the production profiling query is S3-only.
/// # Safety
/// e must be a live exclusively owned classic emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_test_classic_jit_insns(e: *mut Emu) -> f64 {
    // SAFETY: caller supplies a live exclusive emulator.
    let e = unsafe { &mut *e };
    let MachineKind::Esp32(m) = &mut e.m else { panic!("classic only") };
    m.cores.iter().map(|c| c.blocks.jit_instructions).sum::<u64>() as f64
}
