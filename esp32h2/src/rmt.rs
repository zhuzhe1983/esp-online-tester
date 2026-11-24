//! H2-specific finite TX subset, not the C6/S3 register-status layout.
//! IDF v5.5.1@fcae32885b0296b32044cb99ecbdc50d98dddb83 H2 rmt_reg.h/rmt_ll.h:
//! two TX, two RX blocks,48 symbols/block,CONF0+10/14,RAW/ST/ENA/CLR+38/3c/40/44.
//! RX,carrier,wrap,loop,DMA and private FSM status are deliberately unsupported.
use emu_core::Fault;
const WORDS:usize=48;
const TOTAL:usize=WORDS*4;
const SYS_RESET:u32=0x0500_0010;
const INT_MASK:u32=3|(3<<4)|(3<<8);
#[derive(Default)]struct Tx{running:bool,read:usize,remaining:u64,end_after_symbol:bool,bits:Vec<bool>,since_threshold:u32}
pub struct H2Rmt{
 conf:[u32;2],wr:[usize;2],tx:[Tx;2],memory:[u32;TOTAL],known:[bool;TOTAL],
 pub sys:u32,pub int_raw:u32,int_ena:u32,limit:[u32;2],pub done:Vec<(usize,Vec<bool>)>,
}
impl Default for H2Rmt{fn default()->Self{Self{conf:[0x0071_0200;2],wr:[0;2],tx:Default::default(),memory:[0;TOTAL],known:[false;TOTAL],sys:SYS_RESET,int_raw:0,int_ena:0,limit:[128;2],done:vec![]}}}
impl H2Rmt{
 pub fn running(&self,ch:usize)->bool{self.tx[ch].running}
 pub fn busy(&self)->bool{self.tx.iter().any(|c|c.running)}
 pub fn reset(&mut self){*self=Self::default();}
 fn register_clock(&self)->bool{self.sys&(1<<31)!=0}
 fn memory_on(&self)->bool{self.sys&2!=0&&self.sys&4==0}
 pub fn read(&self,off:u32)->Result<u32,Fault>{
  if off==0x68{return Ok(self.sys);}
  if !self.register_clock(){return Err(Fault::Prohibited);}
  match off{
   0x10|0x14=>Ok(self.conf[((off-0x10)/4)as usize]&!((1<<24)|0x87)),
   0x38=>Ok(self.int_raw),0x3c=>Ok(self.int_raw&self.int_ena),0x40=>Ok(self.int_ena),0x44=>Ok(0),
   0x58|0x5c=>Ok(self.limit[((off-0x58)/4)as usize]),0x70=>Ok(0),
   0x400..=0x6fc if off&3==0&&self.memory_on()&&self.sys&1!=0=>{let i=((off-0x400)/4)as usize;if self.known[i]{Ok(self.memory[i])}else{Err(Fault::Prohibited)}},
   _=>Err(Fault::Unmapped),
  }
 }
 pub fn write(&mut self,off:u32,v:u32)->Result<(),Fault>{
  if off==0x68{
   // Clock selection/dividers live inPCR on H2. Preserve the legacy fields
   // at their documented POR rather than inventing a second active clock.
   if v&0x7fff_fff0!=SYS_RESET||v&0xc==0xc{return Err(Fault::Prohibited);}
   if v&4!=0{self.tx=Default::default();self.known.fill(false);self.done.clear();}
   self.sys=v;return Ok(());
  }
  if !self.register_clock(){return Err(Fault::Prohibited);}
  match off{
   0x00|0x04=>{
    if !self.memory_on()||self.sys&1!=0{return Err(Fault::Prohibited);}
    let ch=(off/4)as usize;if self.wr[ch]>=WORDS{return Err(Fault::Prohibited);}let i=ch*WORDS+self.wr[ch];self.memory[i]=v;self.known[i]=true;self.wr[ch]+=1;
   },
   0x10|0x14=>{
    let ch=((off-0x10)/4)as usize;let command=(1<<24)|0x87;
    if v&!0x0177_ffff!=0||v&(1<<23)!=0{return Err(Fault::Prohibited);}
    if self.tx[ch].running&&v&0x80==0&&(v&!command!=self.conf[ch]&!command||v&7!=0){return Err(Fault::Prohibited);}
    if v&1!=0{
     let blocks=((v>>16)&7)as usize;
     if v&((1<<21)|0x18)!=0||blocks==0||ch+blocks>4||!self.memory_on(){return Err(Fault::Prohibited);}
     let other=1-ch;let other_end=other+((self.conf[other]>>16)&7)as usize;
     if self.tx[other].running&&ch<other_end&&other<ch+blocks{return Err(Fault::Prohibited);}
    }
    self.conf[ch]=v&!command;
    if v&0x80!=0{self.tx[ch]=Tx::default();}
    if v&2!=0{self.tx[ch].read=0;}
    if v&4!=0{self.wr[ch]=0;}
    if v&1!=0{self.tx[ch]=Tx{running:true,..Default::default()};}
   },
   0x40=>{if v&!INT_MASK!=0{return Err(Fault::Prohibited);}self.int_ena=v;},
   0x44=>self.int_raw&=!(v&INT_MASK),
   0x58|0x5c=>{if v&!0x1ff!=0{return Err(Fault::Prohibited);}self.limit[((off-0x58)/4)as usize]=v;},
   0x70=>{if v&!3!=0||self.busy(){return Err(Fault::Prohibited);}},
   0x400..=0x6fc if off&3==0=>{if !self.memory_on()||self.sys&1==0{return Err(Fault::Prohibited);}let i=((off-0x400)/4)as usize;self.memory[i]=v;self.known[i]=true;},
   _=>return Err(Fault::Unmapped),
  }Ok(())
 }
 pub fn advance(&mut self,ticks:u64){
  if !self.register_clock()||!self.memory_on(){return;}
  for ch in 0..2{
   let c=&mut self.tx[ch];if !c.running{continue;}let mut left=ticks;
   loop{
    if c.remaining>left{c.remaining-=left;break;}
    left-=c.remaining;c.remaining=0;
    if c.end_after_symbol{c.running=false;self.int_raw|=1<<ch;self.done.push((ch,std::mem::take(&mut c.bits)));break;}
    let allocated=((self.conf[ch]>>16)&7)as usize*WORDS;
    let i=ch*WORDS+c.read;
    if c.read>=allocated||i>=TOTAL||!self.known[i]{c.running=false;self.int_raw|=1<<(4+ch);c.bits.clear();break;}
    let symbol=self.memory[i];let d0=u64::from(symbol&0x7fff);let d1=u64::from((symbol>>16)&0x7fff);
    if d0==0{c.running=false;self.int_raw|=1<<ch;self.done.push((ch,std::mem::take(&mut c.bits)));break;}
    // Reuse only the shared ratio-based digital LED decoder, not C6 MMIO/timing.
    c.bits.push(esp_periph::rmt::symbol_bit(symbol));c.read+=1;c.since_threshold+=1;
    if self.limit[ch]!=0&&c.since_threshold>=self.limit[ch]{c.since_threshold=0;self.int_raw|=1<<(8+ch);}
    let div=(self.conf[ch]>>8)&255;let div=if div==0{256}else{div};
    c.remaining=(d0+d1)*u64::from(div);c.end_after_symbol=d1==0;
    if left==0{break;}
   }
  }
 }
}
