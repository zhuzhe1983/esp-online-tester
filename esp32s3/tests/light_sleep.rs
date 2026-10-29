//! ESP32-S3 light-sleep contracts, without a ROM or firmware fixture.
//! Register/bit sources: ESP-IDF v5.5.2 components/soc/esp32s3/register/soc/
//! {rtc_cntl,rtc_io,gpio,io_mux,systimer,timer_group,interrupt_core0,interrupt_core1}_reg.h;
//! wake/reject masks: esp_hw_support/port/esp32s3/include/soc/rtc.h;
//! RTC_PERIPH requirements and digital-clock compensation: esp_hw_support/sleep_modes.c.
//! Source 39 is ETS_RTC_CORE_INTR_SOURCE in soc/esp32s3/include/soc/interrupts.h.
use emu_core::{Bus as _, Core as _};
use esp_soc::{CoreState, Soc as _, SocBus as _};

const RTC: u32 = 0x6000_8000;
const GPIO: u32 = 0x6000_4000;
const IOMUX: u32 = 0x6000_9000;
const SYSTIMER: u32 = 0x6002_3000;
const TIMG: u32 = 0x6001_f000;
const IRAM: u32 = 0x4037_0000;
const RTC_PERIOD: u32 = 1600; // 240 MHz reference clock / 150 kHz RTC slow clock
const RTC_INPUT: u32 = (1 << 19) | (1 << 13);

fn machine() -> esp32s3::Machine {
    let mut m = esp32s3::machine([0; 6]);
    m.console.capture = true;
    m.quantum = 64;
    m.vq_max = 1;
    m.bus.board = Box::new(esp32s3::board::NoBoard);
    for addr in [IRAM, 0x4000_0400] { m.bus.load_bytes(addr, &[0x06, 0xff, 0xff]).unwrap(); }
    m.cores[0].pc = IRAM;
    m.cores[0].ps = 0;
    m.bus.write32(0x600c_0000, 2).unwrap(); // release and initialize APP before sleep
    m.run(64);
    m.bus.flush_ticks();
    for c in &mut m.cores { c.pc = IRAM; c.ps = 0; c.intenable = 0; }
    m
}
fn advance(m: &mut esp32s3::Machine, cycles: u32) { m.bus.tick(cycles); m.bus.flush_ticks(); }
fn rtc_time(m: &mut esp32s3::Machine) -> u64 {
    m.bus.write32(RTC + 0xc, 1 << 31).unwrap();
    u64::from(m.bus.read32(RTC + 0x10).unwrap()) | (u64::from(m.bus.read32(RTC + 0x14).unwrap() & 0xffff) << 32)
}
fn timer(m: &mut esp32s3::Machine, ticks: u64) {
    m.bus.write32(RTC + 4, ticks as u32).unwrap();
    // rtc_cntl_ll_set_wakeup_timer writes only the 48-bit target, not MAIN_TIMER_ALARM_EN.
    m.bus.write32(RTC + 8, (ticks >> 32) as u32 & 0xffff).unwrap();
}
fn enable(m: &mut esp32s3::Machine, sources: u32) { m.bus.write32(RTC + 0x3c, sources << 15).unwrap(); }
fn sleep(m: &mut esp32s3::Machine) { m.bus.write32(RTC + 0x18, 1 << 31).unwrap(); }
fn held(m: &esp32s3::Machine) {
    for core in 0..2 { assert_eq!(esp32s3::S3::core_state(&m.bus, core), CoreState::Held); }
}
fn woke(m: &mut esp32s3::Machine, cause: u32) {
    assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 3, 1);
    assert_eq!(m.bus.read32(RTC + 0x130).unwrap() & 0x1ffff, cause);
    assert_eq!(m.bus.read32(RTC + 0x18).unwrap() >> 31, 0);
    for core in 0..2 { assert_eq!(esp32s3::S3::core_state(&m.bus, core), CoreState::Running); }
    assert!(!m.bus.sw_reset());
}
fn digital_input(m: &mut esp32s3::Machine, pin: u32, kind: u32) {
    m.bus.write32(IOMUX + 4 + pin * 4, 1 << 9).unwrap();
    m.bus.write32(GPIO + 0x74 + pin * 4, (1 << 10) | (kind << 7)).unwrap();
}
fn rtc_input(m: &mut esp32s3::Machine, pin: u32, cfg: u32) { m.bus.write32(RTC + 0x484 + pin * 4, cfg).unwrap(); }
fn systime(m: &mut esp32s3::Machine) -> u64 {
    m.bus.write32(SYSTIMER + 4, 1 << 30).unwrap();
    u64::from(m.bus.read32(SYSTIMER + 0x44).unwrap()) | (u64::from(m.bus.read32(SYSTIMER + 0x40).unwrap()) << 32)
}
fn timer_group_time(m: &mut esp32s3::Machine) -> u64 {
    m.bus.write32(TIMG + 0xc, 1).unwrap();
    u64::from(m.bus.read32(TIMG + 4).unwrap()) | (u64::from(m.bus.read32(TIMG + 8).unwrap()) << 32)
}

