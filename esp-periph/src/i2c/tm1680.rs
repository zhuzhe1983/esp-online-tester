//! Titan Micro TM1680 write-only LED RAM and command engine.
//! TM1680 v1.1 pp.5,8–9,17; M5GFX 28d47949 Panel_TM1680.{hpp,inl}.
//! RAM addresses count four-bit cells: the first cell travels in the upper nibble.
//! Sources and the v1.1 figure/table discrepancy: docs/platform/corematrix-peripherals.md.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub struct Tm1680State {
    ram:[u8;96], powered:bool, system:bool, display:bool, oscillator:u8, mode:u8,
    duty:u8, blink:u8, cycle:u64, epoch:u64, cpu_hz:u64,
    pub version:u64, pub frames:u64, pub bytes_written:u64,
}
impl Tm1680State {
    pub fn new(cpu_hz:u64)->Self {Self{ram:[0;96],powered:false,system:false,display:false,oscillator:0x98,mode:0,duty:15,blink:0,cycle:0,epoch:0,cpu_hz:cpu_hz.max(1),version:0,frames:0,bytes_written:0}}
    pub fn set_power(&mut self,powered:bool) {
        if self.powered==powered {return;}
        // No retention supply on the CoreMatrix LED board. Power-up RAM is
        // unspecified on silicon; use deterministic dark RAM, never stale pixels.
        self.ram.fill(0);self.system=false;self.display=false;self.oscillator=0x98;
        self.mode=0;self.duty=15;self.blink=0;self.epoch=self.cycle;
        self.powered=powered;self.version=self.version.wrapping_add(1);
    }
    pub fn powered(&self)->bool {self.powered}
    pub fn enabled(&self)->bool {self.powered&&self.system&&self.display&&matches!(self.oscillator,0x98|0x9a)}
    pub fn pwm_steps(&self)->u8 {self.duty+1}
    pub fn mode(&self)->u8 {self.mode}
    pub fn half_period(&self)->Option<u64> {match self.blink {1=>Some((self.cpu_hz/4).max(1)),2=>Some((self.cpu_hz/2).max(1)),3=>Some(self.cpu_hz),_=>None}}
    fn phase(&self)->bool {self.half_period().is_none_or(|half|(self.cycle.saturating_sub(self.epoch)/half)&1==0)}
    pub fn next_deadline(&self)->Option<u64> {
        if !self.enabled() {return None;}
        self.half_period().and_then(|half|self.epoch.checked_add((self.cycle.saturating_sub(self.epoch)/half).saturating_add(1).saturating_mul(half))).filter(|next|*next>self.cycle)
    }
    pub fn advance_to(&mut self,cycle:u64) {let phase=self.phase();self.cycle=self.cycle.max(cycle);if self.enabled()&&phase!=self.phase(){self.version=self.version.wrapping_add(1);}}
    /// Average white intensity, not an optical luminance calibration. The board
    /// is wired for NMOS COM drivers; opposite-polarity PMOS isn't a light source.
    pub fn pixel(&self,row:usize,com:usize)->u8 {
        if !self.enabled()||!self.phase()||self.mode&2!=0 {return 0;}
        let (rows,coms)=if self.mode&1!=0 {(24,16)}else{(32,8)};
        if row>=rows||com>=coms {return 0;}
        let address=row*(coms/4)+com/4;
        if self.ram[address]&(1<<(com&3))!=0 {((u16::from(self.duty+1)*255+8)/16) as u8}else{0}
    }
    fn command(&mut self,byte:u8) {
        match byte {
            0x80=>self.system=false,0x81=>self.system=true,
            0x82=>self.display=false,0x83=>self.display=true,
            0x88..=0x8b=>{self.blink=byte&3;self.epoch=self.cycle;},
            0x90..=0x9f=>self.oscillator=byte&0xfe,
            0xa0..=0xaf=>self.mode=(byte>>2)&3,
            0xb0..=0xbf=>self.duty=byte&15,
            _=>return,
        }
        self.version=self.version.wrapping_add(1);
    }
}

