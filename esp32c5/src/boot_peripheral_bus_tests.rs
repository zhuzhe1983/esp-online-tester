//! Actual owner gates and reset domains. Component seeds below are negative
//! controls, not a claim that a factory application exercised these engines.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_periph::Device;
use esp_soc::SocBus;
const RMT:u32=0x6000_6000;const PCR:u32=0x6009_6000;
const LP_UART:u32=0x600b_1488;const LP_GPIO:u32=0x600b_47f8;const LP_PO:u32=0x600b_0404;
const APM_CLOCKS:[u32;6]=[0x6009_8ff8,0x600b_34f8,0x6009_97f8,0x6009_a7f8,0x600b_38ec,0x6009_98dc];
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn active_rmt()->esp_periph::rmt_compact::RmtCompact{
    let mut r=esp_periph::rmt_compact::RmtCompact::new(crate::bus::CPU_HZ);
    r.write(0x400,0x0010_8010);r.write(0x404,0);r.write(0x40,1);r.write(0x10,0x0001_0101);r
}
#[test]fn strict_rmt_gate_stops_mmio_ticks_irq_frames_and_scheduler_deadlines(){
    let mut legacy=C5Bus::new();legacy.rmt=active_rmt();assert_eq!(legacy.next_deadline(),Some(1));
    legacy.advance(10_000);assert!(!legacy.rmt.rmt.is_running());assert_eq!(legacy.rmt.rmt.tx_count,1);
    assert!(legacy.read32(RMT+0x38).unwrap()&1!=0);assert!(legacy.sources().contains(&(55,true)));
    let mut b=selected();assert!(!b.rmt_clocked());
    // Deliberately seed an engine behind the disabled gate to ensure no old
    // shared controller path can bypass the selected strict-profile owner.
    b.rmt=active_rmt();b.rmt.rmt.int_raw=1;b.rmt.rmt.done.push((0,vec![true]));
    let before=(b.rmt.rmt.ch[0].rd,b.rmt.rmt.ch[0].acc_cycles,b.rmt.rmt.mem);
    assert!(!b.sources().contains(&(55,true)));assert_ne!(b.next_deadline(),Some(1));
    for a in[RMT,RMT+0x10,RMT+0x38,RMT+0x400,RMT+0x6fc]{
        assert_eq!(b.read32(a),Err(Fault::Unsupported));assert_eq!(b.write32(a,0),Err(Fault::Unsupported));
    }
    assert_eq!(b.read8(RMT+0x39),Err(Fault::Unsupported));assert_eq!(b.write16(RMT+0x402,0),Err(Fault::Unsupported));
    b.advance(10_000);assert_eq!((b.rmt.rmt.ch[0].rd,b.rmt.rmt.ch[0].acc_cycles,b.rmt.rmt.mem),before);
    assert!(b.rmt.rmt.is_running());assert_eq!(b.rmt.rmt.done,vec![(0,vec![true])]);assert_eq!(b.rmt.rmt.tx_count,0);
    assert!(!b.sources().contains(&(55,true)));assert_ne!(b.next_deadline(),Some(1));
}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
#[test]fn disabled_rmt_access_is_a_model_limit_without_fabricated_guest_exception(){
    for priced in[false,true]{for operation in[0x0002_a503u32,0x00b2_a023]{
        let mut m=crate::rom_machine([0;6],4<<20);m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;let pc=crate::bus::SRAM_BASE;
        m.bus.load_ram(pc,&operation.to_le_bytes(),4).unwrap();m.cores[0].pc=pc;m.cores[0].x[5]=RMT+0x400;
        m.cores[0].x[10]=0x55;m.cores[0].x[11]=0xaa;m.cores[0].mepc=0x1234;m.cores[0].mcause=99;m.cores[0].mtval=88;
        let status=m.cores[0].mstatus;m.dbg.stop_on_unimplemented=false;if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc:p}if p==pc));
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval,m.cores[0].mstatus),(0x1234,99,88,status));
        assert_eq!(m.cores[0].x[10],0x55);assert_eq!(m.exceptions,0);assert_eq!(m.bus.rmt.rmt.mem[0],0);
    }}
}
#[test]fn cold_selection_rejects_legacy_activity_before_creating_any_profile(){
    for activity in 0..3{
        let mut b=C5Bus::new();match activity{0=>b.rmt=active_rmt(),1=>{b.rmt.rmt.int_raw=1;b.rmt.rmt.int_ena=1;},_=>b.rmt.rmt.done.push((0,vec![false]))}
        let lp=b.lp_wdt.clone();let tg=b.timg_wdt.clone();
        assert!(b.select_validated_rom_profile().unwrap_err().contains("idle legacy RMT"));
        assert_eq!(b.lp_wdt,lp);assert_eq!(b.timg_wdt,tg);assert!(!b.boot_clock.virtual_profile_enabled());
        assert!(b.boot_peripheral_clock.is_none());assert!(b.boot_apm.is_none());assert!(b.boot_saradc.is_none());assert!(b.rmt_clocked());
    }
}
#[test]fn timer_functional_disable_and_tg1_apb_control_use_their_distinct_real_owners(){
    let mut b=selected();let deadline=b.timg_wdt.deadline();
    b.write32(PCR+0x58,0).unwrap();b.write32(PCR+0x64,0).unwrap();
    assert_eq!(b.read32(PCR+0x5c),Ok(1<<22));assert_eq!(b.read32(PCR+0x68),Ok(1<<22));
    assert_eq!(b.timg_wdt.deadline(),deadline);assert_eq!(b.read32(0x6000_9064),Ok(0x50d8_3aa1));
    b.write32(PCR+0x60,0x1c).unwrap();assert_eq!(b.timg_wdt_inputs().apb_clocked,[true,false]);
    assert!(b.read32(0x6000_9064).is_err());assert!(b.write32(0x6000_9064,0).is_err());
    assert_eq!(b.read32(0x6000_8064),Ok(0x50d8_3aa1));assert_eq!(b.timg_wdt.deadline(),deadline);
    b.write32(PCR+0x60,0x1d).unwrap();assert_eq!(b.read32(0x6000_9064),Ok(0x50d8_3aa1));
    b.write32(PCR+0x60,3).unwrap();assert_eq!(b.timg_wdt_inputs().in_reset,[false,true]);assert!(b.write32(0x6000_9064,0).is_err());
}
#[test]fn timg_apb_close_keeps_the_watchdog_functional_source_and_respects_earlier_reset(){
    let mut b=selected();b.write32(PCR+0x54,0x1c).unwrap();assert!(!b.timg_wdt_inputs().apb_clocked[0]);
    assert!(b.read32(0x6000_8048).is_err());assert_eq!(b.read32(PCR+0x5c),Ok(1<<22));
    assert_eq!(b.timg_wdt.deadline(),Some(130_000_000));b.advance(130_000_000);
    assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));
    for overdue in[false,true]{
        let mut b=selected();if overdue{b.cycles=130_000_000;}else{b.request_reset(0x0c);}
        assert!(b.write32(PCR+0x60,0x1c).is_err());assert_eq!(b.boot_clock.timg1_control(),1);
        assert!(b.sw_reset());if overdue{assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));}
    }
}
#[test]fn peripheral_and_pad_closes_retain_declared_domains_and_cannot_cross_pending_reset(){
    let mut legacy=C5Bus::new();assert!(legacy.read32(PCR+0x3c).is_err());assert!(legacy.boot_peripheral_clock.is_none());
    let mut b=selected();b.write32(PCR+0x16c,4).unwrap();b.write32(PCR+0x12c,0).unwrap();
    b.write32(LP_UART,1<<25).unwrap();b.write32(LP_UART,0).unwrap();b.write32(LP_GPIO,0).unwrap();
    let lp_fast=b.boot_clock.rtc_fast_hz();let slow=b.boot_clock.rtc_slow_hz();let w=b.lp_wdt.clone();let timer=b.lp_timer.clone();
    b.write32(LP_PO,0).unwrap();assert_eq!(b.boot_clock.rtc_fast_hz(),lp_fast);assert_eq!(b.boot_clock.rtc_slow_hz(),slow);
    assert_eq!(b.lp_wdt,w);assert_eq!(b.lp_timer,timer);
    let before=b.boot_peripheral_clock.clone();b.request_reset(0x0c);
    for a in[PCR+0x16c,LP_UART,LP_GPIO]{assert!(b.read32(a).is_err());assert!(b.write32(a,0).is_err());}
    assert_eq!(b.boot_peripheral_clock,before);assert!(b.write32(LP_PO,0).is_err());assert!(b.write32(PCR+0x60,0x1c).is_err());
    b.reboot([0;6]);assert_eq!(b.boot_peripheral_clock,before);assert_eq!(b.read32(LP_PO),Ok(0));
    b.request_reset(3);b.reboot([0;6]);assert_eq!(b.read32(PCR+0x16c),Ok(5));assert_eq!(b.read32(PCR+0x12c),Ok(0x7ff));
    assert_eq!(b.read32(LP_UART),Ok(0));assert_eq!(b.read32(LP_GPIO),Ok(0));assert_eq!(b.read32(LP_PO),Ok(0));
    b.request_reset(0x10);b.reboot([0;6]);assert_eq!(b.read32(LP_UART),Ok(3<<24));assert_eq!(b.read32(LP_GPIO),Ok(1));assert_eq!(b.read32(LP_PO),Ok(0x7ff));
    b.request_reset(0x0c);b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(b.boot_peripheral_clock.is_none());assert!(b.rmt_clocked());
    assert!(b.read32(PCR+0x3c).is_err());
}
#[test]fn actual_lp_io_close_loop_uses_both_latches_and_rng_clock_stops_samples(){
    let mut b=selected();b.write32(0x600b_2824,0xfff).unwrap();b.read32(0x600b_2808).unwrap();
    let sample=b.read32(0x600b_2824).unwrap()>>24;assert_eq!(sample,1);
    let clock=b.read32(0x600b_2800).unwrap()&!(1<<27);b.write32(0x600b_2800,clock).unwrap();b.write32(LP_GPIO,0).unwrap();
    assert_eq!(b.read32(0x600b_2800).unwrap()&(1<<27),0);assert_eq!(b.read32(LP_GPIO).unwrap()&1,0);
    b.write32(0x600b_2800,clock&!(1<<24)).unwrap();let held=b.read32(0x600b_2808).unwrap();
    assert_eq!(b.read32(0x600b_2808),Ok(held));assert_eq!(b.read32(0x600b_2824).unwrap()>>24,sample);
}
#[test]fn hp_reset_retains_both_lp_io_latches_and_the_stopped_rng_sequence(){
    let mut b=selected();let mut control=selected();
    for b in[&mut b,&mut control]{
        b.write32(0x600b_2824,0xfff).unwrap();b.read32(0x600b_2808).unwrap();
        let closed=b.read32(0x600b_2800).unwrap()&!((1<<27)|(1<<24));b.write32(0x600b_2800,closed).unwrap();b.write32(LP_GPIO,0).unwrap();
    }
    let closed=b.read32(0x600b_2800).unwrap();let cfg=b.read32(0x600b_2824).unwrap();let last=b.read32(0x600b_2808).unwrap();
    b.request_reset(3);assert_eq!(b.reboot([0;6]),3);
    assert_eq!(b.read32(0x600b_2800),Ok(closed));assert_eq!(b.read32(LP_GPIO),Ok(0));
    assert_eq!(b.read32(0x600b_2824),Ok(cfg));assert_eq!(b.read32(0x600b_2808),Ok(last));
    assert_eq!(b.read32(0x600b_2808),Ok(last));assert_eq!(b.read32(0x600b_2824).unwrap()>>24,1);
    for b in[&mut b,&mut control]{b.write32(0x600b_2800,closed|(1<<24)).unwrap();}
    assert_eq!(b.read32(0x600b_2808),control.read32(0x600b_2808));assert_eq!(b.read32(0x600b_2824).unwrap()>>24,2);
    b.request_reset(0x10);b.reboot([0;6]);assert_eq!(b.read32(0x600b_2800),Ok(0x7f00_0000));assert_eq!(b.read32(LP_GPIO),Ok(1));
    assert_eq!(b.read32(0x600b_2824),Ok(0xffe));assert_eq!(b.read32(0x600b_2808),Ok(0));
}
#[test]fn strict_lp_peri_access_cannot_mutate_gates_or_generate_rng_samples_after_reset_boundary(){
    for overdue in[false,true]{
        let mut b=selected();b.write32(0x600b_2824,0xfff).unwrap();b.read32(0x600b_2808).unwrap();
        let clock=b.read32(0x600b_2800).unwrap();let cfg=b.read32(0x600b_2824).unwrap();
        if overdue{b.cycles=130_000_000;}else{b.request_reset(0x0c);}
        for off in[0,4,8,0x24,0x28,0x3fc]{assert!(b.read32(0x600b_2800+off).is_err());assert!(b.write32(0x600b_2800+off,0).is_err());}
        // Inspect configuration directly only after exercising the real Bus
        // boundary; no direct DATA read is used to generate another sample.
        assert_eq!(b.lp_peri.read(0),Some(clock));assert_eq!(b.lp_peri.read(0x24),Some(cfg));
        assert!(b.sw_reset());if overdue{assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));}
    }
    let mut legacy=C5Bus::new();legacy.request_reset(0x0c);assert_eq!(legacy.write32(0x600b_2800,0),Ok(()));assert_eq!(legacy.read32(0x600b_2800),Ok(0));
}
fn apm_clock_fixture()->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;let mut code=Vec::new();
    for address in APM_CLOCKS{
        for word in[(address.wrapping_add(0x800)&0xffff_f000)|0x2b7,((address&4095)<<20)|0x0002_8293,0x0002_a023]{code.extend_from_slice(&word.to_le_bytes());}
    }
    code.extend_from_slice(&0x0000_006fu32.to_le_bytes());m.bus.load_ram(crate::bus::SRAM_BASE,&code,code.len()).unwrap();m.cores[0].pc=crate::bus::SRAM_BASE;m
}
#[test]fn six_actual_machine_clock_stores_do_not_disable_apm_filters_or_grant_dma(){
    let mut m=apm_clock_fixture();assert!(!m.bus.dma_apm_supported());assert!(matches!(m.run(18),esp_soc::Stop::MaxInsns));
    assert!(m.bus.first_fault.is_none());let a=m.bus.boot_apm.as_ref().unwrap();
    for address in APM_CLOCKS{assert_eq!(a.read(address,true),Some(0));}
    assert!(!a.all_filters_disabled());assert!(!a.all_masters_tee_selected());assert!(!m.bus.dma_apm_supported());
    let before=m.bus.boot_apm.clone();assert!(m.bus.write32(APM_CLOCKS[0],1).is_err());assert_eq!(m.bus.boot_apm,before);
    m.bus.request_reset(0x0c);m.bus.reboot([0;6]);assert_eq!(m.bus.boot_apm,before);
    m.bus.request_reset(3);m.bus.reboot([0;6]);let a=m.bus.boot_apm.as_ref().unwrap();
    for(address,hp)in APM_CLOCKS.into_iter().zip([true,false,true,true,false,true]){assert_eq!(a.read(address,true),Some(u32::from(hp)));}
    m.bus.request_reset(0x10);m.bus.reboot([0;6]);for address in APM_CLOCKS{assert_eq!(m.bus.boot_apm.as_ref().unwrap().read(address,true),Some(1));}
}
