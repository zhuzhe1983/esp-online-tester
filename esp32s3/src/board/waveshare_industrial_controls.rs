//! Source-bounded Audio Board and DIN controller digital subsets.
//! Exact sources/ZIP member hashes are retained with batch 02 research.
//! Native SD, control registers, RTC, GPIO and indicator light transport only;
//! no synthetic network, CAN/RS485 peer, audio samples or voltage/load physics.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,tca9555::{Tca9555,Tca9555State}};
use esp_soc::{board::{Board,BoardModel,BoardEdge},pins::PinRoutes,devices::{
    Ws2812Chain,i2c_bitbang::I2cBitBang,pcf85063::{Pcf85063,Pcf85063State},
    es8311::Es8311,es7210::Es7210,tca9554::{Tca9554,Tca9554State}}};
const HZ:u64=240_000_000;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Profile {Audio,AudioEn,Rs485Can,Rs485CanU,Relay1,Relay1U,PoeDiDo,EthDiRoC,PoeDiRoC,EthDiRo,PoeDiRo}
impl Profile {
    pub const ALL:[Self;11]=[Self::Audio,Self::AudioEn,Self::Rs485Can,Self::Rs485CanU,Self::Relay1,Self::Relay1U,Self::PoeDiDo,Self::EthDiRoC,Self::PoeDiRoC,Self::EthDiRo,Self::PoeDiRo];
    pub fn name(self)->&'static str {match self {Self::Audio=>"waveshare-s3-audio-board",Self::AudioEn=>"waveshare-s3-audio-board-en",Self::Rs485Can=>"waveshare-s3-rs485-can",Self::Rs485CanU=>"waveshare-s3-rs485-can-u",Self::Relay1=>"waveshare-s3-relay-1ch",Self::Relay1U=>"waveshare-s3-relay-1ch-u",Self::PoeDiDo=>"waveshare-s3-poe-eth-8di-8do",Self::EthDiRoC=>"waveshare-s3-eth-8di-8ro-c",Self::PoeDiRoC=>"waveshare-s3-poe-eth-8di-8ro-c",Self::EthDiRo=>"waveshare-s3-eth-8di-8ro",Self::PoeDiRo=>"waveshare-s3-poe-eth-8di-8ro"}}
    pub fn audio(self)->bool {matches!(self,Self::Audio|Self::AudioEn)}
    pub fn eight(self)->bool {matches!(self,Self::PoeDiDo|Self::EthDiRoC|Self::PoeDiRoC|Self::EthDiRo|Self::PoeDiRo)}
    pub fn relay1(self)->bool {matches!(self,Self::Relay1|Self::Relay1U)}
    pub fn i2c_pins(self)->(u8,u8) {if self.audio(){(11,10)}else if self.eight(){(42,41)}else{(39,38)}}
    pub fn led_count(self)->usize {if self.audio(){7}else if self.eight(){1}else{0}}
    pub fn sd_pins(self)->Option<[u8;6]> {if self.audio(){Some([40,42,41,255,255,255])}else if self.eight(){Some([48,47,45,255,255,255])}else{None}}
}
pub fn profile(name:&str)->Option<Profile> {Profile::ALL.into_iter().find(|p|p.name()==name)}
pub fn memory_mib(name:&str)->Option<(usize,usize)> {profile(name).map(|_|(16,8))}
pub fn make_board(name:&str)->Option<Board> {profile(name).map(|p|Box::new(Controls::new(p))as Board)}
pub struct Controls {profile:Profile,wire:I2cBitBang,attached:bool,levels:[bool;2],edges:Vec<BoardEdge>,
    rtc:Arc<Mutex<Pcf85063State>>,exp8:Arc<Mutex<Tca9554State>>,exp16:Arc<Mutex<Tca9555State>>,
    chain:Ws2812Chain,leds:Vec<[u8;3]>,gpio:[Option<bool>;2],card:bool}
