//! C5 LP_TIMER software capture only: fixed IDF5.5.3/5.5.4 headers/LL,
//! TRM1.1 chapter17. Counter48 runs from the supplied slow source; there
//! is no C5 VALID bit. DATE.CLK_EN is a register gate, not counter enable.
//! Epoch0 and unknown-source freezing are explicit functional profile limits.
const BASE:u32=0x600b_0c00;const UPDATE:u32=1<<27;
const TIMEBASE:u128=240_000_000;const MASK:u64=(1u64<<48)-1;
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct LpTimer{
    selected:bool,now:u64,hz:Option<u32>,count:u64,phase:u128,buffer:[u64;2],captures:u64,wraps:u64,
}
impl LpTimer{
    pub fn new(cycle:u64)->Self{Self{selected:false,now:cycle,hz:None,count:0,phase:0,buffer:[0;2],captures:0,wraps:0}}
    /// Select at the validated ROM epoch, before the actual source becomes
    /// known. Re-selection preserves history; no elapsed ticks are backfilled.
    pub fn enable_virtual_profile(&mut self,cycle:u64){if !self.selected{self.now=self.now.max(cycle);self.selected=true;}}
    pub fn disable_virtual_profile(&mut self){self.selected=false;self.hz=None;}
    pub fn virtual_profile_enabled(&self)->bool{self.selected}
    pub fn contains(address:u32)->bool{matches!(address,0x600b_0c10|0x600b_0c14|0x600b_0c18|0x600b_0c1c|0x600b_0c20)}
    pub fn set_slow_clock(&mut self,cycle:u64,hz:Option<u32>)->bool{
        if !self.virtual_profile_enabled()||hz.is_some_and(|v|v==0||u128::from(v)>TIMEBASE){return false;}
        self.pump(cycle);self.hz=hz;true
    }
    pub fn pump(&mut self,cycle:u64){
        if !self.selected{return;}
        let cycle=cycle.max(self.now);
        if let Some(hz)=self.hz{
            let ticks=self.phase+u128::from(cycle-self.now)*u128::from(hz);self.phase=ticks%TIMEBASE;
            let total=u128::from(self.count)+ticks/TIMEBASE;self.count=(total&u128::from(MASK))as u64;
            self.wraps=self.wraps.saturating_add(u64::try_from(total>>48).unwrap_or(u64::MAX));
        }
        self.now=cycle;
    }
    pub fn read_at(&mut self,address:u32,cycle:u64)->Option<u32>{
        if !self.selected||!Self::contains(address){return None;}self.pump(cycle);
        Some(match address-BASE{0x10=>0,0x14=>self.buffer[0]as u32,0x18=>(self.buffer[0]>>32)as u32,
            0x1c=>self.buffer[1]as u32,0x20=>(self.buffer[1]>>32)as u32,_=>return None})
    }
    pub fn write_at(&mut self,address:u32,value:u32,cycle:u64)->bool{
        if !self.selected||!Self::contains(address){return false;}
        // Unknown event selectors/reserved bits and missing-source UPDATE
        // reject before pumping, shifting a buffer or recording a capture.
        if address==BASE+0x10&&(value&!UPDATE!=0||value&UPDATE!=0&&self.hz.is_none()){return false;}
        self.pump(cycle);
        if address==BASE+0x10&&value&UPDATE!=0{
            self.buffer[1]=self.buffer[0];self.buffer[0]=self.count;self.captures=self.captures.saturating_add(1);
        }
        true // The four capture registers are RO; writes have no latch effect.
    }
    /// TRM17.3: counter loops except during LP system reset. HP CPU/Core
    /// preserve it. LP reset creates virtual epoch0; Chip also removes the
    /// analog source observation. The clock owner refreshes retained LP input.
    pub fn reset_domains(&mut self,cycle:u64,lp:bool,analog:bool){
        self.pump(cycle);if !lp&&!analog{return;}
        let selected=self.selected;let hz=if analog{None}else{self.hz};*self=Self::new(cycle);self.selected=selected;self.hz=hz;
    }
    pub fn report(&self)->String{if !self.selected{String::new()}else{format!("LP_TIMER=virtual_epoch0 slow_hz={:?} counter48={} at={} captures={} wraps={} snapshot=software_only",self.hz,self.count,self.now,self.captures,self.wraps)}}
}

