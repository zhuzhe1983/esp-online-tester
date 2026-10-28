//! Classic RTC light-sleep contracts; register layout is ESP-IDF v3.2.3
//! components/soc/esp32/include/soc/rtc_cntl_reg.h (also unchanged in v5.5.4).
use emu_core::Bus as _;
use esp_soc::SocBus as _;

const RTC: u32 = 0x3ff4_8000;
fn machine() -> esp32::Machine {
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.console.capture = true;
    m.bus.load_bytes(0x4008_0000, &[0x06, 0xff, 0xff]).unwrap(); // j self
    m.bus.write32(0x3ff0_0030, 1).unwrap();
    m.bus.write32(0x3ff0_002c, 0).unwrap();
    m.run(128);
    for c in &mut m.cores { c.pc = 0x4008_0000; c.ps = 0; }
    m.bus.write32(RTC + 0x38, 8 << 11).unwrap(); // RTC_TIMER_TRIG_EN
    m
}
fn timer(m: &mut esp32::Machine, ticks: u64) {
    m.bus.write32(RTC + 4, ticks as u32).unwrap();
    m.bus.write32(RTC + 8, (ticks >> 32) as u32).unwrap();
}
fn sleep(m: &mut esp32::Machine) { m.bus.write32(RTC + 0x18, 1 << 31).unwrap(); }

#[test]
fn timer_wake_retains_both_cores_and_digital_clocks() {
    let mut m = machine();
    for c in &mut m.cores {
        c.intenable = 1 << 6;
        c.ccompare[0] = c.ccount.wrapping_add(20);
        c.set_ar(3, 0x1234_5678);
    }
    m.bus.write32(0x3ffb_0000, 0xdead_beef).unwrap();
    // FRC2 is the APB timebase IDF corrects from RTC time after light sleep.
    m.bus.write32(0x3ff4_7028, 1 << 7).unwrap();
    let frc = m.bus.read32(0x3ff4_7024).unwrap();
    let counts: Vec<_> = m.cores.iter().map(|c| (c.insn_count, c.ccount, c.pc, c.intenable, c.ps)).collect();
    let deadline = m.bus.periph.rtc.0.slow_ticks + 100;
    timer(&mut m, deadline);
    sleep(&mut m);
    m.run(1024);
    for (c, expected) in m.cores.iter().zip(&counts) {
        assert_eq!((c.insn_count, c.ccount, c.pc, c.intenable, c.ps), *expected);
    }
    assert_eq!(m.bus.read32(0x3ff4_7024).unwrap(), frc);
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 0);
    // Advance to one reference tick before the RTC deadline, then across it.
    let remaining = deadline * 1600 - m.bus.cycles;
    m.bus.tick(((remaining - 1) / 6) as u32); // default XTAL CPU=40MHz
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 0);
    m.bus.tick(1);
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 1);
    assert_eq!(m.bus.read32(RTC + 0x38).unwrap() & 0x7ff, 8);
    assert_eq!(m.bus.read32(RTC + 0x18).unwrap() >> 31, 0);
    assert!(!m.bus.sw_reset());
    assert_eq!(m.bus.read32(0x3ffb_0000).unwrap(), 0xdead_beef);
    // No interrupt enable is needed for wake, and its RAW status is W1C.
    m.bus.write32(RTC + 0x48, 1).unwrap();
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 0);
    m.run(128);
    assert!(m.cores.iter().zip(&counts).all(|(c, old)| c.insn_count > old.0));
    assert!(m.bus.read32(0x3ff4_7024).unwrap() > frc, "FRC resumes its APB clock after wake");
}

#[test]
fn disabled_timer_and_unrelated_digital_irq_cannot_wake() {
    let mut m = machine();
    timer(&mut m, 0);
    m.bus.write32(RTC + 0x38, 0).unwrap();
    for c in &mut m.cores { c.intenable = u32::MAX; c.interrupt = u32::MAX; }
    let counts: Vec<_> = m.cores.iter().map(|c| (c.insn_count, c.ccount, c.intenable)).collect();
    sleep(&mut m);
    m.run(10_000);
    for (c, expected) in m.cores.iter().zip(&counts) { assert_eq!((c.insn_count, c.ccount, c.intenable), *expected); }
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 0);
    assert!(!m.bus.sw_reset());
}

#[test]
fn timer_uses_all_48_bits_and_wakes_if_already_due() {
    for ticks in [0, (1 << 32) + 19] {
        let mut m = machine();
        m.bus.periph.rtc.0.slow_ticks = ticks;
        timer(&mut m, ticks);
        sleep(&mut m);
        assert_eq!(m.bus.read32(RTC + 0x38).unwrap() & 0x7ff, 8);
        assert!(!m.bus.sw_reset());
    }
    let mut m = machine();
    let deadline = (1 << 32) + m.bus.periph.rtc.0.slow_ticks;
    timer(&mut m, deadline);
    sleep(&mut m);
    m.run(4096);
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 0, "high timer word was truncated");
}

