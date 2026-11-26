//! MLX90640 word-addressed digital subset, Melexis datasheet rev12 and
//! M5Unit-Thermal2-Internal-FW 496530d6 src/mlx90640.cpp. Synthetic raw ADC
//! scene plus explicitly virtual EEPROM, never host-generated temperatures.
//! No optical noise, bad pixels, sensor self-heating or analog calibration.
use super::I2cDevice;
use std::sync::{Arc,Mutex};
pub const CLOCK_HZ:u64=240_000_000;
pub struct Mlx90640State {
    pub eeprom:[u16;832],pub ram:[u16;832],pub control:u16,pub status:u16,
    pub frames:u64,pub reads:u64,pub writes:u64,pub now:u64,pub next:u64,
    scene:[u16;5],
}
impl Default for Mlx90640State {fn default()->Self {
    let mut ee=[0;832];
    // Public virtual calibration vector: gain10000, kVdd1024, vdd25=-8224,
    // KtPTAT1, vPTAT25=16384, alphaPTAT8, pixel alpha10001/2^30.
    // Nonzero uniform Kta/Kv avoid zero-scale normalization in manufacturer code.
    for (i,v) in [(0x21,10000),(0x30,10000),(0x31,16384),(0x32,8),(0x33,0x20ff),(0x34,0x1111),(0x36,0x0101),(0x37,0x0101),(0x38,0x2000),(0x39,1),(0x3f,0x1880)] {ee[i]=v;}
    ee[64..].fill(0x10);
    let mut s=Self{eeprom:ee,ram:[0;832],control:0x1901,status:0,frames:0,reads:0,writes:0,now:0,next:CLOCK_HZ/2,scene:[1000,2000,16,12,4]};s.capture();s
}}
impl Mlx90640State {
    pub fn period(&self)->u64 {(CLOCK_HZ*2)>>((self.control>>7)&7)}
    pub fn stimulus(&mut self,sample:[u16;5])->bool {
        if sample[0]>32767 || sample[1]>32767 || sample[2]>=32 || sample[3]>=24 || sample[4]>32 {return false;}self.scene=sample;true
    }
    fn capture(&mut self) {
        let [background,hotspot,cx,cy,radius]=self.scene;
        for y in 0..24i32 {for x in 0..32i32 {let dx=x-i32::from(cx);let dy=y-i32::from(cy);self.ram[(y*32+x)as usize]=if dx*dx+dy*dy<=i32::from(radius).pow(2) {hotspot}else{background};}}
        // Factory equations yield exactly nominal Vdd3.3V and Ta25C.
        self.ram[768]=8000;self.ram[778]=10000;self.ram[800]=1000;self.ram[810]=(-8224i16)as u16;
    }
    pub fn advance(&mut self,cycle:u64) {
        if cycle<self.now {return;}self.now=cycle;
        if cycle>=self.next {let n=1+(cycle-self.next)/self.period();self.frames=self.frames.wrapping_add(n);self.status=(self.status&0x30)|8|((self.frames as u16)&1);self.capture();self.next=self.next.saturating_add(n.saturating_mul(self.period()));}
    }
    fn word(&self,address:u16)->u16 {match address {0x2400..=0x273f=>self.eeprom[usize::from(address-0x2400)],0x0400..=0x073f=>self.ram[usize::from(address-0x0400)],0x8000=>self.status,0x800d=>self.control,_=>0}}
    fn put(&mut self,address:u16,value:u16) {self.writes+=1;match address {
        0x8000=>self.status=(value&0x30)|(self.status&1),
        0x800d=>{self.control=value;self.next=self.now.saturating_add(self.period());},_=>{}
    }}
}
pub struct Mlx90640 {pub shared:Arc<Mutex<Mlx90640State>>,pointer:u16,phase:u8,hi:u8,read_low:bool,latched:Vec<u16>,read_index:usize}
impl Mlx90640 {pub fn new(shared:Arc<Mutex<Mlx90640State>>)->Self {Self{shared,pointer:0,phase:0,hi:0,read_low:false,latched:Vec::new(),read_index:0}}}
impl I2cDevice for Mlx90640 {
    fn start(&mut self,read:bool)->bool {
        if !read {self.phase=0;}else{
            let mut s=self.shared.lock().expect("MLX90640 state poisoned");s.reads+=1;
            self.latched=(0..834u16).map(|i|s.word(self.pointer.wrapping_add(i))).collect();self.read_index=0;self.read_low=false;
        }true
    }
    fn write(&mut self,b:u8)->bool {match self.phase {
        0=>{self.hi=b;self.phase=1;},1=>{self.pointer=u16::from_be_bytes([self.hi,b]);self.phase=2;},
        2=>{self.hi=b;self.phase=3;},_=>{self.shared.lock().expect("MLX90640 state poisoned").put(self.pointer,u16::from_be_bytes([self.hi,b]));self.pointer=self.pointer.wrapping_add(1);self.phase=2;}
    }true}
    fn read(&mut self)->u8 {
        let word=self.latched.get(self.read_index).copied().unwrap_or(0);let result=if self.read_low {word as u8}else{(word>>8)as u8};
        self.read_low = !self.read_low;if !self.read_low {self.pointer=self.pointer.wrapping_add(1);self.read_index+=1;}result
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn virtual_calibration_matches_manufacturer_ambient_equations_and_subpage_clock() {
        let mut s=Mlx90640State::default();let ee=&s.eeprom;let raw=&s.ram;
        let kvdd=f64::from((ee[0x33]>>8)as i8)*32.0;let vdd25=(f64::from(ee[0x33]&255)-256.0)*32.0-8192.0;
        let vdd=(f64::from(raw[810]as i16)-vdd25)/kvdd+3.3;assert!((vdd-3.3).abs()<1e-9);
        let ptat=f64::from(raw[800]);let art=ptat/(ptat*8.0+f64::from(raw[768]))*262144.0;
        assert_eq!((art-f64::from(ee[0x31]))/(f64::from(ee[0x32]&1023)/8.0)+25.0,25.0);
        assert_eq!(s.status&8,0);s.advance(s.next-1);assert_eq!(s.frames,0);s.advance(s.next);assert_eq!(s.status&9,9);s.put(0x8000,0x30);assert_eq!(s.status&9,1);s.advance(s.next);assert_eq!(s.status&9,8);
    }
}
