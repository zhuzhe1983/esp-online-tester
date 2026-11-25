//! Bus integration of actual digital/REGI2C controls and explicit ideal inputs.
//! These component fixtures do not execute a ROM or establish physical ADC entropy.
use crate::C5Bus;
use emu_core::Bus;
use esp_soc::SocBus;
const ADC:u32=0x6000_e000;
fn running()->C5Bus{
    let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;
    for(a,v)in[(0x600a_f018,4),(0x600b_0158,0x0c00_0000),(0x600a_f81c,0x00ff_fdff)]{b.write32(a,v).unwrap();}
    for(reg,byte)in[(7,0),(7,0x40),(8,1),(8,5),(1,8),(0,102),(4,8),(3,102)]{
        b.write32(0x600a_f804,(1<<24)|(byte<<16)|(reg<<8)|0x69).unwrap();
    }
    assert!(b.boot_analog.as_ref().unwrap().sar_reserved_path_ready());
    for value in[3,1,9,1,5]{b.write32(0x6009_6088,value).unwrap();}
    for(a,v)in[(0x6009_608c,1<<22),(ADC,0x4000_87c0),(ADC+0x18,0x007e_7fff),(ADC+4,0x000c_81fe),(ADC+4,0x010c_81fe)]{b.write32(a,v).unwrap();}
    b
}
#[test]fn actual_clock_and_analog_controls_schedule_captures_independently_of_virtual_rng(){
    let mut b=running();assert_eq!(b.next_deadline(),Some(2000));b.advance(1999);
    assert!(b.boot_saradc.as_ref().unwrap().report().contains("triggers=0"));assert_eq!(b.next_deadline(),Some(1));
    b.advance(1);assert!(b.boot_saradc.as_ref().unwrap().report().contains("triggers=1 captures=[1, 0]"));
    assert_eq!(b.read32(0x600b_2824).unwrap()>>24,0,"timer captures do not invent RNG samples");
    b.write32(0x600b_2824,0xfff).unwrap();let adc=b.boot_saradc.clone();b.read32(0x600b_2828).unwrap();
    assert_eq!(b.read32(0x600b_2824).unwrap()>>24,1);assert_eq!(b.boot_saradc,adc,"RNG reads do not become ADC conversions");
    b.advance(2000);assert!(b.boot_saradc.as_ref().unwrap().report().contains("triggers=2 captures=[1, 1]"));
    assert!(b.boot_analog.as_ref().unwrap().report().contains("initial_codes=Some([2150, 2150])"));
    assert!(b.boot_saradc.as_ref().unwrap().report().contains("raw_inputs=[2048, 2048]"));
}
#[test]fn cpu_core_and_chip_domains_retain_or_reinitialize_the_actual_selected_inputs(){
    let mut b=running();b.advance(2000);let adc=b.boot_saradc.clone();
    b.request_reset(0x0c);assert_eq!(b.reboot([0;6]),0x0c);assert_eq!(b.boot_saradc,adc);
    b.request_reset(3);assert_eq!(b.reboot([0;6]),3);assert_eq!(b.read32(0x6009_6088),Ok(1));
    assert_eq!(b.boot_saradc.as_ref().unwrap().deadline(),None);b.write32(0x6009_6088,5).unwrap();
    assert_eq!(b.read32(ADC+4),Ok(0xa1fe));assert!(b.boot_analog.as_ref().unwrap().sar_reserved_path_ready());
    assert!(b.boot_saradc.as_ref().unwrap().report().contains("triggers=0 captures=[0, 0]"));
    b.request_reset(0x10);assert_eq!(b.reboot([0;6]),0x10);assert!(!b.boot_analog.as_ref().unwrap().sar_reserved_path_ready());
    b.request_reset(1);assert_eq!(b.reboot([0;6]),1);assert!(b.has_validated_rom());
    assert!(b.boot_saradc.as_ref().unwrap().report().contains("raw_inputs=[2048, 2048]"));
    assert!(b.boot_analog.as_ref().unwrap().report().contains("initial_codes=None"));
}
#[test]fn unsupported_live_power_withdrawal_rejects_transactionally_and_explicit_stop_permits_it(){
    let mut b=running();b.advance(1000);b.pump_boot_analog();b.pump_boot_saradc();
    let adc=b.boot_saradc.clone();let analog=b.boot_analog.clone();
    assert!(b.write32(0x600b_0158,0x0800_0000).is_err());
    assert_eq!(b.boot_saradc,adc);assert_eq!(b.boot_analog,analog);assert_eq!(b.next_deadline(),Some(1000));
    b.write32(ADC+4,0x000c_81fe).unwrap();b.write32(0x600b_0158,0x0800_0000).unwrap();
    assert!(!b.boot_analog.as_ref().unwrap().sar_reserved_path_ready());
    assert!(b.boot_saradc.as_ref().unwrap().report().contains("clock_fault=0"));
}
#[test]fn raw_rom_deselection_withdraws_capture_but_preserves_readable_timer_configuration(){
    let mut b=running();b.advance(2000);b.load_bytes(crate::bus::ROM_BASE,&[0;4]).unwrap();
    assert!(!b.has_validated_rom());assert_eq!(b.next_deadline(),None);assert_eq!(b.read32(ADC+4),Ok(0x010c_81fe));
    b.advance(100_000);assert!(b.boot_saradc.as_ref().unwrap().report().contains("raw_inputs=None"));
    assert!(!b.boot_analog.as_ref().unwrap().sar_reserved_path_ready());assert!(!b.sw_reset());
}
