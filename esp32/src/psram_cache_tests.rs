//! Public-driver-derived normal-clock cache trace, not a successful guest boot.
//! IDF v5.1.4 esp_psram_impl_quad.c:1068-1125 and soc/{spi,dport}_reg.h.
use super::*;
const SPI0: u32 = 0x3ff43000;
const SPI1: u32 = 0x3ff42000;
const SCTRL: u32 = (23 << 22) | (1 << 28) | 0x34;
fn control(b: &mut SocBus, opcode: u32) {
    b.write32(SPI1 + 0x34, 5).unwrap();
    b.write32(SPI1 + 0x1c, 1 << 31).unwrap();
    b.write32(SPI1 + 0x24, 0x70000000 | opcode).unwrap();
    b.write32(SPI1, 1 << 18).unwrap();
}
pub(super) fn enable_cache(b: &mut SocBus) {
    control(b, 0x35);
    for (off, value) in [(0x34, 0), (0x54, SCTRL), (0x5c, 0x700000eb), (0x60, 0x70000038)] {
        b.write32(SPI0 + off, value).unwrap();
    }
    for base in [0x3ff00040, 0x3ff00058] {
        b.write32(base, 8).unwrap();
        b.write32(base + 4, 0).unwrap();
    }
}
fn payload(b: &mut SocBus, addr: u32, write: bool, value: u32) -> u32 {
    b.write32(SPI1 + 0x34, 5).unwrap();
    b.write32(SPI1 + 0x1c, (1 << 31) | (1 << 30) | (1 << if write { 27 } else { 28 })).unwrap();
    b.write32(SPI1 + 0x20, 23 << 26).unwrap();
    b.write32(SPI1 + 0x24, 0x70000000 | if write { 0x38 } else { 0xeb }).unwrap();
    b.write32(SPI1 + 4, addr << 8).unwrap();
    b.write32(SPI1 + 0x28, 31).unwrap();
    b.write32(SPI1 + 0x2c, 31).unwrap();
    b.write32(SPI1 + 0x80, value).unwrap();
    b.write32(SPI1, 1 << 18).unwrap();
    assert_eq!(b.read32(SPI1), Ok(0));
    b.read32(SPI1 + 0x80).unwrap()
}
#[test]
fn cache_trace_registers_never_alias_fifo_or_other_controller() {
    let mut b = configured();
    for base in [SPI0, SPI1] {
        for word in 0..16 { b.write32(base + 0x80 + 4 * word, 0x12340000 + word).unwrap(); }
        for off in (0x54..=0x7c).step_by(4) {
            let value = 0xabcd0000 | off;
            b.write32(base + off, value).unwrap();
            assert_eq!(b.read32(base + off), Ok(value), "retained SRAM/cache offset {off:x}");
        }
        for word in 0..16 { assert_eq!(b.read32(base + 0x80 + 4 * word), Ok(0x12340000 + word)); }
        for word in 0..16 { b.write32(base + 0x80 + 4 * word, 0).unwrap(); }
        for off in (0x54..=0x7c).step_by(4) { assert_eq!(b.read32(base + off), Ok(0xabcd0000 | off)); }
    }
    b.write32(SPI0 + 0x5c, 0x700000eb).unwrap();
    assert_eq!(b.read32(SPI1 + 0x5c), Ok(0xabcd005c));
}
#[test]
fn cache_command_cpu_coherence_remap_and_flash_dirty_isolation() {
    let mut b = configured();
    map(&mut b, 0, 7);
    map(&mut b, 1, 7);
    b.write32(0x3ff10000 + 77 * 4, 3).unwrap();
    let versions = b.page_versions().to_vec();
    let flash = b.flash.clone();
    payload(&mut b, 7 * 32768 + 18, true, 0x12345678);
    assert_eq!(b.read32(LOW + 18), Ok(0x12345678));
    assert_eq!(b.read32(LOW + 32768 + 18), Ok(0x12345678));
    b.write32(LOW + 18, 0x87654321).unwrap();
    assert_eq!(payload(&mut b, 7 * 32768 + 18, false, 0), 0x87654321);
    assert_eq!(b.page_versions(), versions, "PSRAM dirty record must not invalidate flash aliases");
    assert_eq!(b.flash, flash);
    map(&mut b, 0, 8);
    assert_eq!(b.read32(LOW + 18), Ok(0));
    map(&mut b, 0, 7);
    assert_eq!(b.read32(LOW + 18), Ok(0x87654321));
    b.write32(0x3ff00040, 8 | 16).unwrap();
    assert_eq!(b.read32(0x3ff00040), Ok(8 | 32), "invalidate completes synchronously");
    assert_eq!(b.read32(LOW + 18), Ok(0x87654321), "invalidate does not clear bytes");
}
#[test]
fn cache_gates_fault_and_restore_without_losing_bytes() {
    let mut b = configured();
    map(&mut b, 0, 0);
    b.write32(LOW, 0x12345678).unwrap();
    let mut cases = vec![(SPI0 + 0x34, 2), (SPI0 + 0x54, SCTRL & !(1 << 2)),
        (SPI0 + 0x54, SCTRL | 2), (SPI0 + 0x54, SCTRL & !(1 << 4)),
        (SPI0 + 0x54, SCTRL ^ (1 << 22)), (SPI0 + 0x5c, 0x70000003),
        (SPI0 + 0x5c, 0x600000eb), (SPI0 + 0x60, 0x70000002),
        (SPI0 + 0x60, 0x60000038), (SPI0 + 0x54, SCTRL & !(1 << 5)),
        (SPI0 + 0x54, SCTRL & !(1 << 28))];
    for (ctrl, high) in [(0x3ff00040, 16), (0x3ff00058, 14)] {
        for value in [0, 8 | (1 << 11), 8 | (1 << high)] { cases.push((ctrl, value)); }
        for value in [8, 32, 64, 128, 256] { cases.push((ctrl + 4, value)); }
    }
    for (reg, value) in cases {
        let before = b.read32(reg).unwrap();
        b.write32(reg, value).unwrap();
        let read_only = reg == SPI0 + 0x60 || (reg == SPI0 + 0x54 && value == SCTRL & !(1 << 28));
        let write_only = reg == SPI0 + 0x5c || (reg == SPI0 + 0x54 && value == SCTRL & !(1 << 5));
        if !read_only { assert_eq!(b.read32(LOW), Err(Fault::Prohibited), "read gate {reg:x}={value:x}"); }
        if !write_only { assert_eq!(b.write32(LOW, 0), Err(Fault::Prohibited), "write gate {reg:x}={value:x}"); }
        b.write32(reg, before).unwrap();
        assert_eq!(b.read32(LOW), Ok(0x12345678));
    }
    control(&mut b, 0xf5);
    assert_eq!(b.read8(LOW), Err(Fault::Prohibited), "device mode gates mapping");
    assert_eq!(b.write8(LOW, 0), Err(Fault::Prohibited));
    control(&mut b, 0x35);
    assert_eq!(b.read32(LOW), Ok(0x12345678));
    map(&mut b, 0, 256);
    assert_eq!(b.read32(LOW), Err(Fault::Unmapped));
    map(&mut b, 0, 0);
    assert_eq!(b.read32(LOW), Ok(0x12345678));
    b.write32(0x3ff00058, 0).unwrap();
    b.write32(0x3ff0002c, 1).unwrap();
    assert_eq!(b.read32(LOW), Ok(0x12345678), "held APP does not gate PRO-only startup");
}
#[test]
fn cache_reboot_resets_controls_device_mappings_and_bytes_in_place() {
    let mut b = configured();
    map(&mut b, 0, 0);
    b.write32(LOW, 0xdeadbeef).unwrap();
    control(&mut b, 0x66); // armed reset must not survive reboot
    let ptr = b.psram.as_ptr();
    b.reboot([0; 6]);
    assert_eq!(b.psram.as_ptr(), ptr);
    assert_eq!(b.psram.len(), 4 << 20, "host presence is retained by P0 lifecycle");
    assert!(b.psram.iter().all(|&v| v == 0));
    for base in [SPI0, SPI1] {
        for off in (0x54..=0x7c).step_by(4) { assert_eq!(b.read32(base + off), Ok(0)); }
    }
    assert!(!b.periph.spi1.0.classic_qpi_enabled());
    for addr in [0x3ff00040, 0x3ff00044, 0x3ff00058, 0x3ff0005c] { assert_eq!(b.read32(addr), Ok(0)); }
    assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
    map(&mut b, 0, 0);
    assert_eq!(b.read8(LOW), Err(Fault::Prohibited), "remapping alone cannot restore cache");
    enable_cache(&mut b);
    control(&mut b, 0x99);
    assert!(b.periph.spi1.0.classic_qpi_enabled(), "old reset enable was cleared");
    assert_eq!(b.read32(LOW), Ok(0));
    b.set_psram_size(0).unwrap();
    map(&mut b, 0, 0);
    assert_eq!(b.read8(LOW), Err(Fault::Unmapped));
    b.reboot([0; 6]);
    assert!(b.psram.is_empty());
}
