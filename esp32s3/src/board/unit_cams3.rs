//! Unit CamS3 U174 (OV2640) / U174-B (PY260), exact common PCB wiring.
//! M5Stack UnitCamS3-UserDemo 3913576d camera_pins.h and 50410249 hal_config.h.
//! DVP transfers explicit host frames through LCD_CAM/GDMA. No image encoder,
//! optical/ISP emulation, PDM microphone, USB UVC or network services.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},camera::{Ov2640,Ov2640State,Py260,Py260State,HostFrameSource},picture::{Picture,PixelFormat,Jpeg,JpegError},pins::PinRoutes,devices::i2c_bitbang::I2cBitBang};
use std::sync::{Arc,Mutex};
enum Sensor {Ov(Arc<Mutex<Ov2640>>),Py(Arc<Mutex<Py260>>)}
impl Sensor {fn reset(&mut self,ov:&Arc<Mutex<Ov2640State>>){match self{Self::Ov(s)=>{*s.lock().unwrap()=Ov2640::new(ov.clone());},Self::Py(s)=>s.lock().unwrap().reset()}}}
struct Sccb {sensor:Arc<Mutex<Sensor>>,ready:Arc<Mutex<bool>>}
impl I2cDevice for Sccb {
    fn start(&mut self,r:bool)->bool {if !*self.ready.lock().unwrap(){return false;}match &mut*self.sensor.lock().unwrap(){Sensor::Ov(s)=>s.lock().unwrap().start(r),Sensor::Py(s)=>s.lock().unwrap().start(r)}}
    fn write(&mut self,b:u8)->bool {if !*self.ready.lock().unwrap(){return false;}match &mut*self.sensor.lock().unwrap(){Sensor::Ov(s)=>s.lock().unwrap().write(b),Sensor::Py(s)=>s.lock().unwrap().write(b)}}
    fn read(&mut self)->u8 {if !*self.ready.lock().unwrap(){return 255;}match &mut*self.sensor.lock().unwrap(){Sensor::Ov(s)=>s.lock().unwrap().read(),Sensor::Py(s)=>s.lock().unwrap().read()}}
}
pub struct UnitCamS3 {five:bool,ov:Arc<Mutex<Ov2640State>>,py:Arc<Mutex<Py260State>>,sensor:Arc<Mutex<Sensor>>,ready:Arc<Mutex<bool>>,i2c:I2cBitBang,source:HostFrameSource,reset:bool,levels:[bool;2],edges:Vec<BoardEdge>,led:[[u8;3];1],version:u64,sd:Option<Box<SpiSdCard>>,present:bool,cs:Option<bool>}
impl UnitCamS3 {
    pub fn new(five:bool)->Self {let ov=Arc::new(Mutex::new(Ov2640State::default()));let py=Arc::new(Mutex::new(Py260State::default()));let sensor=if five{Sensor::Py(Arc::new(Mutex::new(Py260::new(py.clone()))))}else{Sensor::Ov(Arc::new(Mutex::new(Ov2640::new(ov.clone()))))};Self{five,ov,py,sensor:Arc::new(Mutex::new(sensor)),ready:Arc::new(Mutex::new(true)),i2c:I2cBitBang::new(),source:Default::default(),reset:true,levels:[true;2],edges:vec![],led:[[0;3]],version:0,sd:None,present:false,cs:None}}
    fn config(&self)->(u32,u32,u8,bool) {if self.five {let s=*self.py.lock().unwrap();(s.width,s.height,s.format,s.streaming)}else{let s=*self.ov.lock().unwrap();(s.width,s.height,s.image_mode,s.streaming)}}
}
impl BoardModel for UnitCamS3 {
    fn name(&self)->&'static str {if self.five{"unit-cams3-5mp"}else{"unit-cams3"}}
    fn uses_gpio_edges(&self)->bool {false}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>) {
        // PWDN is physically grounded. A floating RESET uses the ideal model's
        // deasserted level; reset RC, rail ramp and startup delays are unmodeled.
        let reset=r.software_output(21)!=Some(false);if self.reset&&!reset{self.sensor.lock().unwrap().reset(&self.ov);}self.reset=reset;*self.ready.lock().unwrap()=reset;
        let scl=r.software_output(41)!=Some(false);let sda=self.i2c.drive(scl,r.software_output(17)!=Some(false));for(i,(pin,level))in[(17,sda),(41,scl)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        let led=if r.software_output(14)==Some(false){[0,0,255]}else{[0;3]};if self.led[0]!=led{self.led[0]=led;self.version=self.version.wrapping_add(1);}
        let cs=r.software_output(9);if self.cs==Some(false)&&cs!=Some(false){if let Some(sd)=&mut self.sd{sd.deselect();}}self.cs=cs;
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.sensor.lock().unwrap().reset(&self.ov);self.i2c.clear();self.levels=[true;2];self.reset=true;*self.ready.lock().unwrap()=true;
        self.i2c.attach(if self.five{0x1f}else{0x30},Box::new(Sccb{sensor:self.sensor.clone(),ready:self.ready.clone()}));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((17,41))).into_iter().map(move|(address,d)|(bus,address,d))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(17,self.levels[0]),(41,self.levels[1])]}
    fn named_pin(&self,id:&str)->Option<u8> {(id=="boot").then_some(0)}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.led,self.version))}
    fn camera_dvp_pins(&self)->Option<[u8;11]> {Some([6,15,16,7,5,10,4,13,12,18,42])}
    fn set_camera_picture(&mut self,p:Picture)->Result<(),&'static str> {self.source.set_camera_picture(p)}
    fn camera_preview(&self,w:u32,h:u32)->Option<Vec<u8>> {self.source.camera_preview(w,h)}
    fn camera_frame_with_clock(&mut self,clock:&dyn Fn(u8)->Option<(f64,u32)>)->Option<(u32,u32,Arc<Vec<u8>>)> {
        let(w,h,format,streaming)=self.config();let(hz,duty)=clock(11)?;if !self.reset||!streaming||!hz.is_finite()||hz<=0.||duty==0||duty>=65535{return None;}
        if self.camera_frame_is_jpeg(){return self.source.jpeg_frame(w,h);}
        let fmt=if self.five{match format{0x10|2=>PixelFormat::Rgb565,0x11|3=>PixelFormat::Yuyv,_=>return None}}else{match format{8=>PixelFormat::Rgb565,0=>PixelFormat::Yuyv,_=>return None}};
        self.source.frame(w,h,fmt,None)
    }
    fn camera_frame_is_jpeg(&self)->bool {let(_,_,format,_)=self.config();if self.five{format==1}else{format&!2==0x10}}
    fn set_camera_jpeg(&mut self,j:Jpeg)->Result<(),JpegError> {let(w,h,_,_)=self.config();self.source.set_camera_jpeg(j,(w,h))}
    fn uses_qspi_pins(&self)->bool {true}
    fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8> {
        if p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge||p.command_bits!=0||p.address_bits!=0||p.dummy_cycles!=0||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|w|*w>1)||p.sclk&(1<<39)==0||p.data[0]&(1<<38)==0||!self.present{return vec![255;rx];}
        if let Some(sd)=&mut self.sd{let bytes=sd.transfer(p.cs&(1<<9)!=0,tx,rx);if !p.keep_cs&&p.software_cs&(1<<9)==0{sd.deselect();}
            if p.input[1]==Some(40){return bytes;}}vec![255;rx]
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,cfg:SdCardConfig)->Result<String,String> {let card=SpiSdCard::new(image,cfg)?;let description=card.describe();self.sd=Some(Box::new(card));self.present=true;Ok(description)}
    fn set_sd_card_present(&mut self,present:bool) {if self.present!=present{if let Some(sd)=&mut self.sd{sd.power_cycle();}}self.present=present;}
    fn report(&self)->String {format!("{} sensor={} SCCB17/41 reset21 XCLK11 DVP=[6,15,16,7,5,10,4,13,12,18,42] host RAW/JPEG via LCD_CAM/GDMA; SD SPI39/38/40 CS9; LED14 active-low blue; PWDN=GND. Approximate 10Hz frame/blanking schedule; no optical ISP/JPEG encoder, microphone/PDM, UVC or networking. Factory image compatibility and hardware unverified.",self.name(),if self.five{"PY260/MEGA_CCM"}else{"OV2640"})}
}
