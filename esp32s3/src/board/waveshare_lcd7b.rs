//! ESP32-S3-(Touch)-LCD-7B 1024x600, source c652c902. The 0x24
//! helper speaks its documented 8-bit firmware protocol, not CH422G or the
//! newer 4.3C/7C 16-bit protocol. Native SDMMC is the current vendor path.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State}};
use esp_soc::{board::{BoardEdge,BoardModel,LcdRgbPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,waveshare_touch_parts::{Ch32Port,Ch32V003}}};
const HZ:u64=240_000_000;const RGB:[u8;16]=[14,38,18,17,10,39,0,45,48,47,21,1,2,42,41,40];
struct TouchPort{inner:Gt911,io:Arc<Mutex<Ch32Port>>,at:u8,address:Arc<Mutex<u8>>}
impl I2cDevice for TouchPort{
 fn start(&mut self,r:bool)->bool{self.io.lock().expect("7B IO").high(1)&&*self.address.lock().expect("GT address")==self.at&&self.inner.start(r)}
 fn write(&mut self,b:u8)->bool{self.io.lock().expect("7B IO").high(1)&&self.inner.write(b)}
 fn read(&mut self)->u8{if self.io.lock().expect("7B IO").high(1){self.inner.read()}else{255}}
}
pub struct WaveshareLcd7b{touch_enabled:bool,io:Arc<Mutex<Ch32Port>>,touch:Arc<Mutex<Gt911State>>,address:Arc<Mutex<u8>>,i2c:I2cBitBang,attached:bool,cycle:u64,levels:[bool;3],edges:Vec<BoardEdge>,reset_high:bool,reset_falls:u64,irq_drive:Option<bool>,frame:Vec<u16>,mapping:Option<[u8;16]>,frames:u64,version:u64,card:bool}
impl WaveshareLcd7b{
 pub fn new(touch_enabled:bool)->Self{Self{touch_enabled,io:Arc::new(Mutex::new(Ch32Port::new())),touch:Arc::new(Mutex::new(Gt911State::new(1024,600,HZ/100))),address:Arc::new(Mutex::new(0x5d)),i2c:I2cBitBang::new(),attached:false,cycle:0,levels:[true;3],edges:Vec::new(),reset_high:false,reset_falls:0,irq_drive:None,frame:vec![0;1024*600],mapping:None,frames:0,version:0,card:false}}
 fn sync(&mut self){if !self.touch_enabled{return;}let(high,falls)={let io=self.io.lock().expect("7B IO");(io.high(1),io.falling_edges(1))};if falls!=self.reset_falls{self.reset_falls=falls;*self.touch.lock().expect("7B touch")=Gt911State::new(1024,600,HZ/100);}
 if high&&!self.reset_high{*self.address.lock().expect("GT address")=if self.irq_drive==Some(false){0x5d}else{0x14};}self.reset_high=high;let irq={let mut t=self.touch.lock().expect("7B touch");t.powered=high;t.advance_to(self.cycle);!high||!t.ready};if irq!=self.levels[2]{self.levels[2]=irq;self.edges.push(BoardEdge{cycle:self.cycle,pin:4,level:irq});}}
 fn visible(&self)->bool{let io=self.io.lock().expect("7B IO");self.mapping.is_some()&&io.high(2)&&io.high(6)}
}
impl BoardModel for WaveshareLcd7b{
 fn name(&self)->&'static str{if self.touch_enabled{"waveshare-s3-touch-lcd7b"}else{"waveshare-s3-lcd7b"}}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;let[clk,hs,vs,de]:[u8;4]=[7,46,3,5];let pads=RGB;let pins=LcdRgbPins{clock:if r.matrix_output(usize::from(clk),154){1<<clk}else{0},hsync:if r.matrix_output(usize::from(hs),151){1<<hs}else{0},vsync:if r.matrix_output(usize::from(vs),152){1<<vs}else{0},enable:if r.matrix_output(usize::from(de),150){1<<de}else{0},data:std::array::from_fn(|i|pads.iter().filter(|&&pin|r.matrix_output(usize::from(pin),133+i as u32)).fold(0,|v,&pin|v|(1<<pin))),width:16};self.lcd_rgb_pins(pins);self.irq_drive=r.software_output(4);let clock=r.software_output(9).unwrap_or(true);let data=self.i2c.drive(clock,r.software_output(8).unwrap_or(true));for(i,(pin,high))in[(8,data),(9,clock)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}self.sync();}
 fn lcd_rgb_pins(&mut self,p:LcdRgbPins){let mut valid=p.width==16&&p.clock&(1<<7)!=0&&p.hsync&(1<<46)!=0&&p.vsync&(1<<3)!=0&&p.enable&(1<<5)!=0;let mut map=[0;16];for(i,pad)in RGB.into_iter().enumerate(){if let Some(bit)=(0..16).find(|bit|p.data[*bit]&(1<<pad)!=0){map[i]=bit as u8;}else{valid=false;}}let next=valid.then_some(map);if next!=self.mapping{self.mapping=next;self.version+=1;}}
 fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]){let Some(map)=self.mapping else{return;};if(w,h)!=(1024,600)||bytes.len()!=self.frame.len()*2||!self.io.lock().expect("7B IO").high(6){return;}for(d,s)in self.frame.iter_mut().zip(bytes.as_chunks::<2>().0){let v=u16::from_le_bytes(*s);*d=map.iter().enumerate().fold(0,|out,(bit,input)|out|(((v>>input)&1)<<bit));}self.frames+=1;self.version+=1;}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x24,Box::new(Ch32V003::new(self.io.clone())));if self.touch_enabled{for at in[0x5d,0x14]{self.i2c.attach(at,Box::new(TouchPort{inner:Gt911::new(self.touch.clone()),io:self.io.clone(),at,address:self.address.clone()}));}}self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((8,9))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.sync();}
 fn touch(&mut self,x:u16,y:u16,d:bool){if self.touch_enabled{self.touch.lock().expect("7B touch").update(x,y,d);self.sync();}}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut x=vec![(0,true),(8,self.levels[0]),(9,self.levels[1])];if self.touch_enabled{x.push((4,self.levels[2]));}x}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([12,11,13,255,255,255],self.card&&self.io.lock().expect("7B IO").high(4)))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((1024,600,if self.visible(){self.frame.clone()}else{vec![0;self.frame.len()]},self.display_version()))}
 fn display_version(&self)->u64{self.version.wrapping_add(self.io.lock().expect("7B IO").writes)}
 fn display_frames(&self)->u64{self.frames}
 fn report(&self)->String{format!("{} 1024x600 exact RGB pads; helper24 byte-wide02/03/04/05 protocol, EX2 BL/EX6 LCDVDD, GT911{} resetEX1/IRQ4 and strap5D/14, SDMMC12/11/13 DAT3EX4high. SPI-card mode, helper ADC/physical PWM curve, CAN/USB mux electrical effect, RS485, RF/antenna, full factory and physical validation absent. frames={}",self.name(),if self.touch_enabled{" single-contact"}else{" absent on non-touchSKU"},self.frames)}
}
#[cfg(test)]mod tests{use super::*;
 fn write(d:&mut dyn I2cDevice,v:&[u8]){assert!(d.start(false));for b in v{assert!(d.write(*b));}d.stop();}
 #[test]fn exact_1024_rgb_pin_bus_gates_and_8bit_helper(){for touch in[false,true]{let mut b=WaveshareLcd7b::new(touch);b.irq_drive=Some(false);let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x24).unwrap().2.as_mut();write(d,&[2,255]);write(d,&[3,255]);assert!(d.start(false));assert!(d.write(3));assert!(d.write(255));assert!(!d.write(255),"not the V006 two-byte alias");b.advance_to(1);let p=LcdRgbPins{clock:1<<7,hsync:1<<46,vsync:1<<3,enable:1<<5,width:16,data:RGB.map(|pin|1<<pin)};b.lcd_rgb_pins(p);let mut f=vec![0;1024*600*2];f[..4].copy_from_slice(&[0,0xf8,0xe0,7]);b.lcd_frame(1024,600,&f);assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);let mut wrong=p;wrong.data[8]=1<<6;b.lcd_rgb_pins(wrong);b.lcd_frame(1024,600,&f);assert_eq!(b.frames,1);b.lcd_rgb_pins(p);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x24).unwrap().2.as_mut();write(d,&[3,0xfb]);assert_eq!(b.display().unwrap().2[0],0);write(d,&[3,0xbf]);b.lcd_frame(1024,600,&f);assert_eq!(b.frames,1);write(d,&[3,255]);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([12,11,13,255,255,255],true)));write(d,&[3,0xef]);assert!(!b.sd_card_slot().unwrap().1);assert_eq!(ds.iter().any(|(_,a,_)|*a==0x5d),touch);}}
}
