use emu_core::Bus;
use esp_soc::SocBus;
const MON:u32=0x600c_2000;
const RAM:u32=esp32c5::bus::SRAM_BASE;
fn machine(words:&[u32])->esp32c5::Machine{
    let mut m=esp32c5::rom_machine([0;6],4<<20);m.quantum=1;
    let bytes:Vec<_>=words.iter().flat_map(|w|w.to_le_bytes()).collect();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();
    m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;
    m.bus.write32(0x6009_6100,1).unwrap();m.bus.write32(MON+0x44,3).unwrap();m
}
#[test]
fn typed_cpu_context_captures_actual_pc_sp_and_data_irq_without_host_or_fetch_events(){
    let mut m=machine(&[0x0005_a503,0x0000_0013]); // lw a0,0(a1); nop
    m.cores[0].x[11]=RAM+0x100;m.bus.write32(RAM+0x100,0x1234_5678).unwrap();
    for(a,v)in[(MON+0x10,RAM+0x100),(MON+0x14,RAM+0x103),(MON,1),(MON+8,1),(0x6001_006c,23)]{m.bus.write32(a,v).unwrap();}
    m.run(1);assert_eq!(m.cores[0].x[10],0x1234_5678);assert_eq!(m.cores[0].pc,RAM+4);
    assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM);assert_eq!(m.bus.read32(MON+0x4c).unwrap(),RAM+0x50000);
    assert_eq!(m.bus.read32(MON+0x30).unwrap(),RAM);assert_eq!(m.bus.read32(MON+0x34).unwrap(),RAM+0x50000);
    assert_eq!(m.bus.irq_sources_of(0,7),vec![27]);assert_eq!(m.bus.read32(MON+4).unwrap(),1);
    m.bus.read32(RAM+0x100).unwrap();m.bus.write32(RAM+0x100,0).unwrap();
    assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM,"host activity has no CPU context");
    // Monitor the next instruction's own bytes. Fetch is not a data transfer.
    m.bus.write32(MON+0x10,RAM+4).unwrap();m.bus.write32(MON+0x14,RAM+7).unwrap();
    m.run(1);assert_eq!(m.cores[0].pc,RAM+8);assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM+4);
    assert_eq!(m.bus.read32(MON+4).unwrap(),0);assert!(m.bus.irq_sources_of(0,7).is_empty());
}
#[test]
fn exact_exception_pc_and_reset_capture_retention_follow_actual_execution(){
    let mut m=machine(&[0x0000_0013,0x0000_0073]);m.cores[0].mtvec=RAM+0x200;
    m.run(1);m.run(1);assert_eq!(m.cores[0].mepc,RAM+4);
    assert_eq!(m.bus.read32(MON+0x70).unwrap(),RAM+4);assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM+4);
    m.bus.request_reset(3);assert_eq!(m.reboot(),3);m.bus.write32(0x6009_6100,1).unwrap();
    assert_eq!(m.bus.read32(MON+0x44).unwrap(),0);assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM+4);
    assert_eq!(m.bus.read32(MON+0x4c).unwrap(),RAM+0x50000);assert_eq!(m.bus.read32(MON+0x70).unwrap(),0);
    // The observer still points at the bus's retained module after reboot.
    m.bus.write32(MON+0x44,3).unwrap();m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x40000;m.run(1);
    assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM);assert_eq!(m.bus.read32(MON+0x4c).unwrap(),RAM+0x40000);
    m.bus.request_reset(1);m.reboot();m.bus.write32(0x6009_6100,1).unwrap();assert_eq!(m.bus.read32(MON+0x48).unwrap(),0);
}
#[test]
fn early_trap_stop_drops_cpu_context_before_host_inspection(){
    let mut m=machine(&[0xffff_ffff]);m.dbg.stop_on_unimplemented=true;
    m.run(1);let pc=m.bus.read32(MON+0x48).unwrap();
    for(a,v)in[(MON+0x10,RAM+0x100),(MON+0x14,RAM+0x103),(MON,3),(MON+8,3)]{m.bus.write32(a,v).unwrap();}
    m.bus.write32(RAM+0x100,42).unwrap();m.bus.read32(RAM+0x100).unwrap();
    assert_eq!(m.bus.read32(MON+0x48).unwrap(),pc);assert_eq!(m.bus.read32(MON+4).unwrap(),0);
}
#[test]
fn malformed_rom_load_is_transactional_and_does_not_install_a_monitor(){
    let mut m=esp32c5::machine([0;6],4<<20);m.bus.sram.fill(0xa5);m.bus.rom.fill(0x5a);
    assert!(m.load_rom(b"not a ROM").unwrap_err().contains("C5 ROM needs a RISC-V ELF32"));
    assert!(m.bus.sram.iter().all(|&b|b==0xa5));assert!(m.bus.rom.iter().all(|&b|b==0x5a));
    assert!(!m.has_observer("c5-hardware-monitor"));
}
#[test]
fn writing_unvalidated_rom_bytes_does_not_enable_opaque_spi0_access_after_reset(){
    let mut m=esp32c5::machine([0;6],4<<20);m.bus.load_bytes(0x4000_0000,&[0x13,0,0,0]).unwrap();
    m.bus.request_reset(3);m.reboot();assert!(m.bus.read32(0x6000_2024).is_err());
}

