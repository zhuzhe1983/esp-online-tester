//! Actual source observations and owner domains, without a firmware claim.
use crate::C5Bus;
use emu_core::Bus;
use esp_soc::SocBus;
const UPDATE:u32=0x600b_0c10;const LOW:u32=UPDATE+4;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn source(b:&mut C5Bus,value:u32){
    for(a,v)in[(0x600a_f018,4),(0x600a_f81c,0x00ff_fbff),(0x600a_f804,0x0100_0e6d|(value<<16))]{b.write32(a,v).unwrap();}
}
fn snapshot(b:&mut C5Bus)->u64{
    assert_eq!(b.read32(UPDATE),Ok(0));b.write32(UPDATE,1<<27).unwrap();
    u64::from(b.read32(LOW).unwrap())|(u64::from(b.read32(LOW+4).unwrap())<<32)
}
#[test]fn source_is_observed_before_first_timer_access_without_backfilling_unknown_cold_ticks(){
    let mut b=selected();b.advance(100_000);let before=b.lp_timer.clone();
    assert!(b.write32(UPDATE,1<<27).is_err());assert_eq!(b.lp_timer,before);
    source(&mut b,128);b.advance(100_000);assert_eq!(snapshot(&mut b),56);
    b.advance(200_000);assert_eq!(snapshot(&mut b),170);assert_eq!(b.read32(LOW+8),Ok(56));
    source(&mut b,129);let paused=b.lp_timer.clone();b.advance(100_000);
    assert!(b.write32(UPDATE,1<<27).is_err());assert_eq!(b.lp_timer,paused);
    source(&mut b,128);b.advance(100_000);assert_eq!(snapshot(&mut b),226);
}
#[test]fn cpu_core_lp_and_chip_resets_follow_the_actual_timer_domain(){
    let mut b=selected();source(&mut b,128);b.advance(100_000);assert_eq!(snapshot(&mut b),56);let timer=b.lp_timer.clone();
    for cause in[0x0c,3]{b.request_reset(cause);assert_eq!(b.reboot([0;6]),cause);assert_eq!(b.lp_timer,timer);}
    b.request_reset(0x10);assert_eq!(b.reboot([0;6]),0x10);assert_eq!(snapshot(&mut b),0);
    b.advance(100_000);assert_eq!(snapshot(&mut b),56);
    b.request_reset(1);assert_eq!(b.reboot([0;6]),1);let timer=b.lp_timer.clone();
    assert!(b.write32(UPDATE,1<<27).is_err());assert_eq!(b.lp_timer,timer);assert!(b.lp_timer.report().contains("slow_hz=None"));
}
#[test]fn pending_global_reset_cannot_advance_snapshot_past_the_earlier_lp_reset(){
    let mut b=selected();source(&mut b,128);b.advance(300_000_000);
    assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));let timer=b.lp_timer.clone();
    assert!(b.read32(UPDATE).is_err());assert!(b.write32(UPDATE,1<<27).is_err());assert_eq!(b.lp_timer,timer);
    assert_eq!(b.reboot([0;6]),7);assert!(b.sw_reset());assert_eq!(b.watchdog_reset_at,Some(228_000_000));
    assert_eq!(b.reboot([0;6]),0x12);assert!(!b.sw_reset());
    // LP reset occurred at228M, not the late300M service observation.
    assert_eq!(snapshot(&mut b),40_800);
}
#[test]fn raw_mode_withdraws_lp_time_without_altering_captured_history(){
    let mut b=selected();source(&mut b,128);b.advance(100_000);assert_eq!(snapshot(&mut b),56);
    b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(!b.lp_timer.virtual_profile_enabled());let timer=b.lp_timer.clone();
    assert!(b.read32(LOW).is_err());b.advance(100_000_000);assert_eq!(b.lp_timer,timer);assert_eq!(b.next_deadline(),None);
}
