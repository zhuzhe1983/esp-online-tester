//! C5 cold-SPI TG0 protection and strict ordinary TG0/TG1 watchdogs.
//! Fixed IDF5.5.3/5.5.4 C5 timer_group_reg.h/mwdt_ll.h; TRM1.1 16.2.2.
//! Source pins and limits: docs/evidence/c5-timg-wdt-disable-2026-11-24.
//! EN=0 does not disable flashboot. The cold profile starts a real source-clock
//! counter at its recorded reset epoch; it never backfills a late attachment.
//! Both ordinary engines execute INT -> Core reset only, XTAL source0; physical pulse widths
//! and other ordinary stage/action combinations remain unsupported.
//! Additional pins/functional limits: docs/evidence/c5-ordinary-mwdt1-2026-11-25
//! and docs/evidence/c5-ordinary-mwdt0-2026-11-25. TG0 handoff requires the
//! real committed cold FLASHBOOT disable; a group reset alone cannot grant it.
const BASE:[u32;2]=[0x6000_8000,0x6000_9000];const PCR:[u32;2]=[0x6009_605c,0x6009_6068];
const KEY:u32=0x50d8_3aa1;const FLASHBOOT:u32=1<<14;const UPDATE:u32=1<<22;const ENABLE:u32=1<<31;
const RW0:u32=0xff9f_f000;const CLOCK_MASK:u32=0x0070_0000;const CLOCK_EN:u32=1<<22;
const TIMEBASE:u64=240_000_000;const XTAL:u64=48_000_000;
const POR:[u32;6]=[0x0004_c000,1<<16,26_000_000,134_217_727,1_048_575,1_048_575];
const ACTIONS:u32=1<<29|3<<27;const STAGES:u32=0x7f80_0000;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub(crate) struct ClockInputs{pub xtal_hz:Option<u32>,pub apb_clocked:[bool;2],pub in_reset:[bool;2]}
#[derive(Clone,Debug,PartialEq,Eq)]
struct Group{shadow:[u32;6],active:[u32;6],key:u32,updates:u64}
impl Group{fn new()->Self{Self{shadow:POR,active:POR,key:KEY,updates:0}}}
#[derive(Clone,Debug,PartialEq,Eq)]
struct Flashboot{origin:u64,latched:bool,tripped:bool,count:u64,phase:u64,feeds:u64,expiries:u64}
#[derive(Clone,Debug,Default,PartialEq,Eq)]
struct Ordinary{stage:usize,count:u64,phase:u64,feeds:u64,interrupts:u64,expiries:u64,raw:bool,ena:bool,tripped:bool,started:bool}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct TimgWdt{
    group:[Group;2],clock:[u32;2],inputs:ClockInputs,cycle:u64,epoch:u64,cold_available:bool,
    profile:Option<Flashboot>,ordinary:[Ordinary;2],cold_handoff:bool,reset_pending:Option<(u8,u64)>,clock_fault:bool,
}

