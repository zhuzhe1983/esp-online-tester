use emu_core::Bus;
use esp_soc::SocBus;

#[test]
fn por_pad_release_is_a_write_trigger_and_unsupported_holds_do_not_change_gpio() {
    let mut b=esp32c5::C5Bus::new();
    b.attach_board(esp32c5::board::make_board("stamp-c5").unwrap());
    for (a,v) in [(0x6009_0070,1<<12),(0x6009_1b44,256),(0x6009_1038,1<<28)]{b.write32(a,v).unwrap();}
    let before=b.board.leds().unwrap().0.to_vec();
    for value in [0,1<<29,1<<31,0xa000_0000]{
        b.write32(0x600b_00e4,value).unwrap();assert_eq!(b.read32(0x600b_00e4).unwrap(),0);
        assert_eq!(b.board.leds().unwrap().0,before);
    }
    for value in [1<<28,1<<30,0x3000_0000,0xc000_0000,1]{
        assert!(b.write32(0x600b_00e4,value).is_err());
        assert_eq!(b.board.leds().unwrap().0,before);
    }
}

#[test]
fn crystal_query_reports_the_explicit_board_clock_and_preserves_por_fields() {
    let mut b=esp32c5::C5Bus::new();b.set_xtal_mhz(48).unwrap();
    let v=b.read32(0x6009_6110).unwrap();
    assert_eq!((v>>24)&127,48);assert_eq!(v&255,0);assert_eq!((v>>8)&255,2);
    assert_eq!((v>>16)&3,0);assert_ne!(v&(1<<31),0);
    b.write32(0x6009_6118,3).unwrap();assert_eq!(b.read32(0x6009_6118).unwrap(),3);
    b.write32(0x6009_6110,!(3<<16)).unwrap();assert_eq!(b.read32(0x6009_6110).unwrap(),v);
    assert!(b.write32(0x6009_6110,2<<16).is_err());assert_eq!(b.read32(0x6009_6110).unwrap(),v);
}

#[test]
fn uart_pcr_reset_gates_real_fifo_access_without_erasing_transmitted_history() {
    let mut b=esp32c5::C5Bus::new();
    b.write32(0x6000_0000,u32::from(b'A')).unwrap();b.uart_input(0,b"old");
    b.write32(0x6009_6000,3).unwrap();assert_eq!(b.read32(0x6009_6000).unwrap(),3);
    assert!(b.write32(0x6000_0000,u32::from(b'X')).is_err());
    b.uart_input(0,b"held");b.write32(0x6009_6000,1).unwrap();
    assert_eq!(b.read32(0x6009_6000).unwrap(),5);assert_eq!(b.read32(0x6000_0000).unwrap(),0);
    b.write32(0x6000_0000,u32::from(b'B')).unwrap();assert_eq!(b.uart[0].tx_out,b"AB");
    b.write32(0x6009_6004,0).unwrap();b.uart_input(0,b"off");
    assert!(b.write32(0x6000_0000,u32::from(b'X')).is_err());
    b.write32(0x6009_6004,1<<22).unwrap();assert_eq!(b.read32(0x6000_0000).unwrap(),0);
    b.uart_input(0,b"C");assert_eq!(b.read32(0x6000_0000).unwrap(),u32::from(b'C'));
    b.write32(0x6009_6000,0).unwrap();assert!(b.read32(0x6000_0020).is_err());
    b.write32(0x6000_1000,u32::from(b'1')).unwrap();assert_eq!(b.uart[1].tx_out,b"1");
    assert!(b.write32(0x6009_6004,(2<<20)|(1<<22)).is_err());assert_eq!(b.read32(0x6009_6004).unwrap(),1<<22);
    assert!(b.write32(0x6009_6008,4).is_err());assert_eq!(b.read32(0x6009_6008).unwrap(),2);
}

#[test]
fn c5_efuse_reload_drives_its_actual_interrupt_source_and_clear() {
    let mut b=esp32c5::C5Bus::new();b.write32(0x6001_0038,20).unwrap();
    b.write32(0x600b_49e4,1).unwrap();assert!(b.irq_sources_of(0,4).is_empty());
    b.write32(0x600b_49cc,0x5aa5).unwrap();b.write32(0x600b_49d8,1).unwrap();
    assert_eq!(b.read32(0x600b_484c).unwrap(),0x10);assert_eq!(b.irq_sources_of(0,4),vec![14]);
    b.write32(0x600b_49e8,1).unwrap();assert!(b.irq_sources_of(0,4).is_empty());
}

#[test]
fn reset_strap_profile_uses_documented_boot_pullups_and_stays_latched() {
    let mut b=esp32c5::C5Bus::new();assert_eq!(b.read32(0x6009_1000).unwrap(),0x18);
    b.gpio_set_input(28,false);assert_eq!(b.read32(0x6009_1000).unwrap(),0x18);
    b.write32(0x6009_1000,0).unwrap();assert_eq!(b.read32(0x6009_1000).unwrap(),0x18);
    b.set_strap(u32::MAX);assert_eq!(b.read32(0x6009_1000).unwrap(),0x7f);
    b.set_strap(8);assert_eq!(b.read32(0x6009_1000).unwrap(),8);
}

