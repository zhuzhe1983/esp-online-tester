//! Actual owner/APB boundaries; no radio or vendor firmware execution claim.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const WIFI:u32=0x600a_f00c;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn apply(b:&mut C5Bus,lp:u32){
    for(a,v)in[(0x600b_000c,0x8000_0000),(0x600a_9c0c,0),(0x600a_f020,lp),(0x600b_00dc,0x8000_0000),(0x600b_00d0,0x1000_0000)]{b.write32(a,v).unwrap();}
}
#[test]fn master_force_and_i2c_permission_do_not_grant_lpc_clock_apb_access(){
    let mut legacy=C5Bus::new();assert!(legacy.read32(WIFI).is_err());assert!(legacy.modem_lp_clock.is_none());
    let mut b=selected();b.write32(0x600a_f01c,4).unwrap();assert!(b.read32(WIFI).is_err());assert!(b.modem_lp_clock.is_none());
    apply(&mut b,0x0400_0000);assert!(b.write32(WIFI,0).is_err());assert!(b.modem_lp_clock.is_none());
    apply(&mut b,0x4000_0000);assert_eq!(b.read32(WIFI),Ok(0));assert!(b.write32(WIFI,1).is_ok());
    let before=b.modem_lp_clock.clone();apply(&mut b,0x0400_0000);assert!(b.read32(WIFI).is_err());assert!(b.write32(WIFI,0).is_err());assert_eq!(b.modem_lp_clock,before);
}
#[test]fn modem_selection_observes_raw_rc_instead_of_hp_bridge_or_lp_mux(){
    let mut b=selected();apply(&mut b,0x4000_0000);b.write32(0x600b_0418,100<<22).unwrap();
    b.write32(0x600b_0400,8).unwrap();b.write32(0x600b_0420,0x7000_0000).unwrap();b.write32(WIFI,2).unwrap();
    assert_eq!(b.boot_clock.rtc_fast_hz(),Some(48_000_000));assert_eq!(b.boot_clock.rc_fast_hz(),None);
    assert_eq!(b.modem_lp_clock.as_ref().unwrap().selected_source_hz(WIFI,b.modem_clock_inputs()),Some(17_500_000));
    b.write32(0x600b_00ac,0).unwrap();assert_eq!(b.modem_lp_clock.as_ref().unwrap().selected_source_hz(WIFI,b.modem_clock_inputs()),None);
    assert_eq!(b.read32(WIFI),Ok(2));
}
#[test]fn pending_reset_retention_and_raw_exit_preserve_the_selector_owner_contract(){
    let mut b=selected();apply(&mut b,0x4000_0000);b.write32(WIFI,2).unwrap();let before=b.modem_lp_clock.clone();
    b.request_reset(0x0c);assert!(b.read32(WIFI).is_err());assert!(b.write32(WIFI,0).is_err());assert_eq!(b.modem_lp_clock,before);
    assert_eq!(b.reboot([0;6]),0x0c);assert_eq!(b.modem_lp_clock,before);assert_eq!(b.read32(WIFI),Ok(2));
    b.request_reset(3);assert_eq!(b.reboot([0;6]),3);apply(&mut b,0x4000_0000);assert_eq!(b.read32(WIFI),Ok(0));
    b.write32(WIFI,4).unwrap();b.request_reset(0x0c);b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();
    assert!(!b.has_validated_rom());assert!(b.modem_lp_clock.is_none());assert!(b.read32(WIFI).is_err());
}
const COEX:u32=0x600a_f008;const RAM:u32=crate::bus::SRAM_BASE;
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
fn guest(words:&[u32],apb:bool)->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;
    if apb{apply(&mut m.bus,0x4000_0000);}
    let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();
    m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;m.cores[0].x[5]=COEX;m
}
#[test]fn real_guest_divider_rmw_preserves_xtal_source_and_exact_pre_gate_ratio(){
    for priced in[false,true]{
        // Read source4, retain its selector, set NUM49, write the actual314.
        let mut m=guest(&[0x0002_a583,0x00f5_f593,0x3105_e593,0x00b2_a023],true);m.bus.write32(COEX,4).unwrap();
        if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}for _ in 0..4{m.run(1);}
        assert_eq!(m.cores[0].pc,RAM+16);assert_eq!(m.bus.read32(COEX),Ok(0x314));let c=m.bus.modem_lp_clock.as_ref().unwrap();let inputs=m.bus.modem_clock_inputs();
        assert_eq!(c.selected_source_hz(COEX,inputs),Some(48_000_000));assert_eq!(c.selected_rate_ratio(COEX,inputs),Some((48_000_000,50)));assert_eq!(c.selected_clock_hz(COEX,inputs),Some(960_000));
        assert!(c.report(inputs).contains("functional_gate_RF_MAC_IRQ_sleep=unsupported"));assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
    }
}
#[test]fn unqualified_lpc_requests_are_model_limits_without_a_fabricated_guest_fault(){
    for priced in[false,true]{for case in 0..6{
        let(a,v,word)=match case{0=>(COEX,0x313,0x00b2_a023),1=>(COEX,0x318,0x00b2_a023),2=>(COEX,0x10004,0x00b2_a023),
            3=>(WIFI,0x314,0x00b2_a023),4=>(COEX,4,0x00b2_8023),_=>(COEX,0,0x0002_a503)};
        let mut m=guest(&[word],case!=5);if case!=5{assert_eq!(m.bus.read32(COEX),Ok(0));}
        m.cores[0].x[5]=a;m.cores[0].x[11]=v;m.cores[0].x[10]=0x55;m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;
        let status=m.cores[0].mstatus;let before=m.bus.modem_lp_clock.clone();m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));
        assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.modem_lp_clock,before);
    }}
    assert_eq!(C5Bus::new().read32(COEX),Err(Fault::Unmapped));let mut b=selected();assert_eq!(b.read32(COEX+8),Err(Fault::Unmapped));
}
#[test]fn coex_divider_retention_and_partial_write_watchdog_barrier_use_real_owners(){
    let mut b=selected();apply(&mut b,0x4000_0000);b.write32(COEX,0x314).unwrap();b.request_reset(0x0c);let before=b.modem_lp_clock.clone();
    assert_eq!(b.write32(COEX,0),Err(Fault::Unsupported));assert_eq!(b.modem_lp_clock,before);b.reboot([0;6]);assert_eq!(b.read32(COEX),Ok(0x314));
    b.request_reset(3);b.reboot([0;6]);apply(&mut b,0x4000_0000);assert_eq!(b.read32(COEX),Ok(0));
    let mut b=selected();apply(&mut b,0x4000_0000);b.write32(COEX,0x314).unwrap();let before=b.modem_lp_clock.clone();b.cycles=130_000_000;
    assert_eq!(b.write8(COEX,4),Err(Fault::Unsupported));assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.modem_lp_clock,before);
}
