//! SHTC3 timed no-stretch digital subset; Sensirion Version 4 (2022-12), §§3.1,5.1–5.11.
//! Explicit virtual environment, latched at measurement start; no analog response or calibration.
use super::I2cDevice;
use std::sync::{Arc,Mutex};

pub const SHTC3_ADDRESS:u8=0x70;
// Table15 specifies only bits11/5:0. Choose zero for every unspecified bit;
// this virtual product ID is not a measured device identity or serial number.
const VIRTUAL_PRODUCT_ID:u16=0x0807;
#[derive(Clone,Copy,PartialEq,Eq)]enum Phase{Off,Starting,Idle,Sleeping,Measuring}
pub struct Shtc3State {
    hz:u64,cycle:u64,phase:Phase,until:Option<u64>,temperature_c:f64,humidity_rh:f64,
    pending:bool,reply:[u8;6],reply_len:usize,read_at:usize,reading:bool,writing:bool,high:Option<u8>,
}
impl Shtc3State {
    /// Nominal virtual 25°C / 50%RH. Deadlines use the datasheet maximums:
    /// startup/wake/reset 240us, normal conversion 12.1ms, low-power 0.8ms.
    /// A zero caller rate is bounded to 1Hz; even that rate waits one full cycle.
    pub fn new(hz:u64)->Self {
        let mut s=Self{hz:hz.max(1),cycle:0,phase:Phase::Off,until:None,temperature_c:25.0,humidity_rh:50.0,
            pending:false,reply:[0xff;6],reply_len:0,read_at:0,reading:false,writing:false,high:None};
        s.set_power(true);s
    }
    fn wait(&mut self,phase:Phase,us:u64){
        self.phase=phase;let delay=(u128::from(self.hz)*u128::from(us)).div_ceil(1_000_000)as u64;
        // An unrepresentable future remains busy, rather than becoming ready now.
        self.until=self.cycle.checked_add(delay);
    }
    pub fn advance_to(&mut self,cycle:u64){
        self.cycle=self.cycle.max(cycle);
        if matches!(self.phase,Phase::Starting|Phase::Measuring)&&self.until.is_some_and(|end|self.cycle>=end){self.phase=Phase::Idle;self.until=None;}
    }
    /// Ambient stimulus; it does not change an in-flight or completed sample.
    pub fn set_environment(&mut self,temperature_c:f64,humidity_rh:f64)->bool {
        if !temperature_c.is_finite()||!humidity_rh.is_finite()||!(-40.0..=125.0).contains(&temperature_c)||!(0.0..=100.0).contains(&humidity_rh){return false;}
        self.temperature_c=temperature_c;self.humidity_rh=humidity_rh;true
    }
    fn clear_wire(&mut self){self.pending=false;self.reply=[0xff;6];self.reply_len=0;self.read_at=0;self.reading=false;self.writing=false;self.high=None;}
    /// Only a physical supply edge resets the sensor. Controller/CPU resets
    /// preserve this powered object and its guest-time conversion deadline.
    pub fn set_power(&mut self,powered:bool){
        if powered==(self.phase!=Phase::Off){return;}
        self.clear_wire();if powered{self.wait(Phase::Starting,240);}else{self.phase=Phase::Off;self.until=None;}
    }
    fn crc(bytes:[u8;2])->u8{let mut crc=0xffu8;for b in bytes{crc^=b;for _ in 0..8{crc=if crc&0x80!=0{(crc<<1)^0x31}else{crc<<1};}}crc}
    fn word(&mut self,at:usize,word:u16){let b=word.to_be_bytes();self.reply[at..at+3].copy_from_slice(&[b[0],b[1],Self::crc(b)]);}
    fn start(&mut self,read:bool)->bool{
        self.reading=false;self.writing=false;self.high=None;
        if matches!(self.phase,Phase::Off|Phase::Starting|Phase::Measuring){return false;}
        if read {
            if self.phase!=Phase::Idle||!self.pending{return false;}
            // This bounded subset consumes the response on an accepted read
            // header. Retrying a result without a new command is not modeled.
            self.pending=false;self.reading=true;self.read_at=0;
        }else{self.pending=false;self.reply_len=0;self.writing=true;}
        true
    }
    fn write(&mut self,byte:u8)->bool{
        if !self.writing{return false;}
        let Some(high)=self.high.take()else{
            let allowed=if self.phase==Phase::Sleeping{byte==0x35}else{matches!(byte,0x35|0xb0|0x80|0xef|0x78|0x58|0x60|0x40)};
            if allowed{self.high=Some(byte);}else{self.writing=false;}return allowed;
        };
        self.writing=false;let command=u16::from_be_bytes([high,byte]);
        if self.phase==Phase::Sleeping&&command!=0x3517{return false;}
        match command {
            0x3517=>self.wait(Phase::Starting,240),
            0xb098=>{self.phase=Phase::Sleeping;self.until=None;},
            0x805d=>{self.clear_wire();self.wait(Phase::Starting,240);},
            0xefc8=>{self.word(0,VIRTUAL_PRODUCT_ID);self.reply_len=3;self.pending=true;},
            0x7866|0x58e0|0x609c|0x401a=>{
                // §5.11 uses 2^16, unlike the fixed factory driver's 65535.
                // Saturation at the code endpoints is explicit quantization.
                let t=((self.temperature_c+45.0)*65536.0/175.0).round().clamp(0.0,65535.0)as u16;
                let h=(self.humidity_rh*65536.0/100.0).round().clamp(0.0,65535.0)as u16;
                let words=if matches!(command,0x58e0|0x401a){[h,t]}else{[t,h]};self.word(0,words[0]);self.word(3,words[1]);
                self.reply_len=6;self.pending=true;self.wait(Phase::Measuring,if matches!(command,0x609c|0x401a){800}else{12100});
            },
            // Stretch commands require a line-hold/ACK API this trait does not
            // provide. Unknown commands never establish a completed response.
            _=>return false,
        }true
    }
    fn read(&mut self)->u8{
        if !self.reading||self.phase!=Phase::Idle||self.read_at>=self.reply_len{return 0xff;}
        let b=self.reply[self.read_at];self.read_at+=1;b
    }
    fn stop(&mut self){self.reading=false;self.writing=false;self.high=None;}
}
pub struct Shtc3{state:Arc<Mutex<Shtc3State>>}
impl Shtc3{pub fn new(state:Arc<Mutex<Shtc3State>>)->Self{Self{state}}}
impl I2cDevice for Shtc3 {
    fn start(&mut self,read:bool)->bool{self.state.lock().expect("SHTC3 state poisoned").start(read)}
    fn write(&mut self,byte:u8)->bool{self.state.lock().expect("SHTC3 state poisoned").write(byte)}
    fn read(&mut self)->u8{self.state.lock().expect("SHTC3 state poisoned").read()}
    fn stop(&mut self){self.state.lock().expect("SHTC3 state poisoned").stop();}
}

