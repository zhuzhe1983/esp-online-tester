//! Exact C5 MODEM_LPCON LP clock selectors; no RF/MAC/timer execution.
//! IDF5.5.3/5.5.4 modem_lpcon_reg/ll.h, modem_clock_hal.c/modem_clock.c.
//! +4 is BLE RTC timer selection, distinct from LP_TIMER at600b0c00.
//! COEX also retains its named12-bit DIV_NUM; BLE/WIFI keep divider0.
//! Raw source, exact divided ratio and integral pre-gate rate are separate.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub(crate) struct ClockInputs{pub rc_slow_hz:Option<u32>,pub rc_fast_hz:Option<u32>,pub xtal_hz:Option<u32>}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct ModemLpClock{words:[u32;3],writes:[u64;3]}
impl ModemLpClock{
    pub fn new()->Self{Self{words:[0;3],writes:[0;3]}}
    fn index(address:u32)->Option<usize>{match address{0x600a_f004=>Some(0),0x600a_f008=>Some(1),0x600a_f00c=>Some(2),_=>None}}
    pub fn contains(address:u32)->bool{Self::index(address).is_some()}
    pub fn read(&self,address:u32,lp_apb_accessible:bool)->Option<u32>{
        if !lp_apb_accessible{return None;}Some(self.words[Self::index(address)?])
    }
    pub fn write(&mut self,address:u32,value:u32,lp_apb_accessible:bool)->bool{
        if !lp_apb_accessible{return false;}let Some(i)=Self::index(address)else{return false;};
        // IDF5.5.3/5.5.4 MODEM_LPCON: selectors[3:0], DIV_NUM[15:4].
        // The original COEX setter stores(divisor-1);420b8798 adds1 on
        // conversion. Only COEX nonzero dividers are qualified here.
        let mask=if i==1{0xffff}else{0xf};if value&!mask!=0||!matches!(value&15,0|1|2|4){return false;}
        self.words[i]=value;self.writes[i]=self.writes[i].saturating_add(1);true
    }
    /// Pre-functional-gate selected raw source only. APB accessibility does
    /// not clock the target, and a selector never proves RF/domain readiness.
    /// No RC32K/XTAL32K/external32K or multi-source support. A programmed
    /// COEX divider does not change this raw-source observation's meaning.
    pub fn selected_source_hz(&self,address:u32,inputs:ClockInputs)->Option<u32>{
        match self.words[Self::index(address)?]&15{
            1=>inputs.rc_slow_hz.filter(|&hz|hz==136_000),2=>inputs.rc_fast_hz.filter(|&hz|hz==17_500_000),
            4=>inputs.xtal_hz.filter(|&hz|hz==48_000_000),_=>None,
        }
    }
    /// Exact sourceHz/(DIV_NUM+1), before the unmodeled functional gate.
    /// This is configuration observation, not a running COEX timer/RF clock.
    pub fn selected_rate_ratio(&self,address:u32,inputs:ClockInputs)->Option<(u32,u16)>{
        let i=Self::index(address)?;Some((self.selected_source_hz(address,inputs)?,((self.words[i]>>4)&0xfff)as u16+1))
    }
    /// Return integer Hz only when exact; fractional rates remain available
    /// through selected_rate_ratio rather than being rounded or guessed.
    pub fn selected_clock_hz(&self,address:u32,inputs:ClockInputs)->Option<u32>{
        let(hz,divisor)=self.selected_rate_ratio(address,inputs)?;let divisor=u32::from(divisor);hz.is_multiple_of(divisor).then(||hz/divisor)
    }
    /// Same declared functional LPCON reset contract as its existing domain
    /// controls: CPU/LP retain; HP/Chip clear. No per-flop silicon claim.
    pub fn reset_domains(&mut self,hp:bool,analog:bool){if hp||analog{*self=Self::new();}}
    pub fn report(&self,inputs:ClockInputs)->String{let mut report=format!("MODEM_LP=selector_only BLE_RTC={:#x}/{:?} COEX={:#x}/{:?} WIFI={:#x}/{:?} writes={}/{}/{} divider=0 functional_gate_RF_MAC_IRQ_sleep=unsupported",
        self.words[0],self.selected_source_hz(0x600a_f004,inputs),self.words[1],self.selected_source_hz(0x600a_f008,inputs),
        self.words[2],self.selected_source_hz(0x600a_f00c,inputs),self.writes[0],self.writes[1],self.writes[2]);
        if self.words[1]>>4!=0{report=report.replace("divider=0","divider=COEX_config");report.push_str(&format!(" COEX_pre_gate DIV_NUM={} divisor={} ratio={:?} integer_hz={:?} fractional_or_unavailable_has_no_integer_rate",
            self.words[1]>>4,(self.words[1]>>4)+1,self.selected_rate_ratio(0x600a_f008,inputs),self.selected_clock_hz(0x600a_f008,inputs)));}report}
}