#[test]
fn timer_wake_retains_both_cores_and_memory_at_the_exact_boundary() {
    let mut m = machine();
    for c in &mut m.cores {
        c.ps = 15;
        c.intenable = 1 << 6;
        c.ccompare[0] = c.ccount.wrapping_add(20);
        c.set_ar(3, 0x1234_5678);
        c.cpenable = 0x55;
    }
    for addr in [0x3fc9_0000, 0x600f_e000, 0x5000_0000] { m.bus.write32(addr, 0xdead_beef).unwrap(); }
    let counts: Vec<_> = m.cores.iter().map(|c| (c.insn_count, c.ccount, c.pc, c.intenable, c.ps, c.ccompare, c.cpenable)).collect();
    let deadline = rtc_time(&mut m) + 100;
    timer(&mut m, deadline);
    enable(&mut m, 8);
    sleep(&mut m);
    held(&m);
    assert!(matches!(m.run_until_cycle(deadline * u64::from(RTC_PERIOD) - 1), esp_soc::RunUntil::Reached));
    held(&m);
    assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 1, 0);
    assert!(matches!(m.run_until_cycle(deadline * u64::from(RTC_PERIOD)), esp_soc::RunUntil::Reached));
    woke(&mut m, 8);
    for (c, expected) in m.cores.iter().zip(&counts) {
        assert_eq!((c.insn_count, c.ccount, c.pc, c.intenable, c.ps, c.ccompare, c.cpenable), *expected);
        assert_eq!(c.get_ar(3), 0x1234_5678);
    }
    for addr in [0x3fc9_0000, 0x600f_e000, 0x5000_0000] { assert_eq!(m.bus.read32(addr).unwrap(), 0xdead_beef); }
    m.run(128);
    assert!(m.cores.iter().zip(counts).all(|(c, old)| c.insn_count > old.0));
}

