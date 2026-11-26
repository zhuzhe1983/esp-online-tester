//! M5Stack NanoH2 C149 official pin map, fetched 2026-11-26.
//! H2 G9 key, G10 RGB supply enable, G11 WS2812 data, G4 blue LED.
//! Uses the independent H2 SRAM/RMT machine; no C6 substitution or RF model.
use esp_soc::{BoardModel,devices::Ws2812Chain,pins::PinRoutes};
pub struct NanoH2 {rgb:Ws2812Chain,powered:bool,leds:[[u8;3];2],updates:u64}
impl Default for NanoH2 {fn default()->Self {Self::new()}}
impl NanoH2 {
 pub fn new()->Self {Self{rgb:Ws2812Chain::new(1),powered:false,leds:[[0;3];2],updates:0}}
 fn publish(&mut self,index:usize,color:[u8;3]) {if self.leds[index]!=color {self.leds[index]=color;self.updates=self.updates.wrapping_add(1);}}
}
impl BoardModel for NanoH2 {
 fn name(&self)->&'static str {"nanoh2"}
 fn named_pin(&self,id:&str)->Option<u8> {matches!(id,"button-a"|"button"|"boot").then_some(9)}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(9,true)]}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {
  let powered=routes.software_output(10)==Some(true);
  if self.powered&&!powered {self.rgb=Ws2812Chain::new(1);self.publish(0,[0;3]);}
  self.powered=powered;
  self.publish(1,if routes.software_output(4)==Some(true) {[0,0,255]} else {[0;3]});
 }
 fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==11&&self.powered {self.rgb.from_bits(bits);self.publish(0,self.rgb.leds[0]);}}
 fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.updates))}
 fn report(&self)->String {format!("[nanoh2] independent ESP32-H2 SRAM/UART/GPIO/RMT; RGB10/11 powered={} rgb={:?} blue4={:?}; ROM/Flash boot, USB, radio, IR and analog power unimplemented",self.powered,self.leds[0],self.leds[1])}
}