pub struct Tm1680 {state:Arc<Mutex<Tm1680State>>,address:Option<usize>,dirty:bool}
impl Tm1680 {
    pub const ADDRESS:u8=0x72;
    pub fn new(state:Arc<Mutex<Tm1680State>>)->Self {Self{state,address:None,dirty:false}}
}
impl I2cDevice for Tm1680 {
    fn start(&mut self,read:bool)->bool {self.address=None;self.dirty=false;!read&&self.state.lock().expect("TM1680 state poisoned").powered}
    fn write(&mut self,byte:u8)->bool {
        let mut state=self.state.lock().expect("TM1680 state poisoned");
        if !state.powered {return false;}
        if let Some(address)=self.address {
            // Invalid/out-of-range pointers are ignored. No unverified wrap back
            // into LED0 when a malformed guest overruns the 96-nibble memory.
            let mut written=false;
            for (offset,nibble) in [(0,byte>>4),(1,byte&15)] {if let Some(cell)=state.ram.get_mut(address.saturating_add(offset)) {if *cell!=nibble {*cell=nibble;state.version=state.version.wrapping_add(1);}self.dirty=true;written=true;}}
            self.address=Some(address.saturating_add(2));if written {state.bytes_written=state.bytes_written.wrapping_add(1);}
        } else if byte&0x80!=0 {state.command(byte);}else{self.address=Some(byte as usize);}
        true
    }
    fn read(&mut self)->u8 {0xff}
    fn stop(&mut self) {if self.dirty {let mut state=self.state.lock().expect("TM1680 state poisoned");state.frames=state.frames.wrapping_add(1);}self.address=None;self.dirty=false;}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device()->(Arc<Mutex<Tm1680State>>,Tm1680) {let state=Arc::new(Mutex::new(Tm1680State::new(160_000_000)));let d=Tm1680::new(state.clone());(state,d)}
    fn send(d:&mut Tm1680,bytes:&[u8]) {assert!(d.start(false));for &b in bytes {assert!(d.write(b));}d.stop();}
    #[test]
    fn write_only_address_power_and_nibble_order_are_wire_visible() {
        let (s,mut d)=device();assert!(!d.start(false));assert!(!d.write(0x81));s.lock().unwrap().set_power(true);
        assert!(!d.start(true));assert!(!d.start_address(0x72,0x73,false));
        send(&mut d,&[0x80,0xa4,0x98,0x81,0xbf,0x83,0,0x12,0x48]);
        let s=s.lock().unwrap();assert_eq!((s.pixel(0,0),s.pixel(0,5),s.pixel(0,10),s.pixel(0,15)),(255,255,255,255));
        assert_eq!((s.pixel(0,1),s.pixel(1,0)),(0,0));assert_eq!((s.frames,s.bytes_written),(1,2));
    }
    #[test]
    fn independent_one_hot_oracle_exercises_every_core_matrix_pixel() {
        let (s,mut d)=device();s.lock().unwrap().set_power(true);send(&mut d,&[0xa4,0x81,0xbf,0x83]);
        // M5GFX display() transposes its page bitmap, nibble-swaps each row byte.
        // This vector comes from the physical ROW/COM net, not from our decoder.
        for y in 0..16 {for x in 0..16 {
            let mut wire=[0u8;33];wire[1+y*2+x/8]=1<<((x&7)^4);send(&mut d,&wire);
            let s=s.lock().unwrap();for row in 0..16 {for col in 0..16 {assert_eq!(s.pixel(row,col),if row==y&&col==x {255}else{0},"source {x},{y}; observed {col},{row}");}}
        }}
    }
    #[test]
    fn brightness_sleep_power_and_blink_preserve_only_documented_retention() {
        let (s,mut d)=device();s.lock().unwrap().set_power(true);send(&mut d,&[0xa4,0x81,0xb0,0x83,0,0x10]);assert_eq!(s.lock().unwrap().pixel(0,0),16);
        send(&mut d,&[0xb7]);assert_eq!(s.lock().unwrap().pixel(0,0),128);
        send(&mut d,&[0x82,0x80]);assert_eq!(s.lock().unwrap().pixel(0,0),0);
        send(&mut d,&[0x81,0x83]);assert_eq!(s.lock().unwrap().pixel(0,0),128);
        send(&mut d,&[0x89]);{let mut s=s.lock().unwrap();assert_eq!(s.next_deadline(),Some(40_000_000));s.advance_to(40_000_000);assert_eq!(s.pixel(0,0),0);s.advance_to(80_000_000);assert_eq!(s.pixel(0,0),128);}
        send(&mut d,&[0x88]);{let mut s=s.lock().unwrap();assert_eq!(s.next_deadline(),None);s.set_power(false);s.set_power(true);}
        send(&mut d,&[0xa4,0x81,0xbf,0x83]);assert_eq!(s.lock().unwrap().pixel(0,0),0);
    }
    #[test]
    fn addressed_partial_writes_high_bit_data_and_invalid_overrun_stay_bounded() {
        let (s,mut d)=device();s.lock().unwrap().set_power(true);send(&mut d,&[0xa4,0x81,0xbf,0x83]);
        send(&mut d,&[4,0x81]);{let s=s.lock().unwrap();assert_eq!((s.pixel(1,3),s.pixel(1,4)),(255,255));}
        send(&mut d,&[95,0xf1,0xff,0xff]);send(&mut d,&[127,0xff]);{let s=s.lock().unwrap();assert_eq!(s.pixel(0,0),0);assert_eq!(s.bytes_written,2);}
        send(&mut d,&[0x90]);assert_eq!(s.lock().unwrap().pixel(1,3),0,"no external clock wired");
        send(&mut d,&[0x98,0x89]);{let mut s=s.lock().unwrap();s.advance_to(u64::MAX);assert_eq!(s.next_deadline(),None);}
    }
}