#[test]
fn systimer_and_timer_group_are_gated_without_catching_up_after_wake() {
    let mut m = machine();
    m.bus.write32(TIMG, (1 << 31) | (1 << 30) | (2 << 13)).unwrap();
    advance(&mut m, 300);
    let sys = systime(&mut m);
    let tg = timer_group_time(&mut m);
    m.bus.write32(SYSTIMER + 0x1c, 0).unwrap();
    m.bus.write32(SYSTIMER + 0x20, sys as u32 + 40).unwrap();
    m.bus.write32(SYSTIMER + 0x50, 1).unwrap();
    m.bus.write32(SYSTIMER, (1 << 30) | (1 << 24)).unwrap();
    m.bus.write32(TIMG + 0x10, tg as u32 + 100).unwrap();
    m.bus.write32(TIMG, (1 << 31) | (1 << 30) | (2 << 13) | (1 << 10)).unwrap();
    let deadline = rtc_time(&mut m) + 100;
    timer(&mut m, deadline);
    enable(&mut m, 8);
    sleep(&mut m);
    let remaining = deadline * u64::from(RTC_PERIOD) - m.bus.cycles;
    advance(&mut m, remaining as u32);
    woke(&mut m, 8);
    assert_eq!(systime(&mut m), sys);
    assert_eq!(timer_group_time(&mut m), tg);
    assert_eq!(m.bus.read32(SYSTIMER + 0x68).unwrap() & 1, 0);
    assert_eq!(m.bus.read32(TIMG + 0x74).unwrap() & 1, 0);
    // Only these new awake cycles reach digital domains: 300/15 SYSTIMER,
    // 300/3/2 TIMG. A delayed accounting flush must not replay the sleep interval.
    advance(&mut m, 300);
    assert_eq!(systime(&mut m), sys + 20);
    assert_eq!(timer_group_time(&mut m), tg + 50);
    assert_eq!(m.bus.read32(SYSTIMER + 0x68).unwrap() & 1, 0);
    assert_eq!(m.bus.read32(TIMG + 0x74).unwrap() & 1, 0);
    advance(&mut m, 300);
    assert_eq!(m.bus.read32(SYSTIMER + 0x68).unwrap() & 1, 1);
    assert_eq!(m.bus.read32(TIMG + 0x74).unwrap() & 1, 1);
}

#[test]
fn timer_is_48_bit_wrap_safe_and_already_due_wakes_immediately() {
    for now in [100, (1 << 32) + 19, (1 << 48) - 2] {
        for overdue in [0, 3] {
            let mut m = machine();
            // The counter has no guest write register: inject only the starting time,
            // then use guest MMIO for all configuration and observable results.
            m.bus.periph.rtc.slow_ticks = now;
            timer(&mut m, now - overdue);
            enable(&mut m, 8);
            sleep(&mut m);
            woke(&mut m, 8);
        }
    }
    for (now, target, ticks) in [(5, (1 << 32) + 5, 4), ((1 << 32) - 2, (1 << 32) + 2, 4), ((1 << 48) - 2, 2, 4)] {
        let mut m = machine();
        m.bus.periph.rtc.slow_ticks = now;
        timer(&mut m, target);
        enable(&mut m, 8);
        sleep(&mut m);
        held(&m);
        let until = ticks * RTC_PERIOD - (m.bus.cycles % u64::from(RTC_PERIOD)) as u32;
        advance(&mut m, until - 1);
        held(&m);
        advance(&mut m, 1);
        if now == 5 { held(&m); assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 1, 0, "high timer word was truncated"); }
        else { woke(&mut m, 8); }
    }
}

#[test]
fn disabled_timer_and_digital_cpu_interrupts_do_not_wake() {
    let mut m = machine();
    timer(&mut m, 0);
    enable(&mut m, 0);
    for c in &mut m.cores { c.ps = 15; c.intenable = u32::MAX; c.interrupt = u32::MAX; }
    let before: Vec<_> = m.cores.iter().map(|c| (c.insn_count, c.ccount, c.pc, c.intenable)).collect();
    sleep(&mut m);
    let until = m.bus.cycles + u64::from(RTC_PERIOD) * 10;
    m.run_until_cycle(until);
    held(&m);
    for (c, expected) in m.cores.iter().zip(before) { assert_eq!((c.insn_count, c.ccount, c.pc, c.intenable), expected); }
    assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 3, 0);
    assert!(!m.bus.sw_reset());
}

