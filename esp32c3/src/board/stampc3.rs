//! Stamp-C3 C056-B: S2/G3 user button and one U4/G2 SK6812 on the official schematic.
//! STAMP-C3 148c263f Arduino LED_RGBW uses 24-bit TYPE_GRB despite its folder name.
//! IDF RGB_LED_Control sets G3 input pull-up; G8/R2 and G9/R12 are pulled high.
use esp_soc::board::BoardModel;
use esp_soc::devices::Ws2812Chain;

pub struct StampC3 {rgb:Ws2812Chain}
impl StampC3 {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for StampC3 {fn default()->Self {Self::new()}}
impl BoardModel for StampC3 {
    fn name(&self)->&'static str {"stampc3"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==2 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(3,true),(8,true),(9,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"g3"=>Some(3),_=>None}}
    fn uses_gpio_edges(&self)->bool {false}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[stampc3] SK6812 RMT updates={} rgb={:?} EN/USB-bridge/analog/network=unverified",self.rgb.updates,self.rgb.leds)}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_button_and_grb_pin_reject_wrong_routes_and_short_frames() {
        let mut b=StampC3::new();assert_eq!(b.named_pin("button"),Some(3));assert_eq!(b.named_pin("boot"),None);assert_eq!(b.named_pin("reset"),None);
        assert_eq!(b.input_levels(),[(3,true),(8,true),(9,true)]);assert!(b.display().is_none());assert!(b.i2c_devices().is_empty());
        let bits:Vec<bool>=[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).map(move |i|v&(0x80>>i)!=0)).collect();
        b.rmt_frame(3,&bits);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
        b.rmt_frame(2,&bits);assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));
        b.rmt_frame(2,&[true;23]);assert_eq!(b.leds().unwrap().1,1);
        assert_eq!(StampC3::new().leds().unwrap(),(&[[0;3]][..],0));
    }
}
