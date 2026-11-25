//! C5 TIMG0 RTC calibration: count selected oscillator edges against XTAL.
//! Fixed IDF5.5.3 rtc_time.c/timer_group_reg.h and TRM1.1 15.4.5/15.7.5.
//! Counter phases start at zero for each measurement; no analog phase or
//! clock-domain latency is claimed. Whole periodic windows aggregate in O(1).
const TIMEBASE:u64=240_000_000;const XTAL:u64=48_000_000;const VALUE_MASK:u32=0x01ff_ffff;
const CYCLING:u32=1<<12;const START:u32=1<<31;const CONFIG_MASK:u32=0xffff_1000;
#[derive(Clone,Copy,Debug)]
pub(crate) struct Rate{pub hz:u32,pub divider:u16}
impl Rate{
    fn denominator(self)->u64{TIMEBASE*u64::from(self.divider)}
    fn ticks(self,cycles:u64,phase:&mut u64)->u64{let n=u128::from(cycles)*u128::from(self.hz)+u128::from(*phase);let d=u128::from(self.denominator());*phase=(n%d)as u64;(n/d)as u64}
    fn until(self,edges:u64,phase:u64)->u64{let n=u128::from(edges)*u128::from(self.denominator());n.saturating_sub(u128::from(phase)).div_ceil(u128::from(self.hz))as u64}
}
pub(crate) struct Calibration{
    config:u32,value:u32,timeout_conf:u32,ready:bool,valid:bool,timeout:bool,running:bool,
    source_count:u64,reference_count:u64,source_phase:u64,reference_phase:u64,reset_left:u64,
    completions:u64,timeouts:u64,
}
impl Calibration{
    pub fn new()->Self{Self{config:0x0001_1000,value:0,timeout_conf:0xffff_ff98,ready:false,valid:false,timeout:false,running:true,
        source_count:0,reference_count:0,source_phase:0,reference_phase:0,reset_left:0,completions:0,timeouts:0}}
    fn cycling(&self)->bool{self.config&CYCLING!=0}
    fn maximum(&self)->u64{u64::from((self.config>>16)&0x7fff)}
    fn threshold(&self)->u64{u64::from(self.timeout_conf>>7)+1} // header: exceeds the threshold
    fn reset_ticks(&self)->u64{u64::from((self.timeout_conf>>3)&15)}
    fn clear_counts(&mut self){self.source_count=0;self.reference_count=0;self.source_phase=0;self.reference_phase=0;self.reset_left=0;}
    fn start(&mut self){self.clear_counts();self.ready=false;self.valid=false;self.timeout=false;self.running=true;}
    fn reference()->Rate{Rate{hz:XTAL as u32,divider:1}}
    fn event(&self,rate:Option<Rate>)->(u64,bool){
        let timeout=Self::reference().until(self.threshold().saturating_sub(self.reference_count),self.reference_phase);
        let done=rate.map(|r|r.until(self.maximum().saturating_sub(self.source_count),self.source_phase));
        done.filter(|d|*d<=timeout).map_or((timeout,false),|d|(d,true))
    }
    pub fn next(&self,rate:Option<Rate>,reference_on:bool)->Option<u64>{
        if !self.running||!reference_on{return None;}
        Some(if self.reset_left!=0{Self::reference().until(self.reset_left,self.reference_phase)}else{self.event(rate).0})
    }
    fn result(&mut self,done:bool,value:u64,count:u64){
        if done{self.value=value as u32&VALUE_MASK;self.completions=self.completions.saturating_add(count);if self.cycling(){self.valid=true;}else{self.ready=true;}}
        else{self.timeout=true;self.timeouts=self.timeouts.saturating_add(count);}
    }
    pub fn advance(&mut self,mut cycles:u64,rate:Option<Rate>,reference_on:bool){
        if !self.running||!reference_on{return;}
        // At most a partial window, cooldown, arithmetic whole-window catch-up
        // and a partial tail. This does not loop once per elapsed measurement.
        while cycles!=0&&self.running{
            if self.reset_left==0&&self.source_count==0&&self.reference_count==0&&self.source_phase==0&&self.reference_phase==0&&self.cycling(){
                let (measurement,done)=self.event(rate);let period=measurement+if done{0}else{Self::reference().until(self.reset_ticks(),0)};
                let count=cycles/period;
                if count!=0{self.result(done,measurement/5,count);cycles%=period;if cycles==0{return;}}
            }
            if self.reset_left!=0{
                let distance=Self::reference().until(self.reset_left,self.reference_phase);let step=cycles.min(distance);
                self.reset_left=self.reset_left.saturating_sub(Self::reference().ticks(step,&mut self.reference_phase));cycles-=step;
                if step<distance{return;}self.clear_counts();continue;
            }
            let (distance,done)=self.event(rate);let step=cycles.min(distance);
            self.reference_count+=Self::reference().ticks(step,&mut self.reference_phase);
            if let Some(r)=rate{self.source_count+=r.ticks(step,&mut self.source_phase);}cycles-=step;
            if step<distance{return;}
            self.result(done,self.reference_count,1);self.clear_counts();self.running=self.cycling();
            if self.running&&!done{self.reset_left=self.reset_ticks();}
        }
    }
    /// A mux/divider change affects subsequent edges. The functional mux starts
    /// its sub-edge phase anew; already counted whole edges are preserved.
    pub fn source_changed(&mut self){self.source_phase=0;}
    pub fn read(&self,offset:u32)->Option<u32>{Some(match offset{
        0x68=>self.config|u32::from(self.ready)<<15,0x6c=>self.value<<7|u32::from(self.valid),
        0x80=>self.timeout_conf|u32::from(self.timeout),_=>return None,
    })}
    pub fn write(&mut self,offset:u32,value:u32)->bool{match offset{
        0x68=>{
            if value&!(CONFIG_MASK|(1<<15))!=0{return false;}let next=value&CONFIG_MASK;
            if next&(CYCLING|START)!=0&&next&0x7fff_0000==0||next&(CYCLING|START)==(CYCLING|START){return false;}
            let new_start=next&START!=0&&self.config&START==0;let new_cycling=next&CYCLING!=0&&self.config&CYCLING==0;
            if self.running&&next&0x7fff_0000!=self.config&0x7fff_0000&&next&(CYCLING|START)!=0&&!new_start&&!new_cycling{return false;}
            self.config=next;
            if new_start||new_cycling{self.start();}else if next&(CYCLING|START)==0{self.running=false;self.ready=false;self.clear_counts();}
        },
        0x6c=>{}, // actual read-only counter result
        0x80=>{if value&6!=0{return false;}self.timeout_conf=value&!7;},_=>return false,
    }true}
    pub fn report(&self)->String{format!("rtc_cal=virtual measured={} timeouts={} running={} value={}",self.completions,self.timeouts,u8::from(self.running),self.value)}
}
