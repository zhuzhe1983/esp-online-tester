//! C5 bounded PMU bias configuration, fixed IDF5.5.3/5.5.4 PMU headers,
//! rtc_clk_init.c and pmu_pvt.c. See c5-boot-bias-2026-11-24 evidence.
//! Awake HP_ACTIVE with PMU selection only. RO codes mirror selected config;
//! they are not measured voltage, PVT calibration, readiness or settling.
const ACTIVE:u32=0x600b_0028;const MODEM:u32=0x600b_005c;const LP_ACTIVE:u32=0x600b_009c;
const TARGET:u32=0xf800_0000;const SELECT:u32=1<<14;const RO_CODES:u32=0x3ff0;
const HP_SLEEP_TARGET:u32=0x07fb_0000;const LP_SLEEP_TARGET:u32=0x07a0_0000;
// Every named field default, including retained sleep/power configuration.
// TRM1.1 reserves several of these fields; pinned IDF/typed guest take priority.
const POR:[u32;3]=[0xc667_f180,0xc667_8000,0xc660_0000];
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct C5BootBias{config:[u32;3],writes:u64}
impl C5BootBias{
    /// Explicit stable external USB3V3 profile; no internal regulator voltage
    /// transfer function or silicon reset capture is implied by these codes.
    pub fn virtual_usb3v3()->Self{Self{config:POR,writes:0}}
    pub fn contains(address:u32)->bool{matches!(address,ACTIVE|MODEM|LP_ACTIVE)}
    pub fn pmu_selected(&self)->bool{self.config[0]&SELECT!=0}
    pub fn active_hp_target(&self)->Option<u8>{self.pmu_selected().then_some((self.config[0]>>27)as u8)}
    pub fn active_lp_target(&self)->Option<u8>{self.pmu_selected().then_some((self.config[2]>>27)as u8)}
    pub fn modem_hp_target(&self)->u8{(self.config[1]>>27)as u8}
    pub fn read(&self,address:u32)->Option<u32>{Some(match address{
        ACTIVE=>(self.config[0]&!RO_CODES)|u32::from(self.active_hp_target()?)<<9|u32::from(self.active_lp_target()?)<<4,
        MODEM=>self.config[1],LP_ACTIVE=>self.config[2],_=>return None,
    })}
    pub fn write(&mut self,address:u32,value:u32)->bool{
        self.write_config(address,value,false)
    }
    /// Called only by the validated-ROM awake profile. The original boot
    /// write API remains strict; these additional fields are future sleep
    /// targets, never a regulator voltage/ready or sleep-entry operation.
    pub fn write_awake_config(&mut self,address:u32,value:u32)->bool{self.write_config(address,value,true)}
    fn write_config(&mut self,address:u32,value:u32,awake:bool)->bool{
        let (index,mut allowed)=match address{ACTIVE=>(0,TARGET|SELECT|RO_CODES),MODEM=>(1,TARGET),LP_ACTIVE=>(2,TARGET),_=>return false};
        if awake{allowed|=if index==2{LP_SLEEP_TARGET}else{HP_SLEEP_TARGET};}
        let Some(current)=self.read(address)else{return false;};
        // PVT handoff and DIG_DBIAS_INIT WT require an actual PVT engine.
        // Other power/sleep/reserved changes cannot silently succeed.
        if (current^value)&!allowed!=0||address==ACTIVE&&(value&SELECT==0||value&8!=0){return false;}
        let latch=allowed&!RO_CODES;
        self.config[index]=(self.config[index]&!latch)|(value&latch);
        self.writes=self.writes.saturating_add(1);true
    }
    /// Declared functional domain contract: CPU/Core preserve the PMU config;
    /// explicit LP or analog reset restores header defaults. Per-register
    /// silicon reset/retention timing is not specified by public bias fields.
    pub fn reset_domains(&mut self,lp:bool,analog:bool){if lp||analog{*self=Self::virtual_usb3v3();}}
    pub fn report(&self)->String{format!("boot_bias=virtual_usb3v3 supply=stable scope=awake_HP_ACTIVE pmu_selected={} hp_target={:?} lp_target={:?} modem_target={} ro_codes=config_mirror voltage=unmodeled settling=unmodeled writes={}",u8::from(self.pmu_selected()),self.active_hp_target(),self.active_lp_target(),self.modem_hp_target(),self.writes)}
}