#[cfg(test)]
mod tests{
    use super::*;
    const INPUT:ClockInputs=ClockInputs{rc_slow_hz:Some(136_000),rc_fast_hz:Some(17_500_000),xtal_hz:Some(48_000_000)};
    const ADDRESSES:[u32;3]=[0x600a_f004,0x600a_f008,0x600a_f00c];
    #[test]
    fn actual_wifi_ble_coex_four_clear_sequence_stays_disabled(){
        let mut c=ModemLpClock::new();
        // Actual caller420031f2: WiFi, BLE RTC, COEX. Each HAL clears
        // selector bits0/1/3/2 separately; no synthetic source enable.
        for address in[0x600a_f00c,0x600a_f004,0x600a_f008]{for bit in[0,1,3,2]{
            let old=c.read(address,true).unwrap();assert_eq!(old,0);assert!(c.write(address,old&!(1<<bit),true));
            assert_eq!(c.selected_source_hz(address,INPUT),None);
        }}
        assert_eq!(c.writes,[4;3]);assert!(c.report(INPUT).contains("writes=4/4/4 divider=0"));
    }
    #[test]
    fn selected_one_hot_raw_source_is_independent_for_each_word(){
        let mut c=ModemLpClock::new();for address in ADDRESSES{
            for(value,hz)in[(1,136_000),(2,17_500_000),(4,48_000_000)]{
                assert!(c.write(address,value,true));assert_eq!(c.read(address,true),Some(value));assert_eq!(c.selected_source_hz(address,INPUT),Some(hz));
            }
            assert!(c.write(address,0,true));assert_eq!(c.selected_source_hz(address,INPUT),None);
        }
        for(address,value,hz)in[(ADDRESSES[0],1,136_000),(ADDRESSES[1],2,17_500_000),(ADDRESSES[2],4,48_000_000)]{
            assert!(c.write(address,value,true));assert_eq!(c.selected_source_hz(address,INPUT),Some(hz));
        }
    }
    #[test]
    fn selector_never_supplies_unknown_zero_or_wrong_oscillator_observations(){
        let mut c=ModemLpClock::new();for address in ADDRESSES{for value in[1,2,4]{
            assert!(c.write(address,value,true));assert_eq!(c.selected_source_hz(address,ClockInputs::default()),None);
            for bad in[0,1,24_000_000,u32::MAX]{let inputs=ClockInputs{rc_slow_hz:Some(bad),rc_fast_hz:Some(bad),xtal_hz:Some(bad)};assert_eq!(c.selected_source_hz(address,inputs),None);}
        }}
        assert!(c.write(ADDRESSES[2],2,true));assert_eq!(c.selected_source_hz(ADDRESSES[2],ClockInputs{rc_fast_hz:Some(24_000_000),..INPUT}),None,"LP_FAST XTAL/2 is not raw RC_FAST");
        assert_eq!(c.selected_source_hz(ADDRESSES[2],INPUT),Some(17_500_000));
    }
    #[test]
    fn apb_permission_is_explicit_and_does_not_change_the_selected_source(){
        let mut c=ModemLpClock::new();for address in ADDRESSES{let before=c.clone();assert_eq!(c.read(address,false),None);assert!(!c.write(address,0,false));assert_eq!(c,before);}
        assert!(c.write(ADDRESSES[2],2,true));let before=c.clone();assert_eq!(c.read(ADDRESSES[2],false),None);assert!(!c.write(ADDRESSES[2],0,false));assert_eq!(c,before);
        assert_eq!(c.selected_source_hz(ADDRESSES[2],INPUT),Some(17_500_000));
    }
    #[test]
    fn unknown_32k_paths_multiple_sources_and_reserved_bits_reject(){
        let mut c=ModemLpClock::new();for address in ADDRESSES{assert!(c.write(address,1,true));for value in[3,5,6,7,8,9,15,0xffff,0x10000,u32::MAX]{
            let before=c.clone();assert!(!c.write(address,value,true));assert_eq!(c,before);
        }if address!=ADDRESSES[1]{for value in[16,17,0xfff0]{let before=c.clone();assert!(!c.write(address,value,true));assert_eq!(c,before);}}}
    }
    #[test]
    fn exact_contains_has_no_lp_timer_master_gate_or_system_alias(){
        let mut c=ModemLpClock::new();for address in ADDRESSES{assert!(ModemLpClock::contains(address));}
        for address in[0,u32::MAX,0x600a_f000,0x600a_f005,0x600a_f010,0x600a_f014,0x600a_f018,0x600a_f01c,0x600a_f020,0x600a_f024,
            0x6009_5004,0x6009_5008,0x6009_500c,0x600b_0c10,0x600b_0c14,0x5000_0004]{
            let before=c.clone();assert!(!ModemLpClock::contains(address));assert_eq!(c.read(address,true),None);assert!(!c.write(address,0,true));assert_eq!(c.selected_source_hz(address,INPUT),None);assert_eq!(c,before);
        }
    }
    #[test]
    fn functional_reset_and_reads_have_no_timer_or_rf_side_effects(){
        let mut c=ModemLpClock::new();for address in ADDRESSES{assert!(c.write(address,2,true));}let before=c.clone();
        // Parent CPU-only and LP-only dispatch both leave hp/analog false.
        c.reset_domains(false,false);assert_eq!(c,before);for _ in 0..8{for address in ADDRESSES{assert_eq!(c.read(address,true),Some(2));c.selected_source_hz(address,INPUT);}c.report(INPUT);}assert_eq!(c,before);
        assert!(c.report(INPUT).contains("functional_gate_RF_MAC_IRQ_sleep=unsupported"));c.reset_domains(true,false);assert_eq!(c,ModemLpClock::new());
        for address in ADDRESSES{assert!(c.write(address,4,true));}c.reset_domains(false,true);assert_eq!(c,ModemLpClock::new());
    }
    const COEX:u32=0x600a_f008;
    #[test]
    fn original_coex_two_rmw_stores_preserve_num49_and_raw_xtal48(){
        let mut c=ModemLpClock::new();let old=c.read(COEX,true).unwrap();assert_eq!(old,0);
        assert!(c.write(COEX,old&!15|4,true));assert_eq!(c.read(COEX,true),Some(4)); //420b8852
        let old=c.read(COEX,true).unwrap();assert!(c.write(COEX,old&!0xfff0|((50-1)<<4),true)); //420b8864
        assert_eq!(c.read(COEX,true),Some(0x314));assert_eq!(c.words,[0,0x314,0]);assert_eq!(c.writes,[0,2,0]);
        assert_eq!(c.selected_source_hz(COEX,INPUT),Some(48_000_000));assert_eq!(c.selected_rate_ratio(COEX,INPUT),Some((48_000_000,50)));
        assert_eq!(c.selected_clock_hz(COEX,INPUT),Some(960_000));assert!(c.report(INPUT).contains("COEX_pre_gate DIV_NUM=49 divisor=50 ratio=Some((48000000, 50)) integer_hz=Some(960000)"));
        assert!(!c.report(INPUT).contains("divider=0"));assert!(c.report(INPUT).contains("functional_gate_RF_MAC_IRQ_sleep=unsupported"));
    }
    #[test]
    fn source_disable_rmw_retains_the_entire_programmed_divider(){
        let mut c=ModemLpClock::new();assert!(c.write(COEX,0x314,true));
        for(bit,expected)in[(0,0x314),(1,0x314),(3,0x314),(2,0x310)]{
            let value=c.read(COEX,true).unwrap()&!(1<<bit);assert!(c.write(COEX,value,true));assert_eq!(c.read(COEX,true),Some(expected));
        }
        assert_eq!(c.selected_source_hz(COEX,INPUT),None);assert_eq!(c.selected_rate_ratio(COEX,INPUT),None);assert_eq!(c.selected_clock_hz(COEX,INPUT),None);
        assert!(c.write(COEX,0x312,true));assert_eq!(c.selected_source_hz(COEX,INPUT),Some(17_500_000));assert_eq!(c.selected_rate_ratio(COEX,INPUT),Some((17_500_000,50)));
        assert_eq!(c.selected_clock_hz(COEX,INPUT),Some(350_000));assert!(c.write(COEX,0xfff0,true));assert_eq!(c.read(COEX,true),Some(0xfff0));
        assert_eq!(c.selected_rate_ratio(COEX,INPUT),None);assert!(c.report(INPUT).contains("DIV_NUM=4095 divisor=4096 ratio=None integer_hz=None"));
    }
    #[test]
    fn divider_zero_max_and_fractional_rates_never_round_to_integer_hz(){
        let mut c=ModemLpClock::new();
        for(value,ratio,hz)in[(4,(48_000_000,1),Some(48_000_000)),(0x2f4,(48_000_000,48),Some(1_000_000)),
            (0x314,(48_000_000,50),Some(960_000)),(0xfff4,(48_000_000,4096),None),
            (0x22,(17_500_000,3),None),(0xfff2,(17_500_000,4096),None),(0x31,(136_000,4),Some(34_000))]{
            assert!(c.write(COEX,value,true));assert_eq!(c.read(COEX,true),Some(value));assert_eq!(c.selected_rate_ratio(COEX,INPUT),Some(ratio));assert_eq!(c.selected_clock_hz(COEX,INPUT),hz);
        }
        assert_eq!(c.selected_source_hz(COEX,INPUT),Some(136_000));
    }
    #[test]
    fn divided_observation_requires_the_actual_raw_input_without_caching_or_fallback(){
        let mut c=ModemLpClock::new();assert!(c.write(COEX,0x314,true));let before=c.clone();
        for inputs in[ClockInputs::default(),ClockInputs{xtal_hz:None,..INPUT},ClockInputs{xtal_hz:Some(0),..INPUT},
            ClockInputs{xtal_hz:Some(40_000_000),..INPUT},ClockInputs{xtal_hz:Some(u32::MAX),..INPUT}]{
            assert_eq!(c.selected_source_hz(COEX,inputs),None);assert_eq!(c.selected_rate_ratio(COEX,inputs),None);assert_eq!(c.selected_clock_hz(COEX,inputs),None);assert_eq!(c,before);
        }
        assert_eq!(c.selected_rate_ratio(COEX,INPUT),Some((48_000_000,50)));assert_eq!(c,before);
        assert!(c.write(COEX,0x312,true));assert_eq!(c.selected_rate_ratio(COEX,ClockInputs{rc_fast_hz:Some(24_000_000),..INPUT}),None);
        assert_eq!(c.selected_clock_hz(COEX,INPUT),Some(350_000));
    }
    #[test]
    fn named_divider_does_not_widen_selectors_reserved_words_or_other_targets(){
        let mut c=ModemLpClock::new();assert!(c.write(COEX,0x314,true));
        for value in[0x313,0x315,0x316,0x317,0x318,0x319,0x31f,0x10004,0xffff_fff4,u32::MAX]{
            let before=c.clone();assert!(!c.write(COEX,value,true));assert_eq!(c,before);
        }
        for address in[ADDRESSES[0],ADDRESSES[2]]{for value in[0x10,0x11,0x314,0xfff0,0xfff4]{
            let before=c.clone();assert!(!c.write(address,value,true));assert_eq!(c,before);
        }}
        for address in[COEX-1,COEX+1,0x600a_f010,0x600a_f018,0x600a_f01c,0x600a_f024,0x600b_0c10]{
            let before=c.clone();assert!(!ModemLpClock::contains(address));assert_eq!(c.selected_rate_ratio(address,INPUT),None);assert_eq!(c.selected_clock_hz(address,INPUT),None);assert!(!c.write(address,0,true));assert_eq!(c,before);
        }
    }
    #[test]
    fn full_divider_state_obeys_apb_and_reset_without_clock_or_rf_progress(){
        let mut c=ModemLpClock::new();assert!(c.write(COEX,0xfff4,true));assert!(c.write(ADDRESSES[0],2,true));assert!(c.write(ADDRESSES[2],1,true));let before=c.clone();
        assert_eq!(c.read(COEX,false),None);assert!(!c.write(COEX,0,false));assert_eq!(c,before);assert_eq!(c.selected_rate_ratio(COEX,INPUT),Some((48_000_000,4096)));
        c.reset_domains(false,false);assert_eq!(c,before);for _ in 0..8{assert_eq!(c.read(COEX,true),Some(0xfff4));c.report(INPUT);c.selected_clock_hz(COEX,INPUT);}assert_eq!(c,before);
        c.reset_domains(true,false);assert_eq!(c,ModemLpClock::new());assert!(c.write(COEX,0x314,true));c.reset_domains(false,true);assert_eq!(c,ModemLpClock::new());
        c.writes[1]=u64::MAX;assert!(c.write(COEX,0x314,true));assert_eq!(c.writes[1],u64::MAX);let before=c.clone();assert!(!c.write(COEX,u32::MAX,true));assert_eq!(c,before);
    }
    #[test]
    fn divider_zero_reports_and_raw_source_meaning_are_unchanged(){
        let mut c=ModemLpClock::new();assert_eq!(c.report(INPUT),"MODEM_LP=selector_only BLE_RTC=0x0/None COEX=0x0/None WIFI=0x0/None writes=0/0/0 divider=0 functional_gate_RF_MAC_IRQ_sleep=unsupported");
        for address in ADDRESSES{for(value,source)in[(1,136_000),(2,17_500_000),(4,48_000_000)]{
            assert!(c.write(address,value,true));assert_eq!(c.selected_source_hz(address,INPUT),Some(source));assert_eq!(c.selected_rate_ratio(address,INPUT),Some((source,1)));assert_eq!(c.selected_clock_hz(address,INPUT),Some(source));
        }}assert!(c.report(INPUT).contains("divider=0"));assert!(!c.report(INPUT).contains("COEX_pre_gate"));
    }
}
