//! ABI setup boundaries only; these tests do not execute factory firmware.
use super::*;
#[path="../../tests/common.rs"]
mod common;

fn sram_image() -> Vec<u8> {
    let mut bytes=vec![0;48];bytes[0]=0xe9;bytes[1]=1;bytes[12]=23;bytes[15]=100;bytes[17]=199;
    bytes[4..8].copy_from_slice(&0x4080_0000u32.to_le_bytes());bytes[24..28].copy_from_slice(&0x4080_0000u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&4u32.to_le_bytes());bytes[32]=0x6f;bytes[47]=0xef^0x6f;bytes
}

#[test]
fn c5_cold_rom_boot_requires_the_bus_profile_and_keeps_sram_startup() {
    // SAFETY: handle and buffers belong exclusively to this test until deletion.
    unsafe {
        let board="waveshare-c5-lcd273";let e=esp32sim_new(board.as_ptr(),board.len(),32,8);assert!(!e.is_null());
        assert_eq!(esp32sim_boot(e,0),1);assert!(!(*e).booted);assert_eq!(esp32sim_run(e,0,0.),9);
        let invalid=b"not an ELF";assert_eq!(esp32sim_load(e,0,invalid.as_ptr(),invalid.len()),1);assert!(!(*e).rom_loaded);
        let bytes=sram_image();assert_eq!(esp32sim_load(e,3,bytes.as_ptr(),bytes.len()),0);assert_eq!(esp32sim_boot(e,1),0);
        let MachineKind::C5(m)=&mut (*e).m else {panic!("C5 machine required");};
        assert_eq!(m.bus.flash.len(),32<<20);assert_eq!(m.cores[0].pc,0x4080_0000);assert!(!m.bus.has_validated_rom());
        // A sticky frontend receipt cannot grant MP ROM compatibility.
        (*e).rom_loaded=true;assert_eq!(esp32sim_boot(e,0),1);
        let MachineKind::C5(m)=&(*e).m else {panic!("C5 machine required");};assert_eq!(m.cores[0].pc,0x4080_0000);
        esp32sim_delete(e);
    }
}

#[test]
#[ignore="needs ESP32SIM_ROM_DIR with the pinned ESP32-C5 revision100 mask ROM ELF"]
fn external_c5_abi_checks_exact_rom_and_requires_fresh_epoch_after_sram_boot() {
    let rom=std::fs::read(esp_soc::rom::require("esp32c5_rev100_rom.elf")).expect("read the pinned ESP32-C5 revision100 mask ROM ELF");
    assert_eq!(rom.len(),472300);
    // SAFETY: handle and ROM/image buffers are exclusively owned throughout.
    unsafe {
        let e=esp32sim_new(b"stamp-c5".as_ptr(),8,4,0);assert!(!e.is_null());
        assert_eq!(esp32sim_load(e,0,rom.as_ptr(),rom.len()),0);assert!((*e).rom_loaded);
        let MachineKind::C5(m)=&(*e).m else {panic!("C5 machine required");};
        assert!(m.bus.has_validated_rom());let before=(m.bus.rom.clone(),m.bus.sram.clone(),m.bus.flash.clone());
        let mut corrupt=rom.clone();corrupt[0x10000]^=1;
        assert_eq!(esp32sim_load(e,0,corrupt.as_ptr(),corrupt.len()),1);
        let MachineKind::C5(m)=&(*e).m else {panic!("C5 machine required");};
        assert_eq!((&m.bus.rom,&m.bus.sram,&m.bus.flash),(&before.0,&before.1,&before.2));assert!(m.bus.has_validated_rom());
        assert_eq!(esp32sim_boot(e,0),0);
        let bytes=sram_image();assert_eq!(esp32sim_load(e,3,bytes.as_ptr(),bytes.len()),0);assert_eq!(esp32sim_boot(e,1),0);
        assert_eq!(esp32sim_boot(e,0),1,"past ROM receipt does not survive a change to SRAM mode");
        assert_eq!(esp32sim_load(e,0,rom.as_ptr(),rom.len()),1,"SRAM startup withdraws the cold profile even without elapsed time");
        assert_eq!(esp32sim_boot(e,0),1);
        let MachineKind::C5(m)=&(*e).m else {panic!("C5 machine required");};
        assert_eq!(m.cores[0].pc,0x4080_0000);assert!(!m.bus.has_validated_rom());assert_eq!(m.insns(),0);assert_eq!(m.bus.cycles,0);
        esp32sim_delete(e);
        let e=esp32sim_new(b"stamp-c5".as_ptr(),8,4,0);assert!(!e.is_null());
        assert_eq!(esp32sim_load(e,0,rom.as_ptr(),rom.len()),0);assert_eq!(esp32sim_boot(e,0),0);
        let MachineKind::C5(m)=&(*e).m else {panic!("C5 machine required");};assert_eq!(m.cores[0].pc,0x4000_0000);assert!(m.web_restart);
        assert_eq!(m.insns(),0,"setup assertions are not firmware execution");assert_eq!(m.bus.cycles,0);esp32sim_delete(e);
    }
}

#[test]
#[ignore="needs ESP32SIM_ROM_DIR and ESP32SIM_C5_FACTORY with the unchanged official LCD273 FactoryOnly BIN"]
fn external_c5_original_lcd273_inputs_validate_through_the_public_abi_without_execution() {
    let rom=std::fs::read(esp_soc::rom::require("esp32c5_rev100_rom.elf")).expect("read the pinned C5 revision100 ROM ELF");
    let path=std::env::var("ESP32SIM_C5_FACTORY").expect("set ESP32SIM_C5_FACTORY to the unchanged official LCD273 FactoryOnly BIN");
    let factory=std::fs::read(path).expect("read ESP32SIM_C5_FACTORY");
    assert_eq!(common::sha256_hex(&factory),"cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942");
    // SAFETY: each input is readable for its call and the handle is uniquely
    // owned here. Loading and selecting a reset PC do not run instructions.
    unsafe {
        let board="waveshare-c5-lcd273";let e=esp32sim_new(board.as_ptr(),board.len(),32,8);assert!(!e.is_null());
        assert_eq!(esp32sim_load(e,0,rom.as_ptr(),rom.len()),0);assert_eq!(esp32sim_load(e,5,factory.as_ptr(),factory.len()),0);
        for at in [0x200c,0x2000f,0x20020,0x8004] {
            let mut corrupt=factory.clone();corrupt[at]^=1;assert_eq!(esp32sim_load(e,5,corrupt.as_ptr(),corrupt.len()),1);
            let MachineKind::C5(m)=&(*e).m else {panic!("C5 machine required");};assert_eq!(&m.bus.flash[..factory.len()],factory.as_slice());
        }
        // Storage restore remains a bounded raw-page operation, not an image load.
        let page=vec![0x5a;4096];assert_eq!(esp32sim_load_at(e,(32<<20)-4096,page.as_ptr(),page.len()),0);
        assert_eq!(esp32sim_load_at(e,32<<20,page.as_ptr(),page.len()),1);
        assert_eq!(esp32sim_boot(e,0),0);let MachineKind::C5(m)=&(*e).m else {panic!("C5 machine required");};
        assert_eq!(m.cores[0].pc,0x4000_0000);assert_eq!(m.insns(),0);assert!(m.bus.has_validated_rom());
        assert_eq!(m.bus.flash[0x2000],0xe9);assert_eq!(m.bus.flash[0x20000],0xe9);assert_eq!(m.bus.flash.last(),Some(&0x5a));esp32sim_delete(e);
    }
}
