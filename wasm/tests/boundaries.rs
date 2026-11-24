//! Host input boundaries must behave identically in native tests and the wasm32 build.
use esp32sim_wasm::*;

#[test]
fn memory_sizes_are_rejected_before_allocating_or_truncating() {
    for size in [33, 2048, 4096, u32::MAX] {
        // SAFETY: The name is readable and rejected construction retains no allocation.
        assert!(unsafe { esp32sim_new(b"none".as_ptr(), 4, size, 0) }.is_null());
        assert!(unsafe { esp32sim_new(b"none".as_ptr(), 4, 1, size) }.is_null());
    }
    let net = esp32sim_net_new(0.0);
    let mac = [2, 0, 0, 0, 0, 1];
    // SAFETY: This test uniquely owns the network and provides valid input buffers.
    unsafe {
        for size in [33, 4096, u32::MAX] {
            assert_eq!(esp32sim_net_add(net, mac.as_ptr(), size, 0.0, 0.0, 0.0, b"none".as_ptr(), 4), u32::MAX);
        }
        // Failed additions did not leave partially initialized nodes behind.
        assert_eq!(esp32sim_net_add(net, mac.as_ptr(), 1, 0.0, 0.0, 0.0, b"none".as_ptr(), 4), 0);
        esp32sim_net_delete(net);
    }
}

#[test]
fn single_and_network_loads_accept_the_same_input_kinds() {
    // SAFETY: Buffers stay readable and handles are uniquely owned throughout the test.
    unsafe {
        let single = esp32sim_new(b"esp32c6".as_ptr(), 7, 1, 0);
        assert!(!single.is_null());
        let net = esp32sim_net_new(0.0);
        let mac = [2, 0, 0, 0, 0, 1];
        let node = esp32sim_net_add(net, mac.as_ptr(), 1, 0.0, 0.0, 0.0, b"none".as_ptr(), 4);
        let cases: &[(u32, &[u8], u32)] = &[
            (6, b"0.1 gpio 2 1\n", 0),
            (6, &[0xff], 1),
            (7, b"P6\n1 1\n255\n\xff\x00\x00", 1),
            (7, b"bad picture", 1),
            (8, b"", 1),
        ];
        for &(kind, data, expected) in cases {
            assert_eq!(esp32sim_load(single, kind, data.as_ptr(), data.len()), expected);
            assert_eq!(esp32sim_net_load(net, node, kind, data.as_ptr(), data.len()), expected);
        }
        for name in ["0x40370000", "40370000", "missing_symbol"] {
            let expected = u32::from(name != "0x40370000");
            assert_eq!(esp32sim_stub(single, name.as_ptr(), name.len(), 0), expected);
            assert_eq!(esp32sim_net_stub(net, node, name.as_ptr(), name.len(), 0), expected);
        }
        esp32sim_delete(single);
        esp32sim_net_delete(net);
    }
}

#[test]
fn stub_symbols_take_precedence_over_hexadecimal_spelling() {
    let mut machine = esp32c6::machine([0; 6], 1 << 20);
    machine.symbols.insert(0x1234, "deadbeef".into());
    machine.symbols.insert(0x5678, "0xcafe".into());
    assert_eq!(machine.resolve_stub("deadbeef"), Some(0x1234));
    assert_eq!(machine.resolve_stub("0xcafe"), Some(0x5678));
    for missing in ["add", "cafe", "dead", "f"] { assert_eq!(machine.resolve_stub(missing), None); }
    assert_eq!(machine.resolve_stub("0x123abc"), Some(0x123abc));
    assert_eq!(machine.resolve_stub("0x0xcafe"), None);
}

#[test]
fn board_aliases_route_to_the_canonical_chip() {
    for (board, hz, flash) in [("bare", 240e6, 1), ("c3", 160e6, 1), ("c6", 160e6, 1), ("lcd147", 160e6, 4), ("c6-lcd147", 160e6, 4)] {
        // SAFETY: The name is readable and the handle is uniquely owned until deletion.
        unsafe {
            let e = esp32sim_new(board.as_ptr(), board.len(), flash, 0);
            assert!(!e.is_null(), "{board}");
            assert_eq!(esp32sim_cpu_hz(e), hz, "{board}");
            assert_eq!(esp32sim_set_spi2_timing(e, 1), u32::from(hz != 240e6));
            esp32sim_delete(e);
            if flash == 4 {
                let invalid = esp32sim_new(board.as_ptr(), board.len(), 1, 0);
                if !invalid.is_null() { esp32sim_delete(invalid); }
                assert!(invalid.is_null(), "{board} must retain the physical 4 MiB capacity gate");
            }
        }
    }
}

