//! C5 boot entropy-path timer and explicit constant virtual input capture.
//! IDF5.5.3/5.5.4 C5 apb_saradc_reg.h/pcr_reg.h + adc_ll.h;
//! TRM1.1 46.5.3/46.5.5/46.5.7; fixed-source receipt and limits:
//! docs/evidence/c5-saradc-entropy-2026-11-24.
//! No physical ADC conversion, calibrated voltage, noise, entropy, ADC DONE,
//! data/status/IRQ/DMA or coupling into the independent deterministic RNG.
const CONF:u32=0x6009_6088;const CLKM:u32=0x6009_608c;
const CTRL:u32=0x6000_e000;const CTRL2:u32=0x6000_e004;const PATT1:u32=0x6000_e018;const PATT2:u32=0x6000_e01c;
const CTRL_POR:u32=0x4003_8240;const CTRL2_POR:u32=0x0000_a1fe;
const CTRL_BOOT:u32=0x4000_87c0;const CTRL2_BOOT:u32=0x000c_81fe;
const TIMER:u32=1<<24;const FUNC_CLOCK:u32=1<<22;const SAR_CLOCK:u32=1<<6;
const TIMEBASE:u64=240_000_000;const XTAL:u64=48_000_000;
const PERIOD:u64=TIMEBASE*2*200;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub(crate) struct ClockInputs{pub xtal_hz:Option<u32>,pub reserved_path_ready:bool}
#[derive(Clone,Debug,PartialEq,Eq)]
struct Capture{raw:[u16;2],phase:u64,pointer:usize,triggers:u64,captures:[u64;2],last:[Option<u16>;2],last_at:Option<u64>}
impl Capture{fn new(raw:[u16;2])->Self{Self{raw,phase:0,pointer:0,triggers:0,captures:[0;2],last:[None;2],last_at:None}}}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct BootSarAdc{
    conf:u32,clkm:u32,ctrl:u32,ctrl2:u32,pattern:[u32;2],cycle:u64,inputs:ClockInputs,
    profile:Option<Capture>,clock_fault:bool,
}
impl BootSarAdc{
    pub fn new()->Self{Self{conf:1,clkm:4<<12,ctrl:CTRL_POR,ctrl2:CTRL2_POR,pattern:[0x00ff_ffff;2],cycle:0,
        inputs:ClockInputs::default(),profile:None,clock_fault:false}}
    pub fn contains(address:u32)->bool{matches!(address,CONF|CLKM|CTRL|CTRL2|PATT1|PATT2)}
    /// Inputs are ideal raw codes supplied explicitly by the selected virtual
    /// board profile, never analog POR/calibration/noise or a seeded RNG.
    pub fn enable_virtual_profile(&mut self,cycle:u64,raw:[u16;2])->Result<(),&'static str>{
        if raw.iter().any(|v|*v>4095){return Err("C5 virtual reserved ADC raw inputs must be 12bit");}
        if let Some(p)=&self.profile{if p.raw!=raw{return Err("C5 virtual reserved ADC inputs require explicit profile deselection before replacement");}}
        else if self.requested(){return Err("C5 virtual reserved ADC cannot attach to an already enabled timer");}
        let mut candidate=self.clone();if !candidate.pump(cycle,candidate.inputs){return Err("C5 virtual reserved ADC clock is unsupported");}
        if candidate.profile.is_none(){candidate.profile=Some(Capture::new(raw));}*self=candidate;Ok(())
    }
    /// Withdraw functional capture without rewriting readable guest bits.
    pub fn disable_virtual_profile(&mut self){self.profile=None;self.clock_fault=false;}
    fn requested(&self)->bool{self.ctrl2&TIMER!=0}
    pub fn virtual_timer_enabled(&self)->bool{self.profile.is_some()&&self.requested()}
    fn gated(&self)->bool{self.conf&2!=0||self.clkm&FUNC_CLOCK==0||self.ctrl&SAR_CLOCK==0}
    fn apb_clocked(&self)->bool{self.conf&4!=0&&self.conf&8==0}
    fn mode_known(&self)->bool{
        // Only the original RNG compatibility path, including its internally
        // selected reserved ADC2 encoding, not public external ADC channels.
        self.clkm&0x003f_ffff==0&&self.ctrl&!SAR_CLOCK==CTRL_BOOT&!SAR_CLOCK&&
            self.ctrl2&!TIMER==CTRL2_BOOT&&self.pattern[0]>>18==0x1f&&(self.pattern[0]>>12)&0x3f==0x27
    }
    fn inputs_known(&self)->bool{self.inputs.xtal_hz==Some(XTAL as u32)&&self.inputs.reserved_path_ready}
    fn executable(&self)->bool{self.profile.is_some()&&self.mode_known()&&(self.gated()||self.inputs_known())}
    fn reset_adc(&mut self){
        self.ctrl=CTRL_POR;self.ctrl2=CTRL2_POR;self.pattern=[0x00ff_ffff;2];self.clock_fault=false;
        if let Some(p)=&mut self.profile{*p=Capture::new(p.raw);}
    }
    /// Old clock/power observations advance first. A lost known source or
    /// unmodeled live analog route is an explicit boundary, not fake progress.
    pub fn pump(&mut self,cycle:u64,inputs:ClockInputs)->bool{
        let cycle=self.cycle.max(cycle);let elapsed=cycle-self.cycle;
        if self.virtual_timer_enabled()&&!self.gated()&&!self.clock_fault{
            if !self.executable(){self.clock_fault=true;}else if let Some(p)=&mut self.profile{
                let old_phase=p.phase;
                let total=u128::from(elapsed)*u128::from(XTAL)+u128::from(old_phase);
                let count=(total/u128::from(PERIOD))as u64;p.phase=(total%u128::from(PERIOD))as u64;
                if count!=0{
                    let first=p.pointer;let amounts=[count.div_ceil(2),count/2];
                    for (n,amount)in amounts.into_iter().enumerate(){let index=(first+n)%2;
                        p.captures[index]=p.captures[index].saturating_add(amount);if amount!=0{p.last[index]=Some(p.raw[index]);}}
                    p.triggers=p.triggers.saturating_add(count);p.pointer=(first+(count%2)as usize)%2;
                    let delta=(u128::from(count)*u128::from(PERIOD)-u128::from(old_phase)).div_ceil(u128::from(XTAL));
                    p.last_at=self.cycle.checked_add(delta as u64);
                }
            }
        }
        self.cycle=cycle;self.inputs=inputs;
        if self.profile.is_some()&&self.requested()&&!self.gated()&&!self.executable(){self.clock_fault=true;}
        !self.clock_fault
    }
    pub fn deadline(&self)->Option<u64>{
        if !self.requested()||self.gated()||self.clock_fault||!self.executable(){return None;}
        let p=self.profile.as_ref()?;self.cycle.checked_add((PERIOD-p.phase).div_ceil(XTAL))
    }
    pub fn read_at(&mut self,address:u32,cycle:u64,inputs:ClockInputs)->Option<u32>{
        if !Self::contains(address)||!self.pump(cycle,inputs){return None;}
        Some(match address{CONF=>self.conf,CLKM=>self.clkm,_ if !self.apb_clocked()=>return None,
            CTRL=>self.ctrl,CTRL2=>self.ctrl2,PATT1=>self.pattern[0],PATT2=>self.pattern[1],_=>return None})
    }
    pub fn write_at(&mut self,address:u32,value:u32,cycle:u64,inputs:ClockInputs)->bool{
        if !Self::contains(address)||!self.pump(cycle,inputs){return false;}
        let mut candidate=self.clone();let was=candidate.requested();
        match address{
            CONF=>{
                if value&!15!=0{return false;}
                // Both actual boot reset pulses converge on complete ADC
                // defaults. Separate physical APB/function flop retention is
                // not inferred from the sparse header reset descriptions.
                if value&!candidate.conf&10!=0{candidate.reset_adc();}candidate.conf=value;
            },
            CLKM=>{if value&!0x007f_ffff!=0{return false;}candidate.clkm=value;},
            _=>{
                if !candidate.apb_clocked()||candidate.conf&2!=0{return false;}
                match address{
                    CTRL=>{
                        if (value^candidate.ctrl)&!(SAR_CLOCK|0xff<<7|7<<15|1<<23)!=0{return false;}
                        if value&(1<<23)!=0{if let Some(p)=&mut candidate.profile{p.pointer=0;}}candidate.ctrl=value;
                    },
                    CTRL2=>{if (value^candidate.ctrl2)&!(0xfff<<12|TIMER)!=0{return false;}candidate.ctrl2=value;},
                    PATT1|PATT2=>{
                        let n=usize::from(address==PATT2);if value&!0x00ff_ffff!=0||was&&value!=candidate.pattern[n]{return false;}candidate.pattern[n]=value;
                    },_=>return false,
                }
            },
        }
        if candidate.requested()&&!candidate.executable(){return false;}
        if !was&&candidate.requested(){if let Some(p)=&mut candidate.profile{p.phase=0;}}
        *self=candidate;true
    }
    /// CPU/LP-only preserve this HP digital controller. Core resets its
    /// digital registers/counters; analog reset also withdraws the virtual
    /// input profile, which must be explicitly selected for the new epoch.
    pub fn reset_domains(&mut self,cycle:u64,hp:bool,_lp:bool,analog:bool){
        self.pump(cycle,self.inputs);if hp||analog{self.reset_adc();self.conf=1;self.clkm=4<<12;}
        if analog{self.profile=None;}
    }
    pub fn report(&self)->String{self.profile.as_ref().map_or_else(
        ||format!("SARADC=unprofiled timer_bit={} raw_inputs=None physical_conversion=unmodeled entropy=none",u8::from(self.requested())),
        |p|format!("SARADC=virtual_reserved_timer raw_inputs={:?} input_provenance=ideal_constant timer={} gated={} triggers={} captures={:?} pointer={} last={:?} last_at={:?} timer_edges={} fractional_phase={} clock_fault={} source_sel={} NUM={} SAR_DIV={} target={} mode_supported={} scheduler=nominal240MHz physical_conversion=unmodeled DONE_data_IRQ_DMA=unsupported entropy=none RNG=independent",
            p.raw,u8::from(self.requested()),u8::from(self.gated()),p.triggers,p.captures,p.pointer,p.last,p.last_at,p.phase/TIMEBASE,p.phase%TIMEBASE,u8::from(self.clock_fault),self.clkm>>20&3,self.clkm>>12&255,self.ctrl>>7&255,self.ctrl2>>12&4095,u8::from(self.mode_known())))}
}

