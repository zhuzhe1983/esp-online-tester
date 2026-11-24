//! C5 LPPERI digital clock/reset controls, fixed IDF v5.5.4 lpperi_reg.h.
//! RNG is an explicitly deterministic virtual source, not silicon entropy or
//! a calibrated RO/timer/CRC mixer. One enabled DATA read requests one virtual
//! sample; SAMPLE_CNT counts these samples modulo256, not physical RO edges.
pub(crate) struct LpPeri{clock:u32,reset:u32,cfg:u32,date:u32,rng:u32,last:u32,samples:u8}
const VIRTUAL_SEED:u32=0xc5e0_1001;
impl LpPeri{
    pub fn new()->Self{Self{clock:0x7f00_0000,reset:0,cfg:0xffe,date:36_774_256,rng:VIRTUAL_SEED,last:0,samples:0}}
    fn reset_rng(&mut self){self.cfg=0xffe;self.rng=VIRTUAL_SEED;self.last=0;self.samples=0;}
    pub fn read(&mut self,offset:u32)->Option<u32>{Some(match offset{
        0=>self.clock,4=>self.reset,0x24=>self.cfg|u32::from(self.samples)<<24,0x3fc=>self.date,
        8|0x28=>{
            if self.clock&(1<<24)!=0&&self.reset&(1<<24)==0&&self.cfg&1!=0{
                let mut x=self.rng;x^=x<<13;x^=x>>17;x^=x<<5;self.rng=x;self.last=x;self.samples=self.samples.wrapping_add(1);
            }self.last
        },_=>return None,
    })}
    pub fn write(&mut self,offset:u32,value:u32)->bool{
        match offset{
            0=>self.clock=value&0xff00_0000,
            4=>{
                // Bus-wide and LP CPU write triggers require devices not yet
                // modeled; they do not silently succeed as cleared register RAM.
                if value&((1<<23)|(1<<31))!=0{return false;}
                self.reset=value&0x7f00_0000;if value&(1<<24)!=0{self.reset_rng();}
            },
            0x24=>{if self.reset&(1<<24)==0{self.cfg=value&0xfff;}},
            8|0x28=>{},0x3fc=>self.date=value,_=>return false,
        }true
    }
}
