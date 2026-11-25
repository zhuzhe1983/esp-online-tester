//! C5 PCR configuration, fixed IDF5.5.3/5.5.4 clk_tree_ll.h/pcr_reg.h.
//! A functional DFS commit does not change the nominal240MHz CPU scheduler.
use crate::clock_tree::{ClockInputs, ClockTree, Root};
pub(crate) struct BootClock{debug:bool,fpga_debug:u32,root:Root,profile:Option<Box<ClockTree>>,sec_source:u8,sec_pll_ready:bool}
impl BootClock{
    pub fn new()->Self{Self{debug:true,fpga_debug:u32::MAX,root:Root::default(),profile:None,sec_source:0,sec_pll_ready:false}}
    pub fn read(&self,offset:u32,xtal_hz:u32)->Option<u32>{Some(match offset{
        0x110=>(u32::from(self.debug)<<31)|((xtal_hz/1_000_000)&127)<<24|u32::from(self.root.source)<<16|2<<8,
        0x118=>u32::from(self.root.cpu),0x11c=>u32::from(self.root.ahb),
        0x120=>u32::from(self.root.apb)<<8|u32::from(self.root.decrease),_=>return None,
    })}
    fn stage(&mut self,offset:u32,value:u32,virtual_sources:bool)->bool{match offset{
        0x110=>{let source=((value>>16)&3)as u8;if !virtual_sources&&source!=0{return false;}self.root.source=source;self.debug=value&(1<<31)!=0;},
        0x118=>self.root.cpu=value as u8,0x11c=>self.root.ahb=value as u8,
        0x120=>{self.root.apb=(value>>8)as u8;self.root.decrease=value as u8;},_=>return false,
    }true}
    pub fn write(&mut self,offset:u32,value:u32)->bool{self.stage(offset,value,false)}
    pub fn enable_virtual_profile(&mut self,cycle:u64){if self.profile.is_none(){self.profile=Some(Box::new(ClockTree::new(cycle,self.root)));}}
    pub fn disable_virtual_profile(&mut self){if self.profile.take().is_some(){self.root=Root::default();self.debug=true;self.fpga_debug=u32::MAX;self.sec_source=0;self.sec_pll_ready=false;}}
    pub fn virtual_profile_enabled(&self)->bool{self.profile.is_some()}
    pub fn contains(address:u32)->bool{matches!(address,0x6009_6110|0x6009_6118|0x6009_611c|0x6009_6120|0x6009_6128|0x6009_6130|0x6009_613c|0x6009_6144|0x6009_6160|0x6009_6054|0x6009_6060|0x6009_6ff4|0x600b_0400|0x600b_0404|0x600b_0418|0x600b_0420|0x6000_8068|0x6000_806c|0x6000_8080)}
    /// Integrate previous inputs first; new analog observations take effect at
    /// this boundary. Pump on both sides of analog writes at the same cycle.
    pub fn pump(&mut self,cycle:u64,inputs:ClockInputs){if let Some(p)=&mut self.profile{p.pump(cycle,inputs);self.sec_pll_ready=inputs.pll_ready;}}
    pub fn deadline(&self)->Option<u64>{self.profile.as_ref()?.deadline()}
    pub fn read_at(&mut self,address:u32,cycle:u64,inputs:ClockInputs)->Option<u32>{
        self.profile.as_ref()?;self.pump(cycle,inputs);
        if address==0x6009_6ff4{return Some(self.fpga_debug);}
        if address==0x6009_613c{return Some(u32::from(self.sec_source));}
        if matches!(address,0x6009_6110|0x6009_6118|0x6009_611c|0x6009_6120){return self.read(address-0x6009_6000,48_000_000);}
        self.profile.as_ref()?.read(address)
    }
    pub fn write_at(&mut self,address:u32,value:u32,cycle:u64,inputs:ClockInputs)->bool{
        if self.profile.is_none()||address==0x6009_613c&&!matches!(value,0|2){return false;}self.pump(cycle,inputs);
        match address{
            // Fixed IDF5.5.3/5.5.4 pcr_reg.h/struct: SEL[1:0], POR0;
            // original42002542/54a RMW selects2=direct PLL480. Only0/2
            // are qualified. SEC reset2, RC_FAST1, selector3 and reserved
            // requests reject; this does not implement a security consumer.
            0x6009_613c=>{self.sec_source=value as u8;true},
            // Fixed IDF5.5.3/5.5.4 pcr_reg.h: FPGA_DEBUG PORffffffff.
            // Only the original silicon init's MSB toggle is qualified here;
            // no FPGA behavior or meaning for its other31 bits is invented.
            0x6009_6ff4=>{if (value^self.fpga_debug)&0x7fff_ffff!=0{return false;}self.fpga_debug=value;true},
            0x6009_6110|0x6009_6118|0x6009_611c|0x6009_6120=>{
                if address==0x6009_6120&&(value&!0xffff!=0||!matches!(value&255,0|1|3)||!matches!((value>>8)&255,0|1|3)){return false;}
                if address==0x6009_6110{
                    let needed=match(value>>16)&3{2=>2,3=>1,_=>0};
                    if needed!=0&&self.profile.as_ref().unwrap().pll_div_gate_mask()&needed==0{return false;}
                }
                let ok=self.stage(address-0x6009_6000,value,true);
                // BUS_CLOCK_UPDATE applies root/CPU/AHB. APB divider fields
                // have no such trigger and apply to the active AHB directly.
                if ok&&address==0x6009_6120{self.profile.as_mut().unwrap().set_apb(self.root.apb,self.root.decrease);}ok
            },
            0x6009_6144=>{if value&!1!=0{return false;}value==0||self.profile.as_mut().unwrap().commit(self.root)},
            _=>self.profile.as_mut().unwrap().write(address,value),
        }
    }
    pub fn cpu_hz(&self)->Option<u32>{self.profile.as_ref()?.cpu_hz()}
    pub fn cpu_uses_xtal(&self)->bool{self.profile.as_ref().is_some_and(|p|p.cpu_uses_xtal())}
    pub fn ahb_hz(&self)->Option<u32>{self.profile.as_ref()?.ahb_hz()}
    pub fn apb_hz(&self)->Option<u32>{self.profile.as_ref()?.apb_hz()}
    pub fn rc_fast_hz(&self)->Option<u32>{self.profile.as_ref()?.rc_fast_hz()}
    pub fn rc_fast_osc_hz(&self)->Option<u32>{self.profile.as_ref()?.rc_fast_osc_hz()}
    pub fn rc_slow_osc_hz(&self)->Option<u32>{self.profile.as_ref()?.rc_slow_osc_hz()}
    pub fn pll_div_gate_mask(&self)->Option<u16>{self.profile.as_ref().map(|p|p.pll_div_gate_mask())}
    pub fn pll_tap_hz(&self,mhz:u16)->Option<u32>{self.profile.as_ref()?.pll_tap_hz(mhz)}
    pub fn sec_clock_source(&self)->Option<u8>{self.profile.as_ref().map(|_|self.sec_source)}
    /// Pre-DPA SEC input source, independent of PCR+128 divider taps.
    /// This is not the downstream CRYPTO/KEYMNG functional rate: TRM9.2
    /// limits CRYPTO_CLK to160MHz through a separate random DPA divider.
    /// A mux write grants neither PLL readiness nor consumer functionality.
    pub fn sec_source_hz(&self)->Option<u32>{match self.sec_clock_source()?{0=>Some(48_000_000),2 if self.sec_pll_ready=>Some(480_000_000),_=>None}}
    pub fn rtc_slow_hz(&self)->Option<u32>{self.profile.as_ref()?.rtc_slow_hz()}
    pub fn timg0_control(&self)->u32{self.profile.as_ref().map_or(1,|p|p.timg0_control())}
    pub fn timg1_control(&self)->u32{self.profile.as_ref().map_or(1,|p|p.timg1_control())}
    pub fn rtc_fast_hz(&self)->Option<u32>{self.profile.as_ref()?.rtc_fast_hz()}
    pub fn fosc_dfreq(&self)->Option<u16>{self.profile.as_ref().map(|p|p.fosc_dfreq())}
    pub fn report(&self)->String{self.profile.as_ref().map_or_else(String::new,|p|{
        let available=[240,160,120,80,60,48,40,20,12].into_iter().enumerate().fold(0u16,|mask,(bit,mhz)|mask|u16::from(self.pll_tap_hz(mhz).is_some())<<bit);
        let mut report=format!("clock_tree=virtual scheduler=nominal240MHz cpu_hz={:?} ahb_hz={:?} apb_hz={:?} hp_rc_fast_hz={:?} lp_fast_hz={:?} lp_slow_hz={:?} fosc_dfreq={:?} pll_div_gates={:#x} pll_taps_available={available:#x} raw_rc_fast_hz={:?} raw_rc_slow_hz={:?} {}",self.cpu_hz(),self.ahb_hz(),self.apb_hz(),self.rc_fast_hz(),self.rtc_fast_hz(),self.rtc_slow_hz(),self.fosc_dfreq(),self.pll_div_gate_mask().unwrap_or(0),self.rc_fast_osc_hz(),self.rc_slow_osc_hz(),p.report());
        if self.sec_clock_source()==Some(2){report.push_str(&format!(" SEC_input_mux=PLL480 pre_DPA_source_hz={:?} consumer=unmodeled",self.sec_source_hz()));}report})}
    /// CPU-only retains all state. HP resets PCR; LP resets LP mux/tuning.
    /// Analog reset includes both. Explicit profile selection is retained.
    pub fn reset_domains(&mut self,cycle:u64,hp:bool,lp:bool,analog:bool){
        if hp||analog{self.root=Root::default();self.debug=true;self.fpga_debug=u32::MAX;self.sec_source=0;}
        if analog{self.sec_pll_ready=false;}
        if let Some(p)=&mut self.profile{p.reset_domains(cycle,hp,lp,analog);}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn fpga_debug_msb_is_separate_from_the_cpu_clock_and_limited_to_qualified_configuration(){
        let mut c=BootClock::new();let inputs=ClockInputs::default();assert_eq!(c.read_at(0x6009_6ff4,0,inputs),None);
        c.enable_virtual_profile(0);let root=c.read(0x110,48_000_000);let rate=c.cpu_hz();assert_eq!(c.read_at(0x6009_6ff4,0,inputs),Some(u32::MAX));
        assert!(c.write_at(0x6009_6ff4,0x7fff_ffff,0,inputs));assert_eq!(c.read_at(0x6009_6ff4,0,inputs),Some(0x7fff_ffff));assert_eq!(c.read(0x110,48_000_000),root);assert_eq!(c.cpu_hz(),rate);
        assert!(!c.write_at(0x6009_6ff4,0x7fff_fffe,0,inputs));assert_eq!(c.read_at(0x6009_6ff4,0,inputs),Some(0x7fff_ffff));
        c.reset_domains(0,false,true,false);assert_eq!(c.read_at(0x6009_6ff4,0,inputs),Some(0x7fff_ffff));c.reset_domains(0,true,false,false);assert_eq!(c.read_at(0x6009_6ff4,0,inputs),Some(u32::MAX));
        assert!(c.write_at(0x6009_6ff4,0x7fff_ffff,0,inputs));c.disable_virtual_profile();assert_eq!(c.read_at(0x6009_6ff4,0,inputs),None);c.enable_virtual_profile(0);assert_eq!(c.read_at(0x6009_6ff4,0,inputs),Some(u32::MAX));
    }
    #[test]
    fn xtal_observation_uses_committed_source_even_when_pll_divider_has_the_same_rate(){
        let mut c=BootClock::new();assert!(!c.cpu_uses_xtal());c.enable_virtual_profile(0);assert!(c.cpu_uses_xtal());
        let input=ClockInputs{pll_ready:true,..ClockInputs::default()};
        for(a,v)in[(0x6009_6110,3<<16),(0x6009_6118,4),(0x6009_611c,4)]{assert!(c.write_at(a,v,1,input));}
        assert!(c.cpu_uses_xtal(),"staged root has not committed");assert!(c.write_at(0x6009_6144,1,2,input));
        assert_eq!(c.cpu_hz(),Some(48_000_000));assert!(!c.cpu_uses_xtal(),"PLL240/5 must not count as XTAL");
        assert!(c.write_at(0x6009_6110,0,3,input));assert!(!c.cpu_uses_xtal());
        assert!(c.write_at(0x6009_6144,1,4,input));assert!(c.cpu_uses_xtal());
        c.disable_virtual_profile();assert!(!c.cpu_uses_xtal());
    }
    #[test]
    fn legacy_clock_remains_strict_and_apb_fields_have_actual_positions(){
        let mut c=BootClock::new();assert_eq!(c.read(0x110,48_000_000),Some(0xb000_0200));
        assert!(!c.write(0x110,3<<16));assert_eq!(c.read(0x110,48_000_000),Some(0xb000_0200));
        assert!(c.write(0x120,0x103));assert_eq!(c.read(0x120,48_000_000),Some(0x103));
        assert!(c.write(0x118,7));assert_eq!(c.read(0x118,48_000_000),Some(7));
        assert!(c.write(0x11c,15));assert_eq!(c.read(0x11c,48_000_000),Some(15));
        assert_eq!(c.read(0x114,48_000_000),None);assert!(!c.write(0x144,1));assert_eq!(c.cpu_hz(),None);
        assert!(!c.virtual_profile_enabled());assert_eq!(c.read_at(0x600b_0418,0,ClockInputs::default()),None);
    }
    #[test]
    fn staged_pll_needs_real_ready_and_a_valid_atomic_commit(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);let off=ClockInputs::default();
        assert_eq!(c.cpu_hz(),Some(48_000_000));assert!(c.write_at(0x6009_6118,2,1,off));
        assert!(c.write_at(0x6009_611c,5,1,off));assert!(c.write_at(0x6009_6110,0x8003_0000,1,off));
        assert_eq!(c.cpu_hz(),Some(48_000_000));assert!(!c.write_at(0x6009_6144,1,2,off));
        let on=ClockInputs{pll_ready:true,..off};assert!(c.write_at(0x6009_6144,1,3,on));
        assert_eq!((c.cpu_hz(),c.ahb_hz(),c.apb_hz()),(Some(80_000_000),Some(40_000_000),Some(40_000_000)));
        assert_eq!(c.read_at(0x6009_6144,4,on),Some(0));assert!(c.write_at(0x6009_6120,0x100,4,on));
        assert_eq!(c.apb_hz(),Some(20_000_000));assert!(c.write_at(0x6009_6144,1,4,on));
        assert!(c.write_at(0x6009_611c,0,4,on));assert!(!c.write_at(0x6009_6144,1,4,on));assert_eq!(c.ahb_hz(),Some(40_000_000));
        assert!(!c.write_at(0x6009_6120,0x200,4,on));assert_eq!(c.read_at(0x6009_6120,4,on),Some(0x100));
        assert!(c.report().contains("cpu_hz=Some(80000000)"));c.pump(5,off);assert_eq!(c.cpu_hz(),None);assert!(!c.write_at(0x6009_6144,2,5,off));
    }
    #[test]
    fn lp_mux_fields_and_domains_reset_independently(){
        let mut c=BootClock::new();let input=ClockInputs{rc_fast_powered:true,slow_dcap:Some(128),pll_ready:false};
        c.enable_virtual_profile(10);assert_eq!(c.read_at(0x600b_0418,10,input),Some(172<<22));
        assert_eq!(c.read_at(0x600b_0400,10,input),Some(4));assert_eq!(c.rtc_fast_hz(),Some(24_000_000));
        assert!(c.write_at(0x600b_0400,0,10,input));assert_eq!(c.rtc_fast_hz(),None,"POR DFREQ172 has no configured virtual RC rate");
        assert!(c.write_at(0x600b_0400,8,10,input));assert_eq!(c.rtc_fast_hz(),Some(48_000_000));assert!(!c.write_at(0x600b_0400,5,10,input));
        assert!(c.write_at(0x600b_0400,4,10,input));assert!(c.write_at(0x600b_0418,100<<22,10,input));
        assert_eq!(c.fosc_dfreq(),Some(100));assert!(!c.write_at(0x600b_0418,101<<22,10,input));
        assert_eq!(c.rtc_slow_hz(),Some(136_000));c.reset_domains(12,false,false,false);assert_eq!(c.fosc_dfreq(),Some(100));
        c.reset_domains(13,true,false,false);assert_eq!(c.fosc_dfreq(),Some(100));
        c.reset_domains(14,false,true,false);assert_eq!(c.fosc_dfreq(),Some(172));assert!(c.deadline().is_some());
        assert!(BootClock::contains(0x600b_0420));assert!(!BootClock::contains(0x600b_041c));
        assert!(c.write_at(0x600b_0420,0x7000_0000,14,input));assert_eq!(c.read_at(0x600b_0420,14,input),Some(0x7000_0000));
        assert!(!c.write_at(0x600b_0420,0,14,input));c.disable_virtual_profile();assert!(!c.virtual_profile_enabled());
        assert_eq!(c.fosc_dfreq(),None);assert_eq!(c.rtc_slow_hz(),None);assert_eq!(c.rtc_fast_hz(),None);
        assert!(!c.write_at(0x600b_0418,100<<22,15,input));assert_eq!(c.read(0x110,48_000_000),Some(0xb000_0200));
        c.enable_virtual_profile(20);c.reset_domains(21,false,false,true);assert_eq!(c.fosc_dfreq(),Some(172));
    }
    const INPUT:ClockInputs=ClockInputs{pll_ready:false,rc_fast_powered:true,slow_dcap:Some(128)};
    const CFG:u32=0x6000_8068;const VALUE:u32=0x6000_806c;const TIMEOUT:u32=0x6000_8080;
    fn w(c:&mut BootClock,address:u32,value:u32,cycle:u64){assert!(c.write_at(address,value,cycle,INPUT),"write {address:#x}={value:#x}");}
    fn r(c:&mut BootClock,address:u32,cycle:u64)->u32{c.read_at(address,cycle,INPUT).unwrap()}
    fn calibration(source:u32,divider:u32,maximum:u32)->BootClock{
        let mut c=BootClock::new();c.enable_virtual_profile(0);c.pump(0,INPUT);
        w(&mut c,CFG,1<<16,0);w(&mut c,0x6009_6130,source|((divider-1)<<8),0);
        w(&mut c,0x600b_0418,100<<22,0);w(&mut c,CFG,maximum<<16,0);w(&mut c,CFG,1<<31|maximum<<16,0);c
    }
    #[test]
    fn rtc_calibration_measures_actual_selected_edges_and_ro_bits_cannot_complete_it(){
        //17 edges of136kHz take125us:30000 nominal cycles,6000 XTAL ticks.
        let mut c=calibration(3,1,17);assert_eq!(c.deadline(),Some(30_000));
        assert_eq!(r(&mut c,CFG,29_999)&(1<<15),0);assert_eq!(r(&mut c,VALUE,29_999),0);
        assert_ne!(r(&mut c,CFG,30_000)&(1<<15),0);assert_eq!(r(&mut c,VALUE,30_000),6000<<7);
        assert_eq!(c.deadline(),None);w(&mut c,VALUE,u32::MAX,30_000);assert_eq!(r(&mut c,VALUE,60_000),6000<<7);
        assert_eq!(r(&mut c,TIMEOUT,60_000)&1,0);assert!(c.report().contains("measured=1 timeouts=0 running=0"));
        w(&mut c,CFG,17<<16|(1<<15),60_000);assert_eq!(r(&mut c,CFG,60_000)&(1<<15),0);
        w(&mut c,CFG,34<<16|1<<31,60_000);assert_eq!(r(&mut c,VALUE,60_000),6000<<7);
        assert_eq!(c.deadline(),Some(120_000));assert_eq!(r(&mut c,VALUE,120_000),12_000<<7);
    }
    #[test]
    fn rtc_calibration_uses_rc_fast_divider_without_rounding_its_source_frequency(){
        //35*(32/17.5MHz)=64us:15360 cycles,3072 XTAL ticks.
        let mut c=calibration(4,32,35);assert_eq!(c.deadline(),Some(15_360));
        assert_eq!(r(&mut c,CFG,15_359)&(1<<15),0);assert_eq!(r(&mut c,VALUE,15_360),3072<<7);
        w(&mut c,CFG,70<<16,15_360);w(&mut c,CFG,70<<16|1<<31,15_360);
        assert_eq!(c.deadline(),Some(46_080));assert_eq!(r(&mut c,VALUE,46_080),6144<<7);
    }
    #[test]
    fn rtc_calibration_unknown_unwritten_analog_input_times_out_and_restart_is_real(){
        let mut c=BootClock::new();let absent=ClockInputs::default();c.enable_virtual_profile(0);
        assert!(c.write_at(CFG,17<<16,0,absent));assert!(c.write_at(0x6009_6130,3,0,absent));
        assert!(c.write_at(TIMEOUT,9<<7|3<<3,0,absent));assert!(c.write_at(CFG,17<<16|1<<31,0,absent));
        assert_eq!(c.deadline(),Some(50));assert_eq!(c.read_at(TIMEOUT,49,absent).unwrap()&1,0);
        assert_eq!(c.read_at(TIMEOUT,50,absent).unwrap()&1,1);assert_eq!(c.read_at(CFG,50,absent).unwrap()&(1<<15),0);
        assert_eq!(c.read_at(VALUE,100_000,absent),Some(0));assert_eq!(c.rtc_slow_hz(),None);
        w(&mut c,CFG,17<<16,100_000);w(&mut c,TIMEOUT,0xffff_ff98,100_000);w(&mut c,CFG,17<<16|1<<31,100_000);
        assert_eq!(r(&mut c,TIMEOUT,100_000)&1,0);assert_eq!(c.deadline(),Some(130_000));assert_eq!(r(&mut c,VALUE,130_000),6000<<7);
    }
    #[test]
    fn rtc_reference_gate_pauses_counts_but_apb_gate_only_blocks_access(){
        let mut c=calibration(3,1,17);w(&mut c,0x6009_6160,0,15_000);assert_eq!(c.deadline(),None);
        c.pump(40_000,INPUT);assert_eq!(r(&mut c,VALUE,40_000),0);w(&mut c,0x6009_6160,4,40_000);
        assert_eq!(c.deadline(),Some(55_000));assert_eq!(r(&mut c,VALUE,54_999),0);assert_eq!(r(&mut c,VALUE,55_000),6000<<7);
        let mut c=calibration(3,1,17);w(&mut c,0x6009_6054,0,0);
        assert_eq!(c.read_at(CFG,15_000,INPUT),None);assert!(!c.write_at(CFG,17<<16,15_000,INPUT));
        c.pump(30_000,INPUT);w(&mut c,0x6009_6054,1,30_000);assert_eq!(r(&mut c,VALUE,30_000),6000<<7);
        w(&mut c,0x6009_6054,3,30_000);assert_eq!(c.deadline(),None);assert_eq!(r(&mut c,VALUE,30_000),0);
        assert!(!c.write_at(CFG,17<<16,30_000,INPUT));w(&mut c,0x6009_6054,1,30_000);assert_eq!(r(&mut c,0x6009_6054,30_000),0x1d);
    }
    #[test]
    fn rtc_source_loss_and_analog_boundary_inputs_never_apply_retroactively(){
        let mut c=calibration(3,1,17);let absent=ClockInputs{slow_dcap:None,..INPUT};
        c.pump(15_000,absent);c.pump(30_000,absent);assert_eq!(c.read_at(CFG,30_000,absent).unwrap()&(1<<15),0);
        c.pump(30_000,INPUT);assert_eq!(c.deadline(),Some(45_000));assert_eq!(r(&mut c,VALUE,45_000),9000<<7);
        let mut c=calibration(4,32,35);w(&mut c,0x600b_0420,0x7000_0000,0);w(&mut c,TIMEOUT,9<<7,0);
        assert_eq!(c.deadline(),Some(50));assert_eq!(r(&mut c,TIMEOUT,50)&1,1);assert_eq!(r(&mut c,VALUE,50),0);
    }
    #[test]
    fn rtc_periodic_large_catch_up_and_irregular_pumps_have_identical_results(){
        let mut a=calibration(3,1,17);let mut b=calibration(3,1,17);
        for c in [&mut a,&mut b]{w(c,CFG,17<<16,0);w(c,CFG,17<<16|1<<12,0);}
        let end=30_000_000_123;a.pump(end,INPUT);
        for cycle in [1,37,29_999,30_000,1_234_567,end]{b.pump(cycle,INPUT);}
        assert_eq!(r(&mut a,CFG,end)&(1<<15),0);assert_eq!(r(&mut a,VALUE,end),6000<<7|1);
        assert_eq!(a.report(),b.report());assert!(a.report().contains("measured=1000000 timeouts=0"));
        assert_eq!(a.deadline(),Some(30_000_030_000));
        let mut c=BootClock::new();c.enable_virtual_profile(0);w(&mut c,TIMEOUT,9<<7|3<<3,0);
        c.pump(65_000_000,INPUT);assert!(c.report().contains("measured=0 timeouts=1000000"));assert_eq!(c.deadline(),Some(65_000_050));
    }
    #[test]
    fn rtc_rejects_invalid_controls_and_disable_removes_all_deadlines(){
        let mut c=calibration(3,1,17);let before=r(&mut c,CFG,0);
        for (address,value) in [(CFG,1<<31),(CFG,(17<<16)|(1<<31)|(1<<12)),(CFG,before|1),(TIMEOUT,6),(0x6009_6130,5),(0x6009_6160,6),(0x6009_6054,0x80),(0x6000_8068+1,1)]{
            assert!(!c.write_at(address,value,0,INPUT),"accepted {address:#x}={value:#x}");
        }
        assert_eq!(r(&mut c,CFG,0),before);assert!(!BootClock::contains(0x6000_8000));assert!(BootClock::contains(CFG));
        c.reset_domains(10_000,false,false,false);assert_eq!(c.deadline(),Some(30_000));
        c.reset_domains(10_000,false,true,false);assert_eq!(c.deadline(),Some(30_000));
        c.reset_domains(10_000,true,false,false);assert_eq!(r(&mut c,CFG,10_000),0x0001_1000);assert_eq!(r(&mut c,VALUE,10_000),0);
        c.pump(u64::MAX,INPUT);assert_eq!(c.deadline(),None);c.disable_virtual_profile();assert_eq!(c.deadline(),None);assert_eq!(c.report(),"");
    }
    #[test]
    fn rtc_nonintegral_period_windows_preserve_the_same_results_under_chunking(){
        for (source,divider,maximum) in [(3,1,1),(3,1,3),(4,32,7),(4,32,35)]{
            let mut a=calibration(source,divider,maximum);let mut b=calibration(source,divider,maximum);
            for c in [&mut a,&mut b]{w(c,CFG,maximum<<16,0);w(c,CFG,maximum<<16|1<<12,0);}
            let end=1_000_000_007;a.pump(end,INPUT);
            for cycle in [1,99,1733,1764,1765,2048,64_001,1_100_003,end]{b.pump(cycle,INPUT);}
            assert_eq!(a.report(),b.report(),"source{source} max{maximum}");assert_eq!(a.deadline(),b.deadline());
            assert_eq!(r(&mut a,VALUE,end),r(&mut b,VALUE,end));
        }
    }
    #[test]
    fn cpu_rc_source_needs_power_tuning_and_hp_gate_and_timeout_rethresholds_actual_count(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);let off=ClockInputs::default();
        assert!(c.write_at(0x600b_0418,100<<22,0,off));assert!(c.write_at(0x6009_6110,0x8001_0000,0,off));
        assert!(!c.write_at(0x6009_6144,1,0,off));assert_eq!(c.cpu_hz(),Some(48_000_000));
        assert!(c.write_at(0x6009_6144,1,0,INPUT));assert_eq!(c.cpu_hz(),Some(17_500_000));assert_eq!(c.rc_fast_hz(),Some(17_500_000));
        w(&mut c,0x600b_0420,0x7000_0000,0);assert_eq!(c.cpu_hz(),None);assert_eq!(c.rc_fast_hz(),None);
        let mut c=calibration(3,1,17);c.pump(50,INPUT);w(&mut c,TIMEOUT,0,50);
        assert_eq!(c.deadline(),Some(50));assert_eq!(r(&mut c,TIMEOUT,51)&1,1);assert_eq!(r(&mut c,CFG,51)&(1<<15),0);
    }
    const PLL_GATES:u32=0x6009_6128;
    const PLL_ON:ClockInputs=ClockInputs{pll_ready:true,rc_fast_powered:false,slow_dcap:None};
    fn select_pll(c:&mut BootClock,source:u32,cpu:u32,ahb:u32){
        for(a,v)in[(0x6009_6118,cpu),(0x6009_611c,ahb),(0x6009_6110,source<<16),(0x6009_6144,1)]{
            assert!(c.write_at(a,v,0,PLL_ON),"write {a:#x}={v:#x}");
        }
    }
    #[test]
    fn pll_named_taps_need_actual_ready_and_only_their_own_gate(){
        let mut c=BootClock::new();assert_eq!(c.pll_div_gate_mask(),None);assert_eq!(c.pll_tap_hz(240),None);c.enable_virtual_profile(0);
        assert_eq!(c.read_at(PLL_GATES,0,ClockInputs::default()),Some(0x1ff));assert_eq!(c.pll_div_gate_mask(),Some(0x1ff));
        for mhz in[240,160,120,80,60,48,40,20,12]{assert_eq!(c.pll_tap_hz(mhz),None);}
        c.pump(0,PLL_ON);for(bit,mhz)in[240,160,120,80,60,48,40,20,12].into_iter().enumerate(){
            assert_eq!(c.pll_tap_hz(mhz),Some(u32::from(mhz)*1_000_000));
            if bit==5{continue;}assert!(c.write_at(PLL_GATES,0x1ff&!(1<<bit),0,PLL_ON));assert_eq!(c.pll_tap_hz(mhz),None);
            assert_eq!(c.cpu_hz(),Some(48_000_000));assert!(c.write_at(PLL_GATES,0x1ff,0,PLL_ON));
        }
        for mhz in[0,1,24,96,320,480,u16::MAX]{assert_eq!(c.pll_tap_hz(mhz),None);}
        c.pump(0,ClockInputs::default());assert_eq!(c.pll_div_gate_mask(),Some(0x1ff));assert_eq!(c.pll_tap_hz(80),None);
        assert!(c.report().contains("pll_div_gates=0x1ff pll_taps_available=0x0"));
    }
    #[test]
    fn typed_initialize_rmw_preserves_cpu240_and_usb48_without_a_pc_bypass(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);select_pll(&mut c,3,2,5);
        assert_eq!((c.cpu_hz(),c.ahb_hz(),c.apb_hz()),(Some(80_000_000),Some(40_000_000),Some(40_000_000)));
        for(clear,expected)in[(2,0x1fd),(4,0x1f9),(8,0x1f1),(16,0x1e1),(64,0x1a1),(128,0x121),(256,0x21)]{
            let old=c.read_at(PLL_GATES,0,PLL_ON).unwrap();assert_eq!(old&!clear,expected);assert!(c.write_at(PLL_GATES,old&!clear,0,PLL_ON));
            assert_eq!(c.pll_div_gate_mask(),Some(expected as u16));assert_eq!(c.cpu_hz(),Some(80_000_000));
        }
        assert_eq!(c.pll_tap_hz(240),Some(240_000_000));assert_eq!(c.pll_tap_hz(48),Some(48_000_000));assert_eq!(c.pll_tap_hz(160),None);
        assert!(c.report().contains("pll_div_gates=0x21 pll_taps_available=0x21"));
        // Not restricted to the factory's fixed clear order: an independent
        // supported re-enable really restores that tap's availability.
        assert!(c.write_at(PLL_GATES,0x29,0,PLL_ON));assert_eq!(c.pll_tap_hz(80),Some(80_000_000));assert_eq!(c.cpu_hz(),Some(80_000_000));
    }
    #[test]
    fn pll_current_cpu_tap_and_unwired_usb48_cannot_be_withdrawn(){
        for(source,cpu,ahb,clear)in[(3,2,5,1),(2,3,3,2)]{
            let mut c=BootClock::new();c.enable_virtual_profile(0);select_pll(&mut c,source,cpu,ahb);let before=c.report();
            for value in[0x1ff&!clear,0x1df,0,0x3ff,u32::MAX]{assert!(!c.write_at(PLL_GATES,value,0,PLL_ON));assert_eq!(c.report(),before);assert_eq!(c.pll_div_gate_mask(),Some(0x1ff));}
        }
        let mut c=BootClock::new();c.enable_virtual_profile(0);assert!(!c.write_at(PLL_GATES,0x1df,0,PLL_ON));assert_eq!(c.cpu_hz(),Some(48_000_000));
    }
    #[test]
    fn pll_closed_tap_rejects_staging_and_a_previous_stage_cannot_commit_after_closure(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);c.pump(0,PLL_ON);
        assert!(c.write_at(PLL_GATES,0x21,0,PLL_ON));let old=c.read(0x110,48_000_000);
        assert!(!c.write_at(0x6009_6110,2<<16,0,PLL_ON));assert_eq!(c.read(0x110,48_000_000),old);assert!(c.cpu_uses_xtal());
        assert!(c.write_at(PLL_GATES,0x23,0,PLL_ON));for(a,v)in[(0x6009_6118,3),(0x6009_611c,3),(0x6009_6110,2<<16)]{assert!(c.write_at(a,v,0,PLL_ON));}
        assert!(c.write_at(PLL_GATES,0x21,0,PLL_ON));assert!(!c.write_at(0x6009_6144,1,0,PLL_ON));assert_eq!(c.cpu_hz(),Some(48_000_000));
        assert!(c.write_at(PLL_GATES,0x23,0,PLL_ON));assert!(c.write_at(0x6009_6144,1,0,PLL_ON));assert_eq!(c.cpu_hz(),Some(40_000_000));
    }
    #[test]
    fn pll_gate_reset_domains_and_raw_deselection_do_not_leak_availability(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);assert!(c.write_at(PLL_GATES,0x21,0,PLL_ON));
        c.reset_domains(0,false,false,false);assert_eq!(c.pll_div_gate_mask(),Some(0x21));
        c.reset_domains(0,false,true,false);assert_eq!(c.pll_div_gate_mask(),Some(0x21));
        c.reset_domains(0,true,false,false);assert_eq!(c.pll_div_gate_mask(),Some(0x1ff));assert!(c.cpu_uses_xtal());
        assert!(c.write_at(PLL_GATES,0x21,0,PLL_ON));c.reset_domains(0,false,false,true);assert_eq!(c.pll_div_gate_mask(),Some(0x1ff));assert_eq!(c.pll_tap_hz(240),None);
        c.disable_virtual_profile();assert_eq!(c.pll_div_gate_mask(),None);assert_eq!(c.pll_tap_hz(48),None);assert_eq!(c.read(0x128,48_000_000),None);
        assert!(!c.write(0x128,0x21));assert_eq!(c.read_at(PLL_GATES,0,PLL_ON),None);assert!(!c.write_at(PLL_GATES,0x21,0,PLL_ON));assert_eq!(c.report(),"");
        c.enable_virtual_profile(1);assert_eq!(c.pll_div_gate_mask(),Some(0x1ff));assert_eq!(c.pll_tap_hz(80),None);
        assert!(BootClock::contains(PLL_GATES));for address in[PLL_GATES-1,PLL_GATES+1,0x6009_6124,0x6009_612c]{assert!(!BootClock::contains(address));}
    }
    #[test]
    fn pll_gates_do_not_change_xtal_calibration_or_create_analog_readiness(){
        let mut c=calibration(3,1,17);let before=c.deadline();assert_eq!(c.timg0_control(),1);
        assert!(c.write_at(PLL_GATES,0x21,0,INPUT));assert_eq!(c.deadline(),before);assert_eq!(c.timg0_control(),1,"TIMG0's XTAL group control is independent");
        assert_eq!(c.pll_tap_hz(240),None);assert_eq!(c.pll_tap_hz(48),None);assert_eq!(r(&mut c,VALUE,30_000),6000<<7);
    }
    #[test]
    fn lp_rc_fast_uses_oscillator_power_and_tuning_without_the_hp_bridge_gate(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);let input=ClockInputs{pll_ready:true,..INPUT};
        assert!(c.write_at(0x600b_0418,100<<22,0,input));assert!(c.write_at(0x600b_0400,0,0,input));
        assert_eq!(c.rtc_fast_hz(),Some(17_500_000));assert_eq!(c.rc_fast_hz(),Some(17_500_000));
        assert!(c.write_at(0x600b_0420,0x7000_0000,0,input));assert_eq!(c.rc_fast_hz(),None);assert_eq!(c.rtc_fast_hz(),Some(17_500_000));
        let unpowered=ClockInputs{rc_fast_powered:false,..input};c.pump(0,unpowered);assert_eq!(c.rtc_fast_hz(),None);assert_eq!(c.rtc_slow_hz(),Some(136_000));
        c.pump(0,input);assert_eq!(c.rtc_fast_hz(),Some(17_500_000));assert_eq!(c.rc_fast_hz(),None);
        for(value,hz)in[(4,24_000_000),(8,48_000_000)]{assert!(c.write_at(0x600b_0400,value,0,unpowered));assert_eq!(c.rtc_fast_hz(),Some(hz));}
    }
    #[test]
    fn lp_fast_switch_leaves_slow_root_pll_taps_and_xtal_consumers_independent(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);select_pll(&mut c,3,2,5);let input=ClockInputs{pll_ready:true,..INPUT};
        assert!(c.write_at(0x600b_0418,100<<22,0,input));assert!(c.write_at(PLL_GATES,0x21,0,input));
        let root=(c.cpu_hz(),c.ahb_hz(),c.apb_hz());
        for(value,hz)in[(0,17_500_000),(4,24_000_000),(8,48_000_000),(0,17_500_000)]{
            assert!(c.write_at(0x600b_0400,value,0,input));assert_eq!(c.rtc_fast_hz(),Some(hz));assert_eq!((c.cpu_hz(),c.ahb_hz(),c.apb_hz()),root);
            assert_eq!(c.rtc_slow_hz(),Some(136_000));assert_eq!(c.rc_fast_hz(),Some(17_500_000));assert_eq!(c.pll_div_gate_mask(),Some(0x21));
            assert_eq!(c.pll_tap_hz(240),Some(240_000_000));assert_eq!(c.pll_tap_hz(80),None);assert_eq!(c.timg0_control(),1);
        }
        assert!(c.report().contains("lp_fast_hz=Some(17500000) lp_slow_hz=Some(136000)"));
        // MSPI's independent analog-ready/SPLL480 path is not rewritten by
        // this mux. No MSPI engine is instantiated in this clock-only fixture.
    }
    #[test]
    fn lp_fast_mux_retention_and_raw_deselection_do_not_reseed_rc_availability(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);assert!(c.write_at(0x600b_0418,100<<22,0,INPUT));assert!(c.write_at(0x600b_0400,0,0,INPUT));
        for(hp,lp,analog)in[(false,false,false),(true,false,false)]{c.reset_domains(0,hp,lp,analog);assert_eq!(c.rtc_fast_hz(),Some(17_500_000));assert_eq!(c.read_at(0x600b_0400,0,INPUT),Some(0));}
        c.reset_domains(0,false,true,false);assert_eq!(c.rtc_fast_hz(),Some(24_000_000));assert_eq!(c.fosc_dfreq(),Some(172));
        assert!(c.write_at(0x600b_0400,0,0,INPUT));assert_eq!(c.rtc_fast_hz(),None);assert!(c.write_at(0x600b_0418,100<<22,0,INPUT));assert_eq!(c.rtc_fast_hz(),Some(17_500_000));
        c.reset_domains(0,false,false,true);assert_eq!(c.rtc_fast_hz(),Some(24_000_000));assert_eq!(c.fosc_dfreq(),Some(172));
        assert!(c.write_at(0x600b_0400,0,0,ClockInputs::default()));assert!(c.write_at(0x600b_0418,100<<22,0,ClockInputs::default()));assert_eq!(c.rtc_fast_hz(),None);
        c.disable_virtual_profile();assert_eq!(c.rtc_fast_hz(),None);assert_eq!(c.rc_fast_hz(),None);assert_eq!(c.report(),"");
        assert!(!c.write_at(0x600b_0400,0,0,INPUT));c.enable_virtual_profile(1);assert_eq!(c.rtc_fast_hz(),Some(24_000_000));assert_eq!(c.rc_fast_hz(),None);
    }
    #[test]
    fn lp_fast_unknown_sources_slow_muxes_and_reserved_dividers_reject(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);assert!(c.write_at(0x600b_0418,100<<22,0,INPUT));assert!(c.write_at(0x600b_0400,0,0,INPUT));let before=c.report();
        for value in[1,2,3,5,6,7,9,10,11,12,16,0xff0,0x1000,u32::MAX]{
            assert!(!c.write_at(0x600b_0400,value,0,INPUT));assert_eq!(c.read_at(0x600b_0400,0,INPUT),Some(0));assert_eq!(c.report(),before);
        }
        assert!(!c.write_at(0x600b_0418,101<<22,0,INPUT));assert_eq!(c.rtc_fast_hz(),Some(17_500_000));
    }
    #[test]
    fn raw_rc_source_observations_do_not_follow_lp_mux_or_hp_bridge(){
        let mut c=BootClock::new();assert_eq!(c.rc_fast_osc_hz(),None);assert_eq!(c.rc_slow_osc_hz(),None);c.enable_virtual_profile(0);
        assert!(c.write_at(0x600b_0418,100<<22,0,INPUT));assert!(c.write_at(0x600b_0420,0x7000_0000,0,INPUT));
        for(value,fast)in[(0,17_500_000),(4,24_000_000),(8,48_000_000)]{
            assert!(c.write_at(0x600b_0400,value,0,INPUT));assert_eq!(c.rtc_fast_hz(),Some(fast));assert_eq!(c.rc_fast_hz(),None);
            assert_eq!(c.rc_fast_osc_hz(),Some(17_500_000));assert_eq!(c.rc_slow_osc_hz(),Some(136_000));
        }
        c.pump(0,ClockInputs::default());assert_eq!(c.rc_fast_osc_hz(),None);assert_eq!(c.rc_slow_osc_hz(),None);
        c.disable_virtual_profile();assert_eq!(c.rc_fast_osc_hz(),None);assert_eq!(c.rc_slow_osc_hz(),None);assert_eq!(c.report(),"");
    }
    const TG0:u32=0x6009_6054;const TG1:u32=0x6009_6060;const LP_PO:u32=0x600b_0404;
    #[test]
    fn timg1_apb_controls_are_independent_and_ready_bits_are_read_only(){
        let mut c=BootClock::new();assert_eq!((c.timg0_control(),c.timg1_control()),(1,1));
        assert_eq!(c.read_at(TG1,0,INPUT),None);assert!(!c.write_at(TG1,0,0,INPUT));assert_eq!(c.read_at(LP_PO,0,INPUT),None);assert!(!c.write_at(LP_PO,0,0,INPUT));
        c.enable_virtual_profile(0);assert_eq!(r(&mut c,TG1,0),0x1d);let report=c.report();let deadline=c.deadline();
        for control in 0..4{for ro in 0..8{
            w(&mut c,TG1,control|(ro<<2),0);assert_eq!(c.timg1_control(),control);assert_eq!(r(&mut c,TG1,0),control|if control&2==0{0x1c}else{0});
            assert_eq!(c.timg0_control(),1);assert_eq!(r(&mut c,TG0,0),0x1d);assert_eq!(c.report(),report);assert_eq!(c.deadline(),deadline);
        }}
        w(&mut c,TG1,1,0);w(&mut c,TG0,0,0);assert_eq!((c.timg0_control(),c.timg1_control()),(0,1));assert_eq!(c.read_at(CFG,0,INPUT),None);
        assert_eq!(r(&mut c,TG1,0),0x1d);w(&mut c,TG0,1,0);assert_eq!(c.timg1_control(),1);
        assert!(BootClock::contains(TG1));assert!(BootClock::contains(LP_PO));
        for address in[TG1-1,TG1+1,0x6009_6058,0x6009_605c,0x6009_6064,0x6009_6068,LP_PO-1,LP_PO+1,0x600b_0408]{assert!(!BootClock::contains(address));assert_eq!(c.read_at(address,0,INPUT),None);assert!(!c.write_at(address,0,0,INPUT));}
        c.disable_virtual_profile();assert_eq!(c.timg1_control(),1);assert_eq!(c.read_at(TG1,0,INPUT),None);c.enable_virtual_profile(0);assert_eq!(r(&mut c,TG1,0),0x1d);assert_eq!(r(&mut c,LP_PO,0),0x7ff);
    }
    #[test]
    fn timg1_gate_and_reset_do_not_stop_or_reset_timg0_rtc_calibration(){
        let mut c=calibration(3,1,17);c.pump(15_000,INPUT);let before=c.report();let config=r(&mut c,CFG,15_000);
        w(&mut c,TG1,3,15_000);assert_eq!(c.timg1_control(),3);assert_eq!(r(&mut c,TG1,15_000),3);assert_eq!(r(&mut c,CFG,15_000),config);assert_eq!(c.report(),before);assert_eq!(c.deadline(),Some(30_000));
        w(&mut c,TG1,0,20_000);w(&mut c,LP_PO,0,20_000);assert_eq!(c.timg0_control(),1);assert_eq!(c.deadline(),Some(30_000));assert_eq!(r(&mut c,VALUE,30_000),6000<<7);
        let measured=c.report();w(&mut c,TG1,3,30_000);w(&mut c,TG1,1,30_000);assert_eq!(c.report(),measured);assert_eq!(r(&mut c,VALUE,30_000),6000<<7);
        w(&mut c,TG0,3,30_000);assert_eq!(r(&mut c,VALUE,30_000),0);assert_eq!(c.timg1_control(),1);
    }
    #[test]
    fn timg1_and_lp_pad_output_latches_have_separate_reset_domains_and_atomic_failures(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);w(&mut c,TG0,0,0);w(&mut c,TG1,2,0);w(&mut c,LP_PO,0,0);
        let before=c.report();let deadline=c.deadline();
        for(address,value)in[(TG1,0x20),(TG1,0x80),(TG1,u32::MAX),(LP_PO,1),(LP_PO,0x400),(LP_PO,0x7ff),(LP_PO,0x800),(LP_PO,u32::MAX)]{
            assert!(!c.write_at(address,value,0,INPUT));assert_eq!((c.timg0_control(),c.timg1_control()),(0,2));assert_eq!(r(&mut c,TG1,0),2);assert_eq!(r(&mut c,LP_PO,0),0);assert_eq!(c.report(),before);assert_eq!(c.deadline(),deadline);
        }
        c.reset_domains(0,false,false,false);assert_eq!((c.timg0_control(),c.timg1_control()),(0,2));assert_eq!(r(&mut c,LP_PO,0),0);
        c.reset_domains(0,false,true,false);assert_eq!((c.timg0_control(),c.timg1_control()),(0,2));assert_eq!(r(&mut c,LP_PO,0),0x7ff);
        assert!(!c.write_at(LP_PO,0x7ff,0,INPUT));assert_eq!(r(&mut c,LP_PO,0),0x7ff);w(&mut c,LP_PO,0,0);w(&mut c,LP_PO,0,0);
        c.reset_domains(0,true,false,false);assert_eq!((c.timg0_control(),c.timg1_control()),(1,1));assert_eq!(r(&mut c,TG1,0),0x1d);assert_eq!(r(&mut c,LP_PO,0),0);
        w(&mut c,TG1,0,0);c.reset_domains(0,false,false,true);assert_eq!(c.timg1_control(),1);assert_eq!(r(&mut c,TG1,0),0x1d);assert_eq!(r(&mut c,LP_PO,0),0x7ff);
    }
    fn internal_rates(c:&BootClock)->[Option<u32>;12]{[c.cpu_hz(),c.ahb_hz(),c.apb_hz(),c.rtc_fast_hz(),c.rtc_slow_hz(),c.rc_fast_hz(),c.rc_fast_osc_hz(),c.rc_slow_osc_hz(),c.pll_tap_hz(240),c.pll_tap_hz(80),c.pll_tap_hz(48),c.pll_div_gate_mask().map(u32::from)]}
    #[test]
    fn closing_lp_clock_outputs_to_pads_does_not_gate_any_internal_source(){
        let input=ClockInputs{pll_ready:true,..INPUT};
        for mode in[0,4,8]{let mut c=BootClock::new();c.enable_virtual_profile(0);select_pll(&mut c,3,2,5);
            assert!(c.write_at(0x600b_0418,100<<22,0,input));assert!(c.write_at(0x600b_0400,mode,0,input));assert!(c.write_at(PLL_GATES,0x21,0,input));assert!(c.write_at(0x600b_0420,0x7000_0000,0,input));
            assert!(c.write_at(0x6009_6ff4,0x7fff_ffff,0,input));assert_eq!(c.read_at(LP_PO,0,input),Some(0x7ff));let rates=internal_rates(&c);let report=c.report();let deadline=c.deadline();
            for _ in 0..2{assert!(c.write_at(LP_PO,0,0,input));assert_eq!(c.read_at(LP_PO,0,input),Some(0));assert_eq!(internal_rates(&c),rates);assert_eq!(c.report(),report);assert_eq!(c.deadline(),deadline);assert_eq!(c.read_at(0x6009_6ff4,0,input),Some(0x7fff_ffff));}
            assert_eq!((c.timg0_control(),c.timg1_control()),(1,1));assert_eq!(c.rc_fast_hz(),None);assert_eq!(c.rc_fast_osc_hz(),Some(17_500_000));assert_eq!(c.rtc_slow_hz(),Some(136_000));
            for value in[1,0x7ff,0x800,1<<31]{assert!(!c.write_at(LP_PO,value,0,input));assert_eq!(c.read_at(LP_PO,0,input),Some(0));assert_eq!(internal_rates(&c),rates);}
        }
    }
    const SEC:u32=0x6009_613c;
    #[test]
    fn original_sec_rmw_selects_direct_pll480_without_granting_readiness(){
        let mut c=BootClock::new();assert!(BootClock::contains(SEC));assert_eq!(c.read(0x13c,48_000_000),None);assert!(!c.write(0x13c,2));
        assert_eq!(c.read_at(SEC,0,INPUT),None);assert!(!c.write_at(SEC,2,0,INPUT));assert_eq!(c.sec_clock_source(),None);assert_eq!(c.sec_source_hz(),None);
        c.enable_virtual_profile(0);let por=c.read_at(SEC,0,INPUT).unwrap();assert_eq!(por,0);assert_eq!(c.sec_source_hz(),Some(48_000_000));
        // Actual42002542 READ,42002544 AND~3,42002546 OR2,4200254a STORE.
        assert!(c.write_at(SEC,por&!3|2,0,INPUT));assert_eq!(c.read_at(SEC,0,INPUT),Some(2));assert_eq!(c.sec_clock_source(),Some(2));
        assert_eq!(c.sec_source_hz(),None);assert!(!c.sec_pll_ready);assert_eq!(c.pll_tap_hz(240),None);
        c.pump(1,PLL_ON);assert_eq!(c.sec_source_hz(),Some(480_000_000));c.pump(2,INPUT);assert_eq!(c.sec_source_hz(),None);assert_eq!(c.sec_clock_source(),Some(2));
        c.pump(3,PLL_ON);assert_eq!(c.sec_source_hz(),Some(480_000_000));assert!(c.write_at(SEC,0,3,PLL_ON));assert_eq!(c.sec_source_hz(),Some(48_000_000));
    }
    #[test]
    fn sec_pll480_is_before_pcr_divider_taps_and_does_not_change_other_rates(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);c.pump(0,PLL_ON);let report=c.report();
        assert!(c.write_at(PLL_GATES,0x20,0,PLL_ON));let rates=internal_rates(&c);let cal=c.profile.as_ref().unwrap().report();
        assert_eq!(c.pll_tap_hz(240),None);assert_eq!(c.pll_tap_hz(80),None);assert_eq!(c.pll_tap_hz(48),Some(48_000_000));
        assert!(c.write_at(SEC,2,0,PLL_ON));assert_eq!(c.sec_source_hz(),Some(480_000_000));assert_eq!(internal_rates(&c),rates);
        // POR RTC timeout33554431, plus its terminal edge, at48MHz:5 CPU cycles/edge.
        assert_eq!(c.profile.as_ref().unwrap().report(),cal);assert_eq!(c.deadline(),Some(167_772_160));
        assert!(c.report().contains("SEC_input_mux=PLL480 pre_DPA_source_hz=Some(480000000) consumer=unmodeled"));
        assert!(c.write_at(SEC,0,0,PLL_ON));assert_eq!(c.sec_source_hz(),Some(48_000_000));
        assert!(c.write_at(PLL_GATES,0x1ff,0,PLL_ON));assert_eq!(c.report(),report,"default SEC adds no old report text");
    }
    #[test]
    fn rejected_sec_controls_are_atomic_and_do_not_cache_fictitious_pll_inputs(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);assert!(c.write_at(SEC,2,0,INPUT));
        let report=c.report();let rates=internal_rates(&c);let deadline=c.deadline();
        for value in[1,3,4,5,6,7,8,1<<31,u32::MAX]{assert!(!c.write_at(SEC,value,100,PLL_ON));
            assert_eq!(c.sec_clock_source(),Some(2));assert_eq!(c.sec_source_hz(),None);assert!(!c.sec_pll_ready);
            assert_eq!(c.report(),report);assert_eq!(internal_rates(&c),rates);assert_eq!(c.deadline(),deadline);
        }
        for address in[SEC-1,SEC+1,0x6009_6134,0x6009_6138,0x6009_6140,0x6009_6148,0x6009_6164,0x6009_6168,0x6008_7028]{
            assert!(!BootClock::contains(address));assert_eq!(c.read_at(address,0,INPUT),None);assert!(!c.write_at(address,0,0,INPUT));
            assert_eq!(c.sec_clock_source(),Some(2));assert_eq!(c.sec_source_hz(),None);assert_eq!(c.report(),report);
        }
    }
    #[test]
    fn sec_mux_and_actual_pll_observation_have_separate_reset_domains(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);assert!(c.write_at(SEC,2,0,PLL_ON));let report=c.report();
        c.reset_domains(0,false,false,false);assert_eq!(c.read_at(SEC,0,PLL_ON),Some(2));assert_eq!(c.report(),report);
        c.reset_domains(0,false,true,false);assert_eq!(c.sec_clock_source(),Some(2));assert_eq!(c.sec_source_hz(),Some(480_000_000));
        c.reset_domains(0,true,false,false);assert_eq!(c.sec_clock_source(),Some(0));assert_eq!(c.sec_source_hz(),Some(48_000_000));assert!(c.sec_pll_ready);
        assert!(c.write_at(SEC,2,0,PLL_ON));c.reset_domains(0,false,false,true);assert_eq!(c.sec_clock_source(),Some(0));assert!(!c.sec_pll_ready);
        assert!(c.write_at(SEC,2,0,INPUT));assert_eq!(c.sec_source_hz(),None);c.pump(0,PLL_ON);assert_eq!(c.sec_source_hz(),Some(480_000_000));
        c.disable_virtual_profile();assert_eq!(c.sec_clock_source(),None);assert_eq!(c.sec_source_hz(),None);assert!(!c.sec_pll_ready);assert_eq!(c.sec_source,0);
        c.enable_virtual_profile(1);assert_eq!(c.sec_clock_source(),Some(0));assert!(c.write_at(SEC,2,1,INPUT));assert_eq!(c.sec_source_hz(),None);
    }
    #[test]
    fn sec_selection_does_not_complete_or_retime_the_running_rtc_calibration(){
        let mut c=calibration(3,1,17);let mut other=calibration(3,1,17);let on=ClockInputs{pll_ready:true,..INPUT};
        assert!(c.write_at(SEC,2,15_000,on));other.pump(15_000,on);
        assert_eq!(c.deadline(),Some(30_000));assert_eq!(c.deadline(),other.deadline());assert_eq!(internal_rates(&c),internal_rates(&other));
        assert_eq!(c.profile.as_ref().unwrap().report(),other.profile.as_ref().unwrap().report());assert_eq!(c.sec_source_hz(),Some(480_000_000));
        assert_eq!(c.read_at(VALUE,29_999,on),Some(0));assert_eq!(c.read_at(VALUE,30_000,on),Some(6000<<7));assert_eq!(other.read_at(VALUE,30_000,on),Some(6000<<7));
    }
    #[test]
    fn sec_mux_accepts_the_parent_observed_factory_clock_snapshot_without_changing_cpu240(){
        let mut c=BootClock::new();c.enable_virtual_profile(0);select_pll(&mut c,3,0,5);assert!(c.write_at(PLL_GATES,0x23,0,PLL_ON));
        assert_eq!((c.cpu_hz(),c.ahb_hz(),c.apb_hz()),(Some(240_000_000),Some(40_000_000),Some(40_000_000)));
        let rates=internal_rates(&c);let value=c.read_at(SEC,0,PLL_ON).unwrap();assert_eq!(value,0);
        assert!(c.write_at(SEC,value&!3|2,0,PLL_ON));assert_eq!(c.sec_source_hz(),Some(480_000_000));assert_eq!(internal_rates(&c),rates);assert_eq!(c.pll_div_gate_mask(),Some(0x23));
    }

}