#[test]
fn lp_aon_scratch_is_real_storage_retained_across_core_reset() {
    let mut b=esp32c5::C5Bus::new();
    for i in 0..10{assert_eq!(b.read32(0x600b_1000+i*4).unwrap(),0);b.write32(0x600b_1000+i*4,0x1234_0000+i).unwrap();}
    b.reboot([0;6]);
    for i in 0..10{assert_eq!(b.read32(0x600b_1000+i*4).unwrap(),0x1234_0000+i);}
    let mut fresh=esp32c5::C5Bus::new();assert_eq!(fresh.read32(0x600b_1010).unwrap(),0);
}

#[test]
fn systimer_deadline_reaches_the_c5_matrix_only_after_real_elapsed_counter_ticks() {
    let mut b=esp32c5::C5Bus::new();b.write32(0x6001_00f4,23).unwrap();
    b.write32(0x6000_a01c,0).unwrap();b.write32(0x6000_a020,32).unwrap();b.write32(0x6000_a050,1).unwrap();
    b.write32(0x6000_a000,0x4700_0002).unwrap();b.write32(0x6000_a064,1).unwrap();
    assert_eq!(b.next_deadline(),Some(480));assert!(b.irq_sources_of(0,7).is_empty());
    b.advance(479);assert!(b.irq_sources_of(0,7).is_empty());assert_eq!(b.next_deadline(),Some(1));
    b.advance(1);assert_eq!(b.irq_sources_of(0,7),vec![61]);assert!(b.report().contains("events=[1, 0, 0]"));
    b.write32(0x6000_a06c,1).unwrap();assert!(b.irq_sources_of(0,7).is_empty());assert_eq!(b.next_deadline(),None);
    b.advance(1500);assert!(b.irq_sources_of(0,7).is_empty());
}

#[test]
fn cache_pcr_controls_mapped_cpu_access_without_resetting_flash_mmu_or_internal_ram(){
    let mut b=esp32c5::C5Bus::new();b.flash=vec![255;65536];b.flash[..4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
    for(a,v)in[(0x6000_203c,0),(0x6000_2384,4),(0x6000_2380,0),(0x6000_237c,0x400)]{b.write32(a,v).unwrap();}
    assert_eq!(b.read32(0x4200_0000).unwrap(),0x1234_5678);assert_eq!(b.read32(0x6009_6104).unwrap(),5);
    b.write32(0x600c_8004,3).unwrap();assert!(b.write32(0x6009_6104,3).is_err());
    assert_eq!(b.read32(0x6009_6104).unwrap(),5);assert_eq!(b.read32(0x600c_8004).unwrap(),3);
    b.write32(0x6009_6104,7).unwrap();assert!(b.read32(0x600c_8004).is_err());assert!(b.fetch(0x4200_0000).is_err());
    assert_eq!(b.read32(0x6000_237c).unwrap(),0x400,"CACHE reset does not invent an MSPI MMU reset coupling");
    b.write32(0x6009_6104,5).unwrap();assert_eq!(b.read32(0x600c_8004).unwrap(),0);
    assert_eq!(b.read32(0x4200_0000).unwrap(),0x1234_5678);
    b.write32(0x6009_6104,4).unwrap();assert!(b.read32(0x4200_0000).is_err());assert!(b.read32(0x600c_8010).is_err());
    b.write32(0x4080_0000,0x0000_0013).unwrap();assert_eq!(b.fetch(0x4080_0000).unwrap(),[0x13,0,0,0]);
    b.write32(0x6009_6104,5).unwrap();assert_eq!(b.read32(0x4200_0000).unwrap(),0x1234_5678);
}

#[test]
fn lp_clock_selection_exposes_por_mux_without_inventing_calibration_or_sources(){
    let mut b=esp32c5::C5Bus::new();assert_eq!(b.read32(0x600b_0400).unwrap(),4);
    b.write32(0x600b_0400,4).unwrap();
    for value in[0,1,5,12,0x14,u32::MAX]{assert!(b.write32(0x600b_0400,value).is_err());assert_eq!(b.read32(0x600b_0400).unwrap(),4);}
    assert_eq!(b.next_deadline(),None);assert!(!b.sw_reset());
}

#[test]
fn unavailable_pmu_and_brownout_interrupts_can_only_stay_disabled(){
    let mut b=esp32c5::C5Bus::new();
    for a in[0x600b_0168,0x600b_2c28]{assert_eq!(b.read32(a).unwrap(),0);b.write32(a,0).unwrap();
        for value in[1,1<<27,1<<30,1<<31,u32::MAX]{assert!(b.write32(a,value).is_err());assert_eq!(b.read32(a).unwrap(),0);}}
    b.write32(0x600b_2c2c,1<<31).unwrap();assert_eq!(b.read32(0x600b_2c2c).unwrap(),0);
    assert!(b.write32(0x600b_2c2c,1).is_err());assert!(!b.sw_reset());assert_eq!(b.next_deadline(),None);
}
