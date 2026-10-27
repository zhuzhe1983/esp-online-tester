//! Stamp-S3 S007 v0.2 schematic and STAMP-S3 6b64d431 LED example:
//! G0 active-low user/boot switch, one WS2812B-2020 GRB LED on G21.
//! 3V3 is virtually present; no analog supply, display FPC or physical networking.
use esp_soc::board::BoardModel;
use esp_soc::devices::Ws2812Chain;
pub struct StampS3 {rgb:Ws2812Chain}
impl StampS3 {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for StampS3 {fn default()->Self {Self::new()}}
impl BoardModel for StampS3 {
    fn name(&self)->&'static str {"stamps3"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==21 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(46,false)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"boot"|"g0"=>Some(0),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[stamps3] WS2812B RMT updates={} rgb={:?} software-GPIO/analog/network=unverified",self.rgb.updates,self.rgb.leds)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn led_pin_grb_button_and_no_sibling_peripherals() {
        let mut b=StampS3::new();assert_eq!(b.named_pin("button"),Some(0));assert_eq!(b.input_levels(),[(0,true),(46,false)]);assert!(b.display().is_none());assert!(b.i2c_devices().is_empty());
        let bits:Vec<bool>=[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).map(move |i|v&(0x80>>i)!=0)).collect();
        b.rmt_frame(35,&bits);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));b.rmt_frame(21,&bits);assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));b.rmt_frame(21,&[true;23]);assert_eq!(b.leds().unwrap().1,1);
    }
}
