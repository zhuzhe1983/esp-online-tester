//! C5 app startup APM teardown only; not an active REE/region security engine.
//! Fixed IDF5.5.3/5.5.4 bootloader_mem.c, apm_hal.c/apm_ll.h, C5 register
//! headers and TRM1.1 chapter18. Receipt: docs/evidence/c5-apm-teardown-2026-11-24.
//! FUNC POR is enabled. HP CPU machine mode is TEE and may perform this
//! teardown; no other master is supported until actual disable/TEE0 writes.
//! Six register CLOCK_GATE words select automatic0/always-on1. They never
//! disable filtering or grant a world. Receipt: c5-apm-auto-clock-2026-11-25.
const FUNC:[u32;4]=[0x6009_90c4,0x6009_98c4,0x600b_38c4,0x6009_a0c4];
const POR:[u8;4]=[0x1f,1,3,3]; // HP_APM/LP_APM0/LP_APM/CPU_APM.
const HP_TEE:u32=0x6009_8000;const LP_TEE:u32=0x600b_3400;
const MASTER_MASK:u32=0x03ce_017f;
const CLOCK:[u32;6]=[0x6009_8ff8,0x600b_34f8,0x6009_97f8,0x6009_a7f8,0x600b_38ec,0x6009_98dc];
#[derive(Clone,Copy)]
enum Register{Filter(usize),HpMode(u8),LpMode,Clock(usize)}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct BootApm{func:[u8;4],hp_tee_written:u32,lp_tee_written:bool,writes:[u64;3],clock:[u8;6],clock_writes:u64}
impl BootApm{
    pub fn new()->Self{Self{func:POR,hp_tee_written:0,lp_tee_written:false,writes:[0;3],clock:[1;6],clock_writes:0}}
    fn register(address:u32)->Option<Register>{
        if let Some(i)=FUNC.iter().position(|&a|a==address){return Some(Register::Filter(i));}
        if let Some(i)=CLOCK.iter().position(|&a|a==address){return Some(Register::Clock(i));}
        if address==LP_TEE{return Some(Register::LpMode);}
        let offset=address.checked_sub(HP_TEE)?;if offset>31*4||!offset.is_multiple_of(4){return None;}
        let id=(offset/4)as u8;if MASTER_MASK&(1u32<<id)==0{return None;}Some(Register::HpMode(id))
    }
    pub fn contains(address:u32)->bool{Self::register(address).is_some()}
    pub fn all_filters_disabled(&self)->bool{self.func==[0;4]}
    pub fn all_masters_tee_selected(&self)->bool{self.hp_tee_written==MASTER_MASK&&self.lp_tee_written}
    pub fn master_tee_selected(&self,id:u8)->bool{
        id<32&&MASTER_MASK&(1u32<<id)!=0&&self.hp_tee_written&(1u32<<id)!=0&&(id!=1||self.lp_tee_written)
    }
    /// This is a supported-context boundary, not a simulated denied read or
    /// exception result. Parent supplies actual constrained CPU context and
    /// master identity at CPU/DMA boundaries. MSTATUS.MPP is not current mode.
    pub fn access_supported(&self,master:u8,cpu_machine:bool)->bool{
        if master==0{return cpu_machine;}
        self.all_filters_disabled()&&self.master_tee_selected(master)
    }
    pub fn read(&self,address:u32,cpu_machine:bool)->Option<u32>{
        if !self.access_supported(0,cpu_machine){return None;}
        match Self::register(address)?{
            Register::Filter(i)=>Some(u32::from(self.func[i])),
            // HP header mode POR0 conflicts with TRM/bootloader's effective
            // non-CPU REE2 reset world. Do not invent a resolved world/code.
            Register::HpMode(id)=>(self.hp_tee_written&(1u32<<id)!=0).then_some(0),
            Register::LpMode=>self.lp_tee_written.then_some(0),
            Register::Clock(i)=>Some(u32::from(self.clock[i])),
        }
    }
    pub fn write(&mut self,address:u32,value:u32,cpu_machine:bool)->bool{
        if !self.access_supported(0,cpu_machine){return false;}
        let Some(reg)=Self::register(address)else{return false;};
        if value!=0&&!matches!(reg,Register::Clock(_)){return false;}
        match reg{
            Register::Filter(i)=>{self.func[i]=0;self.writes[0]=self.writes[0].saturating_add(1);}
            Register::HpMode(id)=>{
                if !self.all_filters_disabled(){return false;}
                self.hp_tee_written|=1u32<<id;self.writes[1]=self.writes[1].saturating_add(1);
            }
            Register::LpMode=>{
                if !self.all_filters_disabled(){return false;}
                self.lp_tee_written=true;self.writes[2]=self.writes[2].saturating_add(1);
            }
            Register::Clock(i)=>{
                if value&!1!=0{return false;}
                self.clock[i]=value as u8;self.clock_writes=self.clock_writes.saturating_add(1);
            }
        }true
    }
    /// Declared functional reset contract, not per-flop silicon retention.
    /// PCR APM reset-event bypass0 remains unsupported; do not substitute its
    /// distinct system-bus bypass1. CPU retains; HP restores HP/LP0/CPU APM
    /// and HP TEE; LP restores LP APM/TEE; Chip restores all and diagnostics.
    /// Their register clock modes use the same declared domains. In particular
    /// LP_APM0 bridge follows its existing HP owner, not its LP name.
    pub fn reset_domains(&mut self,hp:bool,lp:bool,analog:bool){
        if analog{*self=Self::new();return;}
        if hp{self.func[0]=POR[0];self.func[1]=POR[1];self.func[3]=POR[3];self.hp_tee_written=0;for i in[0,2,3,5]{self.clock[i]=1;}}
        if lp{self.func[2]=POR[2];self.lp_tee_written=false;for i in[1,4]{self.clock[i]=1;}}
    }
    pub fn report(&self)->String{format!("APM=teardown_only func=[{:#x},{:#x},{:#x},{:#x}] filters_off={} HP_TEE_written={:#010x}/{MASTER_MASK:#010x} LP_TEE_written={} all_TEE_selected={} writes={}/{}/{} CPU_context=caller_machine_only preinit_nonCPU=unsupported active_REE_regions_locks_IRQ=unsupported register_clock_modes={:?} clock_writes={} automatic0_always_on1",
        self.func[0],self.func[1],self.func[2],self.func[3],u8::from(self.all_filters_disabled()),self.hp_tee_written,u8::from(self.lp_tee_written),u8::from(self.all_masters_tee_selected()),self.writes[0],self.writes[1],self.writes[2],self.clock,self.clock_writes)}
}

