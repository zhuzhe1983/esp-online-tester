//! C5 BOD mode0 configuration under an explicitly ideal, stable USB3V3 input.
//! Fixed IDF5.5.3/5.5.4 C5 brownout HAL/LL, LP_ANA and REGI2C headers;
//! docs/evidence/c5-bod0-startup-2026-11-25 records source and reset limits.
//! The only supported comparator profile uses all four rails at virtual
//! 3300mV and code7 at nominal2510mV. Neither value is a silicon measurement.
//! Dynamic supply events, flash/RF actions and hardware reset are unsupported.
const BASE:u32=0x600b_2c00;const POR:u32=0x0ffc_0100;const MASK:u32=0xffff_ffc0;
const DETECT:u32=1<<29;const CLEAR_COUNT:u32=1<<28;const IRQ:u32=1<<31;
const IDEAL_RAIL_MV:[u16;4]=[3300;4];const CODE7_NOMINAL_MV:u16=2510;
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct BootBrownout{
    control:u32,ena:u32,raw:bool,count:u16,ideal:bool,threshold:Option<u8>,
    writes:u64,count_clears:u64,irq_clears:u64,
}
impl BootBrownout{
    pub fn new()->Self{Self{control:POR,ena:0,raw:false,count:0,ideal:false,threshold:None,writes:0,count_clears:0,irq_clears:0}}
    pub fn contains(address:u32)->bool{matches!(address,BASE|0x600b_2c10|0x600b_2c14|0x600b_2c18|0x600b_2c1c)}
    /// Explicit board/comparator approximation, not a calibrated voltage API.
    /// Selecting it does not preseed the REGI2C threshold or enable detection.
    pub fn enable_ideal_usb3v3(&mut self){self.ideal=true;}
    /// Withdraw observations/output without rewriting guest configuration.
    /// The parent drops this owner when leaving the validated ROM profile.
    pub fn disable_ideal_profile(&mut self){self.ideal=false;}
    /// Only the actual successful guest slave61/reg5 write qualifies code7.
    /// Withdrawing a live input is outside the stable-input contract; reject
    /// before changing state, then let the parent reject that mode change.
    pub fn observe_threshold(&mut self,threshold:Option<u8>)->bool{
        if threshold.is_some_and(|v|v!=7)||self.control&DETECT!=0&&threshold!=Some(7){return false;}
        self.threshold=threshold;true
    }
    fn supply_healthy(&self)->Option<bool>{
        if !self.ideal||self.threshold!=Some(7){return None;}
        Some(IDEAL_RAIL_MV.iter().all(|&mv|mv>CODE7_NOMINAL_MV))
    }
    fn detection_known(&self)->bool{self.control&DETECT!=0&&self.supply_healthy()==Some(true)}
    pub fn read(&self,address:u32)->Option<u32>{
        if !self.ideal{return None;}
        match address{
            BASE=>Some(self.control),0x600b_2c18=>Some(self.ena),0x600b_2c1c=>Some(0),
            // Header: RAW is valid only while mode0 is enabled. Do not
            // turn an unconfigured comparator into an observed good rail.
            0x600b_2c10 if self.detection_known()=>Some(u32::from(self.raw)*IRQ),
            0x600b_2c14 if self.detection_known()=>Some(u32::from(self.raw)*self.ena),
            _=>None,
        }
    }
    pub fn write(&mut self,address:u32,value:u32)->bool{
        if !self.ideal{return false;}
        match address{
            BASE=>{
                // Named controls only. No hardware cause0f/domain dispatch,
                // dynamic counter timing or other threshold is implemented.
                let wait=(value>>8)&0x3ff;
                if value&!MASK!=0||value&IRQ!=0||!matches!(wait,1|2)||(value>>18)&0x3ff!=0x3ff{return false;}
                if value&DETECT!=0&&self.supply_healthy()!=Some(true){return false;}
                if value&CLEAR_COUNT!=0{self.count=0;self.count_clears=self.count_clears.saturating_add(1);}
                // All supported rails remain strictly above the selected
                // nominal threshold, so no brownout pulse reaches count.
                // This is a zero-input trajectory, not a completed alarm.
                self.control=value;
            }
            0x600b_2c18=>{if value&!IRQ!=0{return false;}self.ena=value;}
            0x600b_2c1c=>{
                if value&!IRQ!=0{return false;}
                if value!=0{self.raw=false;self.irq_clears=self.irq_clears.saturating_add(1);}
            }
            _=>return false,
        }
        self.writes=self.writes.saturating_add(1);true
    }
    pub fn irq(&self)->bool{self.ideal&&self.raw&&self.ena&IRQ!=0}
    /// Functional domains from TRM9.1, not a per-flop retention claim.
    /// CPU/Core retain LP controls. LP/System resets digital state; Chip
    /// also withdraws the analog-byte observation and clears diagnostics.
    /// The explicit board input remains selected across controller reset.
    pub fn reset_domains(&mut self,_hp:bool,lp:bool,analog:bool){
        if lp||analog{self.control=POR;self.ena=0;self.raw=false;self.count=0;}
        if analog{self.threshold=None;self.writes=0;self.count_clears=0;self.irq_clears=0;}
    }
    pub fn report(&self)->String{format!("BOD0=configuration_only ideal_usb3v3={} rails_mv={IDEAL_RAIL_MV:?} code7_nominal_mv={CODE7_NOMINAL_MV} threshold={:?} comparator_healthy={:?} control={:#010x} count={} raw={} HP_ena={:#010x} IRQ15={} writes={} count_clear_writes={} irq_clears={} dynamic_voltage_flash_RF_reset=unsupported",
        u8::from(self.ideal),self.threshold,self.supply_healthy(),self.control,self.count,u8::from(self.raw),self.ena,u8::from(self.irq()),self.writes,self.count_clears,self.irq_clears)}
}

