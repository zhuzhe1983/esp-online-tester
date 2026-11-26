//! Synopsys GMAC enhanced descriptors, shared by classic ESP32 and ESP32-P4.
//! Register fields: ESP-IDF v5.4.2 soc/{esp32,esp32p4}/register/soc/
//! emac_{mac,dma}_struct.h; descriptors: hal/include/hal/emac_hal.h.
//! Digital RMII endpoint only: no analog PHY, collision, PTP or checksum offload.
use crate::{Device, WriteEffect};
use std::collections::VecDeque;

pub const OWN: u32 = 1 << 31;
pub const TI: u32 = 1;
pub const RI: u32 = 1 << 6;
pub const RU: u32 = 1 << 7;
pub const FBE: u32 = 1 << 13;
pub const AIS: u32 = 1 << 15;
pub const NIS: u32 = 1 << 16;
/// Byte accesses must reject non-DMA memory and never invoke MMIO side effects.
pub trait Memory {
    fn read(&mut self, address: u32, bytes: usize) -> Option<Vec<u8>>;
    fn write(&mut self, address: u32, bytes: &[u8]) -> bool;
}
#[derive(Default)]
pub struct Emac { state: Option<Box<State>> }
struct State {
    mac: [u32; 64], dma: [u32; 32], ext: [u32; 8],
    phy: [u16; 32], phy_addr: u8, phy_id: u32, ip101_page1_control: u16,
    clock: bool, mdio: bool, rmii: bool, reset: bool,
    mdio_due: Option<u64>, now_ns: u64,
    tx_desc: u32, rx_desc: u32,
    rx: VecDeque<Vec<u8>>, tx: Vec<Vec<u8>>,
}
impl State {
    fn new(phy_addr: u8, phy_id: u32) -> Self {
        let mut s = Self { mac:[0;64],dma:[0;32],ext:[0;8],phy:[0;32],phy_addr,phy_id,ip101_page1_control:0,
            clock:false,mdio:false,rmii:false,reset:false,mdio_due:None,now_ns:0,
            tx_desc:0,rx_desc:0,rx:VecDeque::new(),tx:Vec::new() };
        s.phy_reset(); s
    }
    fn phy_reset(&mut self) {
        self.phy=[0;32]; self.phy[0]=0x3100; self.phy[1]=0x7809;
        self.phy[2]=(self.phy_id>>16) as u16; self.phy[3]=self.phy_id as u16;
        self.phy[4]=0x01e1; self.phy[5]=0x01e1;
        self.phy[20]=16;self.ip101_page1_control=0; // IP101G PCR reset page. Vendor status is computed from the physical link.
    }
    fn link(&self)->bool {self.clock&&self.rmii&&!self.reset&&self.phy[0]&0x0800==0}
    fn phy_read(&self,reg:usize)->u16 {
        if reg==1 {return self.phy[1]|if self.link(){0x24}else{0};}
        // IP101G: source esp_eth_phy_ip101.c v5.4.2 CSSR page16,
        // reg30 bits2:0=6 (100/full), bit8 link. Page1 reg17 is PSCR,
        // while page16 reg17 is ISR; never alias the two registers.
        if self.phy_id>>4==0x0024_30c5 {
            match (self.phy[20]&31,reg) {
                (16,30)=>return if self.link(){0x106}else{0},
                (1,17)=>return self.ip101_page1_control,
                (_,17|30)=>return 0,
                _=>{}
            }
        }
        self.phy[reg]
    }
    fn interrupt(&mut self,bits:u32) {
        self.dma[5]|=bits;
        if bits&(TI|RI)!=0 && bits&self.dma[7]!=0 {self.dma[5]|=NIS;}
        if bits&!(TI|RI|AIS|NIS)!=0 && bits&self.dma[7]!=0 {self.dma[5]|=AIS;}
    }
    fn next(&self,d:&[u32;4],a:u32,tx:bool)->Option<u32> {
        let chain=if tx {d[0]&(1<<20)!=0} else {d[1]&(1<<14)!=0};
        let end=if tx {d[0]&(1<<21)!=0} else {d[1]&(1<<15)!=0};
        if chain {Some(d[3])} else if end {Some(self.dma[if tx {4}else{3}])}
        else {a.checked_add(if self.dma[0]&(1<<7)!=0 {32}else{16})?.checked_add(((self.dma[0]>>2)&31)*4)}
    }
    fn descriptor(memory:&mut dyn Memory,a:u32)->Option<[u32;4]> {
        if a&3!=0 {return None;} let b=memory.read(a,16)?;
        Some(std::array::from_fn(|i|u32::from_le_bytes(b[i*4..i*4+4].try_into().unwrap())))
    }
    fn status(memory:&mut dyn Memory,a:u32,v:u32)->bool {memory.write(a,&v.to_le_bytes())}
    fn tx_step(&mut self,m:&mut dyn Memory) {
        if self.mac[0]&(1<<3)==0||self.dma[6]&(1<<13)==0{return;}
        let mut frame=Vec::new(); let mut visited=Vec::new(); let mut pending=Vec::new();
        let mut a=if self.tx_desc==0 {self.dma[4]}else{self.tx_desc};
        for _ in 0..256 {
            if a==0||visited.contains(&a){break;} visited.push(a);
            let Some(d)=Self::descriptor(m,a) else {self.interrupt(FBE);return;};
            if d[0]&OWN==0 {break;}
            if d[0]&(1<<28)!=0 {frame.clear();pending.clear();}
            if frame.is_empty()&&d[0]&(1<<28)==0 {self.interrupt(1<<5);return;}
            for (buf,len) in [(d[2],d[1]&0x1fff),(d[3],if d[0]&(1<<20)==0 {(d[1]>>16)&0x1fff}else{0})] {
                if len==0 {continue;}
                if frame.len()+len as usize>1518 {self.interrupt(1<<5);return;}
                let Some(bytes)=m.read(buf,len as usize) else {self.interrupt(FBE);return;};frame.extend(bytes);
            }
            let Some(next)=self.next(&d,a,true) else {self.interrupt(FBE);return;};
            pending.push((a,d[0]));
            if d[0]&(1<<29)!=0 {
                if frame.len()<14 {self.interrupt(1<<5);return;}
                // Checksum insertion is deliberately unsupported; never report a valid offload.
                if pending.iter().any(|(_,s)|s&(3<<22)!=0) {self.interrupt(1<<5);return;}
                if self.tx.len()>=64 {self.interrupt(1<<5);return;}
                for &(at,status) in &pending {if !Self::status(m,at,status&!OWN){self.interrupt(FBE);return;}}
                self.tx.push(std::mem::take(&mut frame));pending.clear();
                if d[0]&(1<<30)!=0 {self.interrupt(TI);}
                self.tx_desc=next;
            }
            a=next;
        }
    }
    fn rx_step(&mut self,m:&mut dyn Memory) {
        if self.mac[0]&(1<<2)==0||self.dma[6]&2==0{return;}
        for _ in 0..16 {
            let Some(frame)=self.rx.front() else {break;};
            let mut data=frame.clone(); let crc=crc32(&data);data.extend(crc.to_le_bytes());
            let mut a=if self.rx_desc==0 {self.dma[3]}else{self.rx_desc};
            let mut parts=Vec::new();let mut offset=0;let mut visited=Vec::new();
            while offset<data.len()&&parts.len()<256 {
                if a==0||visited.contains(&a){self.interrupt(RU);return;}visited.push(a);
                let Some(d)=Self::descriptor(m,a) else {self.interrupt(FBE);return;};
                if d[0]&OWN==0 {self.interrupt(RU);return;}
                let mut buffers=Vec::new();
                for (buf,len) in [(d[2],d[1]&0x1fff),(d[3],if d[1]&(1<<14)==0 {(d[1]>>16)&0x1fff}else{0})] {
                    let n=(len as usize).min(data.len()-offset);
                    if n!=0 {if m.read(buf,n).is_none(){self.interrupt(FBE);return;}buffers.push((buf,offset,n));offset+=n;}
                }
                if buffers.is_empty(){self.interrupt(RU);return;}
                let Some(next)=self.next(&d,a,false) else {self.interrupt(FBE);return;};
                parts.push((a,d,buffers));a=next;
            }
            if offset!=data.len(){self.interrupt(RU);return;}
            let last=parts.len()-1;
            for (i,(at,d,buffers)) in parts.iter().enumerate() {
                for &(buf,start,n) in buffers {if !m.write(buf,&data[start..start+n]){self.interrupt(FBE);return;}}
                let status=(if i==0 {1<<9}else{0})|(if i==last {(1<<8)|((data.len() as u32)<<16)}else{0});
                if !Self::status(m,*at,status){self.interrupt(FBE);return;}
                if i==last&&d[1]&(1<<31)==0 {self.interrupt(RI);}
            }
            self.rx_desc=a;self.rx.pop_front();
        }
    }
}
impl Emac {
    pub fn configure(&mut self,phy_addr:u8,phy_id:u32){self.state=Some(Box::new(State::new(phy_addr&31,phy_id)));}
    pub fn attached(&self)->bool {self.state.is_some()}
    pub fn clocked(&self)->bool {self.state.as_ref().is_some_and(|s|s.clock)}
    pub fn set_routes(&mut self,clock:bool,mdio:bool,rmii:bool,reset:bool) {
        if let Some(s)=self.state.as_mut(){
            if reset&&!s.reset {s.phy_reset();s.rx.clear();s.tx.clear();}
            s.clock=clock;s.mdio=mdio;s.rmii=rmii;s.reset=reset;
        }
    }
    pub fn reset(&mut self){if let Some(s)=self.state.as_mut(){let (a,id)=(s.phy_addr,s.phy_id);let phy=s.phy;let page1=s.ip101_page1_control;**s=State::new(a,id);s.phy=phy;s.ip101_page1_control=page1;}}
    pub fn poll(&mut self,now_ns:u64) {
        let Some(s)=self.state.as_mut() else{return;};s.now_ns=now_ns;
        if !s.clock {return;}
        if s.dma[0]&1!=0 && s.rmii {s.mac=[0;64];s.dma=[0;32];s.tx_desc=0;s.rx_desc=0;s.rx.clear();s.tx.clear();s.mdio_due=None;}
        if s.mdio_due.is_some_and(|due|due<=now_ns) {
            let request=s.mac[4];let reg=((request>>6)&31) as usize;let addr=((request>>11)&31) as u8;
            if s.mdio&&!s.reset&&addr==s.phy_addr {
                if request&2!=0 {
                    let value=s.mac[5] as u16;
                    match reg {0=>{if value&0x8000!=0{s.phy_reset();}else{s.phy[0]=value&!0x0200;}},1..=3=>{},17 if s.phy_id>>4==0x0024_30c5&&s.phy[20]&31==1=>s.ip101_page1_control=value,20=>s.phy[20]=value&31,_=>s.phy[reg]=value}
                }else{s.mac[5]=u32::from(s.phy_read(reg));}
            } else if request&2==0 {s.mac[5]=0xffff;}
            s.mac[4]&=!1;s.mdio_due=None;
        }
    }
    pub fn receive(&mut self,frame:&[u8])->Result<(),String> {
        let Some(s)=self.state.as_mut() else{return Err("no physical Ethernet MAC attached".into());};
        if !(14..=1518).contains(&frame.len()){return Err("Ethernet frame must be 14..1518 bytes without FCS".into());}
        if !s.link(){return Err("Ethernet RMII link is inactive".into());}
        if s.rx.len()>=64{return Err("Ethernet receive queue full".into());}
        // MAC address filtering. Broadcast/multicast and promiscuous modes follow GMACFF.
        let mut mac=[0u8;6];mac[..4].copy_from_slice(&s.mac[17].to_le_bytes());mac[4..].copy_from_slice(&s.mac[16].to_le_bytes()[..2]);
        if s.mac[1]&((1<<31)|1)==0&&frame[..6]!=[255;6]&&frame[..6]!=mac&&!(frame[0]&1!=0&&s.mac[1]&(1<<4)!=0){return Ok(());}
        s.rx.push_back(frame.to_vec());Ok(())
    }
    pub fn take_frames(&mut self)->Vec<Vec<u8>> {self.state.as_mut().map_or_else(Vec::new,|s|std::mem::take(&mut s.tx))}
    pub fn step(&mut self,m:&mut dyn Memory) {if let Some(s)=self.state.as_mut(){if s.link(){s.tx_step(m);s.rx_step(m);}}}
}
impl Device for Emac {
    fn read(&mut self,off:u32)->u32 {
        let Some(s)=self.state.as_ref() else{return 0;};let n=((off&0xfff)>>2) as usize;
        match off>>12 {0=>s.mac.get(n).copied().unwrap_or(0),1=>s.dma.get(n).copied().unwrap_or(0),2=>s.ext.get(n).copied().unwrap_or(0),_=>0}
    }
    fn write(&mut self,off:u32,v:u32)->WriteEffect {
        let Some(s)=self.state.as_mut() else{return WriteEffect::NONE;};let n=((off&0xfff)>>2) as usize;
        match off>>12 {
            0=>{if let Some(reg)=s.mac.get_mut(n){*reg=v;if n==4&&v&1!=0{s.mdio_due=Some(s.now_ns.saturating_add(32_000));}}},
            1=>match n {5=>s.dma[5]&=!v,1|2=>{},3=>{s.dma[3]=v;s.rx_desc=v;},4=>{s.dma[4]=v;s.tx_desc=v;},6=>s.dma[6]=v&!(1<<20),_=>if let Some(reg)=s.dma.get_mut(n){*reg=v;}},
            2=>if let Some(reg)=s.ext.get_mut(n){*reg=v;},_=>{}
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self)->u64 {u64::from(self.state.as_ref().is_some_and(|s|s.clock&&s.dma[5]&s.dma[7]&(AIS|NIS)!=0))}
}
fn crc32(data:&[u8])->u32 {let mut crc=!0u32;for &b in data{crc^=u32::from(b);for _ in 0..8{crc=(crc>>1)^if crc&1!=0{0xedb88320}else{0};}}!crc}

#[cfg(test)] mod tests {
    use super::*;
    struct Ram(Vec<u8>);
    impl Memory for Ram {
        fn read(&mut self,a:u32,n:usize)->Option<Vec<u8>> {Some(self.0.get(a as usize..(a as usize).checked_add(n)?)?.to_vec())}
        fn write(&mut self,a:u32,b:&[u8])->bool {let Some(end)=(a as usize).checked_add(b.len())else{return false;};let Some(dst)=self.0.get_mut(a as usize..end)else{return false;};dst.copy_from_slice(b);true}
    }
    fn write_words(r:&mut Ram,a:u32,w:&[u32]){for(i,&v)in w.iter().enumerate(){assert!(r.write(a+4*i as u32,&v.to_le_bytes()));}}
    fn word(r:&mut Ram,a:u32)->u32 {u32::from_le_bytes(r.read(a,4).unwrap().try_into().unwrap())}
    fn mac()->Emac {let mut e=Emac::default();e.configure(1,0x02430c54);e.set_routes(true,true,true,false);e}
    fn mdio(e:&mut Emac,reg:u32,addr:u32){e.write(0x10,1|(reg<<6)|(addr<<11));let now=e.state.as_ref().unwrap().now_ns;e.poll(now+32_000);}
    fn frame()->Vec<u8>{let mut f=vec![0u8;42];f[..6].fill(255);f[6..12].copy_from_slice(&[2,3,4,5,6,7]);f[12..14].copy_from_slice(&[8,6]);f}
    #[test] fn phy_identity_mdio_busy_routes_reset_and_power() {
        let mut e=mac();e.write(0x10,1|(2<<6)|(1<<11));e.poll(31_999);assert_eq!(e.read(0x10)&1,1);e.poll(32_000);assert_eq!(e.read(0x14),0x0243);
        mdio(&mut e,2,2);assert_eq!(e.read(0x14),0xffff);
        e.set_routes(true,false,true,false);mdio(&mut e,2,1);assert_eq!(e.read(0x14),0xffff);
        e.set_routes(true,true,true,true);mdio(&mut e,2,1);assert_eq!(e.read(0x14),0xffff);
        e.set_routes(true,true,true,false);mdio(&mut e,1,1);assert_ne!(e.read(0x14)&4,0);
        mdio(&mut e,30,1);assert_eq!(e.read(0x14),0x106);mdio(&mut e,17,1);assert_eq!(e.read(0x14),0);
        e.write(0x14,0x0800);e.write(0x10,3|(1<<11));e.poll(2_000_000);mdio(&mut e,1,1);e.poll(3_000_000);assert_eq!(e.read(0x14)&4,0);
        e.set_routes(false,true,true,false);e.write(0x10,1|(2<<6)|(1<<11));e.poll(4_000_000);assert_ne!(e.read(0x10)&1,0);
    }
    #[test] fn enhanced_dma_transmits_and_receives_real_bytes_and_fcs() {
        let mut e=mac();let mut r=Ram(vec![0;0x4000]);let f=frame();r.write(0x1000,&f);
        write_words(&mut r,0x100,&[OWN|(1<<28)|(1<<29)|(1<<30)|(1<<20),42,0x1000,0x100]);
        e.write(0x1000,1<<7);e.write(0x1010,0x100);e.write(0x100c,0x200);e.write(0x1018,(1<<13)|2);e.write(0,12);e.write(0x101c,NIS|TI|RI);
        e.step(&mut r);assert_eq!(e.take_frames(),vec![f.clone()]);assert_eq!(word(&mut r,0x100)&OWN,0);assert_eq!(e.irq_sources(),1);
        e.write(0x1014,u32::MAX);assert_eq!(e.irq_sources(),0);
        write_words(&mut r,0x200,&[OWN,(1<<14)|1536,0x2000,0x200]);e.receive(&f).unwrap();e.step(&mut r);
        assert_eq!(r.read(0x2000,42).unwrap(),f);assert_eq!(word(&mut r,0x202a),crc32(&f));
        assert_eq!(word(&mut r,0x200),46<<16|(1<<9)|(1<<8));assert_eq!(e.irq_sources(),1);
    }
    #[test] fn missing_owner_wrong_clock_invalid_bus_and_w1c_do_not_deliver() {
        let mut e=mac();let mut r=Ram(vec![0;0x4000]);let f=frame();r.write(0x1000,&f);
        write_words(&mut r,0x100,&[(1<<28)|(1<<29)|(1<<20),42,0x1000,0x100]);e.write(0x1010,0x100);e.write(0x1018,1<<13);e.write(0,8);
        e.step(&mut r);assert!(e.take_frames().is_empty());
        write_words(&mut r,0x100,&[OWN|(1<<28)|(1<<29)|(1<<20),42,0x1000,0x100]);e.set_routes(false,true,true,false);e.step(&mut r);assert!(e.take_frames().is_empty());assert_ne!(word(&mut r,0x100)&OWN,0);
        e.set_routes(true,true,true,false);write_words(&mut r,0x100,&[OWN|(1<<28)|(1<<29)|(1<<20),42,0xffff_fff0,0x100]);e.write(0x101c,AIS|FBE);e.step(&mut r);assert_eq!(e.irq_sources(),1);assert!(e.take_frames().is_empty());assert_ne!(word(&mut r,0x100)&OWN,0);
        e.write(0x1014,FBE|AIS);assert_eq!(e.irq_sources(),0);
    }
    #[test] fn chained_frame_waits_for_all_owned_segments_and_avoids_cycles() {
        let mut e=mac();let mut r=Ram(vec![0;0x4000]);let f=frame();r.write(0x1000,&f);
        write_words(&mut r,0x100,&[OWN|(1<<28)|(1<<20),20,0x1000,0x120]);write_words(&mut r,0x120,&[(1<<29)|(1<<20),22,0x1014,0x100]);
        e.write(0x1010,0x100);e.write(0x1018,1<<13);e.write(0,8);e.step(&mut r);assert!(e.take_frames().is_empty());assert_ne!(word(&mut r,0x100)&OWN,0);
        write_words(&mut r,0x120,&[OWN|(1<<29)|(1<<20),22,0x1014,0x100]);e.step(&mut r);assert_eq!(e.take_frames(),vec![f]);assert_eq!(word(&mut r,0x100)&OWN,0);assert_eq!(word(&mut r,0x120)&OWN,0);
    }
}
