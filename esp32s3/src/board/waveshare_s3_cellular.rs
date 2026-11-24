//! Waveshare A7670E/SIM7670G V2: exact 16F/8P, RGB38 and one-bit SD5/4/6.
//! Sources: retained V2 schematic and Arduino3.3.4 RGB/SD examples, not HAT pin aliases.
//! No synthetic modem ACK, network, GNSS, camera image or fuel-gauge measurement.
use esp_soc::{board::{Board,BoardModel},devices::Ws2812Chain,pins::PinRoutes};
const HZ:u32=240_000_000;
pub fn make_board(name:&str)->Option<Board>{let name=match name{"waveshare-s3-a7670e-4g-v2"=>"waveshare-s3-a7670e-4g-v2","waveshare-s3-sim7670g-4g-v2"=>"waveshare-s3-sim7670g-4g-v2",_=>return None};Some(Box::new(Cellular{ name,led:Ws2812Chain::new(1).with_gpio_clock(HZ),present:false }))}
struct Cellular{name:&'static str,led:Ws2812Chain,present:bool}
impl BoardModel for Cellular{
 fn name(&self)->&'static str{self.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn uses_gpio_waveform(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,p:PinRoutes<'_>){let level=p.software_output(38);self.led.gpio_drive(c,level.is_some(),level.unwrap_or(false));}
 fn rmt_frame(&mut self,p:u8,b:&[bool]){if p==38{self.led.from_bits(b);}}
 fn next_deadline(&self)->Option<u64>{self.led.gpio_deadline()}
 fn advance_to(&mut self,c:u64){self.led.advance_gpio(c);}
 fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led.leds,self.led.updates))}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([5,4,6,255,255,255],self.present))}
 fn set_sd_card_present(&mut self,p:bool){self.present=p;}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true)]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn report(&self)->String{format!("{}: current V2 ESP32-S3R8 16MiB Flash/8MiB PSRAM; WS2812B GPIO38 GRB/800kHz updates={} RGB={:?}; native1bit TF CLK5/CMD4/D0=6 inserted={}. BOOT0 active-low only. V2 TF DAT3/SD_CS is a separate net; old example SD_CD46 names a data pin, not a proven insertion switch. MAX17048@36 V2 SDA15/SCL16 (V1 SDA3/SCL2), UART modem TX17/RX18, RI40/DTR45, USB19/20, camera FPC and switched supply are source facts only. No modeled gauge measurement/I2C ACK, modem AT/USB CDC/PPP, cellular RF, GNSS position, camera capture, audio, DIP/solar/18650 analog power, physical acceptance or complete application success.",self.name,self.led.updates,self.led.leds,self.present)}
}
#[cfg(test)]mod tests{use super::*;
 fn bits(b:&[u8])->Vec<bool>{b.iter().flat_map(|x|(0..8).map(move|i|x&(0x80>>i)!=0)).collect()}
 #[test]fn both_modems_have_exact_led_card_and_no_fabricated_peripherals(){for name in["waveshare-s3-a7670e-4g-v2","waveshare-s3-sim7670g-4g-v2"]{let mut b=make_board(name).unwrap();b.rmt_frame(48,&bits(&[1,2,3]));assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(38,&bits(&[0x22,0x11,0x33]));assert_eq!(b.leds().unwrap().0,[[0x11,0x22,0x33]]);assert_eq!(b.sd_card_slot(),Some(([5,4,6,255,255,255],false)));b.set_sd_card_present(true);assert!(b.sd_card_slot().unwrap().1);b.set_sd_card_present(false);assert!(!b.sd_card_slot().unwrap().1);assert!(b.display().is_none());assert!(b.i2c_devices().is_empty());assert!(!b.uses_uart_pins());assert_eq!(b.named_pin("boot"),Some(0));assert_eq!(b.named_pin("power"),None);assert!(b.set_camera_picture(esp_soc::picture::Picture{w:1,h:1,rgb:vec![0;3]}).is_err());}}
}