#[cfg(test)]
mod tests{
    use super::*;
    fn selected()->BootBrownout{let mut b=BootBrownout::new();b.enable_ideal_usb3v3();b}
    fn configured()->BootBrownout{
        let mut b=selected();
        for value in[0x0ffc_0200,0x0ffc_0240,0x0ffc_02c0,0x1ffc_02c0,0x0ffc_02c0,0x4ffc_02c0]{assert!(b.write(BASE,value));}
        assert!(b.observe_threshold(Some(7)));assert!(b.write(BASE,0x6ffc_02c0));
        assert!(b.write(BASE+0x1c,IRQ));assert!(b.write(BASE+0x18,IRQ));b
    }
    #[test]
    fn profile_is_explicit_and_only_exact_owned_words_decode(){
        let mut b=BootBrownout::new();let before=b.clone();
        for a in[BASE,BASE+0x10,BASE+0x14,BASE+0x18,BASE+0x1c]{assert!(BootBrownout::contains(a));assert_eq!(b.read(a),None);assert!(!b.write(a,0));}
        for a in[0,BASE+1,BASE+4,BASE+8,BASE+12,BASE+0x20,BASE+0x28,BASE+0x2c,BASE+0x3fc,u32::MAX]{assert!(!BootBrownout::contains(a));assert_eq!(b.read(a),None);assert!(!b.write(a,0));}
        assert_eq!(b,before);assert!(!b.irq());b.enable_ideal_usb3v3();assert_eq!(b.read(BASE),Some(POR));
        assert_eq!(b.read(BASE+0x10),None);assert_eq!(b.read(BASE+0x14),None);
    }
    #[test]
    fn actual_rmw_sequence_enables_detection_and_mask_without_synthetic_event(){
        let b=configured();assert_eq!(b.control,0x6ffc_02c0);assert_eq!(b.count_clears,1);assert_eq!(b.irq_clears,1);
        assert_eq!(b.read(BASE+0x10),Some(0));assert_eq!(b.read(BASE+0x14),Some(0));assert_eq!(b.read(BASE+0x18),Some(IRQ));
        assert_eq!(b.read(BASE+0x1c),Some(0));assert!(!b.irq());assert_eq!(b.supply_healthy(),Some(true));assert_eq!(b.count,0);
        assert!(b.report().contains("code7_nominal_mv=2510 threshold=Some(7) comparator_healthy=Some(true)"));
        let before=b.clone();for _ in 0..100{b.read(BASE+0x10);b.read(BASE+0x14);b.irq();b.report();}assert_eq!(b,before);
    }
    #[test]
    fn detector_enable_requires_actual_threshold_and_known_ideal_input(){
        let mut b=selected();let before=b.clone();assert!(!b.write(BASE,POR|DETECT));assert_eq!(b,before);
        for value in[Some(0),Some(1),Some(2),Some(6),Some(8),Some(255)]{assert!(!b.observe_threshold(value));assert_eq!(b,before);}
        assert!(b.observe_threshold(Some(7)));assert!(b.write(BASE,POR|DETECT));let before=b.clone();
        assert!(!b.observe_threshold(None));assert_eq!(b,before);b.disable_ideal_profile();assert_eq!(b.read(BASE+0x10),None);
        let before=b.clone();assert!(!b.write(BASE,POR|DETECT));assert_eq!(b,before);assert!(!b.irq());
    }
    #[test]
    fn count_clear_is_held_rw_and_irq_clear_is_a_separate_wt(){
        let mut b=configured();let word=b.control;assert!(b.write(BASE,word|CLEAR_COUNT));assert_eq!(b.read(BASE),Some(word|CLEAR_COUNT));
        let before=b.clone();for _ in 0..4{assert_eq!(b.read(BASE),Some(word|CLEAR_COUNT));}assert_eq!(b,before);
        assert!(b.write(BASE,word));assert_eq!(b.count,0);assert!(b.write(BASE+0x1c,0));assert_eq!(b.irq_clears,1);
        assert!(b.write(BASE+0x1c,IRQ));assert_eq!(b.irq_clears,2);assert_eq!(b.read(BASE+0x1c),Some(0));assert_eq!(b.read(BASE),Some(word));
    }
    #[test]
    fn private_latch_fixture_checks_mask_clear_and_count_are_distinct(){
        // Seed internal state only to check register semantics. No public
        // supply/event injection or physical detector is implemented.
        let mut b=configured();b.raw=true;b.count=2;assert!(b.irq());assert_eq!(b.read(BASE+0x14),Some(IRQ));
        assert!(b.write(BASE+0x18,0));assert_eq!(b.read(BASE+0x10),Some(IRQ));assert_eq!(b.read(BASE+0x14),Some(0));assert!(!b.irq());
        assert!(b.write(BASE,b.control|CLEAR_COUNT));assert_eq!(b.count,0);assert!(b.raw);
        assert!(b.write(BASE+0x18,IRQ));assert!(b.irq());assert!(b.write(BASE+0x1c,IRQ));assert!(!b.raw);assert!(!b.irq());
    }
    #[test]
    fn reset_actions_raw_writes_reserved_bits_and_unknown_waits_reject_atomically(){
        let mut b=configured();
        for (a,v) in[(BASE,b.control|IRQ),(BASE,b.control|1),(BASE,b.control&!0x0003_ff00),(BASE,(b.control&!0x0003_ff00)|0x300),
            (BASE,b.control&!0x0ffc_0000),(BASE+0x10,IRQ),(BASE+0x10,0),(BASE+0x14,0),(BASE+0x18,1),(BASE+0x1c,IRQ|1),
            (BASE+0x20,0),(BASE+0x28,IRQ),(BASE+0x2c,IRQ)]{
            let before=b.clone();assert!(!b.write(a,v),"{a:#x}={v:#x}");assert_eq!(b,before);
        }
    }
    #[test]
    fn mask_disable_detection_disable_and_profile_withdrawal_do_not_create_events(){
        let mut b=configured();let config=b.control;assert!(b.write(BASE+0x18,0));assert!(!b.irq());assert_eq!(b.read(BASE+0x14),Some(0));
        assert!(b.write(BASE,config&!DETECT));assert_eq!(b.read(BASE+0x10),None);assert_eq!(b.count,0);
        assert!(b.observe_threshold(None));assert_eq!(b.supply_healthy(),None);let before=b.clone();
        assert!(!b.write(BASE,config));assert_eq!(b,before);b.disable_ideal_profile();assert_eq!(b.control,config&!DETECT);
        b.enable_ideal_usb3v3();assert_eq!(b.read(BASE),Some(config&!DETECT));assert_eq!(b.threshold,None);
    }
    #[test]
    fn cpu_core_retain_lp_system_resets_controls_and_chip_withdraws_analog_observation(){
        let mut b=configured();let before=b.clone();b.reset_domains(false,false,false);assert_eq!(b,before);b.reset_domains(true,false,false);assert_eq!(b,before);
        b.reset_domains(true,true,false);assert_eq!((b.control,b.ena,b.raw,b.count),(POR,0,false,0));assert_eq!(b.threshold,Some(7));assert_eq!(b.writes,before.writes);
        assert!(b.write(BASE,POR|DETECT));b.reset_domains(true,true,true);assert_eq!(b.threshold,None);assert_eq!(b.writes,0);assert!(b.ideal);
        let before=b.clone();assert!(!b.write(BASE,POR|DETECT));assert_eq!(b,before);assert_eq!(b.read(BASE),Some(POR));
    }
}
