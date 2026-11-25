//! Ideal-reference protocol and owner ordering, not physical calibration.
use crate::C5Bus;
use emu_core::Bus;
use esp_soc::SocBus;
const MASTER:u32=0x600a_f804;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn route(b:&mut C5Bus){b.write32(0x600a_f018,4).unwrap();b.write32(0x600a_f81c,0x00ff_feff).unwrap();}
fn start(b:&mut C5Bus){route(b);b.write32(MASTER,0x0100_0061).unwrap();b.write32(MASTER,0x0101_0061).unwrap();}
fn flags(b:&mut C5Bus)->u8{b.write32(MASTER,0x0000_0361).unwrap();(b.read32(MASTER).unwrap()>>16)as u8}
fn report(b:&C5Bus)->String{b.boot_analog.as_ref().unwrap().report()}
#[test]fn real_target_reset_sequence_waits_for_the_explicit_virtual_event(){
    let mut b=selected();route(&mut b);assert!(b.write32(MASTER,0x0101_0061).is_err());
    start(&mut b);assert_eq!(flags(&mut b),0);b.advance(23_999);assert_eq!(flags(&mut b),0);
    b.advance(1);assert_eq!(flags(&mut b),9);assert!(report(&b).contains("attempts=1 completions=1"));
    // Restoring the PLL's actual power latches cannot restart a finished OCODE.
    b.write32(0x600b_00cc,0x7200_0000).unwrap();assert_eq!(flags(&mut b),9);
    assert!(b.write32(MASTER,0x0101_0061).is_err());
}
#[test]fn only_the_committed_cpu_root_can_cancel_and_rearm_the_sequence(){
    let mut b=selected();start(&mut b);b.write32(0x600b_0418,100<<22).unwrap();
    b.write32(0x6009_6110,1<<16).unwrap();b.advance(1_000);
    assert!(b.boot_clock.cpu_uses_xtal());assert!(report(&b).contains("state=Running"));
    b.write32(0x6009_6144,1).unwrap();assert!(!b.boot_clock.cpu_uses_xtal());
    assert!(report(&b).contains("state=Aborted"));b.advance(30_000);assert_eq!(flags(&mut b),0);
    b.write32(0x6009_6110,0).unwrap();b.write32(0x6009_6144,1).unwrap();
    assert!(b.write32(MASTER,0x0101_0061).is_err());start(&mut b);b.advance(24_000);assert_eq!(flags(&mut b),9);
}
#[test]fn declared_cpu_core_lp_retention_and_raw_deselection_are_applied_by_the_owner(){
    let mut b=selected();start(&mut b);b.advance(24_000);assert_eq!(flags(&mut b),9);
    for cause in[0x0c,3]{b.request_reset(cause);assert_eq!(b.reboot([0;6]),cause);route(&mut b);assert_eq!(flags(&mut b),9);}
    b.request_reset(0x10);assert_eq!(b.reboot([0;6]),0x10);route(&mut b);assert_eq!(flags(&mut b),0);
    b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(!b.boot_analog.as_ref().unwrap().virtual_ocode_enabled());
    let analog=b.boot_analog.clone();assert!(b.write32(MASTER,0x0100_0061).is_err());assert_eq!(b.boot_analog,analog);
}
#[test]fn a_pending_owner_reset_cannot_generate_late_done(){
    let mut b=selected();start(&mut b);b.request_reset(0x0c);b.advance(100_000);
    assert!(report(&b).contains("state=Running"));assert!(report(&b).contains("completions=0"));
    let analog=b.boot_analog.clone();assert!(b.write32(MASTER,0x0000_0361).is_err());assert_eq!(b.boot_analog,analog);
    assert_eq!(b.reboot([0;6]),0x0c);assert_eq!(flags(&mut b),9,"CPU-only retains the autonomous target");
}
#[test]fn a_late_service_orders_ocode_against_the_actual_watchdog_reset_time(){
    for (lead,done)in[(12_000,false),(24_000,false),(30_000,true)]{
        let mut b=selected();b.advance(130_000_000-lead);start(&mut b);b.advance(lead+10_000);
        assert!(b.sw_reset());assert_eq!(b.watchdog_reset_at,Some(130_000_000));assert_eq!(b.reset_cause()&31,7);
        assert_eq!(report(&b).contains("state=Done"),done,"lead={lead}");
        assert_eq!(b.reboot([0;6]),7);route(&mut b);assert_eq!(flags(&mut b),if done{9}else{0},"lead={lead}");
    }
}
