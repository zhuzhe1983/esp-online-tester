//! Detached digital-monitor wiring. The published clock observation in these
//! diagnostic fixtures is not original-ROM execution or physical PHY proof.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const USB:u32=0x6000_f000;const RAW:u32=USB+8;const ST:u32=USB+12;const ENA:u32=USB+16;const CLR:u32=USB+20;
const PCR:u32=0x6009_6094;const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn publish_pll(b:&mut C5Bus,ready:bool){let inputs=crate::clock_tree::ClockInputs{pll_ready:ready,..b.clock_inputs()};b.boot_clock.pump(b.cycles,inputs);}
#[test]fn monitor_requires_known_clocks_and_cannot_mutate_the_unimplemented_usb_pcr(){
    assert_eq!(C5Bus::new().read32(RAW),Err(Fault::Unmapped));let mut b=selected();assert!(!b.usb_monitor_clock_qualified());assert_eq!(b.read32(RAW),Err(Fault::Unsupported));
    publish_pll(&mut b,true);assert!(b.usb_monitor_clock_qualified());assert_eq!(b.read32(RAW),Ok(8));b.write32(ENA,2).unwrap();
    for value in[0,1,3,5,7,u32::MAX]{assert_eq!(b.write32(PCR,value),Err(Fault::Unsupported));assert!(b.usb_monitor_clock_qualified());}
    assert_eq!(b.read32(PCR),Err(Fault::Unsupported));publish_pll(&mut b,false);assert_eq!(b.read32(RAW),Err(Fault::Unsupported));assert_eq!(b.write32(ENA,0),Err(Fault::Unsupported));
    publish_pll(&mut b,true);assert_eq!(b.read32(ENA),Ok(2));assert_eq!(b.read32(RAW),Ok(8));
    // The existing clock profile rejects closing PLL48's divider gate. Do
    // not turn that unimplemented mutation into a successful clock loss.
    assert_eq!(b.write32(0x6009_6128,0x1df),Err(Fault::Unsupported));assert_eq!(b.boot_clock.pll_div_gate_mask(),Some(0x1ff));
    publish_pll(&mut b,true);assert!(b.usb_monitor_clock_qualified());assert_eq!(b.read32(RAW),Ok(8));
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
fn fixture(words:&[u32])->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;publish_pll(&mut m.bus,true);
    let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;m
}
#[test]fn actual_guest_raw_read_and_sof_clear_preserve_named_por_without_creating_a_host(){
    for priced in[false,true]{for sealed in[false,true]{
        let mut m=fixture(&[0x0002_a503,0x00b3_2023,0x0002_a603]);m.cores[0].x[5]=RAW;m.cores[0].x[6]=CLR;m.cores[0].x[11]=2;
        if sealed{m.bus.write32(USB,0x45).unwrap();m.bus.write32(USB+4,1).unwrap();}
        if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}for _ in 0..3{m.run(1);}
        assert_eq!(m.cores[0].pc,RAM+12);assert_eq!((m.cores[0].x[10],m.cores[0].x[12]),(8,8));assert_eq!(m.bus.read32(ST),Ok(0));assert_eq!(m.bus.read32(ENA),Ok(0));
        assert_eq!(m.bus.read32(USB+4),Ok(if sealed{0}else{2}),"a sticky historical interrupt does not consume the occupied packet");assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
    }}
}
#[test]fn unqualified_usb_requests_preserve_trap_state_sticky_history_and_unknown_clock_boundaries(){
    for priced in[false,true]{for case in 0..7{
        let(a,v,word)=match case{0=>(RAW,2,0x00b2_a023),1=>(CLR,8,0x00b2_a023),2=>(ENA,8,0x00b2_a023),3=>(CLR,2,0x00b2_8023),
            4=>(PCR,5,0x00b2_a023),5=>(CLR,0,0x0002_a503),_=>(RAW,0,0x0002_a503)};
        let mut m=fixture(&[word]);m.bus.write32(ENA,2).unwrap();if case==6{publish_pll(&mut m.bus,false);}
        m.cores[0].x[5]=a;m.cores[0].x[11]=v;m.cores[0].x[10]=0x55;m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;
        let status=m.cores[0].mstatus;m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);
        if case==6{publish_pll(&mut m.bus,true);}
        assert_eq!(m.bus.read32(ENA),Ok(2));assert_eq!(m.bus.read32(ST),Ok(0));assert_eq!(m.bus.read32(USB+4),Ok(2));
        assert_eq!(m.bus.read32(RAW),Ok(8));m.bus.write32(CLR,2).unwrap();assert_eq!(m.bus.read32(RAW),Ok(8));
    }}
}
#[test]fn usb_cpu_retention_hp_reset_and_partial_write_barrier_preserve_the_detached_contract(){
    let mut b=selected();publish_pll(&mut b,true);b.write32(ENA,2).unwrap();b.request_reset(0x0c);assert_eq!(b.write32(ENA,0),Err(Fault::Unsupported));
    b.reboot([0;6]);assert_eq!(b.read32(ENA),Ok(2));b.request_reset(3);b.reboot([0;6]);publish_pll(&mut b,true);assert_eq!(b.read32(ENA),Ok(0));assert_eq!(b.read32(RAW),Ok(8));
    let mut b=selected();publish_pll(&mut b,true);b.cycles=130_000_000;assert_eq!(b.write8(CLR,2),Err(Fault::Unsupported));
    assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));
}
