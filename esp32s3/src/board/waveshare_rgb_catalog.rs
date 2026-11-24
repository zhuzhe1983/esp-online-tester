//! Two exact newer industrial RGB boards. The retained manufacturer examples
//! agree on the RGB bus except G5/G6, I2C pads, and the 7C LCD power/reset lines.
//! 0x24 implements their actual low-byte-first 16-bit board firmware protocol;
//! that protocol reuse is not a claim about the expander MCU silicon identity.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,LcdRgbPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,waveshare_ch32v006::{Ch32V006,Ch32V006Port},pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210}};
const HZ:u64=240_000_000;
struct Config{name:&'static str,i2c:(u8,u8),green:[u8;2],glass_gate:bool,es8311:bool}
static CONFIGS:&[Config]=&[Config{name:"waveshare-s3-touch-lcd7c",i2c:(47,48),green:[9,8],glass_gate:true,es8311:false},Config{name:"waveshare-s3-touch-lcd43c",i2c:(8,9),green:[48,47],glass_gate:false,es8311:true}];
pub fn make_board(name:&str)->Option<Board>{CONFIGS.iter().find(|p|p.name==name).map(|p|Box::new(WaveshareRgb::new(p))as Board)}
struct TouchPort{inner:Gt911,io:Arc<Mutex<Ch32V006Port>>,address:Arc<Mutex<u8>>,at:u8}
impl I2cDevice for TouchPort{
 fn start(&mut self,r:bool)->bool{self.io.lock().expect("RGB EXIO").level(1)==Some(true)&&*self.address.lock().expect("GT strap")==self.at&&self.inner.start(r)}
 fn write(&mut self,b:u8)->bool{self.io.lock().expect("RGB EXIO").level(1)==Some(true)&&self.inner.write(b)}
 fn read(&mut self)->u8{if self.io.lock().expect("RGB EXIO").level(1)==Some(true){self.inner.read()}else{255}}
}
struct WaveshareRgb{cfg:&'static Config,io:Arc<Mutex<Ch32V006Port>>,touch:Arc<Mutex<Gt911State>>,address:Arc<Mutex<u8>>,rtc:Arc<Mutex<Pcf85063State>>,i2c:I2cBitBang,attached:bool,cycle:u64,levels:[bool;3],edges:Vec<BoardEdge>,reset_high:bool,irq_drive:Option<bool>,reset_falls:u64,frame:Vec<u16>,version:u64,frames:u64,mapping:Option<[u8;16]>,card:bool}
impl WaveshareRgb{
 fn new(cfg:&'static Config)->Self{Self{cfg,io:Arc::new(Mutex::new(Ch32V006Port::new())),touch:Arc::new(Mutex::new(Gt911State::new(800,480,HZ/100))),address:Arc::new(Mutex::new(0x5d)),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),i2c:I2cBitBang::new(),attached:false,cycle:0,levels:[true;3],edges:Vec::new(),reset_high:false,irq_drive:None,reset_falls:0,frame:vec![0;800*480],version:0,frames:0,mapping:None,card:false}}
 fn pads(&self)->[u8;16]{[14,38,18,17,10,39,0,45,self.cfg.green[0],self.cfg.green[1],21,1,2,42,41,40]}
 fn glass(&self)->bool{let p=self.io.lock().expect("RGB EXIO");!self.cfg.glass_gate||(p.level(0)==Some(true)&&p.level(5)==Some(true))}
 fn visible(&self)->bool{let p=self.io.lock().expect("RGB EXIO");self.mapping.is_some()&&p.level(2)==Some(true)&&p.pwm!=Some(0)&&(!self.cfg.glass_gate||(p.level(0)==Some(true)&&p.level(5)==Some(true)))}
 fn sync(&mut self){let(high,falls)={let p=self.io.lock().expect("RGB EXIO");(p.level(1)==Some(true),p.falling_edges(1))};if falls!=self.reset_falls{self.reset_falls=falls;*self.touch.lock().expect("GT state")=Gt911State::new(800,480,HZ/100);}
  if high&&!self.reset_high{*self.address.lock().expect("GT strap")=if self.irq_drive==Some(false){0x5d}else{0x14};}self.reset_high=high;let irq={let mut s=self.touch.lock().expect("GT state");s.powered=high;s.advance_to(self.cycle);!high||!s.ready};if irq!=self.levels[2]{self.levels[2]=irq;self.edges.push(BoardEdge{cycle:self.cycle,pin:4,level:irq});}}
}
impl BoardModel for WaveshareRgb{
 fn name(&self)->&'static str{self.cfg.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;self.irq_drive=r.software_output(4);let(sda,scl)=self.cfg.i2c;let clock=r.software_output(scl).unwrap_or(true);let data=self.i2c.drive(clock,r.software_output(sda).unwrap_or(true));for(i,(pin,high))in[(sda,data),(scl,clock)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}self.sync();}
 fn lcd_rgb_pins(&mut self,p:LcdRgbPins){let controls=p.width==16&&p.clock&(1<<7)!=0&&p.hsync&(1<<46)!=0&&p.vsync&(1<<3)!=0&&p.enable&(1<<5)!=0;let pads=self.pads();let mut map=[0;16];let mut valid=controls;for(i,pad)in pads.into_iter().enumerate(){if let Some(bit)=(0..16).find(|bit|p.data[*bit]&(1<<pad)!=0){map[i]=bit as u8;}else{valid=false;}}let next=valid.then_some(map);if next!=self.mapping{self.mapping=next;self.version+=1;}}
 fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]){self.sync();let Some(map)=self.mapping else{return;};if(w,h)!=(800,480)||bytes.len()!=800*480*2||!self.glass(){return;}for(d,s)in self.frame.iter_mut().zip(bytes.as_chunks::<2>().0){let word=u16::from_le_bytes(*s);*d=map.iter().enumerate().fold(0,|v,(bit,input)|v|(((word>>input)&1)<<bit));}self.frames+=1;self.version+=1;}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x24,Box::new(Ch32V006::new(self.io.clone())));self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));for at in[0x5d,0x14]{self.i2c.attach(at,Box::new(TouchPort{inner:Gt911::new(self.touch.clone()),io:self.io.clone(),address:self.address.clone(),at}));}
  if self.cfg.es8311{self.i2c.attach(0x18,Box::new(Es8311::new()));}self.i2c.attach(0x40,Box::new(Es7210::new()));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some(self.cfg.i2c)).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.rtc.lock().expect("RTC").advance_to(c);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(self.cfg.i2c.0,self.levels[0]),(self.cfg.i2c.1,self.levels[1]),(4,self.levels[2])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.touch.lock().expect("GT state").update(x,y,down);self.sync();}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([12,11,13,255,255,255],self.card&&self.io.lock().expect("RGB EXIO").level(4)==Some(true)))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((800,480,if self.visible(){self.frame.clone()}else{vec![0;self.frame.len()]},self.display_version()))}
 fn display_version(&self)->u64{self.version.wrapping_add(self.io.lock().expect("RGB EXIO").writes)}
 fn display_frames(&self)->u64{self.frames}
 fn report(&self)->String{format!("{} 800x480 RGB565; physical data/control routing; 0x24 16bit IO firmware protocol; GT911 single-contact @strap5D/14; PCF85063; SDMMC1bit12/11/13 EX4-high mode selection. No isolated-IO electrical nets, ES8389, BQ27220, PCM, calibrated touch/glass pose, physical PWM brightness, factory app or hardware validation. frames={}",self.cfg.name,self.frames)}
}
#[cfg(test)]mod tests{use super::*;
 fn write(d:&mut dyn I2cDevice,b:&[u8]){assert!(d.start(false));for b in b{assert!(d.write(*b));}d.stop();}
 #[test]fn actual_rgb_gpio_map_backlight_touch_reset_and_card_mode(){for cfg in CONFIGS{let mut b=WaveshareRgb::new(cfg);b.irq_drive=Some(false);let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x24).unwrap().2.as_mut();write(d,&[2,255,255]);write(d,&[3,255,255]);b.advance_to(1);let pins=LcdRgbPins{clock:1<<7,hsync:1<<46,vsync:1<<3,enable:1<<5,width:16,data:b.pads().map(|p|1<<p)};b.lcd_rgb_pins(pins);let mut pixels=vec![0;800*480*2];pixels[..4].copy_from_slice(&[0,0xf8,0xe0,7]);b.lcd_frame(800,480,&pixels);assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x24).unwrap().2.as_mut();write(d,&[3,0xfb,255]);assert_eq!(b.display().unwrap().2[0],0);write(d,&[3,255,255]);let mut wrong=pins;wrong.clock=1<<6;b.lcd_rgb_pins(wrong);b.lcd_frame(800,480,&pixels);assert_eq!(b.frames,1);b.lcd_rgb_pins(pins);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([12,11,13,255,255,255],true)));let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x24).unwrap().2.as_mut();write(d,&[3,0xed,255]);b.advance_to(2);let touch=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x5d).unwrap().2.as_mut();assert!(!touch.start(false));assert!(!b.sd_card_slot().unwrap().1);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x24).unwrap().2.as_mut();write(d,&[3,255,255]);b.touch(123,234,true);b.advance_to(HZ/100+3);let touch=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x5d).unwrap().2.as_mut();write(touch,&[0x81,0x4e,0]);b.advance_to(HZ/50+4);write(touch,&[0x81,0x4e]);assert!(touch.start(true));let v:Vec<_>=(0..6).map(|_|touch.read()).collect();assert_eq!(v,[0x81,0,123,0,234,0]);}}
}
