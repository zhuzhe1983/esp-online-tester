//! Atom-Lite C008 official schematic: G39 active-low key with external 4.7k pullup,
//! G27 single SK6812 3535. M5Atom ebbf7363 factory example uses 24-bit GRB.
//! Digital RMT subset only: no IR, physical networking, Grove device or analog power.
use esp_soc::board::BoardModel;
use esp_soc::devices::Ws2812Chain;
pub struct AtomLite {rgb:Ws2812Chain}
impl AtomLite {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for AtomLite {fn default()->Self {Self::new()}}
impl BoardModel for AtomLite {
    fn name(&self)->&'static str {"atom-lite"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(39,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"g39"=>Some(39),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[atom-lite] SK6812 RMT updates={} rgb={:?}; IR/network/analog=unverified",self.rgb.updates,self.rgb.leds)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_atom_key_and_rgb_route_are_not_s3_or_stamp_pins() {
        let mut b=AtomLite::new();assert_eq!(b.input_levels(),vec![(39,true)]);assert_eq!(b.named_pin("button"),Some(39));assert_eq!(b.named_pin("boot"),None);
        let bits:Vec<bool>=[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).map(move |i|v&(0x80>>i)!=0)).collect();
        for pin in [21,35,12] {b.rmt_frame(pin,&bits);}assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
        b.rmt_frame(27,&bits);assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));b.rmt_frame(27,&[true;23]);assert_eq!(b.leds().unwrap().1,1);
    }
}
