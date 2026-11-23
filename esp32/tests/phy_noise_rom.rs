//! Execute the original pinned ROM getters, not a host-side rewrite of their algorithm.
use emu_core::Bus;
const RETURN: u32 = 0x4000_0400;
fn setup(pc: u32, enabled: bool) -> esp32::Machine {
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.load_rom(&std::fs::read(esp_soc::rom::require("esp32_rev300_rom.elf")).unwrap())
        .unwrap();
    m.bus.periph.write32(
        0x3ff0_00cc,
        if enabled {
            (1 << 11) | (1 << 16) | (1 << 17)
        } else {
            0
        },
    );
    m.bus.periph.phy_noise.set_input_raw(640);
    m.bus.periph.phy_noise.write(0x38, 1 << 26);
    m.bus.periph.phy_noise.write(0x80, 1);
    let c = &mut m.cores[0];
    c.pc = pc;
    c.ps = (1 << 18) | (1 << 16);
    c.windowbase = 0;
    c.windowstart = 1;
    c.set_ar(1, 0x3ffe_8000);
    c.set_ar(4, RETURN);
    m
}
fn steps(m: &mut esp32::Machine, n: usize) -> bool {
    for _ in 0..n {
        if m.cores[0].pc == RETURN {
            return true;
        }
        xtensa_lx7::step(&mut m.cores[0], &mut m.bus).unwrap();
        m.bus.tick(1);
    }
    m.cores[0].pc == RETURN
}
#[test]
#[ignore = "needs pinned esp32_rev300_rom.elf; actual ROM noise conversion"]
fn external_rom_noise_measurement_trigger_and_result_are_not_constant_success() {
    // Original ROM clamps decoded raw-1024 into [-392,-360]. Full SDK factory
    // has a different upper clamp; these independent expected vectors target ROM.
    for (raw, expected) in [(600, -392i32), (640, -384), (650, -374), (700, -360)] {
        let mut m = setup(0x40003c78, true);
        m.bus.periph.phy_noise.set_input_raw(raw);
        assert!(
            steps(&mut m, 30000),
            "ROM noise getter stuck {:#x}",
            m.cores[0].pc
        );
        assert_eq!(m.cores[0].get_ar(6) as i32, expected);
        assert_eq!(m.bus.periph.phy_noise.completed, 1);
    }
}
#[test]
#[ignore = "needs pinned esp32_rev300_rom.elf; actual ROM gated negative control"]
fn external_rom_noise_measurement_waits_when_clock_is_disabled_then_resumes() {
    let mut m = setup(0x40003c78, false);
    assert!(!steps(&mut m, 5000));
    assert_eq!(m.bus.periph.phy_noise.completed, 0);
    m.bus.periph.write32(0x3ff0_00cc, 1 << 11);
    assert!(steps(&mut m, 30000));
    assert_eq!(m.cores[0].get_ar(6) as i32, -384);
    assert_eq!(m.bus.periph.phy_noise.completed, 1);
}

#[test]
#[ignore = "needs pinned esp32_rev300_rom.elf; execute original BT reset prefix"]
fn external_rom_controller_revision_and_two_soft_reset_handshakes() {
    let mut m = setup(0x4003c714, true);
    m.bus
        .periph
        .write32(0x3ff0_00cc, (1 << 11) | (1 << 16) | (1 << 17));
    let mut reached = false;
    for _ in 0..30000 {
        if m.cores[0].pc == 0x4003c79b {
            reached = true;
            break;
        }
        xtensa_lx7::step(&mut m.cores[0], &mut m.bus).unwrap();
        m.bus.tick(1);
    }
    assert!(
        reached,
        "original reset prefix stuck at {:#x}",
        m.cores[0].pc
    );
    assert_eq!(m.bus.periph.bt_init.completed, 2);
    assert_eq!(m.bus.periph.bt_init.read(0) & 0xc0000000, 0);
    assert_eq!(m.bus.read32(0x3ff71004).unwrap(), 0x08000b00);
    assert_eq!(m.bus.read32(0x3ff71204).unwrap(), 0x08000900);
}

#[test]
#[ignore = "needs pinned esp32_rev300_rom.elf; real sleep-enter is NOT the RF sleep trigger"]
fn external_rom_sleep_enter_programs_duration_mask_ack_and_external_wakeup_disable_only() {
    let mut m = setup(0x400557a4, true);
    m.cores[0].set_ar(6, 500);
    m.cores[0].set_ar(7, 0);
    assert!(steps(&mut m, 2000));
    assert_eq!(m.bus.periph.bt_sleep.read(0x34), 500);
    assert_eq!(m.bus.periph.bt_sleep.read(0x0c), 4);
    assert_eq!(m.bus.periph.bt_sleep.read(0x30), 1 << 31);
    m.bus.tick(50000);
    assert_eq!(m.bus.periph.bt_sleep.entries, 0);
    assert_eq!(m.bus.periph.bt_sleep.read(0x14)&4, 0, "no programmed LP wake without actual sleep entry");
    assert_eq!(m.bus.periph.bt_sleep.read(0x10),0,"mask4 excludes live native clock events");
}
