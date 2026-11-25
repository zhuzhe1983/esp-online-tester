//! C5 LP_WDT registers and opt-in virtual analog SWD with real feed pulses.
//! IDF v5.5.3/v5.5.4 lp_wdt_reg.h/lpwdt_ll.h, TRM1.1 sections9.2/16.3.
//! Analog950ms/850ms timings are explicit virtual inputs, not silicon values.
//! LP_DYN_FAST defaults to XTAL/2=24MHz; its observed rate clocks only feed
//! pulses. The CPU timeline is nominal240MHz; RWDT has its own slow source.
//! Logical reset events do not claim physical pulse/stall timing accuracy.
//! Reset event ordering: docs/evidence/c5-watchdog-order-2026-11-24.
#[path="lp_wdt_rwdt.rs"]mod rwdt;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum LpWdtReset{Swd,RwdtCpu,RwdtCore,RwdtSystem}
impl LpWdtReset{pub fn reason(self)->u8{match self{Self::Swd=>0x12,Self::RwdtCpu=>0x0d,Self::RwdtCore=>0x09,Self::RwdtSystem=>0x10}}}
const KEY:u32=0x50d8_3aa1;const AUTO:u32=1<<18;const DISABLE:u32=1<<30;
const SWD_RW:u32=AUTO|(0x3ff<<20)|DISABLE;const SWD_IRQ:u32=1<<30;
const REQUEST:u64=204_000_000;const EXPIRE:u64=228_000_000;const FAST_TIMEBASE:u64=240_000_000;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct SwdPulse{at:u64,remaining:u64,automatic:bool}
impl SwdPulse{
    fn new(at:u64,ticks:u64,automatic:bool)->Self{Self{at,remaining:ticks*FAST_TIMEBASE,automatic}}
    fn deadline(&self,hz:Option<u32>)->Option<u64>{self.at.checked_add(self.remaining.div_ceil(u64::from(hz?)))}
    fn advance(&mut self,cycle:u64,hz:Option<u32>)->Option<u64>{
        let cycle=cycle.max(self.at);if let Some(at)=self.deadline(hz).filter(|&at|at<=cycle){return Some(at);}
        let consumed=u128::from(cycle-self.at)*u128::from(hz.unwrap_or(0));
        self.remaining=self.remaining.saturating_sub(consumed.min(u128::from(self.remaining))as u64);self.at=cycle;None
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
struct Swd{now:u64,origin:u64,requested:bool,tripped:bool,pulse:Option<SwdPulse>,requests:u64,automatic:u64,software:u64,resets:u64}
impl Swd{fn new(cycle:u64)->Self{Self{now:cycle,origin:cycle,requested:false,tripped:false,pulse:None,requests:0,automatic:0,software:0,resets:0}}}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct LpWdt{
    config:u32,hold:[u32;4],key:u32,swd:u32,swd_key:u32,swd_flag:bool,date:u32,
    clock:Option<Swd>,int_raw:u32,int_ena:u32,swd_reset_at:Option<u64>,pub(crate) deadline:Option<u64>,
    rwdt:Option<rwdt::Rwdt>,dispatched:Option<(LpWdtReset,u64)>,fast_hz:Option<u32>,
}
impl LpWdt{
    pub fn new()->Self{Self{config:0x13214,hold:[200_000,80_000,4095,4095],key:0,swd:300<<20,swd_key:0,swd_flag:false,date:34_676_864,
        clock:None,int_raw:0,int_ena:0,swd_reset_at:None,deadline:None,rwdt:None,dispatched:None,fast_hz:Some(24_000_000)}}
    /// Strict validated-ROM cold-power-on selection only. SWD is enabled at POR;
    /// start its age here, not at the later AUTO write.
    pub fn enable_virtual_swd(&mut self,cycle:u64){if self.clock.is_none(){self.clock=Some(Swd::new(cycle));self.schedule();}}
    /// Select the legacy strict register subset without changing readable bits.
    pub fn disable_virtual_swd(&mut self){self.clock=None;self.swd_reset_at=None;if self.dispatched.is_some_and(|e|e.0==LpWdtReset::Swd){self.dispatched=None;}self.schedule();}
    /// Strict validated-ROM selection. An unavailable/uncalibrated slow source
    /// produces no ticks; its rate is never defaulted from a requested mux.
    pub fn enable_virtual_rwdt(&mut self,cycle:u64,delay_sel:u8)->bool{
        if delay_sel>3||!rwdt::Rwdt::valid(self.config,&self.hold){return false;}
        if let Some(r)=&self.rwdt{return r.delay==delay_sel;}self.rwdt=Some(rwdt::Rwdt::new(cycle,delay_sel));self.schedule();true
    }
    pub fn disable_virtual_rwdt(&mut self){self.rwdt=None;if self.dispatched.is_some_and(|e|e.0!=LpWdtReset::Swd){self.dispatched=None;}self.schedule();}
    pub fn set_slow_clock(&mut self,cycle:u64,hz:Option<u32>)->bool{
        if hz.is_some_and(|v|v==0||v>240_000_000)||self.rwdt.is_none(){return false;}
        self.pump(cycle);let blocked=self.dispatched.is_some()||self.first_reset_event().is_some();
        if let Some(r)=&mut self.rwdt{if blocked&&r.hz!=hz{return false;}r.hz=hz;}self.schedule();true
    }
    /// Actual LP_DYN_FAST observation, independent of RWDT and analog age.
    /// TRM16.3 / IDF5.5.3/5.5.4 lp_clkrst_reg.h FAST_CLK_SEL[3:2]:
    /// RC_FAST,XTAL/2,XTAL. The clock owner supplies power/tuning observations.
    pub fn set_fast_clock(&mut self,cycle:u64,hz:Option<u32>)->bool{
        if !matches!(hz,None|Some(17_500_000|24_000_000|48_000_000))||self.clock.is_none(){return false;}
        if self.dispatched.is_some()||self.first_reset_event().is_some(){return self.fast_hz==hz;}
        self.pump(cycle);if self.dispatched.is_some()||self.first_reset_event().is_some(){return self.fast_hz==hz;}
        self.fast_hz=hz;self.schedule();true
    }
    /// Observation of the separate LP_CLKRST CPU reset gate; default false.
    /// The caller pumps before changing a reset-domain control.
    pub fn set_cpu_reset_enabled(&mut self,enabled:bool){if let Some(r)=&mut self.rwdt{r.cpu_reset=enabled;}self.schedule();}
    pub fn irq(&self)->bool{self.int_raw&self.int_ena&(if self.clock.is_some(){SWD_IRQ}else{0}|if self.rwdt.is_some(){rwdt::IRQ}else{0})!=0}
    /// Historical SWD-only controller tests; no production dispatch occurs.
    #[cfg(test)]
    pub fn take_reset(&mut self)->bool{let reset=self.swd_reset_at.take().is_some();self.schedule();reset}
    #[cfg(test)]
    pub fn take_reset_cause(&mut self)->Option<LpWdtReset>{self.take_reset_event().map(|e|e.0)}
    fn first_reset_event(&self)->Option<(LpWdtReset,u64)>{
        let swd=self.swd_reset_at.map(|at|(LpWdtReset::Swd,at));
        let rwdt=self.rwdt.as_ref().and_then(|r|r.pending).map(|(cause,at,_)|(cause,at));
        match(swd,rwdt){(Some(a),Some(b))=>Some(if b.1<a.1{b}else{a}),(a,b)=>a.or(b)}
    }
    /// One timestamped event per owner reset, independent of pump order.
    /// TRM9.1-1 supplies priority only when the event times are equal.
    pub fn take_reset_event(&mut self)->Option<(LpWdtReset,u64)>{
        if self.dispatched.is_some(){return None;}
        let event=self.first_reset_event()?;self.dispatched=Some(event);self.schedule();Some(event)
    }
    fn width_ticks(&self)->u64{((self.swd>>20)&0x3ff)as u64}
    fn width(&self)->Option<u64>{Some((self.width_ticks()*FAST_TIMEBASE).div_ceil(u64::from(self.fast_hz?)))}
    fn schedule(&mut self){
        if self.dispatched.is_some(){self.deadline=None;return;}
        if let Some((_,at))=self.first_reset_event(){self.deadline=Some(at);return;}
        let swd=self.clock.as_ref().and_then(|s|{
            if self.swd&DISABLE!=0{return None;}
            s.pulse.and_then(|p|p.deadline(self.fast_hz)).into_iter().chain((!s.tripped).then(||s.origin.checked_add(if s.requested{EXPIRE}else{REQUEST})).flatten()).min()
        });
        self.deadline=swd.into_iter().chain(self.rwdt.as_ref().and_then(|r|r.deadline(self.config,&self.hold))).min();
    }
    fn trip(&mut self,s:&mut Swd,at:u64){s.tripped=true;s.pulse=None;s.resets=s.resets.saturating_add(1);self.swd_flag=true;self.swd_reset_at.get_or_insert(at);}
    /// Bounded lazy catch-up. Stable automatic cycles aggregate arithmetically;
    /// no queue or loop proportional to elapsed time.
    pub fn pump(&mut self,cycle:u64){
        if !self.modeled()||self.dispatched.is_some()||self.first_reset_event().is_some(){return;}
        // Fixed-size preview: each individual engine already has arithmetic
        // catch-up. Commit no state after the first reset's actual timestamp.
        let mut next=self.clone();next.advance_to(cycle);
        if let Some((_,at))=next.first_reset_event(){self.advance_to(at);}else{*self=next;}
    }
    fn advance_to(&mut self,cycle:u64){self.pump_swd(cycle);if let Some(r)=&mut self.rwdt{if r.pump(cycle,self.config,&self.hold){self.int_raw|=rwdt::IRQ;}}self.schedule();}
    fn pump_pulse(&mut self,s:&mut Swd,cycle:u64){
        let Some(mut pulse)=s.pulse else{return;};let due=pulse.deadline(self.fast_hz);
        // The analog warning remains independent of a pulse in progress.
        if !s.tripped&&!s.requested&&s.origin.checked_add(REQUEST).is_some_and(|at|at<=cycle&&due.is_none_or(|end|at<end)){
            s.requested=true;s.requests=s.requests.saturating_add(1);self.int_raw|=SWD_IRQ;
        }
        if let Some(end)=s.origin.checked_add(EXPIRE).filter(|&end|!s.tripped&&end<=cycle&&due.is_none_or(|at|end<=at)){self.trip(s,end);}
        else if let Some(at)=pulse.advance(cycle,self.fast_hz){
            if pulse.automatic{s.automatic=s.automatic.saturating_add(1);}else{s.software=s.software.saturating_add(1);}
            s.origin=at;s.requested=false;s.tripped=false;s.pulse=None;
        }else{s.pulse=Some(pulse);}
    }
    fn pump_swd(&mut self,cycle:u64){
        let Some(mut s)=self.clock.take()else{return;};let cycle=cycle.max(s.now);
        if self.swd&DISABLE==0{
            // WD_INTR remains asserted until the actual pulse completes.
            if s.requested{self.int_raw|=SWD_IRQ;}self.pump_pulse(&mut s,cycle);
            if !s.tripped&&s.pulse.is_none(){
                if self.swd&AUTO!=0&&!s.requested{
                    // Each automatic pulse starts a fresh functional phase;
                    // integer completion rounding is not a physical clock claim.
                    if let Some(width)=self.width(){let period=REQUEST+width;let count=cycle.saturating_sub(s.origin)/period;
                        if count!=0{s.origin+=count*period;s.requests=s.requests.saturating_add(count);s.automatic=s.automatic.saturating_add(count);self.int_raw|=SWD_IRQ;}
                    }
                }
                if let Some(at)=s.origin.checked_add(REQUEST).filter(|&at|!s.requested&&at<=cycle){
                    s.requested=true;s.requests=s.requests.saturating_add(1);self.int_raw|=SWD_IRQ;
                    if self.swd&AUTO!=0{s.pulse=Some(SwdPulse::new(at,self.width_ticks(),true));}
                }
                self.pump_pulse(&mut s,cycle);
                if let Some(at)=s.origin.checked_add(EXPIRE).filter(|&at|!s.tripped&&s.pulse.is_none()&&at<=cycle){self.trip(&mut s,at);}
            }
        }
        s.now=cycle;self.clock=Some(s);self.schedule();
    }
    /// CPU/Core reset retain LP controller and analog age. LP reset clears
    /// controller state/pulses but preserves analog request/age/trip/reset flag.
    /// LP reset retains the external fast observation; its clock owner refreshes
    /// it at the actual reset boundary. Analog reset restores default24MHz/age.
    pub fn reset_domains(&mut self,cycle:u64,lp:bool,analog:bool){
        self.pump(cycle);self.dispatched=None;
        // The applied reset also acknowledges coincident lower-priority
        // requests. Preserve later events; do not catch up again in this reset.
        if self.swd_reset_at.is_some_and(|at|at<=cycle){self.swd_reset_at=None;}
        if let Some(r)=&mut self.rwdt{if r.pending.is_some_and(|e|e.1<=cycle){r.pending=None;}}
        self.schedule();if !lp&&!analog{return;}
        let enabled=self.swd&DISABLE==0;let mut clock=self.clock.take();let flag=self.swd_flag;let rwdt=self.rwdt.take();let fast_hz=self.fast_hz;
        *self=Self::new();if analog{if clock.is_some(){clock=Some(Swd::new(cycle));}}
        else{self.fast_hz=fast_hz;self.swd_flag=flag;if let Some(s)=&mut clock{s.pulse=None;if !enabled{s.origin=cycle;s.requested=false;s.tripped=false;}}}
        self.clock=clock;if let Some(old)=rwdt{let mut r=rwdt::Rwdt::new(cycle,old.delay);if !analog{r.hz=old.hz;r.cpu_reset=old.cpu_reset;}self.rwdt=Some(r);}
        if self.clock.as_ref().is_some_and(|s|s.requested){self.int_raw|=SWD_IRQ;}self.schedule();
    }
    fn modeled(&self)->bool{self.clock.is_some()||self.rwdt.is_some()}
    pub fn read(&self,off:u32)->Option<u32>{Some(match off{
        0=>self.config,4..=0x10 if off.is_multiple_of(4)=>self.hold[(off/4-1)as usize],0x14=>0,0x18=>self.key,
        0x1c=>self.swd|u32::from(self.swd_flag),0x20=>self.swd_key,0x3fc=>self.date,
        0x24 if self.modeled()=>self.int_raw,0x28 if self.modeled()=>self.int_raw&self.int_ena,
        0x2c if self.modeled()=>self.int_ena,0x30 if self.modeled()=>0,_=>return None,
    })}
    fn write_swd(&mut self,v:u32)->bool{
        if self.swd_key!=KEY{return true;}let next=v&SWD_RW;
        if let Some(s)=&self.clock{
            if v&!(SWD_RW|1|(1<<19)|(1<<31))!=0||next&(0x3ff<<20)==0{return false;}
            if s.pulse.is_some()&&((self.swd^next)&(0x3ff<<20)!=0||v&(1<<31)!=0){return false;}
            if v&(1<<31)!=0&&next&DISABLE!=0{return false;}
        }else if next&DISABLE==0&&(next&AUTO!=0||(self.swd^next)&DISABLE!=0||v&(1<<31)!=0){return false;
        }
        let was_enabled=self.swd&DISABLE==0;self.swd=next;if v&(1<<19)!=0{self.swd_flag=false;}
        let width=self.width_ticks();if let Some(s)=&mut self.clock{
            if next&DISABLE!=0{s.pulse=None;s.requested=false;s.tripped=false;}
            else{
                if !was_enabled{s.origin=s.now;s.requested=false;s.tripped=false;}
                let software=v&(1<<31)!=0;
                if s.pulse.is_none()&&(software||next&AUTO!=0&&s.requested){s.pulse=Some(SwdPulse::new(s.now,width,!software));}
            }
        }
        self.schedule();true
    }
    pub fn write(&mut self,off:u32,v:u32)->bool{
        match off{
            0x18=>self.key=v,0x20=>self.swd_key=v,
            0x3fc=>{if self.modeled()&&v&(1<<31)!=0{return false;}self.date=v;},
            0=>{if self.key==KEY{
                if let Some(r)=&self.rwdt{
                    if !rwdt::Rwdt::valid(v,&self.hold)||rwdt::Rwdt::active(v)&&r.hz.is_none(){return false;}
                }else if v&((1<<31)|0x7ff8_0000|(1<<8))!=0{return false;}
                self.config=v;self.schedule();
            }},
            4..=0x10 if off.is_multiple_of(4)=>{if self.key==KEY{
                let n=(off/4-1)as usize;if let Some(r)=&self.rwdt{if rwdt::Rwdt::active(self.config)&&(v==0||n==r.stage&&(u64::from(v)<<if n==0{r.delay+1}else{0})<=r.count){return false;}}
                self.hold[n]=v;self.schedule();
            }},
            0x14=>{if self.key==KEY{if let Some(r)=&mut self.rwdt{
                if v&!(1<<31)!=0||v&(1<<31)!=0&&rwdt::Rwdt::active(self.config)&&r.hz.is_none(){return false;}
                if v&(1<<31)!=0{r.feed();self.schedule();}
            }}},
            0x1c=>return self.write_swd(v),
            0x24|0x30 if self.modeled()=>{
                if v&!(SWD_IRQ|(1<<31))!=0{return false;}
                let clear=if self.rwdt.is_some()&&self.key!=KEY{v&!rwdt::IRQ}else{v};self.int_raw&=!clear;
                if self.swd&DISABLE==0&&self.clock.as_ref().is_some_and(|s|s.requested){self.int_raw|=SWD_IRQ;}
            },
            0x28 if self.modeled()=>{}, // RO, never changes interrupt state.
            0x2c if self.modeled()=>{let mask=SWD_IRQ|if self.rwdt.is_some(){rwdt::IRQ}else{0};if v&!mask!=0{return false;}self.int_ena=if self.rwdt.is_some()&&self.key!=KEY{(v&!rwdt::IRQ)|(self.int_ena&rwdt::IRQ)}else{v};},
            _=>return false,
        }true
    }
    pub fn report(&self)->String{
        let fast=match self.fast_hz{Some(24_000_000)=>"24MHz",Some(17_500_000)=>"17.5MHz",Some(48_000_000)=>"48MHz",_=>"unavailable"};
        let swd=self.clock.as_ref().map_or_else(String::new,|s|format!("SWD=virtual950ms/request850ms lp_fast={fast} enabled={} auto={} requests={} auto_feeds={} software_feeds={} resets={} pulse={}",
            u8::from(self.swd&DISABLE==0),u8::from(self.swd&AUTO!=0),s.requests,s.automatic,s.software,s.resets,u8::from(s.pulse.is_some())));
        self.rwdt.as_ref().map_or(swd.clone(),|r|format!("{swd}; RWDT=source_clocked hz={:?} delay={} active={} stage={} count={} feeds={} timeouts={:?} pending={:?} physical_reset_pulse=unmodeled",r.hz,r.delay,u8::from(rwdt::Rwdt::active(self.config)),r.stage,r.count,r.feeds,r.timeouts,r.pending))
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn put(w:&mut LpWdt,off:u32,v:u32){assert!(w.write(off,v),"write {off:#x}={v:#x}");}
    fn active()->LpWdt{let mut w=LpWdt::new();w.enable_virtual_swd(0);put(&mut w,0x20,KEY);w}
    fn config(w:&mut LpWdt,set:u32,clear:u32){let v=w.read(0x1c).unwrap();put(w,0x1c,(v&!clear)|set);}
    #[test]fn only_the_write_trigger_can_clear_an_asserted_flag_and_keys_are_independent(){
        let mut w=LpWdt::new();w.swd_flag=true;put(&mut w,0x18,KEY);put(&mut w,0x1c,(300<<20)|(1<<19));assert_eq!(w.read(0x1c),Some((300<<20)|1));
        put(&mut w,0x20,KEY);put(&mut w,0x1c,300<<20);assert_eq!(w.read(0x1c),Some((300<<20)|1));put(&mut w,0x1c,(300<<20)|(1<<19));assert_eq!(w.read(0x1c),Some(300<<20));
        put(&mut w,0x1c,(300<<20)|1);assert_eq!(w.read(0x1c),Some(300<<20));
    }
    #[test]fn por_age_and_original_bootloader_auto_write_do_not_fabricate_a_feed(){
        let mut w=active();assert_eq!(w.deadline,Some(REQUEST));w.pump(422_856);assert!(!w.take_reset());
        config(&mut w,AUTO,0);assert_eq!(w.read(0x1c),Some(0x12c4_0000));put(&mut w,0x20,0);
        assert_eq!(w.clock.as_ref().unwrap().automatic,0);w.pump(REQUEST);assert_eq!(w.deadline,Some(REQUEST+3000));
        assert_eq!(w.read(0x24),Some(SWD_IRQ));assert!(!w.irq());assert_eq!(w.clock.as_ref().unwrap().automatic,0);
        w.pump(REQUEST+2999);assert_eq!(w.clock.as_ref().unwrap().automatic,0);w.pump(REQUEST+3000);
        assert_eq!(w.clock.as_ref().unwrap().automatic,1);assert_eq!(w.deadline,Some(2*REQUEST+3000));assert!(!w.take_reset());
        for _ in 0..8{w.read(0x1c);w.read(0x24);}assert_eq!(w.clock.as_ref().unwrap().automatic,1);
    }
    #[test]fn software_feed_pulse_restarts_age_and_prevents_only_future_timeout(){
        let mut w=active();put(&mut w,0x2c,SWD_IRQ);w.pump(REQUEST);assert!(w.irq());
        config(&mut w,1<<31,0);assert_eq!(w.read(0x1c),Some(300<<20));w.pump(REQUEST+3000);
        assert_eq!(w.clock.as_ref().unwrap().software,1);assert_eq!(w.deadline,Some(2*REQUEST+3000));put(&mut w,0x30,SWD_IRQ);assert!(!w.irq());
        w.pump(REQUEST+3000+EXPIRE);assert!(w.take_reset());assert!(!w.take_reset());assert_eq!(w.read(0x1c),Some((300<<20)|1));
        let resets=w.clock.as_ref().unwrap().resets;w.pump(u64::MAX);assert_eq!(w.clock.as_ref().unwrap().resets,resets);
        let mut late=active();late.pump(EXPIRE-1);config(&mut late,1<<31,0);late.pump(EXPIRE);
        assert!(late.take_reset());assert_eq!(late.clock.as_ref().unwrap().software,0);assert_eq!(late.clock.as_ref().unwrap().resets,1);
    }
    #[test]fn automatic_catch_up_is_bounded_and_disable_or_auto_off_have_real_effects(){
        let mut w=active();config(&mut w,AUTO,0);let period=REQUEST+3000;let cycles=period*1_000_000+17;
        w.pump(cycles);let s=w.clock.as_ref().unwrap();assert_eq!((s.requests,s.automatic,s.software,s.resets),(1_000_000,1_000_000,0,0));
        assert_eq!(s.origin,period*1_000_000);assert_eq!(w.deadline,Some(s.origin+REQUEST));
        config(&mut w,0,AUTO);let origin=w.clock.as_ref().unwrap().origin;w.pump(origin+EXPIRE);assert!(w.take_reset());
        config(&mut w,DISABLE,0);assert_eq!(w.deadline,None);w.pump(origin+EXPIRE*20);assert!(!w.take_reset());
        config(&mut w,AUTO,DISABLE);let now=w.clock.as_ref().unwrap().now;assert_eq!(w.deadline,Some(now+REQUEST));w.pump(now+period);
        assert_eq!(w.clock.as_ref().unwrap().automatic,1_000_001);assert!(w.report().contains("SWD=virtual950ms/request850ms lp_fast=24MHz"));
    }
    #[test]fn pulse_width_and_irregular_pumps_preserve_real_event_phase(){
        for width in[1,300,1023]{
            let mut once=active();put(&mut once,0x1c,(width<<20)|AUTO);let mut chunks=once.clone();let period=REQUEST+width as u64*10;
            for at in[REQUEST-1,REQUEST,period-1,period,period+17,2*REQUEST+width as u64*10+3,period*10+123]{chunks.pump(at);}
            once.pump(period*10+123);assert_eq!(chunks,once);
            let mut w=active();put(&mut w,0x1c,(width<<20)|AUTO);w.pump(REQUEST);config(&mut w,0,AUTO);
            w.pump(period-1);assert_eq!(w.clock.as_ref().unwrap().automatic,0);w.pump(period);assert_eq!(w.clock.as_ref().unwrap().automatic,1);
            w.pump(period+EXPIRE);assert!(w.take_reset());
        }
    }
    #[test]fn irq_mask_ack_and_write_triggers_never_set_ro_flags_or_feed_while_locked(){
        let mut w=active();put(&mut w,0x20,0);let swd=w.swd;put(&mut w,0x1c,u32::MAX);assert_eq!(w.swd,swd);
        w.pump(REQUEST);assert_eq!(w.read(0x28),Some(0));put(&mut w,0x2c,SWD_IRQ);assert!(w.irq());
        put(&mut w,0x28,u32::MAX);assert!(w.irq());put(&mut w,0x24,SWD_IRQ);assert!(w.irq(),"an unserved analog request remains asserted");
        w.pump(EXPIRE);assert!(w.take_reset());put(&mut w,0x20,KEY);config(&mut w,1<<19,0);assert_eq!(w.read(0x1c).unwrap()&1,0);
        config(&mut w,1,0);assert_eq!(w.read(0x1c).unwrap()&1,0);assert!(w.clock.as_ref().unwrap().tripped);
        config(&mut w,AUTO,0);assert!(w.clock.as_ref().unwrap().pulse.is_some());w.pump(EXPIRE+3000);assert_eq!(w.clock.as_ref().unwrap().automatic,1);
        assert!(!w.clock.as_ref().unwrap().tripped);put(&mut w,0x30,SWD_IRQ);assert!(!w.irq());assert_eq!(w.read(0x30),Some(0));
    }
    #[test]fn hp_lp_and_analog_reset_retention_and_legacy_selection_are_explicit(){
        let mut w=active();config(&mut w,AUTO,0);w.pump(REQUEST);let before=w.clone();w.reset_domains(REQUEST,false,false);assert_eq!(w,before);
        w.reset_domains(REQUEST,true,false);assert_eq!(w.read(0x20),Some(0));assert_eq!(w.read(0x1c),Some(300<<20));
        assert!(w.clock.as_ref().unwrap().requested);assert!(w.clock.as_ref().unwrap().pulse.is_none());assert_eq!(w.deadline,Some(EXPIRE));
        w.pump(EXPIRE);assert!(w.take_reset());w.reset_domains(EXPIRE,true,false);assert_eq!(w.read(0x1c),Some((300<<20)|1));
        put(&mut w,0x20,KEY);config(&mut w,AUTO,0);w.pump(EXPIRE+3000);assert_eq!(w.clock.as_ref().unwrap().automatic,1);
        let config=w.read(0x1c);w.disable_virtual_swd();assert_eq!(w.read(0x1c),config);assert_eq!(w.deadline,None);assert_eq!(w.read(0x24),None);
        let before=w.clone();assert!(!w.write(0x1c,0x12c4_0000));assert_eq!(w,before);
        w.enable_virtual_swd(10_000_000_000);w.reset_domains(10_000_000_000,true,true);assert_eq!(w.read(0x1c),Some(300<<20));
        assert_eq!(w.deadline,Some(10_000_000_000+REQUEST));assert_eq!(w.clock.as_ref().unwrap().resets,0);
    }
    #[test]fn unsupported_requests_reject_transactionally_and_time_never_wraps_or_panics(){
        let mut w=active();put(&mut w,0x18,KEY);
        for(off,v)in[(0x1c,0),(0x1c,(300<<20)|2),(0x1c,(300<<20)|DISABLE|(1<<31)),(0x2c,1<<31),(0,1<<31),(0,1<<28),(0,1<<8),(0x3fc,1<<31),(0x100,0),(0x1d,0)]{
            let before=w.clone();assert!(!w.write(off,v),"accepted{off:#x}={v:#x}");assert_eq!(w,before);
        }
        config(&mut w,1<<31,0);let before=w.clone();assert!(!w.write(0x1c,301<<20));assert_eq!(w,before);assert!(!w.write(0x1c,(300<<20)|(1<<31)));
        let mut huge=active();config(&mut huge,AUTO,0);huge.pump(u64::MAX);assert!(!huge.take_reset());assert_eq!(huge.deadline,None);
        let before=huge.clone();huge.pump(0);assert_eq!(huge,before);
    }
    fn rwdt(hz:u32,delay:u8)->LpWdt{
        let mut w=LpWdt::new();assert!(w.enable_virtual_rwdt(0,delay));assert!(w.set_slow_clock(0,Some(hz)));put(&mut w,0x18,KEY);put(&mut w,0,0x0007_ea00);w
    }
    fn stages(actions:[u32;4])->u32{actions.into_iter().enumerate().fold(0,|v,(n,a)|v|(a<<(28-3*n)))}
    fn short_holds(w:&mut LpWdt){for off in[4,8,12,16]{put(w,off,1);}}
    #[test]fn original_bootloader_configures_system_action_before_actual_enable(){
        let mut w=rwdt(136_000,0);w.pump(573_867);put(&mut w,0,0x4007_ea00);
        assert_eq!(w.read(0),Some(0x4007_ea00));assert_eq!(w.deadline,None);assert_eq!(w.rwdt.as_ref().unwrap().count,0);
        // The guest, not EN alone, feeds before enabling the configured timer.
        // A test threshold, not a claim about the not-yet-executed factory write.
        put(&mut w,4,612_000);put(&mut w,0x14,1<<31);assert_eq!(w.read(0x14),Some(0));put(&mut w,0,0xc007_ea00);
        let at=w.deadline.unwrap();assert!(at>573_867);w.pump(at-1);assert_eq!(w.take_reset_cause(),None);w.pump(at);
        assert_eq!(w.take_reset_cause(),Some(LpWdtReset::RwdtSystem));assert_eq!(LpWdtReset::RwdtSystem.reason(),0x10);
        assert_eq!(w.rwdt.as_ref().unwrap().timeouts,[1,0,0,0]);assert_eq!(w.rwdt.as_ref().unwrap().feeds,1);
    }
    #[test]fn stage0_multiplier_uses_all_four_actual_fuse_values(){
        for delay in 0..4{let mut w=rwdt(240_000_000,delay);short_holds(&mut w);put(&mut w,0,stages([1,0,0,0])|(1<<31));
            let at=1u64<<(delay+1);assert_eq!(w.deadline,Some(at));w.pump(at-1);assert_eq!(w.read(0x24),Some(0));w.pump(at);
            assert_eq!(w.read(0x24),Some(1<<31));assert_eq!((w.rwdt.as_ref().unwrap().stage,w.rwdt.as_ref().unwrap().count),(1,0));
        }
    }
    #[test]fn protected_feed_and_irq_ack_have_separate_real_effects(){
        let mut w=rwdt(240_000_000,0);short_holds(&mut w);put(&mut w,0,stages([1,0,0,0])|(1<<31));
        w.pump(2);assert!(!w.irq());put(&mut w,0x2c,1<<31);assert!(w.irq());assert_eq!(w.read(0x28),Some(1<<31));
        put(&mut w,0x18,0);let before=w.rwdt.clone();put(&mut w,0x14,1<<31);assert_eq!(w.rwdt,before);
        put(&mut w,0x30,1<<31);assert!(w.irq());put(&mut w,0x2c,0);assert!(w.irq(),"RWDT interrupt configuration is protected too");
        put(&mut w,0x20,KEY);put(&mut w,0x14,1<<31);assert_eq!(w.rwdt,before,"the SWD key cannot feed RWDT");
        put(&mut w,0x18,KEY);assert!(!w.write(0x14,1));assert_eq!(w.rwdt,before);
    }
    #[test]fn feed_restarts_stage_count_without_clearing_a_latched_interrupt(){
        let mut w=rwdt(240_000_000,0);short_holds(&mut w);put(&mut w,0,stages([1,0,0,0])|(1<<31));put(&mut w,0x2c,1<<31);w.pump(2);
        put(&mut w,0x14,1<<31);assert!(w.irq());assert_eq!((w.rwdt.as_ref().unwrap().stage,w.rwdt.as_ref().unwrap().count),(0,0));assert_eq!(w.deadline,Some(4));
        put(&mut w,0x30,1<<31);assert!(!w.irq());assert_eq!(w.read(0x30),Some(0));w.pump(4);assert!(w.irq());
        put(&mut w,0x28,u32::MAX);assert!(w.irq());put(&mut w,0x24,1<<31);assert!(!w.irq());
    }
    #[test]fn flashboot_alone_counts_and_forces_stage0_system_reset(){
        let mut w=rwdt(240_000_000,0);short_holds(&mut w);put(&mut w,0,1<<12);assert_eq!(w.config>>31,0);assert_eq!(w.deadline,Some(2));
        w.pump(2);assert_eq!(w.take_reset_cause(),Some(LpWdtReset::RwdtSystem));assert_eq!(w.int_raw,0);
        w.reset_domains(2,true,false);assert_eq!(w.read(0),Some(0x13214));assert_eq!(w.rwdt.as_ref().unwrap().hz,Some(240_000_000));
    }
    #[test]fn reset_actions_respect_cpu_gate_and_preserve_controller_across_hp_reset(){
        for(action,cause,pulse)in[(2,LpWdtReset::RwdtCpu,Some(6)),(3,LpWdtReset::RwdtCore,Some(5)),(4,LpWdtReset::RwdtSystem,None)]{
            let mut w=rwdt(240_000_000,0);short_holds(&mut w);put(&mut w,0,(1<<31)|stages([action,0,0,0])|(1<<11)|(6<<16)|(5<<13));
            if action==2{w.pump(2);assert_eq!(w.take_reset_cause(),None);put(&mut w,0x14,1<<31);w.set_cpu_reset_enabled(true);}
            let at=w.deadline.unwrap();w.pump(at);let pending=w.rwdt.as_ref().unwrap().pending;assert_eq!(pending,Some((cause,at,pulse)));
            assert_eq!(w.take_reset_cause(),Some(cause));let mut applied=w.clone();applied.dispatched=None;applied.rwdt.as_mut().unwrap().pending=None;applied.schedule();
            w.reset_domains(at,false,false);assert_eq!(w,applied,"owner acknowledges the request while preserving LP counter/phase");
            assert_eq!(cause.reason(),match action{2=>0x0d,3=>9,_=>0x10});
        }
        let mut w=rwdt(240_000_000,0);w.set_cpu_reset_enabled(true);short_holds(&mut w);put(&mut w,0,(1<<31)|stages([2,0,0,0]));w.pump(2);assert_eq!(w.take_reset_cause(),None,"PROCPU11 is independently required");
    }
    #[test]fn fractional_clock_changes_gating_and_chunked_pumps_keep_actual_tick_phase(){
        let mut once=rwdt(136_000,0);for off in[4,8,12,16]{put(&mut once,off,1000);}put(&mut once,0,(1<<31)|stages([1,1,1,1]));let mut chunks=once.clone();
        let at=123_456_789;for t in[1,1000,9999,70_000,1_234_567,at]{chunks.pump(t);}once.pump(at);assert_eq!(chunks,once);
        let count=once.rwdt.as_ref().unwrap().count;assert!(once.set_slow_clock(at,None));assert_eq!(once.deadline,None);once.pump(at+1_000_000);assert_eq!(once.rwdt.as_ref().unwrap().count,count);
        let unavailable=once.clone();assert!(!once.write(0x14,1<<31));assert_eq!(once,unavailable);assert!(!once.write(0,once.config));assert_eq!(once,unavailable);
        assert!(once.set_slow_clock(at+1_000_000,Some(136_000)));assert!(once.deadline.is_some());let before=once.clone();once.pump(0);assert_eq!(once,before);
        let mut changed=rwdt(3,0);short_holds(&mut changed);put(&mut changed,0,(1<<31)|stages([1,0,0,0]));changed.pump(40_000_000);assert_eq!(changed.rwdt.as_ref().unwrap().count,0);
        assert!(changed.set_slow_clock(40_000_000,Some(6)));assert_eq!(changed.deadline,Some(100_000_000));changed.pump(100_000_000);assert_eq!(changed.int_raw,1<<31);
    }
    #[test]fn stage_loop_catch_up_is_bounded_and_disabled_counter_is_not_fed(){
        let mut w=rwdt(240_000_000,0);short_holds(&mut w);put(&mut w,0,(1<<31)|stages([1,0,1,0]));w.pump(5_000_000_000);
        assert_eq!(w.rwdt.as_ref().unwrap().timeouts,[1_000_000_000;4]);assert_eq!(w.rwdt.as_ref().unwrap().stage,0);assert_eq!(w.rwdt.as_ref().unwrap().feeds,0);
        w.pump(5_000_000_001);put(&mut w,0,stages([1,0,1,0]));let count=w.rwdt.as_ref().unwrap().count;w.pump(6_000_000_000);assert_eq!(w.rwdt.as_ref().unwrap().count,count);assert_eq!(w.deadline,None);
        put(&mut w,0,(1<<31)|stages([1,0,1,0]));assert_eq!(w.deadline,Some(6_000_000_001));w.pump(6_000_000_001);assert_eq!(w.rwdt.as_ref().unwrap().stage,1);
    }
    #[test]fn rwdt_selection_resets_and_unsupported_inputs_are_transactional(){
        let mut w=LpWdt::new();let before=w.clone();assert!(!w.enable_virtual_rwdt(0,4));assert!(!w.set_slow_clock(0,Some(136_000)));assert_eq!(w,before);
        assert!(w.enable_virtual_rwdt(0,0));put(&mut w,0x18,KEY);put(&mut w,0,0x4007_ea00);let before=w.clone();assert!(!w.write(0,0xc007_ea00));assert_eq!(w,before);
        assert!(w.set_slow_clock(0,Some(136_000)));for(off,value)in[(0,(1<<31)|stages([5,0,0,0])),(0,1<<8),(0,1<<10),(0x14,1),(0x100,0)]{let before=w.clone();assert!(!w.write(off,value));assert_eq!(w,before);}
        for hz in[0,240_000_001,u32::MAX]{let before=w.clone();assert!(!w.set_slow_clock(100,Some(hz)));assert_eq!(w,before);}
        short_holds(&mut w);put(&mut w,0,(1<<31)|stages([1,0,0,0]));let before=w.clone();assert!(!w.write(4,0));assert_eq!(w,before);
        w.reset_domains(100,true,true);assert_eq!(w.rwdt.as_ref().unwrap().hz,None);assert_eq!(w.rwdt.as_ref().unwrap().count,0);
        let readable=w.read(0);w.disable_virtual_rwdt();assert_eq!(w.read(0),readable);assert_eq!(w.deadline,None);assert_eq!(w.read(0x24),None);assert!(!w.irq());
    }
    #[test]fn swd_and_rwdt_share_irq_line_but_keep_independent_masks_and_deadlines(){
        let mut w=active();config(&mut w,AUTO,0);let swd=w.report();assert!(w.enable_virtual_rwdt(0,0));assert!(w.set_slow_clock(0,Some(240_000_000)));put(&mut w,0x18,KEY);short_holds(&mut w);
        put(&mut w,0,(1<<31)|stages([1,0,0,0]));assert_eq!(w.deadline,Some(2));put(&mut w,0x2c,1<<31);w.pump(2);assert!(w.irq());
        put(&mut w,0x30,1<<31);assert!(!w.irq());put(&mut w,0,0);assert_eq!(w.deadline,Some(REQUEST));w.disable_virtual_rwdt();assert_eq!(w.report(),swd);
        w.pump(REQUEST);assert_eq!(w.int_raw,SWD_IRQ);put(&mut w,0x2c,SWD_IRQ);assert!(w.irq());w.disable_virtual_swd();assert_eq!(w.deadline,None);
    }
    #[test]fn zero_holds_and_huge_time_have_bounded_explicit_behavior(){
        let mut w=rwdt(240_000_000,3);put(&mut w,4,0);let before=w.clone();assert!(!w.write(0,1<<31));assert_eq!(w,before,"active zero thresholds are unsupported");
        put(&mut w,4,u32::MAX);put(&mut w,0,(1<<31)|stages([4,0,0,0]));let at=u64::from(u32::MAX)<<4;assert_eq!(w.deadline,Some(at));w.pump(u64::MAX);
        assert_eq!(w.rwdt.as_ref().unwrap().pending,Some((LpWdtReset::RwdtSystem,at,None)));assert_eq!(w.rwdt.as_ref().unwrap().timeouts,[1,0,0,0]);
        let before=w.clone();w.pump(u64::MAX);assert_eq!(w,before,"a pending reset is one scheduling boundary, not a queue");
        assert!(!w.set_slow_clock(u64::MAX,None));assert_eq!(w,before,"drain the pending reset before changing its source observation");
        put(&mut w,0x14,1<<31);assert_eq!(w.take_reset_cause(),Some(LpWdtReset::RwdtSystem),"a late feed cannot retract an emitted reset");
        let mut all_off=rwdt(240_000_000,0);short_holds(&mut all_off);put(&mut all_off,0,1<<31);all_off.pump(u64::MAX);
        assert_eq!(all_off.take_reset_cause(),None);assert_eq!(all_off.int_raw,0);assert_eq!(all_off.rwdt.as_ref().unwrap().timeouts,[u64::MAX/5;4]);
    }
    #[test]fn simultaneous_reset_sources_keep_the_documented_swd_priority(){
        let mut w=rwdt(240_000_000,0);w.enable_virtual_swd(0);put(&mut w,4,(EXPIRE/2)as u32);put(&mut w,0,(1<<31)|stages([4,0,0,0]));
        assert_eq!(w.deadline,Some(REQUEST));w.pump(REQUEST);assert_eq!(w.deadline,Some(EXPIRE));w.pump(EXPIRE);
        assert_eq!(w.take_reset_cause(),Some(LpWdtReset::Swd));assert_eq!(LpWdtReset::Swd.reason(),0x12);
    }
    fn competing(rwdt_at:u32)->LpWdt{
        let mut w=rwdt(240_000_000,0);w.enable_virtual_swd(0);put(&mut w,4,rwdt_at/2);put(&mut w,0,(1<<31)|stages([3,0,0,0]));w
    }
    #[test]fn late_pump_selects_earliest_time_and_uses_priority_only_for_a_tie(){
        for(rwdt_at,cause,at)in[(130_000_000,LpWdtReset::RwdtCore,130_000_000),(300_000_000,LpWdtReset::Swd,EXPIRE),(228_000_000,LpWdtReset::Swd,EXPIRE)]{
            let mut w=competing(rwdt_at);w.pump(400_000_000);let pending=w.clone();assert!(!w.set_slow_clock(400_000_000,None));assert_eq!(w,pending);
            assert_eq!(w.take_reset_event(),Some((cause,at)));
            assert_eq!(w.clock.as_ref().unwrap().now,at);assert_eq!(w.rwdt.as_ref().unwrap().now,at);
            if rwdt_at<EXPIRE as u32{assert_eq!(w.swd_reset_at,None);assert!(!w.swd_flag);assert_eq!(w.int_raw&SWD_IRQ,0,"no future SWD request may leak from the preview");}
            if rwdt_at>EXPIRE as u32{assert_eq!(w.rwdt.as_ref().unwrap().pending,None);assert_eq!(w.rwdt.as_ref().unwrap().timeouts,[0;4]);}
        }
    }
    #[test]fn chunked_and_late_pumps_commit_the_same_first_reset_state(){
        let mut once=competing(130_000_000);let mut chunks=once.clone();once.pump(400_000_000);
        for at in[1,50_000_000,129_999_999,130_000_000,200_000_000,400_000_000]{chunks.pump(at);}
        assert_eq!(once,chunks);assert_eq!(once.take_reset_event(),Some((LpWdtReset::RwdtCore,130_000_000)));
    }
    #[test]fn dispatched_event_blocks_progress_and_further_events_until_owner_reset(){
        let mut w=competing(130_000_000);w.pump(400_000_000);assert_eq!(w.take_reset_event(),Some((LpWdtReset::RwdtCore,130_000_000)));assert_eq!(w.deadline,None);
        let before=w.clone();for at in[130_000_000,400_000_000,u64::MAX]{w.pump(at);assert_eq!(w.take_reset_event(),None);assert_eq!(w,before);}
        assert!(!w.set_slow_clock(u64::MAX,None));assert_eq!(w,before);
        w.reset_domains(130_000_000,false,false);assert_eq!(w.rwdt.as_ref().unwrap().feeds,0);assert_eq!(w.rwdt.as_ref().unwrap().stage,1);assert_eq!(w.rwdt.as_ref().unwrap().count,0);
        w.pump(400_000_000);assert_eq!(w.take_reset_event(),Some((LpWdtReset::Swd,EXPIRE)),"the independent later event needs a real owner reset first");
    }
    #[test]fn swd_trip_time_is_sticky_through_late_pumps_feed_and_flag_clear(){
        let mut w=active();w.pump(u64::MAX);assert_eq!(w.swd_reset_at,Some(EXPIRE));assert_eq!(w.clock.as_ref().unwrap().now,EXPIRE);
        config(&mut w,(1<<31)|(1<<19),0);assert!(!w.swd_flag);w.pump(u64::MAX);
        assert_eq!(w.swd_reset_at,Some(EXPIRE));assert_eq!(w.clock.as_ref().unwrap().software,0);assert_eq!(w.take_reset_event(),Some((LpWdtReset::Swd,EXPIRE)));
        w.pump(u64::MAX);assert_eq!(w.take_reset_event(),None);assert_eq!(w.clock.as_ref().unwrap().software,0);
        w.reset_domains(EXPIRE,true,false);assert_eq!(w.first_reset_event(),None);assert_eq!(w.dispatched,None);
    }
    #[test]fn a_real_completed_feed_moves_future_expiry_not_an_existing_event(){
        let mut w=active();w.pump(REQUEST);config(&mut w,1<<31,0);w.pump(2*EXPIRE);
        let at=REQUEST+3000+EXPIRE;assert_eq!(w.clock.as_ref().unwrap().software,1);assert_eq!(w.swd_reset_at,Some(at));
        assert_eq!(w.take_reset_event(),Some((LpWdtReset::Swd,at)));
    }
    #[test]fn fractional_rwdt_event_time_and_phase_survive_the_owner_core_reset(){
        let mut w=rwdt(136_000,0);short_holds(&mut w);put(&mut w,0,(1<<31)|stages([3,4,0,0]));w.pump(u64::MAX);
        assert_eq!(w.take_reset_event(),Some((LpWdtReset::RwdtCore,3530)));w.reset_domains(3530,false,false);
        assert_eq!(w.rwdt.as_ref().unwrap().feeds,0);w.pump(u64::MAX);assert_eq!(w.take_reset_event(),Some((LpWdtReset::RwdtSystem,5295)));
    }
    #[test]fn owner_reset_acknowledges_coincident_requests_without_replaying_them(){
        let mut w=rwdt(240_000_000,0);short_holds(&mut w);w.set_cpu_reset_enabled(true);put(&mut w,0,(1<<31)|(1<<11)|stages([2,0,0,0]));w.pump(2);
        // An independent higher-priority Core Reset covers this simultaneous
        // CPU reset even if the owner did not dispatch the LP cause itself.
        assert_eq!(w.first_reset_event(),Some((LpWdtReset::RwdtCpu,2)));w.reset_domains(2,false,false);assert_eq!(w.take_reset_event(),None);
        assert_eq!(w.rwdt.as_ref().unwrap().stage,1);assert_eq!(w.rwdt.as_ref().unwrap().feeds,0);w.pump(3);assert_eq!(w.take_reset_event(),None);
        let mut both=competing(EXPIRE as u32);both.pump(u64::MAX);assert_eq!(both.take_reset_event(),Some((LpWdtReset::Swd,EXPIRE)));
        both.reset_domains(EXPIRE,true,false);assert_eq!(both.first_reset_event(),None);assert_eq!(both.dispatched,None);
    }
    #[test]fn strict_profile_deselection_clears_only_its_own_dispatch_barrier(){
        let mut w=competing(130_000_000);w.pump(u64::MAX);assert_eq!(w.take_reset_event(),Some((LpWdtReset::RwdtCore,130_000_000)));
        w.disable_virtual_rwdt();w.pump(u64::MAX);assert_eq!(w.take_reset_event(),Some((LpWdtReset::Swd,EXPIRE)));
        w.disable_virtual_swd();let before=w.clone();w.pump(u64::MAX);assert_eq!(w,before);assert_eq!(w.take_reset_event(),None);assert_eq!(w.deadline,None);
    }
    #[test]fn observed_fast_rates_change_only_the_pulse_time_and_invalid_inputs_are_transactional(){
        let mut legacy=LpWdt::new();let before=legacy.clone();assert!(!legacy.set_fast_clock(9,Some(17_500_000)));assert_eq!(legacy,before);
        for(hz,duration,label)in[(17_500_000,4115,"17.5MHz"),(24_000_000,3000,"24MHz"),(48_000_000,1500,"48MHz")]{
            let mut w=active();assert!(w.set_fast_clock(0,Some(hz)));config(&mut w,1<<31,0);assert_eq!(w.deadline,Some(duration));
            for invalid in[0,1,17_500_001,240_000_000,u32::MAX]{let before=w.clone();assert!(!w.set_fast_clock(u64::MAX,Some(invalid)));assert_eq!(w,before);}
            w.pump(duration-1);assert_eq!(w.clock.as_ref().unwrap().software,0);w.pump(duration);let swd=w.clock.as_ref().unwrap();assert_eq!((swd.software,swd.origin,swd.requests),(1,duration,0));
            assert_eq!(w.deadline,Some(duration+REQUEST));assert!(w.report().contains(&format!("lp_fast={label}")));
        }
        let w=active();assert_eq!(w.report(),"SWD=virtual950ms/request850ms lp_fast=24MHz enabled=1 auto=0 requests=0 auto_feeds=0 software_feeds=0 resets=0 pulse=0");
    }
    #[test]fn one_cycle_source_boundaries_preserve_fractional_feed_progress(){
        let mut w=active();config(&mut w,1<<31,0);assert!(w.set_fast_clock(1,Some(17_500_000)));
        assert_eq!(w.clock.as_ref().unwrap().pulse.unwrap().remaining,71_976_000_000);
        assert!(w.set_fast_clock(2,Some(48_000_000)));assert_eq!(w.clock.as_ref().unwrap().pulse.unwrap().remaining,71_958_500_000);
        // 1*24M + 1*17.5M + 1499*48M is 6.5M short of300*240M.
        assert_eq!(w.deadline,Some(1502));w.pump(1501);assert_eq!(w.clock.as_ref().unwrap().software,0);assert_eq!(w.clock.as_ref().unwrap().pulse.unwrap().remaining,6_500_000);
        w.pump(1502);assert_eq!(w.clock.as_ref().unwrap().software,1);assert_eq!(w.clock.as_ref().unwrap().origin,1502);assert_eq!(w.deadline,Some(1502+REQUEST));
    }
    #[test]fn losing_fast_clock_pauses_only_the_feed_and_resume_keeps_the_fraction(){
        let mut w=active();assert!(w.set_fast_clock(0,Some(17_500_000)));config(&mut w,1<<31,0);assert!(w.set_fast_clock(1,None));
        assert_eq!(w.clock.as_ref().unwrap().pulse.unwrap().remaining,71_982_500_000);assert_eq!(w.deadline,Some(REQUEST));
        w.pump(1_000_000);assert_eq!(w.clock.as_ref().unwrap().pulse.unwrap().remaining,71_982_500_000);assert_eq!(w.clock.as_ref().unwrap().origin,0);
        let before=w.clone();assert!(!w.write(0x1c,301<<20));assert_eq!(w,before);assert!(!w.write(0x1c,(300<<20)|(1<<31)));
        assert!(w.set_fast_clock(2_000_000,Some(48_000_000)));assert_eq!(w.deadline,Some(2_001_500));w.pump(2_001_499);assert_eq!(w.clock.as_ref().unwrap().software,0);
        w.pump(2_001_500);assert_eq!(w.clock.as_ref().unwrap().software,1);assert_eq!(w.clock.as_ref().unwrap().origin,2_001_500);
    }
    #[test]fn analog_request_crosses_an_inflight_software_pulse_and_loss_still_expires(){
        let mut w=active();w.pump(REQUEST-1000);config(&mut w,1<<31,0);let mut once=w.clone();w.pump(REQUEST);
        assert_eq!(w.clock.as_ref().unwrap().requests,1);assert!(w.clock.as_ref().unwrap().requested);assert_eq!(w.int_raw,SWD_IRQ);assert_eq!(w.deadline,Some(REQUEST+2000));
        w.pump(REQUEST+2000);once.pump(REQUEST+2000);assert_eq!(w,once);assert_eq!((w.clock.as_ref().unwrap().software,w.clock.as_ref().unwrap().origin),(1,REQUEST+2000));
        let mut lost=active();assert!(lost.set_fast_clock(REQUEST-1,None));config(&mut lost,1<<31,0);lost.pump(REQUEST);
        assert_eq!(lost.clock.as_ref().unwrap().requests,1);assert_eq!(lost.deadline,Some(EXPIRE));lost.pump(EXPIRE-1);assert_eq!(lost.clock.as_ref().unwrap().software,0);
        lost.pump(u64::MAX);assert_eq!(lost.first_reset_event(),Some((LpWdtReset::Swd,EXPIRE)));assert_eq!(lost.clock.as_ref().unwrap().now,EXPIRE);assert_eq!(lost.clock.as_ref().unwrap().origin,0);assert_eq!(lost.clock.as_ref().unwrap().software,0);
    }
    #[test]fn expiry_wins_at_the_same_cycle_as_completion_or_a_source_change(){
        let mut w=active();assert!(w.set_fast_clock(0,Some(17_500_000)));w.pump(EXPIRE-4115);config(&mut w,1<<31,0);assert_eq!(w.deadline,Some(EXPIRE));
        assert!(!w.set_fast_clock(EXPIRE,Some(48_000_000)));assert_eq!(w.fast_hz,Some(17_500_000));assert_eq!(w.first_reset_event(),Some((LpWdtReset::Swd,EXPIRE)));assert_eq!(w.clock.as_ref().unwrap().software,0);
        let before=w.clone();assert!(w.set_fast_clock(u64::MAX,Some(17_500_000)));assert_eq!(w,before);assert!(!w.set_fast_clock(u64::MAX,None));assert_eq!(w,before);
        assert_eq!(w.take_reset_event(),Some((LpWdtReset::Swd,EXPIRE)));let before=w.clone();assert!(w.set_fast_clock(u64::MAX,Some(17_500_000)));assert!(!w.set_fast_clock(u64::MAX,Some(24_000_000)));assert_eq!(w,before);
        let mut earlier=active();assert!(earlier.set_fast_clock(0,Some(17_500_000)));earlier.pump(EXPIRE-4116);config(&mut earlier,1<<31,0);earlier.pump(EXPIRE);assert_eq!(earlier.first_reset_event(),None);assert_eq!(earlier.clock.as_ref().unwrap().software,1);assert_eq!(earlier.clock.as_ref().unwrap().origin,EXPIRE-1);
    }
    #[test]fn fast_auto_catchup_is_bounded_and_chunk_equivalent_through_u64_max(){
        for(hz,width)in[(17_500_000,4115),(24_000_000,3000),(48_000_000,1500)]{
            let mut once=active();assert!(once.set_fast_clock(0,Some(hz)));config(&mut once,AUTO,0);let mut chunks=once.clone();let period=REQUEST+width;let at=period*1_000_000+REQUEST+width-1;
            for t in[1,REQUEST,period-1,period,period*999_999,at]{chunks.pump(t);}once.pump(at);assert_eq!(once,chunks);
            assert_eq!((once.clock.as_ref().unwrap().automatic,once.clock.as_ref().unwrap().requests),(1_000_000,1_000_001));assert_eq!(once.deadline,Some(at+1));
            once.pump(u64::MAX);chunks.pump(u64::MAX);assert_eq!(once,chunks);assert_eq!(once.first_reset_event(),None);assert_eq!(once.deadline,None);let before=once.clone();once.pump(0);assert_eq!(once,before);
        }
        let mut lost=active();assert!(lost.set_fast_clock(0,None));config(&mut lost,AUTO,0);lost.pump(u64::MAX);assert_eq!(lost.first_reset_event(),Some((LpWdtReset::Swd,EXPIRE)));assert_eq!(lost.clock.as_ref().unwrap().automatic,0);
    }
    #[test]fn fast_observations_and_inflight_progress_follow_reset_domains(){
        let mut w=active();assert!(w.set_fast_clock(0,Some(17_500_000)));config(&mut w,1<<31,0);w.pump(1);let before=w.clone();w.reset_domains(1,false,false);assert_eq!(w,before);
        w.reset_domains(1,true,false);assert_eq!(w.fast_hz,Some(17_500_000));assert!(w.clock.as_ref().unwrap().pulse.is_none());assert_eq!(w.clock.as_ref().unwrap().origin,0);assert_eq!(w.deadline,Some(REQUEST));
        w.reset_domains(1,true,true);assert_eq!(w.fast_hz,Some(24_000_000));assert_eq!(w.clock.as_ref().unwrap().origin,1);assert_eq!(w.deadline,Some(REQUEST+1));assert_eq!(w.clock.as_ref().unwrap().software,0);
        w.disable_virtual_swd();let before=w.clone();assert!(!w.set_fast_clock(10,Some(48_000_000)));assert_eq!(w,before);
    }
    #[test]fn earlier_rwdt_truncates_fast_pulse_without_changing_the_slow_source(){
        let mut w=rwdt(240_000_000,0);short_holds(&mut w);put(&mut w,0,(1<<31)|stages([3,0,0,0]));w.enable_virtual_swd(0);assert!(w.set_fast_clock(0,Some(17_500_000)));put(&mut w,0x20,KEY);config(&mut w,1<<31,0);
        w.pump(u64::MAX);assert_eq!(w.first_reset_event(),Some((LpWdtReset::RwdtCore,2)));let swd=w.clock.as_ref().unwrap();assert_eq!(swd.now,2);assert_eq!((swd.software,swd.requests,swd.origin),(0,0,0));assert_eq!(swd.pulse.unwrap().remaining,71_965_000_000);
        assert_eq!(w.rwdt.as_ref().unwrap().hz,Some(240_000_000));let before=w.clone();assert!(w.set_fast_clock(u64::MAX,Some(17_500_000)));assert!(!w.set_fast_clock(u64::MAX,None));assert_eq!(w,before);
        assert_eq!(w.take_reset_event(),Some((LpWdtReset::RwdtCore,2)));w.reset_domains(2,false,false);assert_eq!(w.fast_hz,Some(17_500_000));w.pump(4115);
        assert_eq!(w.first_reset_event(),Some((LpWdtReset::RwdtCore,7)));assert_eq!(w.clock.as_ref().unwrap().software,0);assert_eq!(w.clock.as_ref().unwrap().now,7);
        assert_eq!(w.take_reset_event(),Some((LpWdtReset::RwdtCore,7)));w.reset_domains(7,false,false);w.disable_virtual_rwdt();w.pump(4115);assert_eq!(w.clock.as_ref().unwrap().software,1);assert_eq!(w.clock.as_ref().unwrap().origin,4115);
    }

    #[test]fn auto_pulse_keeps_its_kind_and_phase_across_clock_events_and_auto_disable(){
        let mut once=active();config(&mut once,AUTO,0);let mut chunks=once.clone();
        for t in[REQUEST-1,REQUEST,REQUEST+1]{chunks.pump(t);}assert!(once.set_fast_clock(REQUEST+1,Some(17_500_000)));assert!(chunks.set_fast_clock(REQUEST+1,Some(17_500_000)));assert_eq!(once,chunks);
        assert!(once.set_fast_clock(REQUEST+2,None));assert!(chunks.set_fast_clock(REQUEST+2,None));for t in[REQUEST+3,REQUEST+9999]{chunks.pump(t);}
        assert!(once.set_fast_clock(REQUEST+10_000,Some(48_000_000)));assert!(chunks.set_fast_clock(REQUEST+10_000,Some(48_000_000)));assert_eq!(once,chunks);assert_eq!(once.deadline,Some(REQUEST+11_500));
        once.pump(REQUEST+10_001);chunks.pump(REQUEST+10_001);config(&mut once,0,AUTO);config(&mut chunks,0,AUTO);for t in[REQUEST+10_002,REQUEST+11_499]{chunks.pump(t);}once.pump(REQUEST+11_499);assert_eq!(once,chunks);
        assert_eq!(once.clock.as_ref().unwrap().automatic,0);once.pump(REQUEST+11_500);chunks.pump(REQUEST+11_500);assert_eq!(once,chunks);let swd=once.clock.as_ref().unwrap();assert_eq!((swd.requests,swd.automatic,swd.software,swd.origin),(1,1,0,REQUEST+11_500));
        assert_eq!(once.deadline,Some(2*REQUEST+11_500));
    }

}
