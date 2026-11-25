//! C5 KM initialization and DPA configuration only, fixed IDF5.5.3/5.5.4.
//! The reset acknowledgement is an explicit virtual1us protocol, not a KM
//! clock edge, physical settling time, deployed key or crypto capability.
//! Receipt: docs/evidence/c5-security-init-2026-11-25.
const CONF:u32=0x6009_6164;const POWER:u32=0x6009_6168;
const STATIC:u32=0x6008_7018;const STATE:u32=0x6008_7028;const DPA:u32=0x6009_5008;
const STATIC_POR:u32=15<<5;const RESET_LATENCY:u64=240;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub(crate) struct Inputs{
    pub sec_source_hz:Option<u32>,pub apb_available:bool,pub crypto_domain_available:bool,
    pub efuse_dpa_level:Option<u8>,pub flash_encryption_enabled:Option<bool>,pub force_km_flash:Option<bool>,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Phase{HeaderPor,Held,Pending(u64),Complete,NeedsReset}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct BootSecurity{
    conf:u8,power:u8,static_conf:u32,dpa:u8,phase:Phase,cycle:u64,inputs:Inputs,
    reads:[u64;5],writes:[u64;5],asserts:u64,releases:u64,completions:u64,cancellations:u64,
    last_assert:Option<u64>,last_release:Option<u64>,last_complete:Option<u64>,last_cancel:Option<u64>,
}
impl BootSecurity{
    pub fn new()->Self{Self{conf:0,power:4,static_conf:STATIC_POR,dpa:0,phase:Phase::HeaderPor,cycle:0,inputs:Inputs::default(),
        reads:[0;5],writes:[0;5],asserts:0,releases:0,completions:0,cancellations:0,
        last_assert:None,last_release:None,last_complete:None,last_cancel:None}}
    fn index(address:u32)->Option<usize>{match address{CONF=>Some(0),POWER=>Some(1),STATIC=>Some(2),STATE=>Some(3),DPA=>Some(4),_=>None}}
    pub fn contains(address:u32)->bool{Self::index(address).is_some()}
    fn ready(&self)->bool{matches!(self.phase,Phase::HeaderPor|Phase::Complete)}
    fn context(inputs:Inputs)->bool{inputs.apb_available&&inputs.crypto_domain_available&&
        matches!(inputs.sec_source_hz,Some(48_000_000|480_000_000))&&inputs.efuse_dpa_level==Some(0)&&
        inputs.flash_encryption_enabled==Some(false)&&inputs.force_km_flash==Some(false)}
    fn operating(&self,inputs:Inputs)->bool{Self::context(inputs)&&self.power==2&&self.conf&3==1}
    fn reset_context(&self,inputs:Inputs)->bool{self.operating(inputs)&&self.dpa&3==0}
    fn observable(&self)->bool{self.phase==Phase::Complete&&self.operating(self.inputs)}
    fn cancel(&mut self,cycle:u64){if matches!(self.phase,Phase::Pending(_)){
        self.phase=Phase::NeedsReset;self.cancellations=self.cancellations.saturating_add(1);self.last_cancel=Some(cycle);
    }}
    /// Old observations own elapsed time. An event at this cycle precedes
    /// new input observations; parent watchdog/reset arbitration runs first.
    /// False records an unsupported active-input loss (cancelled, requiring
    /// a fresh reset) or rejects a backwards timestamp without mutation.
    pub fn pump(&mut self,cycle:u64,inputs:Inputs)->bool{
        if cycle<self.cycle{return false;}
        if let Phase::Pending(at)=self.phase{if cycle>=at{
            if self.reset_context(self.inputs){self.phase=Phase::Complete;self.completions=self.completions.saturating_add(1);self.last_complete=Some(at);}
            else{self.cancel(cycle);self.cycle=cycle;self.inputs=inputs;return false;}
        }}
        self.cycle=cycle;self.inputs=inputs;
        if matches!(self.phase,Phase::Pending(_))&&!self.reset_context(inputs){self.cancel(cycle);return false;}true
    }
    pub fn deadline(&self)->Option<u64>{if let Phase::Pending(at)=self.phase{Some(at)}else{None}}
    /// PCR/DPA are configuration words and require explicit APB access even
    /// for POR. Header READY1 never qualifies STATIC/STATE observations.
    pub fn read_at(&mut self,address:u32,cycle:u64,inputs:Inputs)->Option<u32>{
        let i=Self::index(address)?;if !self.pump(cycle,inputs)||!inputs.apb_available{return None;}
        let value=match address{CONF=>u32::from(self.conf)|u32::from(self.ready())<<2,POWER=>u32::from(self.power),
            STATIC if self.observable()=>self.static_conf,STATE if self.observable()=>0,DPA=>u32::from(self.dpa),_=>return None};
        self.reads[i]=self.reads[i].saturating_add(1);Some(value)
    }
    /// Register changes are transactional after legitimate old-time/input
    /// advancement. RO writes cannot create reset completion; no START,
    /// deployment, key memory/valid/result, crypto data or IRQ words exist.
    pub fn write_at(&mut self,address:u32,value:u32,cycle:u64,inputs:Inputs)->bool{
        let Some(i)=Self::index(address)else{return false;};if !self.pump(cycle,inputs)||!inputs.apb_available{return false;}
        let mut next=self.clone();match address{
            CONF=>{
                if value&!7!=0{return false;}let control=(value&3)as u8;let old_reset=self.conf&2!=0;let reset=control&2!=0;
                next.conf=control; // bit2 is RO: ignore its bus value, never latch it
                if reset&&!old_reset{
                    next.cancel(cycle);next.phase=Phase::Held;next.static_conf=STATIC_POR;
                    next.asserts=next.asserts.saturating_add(1);next.last_assert=Some(cycle);
                }else if old_reset&&!reset{
                    if !next.reset_context(inputs){return false;}let Some(at)=cycle.checked_add(RESET_LATENCY)else{return false;};
                    next.phase=Phase::Pending(at);next.releases=next.releases.saturating_add(1);next.last_release=Some(cycle);
                }
            },
            POWER=>{if !matches!(value,0|2|4){return false;}next.power=value as u8;},
            STATIC=>{if !self.observable()||(value^self.static_conf)&!2!=0{return false;}next.static_conf=value;},
            STATE=>return false,
            DPA=>{if value&!7!=0{return false;}next.dpa=value as u8;},
            _=>return false,
        }
        if matches!(next.phase,Phase::Pending(_))&&!next.reset_context(inputs){next.cancel(cycle);}
        next.writes[i]=next.writes[i].saturating_add(1);*self=next;true
    }
    /// CPU/LP preserve this HP configuration and pending event. HP/Chip
    /// restore POR and withdraw events without first completing an event at
    /// the reset boundary. This is declared virtual functional retention.
    pub fn reset_domains(&mut self,cycle:u64,hp:bool,analog:bool){if hp||analog{*self=Self::new();self.cycle=cycle;}}
    pub fn report(&self)->String{format!("SECURITY=KM_init_virtual_protocol1us nominal_latency={RESET_LATENCY} phase={:?} CONF={:#x} POWER={:#x} STATIC={:#x} DPA={:#x} ready_origin={} runtime_idle_qualified={} deadline={:?} reset_asserts={} releases={} completions={} cancellations={} assert_at={:?} release_at={:?} complete_at={:?} cancel_observed_at={:?} reads={:?} writes={:?} sec_source_hz={:?} domain={} APB={} efuse_dpa={:?} flash_encrypted={:?} force_km_flash={:?} crypto_functional_hz_physical_settling_key_valid_deploy_data_IRQ=unsupported",
        self.phase,u32::from(self.conf)|u32::from(self.ready())<<2,self.power,self.static_conf,self.dpa,
        match self.phase{Phase::HeaderPor=>"header_POR",Phase::Complete=>"virtual_reset_event",_=>"not_released"},u8::from(self.observable()),
        self.deadline(),self.asserts,self.releases,self.completions,self.cancellations,self.last_assert,self.last_release,self.last_complete,self.last_cancel,
        self.reads,self.writes,self.inputs.sec_source_hz,u8::from(self.inputs.crypto_domain_available),u8::from(self.inputs.apb_available),
        self.inputs.efuse_dpa_level,self.inputs.flash_encryption_enabled,self.inputs.force_km_flash)}
}

#[cfg(test)]
mod tests{
    use super::*;
    const KNOWN:Inputs=Inputs{sec_source_hz:Some(480_000_000),apb_available:true,crypto_domain_available:true,
        efuse_dpa_level:Some(0),flash_encryption_enabled:Some(false),force_km_flash:Some(false)};
    const APB:Inputs=Inputs{sec_source_hz:None,apb_available:true,crypto_domain_available:false,
        efuse_dpa_level:None,flash_encryption_enabled:None,force_km_flash:None};
    fn put(s:&mut BootSecurity,address:u32,value:u32,cycle:u64,inputs:Inputs){assert!(s.write_at(address,value,cycle,inputs),"write {address:#x}={value:#x} at{cycle}");}
    fn powered(s:&mut BootSecurity,cycle:u64,inputs:Inputs){put(s,POWER,0,cycle,inputs);put(s,POWER,2,cycle,inputs);put(s,CONF,5,cycle,inputs);}
    fn start(s:&mut BootSecurity,cycle:u64){powered(s,cycle,KNOWN);put(s,CONF,7,cycle,KNOWN);assert_eq!(s.read_at(CONF,cycle,KNOWN),Some(3));put(s,CONF,1,cycle,KNOWN);}
    fn complete(s:&mut BootSecurity){start(s,0);assert!(s.pump(240,KNOWN));}
    fn reject(s:&mut BootSecurity,address:u32,value:u32,cycle:u64,inputs:Inputs){assert!(s.pump(cycle,inputs));let before=s.clone();assert!(!s.write_at(address,value,cycle,inputs));assert_eq!(*s,before);}
    #[test]
    fn constructor_por_is_not_runtime_completion_or_unknown_access(){
        let mut s=BootSecurity::new();for address in[CONF,POWER,STATIC,STATE,DPA]{assert_eq!(s.read_at(address,0,Inputs::default()),None);}
        assert_eq!(s.read_at(CONF,0,APB),Some(4));assert_eq!(s.read_at(POWER,0,APB),Some(4));assert_eq!(s.read_at(DPA,0,APB),Some(0));
        assert_eq!(s.read_at(STATE,0,KNOWN),None);assert_eq!(s.read_at(STATIC,0,KNOWN),None);assert_eq!(s.completions,0);
        powered(&mut s,0,KNOWN);assert_eq!(s.read_at(STATE,0,KNOWN),None);assert_eq!(s.completions,0);assert_eq!(s.deadline(),None);
        assert!(s.report().contains("ready_origin=header_POR runtime_idle_qualified=0"));
    }
    #[test]
    fn original_reset_poll_select_efuse_and_dpa_sequence_has_finite_event(){
        let mut s=BootSecurity::new();assert_eq!(s.read_at(POWER,0,KNOWN),Some(4));powered(&mut s,0,KNOWN);
        put(&mut s,CONF,7,1,KNOWN);assert_eq!(s.read_at(CONF,1,KNOWN),Some(3));put(&mut s,CONF,1,2,KNOWN);
        assert_eq!(s.deadline(),Some(242));for cycle in[2,3,120,241]{assert_eq!(s.read_at(CONF,cycle,KNOWN),Some(1));assert_eq!(s.completions,0);}
        assert_eq!(s.read_at(CONF,242,KNOWN),Some(5));assert_eq!(s.last_complete,Some(242));assert_eq!(s.read_at(STATE,242,KNOWN),Some(0));
        assert_eq!(s.read_at(STATE,242,KNOWN),Some(0));assert_eq!(s.read_at(STATIC,242,KNOWN),Some(0x1e0));put(&mut s,STATIC,0x1e2,242,KNOWN);
        put(&mut s,DPA,4,242,KNOWN);put(&mut s,DPA,5,242,KNOWN);assert_eq!(s.read_at(STATE,242,KNOWN),Some(0));
        assert_eq!(s.read_at(STATIC,242,KNOWN),Some(0x1e2));assert_eq!((s.asserts,s.releases,s.completions),(1,1,1));
    }
    #[test]
    fn no_assert_edge_ro_bus_value_or_repeated_reads_can_schedule_completion(){
        let mut s=BootSecurity::new();powered(&mut s,0,KNOWN);put(&mut s,CONF,1,0,KNOWN);put(&mut s,CONF,5,0,KNOWN);
        assert_eq!(s.deadline(),None);assert!(s.pump(10000,KNOWN));assert_eq!(s.completions,0);assert_eq!(s.read_at(STATE,10000,KNOWN),None);
        put(&mut s,CONF,7,10000,KNOWN);put(&mut s,CONF,7,10000,KNOWN);assert_eq!(s.asserts,1);assert_eq!(s.read_at(CONF,10000,KNOWN),Some(3));
        put(&mut s,CONF,5,10000,KNOWN);assert_eq!(s.read_at(CONF,10000,KNOWN),Some(1));let at=s.deadline();
        put(&mut s,CONF,5,10000,KNOWN);for _ in 0..100{assert_eq!(s.read_at(CONF,10000,KNOWN),Some(1));assert_eq!(s.read_at(STATE,10000,KNOWN),None);}
        assert_eq!(s.deadline(),at);assert_eq!(s.releases,1);assert_eq!(s.completions,0);
    }
    #[test]
    fn release_requires_actual_power_gate_source_domain_and_known_fuses(){
        let bad=[Inputs{sec_source_hz:None,..KNOWN},Inputs{sec_source_hz:Some(0),..KNOWN},Inputs{sec_source_hz:Some(160_000_000),..KNOWN},
            Inputs{crypto_domain_available:false,..KNOWN},Inputs{efuse_dpa_level:None,..KNOWN},Inputs{efuse_dpa_level:Some(1),..KNOWN},
            Inputs{flash_encryption_enabled:None,..KNOWN},Inputs{flash_encryption_enabled:Some(true),..KNOWN},
            Inputs{force_km_flash:None,..KNOWN},Inputs{force_km_flash:Some(true),..KNOWN}];
        for inputs in bad{let mut s=BootSecurity::new();powered(&mut s,0,inputs);put(&mut s,CONF,7,0,inputs);reject(&mut s,CONF,1,0,inputs);assert_eq!(s.phase,Phase::Held);assert_eq!(s.deadline(),None);}
        for power in[0,4]{let mut s=BootSecurity::new();put(&mut s,POWER,power,0,KNOWN);put(&mut s,CONF,3,0,KNOWN);reject(&mut s,CONF,1,0,KNOWN);}
        let mut s=BootSecurity::new();powered(&mut s,0,KNOWN);put(&mut s,CONF,2,0,KNOWN);reject(&mut s,CONF,0,0,KNOWN);
        let no_apb=Inputs{apb_available:false,..KNOWN};reject(&mut s,POWER,2,0,no_apb);assert_eq!(s.read_at(CONF,0,no_apb),None);
    }
    #[test]
    fn input_loss_cancels_once_and_restoration_requires_new_reset(){
        for lost in[Inputs::default(),Inputs{sec_source_hz:None,..KNOWN},Inputs{crypto_domain_available:false,..KNOWN},
            Inputs{efuse_dpa_level:None,..KNOWN},Inputs{flash_encryption_enabled:Some(true),..KNOWN}]{
            let mut s=BootSecurity::new();start(&mut s,0);assert!(!s.pump(239,lost));assert_eq!(s.deadline(),None);assert_eq!(s.cancellations,1);
            assert!(s.pump(1000,KNOWN));assert_eq!(s.completions,0);assert_eq!(s.read_at(STATE,1000,KNOWN),None);put(&mut s,CONF,1,1000,KNOWN);assert_eq!(s.deadline(),None);
            put(&mut s,CONF,3,1000,KNOWN);put(&mut s,CONF,1,1000,KNOWN);assert_eq!(s.deadline(),Some(1240));assert!(s.pump(1240,KNOWN));assert_eq!(s.completions,1);
        }
    }
    #[test]
    fn old_inputs_own_elapsed_time_and_deadline_before_new_observations(){
        let mut s=BootSecurity::new();start(&mut s,10);let lost=Inputs{sec_source_hz:None,..KNOWN};assert!(s.pump(250,lost));
        assert_eq!(s.last_complete,Some(250));assert_eq!(s.completions,1);assert_eq!(s.cancellations,0);assert_eq!(s.read_at(STATE,250,lost),None);
        assert_eq!(s.read_at(CONF,250,lost),Some(5));assert_eq!(s.read_at(STATE,250,KNOWN),Some(0));
        let mut s=BootSecurity::new();start(&mut s,0);put(&mut s,DPA,5,240,KNOWN);assert_eq!(s.phase,Phase::Complete);assert_eq!(s.completions,1);
    }
    #[test]
    fn owned_gate_power_dpa_or_new_reset_cancels_pending_without_hidden_resume(){
        for(address,value)in[(CONF,0),(POWER,0),(POWER,4),(DPA,5),(DPA,7)]{
            let mut s=BootSecurity::new();start(&mut s,0);put(&mut s,address,value,239,KNOWN);assert_eq!(s.cancellations,1);assert_eq!(s.phase,Phase::NeedsReset);
            assert!(s.pump(1000,KNOWN));assert_eq!(s.completions,0);assert_eq!(s.deadline(),None);
        }
        let mut s=BootSecurity::new();start(&mut s,0);put(&mut s,CONF,3,120,KNOWN);assert_eq!(s.phase,Phase::Held);assert_eq!((s.cancellations,s.asserts),(1,2));
        put(&mut s,CONF,1,120,KNOWN);assert_eq!(s.deadline(),Some(360));
    }
    #[test]
    fn dpa_named_configuration_is_not_a_crypto_rate_or_new_reset_qualification(){
        let mut s=BootSecurity::new();complete(&mut s);for value in 0..=7{put(&mut s,DPA,value,240,KNOWN);assert_eq!(s.read_at(DPA,240,KNOWN),Some(value));assert_eq!(s.read_at(STATE,240,KNOWN),Some(0));}
        put(&mut s,DPA,5,240,KNOWN);put(&mut s,CONF,3,240,KNOWN);reject(&mut s,CONF,1,240,KNOWN);assert_eq!(s.completions,1);
        put(&mut s,DPA,4,240,KNOWN);put(&mut s,CONF,1,240,KNOWN);assert_eq!(s.deadline(),Some(480));
        assert!(s.report().contains("crypto_functional_hz_physical_settling_key_valid_deploy_data_IRQ=unsupported"));
    }
    #[test]
    fn restoring_known_off_configuration_does_not_replay_cancelled_event(){
        let mut s=BootSecurity::new();start(&mut s,0);put(&mut s,DPA,5,120,KNOWN);put(&mut s,DPA,0,120,KNOWN);
        put(&mut s,CONF,5,120,KNOWN);assert_eq!(s.deadline(),None);assert!(s.pump(1000,KNOWN));
        assert_eq!(s.completions,0);assert_eq!(s.read_at(STATE,1000,KNOWN),None);assert_eq!(s.read_at(CONF,1000,KNOWN),Some(1));
        put(&mut s,CONF,3,1000,KNOWN);put(&mut s,CONF,1,1000,KNOWN);assert_eq!(s.deadline(),Some(1240));
    }
    #[test]
    fn xtal_and_spll_are_only_availability_inputs_not_protocol_rates(){
        for hz in[48_000_000,480_000_000]{let inputs=Inputs{sec_source_hz:Some(hz),..KNOWN};let mut s=BootSecurity::new();powered(&mut s,1,inputs);put(&mut s,CONF,3,1,inputs);put(&mut s,CONF,1,1,inputs);assert_eq!(s.deadline(),Some(241));assert!(s.pump(241,inputs));assert_eq!(s.completions,1);}
    }
    #[test]
    fn exact_decode_and_reserved_state_start_key_data_and_ro_writes_are_strict(){
        let mut s=BootSecurity::new();complete(&mut s);
        for address in(0x6008_7000..=0x6008_7fff).chain(0x6009_5000..=0x6009_5010).chain(0x6009_6160..=0x6009_616c){
            let expected=matches!(address,CONF|POWER|STATIC|STATE|DPA);assert_eq!(BootSecurity::contains(address),expected);
            if !expected{reject(&mut s,address,0,240,KNOWN);assert_eq!(s.read_at(address,240,KNOWN),None);}
        }
        for(address,values)in[(CONF,vec![8,u32::MAX]),(POWER,vec![1,6,7,u32::MAX]),(STATIC,vec![0,2,0x1e1,0x9e0,u32::MAX]),(STATE,vec![0,1,u32::MAX]),(DPA,vec![8,u32::MAX])]{for value in values{reject(&mut s,address,value,240,KNOWN);}}
        put(&mut s,STATIC,0x1e2,240,KNOWN);put(&mut s,STATIC,0x1e0,240,KNOWN);assert_eq!(s.completions,1);
        for value in[1,5]{put(&mut s,CONF,value,240,KNOWN);assert_eq!(s.read_at(CONF,240,KNOWN),Some(5));assert_eq!(s.completions,1);}
    }
    #[test]
    fn overflow_backwards_time_and_diagnostics_never_wrap_or_panic(){
        let mut s=BootSecurity::new();let late=u64::MAX-239;powered(&mut s,late,KNOWN);put(&mut s,CONF,3,late,KNOWN);reject(&mut s,CONF,1,late,KNOWN);assert_eq!(s.deadline(),None);
        let before=s.clone();assert!(!s.pump(late-1,Inputs::default()));assert_eq!(s,before);
        s.reset_domains(u64::MAX-240,true,false);powered(&mut s,u64::MAX-240,KNOWN);put(&mut s,CONF,3,u64::MAX-240,KNOWN);put(&mut s,CONF,1,u64::MAX-240,KNOWN);
        assert_eq!(s.deadline(),Some(u64::MAX));s.completions=u64::MAX;assert!(s.pump(u64::MAX,KNOWN));assert_eq!(s.completions,u64::MAX);assert_eq!(s.last_complete,Some(u64::MAX));
        s.writes=[u64::MAX;5];put(&mut s,DPA,5,u64::MAX,KNOWN);assert_eq!(s.writes,[u64::MAX;5]);
    }
    #[test]
    fn cpu_lp_retention_and_hp_chip_reset_priority_preserve_declared_domains(){
        let mut s=BootSecurity::new();start(&mut s,0);let before=s.clone();s.reset_domains(120,false,false);assert_eq!(s,before);s.reset_domains(120,false,false);assert_eq!(s,before);
        assert!(s.pump(240,KNOWN));assert_eq!(s.completions,1);put(&mut s,DPA,5,240,KNOWN);put(&mut s,STATIC,0x1e2,240,KNOWN);
        for(hp,analog)in[(true,false),(false,true),(true,true)]{let mut t=s.clone();t.reset_domains(300,hp,analog);assert_eq!(t.phase,Phase::HeaderPor);assert_eq!(t.cycle,300);assert_eq!(t.inputs,Inputs::default());assert_eq!(t.static_conf,STATIC_POR);assert_eq!(t.dpa,0);assert_eq!(t.deadline(),None);assert_eq!(t.completions,0);}
        let mut s=BootSecurity::new();start(&mut s,0);s.reset_domains(240,true,false);assert_eq!(s.completions,0);assert_eq!(s.deadline(),None);
    }
}