#[test]
fn watchdog_pause_is_s3_bit9_and_retains_elapsed_count() {
    for pause in [0, 1 << 7, 1 << 9] {
        let mut m = machine();
        m.bus.write32(RTC + 0xb0, 0x50d8_3aa1).unwrap();
        m.bus.write32(RTC + 0x9c, 10).unwrap();
        m.bus.write32(RTC + 0x98, (1 << 31) | (4 << 28) | pause).unwrap();
        advance(&mut m, 3 * RTC_PERIOD);
        let deadline = rtc_time(&mut m) + 20;
        timer(&mut m, deadline);
        enable(&mut m, 8);
        sleep(&mut m);
        advance(&mut m, 20 * RTC_PERIOD);
        assert_eq!(m.bus.sw_reset(), pause != 1 << 9, "pause bits {pause:#x}");
        if pause == 1 << 9 {
            woke(&mut m, 8);
            assert_ne!(m.bus.read32(RTC + 0x98).unwrap() & (1 << 31), 0, "sleep must not disable WDT");
            advance(&mut m, 6 * RTC_PERIOD);
            assert!(!m.bus.sw_reset());
            advance(&mut m, RTC_PERIOD);
            assert!(m.bus.sw_reset(), "wake must resume the remaining seven ticks, not restart WDT");
        }
        assert_eq!(m.bus.reset_cause(), 16);
    }
}

#[test]
fn unpaused_watchdog_still_resets_with_no_wake_source() {
    let mut m = machine();
    enable(&mut m, 0);
    m.bus.write32(RTC + 0xb0, 0x50d8_3aa1).unwrap();
    m.bus.write32(RTC + 0x9c, 10).unwrap();
    m.bus.write32(RTC + 0x98, (1 << 31) | (4 << 28)).unwrap();
    sleep(&mut m);
    let before = m.insns();
    assert!(matches!(m.run(100_000), esp_soc::Stop::SwReset));
    assert_eq!(m.insns(), before);
    assert_eq!(m.bus.reset_cause(), 16);
}

#[test]
fn gpio_wakeup_uses_selected_level_without_cpu_interrupt_enable() {
    for (kind, level) in [(4, false), (5, true)] {
        let mut m = machine();
        enable(&mut m, 4);
        digital_input(&mut m, 40, kind); // non-RTC GPIO, no GPIO_PIN_INT_ENA bits
        m.bus.gpio_set_input(40, !level);
        sleep(&mut m);
        held(&m);
        m.bus.gpio_set_input(41, level);
        held(&m);
        m.bus.gpio_set_input(40, level);
        woke(&mut m, 4);
        assert_eq!(m.bus.read32(RTC + 0x40).unwrap(), 0);
    }
    for kind in [0, 1, 2, 3] {
        let mut m = machine();
        enable(&mut m, 4);
        digital_input(&mut m, 40, kind);
        m.bus.gpio_set_input(40, false);
        sleep(&mut m);
        m.bus.gpio_set_input(40, true);
        m.bus.gpio_set_input(40, false);
        held(&m);
        assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 3, 0, "edge/disabled mode {kind} cannot wake");
    }
}

#[test]
fn gpio_wakeup_obeys_sleep_input_enable_selection() {
    for (mux, wakes) in [(0, false), (1 << 9, true), ((1 << 1) | (1 << 9), false), ((1 << 1) | (1 << 4), true)] {
        let mut m = machine();
        enable(&mut m, 4);
        digital_input(&mut m, 40, 5);
        m.bus.write32(IOMUX + 4 + 40 * 4, mux).unwrap();
        m.bus.gpio_set_input(40, false);
        sleep(&mut m);
        m.bus.gpio_set_input(40, true);
        if wakes { woke(&mut m, 4); } else { held(&m); }
    }
}

#[test]
fn ext0_wakeup_uses_s3_rtc_pin_number_and_selected_level() {
    for level in [false, true] {
        let mut m = machine();
        enable(&mut m, 1);
        rtc_input(&mut m, 21, RTC_INPUT);
        m.bus.write32(RTC + 0x4dc, 21 << 27).unwrap(); // S3 RTC IO21 is GPIO21
        m.bus.write32(RTC + 0x64, u32::from(level) << 30).unwrap();
        m.bus.gpio_set_input(21, !level);
        sleep(&mut m);
        m.bus.gpio_set_input(0, level);
        held(&m);
        m.bus.gpio_set_input(21, level);
        woke(&mut m, 1);
    }
    for pin in [22, 31] {
        let mut m = machine();
        enable(&mut m, 1);
        m.bus.write32(RTC + 0x4dc, pin << 27).unwrap();
        m.bus.write32(RTC + 0x64, 1 << 30).unwrap();
        sleep(&mut m);
        m.bus.gpio_set_input(pin as u8, true);
        held(&m);
    }
}