#[test]
fn ext0_wakes_without_reset_and_requires_the_selected_pad_level() {
    let mut m = machine();
    m.bus.write32(RTC + 0x38, 1 << 11).unwrap();
    m.bus.write32(0x3ff4_8498, (1 << 19) | (1 << 13)).unwrap();
    m.bus.write32(0x3ff4_84bc, 11 << 27).unwrap(); // GPIO0 = RTC IO11
    m.bus.gpio_set_input(0, true);
    sleep(&mut m);
    let before = m.insns();
    m.bus.gpio_set_input(35, false);
    m.run(1024);
    assert_eq!(m.insns(), before);
    m.bus.gpio_set_input(0, false);
    assert!(!m.bus.sw_reset());
    assert_eq!(m.bus.read32(RTC + 0x38).unwrap() & 0x7ff, 1);
    m.run(128);
    assert!(m.insns() > before);
}

#[test]
fn watchdog_pause_is_guest_controlled_and_resumes_after_wake() {
    for pause in [0, 1 << 7] {
        let mut m = machine();
        m.bus.write32(RTC + 0xa4, 0x50d8_3aa1).unwrap();
        m.bus.write32(RTC + 0x90, 10).unwrap();
        m.bus.write32(RTC + 0x8c, (1 << 31) | (4 << 28) | pause).unwrap();
        let deadline = m.bus.periph.rtc.0.slow_ticks + 20;
        timer(&mut m, deadline);
        sleep(&mut m);
        m.bus.tick(1600 * 20 / 6 + 1);
        assert_eq!(m.bus.sw_reset(), pause == 0);
        if pause != 0 {
            assert_ne!(m.bus.read32(RTC + 0x8c).unwrap() & (1 << 31), 0, "sleep must not disable WDT");
            m.bus.tick(1600 * 10 / 6 + 1);
            assert!(m.bus.sw_reset(), "watchdog must resume after wake");
        }
        assert_eq!(m.bus.reset_cause(), 16);
    }
}

#[test]
fn rtc_timer_wraps_at_48_bits() {
    let mut m = machine();
    m.bus.periph.rtc.0.slow_ticks = (1 << 48) - 2;
    timer(&mut m, 2);
    sleep(&mut m);
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 0);
    m.bus.tick(1600 / 6);
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 0);
    m.bus.tick(4 * 1600 / 6);
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap() & 1, 1);
}

#[test]
fn sleep_entered_by_guest_stops_the_rest_of_its_quantum() {
    let mut m = machine();
    m.bus.write32(RTC + 0x38, 0).unwrap();
    // s32i a2,a3,0; addi.n a4,a4,1; j self. Registers avoid fixture-specific hooks.
    m.bus.load_bytes(0x4008_0000, &[0x22, 0x63, 0x00, 0x4b, 0x44, 0x06, 0xff, 0xff]).unwrap();
    m.cores[0].set_ar(2, 1 << 31);
    m.cores[0].set_ar(3, RTC + 0x18);
    m.cores[0].set_ar(4, 7);
    m.cores[1].pc = 0x4008_0005;
    let second = m.cores[1].insn_count;
    m.run(128);
    assert_eq!(m.cores[0].get_ar(4), 7, "an instruction after SLEEP_EN retired");
    assert_eq!(m.cores[0].pc, 0x4008_0003);
    assert_eq!(m.cores[1].insn_count, second, "second core executed after entry");
}

#[test]
fn rtc_memory_domains_follow_pwc_during_light_sleep() {
    // rtc_sleep_init applies RTC memory PD/FORCE_PU bits in both modes;
    // v3.2.3 components/soc/esp32/rtc_sleep.c, rtc_fastmem_pd_en/rtc_slowmem_pd_en.
    for (pwc, retained) in [(0, true), ((1 << 17) | (1 << 14), false),
        ((1 << 15) | (1 << 12), false), ((1 << 17) | (1 << 14) | (1 << 16) | (1 << 13), true)] {
        let mut m = machine();
        m.bus.write32(RTC + 0x38, 0).unwrap();
        m.bus.write32(RTC + 0x80, pwc).unwrap();
        for addr in [0x5000_0000, 0x3ff8_0000] { m.bus.write32(addr, 0x1234_5678).unwrap(); }
        sleep(&mut m);
        for addr in [0x5000_0000, 0x3ff8_0000] {
            assert_eq!(m.bus.read32(addr).unwrap(), if retained { 0x1234_5678 } else { 0 });
        }
    }
}

#[test]
fn unpaused_watchdog_resets_with_no_wake_source() {
    let mut m = machine();
    m.bus.write32(RTC + 0x38, 0).unwrap();
    m.bus.write32(RTC + 0xa4, 0x50d8_3aa1).unwrap();
    m.bus.write32(RTC + 0x90, 10).unwrap();
    m.bus.write32(RTC + 0x8c, (1 << 31) | (4 << 28)).unwrap();
    sleep(&mut m);
    let before = m.insns();
    assert!(matches!(m.run(10_000), esp32::Stop::SwReset));
    assert_eq!(m.insns(), before);
    assert_eq!(m.bus.reset_cause(), 16);
}