impl TimgWdt{
    pub fn new()->Self{Self{group:std::array::from_fn(|_|Group::new()),clock:[CLOCK_EN;2],inputs:ClockInputs::default(),cycle:0,epoch:0,
        cold_available:true,profile:None,ordinary:std::array::from_fn(|_|Ordinary::default()),cold_handoff:false,reset_pending:None,clock_fault:false}}
    fn register(address:u32)->Option<(usize,u32)>{
        BASE.iter().enumerate().find_map(|(i,base)|{let offset=address.checked_sub(*base)?;(offset.is_multiple_of(4)&&(0x48..=0x64).contains(&offset)).then_some((i,offset))})
    }
    fn irq_register(address:u32)->Option<(usize,u32)>{BASE.iter().enumerate().find_map(|(i,base)|{let offset=address.checked_sub(*base)?;matches!(offset,0x70|0x74|0x78|0x7c).then_some((i,offset))})}
    pub fn contains(address:u32)->bool{PCR.contains(&address)||Self::register(address).is_some()||Self::irq_register(address).is_some()}
    pub fn virtual_profile_enabled(&self)->bool{self.profile.is_some()}
    /// Only a fresh cold/Chip epoch may create this latch. Re-selecting the
    /// already selected profile preserves its complete measured history.
    pub fn enable_cold_spi_boot(&mut self,cycle:u64,inputs:ClockInputs)->Result<(),&'static str>{
        if self.profile.is_some(){let mut candidate=self.clone();if !candidate.pump(cycle,inputs){return Err("C5 MWDT flashboot clock became unsupported");}*self=candidate;return Ok(());}
        if !self.cold_available||cycle!=self.epoch||cycle!=self.cycle{return Err("C5 MWDT cold profile must be selected at its fresh reset epoch before elapsed time or MMIO");}
        if inputs.xtal_hz!=Some(XTAL as u32)||inputs.in_reset[0]{return Err("C5 MWDT cold SPI profile requires the explicit 48MHz XTAL and released TG0 reset");}
        self.inputs=inputs;self.profile=Some(Flashboot{origin:cycle,latched:true,tripped:false,count:0,phase:0,feeds:0,expiries:0});
        self.cold_available=false;self.clock_fault=false;Ok(())
    }
    /// Preserve readable configuration, withdraw every virtual timer/action.
    /// Only a later explicit Chip reset creates another selectable cold epoch.
    pub fn disable_virtual_profile(&mut self){self.profile=None;self.ordinary=std::array::from_fn(|_|Ordinary::default());self.cold_handoff=false;self.reset_pending=None;self.clock_fault=false;self.cold_available=false;}
    fn running(&self)->bool{self.profile.as_ref().is_some_and(|p|p.latched&&!p.tripped)&&self.group[0].active[0]&FLASHBOOT!=0}
    fn reset_boundary(&self)->bool{self.reset_pending.is_some()||self.profile.as_ref().is_some_and(|p|p.tripped)||self.ordinary.iter().any(|p|p.tripped)}
    fn gated(&self)->bool{self.clock[0]&CLOCK_EN==0||self.inputs.in_reset[0]}
    fn source_known(&self)->bool{self.clock[0]&(3<<20)==0&&self.inputs.xtal_hz==Some(XTAL as u32)&&self.group[0].active[1]>>16!=0}
    fn ordinary_qualified(&self,i:usize)->bool{self.profile.is_some()&&(i==1||self.cold_handoff)}
    fn ordinary_running(&self,i:usize)->bool{self.ordinary_qualified(i)&&self.group[i].active[0]&ENABLE!=0&&!self.ordinary[i].tripped}
    fn ordinary_clock_supported(&self,i:usize)->bool{self.clock[i]&(3<<20)==0&&matches!(self.inputs.xtal_hz,None|Some(48_000_000))}
    fn ordinary_gated(&self,i:usize)->bool{self.clock[i]&CLOCK_EN==0||self.inputs.in_reset[i]||self.inputs.xtal_hz.is_none()}
    fn ordinary_supported(config:&[u32;6])->bool{config[0]&(STAGES|FLASHBOOT)==ACTIONS&&config[1]>>16!=0&&config[2]!=0&&config[3]!=0}
    fn ordinary_fault(&self,i:usize)->bool{self.ordinary_running(i)&&!self.ordinary_clock_supported(i)}
    fn clock_ok(&self)->bool{!self.clock_fault&&!(0..2).any(|i|self.ordinary_fault(i))}
    fn edge_deadline(&self,edges:u64,phase:u64,prescale:u32)->Option<u64>{
        let divisor=TIMEBASE*u64::from(prescale);let delta=(u128::from(edges)*u128::from(divisor)).saturating_sub(u128::from(phase)).div_ceil(u128::from(XTAL));
        self.cycle.checked_add(u64::try_from(delta).ok()?)
    }
    fn ordinary_deadline(&self,i:usize,reset:bool)->Option<u64>{
        if !self.ordinary_running(i)||self.ordinary_gated(i)||self.ordinary_fault(i){return None;}
        let p=&self.ordinary[i];let cfg=&self.group[i].active;let mut edges=u64::from(cfg[2+p.stage]).saturating_sub(p.count);
        if reset&&p.stage==0{edges+=u64::from(cfg[3]);}self.edge_deadline(edges,p.phase,cfg[1]>>16)
    }
    /// Only the implemented WDT bit2 owns sources58/60; general timer bit0 has
    /// no owner here. APB gating and a masked CPU do not erase a raw latch.
    pub fn sources(&self)->[(usize,bool);2]{std::array::from_fn(|i|(58+2*i,self.ordinary_qualified(i)&&self.ordinary[i].raw&&self.ordinary[i].ena&&!self.inputs.in_reset[i]))}
    #[cfg(test)]
    pub fn source(&self)->(usize,bool){self.sources()[1]}
    fn reset_group(&mut self,index:usize){
        self.group[index]=Group::new();self.ordinary[index]=Ordinary::default();if index==0{if let Some(p)=&mut self.profile{p.latched=false;p.tripped=false;p.count=0;p.phase=0;}}
    }
    /// Advance old observations first, then latch new gates. Ordinary None means
    /// explicitly unavailable edges and resumes its saved phase; conflicting
    /// known rates fail. Cold TG0 retains its original sticky clock-fault policy.
    pub fn pump(&mut self,cycle:u64,inputs:ClockInputs)->bool{
        // An emitted reset freezes observations at its actual event boundary.
        // Parent arbitration may preview beyond it, but only reset dispatch
        // can release the controller to observe a subsequent clock epoch.
        if self.reset_boundary(){return self.clock_ok();}
        let cycle=self.cycle.max(cycle);let stop=self.cold_deadline().into_iter().chain((0..2).filter_map(|i|self.ordinary_deadline(i,true))).filter(|at|*at<=cycle).min().unwrap_or(cycle);
        let elapsed=stop-self.cycle;
        if elapsed!=0&&self.profile.is_none(){self.cold_available=false;}
        if self.running()&&!self.gated()&&!self.clock_fault{
            if !self.source_known(){self.clock_fault=true;}else{
                let divisor=TIMEBASE*u64::from(self.group[0].active[1]>>16);let hold=u64::from(self.group[0].active[2]);
                let p=self.profile.as_mut().unwrap();let old_phase=p.phase;let remaining=hold.saturating_sub(p.count);
                let total=u128::from(elapsed)*u128::from(XTAL)+u128::from(old_phase);
                let ticks=(total/u128::from(divisor))as u64;p.phase=(total%u128::from(divisor))as u64;
                if ticks>=remaining{
                    let delta=(u128::from(remaining)*u128::from(divisor)).saturating_sub(u128::from(old_phase)).div_ceil(u128::from(XTAL));
                    self.reset_pending=self.cycle.checked_add(delta as u64).map(|at|(7,at));p.count=hold;p.phase=0;p.tripped=true;p.expiries=p.expiries.saturating_add(1);
                }
                else{p.count+=ticks;}
            }
        }
        for i in 0..2{self.advance_ordinary(i,elapsed);}
        if let Some((_,at))=self.reset_pending{self.cycle=at;return self.clock_ok();}
        self.cycle=stop;
        for i in 0..2{if inputs.in_reset[i]&&!self.inputs.in_reset[i]{self.reset_group(i);}}
        self.inputs=inputs;if self.running()&&!self.gated()&&!self.source_known(){self.clock_fault=true;}
        self.clock_ok()
    }
    fn advance_ordinary(&mut self,i:usize,elapsed:u64){
        if !self.ordinary_running(i)||self.ordinary_gated(i)||self.ordinary_fault(i){return;}
        let cfg=self.group[i].active;let divisor=TIMEBASE*u64::from(cfg[1]>>16);let old_phase=self.ordinary[i].phase;
        let total=u128::from(elapsed)*u128::from(XTAL)+u128::from(old_phase);let mut ticks=total/u128::from(divisor);
        let mut edges=u64::from(cfg[2+self.ordinary[i].stage]).saturating_sub(self.ordinary[i].count);
        if self.ordinary[i].stage==0&&ticks>=u128::from(edges){
            ticks-=u128::from(edges);self.ordinary[i].raw=true;self.ordinary[i].interrupts=self.ordinary[i].interrupts.saturating_add(1);
            self.ordinary[i].stage=1;self.ordinary[i].count=0;
        }else{edges=0;}
        let hold=u64::from(cfg[2+self.ordinary[i].stage]);let remaining=hold.saturating_sub(self.ordinary[i].count);
        if self.ordinary[i].stage==1&&ticks>=u128::from(remaining){
            let Some(at)=self.edge_deadline(edges+remaining,old_phase,cfg[1]>>16)else{return;};
            if self.reset_pending.is_none(){self.reset_pending=Some((7+i as u8,at));} // equal-time TG0 cause7 wins
            self.ordinary[i].count=hold;self.ordinary[i].phase=0;self.ordinary[i].tripped=true;self.ordinary[i].expiries=self.ordinary[i].expiries.saturating_add(1);
        }else{self.ordinary[i].count+=ticks as u64;self.ordinary[i].phase=(total%u128::from(divisor))as u64;}
    }
    fn cold_deadline(&self)->Option<u64>{
        if !self.running()||self.gated()||self.clock_fault||!self.source_known(){return None;}
        let p=self.profile.as_ref()?;let divisor=TIMEBASE*u64::from(self.group[0].active[1]>>16);
        let edges=u64::from(self.group[0].active[2]).saturating_sub(p.count);
        let delta=(u128::from(edges)*u128::from(divisor)).saturating_sub(u128::from(p.phase)).div_ceil(u128::from(XTAL));
        self.cycle.checked_add(u64::try_from(delta).ok()?)
    }
    pub fn deadline(&self)->Option<u64>{if self.reset_boundary(){None}else{self.cold_deadline().into_iter().chain((0..2).filter_map(|i|self.ordinary_deadline(i,false))).min()}}
    /// An emitted event stays a frozen dispatch barrier after consumption.
    pub fn take_reset_event(&mut self)->Option<(u8,u64)>{self.reset_pending.take()}
    #[cfg(test)]
    fn take_core_reset(&mut self)->Option<u64>{self.take_reset_event().map(|(_,at)|at)}
    pub fn read_at(&mut self,address:u32,cycle:u64,inputs:ClockInputs)->Option<u32>{
        if !Self::contains(address)||!self.pump(cycle,inputs)||self.reset_boundary(){return None;}self.cold_available=false;
        if let Some(i)=PCR.iter().position(|a|*a==address){return Some(self.clock[i]);}
        if let Some((i,offset))=Self::irq_register(address){
            if !self.ordinary_qualified(i)||!inputs.apb_clocked[i]||inputs.in_reset[i]{return None;}
            return Some(match offset{0x70=>u32::from(self.ordinary[i].ena)<<2,0x74=>u32::from(self.ordinary[i].raw)<<2,0x78=>u32::from(self.ordinary[i].raw&&self.ordinary[i].ena)<<2,0x7c=>0,_=>return None});
        }
        let (i,offset)=Self::register(address)?;if !inputs.apb_clocked[i]{return None;}
        Some(match offset{0x48..=0x5c=>self.group[i].shadow[((offset-0x48)/4)as usize],0x60=>0,0x64=>self.group[i].key,_=>return None})
    }
    pub fn write_at(&mut self,address:u32,value:u32,cycle:u64,inputs:ClockInputs)->bool{
        if !Self::contains(address)||!self.pump(cycle,inputs)||self.reset_boundary(){return false;}self.cold_available=false;
        if let Some(i)=PCR.iter().position(|a|*a==address){
            if value&!CLOCK_MASK!=0||(i==0&&self.running()||self.ordinary_running(i))&&value&(3<<20)!=0{return false;}
            self.clock[i]=value;return true;
        }
        if let Some((i,offset))=Self::irq_register(address){
            // Fixed headers identify the shared WDT bit, but do not settle
            // key-lock interaction. The qualified init writes are unlocked;
            // locked shared-IRQ writes reject rather than invent an ignore.
            if !self.ordinary_qualified(i)||!inputs.apb_clocked[i]||inputs.in_reset[i]||self.group[i].key!=KEY||value&!4!=0{return false;}
            match offset{0x70=>self.ordinary[i].ena=value&4!=0,0x74|0x7c=>{if value&4!=0{self.ordinary[i].raw=false;}},_=>return false}return true;
        }
        let (i,offset)=Self::register(address).unwrap();if !inputs.apb_clocked[i]||inputs.in_reset[i]{return false;}
        if offset==0x64{self.group[i].key=value;return true;}
        if self.group[i].key!=KEY{return true;} // actual protected writes are ignored
        let running=i==0&&self.running();
        match offset{
            0x48=>{
                if value&!(RW0|UPDATE)!=0{return false;}let next=value&RW0;
                if next&FLASHBOOT!=0&&self.group[i].shadow[0]&FLASHBOOT==0{return false;}
                if running&&(next^self.group[i].shadow[0])&!FLASHBOOT!=0{return false;}
                if value&UPDATE!=0&&self.ordinary[i].started&&self.group[i].shadow[1]!=self.group[i].active[1]&&self.ordinary[i].phase!=0{return false;}
                if next&ENABLE!=0{
                    if !self.ordinary_qualified(i)||!self.ordinary_clock_supported(i){return false;}
                    let mut config=if value&UPDATE!=0{self.group[i].shadow}else{self.group[i].active};
                    config[0]=if value&UPDATE!=0{next}else{config[0]&!ENABLE|ENABLE};
                    if !Self::ordinary_supported(&config)||next&(STAGES|FLASHBOOT)!=ACTIONS{return false;}
                    if self.ordinary_running(i)&&config[1]!=self.group[i].active[1]{return false;}
                }
                self.group[i].shadow[0]=next;
                self.group[i].active[0]=(self.group[i].active[0]&!ENABLE)|(next&ENABLE);if next&ENABLE!=0{self.ordinary[i].started=true;}
                if value&UPDATE!=0{
                    self.group[i].active=self.group[i].shadow;self.group[i].updates=self.group[i].updates.saturating_add(1);
                    if i==0&&next&FLASHBOOT==0{if let Some(p)=&mut self.profile{if p.latched{self.cold_handoff=true;}p.latched=false;}}
                }
            },
            0x4c=>{
                if value&!0xffff_0001!=0||(running||self.ordinary_running(i))&&value&0xffff_0000!=self.group[i].active[1]{return false;}
                self.group[i].shadow[1]=value&0xffff_0000;
                if value&1!=0{if self.ordinary_qualified(i){self.ordinary[i].phase=0;}else if i==0{if let Some(p)=&mut self.profile{p.phase=0;}}}
            },
            0x50..=0x5c=>{let n=((offset-0x48)/4)as usize;if running&&value!=self.group[i].active[n]||self.ordinary_running(i)&&n<4&&value==0{return false;}self.group[i].shadow[n]=value;},
            0x60=>{if self.ordinary_qualified(i){let p=&mut self.ordinary[i];p.count=0;p.stage=0;p.feeds=p.feeds.saturating_add(1);}else if i==0{if let Some(p)=&mut self.profile{if !p.tripped{p.count=0;p.feeds=p.feeds.saturating_add(1);}}}},
            _=>return false,
        }
        // A committed shorter hold can already be due at this boundary. No
        // past timestamp or implicit feed is fabricated by the UPDATE.
        if offset==0x48{self.pump(self.cycle,self.inputs);}true
    }
    /// CPU-only retains state. HP/Core or System reset clears both TIMG
    /// controllers without rearming cold flashboot. Chip resets create a new
    /// epoch, which still requires explicit source-checked cold selection.
    pub fn reset_domains(&mut self,cycle:u64,hp:bool,lp:bool,analog:bool){
        self.pump(cycle,self.inputs);if !hp&&!lp&&!analog{return;}self.cycle=self.cycle.max(cycle);
        self.reset_group(0);self.reset_group(1);self.clock=[CLOCK_EN;2];self.reset_pending=None;self.clock_fault=false;
        self.cold_available=false;if analog{self.profile=None;self.cold_handoff=false;self.epoch=self.cycle;self.cold_available=true;}
    }
    pub fn report(&self)->String{let mut report=self.profile.as_ref().map_or_else(
        ||format!("MWDT=unprofiled selected={} ordinary_enable=unsupported flashboot_bits={}/{}",u8::from(self.virtual_profile_enabled()),u8::from(self.group[0].shadow[0]&FLASHBOOT!=0),u8::from(self.group[1].shadow[0]&FLASHBOOT!=0)),
        |p|format!("MWDT0=cold_SPI_source_clocked selected={} origin={} XTAL=48MHz prescale={} active={} count={} phase={} feeds={} expiries={} pending_core7_at={:?} clock_fault={} updates={}/{} MWDT1_auto=0 ordinary_enable=unsupported physical_reset_pulse=unmodeled",
            u8::from(self.virtual_profile_enabled()),p.origin,self.group[0].active[1]>>16,u8::from(self.running()),p.count,p.phase,p.feeds,p.expiries,self.reset_pending.filter(|e|e.0==7).map(|e|e.1),u8::from(self.clock_fault),self.group[0].updates,self.group[1].updates));
        for i in 0..2{if self.ordinary[i].started{report=report.replace("ordinary_enable=unsupported","ordinary_enable=strict");let p=&self.ordinary[i];report.push_str(&format!(" MWDT{}=ordinary_XTAL48_INT_Core{} active={} stage={} count={} phase={} feeds={} IRQs={} resets={} raw={} ena={} source{}={} pending={:?} clock_fault={}",
            i,7+i,u8::from(self.ordinary_running(i)),p.stage,p.count,p.phase,p.feeds,p.interrupts,p.expiries,u8::from(p.raw),u8::from(p.ena),58+2*i,u8::from(self.sources()[i].1),self.reset_pending,u8::from(self.ordinary_fault(i))));}}report}
}