#[test]
fn complete_stub_specs_and_wifi_report_invalid_configuration() {
    // SAFETY: every buffer is live for its call and handles are exclusively owned here.
    unsafe {
        let e = esp32sim_new(b"none".as_ptr(), 4, 4, 2);
        assert!(!e.is_null());
        for spec in ["0x40000000=true", "0x40000000=false", "0x40000000=0xffffffff"] {
            assert_eq!(esp32sim_stub_spec(e, spec.as_ptr(), spec.len()), 0);
        }
        for spec in ["0x40000000=typo", "0x40000000=12abc", "0x40000000=1=2", "=1", "0x40000000=4294967296"] {
            assert_eq!(esp32sim_stub_spec(e, spec.as_ptr(), spec.len()), 1, "{spec}");
        }
        let valid = b"ssid=test,chan=1";
        assert_eq!(esp32sim_wifi(e, valid.as_ptr(), valid.len()), 0);
        for spec in ["ssid=test,chan=0", "ssid=test,unknown=1", "ssid=test,bssid=bad"] {
            assert_eq!(esp32sim_wifi(e, spec.as_ptr(), spec.len()), 1);
        }
        esp32sim_delete(e);
        let net = esp32sim_net_new(0.0);
        let mac = [2u8, 0, 0, 0, 0, 1];
        assert_eq!(esp32sim_net_add(net, mac.as_ptr(), 1, 0.0, 0.0, 0.0, b"typo".as_ptr(), 4), u32::MAX);
        assert_eq!(esp32sim_net_add(net, mac.as_ptr(), 1, 0.0, 0.0, 0.0, b"none".as_ptr(), 4), 0);
        let spec = b"0x40000000=true";
        assert_eq!(esp32sim_net_stub_spec(net, 0, spec.as_ptr(), spec.len()), 0);
        let spec = b"0x40000000=typo";
        assert_eq!(esp32sim_net_stub_spec(net, 0, spec.as_ptr(), spec.len()), 1);
        esp32sim_net_delete(net);
    }
}

#[test]
fn ethernet_relay_validates_frames_on_every_chip() {
    for board in ["none", "esp32c3", "esp32c6"] {
        // SAFETY: uniquely owned emulator and buffers live for each call.
        unsafe {
            let e = esp32sim_new(board.as_ptr(), board.len(), 1, 0);
            assert!(!e.is_null());
            assert_eq!(esp32sim_ethernet_take(e), 0);
            assert!(esp32sim_ethernet_ptr(e, 0).is_null());
            assert_eq!(esp32sim_ethernet_len(e, usize::MAX), 0);
            assert_eq!(esp32sim_ethernet_relay(e, 1), 0);
            assert_eq!(esp32sim_ethernet_receive(e, [0; 14].as_ptr(), 14), 1);
            assert_eq!(esp32sim_wifi(e, std::ptr::null(), 0), 0);
            for len in [0, 13, 1519, usize::MAX] {
                assert_eq!(esp32sim_ethernet_receive(e, std::ptr::null(), len), 1);
            }
            assert_eq!(esp32sim_ethernet_receive(e, std::ptr::null(), 14), 1);
            for _ in 0..64 { assert_eq!(esp32sim_ethernet_receive(e, [0; 14].as_ptr(), 14), 0); }
            assert_eq!(esp32sim_ethernet_receive(e, [0; 14].as_ptr(), 14), 1);
            assert_eq!(esp32sim_ethernet_relay(e, 0), 0);
            assert_eq!(esp32sim_ethernet_receive(e, [0; 14].as_ptr(), 14), 1);
            esp32sim_delete(e);
        }
    }
}

#[test]
fn full_ble_rejects_other_chips() {
    // SAFETY: each readable name and handle is exclusively owned until deletion.
    unsafe {
        for chip in ["none", "esp32c6", "esp32c3"] {
            let e = esp32sim_new(chip.as_ptr(), chip.len(), 4, 0);
            assert!(!e.is_null());
            assert_eq!(esp32sim_ble_full(e), u32::from(chip != "esp32c3"));
            esp32sim_delete(e);
        }
    }
}