#[test]
fn ext1_any_low_or_any_high_records_only_active_selected_rtc_pads() {
    for level in [false, true] {
        let mut m = machine();
        enable(&mut m, 2);
        for pin in [2, 3] { rtc_input(&mut m, pin, RTC_INPUT); m.bus.gpio_set_input(pin as u8, !level); }
        m.bus.write32(RTC + 0xe0, (1 << 2) | (1 << 3)).unwrap();
        m.bus.write32(RTC + 0x64, u32::from(level) << 31).unwrap();
        m.bus.write32(RTC + 0x88, 1 << 20).unwrap(); // EXT1 works while RTC_PERIPH is powered down
        sleep(&mut m);
        m.bus.gpio_set_input(4, level);
        held(&m);
        m.bus.gpio_set_input(2, level); // pin3 stays inactive: S3 ANY_LOW, not classic ALL_LOW
        woke(&mut m, 2);
        assert_eq!(m.bus.read32(RTC + 0xe4).unwrap(), 1 << 2);
        m.bus.write32(RTC + 0xe4, 0).unwrap();
        assert_eq!(m.bus.read32(RTC + 0xe4).unwrap(), 1 << 2, "EXT1 status is read-only");
        m.bus.write32(RTC + 0xe0, (1 << 22) | (1 << 3)).unwrap();
        assert_eq!(m.bus.read32(RTC + 0xe4).unwrap(), 0);
        assert_eq!(m.bus.read32(RTC + 0xe0).unwrap(), 1 << 3, "status clear is self-clearing and preserves selection");
    }
}

#[test]
fn rtc_wakeup_requires_rtc_mux_function_and_selected_input_enable() {
    for source in [1, 2] {
        for (pad, wakes) in [(0, false), (1 << 13, false), (1 << 19, false),
            (RTC_INPUT | (1 << 17), false), (RTC_INPUT, true),
            (RTC_INPUT | (1 << 16), false), ((1 << 19) | (1 << 16) | (1 << 15), true)] {
            let mut m = machine();
            enable(&mut m, source);
            rtc_input(&mut m, 2, pad);
            m.bus.write32(RTC + 0x4dc, 2 << 27).unwrap();
            m.bus.write32(RTC + 0xe0, 1 << 2).unwrap();
            m.bus.write32(RTC + 0x64, (1 << 30) | (1 << 31)).unwrap();
            m.bus.gpio_set_input(2, false);
            sleep(&mut m);
            m.bus.gpio_set_input(2, true);
            if wakes { woke(&mut m, source); } else { held(&m); }
        }
    }
    let mut m = machine();
    enable(&mut m, 2);
    m.bus.write32(RTC + 0xe0, 1 << 23).unwrap(); // outside the 22 RTC pads, not a wake source
    m.bus.write32(RTC + 0x64, 1 << 31).unwrap();
    sleep(&mut m);
    m.bus.gpio_set_input(23, true);
    held(&m);
}

#[test]
fn ext0_and_gpio_need_rtc_peripheral_power() {
    for source in [1, 4] {
        for (pwc, wakes) in [(0, true), (1 << 20, false), (1 << 18, false), ((1 << 20) | (1 << 19), true)] {
            let mut m = machine();
            enable(&mut m, source);
            rtc_input(&mut m, 2, RTC_INPUT);
            digital_input(&mut m, 2, 5);
            m.bus.write32(RTC + 0x4dc, 2 << 27).unwrap();
            m.bus.write32(RTC + 0x64, 1 << 30).unwrap();
            m.bus.write32(RTC + 0x88, pwc).unwrap();
            m.bus.gpio_set_input(2, false);
            sleep(&mut m);
            m.bus.gpio_set_input(2, true);
            if wakes { woke(&mut m, source); } else { held(&m); }
        }
    }
}