#[cfg(test)]
mod tests{
    use super::*;
    fn active(hz:Option<u32>)->LpTimer{let mut t=LpTimer::new(0);t.enable_virtual_profile(0);assert!(t.set_slow_clock(0,hz));t}
    fn snapshot(t:&mut LpTimer,cycle:u64)->u64{
        assert_eq!(t.read_at(BASE+0x10,cycle),Some(0));assert!(t.write_at(BASE+0x10,UPDATE,cycle));
        assert_eq!(t.read_at(BASE+0x10,cycle),Some(0));u64::from(t.read_at(BASE+0x14,cycle).unwrap())|u64::from(t.read_at(BASE+0x18,cycle).unwrap())<<32
    }
    #[test]fn strict_profile_and_unknown_source_do_not_fabricate_current_time(){
        let mut t=LpTimer::new(0);assert!(!t.virtual_profile_enabled());assert_eq!(t.report(),"");let before=t.clone();
        t.pump(u64::MAX);assert_eq!(t,before);assert_eq!(t.read_at(BASE+0x10,50),None);assert!(!t.write_at(BASE+0x10,UPDATE,50));assert!(!t.set_slow_clock(50,Some(136_000)));assert_eq!(t,before);
        t.enable_virtual_profile(0);assert!(t.virtual_profile_enabled());t.pump(400_000);let before=t.clone();
        assert!(!t.write_at(BASE+0x10,UPDATE,796_224));assert_eq!(t,before);assert_eq!(t.captures,0);
        assert!(t.set_slow_clock(400_000,Some(136_000)));assert_eq!(snapshot(&mut t,796_224),224);assert_eq!(t.captures,1);
    }
    #[test]fn original_bit27_sequence_captures_real_count_without_valid_or_enable(){
        let mut t=active(Some(136_000));assert_eq!(t.read_at(BASE+0x14,0),Some(0));assert_eq!(t.read_at(BASE+0x18,0),Some(0));
        assert_eq!(snapshot(&mut t,240_000_000),136_000);assert_eq!(t.captures,1);
        let before=t.clone();for _ in 0..8{assert_eq!(t.read_at(BASE+0x10,240_000_000),Some(0));assert_eq!(t.read_at(BASE+0x14,240_000_000),Some(136_000));}assert_eq!(t,before);
        t.pump(480_000_000);assert_eq!(t.count,272_000);assert_eq!(t.read_at(BASE+0x14,480_000_000),Some(136_000));assert_eq!(t.captures,1);
    }
    #[test]fn captures_shift_two_real_48_bit_values_and_writes_cannot_change_ro_buffers(){
        let mut t=active(Some(240_000_000));let a=(1u64<<32)+7;assert_eq!(snapshot(&mut t,a),a);
        let b=(2u64<<32)+13;assert_eq!(snapshot(&mut t,b),b);
        assert_eq!(t.read_at(BASE+0x1c,b),Some(7));assert_eq!(t.read_at(BASE+0x20,b),Some(1));assert_eq!(t.read_at(BASE+0x18,b),Some(2));
        for off in[0x14,0x18,0x1c,0x20]{assert!(t.write_at(BASE+off,u32::MAX,b));}assert_eq!(t.buffer,[b,a]);assert_eq!(t.captures,2);
        assert_eq!(snapshot(&mut t,b+1),b+1);assert_eq!(t.buffer,[b+1,b]);
    }
    #[test]fn counter_wraps_at48_bits_in_constant_work_and_captures_keep_history(){
        let mut t=active(Some(240_000_000));assert_eq!(snapshot(&mut t,MASK-1),MASK-1);assert_eq!(t.read_at(BASE+0x18,MASK-1),Some(0xffff));
        assert_eq!(snapshot(&mut t,MASK+4),3);assert_eq!(t.wraps,1);assert_eq!(t.read_at(BASE+0x18,MASK+4),Some(0));assert_eq!(t.buffer[1],MASK-1);
        let mut huge=active(Some(136_000));huge.pump(u64::MAX);let ticks=u128::from(u64::MAX)*136_000/240_000_000;
        assert_eq!(huge.count,(ticks% (1u128<<48))as u64);assert_eq!(u128::from(huge.wraps),ticks>>48);
        let before=huge.clone();huge.pump(0);assert_eq!(huge,before);
    }
    #[test]fn rate_changes_integrate_old_ticks_and_preserve_fractional_phase(){
        let mut t=active(Some(3));assert!(t.set_slow_clock(40_000_000,Some(6)));assert_eq!(t.count,0);
        assert_eq!(snapshot(&mut t,60_000_000),1);assert!(t.set_slow_clock(80_000_000,None));let count=t.count;let phase=t.phase;
        t.pump(1_000_000_000);assert_eq!((t.count,t.phase),(count,phase));let before=t.clone();assert!(!t.write_at(BASE+0x10,UPDATE,u64::MAX));assert_eq!(t,before);
        assert!(t.set_slow_clock(1_000_000_000,Some(6)));assert_eq!(snapshot(&mut t,1_020_000_000),2);
    }
    #[test]fn arbitrary_pump_chunks_preserve_exact_source_phase_and_values(){
        let mut once=active(Some(136_000));let mut chunks=once.clone();let end=123_456_789;
        once.pump(end);for at in[1,100,17_000,423_296,796_224,10_000_000,end]{chunks.pump(at);}assert_eq!(once,chunks);
        assert_eq!(snapshot(&mut once,end),snapshot(&mut chunks,end));assert_eq!(once,chunks);
    }
    #[test]fn unknown_fields_engines_and_invalid_rates_reject_before_mutation(){
        let mut t=active(Some(136_000));snapshot(&mut t,100_000);
        for address in[BASE,BASE+4,BASE+8,BASE+12,BASE+0x11,BASE+0x24,BASE+0x28,BASE+0x30,BASE+0x3fc,0,u32::MAX]{
            let before=t.clone();assert!(!LpTimer::contains(address));assert_eq!(t.read_at(address,1_000_000),None);assert!(!t.write_at(address,u32::MAX,1_000_000));assert_eq!(t,before);
        }
        for value in[1,1<<26,1<<28,1<<29,1<<30,1<<31,UPDATE|1,UPDATE|(1<<28),u32::MAX]{let before=t.clone();assert!(!t.write_at(BASE+0x10,value,1_000_000));assert_eq!(t,before);}
        for hz in[0,240_000_001,u32::MAX]{let before=t.clone();assert!(!t.set_slow_clock(1_000_000,Some(hz)));assert_eq!(t,before);}
        let captures=t.captures;assert!(t.write_at(BASE+0x10,0,100_000));assert_eq!(t.captures,captures);
    }
    #[test]fn cpu_and_core_preserve_running_count_phase_and_captured_values(){
        let mut t=active(Some(136_000));snapshot(&mut t,1_000_000);let mut expected=t.clone();expected.pump(1_000_001);
        t.reset_domains(1_000_001,false,false);assert_eq!(t,expected);let buffers=t.buffer;
        t.reset_domains(2_000_000,false,false);assert_eq!(t.buffer,buffers);assert!(t.count>buffers[0]);
    }
    #[test]fn lp_reset_clears_counter_and_buffers_chip_also_withdraws_analog_source(){
        let mut t=active(Some(136_000));snapshot(&mut t,240_000_000);t.reset_domains(240_000_000,true,false);
        assert_eq!(t.buffer,[0;2]);assert_eq!((t.count,t.phase,t.captures,t.wraps),(0,0,0,0));assert_eq!(t.hz,Some(136_000));assert!(t.virtual_profile_enabled());
        assert_eq!(snapshot(&mut t,480_000_000),136_000);t.reset_domains(480_000_000,true,true);assert_eq!(t.hz,None);assert_eq!(t.buffer,[0;2]);
        let before=t.clone();assert!(!t.write_at(BASE+0x10,UPDATE,1_000_000_000));assert_eq!(t,before);
        assert!(t.set_slow_clock(1_000_000_000,Some(136_000)));assert_eq!(snapshot(&mut t,1_240_000_000),136_000);
    }
    #[test]fn selection_is_idempotent_and_legacy_mode_has_no_autonomous_counter(){
        let mut t=active(Some(136_000));snapshot(&mut t,240_000_000);let before=t.clone();t.enable_virtual_profile(480_000_000);assert_eq!(t,before);
        let buffers=t.buffer;t.disable_virtual_profile();let before=t.clone();t.pump(u64::MAX);assert_eq!(t,before);assert_eq!(t.read_at(BASE+0x14,u64::MAX),None);assert_eq!(t.report(),"");
        t.enable_virtual_profile(480_000_000);assert_eq!(t.buffer,buffers);assert_eq!(t.hz,None);assert!(t.set_slow_clock(480_000_000,Some(136_000)));
        assert_eq!(snapshot(&mut t,720_000_000),272_000);assert!(t.report().contains("captures=2"));
    }
}
