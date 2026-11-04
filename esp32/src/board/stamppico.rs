//! Stamp-Pico C050-B: G39/S1 with R3 5.1k pull-up and LED1 SK6812_3535/G27.
//! UIFlow2 e437118 hardware/rgb.py + driver/neopixel/sk6812.py explicitly use GRB.
//! Older STAMP-PICO90e28e6 LED.ino selects RGB; raw frames keep that R/G discrepancy.
use esp_soc::board::BoardModel;
use esp_soc::devices::Ws2812Chain;
pub struct StampPico {rgb:Ws2812Chain}
impl StampPico {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for StampPico {fn default()->Self {Self::new()}}
impl BoardModel for StampPico {
    fn name(&self)->&'static str {"stamppico"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(39,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"g39"=>Some(39),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[stamppico] SK6812 RMT updates={} rgb={:?}; EN/downloader/network/analog=unverified",self.rgb.updates,self.rgb.leds)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pico_package_identity_is_set_only_on_cold_board_attach() {
        let mut bus=crate::bus::SocBus::new(4<<20,[0;6]);
        let before=bus.periph.efuse.ram.read(0x0c);
        bus.board=Box::new(StampPico::new());bus.attach_board_devices();
        assert_eq!(bus.periph.efuse.ram.read(0x0c),(before&!(7<<9))|(5<<9));
    }
    #[test]
    fn pico_g39_g27_routes_follow_schematic_not_other_stamp_chips() {
        let mut b=StampPico::new();assert_eq!(b.input_levels(),vec![(0,true),(39,true)]);assert_eq!(b.named_pin("button"),Some(39));assert_eq!(b.named_pin("boot"),None);
        let bits:Vec<bool>=[0x12u8,0x34,0x56].into_iter().flat_map(|v|(0..8).map(move |i|v&(0x80>>i)!=0)).collect();
        for pin in [21,35,12] {b.rmt_frame(pin,&bits);}assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
        b.rmt_frame(27,&bits);assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));b.rmt_frame(27,&[true;23]);assert_eq!(b.leds().unwrap().1,1);
    }
}
