//! K016-P2 is PICO-V3-02, NOT the AXP192/PICO-D4 StickC-Plus.
//! Official StickC-Plus2 pin table and schematic: LCD DC14/RST12/BL27,
//! keys37/39/35, HOLD4, battery ADC38, red LED/IR19; I2C21/22.
//! LCD address offsets follow M5GFX board_M5StickCPlus2 and ST7789.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,mpu6886::{Mpu6886,Mpu6886Sample}};
use esp_soc::{BoardModel,board::{SpiPins,VirtualCycle},devices::{DcsPanel,bm8563::{Bm8563,Bm8563State},i2c_bitbang::I2cBitBang}};
pub struct StickCPlus2 {panel:DcsPanel,reset:bool,backlight:bool,pwm:Option<bool>,version:u64,rtc:Arc<Mutex<Bm8563State>>,imu:Arc<Mutex<Mpu6886Sample>>,led:[[u8;3];1],led_version:u64,hold:bool}
impl Default for StickCPlus2 {fn default()->Self{Self::new()}}
impl StickCPlus2 {
    pub fn new()->Self {Self{panel:DcsPanel::st7789(),reset:false,backlight:false,pwm:None,version:0,rtc:Arc::new(Mutex::new(Bm8563State::new(240_000_000))),imu:Default::default(),led:[[0;3]],led_version:0,hold:false}}
    fn lit(&self)->bool{self.pwm.unwrap_or(self.backlight)}
}
impl BoardModel for StickCPlus2 {
    fn name(&self)->&'static str{"stickc-plus2"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {for &(pin,high) in changes {match pin {
        14=>self.panel.dc=high,
        12=>{if !high&&!self.reset{self.panel.reset();}self.reset= !high;self.version=self.version.wrapping_add(1);},
        27=>{self.backlight=high;self.version=self.version.wrapping_add(1);},
        4=>self.hold=high,
        19=>{self.led=[if high{[255,0,0]}else{[0;3]}];self.led_version=self.led_version.wrapping_add(1);},_=>{}
    }}}
    fn uses_spi_pins(&self)->bool{true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if !self.reset&&pins.sclk&(1<<13)!=0&&pins.mosi&(1<<15)!=0&&pins.cs&(1<<5)!=0 {for &b in tx{self.panel.byte(b);}self.version=self.version.wrapping_add(1);}
        vec![255;rx_len]
    }
    fn uses_gpio_pwm(&self)->bool{true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {let previous=self.lit();self.pwm=pwm(27).map(|(_,d)|d!=0);if previous!=self.lit(){self.version=self.version.wrapping_add(1);}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {let mut bus=I2cBitBang::new();bus.attach(0x51,Box::new(Bm8563::new(self.rtc.clone())));bus.attach(0x68,Box::new(Mpu6886::new(self.imu.clone())));(0..2).flat_map(|n|bus.controller_devices(Some((21,22))).into_iter().map(move|(a,d)|(n,a,d))).collect()}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(37,true),(39,true),(35,true),(21,true),(22,true)]}
    fn named_pin(&self,id:&str)->Option<u8>{match id{"button-a"=>Some(37),"button-b"=>Some(39),"power"|"button-c"=>Some(35),_=>None}}
    fn advance_to(&mut self,cycle:VirtualCycle){self.rtc.lock().unwrap().advance_to(cycle);}
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{if !a.iter().chain(g.iter()).all(|v|v.is_finite()){return false;}let mut s=self.imu.lock().unwrap();s.accel_g=a.map(|v|v.clamp(-16.,16.));s.gyro_dps=g.map(|v|v.clamp(-2000.,2000.));s.version=s.version.wrapping_add(1);true}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((135,240,if !self.reset&&self.lit(){super::tdisplay::lcd_visible(&self.panel)}else{vec![0;135*240]},self.display_version()))}
    fn display_version(&self)->u64{self.version.wrapping_add(self.panel.pixels_written).wrapping_add(self.panel.resets)}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.clone(),240,320))}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led,self.led_version))}
    fn report(&self)->String{format!("[stickc-plus2] ST7789 frames={} pixels={} BL={} HOLD={}; MPU6886/BM8563 polled; no AXP192; battery ADC/microphone/buzzer/IR waveform/power-off/wake unmodeled",self.panel.frames,self.panel.pixels_written,self.lit(),self.hold)}
}
#[cfg(test)]mod tests {
    use super::*;
    fn cmd(b:&mut StickCPlus2,c:u8,data:&[u8]){let pins=SpiPins{sclk:1<<13,mosi:1<<15,cs:1<<5,miso:None,keep_cs:false};b.gpio_changes(&[(14,false)]);b.spi_transfer_pins(2,pins,&[c],0);b.gpio_changes(&[(14,true)]);b.spi_transfer_pins(2,pins,data,0);}
    #[test]fn exact_pins_panel_power_and_absent_axp192(){let mut b=StickCPlus2::new();assert_eq!(b.named_pin("power"),Some(35));let devices=b.i2c_devices();assert_eq!(devices.iter().map(|(n,a,_)|(*n,*a)).collect::<Vec<_>>(),[(0,0x51),(0,0x68),(1,0x51),(1,0x68)]);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[8]);cmd(&mut b,0x2a,&[0,52,0,52]);cmd(&mut b,0x2b,&[0,40,0,40]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0);b.gpio_changes(&[(27,true)]);assert_eq!(b.display().unwrap().2[0],0xf800);b.gpio_changes(&[(18,false),(23,false)]);assert_eq!(b.display().unwrap().2[0],0xf800);b.gpio_changes(&[(12,false)]);assert_eq!(b.display().unwrap().2[0],0);}
}
