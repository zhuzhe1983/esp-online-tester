//! C5 MODEM_SYSCON CLK_CONF1 WiFi requests, not a WiFi/PHY clock engine.
//! Fixed IDF5.5.3/5.5.4 C5 register/LL and original WIFI47 dependency path.
//! Contract/source limits: docs/evidence/c5-wifi-clock-requests-2026-11-25.
const ADDRESS:u32=0x600a_9c14;
const WIFI_APB:u32=0x400;const BB44:u32=4;const MAC:u32=0x200;const BB_MISC:u32=0x1fb;
const SUPPORTED:u32=WIFI_APB|BB44|MAC|BB_MISC;
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct ModemWifiClock{requests:u16,writes:u64}
impl ModemWifiClock{
    pub fn new()->Self{Self{requests:0,writes:0}}
    pub fn contains(address:u32)->bool{address==ADDRESS}
    /// The caller supplies the applied MODEM_APB permission. WIFI_APB's
    /// separate request bit10 never grants access to this configuration bus.
    pub fn read(&self,address:u32,modem_apb_accessible:bool)->Option<u32>{
        (modem_apb_accessible&&Self::contains(address)).then_some(u32::from(self.requests))
    }
    pub fn write(&mut self,address:u32,value:u32,modem_apb_accessible:bool)->bool{
        if !modem_apb_accessible||!Self::contains(address)||value&!SUPPORTED!=0||!matches!(value&BB_MISC,0|BB_MISC){return false;}
        // Only the witnessed four groups. Individual partial BB_MISC groups
        // are not qualified, despite their named single-bit hardware fields.
        // FE/BT bits11..21 and reserved22..31 remain unsupported.
        self.requests=value as u16;self.writes=self.writes.saturating_add(1);true
    }
    /// Stored requests only, with no effective Hz, ready, APB grant or event.
    pub fn requested_bits(&self)->u16{self.requests}
    /// Same declared functional HP domain as the existing SYSCON controls.
    /// CPU/LP retain; HP/Chip clear. Not per-flop silicon retention evidence.
    pub fn reset_domains(&mut self,hp:bool,analog:bool){if hp||analog{*self=Self::new();}}
    pub fn report(&self)->String{let r=u32::from(self.requested_bits());format!("WIFI_CLK=requests_only word={r:#05x} WIFI_APB_request={} BB44M_request={} MAC_request={} BB_MISC_request={} writes={} effective_Hz_RF_MAC_PHY_IRQ=unsupported",
        u8::from(r&WIFI_APB!=0),u8::from(r&BB44!=0),u8::from(r&MAC!=0),u8::from(r&BB_MISC!=0),self.writes)}
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_address_and_header_por_require_the_actual_modem_apb_input(){
        let mut c=ModemWifiClock::new();assert!(ModemWifiClock::contains(ADDRESS));assert_eq!(c.requested_bits(),0);
        assert_eq!(c.read(ADDRESS,true),Some(0));let before=c.clone();assert_eq!(c.read(ADDRESS,false),None);assert!(!c.write(ADDRESS,0,false));assert_eq!(c,before);
        for a in[0,ADDRESS-1,ADDRESS+1,0x600a_9c04,0x600a_9c08,0x600a_9c0c,0x600a_9c10,0x600a_9c18,0x600a_f018,u32::MAX]{
            assert!(!ModemWifiClock::contains(a));assert_eq!(c.read(a,true),None);assert!(!c.write(a,0,true));assert_eq!(c,before);
        }
    }
    #[test]
    fn original_four_rmw_requests_and_their_real_clear_masks_are_independent(){
        let mut c=ModemWifiClock::new();for(mask,expected)in[(WIFI_APB,0x400),(BB44,0x404),(MAC,0x604),(BB_MISC,0x7ff)]{
            let old=c.read(ADDRESS,true).unwrap();assert!(c.write(ADDRESS,old|mask,true));assert_eq!(c.read(ADDRESS,true),Some(expected));
        }
        for(mask,expected)in[(BB_MISC,0x604),(MAC,0x404),(BB44,0x400),(WIFI_APB,0)]{
            let old=c.read(ADDRESS,true).unwrap();assert!(c.write(ADDRESS,old&!mask,true));assert_eq!(c.read(ADDRESS,true),Some(expected));
        }
        assert_eq!(c.writes,8);assert!(c.report().contains("effective_Hz_RF_MAC_PHY_IRQ=unsupported"));
    }
    #[test]
    fn all_sixteen_supported_group_combinations_read_back_without_other_bits(){
        let mut c=ModemWifiClock::new();let groups=[WIFI_APB,BB44,MAC,BB_MISC];
        for choices in 0..16{let mut word=0;for(i,mask)in groups.into_iter().enumerate(){if choices&(1<<i)!=0{word|=mask;}}
            assert!(c.write(ADDRESS,word,true));assert_eq!(u32::from(c.requested_bits()),word);assert_eq!(c.read(ADDRESS,true),Some(word));
        }
        assert_eq!(c.writes,16);assert!(c.report().contains("WIFI_APB_request=1 BB44M_request=1 MAC_request=1 BB_MISC_request=1"));
    }
    #[test]
    fn partial_bb_named_fe_bt_and_reserved_requests_reject_before_mutation(){
        let mut c=ModemWifiClock::new();assert!(c.write(ADDRESS,SUPPORTED,true));
        for v in[1,2,8,16,32,64,128,256,0x1fa,0xfb,0x17b,0x401,0x800,0x1000,0x8000,0x10000,0x20000,0x40000,0x80000,0x100000,0x200000,0x400000,0x80000000,u32::MAX]{
            let before=c.clone();assert!(!c.write(ADDRESS,v,true),"{v:#x}");assert_eq!(c,before);
        }
    }
    #[test]
    fn withdrawing_bus_permission_never_erases_or_turns_requests_into_a_grant(){
        let mut c=ModemWifiClock::new();assert!(c.write(ADDRESS,SUPPORTED,true));let before=c.clone();
        for _ in 0..8{assert_eq!(c.read(ADDRESS,false),None);assert!(!c.write(ADDRESS,0,false));assert_eq!(c.requested_bits(),0x7ff);c.report();}assert_eq!(c,before);
        assert_eq!(c.read(ADDRESS,true),Some(SUPPORTED));
    }
    #[test]
    fn cpu_lp_retain_and_hp_chip_reset_the_declared_configuration_domain(){
        let mut c=ModemWifiClock::new();assert!(c.write(ADDRESS,SUPPORTED,true));let before=c.clone();
        c.reset_domains(false,false);assert_eq!(c,before);c.reset_domains(true,false);assert_eq!(c,ModemWifiClock::new());
        assert!(c.write(ADDRESS,WIFI_APB|BB44,true));c.reset_domains(false,true);assert_eq!(c,ModemWifiClock::new());
    }
    #[test]
    fn bounded_diagnostics_saturate_and_observations_do_not_advance_any_engine(){
        let mut c=ModemWifiClock::new();c.writes=u64::MAX;assert!(c.write(ADDRESS,SUPPORTED,true));assert_eq!(c.writes,u64::MAX);let before=c.clone();
        for _ in 0..64{c.read(ADDRESS,true);c.requested_bits();c.report();}assert_eq!(c,before);
    }
}
