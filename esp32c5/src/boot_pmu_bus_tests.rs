//! Config routing/retention fixtures, not application or physical power proof.
use crate::C5Bus;
use emu_core::Bus;
use esp_soc::SocBus;
const BASE:u32=0x600b_0000;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
#[test]fn awake_pmu_requires_the_validated_profile_and_rejects_pending_reset(){
    let mut b=C5Bus::new();
    for addr in[BASE,BASE+0x68,BASE+0x130]{assert!(b.read32(addr).is_err());assert!(b.write32(addr,0).is_err());}
    assert!(b.boot_pmu.is_none());
    b=selected();b.write32(BASE+0x68,0x0020_0000).unwrap();let pmu=b.boot_pmu.clone();
    b.request_reset(0x0c);assert!(b.read32(BASE+0x68).is_err());assert!(b.write32(BASE+0x68,0).is_err());assert_eq!(b.boot_pmu,pmu);
    b.reboot([0;6]);b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();
    assert!(b.read32(BASE+0x68).is_err());assert!(b.write32(BASE+0x68,0).is_err());assert_eq!(b.boot_pmu,pmu);
}
#[test]fn validated_awake_bias_entry_has_one_owner_and_raw_mode_keeps_the_strict_writer(){
    let mut b=selected();
    for addr in[BASE+0x28,BASE+0x5c,BASE+0x9c]{let v=b.read32(addr).unwrap();b.write32(addr,(v&!0xf800_0000)|(28<<27)).unwrap();}
    for(addr,mask)in[(BASE+0x28,0x07fb_0000),(BASE+0x5c,0x07fb_0000),(BASE+0x9c,0x07a0_0000)]{
        let v=b.read32(addr).unwrap();b.write32(addr,v&!mask).unwrap();
    }
    assert_eq!(b.read32(BASE+0x28),Ok(0xe004_f9c0));assert_eq!(b.read32(BASE+0x5c),Ok(0xe004_8000));assert_eq!(b.read32(BASE+0x9c),Ok(0xe040_0000));
    assert!(b.boot_pmu.is_none(),"shared bias addresses must not allocate a second owner");
    let v=b.read32(BASE+0x28).unwrap();let bias=b.boot_bias.clone();assert!(b.write32(BASE+0x28,v|8).is_err());assert_eq!(b.boot_bias,bias);
    b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();let bias=b.boot_bias.clone();
    assert!(b.write32(BASE+0x28,v^(1<<16)).is_err());assert_eq!(b.boot_bias,bias);
}
#[test]fn safe_awake_targets_and_request_latches_do_not_create_clock_ready_or_sleep(){
    let mut b=selected();b.write32(BASE,0).unwrap();b.write32(BASE+0x14,0x7000_0000).unwrap();
    b.write32(BASE+0x18,0x0200_0000).unwrap();b.write32(BASE+0x24,0x0800_0000).unwrap();
    b.write32(BASE+0x68,0x0020_0000).unwrap();b.write32(BASE+0x130,0x20000).unwrap();
    b.write32(BASE+0xac,0x5000_0000).unwrap();assert_eq!(b.read32(BASE+0xac),Ok(0x5000_0000));
    assert!(!b.clock_inputs().pll_ready);assert_eq!(b.boot_clock.cpu_hz(),Some(48_000_000));assert_eq!(b.boot_clock.rtc_slow_hz(),None);
    let pmu=b.boot_pmu.clone();assert!(b.write32(BASE,0x0020_0000).is_err());assert!(b.write32(BASE+4,0).is_err());
    assert!(b.write32(BASE+0x124,1).is_err(),"unimplemented sleep entry must not become target config");assert_eq!(b.boot_pmu,pmu);
    assert!(b.boot_pmu.as_ref().unwrap().report().contains("future_MODEM_SLEEP=unexecuted"));
}
#[test]fn pmu_config_is_retained_by_cpu_core_and_reset_by_lp_domains(){
    let mut b=selected();b.write32(BASE+0x68,0x0020_0000).unwrap();b.write32(BASE+0x130,0x20000).unwrap();let pmu=b.boot_pmu.clone();
    for cause in[0x0c,3]{b.request_reset(cause);assert_eq!(b.reboot([0;6]),cause);assert_eq!(b.boot_pmu,pmu);}
    b.request_reset(0x10);assert_eq!(b.reboot([0;6]),0x10);assert_eq!(b.read32(BASE+0x68),Ok(0));assert_eq!(b.read32(BASE+0x130),Ok(0));
    assert!(b.boot_pmu.as_ref().unwrap().report().contains("writes=0"));
}
