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
fn pmic_observes_nominal_clock_against_current_speed_without_changing_ack() {
    let state=Arc::new(Mutex::new(M5pm1State::default()));
    let mut pmic=M5pm1::new(state.clone());
    assert!(!pmic.start_address_with_clock(0x6e,0x4f,false,Some(400_000)));
    assert_eq!(state.lock().unwrap().i2c_address_phases,0);
    assert!(pmic.start_address_with_clock(0x6e,0x6e,false,Some(400_000)));
    {let s=state.lock().unwrap();assert_eq!((s.i2c_first_hz,s.i2c_last_hz,s.i2c_max_hz),(Some(400_000),Some(400_000),400_000));assert_eq!(s.i2c_speed_mismatches,1);assert_eq!(s.i2c_last_mismatch_limit_hz,Some(100_000));}
    // The safe order sets the slave speed while the master is still at 100 kHz.
    assert!(pmic.start_address_with_clock(0x6e,0x6e,false,Some(100_000)));
    pmic.write(0x09);pmic.write(0x10);pmic.stop();
    assert!(pmic.start_address_with_clock(0x6e,0x6e,true,Some(400_000)));
    assert!(pmic.start_address_with_clock(0x6e,0x6e,true,Some(100_000)));
    {let s=state.lock().unwrap();assert_eq!(s.i2c_speed_hz,400_000);assert_eq!(s.i2c_speed_mismatches,1);}
    write(&mut pmic,0x09,&[0]);
    assert!(pmic.start_address_with_clock(0x6e,0x6e,false,Some(400_000)));
    // Bit-banged/direct accesses explicitly replace the last known clock with unknown.
    assert!(pmic.start(false));
    let s=state.lock().unwrap();assert_eq!(s.i2c_last_hz,None);assert_eq!(s.i2c_first_hz,Some(400_000));assert_eq!(s.i2c_speed_mismatches,2);
    assert_eq!((s.i2c_last_mismatch_hz,s.i2c_last_mismatch_limit_hz),(Some(400_000),Some(100_000)));
}

#[test]
fn pmic_clock_diagnostic_allows_small_nominal_divider_rounding() {
    let state=Arc::new(Mutex::new(M5pm1State::default()));let mut pmic=M5pm1::new(state.clone());
    assert!(pmic.start_address_with_clock(0x6e,0x6e,false,Some(100_500)));
    assert!(pmic.start_address_with_clock(0x6e,0x6e,false,Some(101_000)));
    assert_eq!(state.lock().unwrap().i2c_speed_mismatches,0);
    assert!(pmic.start_address_with_clock(0x6e,0x6e,false,Some(101_001)));
    assert_eq!(state.lock().unwrap().i2c_speed_mismatches,1);
    write(&mut pmic,0x09,&[0x10]);
    assert!(pmic.start_address_with_clock(0x6e,0x6e,false,Some(400_500)));
    assert_eq!(state.lock().unwrap().i2c_speed_mismatches,1);
}

