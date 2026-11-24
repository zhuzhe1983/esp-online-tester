//! C5 LP_WDT digital configuration subset. Fixed IDF v5.5.4 lp_wdt_reg.h
//! and lpwdt_ll.h define separate RWDT/SWD keys, both 0x50d83aa1 on C5.
//! No calibrated LP clock, autonomous timeout, reset pulse or IRQ is modeled.
//! Enabling RWDT, re-enabling SWD, or requesting an active SWD feed faults.
const KEY:u32=0x50d8_3aa1;
const SWD_RW:u32=(1<<18)|(0x3ff<<20)|(1<<30);
pub(crate) struct LpWdt{config:u32,hold:[u32;4],key:u32,swd:u32,swd_key:u32,swd_flag:bool,date:u32}
impl LpWdt{
    pub fn new()->Self{Self{config:0x13214,hold:[200_000,80_000,4095,4095],key:0,swd:300<<20,swd_key:0,swd_flag:false,date:34_676_864}}
    pub fn read(&self,off:u32)->Option<u32>{Some(match off{
        0=>self.config,4..=0x10 if off.is_multiple_of(4)=>self.hold[(off/4-1)as usize],0x14=>0,0x18=>self.key,
        0x1c=>self.swd|u32::from(self.swd_flag),0x20=>self.swd_key,0x3fc=>self.date,_=>return None,
    })}
    pub fn write(&mut self,off:u32,v:u32)->bool{
        match off{
            0x18=>self.key=v,0x20=>self.swd_key=v,0x3fc=>self.date=v,
            0=>{if self.key==KEY{if v&(1<<31)!=0{return false;}self.config=v;}},
            4..=0x10 if off.is_multiple_of(4)=>{if self.key==KEY{self.hold[(off/4-1)as usize]=v;}},
            0x14=>{}, // feed is a write trigger; RWDT remains disabled in this subset
            0x1c=>{if self.swd_key==KEY{
                let next=v&SWD_RW;
                if next&(1<<30)==0&&((self.swd^next)&((1<<30)|(1<<18))!=0||v&(1<<31)!=0){return false;}
                self.swd=next;if v&(1<<19)!=0{self.swd_flag=false;}
            }},
            _=>return false,
        }true
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn only_the_write_trigger_can_clear_an_asserted_flag_and_keys_are_independent(){
        let mut w=LpWdt::new();w.swd_flag=true;w.write(0x18,KEY);w.write(0x1c,(300<<20)|(1<<19));assert_eq!(w.read(0x1c),Some((300<<20)|1));
        w.write(0x20,KEY);w.write(0x1c,300<<20);assert_eq!(w.read(0x1c),Some((300<<20)|1));w.write(0x1c,(300<<20)|(1<<19));assert_eq!(w.read(0x1c),Some(300<<20));
        w.write(0x1c,(300<<20)|1);assert_eq!(w.read(0x1c),Some(300<<20),"guest cannot set the RO reset flag");
    }
}
