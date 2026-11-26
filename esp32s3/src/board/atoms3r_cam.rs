//! Source-backed AtomS3R camera/extension family. Exact-SKU pins and current scope:
//! docs/platform/atoms3r-family-runtime-20261126.md. Main v0.4.1 / camera daughter schematic:
//! camera SCCB SDA12/SCL9, G18 active-low power; BMI270 SDA45/SCL0. Audio SDA38/SCL39
//! remains absent. Camera variants capture explicit virtual sources via LCD_CAM/GDMA.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample}};
use esp_soc::{board::{BoardModel,BoardEdge,VirtualCycle},devices::i2c_bitbang::I2cBitBang,camera::{Gc0308,Gc0308State,Ov3660,Ov3660State,HostFrameSource},picture::{Picture,PixelFormat,Jpeg,JpegError},pins::PinRoutes};
#[derive(Clone,Copy,PartialEq,Eq)]
pub enum Variant {Ext,Cam,CamChatbot,M12,Volcengine}
impl Variant {
    fn name(self)->&'static str {match self {Self::Ext=>"atoms3r-ext",Self::Cam=>"atoms3r-cam",Self::CamChatbot=>"atoms3r-cam-chatbot",Self::M12=>"atoms3r-m12",Self::Volcengine=>"atoms3r-m12-volcengine"}}
    fn has_gc0308(self)->bool {matches!(self,Self::Cam|Self::CamChatbot)}
    fn has_ov3660(self)->bool {matches!(self,Self::M12|Self::Volcengine)}
}
struct OvSccb {sensor:Arc<Mutex<Ov3660>>,power:Arc<Mutex<bool>>}
impl I2cDevice for OvSccb {
    fn start(&mut self,r:bool)->bool {*self.power.lock().unwrap()&&self.sensor.lock().unwrap().start(r)}
    fn write(&mut self,b:u8)->bool {*self.power.lock().unwrap()&&self.sensor.lock().unwrap().write(b)}
    fn read(&mut self)->u8 {if *self.power.lock().unwrap(){self.sensor.lock().unwrap().read()}else{255}}
}
struct OvCamera {state:Arc<Mutex<Ov3660State>>,sensor:Arc<Mutex<Ov3660>>,power:Arc<Mutex<bool>>}
impl OvCamera {
    fn new()->Self {let state=Arc::new(Mutex::new(Ov3660State::default()));Self{sensor:Arc::new(Mutex::new(Ov3660::new(state.clone()))),state,power:Arc::new(Mutex::new(false))}}
    fn power(&mut self,on:bool) {let mut power=self.power.lock().unwrap();if *power!=on{self.sensor.lock().unwrap().reset();*power=on;}}
}
pub struct AtomS3RCam {variant:Variant,i2c:[I2cBitBang;2],levels:[bool;4],edges:Vec<BoardEdge>,imu:Arc<Mutex<Bmi270Sample>>,camera:Option<Arc<Mutex<Gc0308State>>>,ov:Option<OvCamera>,source:HostFrameSource,events:u64}
impl Default for AtomS3RCam {fn default()->Self {Self::new()}}
impl AtomS3RCam {
    pub fn new()->Self {Self::with_variant(Variant::CamChatbot)}
    pub fn with_variant(variant:Variant)->Self {Self{variant,i2c:std::array::from_fn(|_|I2cBitBang::new()),levels:[true;4],edges:Vec::new(),imu:Default::default(),camera:variant.has_gc0308().then(Default::default),ov:variant.has_ov3660().then(OvCamera::new),source:Default::default(),events:0}}
    fn has_camera(&self)->bool {self.variant!=Variant::Ext}
}
impl BoardModel for AtomS3RCam {
    fn name(&self)->&'static str {self.variant.name()}
    fn gpio_events(&self)->u64 {self.events}
    fn uses_gpio_edges(&self)->bool {false}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>) {
        let(mut enabled,mut output)=(0,0);for pin in[45,0,12,9,18]{if let Some(high)=r.software_output(pin){enabled|=1u64<<pin;if high{output|=1u64<<pin;}}}
        self.gpio_output_at(cycle,&[],enabled,output);
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.events+=changes.len() as u64;let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        if let Some(camera)=&self.camera {camera.lock().unwrap().power(!released(18));}
        if let Some(camera)=&mut self.ov {camera.power(!released(18));}
        for (bus,(sda_pin,scl_pin)) in [(45,0),(12,9)].into_iter().enumerate().take(if self.has_camera() {2} else {1}) {
            let scl=released(scl_pin);let sda=self.i2c[bus].drive(scl,released(sda_pin));
            for (j,(pin,level)) in [(sda_pin,sda),(scl_pin,scl)].into_iter().enumerate() {let i=bus*2+j;if level!=self.levels[i] {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        }
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        for bus in &mut self.i2c {bus.clear();}self.levels=[true;4];if let Some(camera)=&self.camera {camera.lock().unwrap().power(false);}
        if let Some(camera)=&mut self.ov {camera.power(false);camera.sensor.lock().unwrap().reset();}
        self.i2c[0].attach(0x68,Box::new(Bmi270::new(self.imu.clone())));if let Some(camera)=&self.camera {self.i2c[1].attach(0x21,Box::new(Gc0308::new(camera.clone())));}
        if let Some(camera)=&self.ov {self.i2c[1].attach(0x3c,Box::new(OvSccb{sensor:camera.sensor.clone(),power:camera.power.clone()}));}
        let mut result=Vec::new();for bus in 0..2 {for (i,pins) in [(45,0),(12,9)].into_iter().enumerate() {result.extend(self.i2c[i].controller_devices(Some(pins)).into_iter().map(|(a,d)|(bus,a,d)));}}result
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut levels:Vec<_>=[45,0,12,9].into_iter().zip(self.levels).take(if self.has_camera() {4} else {2}).collect();if self.variant==Variant::Ext {levels.push((41,true));}levels}
    fn named_pin(&self,name:&str)->Option<u8> {if self.variant==Variant::Ext&&matches!(name,"button-a"|"btnA"|"g41") {Some(41)} else {None}}
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool {if !a.iter().chain(g.iter()).all(|v|v.is_finite()) {return false;}let mut s=self.imu.lock().unwrap();s.accel_g=a;s.gyro_dps=g;s.version=s.version.wrapping_add(1);true}
    fn camera_dvp_pins(&self)->Option<[u8;11]> {self.has_camera().then_some([3,42,46,48,4,17,11,13,40,14,10])}
    fn set_camera_picture(&mut self,p:Picture)->Result<(),&'static str> {if !self.has_camera(){return Err("no camera on AtomS3R-Ext");}self.source.set_camera_picture(p)}
    fn camera_preview(&self,w:u32,h:u32)->Option<Vec<u8>> {self.source.camera_preview(w,h)}
    fn set_camera_jpeg(&mut self,j:Jpeg)->Result<(),JpegError> {let c=self.ov.as_ref().ok_or(JpegError::Unsupported)?;let s=*c.state.lock().unwrap();self.source.set_camera_jpeg(j,(s.width,s.height))}
    fn camera_frame_is_jpeg(&self)->bool {self.ov.as_ref().is_some_and(|c|c.state.lock().unwrap().jpeg)}
    fn camera_frame_with_clock(&mut self,clock:&dyn Fn(u8)->Option<(f64,u32)>)->Option<(u32,u32,Arc<Vec<u8>>)> {
        let(hz,duty)=clock(21)?;if !hz.is_finite()||hz<=0.||duty==0||duty>=65535{return None;}
        if let Some(c)=&self.camera {let output=c.lock().unwrap().output()?;return self.source.oriented_frame(output.width,output.height,output.format,Some((output.window,640,480)),output.mirror,output.flip);}
        let c=self.ov.as_ref()?;if !*c.power.lock().unwrap(){return None;}let s=*c.state.lock().unwrap();if !s.streaming{return None;}
        if s.jpeg{return self.source.jpeg_frame(s.width,s.height);}
        let format=match s.format{0x61=>PixelFormat::Rgb565,0x30=>PixelFormat::Yuyv,0x10=>PixelFormat::Grayscale,_=>return None};
        self.source.oriented_frame(s.width,s.height,format,s.window.map(|w|(w,2080,1548)),s.mirror,s.flip)
    }
    fn report(&self)->String {
        let camera=if let Some(camera)=&self.camera {let s=camera.lock().unwrap();format!("GC0308 power={} writes={}; RAW RGB565/YUYV/Y8",s.powered,s.writes)} else if let Some(c)=&self.ov {format!("OV3660 power={}; RAW/host JPEG",*c.power.lock().unwrap())}else{"G41=active-low; no camera or display".into()};
        format!("[{}] {} BMI270=polled-subset; physical SCCB12/9 XCLK21 POWER_N18 DVP->LCD_CAM/GDMA; virtual 10Hz frames, no ISP/encoder/startup/power-ramp or hardware calibration. UVC/audio/BMM150/IR unimplemented",self.name(),camera)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sccb_power_and_independent_i2c_routes() {
        let mut b=AtomS3RCam::new();let mut devices=b.i2c_devices();assert_eq!(devices.len(),4);
        for (_,a,d) in &devices {assert_eq!(d.pins(),Some(if *a==0x68 {(45,0)} else {(12,9)}));assert_ne!(*a,0x18);}
        let cam=devices.iter_mut().find(|(_,a,_)|*a==0x21).unwrap().2.as_mut();assert!(!cam.start(false));
        b.gpio_output_at(0,&[],1<<18,0);assert!(cam.start(false));cam.write(0);cam.start(true);assert_eq!(cam.read(),0x9b);
        b.gpio_output_at(1,&[],1<<18,1<<18);assert!(!cam.start(false));assert!(b.display().is_none());assert_eq!(b.named_pin("button-a"),None);
        let mut efuse=esp_periph::Efuse::new([0;6]);crate::board::configure_chip_identity(b.name(),&mut efuse);assert_eq!((efuse.ram.read(0x50)>>21)&7,1);
    }
    fn write(d:&mut dyn I2cDevice,r:u8,bytes:&[u8]) {assert!(d.start(false));assert!(d.write(r));for &v in bytes {assert!(d.write(v));}d.stop();}
    fn read(d:&mut dyn I2cDevice,r:u8,count:usize)->Vec<u8> {write(d,r,&[]);assert!(d.start(true));let values=(0..count).map(|_|d.read()).collect();d.stop();values}
    #[test]
    fn exact_sku_boards_keep_sensor_bus_identity_and_unknown_devices_absent() {
        for (name,gc0308,button) in [("atoms3r-ext",false,true),("atoms3r-cam",true,false),("atoms3r-cam-chatbot",true,false),("atoms3r-m12",false,false),("atoms3r-m12-volcengine",false,false)] {
            let mut b=crate::board::make_board(name).unwrap();assert_eq!(b.name(),name);assert!(b.display().is_none());assert!(b.leds().is_none());
            assert_eq!(b.named_pin("button-a"),button.then_some(41));
            let mut efuse=esp_periph::Efuse::new([0;6]);crate::board::configure_chip_identity(b.name(),&mut efuse);assert_eq!((efuse.ram.read(0x50)>>21)&7,1);
            let mut devices=b.i2c_devices();assert_eq!(devices.len(),if button {2}else{4});
            for (_,a,d) in &devices {assert_eq!(d.pins(),Some(if *a==0x68 {(45,0)}else{(12,9)}));assert!(![0x10,0x18,0x30,0x76].contains(a));}
            let imu=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x68).unwrap().2.as_mut();
            assert!(!imu.start_address(0x68,0x69,false));assert_eq!(read(imu,0,1),[0x24]);
            write(imu,0x59,&[0]);write(imu,0x5e,&[0;8192]);write(imu,0x59,&[1]);write(imu,0x7d,&[6]);
            assert!(b.set_imu_sample([0.5,-1.0,0.0],[125.0,0.0,-125.0]));
            assert_eq!(read(imu,0x0c,12),[0,8,0,240,0,0,0,8,0,0,0,248],"8g / 2000dps reset ranges must produce real register bytes");
            assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));
            // Reattaching recreates devices and their reset registers, while retaining the supplied environment sample.
            devices=b.i2c_devices();let imu=devices.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x68).unwrap().2.as_mut();
            assert_eq!(read(imu,0x7d,1),[0]);assert_eq!(read(imu,0x0c,6),[0;6]);
            if gc0308 {
                let cam=devices.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x21).unwrap().2.as_mut();assert!(!cam.start(false));
                b.gpio_output_at(0,&[],1<<18,0);assert_eq!(read(cam,0,1),[0x9b]);
                write(cam,0x24,&[0xa6]);b.gpio_output_at(1,&[],1<<18,1<<18);assert!(!cam.start(false));
                b.gpio_output_at(2,&[],1<<18,0);assert_eq!(read(cam,0x24,1),[0xa2]);
                let _=b.i2c_devices();assert!(!cam.start(false),"board reset must turn off the old camera handle as well");
            }else if !button {
                let cam=devices.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x3c).unwrap().2.as_mut();assert!(!cam.start(false));
                b.gpio_output_at(0,&[],1<<18,0);write(cam,0x30,&[0x0a]);assert!(cam.start(true));assert_eq!((cam.read(),cam.read()),(0x36,0x60));
                b.gpio_output_at(1,&[],1<<18,1<<18);assert!(!cam.start(false));
                assert!(b.camera_dvp_pins().is_some());
            }
        }
    }

}