#[cfg(test)]
mod tests{
    use super::*;
    const INPUT:ClockInputs=ClockInputs{xtal_hz:Some(48_000_000),apb_clocked:[true;2],in_reset:[false;2]};
    fn cold()->TimgWdt{let mut w=TimgWdt::new();w.enable_cold_spi_boot(0,INPUT).unwrap();w}
    fn r(w:&mut TimgWdt,group:usize,offset:u32,cycle:u64)->u32{w.read_at(BASE[group]+offset,cycle,INPUT).unwrap()}
    fn wr(w:&mut TimgWdt,group:usize,offset:u32,value:u32,cycle:u64){assert!(w.write_at(BASE[group]+offset,value,cycle,INPUT),"TG{group}+{offset:#x}={value:#x}");}
    #[test]
    fn full_por_key_and_inactive_tg1_are_distinct_from_tg0_cold_flashboot(){
        let mut w=cold();for g in 0..2{for (n,value)in POR.iter().enumerate(){assert_eq!(r(&mut w,g,0x48+4*n as u32,0),*value);}assert_eq!(r(&mut w,g,0x64,0),KEY);assert_eq!(r(&mut w,g,0x60,0),0);}
        assert_eq!(w.clock,[CLOCK_EN;2]);assert_eq!(w.deadline(),Some(130_000_000));assert!(w.virtual_profile_enabled());
        assert_eq!(r(&mut w,0,0x48,0)&ENABLE,0);assert!(w.running());assert!(w.report().contains("MWDT1_auto=0"));
    }
    #[test]
    fn profile_selection_is_transactional_fresh_and_idempotent_without_history_replay(){
        let mut w=TimgWdt::new();for input in [ClockInputs::default(),ClockInputs{xtal_hz:Some(40_000_000),..INPUT},ClockInputs{in_reset:[true,false],..INPUT}]{let before=w.clone();assert!(w.enable_cold_spi_boot(0,input).is_err());assert_eq!(w,before);}
        let before=w.clone();assert!(w.enable_cold_spi_boot(1,INPUT).is_err());assert_eq!(w,before);
        w.enable_cold_spi_boot(0,INPUT).unwrap();assert!(w.pump(573_888,INPUT));let before=w.clone();w.enable_cold_spi_boot(573_888,INPUT).unwrap();assert_eq!(w,before);
        assert_eq!(w.profile.as_ref().unwrap().count,114_777);assert_eq!(w.deadline(),Some(130_000_000));
        let mut late=TimgWdt::new();late.pump(1,INPUT);assert!(late.enable_cold_spi_boot(1,INPUT).is_err());
        let mut touched=TimgWdt::new();assert_eq!(r(&mut touched,0,0x64,0),KEY);assert!(touched.enable_cold_spi_boot(0,INPUT).is_err());
    }
    #[test]
    fn original_rom_key_clear_update_lock_sequence_really_disarms_only_at_commit(){
        let mut w=cold();wr(&mut w,0,0x64,KEY,573_888); //PC400212f2
        let cfg=r(&mut w,0,0x48,573_889);assert_eq!(cfg,0x4c000);
        wr(&mut w,0,0x48,cfg&!FLASHBOOT,573_890); //PC400213b0
        assert_eq!(w.deadline(),Some(130_000_000));assert!(w.running());
        let cfg=r(&mut w,0,0x48,573_891);assert_eq!(cfg,0x48000);
        wr(&mut w,0,0x48,cfg|UPDATE,573_892); //PC400213ba
        assert_eq!(w.deadline(),None);assert!(!w.running());assert_eq!(r(&mut w,0,0x48,573_893),0x48000);
        wr(&mut w,0,0x64,0,573_894);assert_eq!(w.group[0].updates,1);assert_eq!(r(&mut w,0,0x64,573_894),0);
        assert!(w.pump(228_000_000,INPUT));assert_eq!(w.take_core_reset(),None);assert_eq!(w.profile.as_ref().unwrap().count,114_778);
    }
    #[test]
    fn locked_writes_are_ignored_and_unlocked_ordinary_activation_is_rejected(){
        let mut w=cold();wr(&mut w,0,0x64,0,0);let before=w.clone();
        for (offset,value)in [(0x48,u32::MAX),(0x4c,0),(0x50,0),(0x60,1)]{wr(&mut w,0,offset,value,0);assert_eq!(w,before);}
        wr(&mut w,0,0x64,KEY,0);let cfg=r(&mut w,0,0x48,0);let before=w.clone();assert!(!w.write_at(BASE[0]+0x48,cfg|ENABLE,0,INPUT));assert_eq!(w,before);
        assert!(!w.write_at(BASE[1]+0x48,POR[0]|ENABLE,0,INPUT));assert_eq!(w,before);
    }
    #[test]
    fn inactive_full_fields_stage_and_commit_but_never_create_counter_or_irq_actions(){
        let mut w=TimgWdt::new();for g in 0..2{
            wr(&mut w,g,0x4c,0xffff_0001,0);assert_eq!(r(&mut w,g,0x4c,0),0xffff_0000);
            for (off,value)in [(0x50,0),(0x54,u32::MAX),(0x58,22),(0x5c,33)]{wr(&mut w,g,off,value,0);assert_eq!(r(&mut w,g,off,0),value);}
            wr(&mut w,g,0x48,0x7f9f_b000,0);assert_eq!(w.group[g].active,POR);assert_eq!(r(&mut w,g,0x48,0),0x7f9f_b000);
            wr(&mut w,g,0x48,0x7fdf_b000,0);assert_eq!(w.group[g].active,w.group[g].shadow);assert_eq!(r(&mut w,g,0x48,0)&UPDATE,0);
            wr(&mut w,g,0x60,u32::MAX,0);assert_eq!(r(&mut w,g,0x60,0),0);
            let before=w.clone();assert!(!w.write_at(BASE[g]+0x48,0x7f9f_f000,0,INPUT));assert_eq!(w,before);
        }assert_eq!(w.deadline(),None);assert_eq!(w.take_core_reset(),None);
    }
    #[test]
    fn active_clock_gate_pauses_real_ticks_and_conflicting_sources_reject(){
        let mut w=cold();for sel in 1..=3{let before=w.clone();assert!(!w.write_at(PCR[0],CLOCK_EN|sel<<20,0,INPUT));assert_eq!(w,before);}
        assert!(w.write_at(PCR[0],0,250,INPUT));assert_eq!(w.profile.as_ref().unwrap().count,50);assert_eq!(w.deadline(),None);
        assert!(w.pump(10_000,INPUT));assert_eq!(w.profile.as_ref().unwrap().count,50);assert!(w.write_at(PCR[0],CLOCK_EN,10_000,INPUT));
        assert_eq!(w.deadline(),Some(130_009_750));w.pump(130_009_751,INPUT);assert_eq!(w.take_core_reset(),Some(130_009_750));
        let mut inactive=TimgWdt::new();assert!(inactive.write_at(PCR[1],CLOCK_EN|2<<20,0,INPUT));assert_eq!(inactive.read_at(PCR[1],0,INPUT),Some(CLOCK_EN|2<<20));
    }
    #[test]
    fn apb_gate_blocks_registers_without_disabling_independent_source_clock(){
        let mut w=cold();let gated=ClockInputs{apb_clocked:[false,true],..INPUT};assert!(w.pump(100,gated));
        assert_eq!(w.read_at(BASE[0]+0x48,100,gated),None);assert!(!w.write_at(BASE[0]+0x64,KEY,100,gated));
        assert_eq!(w.read_at(PCR[0],100,gated),Some(CLOCK_EN));w.pump(130_000_001,gated);assert_eq!(w.take_core_reset(),Some(130_000_000));
    }
    #[test]
    fn real_feed_and_divider_reset_commands_change_count_phase_and_deadlines(){
        let mut w=cold();w.pump(100,INPUT);assert_eq!(w.profile.as_ref().unwrap().count,20);
        wr(&mut w,0,0x60,0,102);assert_eq!(w.deadline(),Some(130_000_100));wr(&mut w,0,0x60,u32::MAX,200);assert_eq!(w.deadline(),Some(130_000_200));
        wr(&mut w,0,0x64,0,250);wr(&mut w,0,0x60,1,250);assert_eq!(w.profile.as_ref().unwrap().count,10);assert_eq!(w.deadline(),Some(130_000_200));
        wr(&mut w,0,0x64,KEY,253);wr(&mut w,0,0x4c,1<<16|1,253);assert_eq!(w.deadline(),Some(130_000_203));
        assert_eq!(w.profile.as_ref().unwrap().feeds,2);assert_eq!(r(&mut w,0,0x4c,253),1<<16);
    }
    #[test]
    fn exact_core_event_time_precedes_later_swd_and_freezes_future_observations(){
        let mut w=cold();w.pump(129_999_999,INPUT);assert_eq!(w.take_core_reset(),None);
        let future=ClockInputs{xtal_hz:None,apb_clocked:[false;2],in_reset:[true;2]};assert!(w.pump(228_000_001,future));
        assert_eq!(w.cycle,130_000_000);assert_eq!(w.inputs,INPUT);assert_eq!(w.take_core_reset(),Some(130_000_000));assert_eq!(w.take_core_reset(),None);
        let before=w.clone();assert!(w.pump(u64::MAX,future));assert_eq!(w,before);
        assert!(!w.write_at(BASE[0]+0x60,1,228_000_001,future));assert_eq!(w,before);
    }
    #[test]
    fn reset_domains_preserve_cpu_but_core_does_not_rearm_and_chip_requires_new_selection(){
        let mut w=cold();w.pump(1234,INPUT);let before=w.clone();w.reset_domains(1234,false,false,false);assert_eq!(w,before);
        w.reset_domains(1234,true,false,false);assert_eq!(w.deadline(),None);assert_eq!(r(&mut w,0,0x48,1234),POR[0]);
        w.enable_cold_spi_boot(1234,INPUT).unwrap();assert_eq!(w.deadline(),None);assert_eq!(w.profile.as_ref().unwrap().origin,0);
        w.reset_domains(2000,false,false,true);assert!(!w.virtual_profile_enabled());w.enable_cold_spi_boot(2000,INPUT).unwrap();assert_eq!(w.deadline(),Some(130_002_000));
        w.reset_domains(2001,false,true,false);assert_eq!(w.deadline(),None);
    }
    #[test]
    fn group_reset_and_raw_selection_withdraw_timers_without_inventing_new_cold_history(){
        let mut w=cold();let reset=ClockInputs{in_reset:[true,false],..INPUT};w.pump(1000,reset);
        assert_eq!(w.deadline(),None);assert!(!w.write_at(BASE[0]+0x64,KEY,1000,reset));w.pump(1001,INPUT);assert_eq!(w.deadline(),None);
        let readable=r(&mut w,0,0x48,1001);w.disable_virtual_profile();assert_eq!(r(&mut w,0,0x48,1001),readable);
        assert_eq!(w.deadline(),None);assert_eq!(w.take_core_reset(),None);let before=w.clone();assert!(w.enable_cold_spi_boot(1001,INPUT).is_err());assert_eq!(w,before);
    }
    #[test]
    fn unknown_active_clock_is_explicit_failure_and_failed_reselection_is_atomic(){
        let mut w=cold();assert!(!w.pump(100,ClockInputs{xtal_hz:None,..INPUT}));assert_eq!(w.profile.as_ref().unwrap().count,20);
        assert_eq!(w.deadline(),None);let before=w.clone();assert!(w.enable_cold_spi_boot(100,INPUT).is_err());assert_eq!(w,before);assert!(w.report().contains("clock_fault=1"));
    }
    #[test]
    fn fractional_source_ticks_are_chunk_invariant_and_huge_jumps_are_bounded(){
        let mut a=cold();let mut b=cold();a.pump(573_888,INPUT);for at in [1,2,3,99,1000,573_888]{b.pump(at,INPUT);}assert_eq!(a,b);
        a.pump(u64::MAX,INPUT);assert_eq!(a.take_core_reset(),Some(130_000_000));assert_eq!(a.profile.as_ref().unwrap().expiries,1);
    }
    #[test]
    fn unknown_shared_timer_irq_and_reserved_controls_do_not_alias_wdt(){
        let mut w=cold();for address in [0,BASE[0],BASE[0]+0x20,BASE[0]+0x68,BASE[0]+0x6c,BASE[0]+0x80,BASE[0]+0x84,BASE[0]+0x88,BASE[0]+0x49,0x6009_6054,u32::MAX]{
            let before=w.clone();assert!(!TimgWdt::contains(address));assert_eq!(w.read_at(address,0,INPUT),None);assert!(!w.write_at(address,0,0,INPUT));assert_eq!(w,before);
        }
        for (address,value)in [(BASE[0]+0x48,POR[0]|1),(BASE[0]+0x48,POR[0]|1<<21),(BASE[0]+0x4c,1<<16|2),(BASE[0]+0x50,1),(PCR[0],1)]{
            let before=w.clone();assert!(!w.write_at(address,value,0,INPUT));assert_eq!(w,before);
        }
    }
    fn disarm_cold(w:&mut TimgWdt){wr(w,0,0x48,0x0044_8000,0);}
    fn init_iwdt(w:&mut TimgWdt,at:u64,prescale:u32,holds:[u32;2]){
        init_mwdt(w,1,at,prescale,holds);
    }
    fn init_mwdt(w:&mut TimgWdt,group:usize,at:u64,prescale:u32,holds:[u32;2]){
        prepare_mwdt(w,group,at,prescale,holds);for(off,value)in[(0x60,1),(0x48,0xb81f_8000),(0x64,0)]{wr(w,group,off,value,at);}
    }
    fn prepare_mwdt(w:&mut TimgWdt,group:usize,at:u64,prescale:u32,holds:[u32;2]){
        // Actual ROM initialization order; no PCR WDT clock writes.
        for(off,value)in[(0x48,0x0004_8000),(0x64,KEY),(0x48,0x0004_8000),
            (0x48,0x0004_8000),(0x48,0x0044_8000),(0x48,0x0004_8000),(0x48,0x0044_8000),
            (0x48,0x0004_8000),(0x48,0x0044_8000),(0x48,0x0004_8000),(0x48,0x0044_8000),
            (0x7c,4),(0x70,4),(0x48,0x001c_8000),(0x48,0x005c_8000),(0x48,0x001f_8000),
            (0x48,0x005f_8000),(0x4c,prescale<<16),(0x48,0x005f_8000),(0x64,0),
            (0x64,KEY),(0x48,0x201f_8000),(0x50,holds[0]),(0x48,0x205f_8000),
            (0x48,0x381f_8000),(0x54,holds[1]),(0x48,0x385f_8000)]{wr(w,group,off,value,at);}
    }
    fn iwdt(keep_cold:bool,prescale:u32,holds:[u32;2])->TimgWdt{
        let mut w=cold();if !keep_cold{disarm_cold(&mut w);}init_iwdt(&mut w,0,prescale,holds);w
    }
    #[test]
    fn original_rom_iwdt_commits_prescale_and_stages_before_direct_enable(){
        let mut w=iwdt(false,24_000,[10_000,10_000]);assert_eq!(w.clock,[CLOCK_EN;2]);
        assert_eq!(w.group[1].updates,9);assert_eq!(w.group[1].active,[0xb81f_8000,0x5dc0_0000,10_000,10_000,1_048_575,1_048_575]);
        assert_eq!(w.ordinary[1].feeds,1);assert_eq!(r(&mut w,1,0x48,0),0xb81f_8000);assert_eq!(w.source(),(60,false));
        assert_eq!(w.deadline(),Some(1_200_000_000));w.pump(1_199_999_999,INPUT);assert_eq!(w.ordinary[1].count,9999);
        assert_eq!(w.ordinary[1].phase,5_759_952_000_000);assert_eq!(w.deadline(),Some(1_200_000_000));
        w.pump(1_200_000_000,INPUT);assert_eq!((w.ordinary[1].stage,w.ordinary[1].count),(1,0));assert_eq!(w.source(),(60,true));
        assert_eq!(r(&mut w,1,0x74,1_200_000_000),4);assert_eq!(r(&mut w,1,0x78,1_200_000_000),4);
        assert_eq!(w.deadline(),Some(2_400_000_000));w.pump(u64::MAX,INPUT);assert_eq!(w.take_reset_event(),Some((8,2_400_000_000)));
        assert_eq!(w.ordinary[1].expiries,1);assert_eq!(w.cycle,2_400_000_000);assert_eq!(w.deadline(),None);
    }
    #[test]
    fn tick_shadow_updates_never_feed_and_runtime_timeout_is_300ms_then_600ms(){
        let mut w=iwdt(false,24_000,[10_000,10_000]);let at=2_400_000;w.pump(at,INPUT);assert_eq!(w.ordinary[1].count,20);
        wr(&mut w,1,0x64,KEY,at);wr(&mut w,1,0x48,0xb81f_8000,at);wr(&mut w,1,0x50,600,at);
        assert_eq!(w.group[1].active[2],10_000);assert_eq!(w.deadline(),Some(1_200_000_000));
        wr(&mut w,1,0x48,0xb85f_8000,at);assert_eq!(w.ordinary[1].count,20);assert_eq!(w.ordinary[1].feeds,1);assert_eq!(w.deadline(),Some(72_000_000));
        wr(&mut w,1,0x48,0xb81f_8000,at);wr(&mut w,1,0x54,1200,at);assert_eq!(w.group[1].active[3],10_000);
        wr(&mut w,1,0x48,0xb85f_8000,at);wr(&mut w,1,0x60,1,at);wr(&mut w,1,0x64,0,at);
        assert_eq!(w.ordinary[1].feeds,2);assert_eq!(w.ordinary[1].count,0);assert_eq!(w.deadline(),Some(74_400_000));
        w.pump(74_399_999,INPUT);assert_eq!(w.source(),(60,false));w.pump(74_400_000,INPUT);assert_eq!(w.source(),(60,true));
        assert_eq!(w.deadline(),Some(218_400_000));w.pump(218_399_999,INPUT);assert_eq!(w.take_reset_event(),None);
        w.pump(218_400_000,INPUT);assert_eq!(w.take_reset_event(),Some((8,218_400_000)));
    }
    #[test]
    fn real_irq_raw_mask_clear_and_feed_preserve_the_level_latch(){
        let mut w=iwdt(false,1,[2,3]);wr(&mut w,1,0x64,KEY,0);wr(&mut w,1,0x70,0,0);w.pump(10,INPUT);
        assert_eq!(w.source(),(60,false));assert_eq!(r(&mut w,1,0x74,10),4);assert_eq!(r(&mut w,1,0x78,10),0);
        wr(&mut w,1,0x70,4,10);assert_eq!(w.source(),(60,true));wr(&mut w,1,0x60,u32::MAX,10);
        assert_eq!(w.ordinary[1].stage,0);assert_eq!(w.ordinary[1].count,0);assert_eq!(r(&mut w,1,0x74,10),4);
        wr(&mut w,1,0x74,0,10);assert_eq!(w.source(),(60,true));wr(&mut w,1,0x74,4,10);assert_eq!(w.source(),(60,false));
        w.pump(20,INPUT);assert_eq!(w.source(),(60,true));wr(&mut w,1,0x7c,4,20);assert_eq!(r(&mut w,1,0x7c,20),0);
        assert_eq!(w.source(),(60,false));assert_eq!(w.deadline(),Some(35));assert_eq!(w.ordinary[1].interrupts,2);
        for(off,value)in[(0x70,1),(0x74,1),(0x78,0),(0x7c,5),(0x70,u32::MAX)]{let before=w.clone();assert!(!w.write_at(BASE[1]+off,value,20,INPUT));assert_eq!(w,before);}
        wr(&mut w,1,0x64,0,20);let before=w.clone();for(off,value)in[(0x48,u32::MAX),(0x50,0),(0x60,1)]{wr(&mut w,1,off,value,20);assert_eq!(w,before);}
        for off in [0x70,0x74,0x7c]{assert!(!w.write_at(BASE[1]+off,4,20,INPUT));assert_eq!(w,before);}
    }
    #[test]
    fn fractional_feed_divider_reset_and_none_resume_keep_old_source_phase(){
        let mut w=iwdt(false,1,[2,3]);wr(&mut w,1,0x64,KEY,0);w.pump(1,INPUT);assert_eq!(w.ordinary[1].phase,48_000_000);
        wr(&mut w,1,0x60,1,1);assert_eq!(w.ordinary[1].phase,48_000_000);assert_eq!(w.deadline(),Some(10));
        wr(&mut w,1,0x4c,0x0001_0001,2);assert_eq!(w.ordinary[1].phase,0);assert_eq!(w.deadline(),Some(12));
        let absent=ClockInputs{xtal_hz:None,..INPUT};assert!(w.pump(3,absent));assert_eq!(w.ordinary[1].phase,48_000_000);assert_eq!(w.deadline(),None);
        let state=w.ordinary[1].clone();assert!(w.pump(10_000,absent));assert_eq!(w.ordinary[1],state);assert!(w.pump(10_000,INPUT));
        assert_eq!(w.deadline(),Some(10_009));w.pump(10_008,INPUT);assert_eq!(w.ordinary[1].count,1);w.pump(10_009,INPUT);assert_eq!(w.source(),(60,true));
        assert_eq!(w.deadline(),Some(10_024));w.pump(10_024,INPUT);assert_eq!(w.take_reset_event(),Some((8,10_024)));
    }
    #[test]
    fn apb_and_functional_gate_are_independent_and_irq_is_not_a_clock_pulse(){
        let mut w=iwdt(false,1,[2,3]);assert!(w.write_at(PCR[1],0,1,INPUT));assert_eq!(w.ordinary[1].phase,48_000_000);assert_eq!(w.deadline(),None);
        w.pump(1000,INPUT);assert_eq!(w.ordinary[1].count,0);assert!(w.write_at(PCR[1],CLOCK_EN,1000,INPUT));assert_eq!(w.deadline(),Some(1009));
        let off=ClockInputs{apb_clocked:[true,false],..INPUT};assert!(w.pump(1009,off));assert_eq!(w.source(),(60,true));
        assert_eq!(w.read_at(BASE[1]+0x74,1009,off),None);assert!(!w.write_at(BASE[1]+0x60,1,1009,off));
        assert!(w.write_at(PCR[1],0,1009,off));assert_eq!(w.source(),(60,true));assert_eq!(w.deadline(),None);
        assert!(w.write_at(PCR[1],CLOCK_EN,2000,off));assert_eq!(w.deadline(),Some(2015));w.pump(2015,off);assert_eq!(w.take_reset_event(),Some((8,2015)));
    }
    #[test]
    fn unknown_activation_sources_actions_and_zero_holds_are_transactional(){
        let mut w=iwdt(false,1,[2,3]);wr(&mut w,1,0x64,KEY,0);
        for(off,value)in[(0x48,0xd81f_8000),(0x48,0xa01f_8000),(0x48,0xba1f_8000),(0x48,0xb83f_8000),(0x48,0xb81f_c000),(0x50,0),(0x54,0),(0x4c,0),(0x4c,2<<16)]{
            let before=w.clone();assert!(!w.write_at(BASE[1]+off,value,0,INPUT),"offset={off:#x} value={value:#x}");assert_eq!(w,before);
        }
        for sel in 1..=3{let before=w.clone();assert!(!w.write_at(PCR[1],CLOCK_EN|sel<<20,0,INPUT));assert_eq!(w,before);}
        let wrong=ClockInputs{xtal_hz:Some(40_000_000),..INPUT};assert!(!w.pump(1,wrong));assert_eq!(w.ordinary[1].phase,48_000_000);
        assert_eq!(w.deadline(),None);assert!(w.pump(100,INPUT));assert_eq!(w.ordinary[1].phase,48_000_000);assert_eq!(w.deadline(),Some(109));
        w.disable_virtual_profile();let before=w.clone();assert!(!w.write_at(BASE[1]+0x48,0xb81f_8000,100,INPUT));assert_eq!(w,before);
        assert_eq!(w.read_at(BASE[1]+0x74,100,INPUT),None);assert_eq!(w.source(),(60,false));
        let mut staged=cold();disarm_cold(&mut staged);wr(&mut staged,1,0x48,0x381f_8000,0);let before=staged.clone();
        assert!(!staged.write_at(BASE[1]+0x48,0xb81f_8000,0,INPUT));assert_eq!(staged,before,"EN must not make uncommitted stages active");
    }
    #[test]
    fn bounded_chunk_catchup_crosses_irq_then_core_and_timestamp_overflow_is_idle(){
        let mut a=iwdt(false,3,[17,19]);let mut b=a.clone();a.pump(u64::MAX,INPUT);
        for at in [1,20,254,255,300,539,540,u64::MAX]{b.pump(at,INPUT);}assert_eq!(a,b);assert_eq!(a.take_reset_event(),Some((8,540)));
        assert_eq!(a.ordinary[1].interrupts,1);assert_eq!(a.ordinary[1].expiries,1);
        let mut late=cold();disarm_cold(&mut late);late.pump(u64::MAX-10,INPUT);init_iwdt(&mut late,u64::MAX-10,1,[2,3]);
        assert_eq!(late.deadline(),Some(u64::MAX));assert!(late.pump(u64::MAX,INPUT));assert_eq!(late.source(),(60,true));
        assert_eq!(late.deadline(),None);assert_eq!(late.take_reset_event(),None);assert_eq!(late.ordinary[1].count,0);
        let mut max=iwdt(false,65535,[u32::MAX;2]);assert_eq!(max.deadline(),Some(1_407_353_408_389_125));
        max.pump(u64::MAX,INPUT);assert_eq!(max.take_reset_event(),Some((8,2_814_706_816_778_250)));
    }
    #[test]
    fn cold_and_ordinary_core_races_commit_only_through_the_first_reset(){
        let mut earlier=iwdt(true,1,[5_000_000,15_000_000]);earlier.pump(u64::MAX,INPUT);
        assert_eq!(earlier.take_reset_event(),Some((8,100_000_000)));assert_eq!(earlier.profile.as_ref().unwrap().count,20_000_000);
        assert!(!earlier.profile.as_ref().unwrap().tripped);let before=earlier.clone();assert!(earlier.pump(u64::MAX,ClockInputs::default()));assert_eq!(earlier,before);
        let mut later=iwdt(true,1,[10_000_000,20_000_000]);later.pump(u64::MAX,INPUT);assert_eq!(later.take_reset_event(),Some((7,130_000_000)));
        assert_eq!(later.ordinary[1].count,16_000_000);assert_eq!(later.ordinary[1].expiries,0);assert_eq!(later.ordinary[1].interrupts,1);
        let mut equal=iwdt(true,1,[10_000_000,16_000_000]);equal.pump(u64::MAX,INPUT);assert_eq!(equal.take_reset_event(),Some((7,130_000_000)));
        assert!(equal.ordinary[1].tripped);assert_eq!(equal.ordinary[1].expiries,1);assert_eq!(equal.take_reset_event(),None);
    }
    #[test]
    fn reset_pending_barriers_and_controller_domain_resets_keep_their_qualification(){
        let mut w=iwdt(false,1,[2,3]);w.pump(10,INPUT);let before=w.clone();w.reset_domains(10,false,false,false);assert_eq!(w,before);
        let reset=ClockInputs{in_reset:[false,true],..INPUT};assert!(w.pump(11,reset));assert_eq!(w.source(),(60,false));assert_eq!(w.ordinary[1],Ordinary::default());
        w.pump(12,INPUT);assert_eq!(r(&mut w,1,0x48,12),POR[0]);assert_eq!(w.deadline(),None);
        let mut w=iwdt(false,1,[2,3]);w.pump(25,INPUT);let before=w.clone();assert!(!w.write_at(BASE[1]+0x60,1,25,INPUT));assert_eq!(w,before);
        assert_eq!(w.take_reset_event(),Some((8,25)));let before=w.clone();assert!(!w.write_at(BASE[1]+0x48,0,26,INPUT));assert_eq!(w,before);
        assert!(w.pump(u64::MAX,ClockInputs::default()));assert_eq!(w,before);w.reset_domains(25,true,false,false);
        assert_eq!(w.clock,[CLOCK_EN;2]);assert_eq!(r(&mut w,1,0x48,25),POR[0]);assert_eq!(r(&mut w,1,0x74,25),0);assert_eq!(w.deadline(),None);
        w.reset_domains(30,false,false,true);assert!(!w.virtual_profile_enabled());w.enable_cold_spi_boot(30,INPUT).unwrap();assert_eq!(w.deadline(),Some(130_000_030));
    }
    #[test]
    fn shorter_hold_update_expires_at_the_commit_boundary_without_backdating(){
        let mut w=iwdt(false,1,[10,20]);w.pump(34,INPUT);assert_eq!(w.ordinary[1].count,6);assert_eq!(w.ordinary[1].phase,192_000_000);
        wr(&mut w,1,0x64,KEY,34);wr(&mut w,1,0x50,2,34);assert_eq!(w.deadline(),Some(50));
        wr(&mut w,1,0x48,0xb85f_8000,34);assert_eq!(w.ordinary[1].stage,1);assert_eq!(w.ordinary[1].count,0);assert_eq!(w.ordinary[1].interrupts,1);
        assert_eq!(w.ordinary[1].phase,192_000_000);assert_eq!(w.deadline(),Some(130));assert_eq!(w.ordinary[1].feeds,1);
    }
    #[test]
    fn a_disabled_prescaler_change_needs_divider_reset_to_discard_no_fraction(){
        let mut w=iwdt(false,1,[2,3]);wr(&mut w,1,0x64,KEY,0);wr(&mut w,1,0x48,0x381f_8000,1);
        assert_eq!(w.ordinary[1].phase,48_000_000);wr(&mut w,1,0x4c,2<<16,1);let before=w.clone();
        assert!(!w.write_at(BASE[1]+0x48,0x385f_8000,1,INPUT));assert_eq!(w,before);
        wr(&mut w,1,0x4c,2<<16|1,1);wr(&mut w,1,0x48,0x385f_8000,1);wr(&mut w,1,0x60,1,1);wr(&mut w,1,0x48,0xb81f_8000,1);
        assert_eq!(w.ordinary[1].phase,0);assert_eq!(w.deadline(),Some(21));
    }
    fn twdt(prescale:u32,holds:[u32;2])->TimgWdt{let mut w=cold();disarm_cold(&mut w);init_mwdt(&mut w,0,0,prescale,holds);w}
    #[test]
    fn original_twdt_allocate_then_restart_runs_only_after_real_direct_enable(){
        let mut w=cold();disarm_cold(&mut w);assert!(w.cold_handoff);
        w.pump(7,ClockInputs{in_reset:[true,false],..INPUT});w.pump(8,INPUT);prepare_mwdt(&mut w,0,8,24_000,[10_000,20_000]);wr(&mut w,0,0x64,0,8);
        assert_eq!(w.group[0].active,[0x381f_8000,0x5dc0_0000,10_000,20_000,1_048_575,1_048_575]);
        assert_eq!(w.group[0].updates,9);assert_eq!(w.clock,[CLOCK_EN;2]);assert_eq!(w.deadline(),None);assert_eq!(w.sources(),[(58,false),(60,false)]);
        // TWDT restart: unlock, ROM-enable's FEED/EN, a second real FEED, lock.
        for(off,value)in[(0x64,KEY),(0x60,1),(0x48,0xb81f_8000),(0x60,1),(0x64,0)]{wr(&mut w,0,off,value,10);}
        assert_eq!(w.ordinary[0].feeds,2);assert_eq!(w.profile.as_ref().unwrap().feeds,0);assert_eq!(w.deadline(),Some(1_200_000_010));
        w.pump(1_200_000_010,INPUT);assert_eq!(w.sources(),[(58,true),(60,false)]);assert_eq!(w.deadline(),Some(3_600_000_010));
        w.pump(u64::MAX,INPUT);assert_eq!(w.take_reset_event(),Some((7,3_600_000_010)));assert_eq!(w.ordinary[0].expiries,1);
    }
    #[test]
    fn a_group_reset_or_staged_flashboot_clear_cannot_grant_tg0_handoff(){
        let mut staged=cold();wr(&mut staged,0,0x48,0x0004_8000,0);assert!(!staged.cold_handoff);
        assert_eq!(staged.read_at(BASE[0]+0x7c,0,INPUT),None);assert_eq!(staged.sources(),[(58,false),(60,false)]);
        let mut locked=cold();wr(&mut locked,0,0x64,0,0);wr(&mut locked,0,0x48,0x0044_8000,0);assert!(!locked.cold_handoff);assert_eq!(locked.deadline(),Some(130_000_000));
        let mut reset=cold();reset.pump(0,ClockInputs{in_reset:[true,false],..INPUT});reset.pump(0,INPUT);
        for(off,value)in[(0x48,0x381f_8000),(0x4c,1<<16),(0x50,2),(0x54,3),(0x48,0x385f_8000)]{wr(&mut reset,0,off,value,0);}
        assert!(!reset.cold_handoff);let before=reset.clone();assert!(!reset.write_at(BASE[0]+0x48,0xb81f_8000,0,INPUT));assert_eq!(reset,before);
        assert!(!reset.write_at(BASE[0]+0x70,4,0,INPUT));assert_eq!(reset,before);
        let mut valid=twdt(1,[2,3]);assert!(valid.cold_handoff);valid.reset_domains(0,true,false,false);assert!(valid.cold_handoff);
        valid.reset_domains(1,false,false,true);assert!(!valid.cold_handoff);assert_eq!(valid.sources(),[(58,false),(60,false)]);
        valid.enable_cold_spi_boot(1,INPUT).unwrap();assert!(!valid.cold_handoff);valid.disable_virtual_profile();assert!(!valid.cold_handoff);
    }
    #[test]
    fn ordinary_instances_share_one_engine_but_have_independent_irqs_and_earliest_resets(){
        let mut w=twdt(1,[2,3]);init_iwdt(&mut w,0,1,[1,6]);assert_eq!(w.deadline(),Some(5));
        w.pump(5,INPUT);assert_eq!(w.sources(),[(58,false),(60,true)]);assert_eq!(w.deadline(),Some(10));
        w.pump(10,INPUT);assert_eq!(w.sources(),[(58,true),(60,true)]);assert_eq!(w.deadline(),Some(25));
        w.pump(u64::MAX,INPUT);assert_eq!(w.take_reset_event(),Some((7,25)));assert_eq!(w.ordinary[1].count,4);assert!(!w.ordinary[1].tripped);
        let mut first=twdt(1,[2,6]);init_iwdt(&mut first,0,1,[1,2]);first.pump(u64::MAX,INPUT);assert_eq!(first.take_reset_event(),Some((8,15)));
        assert_eq!(first.ordinary[0].count,1);assert!(!first.ordinary[0].tripped);
        let mut equal=twdt(1,[2,3]);init_iwdt(&mut equal,0,1,[1,4]);equal.pump(u64::MAX,INPUT);assert_eq!(equal.take_reset_event(),Some((7,25)));
        assert!(equal.ordinary[0].tripped&&equal.ordinary[1].tripped);
    }
    #[test]
    fn original_twdt_interrupt_handler_feed_and_clear_are_separate_real_operations(){
        let mut w=twdt(1,[2,4]);w.pump(10,INPUT);assert_eq!(w.sources()[0],(58,true));assert_eq!(r(&mut w,0,0x74,10),4);
        // timeout_triggered: KEY, handle_intr FEED then CLR|4, lock.
        wr(&mut w,0,0x64,KEY,10);wr(&mut w,0,0x60,1,10);assert_eq!(w.sources()[0],(58,true));assert_eq!(w.ordinary[0].stage,0);
        wr(&mut w,0,0x7c,4,10);wr(&mut w,0,0x64,0,10);assert_eq!(w.sources()[0],(58,false));assert_eq!(w.deadline(),Some(20));
        w.pump(20,INPUT);assert_eq!(w.sources()[0],(58,true));let before=w.clone();wr(&mut w,0,0x60,1,20);assert_eq!(w,before,"locked feed ignored");
        assert!(!w.write_at(BASE[0]+0x7c,4,20,INPUT));assert_eq!(w,before);w.pump(40,INPUT);assert_eq!(w.take_reset_event(),Some((7,40)));
    }
    #[test]
    fn tg0_gate_loss_resume_group_reset_and_apb_qualification_leave_tg1_independent(){
        let mut w=twdt(1,[2,3]);init_iwdt(&mut w,0,1,[1000,1000]);assert!(w.write_at(PCR[0],0,1,INPUT));
        w.pump(1000,INPUT);assert_eq!(w.ordinary[0].phase,48_000_000);assert_eq!(w.ordinary[0].count,0);assert_eq!(w.ordinary[1].count,200);
        assert!(w.write_at(PCR[0],CLOCK_EN,1000,INPUT));assert_eq!(w.deadline(),Some(1009));
        let absent=ClockInputs{xtal_hz:None,..INPUT};w.pump(1001,absent);assert_eq!(w.ordinary[0].phase,96_000_000);
        w.pump(2000,absent);w.pump(2000,INPUT);assert_eq!(w.deadline(),Some(2008));
        let off=ClockInputs{apb_clocked:[false,true],..INPUT};w.pump(2008,off);assert_eq!(w.sources()[0],(58,true));
        assert_eq!(w.read_at(BASE[0]+0x74,2008,off),None);assert!(!w.write_at(BASE[0]+0x60,1,2008,off));
        let retained=w.ordinary[1].clone();w.pump(2008,ClockInputs{in_reset:[true,false],..INPUT});assert_eq!(w.ordinary[0],Ordinary::default());assert_eq!(w.ordinary[1],retained);
        w.pump(2008,INPUT);assert!(w.cold_handoff);assert_eq!(w.sources(),[(58,false),(60,false)]);assert_eq!(w.deadline(),Some(5999));
    }
    #[test]
    fn tg0_uses_the_same_strict_invalid_actions_and_chunk_bounded_arithmetic(){
        let mut w=twdt(3,[17,19]);wr(&mut w,0,0x64,KEY,0);
        for(off,value)in[(0x48,0xd81f_8000),(0x48,0xba1f_8000),(0x50,0),(0x54,0),(0x4c,0),(0x78,0),(0x70,1)]{
            let before=w.clone();assert!(!w.write_at(BASE[0]+off,value,0,INPUT));assert_eq!(w,before);
        }
        for sel in 1..=3{let before=w.clone();assert!(!w.write_at(PCR[0],CLOCK_EN|sel<<20,0,INPUT));assert_eq!(w,before);}
        let mut chunks=w.clone();w.pump(u64::MAX,INPUT);for at in [1,20,254,255,300,539,540,u64::MAX]{chunks.pump(at,INPUT);}assert_eq!(w,chunks);
        assert_eq!(w.take_reset_event(),Some((7,540)));assert_eq!(w.profile.as_ref().unwrap().expiries,0);assert_eq!(w.ordinary[0].interrupts,1);
    }
}