#[test]
fn cpu_only_reset_retains_real_hp_peripherals_matrix_and_monitor_state(){
    for cause in[0x0b,0x0c,0x0d,0x11,0x18,0x1a]{
        let mut m=machine(&[0x0005_a503]);m.cores[0].x[11]=RAM+0x100;
        m.bus.write32(RAM+0x100,0x1234_5678).unwrap();
        for(a,v)in[(MON+0x10,RAM+0x100),(MON+0x14,RAM+0x103),(MON,1),(MON+8,1),(0x6001_006c,23)]{m.bus.write32(a,v).unwrap();}
        m.bus.uart_input(0,b"Q");m.bus.write32(0x600b_1000,0x3141_5926).unwrap();m.run(1);
        m.bus.request_reset(cause);assert_eq!(m.reboot(),cause);
        assert_eq!(m.bus.read32(0x6009_6100).unwrap(),1,"CPU reset retains peripheral clock control");
        assert_eq!(m.bus.read32(MON+0x44).unwrap(),0);assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM);
        assert_eq!(m.bus.read32(MON+0x4c).unwrap(),RAM+0x50000);assert_eq!(m.bus.read32(MON+4).unwrap(),0,"live CPU conditions reset; sampled IRQ remains until another CPU sample");
        assert_eq!(m.bus.irq_sources_of(0,7),vec![27],"matrix routes and peripheral IRQ survive core-local reset");
        assert_eq!(m.bus.read32(0x6000_0000).unwrap(),u32::from(b'Q'),"RX FIFO is an HP peripheral");
        assert_eq!(m.bus.read32(0x600b_1000).unwrap(),0x3141_5926);assert!(m.bus.first_fault.is_none());
    }
}

#[test]
fn every_documented_hp_core_reset_retains_pc_sp_but_resets_monitor_controls(){
    for cause in[3,5,7,8,9,0x14,0x15,0x16]{
        let mut m=machine(&[0x0000_0013]);m.run(1);m.bus.request_reset(cause);m.reboot();
        assert_eq!(m.bus.read32(0x6009_6100).unwrap(),0);m.bus.write32(0x6009_6100,1).unwrap();
        assert_eq!(m.bus.read32(MON+0x44).unwrap(),0);assert_eq!(m.bus.read32(MON+0x48).unwrap(),RAM);
        assert_eq!(m.bus.read32(MON+0x4c).unwrap(),RAM+0x50000);
    }
}

#[test]
fn later_breakpoint_observer_cannot_turn_host_inspection_into_cpu_bus_traffic(){
    for until_cycle in[false,true]{
        let mut m=machine(&[0x0000_0013]);
        for(a,v)in[(MON+0x10,RAM+0x100),(MON+0x14,RAM+0x103),(MON,3),(MON+8,3),(0x6001_006c,23)]{m.bus.write32(a,v).unwrap();}
        m.add_observer(Box::new(esp_soc::observers::Breakpoints{pcs:vec![RAM]}));
        if until_cycle{assert!(matches!(m.run_until_cycle(1),esp_soc::RunUntil::Stop(esp_soc::Stop::Breakpoint(RAM))));}
        else{assert!(matches!(m.run(1),esp_soc::Stop::Breakpoint(RAM)));}
        assert_eq!(m.insns(),0);m.bus.write32(RAM+0x100,42).unwrap();m.bus.read32(RAM+0x100).unwrap();
        assert!(m.bus.irq_sources_of(0,7).is_empty());assert_eq!(m.bus.read32(MON+0x30).unwrap(),0);
        assert_eq!(m.bus.read32(MON+0x34).unwrap(),0);assert_eq!(m.bus.read32(MON+4).unwrap(),0);
    }
}
