//! Original pinned ROM LP elapsed -> native correction protocol, never guest patches.
use emu_core::Bus;
const RETURN: u32 = 0x40000400;
fn machine() -> esp32::Machine {
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.load_rom(&std::fs::read(esp_soc::rom::require("esp32_rev300_rom.elf")).unwrap())
        .unwrap();
    m.bus.periph.write32(0x3ff000d0, 0);
    m.bus.periph.write32(0x3ff000cc, 1 << 11);
    // Observed integer divider configuration with an explicit500kHz nominal fixture.
    m.bus.periph.write32(0x3ff000d4, 79);
    m.bus.periph.write32(0x3ff000d8, 0x04000001);
    m
}
#[test]
#[ignore = "needs pinned esp32_rev300_rom.elf; original LP-cycle conversion and correction writes"]
fn external_rom_lp_elapsed_drives_real_correction_request_without_hci_or_irq_zero() {
    let mut m = machine();
    m.bus.periph.write32(0x3ff71030, 7);
    m.bus.periph.tick(24000);
    m.bus.periph.tick(32 * 480);
    let status = m.bus.periph.read32(0x3ff71030);
    m.bus.periph.write32(0x3ff71030, status | 16);
    m.bus.periph.tick(24000);
    let elapsed = m.bus.periph.read32(0x3ff71038);
    assert_eq!(elapsed, 82);
    m.bus.periph.write32(0x3ff71018, 4);
    let ip = m.bus.read32_unpriced(0x3ffae70c).unwrap();
    assert_eq!(
        m.bus.read32_unpriced(ip + 0xb8c).unwrap(),
        0,
        "test expects original ROM fallback conversion, not SDK override"
    );
    let c = &mut m.cores[0];
    c.pc = 0x400557fc;
    c.ps = (1 << 18) | (1 << 16);
    c.windowbase = 0;
    c.windowstart = 1;
    c.set_ar(1, 0x3ffe8000);
    c.set_ar(4, RETURN);
    for _ in 0..3000 {
        if m.cores[0].pc == RETURN {
            break;
        }
        xtensa_lx7::step(&mut m.cores[0], &mut m.bus).unwrap();
        m.bus.tick(1);
    }
    assert_eq!(
        m.cores[0].pc, RETURN,
        "original ROM sleep wakeup must return"
    );
    assert_ne!(m.bus.periph.bt_sleep.read(0x44) & (1 << 31), 0);
    assert_ne!(m.bus.periph.bt_sleep.read(0x30) & 8, 0);
    assert_eq!(m.bus.periph.bt_sleep.read(0x0c), 1);
    let fine = m.bus.periph.bt_sleep.read(0x40);
    let coarse = m.bus.periph.bt_sleep.read(0x44) & 0xfffffff;
    // ROM fallback is32768Hz LP conversion, independently bounded here; our
    //500kHz fixture is intentionally different, so this checks the real codec,
    // not calibrated elapsed wall time. Never retune an oscillator to force init.
    let encoded_us = coarse as u64 * 625 / 2 + if fine == 624 { 0 } else { 625 - fine as u64 };
    assert!(
        encoded_us.abs_diff(elapsed as u64 * 1_000_000 / 32768) <= 2,
        "elapsed={elapsed} fine={fine} coarse={coarse}"
    );
    m.bus.tick(240);
    assert_eq!(m.bus.periph.bt_sleep.corrections, 1);
    assert_eq!(m.bus.periph.bt_sleep.read(0x30) & 8, 0);
    assert_eq!(
        m.bus.periph.bt_sleep.read(0x14),
        0,
        "correction completion is not permission to invent IRQ0"
    );
}

#[test]
#[ignore = "needs pinned esp32_rev300_rom.elf; native counter carry dispatches actual original IRQ handler"]
fn external_rom_native_irq_zero_dispatches_clock_isr_from_live_counter_boundary() {
    let mut m = machine();
    m.bus.periph.write32(0x3ff000cc, 0x30800);
    m.bus.periph.write32(0x3ff7100c, 1);
    let ip = m.bus.read32_unpriced(0x3ffae70c).unwrap();
    assert_eq!(m.bus.read32_unpriced(ip + 0x640).unwrap(), 0x4003a7a8);
    m.bus.periph.tick(149999);
    assert_eq!(m.bus.periph.read32(0x3ff71010), 0);
    m.bus.periph.tick(1);
    assert_eq!(m.bus.periph.read32(0x3ff71010), 1);
    let c = &mut m.cores[0];
    c.pc = 0x40055248;
    c.ps = (1 << 18) | (1 << 16);
    c.windowbase = 0;
    c.windowstart = 1;
    c.set_ar(1, 0x3ffe8000);
    c.set_ar(4, RETURN);
    for _ in 0..10000 {
        if m.cores[0].pc == RETURN {
            break;
        }
        xtensa_lx7::step(&mut m.cores[0], &mut m.bus).unwrap();
        m.bus.tick(1);
    }
    assert_eq!(
        m.cores[0].pc, RETURN,
        "original native IRQ0 handler must complete"
    );
    assert_eq!(
        m.bus.periph.read32(0x3ff71010),
        0,
        "original handler acknowledged real source"
    );
    assert!(
        m.bus.periph.bt_clock.completed > 0,
        "r_ld_fm_clk_isr sampled native clock"
    );
}
