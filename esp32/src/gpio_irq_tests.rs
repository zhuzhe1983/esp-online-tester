use super::*;

// Independent, uncached implementation of the pre-change query path.
fn slow_raw(g: &ClassicGpio) -> u64 {
    let mut raw = g.gpio.status & ((1 << 40) - 1);
    for pin in 0..40 {
        let typ = (g.gpio.pin[pin] >> 7) & 7;
        if g.input_enabled(pin)
            && ((typ == 4 && !g.pad_level(pin)) || (typ == 5 && g.pad_level(pin)))
        {
            raw |= 1 << pin;
        }
    }
    raw
}
fn check(p: &mut Peripherals) {
    let level_pins = (0..40).fold(0, |mask, pin| {
        mask | if matches!((p.gpio.gpio.pin[pin] >> 7) & 7, 4 | 5) {
            1 << pin
        } else {
            0
        }
    });
    assert_eq!(p.gpio.level_pins, level_pins, "configured level mask");
    let raw = slow_raw(&p.gpio);
    assert_eq!(p.gpio.raw_status(), raw, "cached raw status");
    assert_eq!(p.read32(0x3ff4_4044), raw as u32);
    assert_eq!(p.read32(0x3ff4_4050), (raw >> 32) as u32);
    let all = Dispatch::source_status(p);
    let mut expected_lines = [0; 2];
    for (core, expected_line) in expected_lines.iter_mut().enumerate() {
        let mut sources = [all[0], all[1], all[2]];
        for nmi in [false, true] {
            let route = if core == 0 { 2 } else { 0 } + usize::from(nmi);
            let expected = (0..40).fold(0, |mask, pin| {
                mask | if raw & (1 << pin) != 0 && p.gpio.gpio.pin[pin] & (1 << (13 + route)) != 0 {
                    1 << pin
                } else {
                    0
                }
            });
            assert_eq!(
                p.gpio.irq_status(core, nmi),
                expected,
                "core {core} nmi {nmi}"
            );
            let off = if core == 0 { 0x68 } else { 0x60 } + if nmi { 4 } else { 0 };
            assert_eq!(p.gpio.read(off), expected as u32);
            assert_eq!(p.gpio.read(off + 0x14), (expected >> 32) as u32);
            if expected != 0 {
                let src = if nmi { SRC_GPIO_NMI } else { SRC_GPIO };
                sources[src / 32] |= 1 << (src % 32);
            }
        }
        assert_eq!(p.source_status(core), sources);
        assert_eq!(p.read32(0x3ff0_00ec + core as u32 * 12), sources[0]);
        *expected_line = p.dport.cpu_lines(core, &sources);
    }
    let mut actual = [0; 2];
    p.all_cpu_lines(&mut actual);
    assert_eq!(actual, expected_lines);
    assert!(!p.gpio.irq(2, false));
}
fn config(p: &mut Peripherals, pin: usize, typ: u32, routes: u32) {
    p.write32(
        0x3ff4_4000 + 0x88 + pin as u32 * 4,
        (typ << 7) | (routes << 13),
    );
}
fn mux(p: &mut Peripherals, pin: usize, value: u32) {
    if IOMUX_OFFSETS[pin] != u32::MAX {
        p.write32(0x3ff4_9000 + IOMUX_OFFSETS[pin], value);
    }
}

#[test]
fn gpio_irq_routes_types_banks_and_software_status() {
    let mut p = Peripherals::new([0; 6]);
    check(&mut p);
    for core in 0..2 {
        let base = if core == 0 { 0x104 } else { 0x218 };
        p.write32(0x3ff0_0000 + base + SRC_GPIO as u32 * 4, 7);
        p.write32(0x3ff0_0000 + base + SRC_GPIO_NMI as u32 * 4, 14);
    }
    for pin in [0, 4, 31, 32, 39] {
        mux(&mut p, pin, (2 << 12) | (1 << 9));
        for typ in 0..8 {
            for routes in 0..32 {
                config(&mut p, pin, typ, routes);
                check(&mut p);
                for level in [false, true, false] {
                    p.gpio.set_input(pin as u8, level);
                    check(&mut p);
                    let bank = if pin < 32 { 0 } else { 0xc };
                    let bit = 1 << (pin % 32);
                    p.write32(0x3ff4_404c + bank, bit);
                    check(&mut p);
                    p.write32(0x3ff4_4048 + bank, bit);
                    check(&mut p);
                    p.write32(0x3ff4_4044 + bank, 0);
                    check(&mut p);
                }
            }
        }
        config(&mut p, pin, 0, 0);
    }
}

