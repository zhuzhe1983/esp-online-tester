//! K125 AtomS3U official pin map: G41 button, G35 WS2812, G12 IR,
//! PDM G38/G39. Only the button and hardware-RMT RGB route are modeled.
use esp_soc::{board::BoardModel,devices::Ws2812Chain};
pub struct AtomS3U {rgb:Ws2812Chain}
impl AtomS3U {pub fn new()->Self{Self{rgb:Ws2812Chain::new(1)}}}
impl Default for AtomS3U {fn default()->Self{Self::new()}}
impl BoardModel for AtomS3U {
    fn name(&self)->&'static str{"atoms3u"}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(41,true)]}
    fn named_pin(&self,id:&str)->Option<u8>{match id{"button-a"|"btnA"|"g41"=>Some(41),"boot"|"g0"=>Some(0),_=>None}}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==35{self.rgb.from_bits(bits);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String{format!("[atoms3u] G41 button, G35 RMT WS2812 updates={}; PDM/USB-OTG/IR/software-GPIO and external routing unmodeled",self.rgb.updates)}
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn physical_button_and_rgb_route_without_fake_audio(){let mut b=AtomS3U::new();let bits:Vec<bool>=[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).map(move|i|v&(0x80>>i)!=0)).collect();assert_eq!(b.named_pin("button-a"),Some(41));assert!(b.display().is_none());assert!(b.i2c_devices().is_empty());b.rmt_frame(12,&bits);assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(35,&bits);assert_eq!(b.leds().unwrap().0,&[[0x34,0x12,0x56]]);b.rmt_frame(35,&[true;23]);assert_eq!(b.leds().unwrap().1,1);}
}
