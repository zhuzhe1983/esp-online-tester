//! Strictly opt-in ideal-reference OCODE handshake, not physical calibration.
//! IDF5.5.3/5.5.4 C5 ocode_init.c, regi2c_lp_bias.h and HP REGI2C patch.
//! The public sources give neither target POR nor convergence/retention data.
//! Only RESETB0 and derived O_DONE0/BG_O_DONE3 are exposed; no code/voltage.
const SETTLE_CYCLES:u64=24_000; // Explicit virtual100us on nominal240MHz time.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum State{Unarmed,Reset,Running,Done,Aborted}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct Ocode{
    pub enabled:bool,now:u64,cpu_xtal:bool,resetb:u8,state:State,pub deadline:Option<u64>,
    resets:u64,attempts:u64,completions:u64,cancellations:u64,polls:u64,
    cpu_observation:Option<(u64,bool)>,reset_at:Option<u64>,start_at:Option<u64>,start_cpu_at:Option<u64>,
    first_reg3:Option<(u64,u8)>,last_reg3:Option<(u64,u8)>,cancel_at:Option<u64>,
}
impl Ocode{
    // Explicit virtual byte0=0; this does not count as a guest RESETB write.
    pub fn new()->Self{Self{enabled:false,now:0,cpu_xtal:false,resetb:0,state:State::Unarmed,deadline:None,
        resets:0,attempts:0,completions:0,cancellations:0,polls:0,cpu_observation:None,reset_at:None,start_at:None,start_cpu_at:None,
        first_reg3:None,last_reg3:None,cancel_at:None}}
    pub fn enable(&mut self,cycle:u64){if !self.enabled{*self=Self::new();self.enabled=true;self.now=cycle;}}
    pub fn disable(&mut self){*self=Self::new();}
    fn flags(&self)->u8{if self.state==State::Done{9}else{0}}
    fn cancel(&mut self){
        if self.state==State::Running{self.cancellations=self.cancellations.saturating_add(1);self.cancel_at=Some(self.now);}
        self.deadline=None;self.state=State::Aborted;
    }
    // These are guards for the observed software sequence, not a claim about
    // all silicon dependencies. Master route/gating does not clock the target.
    pub fn reconcile(&mut self,quiet:bool){
        if self.enabled&&self.state==State::Running&&(!self.cpu_xtal||!quiet){self.cancel();}
    }
    pub fn pump(&mut self,cycle:u64,quiet:bool){
        if !self.enabled{return;}
        self.now=self.now.max(cycle);self.reconcile(quiet);
        if self.deadline.is_some_and(|at|at<=self.now){
            self.deadline=None;self.state=State::Done;self.completions=self.completions.saturating_add(1);
        }
    }
    // The wrapper pumps with the previous observation before calling this.
    pub fn observe_cpu_xtal(&mut self,cpu_xtal:bool,quiet:bool){
        if !self.enabled{return;}self.cpu_xtal=cpu_xtal;self.cpu_observation=Some((self.now,cpu_xtal));self.reconcile(quiet);
    }
    pub fn transfer(&mut self,reg:u8,write:Option<u8>,quiet:bool)->Option<u8>{
        if !self.enabled{return None;}
        match(reg,write){
            (0,None)=>Some(self.resetb),
            (0,Some(0))=>{
                if self.state==State::Running{self.cancellations=self.cancellations.saturating_add(1);self.cancel_at=Some(self.now);}
                self.resetb=0;self.state=State::Reset;self.deadline=None;self.resets=self.resets.saturating_add(1);
                self.reset_at=Some(self.now);self.start_at=None;self.start_cpu_at=None;Some(0)
            }
            (0,Some(1))=>{
                if self.state!=State::Reset||!self.cpu_xtal||!quiet{return None;}
                let at=self.now.checked_add(SETTLE_CYCLES)?;
                self.resetb=1;self.state=State::Running;self.deadline=Some(at);self.attempts=self.attempts.saturating_add(1);
                self.start_at=Some(self.now);self.start_cpu_at=self.cpu_observation.map(|v|v.0);Some(1)
            }
            (3,None)=>{let value=self.flags();self.polls=self.polls.saturating_add(1);let event=(self.now,value);
                self.first_reg3.get_or_insert(event);self.last_reg3=Some(event);Some(value)}
            _=>None,
        }
    }
    /// Declared virtual retention, not a silicon target reset-domain claim.
    /// CPU-only retains; HP cancels an unfinished sequence and withdraws its
    /// CPU observation, but preserves completed flags. LP clears target state;
    /// analog also clears diagnostic counters. Profile selection survives.
    pub fn reset(&mut self,hp:bool,lp:bool,analog:bool){
        if !self.enabled{return;}
        if analog{let now=self.now;*self=Self::new();self.enabled=true;self.now=now;return;}
        if lp{
            if self.state==State::Running{self.cancellations=self.cancellations.saturating_add(1);self.cancel_at=Some(self.now);}
            self.cpu_xtal=false;self.cpu_observation=None;self.resetb=0;self.state=State::Unarmed;self.deadline=None;
            self.reset_at=None;self.start_at=None;self.start_cpu_at=None;
        }else if hp{
            self.cpu_xtal=false;self.cpu_observation=None;
            if self.state!=State::Done{self.cancel();}
        }
    }
    pub fn report(&self)->String{format!("ocode_profile=ideal_reference_virtual100us enabled={} state={:?} cpu_xtal={} cpu_observation={:?} resetb={} reset0_at={:?} reset1_at={:?} start_cpu_observed_at={:?} flags={:#x} deadline={:?} resets={} attempts={} completions={} cancellations={} cancel_observed_at={:?} polls={} first_reg3={:?} last_reg3={:?} code=unsupported physical_calibration=unmodeled",
        u8::from(self.enabled),self.state,u8::from(self.cpu_xtal),self.cpu_observation,self.resetb,self.reset_at,self.start_at,self.start_cpu_at,self.flags(),self.deadline,self.resets,self.attempts,self.completions,self.cancellations,self.cancel_at,self.polls,self.first_reg3,self.last_reg3)}
}
