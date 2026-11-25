//! C5 native SYSCON requests; no effective WiFi/PHY clock or readiness.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const CLOCK:u32=0x600a_9c14;const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn apply(b:&mut C5Bus,modem:u32,lp:u32){
    for(a,v)in[(0x600b_000c,0x8000_0000),(0x600a_9c0c,modem),(0x600a_f020,lp),(0x600b_00dc,0x8000_0000),(0x600b_00d0,0x1000_0000)]{b.write32(a,v).unwrap();}
}
#[test]fn wifi_apb_request_does_not_supply_its_own_modem_apb_provider(){
    let mut legacy=C5Bus::new();assert_eq!(legacy.read32(CLOCK),Err(Fault::Unmapped));assert!(legacy.modem_wifi_clock.is_none());
    let mut b=selected();assert_eq!(b.read32(CLOCK),Err(Fault::Unsupported));assert!(b.modem_wifi_clock.is_none());
    b.write32(0x600a_f01c,4).unwrap();apply(&mut b,0,0x4400_0000);assert_eq!(b.write32(CLOCK,0x400),Err(Fault::Unsupported));assert!(b.modem_wifi_clock.is_none());
    apply(&mut b,0x4000_0000,0);assert_eq!(b.read32(CLOCK),Ok(0));b.write32(CLOCK,0x400).unwrap();let before=b.modem_wifi_clock.clone();
    apply(&mut b,0,0x4400_0000);assert_eq!(b.read32(CLOCK),Err(Fault::Unsupported));assert_eq!(b.write32(CLOCK,0),Err(Fault::Unsupported));assert_eq!(b.modem_wifi_clock,before);
    apply(&mut b,0x4000_0000,0);assert_eq!(b.read32(CLOCK),Ok(0x400));assert_eq!(b.read32(0x600a_9c04),Ok(0x0020_0002));
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
fn guest(words:&[u32])->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;apply(&mut m.bus,0x4000_0000,0);
    let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;m.cores[0].x[5]=CLOCK;m
}
#[test]fn original_mac_frontier_never_fabricates_data_or_a_guest_access_exception(){
    const MAC:u32=0x600a_d858;
    for priced in[false,true]{for word in[0x0002_a503,0x00b2_a023,0x00b2_8023]{
        let mut m=guest(&[word]);m.cores[0].x[5]=MAC;m.cores[0].x[10]=0x55;m.cores[0].x[11]=0;
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;let before=m.bus.modem_wifi_clock.clone();
        m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));assert_eq!(m.cores[0].pc,RAM);
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.modem_wifi_clock,before);
    }}
    assert_eq!(C5Bus::new().read32(MAC),Err(Fault::Unmapped));assert_eq!(selected().read32(MAC+4),Err(Fault::Unmapped));
}
#[test]fn known_mac_model_boundary_still_services_an_earlier_watchdog(){
    for write in[false,true]{let mut b=selected();apply(&mut b,0x4000_0000,0);b.write32(CLOCK,0x400).unwrap();let before=b.modem_wifi_clock.clone();b.cycles=130_000_000;
        let result=if write{b.write8(0x600a_d858,0)}else{b.read32(0x600a_d858).map(|_|())};assert_eq!(result,Err(Fault::Unsupported));
        assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.modem_wifi_clock,before);
    }
}
#[test]fn real_guest_executes_all_four_native_wifi_gate_rmw_groups(){
    let mut words=Vec::new();for mask in[0x400,4,0x200,0x1fb]{words.extend([0x0002_a583,0x0005_e593|(mask<<20),0x00b2_a023]);}words.push(0x0002_a503);
    for priced in[false,true]{let mut m=guest(&words);if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        for expected in[0x400,0x404,0x604,0x7ff]{for _ in 0..3{m.run(1);}assert_eq!(m.bus.read32(CLOCK),Ok(expected));}
        m.run(1);assert_eq!(m.cores[0].pc,RAM+52);assert_eq!(m.cores[0].x[10],0x7ff);assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
        let a=m.bus.boot_analog.as_ref().unwrap();assert!(!a.pll_ready());assert!(!a.sar_reserved_path_ready());assert_eq!(a.bod_threshold(),None);assert_eq!(a.deadline(),None);
        assert_eq!(m.bus.read32(0x600a_f018),Ok(0));assert_eq!(m.bus.read32(0x600a_9c04),Ok(0x0020_0002));assert!(m.bus.modem_lp_clock.is_none());
        assert!(m.bus.modem_wifi_clock.as_ref().unwrap().report().contains("effective_Hz_RF_MAC_PHY_IRQ=unsupported"));
    }
}
#[test]fn unimplemented_wifi_groups_and_partial_or_unclocked_requests_are_model_limits(){
    for priced in[false,true]{for case in 0..6{
        let(value,word)=match case{0=>(1,0x00b2_a023),1=>(0x800,0x00b2_a023),2=>(0x400000,0x00b2_a023),3=>(0x400,0x00b2_8023),4=>(0x400,0x00b2_9023),_=>(0,0x0002_a503)};
        let mut m=guest(&[word]);assert_eq!(m.bus.read32(CLOCK),Ok(0));if case==5{m.bus.write32(CLOCK,0x400).unwrap();apply(&mut m.bus,0,0x4400_0000);}
        m.cores[0].x[11]=value;m.cores[0].x[10]=0x55;m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;let before=m.bus.modem_wifi_clock.clone();
        m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));assert_eq!(m.cores[0].pc,RAM);
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.modem_wifi_clock,before);
    }}
    assert_eq!(selected().read32(CLOCK+4),Err(Fault::Unmapped));
}
#[test]fn wifi_request_owner_retains_resets_drops_and_obeys_old_watchdogs(){
    let mut b=selected();apply(&mut b,0x4000_0000,0);b.write32(CLOCK,0x7ff).unwrap();let before=b.modem_wifi_clock.clone();
    b.request_reset(0x0c);assert_eq!(b.read32(CLOCK),Err(Fault::Unsupported));assert_eq!(b.write32(CLOCK,0),Err(Fault::Unsupported));assert_eq!(b.modem_wifi_clock,before);
    b.reboot([0;6]);assert_eq!(b.read32(CLOCK),Ok(0x7ff));b.request_reset(3);b.reboot([0;6]);apply(&mut b,0x4000_0000,0);assert_eq!(b.read32(CLOCK),Ok(0));
    b.write32(CLOCK,0x400).unwrap();b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(b.modem_wifi_clock.is_none());assert_eq!(b.read32(CLOCK),Err(Fault::Unmapped));
    for access in 0..4{let mut b=selected();apply(&mut b,0x4000_0000,0);b.write32(CLOCK,0x400).unwrap();let before=b.modem_wifi_clock.clone();b.cycles=130_000_000;
        let result=match access{0=>b.read32(CLOCK).map(|_|()),1=>b.write8(CLOCK,0),2=>b.write16(CLOCK,0),_=>b.write32(CLOCK,0)};assert_eq!(result,Err(Fault::Unsupported));
        assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.modem_wifi_clock,before);
    }
}
