//! Actual BOD control/REGI2C wiring under a declared constant ideal supply.
//! No original firmware, physical voltage transition or alarm is claimed.
use crate::C5Bus;
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const BOD:u32=0x600b_2c00;const MASTER:u32=0x600a_f800;const POR:u32=0x0ffc_0100;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
fn threshold(b:&mut C5Bus){
    b.write32(0x600a_f018,4).unwrap();b.write32(MASTER+0x1c,0x00ff_feff).unwrap();
    b.write32(MASTER+4,0x0561).unwrap();assert_eq!(b.read32(MASTER+4).unwrap()>>16,0);
    assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),None);
    b.write32(MASTER+4,0x0107_0561).unwrap();assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),Some(7));
}
fn configured()->C5Bus{
    let mut b=selected();for word in[0x0ffc_0200,0x0ffc_0240,0x0ffc_02c0,0x1ffc_02c0,0x0ffc_02c0,0x4ffc_02c0]{b.write32(BOD,word).unwrap();}
    threshold(&mut b);b.write32(BOD,0x6ffc_02c0).unwrap();b.write32(BOD+0x1c,1<<31).unwrap();b.write32(BOD+0x18,1<<31).unwrap();b
}
#[test]fn unknown_comparator_and_wrong_target_do_not_acquire_threshold_permission(){
    let mut legacy=C5Bus::new();assert!(legacy.read32(BOD).is_err());assert!(legacy.boot_brownout.is_none());
    let mut b=selected();assert_eq!(b.read32(BOD),Ok(POR));let before=b.boot_brownout.clone();
    assert_eq!(b.read32(BOD+0x10),Err(Fault::Unsupported));assert!(b.write32(BOD,POR|(1<<29)).is_err());assert_eq!(b.boot_brownout,before);
    assert!(b.write32(MASTER+4,0x0107_0561).is_err(),"the real master clock/route still apply");
    assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),None);assert_eq!(b.boot_brownout,before);
    b.write32(0x600a_f018,4).unwrap();b.write32(MASTER+0x1c,0x00ff_ff7f).unwrap();
    assert!(b.write32(MASTER+4,0x0107_0561).is_err(),"PLL routing cannot address the BOD target");assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),None);
    threshold(&mut b);b.write32(BOD,POR|(1<<29)).unwrap();assert_eq!(b.read32(BOD+0x10),Ok(0));
    assert!(b.boot_brownout.as_ref().unwrap().report().contains("threshold=Some(7)"));
}
#[test]fn guest_control_sequence_and_bod1_have_distinct_owners_without_synthetic_alarm(){
    let mut b=configured();assert_eq!(b.read32(BOD),Ok(0x6ffc_02c0));assert_eq!(b.read32(BOD+0x18),Ok(1<<31));
    assert_eq!(b.read32(BOD+0x10),Ok(0));assert_eq!(b.read32(BOD+0x14),Ok(0));assert!(b.sources().contains(&(15,false)));
    b.write32(0x6001_003c,17).unwrap();assert!(b.irq_sources_of(0,1).is_empty());
    b.write32(BOD+4,1<<31).unwrap();assert_eq!(b.read32(BOD+4),Ok(1<<31));assert_eq!(b.read32(BOD),Ok(0x6ffc_02c0));
    assert_eq!(b.read32(BOD+0x10),Ok(0));assert!(b.write32(BOD,0xeffc_02c0).is_err(),"unmodeled hardware reset action is not a successful configuration");
    assert_eq!(b.read32(BOD),Ok(0x6ffc_02c0));assert!(!b.sw_reset());
    b.write32(BOD,0x7ffc_02c0).unwrap();assert_eq!(b.read32(BOD),Ok(0x7ffc_02c0));
    b.write32(BOD,0x6ffc_02c0).unwrap();assert_eq!(b.read32(BOD+0x1c),Ok(0));assert_eq!(b.read32(BOD+0x14),Ok(0));
}
#[test]fn lp_analog_threshold_and_digital_bod_controls_follow_separate_reset_domains(){
    let mut b=configured();let before=b.boot_brownout.clone();
    for cause in[0x0c,3]{b.request_reset(cause);b.reboot([0;6]);assert_eq!(b.boot_brownout,before);assert_eq!(b.read32(BOD),Ok(0x6ffc_02c0));assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),Some(7));}
    b.request_reset(0x10);b.reboot([0;6]);assert_eq!(b.read32(BOD),Ok(POR));assert_eq!(b.read32(BOD+0x18),Ok(0));
    assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),Some(7));b.write32(BOD,POR|(1<<29)).unwrap();
    b.request_reset(1);b.reboot([0;6]);assert_eq!(b.read32(BOD),Ok(POR));assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),None);
    assert!(b.write32(BOD,POR|(1<<29)).is_err());
    b.request_reset(0x0c);b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();assert!(b.boot_brownout.is_none());assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),None);assert!(b.read32(BOD).is_err());
}
#[test]fn pending_or_earlier_watchdog_reset_blocks_bod_and_threshold_transactions(){
    for overdue in[false,true]{
        let mut b=configured();let before=b.boot_brownout.clone();let writes=target_writes(&b);
        if overdue{b.cycles=130_000_000;}else{b.request_reset(0x0c);}
        for a in[BOD,BOD+0x10,BOD+0x14,BOD+0x18,BOD+0x1c]{assert!(b.read32(a).is_err());assert!(b.write32(a,0).is_err());}
        assert!(b.write32(MASTER+4,0x0107_0561).is_err());assert_eq!(target_writes(&b),writes);assert_eq!(b.boot_brownout,before);
        assert_eq!(b.boot_analog.as_ref().unwrap().bod_threshold(),Some(7));assert!(b.sw_reset());
        if overdue{assert_eq!(b.reset_cause(),7);assert_eq!(b.watchdog_reset_at,Some(130_000_000));}
    }
}
fn target_writes(b:&C5Bus)->String{b.boot_analog.as_ref().unwrap().report().split_whitespace().find(|s|s.starts_with("regi2c_writes=")).unwrap().to_owned()}
