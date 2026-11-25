//! Source-clocked functional C5 RWDT; no physical reset pulse/stall timing.
use super::LpWdtReset;
const TIMEBASE:u128=240_000_000;
pub(super) const IRQ:u32=1<<31;
const ACTIVE:u32=(1<<31)|(1<<12);
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct Rwdt{
    pub now:u64,pub hz:Option<u32>,pub delay:u8,pub stage:usize,pub count:u64,phase:u128,
    pub cpu_reset:bool,pub pending:Option<(LpWdtReset,u64,Option<u8>)>,pub feeds:u64,pub timeouts:[u64;4],
}
impl Rwdt{
    pub fn new(cycle:u64,delay:u8)->Self{Self{now:cycle,hz:None,delay,stage:0,count:0,phase:0,cpu_reset:false,pending:None,feeds:0,timeouts:[0;4]}}
    pub fn active(config:u32)->bool{config&ACTIVE!=0}
    pub fn valid(config:u32,hold:&[u32;4])->bool{
        config&((1<<8)|(1<<10))==0&&(0..4).all(|n|(config>>(28-3*n))&7<=4)
            &&(!Self::active(config)||hold.iter().all(|&v|v!=0))
    }
    fn threshold(&self,hold:&[u32;4],stage:usize)->u64{u64::from(hold[stage])<<if stage==0{self.delay+1}else{0}}
    fn action(config:u32,stage:usize)->u32{if stage==0&&config&(1<<12)!=0{4}else{(config>>(28-3*stage))&7}}
    fn cause(&self,config:u32,stage:usize)->Option<LpWdtReset>{match Self::action(config,stage){
        2 if self.cpu_reset&&config&(1<<11)!=0=>Some(LpWdtReset::RwdtCpu),3=>Some(LpWdtReset::RwdtCore),4=>Some(LpWdtReset::RwdtSystem),_=>None,
    }}
    fn until(&self,ticks:u128)->Option<u64>{
        let hz=u128::from(self.hz?);let cycles=(ticks*TIMEBASE).saturating_sub(self.phase).div_ceil(hz);
        self.now.checked_add(u64::try_from(cycles).ok()?)
    }
    pub fn deadline(&self,config:u32,hold:&[u32;4])->Option<u64>{
        if let Some((_,at,_))=self.pending{return Some(at);}
        if !Self::active(config)||self.hz.is_none(){return None;}
        self.until(u128::from(self.threshold(hold,self.stage).saturating_sub(self.count).max(1)))
    }
    pub fn feed(&mut self){self.stage=0;self.count=0;self.feeds=self.feeds.saturating_add(1);}
    pub fn pump(&mut self,cycle:u64,config:u32,hold:&[u32;4])->bool{
        // A logical reset is a scheduling boundary. Do not execute later
        // watchdog stages before the owner has consumed/applied that request.
        if self.pending.is_some(){return false;}
        let cycle=cycle.max(self.now);let Some(hz)=self.hz else{self.now=cycle;return false;};
        let initial_now=self.now;let initial_phase=self.phase;let total=initial_phase+u128::from(cycle-self.now)*u128::from(hz);
        let mut ticks=total/TIMEBASE;let mut consumed=0;let mut irq=false;
        if Self::active(config){
            let repeating=(0..4).all(|n|self.cause(config,n).is_none());
            while ticks!=0{
                if repeating&&self.stage==0&&self.count==0{
                    let period:u128=(0..4).map(|n|u128::from(self.threshold(hold,n))).sum();let loops=ticks/period;
                    if loops!=0{
                        let add=u64::try_from(loops).unwrap_or(u64::MAX);
                        for(n,timeouts)in self.timeouts.iter_mut().enumerate(){*timeouts=timeouts.saturating_add(add);irq|=Self::action(config,n)==1;}
                        ticks%=period;consumed+=loops*period;if ticks==0{break;}
                    }
                }
                let remaining=u128::from(self.threshold(hold,self.stage).saturating_sub(self.count).max(1));
                if ticks<remaining{self.count+=ticks as u64;break;}
                ticks-=remaining;consumed+=remaining;let stage=self.stage;self.count=0;self.stage=(stage+1)&3;
                self.timeouts[stage]=self.timeouts[stage].saturating_add(1);irq|=Self::action(config,stage)==1;
                if let Some(cause)=self.cause(config,stage){
                    let elapsed=(consumed*TIMEBASE).saturating_sub(initial_phase).div_ceil(u128::from(hz));
                    // This event lies within the supplied u64 cycle interval.
                    let at=initial_now.saturating_add(elapsed as u64);
                    self.phase=(initial_phase+u128::from(at-initial_now)*u128::from(hz))%TIMEBASE;self.now=at;
                    let pulse=match cause{LpWdtReset::RwdtCpu=>Some(((config>>16)&7)as u8),LpWdtReset::RwdtCore=>Some(((config>>13)&7)as u8),_=>None};
                    self.pending=Some((cause,at,pulse));return irq;
                }
            }
        }
        self.phase=total%TIMEBASE;self.now=cycle;irq
    }
}
