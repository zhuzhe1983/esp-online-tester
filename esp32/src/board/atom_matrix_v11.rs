//! C008-B-V11: official Atom-Matrix_v1.1 pin map and ATOM_KEYPAD V1.2 (2025-12-11).
//! G27 25 RGB LEDs / G39 key; BMI270 address68, SDA25/SCL21 (not MPU6886).
//! BMI270 digital polling subset: Bosch DS000-08; no FIFO, feature engine or physical timing.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle};
use esp_soc::devices::{Ws2812Chain,i2c_bitbang::I2cBitBang};
pub struct AtomMatrixV11 {rgb:Ws2812Chain,imu:Arc<Mutex<Bmi270Sample>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>}
impl AtomMatrixV11 {pub fn new()->Self {Self{rgb:Ws2812Chain::new(25),imu:Default::default(),i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new()}}}
impl Default for AtomMatrixV11 {fn default()->Self {Self::new()}}
impl BoardModel for AtomMatrixV11 {
    fn name(&self)->&'static str {"atom-matrix-v11"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,_changes:&[(u8,bool)],enabled:u64,output:u64) {
        let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(21);let sda=self.i2c.drive(scl,released(25));
        for (index,(pin,level)) in [(25,sda),(21,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(39,true),(25,self.levels[0]),(21,self.levels[1])]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button-a"|"btnA"|"g39"=>Some(39),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x68,Box::new(Bmi270::new(self.imu.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((25,21))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        let mut sample=self.imu.lock().expect("Atom-Matrix v1.1 IMU sample mutex poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn report(&self)->String {format!("[atom-matrix-v11] RMT27 updates={} rgb={:?}; BMI270 polling subset; IR/network/analog unverified",self.rgb.updates,self.rgb.leds)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v11_has_bmi270_not_mpu6886_and_25_g27_pixels() {
        let mut b=AtomMatrixV11::new();let mut ds=b.i2c_devices();assert_eq!(ds.len(),2);
        for (_,a,d) in &ds {assert_eq!(*a,0x68);assert_eq!(d.pins(),Some((25,21)));}
        let d=ds[0].2.as_mut();d.start(false);d.write(0);d.start(true);assert_eq!(d.read(),0x24);
        d.start(false);d.write(0x75);d.start(true);assert_eq!(d.read(),0);
        d.start(false);d.write(0x41);d.write(3);let d=ds[1].2.as_mut();d.start(false);d.write(0x41);d.start(true);assert_eq!(d.read(),3);
        assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));assert!(b.set_imu_sample([0.0,1.0,0.0],[0.0;3]));
        let bits:Vec<_>=(0..25).flat_map(|n|[n,0x40+n,0x80+n]).flat_map(|v|(0..8).map(move|i|v&(0x80>>i)!=0)).collect();
        b.rmt_frame(26,&bits);assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(27,&bits);assert_eq!(b.leds().unwrap().0[24],[0x58,24,0x98]);
    }
    #[test]
    fn pico_package_identity_is_cold_board_scoped() {
        let mut b=crate::bus::SocBus::new(4*1024*1024,[0;6]);
        let old=b.periph.efuse.ram.read(0x0c);assert_eq!((old>>9)&7,0);
        b.board=Box::new(AtomMatrixV11::new());b.attach_board_devices();
        assert_eq!(b.periph.efuse.ram.read(0x0c),(old&!(7<<9))|(5<<9));
    }

}
