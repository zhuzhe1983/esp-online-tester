//! C61 BLE security and legacy advertising/scanning, independently decoded from the IDF v5.5.4 C61
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
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BleFrame {pub timestamp_us:u32,pub channel:u8,pub pdu:Vec<u8>}
#[derive(Default)]
pub struct BleMac {ram:RegRam,pub trace:bool,pub now_cycles:u64,epoch_cycles:u64,epoch_ticks:u32,current:u32,started:bool,pub running:bool,deadline_us:u32,pub events:u64,tx:std::collections::VecDeque<BleFrame>,nearby:Vec<(esp_soc::ble::peer::Nearby,u32)>,pub received:u64}
impl BleMac {
    fn ticks(&self)->u32 {self.epoch_ticks.wrapping_add((self.now_cycles.saturating_sub(self.epoch_cycles)/160) as u32)}
    pub fn service(&mut self,sram:&mut [u8],cycles:u64)->bool {
        self.now_cycles=cycles;let before=self.irq_sources();
        if !self.running {return false;}
        let ptr=self.current as usize;
        let Some(data)=sram.get(ptr..ptr+72) else {self.running=false;return false};
        let words:[u32;18]=std::array::from_fn(|i|u32::from_le_bytes(data[i*4..i*4+4].try_into().unwrap()));let word=|off:usize|words[off/4];
        if !matches!((word(4)>>20)&15,6|12|13) {self.running=false;return false;}
        if !self.started {
            self.deadline_us=word(8);
            if (self.ticks().wrapping_sub(self.deadline_us) as i32) < 0 {return false;}
            self.started=true;self.deadline_us=word(12);
            if (word(4)>>20)&15==6 {self.transmit(sram,&words);} else {self.receive_nearby(sram,&words);}
            self.ram.write(0x900,self.current|1<<31|1<<29);self.ram.write(0x904,self.current);
        }
        if (self.ticks().wrapping_sub(self.deadline_us) as i32) < 0 {return false;}
        let next=word(0)&0xfffff;
        sram[ptr+40..ptr+44].copy_from_slice(&0u32.to_le_bytes());self.events+=1;
        if self.trace {eprintln!("[C61 BLE END] {:05x} next {:05x} at {}",self.current,next,self.ticks());}
        self.current=next;self.started=false;
        self.ram.write(0x900,if next==0 {0}else{next|1<<31|1<<29});self.ram.write(0x904,next);
        self.ram.write(0x30c,self.ram.read(0x30c)|1<<28);
        if next==0 {self.running=false;self.ram.write(0x31c,self.ram.read(0x31c)|8);}
        before!=self.irq_sources()
    }
    pub fn set_nearby(&mut self,list:Vec<esp_soc::ble::peer::Nearby>)->Result<(),String> {
        if list.len()>esp_soc::ble::peer::NEARBY_MAX {return Err("at most 32 nearby BLE advertisers".into());}
        for (i,n) in list.iter().enumerate() {
            if !n.scan_response.is_empty() {return Err("C61 nearby scan-response exchange is not implemented".into());}
            if n.addr_type>1 || (n.addr_type==1 && n.addr[5]>>6==2) || n.adv.len()>31 || n.scan_response.len()>31 || !((-127..=20).contains(&n.rssi)||n.rssi==127) {return Err("invalid BLE address type, RSSI, or legacy advertising length".into());}
            if list[..i].iter().any(|other|other.addr==n.addr) {return Err("duplicate nearby BLE address".into());}
        }
        let now=self.ticks();self.nearby=list.into_iter().enumerate().map(|(i,n)|(n,now.wrapping_add(i as u32*310))).collect();Ok(())
    }
    pub fn surviving_nearby(&self)->Vec<esp_soc::ble::peer::Nearby> {self.nearby.iter().map(|(n,_)|n.clone()).collect()}
    fn receive_nearby(&mut self,sram:&mut [u8],schedule:&[u32;18]) {
        let now=self.ticks();let Some(index)=self.nearby.iter().position(|(_,at)|(now.wrapping_sub(*at) as i32)>=0) else {return};
        let n=&self.nearby[index].0;let mut pdu=vec![6|n.addr_type<<6,6+n.adv.len() as u8];pdu.extend_from_slice(&n.addr);pdu.extend_from_slice(&n.adv);
        let rssi=n.rssi;
        if self.receive(sram,schedule,&pdu,rssi) {self.nearby[index].1=now.wrapping_add(100_000);}
    }
    fn receive(&mut self,sram:&mut [u8],schedule:&[u32;18],pdu:&[u8],rssi:i8)->bool {
        if !(8..=39).contains(&pdu.len()) || pdu[1] as usize+2!=pdu.len() || !matches!((schedule[5]>>8)&127,0|24|78) || (schedule[4]>>28)&3!=0 {return false;}
        let link=(schedule[1]&0xfffff) as usize;let Some(l)=sram.get(link..link+120) else {return false};
        let mut header=u32::from_le_bytes(l[8..12].try_into().unwrap())&0xfffff;
        for _ in 0..256 {
            let Some(at)=header.checked_sub(4).map(|v|v as usize) else {return false};let Some(h)=sram.get(at..at+16) else {return false};
            let next=u32::from_le_bytes(h[4..8].try_into().unwrap())&0xfffff;let packet=(u32::from_le_bytes(h[8..12].try_into().unwrap())&0xfffff) as usize;
            if packet==0 {if next==header {return false;}header=next;continue;}
            let Some(r)=sram.get_mut(packet..packet+28+pdu.len()) else {return false};
            if u32::from_le_bytes(r[12..16].try_into().unwrap())&0xffffff!=0xffffff || u16::from_le_bytes(r[24..26].try_into().unwrap())!=0xffff {return false;}
            r[12..16].copy_from_slice(&((rssi as u8 as u32)<<24).to_le_bytes());r[16..20].copy_from_slice(&schedule[2].to_le_bytes());r[20..24].copy_from_slice(&0xfffffu32.to_le_bytes());
            // scan_alloc_memory +f8..128 programs S+32 with the scan identity;
            // scan_rx_process +2a..7e validates the same identity in R+24.
            let metadata=(schedule[8]&0xffff)|((schedule[5]>>8)&127)<<16|((schedule[4]>>28)&3)<<24;
            r[24..28].copy_from_slice(&metadata.to_le_bytes());r[28..].copy_from_slice(pdu);
            let current=u32::from_le_bytes(sram[link+8..link+12].try_into().unwrap());sram[link+8..link+12].copy_from_slice(&((current&!0xfffff)|header).to_le_bytes());
            let state=u32::from_le_bytes(sram[link+20..link+24].try_into().unwrap());sram[link+20..link+24].copy_from_slice(&(state&!(1<<27)).to_le_bytes());
            self.received+=1;if self.trace {eprintln!("[C61 BLE RX] S={:05x} L={link:05x} H={at:05x} R={packet:05x} metadata={metadata:08x} pdu={}",self.current,esp_soc::ble::peer::hex(pdu));}return true;
        }false
    }
    pub fn take_tx(&mut self)->Option<BleFrame> {self.tx.pop_front()}
    fn transmit(&mut self,sram:&[u8],schedule:&[u32;18]) {
        let link=(schedule[1]&0xfffff) as usize;let Some(l)=sram.get(link..link+120) else {return};
        let lw=|off:usize|u32::from_le_bytes(l[off..off+4].try_into().unwrap());
        let Some(header)=(lw(0)&0xfffff).checked_sub(4) else {return};let Some(h)=sram.get(header as usize..header as usize+16) else {return};
        let ptr=(u32::from_le_bytes(h[8..12].try_into().unwrap())&0xfffff) as usize;if ptr==0 {return;}
        let Some(t)=sram.get(ptr..ptr+18) else {return};let kind=t[16]&15;let length=t[17] as usize;
        if !matches!(kind,0|2|4|6) || !(6..=37).contains(&length) {return;}
        let Some(data)=sram.get(ptr+18..ptr+18+length-6) else {return};
        let mut address=[0u8;6];match (lw(28)>>28)&3 {
            0=>{let base=if t[16]&64==0 {0x80c}else{0x814};address[..4].copy_from_slice(&self.ram.read(base).to_le_bytes());address[4..].copy_from_slice(&self.ram.read(base+4).to_le_bytes()[..2]);},
            1=>address.copy_from_slice(&l[52..58]),_=>return,
        }
        let channel=match (schedule[5]>>8)&127 {0=>37,24=>38,78=>39,_=>return};
        let mut pdu=Vec::with_capacity(length+2);pdu.extend_from_slice(&[t[16],t[17]]);pdu.extend_from_slice(&address);pdu.extend_from_slice(data);
        if self.trace {eprintln!("[C61 BLE TX] us={} channel={} pdu={}",schedule[2],channel,esp_soc::ble::peer::hex(&pdu));}
        if self.tx.len()==256 {self.tx.pop_front();}self.tx.push_back(BleFrame{timestamp_us:schedule[2],channel,pdu});
    }
    pub fn trace_descriptors(&self,sram:&[u8]) {
        let mut ptr=self.ram.read(0x8fc)&0xfffff;
        for _ in 0..8 {if ptr==0 {break;}let Some(data)=sram.get(ptr as usize..ptr as usize+72) else {break};let words:Vec<_>=data.as_chunks::<4>().0.iter().map(|w|u32::from_le_bytes(*w)).collect();eprintln!("[C61 BLE DESC] {ptr:05x} {words:08x?}");ptr=words[0]&0xfffff;}
    }
}
impl Device for BleMac {
    fn irq_sources(&self)->u64 {u64::from((self.ram.read(0x304)&self.ram.read(0x30c))|(self.ram.read(0x314)&self.ram.read(0x31c))!=0)}
    fn read(&mut self,off:u32)->u32 {self.ram.read(off)}
    fn write(&mut self,off:u32,value:u32)->WriteEffect {
        if self.trace && off!=0x928 {eprintln!("[C61 BLE] W {:03x} {:08x} at {}",off,value,self.now_cycles);}
        if off==0x028 && value&1!=0 {self.current=self.ram.read(0x8fc)&0xfffff;self.started=false;self.running=self.current!=0;self.ram.write(0x900,if self.running {self.current|1<<31|1<<29}else{0});self.ram.write(0x904,self.current);}
        if off==0x02c && value&1!=0 {self.running=false;self.ram.write(0x900,0);self.ram.write(0x904,0);}
        if matches!(off,0x308|0x318) {let raw=off+4;self.ram.write(raw,self.ram.read(raw)&!value);return WriteEffect::NONE;}
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

#[cfg(test)]
mod scheduler_tests {
    use super::*;
    fn put(memory:&mut [u8],at:usize,value:u32) {memory[at..at+4].copy_from_slice(&value.to_le_bytes());}
    #[test]
    fn c61_scheduler_completion_returns_ownership_and_irq_then_advances() {
        let mut mac=BleMac::default();let mut memory=vec![0;1024];
        for (ptr,next,start,end) in [(0x100,0x200,100,200),(0x200,0,300,400)] {for (offset,value) in [(0,next),(4,6<<20|0x300),(8,start),(12,end),(40,0x2000)] {put(&mut memory,ptr+offset,value);}}
        mac.write(0x304,1<<28);mac.write(0x314,8);mac.write(0x8fc,0x100);mac.write(0x28,1);
        assert_eq!(mac.read(0x900),0xa0000100,"prefetch is visible before the scheduled radio interval");
        mac.service(&mut memory,199*160);assert_eq!(mac.events,0);assert_eq!(mac.irq_sources(),0);
        assert!(mac.service(&mut memory,200*160));assert_eq!(mac.events,1);assert_eq!(mac.read(0x900),0xa0000200);assert_eq!(mac.read(0x904),0x200);assert_eq!(&memory[0x128..0x12c],&[0;4]);
        mac.write(0x308,1<<28);assert_eq!(mac.irq_sources(),0);mac.service(&mut memory,400*160);
        assert_eq!(mac.events,2);assert_eq!(mac.read(0x900),0);assert!(!mac.running);assert_eq!(mac.read(0x31c),8);
        mac.write(0x308,u32::MAX);mac.write(0x318,u32::MAX);assert_eq!(mac.irq_sources(),0);
    }
    #[test]
    fn unknown_operations_and_invalid_descriptors_never_report_completion() {
        let mut mac=BleMac::default();let mut memory=vec![0;1024];mac.write(0x8fc,0x100);mac.write(0x28,1);mac.service(&mut memory,16000);assert_eq!(mac.events,0);assert!(!mac.running);
        mac.write(0x8fc,0xffffc);mac.write(0x28,1);mac.service(&mut memory,16000);assert_eq!(mac.events,0);assert!(!mac.running);
    }
}

#[cfg(test)]
mod rx_tests {
    use super::*;
    fn put(memory:&mut [u8],at:usize,value:u32) {memory[at..at+4].copy_from_slice(&value.to_le_bytes());}
    fn fixture()->(BleMac,Vec<u8>,[u32;18]) {
        let mut memory=vec![0;4096];let mut schedule=[0u32;18];schedule[1]=12<<20|0x200;schedule[2]=123;schedule[4]=1<<20;schedule[5]=24<<8;schedule[8]=6;
        for (at,value) in [(0x208,0xff00000|0x304),(0x214,1<<27),(0x304,0x324),(0x324,0x344),(0x328,0x400),(0x40c,0xffffff),(0x418,0xffff)] {put(&mut memory,at,value);}
        (BleMac::default(),memory,schedule)
    }
    #[test]
    fn c61_rx_walks_empty_head_and_returns_packet_through_hardware_fields() {
        let (mut mac,mut memory,schedule)=fixture();let pdu=b"\x06\x09\x01\x02\x03\x04\x05\x06\x02\x01\x06";
        assert!(mac.receive(&mut memory,&schedule,pdu,-42));
        assert_eq!(&memory[0x41c..0x41c+pdu.len()],pdu);assert_eq!(memory[0x40f],(-42i8) as u8);assert_eq!(&memory[0x410..0x414],&123u32.to_le_bytes());
        assert_eq!(&memory[0x414..0x418],&0xfffffu32.to_le_bytes());assert_eq!(&memory[0x418..0x41c],&0x00180006u32.to_le_bytes());
        assert_eq!(&memory[0x208..0x20c],&0x0ff00324u32.to_le_bytes());assert_eq!(&memory[0x214..0x218],&0u32.to_le_bytes());
        assert_eq!(memory[0x300],0,"guest software owns the skipped header and software list head");
        let before=memory.clone();assert!(!mac.receive(&mut memory,&schedule,pdu,-1));assert_eq!(memory,before,"do not overwrite an unconsumed completed buffer");
    }
    #[test]
    fn c61_rx_rejects_invalid_chain_buffer_and_air_packet() {
        let (mut mac,mut memory,schedule)=fixture();let pdu=b"\x06\x06\x01\x02\x03\x04\x05\x06";
        put(&mut memory,0x304,0x304);let before=memory.clone();assert!(!mac.receive(&mut memory,&schedule,pdu,-42));assert_eq!(memory,before);
        put(&mut memory,0x304,0x324);put(&mut memory,0x328,0xffffc);let before=memory.clone();assert!(!mac.receive(&mut memory,&schedule,pdu,-42));assert_eq!(memory,before);
        let (_,mut memory,schedule)=fixture();let before=memory.clone();assert!(!mac.receive(&mut memory,&schedule,b"\x06\x07",-42));assert_eq!(memory,before);
    }
    #[test]
    fn c61_nearby_configuration_is_bounded_and_preserves_previous_on_error() {
        let mut mac=BleMac::default();let peer=esp_soc::ble::peer::Nearby::parse("name=test").unwrap();mac.set_nearby(vec![peer.clone()]).unwrap();
        let mut response=peer.clone();response.scan_response=vec![2,1,6];assert!(mac.set_nearby(vec![response]).is_err());
        assert!(mac.set_nearby(vec![peer.clone();33]).is_err());assert!(mac.set_nearby(vec![peer.clone();2]).is_err());assert_eq!(mac.surviving_nearby(),vec![peer]);
        mac.set_nearby(Vec::new()).unwrap();assert!(mac.surviving_nearby().is_empty());
    }
}
