//! C5 targeted internal analog bytes and explicitly virtual100us SPLL lock.
//! IDF5.5.3/5.5.4 regi2c_dig_reg/bbpll, clk_tree_ll, regi2c_ctrl_ll.
//! Unknown analog POR/lock timing is not inferred from another chip.
const LOCK_CYCLES:u64=24_000;
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct Analog{
    pub enabled:bool,pub now:u64,conf:u32,power:u8,fosc:u32,dig:[u8;4],bb:[u8;5],dcap_written:bool,
    armed:bool,done:bool,locked:bool,stopped:bool,pub deadline:Option<u64>,attempts:u64,completions:u64,
}
impl Analog{
    pub fn new()->Self{Self{enabled:false,now:0,conf:0x00e408,power:0,fosc:0x4000_0000,dig:[0,0,0x0c,0],bb:[0;5],dcap_written:false,
        armed:false,done:false,locked:false,stopped:false,deadline:None,attempts:0,completions:0}}
    pub fn enable(&mut self,cycle:u64){if !self.enabled{self.enabled=true;self.now=cycle;}}
    pub fn disable(&mut self){self.enabled=false;self.cancel();self.armed=false;}
    fn cancel(&mut self){self.done=false;self.locked=false;self.stopped=false;self.deadline=None;}
    fn configured(&self)->bool{self.bb==[0x51,10,0x11,0x70,3]}
    pub fn powered(&self)->bool{self.enabled&&self.power&14==14}
    // Actual WT latches, not !ready: OCODE's observed XTAL software path
    // disables GLOBAL_ICG/XPD_BBPLL_I2C/XPD_BBPLL but may retain BB_I2C.
    pub fn ocode_quiet(&self)->bool{self.enabled&&self.power&13==0}
    pub fn power_latches(&self)->u8{self.power}
    pub fn ready(&self)->bool{self.powered()&&self.power&1!=0&&self.configured()&&self.locked&&self.done&&self.stopped}
    pub fn rc_fast_powered(&self)->bool{self.enabled&&self.fosc&(1<<30)!=0}
    pub fn slow_dcap(&self)->Option<u8>{(self.enabled&&self.dcap_written).then_some(self.dig[3])}
    pub fn read_conf(&self)->u32{self.conf|u32::from(self.done)<<24}
    pub fn read_fosc(&self)->u32{self.fosc}
    // XTAL32K power-request28 is a configuration latch only: the explicit
    // board has no XTAL32K signal/ready. RC32K29/slow-OSC power-down31 reject.
    // FOSC30 remains the actual input for the bounded RC_FAST clock model.
    pub fn write_fosc(&mut self,v:u32)->bool{if v&!((1<<28)|(1<<30))!=0{return false;}self.fosc=v;true}
    pub fn write_power(&mut self,v:u32)->bool{
        // Four real WT high/low latch pairs; no XTAL/RF/retention controls.
        if v&!0x7200_0039!=0{return false;}
        for(high,low)in[(25,0),(28,3),(29,4),(30,5)]{if v&(1<<high)!=0&&v&(1<<low)!=0{return false;}}
        let was=self.powered();for(i,(high,low))in[(25,0),(28,3),(29,4),(30,5)].into_iter().enumerate(){
            if v&(1<<high)!=0{self.power|=1<<i;}else if v&(1<<low)!=0{self.power&=!(1<<i);}
        }
        if was&&!self.powered(){self.armed=false;self.cancel();}true
    }
    pub fn write_conf(&mut self,v:u32)->bool{
        // Header high byte is RO status, including derived CAL_DONE24.
        let next=v&0x00ff_ffff;if (next^self.conf)&!12!=0||next&12==12{return false;}
        let old=self.conf;self.conf=next;
        match next&12{
            8=>{self.armed=true;self.cancel();},
            0=>{self.armed=false;self.deadline=None;self.stopped=false;},
            4=>{self.armed=false;self.deadline=None;self.stopped=old&12==0&&self.done&&self.locked;},
            _=>return false,
        }true
    }
    fn slot(block:u8,reg:u8)->Option<(usize,u8)>{Some(match(block,reg){
        (0x6d,5)=>(0,0x80),(0x6d,7)=>(1,0x80),(0x6d,13)=>(2,0x0c),(0x6d,14)=>(3,0xff),
        (0x66,2)=>(0,0x7f),(0x66,3)=>(1,0xff),(0x66,5)=>(2,0x77),(0x66,6)=>(3,0xf0),(0x66,9)=>(4,3),_=>return None,
    })}
    pub fn byte(&self,block:u8,reg:u8)->Option<u8>{let(i,_)=Self::slot(block,reg)?;Some(if block==0x6d{self.dig[i]}else{self.bb[i]})}
    pub fn write_byte(&mut self,block:u8,reg:u8,v:u8)->bool{
        let Some((i,mask))=Self::slot(block,reg)else{return false;};let word=if block==0x6d{&mut self.dig[i]}else{&mut self.bb[i]};
        if (*word^v)&!mask!=0{return false;}*word=v;if block==0x6d&&reg==14{self.dcap_written=true;}
        if block==0x66&&!self.configured(){self.cancel();}true
    }
    pub fn reconcile(&mut self,master_clock:bool,route:bool){
        if !self.enabled||!self.powered()||!self.configured(){self.cancel();return;}
        if self.locked{return;}
        if self.armed&&self.conf&12==8&&master_clock&&route{
            if self.deadline.is_none(){if let Some(at)=self.now.checked_add(LOCK_CYCLES){self.deadline=Some(at);self.attempts=self.attempts.saturating_add(1);}}
        }else{self.deadline=None;}
    }
    pub fn pump(&mut self,cycle:u64,master_clock:bool,route:bool){
        if !self.enabled{return;}
        self.now=self.now.max(cycle);self.reconcile(master_clock,route);
        if self.deadline.is_some_and(|at|at<=self.now){self.deadline=None;self.done=true;self.locked=true;self.completions=self.completions.saturating_add(1);}
    }
    pub fn reset(&mut self,hp:bool,lp:bool,analog:bool){
        if analog{let enabled=self.enabled;let now=self.now;*self=Self::new();self.enabled=enabled;self.now=now;return;}
        if hp||lp{self.conf=0x00e408;self.armed=false;self.cancel();}
        if lp{self.power=0;self.fosc=0x4000_0000;}
    }
    pub fn report(&self)->String{format!("pll_profile={} ready={} cal_done={} attempts={} locks={} slow_dcap={:?}",
        if self.enabled{"virtual100us"}else{"disabled"},u8::from(self.ready()),u8::from(self.done),self.attempts,self.completions,self.slow_dcap())}
}
