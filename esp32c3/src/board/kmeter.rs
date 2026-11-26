//! U133 original ESP32-C3 KMeter, not the STM32 isolated successor.
//! M5Unit-KMeter-Internal-FW c97d31ed: MAX31855 MISO5/SCLK6/CS7, read LED1.
use esp_soc::{BoardModel,board::SpiPins,pins::PinRoutes};
use esp_periph::i2c::env4::Environment;
pub struct Kmeter {pending:Option<(u64,u32)>,frame:u32,cycle:u64,leds:[[u8;3];1],updates:u64,reads:u64}
impl Default for Kmeter {fn default()->Self {Self::new()}}
impl Kmeter {pub fn new()->Self {Self{pending:None,frame:0x00011901,cycle:0,leds:[[0;3];1],updates:0,reads:0}}}
impl BoardModel for Kmeter {
 fn name(&self)->&'static str {"m5-kmeter"}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>) {self.advance_to(cycle);let next=if r.software_output(1)==Some(true) {[255,0,0]}else{[0;3]};if self.leds[0]!=next {self.leds[0]=next;self.updates=self.updates.wrapping_add(1);}}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(8,true),(9,true)]}
 fn uses_spi_pins(&self)->bool {true}
 fn spi_transfer_pins(&mut self,host:u8,p:SpiPins,_tx:&[u8],rx_len:usize)->Vec<u8> {
  if host!=2||p.sclk&(1<<6)==0||p.cs&(1<<7)==0||p.miso!=Some(5)||rx_len!=4 {return vec![255;rx_len];}
  self.reads=self.reads.wrapping_add(1);self.frame.to_be_bytes().to_vec()
 }
 fn accessory_environment_at(&mut self,cycle:u64,model:&str,port:&str,connected:bool,s:Environment)->bool {
  if model!="max31855k"||port!="thermocouple"||!s.temperature_c.is_finite()||!(-200.0..=1350.0).contains(&s.temperature_c) {return false;}
  self.advance_to(cycle);
  // MAX31855 Rev5: signed14-bit hot junction /0.25C; signed12-bit cold /0.0625C.
  // Synthetic cold junction is explicitly fixed25C; absent probe sets OC+D16.
  let value=if connected {(((s.temperature_c*4.0).round()as i32 as u32)&0x3fff)<<18|(400<<4)}else{0x00011901};
  self.pending=Some((cycle.saturating_add(crate::periph::CPU_HZ/10),value));true
 }
 fn advance_to(&mut self,cycle:u64) {self.cycle=self.cycle.max(cycle);if self.pending.is_some_and(|(at,_)|at<=self.cycle) {self.frame=self.pending.take().unwrap().1;}}
 fn next_deadline(&self)->Option<u64> {self.pending.map(|(at,_)|at)}
 fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.updates))}
 fn report(&self)->String {format!("[m5-kmeter] original C3 MAX31855 digital converter SCLK6/MISO5/CS7 reads={} frame={:08x}; environment max31855k/thermocouple hot-junction source, synthetic cold25C, nominal100ms conversion; GPIO1 red read LED; physical Flash unknown (4MiB virtual); I2C slave app, thermal inertia/analog error and hardware validation absent",self.reads,self.frame)}
}
