//! Atom Voice / AtomEcho C008-C official PinMap and M5-ProductExampleCodes
//! 1bc38736 factory RMT source: G39 button, G27 single GRB SK6812.
//! NS4168 G22/19/33 and SPM1423 G33/23 are reserved but not modelled here.
use esp_soc::board::BoardModel;
use esp_soc::devices::Ws2812Chain;
pub struct AtomVoice {rgb:Ws2812Chain}
impl AtomVoice {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for AtomVoice {fn default()->Self {Self::new()}}
impl BoardModel for AtomVoice {
    fn name(&self)->&'static str {"atom-voice"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(39,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"g39"=>Some(39),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[atom-voice] SK6812 RMT updates={} rgb={:?}; NS4168/SPM1423/classic-BT/A2DP unmodeled",self.rgb.updates,self.rgb.leds)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn voice_routes_led_without_claiming_audio_pins() {
        let mut b=AtomVoice::new();assert_eq!(b.name(),"atom-voice");assert_eq!(b.input_levels(),vec![(39,true)]);assert_eq!(b.named_pin("button"),Some(39));
        let bits:Vec<bool>=[0u8,5,0].into_iter().flat_map(|v|(0..8).map(move|i|v&(0x80>>i)!=0)).collect();
        for pin in [19,22,23,33] {b.rmt_frame(pin,&bits);}assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
        b.rmt_frame(27,&bits);assert_eq!(b.leds().unwrap(),(&[[5,0,0]][..],1));b.rmt_frame(27,&bits[..23]);assert_eq!(b.leds().unwrap().1,1);assert!(b.display().is_none());
    }
}
