//! Public constructor memory boundary only; factory execution is a separate gate.
use esp32sim_wasm::{esp32sim_new,esp32sim_delete};
#[test]
fn waveshare_classic_touch35_constructor_requires_exact_memory() {
    let name="waveshare-esp32-touch35";
    for (flash,psram,valid)in[(16,2,true),(4,2,false),(16,0,false),(16,4,false),(16,8,false)] {
        // SAFETY: static name bytes are readable; a successful unique allocation
        // is destroyed exactly once below and never used after deletion.
        let emulator=unsafe{esp32sim_new(name.as_ptr(),name.len(),flash,psram)};
        assert_eq!(!emulator.is_null(),valid,"{flash}/{psram} MiB");
        if !emulator.is_null(){
            // SAFETY: the checked pointer is this test's unique live allocation.
            unsafe{esp32sim_delete(emulator)};
        }
    }
}

#[test]
fn waveshare_legacy_and_s2_public_factory_routes_exact_capacities() {
    for (name,flash,psram)in[
        ("waveshare-esp32-devkit-wroom-32e-n4",4,0),
        ("waveshare-esp32-devkit-wroom-32ue-n16",16,0),
        ("waveshare-esp32-devkit-wrover-e-n16r8",16,8),
        ("waveshare-esp32-devkit-wrover-ie-n16r8",16,8),
        ("waveshare-epaper-esp32-driver-v3",4,0),
        ("waveshare-s2-pico",4,8),("waveshare-s2-lcd096",4,8),
    ] {
        for (f,p,valid)in[(flash,psram,true),(flash*2,psram,false),(flash,if psram==0{8}else{0},false)] {
            // SAFETY: name is readable and the uniquely returned object is deleted once.
            let ptr=unsafe{esp32sim_new(name.as_ptr(),name.len(),f,p)};
            assert_eq!(!ptr.is_null(),valid,"{name}: {f}/{p}");
            if !ptr.is_null(){unsafe{esp32sim_delete(ptr)};}
        }
    }
    // These two have no source-established physical Flash density. 4/0 below
    // is an explicit virtual configuration, not a physical-capacity test.
    for name in["waveshare-general-driver-robots","waveshare-servo-driver-esp32"] {
        // SAFETY: name is readable; the live unique object is deleted once.
        let ptr=unsafe{esp32sim_new(name.as_ptr(),name.len(),4,0)};
        assert!(!ptr.is_null(),"{name}");unsafe{esp32sim_delete(ptr)};
    }
}
