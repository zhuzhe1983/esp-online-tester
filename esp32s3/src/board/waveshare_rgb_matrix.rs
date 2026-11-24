//! ESP32-S3 RGB Matrix controller and exact kit grid identities.
//! The physical HUB75 pin order is from the pinned manufacturer sdkconfig.
//! This is a standard two-row, one-bit scan diagnostic. PWM integration,
//! nonstandard F/B scan remapping and panel driver configuration are absent.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,LcdI80Pins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210}};
const HZ:u64=240_000_000;
const COLORS:[u8;6]=[4,5,6,7,15,16];const ROWS:[u8;5]=[18,8,3,42,9];
const PROFILES:&[(&str,usize,usize)]=&[("waveshare-s3-rgb-matrix",0,0),("waveshare-s3-rgb-matrix-p2-64x64",64,64),("waveshare-s3-rgb-matrix-p2-64x64-b",64,64),("waveshare-s3-rgb-matrix-p3-64x64",64,64),("waveshare-s3-rgb-matrix-p3-64x64-f",64,64),("waveshare-s3-rgb-matrix-p5-64x32",64,32),("waveshare-s3-rgb-matrix-p25-64x32",64,32),("waveshare-s3-rgb-matrix-p25-64x32-b",64,32),("waveshare-s3-rgb-matrix-p25-96x48-f",96,48)];
pub fn memory_mib(name:&str)->Option<(usize,usize)>{PROFILES.iter().any(|x|x.0==name).then_some((32,16))}
pub fn make_board(name:&str)->Option<Board>{PROFILES.iter().find(|p|p.0==name).map(|&(n,w,h)|Box::new(WaveshareRgbMatrix::new(n,w,h))as Board)}
pub struct WaveshareRgbMatrix{name:&'static str,w:usize,h:usize,shift:Vec<u8>,latched:Vec<u8>,image:Vec<u16>,clock:bool,latch:bool,version:u64,cycles:u64,i2c:I2cBitBang,rtc:Arc<Mutex<Pcf85063State>>,attached:bool,card:bool}
impl WaveshareRgbMatrix{
 fn new(name:&'static str,w:usize,h:usize)->Self{Self{name,w,h,shift:vec![0;w],latched:vec![0;w],image:vec![0;w*h],clock:false,latch:false,version:0,cycles:0,i2c:I2cBitBang::new(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),attached:false,card:false}}
 fn signals(&mut self,levels:u64,valid:u64,clock:bool){if self.w==0{return;}let required=COLORS.into_iter().chain(ROWS).chain([40,2]).fold(0,|v,p|v|(1u64<<p));if valid&required!=required{return;}
 if clock&&!self.clock{self.shift.rotate_left(1);self.shift[self.w-1]=COLORS.into_iter().enumerate().fold(0,|v,(i,p)|v|(((levels>>p)&1)as u8)<<i);}
 let latch=levels&(1<<40)!=0;if latch&&!self.latch{self.latched.copy_from_slice(&self.shift);}
 if levels&(1<<2)==0{let row=ROWS.into_iter().enumerate().fold(0usize,|v,(i,p)|v|(((levels>>p)&1)as usize)<<i);if row<self.h/2{let mut changed=false;for x in 0..self.w{for half in 0..2{let bits=(self.latched[x]>>(half*3))&7;let color=if bits&1!=0{0xf800}else{0}|if bits&2!=0{0x07e0}else{0}|if bits&4!=0{0x001f}else{0};let at=(row+half*self.h/2)*self.w+x;if self.image[at]!=color{self.image[at]=color;changed=true;}}}if changed{self.version+=1;}}}
 self.clock=clock;self.latch=latch;}
}
impl BoardModel for WaveshareRgbMatrix{
 fn name(&self)->&'static str{self.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn uses_gpio_waveform(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let mut levels=0;let mut valid=0;for p in COLORS.into_iter().chain(ROWS).chain([40,2,41]){if let Some(high)=r.software_output(p){valid|=1u64<<p;if high{levels|=1u64<<p;}}}if valid&(1<<41)!=0{self.signals(levels,valid,levels&(1<<41)!=0);}}
 fn lcd_i80(&mut self,p:LcdI80Pins,_data:bool,words:&[u16]){if p.clock&(1<<41)==0||p.width!=16{return;}let valid=p.data.into_iter().fold(0,|a,b|a|b);for &word in words{let levels=p.data.into_iter().enumerate().fold(0,|m,(i,p)|if word&(1<<i)!=0{m|p}else{m});self.signals(levels,valid,false);self.signals(levels,valid,true);}}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((47,48))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycles=c;self.rtc.lock().expect("Matrix RTC").advance_to(c);}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true)]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([1,44,17,255,255,255],self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{(self.w>0).then(||(self.w as u32,self.h as u32,self.image.clone(),self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.version}
 fn report(&self)->String{format!("{}32F16P HUB75 CLK41 LAT40 OE2 RGB1[4,5,6]RGB2[7,15,16]ADDR[18,8,3,42,9]; {}x{}standard two-row one-bit scan diagnostic only. No PWM brightness/color integration, FM6126A/FM6124/MBI5124/DP3246 register configuration, F/B folded scan remapping, audio PCM, SHTC3/QMI8658 or physical panel power. Bare board has no assumed panel. RTC/audio register controls SDA47/SCL48 SDMMC1bitCLK1CMD44D0=17",self.name,self.w,self.h)}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn actual_hub75_routes_shift_latch_and_oe_create_pixels_but_wrong_clock_does_not(){let mut b=WaveshareRgbMatrix::new("waveshare-s3-rgb-matrix-p2-64x64",64,64);let order=[4,5,6,7,15,16,40,2,18,8,3,42,9,255,255,255];let mut p=LcdI80Pins{clock:1<<41,width:16,..Default::default()};for(i,pin)in order.into_iter().enumerate(){if pin!=255{p.data[i]=1u64<<pin;}}let mut words=vec![1<<7;63];words.push((1<<7)|1|(4<<3));words.push((1<<7)|(1<<6));words.push(0);b.lcd_i80(p,true,&words);assert_eq!(b.image[63],0xf800);assert_eq!(b.image[32*64+63],0x001f);let old=b.image.clone();p.clock=1<<39;b.lcd_i80(p,true,&[0;128]);assert_eq!(old,b.image);assert!(WaveshareRgbMatrix::new("waveshare-s3-rgb-matrix",0,0).display().is_none());}
}
