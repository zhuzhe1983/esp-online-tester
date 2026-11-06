//! C61 BLE security engine, independently decoded from the IDF v5.5.4 C61
//! controller archive; see docs/esp32c61-ble.md for fixed source provenance.
use esp_periph::{Device,RegRam,WriteEffect};
#[derive(Default)]
pub struct BleSecurity {ram:RegRam,resolve_pending:bool,aes_pending:bool}
impl BleSecurity {
    pub fn pending(&self)->bool {self.resolve_pending||self.aes_pending}
    pub fn service(&mut self,sram:&mut [u8]) {
        if self.resolve_pending {self.resolve_pending=false;self.resolve(sram);}
        if self.aes_pending {self.aes_pending=false;self.encrypt(sram);}
    }
    fn range(address:u32,count:usize,len:usize)->Option<std::ops::Range<usize>> {let start=address.checked_sub(0x40800000)? as usize;let end=start.checked_add(count)?;(end<=len).then_some(start..end)}
    fn resolve(&mut self,sram:&[u8]) {
        let query=self.ram.read(0x8c);let Some(q)=Self::range(0x40800000|(query&0xfffff),6,sram.len()) else {return};
        let mut entry=self.ram.read(0x88);let mut seen=std::collections::HashSet::new();
        self.ram.write(0xa0,0);self.ram.write(0xa4,0xfffff);self.ram.write(0xa8,0);
        for _ in 0..256 {
            if entry==0 {self.ram.write(0xbc,self.ram.read(0xbc)|4);return;}
            if entry&3!=0 || !seen.insert(entry) {return;}
            let Some(r)=Self::range(entry,64,sram.len()) else {return};let data=&sram[r];
            let control=u32::from_le_bytes(data[..4].try_into().unwrap());
            // r_ble_ll_resolv_list_add: identity address at +32, type bit16;
            // insert_eof_head emits 0x80000000 as the empty sentinel.
            if control&0x8000!=0 && (control>>16)&1==(query>>20)&1 && data[32..38]==sram[q.clone()] {
                self.ram.write(0xa0,1<<8);self.ram.write(0xa4,entry&0xfffff);self.ram.write(0xa8,1);self.ram.write(0xbc,self.ram.read(0xbc)|4);return;
            }
            if control&(1<<31)!=0 {self.ram.write(0xbc,self.ram.read(0xbc)|4);return;}
            entry=u32::from_le_bytes(data[4..8].try_into().unwrap());
        }
    }
    fn encrypt(&mut self,sram:&mut [u8]) {
        // r_ble_hw_encrypt_block programs one AES-128 ECB block: key words
        // +10..1c, input/output pointers +20/+24, length +0c and trigger +04.
        if self.ram.read(0x08)!=1 || self.ram.read(0x0c)!=16 {return;}
        let Some(input)=Self::range(self.ram.read(0x20),16,sram.len()) else {return};
        let Some(output)=Self::range(self.ram.read(0x24),16,sram.len()) else {return};
        let mut key=[0;16];for i in 0..4 {key[i*4..i*4+4].copy_from_slice(&self.ram.read(0x10+i as u32*4).to_le_bytes());}
        let block: [u8;16]=sram[input].try_into().unwrap();let encrypted=esp_periph::crypto::aes128_encrypt_block(&key,&block);sram[output].copy_from_slice(&encrypted);
        self.ram.write(0xc4,1);
    }
}
impl Device for BleSecurity {
    fn read(&mut self,off:u32)->u32 {self.ram.read(off)}
    fn write(&mut self,off:u32,value:u32)->WriteEffect {
        match off {
            0x80=>{if value&1!=0 {self.resolve_pending=true;self.ram.write(0xbc,self.ram.read(0xbc)&!4);}},
            0x04=>{if value&1!=0 {self.aes_pending=true;self.ram.write(0xc4,0);}},
            _=>self.ram.write(off,value),
        }
        WriteEffect::NONE
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn put(sram:&mut [u8],off:usize,value:u32) {sram[off..off+4].copy_from_slice(&value.to_le_bytes());}
    fn search(engine:&mut BleSecurity,sram:&mut [u8],kind:u32) {engine.write(0x88,0x40800100);engine.write(0x8c,0x200|kind<<20);engine.write(0x80,1);engine.service(sram);}
    #[test]
    fn empty_identity_type_and_cycle_search() {
        let mut engine=BleSecurity::default();let mut sram=vec![0;1024];put(&mut sram,0x100,1<<31);search(&mut engine,&mut sram,0);assert_eq!(engine.read(0xbc)&4,4);assert_eq!(engine.read(0xa8),0);
        put(&mut sram,0x100,1<<31|1<<16|1<<15);sram[0x120..0x126].copy_from_slice(&[1,2,3,4,5,6]);sram[0x200..0x206].copy_from_slice(&[1,2,3,4,5,6]);
        search(&mut engine,&mut sram,1);assert_eq!(engine.read(0xa8),1);assert_eq!(engine.read(0xa4),0x100);assert_eq!(engine.read(0xa0),1<<8);
        search(&mut engine,&mut sram,0);assert_eq!(engine.read(0xa8),0);assert_eq!(engine.read(0xbc)&4,4);
        put(&mut sram,0x100,0);put(&mut sram,0x104,0x40800100);search(&mut engine,&mut sram,0);assert_eq!(engine.read(0xbc)&4,0,"a cyclic malformed list must not produce a successful completion");
    }
    #[test]
    fn ble_aes_dma_nist_vector_and_invalid_memory() {
        let mut engine=BleSecurity::default();let mut sram=vec![0;64];sram[..16].copy_from_slice(&[0x00,0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88,0x99,0xaa,0xbb,0xcc,0xdd,0xee,0xff]);
        for i in 0..4 {engine.write(0x10+i*4,u32::from_le_bytes(std::array::from_fn(|n|i as u8*4+n as u8)));}
        for (off,v) in [(8,1),(12,16),(0x20,0x40800000),(0x24,0x40800010),(4,1)] {engine.write(off,v);}engine.service(&mut sram);
        assert_eq!(sram[16..32],[0x69,0xc4,0xe0,0xd8,0x6a,0x7b,0x04,0x30,0xd8,0xcd,0xb7,0x80,0x70,0xb4,0xc5,0x5a]);assert_eq!(engine.read(0xc4),1);
        engine.write(0x24,0x60091000);engine.write(4,1);engine.service(&mut sram);assert_eq!(engine.read(0xc4),0);
    }
}

/// Link-layer registers at 0x600a1000. The controller's sleep_timer_tick_get
/// latches a 1 MHz counter with +0x928 bit27, then reads +0x924. Its
/// sleep_time_sync_set writes +0x92c and requests a load with bit31.
#[derive(Default)]
pub struct BleMac {ram:RegRam,pub trace:bool,pub now_cycles:u64,epoch_cycles:u64,epoch_ticks:u32}
impl BleMac {
    fn ticks(&self)->u32 {self.epoch_ticks.wrapping_add((self.now_cycles.saturating_sub(self.epoch_cycles)/160) as u32)}
    pub fn trace_descriptors(&self,sram:&[u8]) {
        let mut ptr=self.ram.read(0x8fc)&0xfffff;
        for _ in 0..8 {if ptr==0 {break;}let Some(data)=sram.get(ptr as usize..ptr as usize+72) else {break};let words:Vec<_>=data.as_chunks::<4>().0.iter().map(|w|u32::from_le_bytes(*w)).collect();eprintln!("[C61 BLE DESC] {ptr:05x} {words:08x?}");ptr=words[0]&0xfffff;}
    }
}
impl Device for BleMac {
    fn read(&mut self,off:u32)->u32 {self.ram.read(off)}
    fn write(&mut self,off:u32,value:u32)->WriteEffect {
        if self.trace && off!=0x928 {eprintln!("[C61 BLE] W {:03x} {:08x} at {}",off,value,self.now_cycles);}
        if off==0x928 {
            if value&(1<<31)!=0 {self.epoch_ticks=self.ram.read(0x92c);self.epoch_cycles=self.now_cycles;}
            if value&(1<<27)!=0 {self.ram.write(0x924,self.ticks());}
            self.ram.write(off,value&!((1<<27)|(1<<31)));
        } else {self.ram.write(off,value);}
        WriteEffect::NONE
    }
}
#[cfg(test)]
mod timer_tests {
    use super::*;
    #[test]
    fn c61_timer_latches_microseconds_loads_and_wraps() {
        let mut mac=BleMac{now_cycles:160*1234+159,..Default::default()};mac.write(0x928,4|1<<27);
        assert_eq!(mac.read(0x928),4);assert_eq!(mac.read(0x924),1234);
        mac.now_cycles+=160;assert_eq!(mac.read(0x924),1234,"latch is stable until triggered");
        mac.write(0x92c,u32::MAX);mac.write(0x928,4|1<<31);mac.now_cycles+=320;mac.write(0x928,4|1<<27);
        assert_eq!(mac.read(0x924),1);assert_eq!(mac.read(0x928),4);
    }
}
