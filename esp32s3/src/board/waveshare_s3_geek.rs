//! Exact ESP32-S3-GEEK SKU26405; not C6-GEEK GPIO or memory aliases.
//! Pinned sources: waveshare-s3-tail-second-five-sources manifest. 135x240
//! native ST7789 aperture is presented as the manufacturer's 240x135 landscape.
use esp_periph::sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard};
use esp_soc::{board::{BoardModel,SpiQspiPins},devices::DcsPanel,pins::PinRoutes};
pub struct WaveshareS3Geek{panel:DcsPanel,command:u8,lcm:u8,reset:bool,dc:Option<bool>,gpio_light:u32,pwm:Option<u32>,light:u32,version:u64,rejected:u64,sd:Option<SpiSdCard>,present:bool,sd_rejected:u64}
impl WaveshareS3Geek{
 pub fn new()->Self{Self{panel:DcsPanel::st7789(),command:0,lcm:0x2c,reset:false,dc:None,gpio_light:0,pwm:None,light:0,version:0,rejected:0,sd:None,present:false,sd_rejected:0}}
 fn sync_light(&mut self){let d=self.pwm.unwrap_or(self.gpio_light);if d!=self.light{self.light=d;self.version+=1;}}
 fn single(p:SpiQspiPins)->bool{!p.unsupported_mode&&!p.sio&&!p.clock_output_edge&&p.dummy_cycles==0&&p.address_bits==0&&p.command_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8)&&p.read_bits.is_multiple_of(8)&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|v|*v<=1)}
}
impl BoardModel for WaveshareS3Geek{
 fn name(&self)->&'static str{"waveshare-s3-geek"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,_c:u64,r:PinRoutes<'_>){let reset=r.software_output(9)==Some(true);if self.reset&&!reset{self.panel.reset();self.command=0;self.lcm=0x2c;}
  if self.reset!=reset{self.version+=1;}self.reset=reset;self.dc=r.software_output(8);self.gpio_light=if r.software_output(7)==Some(true){65535}else{0};self.sync_light();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,_c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.pwm=f(7).map(|(_,d)|d.min(65535));self.sync_light();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{
  if p.sclk&(1<<36)!=0&&p.data[0]&(1<<35)!=0{let valid=self.present&&Self::single(p)&&!p.clock_idle_high&&p.command_bits==0;if !valid{self.sd_rejected+=1;return vec![255;rx];}
   if let Some(card)=&mut self.sd{let reply=card.transfer(p.cs&(1<<34)!=0,tx,rx);if p.input[1]==Some(37){return reply;}}return vec![255;rx];}
  if p.cs&(1<<10)==0{return vec![255;rx];}
  if !self.reset||self.dc.is_none()||!Self::single(p)||p.sclk&(1<<12)==0||p.data[0]&(1<<11)==0{self.rejected+=1;return vec![255;rx];}self.panel.dc=self.dc==Some(true);
  if self.panel.dc&&matches!(self.command,0x2c|0x3c)&&self.panel.colmod&7!=5{self.rejected+=1;return vec![255;rx];}
  for &v in tx{if !self.panel.dc{self.command=v;if v==1{self.lcm=0x2c;}}else if self.command==0xc0{self.lcm=v;}self.panel.byte(v);}self.version+=1;vec![255;rx]
 }
 fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{let card=SpiSdCard::new(image,config)?;let desc=card.describe();self.sd=Some(card);self.present=true;Ok(desc)}
 fn set_sd_card_present(&mut self,p:bool){if self.present!=p{if let Some(card)=&mut self.sd{card.power_cycle();}}self.present=p;}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true)]}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let mut f=vec![0;240*135];if self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping{for y in 0..135{for x in 0..240{
  let mut col=186-y;let mut row=40+x;if self.lcm&8==0{col=239-col;}
  if self.lcm&0x40!=0{row=319-row;}let mut value=self.panel.gram[row*240+col];if self.panel.madctl&8!=0{value=(value&0x7e0)|(value>>11)|((value&31)<<11);}let d=self.light;let v=u32::from(value);f[y*240+x]=((((v>>11)*d/65535)<<11)|((((v>>5)&63)*d/65535)<<5)|((v&31)*d/65535))as u16;
 }}}Some((240,135,f,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("ESP32-S3-GEEK SKU26405 16MiBFlash/2MiBPSRAM; ST7789 240x320GRAM native135x240 aperture col52..186,row40..279 presented240x135 landscape; CLK12 MOSI11 CS10 DC8 RST9 BL7. SPI TF CLK36 MOSI35 MISO37 CS34; BOOT0 only. pixels={} RAMWR={} madctl={:02x} lcm={:02x} colmod={:02x} rejected={} SDreject={}. LCD TX continues during full-duplex host reads; unwired readback FF; modes0/3 use outEdge0. OriginalLCD_Driver sample MADCTL70 with135x240+52/40 bounds differs from canonical landscape aperture and is not rewritten. No invented RTC/IMU/touch/I2C device, nativeSDMMC, USB host peripherals, RF, analog gamma/LCM XBGR-XMV-XINV, physical glass/brightness or full application acceptance.",self.panel.pixels_written,self.panel.frames,self.panel.madctl,self.lcm,self.panel.colmod,self.rejected,self.sd_rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareS3Geek,dc:bool,on:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(p,v)in[(7,on),(8,dc),(9,on)]{g.enable|=1<<p;if v{g.out|=1<<p;}m.write(4+4*p,1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));b.gpio_pwm_at(0,&|_|None);}
 fn pins(n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<12,data:[1<<11,0,0,0],cs:1<<10,data_width:1,data_bits:n as u32*8,clock_idle_high:true,..Default::default()}}
 fn cmd(b:&mut WaveshareS3Geek,c:u8,a:&[u8]){route(b,false,true);b.spi_transfer_qspi_pins(2,pins(1),&[c],0);route(b,true,true);b.spi_transfer_qspi_pins(2,pins(a.len()),a,0);}
 #[test]fn exact_landscape_aperture_modes_wrong_routes_reset_and_backlight(){let mut b=WaveshareS3Geek::new();for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x3a,&[5]);cmd(&mut b,0x36,&[0x60]);cmd(&mut b,0x2a,&[0,40,0,40]);cmd(&mut b,0x2b,&[0,53,0,53]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);let n=b.panel.pixels_written;let mut wrong=pins(2);wrong.sclk=1<<11;b.spi_transfer_qspi_pins(2,wrong,&[0,31],0);wrong=pins(2);wrong.clock_output_edge=true;b.spi_transfer_qspi_pins(2,wrong,&[0,31],0);assert_eq!(n,b.panel.pixels_written);let mut duplex=pins(2);duplex.full_duplex=true;duplex.read_bits=16;duplex.read_width=1;assert_eq!(b.spi_transfer_qspi_pins(2,duplex,&[7,0xe0],2),[255,255]);assert_eq!(b.display().unwrap().2[0],0x7e0);b.gpio_pwm_at(0,&|p|(p==7).then_some((1000.,0)));assert_eq!(b.display().unwrap().2[0],0);route(&mut b,true,false);assert!(!b.panel.on);assert_eq!(b.named_pin("boot"),Some(0));assert!(b.i2c_devices().is_empty());assert!(b.sd_card_slot().is_none());}
 #[test]fn distinct_spi_tf_bus_chip_select_and_unwired_readback(){let mut b=WaveshareS3Geek::new();b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();let mut p=SpiQspiPins{sclk:1<<36,data:[1<<35,0,0,0],input:[None,Some(37),None,None],data_width:1,data_bits:80,..Default::default()};b.spi_transfer_qspi_pins(3,p,&[255;10],0);p.cs=1<<34;p.data_bits=48;p.read_bits=64;p.read_width=1;assert!(b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8).contains(&1));p.input[1]=Some(13);assert_eq!(b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8),[255;8]);b.set_sd_card_present(false);p.input[1]=Some(37);assert_eq!(b.spi_transfer_qspi_pins(3,p,&[255],1),[255]);}
}