#[test]
fn gpio_irq_level_invalidation_pad_signal_pull_and_rtc() {
    let mut p = Peripherals::new([0; 6]);
    let pin = 4;
    config(&mut p, pin, 5, 15);
    check(&mut p);
    mux(&mut p, pin, (2 << 12) | (1 << 9));
    check(&mut p);
    p.gpio.set_input(4, false);
    check(&mut p);
    mux(&mut p, pin, (2 << 12) | (1 << 9) | (1 << 8));
    p.gpio.release_input(4);
    check(&mut p);
    mux(&mut p, pin, (2 << 12) | (1 << 9) | (1 << 7));
    check(&mut p);
    p.write32(0x3ff4_4024, 1 << pin);
    p.write32(0x3ff4_4008, 1 << pin);
    check(&mut p);
    p.write32(0x3ff4_4540, 71);
    check(&mut p);
    p.gpio.set_output_signal(71, true, true);
    check(&mut p);
    p.gpio.set_output_signal(71, false, true);
    check(&mut p);
    p.gpio.set_output_signal(71, true, false);
    check(&mut p);
    mux(&mut p, pin, (1 << 12) | (1 << 9));
    p.gpio.set_output_signal(12, true, true);
    check(&mut p);
    mux(&mut p, pin, 1 << 12);
    check(&mut p);
    mux(&mut p, pin, (2 << 12) | (1 << 9));
    // Exercise RTC_IO's digital-pad takeover and release through the MMIO path.
    p.write32(0x3ff4_8494, 1 << 19); // TOUCH_PAD0 / GPIO4 MUX_SEL
    check(&mut p);
    p.gpio.set_input(4, false);
    check(&mut p);
    p.write32(0x3ff4_8494, 0);
    check(&mut p);
    let adc = std::mem::take(&mut p.adc);
    p.restore_rtc_analog(adc);
    check(&mut p);
    // Preserve the old adapter's low-offset aliases, too.
    p.gpio.write(0x74, (5 << 7) | (15 << 13));
    mux(&mut p, 0, (2 << 12) | (1 << 9));
    check(&mut p);
}

#[test]
fn gpio_irq_cached_queries_match_uncached_mixed_trace() {
    let mut p = Peripherals::new([0; 6]);
    let mut rng = 0xcafe_babe_u32;
    for _ in 0..12000 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        let pin = (rng as usize >> 8) % 40;
        match rng % 9 {
            0 => config(&mut p, pin, (rng >> 16) & 7, (rng >> 20) & 31),
            1 => {
                p.gpio.set_input(pin as u8, rng & 0x10000 != 0);
            }
            2 => p.gpio.release_input(pin as u8),
            3 if IOMUX_OFFSETS[pin] != u32::MAX => mux(&mut p, pin, rng),
            4 => p.gpio.set_output_signal(
                (rng as usize >> 16) % 256,
                rng & 0x100 != 0,
                rng & 0x200 != 0,
            ),
            5 => p.write32(
                0x3ff4_4000 + [4, 8, 12, 16, 20, 24, 32, 36, 40, 44, 48, 52][pin % 12],
                rng,
            ),
            6 => p.write32(
                0x3ff4_4000 + [0x44, 0x48, 0x4c, 0x50, 0x54, 0x58][pin % 6],
                rng,
            ),
            7 if pin < 34 => p.write32(
                0x3ff4_4530 + pin as u32 * 4,
                (rng & 0xe00) | ((rng >> 16) % 257),
            ),
            _ => p.write32(0x3ff4_8494, rng),
        }
        check(&mut p);
    }
}

#[test]
fn gpio_irq_queries_and_unchanged_outputs_do_not_refresh() {
    let mut p = Peripherals::new([0; 6]);
    mux(&mut p, 4, (2 << 12) | (1 << 9));
    config(&mut p, 4, 5, 15);
    p.gpio.set_output_signal(71, true, true);
    let before = p.gpio.level_refreshes;
    for _ in 0..10 {
        check(&mut p);
        p.gpio.set_output_signal(71, true, true);
        p.write32(0x3ff4_404c, 1 << 4);
    }
    assert_eq!(p.gpio.level_refreshes, before);
}

