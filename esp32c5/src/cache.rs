//! C5 cache gates and maintenance of the coherent no-line functional cache.
//! Fixed IDF v5.5.3/v5.5.4 cache_reg.h, cache_ll.h and pcr_reg.h. CPU
//! accesses use backing arrays synchronously: no resident/dirty lines, pending
//! fills or dirty writeback queue exist. This is not a cache latency model.
//! Access-error control: IDF5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0
//! cache_reg.h+cache_ll.h and esp_system/port/soc/esp32c5/cache_err_int.c;
//! identical in5.5.4@735507283d5b2f9fb363a1901172dbd9e847945d. Only the
//! shared-cache fail bit4 register controls are represented; CPU-event causality
//! is unqualified. The LL whole-word mask31 requests
//! reserved fields with unknown readback; no ICACHE2 latch/engine is invented.
const CPU_ACCESS_FAIL:u32=1<<4;
enum Contents{CoherentPassthrough}
enum Freeze{Disabled,Quiescing,Frozen}
pub(crate) struct CacheControls{
    shut:u32,date:u32,pcr:u32,contents:Contents,freeze:Freeze,
    sync_done:bool,reset_invalidate:bool,sync_map:u32,sync_addr:u32,sync_size:u32,sync_count:u64,
    preload_order:u32,autoload_config:u32,
    fail_trace:u32,fail_ena:u32,fail_ena_unknown:bool,fail_raw:u32,
}
impl CacheControls{
    pub fn new()->Self{
        let mut c=Self{shut:0,date:36_773_968,pcr:5,contents:Contents::CoherentPassthrough,freeze:Freeze::Disabled,sync_done:false,reset_invalidate:true,sync_map:63,sync_addr:0,sync_size:0,sync_count:0,preload_order:0,autoload_config:0,fail_trace:0,fail_ena:0,fail_ena_unknown:false,fail_raw:0};
        c.finish_reset_invalidate();c
    }
    fn operational(&self)->bool{self.pcr&3==1}
    pub fn cpu_allowed(&self,instruction:bool)->bool{self.operational()&&self.shut&if instruction{1}else{2}==0}
    /// Called only for a genuine guest CPU request to the cached window after
    /// other policy gates pass. DMA, host inspection, MMU/physical-capacity
    /// errors and line/fill errors are not inputs to this narrow observer.
    /// SHUT disables access, but the fixed C5 sources do not establish whether
    /// that request stalls or fails, even with TRACE1. Reject every blocked
    /// request without inventing a RAW bit, IRQ or guest-fault propagation.
    pub fn observe_cpu_failure(&mut self,instruction:bool)->Result<(),&'static str>{
        if self.cpu_allowed(instruction){return Ok(());}
        Err("blocked CPU cache request stall/fault semantics are unqualified")
    }
    pub fn irq(&self)->bool{self.fail_raw&self.fail_ena!=0}
    /// Fixed C5 interrupts.h: ETS_CACHE_INTR_SOURCE=29 (CPU line is guest-routed).
    pub fn source(&self)->(usize,bool){(29,self.irq())}
    /// Raw owner diagnostics, not an architectural read of uncertain ENA bits.
    pub fn error_report(&self)->String{
        format!("cache_fail=coherent_no_line shared_bit4 trace={} ena={:#x} raw={:#x} irq={} ena_readback={} blocked_requests=unsupported event_causality=unqualified propagation=mode0_config_only line_fill_errors=unmodeled",
            self.fail_trace,self.fail_ena,self.fail_raw,u8::from(self.irq()),if self.fail_ena_unknown{"unknown_reserved"}else{"qualified"})
    }
    /// PCR_CACHE_CONF is accessible independently of the cache gate/reset.
    pub fn read_pcr(&self)->u32{self.pcr}
    pub fn write_pcr(&mut self,value:u32)->bool{
        // IDF names PU/PD bits2/3, reset4; both descriptions say "Set0"
        // for opposite effects. TRM1.1 Register9.61 instead reserves them.
        // Preserve the matching-firmware header default; do not guess power
        // polarity. Reject unsupported power changes before resetting state.
        if value&12!=4{return false;}
        let next=value&15;
        if next&2!=0{
            self.shut=0;self.date=36_773_968;self.sync_done=false;self.reset_invalidate=true;
            self.freeze=Freeze::Disabled;
            self.sync_map=63;self.sync_addr=0;self.sync_size=0;self.sync_count=0;self.preload_order=0;self.autoload_config=0;
            self.fail_trace=0;self.fail_ena=0;self.fail_ena_unknown=false;self.fail_raw=0;
        }
        self.pcr=next;self.finish_reset_invalidate();self.finish_freeze();true
    }
    fn finish_reset_invalidate(&mut self){
        // INVALIDATE_ENA resets to1, SYNC_DONE to0. Run that real reset
        // operation when clocked and released, rather than returning a made-up
        // reset DONE. With coherent passthrough its resident/dirty set is empty.
        if self.operational()&&self.reset_invalidate{self.finish_empty_sync();self.reset_invalidate=false;}
    }
    fn finish_empty_sync(&mut self){
        match self.contents{
            Contents::CoherentPassthrough=>{
                // Invalidate/clean/writeback apply to the actual empty set.
                // Every CPU store already reaches the backing array before
                // another MMIO access; no queued data is silently discarded.
                self.sync_count=self.sync_count.saturating_add(1);self.sync_done=true;
            }
        }
    }
    fn finish_freeze(&mut self){
        if !self.operational()||self.reset_invalidate||!matches!(self.freeze,Freeze::Quiescing){return;}
        match self.contents{
            // Every prior backing access completed before this MMIO request;
            // this representation has no cache allocation or fill queue to
            // drain. Acknowledge only this actual quiescence. Physical cache
            // miss ACK_BUSY/ACK_ERROR timing is not represented here.
            Contents::CoherentPassthrough=>self.freeze=Freeze::Frozen,
        }
    }
    pub fn read(&self,off:u32)->Option<u32>{
        if !self.operational(){return None;}
        Some(match off{
            0=>0,4=>self.shut,
            // HRO: fixed4KiB I/64B line and32KiB shared/32B line.
            0x10=>16,0x14=>8,0x18=>128,0x1c=>4,
            0x28=>match self.freeze{Freeze::Disabled=>0,Freeze::Quiescing=>1<<16,Freeze::Frozen=>(1<<16)|(1<<18)},
            0x94=>if self.reset_invalidate{1}else if self.sync_done{16}else{0},
            0x98=>self.sync_map,0x9c=>self.sync_addr,0xa0=>self.sync_size,
            // Both disabled engines are genuinely quiescent (header DONE1).
            // No start request is accepted or completed without a fill engine.
            0xd4=>self.preload_order|2,0x130=>self.autoload_config|2,
            0x164=>0, // Mode0 configuration only; blocked CPU outcomes remain unsupported.
            0x168=>{if self.fail_ena_unknown{return None;}self.fail_ena},
            0x170=>self.fail_raw,0x174=>self.fail_raw&self.fail_ena,
            0x3d0=>self.fail_trace,
            0x3fc=>self.date,_=>return None,
        })
    }
    pub fn write(&mut self,off:u32,value:u32)->bool{
        if !self.operational(){return false;}
        match off{
            0=>value&12==0, // The only named fields are explicitly Reserved.
            4=>{self.shut=value&3;true},
            0x10|0x14|0x18|0x1c=>true, // HRO stores do not change geometry.
            0x28=>{
                // C5 EN16, MODE17, RO DONE18. The register and ROM API
                // disagree on the physical miss response of MODE; only the
                // original ROM's mode0 lifecycle is supported. Reserved and
                // mode1 writes reject before changing the current freeze.
                if value&!((1<<16)|(1<<18))!=0{return false;}
                if value&(1<<16)==0{self.freeze=Freeze::Disabled;}
                else if matches!(self.freeze,Freeze::Disabled){self.freeze=Freeze::Quiescing;self.finish_freeze();}true
            }
            0x94=>{
                let op=value&15;if op==0{return true;}
                // Header mutually exclusive sync commands; ROM cache.h names
                // map16 FLASH_CACHE. Other maps/engines remain unsupported.
                if op.count_ones()!=1||self.sync_map!=16{return false;}
                self.sync_done=false;self.finish_empty_sync();true
            }
            0x98=>{self.sync_map=value&63;true},0x9c=>{self.sync_addr=value;true},0xa0=>{self.sync_size=value&0x01ff_ffff;true},
            0xd4=>{if value&1!=0{return false;}self.preload_order=value&4;true},
            0x130=>{if value&1!=0{return false;}self.autoload_config=value&0x31c;true},
            0x164=>value==0,
            0x168=>{
                // The source's initial RMW writes31; only bit4 has a functional
                // C5 meaning. Reserved2/unnamed0/1/3 readback is unqualified,
                // even after a later pure-mask write, until actual cache reset.
                if !matches!(value,0|CPU_ACCESS_FAIL|31){return false;}
                self.fail_ena=value&CPU_ACCESS_FAIL;self.fail_ena_unknown|=value==31;true
            },
            0x16c=>{
                if !matches!(value,0|CPU_ACCESS_FAIL|31){return false;}
                self.fail_raw&=!(value&CPU_ACCESS_FAIL);true
            },
            0x170=>false, // R/WTC/SS is not permission to fabricate software-set events.
            0x174=>true, // Explicit RO status; stores cannot clear its source.
            0x3d0=>{if value&!1!=0{return false;}self.fail_trace=value;true},
            0x3fc=>{self.date=value&0x0fff_ffff;true},_=>false,
        }
    }
}

