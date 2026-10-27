//! Stamp-S3 S007 v0.2 schematic and STAMP-S3 6b64d431 LED example:
//! G0 active-low user/boot switch, one WS2812B-2020 GRB LED on G21.
//! 3V3 is virtually present; no analog supply, display FPC or physical networking.
use esp_soc::board::{BoardModel,VirtualCycle};
use esp_soc::devices::Ws2812Chain;
pub struct StampS3 {rgb:Ws2812Chain,gated:bool,powered:bool}
impl StampS3 {
    pub fn new()->Self {Self{rgb:Ws2812Chain::new(1),gated:false,powered:true}}
    // S007-V033 schematic v0.3.3: G38/DISP_BL -> AW35122FDR EN -> BL_3V3.
    // Digital subset requires an actively driven high; analog ramp/leakage is absent.
    pub fn s3a()->Self {Self{gated:true,powered:false,..Self::new()}}
}
impl Default for StampS3 {fn default()->Self {Self::new()}}
impl BoardModel for StampS3 {
    fn name(&self)->&'static str {if self.gated {"stamps3a"}else{"stamps3"}}
    fn gpio_output_at(&mut self,_cycle:VirtualCycle,_changes:&[(u8,bool)],enabled:u64,output:u64) {
        if !self.gated {return;}
        let powered=enabled&output&(1u64<<38)!=0;
        if self.powered && !powered {self.rgb.leds.fill([0;3]);self.rgb.updates+=1;}
        self.powered=powered;
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==21 && self.powered {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(46,false)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"boot"|"g0"=>Some(0),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {if self.gated {format!("[stamps3a] WS2812B powered={} RMT updates={} rgb={:?} software-GPIO/analog/network=unverified",self.powered,self.rgb.updates,self.rgb.leds)}else{format!("[stamps3] WS2812B RMT updates={} rgb={:?} software-GPIO/analog/network=unverified",self.rgb.updates,self.rgb.leds)}}
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
    #[test]
    fn s3a_supply_requires_g38_output_and_new_frame_after_power_returns() {
        let mut b=StampS3::s3a();let bits=vec![true;24];let gate=1u64<<38;
        assert_eq!(b.name(),"stamps3a");b.rmt_frame(21,&bits);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],0));
        b.gpio_output_at(0,&[],0,gate);b.rmt_frame(21,&bits);assert_eq!(b.leds().unwrap().1,0,"input latch cannot supply the LED");
        b.gpio_output_at(1,&[],gate,gate);b.rmt_frame(21,&bits);assert_eq!(b.leds().unwrap(),(&[[255;3]][..],1));
        b.gpio_output_at(2,&[],gate,0);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],2));b.rmt_frame(21,&bits);assert_eq!(b.leds().unwrap().1,2);
        b.gpio_output_at(3,&[],gate,gate);assert_eq!(b.leds().unwrap(),(&[[0;3]][..],2));b.rmt_frame(21,&bits);assert_eq!(b.leds().unwrap().0,&[[255;3]]);
        b.gpio_output_at(4,&[],0,gate);assert_eq!(b.leds().unwrap().0,&[[0;3]],"changing G38 to input cuts virtual supply");
        let mut original=StampS3::new();original.gpio_output_at(0,&[],0,0);original.rmt_frame(21,&bits);assert_eq!(original.leds().unwrap().0,&[[255;3]],"base S3 has direct supply");
    }
}
