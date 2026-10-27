//! M5Stack AtomS3-Lite C124: one WS2812C-2020 on G35 and active-low button G41.
//! Official Sch_M5_AtomS3_v1.0.pdf (linked by the C124 page) and M5AtomS3
//! 440c112f LedDisplay.h/.cpp. IR G4, analog power/reset and radios are not modeled.
//! Reuses the shared WS2812-class decoder policy, not calibrated LED timing.
use esp_soc::board::{BoardModel,VirtualCycle};
use esp_soc::devices::Ws2812Chain;

pub struct AtomS3Lite {rgb:Ws2812Chain,gpio_events:u64}
impl AtomS3Lite {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1).with_gpio_clock(crate::periph::CPU_HZ as u32),gpio_events:0}}}
impl Default for AtomS3Lite {fn default()->Self {Self::new()}}
impl BoardModel for AtomS3Lite {
    fn name(&self)->&'static str {"atoms3-lite"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_events+=changes.len() as u64;
        self.rgb.gpio_drive(cycle,enabled&(1<<35)!=0,output&(1<<35)!=0);
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==35 {self.rgb.from_bits(bits);}}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.rgb.advance_gpio(cycle);}
    fn next_deadline(&self)->Option<VirtualCycle> {self.rgb.gpio_deadline()}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(41,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"button"|"btnA"|"g41"=>Some(41),"boot"|"g0"=>Some(0),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn report(&self)->String {format!("[atoms3-lite] WS2812C updates={} rgb={:?} IR=unimplemented",self.rgb.updates,self.rgb.leds)}
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
    fn gpio_waveform_uses_g35_output_enable_and_latches_after_quiet() {
        let mut b=AtomS3Lite::new();let mut t=0;
        for pin in [4,35] {for bit in bits(&[0x78,0x9a,0xbc]) {
            b.gpio_output_at(t,&[],1<<pin,1<<pin);t+=if bit {192}else{96};
            b.gpio_output_at(t,&[],1<<pin,0);t+=204;
        }t+=240*300;b.advance_to(t);}
        assert_eq!(b.leds().unwrap(),(&[[0x9a,0x78,0xbc]][..],1));
        b.gpio_output_at(t,&[],1<<35,1<<35);b.gpio_output_at(t+96,&[],0,0);b.advance_to(t+240*300);
        assert_eq!(b.leds().unwrap().1,1);assert_eq!(b.next_deadline(),None);
    }
}
