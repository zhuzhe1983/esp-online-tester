//! M5Stack AtomS3-Lite C124: one WS2812C-2020 on G35 and active-low button G41.
//! Official Sch_M5_AtomS3_v1.0.pdf (linked by the C124 page) and M5AtomS3
//! 440c112f LedDisplay.h/.cpp. IR G4, analog power/reset and radios are not modeled.
//! Reuses the shared WS2812-class decoder policy, not calibrated LED timing.
use esp_soc::board::BoardModel;
use esp_soc::devices::Ws2812Chain;

pub struct AtomS3Lite {rgb:Ws2812Chain}
impl AtomS3Lite {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for AtomS3Lite {fn default()->Self {Self::new()}}
impl BoardModel for AtomS3Lite {
    fn name(&self)->&'static str {"atoms3-lite"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==35 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(41,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"button"|"btnA"|"g41"=>Some(41),"boot"|"g0"=>Some(0),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[atoms3-lite] WS2812C RMT updates={} rgb={:?} IR/software-GPIO=unimplemented",self.rgb.updates,self.rgb.leds)}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bits(bytes:&[u8])->Vec<bool> {bytes.iter().flat_map(|b|(0..8).map(move |i|b&(0x80>>i)!=0)).collect()}
    #[test]
    fn physical_rgb_route_grb_button_and_no_invented_peripherals() {
        let mut b=AtomS3Lite::new();assert_eq!(b.input_levels(),[(0,true),(41,true)]);assert_eq!(b.named_pin("button"),Some(41));
        assert!(b.display().is_none());assert!(b.i2c_devices().is_empty());assert!(!b.set_imu_sample([0.;3],[0.;3]));
        b.rmt_frame(4,&bits(&[1,2,3]));assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
        b.rmt_frame(35,&bits(&[0x12,0x34,0x56]));assert_eq!(b.leds().unwrap(),(&[[0x34,0x12,0x56]][..],1));
        b.rmt_frame(35,&[true;23]);assert_eq!(b.leds().unwrap().1,1);
    }
    #[test]
    fn software_gpio_does_not_invent_rgb_without_ex218_timing_opt_in() {
        let mut b=AtomS3Lite::new();assert!(!b.uses_gpio_waveform());
        for (i,bit) in bits(&[0x78,0x9a,0xbc]).iter().enumerate() {
            let t=i as u64*300;b.gpio_output_at(t,&[(35,true)],1<<35,1<<35);
            b.gpio_output_at(t+if *bit {192}else{96},&[(35,false)],1<<35,0);
        }
        b.advance_to(100000);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
    }
}