#[test]
fn gpio_irq_output_writes_update_unmuxed_fallback_input() {
    let mut p = Peripherals::new([0; 6]);
    mux(&mut p, 4, 1 << 9); // input enabled, no digital output function
    config(&mut p, 4, 5, 15);
    check(&mut p);
    p.write32(0x3ff4_4024, 1 << 4);
    check(&mut p);
    assert_eq!(p.gpio.raw_status(), 0);
    p.write32(0x3ff4_4008, 1 << 4);
    check(&mut p);
    assert_eq!(p.gpio.raw_status(), 1 << 4);
    p.write32(0x3ff4_400c, 1 << 4);
    check(&mut p);
}

#[test]
fn gpio_irq_restore_takeover_and_live_other_sources() {
    let mut p = Peripherals::new([0; 6]);
    mux(&mut p, 4, (2 << 12) | (1 << 9));
    config(&mut p, 4, 5, 15);
    p.gpio.set_input(4, true);
    p.write32(0x3ff4_404c, 1 << 4); // isolate live level status from the sticky bit
    p.write32(0x3ff4_8494, 1 << 19);
    assert_eq!(
        p.gpio.raw_status(),
        0,
        "RTC takeover without driven-output change"
    );
    check(&mut p);
    p.write32(0x3ff4_8494, 0);
    assert_eq!(p.gpio.raw_status(), 1 << 4);
    check(&mut p);
    let mut old = Peripherals::new([0; 6]);
    old.write32(0x3ff4_8494, 1 << 19);
    p.restore_rtc_analog(old.adc);
    assert_eq!(p.gpio.raw_status(), 0);
    check(&mut p);
    p.restore_rtc_analog(ClassicAdc::new());
    assert_eq!(p.gpio.raw_status(), 1 << 4);
    check(&mut p);
    config(&mut p, 4, 0, 0);
    for core in 0..2 {
        let base = if core == 0 { 0x104 } else { 0x218 };
        p.write32(0x3ff0_0000 + base + 4 * SRC_UART0 as u32, 8);
        p.write32(0x3ff0_0000 + base + 4 * SRC_TG0_T0 as u32, 9);
    }
    p.write32(0x3ff4_000c, 1 << 8);
    check(&mut p);
    p.uart[0].host_input(b"x"); // Host mutation, without GPIO invalidation.
    let mut lines = [0; 2];
    p.all_cpu_lines(&mut lines);
    assert_eq!(lines, [1 << 8; 2]);
    check(&mut p);
    p.write32(
        0x3ff5_f000,
        (1 << 31) | (1 << 30) | (1 << 11) | (1 << 10) | (2 << 13),
    );
    p.write32(0x3ff5_f010, 10);
    p.write32(0x3ff5_f098, 1);
    p.tick(600);
    p.all_cpu_lines(&mut lines);
    assert_eq!(lines, [(1 << 8) | (1 << 9); 2]);
    check(&mut p);
    p.write32(0x3ff5_f0a4, 1);
    p.all_cpu_lines(&mut lines);
    assert_eq!(lines, [1 << 8; 2]);
    check(&mut p);
}

#[test]
fn a_gpio_pad_left_at_function_0_drives_its_pin() {
    let mut g = ClassicGpio::new();
    g.write(0x24, 1 << 23); // GPIO_ENABLE_W1TS_REG
    g.write(0x08, 1 << 23); // GPIO_OUT_W1TS_REG
    assert_eq!(g.driven(23), Some(true));
    assert!(g.gpio.changes.contains(&(23, true)));
}

#[test]
fn a_pad_whose_function_0_is_not_gpio_stays_undriven_until_function_2() {
    let mut g = ClassicGpio::new();
    g.write(0x24, 1 << 15);
    g.write(0x08, 1 << 15);
    assert_eq!(g.driven(15), None); // MTDO at function 0, io_mux_reg.h
    g.write(IOMUX + IOMUX_OFFSETS[15], 2 << 12);
    assert_eq!(g.driven(15), Some(true));
}

#[test]
fn the_shared_router_sees_a_pad_left_at_function_0_as_gpio_before_any_io_mux_write() {
    let mut g = ClassicGpio::new();
    g.write(0x24, 1 << 23 | 1 << 15);
    g.write(0x08, 1 << 23 | 1 << 15);
    assert_eq!(g.routes().software_output(23), Some(true));
    assert_eq!(g.routes().software_output(15), None); // MTDO at function 0, io_mux_reg.h
}
