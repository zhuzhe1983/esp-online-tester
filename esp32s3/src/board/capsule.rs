//! K129/K129-V11: official Capsule pin maps and Stamp-S3/S3A RGB supply.
//! Digital subset only: always-present virtual supply, GPIO42 wake input,
//! G0 boot, G21 RMT RGB, BMI270@69 and BM8563@51 on SDA8/SCL10.
//! HOLD46, RTC wake/alarm, microphone, buzzer, IR and SD transport are absent.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,bmi270::{Bmi270,Bmi270Sample}};
use esp_soc::{board::{BoardModel,BoardEdge,VirtualCycle},devices::{i2c_bitbang::I2cBitBang,bm8563::{Bm8563,Bm8563State}}};
use super::stamps3::StampS3;

pub struct Capsule {v11:bool,stamp:StampS3,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,imu:Arc<Mutex<Bmi270Sample>>,rtc:Arc<Mutex<Bm8563State>>}
impl Capsule {
    pub fn new(v11:bool)->Self {Self{v11,stamp:if v11 {StampS3::s3a()}else{StampS3::new()},i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),imu:Default::default(),rtc:Arc::new(Mutex::new(Bm8563State::new(crate::periph::CPU_HZ)))}}
}
impl BoardModel for Capsule {
    fn name(&self)->&'static str {if self.v11 {"capsule-v11"}else{"capsule"}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.stamp.gpio_output_at(cycle,changes,enabled,output);
        let released=|pin:u8|enabled&(1u64<<pin)==0||output&(1u64<<pin)!=0;
        let scl=released(10);let sda=self.i2c.drive(scl,released(8));
        for(i,(pin,level))in [(8,sda),(10,scl)].into_iter().enumerate(){if level!=self.levels[i]{self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {self.stamp.rmt_frame(pin,bits);}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {self.stamp.leds()}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x69,Box::new(Bmi270::new(self.imu.clone())));self.i2c.attach(0x51,Box::new(Bm8563::new(self.rtc.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((8,10))).into_iter().map(move |(a,d)|(bus,a,d))).collect()
    }
    fn advance_to(&mut self,cycle:VirtualCycle){self.rtc.lock().expect("Capsule RTC state").advance_to(cycle);}
    fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(42,true),(8,self.levels[0]),(10,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8>{match name{"wake"|"g42"=>Some(42),"boot"|"g0"=>Some(0),_=>None}}
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{if !a.iter().chain(g.iter()).all(|v|v.is_finite()){return false;}let mut s=self.imu.lock().expect("Capsule IMU state");s.accel_g=a;s.gyro_dps=g;s.version=s.version.wrapping_add(1);true}
    fn report(&self)->String{format!("[{}] BMI270@69 polled + BM8563 calendar on SDA8/SCL10; G42 wake input; {}; virtual always-on supply, RTC alarm/wake/HOLD46/SD/IR/PDM/buzzer unmodeled",self.name(),self.stamp.report())}
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn exact_bus_addresses_shared_calendar_and_revision_supply(){
        for v11 in[false,true]{let mut b=Capsule::new(v11);let mut devices=b.i2c_devices();assert_eq!(devices.len(),4);assert!(devices.iter().all(|(_,a,d)|[0x69,0x51].contains(a)&&d.pins()==Some((8,10))));assert!(devices.iter().all(|(_,a,_)|*a!=0x68));
            let d=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x69).unwrap().2.as_mut();assert!(!d.start_address(0x69,0x68,false));assert!(d.start(false));d.write(0);d.start(true);assert_eq!(d.read(),0x24);d.stop();
            let d=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x51).unwrap().2.as_mut();d.start(false);d.write(2);d.write(0);d.stop();b.advance_to(crate::periph::CPU_HZ);
            let d=devices.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x51).unwrap().2.as_mut();d.start(false);d.write(2);d.start(true);assert_eq!(d.read(),1);d.stop();
            assert_eq!(b.named_pin("wake"),Some(42));assert_eq!(b.named_pin("boot"),Some(0));assert!(b.display().is_none());assert!(b.sd_card_slot().is_none());
            b.rmt_frame(21,&[true;24]);assert_eq!(b.leds().unwrap().0,if v11{&[[0;3]][..]}else{&[[255;3]][..]});
            b.gpio_output_at(1,&[],1<<38,1<<38);b.rmt_frame(21,&[true;24]);assert_eq!(b.leds().unwrap().0,&[[255;3]]);b.gpio_output_at(2,&[],1<<38,0);assert_eq!(b.leds().unwrap().0,if v11{&[[0;3]][..]}else{&[[255;3]][..]});
            assert!(!b.set_imu_sample([f64::NAN,0.,0.],[0.;3]));assert!(b.set_imu_sample([0.,1.,0.],[1.;3]));
        }
    }
}