impl Controls {
    pub fn new(profile:Profile)->Self {Self{profile,wire:I2cBitBang::new(),attached:false,levels:[true;2],edges:vec![],rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),exp8:Default::default(),exp16:Default::default(),chain:Ws2812Chain::new(profile.led_count().max(1)).with_gpio_clock(HZ as u32),leds:vec![[0;3];profile.led_count()],gpio:[None;2],card:false}}
    fn sync_leds(&mut self) {for(a,[r,g,b])in self.leds.iter_mut().zip(&self.chain.leds){*a=[*g,*r,*b];}}
}
impl BoardModel for Controls {
    fn name(&self)->&'static str {self.profile.name()}
    fn uses_gpio_edges(&self)->bool {false}
    fn uses_gpio_routes(&self)->bool {true}
    fn uses_gpio_waveform(&self)->bool {!self.leds.is_empty()}
    fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>) {
        self.advance_to(c);let(sp,cp)=self.profile.i2c_pins();let scl=r.software_output(cp)!=Some(false);let sda=self.wire.drive(scl,r.software_output(sp)!=Some(false));
        for(i,(pin,level))in[(sp,sda),(cp,scl)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}
        if !self.leds.is_empty(){let l=r.software_output(38);self.chain.gpio_drive(c,l.is_some(),l.unwrap_or(false));self.sync_leds();}
        self.gpio=[if self.profile.relay1(){r.software_output(47)}else{None},if self.profile.eight(){r.software_output(46)}else{None}];
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        if !self.attached {self.wire.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));
            if self.profile.audio(){self.wire.attach(0x18,Box::new(Es8311::new()));self.wire.attach(0x40,Box::new(Es7210::new()));self.wire.attach(0x20,Box::new(Tca9555::new(self.exp16.clone())));}
            else if self.profile.eight(){self.wire.attach(0x20,Box::new(Tca9554::new(self.exp8.clone())));}self.attached=true;
        }
        (0..2).flat_map(|b|self.wire.controller_devices(Some(self.profile.i2c_pins())).into_iter().map(move|(a,d)|(b,a,d))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {let(sda,scl)=self.profile.i2c_pins();let mut p=vec![(0,true),(sda,self.levels[0]),(scl,self.levels[1])];if self.profile.eight(){p.extend((4..12).map(|p|(p,true)));}p}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"boot"=>Some(0),"di1" if self.profile.eight()=>Some(4),"di2" if self.profile.eight()=>Some(5),"di3" if self.profile.eight()=>Some(6),"di4" if self.profile.eight()=>Some(7),"di5" if self.profile.eight()=>Some(8),"di6" if self.profile.eight()=>Some(9),"di7" if self.profile.eight()=>Some(10),"di8" if self.profile.eight()=>Some(11),_=>None}}
    fn button_at(&mut self,_cycle:u64,id:&str,pressed:bool)->bool {if !self.profile.audio(){return false;}let pin=match id{"key-1"=>9,"key-2"=>10,"key-3"=>11,_=>return false};self.exp16.lock().expect("Audio TCA9555").set_input(pin,!pressed);true}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn advance_to(&mut self,c:u64) {self.rtc.lock().expect("industrial RTC").advance_to(c);if !self.leds.is_empty(){self.chain.advance_gpio(c);self.sync_leds();}}
    fn next_deadline(&self)->Option<u64> {if self.leds.is_empty(){None}else{self.chain.gpio_deadline()}}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==38&&!self.leds.is_empty(){self.chain.from_bits(bits);self.sync_leds();}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {(!self.leds.is_empty()).then_some((&self.leds,self.chain.updates))}
    fn sd_card_slot(&self)->Option<([u8;6],bool)> {self.profile.sd_pins().map(|pins|(pins,self.card))}
    fn set_sd_card_present(&mut self,present:bool) {self.card=present;}
    fn hardware_diagnostics(&self)->Option<String> {
        let boolstr=|v:Option<bool>|v.map_or("null",|v|if v{"true"}else{"false"});
        let exp=self.exp8.lock().expect("industrial TCA9554");let channels=(0..8).map(|p|boolstr(if self.profile.eight(){exp.driven(p)}else{None})).collect::<Vec<_>>().join(",");
        Some(format!("{{\"outputType\":\"{}\",\"channelDriveHigh\":[{}],\"relay1CoilDrive\":{},\"buzzerGpioDrive\":{},\"analogLoadsImplemented\":false,\"networkImplemented\":false,\"audioPcmImplemented\":false}}",if self.profile==Profile::PoeDiDo{"darlington-sink"}else if self.profile.eight(){"relay-coil"}else{"none"},channels,boolstr(self.gpio[0]),boolstr(self.gpio[1])))
    }
    fn report(&self)->String {format!("{}16F8P PCF85063 RTC SDA/SCL={:?}; audio TCA9555/ES8311/ES7210 controls+three expander keys+7 RGB LEDs or industrial TCA9554 eight output controls+GPIO4..11 digital inputs, exact per profile. Relay1 GPIO47, 8ch buzzer GPIO46 and RGB GPIO38. Native SD pins={:?}. RGB wire order follows explicit manufacturer RGB configuration / swapped Arduino neopixelWrite. No W5500 network, CAN/RS485 peer, PCM, ADC/battery/PoE physics, electrical loads, optional display/camera. Factory boot unverified.",self.name(),self.profile.i2c_pins(),self.profile.sd_pins())}
}
