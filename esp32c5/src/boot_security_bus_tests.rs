//! Actual MMIO/scheduler/guest checks of the declared virtual initialization.
//! These fixtures do not claim original factory, crypto or physical timing.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const CONF:u32=0x6009_6164;const POWER:u32=0x6009_6168;const STATIC:u32=0x6008_7018;
const STATE:u32=0x6008_7028;const DPA:u32=0x6009_5008;const SEC:u32=0x6009_613c;const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn domain(b:&mut C5Bus){assert_eq!(b.read32(0x600b_0000),Ok(0));assert!(b.security_inputs().crypto_domain_available);}
fn awake()->C5Bus{let mut b=selected();domain(&mut b);b}
fn arm(b:&mut C5Bus){for(a,v)in[(POWER,0),(POWER,2),(CONF,5),(CONF,7),(CONF,1)]{b.write32(a,v).unwrap();}}
fn report(b:&C5Bus)->String{b.boot_security.as_ref().unwrap().report()}
#[test]fn real_mmio_init_waits_for_its_virtual_event_and_dpa_does_not_grant_crypto(){
    let mut b=awake();assert_eq!(b.read32(CONF),Ok(4));assert_eq!(b.read32(STATE),Err(Fault::Unsupported));arm(&mut b);
    assert_eq!(b.boot_security.as_ref().unwrap().deadline(),Some(240));assert_eq!(b.next_deadline(),Some(240));
    b.advance(239);assert_eq!(b.read32(CONF),Ok(1));assert_eq!(b.read32(STATE),Err(Fault::Unsupported));b.advance(1);
    assert_eq!(b.read32(CONF),Ok(5));assert_eq!(b.read32(STATE),Ok(0));assert_eq!(b.read32(STATIC),Ok(0x1e0));
    b.write32(STATIC,0x1e2).unwrap();for value in[4,5]{b.write32(DPA,value).unwrap();}assert_eq!(b.read32(STATIC),Ok(0x1e2));assert_eq!(b.read32(STATE),Ok(0));
    b.write32(CONF,3).unwrap();assert_eq!(b.write32(CONF,1),Err(Fault::Unsupported));assert_eq!(b.boot_security.as_ref().unwrap().deadline(),None);
    assert!(report(&b).contains("completions=1"));assert_eq!(b.read32(STATE),Err(Fault::Unsupported));
}
#[test]fn a_later_pmu_observation_cannot_backdate_the_reset_completion(){
    let mut b=selected();assert!(!b.security_inputs().crypto_domain_available);
    for(a,v)in[(POWER,0),(POWER,2),(CONF,5),(CONF,7)]{b.write32(a,v).unwrap();}
    assert_eq!(b.write32(CONF,1),Err(Fault::Unsupported));assert_eq!(b.boot_security.as_ref().unwrap().deadline(),None);
    b.advance(100);domain(&mut b);assert!(report(&b).contains("completions=0"));
    b.write32(CONF,1).unwrap();assert_eq!(b.boot_security.as_ref().unwrap().deadline(),Some(340));b.advance(239);assert_eq!(b.read32(CONF),Ok(1));
    b.advance(1);assert_eq!(b.read32(STATE),Ok(0));assert!(report(&b).contains("complete_at=Some(340)"));
}
#[test]fn cpu_retains_autonomous_init_hp_clears_it_and_raw_loading_withdraws_it(){
    let mut b=awake();arm(&mut b);b.request_reset(0x0c);let before=b.boot_security.clone();
    assert_eq!(b.write32(CONF,5),Err(Fault::Unsupported));b.advance(300);assert_eq!(b.boot_security,before);
    assert_eq!(b.reboot([0;6]),0x0c);assert_eq!(b.read32(STATE),Ok(0));assert!(report(&b).contains("complete_at=Some(240)"));
    b.request_reset(3);b.reboot([0;6]);assert_eq!(b.read32(CONF),Ok(4));assert_eq!(b.read32(STATE),Err(Fault::Unsupported));assert!(report(&b).contains("completions=0"));
    arm(&mut b);b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(b.boot_security.is_none());assert_eq!(b.read32(CONF),Err(Fault::Unmapped));
}
#[test]fn a_late_joint_service_completes_only_before_the_actual_watchdog_boundary(){
    for delta in[-1i64,0,1]{
        let mut b=awake();let start=(130_000_000i64-240+delta)as u64;b.advance(start);arm(&mut b);b.advance(1000);
        assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));
        assert_eq!(report(&b).contains("completions=1"),delta<0,"virtual reset completion at WDT{delta:+}");
        assert_eq!(b.write8(CONF,5),Err(Fault::Unsupported));b.reboot([0;6]);assert_eq!(b.read32(CONF),Ok(4));assert!(report(&b).contains("completions=0"));
    }
}
#[test]fn an_actual_sec_source_withdrawal_cancels_pending_init_without_hidden_resume(){
    let mut b=awake();arm(&mut b);b.advance(100);b.write32(SEC,2).unwrap();assert_eq!(b.boot_clock.sec_source_hz(),None);
    assert_eq!(b.boot_security.as_ref().unwrap().deadline(),None);assert!(report(&b).contains("cancellations=1"));b.advance(200);
    b.write32(SEC,0).unwrap();assert_eq!(b.read32(CONF),Ok(1));assert_eq!(b.read32(STATE),Err(Fault::Unsupported));
    b.write32(CONF,5).unwrap();assert_eq!(b.boot_security.as_ref().unwrap().deadline(),None);
    b.write32(CONF,3).unwrap();b.write32(CONF,1).unwrap();assert_eq!(b.boot_security.as_ref().unwrap().deadline(),Some(540));b.advance(240);assert_eq!(b.read32(STATE),Ok(0));
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
fn fixture(words:&[u32])->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;domain(&mut m.bus);
    let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;m
}
#[test]fn a_plain_and_priced_guest_really_resets_polls_ready_reads_idle_and_selects_the_efuse_source(){
    // The guest writes PU/gate/assert/release, polls READY, reads STATE and
    // STATIC, then executes its own OR2/write. No host completion shortcut.
    let words=[0x00b2_a023,0x00c2_a023,0x00d3_2023,0x00e3_2023,0x00f3_2023,
        0x0003_2503,0x0045_7513,0xfe05_0ce3,0x0003_a503,0x000e_2583,0x0025_e593,0x00be_2023,0x0000_006f];
    for priced in[false,true]{
        let mut m=fixture(&words);m.cores[0].x[5]=POWER;m.cores[0].x[6]=CONF;m.cores[0].x[7]=STATE;m.cores[0].x[28]=STATIC;
        for(i,v)in[0,2,5,7,1].into_iter().enumerate(){m.cores[0].x[11+i]=v;}if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        m.run(400);assert_eq!(m.cores[0].pc,RAM+48);assert_eq!(m.bus.read32(STATIC),Ok(0x1e2));assert_eq!(m.bus.read32(STATE),Ok(0));
        assert!(report(&m.bus).contains("reset_asserts=1 releases=1 completions=1"));assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
    }
}
#[test]fn unsupported_security_requests_preserve_guest_trap_and_controller_state(){
    for priced in[false,true]{for case in 0..5{
        let(a,v,word)=match case{0=>(STATE,1,0x00b2_a023),1=>(POWER,6,0x00b2_a023),2=>(STATIC,0x1e2,0x00b2_a023),3=>(CONF,5,0x00b2_8023),_=>(STATE,0,0x0002_a503)};
        let mut m=fixture(&[word]);m.cores[0].x[5]=a;m.cores[0].x[11]=v;m.cores[0].x[10]=0x55;m.bus.pump_boot_security();
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;let before=m.bus.boot_security.clone();
        m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));
        assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.boot_security,before);
    }}
    let mut b=awake();assert_eq!(b.read32(STATE-4),Err(Fault::Unmapped));assert_eq!(C5Bus::new().read32(CONF),Err(Fault::Unmapped));
}
