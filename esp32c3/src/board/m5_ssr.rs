//! U139 ACSSR: Sch_UNIT_ACSSR, ESP32-C3FH4, G4 relay-control output.
//! U140 DCSSR: its own rear label confirms G0..G6; package/Flash unknown.
//! Digital controller subset only: no load current, AC zero-crossing, heat or RF.
use esp_soc::{BoardModel,devices::Ws2812Chain,pins::PinRoutes};
#[derive(Clone,Copy)]pub enum Variant {Ac,Dc}
pub struct M5Ssr {variant:Variant,rgb:Ws2812Chain,relay:bool,leds:[[u8;3];2],updates:u64}
impl M5Ssr {
 pub fn new(variant:Variant)->Self {Self{variant,rgb:Ws2812Chain::new(1),relay:false,leds:[[0;3];2],updates:0}}
 fn publish(&mut self) {let next=[self.rgb.leds[0],if self.relay {[255,0,0]}else{[0;3]}];if next!=self.leds {self.leds=next;self.updates=self.updates.wrapping_add(1);}}
}
impl BoardModel for M5Ssr {
 fn name(&self)->&'static str {match self.variant {Variant::Ac=>"m5-acssr",Variant::Dc=>"m5-dcssr"}}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {self.relay=routes.software_output(4)==Some(true);self.publish();}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(3,true)]}
 fn named_pin(&self,id:&str)->Option<u8> {matches!(id,"button"|"mode").then_some(3)}
 fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==2 {self.rgb.from_bits(bits);self.publish();}}
 fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.updates))}
 fn report(&self)->String {format!("[{}] controller digital subset: GPIO4 relay_ctl={} RGB2={:?} button3; {} I2C slave firmware, RS485 protocol, physical load/zero-crossing/leakage/temperature and RF unmodeled",self.name(),self.relay,self.rgb.leds[0],match self.variant {Variant::Ac=>"C3FH4 4MiB;",Variant::Dc=>"C3 package/physical Flash unknown, 4MiB simulation setting;"})}
}
