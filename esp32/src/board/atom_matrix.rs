//! Atom-Matrix C008-B (not v1.1): docs.m5stack.com/en/core/Atom-Matrix.
//! G39 active-low user key, G27 25 WS2812C-2020 GRB LEDs, MPU6886 0x68
//! on SDA25/SCL21. M5Atom ebbf7363: Wire.begin(25,21), LED index x+y*5.
//! Polled IMU subset reuses DS-000193 rev1.1 model; no FIFO/interrupt/ODR timing.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,mpu6886::{Mpu6886,Mpu6886Sample}};
use esp_soc::board::BoardModel;
use esp_soc::devices::{Ws2812Chain,i2c_bitbang::I2cBitBang};
pub struct AtomMatrix {rgb:Ws2812Chain,imu:Arc<Mutex<Mpu6886Sample>>}
impl AtomMatrix {pub fn new()->Self {Self{rgb:Ws2812Chain::new(25),imu:Default::default()}}}
impl Default for AtomMatrix {fn default()->Self {Self::new()}}
impl BoardModel for AtomMatrix {
    fn name(&self)->&'static str {"atom-matrix"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(39,true),(25,true),(21,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"g39"=>Some(39),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        // Share the physical sensor register state between both possible controller routes.
        let mut shared=I2cBitBang::new();shared.attach(0x68,Box::new(Mpu6886::new(self.imu.clone())));
        (0..2).flat_map(|bus|shared.controller_devices(Some((25,21))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        let mut sample=self.imu.lock().expect("Atom-Matrix IMU sample mutex poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn report(&self)->String {format!("[atom-matrix] WS2812C RMT updates={} rgb={:?}; MPU6886=polled-subset IR/network/analog=unverified",self.rgb.updates,self.rgb.leds)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrix_chain_keeps_all_25_pixels_and_ignores_wrong_pin() {
        let mut b=AtomMatrix::new();let bytes:Vec<u8>=(0..25).flat_map(|i|[i,0x40+i,0x80+i]).collect();let bits:Vec<bool>=bytes.into_iter().flat_map(|v|(0..8).map(move |i|v&(0x80>>i)!=0)).collect();
        b.rmt_frame(35,&bits);assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(27,&bits);assert_eq!(b.leds().unwrap().0.len(),25);assert_eq!(b.leds().unwrap().0[24],[0x58,24,0x98]);assert_eq!(b.named_pin("button"),Some(39));
    }
    #[test]
    fn imu_is_pin_scoped_shared_and_accepts_finite_samples() {
        let mut b=AtomMatrix::new();let mut devices=b.i2c_devices();assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,d)|*a==0x68 && d.pins()==Some((25,21))));
        let d=devices[0].2.as_mut();d.start(false);d.write(0x75);d.start(true);assert_eq!(d.read(),0x19);d.start(false);d.write(0x6b);d.write(1);
        assert!(b.set_imu_sample([1.0,-1.0,0.0],[0.0;3]));let d=devices[1].2.as_mut();d.start(false);d.write(0x3b);d.start(true);assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(),[0x40,0,0xc0,0,0,0]);assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));
    }
}
