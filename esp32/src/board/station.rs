//! M5Station K123 / K124-B digital display, keys and internal I2C subset.
//! Official wiring: docs.m5stack.com/en/core/Station-{485,Bat}; M5GFX
//! 28d47949bdc84f30881623476f555398e2194313 board_spec/boards/m5station.json.
//! The two SKUs have separate identities. RS485, INA3221/199, port switches,
//! RGB, battery/analog power and factory firmware compatibility are not modeled.
use std::sync::{Arc,Mutex,atomic::{AtomicU64,Ordering}};
use esp_periph::i2c::{I2cDevice,axp192::{Axp192,Axp192State},mpu6886::{Mpu6886,Mpu6886Sample}};
use esp_soc::{board::{BoardModel,SpiPins},devices::{DcsPanel,bm8563::{Bm8563,Bm8563State},i2c_bitbang::I2cBitBang}};

struct Power {inner:Axp192,writes:Arc<AtomicU64>}
impl I2cDevice for Power {
    fn start(&mut self,read:bool)->bool {self.inner.start(read)}
    fn write(&mut self,b:u8)->bool {let ack=self.inner.write(b);if ack {self.writes.fetch_add(1,Ordering::Relaxed);}ack}
    fn read(&mut self)->u8 {self.inner.read()}
    fn stop(&mut self) {self.inner.stop();}
}
pub struct Station {name:&'static str,panel:DcsPanel,pmic:Arc<Mutex<Axp192State>>,imu:Arc<Mutex<Mpu6886Sample>>,rtc:Arc<Mutex<Bm8563State>>,writes:Arc<AtomicU64>,cs:bool,reset:bool,events:u64,spi:u64,dropped:u64}
impl Station {
    pub fn new(battery:bool)->Self {Self{name:if battery {"station-bat"}else{"station-485"},panel:DcsPanel::st7789(),pmic:Arc::new(Mutex::new(Axp192State::new(crate::periph::CPU_HZ))),imu:Default::default(),rtc:Arc::new(Mutex::new(Bm8563State::new(crate::periph::CPU_HZ))),writes:Default::default(),cs:true,reset:true,events:0,spi:0,dropped:0}}
}
impl BoardModel for Station {
    fn name(&self)->&'static str {self.name}
    fn gpio_events(&self)->u64 {self.events}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {for &(pin,high) in changes {self.events+=1;match pin {5=>self.cs=high,19=>self.panel.dc=high,15=>{if self.reset&&!high {self.panel.reset();}self.reset=high;},_=>{}}}}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if self.reset&&(!self.cs||pins.cs&(1<<5)!=0)&&pins.sclk&(1<<18)!=0&&pins.mosi&(1<<23)!=0 {for &b in tx {self.panel.byte(b);}self.spi+=tx.len()as u64;}else{self.dropped+=tx.len()as u64;}vec![255;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let mut bus=I2cBitBang::new();bus.attach(0x34,Box::new(Power{inner:Axp192::new(self.pmic.clone()),writes:self.writes.clone()}));bus.attach(0x68,Box::new(Mpu6886::new(self.imu.clone())));bus.attach(0x51,Box::new(Bm8563::new(self.rtc.clone())));
        (0..2).flat_map(|n|bus.controller_devices(Some((21,22))).into_iter().map(move|(a,d)|(n,a,d))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(37,true),(38,true),(39,true),(21,true),(22,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button-a"=>Some(37),"button-b"=>Some(38),"button-c"=>Some(39),_=>None}}
    fn advance_to(&mut self,cycle:u64) {self.rtc.lock().unwrap().advance_to(cycle);}
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool {if !a.iter().chain(g.iter()).all(|v|v.is_finite()){return false;}let mut s=self.imu.lock().unwrap();s.accel_g=a;s.gyro_dps=g;s.version=s.version.wrapping_add(1);true}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        // ST7789V2 135x240 glass, rotation offset 1: same native window
        // (columns 52..186, rows 40..279) documented by the manufacturer driver.
        let blank=!self.reset||!self.panel.on||self.panel.sleeping||!self.pmic.lock().unwrap().ldo_enabled(3);let mut frame=Vec::with_capacity(240*135);
        for y in 0..135 {for x in 0..240 {frame.push(if blank {0}else{self.panel.gram[(40+x)*240+186-y]});}}
        Some((240,135,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.events).wrapping_add(self.writes.load(Ordering::Relaxed))}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn report(&self)->String {format!("[{}] ST7789V2 frames={} SPI={} dropped={}; G37/38/39, hardware I2C21/22 AXP192 register/LDO3-backlight, MPU6886 polled, BM8563 calendar; INA3221/199, RS485, RGB, Grove power, battery and factory compatibility unverified",self.name,self.panel.frames,self.spi,self.dropped)}
}
#[cfg(test)]mod tests {
    use super::*;
    fn routes()->SpiPins {SpiPins{sclk:1<<18,mosi:1<<23,miso:None,cs:1<<5,keep_cs:false}}
    fn cmd(b:&mut Station,c:u8,data:&[u8]) {b.gpio_changes(&[(19,false)]);b.spi_transfer_pins(2,routes(),&[c],0);b.gpio_changes(&[(19,true)]);b.spi_transfer_pins(2,routes(),data,0);}
    fn power(b:&mut Station,value:u8) {let mut d=b.i2c_devices();let p=&mut d.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x34).unwrap().2;p.start(false);p.write(0x12);p.write(value);p.stop();}
    #[test]fn both_skus_route_real_spi_and_backlight_without_other_board_devices() {
        for battery in [false,true] {let mut b=Station::new(battery);assert_eq!(b.name(),if battery {"station-bat"}else{"station-485"});cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0x68]);cmd(&mut b,0x2a,&[0,40,0,40]);cmd(&mut b,0x2b,&[0,53,0,53]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0);power(&mut b,8);assert_eq!(b.display().unwrap().2[0],0xf800);let v=b.display_version();power(&mut b,0);assert!(b.display_version()>v);assert_eq!(b.display().unwrap().2[0],0);power(&mut b,8);assert_eq!(b.display().unwrap().2[0],0xf800);
        let mut wrong=routes();wrong.mosi=1<<19;b.spi_transfer_pins(2,wrong,&[255,255],0);assert_eq!(b.dropped,2);assert_eq!(b.display().unwrap().2[0],0xf800);b.gpio_changes(&[(15,false)]);assert!(b.display().unwrap().2.iter().all(|v|*v==0));assert_eq!(b.named_pin("button-a"),Some(37));assert_eq!(b.named_pin("power"),None);let devices=b.i2c_devices();assert_eq!(devices.len(),6);assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((21,22))));assert!(!devices.iter().any(|(_,a,_)|*a==0x40));}
    }
}
