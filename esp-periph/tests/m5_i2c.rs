use esp_periph::i2c::{m5::{M5ioe1, M5ioe1State, M5pm1, M5pm1State}, I2cDevice};
use std::sync::{Arc, Mutex};

fn read(device: &mut dyn I2cDevice, reg: u8, count: usize) -> Vec<u8> {
    assert!(device.start(false));
    assert!(device.write(reg));
    assert!(device.start(true));
    let bytes = (0..count).map(|_| device.read()).collect();
    device.stop();
    bytes
}

fn write(device: &mut dyn I2cDevice, reg: u8, bytes: &[u8]) {
    assert!(device.start(false));
    assert!(device.write(reg));
    for &byte in bytes { assert!(device.write(byte)); }
    device.stop();
}

fn update(device: &mut dyn I2cDevice, reg: u8, set: u8, clear: u8) {
    let value = read(device, reg, 1)[0];
    write(device, reg, &[(value & !clear) | set]);
}

#[test]
fn stopwatch_pmic_wake_and_power_up_rmw() {
    let state = Arc::new(Mutex::new(M5pm1State::default()));
    let mut device: Box<dyn I2cDevice> = Box::new(M5pm1::new(state.clone()));
    assert_eq!(M5pm1::ADDRESS, 0x6e);
    assert_eq!(read(&mut *device, 0x00, 2), [0x50, 0x20]);
    assert_eq!(read(&mut *device, 0x06, 1), [0x17]);
    assert_eq!(read(&mut *device, 0x49, 1), [0x2a]);

    write(&mut *device, 0x06, &[0xbf]);
    write(&mut *device, 0x16, &[0xff]);
    write(&mut *device, 0x10, &[0x1f]);
    update(&mut *device, 0x06, 0, 0x10);
    write(&mut *device, 0x09, &[0]);
    write(&mut *device, 0x0a, &[0]);
    update(&mut *device, 0x06, 0x07, 0x10);
    update(&mut *device, 0x16, 0, 0x30);
    update(&mut *device, 0x10, 0, 0x04);
    update(&mut *device, 0x49, 0x80, 0);

    assert_eq!(read(&mut *device, 0x06, 1), [0xaf]);
    assert_eq!(read(&mut *device, 0x09, 2), [0, 0]);
    assert_eq!(read(&mut *device, 0x16, 1), [0xcf]);
    assert_eq!(read(&mut *device, 0x10, 1), [0x1b]);
    assert_eq!(read(&mut *device, 0x49, 1), [0xaa]);
    assert!(!state.lock().unwrap().power_off_requested);
}

#[test]
fn stopwatch_battery_reads_host_voltage_and_active_low_charge_input() {
    let state = Arc::new(Mutex::new(M5pm1State::default()));
    let mut device = M5pm1::new(state.clone());
    assert_eq!(read(&mut device, 0x22, 2), 3900u16.to_le_bytes());
    assert_eq!(read(&mut device, 0x12, 1), [4]);
    state.lock().unwrap().battery_mv = 4150;
    state.lock().unwrap().charging = true;
    assert_eq!(read(&mut device, 0x22, 2), 4150u16.to_le_bytes());
    assert_eq!(read(&mut device, 0x12, 1), [0]);
    state.lock().unwrap().battery_mv = 0;
    assert_eq!(read(&mut device, 0x22, 2), [0, 0]);
}

#[test]
fn stopwatch_shutdown_is_observable_only_after_system_command() {
    let state = Arc::new(Mutex::new(M5pm1State::default()));
    let mut device: Box<dyn I2cDevice> = Box::new(M5pm1::new(state.clone()));
    write(&mut *device, 0x07, &[0x7f]);
    update(&mut *device, 0x06, 0, 0x10);
    write(&mut *device, 0x07, &[0]);
    assert_eq!(read(&mut *device, 0x06, 1), [0x07]);
    assert_eq!(read(&mut *device, 0x07, 1), [0]);
    assert!(!state.lock().unwrap().power_off_requested);
    for command in [0x01, 0xa0, 0xa2, 0xa3] {
        write(&mut *device, 0x0c, &[command]);
        assert!(!state.lock().unwrap().power_off_requested);
    }
    assert!(device.start(false));
    assert!(device.write(0x0c));
    assert!(!state.lock().unwrap().power_off_requested);
    assert!(device.write(0xa1));
    assert!(state.lock().unwrap().power_off_requested);
    device.stop();
    write(&mut *device, 0x09, &[0]);
    assert!(state.lock().unwrap().power_off_requested);
}

#[test]
fn reattaching_pmic_clears_shutdown_without_changing_host_inputs() {
    let state = Arc::new(Mutex::new(M5pm1State {
        battery_mv: 4150, charging: true, power_off_requested: false, led_enabled: true, grove_5v_enabled: false, driven_high: 0,
    }));
    let mut device = M5pm1::new(state.clone());
    write(&mut device, 0x0c, &[0xa1]);
    assert!(state.lock().unwrap().power_off_requested);
    let mut device = M5pm1::new(state.clone());
    assert!(!state.lock().unwrap().power_off_requested);
    assert_eq!(read(&mut device, 0x22, 2), 4150u16.to_le_bytes());
    assert_eq!(read(&mut device, 0x12, 1), [0]);
    assert_eq!(read(&mut device, 0x06, 1), [0x17]);
}

