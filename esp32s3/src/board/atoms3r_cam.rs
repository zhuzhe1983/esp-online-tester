//! K147-CAM: AtomS3R-CAM plus Atomic Voice Base. Main v0.4.1 / camera daughter schematic:
//! camera SCCB SDA12/SCL9, G18 active-low power; BMI270 SDA45/SCL0. Audio SDA38/SCL39
//! remains absent. This first slice provides sensor configuration, not DVP capture or audio.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample}};
use esp_soc::{board::{BoardModel,BoardEdge,VirtualCycle},devices::i2c_bitbang::I2cBitBang,camera::{Gc0308,Gc0308State}};
pub struct AtomS3RCam {i2c:[I2cBitBang;2],levels:[bool;4],edges:Vec<BoardEdge>,imu:Arc<Mutex<Bmi270Sample>>,camera:Arc<Mutex<Gc0308State>>,events:u64}
impl Default for AtomS3RCam {fn default()->Self {Self::new()}}
impl AtomS3RCam {pub fn new()->Self {Self{i2c:std::array::from_fn(|_|I2cBitBang::new()),levels:[true;4],edges:Vec::new(),imu:Default::default(),camera:Default::default(),events:0}}}
impl BoardModel for AtomS3RCam {
    fn name(&self)->&'static str {"atoms3r-cam-chatbot"}
    fn gpio_events(&self)->u64 {self.events}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.events+=changes.len() as u64;let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        self.camera.lock().unwrap().power(!released(18));
        for (bus,(sda_pin,scl_pin)) in [(45,0),(12,9)].into_iter().enumerate() {
            let scl=released(scl_pin);let sda=self.i2c[bus].drive(scl,released(sda_pin));
            for (j,(pin,level)) in [(sda_pin,sda),(scl_pin,scl)].into_iter().enumerate() {let i=bus*2+j;if level!=self.levels[i] {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        }
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        for bus in &mut self.i2c {bus.clear();}self.camera.lock().unwrap().power(false);
        self.i2c[0].attach(0x68,Box::new(Bmi270::new(self.imu.clone())));self.i2c[1].attach(0x21,Box::new(Gc0308::new(self.camera.clone())));
        let mut result=Vec::new();for bus in 0..2 {for (i,pins) in [(45,0),(12,9)].into_iter().enumerate() {result.extend(self.i2c[i].controller_devices(Some(pins)).into_iter().map(|(a,d)|(bus,a,d)));}}result
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {[45,0,12,9].into_iter().zip(self.levels).collect()}
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool {if !a.iter().chain(g.iter()).all(|v|v.is_finite()) {return false;}let mut s=self.imu.lock().unwrap();s.accel_g=a;s.gyro_dps=g;s.version=s.version.wrapping_add(1);true}
    fn report(&self)->String {let s=self.camera.lock().unwrap();format!("[atoms3r-cam-chatbot] GC0308 power={} writes={} BMI270=polled-subset; capture/audio unimplemented",s.powered,s.writes)}
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
}