#[cfg(test)]
mod tests{
    use super::CacheControls;
    #[test]
    fn pcr_gate_and_reset_control_real_cpu_and_mmio_access(){
        let mut c=CacheControls::new();assert_eq!(c.read_pcr(),5);assert!(c.cpu_allowed(true));assert!(c.cpu_allowed(false));
        assert!(c.write(4,1));assert!(!c.cpu_allowed(true));assert!(c.cpu_allowed(false));
        assert!(c.write_pcr(4));assert!(!c.cpu_allowed(true));assert!(!c.cpu_allowed(false));assert_eq!(c.read(4),None);assert!(!c.write(4,0));assert_eq!(c.read_pcr(),4);
        assert!(c.write_pcr(5));assert_eq!(c.read(4),Some(1));assert!(c.write(0x3fc,123));
        assert!(c.write_pcr(7));assert!(!c.cpu_allowed(true));assert_eq!(c.read(0x94),None);assert_eq!(c.sync_count,0);assert!(c.reset_invalidate);
        assert!(c.write_pcr(5));assert_eq!(c.read(4),Some(0));assert_eq!(c.read(0x3fc),Some(36_773_968));assert_eq!(c.read(0x94),Some(16));assert_eq!(c.sync_count,1);
    }
    #[test]
    fn power_conflict_is_a_transactional_boundary_not_a_guessed_polarity(){
        let mut c=CacheControls::new();assert!(c.write(4,3));assert!(c.write(0x9c,0x4200_1234));
        for power in [0,8,12]{for control in 0..4{assert!(!c.write_pcr(power|control));assert_eq!(c.read_pcr(),5);assert_eq!(c.read(4),Some(3));assert_eq!(c.read(0x9c),Some(0x4200_1234));assert_eq!(c.sync_count,1);}}
        assert!(c.write_pcr(6));assert_eq!(c.sync_count,0);assert!(c.reset_invalidate);
        assert!(c.write_pcr(4));assert_eq!(c.sync_count,0);assert!(c.reset_invalidate);
        assert!(c.write_pcr(5));assert_eq!(c.sync_count,1);assert!(!c.reset_invalidate);assert_eq!(c.read(0x9c),Some(0));
    }
    #[test]
    fn documented_empty_set_maintenance_completes_only_accepted_requests(){
        let mut c=CacheControls::new();assert_eq!(c.read(0x94),Some(16));assert_eq!(c.read(0x98),Some(63));
        assert!(!c.write(0x94,1));assert_eq!(c.sync_count,1); // Default all-IP map is not a supported software mode.
        assert!(c.write(0x98,16));assert!(c.write(0x9c,0x4200_2000));assert!(c.write(0xa0,64));
        for (n,op)in[1,2,4,8].into_iter().enumerate(){assert!(c.write(0x94,op));assert_eq!(c.read(0x94),Some(16));assert_eq!(c.sync_count,n as u64+2);}
        for op in[3,5,6,7,9,10,12,15]{assert!(!c.write(0x94,op));assert_eq!(c.read(0x94),Some(16));assert_eq!(c.sync_count,5);}
        assert!(c.write(0x94,16));assert_eq!(c.sync_count,5); // DONE is RO, not a command.
        assert!(c.write(0xa0,u32::MAX));assert_eq!(c.read(0xa0),Some(0x01ff_ffff));assert_eq!(c.read(0x9c),Some(0x4200_2000));
        for map in[0,4,32,63]{assert!(c.write(0x98,map));assert!(!c.write(0x94,1));assert_eq!(c.sync_count,5);}
    }
    #[test]
    fn preload_and_autoload_report_only_disabled_quiescence(){
        let mut c=CacheControls::new();assert_eq!(c.read(0xd4),Some(2));assert_eq!(c.read(0x130),Some(2));
        assert!(c.write(0xd4,6));assert_eq!(c.read(0xd4),Some(6));assert!(!c.write(0xd4,1));assert_eq!(c.read(0xd4),Some(6));
        assert!(c.write(0x130,0x31e));assert_eq!(c.read(0x130),Some(0x31e));assert!(!c.write(0x130,0x31f));assert_eq!(c.read(0x130),Some(0x31e));
        assert_eq!(c.sync_count,1);assert!(c.write_pcr(7));assert!(c.write_pcr(5));assert_eq!(c.read(0xd4),Some(2));assert_eq!(c.read(0x130),Some(2));
    }
    #[test]
    fn original_rom_boot_control_sequence_has_no_invented_cache_enable_bit(){
        let mut c=CacheControls::new();assert!(c.write_pcr(c.read_pcr()|1));assert!(c.write_pcr(c.read_pcr()|2));assert!(c.write_pcr(c.read_pcr()&!2));
        // Cache_MMU_Init writes MSPI itself; Cache_Enable_Cache(0) clears
        // the two bus masks, and Resume_Autoload(0) does not touch MMIO.
        assert!(c.write(4,3));assert!(c.write(4,c.read(4).unwrap()&!3));assert!(c.cpu_allowed(true));assert!(c.cpu_allowed(false));
        assert_eq!(c.read(0x130),Some(2));assert_eq!(c.sync_count,1);
    }
    #[test]
    fn absent_line_engines_and_geometry_stay_explicit(){
        let mut c=CacheControls::new();for(off,value)in[(0x10,16),(0x14,8),(0x18,128),(0x1c,4)]{assert_eq!(c.read(off),Some(value));assert!(c.write(off,u32::MAX));assert_eq!(c.read(off),Some(value));}
        for off in[0x2c,0x30,0x84,0xbc,0x234]{assert_eq!(c.read(off),None);assert!(!c.write(off,1));}
        assert!(!c.write(0,4));assert!(!c.write(0,8));assert_eq!(c.read(0),Some(0));
    }
    #[test]
    fn freeze_mode0_quiesces_and_disable_clears_ack_without_new_work(){
        let mut c=CacheControls::new();assert_eq!(c.read(0x28),Some(0));
        assert!(c.write(0x28,1<<18));assert_eq!(c.read(0x28),Some(0)); // DONE is RO.
        assert!(c.write(0x28,1<<16));assert_eq!(c.read(0x28),Some(0x50000));assert!(c.cpu_allowed(true));assert!(c.cpu_allowed(false));
        assert!(c.write(0x28,0x50000));assert_eq!(c.read(0x28),Some(0x50000));assert_eq!(c.sync_count,1);
        assert_eq!(c.read(0xd4),Some(2));assert_eq!(c.read(0x130),Some(2)); // No new fills.
        assert!(c.write(0x28,1<<18));assert_eq!(c.read(0x28),Some(0));
        assert!(c.write(0x28,1<<16));assert_eq!(c.read(0x28),Some(0x50000));
    }
    #[test]
    fn freeze_invalid_modes_gates_and_reset_are_transactional(){
        let mut c=CacheControls::new();assert!(c.write(0x28,1<<16));
        for value in[1,1<<8,1<<9,1<<17,(1<<16)|(1<<17),u32::MAX]{assert!(!c.write(0x28,value));assert_eq!(c.read(0x28),Some(0x50000));assert_eq!(c.sync_count,1);}
        assert!(c.write_pcr(4));assert_eq!(c.read(0x28),None);assert!(!c.write(0x28,0));assert!(c.write_pcr(5));assert_eq!(c.read(0x28),Some(0x50000));
        assert!(c.write(4,3));assert!(!c.cpu_allowed(true));assert!(!c.cpu_allowed(false));assert_eq!(c.read(0x28),Some(0x50000));
        assert!(c.write_pcr(7));assert_eq!(c.read(0x28),None);assert!(c.write_pcr(5));assert_eq!(c.read(0x28),Some(0));assert!(c.cpu_allowed(true));assert!(c.cpu_allowed(false));
    }
    fn error_state(c:&CacheControls)->(u32,u32,bool,u32,u32,u32,u64){
        (c.fail_trace,c.fail_ena,c.fail_ena_unknown,c.fail_raw,c.shut,c.pcr,c.sync_count)
    }
    // Register-component stimulus only. Production has no qualified event
    // source or force-IRQ API; this does not claim guest/hardware causality.
    fn seed_raw_for_register_component(c:&mut CacheControls){c.fail_raw=16;}
    #[test]
    fn original_app_tracer_clear_enable_words_preserve_unknown_reserved_readback(){
        let mut c=CacheControls::new();
        // Original app:42001118/1124 read/write3d0,1128 write16c,
        // 112a/1132 read/write168. These are MMIO requests, not guest execution.
        assert_eq!(c.read(0x3d0),Some(0));assert!(c.write(0x3d0,c.read(0x3d0).unwrap()|1));
        assert!(c.write(0x16c,31));assert_eq!(c.read(0x168),Some(0));assert!(c.write(0x168,c.read(0x168).unwrap()|31));
        assert_eq!(c.read(0x168),None);assert_eq!(c.read(0x16c),None);
        assert_eq!(c.read(0x170),Some(0));assert_eq!(c.read(0x174),Some(0));assert_eq!(c.source(),(29,false));
        assert!(c.error_report().contains("ena=0x10 raw=0x0 irq=0 ena_readback=unknown_reserved"));
        let before=error_state(&c);assert!(c.write(0x168,31));assert!(c.write(0x16c,31));assert_eq!(error_state(&c),before);
    }
    #[test]
    fn seeded_register_flag_obeys_mask_clear_and_source_without_event_causality(){
        let mut c=CacheControls::new();seed_raw_for_register_component(&mut c);
        assert_eq!(c.read(0x170),Some(16));assert_eq!(c.read(0x174),Some(0));assert!(!c.irq());
        assert!(c.write(0x168,16));assert_eq!(c.read(0x168),Some(16));assert_eq!(c.read(0x174),Some(16));assert_eq!(c.source(),(29,true));
        assert!(c.write(0x168,0));assert!(!c.irq());assert_eq!(c.read(0x170),Some(16));
        assert!(c.write(0x168,16));assert!(c.irq());assert!(c.write(0x174,u32::MAX));assert!(c.irq());
        assert!(c.write(0x16c,0));assert!(c.irq());assert!(c.write(0x16c,16));assert_eq!(c.source(),(29,false));assert_eq!(c.read(0x170),Some(0));
        seed_raw_for_register_component(&mut c);assert!(c.irq());assert!(c.write(0x16c,31));assert_eq!(c.source(),(29,false));
    }
    #[test]
    fn cpu_channels_and_no_line_success_do_not_invent_instruction_cache_errors(){
        let mut c=CacheControls::new();assert!(c.write(0x3d0,1));assert!(c.write(0x168,31));
        for instruction in[true,false]{assert!(c.cpu_allowed(instruction));c.observe_cpu_failure(instruction).unwrap();}
        assert_eq!(c.read(0x170),Some(0));assert_eq!(c.source(),(29,false));
        assert!(c.write(4,1));c.observe_cpu_failure(false).unwrap();assert_eq!(c.read(0x170),Some(0));
        assert!(c.observe_cpu_failure(true).is_err());assert_eq!(c.read(0x170),Some(0));
        assert!(c.write(4,2));c.observe_cpu_failure(true).unwrap();assert_eq!(c.read(0x170),Some(0));
        assert!(c.observe_cpu_failure(false).is_err());assert_eq!(c.read(0x170),Some(0));
        assert_eq!(c.read(0x174),Some(0));assert_eq!(c.source(),(29,false));
    }
    #[test]
    fn unqualified_failures_keep_state_and_do_not_inject_irq(){
        let mut c=CacheControls::new();assert!(c.write(0x168,16));assert!(c.write(4,3));
        for trace in[0,1]{
            assert!(c.write(0x3d0,trace));let before=error_state(&c);
            for instruction in[true,false]{assert_eq!(c.observe_cpu_failure(instruction),Err("blocked CPU cache request stall/fault semantics are unqualified"));assert_eq!(error_state(&c),before);}
        }
        assert!(c.write(0x3d0,1));assert!(c.write_pcr(4));let before=error_state(&c);
        for instruction in[true,false]{assert!(c.observe_cpu_failure(instruction).is_err());assert_eq!(error_state(&c),before);}
        assert_eq!(c.source(),(29,false));assert!(c.write_pcr(7));let before=error_state(&c);
        assert!(c.observe_cpu_failure(false).is_err());assert_eq!(error_state(&c),before);assert!(!c.irq());
    }
    #[test]
    fn seeded_register_flag_retains_across_gates_and_real_reset_clears_error_state(){
        let mut c=CacheControls::new();assert!(c.write(0x3d0,1));assert!(c.write(0x168,31));assert!(c.write(4,1));
        seed_raw_for_register_component(&mut c);assert!(c.irq());assert!(c.write(0x3d0,0));assert!(c.irq());
        let before=error_state(&c);assert!(c.observe_cpu_failure(true).is_err());assert_eq!(error_state(&c),before);
        assert!(c.write_pcr(4));assert_eq!(c.source(),(29,true));assert_eq!(c.read(0x170),None);assert!(!c.write(0x16c,16));
        assert!(c.write_pcr(5));assert_eq!(c.read(0x170),Some(16));assert_eq!(c.read(0x168),None);
        let before=error_state(&c);assert!(!c.write_pcr(0));assert_eq!(error_state(&c),before);
        assert!(c.write_pcr(7));assert_eq!(c.source(),(29,false));assert!(c.write_pcr(5));
        for off in[0x164,0x168,0x170,0x174,0x3d0]{assert_eq!(c.read(off),Some(0));}
        assert!(c.error_report().contains("ena_readback=qualified"));assert!(c.cpu_allowed(true));assert!(c.cpu_allowed(false));
    }
    #[test]
    fn pure_enable_requests_are_qualified_until_a_reserved_whole_word_write(){
        let mut c=CacheControls::new();
        for mask in[16,16,0,0]{assert!(c.write(0x168,mask));assert_eq!(c.read(0x168),Some(mask));}
        assert!(c.write(0x168,31));
        for mask in[31,16,0,31]{assert!(c.write(0x168,mask));assert_eq!(c.read(0x168),None);assert_eq!(c.fail_ena,mask&16);assert!(!c.irq());}
        assert!(c.write_pcr(6));assert!(c.write_pcr(5));assert_eq!(c.read(0x168),Some(0));
    }
    #[test]
    fn unsupported_error_register_requests_are_transactional(){
        let mut c=CacheControls::new();assert!(c.write(0x3d0,1));assert!(c.write(0x168,31));seed_raw_for_register_component(&mut c);
        let before=error_state(&c);
        for off in[0x168,0x16c]{for value in[1,2,4,8,15,17,20,32,u32::MAX]{assert!(!c.write(off,value));assert_eq!(error_state(&c),before);}}
        for value in[1,4,16,20,u32::MAX]{assert!(!c.write(0x164,value));assert_eq!(error_state(&c),before);}
        for value in[0,4,16,31,u32::MAX]{assert!(!c.write(0x170,value));assert_eq!(error_state(&c),before);}
        for value in[2,3,u32::MAX]{assert!(!c.write(0x3d0,value));assert_eq!(error_state(&c),before);}
        assert!(c.write(0x164,0));assert_eq!(error_state(&c),before);assert_eq!(c.read(0x164),Some(0));
    }
}
