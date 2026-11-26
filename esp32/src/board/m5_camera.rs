//! M5Stack classic-ESP32 camera wiring, selected by exact SKU/revision.
//! Source receipts and revision choices: docs/platform/research/m5-classic-camera-runtime.json.
//! Reuses the native classic I2S/DVP/DMA adapter; no display or synthetic network video.
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiQspiPins},camera::{HostFrameSource,Ov2640,Ov2640State,Ov3660,Ov3660State},picture::{Picture,PixelFormat,Jpeg,JpegError},pins::PinRoutes,w5500::W5500};
use std::sync::{Arc,Mutex};
use super::aithinker::{FRAME_CYCLES,VSYNC_CYCLES};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SensorKind {Ov2640,Ov3660}
#[derive(Clone,Copy)]
pub struct Profile {
    pub name:&'static str,pub sensor:SensorKind,pub sda:u8,pub scl:u8,
    /// D0..D7, PCLK, HREF, VSYNC. Sensor D2..D9 is named D0..D7 here.
    pub dvp:[u8;11],pub xclk:u8,pub reset:u8,pub flash_mib:usize,pub psram_mib:Option<usize>,
    pub button:Option<u8>,pub led:Option<u8>,
}
const STANDARD:[u8;11]=[32,35,34,5,39,18,36,19,21,26,22];
const BASE:Profile=Profile{name:"",sensor:SensorKind::Ov2640,sda:25,scl:23,dvp:STANDARD,xclk:27,reset:15,flash_mib:4,psram_mib:None,button:None,led:Some(14)};
pub const PROFILES:&[Profile]=&[
    // U017's pictured quick-start card is A. B remains a separate selectable
    // board name; never silently probe/swap SIOD and VSYNC at runtime.
    Profile{name:"m5stack-u017",psram_mib:Some(4),..BASE},
    Profile{name:"m5stack-u017-b",sda:22,dvp:[32,35,34,5,39,18,36,19,21,26,25],psram_mib:Some(4),..BASE},
    Profile{name:"m5stack-u017-pcba",sda:22,dvp:[32,35,34,5,39,18,36,19,21,26,25],psram_mib:Some(4),..BASE},
    Profile{name:"m5stack-u038",sda:22,dvp:[32,35,34,5,39,18,36,19,21,26,25],psram_mib:Some(4),..BASE},
    Profile{name:"m5stack-u037",sda:22,dvp:[32,35,34,5,39,18,36,19,21,26,25],psram_mib:Some(4),..BASE},
    // Original U007 has D0 on G17, not the PSRAM camera family's G32.
    Profile{name:"m5stack-u007",dvp:[17,35,34,5,39,18,36,19,21,26,22],led:Some(16),..BASE},
    Profile{name:"m5stack-u109",led:Some(4),..BASE},
    Profile{name:"m5stack-u109-x",led:Some(4),..BASE},
    Profile{name:"m5stack-u109-b",led:Some(4),..BASE},
    Profile{name:"m5stack-u121",sda:14,scl:12,flash_mib:16,psram_mib:Some(8),button:Some(37),led:Some(0),..BASE},
    Profile{name:"m5stack-u121-b",sda:14,scl:12,flash_mib:16,psram_mib:Some(8),button:Some(37),led:Some(0),..BASE},
    Profile{name:"m5stack-u121-b-v11",sensor:SensorKind::Ov3660,sda:14,scl:12,flash_mib:16,psram_mib:Some(8),button:Some(37),led:Some(0),..BASE},
    Profile{name:"m5stack-u082",sensor:SensorKind::Ov3660,psram_mib:Some(8),led:Some(2),..BASE},
    Profile{name:"m5stack-u082-x",sensor:SensorKind::Ov3660,psram_mib:Some(8),led:Some(2),..BASE},
    Profile{name:"m5stack-u082-f",sensor:SensorKind::Ov3660,psram_mib:Some(8),led:Some(2),..BASE},
];
pub fn profile(name:&str)->Option<&'static Profile> {PROFILES.iter().find(|p|p.name==name)}
pub fn memory_mib(name:&str)->Option<(usize,usize)> {let p=profile(name)?;Some((p.flash_mib,p.psram_mib?))}
pub fn make_board(name:&str)->Option<Board> {Some(Box::new(M5Camera::new(profile(name)?)))}