#[test]
fn pmic_download_lock_and_led_observation_preserve_register_semantics() {
    let state=Arc::new(Mutex::new(M5pm1State::default()));let mut pmic=M5pm1::new(state.clone());
    write(&mut pmic,0x49,&[0x2b]); // unrelated single-press setting is not a lock
    write(&mut pmic,0x06,&[7]); // factory firmware intentionally turns the LED off
    {let s=state.lock().unwrap();assert!(!s.led_enabled);assert!(!s.download_locked);assert_eq!(s.download_lock_writes,0);}
    write(&mut pmic,0x49,&[0xab]);write(&mut pmic,0x49,&[0xab]);
    {let s=state.lock().unwrap();assert!(s.download_locked);assert_eq!(s.download_lock_writes,2);}
    write(&mut pmic,0x49,&[0x2b]);
    assert_eq!(read(&mut pmic,0x49,1),[0x2b]);
    {let s=state.lock().unwrap();assert!(!s.download_locked);assert_eq!(s.download_unlock_attempts,1);}
    let _reattached=M5pm1::new(state.clone());
    let s=state.lock().unwrap();assert!(s.led_enabled);assert!(!s.download_locked);assert_eq!(s.download_lock_writes,0);assert_eq!(s.download_unlock_attempts,0);assert_eq!(s.i2c_address_phases,0);
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
        battery_mv: 4150, charging: true, ..Default::default()
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

#[test]
fn pmic_board_input_and_ldo_hold_do_not_override_output_direction() {
    let state=Arc::new(Mutex::new(M5pm1State{input_mask:2,input_high:2,..Default::default()}));
    let mut device=M5pm1::new(state.clone());assert_eq!(read(&mut device,0x12,1)[0]&2,2);
    write(&mut device,0x10,&[2]);write(&mut device,0x13,&[0]);assert_eq!(read(&mut device,0x12,1)[0]&2,0);
    write(&mut device,0x11,&[2]);assert_eq!(read(&mut device,0x12,1)[0]&2,2);
    assert!(state.lock().unwrap().ldo3v3_enabled);write(&mut device,0x06,&[0x13]);assert!(!state.lock().unwrap().ldo3v3_enabled);
    write(&mut device,0x06,&[0x17]);write(&mut device,0x07,&[0x20]);write(&mut device,0x0c,&[0xa1]);assert!(state.lock().unwrap().ldo3v3_enabled);
    write(&mut device,0x07,&[0]);assert!(!state.lock().unwrap().ldo3v3_enabled);
    let mut device=M5pm1::new(state.clone());assert_eq!(read(&mut device,0x12,1)[0]&2,2);assert!(state.lock().unwrap().ldo3v3_enabled);
}

#[test]
fn pmic_gpio_irq_is_latched_on_both_edges_and_clears_with_zero() {
    // CoreMatrix KEY1..3 -> PM1 GPIO0..2; PM1 GPIO3 -> ESP32-C61 GPIO2.
    let state=Arc::new(Mutex::new(M5pm1State{input_mask:0x17,input_high:7,..Default::default()}));
    let mut device=M5pm1::new(state.clone());
    state.lock().unwrap().set_input(2,false);
    assert_eq!(read(&mut device,0x40,1),[0]); // no IRQ output configured
    write(&mut device,0x10,&[8]);write(&mut device,0x13,&[0]);
    write(&mut device,0x16,&[0x40]); // GPIO3=IRQ, GPIO0..2=ordinary input
    assert_eq!(state.lock().unwrap().irq_pins,8);assert_eq!(read(&mut device,0x40,1),[0]);
    state.lock().unwrap().set_input(2,true);
    assert_eq!(read(&mut device,0x40,1),[4]);assert!(state.lock().unwrap().irq_low);
    assert_eq!(read(&mut device,0x12,1)[0]&8,0);
    write(&mut device,0x40,&[0xff]);assert_eq!(read(&mut device,0x40,1),[4]);
    state.lock().unwrap().set_input(1,false);
    assert_eq!(read(&mut device,0x40,1),[6]);
    write(&mut device,0x40,&[!4]);assert_eq!(read(&mut device,0x40,1),[2]);
    write(&mut device,0x40,&[!2]);assert!(!state.lock().unwrap().irq_low);
    assert_eq!(read(&mut device,0x12,1)[0]&8,8);
    write(&mut device,0x40,&[0xff]);assert_eq!(read(&mut device,0x40,1),[0]);
    state.lock().unwrap().set_input(0,false);
    write(&mut device,0x16,&[0]);assert_eq!(read(&mut device,0x40,3),[0,0,0]);
}

#[test]
fn pmic_gpio_irq_masks_gate_output_without_discarding_pending_edges() {
    let state=Arc::new(Mutex::new(M5pm1State{input_mask:0x17,input_high:7,..Default::default()}));
    let mut device=M5pm1::new(state.clone());
    write(&mut device,0x16,&[0x40]);write(&mut device,0x43,&[4]);
    state.lock().unwrap().set_input(2,false);
    assert_eq!(read(&mut device,0x40,1),[4]);assert!(!state.lock().unwrap().irq_low);
    write(&mut device,0x43,&[0]);assert!(state.lock().unwrap().irq_low);
    write(&mut device,0x40,&[0]);
    write(&mut device,0x10,&[4]); // output KEY3 isn't scanned
    state.lock().unwrap().set_input(2,true);
    state.lock().unwrap().set_input(3,false); // IRQ output isn't scanned
    state.lock().unwrap().set_input(31,true); // invalid guest/host pin ignored
    assert_eq!(read(&mut device,0x40,1),[0]);
    write(&mut device,0x10,&[0]);write(&mut device,0x16,&[0x70]); // GPIO2=other
    state.lock().unwrap().set_input(2,false);
    assert_eq!(read(&mut device,0x40,1),[0]);
}

#[test]
fn pmic_dcdc_shutdown_hold_and_gpio_wake_preserve_wake_configuration() {
    let state=Arc::new(Mutex::new(M5pm1State{input_mask:7,input_high:7,..Default::default()}));
    let mut device=M5pm1::new(state.clone());
    write(&mut device,0x18,&[4]);write(&mut device,0x19,&[0]); // KEY3 falling wake
    write(&mut device,0xa0,&[0x12,0x34]);
    write(&mut device,0x06,&[0x1f]);write(&mut device,0x07,&[0x60]);
    write(&mut device,0x0c,&[0xa1]);
    {let s=state.lock().unwrap();assert!(!s.dcdc_enabled);assert!(!s.led_enabled);assert!(s.ldo3v3_enabled);assert!(s.grove_5v_enabled);}
    state.lock().unwrap().set_input(1,false);assert!(state.lock().unwrap().power_off_requested);
    state.lock().unwrap().set_input(2,false);
    {let s=state.lock().unwrap();assert!(s.wake_requested);assert!(!s.power_off_requested);assert!(s.dcdc_enabled);assert_eq!(s.wake_source(),0x20);}
    assert_eq!(read(&mut device,0x18,2),[4,0]);assert_eq!(read(&mut device,0xa0,2),[0x12,0x34]);
    assert_eq!(read(&mut device,0x07,1),[0]); // wake restart clears output holds
    write(&mut device,0x16,&[0x40]);assert_eq!(read(&mut device,0x42,1),[2]);
    write(&mut device,0x42,&[0]);assert_eq!(read(&mut device,0x42,1),[2]); // wake source still asserted
    write(&mut device,0x05,&[0]);write(&mut device,0x42,&[0]);assert!(!state.lock().unwrap().irq_low);
    write(&mut device,0x05,&[0xff]);assert_eq!(read(&mut device,0x05,1),[0]);
}

#[test]
fn pmic_gpio_wake_rejects_wrong_edge_gpio1_and_conflicting_pairs() {
    for (enabled,edge,pin,level,wakes) in [(4,4,2,false,false),(4,4,2,true,true),(2,0,1,false,false),(5,0,2,false,false),(0x18,0,4,false,false)] {
        let initial=if level {0}else{0x1f};
        let state=Arc::new(Mutex::new(M5pm1State{input_mask:0x1f,input_high:initial,..Default::default()}));
        let mut device=M5pm1::new(state.clone());
        write(&mut device,0x18,&[enabled,edge]);write(&mut device,0x0c,&[0xa1]);
        state.lock().unwrap().set_input(pin,level);
        assert_eq!(state.lock().unwrap().wake_requested,wakes,"enable={enabled} edge={edge} pin={pin}");
    }
}

#[test]
fn pmic_power_button_status_irq_double_click_and_long_lock() {
    let state=Arc::new(Mutex::new(M5pm1State::default()));let mut device=M5pm1::new(state.clone());
    write(&mut device,0x49,&[0x2b]);write(&mut device,0x4a,&[1]);write(&mut device,0x16,&[0x40]);
    state.lock().unwrap().power_button(true,100);
    assert_eq!(state.lock().unwrap().next_deadline_ms(),Some(2100));
    assert_eq!(read(&mut device,0x48,1),[0x81]);assert_eq!(read(&mut device,0x48,1),[1]);
    write(&mut device,0x48,&[0xff]);assert_eq!(read(&mut device,0x48,1),[1]);
    state.lock().unwrap().power_button(false,150);assert_eq!(read(&mut device,0x48,1),[0]);
    assert_eq!(state.lock().unwrap().next_deadline_ms(),Some(400));
    state.lock().unwrap().advance_to_ms(399);assert_eq!(read(&mut device,0x42,1),[0]);
    state.lock().unwrap().advance_to_ms(400);assert_eq!(read(&mut device,0x42,1),[1]);assert!(state.lock().unwrap().irq_low);
    write(&mut device,0x42,&[0]);
    for (pressed,t) in [(true,500),(false,550),(true,650),(false,700)] {state.lock().unwrap().power_button(pressed,t);}
    assert_eq!(read(&mut device,0x42,1),[4]);assert!(!state.lock().unwrap().power_off_requested);
    write(&mut device,0x42,&[0]);write(&mut device,0x49,&[0xab]);
    state.lock().unwrap().power_button(true,1000);state.lock().unwrap().advance_to_ms(3000);
    assert!(!state.lock().unwrap().download_requested);
    state.lock().unwrap().power_button(false,3001);state.lock().unwrap().advance_to_ms(4000);
    assert_eq!(read(&mut device,0x42,1),[0]); // locked long press isn't a short click
    assert_eq!(state.lock().unwrap().next_deadline_ms(),None);
}

#[test]
fn pmic_power_button_reset_shutdown_and_wake_use_guest_time() {
    let state=Arc::new(Mutex::new(M5pm1State::default()));let mut device=M5pm1::new(state.clone());
    state.lock().unwrap().power_button(true,0);state.lock().unwrap().power_button(false,50);
    state.lock().unwrap().advance_to_ms(299);assert!(!state.lock().unwrap().reset_requested);
    state.lock().unwrap().advance_to_ms(300);assert!(state.lock().unwrap().reset_requested);
    assert_eq!(read(&mut device,0x05,1),[8]);
    for (pressed,t) in [(true,400),(false,450),(true,550),(false,600)] {state.lock().unwrap().power_button(pressed,t);}
    assert!(state.lock().unwrap().power_off_requested);assert!(!state.lock().unwrap().dcdc_enabled);
    state.lock().unwrap().power_button(true,800);
    assert!(state.lock().unwrap().wake_requested);assert!(state.lock().unwrap().dcdc_enabled);
    state.lock().unwrap().power_button(false,850);state.lock().unwrap().advance_to_ms(2000);
    assert_eq!(read(&mut device,0x05,1),[12]); // a wake press doesn't become another click
}

#[test]
fn pmic_commands_require_key_and_preserve_rtc_and_wake_config_on_reset() {
    let state=Arc::new(Mutex::new(M5pm1State::default()));let mut device=M5pm1::new(state.clone());
    write(&mut device,0xa0,&[0x55]);write(&mut device,0x18,&[4,4]);write(&mut device,0x16,&[0x40]);
    for command in [2,0x92,0xb2,0xa0] {write(&mut device,0x0c,&[command]);assert!(!state.lock().unwrap().reset_requested);}
    write(&mut device,0x0c,&[0xa2]);assert!(state.lock().unwrap().reset_requested);assert_eq!(read(&mut device,0x05,1),[0x10]);
    assert_eq!(read(&mut device,0x16,4),[0,0,4,4]);assert_eq!(read(&mut device,0xa0,1),[0x55]);
    write(&mut device,0x49,&[0xaa]);write(&mut device,0x0c,&[0xa3]);assert!(!state.lock().unwrap().download_requested);
    let mut device=M5pm1::new(state.clone());assert!(!state.lock().unwrap().reset_requested);assert_eq!(read(&mut device,0xa0,1),[0]);
    write(&mut device,0x0c,&[0xa3]);assert!(state.lock().unwrap().download_requested);
}

#[test]
fn expander_pwm_frequency_and_corematrix_buzzer_channel_follow_guest_registers() {
    let state=Arc::new(Mutex::new(M5ioe1State::default()));let mut device=M5ioe1::new(state.clone(),0);
    write(&mut device,0x25,&1500u16.to_le_bytes());
    write(&mut device,0x1f,&[0xff,0x87]); // PWM3/IO11, ~50% duty, enabled
    assert_eq!(state.lock().unwrap().pwm,[0;4]);
    write(&mut device,0x04,&[4]);write(&mut device,0x14,&[0x3b]);
    {let s=state.lock().unwrap();assert_eq!(s.pwm_frequency_hz,1500);assert_eq!(s.pwm[2],2047);}
    write(&mut device,0x20,&[0xc7]);assert_eq!(state.lock().unwrap().pwm[2],2048);
    write(&mut device,0x25,&[0,0]);assert_eq!(state.lock().unwrap().pwm,[0;4]);
    assert_eq!(state.lock().unwrap().pwm_frequency_hz,0);
}