#[test]
fn rejection_needs_light_enable_and_matching_source_and_has_separate_clear() {
    for source in [1, 2, 4, 8] {
        for (conf, rejects) in [(0, false), ((1 << 31) | (source << 12), false),
            (1 << 30, false), ((1 << 30) | ((source ^ 0xf) << 12), false), ((1 << 30) | (source << 12), true)] {
            let mut m = machine();
            enable(&mut m, source);
            timer(&mut m, 0);
            rtc_input(&mut m, 2, RTC_INPUT);
            digital_input(&mut m, 2, 5);
            m.bus.write32(RTC + 0x4dc, 2 << 27).unwrap();
            m.bus.write32(RTC + 0xe0, 1 << 2).unwrap();
            m.bus.write32(RTC + 0x64, (1 << 30) | (1 << 31)).unwrap();
            m.bus.gpio_set_input(2, true);
            m.bus.write32(RTC + 0x68, conf).unwrap();
            sleep(&mut m);
            assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 3, if rejects { 2 } else { 1 }, "source={source} conf={conf:#x}");
            assert_eq!(m.bus.read32(RTC + 0x128).unwrap(), if rejects { source } else { 0 });
            assert_eq!(m.bus.read32(RTC + 0x130).unwrap(), if rejects { 0 } else { source });
            assert_eq!(esp32s3::S3::core_state(&m.bus, 0), CoreState::Running);
            if rejects {
                m.bus.write32(RTC + 0x128, 0).unwrap();
                assert_eq!(m.bus.read32(RTC + 0x128).unwrap(), source, "reject cause is read-only");
                m.bus.write32(RTC + 0x18, 2).unwrap();
                assert_eq!(m.bus.read32(RTC + 0x128).unwrap(), 0);
                assert_eq!(m.bus.read32(RTC + 0x18).unwrap() & 2, 0);
                assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 3, 2, "cause clear must not clear RAW");
                m.bus.write32(RTC + 0x4c, 2).unwrap();
                assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 3, 0);
            }
        }
    }
}

#[test]
fn rtc_raw_status_mask_and_w1c_route_source39_to_both_cores() {
    let mut m = machine();
    for (base, line) in [(0x600c_2000, 9), (0x600c_2800, 12)] { m.bus.write32(base + 39 * 4, line).unwrap(); }
    timer(&mut m, 0);
    enable(&mut m, 8);
    sleep(&mut m);
    woke(&mut m, 8);
    assert_eq!(m.bus.read32(RTC + 0x48).unwrap(), 0);
    m.run(1);
    assert_eq!(m.cores[0].ext_irq_lines & (1 << 9), 0);
    m.bus.write32(RTC + 0x40, 1).unwrap();
    assert_eq!(m.bus.read32(RTC + 0x48).unwrap() & 1, 1);
    m.run(1);
    assert_ne!(m.cores[0].ext_irq_lines & (1 << 9), 0);
    assert_ne!(m.cores[1].ext_irq_lines & (1 << 12), 0);
    assert_ne!(m.bus.read32(0x600c_2190).unwrap() & (1 << 7), 0);
    for off in [0x44, 0x48, 0x130] { m.bus.write32(RTC + off, 0).unwrap(); }
    assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 1, 1);
    assert_eq!(m.bus.read32(RTC + 0x130).unwrap(), 8);
    m.bus.write32(RTC + 0x4c, 2).unwrap();
    assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 1, 1, "W1C must clear only selected bits");
    m.bus.write32(RTC + 0x4c, 1).unwrap();
    assert_eq!(m.bus.read32(RTC + 0x4c).unwrap(), 0);
    assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 1, 0);
    assert_eq!(m.bus.read32(RTC + 0x48).unwrap() & 1, 0);
    assert_eq!(m.bus.read32(RTC + 0x40).unwrap(), 1);
    m.run(1);
    assert_eq!(m.cores[0].ext_irq_lines & (1 << 9), 0);
    assert_eq!(m.cores[1].ext_irq_lines & (1 << 12), 0);
    // Also exercise hardware-generated RAW while ENA was already set at entry.
    sleep(&mut m);
    m.run(1);
    assert_ne!(m.cores[0].ext_irq_lines & (1 << 9), 0);
    assert_ne!(m.cores[1].ext_irq_lines & (1 << 12), 0);
}

