//! Distinct exact Waveshare H2 boards; identical RGB/BOOT nets do not imply C6.
use esp_soc::{Board,BoardModel,devices::Ws2812Chain};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Variant{Zero,DevKitN4}
impl Variant{
 pub fn name(self)->&'static str{match self{Self::Zero=>"waveshare-h2-zero",Self::DevKitN4=>"waveshare-h2-dev-kit-n4"}}
 pub fn from_name(name:&str)->Option<Self>{match name{"waveshare-h2-zero"=>Some(Self::Zero),"waveshare-h2-dev-kit-n4"=>Some(Self::DevKitN4),_=>None}}
}
pub struct WaveshareH2{variant:Variant,rgb:Ws2812Chain}
impl WaveshareH2{pub fn new(variant:Variant)->Self{Self{variant,rgb:Ws2812Chain::new(1)}}}
impl BoardModel for WaveshareH2{
 fn name(&self)->&'static str{self.variant.name()}
 fn named_pin(&self,id:&str)->Option<u8>{matches!(id,"boot"|"button").then_some(9)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(9,true)]}
 fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==8{self.rgb.from_bits(bits);}}
 fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.rgb.leds,self.rgb.updates))}
 fn report(&self)->String{format!("[{}] trueESP32-H2 4MiB BOOT9 RGB8 rgb={:?} updates={}; SRAM/UART/GPIO/finiteRMT subset; ROM/flash-cache/interrupt-routing/USB/radio unimplemented; noWiFi/display/touch/TF",self.name(),self.rgb.leds,self.rgb.updates)}
}
pub fn make_board(name:&str)->Option<Board>{if matches!(name,"none"|"bare"){return Some(Box::new(esp_soc::NoBoard));}Variant::from_name(name).map(|v|Box::new(WaveshareH2::new(v))as Board)}
