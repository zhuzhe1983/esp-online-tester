//! Public factory routing must preserve exact wafer families and board inputs.
use super::*;

#[test]
fn waveshare_p4_exact_profiles_choose_the_matching_production_wafer() {
    // SAFETY: each literal lives through the call and each non-null handle is
    // exclusively owned, inspected, then deleted exactly once.
    unsafe {
        for (name, flash, revision) in [
            ("waveshare-p4-lcd35-v13", 16, 100),
            ("waveshare-p4-lcd35-v32", 16, 301),
            ("waveshare-p4-lcd43-v13", 32, 100),
            ("waveshare-p4-lcd43-v32", 32, 301),
            ("waveshare-p4-lcd5-v13", 32, 100),
            ("waveshare-p4-lcd5-v32", 32, 301),
            ("waveshare-p4-knob-amoled175", 32, 301),
        ] {
            let e = esp32sim_new(name.as_ptr(), name.len(), flash, 32);
            assert!(!e.is_null(), "{name}");
            match (&(*e).m, revision) {
                (MachineKind::P4(m), 100) => {
                    assert_eq!(m.bus.board.name(), name);
                    assert_eq!(m.bus.flash.len(), (flash as usize) << 20);
                }
                (MachineKind::P4Rev3(m), 301) => {
                    assert_eq!(m.bus.board.name(), name);
                    assert_eq!(m.bus.flash.len(), (flash as usize) << 20);
                }
                _ => panic!("{name}: wrong production chip revision"),
            }
            esp32sim_delete(e);
            let unverified = format!("{name}+c6");
            assert!(esp32sim_new(unverified.as_ptr(), unverified.len(), flash, 32).is_null(),
                    "an exact SKU cannot silently acquire an unverified co-simulation topology");
        }
    }
}

#[test]
fn waveshare_s31_factory_keeps_rgb_bus_pins_free_of_devkit_identity() {
    // SAFETY: this test uniquely owns the handle until deletion.
    unsafe {
        let name = "waveshare-s31-touch-lcd43c";
        let e = esp32sim_new(name.as_ptr(), name.len(), 16, 16);
        assert!(!e.is_null());
        let MachineKind::S31(m) = &mut (*e).m else { panic!("S31 factory required"); };
        assert_eq!(m.bus.board.name(), name);
        let inputs = m.bus.board_inputs.as_ref().unwrap();
        assert_eq!(inputs.psram_bytes, 16 << 20);
        assert!(inputs.efuse.is_empty(), "Mosaico identity cannot migrate to Waveshare");
        assert!(inputs.inputs.is_empty(), "GPIO36 is a real RGB data lane, not a devkit flash strap");
        assert_eq!(m.bus.board.named_pin("boot"), Some(61));
        let (width, height, frame, _) = m.bus.board.display().unwrap();
        assert_eq!((width, height, frame.len()), (800, 480, 800 * 480));
        esp32sim_delete(e);
    }
}

#[test]
fn waveshare_c3_public_sd_loader_routes_to_the_physical_board_slot() {
    // SAFETY: each handle and image buffer is owned only by this test.
    unsafe {
        let image = vec![0x5a; 1 << 20];
        for (name, accepted) in [("waveshare-c3-lcd147", true), ("esp32c3", false)] {
            let e = esp32sim_new(name.as_ptr(), name.len(), 4, 0);
            assert!(!e.is_null());
            assert_eq!(crate::storage::load_sd(&mut *e, &image), u32::from(!accepted));
            assert_eq!(crate::storage::esp32sim_sd_len(e), if accepted { image.len() } else { 0 });
            if accepted {
                let MachineKind::C3(m) = &mut (*e).m else { panic!("C3 machine required"); };
                m.reboot();
                assert_eq!(crate::storage::esp32sim_sd_len(e), image.len());
                assert_eq!(*crate::storage::esp32sim_sd_ptr(e).add(512), 0x5a);
            }
            esp32sim_delete(e);
        }
    }
}
