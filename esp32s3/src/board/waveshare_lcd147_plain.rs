//! Plain LCD1.47, not the B/touch products: official Test BSP and schematic.
//! SPI40/45/42/DC41/RST39, active-high BL48, one WS2812 at38, native4bit TF.
use esp_soc::{board::{BoardModel,SpiQspiPins},devices::{DcsPanel,Ws2812Chain},pins::PinRoutes};
pub struct WaveshareLcd147Plain { panel:DcsPanel,command:u8,dc:Option<bool>,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,version:u64,rejected:u64,last_rejected:Option<SpiQspiPins>,bad_format:u64,led:Ws2812Chain,card:bool }
impl Default for WaveshareLcd147Plain { fn default()->Self{Self::new()} }
impl WaveshareLcd147Plain {
 pub fn new()->Self{Self{panel:DcsPanel::st7789(),command:0,dc:None,reset:false,light:0,gpio_light:0,pwm:None,version:0,rejected:0,last_rejected:None,bad_format:0,led:Ws2812Chain::new(1),card:false}}
 fn sync(&mut self){let l=self.pwm.unwrap_or(self.gpio_light);if l!=self.light{self.light=l;self.version+=1;}}
}
impl BoardModel for WaveshareLcd147Plain {
 fn name(&self)->&'static str{"waveshare-s3-lcd147"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,_c:u64,r:PinRoutes<'_>){let reset=r.software_output(39)==Some(true);if self.reset&&!reset{self.panel.reset();self.command=0;}
 if self.reset!=reset{self.version+=1;}self.reset=reset;self.dc=r.software_output(41);self.gpio_light=if r.software_output(48)==Some(true){65535}else{0};self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,_c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.pwm=f(48).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{
  if p.cs&(1<<42)==0{return vec![255;rx];}
  let valid=self.reset&&self.dc.is_some()&&p.sclk&(1<<40)!=0&&p.data[0]&(1<<45)!=0&&!p.unsupported_mode&&!p.sio&&!p.clock_output_edge&&p.dummy_cycles==0&&p.address_bits==0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|w|*w<=1)&&p.command_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8);
  if !valid{self.rejected+=1;self.last_rejected=Some(p);return vec![255;rx];}self.panel.dc=self.dc==Some(true);
  if self.panel.dc&&matches!(self.command,0x2c|0x3c)&&self.panel.colmod&7!=5{self.rejected+=1;self.bad_format+=1;return vec![255;rx];}
  for &v in b{if !self.panel.dc{self.command=v;}self.panel.byte(v);}self.version+=1;vec![255;rx]
 }
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true)]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn rmt_frame(&mut self,p:u8,b:&[bool]){if p==38{self.led.from_bits(b);}}
 fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led.leds,self.led.updates))}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([14,15,16,18,17,21],self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let mut f=vec![0;172*320];if self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping{for y in 0..320{for x in 0..172{let mut p=self.panel.gram[y*240+239-(x+34)];if self.panel.madctl&8!=0{p=(p&0x7e0)|(p>>11)|((p&31)<<11);}let v=u32::from(p);let d=self.light;f[y*172+x]=((((v>>11)*d/65535)<<11)|((((v>>5)&63)*d/65535)<<5)|((v&31)*d/65535))as u16;}}}Some((172,320,f,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("{} plain16F8P ST7789 native240x320/visible172x320 Xgap34 referenceMADCTL40; SPI40/45/CS42/DC41/RST39 BL48high; one WS2812 GRB38; SDMMC4bit14/15/16/18/17/21. pixels={} frames={} rejected={} badformat={} MADCTL={:02x} COLMOD={:02x} lastRejected={:?}. No touch/IMU/RTC device fitted or invented; analog display response and acrylic LED optical spreading uncalibrated, complete original app and physical hardware unvalidated.",self.name(),self.panel.pixels_written,self.panel.frames,self.rejected,self.bad_format,self.panel.madctl,self.panel.colmod,self.last_rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareLcd147Plain,dc:bool,on:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(p,v)in[(39,on),(48,on),(41,dc)]{g.enable|=1<<p;if v{g.out|=1<<p;}m.write(4+4*p,1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins(n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<40,data:[1<<45,0,0,0],cs:1<<42,data_width:1,data_bits:n as u32*8,..Default::default()}}
 fn cmd(b:&mut WaveshareLcd147Plain,c:u8,a:&[u8]){route(b,false,true);b.spi_transfer_qspi_pins(3,pins(1),&[c],0);route(b,true,true);b.spi_transfer_qspi_pins(3,pins(a.len()),a,0);}
 #[test]fn plain_panel_window_full_duplex_and_wrong_b_variant_pins(){let mut b=WaveshareLcd147Plain::new();cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0x40]);cmd(&mut b,0x2a,&[0,34,0,35]);cmd(&mut b,0x2b,&[0,19,0,19]);cmd(&mut b,0x2c,&[]);let mut p=pins(4);p.full_duplex=true;p.clock_idle_high=true;assert_eq!(b.spi_transfer_qspi_pins(3,p,&[0xf8,0,0x07,0xe0],4),[255;4]);assert_eq!(&b.display().unwrap().2[19*172..19*172+2],&[0xf800,0x07e0]);let n=b.panel.pixels_written;p.data[0]=1<<46;b.spi_transfer_qspi_pins(3,p,&[0,0],2);assert_eq!(b.panel.pixels_written,n);route(&mut b,true,false);assert!(b.display().unwrap().2.iter().all(|v|*v==0));}
 #[test]fn exact_single_led_and_native_sd_and_duty(){let mut b=WaveshareLcd147Plain::new();let bits=(0..24).rev().map(|i|(0x123456>>i)&1!=0).collect::<Vec<_>>();b.rmt_frame(15,&bits);assert_eq!(b.leds().unwrap().0,[[0,0,0]]);b.rmt_frame(38,&bits);assert_eq!(b.leds().unwrap().0,[[0x34,0x12,0x56]]);b.gpio_pwm_at(0,&|p|(p==48).then_some((3000.0,32768)));assert_eq!(b.light,32768);b.gpio_pwm_at(0,&|p|(p==48).then_some((9000.0,32768)));assert_eq!(b.light,32768);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([14,15,16,18,17,21],true)));assert!(b.i2c_devices().is_empty());}
}
