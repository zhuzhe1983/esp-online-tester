//! C5 cache CPU-bus gates and immutable geometry, not a cache-line engine.
//! ESP-IDF v5.5.4 cache_reg.h and cache_ll_l1_{en,dis}able_bus:
//! 600c8004 bit0 = core0 IBUS0, bit1 = core0 DBUS0, both POR enabled.
//! Maintenance, freeze, prefetch, tracing and tag/data inspection are absent.
pub(crate) struct CacheControls{shut:u32,date:u32}
impl CacheControls{
    pub fn new()->Self{Self{shut:0,date:36_773_968}}
    pub fn cpu_allowed(&self,instruction:bool)->bool{self.shut&if instruction{1}else{2}==0}
    pub fn read(&self,off:u32)->Option<u32>{Some(match off{
        0=>0,4=>self.shut,
        // Header HRO: fixed 4KiB I/64B line and 32KiB shared/32B line.
        0x10=>16,0x14=>8,0x18=>128,0x1c=>4,0x3fc=>self.date,_=>return None,
    })}
    pub fn write(&mut self,off:u32,value:u32)->bool{match off{
        0=>value&12==0, // the only named fields here are explicitly Reserved
        4=>{self.shut=value&3;true},
        0x10|0x14|0x18|0x1c=>true, // HRO fields cannot be changed by stores
        0x3fc=>{self.date=value&0x0fff_ffff;true},_=>false,
    }}
}