#[cfg(test)]
mod clock_tests{
    use super::*;
    // Original peripheral_clock_init RMW order, fixed IDF5.5.3/5.5.4.
    const STORES:[u32;6]=[0x6009_8ff8,0x600b_34f8,0x6009_97f8,0x6009_a7f8,0x600b_38ec,0x6009_98dc];
    fn configure(a:&mut BootApm,stage:u8){
        if stage>0{for address in FUNC{assert!(a.write(address,0,true));}}
        if stage>1{
            for id in[0,1,2,3,4,5,6,8,17,18,19,22,23,24,25]{assert!(a.write(HP_TEE+id*4,0,true));}
            assert!(a.write(LP_TEE,0,true));
        }
    }
    fn decisions(a:&BootApm)->Vec<bool>{
        let mut v=vec![a.all_filters_disabled(),a.all_masters_tee_selected(),a.all_filters_disabled()&&a.all_masters_tee_selected()];
        for id in 0..=255{v.push(a.master_tee_selected(id));v.push(a.access_supported(id,false));v.push(a.access_supported(id,true));}v
    }
    #[test]
    fn original_six_clr0_operations_preserve_security_por_and_record_only_clock_writes(){
        let mut a=BootApm::new();for address in STORES{
            assert!(BootApm::contains(address));let word=a.read(address,true).unwrap();assert_eq!(word,1);
            assert!(a.write(address,word&!1,true));assert_eq!(a.read(address,true),Some(0));
            assert_eq!(a.func,POR);assert_eq!(a.writes,[0;3]);assert!(!a.all_filters_disabled());assert!(!a.all_masters_tee_selected());
        }
        assert_eq!(a.clock,[0;6]);assert_eq!(a.clock_writes,6);assert!(a.access_supported(0,true));assert!(!a.access_supported(0,false));
        assert!(a.report().contains("register_clock_modes=[0, 0, 0, 0, 0, 0] clock_writes=6 automatic0_always_on1"));
    }
    #[test]
    fn automatic_and_always_on_never_change_cpu_dma_filter_or_world_decisions(){
        for stage in 0..=2{
            let mut a=BootApm::new();configure(&mut a,stage);let expected=decisions(&a);
            let state=(a.func,a.hp_tee_written,a.lp_tee_written,a.writes);
            for mode in[0,1,0,1]{for address in STORES{
                assert!(a.write(address,mode,true));assert_eq!(a.read(address,true),Some(mode));
                assert_eq!((a.func,a.hp_tee_written,a.lp_tee_written,a.writes),state);assert_eq!(decisions(&a),expected);
            }}
            assert_eq!(a.clock_writes,24);if stage==2{assert_eq!(a.writes,[4,15,1]);}
        }
    }
    #[test]
    fn nonmachine_reserved_and_unknown_clock_requests_reject_before_any_mutation(){
        for stage in[0,2]{
            let mut a=BootApm::new();configure(&mut a,stage);
            for address in STORES{
                for value in[0,1,2,u32::MAX]{let before=a.clone();assert_eq!(a.read(address,false),None);assert!(!a.write(address,value,false));assert_eq!(a,before);}
                for value in[2,3,4,1<<31,u32::MAX]{let before=a.clone();assert!(!a.write(address,value,true));assert_eq!(a,before);}
                for address in[address-4,address+4,address-1,address+1]{
                    let before=a.clone();assert!(!BootApm::contains(address));assert_eq!(a.read(address,true),None);assert!(!a.write(address,0,true));assert!(!a.write(address,1,true));assert_eq!(a,before);
                }
            }
            for address in[0,u32::MAX]{let before=a.clone();assert_eq!(a.read(address,true),None);assert!(!a.write(address,0,true));assert_eq!(a,before);}
        }
    }
    #[test]
    fn clock_modes_follow_the_declared_hp_lp_and_chip_reset_domains(){
        let mut a=BootApm::new();configure(&mut a,2);for address in STORES{assert!(a.write(address,0,true));}let initial=a.clone();
        a.reset_domains(false,false,false);assert_eq!(a,initial);
        for(hp,lp,expected)in[(true,false,[1,0,1,1,0,1]),(false,true,[0,1,0,0,1,0]),(true,true,[1;6])]{
            let mut a=initial.clone();a.reset_domains(hp,lp,false);assert_eq!(a.clock,expected);assert_eq!(a.clock_writes,6);assert_eq!(a.writes,[4,15,1]);
            for(address,value)in STORES.into_iter().zip(expected){assert_eq!(a.read(address,true),Some(u32::from(value)));}
        }
        a.reset_domains(false,false,true);assert_eq!(a,BootApm::new());
    }
    #[test]
    fn decoder_owns_exact_twenty_six_words_and_clock_reads_do_not_invent_work(){
        let mut expected=Vec::from(FUNC);expected.extend(STORES);expected.push(LP_TEE);
        for id in[0,1,2,3,4,5,6,8,17,18,19,22,23,24,25]{expected.push(HP_TEE+id*4);}
        expected.sort_unstable();expected.dedup();assert_eq!(expected.len(),26);
        for(start,end)in[(0x6009_8000,0x6009_a800),(0x600b_3400,0x600b_3900)]{
            for address in(start..end).step_by(4){assert_eq!(BootApm::contains(address),expected.contains(&address),"{address:#x}");}
        }
        let a=BootApm::new();let before=a.clone();for _ in 0..4{for address in STORES{assert_eq!(a.read(address,true),Some(1));}a.report();}assert_eq!(a,before);
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    // Exact typed bootloader_init_mem sequence. All stores carry zero;
    // the LP CPU has an additional LP_TEE target after HP master1.
    const STORES:[u32;20]=[0x6009_90c4,0x6009_98c4,0x600b_38c4,0x6009_a0c4,
        0x6009_8000,0x6009_8004,0x600b_3400,0x6009_8008,0x6009_800c,0x6009_8010,
        0x6009_8014,0x6009_8018,0x6009_8020,0x6009_8044,0x6009_8048,0x6009_804c,
        0x6009_8058,0x6009_805c,0x6009_8060,0x6009_8064];
    fn put(a:&mut BootApm,address:u32){assert!(a.write(address,0,true),"write {address:#x}");}
    fn disable(a:&mut BootApm){for &address in &STORES[..4]{put(a,address);}}
    fn initialize(a:&mut BootApm){for address in STORES{put(a,address);}}
    #[test]
    fn enabled_por_is_real_and_never_claimed_inactive_or_seeded_all_tee(){
        let a=BootApm::new();for(address,por)in FUNC.into_iter().zip([31,1,3,3]){assert_eq!(a.read(address,true),Some(por));}
        assert!(!a.all_filters_disabled());assert!(!a.all_masters_tee_selected());
        assert!(a.access_supported(0,true));assert!(!a.access_supported(0,false));
        for id in 1..=255{assert!(!a.access_supported(id,true));assert!(!a.master_tee_selected(id));}
        assert!(a.report().contains("func=[0x1f,0x1,0x3,0x3] filters_off=0"));assert!(a.report().contains("writes=0/0/0"));
    }
    #[test]
    fn unchanged_typed_twenty_stores_disable_filters_then_select_each_actual_world(){
        let mut a=BootApm::new();for(i,address)in STORES.into_iter().enumerate(){
            assert!(BootApm::contains(address));put(&mut a,address);assert_eq!(a.read(address,true),Some(0));
            assert_eq!(a.all_filters_disabled(),i>=3);assert_eq!(a.all_masters_tee_selected(),i==19);
        }
        assert!(a.report().contains("HP_TEE_written=0x03ce017f/0x03ce017f LP_TEE_written=1 all_TEE_selected=1 writes=4/15/1"));
        for id in[0,1,2,3,4,5,6,8,17,18,19,22,23,24,25]{assert!(a.master_tee_selected(id));assert!(a.access_supported(id,true));}
        for id in[7,9,16,20,21,26,31,32,255]{assert!(!a.access_supported(id,true));}
    }
    #[test]
    fn mode_selection_requires_all_four_actual_filter_disables(){
        let mut a=BootApm::new();for address in FUNC{
            for &address in &STORES[4..]{let before=a.clone();assert!(!a.write(address,0,true));assert_eq!(a,before);}
            put(&mut a,address);
        }
        put(&mut a,HP_TEE);assert!(a.master_tee_selected(0));assert!(!a.all_masters_tee_selected());
    }
    #[test]
    fn conflicting_world_por_is_unsupported_until_actual_guest_selection(){
        let mut a=BootApm::new();for &address in &STORES[4..]{assert_eq!(a.read(address,true),None);}
        disable(&mut a);assert_eq!(a.read(HP_TEE,true),None);assert_eq!(a.read(LP_TEE,true),None);
        put(&mut a,HP_TEE);assert_eq!(a.read(HP_TEE,true),Some(0));assert_eq!(a.read(HP_TEE+4,true),None);
        put(&mut a,LP_TEE);assert_eq!(a.read(LP_TEE,true),Some(0));assert!(!a.master_tee_selected(1));
    }
    #[test]
    fn lp_master_needs_both_real_targets_and_other_masters_remain_unknown(){
        let mut a=BootApm::new();disable(&mut a);put(&mut a,HP_TEE+4);assert!(!a.access_supported(1,false));
        put(&mut a,LP_TEE);assert!(a.access_supported(1,false));assert!(!a.access_supported(2,false));
        put(&mut a,HP_TEE+8);assert!(a.access_supported(2,false));assert!(!a.all_masters_tee_selected());
        a.reset_domains(true,false,false);disable(&mut a);assert!(!a.access_supported(1,false));put(&mut a,HP_TEE+4);assert!(a.access_supported(1,false));
    }
    #[test]
    fn current_machine_context_is_explicit_and_never_replaced_by_zero_world_or_mpp(){
        let mut a=BootApm::new();for address in STORES{
            let before=a.clone();assert_eq!(a.read(address,false),None);assert!(!a.write(address,0,false));assert_eq!(a,before);
        }
        initialize(&mut a);assert!(!a.access_supported(0,false));for address in STORES{
            let before=a.clone();assert_eq!(a.read(address,false),None);assert!(!a.write(address,0,false));assert_eq!(a,before);
        }
    }
    #[test]
    fn active_filter_partial_clear_ree_and_lock_requests_reject_transactionally(){
        for initialized in[false,true]{
            let mut a=BootApm::new();if initialized{initialize(&mut a);}
            for address in STORES{for value in[1,2,3,4,7,15,30,31,0x8000_0000,u32::MAX]{
                let before=a.clone();assert!(!a.write(address,value,true),"accepted {address:#x}={value:#x}");assert_eq!(a,before);
            }}
        }
    }
    #[test]
    fn contains_lists_exact_words_not_regions_irqs_tee_peripherals_or_cache_aliases(){
        let mut a=BootApm::new();for address in[0,1,u32::MAX,0x6009_9000,0x6009_9004,0x6009_9008,0x6009_900c,
            0x6009_90c8,0x6009_90cc,0x6009_9118,0x6009_80c4,0x6009_8001,0x6009_801c,0x6009_8024,
            0x6009_8040,0x6009_8050,0x6009_8054,0x6009_8068,0x6009_807c,0x600b_3404,0x6009_98c0,0x600b_38c8,
            0x600c_8000,0x600c_80c4]{let before=a.clone();assert!(!BootApm::contains(address));assert_eq!(a.read(address,true),None);assert!(!a.write(address,0,true));assert_eq!(a,before);}
        let count=(0x6009_8000..=0x6009_a0c4).step_by(4).filter(|&address|BootApm::contains(address)).count();assert_eq!(count,21);
        assert!(BootApm::contains(LP_TEE));assert!(BootApm::contains(0x600b_38c4));
    }
    #[test]
    fn cpu_core_system_and_chip_reset_follow_declared_domains_and_withdraw_readiness(){
        let mut a=BootApm::new();initialize(&mut a);let before=a.clone();a.reset_domains(false,false,false);assert_eq!(a,before);
        a.reset_domains(true,false,false);assert_eq!(a.func,[31,1,0,3]);assert_eq!(a.read(HP_TEE,true),None);assert_eq!(a.read(LP_TEE,true),Some(0));
        assert!(!a.all_filters_disabled());assert!(!a.all_masters_tee_selected());assert!(!a.access_supported(2,false));assert_eq!(a.writes,[4,15,1]);
        initialize(&mut a);a.reset_domains(false,true,false);assert_eq!(a.func,[0,0,3,0]);assert_eq!(a.read(HP_TEE,true),Some(0));assert_eq!(a.read(LP_TEE,true),None);
        assert!(!a.all_filters_disabled());assert!(!a.all_masters_tee_selected());assert!(!a.access_supported(1,false));
        a.reset_domains(true,true,false);assert_eq!(a.func,POR);assert_eq!(a.read(HP_TEE,true),None);assert_eq!(a.read(LP_TEE,true),None);
        initialize(&mut a);a.reset_domains(false,false,true);assert_eq!(a,BootApm::new());
    }
    #[test]
    fn repeated_zero_stores_record_real_writes_and_reads_report_are_pure(){
        let mut a=BootApm::new();initialize(&mut a);initialize(&mut a);assert_eq!(a.writes,[8,30,2]);assert!(a.all_masters_tee_selected());
        let before=a.clone();for _ in 0..5{for address in STORES{assert_eq!(a.read(address,true),Some(0));}a.report();}
        assert_eq!(a,before);assert!(a.report().contains("active_REE_regions_locks_IRQ=unsupported"));
    }
}
