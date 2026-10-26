#[test]
fn released_pad_resolves_output_pulls_and_interrupts() {
    let mut gpio = esp_periph::Gpio::new();
    for pin in [0, 7, 32, 48] {
        let bit = 1u64 << pin;
        gpio.write(0x74 + 4 * pin as u32, (3 << 7) | (1 << 13));
        gpio.set_input(pin as u8, false);
        gpio.set_pulls(pin as u8, true, false);
        assert_eq!(gpio.input & bit, 0);
        assert!(gpio.release_input(pin as u8));
        assert_ne!(gpio.input & bit, 0);
        assert_ne!(gpio.status & bit, 0);
        let (out, ena, shift) = if pin < 32 {
            (4, 0x24, pin)
        } else {
            (0x10, 0x30, pin - 32)
        };
        gpio.write(ena, 1 << shift);
        assert_eq!(gpio.input & bit, 0);
        gpio.write(out, 1 << shift);
        assert_ne!(gpio.input & bit, 0);
        gpio.set_input(pin as u8, false);
        assert!(!gpio.level(pin as u8));
        gpio.release_input(pin as u8);
        assert!(gpio.level(pin as u8));
        gpio.set_pulls(pin as u8, false, true);
        gpio.write(ena + 4, 1 << shift);
        assert!(!gpio.level(pin as u8));
        gpio.set_pulls(pin as u8, false, false);
        assert!(gpio.level(pin as u8));
    }
    let before = gpio.input;
    assert!(!gpio.release_input(255));
    assert!(!gpio.set_input(255, false));
    gpio.set_pulls(255, false, true);
    assert_eq!(gpio.input, before);
}

#[test]
fn output_edges_only_queue_for_selected_matrix_inputs() {
    for select in [0x40, 0x80] {
        let mut gpio = esp_periph::Gpio::new();
        gpio.input_select = select;
        for pin in [4, 40] {
            if select == 0x40 && pin == 40 { continue; }
            let (out, enable) = if pin < 32 { (4, 0x24) } else { (0x10, 0x30) };
            let bit = 1 << (pin % 32);
            gpio.write(enable, bit);
            gpio.write(out, bit);
            assert!(gpio.input_changes.is_empty());
            gpio.write(0x154, select | pin);
            gpio.write(0x158, select | pin); // A second route must survive removal of the first.
            gpio.write(0x154, 0x3c);
            gpio.write(out, 0);
            assert_eq!(gpio.input_changes, [(pin as u8, false)]);
            gpio.input_changes.clear();
            gpio.write(0x158, pin | (select / 2)); // Inversion without matrix selection.
            gpio.write(out, bit);
            assert!(gpio.input_changes.is_empty());
            gpio.write(0x158, select | pin | (select / 2));
            gpio.write(out, 0);
            assert_eq!(gpio.input_changes, [(pin as u8, false)]);
            gpio.input_changes.clear();
            gpio.write(0x158, select | (select / 2 - 1)); // Constant/invalid pin.
            gpio.write(out, bit);
            assert!(gpio.input_changes.is_empty());
        }
    }
}

#[test]
fn input_change_result_does_not_depend_on_irq_configuration() {
    let mut gpio = esp_periph::Gpio::new();
    assert!(gpio.set_input(4, false));
    assert!(!gpio.set_input(4, false));
    assert!(gpio.release_input(4));
    assert!(!gpio.release_input(4));
    assert_eq!(gpio.status, 0);
}

#[test]
fn open_drain_high_releases_to_pulls_and_low_drives_without_losing_external_inputs() {
    let mut gpio = esp_periph::Gpio::new();
    for pin in [5u8, 37] {
        let mask = 1u32 << (pin & 31);
        let (out_set, out_clear, enable_set, enable_clear) = if pin < 32 { (8, 12, 0x24, 0x28) } else { (0x14, 0x18, 0x30, 0x34) };
        gpio.set_pulls(pin, false, true);
        gpio.write(out_set, mask); gpio.write(enable_set, mask);
        assert!(gpio.level(pin), "push-pull HIGH overcomes pulldown");
        gpio.write(0x74 + u32::from(pin) * 4, 4);
        assert!(!gpio.level(pin), "PAD_DRIVER makes HIGH release to pulldown immediately");
        gpio.set_pulls(pin, true, false);
        assert!(gpio.level(pin), "released open drain follows pullup");
        gpio.write(out_clear, mask);
        assert!(!gpio.level(pin), "open-drain LOW drives against pullup");
        gpio.write(enable_clear, mask);
        assert!(gpio.level(pin), "disabled LOW output releases");
        gpio.write(enable_set, mask); gpio.write(out_set, mask);
        gpio.set_input(pin, false);
        assert!(!gpio.level(pin), "external LOW on a released line wins over pullup");
        gpio.set_input(pin, true); gpio.set_pulls(pin, false, true);
        assert!(gpio.level(pin), "external HIGH on a released line wins over pulldown");
        gpio.release_input(pin);
        assert!(!gpio.level(pin), "releasing an external drive restores the original pull");
        gpio.write(0x74 + u32::from(pin) * 4, 0);
        assert!(gpio.level(pin), "return to push-pull takes effect without another OUT write");
    }
}