#[derive(Clone)]
enum Sensor {
    Ov2640(Arc<Mutex<Ov2640>>,Arc<Mutex<Ov2640State>>),
    Ov3660(Arc<Mutex<Ov3660>>,Arc<Mutex<Ov3660State>>),
}
struct FrameConfig {width:u32,height:u32,streaming:bool,jpeg:bool,format:Option<PixelFormat>,window:Option<([u32;4],u32,u32)>}
impl Sensor {
    fn new(kind:SensorKind)->Self {match kind {
        SensorKind::Ov2640=>{let state=Arc::new(Mutex::new(Ov2640State::default()));Self::Ov2640(Arc::new(Mutex::new(Ov2640::new(state.clone()))),state)},
        SensorKind::Ov3660=>{let state=Arc::new(Mutex::new(Ov3660State::default()));Self::Ov3660(Arc::new(Mutex::new(Ov3660::new(state.clone()))),state)},
    }}
    fn hardware_reset(&self) {match self {Self::Ov2640(device,state)=>*device.lock().unwrap()=Ov2640::new(state.clone()),Self::Ov3660(device,_)=>device.lock().unwrap().reset()}}
    fn config(&self)->FrameConfig {match self {
        Self::Ov2640(_,state)=>{let s=*state.lock().unwrap();FrameConfig{width:s.width,height:s.height,streaming:s.streaming,jpeg:s.image_mode&!2==0x10,format:match s.image_mode {0=>Some(PixelFormat::Yuyv),8=>Some(PixelFormat::Rgb565),_=>None},window:None}},
        Self::Ov3660(_,state)=>{let s=*state.lock().unwrap();FrameConfig{width:s.width,height:s.height,streaming:s.streaming,jpeg:s.jpeg,format:match s.format {0x61=>Some(PixelFormat::Rgb565),0x30=>Some(PixelFormat::Yuyv),_=>None},window:s.window.map(|w|(w,2080,1548))}},
    }}
}
struct Sccb {sensor:Sensor,pins:(u8,u8),released:Arc<Mutex<bool>>,supply:Arc<Mutex<bool>>}
impl I2cDevice for Sccb {
    fn pins(&self)->Option<(u8,u8)> {Some(self.pins)}
    fn start(&mut self,read:bool)->bool {if !*self.released.lock().unwrap()||!*self.supply.lock().unwrap(){return false;}match &self.sensor {Sensor::Ov2640(d,_)=>d.lock().unwrap().start(read),Sensor::Ov3660(d,_)=>d.lock().unwrap().start(read)}}
    fn write(&mut self,b:u8)->bool {if !*self.released.lock().unwrap()||!*self.supply.lock().unwrap(){return false;}match &self.sensor {Sensor::Ov2640(d,_)=>d.lock().unwrap().write(b),Sensor::Ov3660(d,_)=>d.lock().unwrap().write(b)}}
    fn read(&mut self)->u8 {if !*self.released.lock().unwrap()||!*self.supply.lock().unwrap(){return 255;}match &self.sensor {Sensor::Ov2640(d,_)=>d.lock().unwrap().read(),Sensor::Ov3660(d,_)=>d.lock().unwrap().read()}}
    fn stop(&mut self) {match &self.sensor {Sensor::Ov2640(d,_)=>d.lock().unwrap().stop(),Sensor::Ov3660(d,_)=>d.lock().unwrap().stop()}}
}
pub struct M5Camera {
    profile:&'static Profile,sensor:Sensor,source:HostFrameSource,released:Arc<Mutex<bool>>,supply:Arc<Mutex<bool>>,
    now:u64,next:Option<u64>,vsync:bool,edges:Vec<BoardEdge>,frame_start:Option<u64>,
    ethernet:Option<W5500>,ethernet_irq:bool,ethernet_software_cs:bool,ethernet_next:Option<u64>,
}
impl M5Camera {
    pub fn new(profile:&'static Profile)->Self {Self{profile,sensor:Sensor::new(profile.sensor),source:HostFrameSource::default(),released:Arc::new(Mutex::new(true)),supply:Arc::new(Mutex::new(true)),now:0,next:None,vsync:false,edges:Vec::new(),frame_start:None,ethernet:profile.name.starts_with("m5stack-u121").then(W5500::new),ethernet_irq:false,ethernet_software_cs:false,ethernet_next:None}}
    fn clock_live(&self,clock:&dyn Fn(u8)->Option<(f64,u32)>)->bool {clock(self.profile.xclk).is_some_and(|(hz,duty)|hz.is_finite()&&hz>0.0&&duty>0&&duty<65535)}
    fn stop_frame(&mut self,cycle:u64) {self.next=None;self.frame_start=None;if self.vsync {self.vsync=false;self.edges.push(BoardEdge{cycle,pin:self.profile.dvp[10],level:false});}self.now=cycle;}
    fn update_ethernet(&mut self,cycle:u64) {if let Some(chip)=&self.ethernet {let irq=chip.irq_asserted();if irq!=self.ethernet_irq {self.edges.push(BoardEdge{cycle,pin:2,level:!irq});self.ethernet_irq=irq;}self.ethernet_next=chip.macraw_open().then(||cycle.saturating_add(crate::periph::CPU_HZ/1000));}}
    fn poll_ethernet(&mut self,cycle:u64) {if self.ethernet_next.is_some_and(|at|at<=cycle){if let Some(chip)=&mut self.ethernet {chip.poll(cycle/(crate::periph::CPU_HZ/1_000_000));}self.update_ethernet(cycle);}}
}
impl BoardModel for M5Camera {
    fn name(&self)->&'static str {self.profile.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>) {
        // RESET is active low. Undriven RESET uses the board's pull-up policy;
        // PWDN is tied low/no MCU wire, never reinterpret -1 as G0/G255.
        let released=routes.software_output(self.profile.reset)!=Some(false);
        let previous=*self.released.lock().unwrap();
        if previous&&!released {self.sensor.hardware_reset();self.stop_frame(cycle);}
        *self.released.lock().unwrap()=released;
        if let Some(chip)=&mut self.ethernet {let selected=routes.software_output(4)==Some(false);if self.ethernet_software_cs&&!selected {chip.end_transaction();}self.ethernet_software_cs=selected;}
    }
    fn gpio_output_at(&mut self,_cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {if changes.iter().any(|&(pin,level)|pin==4&&level){if let Some(chip)=&mut self.ethernet{chip.end_transaction();}}}
    fn uses_qspi_pins(&self)->bool {self.ethernet.is_some()}
    fn spi_transfer_qspi_pins(&mut self,_host:u8,pins:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        let Some(chip)=&mut self.ethernet else{return vec![255;rx_len];};
        let widths=[pins.command_width,pins.address_width,pins.data_width,pins.read_width];
        let valid=!pins.unsupported_mode&&!pins.sio&&pins.dummy_cycles==0&&!pins.clock_output_edge&&widths.iter().all(|&w|w<=1)&&pins.command_bits.is_multiple_of(8)&&pins.address_bits.is_multiple_of(8)&&pins.data_bits.is_multiple_of(8)&&pins.read_bits.is_multiple_of(8)
            &&pins.sclk&(1<<23)!=0&&pins.data[0]&(1<<13)!=0&&pins.cs&(1<<4)!=0;
        if !valid {chip.end_transaction();return vec![255;rx_len];}
        let prefix=usize::from(pins.command_bits/8)+usize::from(pins.address_bits/8);
        let bytes=chip.transfer(tx,rx_len,prefix,pins.full_duplex,self.now/(crate::periph::CPU_HZ/1_000_000));
        if !pins.keep_cs&&pins.software_cs&(1<<4)==0 {chip.end_transaction();}
        self.update_ethernet(self.now);
        if pins.input[1]==Some(38){bytes}else{vec![255;rx_len]}
    }
    fn ethernet_link(&mut self)->Option<&mut esp_soc::ethernet::EthernetLink> {self.ethernet.as_mut().map(|chip|&mut chip.link)}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut p=vec![(self.profile.dvp[10],self.vsync)];if let Some(b)=self.profile.button {p.push((b,true));}
        if self.ethernet.is_some(){p.push((2,!self.ethernet_irq));}p}
    fn named_pin(&self,id:&str)->Option<u8> {if matches!(id,"button"|"btnA"|"button-user"){self.profile.button}else{None}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let address=if self.profile.sensor==SensorKind::Ov2640 {0x30}else{0x3c};
        (0..2).map(|port|(port,address,Box::new(Sccb{sensor:self.sensor.clone(),pins:(self.profile.sda,self.profile.scl),released:self.released.clone(),supply:self.supply.clone()})as Box<dyn I2cDevice>)).collect()
    }
    fn camera_dvp_pins(&self)->Option<[u8;11]> {Some(self.profile.dvp)}
    fn next_deadline(&self)->Option<u64> {self.next.into_iter().chain(self.ethernet_next).min()}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn camera_frame_start(&mut self,claim:bool)->Option<u64> {if claim{self.frame_start.take()}else{self.frame_start}}
    fn invalidate_camera_frame_start(&mut self) {self.frame_start=None;}
    fn advance_to(&mut self,cycle:u64) {
        if cycle<self.now {return;}
        self.poll_ethernet(cycle);
        while let Some(at)=self.next.filter(|&at|at<=cycle) {
            self.vsync= !self.vsync;if !self.vsync{self.frame_start=Some(at);}
            self.edges.push(BoardEdge{cycle:at,pin:self.profile.dvp[10],level:self.vsync});
            self.next=at.checked_add(if self.vsync{VSYNC_CYCLES}else{FRAME_CYCLES-VSYNC_CYCLES});
        }self.now=cycle;
    }
    fn advance_inputs_with_clock(&mut self,cycle:u64,clock:&dyn Fn(u8)->Option<(f64,u32)>) {
        if cycle<self.now{return;}
        self.poll_ethernet(cycle);
        if *self.supply.lock().unwrap()&&*self.released.lock().unwrap()&&self.sensor.config().streaming&&self.clock_live(clock) {if self.next.is_none(){self.next=cycle.checked_add(FRAME_CYCLES);}self.advance_to(cycle);}else{self.stop_frame(cycle);}
    }
    fn set_camera_picture(&mut self,p:Picture)->Result<(),&'static str> {self.source.set_camera_picture(p)}
    fn camera_preview(&self,w:u32,h:u32)->Option<Vec<u8>> {self.source.camera_preview(w,h)}
    fn set_camera_jpeg(&mut self,jpeg:Jpeg)->Result<(),JpegError> {let c=self.sensor.config();self.source.set_camera_jpeg(jpeg,(c.width,c.height))}
    fn camera_frame_is_jpeg(&self)->bool {self.sensor.config().jpeg}
    fn camera_frame_with_clock(&mut self,clock:&dyn Fn(u8)->Option<(f64,u32)>)->Option<(u32,u32,Arc<Vec<u8>>)> {
        let c=self.sensor.config();
        if !*self.supply.lock().unwrap()||!*self.released.lock().unwrap()||!c.streaming||!self.clock_live(clock){return None;}
        if c.jpeg {self.source.jpeg_frame(c.width,c.height)}else{self.source.frame(c.width,c.height,c.format?,c.window)}
    }
    fn report(&self)->String {format!("[{}] {:?} SCCB G{}/G{}, XCLK G{}, RESET G{}, native I2S0 DVP/DMA; host RGB565/YUV/JPEG bytes; external supply digital subset; W5500 MACRAW={}. Optics/ISP/RF/battery/RTC/TCP-offload/factory compatibility unverified",self.profile.name,self.profile.sensor,self.profile.sda,self.profile.scl,self.profile.xclk,self.profile.reset,self.ethernet.is_some())}
    fn camera_supply_at(&mut self,cycle:u64,present:bool)->bool {let old=*self.supply.lock().unwrap();if old&&!present {self.sensor.hardware_reset();self.stop_frame(cycle);}*self.supply.lock().unwrap()=present;true}
}
