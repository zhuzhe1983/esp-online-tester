//! SX1276 LoRa digital subset local to T-Beam; no RF, FSK/OOK or automatic peer.
//! Semtech DS.SX1276-7-8-9.W.APP Rev 7 §§4.1, 6.4 and 7; register usage
//! cross-checked with LilyGo-LoRa-Series 1991b1ed bundled RadioLib SX127x.
use std::collections::VecDeque;
use esp_soc::devices::sx1262::{LoRaConfig,LoRaInput,LoRaPacket};
use crate::periph::CPU_HZ;
enum Event {Tx(LoRaPacket),Rx(Vec<u8>,i16,i8,bool),Timeout}
pub struct Sx1276 {
    common:[u8;128],lora:[u8;64],fsk:[u8;64],fifo:[u8;256],pub powered:bool,pub reset_high:bool,
    selected:bool,address:Option<u8>,write:bool,now:u64,event:Option<(u64,Event)>,
    pub tx_count:u64,pub rx_count:u64,pub rejected:u64,packets:VecDeque<LoRaPacket>,
}
impl Sx1276 {
    pub fn new()->Self {let mut s=Self{common:[0;128],lora:[0;64],fsk:[0;64],fifo:[0;256],powered:false,reset_high:true,selected:false,address:None,write:false,now:0,event:None,tx_count:0,rx_count:0,rejected:0,packets:VecDeque::new()};s.reset();s}
    fn reset(&mut self) {
        self.common.fill(0);self.lora.fill(0);self.fsk.fill(0);self.fifo.fill(0);self.address=None;self.event=None;
        for (a,v) in [(1,1),(6,0x6c),(7,0x80),(9,0x4f),(0xa,9),(0xb,0x2b),(0xc,0x20),(0x42,0x12),(0x4b,9),(0x4d,0x84)] {self.common[a]=v;}
        for (a,v) in [(0xe,0x80),(0x18,0x10),(0x1d,0x72),(0x1e,0x70),(0x1f,0x64),(0x21,8),(0x22,1),(0x23,0xff),(0x26,4),(0x31,3),(0x33,0x27),(0x37,0xa),(0x39,0x12),(0x3b,0x1d)] {self.lora[a]=v;}
        self.fsk[0x3b]=2;self.fsk[0x3e]=0x80;self.fsk[0x3f]=0x40;
    }
    pub fn controls(&mut self,power:bool,reset:bool) {if (!power && self.powered)||(!reset && self.reset_high) {self.reset();}self.powered=power;self.reset_high=reset;}
    fn is_lora(&self)->bool {self.common[1]&0x80!=0}
    fn mode(&self)->u8 {self.common[1]&7}
    pub fn select(&mut self,low:bool) {if low!=self.selected {self.address=None;self.selected=low;}}
    pub fn transfer(&mut self,tx:&[u8],rx_len:usize)->Vec<u8> {
        let mut rx=vec![0xff;rx_len];if !self.powered||!self.reset_high||!self.selected {return rx;}
        for i in 0..tx.len().max(rx_len) {let b=tx.get(i).copied().unwrap_or(0);let out=if let Some(a)=self.address {let v=if self.write {self.write_reg(a,b);0}else {self.read_reg(a)};if a!=0 {self.address=Some(a.wrapping_add(1)&0x7f);}v}else {self.address=Some(b&0x7f);self.write=b&0x80!=0;0};if let Some(v)=rx.get_mut(i) {*v=out;}}
        rx
    }
    fn read_reg(&mut self,a:u8)->u8 {match a {
        0=>{let p=self.lora[0xd];self.lora[0xd]=p.wrapping_add(1);self.fifo[usize::from(p)]},
        0xd..=0x3f=>if self.is_lora(){self.lora[usize::from(a)]}else {self.fsk[usize::from(a)]},
        _=>self.common[usize::from(a)],
    }}
    fn write_reg(&mut self,a:u8,v:u8) {match a {
        0=>{let p=self.lora[0xd];self.lora[0xd]=p.wrapping_add(1);self.fifo[usize::from(p)]=v;},
        1=>{let old=self.mode();self.common[1]=v;if old!=self.mode() {self.event=None;if self.is_lora() {match self.mode() {
            3=>if let Some(config)=self.config() {let len=self.lora[0x22];let payload=(0..len).map(|i|self.fifo[usize::from(self.lora[0xe].wrapping_add(i))]).collect();let at=self.now.saturating_add(self.airtime_cycles(usize::from(len)));self.event=Some((at,Event::Tx(LoRaPacket{config,payload,cycle:at})));}else{self.rejected+=1;},
            6=>{let symbols=(u64::from(self.lora[0x1e]&3)<<8)|u64::from(self.lora[0x1f]);if let Some(period)=self.symbol_cycles(){self.event=Some((self.now.saturating_add(symbols*period),Event::Timeout));}},
            _=>{},
        }}else if self.mode()==3 {self.rejected+=1;}}},
        0x12 if self.is_lora()=>self.lora[0x12]&=!v,
        0x10|0x13..=0x1c if self.is_lora()=>{},
        0xd..=0x3f=>if self.is_lora(){self.lora[usize::from(a)]=v;}else {self.fsk[usize::from(a)]=if a==0x3b {v&!0x60}else{v};},
        0x42=>{},
        _=>self.common[usize::from(a)]=v,
    }}
    fn symbol_cycles(&self)->Option<u64> {let sf=self.lora[0x1e]>>4;let bw=*[(125000u64,16u64),(125000,12),(125000,8),(125000,6),(125000,4),(125000,3),(62500,1),(125000,1),(250000,1),(500000,1)].get(usize::from(self.lora[0x1d]>>4))?;if !(6..=12).contains(&sf){return None;}Some(((1u64<<sf)*CPU_HZ*bw.1).div_ceil(bw.0))}
    fn airtime_cycles(&self,len:usize)->u64 {let sf=self.lora[0x1e]>>4;let low=if self.lora[0x26]&8!=0 {2}else{0};let bits=(8*len.min(255) as i64-4*i64::from(sf)+28+if self.lora[0x1e]&4!=0 {16}else{0}-if self.lora[0x1d]&1!=0 {20}else{0}).max(0) as u64;let payload=8+bits.div_ceil(4*u64::from(sf-low))*u64::from(((self.lora[0x1d]>>1)&7)+4);let preamble=u64::from(u16::from_be_bytes([self.lora[0x20],self.lora[0x21]]));(self.symbol_cycles().unwrap_or(1)*((preamble+payload)*4+17)).div_ceil(4)}
    pub fn config(&self)->Option<LoRaConfig> {
        if !self.is_lora() {return None;}self.symbol_cycles()?;let cr=(self.lora[0x1d]>>1)&7;if !(1..=4).contains(&cr){return None;}
        // Common peer ABI uses SX126x frequency/BW/sync encodings. Convert the
        // SX1276's 2^19 frequency word and 8-bit sync symbol, preserving Hz.
        let frequency=((u32::from(self.common[6])<<16)|(u32::from(self.common[7])<<8)|u32::from(self.common[8]))<<6;
        let sw=u16::from(self.lora[0x39]);Some(LoRaConfig{frequency,sf:self.lora[0x1e]>>4,bandwidth:[0,8,1,9,2,10,3,4,5,6][usize::from(self.lora[0x1d]>>4)],coding_rate:cr,sync_word:((sw&0xf0)<<8)|0x0404|((sw&15)<<4),implicit_header:self.lora[0x1d]&1!=0,crc:self.lora[0x1e]&4!=0,invert_iq:self.lora[0x33]&0x40!=0})
    }
    pub fn next_deadline(&self)->Option<u64> {self.event.as_ref().map(|e|e.0)}
    pub fn advance_to(&mut self,now:u64) {
        self.now=now;if !self.event.as_ref().is_some_and(|e|e.0<=now){return;}let (_,event)=self.event.take().unwrap();match event {
            Event::Tx(p)=>{self.lora[0x12]|=8;self.common[1]=(self.common[1]&!7)|1;self.tx_count+=1;if self.packets.len()==16 {self.packets.pop_front();}self.packets.push_back(p);},
            Event::Rx(data,rssi,snr,crc)=>{let start=self.lora[0xf];for (i,b) in data.iter().enumerate(){self.fifo[(usize::from(start)+i)&255]= *b;}self.lora[0x10]=start;self.lora[0x13]=data.len() as u8;self.lora[0x19]=snr as u8;self.lora[0x1a]=(rssi+157).clamp(0,255) as u8;self.lora[0x12]|=0x50|if crc{0x20}else{0};self.rx_count+=1;if self.mode()==6 {self.common[1]=(self.common[1]&!7)|1;}},
            Event::Timeout=>{self.lora[0x12]|=0x80;self.common[1]=(self.common[1]&!7)|1;},
        }
    }
    pub fn irq(&self)->[bool;3] {if !self.powered||!self.reset_high||!self.is_lora(){return [false;3];}let flags=self.lora[0x12]&!self.lora[0x11];let map=self.common[0x40];[match map>>6 {0=>flags&0x40!=0,1=>flags&8!=0,2=>flags&4!=0,_=>false},match (map>>4)&3 {0=>flags&0x80!=0,1=>flags&2!=0,_=>false},false]}
    pub fn receive(&mut self,p:&LoRaInput)->bool {if !self.powered||!self.reset_high||!matches!(self.mode(),5|6)||self.config().as_ref()!=Some(&p.config)||p.payload.len()>255||self.event.as_ref().is_some_and(|e|matches!(e.1,Event::Rx(..)))||p.config.implicit_header&&usize::from(self.lora[0x22])!=p.payload.len(){return false;}let at=self.now.saturating_add(self.airtime_cycles(p.payload.len()));if self.event.as_ref().is_some_and(|e|e.0<at){return false;}self.event=Some((at,Event::Rx(p.payload.clone(),p.rssi_dbm,p.snr_quarter_db,p.crc_error)));true}
    pub fn snapshot(&self)->String {let config=self.config().map(|c|c.json()).unwrap_or_else(||"null".into());let tx=self.packets.iter().map(|p|format!("{{\"config\":{},\"payloadHex\":\"{}\",\"cycle\":{}}}",p.config.json(),p.payload.iter().map(|b|format!("{b:02x}")).collect::<String>(),p.cycle)).collect::<Vec<_>>().join(",");format!("{{\"model\":\"sx1276-lora-digital\",\"physicalRf\":false,\"powered\":{},\"resetHigh\":{},\"mode\":{},\"irq\":{},\"config\":{config},\"transmitted\":[{tx}],\"txCount\":{},\"rxCount\":{},\"rejectedCommands\":{}}}",self.powered,self.reset_high,self.mode(),self.lora[0x12],self.tx_count,self.rx_count,self.rejected)}
}
#[cfg(test)] mod tests {
    use super::*;
    fn command(s:&mut Sx1276,tx:&[u8])->Vec<u8> {s.select(true);let r=s.transfer(tx,tx.len());s.select(false);r}
    fn write(s:&mut Sx1276,a:u8,v:u8) {command(s,&[a|0x80,v]);}
    fn configured()->Sx1276 {let mut s=Sx1276::new();s.controls(true,true);for (a,v) in [(1,0x80),(6,0xd9),(7,0),(8,0),(0x1d,0x74),(0x1e,0xa0),(0x20,0),(0x21,15),(0x39,0x12),(1,0x81)] {write(&mut s,a,v);}s}
    fn peer(s:&Sx1276)->LoRaInput {LoRaInput{config:s.config().unwrap(),payload:b"PEER".to_vec(),rssi_dbm:-70,snr_quarter_db:24,crc_error:false}}
    #[test] fn spi_selection_and_split_transfers_obey_reset_power_and_fifo_wrap() {
        let mut s=configured();assert_eq!(s.transfer(&[0x42,0],2),[255,255]);assert_eq!(command(&mut s,&[0x42,0])[1],0x12);
        s.select(true);s.transfer(&[0x8d],0);s.transfer(&[255],0);s.select(false);command(&mut s,&[0x80,10,20]);write(&mut s,0xd,255);assert_eq!(&command(&mut s,&[0,0,0])[1..],[10,20]);
        s.controls(false,true);assert_eq!(command(&mut s,&[0x42,0]),[255,255]);s.controls(true,false);assert_eq!(command(&mut s,&[0x42,0]),[255,255]);s.controls(true,true);assert_eq!(command(&mut s,&[0x42,0])[1],0x12);
    }
    #[test] fn tx_finishes_after_airtime_maps_dio_and_never_echoes() {
        let mut s=configured();write(&mut s,0x40,0x40);write(&mut s,0xe,252);write(&mut s,0xd,252);command(&mut s,&[0x80,1,2,3,4]);write(&mut s,0x22,4);write(&mut s,1,0x83);let at=s.next_deadline().unwrap();assert!(at>CPU_HZ/10);s.advance_to(at-1);assert_eq!(s.tx_count,0);assert!(!s.irq()[0]);s.advance_to(at);assert_eq!(s.tx_count,1);assert_eq!(s.rx_count,0);assert!(s.irq()[0]);assert_eq!(s.packets[0].payload,[1,2,3,4]);assert_eq!(s.packets[0].config.frequency,0x36400000);assert_eq!(s.packets[0].config.sync_word,0x1424);write(&mut s,0x11,8);assert!(!s.irq()[0]);write(&mut s,0x11,0);assert!(s.irq()[0]);write(&mut s,0x12,8);assert!(!s.irq()[0]);
    }
    #[test] fn receive_requires_matching_peer_then_latches_fifo_irq_and_crc() {
        let mut s=configured();let mut p=peer(&s);assert!(!s.receive(&p));write(&mut s,1,0x85);p.config.frequency+=64;assert!(!s.receive(&p));p.config.frequency-=64;p.crc_error=true;assert!(s.receive(&p));assert!(!s.receive(&p));let at=s.next_deadline().unwrap();s.advance_to(at-1);assert!(!s.irq()[0]);s.advance_to(at);assert_eq!(s.rx_count,1);assert!(s.irq()[0]);assert_eq!(s.lora[0x12]&0x60,0x60);let start=s.lora[0x10];write(&mut s,0xd,start);assert_eq!(&command(&mut s,&[0,0,0,0,0])[1..],b"PEER");write(&mut s,0x12,0xff);assert!(!s.irq()[0]);assert_eq!(s.mode(),5);
    }
    #[test] fn power_loss_and_standby_cancel_pending_packets_and_invalid_modulation_is_safe() {
        let mut s=configured();write(&mut s,1,0x83);let at=s.next_deadline().unwrap();s.controls(false,true);s.advance_to(at);assert_eq!(s.tx_count,0);assert!(s.next_deadline().is_none());s=configured();write(&mut s,1,0x85);assert!(s.receive(&peer(&s)));write(&mut s,1,0x81);s.advance_to(CPU_HZ);assert_eq!(s.rx_count,0);for v in 0..=255 {write(&mut s,0x1d,v);write(&mut s,0x1e,v);write(&mut s,1,0x81);write(&mut s,1,0x83);s.advance_to(s.now+CPU_HZ);}assert!(s.rejected>0);
    }
}