#[cfg(test)]
mod tests{
    use super::*;
    const READY:ClockInputs=ClockInputs{xtal_hz:Some(48_000_000),reserved_path_ready:true};
    fn wr(a:&mut BootSarAdc,address:u32,value:u32,at:u64){assert!(a.write_at(address,value,at,READY),"{address:#x}={value:#x}");}
    fn selected(raw:[u16;2])->BootSarAdc{let mut a=BootSarAdc::new();a.enable_virtual_profile(0,raw).unwrap();a.pump(0,READY);a}
    fn boot_config(a:&mut BootSarAdc,at:u64){
        assert_eq!(a.read_at(CONF,at,READY),Some(1));for value in [3,1,9,1,5]{wr(a,CONF,value,at);}
        wr(a,CLKM,0x404000,at);wr(a,CLKM,0x404000,at);wr(a,CTRL,CTRL_POR,at);
        for value in [FUNC_CLOCK,FUNC_CLOCK,FUNC_CLOCK]{wr(a,CLKM,value,at);}
        wr(a,PATT1,0x7fffff,at);wr(a,PATT1,0x7e7fff,at);
        wr(a,CTRL,0x40008240,at);wr(a,CTRL,CTRL_BOOT,at);wr(a,CTRL2,CTRL2_BOOT,at);
    }
    fn running(raw:[u16;2])->BootSarAdc{let mut a=selected(raw);boot_config(&mut a,0);wr(&mut a,CTRL2,CTRL2_BOOT|TIMER,0);a}
    #[test]
    fn complete_por_words_and_exact_apb_gate_are_retained(){
        let mut a=BootSarAdc::new();assert_eq!(a.read_at(CONF,0,READY),Some(1));assert_eq!(a.read_at(CLKM,0,READY),Some(0x4000));
        assert_eq!(a.read_at(CTRL,0,READY),None);wr(&mut a,CONF,5,0);
        for(address,value)in [(CTRL,CTRL_POR),(CTRL2,CTRL2_POR),(PATT1,0xffffff),(PATT2,0xffffff)]{assert_eq!(a.read_at(address,0,READY),Some(value));}
        assert_eq!(a.deadline(),None);assert!(a.report().contains("raw_inputs=None"));
    }
    #[test]
    fn original_digital_sequence_configures_then_real_edges_trigger_capture(){
        let mut a=selected([2048;2]);boot_config(&mut a,586_112);
        assert_eq!((a.conf,a.clkm,a.ctrl,a.ctrl2,a.pattern[0]),(5,0x400000,0x400087c0,0xc81fe,0x7e7fff));assert_eq!(a.deadline(),None);
        wr(&mut a,CTRL2,0x010c81fe,586_112);assert_eq!(a.deadline(),Some(588_112));
        a.pump(588_111,READY);assert_eq!(a.profile.as_ref().unwrap().last,[None;2]);assert_eq!(a.read_at(CTRL2,588_111,READY),Some(0x010c81fe));
        a.pump(588_112,READY);let p=a.profile.as_ref().unwrap();assert_eq!((p.triggers,p.captures,p.pointer,p.last,p.last_at),(1,[1,0],1,[Some(2048),None],Some(588_112)));
    }
    #[test]
    fn capture_is_alternating_declared_input_and_never_a_calibration_or_rng_value(){
        let mut a=running([7,4095]);a.pump(8000,READY);let p=a.profile.as_ref().unwrap();
        assert_eq!((p.triggers,p.captures,p.pointer,p.last,p.last_at),(4,[2,2],0,[Some(7),Some(4095)],Some(8000)));
        let report=a.report();for text in ["input_provenance=ideal_constant","entropy=none","RNG=independent","physical_conversion=unmodeled","SAR_DIV=15"]{assert!(report.contains(text));}
        wr(&mut a,CTRL2,CTRL2_BOOT,8000);a.pump(80_000,READY);assert_eq!(a.profile.as_ref().unwrap().triggers,4);assert_eq!(a.deadline(),None);
    }
    #[test]
    fn source_edge_phase_and_capture_counts_are_chunk_invariant(){
        let mut a=running([0,4095]);let mut b=a.clone();a.pump(8423,READY);for at in [99,1234,1999,2000,2999,5001,8423]{b.pump(at,READY);}assert_eq!(a,b);
        assert_eq!(a.deadline(),Some(10_000));assert_eq!(a.profile.as_ref().unwrap().phase/TIMEBASE,84);
    }
    #[test]
    fn functional_clock_gates_pause_phase_but_apb_gate_only_blocks_registers(){
        let mut a=running([2048;2]);wr(&mut a,CLKM,0,1000);assert_eq!(a.deadline(),None);a.pump(10_000,READY);
        assert_eq!(a.profile.as_ref().unwrap().phase,PERIOD/2);wr(&mut a,CLKM,FUNC_CLOCK,10_000);assert_eq!(a.deadline(),Some(11_000));
        wr(&mut a,CONF,1,10_000);assert_eq!(a.read_at(CTRL,10_000,READY),None);assert!(!a.write_at(CTRL,CTRL_BOOT,10_000,READY));
        a.pump(11_000,READY);assert_eq!(a.profile.as_ref().unwrap().triggers,1);assert_eq!(a.deadline(),Some(13_000));
    }
    #[test]
    fn sar_clock_gate_pauses_the_same_timer_source_phase(){
        let mut a=running([2048;2]);wr(&mut a,CTRL,CTRL_BOOT&!SAR_CLOCK,1000);a.pump(10_000,READY);assert_eq!(a.deadline(),None);
        wr(&mut a,CTRL,CTRL_BOOT,10_000);assert_eq!(a.deadline(),Some(11_000));assert_eq!(a.profile.as_ref().unwrap().triggers,0);
    }
    #[test]
    fn old_source_advances_before_new_unavailable_clock_or_analog_observations(){
        for future in [ClockInputs{xtal_hz:None,..READY},ClockInputs{reserved_path_ready:false,..READY}]{
            let mut a=running([2048;2]);assert!(!a.pump(2001,future));let p=a.profile.as_ref().unwrap();assert_eq!((p.triggers,p.last_at),(1,Some(2000)));
            assert_eq!(a.deadline(),None);assert!(a.report().contains("clock_fault=1"));assert!(!a.pump(20_000,READY));assert_eq!(a.profile.as_ref().unwrap().triggers,1);
        }
    }
    #[test]
    fn start_requires_explicit_profile_actual_inputs_and_complete_supported_mode(){
        let mut a=BootSarAdc::new();boot_config(&mut a,0);let before=a.clone();assert!(!a.write_at(CTRL2,CTRL2_BOOT|TIMER,0,READY));assert_eq!(a,before);
        a.enable_virtual_profile(0,[2048;2]).unwrap();for input in [ClockInputs::default(),ClockInputs{xtal_hz:Some(40_000_000),..READY},ClockInputs{reserved_path_ready:false,..READY}]{
            a.pump(0,input);let before=a.clone();assert!(!a.write_at(CTRL2,CTRL2_BOOT|TIMER,0,input));assert_eq!(a,before);
        }assert!(a.write_at(CTRL2,CTRL2_BOOT|TIMER,0,READY));assert_eq!(a.deadline(),Some(2000));
    }
    #[test]
    fn unsupported_sources_dividers_targets_and_public_adc_patterns_do_not_start(){
        for(address,value)in [(CLKM,FUNC_CLOCK|1<<20),(CLKM,FUNC_CLOCK|2<<20),(CLKM,FUNC_CLOCK|1<<12),(CLKM,FUNC_CLOCK|1),
            (CTRL2,0x000c91fe),(CTRL,CTRL_BOOT+0x80),(CTRL,CTRL_BOOT&!(7<<15)),(PATT1,0x27e7ff)]{
            let mut a=selected([2048;2]);boot_config(&mut a,0);wr(&mut a,address,value,0);let before=a.clone();
            assert!(!a.write_at(CTRL2,a.ctrl2|TIMER,0,READY));assert_eq!(a,before);assert_eq!(a.deadline(),None);
        }
    }
    #[test]
    fn live_unknown_modes_start_dma_and_reserved_bits_reject_transactionally(){
        let mut a=running([2048;2]);for(address,value)in [(CTRL,CTRL_BOOT|1),(CTRL,CTRL_BOOT|2),(CTRL,CTRL_BOOT|1<<23),(CTRL,CTRL_BOOT|1<<18),
            (CTRL2,0x010c91fe),(CTRL2,(CTRL2_BOOT|TIMER)^1),(PATT1,0xffffff),(PATT2,0),(CLKM,FUNC_CLOCK|1<<20),(CONF,0x15),(PATT1,0xff7e7fff)]{
            let before=a.clone();assert!(!a.write_at(address,value,0,READY),"{address:#x}={value:#x}");assert_eq!(a,before);
        }
    }
    #[test]
    fn profile_selection_is_atomic_idempotent_and_cannot_replay_live_history(){
        let mut a=running([2048;2]);a.pump(2222,READY);let before=a.clone();a.enable_virtual_profile(2222,[2048;2]).unwrap();assert_eq!(a,before);
        for raw in [[0,4096],[2047,2048]]{let before=a.clone();assert!(a.enable_virtual_profile(2222,raw).is_err());assert_eq!(a,before);}
        let visible=a.ctrl2;a.disable_virtual_profile();assert_eq!(a.ctrl2,visible);assert_eq!(a.deadline(),None);assert!(a.pump(5000,ClockInputs::default()));
        let before=a.clone();assert!(a.enable_virtual_profile(5000,[2048;2]).is_err());assert_eq!(a,before);
        wr(&mut a,CTRL2,CTRL2_BOOT,5000);a.enable_virtual_profile(5000,[2048;2]).unwrap();wr(&mut a,CTRL2,CTRL2_BOOT|TIMER,5000);assert_eq!(a.deadline(),Some(7000));
    }
    #[test]
    fn timer_reenable_starts_a_new_interval_without_replaying_or_resetting_pattern_pointer(){
        let mut a=running([3,4]);a.pump(2001,READY);wr(&mut a,CTRL2,CTRL2_BOOT,2001);wr(&mut a,CTRL2,CTRL2_BOOT|TIMER,2500);
        assert_eq!(a.deadline(),Some(4500));a.pump(4500,READY);let p=a.profile.as_ref().unwrap();assert_eq!((p.triggers,p.pointer,p.last),(2,0,[Some(3),Some(4)]));
    }
    #[test]
    fn original_disable_stops_before_analog_route_and_default_patterns_are_restored(){
        let mut a=running([2048;2]);a.pump(2222,READY);wr(&mut a,CTRL2,CTRL2_BOOT,2222);
        wr(&mut a,PATT1,0xffffff,2222);wr(&mut a,PATT2,0xffffff,2222);
        let deinitialized=ClockInputs{reserved_path_ready:false,..READY};assert!(a.pump(2222,deinitialized));
        assert!(a.write_at(CLKM,FUNC_CLOCK|4<<12,2222,deinitialized));assert!(a.write_at(CTRL,CTRL_BOOT,2222,deinitialized));
        assert!(a.pump(22_222,deinitialized));assert_eq!(a.deadline(),None);assert_eq!(a.profile.as_ref().unwrap().triggers,1);assert!(!a.clock_fault);
    }
    #[test]
    fn reset_domains_preserve_cpu_and_lp_but_core_clears_digital_state(){
        let mut a=running([2048;2]);a.pump(2222,READY);let before=a.clone();a.reset_domains(2222,false,false,false);assert_eq!(a,before);
        a.reset_domains(2222,false,true,false);assert_eq!(a,before);a.reset_domains(2222,true,false,false);
        assert_eq!((a.conf,a.clkm,a.ctrl,a.ctrl2),(1,0x4000,CTRL_POR,CTRL2_POR));assert_eq!(a.deadline(),None);assert_eq!(a.profile.as_ref().unwrap().triggers,0);
        boot_config(&mut a,2222);wr(&mut a,CTRL2,CTRL2_BOOT|TIMER,2222);assert_eq!(a.deadline(),Some(4222));
        a.reset_domains(2222,false,false,true);assert!(a.profile.is_none());assert_eq!(a.deadline(),None);
    }
    #[test]
    fn both_real_reset_pulses_converge_on_defaults_without_manufactured_done(){
        for value in [7,13]{let mut a=running([2048;2]);a.pump(2222,READY);wr(&mut a,CONF,value,2222);
            assert_eq!(a.ctrl2,CTRL2_POR);assert_eq!(a.profile.as_ref().unwrap().last,[None;2]);assert_eq!(a.deadline(),None);
            assert!(!a.write_at(CTRL,CTRL_BOOT,2222,READY));assert_eq!(a.read_at(CONF,2222,READY),Some(value));
        }
    }
    #[test]
    fn only_six_exact_configuration_addresses_exist(){
        let mut a=running([2048;2]);for address in [0,CTRL+1,CTRL+8,CTRL+0xc,CTRL+0x10,CTRL+0x14,CTRL+0x20,CTRL+0x2c,CTRL+0x30,
            CTRL+0x40,CTRL+0x44,CTRL+0x48,CTRL+0x4c,CTRL+0x50,CTRL+0x54,CONF-4,CLKM+4,u32::MAX]{
            let before=a.clone();assert!(!BootSarAdc::contains(address));assert_eq!(a.read_at(address,3000,READY),None);assert!(!a.write_at(address,u32::MAX,3000,READY));assert_eq!(a,before);
        }
    }
    #[test]
    fn huge_jumps_count_in_constant_work_and_keep_checked_deadlines(){
        let mut a=running([0,4095]);assert!(a.pump(u64::MAX,READY));let p=a.profile.as_ref().unwrap();let n=u64::MAX/2000;
        assert_eq!(p.triggers,n);assert_eq!(p.captures,[n.div_ceil(2),n/2]);assert_eq!(p.last_at,Some(n*2000));assert_eq!(a.deadline(),None);
        let before=a.clone();a.pump(u64::MAX,READY);assert_eq!(a,before);
    }
}