#[cfg(test)]
mod tests{
    use super::*;
    fn w(b:&mut C5BootBias,address:u32,value:u32){assert!(b.write(address,value),"write {address:#x}={value:#x}");}
    fn target(b:&mut C5BootBias,address:u32,code:u8){let old=b.read(address).unwrap();w(b,address,(old&!TARGET)|(u32::from(code)<<27));}
    fn ro(b:&C5BootBias)->(u8,u8){let v=b.read(ACTIVE).unwrap();(((v>>9)&31)as u8,((v>>4)&31)as u8)}
    #[test]
    fn complete_header_defaults_are_retained_without_host_seeding_guest_targets(){
        let b=C5BootBias::virtual_usb3v3();assert_eq!(b.read(ACTIVE),Some(0xc667_f180));
        assert_eq!(b.read(MODEM),Some(0xc667_8000));assert_eq!(b.read(LP_ACTIVE),Some(0xc660_0000));
        assert_eq!((b.active_hp_target(),b.active_lp_target(),b.modem_hp_target()),(Some(24),Some(24),24));
        assert_eq!(ro(&b),(24,24));assert!(b.report().contains("voltage=unmodeled settling=unmodeled writes=0"));
    }
    #[test]
    fn unchanged_factory_bias_rmw_sequence_sets_actual_targets_and_preserves_power_bits(){
        let mut b=C5BootBias::virtual_usb3v3();let v=b.read(ACTIVE).unwrap();w(&mut b,ACTIVE,v|SELECT); //PC4084c998
        target(&mut b,ACTIVE,28); //PC4084c9aa, get_act_hp_dbias default28
        assert_eq!(ro(&b),(28,24));target(&mut b,MODEM,28); //PC4084c9ba
        assert_eq!(ro(&b),(28,24));target(&mut b,LP_ACTIVE,28); //PC4084c9c6
        assert_eq!(ro(&b),(28,28));assert_eq!(b.modem_hp_target(),28);
        assert_eq!(b.read(ACTIVE),Some(0xe667_f9c0));assert_eq!(b.read(MODEM),Some(0xe667_8000));assert_eq!(b.read(LP_ACTIVE),Some(0xe660_0000));
        assert_eq!(b.read(ACTIVE).unwrap()&!(TARGET|RO_CODES),POR[0]&!(TARGET|RO_CODES));
        assert!(b.report().contains("writes=4"));
    }
    #[test]
    fn independent_nondefault_targets_drive_ro_codes_and_inactive_modem_is_only_configuration(){
        let mut b=C5BootBias::virtual_usb3v3();target(&mut b,ACTIVE,3);target(&mut b,LP_ACTIVE,19);target(&mut b,MODEM,31);
        assert_eq!(ro(&b),(3,19));assert_eq!((b.active_hp_target(),b.active_lp_target(),b.modem_hp_target()),(Some(3),Some(19),31));
        target(&mut b,ACTIVE,0);target(&mut b,LP_ACTIVE,31);assert_eq!(ro(&b),(0,31));
        let read=b.read(ACTIVE).unwrap();w(&mut b,ACTIVE,read^RO_CODES);assert_eq!(b.read(ACTIVE),Some(read));
        for _ in 0..8{assert_eq!(ro(&b),(0,31));}assert!(b.pmu_selected());
    }
    #[test]
    fn pvt_init_power_sleep_and_reserved_requests_reject_before_any_state_changes(){
        let mut b=C5BootBias::virtual_usb3v3();target(&mut b,ACTIVE,28);target(&mut b,LP_ACTIVE,22);
        for address in [ACTIVE,MODEM,LP_ACTIVE]{
            let current=b.read(address).unwrap();
            let forbidden=if address==ACTIVE{(0..=3).chain(15..=26).collect::<Vec<_>>()}else{(0..=26).collect::<Vec<_>>()};
            for bit in forbidden{let before=b.clone();assert!(!b.write(address,current^(1<<bit)),"accepted {address:#x} bit{bit}");assert_eq!(b,before);}
        }
        let v=b.read(ACTIVE).unwrap();for value in [(v&!SELECT),(v|8),(v&!SELECT)|(31<<27),u32::MAX]{let before=b.clone();assert!(!b.write(ACTIVE,value));assert_eq!(b,before);}
    }
    #[test]
    fn reset_contract_preserves_cpu_core_and_restores_only_explicit_lp_analog_domains(){
        let mut b=C5BootBias::virtual_usb3v3();target(&mut b,ACTIVE,28);target(&mut b,LP_ACTIVE,22);let before=b.clone();
        b.reset_domains(false,false);assert_eq!(b,before);b.reset_domains(true,false);assert_eq!(b,C5BootBias::virtual_usb3v3());
        target(&mut b,MODEM,17);b.reset_domains(false,true);assert_eq!(b,C5BootBias::virtual_usb3v3());
    }
    #[test]
    fn only_three_exact_word_registers_exist_and_other_power_clock_engines_do_not_alias(){
        let mut b=C5BootBias::virtual_usb3v3();for address in [0,ACTIVE-4,ACTIVE+1,ACTIVE+4,MODEM+4,LP_ACTIVE+4,0x600b_00ac,0x600b_00cc,u32::MAX]{
            let before=b.clone();assert!(!C5BootBias::contains(address));assert_eq!(b.read(address),None);assert!(!b.write(address,0));assert_eq!(b,before);
        }
        for address in [ACTIVE,MODEM,LP_ACTIVE]{assert!(C5BootBias::contains(address));}
    }
    #[test]
    fn awake_app_sleep_targets_preserve_main_power_selector_and_actual_ro_codes(){
        let mut b=C5BootBias::virtual_usb3v3();target(&mut b,ACTIVE,28);target(&mut b,MODEM,28);target(&mut b,LP_ACTIVE,28);
        for address in [ACTIVE,MODEM,LP_ACTIVE]{let mask=if address==LP_ACTIVE{LP_SLEEP_TARGET}else{HP_SLEEP_TARGET};
            let old=b.read(address).unwrap();assert!(!b.write(address,old&!mask));assert!(b.write_awake_config(address,old&!mask));}
        assert_eq!(b.read(ACTIVE),Some(0xe004_f9c0));assert_eq!(b.read(MODEM),Some(0xe004_8000));assert_eq!(b.read(LP_ACTIVE),Some(0xe040_0000));
        assert_eq!(ro(&b),(28,28));assert!(b.pmu_selected());assert_eq!(b.read(ACTIVE).unwrap()&8,0);
        for(address,bit)in [(ACTIVE,18),(MODEM,18),(LP_ACTIVE,22)]{let value=b.read(address).unwrap();let before=b.clone();assert!(!b.write_awake_config(address,value&!(1<<bit)));assert_eq!(b,before);}
    }
    #[test]
    fn awake_config_is_not_pvt_init_or_a_host_seed_for_unwritten_parameter_fields(){
        let mut b=C5BootBias::virtual_usb3v3();target(&mut b,ACTIVE,3);target(&mut b,LP_ACTIVE,19);
        let value=b.read(ACTIVE).unwrap();for value in [value|8,value&!SELECT,value^(1<<15),value|1]{
            let before=b.clone();assert!(!b.write_awake_config(ACTIVE,value));assert_eq!(b,before);}
        assert!(b.write_awake_config(ACTIVE,value&!HP_SLEEP_TARGET));assert_eq!(ro(&b),(3,19));
        let value=b.read(ACTIVE).unwrap();assert!(b.write_awake_config(ACTIVE,value^RO_CODES));assert_eq!(ro(&b),(3,19));
    }
}