#[cfg(test)]mod tests{
    use super::*;
    fn setup()->(Arc<Mutex<Shtc3State>>,Shtc3){let s=Arc::new(Mutex::new(Shtc3State::new(1_000_000)));(s.clone(),Shtc3::new(s))}
    fn at(s:&Arc<Mutex<Shtc3State>>,cycle:u64){s.lock().unwrap().advance_to(cycle);}
    fn command(d:&mut Shtc3,command:u16)->bool{if !d.start(false){return false;}let [hi,lo]=command.to_be_bytes();let ack=d.write(hi)&&d.write(lo);d.stop();ack}
    fn read(d:&mut Shtc3,len:usize)->Option<Vec<u8>>{if !d.start(true){return None;}let bytes=(0..len).map(|_|d.read()).collect();d.stop();Some(bytes)}
    // Independently expressed bitwise remainder for the datasheet polynomial.
    fn crc(bytes:&[u8])->u8{let mut rem=0xffu16;for &b in bytes{rem^=u16::from(b);for _ in 0..8{rem<<=1;if rem&0x100!=0{rem^=0x131;}}}rem as u8}
    #[test]fn startup_wake_sleep_and_idle_reset_use_guest_time(){
        let(s,mut d)=setup();assert!(!d.start(false));at(&s,239);assert!(!d.start(false));at(&s,240);
        assert!(command(&mut d,0xb098));assert!(!d.start(true));assert!(!command(&mut d,0xefc8));assert!(command(&mut d,0x3517));
        assert!(!d.start(false));at(&s,479);assert!(!d.start(false));at(&s,480);assert!(command(&mut d,0xefc8));assert!(read(&mut d,3).is_some());
        assert!(command(&mut d,0x805d));at(&s,719);assert!(!d.start(false));at(&s,720);assert!(command(&mut d,0xefc8));
    }
    #[test]fn id_crc_partial_stop_and_repeated_start_are_bounded(){
        assert_eq!(crc(&[0xbe,0xef]),0x92);assert_eq!(Shtc3State::crc([0xbe,0xef]),0x92);
        let(s,mut d)=setup();at(&s,240);assert!(d.start_address(0x70,0x70,false));assert!(d.write(0xef));assert!(d.write(0xc8));
        assert_eq!(read(&mut d,2),Some(vec![0x08,0x07]));assert!(!d.start(true),"result reread is outside the subset");
        assert!(command(&mut d,0xefc8));assert_eq!(read(&mut d,4),Some(vec![8,7,crc(&[8,7]),0xff]));
        assert!(!d.start_address(0x70,0x71,false));assert!(!d.start_address(0x70,0,false));
    }
    #[test]fn four_no_stretch_commands_observe_busy_both_directions_and_word_order(){
        for(cmd,delay,rh_first)in[(0x7866,12100,false),(0x58e0,12100,true),(0x609c,800,false),(0x401a,800,true)]{
            let(s,mut d)=setup();at(&s,240);assert!(command(&mut d,cmd));assert!(!d.start(false));assert!(!d.start(true));
            at(&s,240+delay-1);assert!(!d.start(false));assert!(!d.start(true));at(&s,240+delay);
            let bytes=read(&mut d,7).unwrap();let words=if rh_first{[0x8000u16,0x6666]}else{[0x6666,0x8000]};
            for(i,word)in words.into_iter().enumerate(){assert_eq!(&bytes[i*3..i*3+2],word.to_be_bytes());assert_eq!(bytes[i*3+2],crc(&bytes[i*3..i*3+2]));}assert_eq!(bytes[6],0xff);
        }
    }
    #[test]fn malformed_half_overlong_and_stretch_commands_cannot_create_ready(){
        let(s,mut d)=setup();at(&s,240);
        for cmd in[0,0x3516,0xefc9,0x7867,0x7ca2,0x5c24,0x6458,0x44de,0xffff]{assert!(!command(&mut d,cmd));assert!(!d.start(true));}
        assert!(d.start(false));assert!(d.write(0xef));d.stop();assert!(!d.start(true));
        assert!(d.start(false));assert!(d.write(0x78));assert!(!d.start(true));assert!(!d.write(0x66));assert!(!d.start(true));
        assert!(d.start(false));assert!(d.write(0xef));assert!(d.write(0xc8));assert!(!d.write(0));d.stop();assert!(read(&mut d,3).is_some(),"the complete preceding ID command already executed");
        assert!(!d.write(0xef));assert_eq!(d.read(),0xff);
    }
    #[test]fn environment_is_validated_and_latched_at_measurement_start(){
        let(s,mut d)=setup();at(&s,240);assert!(command(&mut d,0x7866));
        {let mut v=s.lock().unwrap();assert!(v.set_environment(40.0,75.0));for(t,h)in[(f64::NAN,50.0),(25.0,f64::INFINITY),(-41.0,0.0),(126.0,0.0),(0.0,-1.0),(0.0,101.0)]{assert!(!v.set_environment(t,h));}assert_eq!((v.temperature_c,v.humidity_rh),(40.0,75.0));}
        at(&s,12340);let first=read(&mut d,6).unwrap();assert_eq!(&first[..2],&[0x66,0x66]);assert_eq!(&first[3..5],&[0x80,0]);
        assert!(command(&mut d,0x7866));at(&s,24440);let second=read(&mut d,6).unwrap();assert_eq!(&second[..2],&[0x7c,0x58]);assert_eq!(&second[3..5],&[0xc0,0]);
        for(t,h)in[(-40.0,0.0),(125.0,100.0)]{s.lock().unwrap().set_environment(t,h);assert!(command(&mut d,0x609c));let end=s.lock().unwrap().cycle+800;at(&s,end);let b=read(&mut d,6).unwrap();let st=u16::from_be_bytes([b[0],b[1]]);let sh=u16::from_be_bytes([b[3],b[4]]);assert!((-45.0+175.0*f64::from(st)/65536.0-t).abs()<0.003);assert!((100.0*f64::from(sh)/65536.0-h).abs()<0.002);}
    }
    #[test]fn power_edges_cancel_measurements_but_repeated_power_on_does_not(){
        let(s,mut d)=setup();at(&s,240);assert!(command(&mut d,0x7866));at(&s,1000);s.lock().unwrap().set_power(true);at(&s,12340);assert!(read(&mut d,6).is_some());
        assert!(command(&mut d,0x7866));s.lock().unwrap().set_power(false);assert!(!d.start(false));at(&s,1_000_000);assert!(!d.start(true));s.lock().unwrap().set_power(true);
        at(&s,1_000_239);assert!(!d.start(false));at(&s,1_000_240);assert!(!d.start(true));assert!(command(&mut d,0xefc8));
        assert!(d.start(false));assert!(d.write(0xef));{let mut v=s.lock().unwrap();v.set_power(false);v.set_power(true);}at(&s,1_000_480);assert!(!d.write(0xc8));assert!(!d.start(true));
    }
    #[test]fn monotonic_clock_rounding_and_unrepresentable_future_never_panic_or_finish_early(){
        let(s,mut d)=setup();at(&s,240);at(&s,0);assert_eq!(s.lock().unwrap().cycle,240);assert!(command(&mut d,0x7866));at(&s,u64::MAX);assert!(read(&mut d,6).is_some());
        assert!(command(&mut d,0x7866));at(&s,u64::MAX);assert!(!d.start(true));assert!(!d.start(false));
        let s=Arc::new(Mutex::new(Shtc3State::new(1)));let mut d=Shtc3::new(s.clone());assert!(!d.start(false));at(&s,1);assert!(command(&mut d,0x609c));assert!(!d.start(true));at(&s,2);assert!(read(&mut d,6).is_some());
    }
}
