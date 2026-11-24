//! Exact Waveshare S3 industrial/control products, source-bounded digital subset.
//! Sources: docs/research/waveshare-s3-industrial-2026-11-24/*/sources.json.
//! No connected LED strip, CAN/RS485 peer, LoRa peer, camera frame or audio PCM
//! is invented. Connector tracing is observation, never a peripheral reply.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins},pins::PinRoutes,
    devices::{i2c_bitbang::I2cBitBang,pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210}};
const HZ:u64=240_000_000;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Profile {Rs485Wled,Can2,Can2U,CamOv5640,CamOv3660,CamGc2145,CamGc0308,Lr1121Hf,Lr1121HfKit,Lr1121Lf,Lr1121LfKit}
impl Profile {
    pub const ALL:[Self;11]=[Self::Rs485Wled,Self::Can2,Self::Can2U,Self::CamOv5640,Self::CamOv3660,Self::CamGc2145,Self::CamGc0308,Self::Lr1121Hf,Self::Lr1121HfKit,Self::Lr1121Lf,Self::Lr1121LfKit];
    pub fn name(self)->&'static str {match self {Self::Rs485Wled=>"waveshare-s3-rs485-wled",Self::Can2=>"waveshare-s3-can-2ch",Self::Can2U=>"waveshare-s3-can-2ch-u",Self::CamOv5640=>"waveshare-s3-cam-ov5640",Self::CamOv3660=>"waveshare-s3-cam-ov3660",Self::CamGc2145=>"waveshare-s3-cam-gc2145",Self::CamGc0308=>"waveshare-s3-cam-gc0308",Self::Lr1121Hf=>"waveshare-s3-lr1121-hf",Self::Lr1121HfKit=>"waveshare-s3-lr1121-hf-kit",Self::Lr1121Lf=>"waveshare-s3-lr1121-lf",Self::Lr1121LfKit=>"waveshare-s3-lr1121-lf-kit"}}
    pub fn memory_mib(self)->(usize,usize) {if self.radio(){(4,2)}else{(16,8)}}
    pub fn camera(self)->bool {matches!(self,Self::CamOv5640|Self::CamOv3660|Self::CamGc2145|Self::CamGc0308)}
    pub fn radio(self)->bool {matches!(self,Self::Lr1121Hf|Self::Lr1121HfKit|Self::Lr1121Lf|Self::Lr1121LfKit)}
    pub fn boot_button(self)->bool {!matches!(self,Self::Lr1121Hf|Self::Lr1121Lf)}
    pub fn i2c_pins(self)->Option<(u8,u8)> {match self {Self::Rs485Wled=>Some((15,14)),Self::Can2|Self::Can2U=>Some((39,38)),v if v.camera()=>Some((8,7)),_=>None}}
}
pub fn profile(name:&str)->Option<Profile> {Profile::ALL.into_iter().find(|p|p.name()==name)}
pub fn memory_mib(name:&str)->Option<(usize,usize)> {profile(name).map(Profile::memory_mib)}
pub fn make_board(name:&str)->Option<Board> {profile(name).map(|p|Box::new(Industrial::new(p))as Board)}
pub struct Industrial {
    profile:Profile,i2c:I2cBitBang,attached:bool,levels:[bool;2],edges:Vec<BoardEdge>,
    rtc:Arc<Mutex<Pcf85063State>>,outputs:[Option<bool>;4],rmt_bits:[u64;2],rmt_frames:[u64;2],radio_spi_bytes:u64,
}
impl Industrial {pub fn new(profile:Profile)->Self {Self{profile,i2c:I2cBitBang::new(),attached:false,levels:[true;2],edges:vec![],rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),outputs:[None;4],rmt_bits:[0;2],rmt_frames:[0;2],radio_spi_bytes:0}}}
impl BoardModel for Industrial {
    fn name(&self)->&'static str {self.profile.name()}
    fn uses_gpio_edges(&self)->bool {false}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>) {
        self.advance_to(c);
        if let Some((sda_pin,scl_pin))=self.profile.i2c_pins() {let scl=r.software_output(scl_pin)!=Some(false);let sda=self.i2c.drive(scl,r.software_output(sda_pin)!=Some(false));
            for(i,(pin,level))in[(sda_pin,sda),(scl_pin,scl)].into_iter().enumerate() {if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}
        }
        self.outputs=if self.profile==Profile::Rs485Wled {[2,16,3,4].map(|p|r.software_output(p))}else if self.profile.radio(){[39,42,40,45].map(|p|r.software_output(p))}else{[None;4]};
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        if !self.attached {match self.profile {
            Profile::Rs485Wled=>{self.i2c.attach(0x40,Box::new(Es7210::new()));},
            Profile::Can2|Profile::Can2U=>{self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));},
            v if v.camera()=>{self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));},_=>{}
        }self.attached=true;}
        (0..2).flat_map(|b|self.i2c.controller_devices(self.profile.i2c_pins()).into_iter().map(move|(a,d)|(b,a,d))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {let mut p=vec![(0,true)];if self.profile.camera(){p.push((15,true));}
        if let Some((sda,scl))=self.profile.i2c_pins(){p.extend([(sda,self.levels[0]),(scl,self.levels[1])]);}p}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"boot" if self.profile.boot_button()=>Some(0),"power" if self.profile.camera()=>Some(15),_=>None}}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn advance_to(&mut self,c:u64) {if matches!(self.profile,Profile::Can2|Profile::Can2U){self.rtc.lock().expect("CAN2 RTC state").advance_to(c);}}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if self.profile==Profile::Rs485Wled {if let Some(i)=[2,16].into_iter().position(|p|p==pin){self.rmt_bits[i]=self.rmt_bits[i].saturating_add(bits.len()as u64);self.rmt_frames[i]=self.rmt_frames[i].saturating_add(1);}}}
    fn uses_spi_pins(&self)->bool {self.profile.radio()}
    fn spi_transfer_pins(&mut self,_host:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        if self.profile.radio()&&self.outputs[0]==Some(true)&&p.cs&(1<<42)!=0&&p.sclk&(1<<40)!=0&&p.mosi&(1<<45)!=0 {self.radio_spi_bytes=self.radio_spi_bytes.saturating_add(tx.len()as u64);}
        vec![0xff;rx] // Disconnected modeled radio bus: no fabricated chip/status reply.
    }
    fn hardware_diagnostics(&self)->Option<String> {let levels=self.outputs.map(|v|v.map_or("null",|v|if v{"true"}else{"false"})).join(",");Some(format!("{{\"gpioOutputLevels\":[{levels}],\"externalLedRmtFrames\":{:?},\"externalLedRmtBits\":{:?},\"radioSpiObservedBytes\":{},\"attachedLedStrip\":false,\"radioReplyImplemented\":false,\"cameraCaptureImplemented\":false}}",self.rmt_frames,self.rmt_bits,self.radio_spi_bytes))}
    fn report(&self)->String {format!("{} {}F{}P; SDA/SCL={:?}; ES7210/ES8311 control ports or PCF85063 RTC only per exact source; BOOT physical={}; GPIO/RMT/SPI connector observation only; no ES8389/PCM, differential CAN/RS485, XL2515, LR1121 replies/radio, camera SCCB/DVP, CH32 IO/power, camera SD transport or optional LCD. Bare LR module has no USB/physical key; adapter kit adds BOOT. Factory boot unverified.",self.name(),self.profile.memory_mib().0,self.profile.memory_mib().1,self.profile.i2c_pins(),self.profile.boot_button())}
}
