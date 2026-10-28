//! Original T-Deck ESP32-C3 keyboard I2C protocol, not an emulated second CPU.
//! Xinyuan-LilyGO/T-Deck 2027dee7 examples/Keyboard_ESP32C3: 5 columns x 7 rows,
//! 03/04 raw/key mode, 01/02 backlight commands. One complete scan is 35 x 1 ms;
//! sub-scan sampling, C3 boot latency and electrical key bounce are not modeled.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
const KEYS:[[u8;7];5]=[*b"qw\0a\0 \0",*b"esdpxz\0",*b"rgt\0vcf",*b"uhy\rbnj",*b"oli\x08$mk"];
const SYMBOLS:[[u8;7];5]=[*b"#1\0*\0\x000",*b"245@87\0",*b"3/(\0?96",*b"_:)\0!,;",*b"+\"-\0\0.'"];
pub(super) struct KeyboardState {pub powered:bool,pub raw:bool,keys:[u8;5],scanned:[u8;5],pending:u8,period:u64,deadline:Option<u64>,pub brightness:u8,configured_brightness:u8,default_brightness:u8,pub scans:u64,pub reads:u64}
impl KeyboardState {
    pub fn new(hz:u64)->Self {Self{powered:false,raw:false,keys:[0;5],scanned:[0;5],pending:0,period:(hz/1000*35).max(1),deadline:None,brightness:0,configured_brightness:0,default_brightness:127,scans:0,reads:0}}
    pub fn power(&mut self,cycle:u64,on:bool) {if self.powered && !on {self.raw=false;self.scanned=[0;5];self.pending=0;self.deadline=None;self.brightness=0;self.configured_brightness=0;self.default_brightness=127;}
        if !self.powered && on && self.keys!=self.scanned {self.deadline=Some(cycle.saturating_add(self.period));}self.powered=on;}
    pub fn key(&mut self,cycle:u64,col:u8,row:u8,pressed:bool)->bool {
        if col>=5 || row>=7 {return false;}let bit=1<<row;let value=&mut self.keys[col as usize];if pressed {*value|=bit;}else{*value&=!bit;}
        if self.powered && self.keys!=self.scanned && self.deadline.is_none() {self.deadline=Some(cycle.saturating_add(self.period));}true
    }
    pub fn next_deadline(&self)->Option<u64> {self.deadline}
    pub fn advance(&mut self,cycle:u64) {
        if !self.powered || !self.deadline.is_some_and(|d|d<=cycle) {return;}self.deadline=None;self.scans+=1;
        let changed=std::array::from_fn::<_,5,_>(|i|self.keys[i]&!self.scanned[i]);self.scanned=self.keys;
        let active=|c:usize,r:u8|self.keys[c]&(1<<r)!=0;
        // Preserve the C3 firmware's single last-character mailbox and scan order.
        for row in 0..7 {for col in 0..5 {if changed[col]&(1<<row)!=0 && (KEYS[col][row]!=0 || SYMBOLS[col][row]!=0) {
            let mut ch=if active(0,2){SYMBOLS[col][row]}else{KEYS[col][row]};if active(1,6)||active(2,3){ch=ch.wrapping_sub(32);}self.pending=ch;
        }}}
        if changed[3]&8!=0 {self.pending=13;}
        if changed[4]&8!=0 {self.pending=8;}
        if active(0,4) && changed[3]&(1<<4)!=0 {self.brightness=if self.brightness==0 {if self.configured_brightness==0 {self.default_brightness}else{self.configured_brightness}}else{0};self.pending=0;}
        if active(0,4) && changed[2]&(1<<5)!=0 {self.pending=12;}
    }
}
pub(super) struct Keyboard {state:Arc<Mutex<KeyboardState>>,command:Option<u8>,response:[u8;5],at:usize,len:usize}
impl Keyboard {pub fn new(state:Arc<Mutex<KeyboardState>>)->Self {Self{state,command:None,response:[0;5],at:0,len:0}}}
impl I2cDevice for Keyboard {
    fn start(&mut self,read:bool)->bool {
        let mut s=self.state.lock().expect("T-Deck keyboard poisoned");if !s.powered {return false;}
        if read {self.at=0;s.reads+=1;if s.raw {self.response=s.scanned;self.len=5;}else{self.response[0]=s.pending;s.pending=0;self.len=1;}}else{self.command=None;}true
    }
    fn write(&mut self,b:u8)->bool {
        let mut s=self.state.lock().expect("T-Deck keyboard poisoned");match self.command.take() {
            Some(1)=>{s.brightness=b;s.configured_brightness=b;self.command=Some(2);},Some(2)=>{if b>30{s.default_brightness=b;}},_=>match b {1|2=>self.command=Some(b),3=>s.raw=true,4=>s.raw=false,_=>{}}
        }true
    }
    fn read(&mut self)->u8 {let b=if self.at<self.len{self.response[self.at]}else{0};self.at=self.at.saturating_add(1);b}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_scan_snapshot_releases_and_modes_obey_c3_protocol() {
        let state=Arc::new(Mutex::new(KeyboardState::new(1000)));let mut d=Keyboard::new(state.clone());assert!(!d.start(true));state.lock().unwrap().power(0,true);
        d.start(false);d.write(3);state.lock().unwrap().key(10,1,2,true);state.lock().unwrap().advance(44);d.start(true);assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[0;5]);
        state.lock().unwrap().advance(45);d.start(true);assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[0,4,0,0,0]);
        state.lock().unwrap().key(50,1,2,false);state.lock().unwrap().advance(85);d.start(true);assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[0;5]);
        d.start(false);d.write(4);d.start(true);assert_eq!(d.read(),b'd');d.start(true);assert_eq!(d.read(),0);
        d.start(false);d.write(1);d.write(80);assert_eq!(state.lock().unwrap().brightness,80);state.lock().unwrap().power(100,false);assert!(!d.start(true));state.lock().unwrap().power(0,true);d.start(true);assert_eq!(d.read(),0);assert!(!state.lock().unwrap().raw);
        state.lock().unwrap().key(110,0,0,true);state.lock().unwrap().power(120,false);state.lock().unwrap().advance(200);assert_eq!(state.lock().unwrap().next_deadline(),None);state.lock().unwrap().power(300,true);state.lock().unwrap().advance(334);d.start(true);assert_eq!(d.read(),0);state.lock().unwrap().advance(335);d.start(true);assert_eq!(d.read(),b'q');
    }
}
