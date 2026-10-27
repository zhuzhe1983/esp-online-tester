//! SX1262 digital LoRa packet subset, not an RF propagation model.
//! Semtech DS.SX1261-2.W.APP Rev 2.2 (Dec 2024), tables 8-2 and 12-1,
//! sections 13.1–13.6; RadioLib b91c6af1 SX126x.cpp for version discovery and
//! the integer LoRa time-on-air equation (DS section 6.1.4 refers to drivers).
use std::collections::VecDeque;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct LoRaConfig {
    /// SX1262 RF frequency word: frequency Hz = word * 32 MHz / 2^25.
    pub frequency:u32,pub sf:u8,pub bandwidth:u8,pub coding_rate:u8,pub sync_word:u16,
    pub implicit_header:bool,pub crc:bool,pub invert_iq:bool,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct LoRaPacket {pub config:LoRaConfig,pub payload:Vec<u8>,pub cycle:u64}
impl LoRaConfig {
    pub fn json(&self)->String {format!("{{\"frequency\":{},\"sf\":{},\"bandwidth\":{},\"codingRate\":{},\"syncWord\":{},\"implicitHeader\":{},\"crc\":{},\"invertIq\":{}}}",self.frequency,self.sf,self.bandwidth,self.coding_rate,self.sync_word,self.implicit_header,self.crc,self.invert_iq)}
}
pub struct LoRaInput {pub config:LoRaConfig,pub payload:Vec<u8>,pub rssi_dbm:i16,pub snr_quarter_db:i8,pub crc_error:bool}
impl LoRaInput {
    pub fn parse(json:&str)->Option<Self> {
        use crate::json::{parse_json,Json};
        let obj=parse_json(json).ok()?;let config=obj.get("config")?;
        let number=|j:&Json,key|match j.get(key)? {Json::Int(n)=>Some(*n),_=>None};
        let boolean=|j:&Json,key|match j.get(key)? {Json::Bool(b)=>Some(*b),_=>None};
        let hex=obj.get("payloadHex")?.as_str()?;if !hex.len().is_multiple_of(2)||hex.len()>510 {return None;}
        let nibble=|n:u8|match n {b'0'..=b'9'=>Some(n-b'0'),b'a'..=b'f'=>Some(n-b'a'+10),b'A'..=b'F'=>Some(n-b'A'+10),_=>None};
        let payload=hex.as_bytes().as_chunks::<2>().0.iter().map(|p|Some(nibble(p[0])?*16+nibble(p[1])?)).collect::<Option<Vec<_>>>()?;
        let config=LoRaConfig{frequency:number(config,"frequency")?.try_into().ok()?,sf:number(config,"sf")?.try_into().ok()?,bandwidth:number(config,"bandwidth")?.try_into().ok()?,coding_rate:number(config,"codingRate")?.try_into().ok()?,sync_word:number(config,"syncWord")?.try_into().ok()?,implicit_header:boolean(config,"implicitHeader")?,crc:boolean(config,"crc")?,invert_iq:boolean(config,"invertIq")?};
        if !(5..=12).contains(&config.sf)||bandwidth(config.bandwidth).is_none()||!(1..=4).contains(&config.coding_rate)||config.frequency==0 {return None;}
        let rssi_dbm=number(&obj,"rssiDbm")?.try_into().ok()?;if !(-127..=0).contains(&rssi_dbm) {return None;}
        Some(Self{config,payload,rssi_dbm,snr_quarter_db:number(&obj,"snrQuarterDb")?.try_into().ok()?,crc_error:boolean(&obj,"crcError")?})
    }
}
#[derive(Default)]
pub struct Stats {pub commands:u64,pub tx:u64,pub rx:u64,pub timeouts:u64,pub rejected:u64,pub dropped:u64,pub unsupported:[u64;4]}
enum Event {Tx(LoRaPacket),Rx(Vec<u8>,u8,i8,bool),Timeout}

pub struct Sx1262 {
    hz:u64,now:u64,powered:bool,reset_high:bool,antenna:bool,selected:bool,accept:bool,overflow:bool,
    transaction:Vec<u8>,registers:Box<[u8;4096]>,buffer:[u8;256],mode:u8,status:u8,
    busy_until:Option<u64>,event:Option<(u64,Event)>,sleep_warm:bool,
    packet_type:Option<u8>,frequency:u32,modulation:Option<[u8;4]>,packet_params:Option<[u8;6]>,
    tx_base:u8,rx_base:u8,rx_len:u8,rx_start:u8,rssi:u8,snr:i8,irq:u16,irq_mask:u16,dio1_mask:u16,
    fallback:u8,continuous_rx:bool,rx_deadline:Option<u64>,tcxo_ticks:u32,tx_ramp:u8,errors:u16,
    tx_packets:VecDeque<LoRaPacket>,pub stats:Stats,
}
impl Sx1262 {
    pub fn new(hz:u64)->Self {
        let mut s=Self {hz,now:0,powered:false,reset_high:false,antenna:false,selected:false,accept:false,overflow:false,transaction:Vec::new(),registers:Box::new([0;4096]),buffer:[0;256],mode:0x20,status:0,busy_until:None,event:None,sleep_warm:false,packet_type:None,frequency:0,modulation:None,packet_params:None,tx_base:0,rx_base:0,rx_len:0,rx_start:0,rssi:255,snr:0,irq:0,irq_mask:0,dio1_mask:0,fallback:0x20,continuous_rx:false,rx_deadline:None,tcxo_ticks:0,tx_ramp:0,errors:0,tx_packets:VecDeque::new(),stats:Stats::default()};s.reset();s
    }
    fn reset(&mut self) {
        self.registers.fill(0);
        // RadioLib b91c6af1 SX1262.h explicitly records that SX1262 reports
        // "SX1261" at 0x0320. The unpublicized ROM suffix is unknown, not an
        // invented silicon revision; findChip compares these six bytes only.
        self.registers[0x320..0x326].copy_from_slice(b"SX1261");
        for (a,v) in [(0x6b8,1),(0x6bc,0x1d),(0x6bd,0x0f),(0x6be,0x10),(0x6bf,0x21),(0x736,0x0d),(0x740,0x14),(0x741,0x24),(0x806,0x20),(0x88f,0x0c),(0x889,1),(0x8ac,0x94),(0x8d8,0xc8),(0x8e7,0x38),(0x911,5),(0x912,5),(0x920,1)] {self.registers[a]=v;}
        self.buffer.fill(0);self.transaction.clear();self.selected=false;self.accept=false;self.overflow=false;
        self.mode=0x20;self.status=0;self.busy_until=None;self.event=None;self.packet_type=None;self.frequency=0;self.modulation=None;self.packet_params=None;
        self.tx_base=0;self.rx_base=0;self.rx_len=0;self.rx_start=0;self.irq=0;self.irq_mask=0;self.dio1_mask=0;self.fallback=0x20;self.continuous_rx=false;self.rx_deadline=None;self.tcxo_ticks=0;self.tx_ramp=0;self.errors=0;
    }
    pub fn controls(&mut self,powered:bool,reset_high:bool,antenna:bool) {
        if (!powered && self.powered)||(!reset_high && self.reset_high) {self.reset();}
        if !antenna && self.event.as_ref().is_some_and(|e|matches!(e.1,Event::Rx(..))) {self.event=self.rx_deadline.map(|d|(d,Event::Timeout));}
        if powered && reset_high && (!self.powered || !self.reset_high) {self.busy_until=Some(self.after_us(3500));}
        self.powered=powered;self.reset_high=reset_high;self.antenna=antenna;
    }
    fn after_us(&self,us:u64)->u64 {self.now.saturating_add((u128::from(us)*u128::from(self.hz)).div_ceil(1_000_000).min(u128::from(u64::MAX)) as u64)}
    fn ticks_cycles(&self,ticks:u32)->u64 {(u128::from(ticks)*u128::from(self.hz)).div_ceil(64000).min(u128::from(u64::MAX)) as u64}
    pub fn busy(&self)->bool {self.powered && (!self.reset_high || self.mode==0 || self.busy_until.is_some_and(|d|d>self.now))}
    pub fn irq(&self)->bool {self.powered && self.reset_high && self.irq & self.irq_mask & self.dio1_mask!=0}
    pub fn next_deadline(&self)->Option<u64> {self.busy_until.into_iter().chain(self.event.as_ref().map(|e|e.0)).min()}
    fn raise(&mut self,bits:u16) {self.irq|=bits & self.irq_mask;}
    pub fn advance_to(&mut self,now:u64) {
        self.now=now;
        if self.busy_until.is_some_and(|d|d<=now) {self.busy_until=None;}
        if self.event.as_ref().is_some_and(|e|e.0<=now) {let (at,event)=self.event.take().unwrap();match event {
            Event::Tx(mut p)=>{p.cycle=at;self.stats.tx+=1;self.status=0x0c;self.mode=self.fallback;self.raise(1);if self.antenna {if self.tx_packets.len()==16 {self.tx_packets.pop_front();self.stats.dropped+=1;}self.tx_packets.push_back(p);}else {self.stats.dropped+=1;}}
            Event::Rx(data,rssi,snr,crc_error)=>{self.rx_len=data.len() as u8;self.rx_start=self.rx_base;for (i,b) in data.into_iter().enumerate(){self.buffer[(usize::from(self.rx_base)+i)&255]=b;}self.rssi=rssi;self.snr=snr;self.stats.rx+=1;self.status=4;self.raise(2|if crc_error {64}else{0});if !self.continuous_rx {self.mode=self.fallback;}self.rx_deadline=None;}
            Event::Timeout=>{self.rx_deadline=None;self.stats.timeouts+=1;self.status=0;self.mode=0x20;self.raise(512);}
        }}
    }
    pub fn select(&mut self,low:bool) {
        if low==self.selected {return;}
        if low {
            if self.powered && self.reset_high && self.mode==0 {let warm=self.sleep_warm;if !warm {self.reset();}self.mode=0x20;self.busy_until=Some(self.after_us(if warm {340}else{3500}));}
            self.accept=self.powered && self.reset_high && !self.busy();self.transaction.clear();self.overflow=false;
        } else if self.accept && !self.transaction.is_empty() {
            let command=std::mem::take(&mut self.transaction);self.execute(&command);self.transaction=command;self.transaction.clear();
        }
        self.selected=low;
    }
    pub fn transfer(&mut self,tx:&[u8],rx_len:usize)->Vec<u8> {
        let mut rx=vec![0xff;rx_len];if !self.selected || !self.accept {return rx;}
        for i in 0..tx.len().max(rx_len) {let b=tx.get(i).copied().unwrap_or(0);let out=self.shift(b);if let Some(r)=rx.get_mut(i){*r=out;}}
        rx
    }
    fn shift(&mut self,b:u8)->u8 {
        if self.transaction.len()>=260 {self.overflow=true;return self.mode|8;}
        let i=self.transaction.len();self.transaction.push(b);if i==0 {
            // A recognized new write enters command processing. A previous
            // invalid wake-up/NOP must not poison the status returned during a
            // subsequent valid command (RadioLib standby sends 00 then 80).
            // Full parameter validation still happens at NSS rise and is visible
            // to the following GetStatus; read commands preserve that result.
            if matches!(b,0x80|0xc1|0x84|0x0d|0x0e|0x08|0x02|0x86|0x8a|0x8b|0x8c|0x8f|0x93|0x8e|0x97|0x89|0x98|0x96|0x9d|0x9f|0x95|0x88|0xa0|0x07|0x00|0x82|0x83) {self.status=0;}
            return 0;
        }
        let c=&self.transaction;let stat=self.mode|self.status;
        match c[0] {
            0x1d if i>=4=>{let addr=(usize::from(u16::from_be_bytes([c[1],c[2]]))+i-4)&0xffff;self.registers.get(addr).copied().unwrap_or(0)},
            0x1e if i>=3=>self.buffer[(usize::from(c[1])+i-3)&255],
            0x12 if i==2=>(self.irq>>8) as u8,0x12 if i==3=>self.irq as u8,
            0x11 if i==2=>self.packet_type.unwrap_or(0),
            0x13 if i==2=>self.rx_len,0x13 if i==3=>self.rx_start,
            0x14 if i==2||i==4=>self.rssi,0x14 if i==3=>self.snr as u8,
            0x15 if i==2=>self.rssi,
            0x17 if i==2=>(self.errors>>8) as u8,0x17 if i==3=>self.errors as u8,
            0x10 if (2..8).contains(&i)=>{let counters=[self.stats.rx.min(65535) as u16,0,0];counters[(i-2)/2].to_be_bytes()[(i-2)%2]},
            _=>stat,
        }
    }
    fn reject(&mut self,opcode:u8) {self.status=8;self.stats.rejected+=1;self.stats.unsupported[usize::from(opcode)/64]|=1u64<<(opcode%64);}
    fn execute(&mut self,c:&[u8]) {
        let op=c[0];let p=&c[1..];self.stats.commands+=1;
        if self.overflow {self.reject(op);return;}
        // Read commands do not assert BUSY or erase a previous command error.
        match op {0xc0|0x1d|0x1e|0x10|0x11|0x12|0x13|0x14|0x15|0x17=>return,_=>{}}
        self.status=0;let mut busy_us=1; // ordinary write processing is a nominal digital delay, not silicon timing.
        match op {
            0x80 if p.len()==1 && p[0]<=1=>{self.mode=0x20+(p[0]<<4);self.event=None;self.rx_deadline=None;busy_us=if p[0]==1 {31}else{1};}
            0xc1 if p.is_empty()=>{self.mode=0x40;self.event=None;self.rx_deadline=None;busy_us=50;}
            0x84 if p.len()==1=>{self.event=None;self.rx_deadline=None;self.buffer.fill(0);self.mode=0;self.sleep_warm=p[0]&4!=0;busy_us=500;}
            0x0d if p.len()>=3=>{let a=usize::from(u16::from_be_bytes([p[0],p[1]]));for (i,&v) in p[2..].iter().enumerate(){let n=a+i;if !(0x320..0x330).contains(&n) {if let Some(r)=self.registers.get_mut(n){*r=v;}else{self.reject(op);break;}}}}
            0x0e if p.len()>=2=>{for (i,&v) in p[1..].iter().enumerate(){self.buffer[(usize::from(p[0])+i)&255]=v;}}
            0x08 if p.len()==8=>{self.irq_mask=u16::from_be_bytes([p[0],p[1]])&0x3ff;self.dio1_mask=u16::from_be_bytes([p[2],p[3]])&0x3ff;}
            0x02 if p.len()==2=>{self.irq&=!u16::from_be_bytes([p[0],p[1]]);}
            0x86 if p.len()==4=>self.frequency=u32::from_be_bytes(p.try_into().unwrap()),
            0x8a if p==[1]=>{self.packet_type=Some(1);self.modulation=None;self.packet_params=None;}
            0x8b if p.len()==4 && (5..=12).contains(&p[0]) && bandwidth(p[1]).is_some() && (1..=4).contains(&p[2]) && p[3]<=1=>self.modulation=Some(p.try_into().unwrap()),
            0x8c if p.len()==6 && p[2]<=1 && p[4]<=1 && p[5]<=1=>self.packet_params=Some(p.try_into().unwrap()),
            0x8f if p.len()==2=>{self.tx_base=p[0];self.rx_base=p[1];}
            0x93 if p.len()==1 && [0x20,0x30,0x40].contains(&p[0])=>self.fallback=p[0],
            0x8e if p.len()==2 && (-9..=22).contains(&(p[0] as i8)) && p[1]<=7=>self.tx_ramp=p[1],
            0x97 if p.len()==4 && p[0]<=7=>{self.tcxo_ticks=word24(&p[1..]);busy_us=(u64::from(self.tcxo_ticks)*1_000_000).div_ceil(64000);}
            0x89 if p.len()==1 && p[0]&0x80==0=>busy_us=3500,
            0x98 if p.len()==2=>busy_us=3500,
            0x96|0x9d|0x9f if p.len()==1 && p[0]<=1=>{},
            0x95 if p.len()==4 && p[0]<=4 && p[1]<=7 && p[2]==0 && p[3]==1=>{},
            0x88 if p.len()==7=>{}, // CAD parameters retained only by caller; CAD itself is unsupported.
            0xa0 if p.len()==1=>{}, // no preamble detector in the explicit packet-input subset.
            0x07 if p==[0,0]=>self.errors=0,
            0x00 if p==[0,0,0,0,0,0]=>self.stats.rx=0,
            0x82|0x83 if p.len()==3=>{
                let Some(config)=self.config() else {self.status=10;self.stats.rejected+=1;return;};
                let ticks=word24(p);self.event=None;self.rx_deadline=None;
                busy_us=if op==0x82 {83}else{126};busy_us+=(u64::from(self.tcxo_ticks)*1_000_000).div_ceil(64000);
                let start=self.after_us(busy_us);
                if op==0x82 {self.mode=0x50;self.continuous_rx=ticks==0xffffff;if ticks!=0 && !self.continuous_rx {let end=start.saturating_add(self.ticks_cycles(ticks));self.rx_deadline=Some(end);self.event=Some((end,Event::Timeout));}}
                else {self.mode=0x60;let len=usize::from(self.packet_params.unwrap()[3]);let payload=(0..len).map(|i|self.buffer[(usize::from(self.tx_base)+i)&255]).collect();
                    let ramp=[10,20,40,80,200,800,1700,3400][usize::from(self.tx_ramp)];
                    let done=self.after_us(busy_us+ramp+self.airtime_us(len));
                    let timeout=start.saturating_add(self.ticks_cycles(ticks));
                    self.event=Some(if ticks!=0 && timeout<done {(timeout,Event::Timeout)}else{(done,Event::Tx(LoRaPacket{config,payload,cycle:done}))});
                }
            }
            _=>{self.reject(op);return;}
        }
        self.busy_until=Some(self.after_us(busy_us));
    }
    pub fn config(&self)->Option<LoRaConfig> {
        if self.packet_type!=Some(1)||self.frequency==0 {return None;}
        let m=self.modulation?;let p=self.packet_params?;
        Some(LoRaConfig{frequency:self.frequency,sf:m[0],bandwidth:m[1],coding_rate:m[2],sync_word:u16::from_be_bytes([self.registers[0x740],self.registers[0x741]]),implicit_header:p[2]!=0,crc:p[4]!=0,invert_iq:p[5]!=0})
    }
    pub fn airtime_us(&self,len:usize)->u64 {
        let Some(m)=self.modulation else {return 0};let Some(p)=self.packet_params else{return 0};
        let (numerator,denominator)=bandwidth(m[1]).unwrap();let symbol=((1u64<<m[0])*1_000_000*denominator).div_ceil(numerator);
        let divisor=4*u64::from(m[0]-if m[3]!=0 {2}else{0});
        let bits=(8*len.min(255) as i64+16*i64::from(p[4])-4*i64::from(m[0])+if m[0]<=6 {0}else{8}+if p[2]==0 {20}else{0}).max(0) as u64;
        let symbols4=(u64::from(u16::from_be_bytes([p[0],p[1]]))+8)*4+if m[0]<=6 {25}else{17}+bits.div_ceil(divisor)*u64::from(m[2]+4)*4;
        (symbol*symbols4).div_ceil(4)
    }
    /// Explicit digital peer input. It is neither an echo nor a simulated RF signal.
    pub fn receive(&mut self,config:&LoRaConfig,data:&[u8],rssi_dbm:i16,snr_quarter_db:i8,crc_error:bool)->bool {
        if !self.powered||!self.reset_high||!self.antenna||self.busy()||self.mode!=0x50||data.len()>255||self.config().as_ref()!=Some(config)||self.event.as_ref().is_some_and(|e|matches!(e.1,Event::Rx(..))) {return false;}
        if config.implicit_header && self.packet_params.is_some_and(|p|usize::from(p[3])!=data.len()) {return false;}
        let at=self.after_us(self.airtime_us(data.len()));
        if self.event.as_ref().is_some_and(|e|e.0<=at) {return false;}
        self.event=Some((at,Event::Rx(data.to_vec(),(-2*i32::from(rssi_dbm)).clamp(0,255) as u8,snr_quarter_db,crc_error)));true
    }
    pub fn take_tx(&mut self)->Option<LoRaPacket> {self.tx_packets.pop_front()}
    pub fn snapshot(&self)->String {
        let config=self.config().map_or_else(||"null".into(),|c|c.json());
        let tx=self.tx_packets.iter().map(|p|{let hex=p.payload.iter().map(|b|format!("{b:02x}")).collect::<String>();format!("{{\"config\":{},\"payloadHex\":\"{hex}\",\"cycle\":{}}}",p.config.json(),p.cycle)}).collect::<Vec<_>>().join(",");
        format!("{{\"model\":\"sx1262-lora-digital\",\"physicalRf\":false,\"powered\":{},\"resetHigh\":{},\"antennaEnabled\":{},\"busy\":{},\"mode\":{},\"irq\":{},\"config\":{config},\"transmitted\":[{tx}],\"txCount\":{},\"rxCount\":{},\"timeoutCount\":{},\"rejectedCommands\":{},\"droppedTx\":{}}}",self.powered,self.reset_high,self.antenna,self.busy(),self.mode,self.irq,self.stats.tx,self.stats.rx,self.stats.timeouts,self.stats.rejected,self.stats.dropped)
    }
}
fn word24(p:&[u8])->u32 {(u32::from(p[0])<<16)|(u32::from(p[1])<<8)|u32::from(p[2])}
fn bandwidth(code:u8)->Option<(u64,u64)> {match code {0=>Some((125000,16)),8=>Some((125000,12)),1=>Some((125000,8)),9=>Some((125000,6)),2=>Some((125000,4)),10=>Some((125000,3)),3=>Some((62500,1)),4=>Some((125000,1)),5=>Some((250000,1)),6=>Some((500000,1)),_=>None}}

#[cfg(test)]
mod tests {
    use super::*;
    const HZ:u64=240_000_000;
    fn ready()->Sx1262 {let mut s=Sx1262::new(HZ);s.controls(true,true,true);assert!(s.busy());s.advance_to(840_000);s}
    fn cmd(s:&mut Sx1262,c:&[u8])->Vec<u8> {s.select(true);let r=s.transfer(c,c.len());s.select(false);if let Some(d)=s.busy_until {s.advance_to(d);}r}
    fn configured()->Sx1262 {
        let mut s=ready();for c in [vec![0x80,0],vec![0x8a,1],vec![0x86,0x36,0x40,0,0],vec![0x8b,12,3,4,1],vec![0x8c,0,10,0,5,1,0],vec![0x0d,7,0x40,0x34,0x44],vec![0x08,3,0xff,3,0xff,0,0,0,0]] {cmd(&mut s,&c);assert_eq!(s.status,0);}
        s
    }
    #[test]
    fn powered_reset_busy_chip_select_and_split_register_reads() {
        let mut s=Sx1262::new(HZ);s.select(true);assert_eq!(s.transfer(&[0xc0,0],2),[255,255]);s.select(false);
        s.controls(true,true,true);s.select(true);assert_eq!(s.transfer(&[0xc0,0],2),[255,255]);s.select(false);s.advance_to(840_000);
        s.select(true);assert_eq!(s.transfer(&[0x1d,3],2),[0,0x20]);let r=s.transfer(&[0x20,0,0,0,0,0,0,0],8);assert_eq!(&r[2..],b"SX1261");s.select(false);
        cmd(&mut s,&[0x0d,3,0x20,0]);assert_eq!(cmd(&mut s,&[0x1d,3,0x20,0,0])[4],b'S');
        cmd(&mut s,&[0x0d,7,0x40,0xaa]);assert_eq!(cmd(&mut s,&[0x1d,7,0x40,0,0])[4],0xaa);
        s.controls(true,false,true);assert!(s.busy());s.controls(true,true,true);s.advance_to(s.next_deadline().unwrap());assert_eq!(cmd(&mut s,&[0x1d,7,0x40,0,0])[4],0x14);
    }
    #[test]
    fn tx_uses_guest_buffer_configuration_airtime_and_masked_irq_without_echo() {
        let mut s=configured();cmd(&mut s,&[0x0e,0,b'H',b'e',b'l',b'l',b'o']);cmd(&mut s,&[0x83,0,0,0]);
        assert_eq!(s.mode,0x60);assert!(!s.irq());assert!(s.take_tx().is_none());let end=s.event.as_ref().unwrap().0;
        s.advance_to(end-1);assert!(s.take_tx().is_none());s.advance_to(end);assert!(s.irq());assert_eq!(s.stats.rx,0);
        let packet=s.take_tx().unwrap();assert_eq!(packet.payload,b"Hello");assert_eq!(packet.config.frequency,0x36400000);assert_eq!(packet.config.sync_word,0x3444);assert_eq!(packet.cycle,end);
        assert_eq!(&cmd(&mut s,&[0x12,0,0,0])[2..],[0,1]);cmd(&mut s,&[0x02,0,1]);assert!(!s.irq());assert!(s.take_tx().is_none());
    }
    #[test]
    fn no_peer_times_out_and_explicit_rx_requires_matching_config_and_completion() {
        let mut s=configured();cmd(&mut s,&[0x82,0,0,64]);let deadline=s.event.as_ref().unwrap().0;s.advance_to(deadline);
        assert_eq!(s.irq,512);assert_eq!(s.stats.rx,0);cmd(&mut s,&[0x02,0xff,0xff]);cmd(&mut s,&[0x82,0xff,0xff,0xff]);
        let config=s.config().unwrap();let mut wrong=config.clone();wrong.frequency+=1;
        assert!(!s.receive(&wrong,b"peer",-70,24,false));assert!(s.receive(&config,b"peer",-70,24,false));assert!(!s.receive(&config,b"overlap",-70,24,false));
        let at=s.event.as_ref().unwrap().0;s.advance_to(at-1);assert!(!s.irq());s.advance_to(at);assert!(s.irq());assert_eq!(s.mode,0x50);
        assert_eq!(&cmd(&mut s,&[0x13,0,0,0])[2..],[4,0]);assert_eq!(&cmd(&mut s,&[0x1e,0,0,0,0,0,0])[3..],b"peer");
        assert_eq!(&cmd(&mut s,&[0x14,0,0,0,0])[2..],[140,24,140]);
        cmd(&mut s,&[0x02,0xff,0xff]);assert!(!s.irq());assert!(s.event.is_none());assert_eq!(s.stats.rx,1);
    }
    #[test]
    fn invalid_parameters_and_unsupported_modes_do_not_complete_successfully() {
        let mut s=ready();for c in [&[0x83,0,0,0][..],&[0x8a,0],&[0x8b,0,3,4,1],&[0xc5],&[0x80,2]] {cmd(&mut s,c);assert!(matches!(s.status,8|10));assert!(s.event.is_none());}
        s.select(true);s.transfer(&vec![0x0d;1024],0);s.select(false);assert_eq!(s.status,8);assert!(s.transaction.is_empty());
        cmd(&mut s,&[0]);assert_eq!(s.status,8);assert_eq!(cmd(&mut s,&[0x80,0])[1]&14,0);assert_eq!(s.status,0);
    }
    #[test]
    fn timeout_and_power_loss_cancel_transmit_and_fifo_offsets_wrap() {
        let mut s=configured();cmd(&mut s,&[0x8f,254,253]);cmd(&mut s,&[0x0e,254,1,2,3,4,5]);assert_eq!(&cmd(&mut s,&[0x1e,254,0,0,0,0,0,0])[3..],[1,2,3,4,5]);
        cmd(&mut s,&[0x83,0,0,1]);s.advance_to(s.event.as_ref().unwrap().0);assert_eq!(s.stats.tx,0);assert_eq!(s.irq,512);
        cmd(&mut s,&[0x83,0,0,0]);s.controls(false,true,true);s.advance_to(s.now+HZ*100);assert!(s.take_tx().is_none());assert_eq!(s.stats.tx,0);assert!(!s.irq());
    }
    #[test]
    fn antenna_loss_cancels_receive_but_preserves_single_rx_timeout_and_crc_errors() {
        let mut s=configured();let config=s.config().unwrap();cmd(&mut s,&[0x82,0x0f,0x42,0x40]);
        let timeout=s.rx_deadline.unwrap();assert!(s.receive(&config,b"PEER",-70,24,false));
        s.controls(true,true,false);s.advance_to(timeout-1);assert_eq!(s.stats.rx,0);assert!(!s.irq());
        s.advance_to(timeout);assert_eq!(s.irq,512);assert_eq!(s.stats.timeouts,1);
        s.controls(true,true,true);cmd(&mut s,&[0x02,0xff,0xff]);cmd(&mut s,&[0x82,0xff,0xff,0xff]);
        assert!(s.receive(&config,b"BAD",-90,-8,true));s.advance_to(s.event.as_ref().unwrap().0);
        assert_eq!(s.irq,2|64);assert_eq!(&cmd(&mut s,&[0x1e,0,0,0,0,0])[3..],b"BAD");
    }
    #[test]
    fn warm_sleep_keeps_configuration_but_cold_sleep_and_reset_clear_it() {
        let mut s=configured();let config=s.config().unwrap();cmd(&mut s,&[0x0e,0,123]);cmd(&mut s,&[0x84,4]);
        assert!(s.busy());assert_eq!(s.buffer[0],0);s.select(true);assert_eq!(s.transfer(&[0xc0,0],2),[255,255]);s.select(false);
        s.advance_to(s.next_deadline().unwrap());assert_eq!(s.config(),Some(config));assert_eq!(s.mode,0x20);
        cmd(&mut s,&[0x84,0]);s.select(true);s.select(false);s.advance_to(s.next_deadline().unwrap());
        assert!(s.config().is_none());assert_eq!(s.registers[0x740],0x14);
    }
    #[test]
    fn host_packet_parser_rejects_fractional_out_of_range_and_oversize_values() {
        let config=configured().config().unwrap().json();let good=format!("{{\"config\":{config},\"payloadHex\":\"50454552\",\"rssiDbm\":-70,\"snrQuarterDb\":24,\"crcError\":false}}");
        assert_eq!(LoRaInput::parse(&good).unwrap().payload,b"PEER");
        for bad in [good.replace("-70","-70.1"),good.replace("-70","1"),good.replace("50454552","z0"),good.replace("50454552",&"ff".repeat(256)),good.replace("\"sf\":12","\"sf\":300"),good.replace("\"crcError\":false","\"crcError\":0")] {assert!(LoRaInput::parse(&bad).is_none());}
    }
}