#[test]
fn stopwatch_expander_rmw_and_reset_pulse_leave_rail_on() {
    let state = Arc::new(Mutex::new(M5ioe1State::default()));
    let mut device: Box<dyn I2cDevice> = Box::new(M5ioe1::new(state.clone(), 0x1234));
    assert_eq!(M5ioe1::ADDRESS, 0x4f);
    assert_eq!(read(&mut *device, 0x00, 2), [0x34, 0x12]);
    assert_eq!(*state.lock().unwrap(), M5ioe1State::default());
    write(&mut *device, 0x23, &[0]);
    update(&mut *device, 0x13, 0, 0x9d);
    update(&mut *device, 0x03, 0x9d, 0);
    update(&mut *device, 0x05, 0x99, 0);
    let ready = M5ioe1State { driven_high: 0x99, oled_rail: true, oled_reset_high: true, touch_reset_high: true, ..Default::default() };
    assert_eq!(*state.lock().unwrap(), ready);
    assert_eq!(read(&mut *device, 0x13, 1), [0x62]);
    assert_eq!(read(&mut *device, 0x03, 1), [0x9d]);
    assert_eq!(read(&mut *device, 0x05, 1), [0x99]);
    assert_eq!(read(&mut *device, 0x23, 1), [0]);
    update(&mut *device, 0x05, 0, 0x18);
    assert_eq!(*state.lock().unwrap(), M5ioe1State { driven_high: 0x81, oled_rail: true, ..Default::default() });
    assert_eq!(read(&mut *device, 0x05, 1), [0x81]);
    update(&mut *device, 0x05, 0x18, 0);
    assert_eq!(*state.lock().unwrap(), ready);
    update(&mut *device, 0x05, 0, 0x80);
    assert_eq!(*state.lock().unwrap(), M5ioe1State { driven_high: 0x19, oled_rail: false, ..ready });
}

#[test]
fn expander_levels_require_output_mode_and_push_pull_drive() {
    let state = Arc::new(Mutex::new(M5ioe1State::default()));
    let mut device = M5ioe1::new(state.clone(), 0);
    write(&mut device, 0x05, &[0x98]);
    assert_eq!(*state.lock().unwrap(), M5ioe1State::default());
    write(&mut device, 0x03, &[0x98]);
    assert_eq!(*state.lock().unwrap(), M5ioe1State::default());
    write(&mut device, 0x13, &[0x67]);
    assert_eq!(*state.lock().unwrap(), M5ioe1State { driven_high: 0x98, oled_rail: true, oled_reset_high: true, touch_reset_high: true, ..Default::default() });
    update(&mut device, 0x03, 0, 0x10);
    assert!(!state.lock().unwrap().oled_reset_high);
    update(&mut device, 0x13, 0x80, 0);
    assert!(!state.lock().unwrap().oled_rail);
    assert!(state.lock().unwrap().touch_reset_high);
}

#[test]
fn expander_gpio_input_reads_driven_outputs_in_both_banks() {
    // Official UserDemo V0.5 polls IO8 via M5IOE1 1.0.8 digitalRead after
    // writing L3B_EN. The input registers must observe the driven output.
    let mut device = M5ioe1::new(Default::default(), 0);
    write(&mut device, 0x03, &[0x98, 0xff]);
    write(&mut device, 0x05, &[0x98, 0xff]);
    assert_eq!(read(&mut device, 0x07, 2), [0, 0]); // default open drain
    write(&mut device, 0x13, &[0x67, 0]);
    assert_eq!(read(&mut device, 0x07, 2), [0x98, 0x3f]);
    write(&mut device, 0x07, &[0, 0]); // GPIO_IN is read-only
    assert_eq!(read(&mut device, 0x07, 2), [0x98, 0x3f]);
    write(&mut device, 0x05, &[0x18, 0x15]);
    assert_eq!(read(&mut device, 0x07, 2), [0x18, 0x15]);
    write(&mut device, 0x03, &[0, 0]); // inputs have no supplied external level
    assert_eq!(read(&mut device, 0x07, 2), [0, 0]);
    write(&mut device, 0x07, &[0xff, 0xff]);
    assert_eq!(read(&mut device, 0x07, 2), [0, 0]);
}

#[test]
fn identity_and_battery_registers_are_read_only_and_bursts_increment() {
    let mut pmic = M5pm1::new(Default::default());
    write(&mut pmic, 0x00, &[0; 4]);
    assert_eq!(read(&mut pmic, 0x00, 4), [0x50, 0x20, 0x05, 0x06]);
    write(&mut pmic, 0x22, &[0, 0]);
    assert_eq!(read(&mut pmic, 0x22, 2), 3900u16.to_le_bytes());
    write(&mut pmic, 0x09, &[0x10, 0x20]);
    assert_eq!(read(&mut pmic, 0x09, 2), [0x10, 0x20]);
    let mut expander = M5ioe1::new(Default::default(), 0x1234);
    write(&mut expander, 0x00, &[0, 0, 0, 0x9d]);
    assert_eq!(read(&mut expander, 0x00, 4), [0x34, 0x12, b'A', 0x9d]);
}
