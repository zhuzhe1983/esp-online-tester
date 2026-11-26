//! U149: independently pinned internal firmware496530d6. MLX90640 0x33
//! SDA21/SCL22, SK6812 G27 (SPI 3-bit symbol or RMT), button39, buzzer25.
//! External SDA33/SCL32 is the ESP32 slave0x32, not a second local sensor.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,mlx90640::{Mlx90640,Mlx90640State}};
use esp_soc::{BoardModel,board::{SpiPins,VirtualCycle},devices::{Ws2812Chain,i2c_bitbang::I2cBitBang}};
pub struct UnitThermal2 {thermal:Arc<Mutex<Mlx90640State>>,rgb:Ws2812Chain,buzzer:Option<(f64,u32)>}
impl UnitThermal2 {pub fn new()->Self {Self{thermal:Default::default(),rgb:Ws2812Chain::new(1),buzzer:None}}}
impl Default for UnitThermal2 {fn default()->Self {Self::new()}}
impl BoardModel for UnitThermal2 {
    fn name(&self)->&'static str {"unit-thermal2"}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(39,true),(21,true),(22,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"=>Some(39),_=>None}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {let mut shared=I2cBitBang::new();shared.attach(0x33,Box::new(Mlx90640::new(self.thermal.clone())));(0..2).flat_map(|bus|shared.controller_devices(Some((21,22))).into_iter().map(move|(a,d)|(bus,a,d))).collect()}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.thermal.lock().expect("Thermal2 state poisoned").advance(cycle);}
    fn next_deadline(&self)->Option<VirtualCycle> {Some(self.thermal.lock().expect("Thermal2 state poisoned").next)}
    fn thermal_environment_at(&mut self,cycle:VirtualCycle,sample:[u16;5])->bool {let mut s=self.thermal.lock().expect("Thermal2 state poisoned");s.advance(cycle);s.stimulus(sample)}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        // Official NeoPixel_Class emits 24 symbols100/110 at2.5MHz, followed
        // by zeros for the reset interval; no SCLK or CS is connected to LED.
        if p.mosi&(1<<27)!=0 && tx.len()>=10 {
            let bits:Vec<_>=tx.iter().flat_map(|v|(0..8).rev().map(move|i|v&(1<<i)!=0)).collect();
            if bits.len()>=80 && bits[..72].as_chunks::<3>().0.iter().all(|b|b[0]&&!b[2]) && bits[72..].iter().all(|b|!*b) {let rgb:Vec<_>=bits[..72].as_chunks::<3>().0.iter().map(|b|b[1]).collect();self.rgb.from_bits(&rgb);}
        }vec![255;rx]
    }
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {self.buzzer=pwm(25);}
    fn hardware_diagnostics(&self)->Option<String> {let s=self.thermal.lock().expect("Thermal2 state poisoned");Some(format!("{{\"sensor\":\"MLX90640\",\"calibration\":\"virtual-uniform-v1\",\"frames\":{},\"subpage\":{},\"ready\":{},\"buzzerHz\":{},\"buzzerDuty\":{}}}",s.frames,s.status&1,s.status&8!=0,self.buzzer.map_or(0.0,|p|p.0),self.buzzer.map_or(0,|p|p.1)))}
    fn report(&self)->String {let s=self.thermal.lock().expect("Thermal2 state poisoned");format!("[unit-thermal2] MLX90640 synthetic ADC/virtual EEPROM frames={} reads={} writes={}; G27 RGB updates={}; G25 PWM={:?}; external slave0x32/optical calibration/audio synthesis unmodeled",s.frames,s.reads,s.writes,self.rgb.updates,self.buzzer)}
}
