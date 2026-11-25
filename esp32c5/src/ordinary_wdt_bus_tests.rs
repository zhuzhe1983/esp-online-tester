//! Real MMIO, clock deadlines, CLIC delivery and reset arbitration. These
//! diagnostic guests do not claim original FreeRTOS or factory execution.
use crate::{C5Bus,Machine};
use emu_core::Bus;
use esp_soc::SocBus;
const BASE:[u32;2]=[0x6000_8000,0x6000_9000];const KEY:u32=0x50d8_3aa1;
const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn handoff(b:&mut C5Bus){for(off,v)in[(0x64,KEY),(0x48,0x0044_8000),(0x64,0)]{b.write32(BASE[0]+off,v).unwrap();}}
fn prepare(b:&mut C5Bus,g:usize,prescale:u32,holds:[u32;2]){
    // Fixed ROM init/UPDATE order, followed by the caller's two stage writes.
    for(off,v)in[(0x48,0x0004_8000),(0x64,KEY),(0x48,0x0004_8000),
        (0x48,0x0004_8000),(0x48,0x0044_8000),(0x48,0x0004_8000),(0x48,0x0044_8000),
        (0x48,0x0004_8000),(0x48,0x0044_8000),(0x48,0x0004_8000),(0x48,0x0044_8000),
        (0x7c,4),(0x70,4),(0x48,0x001c_8000),(0x48,0x005c_8000),(0x48,0x001f_8000),
        (0x48,0x005f_8000),(0x4c,prescale<<16),(0x48,0x005f_8000),(0x64,0),
        (0x64,KEY),(0x48,0x201f_8000),(0x50,holds[0]),(0x48,0x205f_8000),
        (0x48,0x381f_8000),(0x54,holds[1]),(0x48,0x385f_8000)]{b.write32(BASE[g]+off,v).unwrap();}
}
fn arm(b:&mut C5Bus,g:usize,prescale:u32,holds:[u32;2]){
    prepare(b,g,prescale,holds);for(off,v)in[(0x60,1),(0x48,0xb81f_8000),(0x64,0)]{b.write32(BASE[g]+off,v).unwrap();}
}
#[test]fn real_stage_deadlines_publish_both_sources_and_mark_the_irq_input_dirty(){
    for(g,base)in BASE.into_iter().enumerate(){
        let mut b=selected();handoff(&mut b);prepare(&mut b,g,24_000,[2,3]);assert_eq!(b.timg_wdt.deadline(),None);
        b.write32(base+0x60,1).unwrap();b.write32(base+0x48,0xb81f_8000).unwrap();
        assert_eq!(b.timg_wdt.deadline(),Some(240_000),"direct EN needs no extra UPDATE");
        b.write32(0x6001_0000+(58+2*g as u32)*4,17).unwrap();b.write32(0x2080_0000,3).unwrap();b.write32(0x2080_1044,0x3fc0_0100).unwrap();
        b.irq_dirty=false;b.advance(239_999);assert!(!b.timg_wdt.sources()[g].1);assert!(!b.irq_dirty);
        b.advance(1);assert!(b.irq_dirty);assert!(b.timg_wdt.sources()[g].1);assert_eq!(b.refresh_irq(0).unwrap()&31,1);
        assert_eq!(b.irq_sources_of(0,1),vec![58+2*g]);assert_eq!(b.timg_wdt.deadline(),Some(600_000));
    }
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
fn waiting_guest()->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;handoff(&mut m.bus);arm(&mut m.bus,1,24_000,[2,100]);
    let code=[0x1050_0073u32,0x0000_006f].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();m.bus.load_ram(RAM,&code,code.len()).unwrap();
    // Handler: unlock -> actual FEED -> actual IRQ CLR -> lock -> count -> mret.
    let handler=[0x0073_2023u32,0x00b4_2023,0x00c4_a023,0x0003_2023,0x0015_0513,0x3020_0073].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
    m.bus.load_ram(RAM+0x200,&handler,handler.len()).unwrap();m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;
    m.cores[0].x[6]=BASE[1]+0x64;m.cores[0].x[7]=KEY;m.cores[0].x[8]=BASE[1]+0x60;m.cores[0].x[9]=BASE[1]+0x7c;m.cores[0].x[11]=1;m.cores[0].x[12]=4;
    m.cores[0].mtvec=(RAM+0x200)|3;m.cores[0].mstatus|=8;
    m.bus.write32(0x6001_00f0,17).unwrap();m.bus.write32(0x2080_0000,3).unwrap();m.bus.write32(0x2080_1044,0x3fc0_0100).unwrap();m
}
#[test]fn a_waiting_guest_is_woken_by_the_watchdog_and_its_handler_really_feeds_and_clears(){
    for priced in[false,true]{
        let mut m=waiting_guest();if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        m.run(1);assert!(m.cores[0].waiting);
        // Plain run budgets count skipped idle cycles; modeled run settles
        // to the next real device event after the WFI instruction itself.
        if !priced{m.run(240_000-m.bus.cycles());}assert_eq!(m.bus.cycles(),240_000);
        m.run(1);assert_eq!(m.cores[0].pc,RAM+0x200);assert_eq!(m.cores[0].mcause&0x8000_0fff,0x8000_0011);
        assert_eq!(m.bus.irq_sources_of(0,1),vec![60]);for _ in 0..6{m.run(1);}
        assert_eq!(m.cores[0].pc,RAM+4);assert_eq!(m.cores[0].x[10],1);assert_eq!(m.bus.read32(BASE[1]+0x74),Ok(0));assert!(m.bus.irq_sources_of(0,1).is_empty());
        assert!(m.bus.timg_wdt.report().contains("feeds=2 IRQs=1"));assert!(m.bus.first_fault.is_none());
    }
}
#[test]fn real_tg0_handoff_is_required_and_a_group_reset_cannot_grant_it(){
    let mut b=selected();assert!(b.read32(BASE[0]+0x7c).is_err());b.write32(0x6009_6054,3).unwrap();b.write32(0x6009_6054,1).unwrap();
    for(off,v)in[(0x64,KEY),(0x48,0x381f_8000),(0x4c,1<<16),(0x50,2),(0x54,3),(0x48,0x385f_8000)]{b.write32(BASE[0]+off,v).unwrap();}
    assert!(b.write32(BASE[0]+0x48,0xb81f_8000).is_err());assert!(b.read32(BASE[0]+0x7c).is_err());assert!(!b.sources().contains(&(58,true)));
    let mut b=selected();handoff(&mut b);arm(&mut b,0,24_000,[10_000,20_000]);assert_eq!(b.timg_wdt.deadline(),Some(1_200_000_000));
}
#[test]fn unsupported_watchdog_operations_stop_without_a_fabricated_guest_exception(){
    for priced in[false,true]{for case in 0..4{
        let mut m=waiting_guest();let load=case==3;let code=if load{0x0002_a503u32}else{0x00b2_a023};
        m.bus.load_ram(RAM,&code.to_le_bytes(),4).unwrap();
        let(a,v)=match case{
            0=>(BASE[1]+0x7c,4), // locked shared IRQ: protection semantics unqualified
            1=>{m.bus.write32(BASE[1]+0x64,KEY).unwrap();(BASE[1]+0x70,1)}, // unowned timer bit
            2=>(0x6009_6068,5<<20), // active unsupported functional-clock selector
            _=>{m.bus.write32(0x6009_6060,0x1c).unwrap();(BASE[1]+0x74,0)}, // APB closed
        };
        m.cores[0].x[5]=a;m.cores[0].x[11]=v;m.cores[0].x[10]=0x55;
        m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;let status=m.cores[0].mstatus;let before=m.bus.timg_wdt.clone();
        m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc}if pc==RAM));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));
        assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.timg_wdt,before);
    }}
    // Nearby words remain outside this owner; the unsupported classification
    // does not turn an entire TIMG page into an implemented register block.
    let mut b=selected();handoff(&mut b);assert_eq!(b.read32(BASE[1]+0x6c),Err(emu_core::Fault::Unmapped));
}
#[test]fn functional_gate_preserves_phase_and_apb_gate_does_not_stop_the_counter(){
    let mut b=selected();handoff(&mut b);arm(&mut b,1,1,[2,3]);b.advance(1);b.write32(0x6009_6068,0).unwrap();assert_eq!(b.timg_wdt.deadline(),None);
    b.advance(999);b.write32(0x6009_6068,1<<22).unwrap();assert_eq!(b.timg_wdt.deadline(),Some(1009));b.write32(0x6009_6060,0x1c).unwrap();
    b.advance(9);assert!(b.sources().contains(&(60,true)));assert!(b.read32(BASE[1]+0x74).is_err());assert!(b.write32(BASE[1]+0x60,1).is_err());
    b.write32(0x6009_6060,0x1d).unwrap();b.write32(BASE[1]+0x64,KEY).unwrap();b.write32(BASE[1]+0x7c,4).unwrap();assert!(!b.sources().contains(&(60,true)));
    assert_eq!(b.timg_wdt.deadline(),Some(1024));b.advance(15);assert_eq!(b.reset_cause(),8);assert_eq!(b.watchdog_reset_at,Some(1024));
}
#[test]fn late_joint_catchup_keeps_cause8_and_commits_lp_only_through_the_earlier_core_reset(){
    let mut b=selected();handoff(&mut b);arm(&mut b,1,1,[2,3]);b.advance(300_000_000);
    assert!(b.sw_reset());assert_eq!(b.reset_cause(),8);assert_eq!(b.watchdog_reset_at,Some(25));assert!(b.lp_wdt.report().contains("requests=0"));
    let tg=b.timg_wdt.clone();b.pump_boot_watchdogs();assert_eq!(b.timg_wdt,tg);
    // Core reset retains the LP timeline; its later System reset must still
    // dispatch at its own deadline on this deliberately late host catch-up.
    assert_eq!(b.reboot([0;6]),8);assert!(b.sw_reset());assert_eq!(b.reset_cause(),0x12);assert_eq!(b.watchdog_reset_at,Some(228_000_000));
    assert_eq!(b.reboot([0;6]),0x12);assert!(!b.sw_reset());assert_eq!(b.read32(BASE[1]+0x48).unwrap()&(1<<31),0);assert_eq!(b.timg_wdt.sources(),[(58,false),(60,false)]);
    let mut b=selected();handoff(&mut b);arm(&mut b,0,1,[2,3]);arm(&mut b,1,1,[1,4]);b.advance(300_000_000);
    assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(25));assert!(b.lp_wdt.report().contains("requests=0"));
}
#[test]fn emitted_reset_and_cpu_reset_retention_precede_late_feed_clear_and_clock_writes(){
    let mut b=selected();handoff(&mut b);arm(&mut b,1,1,[2,3]);b.advance(10);let before=b.timg_wdt.clone();
    b.request_reset(0x0c);assert!(b.write32(BASE[1]+0x64,KEY).is_err());assert_eq!(b.timg_wdt,before);b.reboot([0;6]);assert_eq!(b.timg_wdt,before);assert!(b.sources().contains(&(60,true)));
    b.cycles=25;assert!(b.write32(BASE[1]+0x60,1).is_err());assert_eq!(b.reset_cause(),8);assert_eq!(b.watchdog_reset_at,Some(25));let before=b.timg_wdt.clone();
    for(a,v)in[(BASE[1]+0x64,KEY),(BASE[1]+0x7c,4),(BASE[1]+0x48,0x381f_8000),(0x6009_6068,0),(0x6009_6060,3)]{assert!(b.write32(a,v).is_err());assert_eq!(b.timg_wdt,before);}
}
#[test]fn same_cycle_lp_reset_rank_is_preserved_for_the_new_tg1_core_reason(){
    for(action,expected)in[(2,8),(3,9),(4,0x10)]{
        let mut b=selected();handoff(&mut b);arm(&mut b,1,1,[1,705]);
        for(a,v)in[(0x6000_8068,0x0001_0000),(0x600a_f018,4),(0x600a_f81c,0x00ff_fbff),(0x600a_f804,0x0180_0e6d)]{b.write32(a,v).unwrap();}
        b.lp_wdt.set_cpu_reset_enabled(true);let lp=0x600b_1c00;b.write32(lp+0x18,KEY).unwrap();
        for(off,v)in[(4,1),(8,100),(12,100),(16,100),(0,(1<<31)|(action<<28)|(1<<11)),(0x14,1<<31)]{b.write32(lp+off,v).unwrap();}
        b.advance(3530);assert_eq!(b.watchdog_reset_at,Some(3530));assert_eq!(b.reset_cause(),expected);
        b.reboot([0;6]);assert!(!b.sw_reset(),"lower-priority same-time reset must not dispatch twice");
    }
}
