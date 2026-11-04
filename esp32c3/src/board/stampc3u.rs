//! Stamp-C3U C122-B: S2/G9 user button, U3/G2 SK6812 and native USB18/19.
//! Official stamp_c3u_sch_01: G8/R2 and G9/R6 have 10k pull-ups; S1 drives EN.
//! STAMP-C3 148c263f IDF example selects BTN_PIN9 for STAMP_C3U; Arduino uses GRB.
use esp_soc::board::BoardModel;
use esp_soc::devices::Ws2812Chain;

pub struct StampC3U {rgb:Ws2812Chain}
impl StampC3U {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for StampC3U {fn default()->Self {Self::new()}}
impl BoardModel for StampC3U {
    fn name(&self)->&'static str {"stampc3u"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==2 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(8,true),(9,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"g9"=>Some(9),_=>None}}
    fn uses_gpio_edges(&self)->bool {false}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[stampc3u] SK6812 RMT updates={} rgb={:?} EN/USB-transport/analog/network=unverified",self.rgb.updates,self.rgb.leds)}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_button_is_g9_and_never_c3_g3_or_en() {
        let mut b=StampC3U::new();assert_eq!(b.named_pin("button"),Some(9));assert_eq!(b.named_pin("g3"),None);assert_eq!(b.named_pin("reset"),None);
        assert_eq!(b.input_levels(),[(8,true),(9,true)]);assert!(b.display().is_none());
        let bits:Vec<bool>=[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).map(move |i|v&(0x80>>i)!=0)).collect();
        b.rmt_frame(9,&bits);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
        b.rmt_frame(2,&bits);assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));
        assert_eq!(StampC3U::new().leds().unwrap(),(&[[0;3]][..],0));
    }
}
