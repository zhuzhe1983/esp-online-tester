//! Bounded raw Ethernet endpoint for wired controllers, independent of Wi-Fi association.
//! The default peer is VirtualNet; relay mode uses the existing host Ethernet ABI.
use std::collections::VecDeque;
pub struct EthernetLink {
    net: crate::net::VirtualNet,
    relay: bool,
    tx: Vec<Vec<u8>>,
    rx: VecDeque<Vec<u8>>,
    last_poll: u64,
    tap: Option<crate::netcap::NetTap>,
}
impl Default for EthernetLink {fn default()->Self{Self::new()}}
impl EthernetLink {
    pub fn new()->Self {Self{net:crate::net::VirtualNet::new(false),relay:false,tx:Vec::new(),rx:VecDeque::new(),last_poll:0,tap:None}}
    pub fn set_tap(&mut self,tap:crate::netcap::NetTap){self.tap=Some(tap);}
    pub fn configure(&mut self,nat:Option<crate::nat::Nat>,epoch_s:Option<u64>){self.net.nat=nat;self.net.epoch_us=epoch_s.map(|s|s.saturating_mul(1_000_000));}
    pub fn set_relay(&mut self,enabled:bool){if self.relay!=enabled{self.tx.clear();self.rx.clear();self.relay=enabled;}}
    pub fn transmit(&mut self,frame:&[u8],now_us:u64)->Result<(),String>{
        if !(14..=1518).contains(&frame.len()){return Err("Ethernet frame must be 14..1518 bytes without FCS".into());}
        if let Some(tap)=&self.tap {if let Ok(mut tap)=tap.lock(){tap.record(crate::netcap::Dir::FromStation,now_us,frame);}}
        if self.relay {if self.tx.len()>=64{return Err("Ethernet transmit queue full".into());}self.tx.push(frame.to_vec());}
        else {for reply in self.net.handle(frame,now_us){if self.rx.len()<64{self.rx.push_back(reply);}}}
        Ok(())
    }
    pub fn poll(&mut self,now_us:u64){if !self.relay&&now_us.wrapping_sub(self.last_poll)>=500{self.last_poll=now_us;for frame in self.net.poll(now_us){if self.rx.len()<64{self.rx.push_back(frame);}}}}
    pub fn next_frame(&mut self)->Option<Vec<u8>>{self.rx.pop_front()}
    pub fn receive(&mut self,frame:&[u8])->Result<(),String>{
        if !self.relay{return Err("enable Ethernet relay before injecting host frames".into());}
        if !(14..=1518).contains(&frame.len()){return Err("Ethernet frame must be 14..1518 bytes without FCS".into());}
        if self.rx.len()>=64{return Err("Ethernet receive queue full".into());}self.rx.push_back(frame.to_vec());Ok(())
    }
    pub fn take_frames(&mut self)->Vec<Vec<u8>>{if self.relay{std::mem::take(&mut self.tx)}else{Vec::new()}}
    /// Guest hardware reset discards queues and old socket flows, preserving host relay selection.
    pub fn reset(&mut self){self.tx.clear();self.rx.clear();self.last_poll=0;if let Some(nat)=self.net.nat.as_mut(){nat.abort_all();}}
}