#[test]
fn sleep_entered_by_guest_stops_the_rest_of_its_quantum() {
    for jit in [false, true] {
        let mut m = machine();
        for c in &mut m.cores { c.set_jit(jit); }
        enable(&mut m, 0);
        // s32i a2,a3,0; addi.n a4,a4,1; j self, independently encoded.
        m.bus.load_bytes(IRAM, &[0x22, 0x63, 0x00, 0x4b, 0x44, 0x06, 0xff, 0xff]).unwrap();
        m.cores[0].set_ar(2, 1 << 31);
        m.cores[0].set_ar(3, RTC + 0x18);
        m.cores[0].set_ar(4, 7);
        m.cores[1].pc = IRAM + 5;
        let second = m.cores[1].insn_count;
        m.run(128);
        held(&m);
        assert_eq!(m.cores[0].get_ar(4), 7, "instruction after SLEEP_EN retired, jit={jit}");
        assert_eq!(m.cores[0].pc, IRAM + 3);
        assert_eq!(m.cores[1].insn_count, second, "APP executed after PRO entered sleep, jit={jit}");
    }
}

#[test]
fn deep_sleep_request_does_not_report_an_ordinary_light_sleep_wake() {
    let mut m = machine();
    timer(&mut m, 0);
    enable(&mut m, 8);
    m.bus.write32(RTC + 0x90, 1 << 31).unwrap(); // DIG_PWC.DG_WRAP_PD_EN
    sleep(&mut m);
    assert_eq!(m.bus.read32(RTC + 0x44).unwrap() & 1, 0);
    assert_eq!(m.bus.read32(RTC + 0x130).unwrap(), 0);
    // Deep-sleep/reset modelling is outside this light-sleep implementation.
    assert_eq!(esp32s3::S3::core_state(&m.bus, 0), CoreState::Running);
}

#[test]
fn autonomous_board_deadline_publishes_and_wakes_while_cpus_sleep() {
    // An externally powered panel/IRQ source keeps its own clock while the SoC sleeps.
    struct WakeBoard { deadline: u64, fired: bool, edges: Vec<esp_soc::board::BoardEdge> }
    impl esp32s3::board::BoardModel for WakeBoard {
        fn name(&self) -> &'static str { "sleep-wake-test" }
        fn input_levels(&self) -> Vec<(u8, bool)> { vec![(4, !self.fired)] }
        fn next_deadline(&self) -> Option<u64> { (!self.fired).then_some(self.deadline) }
        fn advance_to(&mut self, cycle: u64) {
            if !self.fired && cycle >= self.deadline {
                self.fired = true;
                self.edges.push(esp_soc::board::BoardEdge { cycle: self.deadline, pin: 4, level: false });
            }
        }
        fn take_edges(&mut self) -> Vec<esp_soc::board::BoardEdge> { std::mem::take(&mut self.edges) }
        fn display_frames(&self) -> u64 { u64::from(self.fired) }
    }
    let mut m = machine();
    let deadline = m.bus.cycles + 4321;
    m.bus.board = Box::new(WakeBoard { deadline, fired: false, edges: Vec::new() });
    m.bus.attach_board_devices();
    digital_input(&mut m, 4, 4);
    enable(&mut m, 4);
    sleep(&mut m);
    held(&m);
    m.run_until_cycle(deadline - 1);
    held(&m);
    assert_eq!(m.bus.board.display_frames(), 0);
    m.run_until_cycle(deadline);
    woke(&mut m, 4);
    assert_eq!(m.bus.board.display_frames(), 1);
}
