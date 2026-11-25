//! Component integration: actual MMIO clock inputs, IRQ routing and domains.
//! These fixtures do not claim that a ROM, application or board executed.
use crate::C5Bus;
use emu_core::Bus;
use esp_soc::SocBus;
const WDT:u32=0x600b_1c00;const KEY:u32=0x50d8_3aa1;
fn clocked()->C5Bus{
    let mut b=clocked_tg0();disarm_tg0(&mut b);b
}
fn clocked_tg0()->C5Bus{
    let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;
    // Stop the independent TIMG0 calibration engine. Select the actual DIG
    // route/master and write DCAP128, rather than supplying a fake Hz value.
    for(a,v)in[(0x6000_8068,0x0001_0000),(0x600a_f018,4),(0x600a_f81c,0x00ff_fbff),(0x600a_f804,0x0180_0e6d)]{b.write32(a,v).unwrap();}
    assert_eq!(b.boot_clock.rtc_slow_hz(),Some(136_000));b
}
#[test]fn actual_lp_fast_mux_preserves_inflight_ticks_and_the_independent_slow_clock(){
    let mut b=clocked();b.write32(0x600b_0418,100<<22).unwrap();
    b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<31)).unwrap();b.advance(300);
    b.write32(0x600b_0400,0).unwrap();assert_eq!(b.boot_clock.rtc_fast_hz(),Some(17_500_000));
    assert_eq!(b.boot_clock.rtc_slow_hz(),Some(136_000));assert_eq!(b.lp_wdt.deadline,Some(4003));
    b.advance(3702);assert!(b.lp_wdt.report().contains("software_feeds=0"));b.advance(1);
    assert!(b.lp_wdt.report().contains("lp_fast=17.5MHz"));assert!(b.lp_wdt.report().contains("software_feeds=1"));
    assert_eq!(b.lp_wdt.deadline,Some(4003+204_000_000));
    // HP FOSC bridge gating must not remove the independent LP oscillator.
    b.write32(0x600b_0420,0x7000_0000).unwrap();assert_eq!(b.boot_clock.rc_fast_hz(),None);assert_eq!(b.boot_clock.rtc_fast_hz(),Some(17_500_000));
    b.write32(WDT+0x1c,(300<<20)|(1<<31)).unwrap();assert_eq!(b.lp_wdt.deadline,Some(4003+4115));
    b.write32(0x600b_0400,8).unwrap();assert_eq!(b.boot_clock.rtc_fast_hz(),Some(48_000_000));assert_eq!(b.lp_wdt.deadline,Some(4003+1500));
    assert_eq!(b.boot_clock.rtc_slow_hz(),Some(136_000));
}
#[test]fn actual_lp_source_loss_and_reset_domains_do_not_reseed_analog_age(){
    let mut b=clocked();b.write32(0x600b_0418,100<<22).unwrap();b.write32(0x600b_0400,0).unwrap();
    b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<31)).unwrap();b.advance(1);
    b.write32(0x600b_00ac,0).unwrap();assert_eq!(b.boot_clock.rtc_fast_hz(),None);assert_eq!(b.lp_wdt.deadline,Some(204_000_000));
    b.advance(9999);assert!(b.lp_wdt.report().contains("software_feeds=0"));b.write32(0x600b_00ac,1<<30).unwrap();
    assert_eq!(b.lp_wdt.deadline,Some(14_114));
    b.request_reset(0x0c);assert_eq!(b.reboot([0;6]),0x0c);assert!(b.lp_wdt.report().contains("lp_fast=17.5MHz"));assert_eq!(b.lp_wdt.deadline,Some(14_114));
    b.request_reset(0x10);assert_eq!(b.reboot([0;6]),0x10);assert_eq!(b.boot_clock.rtc_fast_hz(),Some(24_000_000));
    assert!(b.lp_wdt.report().contains("lp_fast=24MHz"));assert_eq!(b.lp_wdt.deadline,Some(204_000_000));
}
#[test]fn pll_gate_mmio_cannot_publish_clocks_past_a_pending_owner_reset(){
    for overdue in[false,true]{
        let mut b=clocked_tg0();b.read32(0x6009_60c0).unwrap();let before=b.transport.as_ref().unwrap().report();
        if overdue{b.cycles=130_000_000;}else{b.request_reset(0x0c);}
        assert!(b.write32(0x6009_6128,0x21).is_err());assert!(b.read32(0x6009_6128).is_err());b.pump_boot_clock();
        assert_eq!(b.boot_clock.pll_div_gate_mask(),Some(0x1ff));assert_eq!(b.transport.as_ref().unwrap().report(),before);
        assert!(b.sw_reset());if overdue{assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.reset_cause(),7);}
    }
}
#[test]fn raw_profile_exit_with_a_pending_reset_still_withdraws_transport_sources(){
    let mut b=clocked_tg0();b.read32(0x6009_60c0).unwrap();b.write32(0x6009_6128,0x21).unwrap();
    // Explicit component observations exercise withdrawal, not analog readiness.
    b.transport.as_mut().unwrap().set_clock_sources(b.cycles,Some(17_500_000),true,0x21);
    assert!(b.transport.as_ref().unwrap().report().contains("XTAL/RC_FAST/PLL"));b.request_reset(0x0c);
    b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(!b.has_validated_rom());assert!(b.sw_reset());
    assert_eq!(b.boot_clock.pll_div_gate_mask(),None);let report=b.transport.as_ref().unwrap().report();
    assert!(report.contains("XTAL-only"));assert!(!report.contains("pll_div_gates="));
}
#[test]fn late_joint_pump_applies_tg0_before_the_later_swd_without_future_lp_state(){
    let mut b=clocked_tg0();b.advance(300_000_000);
    assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));
    assert_eq!(b.read32(WDT+0x1c).unwrap()&1,0);assert!(b.lp_wdt.report().contains("requests=0"));
    assert_eq!(b.lp_wdt.deadline,Some(204_000_000));
    // The preserved LP timeline may deliver its later event after the first
    // reset is applied. Each service/reboot remains a single bounded boundary.
    assert_eq!(b.reboot([0;6]),7);assert!(b.sw_reset());assert_eq!(b.reset_cause(),0x12);
    assert_eq!(b.watchdog_reset_at,Some(228_000_000));assert_eq!(b.reboot([0;6]),0x12);assert!(!b.sw_reset());
}
#[test]fn same_cycle_swd_outranks_tg0_and_system_reset_acknowledges_both(){
    let mut b=clocked_tg0();b.write32(0x6009_605c,0).unwrap();b.advance(98_000_000);
    b.write32(0x6009_605c,1<<22).unwrap();assert_eq!(b.timg_wdt.deadline(),Some(228_000_000));
    b.advance(130_000_000);assert_eq!(b.reset_cause(),0x12);assert_eq!(b.watchdog_reset_at,Some(228_000_000));
    assert_eq!(b.reboot([0;6]),0x12);assert!(!b.sw_reset());assert_eq!(b.timg_wdt.deadline(),None);
}
#[test]fn earlier_rwdt_reset_cannot_commit_a_future_tg0_expiry(){
    let mut b=clocked_tg0();configure(&mut b,1,(1<<31)|(4<<28));b.advance(300_000_000);
    assert_eq!(b.reset_cause(),0x10);assert_eq!(b.watchdog_reset_at,Some(3530));
    assert_eq!(b.read32(WDT+0x1c).unwrap()&1,0);
    assert!(b.timg_wdt.report().contains("count=706 "));assert!(b.timg_wdt.report().contains("expiries=0"));
    let lp=b.lp_wdt.clone();let tg=b.timg_wdt.clone();
    assert!(b.read32(0x6000_8048).is_err());assert!(b.write32(0x6000_8060,0).is_err());assert!(b.write32(WDT+0x14,1<<31).is_err());
    assert_eq!(b.lp_wdt,lp);assert_eq!(b.timg_wdt,tg);assert_eq!(b.reset_cause(),0x10);assert_eq!(b.watchdog_reset_at,Some(3530));
}
#[test]fn same_cycle_tg0_core_reset_acknowledges_the_lower_priority_rwdt_cpu_request(){
    let mut b=clocked_tg0();b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<30)).unwrap();
    // Explicit component input for the unmodeled LP_CLKRST permission, not a
    // claim that guest +0x14 gate writes are supported in the product runtime.
    b.lp_wdt.set_cpu_reset_enabled(true);configure(&mut b,64_600,(1<<31)|(2<<28)|(1<<11));
    b.write32(0x6009_605c,0).unwrap();b.advance(98_000_000);b.write32(0x6009_605c,1<<22).unwrap();
    b.advance(130_000_000);assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(228_000_000));
    assert_eq!(b.reboot([0;6]),7);assert!(!b.sw_reset());assert!(b.lp_wdt.deadline.is_some());
}
#[test]fn cpu_only_reset_releases_lp_dispatch_barrier_and_preserves_tg0_age(){
    let mut b=clocked_tg0();b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<30)).unwrap();
    b.lp_wdt.set_cpu_reset_enabled(true);configure(&mut b,1,(1<<31)|(2<<28)|(1<<11));b.advance(3530);
    assert_eq!(b.reset_cause(),0x0d);assert_eq!(b.lp_wdt.deadline,None);
    assert_eq!(b.reboot([0;6]),0x0d);assert!(!b.sw_reset());assert!(b.lp_wdt.deadline.is_some());
    assert_eq!(b.timg_wdt.deadline(),Some(130_000_000));assert_eq!(b.read32(WDT+0x18),Ok(KEY));
}
#[test]fn profile_reselection_retains_age_and_failed_reactivation_is_transactional(){
    let mut b=clocked_tg0();b.advance(12_345);b.pump_boot_watchdogs();let before=b.timg_wdt.clone();b.select_validated_rom_profile().unwrap();
    assert_eq!(b.timg_wdt,before);assert_eq!(b.timg_wdt.deadline(),Some(130_000_000));
    b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();let lp=b.lp_wdt.clone();let tg=b.timg_wdt.clone();
    assert!(b.select_validated_rom_profile().unwrap_err().contains("fresh reset epoch"));
    assert_eq!(b.lp_wdt,lp);assert_eq!(b.timg_wdt,tg);assert!(!b.has_validated_rom());assert!(!b.boot_clock.virtual_profile_enabled());
    // An actual Chip reset creates a fresh epoch, even after raw-ROM mode.
    b.request_reset(1);b.reboot([0;6]);b.select_validated_rom_profile().unwrap();
    assert_eq!(b.timg_wdt.deadline(),Some(12_345+130_000_000));
}
#[test]fn diagnostic_bringup_stops_at_a_reset_before_retiring_another_instruction(){
    let mut m=crate::Bringup::new();m.bus=clocked_tg0();m.bus.advance(130_000_000);
    let pc=m.cpu.pc;let before=m.cpu.insn_count;let result=m.run(1);
    assert_eq!(result.stop,crate::Stop::Reset(7));assert_eq!(result.steps,0);
    assert_eq!(m.cpu.pc,pc);assert_eq!(m.cpu.insn_count,before);
}
#[test]fn late_cpu_reset_settles_an_already_due_tg0_before_another_guest_instruction(){
    let mut b=clocked_tg0();b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<30)).unwrap();
    b.lp_wdt.set_cpu_reset_enabled(true);configure(&mut b,1,(1<<31)|(2<<28)|(1<<11));
    for off in[8,12,16]{b.write32(WDT+off,1_000_000_000).unwrap();}
    b.advance(300_000_000);assert_eq!(b.reset_cause(),0x0d);assert_eq!(b.watchdog_reset_at,Some(3530));
    assert_eq!(b.reboot([0;6]),0x0d);assert!(b.sw_reset());assert_eq!(b.reset_cause(),7);
    assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.reboot([0;6]),7);assert!(!b.sw_reset());
}
fn disarm_tg0(b:&mut C5Bus){
    b.write32(0x6000_8064,KEY).unwrap();let config=b.read32(0x6000_8048).unwrap()&!(1<<14);
    b.write32(0x6000_8048,config).unwrap();b.write32(0x6000_8048,config|(1<<22)).unwrap();b.write32(0x6000_8064,0).unwrap();
}
fn configure(b:&mut C5Bus,hold:u32,config:u32){
    b.write32(WDT+0x18,KEY).unwrap();
    for(off,v)in[(4,hold),(8,100),(12,100),(16,100),(0,config),(0x14,1<<31)]{b.write32(WDT+off,v).unwrap();}
}
fn next(b:&mut C5Bus){let at=b.lp_wdt.deadline.unwrap();b.advance(at-b.cycles());}
#[test]fn actual_dcap_input_drives_rwdt_irq_and_core_reset_retains_lp_state(){
    let mut b=clocked();b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<30)).unwrap();
    b.write32(0x6001_0048,23).unwrap();b.write32(WDT+0x18,KEY).unwrap();b.write32(WDT+0x2c,1<<31).unwrap();
    configure(&mut b,1,(1<<31)|(1<<28)|(3<<25));next(&mut b);
    assert!(!b.sw_reset());assert_eq!(b.read32(WDT+0x24),Ok(1<<31));assert_eq!(b.irq_sources_of(0,7),vec![18]);
    next(&mut b);assert!(b.sw_reset());assert_eq!(b.reset_cause(),0x09);
    let config=b.read32(WDT).unwrap();assert_eq!(b.lp_wdt.deadline,None,"dispatched reset freezes the controller");
    assert_eq!(b.reboot([0;6]),0x09);assert!(!b.sw_reset());assert!(b.has_validated_rom());
    assert_eq!(b.read32(WDT),Ok(config));assert_eq!(b.read32(WDT+0x18),Ok(KEY));
    assert_eq!(b.read32(WDT+0x24),Ok(1<<31));assert_eq!(b.cycles(),180_000);assert_eq!(b.lp_wdt.deadline,Some(180_000+24_000_000_000u64.div_ceil(136_000)));
}
#[test]fn dispatched_swd_wins_simultaneous_rwdt_until_the_reset_is_applied(){
    let mut b=clocked();
    // Stage0's fuse multiplier2:129200 source ticks ==228M nominal cycles.
    configure(&mut b,64_600,(1<<31)|(4<<28));b.advance(228_000_000);
    assert!(b.sw_reset());assert_eq!(b.reset_cause(),0x12);
    for _ in 0..3{b.pump_boot_clock();b.pump_boot_watchdogs();b.read32(WDT).unwrap();}
    b.advance(100);assert_eq!(b.reset_cause(),0x12,"do not replace the dispatched higher-priority SWD cause");
    assert_eq!(b.reboot([0;6]),0x12);assert!(!b.sw_reset());
    assert_eq!(b.read32(WDT),Ok(0x13214));assert_eq!(b.read32(WDT+0x18),Ok(0));
}
#[test]fn source_withdrawal_and_raw_rom_deselection_cannot_keep_a_watchdog_running(){
    let mut b=clocked();b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<30)).unwrap();
    configure(&mut b,10,(1<<31)|(4<<28));b.advance(10_000);
    b.write32(0x600a_f804,0x0181_0e6d).unwrap();assert_eq!(b.boot_clock.rtc_slow_hz(),None);
    assert_eq!(b.lp_wdt.deadline,None);b.advance(1_000_000);assert!(!b.sw_reset());
    b.write32(0x600a_f804,0x0180_0e6d).unwrap();assert!(b.lp_wdt.deadline.is_some());
    b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(!b.has_validated_rom());
    assert_eq!(b.next_deadline(),None);assert!(b.read32(WDT+0x24).is_err());
    b.advance(1_000_000_000);assert!(!b.sw_reset());
}
#[test]fn leaving_the_validated_profile_invalidates_a_presented_watchdog_irq(){
    let mut b=clocked();b.write32(WDT+0x20,KEY).unwrap();b.write32(WDT+0x1c,(300<<20)|(1<<30)).unwrap();
    b.write32(0x6001_0048,23).unwrap();b.write32(WDT+0x18,KEY).unwrap();b.write32(WDT+0x2c,1<<31).unwrap();
    configure(&mut b,1,(1<<31)|(1<<28));next(&mut b);assert_eq!(b.irq_sources_of(0,7),vec![18]);
    b.refresh_irq(0);assert_ne!(b.clic.input&(1<<23),0);b.irq_dirty=false;b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();
    assert!(b.irq_dirty,"raw ROM loading must request an IRQ refresh without a transport");
    b.refresh_irq(0);assert_eq!(b.clic.input&(1<<23),0);assert!(b.irq_sources_of(0,7).is_empty());assert!(!b.lp_wdt.irq());
}
