//! ST25R3916 reader-side digital subset, initially with no RF peer.
//! ST DS12484 Rev 8: I2C §4.3.4, commands §4.4, registers §4.5. Identity
//! 0x2a is the datasheet revision, not a claim about a measured PaperMono IC.
//! No card is invented: completed transmissions expire through the programmed
//! no-response timer. Analog calibration, wake-up and card emulation are absent.
use super::I2cDevice;
use std::collections::VecDeque;
use std::sync::{Arc,Mutex};

const FC:u64=13_560_000;
const OSC:usize=0;const DIRECT:usize=1;const FIELD:usize=2;const TX:usize=3;const NRT:usize=4;const GUARD:usize=5;
#[derive(Clone,Copy)]
enum Access { Command, Write(u8), Read(u8), FifoWrite, FifoRead, Ignore }
pub struct St25r3916State {
    pub powered:bool, a:[u8;64],b:[u8;64],test:[u8;64],fifo:VecDeque<u8>,fifo_error:u8,
    now:u64,hz:u64,deadlines:[Option<u64>;6],osc_ok:bool,rx_on:bool,tx_on:bool,
    pub commands:u64,pub transmissions:u64,pub no_response:u64,pub unsupported:u64,
    pub last_tx:Vec<u8>,pub modes_seen:u16,pub unsupported_commands:u64,
}
impl St25r3916State {
    pub fn new(hz:u64)->Self {let mut s=Self{powered:false,a:[0;64],b:[0;64],test:[0;64],fifo:VecDeque::new(),fifo_error:0,now:0,hz:hz.max(1),deadlines:[None;6],osc_ok:false,rx_on:false,tx_on:false,commands:0,transmissions:0,no_response:0,unsupported:0,last_tx:Vec::new(),modes_seen:0,unsupported_commands:0};s.reset();s}
    fn reset(&mut self) {
        self.a=[0;64];self.b=[0;64];self.test=[0;64];self.clear_fifo();self.deadlines=[None;6];self.osc_ok=false;self.rx_on=false;self.tx_on=false;
        // DS12484 Rev 8 §4.5: nonzero power-on defaults; undefined readouts are zero.
        for (r,v) in [(3,8),(12,0x2c),(13,0xd8),(15,8),(21,0x80),(38,0x80),(39,0x80),(40,0x70),(41,0x70),(42,0x33),(43,0x33),(63,0x2a)] {self.a[r]=v;}
        for (r,v) in [(11,0x0c),(12,0x93),(21,0x33),(40,0x10),(41,0x7c)] {self.b[r]=v;}
    }
    pub fn set_powered(&mut self,powered:bool) {if self.powered!=powered {self.reset();self.powered=powered;}}
    fn clear_fifo(&mut self) {self.fifo.clear();self.fifo_error=0;self.deadlines[TX]=None;self.tx_on=false;}
    pub fn irq(&self)->bool {self.powered&&(0..4).any(|i|self.a[0x1a+i]&!self.a[0x16+i]!=0)}
    pub fn next_deadline(&self)->Option<u64> {self.deadlines.iter().flatten().copied().min()}
    fn after_us(&self,us:u64)->u64 {self.now.saturating_add(self.hz.saturating_mul(us).div_ceil(1_000_000).max(1))}
    fn start_nrt(&mut self,at:u64) {
        // §4.2.7/§4.5.22–24: 16-bit count; zero disables, step 64/fc or 4096/fc.
        let count=u64::from(u16::from_be_bytes([self.a[0x10],self.a[0x11]]));
        self.deadlines[NRT]=(count!=0).then(||at.saturating_add((count*if self.a[0x12]&1!=0 {4096}else{64}*self.hz).div_ceil(FC)));
    }
    pub fn advance_to(&mut self,cycle:u64) {
        self.now=self.now.max(cycle);
        while let Some((index,at))=self.deadlines.iter().enumerate().filter_map(|(i,d)|d.map(|at|(i,at))).filter(|(_,at)|*at<=self.now).min_by_key(|(_,at)|*at) {
            self.deadlines[index]=None;
            match index {
                OSC=>{self.osc_ok=true;self.a[0x1a]|=0x80;},
                // Nominal 3.3V supply, no analog PSRR or measured voltage claim. §4.5.57
                DIRECT=>{self.b[0x2c]=0xb0;self.a[0x1b]|=0x80;},
                FIELD=>{self.a[2]|=8;self.a[0x1d]|=0x20;
                    // §4.4.5/table 14, §4.5.29: 75 us + B15 * 2048/fc.
                    self.deadlines[GUARD]=Some(at.saturating_add(self.hz*75/1_000_000+(u64::from(self.b[0x15])*2048*self.hz).div_ceil(FC)));},
                GUARD=>{self.a[0x1b]|=2;},
                TX=>{self.tx_on=false;self.rx_on=self.a[2]&0x40!=0;self.a[0x1a]|=8;self.start_nrt(at);},
                NRT=>{self.rx_on=false;self.a[0x1b]|=0x40;self.no_response+=1;},
                _=>{},
            }
        }
    }
    fn command(&mut self,cmd:u8) {
        self.commands+=1;
        match cmd {
            0xc0|0xc1=>self.reset(),
            0xc2|0xc3 if self.a[2]&0x80!=0=>{let keep_nrt=if self.a[0x12]&2!=0 {self.deadlines[NRT]}else{None};self.clear_fifo();self.deadlines[DIRECT]=None;self.deadlines[FIELD]=None;self.deadlines[GUARD]=None;self.deadlines[NRT]=keep_nrt;self.rx_on=false;self.a[0x1a..=0x1d].fill(0);},
            0xdb if self.osc_ok=>self.clear_fifo(),
            0xc4..=0xc7 if self.osc_ok && self.a[2]&8!=0 && self.a[3]&0x80==0=> {
                let mode=(self.a[3]>>3)&15;
                if !matches!(mode,1|2|3|14|15)||cmd>=0xc6&&mode!=1 {self.unsupported+=1;self.unsupported_commands|=1u64<<(cmd&63);return;}
                let bits=if cmd>=0xc6 {7}else{u64::from(u16::from_be_bytes([self.a[0x22],self.a[0x23]]))};
                let bytes=bits.div_ceil(8) as usize;
                if cmd<0xc6 && (bits==0||bytes>self.fifo.len()) {self.fifo_error|=0x20;return;}
                self.last_tx=if cmd==0xc6 {vec![0x26]}else if cmd==0xc7 {vec![0x52]}else{self.fifo.drain(..bytes).collect()};
                self.transmissions+=1;self.modes_seen|=1<<mode;self.tx_on=true;self.rx_on=false;self.deadlines[NRT]=None;
                // Digital payload duration only; RF framing/stream modulation timing is not calibrated.
                let fc_per_bit=128u64>>((self.a[4]>>4)&3);
                self.deadlines[TX]=Some(self.now.saturating_add((bits.max(1)*fc_per_bit*self.hz).div_ceil(FC)));
            },
            0xc8|0xc9 if self.osc_ok && self.a[2]&3==1=>{
                // No external RF field. Initial/response delay + selected waiting slots.
                let clocks=if cmd==0xc8 {4096}else{768}+u64::from(self.a[0x0a]&3)*512;
                self.deadlines[FIELD]=Some(self.now.saturating_add((clocks*self.hz).div_ceil(FC)));},
            0xd0=>{self.rx_on=false;},
            0xd1=>{self.rx_on=self.osc_ok&&self.a[2]&0x40!=0;},
            0xd5|0xda if self.osc_ok=>{self.a[0x2d]=0;self.a[0x2e]=0;},
            0xd6 if self.osc_ok=>{self.deadlines[DIRECT]=Some(self.after_us(1000));},
            0xe3 if self.osc_ok=>self.start_nrt(self.now),
            0xe8 if self.osc_ok=>{self.deadlines[NRT]=None;},
            _=>{self.unsupported+=1;self.unsupported_commands|=1u64<<(cmd&63);},
        }
    }
    fn write_register(&mut self,bank:u8,reg:u8,value:u8) {
        let r=usize::from(reg);
        if r>=64{return;}
        if bank==2 {self.test[r]=value;return;}
        if bank==1 {if matches!(r,5|6|11..=13|15|21|40..=42|48..=51){self.b[r]=value;}return;}
        // Read-only and undefined addresses ignore writes (§4.5 tables 17/18).
        if !matches!(r,0..=25|34|35|38..=44|47|50..=52|55|56|59|60) {return;}
        if r==3&&!self.osc_ok {return;}
        let old=self.a[r];self.a[r]=value;
        if r==2 {
            if value&0x80==0 {self.osc_ok=false;self.deadlines=[None;6];self.tx_on=false;self.rx_on=false;}
            else if old&0x80==0 {self.deadlines[OSC]=Some(self.after_us(700));}
            if value&8==0 {self.deadlines[TX]=None;self.tx_on=false;}
            if value&0x40==0 {self.rx_on=false;}
        }
    }
    fn read_register(&mut self,bank:u8,reg:u8)->u8 {
        let r=usize::from(reg);
        if r>=64{return 0;}
        if bank==2 {return self.test[r];}
        if bank==1 {return self.b[r];}
        match r {
            0x1a..=0x1d=>{let v=self.a[r];self.a[r]=0;v},
            0x1e=>self.fifo.len() as u8,
            0x1f=>(((self.fifo.len()>>8) as u8&3)<<6)|self.fifo_error,
            0x31=>(u8::from(self.osc_ok)<<4)|(u8::from(self.tx_on)<<5)|(u8::from(self.rx_on)<<3),
            _=>self.a[r],
        }
    }
}
pub struct St25r3916 {state:Arc<Mutex<St25r3916State>>,access:Access,bank:u8}
impl St25r3916 {pub fn new(state:Arc<Mutex<St25r3916State>>)->Self {Self{state,access:Access::Command,bank:0}}}
impl I2cDevice for St25r3916 {
    fn start(&mut self,read:bool)->bool {if !read {self.access=Access::Command;}self.state.lock().expect("ST25R3916 mutex poisoned").powered}
    fn write(&mut self,byte:u8)->bool {
        let mut s=self.state.lock().expect("ST25R3916 mutex poisoned");if !s.powered{return false;}
        match self.access {
            Access::Command=>match byte {
                0xfb=>self.bank=1,0xfc=>self.bank=2,
                0..=0x3f=>self.access=Access::Write(byte),0x40..=0x7f=>self.access=Access::Read(byte&63),
                0x80=>self.access=Access::FifoWrite,0x9f=>self.access=Access::FifoRead,
                0xc0..=0xff=>{s.command(byte);self.access=Access::Command;},
                _=>{s.unsupported+=1;self.access=Access::Ignore;},
            },
            Access::Write(reg)=>{s.write_register(self.bank,reg,byte);self.access=Access::Write(reg.saturating_add(1));},
            Access::FifoWrite=>{if s.fifo.len()<512{s.fifo.push_back(byte);}else{s.fifo_error|=0x10;}},
            _=>{},
        }true
    }
    fn read(&mut self)->u8 {
        let mut s=self.state.lock().expect("ST25R3916 mutex poisoned");if !s.powered{return 0xff;}
        match self.access {
            Access::Read(reg)=>{self.access=Access::Read(reg.saturating_add(1));s.read_register(self.bank,reg)},
            Access::FifoRead=>s.fifo.pop_front().unwrap_or_else(||{s.fifo_error|=0x20;0}),
            _=>0,
        }
    }
    fn stop(&mut self) {self.bank=0;self.access=Access::Command;}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup()->(St25r3916,Arc<Mutex<St25r3916State>>) {let state=Arc::new(Mutex::new(St25r3916State::new(240_000_000)));state.lock().unwrap().set_powered(true);(St25r3916::new(state.clone()),state)}
    fn write(d:&mut St25r3916,data:&[u8]) {assert!(d.start(false));for b in data {assert!(d.write(*b));}d.stop();}
    fn read(d:&mut St25r3916,prefix:&[u8],n:usize)->Vec<u8> {assert!(d.start(false));for b in prefix {assert!(d.write(*b));}assert!(d.start(true));let v=(0..n).map(|_|d.read()).collect();d.stop();v}
    fn ready(d:&mut St25r3916,s:&Arc<Mutex<St25r3916State>>) {write(d,&[2,0xc8]);s.lock().unwrap().advance_to(168_000);}
    #[test]
    fn power_defaults_read_only_banks_and_bounds() {
        let (mut d,s)=setup();assert_eq!(read(&mut d,&[0x7f],2),[0x2a,0]);
        write(&mut d,&[0x3f,0]);assert_eq!(read(&mut d,&[0x7f],1),[0x2a]);
        write(&mut d,&[0xfb,0x0c,0x47]);assert_eq!(read(&mut d,&[0xfb,0x4c],1),[0x47]);assert_eq!(read(&mut d,&[0x4c],1),[0x2c]);
        write(&mut d,&[0xfc,4,0x10]);assert_eq!(read(&mut d,&[0xfc,0x44],1),[0x10]);
        s.lock().unwrap().set_powered(false);assert!(!d.start(false));assert!(!d.write(0));assert_eq!(d.read(),0xff);
        s.lock().unwrap().set_powered(true);assert_eq!(read(&mut d,&[0xfb,0x4c],1),[0x93]);
        write(&mut d,&[0xc1]);assert_eq!(read(&mut d,&[0x43],1),[8]);
    }
    #[test]
    fn oscillator_irq_masks_and_read_clear() {
        let (mut d,s)=setup();write(&mut d,&[0x16,0x80]);write(&mut d,&[2,0x80]);
        s.lock().unwrap().advance_to(167_999);assert_eq!(read(&mut d,&[0x71],1),[0]);
        s.lock().unwrap().advance_to(168_000);assert_eq!(read(&mut d,&[0x71],1),[0x10]);assert!(!s.lock().unwrap().irq());
        write(&mut d,&[0x16,0]);assert!(s.lock().unwrap().irq());assert_eq!(read(&mut d,&[0x5a],1),[0x80]);assert!(!s.lock().unwrap().irq());assert_eq!(read(&mut d,&[0x5a],1),[0]);
        write(&mut d,&[2,0]);assert_eq!(read(&mut d,&[0x71],1),[0]);assert_eq!(s.lock().unwrap().next_deadline(),None);
    }
    #[test]
    fn fifo_bounded_overflow_underflow_clear_and_no_invented_rx() {
        let (mut d,s)=setup();ready(&mut d,&s);let mut bytes=vec![0x80];bytes.extend((0..513).map(|i|i as u8));write(&mut d,&bytes);
        assert_eq!(read(&mut d,&[0x5e],2),[0,0x90]);assert_eq!(read(&mut d,&[0x9f],3),[0,1,2]);assert_eq!(read(&mut d,&[0x5e],2),[253,0x50]);
        write(&mut d,&[0xdb]);assert_eq!(read(&mut d,&[0x5e],2),[0,0]);assert_eq!(read(&mut d,&[0x9f],1),[0]);assert_eq!(read(&mut d,&[0x5f],1),[0x20]);
        write(&mut d,&[0xdb]);write(&mut d,&[0x22,0,16]);write(&mut d,&[0x80,0x93,0x20]);write(&mut d,&[0xc5]);
        let end=s.lock().unwrap().next_deadline().unwrap();s.lock().unwrap().advance_to(end);
        assert_eq!(s.lock().unwrap().last_tx,[0x93,0x20]);assert_eq!(read(&mut d,&[0x5e],2),[0,0]);assert_eq!(read(&mut d,&[0x5a],1)[0]&0x38,8);
    }
    #[test]
    fn field_on_requires_detector_and_obeys_programmed_guard_timer() {
        let (mut d,s)=setup();ready(&mut d,&s);read(&mut d,&[0x5a],4);
        write(&mut d,&[0xc8]);assert_eq!(s.lock().unwrap().next_deadline(),None);
        write(&mut d,&[2,0xc1]);write(&mut d,&[0xfb,0x15,2]);write(&mut d,&[0xc8]);
        let field=s.lock().unwrap().next_deadline().unwrap();assert_eq!(field-168_000,(4096*240_000_000u64).div_ceil(FC));s.lock().unwrap().advance_to(field);
        assert_eq!(read(&mut d,&[0x5d],1),[0x20]);assert_eq!(read(&mut d,&[0x5b],1),[0]);assert_eq!(read(&mut d,&[0x42],1)[0]&8,8);
        let guard=s.lock().unwrap().next_deadline().unwrap();assert_eq!(guard-field,18_000+(2*2048*240_000_000u64).div_ceil(FC));s.lock().unwrap().advance_to(guard);assert_eq!(read(&mut d,&[0x5b],1),[2]);
    }
    #[test]
    fn reqa_programmed_timeout_and_stop_cancel() {
        let (mut d,s)=setup();ready(&mut d,&s);read(&mut d,&[0x5a],4);write(&mut d,&[0x10,0,10]);write(&mut d,&[0xc6]);
        let tx_end=s.lock().unwrap().next_deadline().unwrap();s.lock().unwrap().advance_to(tx_end);assert_eq!(s.lock().unwrap().last_tx,[0x26]);
        let timeout=s.lock().unwrap().next_deadline().unwrap();assert_eq!(timeout-tx_end,(10*64*240_000_000u64).div_ceil(FC));
        s.lock().unwrap().advance_to(timeout-1);assert_eq!(read(&mut d,&[0x5b],1),[0]);s.lock().unwrap().advance_to(timeout);assert_eq!(read(&mut d,&[0x5b],1),[0x40]);assert_eq!(s.lock().unwrap().no_response,1);
        write(&mut d,&[0xc7]);write(&mut d,&[0xc2]);s.lock().unwrap().advance_to(timeout+1_000_000);assert_eq!(s.lock().unwrap().no_response,1);assert_eq!(read(&mut d,&[0x5a],4),[0,0,0,0]);
        write(&mut d,&[0xe3]);assert!(s.lock().unwrap().next_deadline().is_some());write(&mut d,&[0xe8]);assert_eq!(s.lock().unwrap().next_deadline(),None);
    }
}
