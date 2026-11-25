//! Source-backed startup IRQ wiring through actual guest instructions.
//! These small fixtures do not claim original factory/FreeRTOS execution.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const RAM:u32=crate::bus::SRAM_BASE;const SW:u32=0x600c_5090;const CACHE:u32=0x600c_8000;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn fixture(words:&[u32])->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;
    let mut bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();bytes.extend_from_slice(&0x0000_006fu32.to_le_bytes());
    m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;m
}
fn map_flash(b:&mut C5Bus){
    b.flash[..4].copy_from_slice(&0x0000_0013u32.to_le_bytes());
    for(a,v)in[(0x6000_203c,0),(0x6000_2384,4),(0x6000_2380,0),(0x6000_237c,0x400)]{b.write32(a,v).unwrap();}
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
#[test]fn actual_software_set_routes_source23_to_clic_and_the_guest_handler_clears_it(){
    for priced in[false,true]{
        let mut m=fixture(&[0x00b2_a023,0x0015_0513,0x0000_006f]); // sw a1,0(t0); addi a0,a0,1
        let handler=[0x0002_a023u32,0x0016_0613,0x3020_0073].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
        m.bus.load_ram(RAM+0x200,&handler,handler.len()).unwrap();m.cores[0].x[5]=SW;m.cores[0].x[11]=1;
        m.cores[0].mtvec=(RAM+0x200)|3;m.cores[0].mstatus|=8;
        m.bus.write32(0x2080_0000,3).unwrap();m.bus.write32(0x2080_1044,0x3fc0_0100).unwrap();m.bus.write32(0x6001_005c,17).unwrap();
        if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        m.run(1);assert_eq!(m.cores[0].pc,RAM+4);assert_eq!(m.bus.read32(SW),Ok(1));assert_eq!(m.bus.irq_sources_of(0,1),vec![23]);
        m.run(1);assert_eq!(m.cores[0].pc,RAM+0x200);assert_eq!(m.cores[0].mcause&0x8000_0fff,0x8000_0011);
        m.run(1);assert_eq!(m.bus.read32(SW),Ok(0));assert!(m.bus.irq_sources_of(0,1).is_empty());
        m.run(1);m.run(1);assert_eq!(m.cores[0].pc,RAM+4);m.run(1);
        assert_eq!((m.cores[0].x[10],m.cores[0].x[12]),(1,1));assert!(m.bus.first_fault.is_none());
    }
}
#[test]fn software_owner_domains_raw_exit_and_reset_barriers_do_not_consume_the_level(){
    let mut legacy=C5Bus::new();assert!(legacy.read32(SW).is_err());assert!(legacy.software_interrupt.is_none());
    let mut b=selected();b.write32(SW,1).unwrap();let before=b.software_interrupt.clone();
    for _ in 0..3{assert_eq!(b.read32(SW),Ok(1));assert!(b.sources().contains(&(23,true)));}assert_eq!(b.software_interrupt,before);
    b.request_reset(0x0c);assert!(b.read32(SW).is_err());assert!(b.write32(SW,0).is_err());assert_eq!(b.software_interrupt,before);
    b.reboot([0;6]);assert_eq!(b.software_interrupt,before);b.request_reset(3);b.reboot([0;6]);assert_eq!(b.read32(SW),Ok(0));
    b.write32(SW,1).unwrap();b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(b.software_interrupt.is_none());assert!(!b.sources().contains(&(23,true)));
    let mut b=selected();b.cycles=130_000_000;assert!(b.write32(SW,1).is_err());assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert!(!b.software_interrupt.as_ref().unwrap().irq());
}
#[test]fn cache_bus_shut_does_not_invent_a_failure_irq_or_architectural_abort(){
    for priced in[false,true]{for fetch in[false,true]{
        let mut m=fixture(&[0x0002_a503]);map_flash(&mut m.bus);m.cores[0].x[5]=0x4200_0000;
        if fetch{m.cores[0].pc=0x4200_0000;}
        m.bus.write32(CACHE+0x3d0,1).unwrap();m.bus.write32(CACHE+0x168,16).unwrap();m.bus.write32(CACHE+4,if fetch{1}else{2}).unwrap();
        m.bus.write32(0x6001_0074,25).unwrap();m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;
        let status=m.cores[0].mstatus;let pc=m.cores[0].pc;m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc:p}if p==pc));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));
        assert_eq!(m.bus.read32(CACHE+0x170),Ok(0));assert_eq!(m.bus.read32(CACHE+0x174),Ok(0));assert!(m.bus.irq_sources_of(0,9).is_empty());assert_eq!(m.exceptions,0);
    }}
}
#[test]fn cache_fail_observer_does_not_turn_host_or_pma_denials_into_cache_events(){
    let mut m=fixture(&[0x0002_a503]);map_flash(&mut m.bus);m.cores[0].x[5]=0x4200_0000;
    m.bus.write32(CACHE+0x3d0,1).unwrap();m.bus.write32(CACHE+0x168,16).unwrap();m.bus.write32(CACHE+4,2).unwrap();
    assert_eq!(m.bus.read32(0x4200_0000),Err(Fault::Prohibited));assert_eq!(m.bus.read32(CACHE+0x170),Ok(0));
    m.cores[0].enable_c5_memory_protection().unwrap();m.cores[0].try_write_csr(0xbc0,0).unwrap();
    m.cores[0].try_write_csr(0xbd0,(0x4200_0000>>2)|0x1ff).unwrap();m.cores[0].try_write_csr(0xbc0,0xc000_0014).unwrap();
    assert!(matches!(m.run(1),esp_soc::Stop::Exceptions(1)));assert_eq!(m.cores[0].mcause,5);assert_eq!(m.bus.read32(CACHE+0x170),Ok(0));assert!(!m.bus.sources().contains(&(29,true)));
}
#[test]fn unknown_reserved_ena_readback_stops_plain_and_priced_without_fabricated_exception(){
    for priced in[false,true]{
        let mut m=fixture(&[0x0002_a503]);m.cores[0].x[5]=CACHE+0x168;m.cores[0].x[10]=0x55;
        m.bus.write32(CACHE+0x3d0,1).unwrap();m.bus.write32(CACHE+0x168,31).unwrap();
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;m.dbg.stop_on_unimplemented=false;
        if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));
        assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.read32(CACHE+0x170),Ok(0));
    }
}
#[test]fn unqualified_cache_failure_is_a_model_limit_without_fabricated_raw_or_guest_fault(){
    for gated in[false,true]{
        let mut m=fixture(&[0x0002_a503]);map_flash(&mut m.bus);m.cores[0].x[5]=0x4200_0000;
        if gated{m.bus.write32(CACHE+0x3d0,1).unwrap();m.bus.write32(0x6009_6104,4).unwrap();}else{m.bus.write32(CACHE+4,2).unwrap();}
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;m.dbg.stop_on_unimplemented=false;
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval),(0x1234,99,88));assert!(!m.bus.sources().contains(&(29,true)));
        if gated{m.bus.write32(0x6009_6104,5).unwrap();}assert_eq!(m.bus.read32(CACHE+0x170),Ok(0));
    }
}
#[test]fn cache_controls_honor_pending_reset_cpu_retention_and_hp_reset(){
    let mut b=selected();b.write32(CACHE+0x3d0,1).unwrap();b.write32(CACHE+0x168,31).unwrap();let before=b.cache.error_report();
    b.request_reset(0x0c);for a in[CACHE+0x3d0,CACHE+0x168,CACHE+0x16c,CACHE+0x170,CACHE+0x174,CACHE+4,0x6009_6104]{assert!(b.read32(a).is_err());assert!(b.write32(a,0).is_err());}
    assert!(b.write32(0x6009_6104,7).is_err());assert_eq!(b.cache.read_pcr(),5);assert_eq!(b.cache.read(4),Some(0));
    assert_eq!(b.cache.error_report(),before);b.reboot([0;6]);assert_eq!(b.cache.error_report(),before);
    assert_eq!(b.read32(CACHE+0x168),Err(Fault::Unsupported));b.request_reset(3);b.reboot([0;6]);assert_eq!(b.read32(CACHE+0x168),Ok(0));assert_eq!(b.read32(CACHE+0x3d0),Ok(0));
    let mut b=selected();b.write32(CACHE+0x3d0,1).unwrap();b.write32(CACHE+0x168,31).unwrap();let before=b.cache.error_report();b.cycles=130_000_000;
    assert!(b.write32(0x6009_6104,7).is_err());assert!(b.write32(CACHE+4,3).is_err());assert_eq!(b.cache.error_report(),before);assert_eq!(b.cache.read(4),Some(0));
    assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));
}
#[test]fn unknown_cache_ena_is_also_a_model_limit_in_default_sram_bringup(){
    let mut m=crate::Bringup::new();let code=[0x00b2_a023u32,0x0002_a503].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
    m.bus.load_ram(RAM,&code,code.len()).unwrap();m.cpu.pc=RAM;m.cpu.x[5]=CACHE+0x168;m.cpu.x[11]=31;m.cpu.x[10]=0x55;
    m.cpu.mepc=0x1234;m.cpu.mcause=99;m.cpu.mtval=88;let status=m.cpu.mstatus;
    assert_eq!(m.run(1).stop,crate::Stop::Budget);assert_eq!(m.run(1).stop,crate::Stop::Trap(riscv_rv32::Trap::ModelLimit(RAM+4)));
    assert_eq!((m.cpu.mepc,m.cpu.mcause,m.cpu.mtval,m.cpu.mstatus),(0x1234,99,88,status));assert_eq!(m.cpu.x[10],0x55);
}
fn map_psram(b:&mut C5Bus){
    b.configure_psram(8<<20).unwrap();for p in[15,17,18,20,21,22]{b.write32(0x6009_0000+4*p,0x800).unwrap();}
    // Existing Quad contract: selectCS1, enterQPI, configure real cache phases
    // and MMU page0. No die state or translated-memory result is injected.
    for(a,v)in[(0x6000_3034,1),(0x6000_3008,0),(0x6000_3018,1<<31),(0x6000_301c,23<<26),(0x6000_3020,(7<<28)|0x35),(0x6000_3000,1<<18),
        (0x6000_2040,(1<<20)|(23<<14)|(5<<6)|0x34),(0x6000_2048,(7<<28)|0xeb),(0x6000_204c,(7<<28)|0x38),
        (0x6000_203c,0),(0x6000_2384,4),(0x6000_2380,0),(0x6000_237c,0x600)]{b.write32(a,v).unwrap();}
    b.write32(0x4200_0000,17).unwrap();assert_eq!(&b.psram[..4],&17u32.to_le_bytes());
}
#[test]fn lr_sc_and_amo_to_real_mapped_psram_respect_the_same_unknown_cache_boundary(){
    for priced in[false,true]{for operation in[0x1002_a52fu32,0x18b2_a52f,0x00b2_a52f]{
        let sc=operation==0x18b2_a52f;let words=[0x1002_a52f,operation];let mut m=fixture(if sc{&words}else{&words[1..]});map_psram(&mut m.bus);
        m.cores[0].x[5]=0x4200_0000;m.cores[0].x[11]=18;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        if sc{assert!(matches!(m.run(1),esp_soc::Stop::MaxInsns));assert_eq!(m.cores[0].reservation,Some(0x4200_0000));}
        m.bus.write32(CACHE+0x3d0,1).unwrap();m.bus.write32(CACHE+0x168,16).unwrap();m.bus.write32(CACHE+4,2).unwrap();
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;m.cores[0].x[10]=0x55;let status=m.cores[0].mstatus;let pc=m.cores[0].pc;let reservation=m.cores[0].reservation;
        m.dbg.stop_on_unimplemented=false;assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc:p}if p==pc));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.cores[0].reservation,reservation);
        assert_eq!(&m.bus.psram[..4],&17u32.to_le_bytes());assert_eq!(m.bus.read32(CACHE+0x170),Ok(0));assert!(!m.bus.sources().contains(&(29,true)));assert_eq!(m.exceptions,0);
    }}
}
