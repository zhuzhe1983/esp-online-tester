//! ESP32-S3-RLCD-4.2 manufacturer full-window monochrome wire path.
//! Pinned examples/arduino/09_LVGL_V9_Test/display_bsp.cpp supplies the
//! 4-column x2-row bit ordering and fixed2A/2B ranges. V2 changes only the
//! mechanical spacer; no controller silicon identity or optical calibration.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardEdge,BoardModel,SpiPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,spi_bitbang::SpiBitBang,pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210}};
const HZ:u64=240_000_000;
pub struct WaveshareRlcd{ram:Vec<u8>,frame:Vec<u16>,at:usize,command:u8,params:[u8;2],count:usize,columns:[u8;2],pages:[u8;2],format:u8,orientation:u8,awake:bool,on:bool,reset:bool,dc:Option<bool>,version:u64,frames:u64,cycle:u64,spi:SpiBitBang,i2c:I2cBitBang,rtc:Arc<Mutex<Pcf85063State>>,attached:bool,levels:[bool;2],edges:Vec<BoardEdge>,card:bool}
impl Default for WaveshareRlcd{fn default()->Self{Self::new()}}
impl WaveshareRlcd{
 pub fn new()->Self{Self{ram:vec![255;15000],frame:vec![0xffff;300*400],at:0,command:0,params:[0;2],count:0,columns:[0;2],pages:[0;2],format:0,orientation:0,awake:false,on:false,reset:false,dc:None,version:0,frames:0,cycle:0,spi:SpiBitBang::new(11,12,40),i2c:I2cBitBang::new(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),attached:false,levels:[true;2],edges:Vec::new(),card:false}}
 fn valid(&self)->bool{self.columns==[0x12,0x2a]&&self.pages==[0,0xc7]&&self.format==0x11&&self.orientation==0x48}
 fn reset_registers(&mut self){self.command=0;self.count=0;self.at=0;self.awake=false;self.on=false;self.columns=[0;2];self.pages=[0;2];self.format=0;self.orientation=0;self.version+=1;}
 fn write(&mut self,dc:bool,data:&[u8]){for &b in data{
  if !dc{self.command=b;self.count=0;match b{0x01=>self.reset_registers(),0x10=>{self.awake=false;self.version+=1;},0x11=>{self.awake=true;self.version+=1;},0x28=>{self.on=false;self.version+=1;},0x29=>{self.on=true;self.version+=1;},0x2c=>self.at=0,_=>{}}continue;}
  if self.command==0x2c{if self.valid()&&self.at<self.ram.len(){let index=self.at;self.ram[index]=b;let x=(index%75)*4;let y=(index/75)*2;for column in 0..4{for row in 0..2{self.frame[(y+row)*300+x+column]=if b&(1<<(7-column*2-row))!=0{0xffff}else{0};}}self.at+=1;self.version+=1;if self.at==self.ram.len(){self.frames+=1;}}continue;}
  if self.count<2{self.params[self.count]=b;}self.count=self.count.saturating_add(1);match(self.command,self.count){(0x2a,2)=>self.columns=self.params,(0x2b,2)=>self.pages=self.params,(0x3a,1)=>self.format=b,(0x36,1)=>self.orientation=b,_=>{}}
 }}
}
impl BoardModel for WaveshareRlcd{
 fn name(&self)->&'static str{"waveshare-s3-rlcd42"}
 fn uses_gpio_routes(&self)->bool{true}
 fn uses_gpio_waveform(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;let reset=r.software_output(41)==Some(true);if self.reset&&!reset{self.reset_registers();}self.reset=reset;self.dc=r.software_output(5);if let(true,Some(dc),Some(clk),Some(mosi),Some(cs))=(reset,self.dc,r.software_output(11),r.software_output(12),r.software_output(40)){self.spi.pin(12,mosi);self.spi.pin(40,cs);if let Some(b)=self.spi.pin(11,clk){self.write(dc,&[b]);}}else{self.spi=SpiBitBang::new(11,12,40);}let scl=r.software_output(14).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(13).unwrap_or(true));for(i,(pin,high))in[(13,sda),(14,scl)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}}
 fn uses_spi_pins(&self)->bool{true}
 fn spi_transfer_pins(&mut self,_h:u8,p:SpiPins,b:&[u8],rx:usize)->Vec<u8>{if self.reset&&p.sclk&(1<<11)!=0&&p.mosi&(1<<12)!=0&&p.cs&(1<<40)!=0&&rx==0{if let Some(dc)=self.dc{self.write(dc,b);}}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((13,14))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.rtc.lock().expect("RLCD RTC").advance_to(c);}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(13,self.levels[0]),(14,self.levels[1])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([38,21,39,255,255,255],self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((300,400,if self.reset&&self.awake&&self.on&&self.valid(){self.frame.clone()}else{vec![0;300*400]},self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.frames}
 fn report(&self)->String{format!("ESP32-S3-RLCD4.2 full-window300x400 packed4column/2row mono; SPI11/12CS40DC5RST41, MADCTL48 COLMOD11 only; no backlight. I2C13/14 PCF85063 codec controls, SDMMC38/21/39. {}full frames. Vendor voltage/gamma, TE6 cadence, inversion modes, SHTC3, ADC/physical PWR latch, PCM and optical/physical acceptance absent; reset RAM retention is a digital policy, not a measured guarantee.",self.frames)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareRlcd,dc:bool,reset:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(p,v)in[(5,dc),(41,reset)]{g.enable|=1<<p;if v{g.out|=1<<p;}m.write(4+4*p,1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins()->SpiPins{SpiPins{sclk:1<<11,mosi:1<<12,cs:1<<40,..Default::default()}}
 fn cmd(b:&mut WaveshareRlcd,c:u8,d:&[u8]){route(b,false,true);b.spi_transfer_pins(2,pins(),&[c],0);route(b,true,true);b.spi_transfer_pins(2,pins(),d,0);}
 #[test]fn source_packing_is_not_linear_bitmap_and_wrong_clock_never_writes(){let mut b=WaveshareRlcd::new();cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x36,&[0x48]);cmd(&mut b,0x3a,&[0x11]);cmd(&mut b,0x2a,&[0x12,0x2a]);cmd(&mut b,0x2b,&[0,0xc7]);cmd(&mut b,0x2c,&[]);b.spi_transfer_pins(2,SpiPins{sclk:1<<10,..pins()},&[0xff],0);assert_eq!(b.at,0);let mut bytes=vec![0;15000];bytes[0]=0x80;bytes[14999]=1;b.spi_transfer_pins(2,pins(),&bytes,0);let f=b.display().unwrap().2;assert_eq!(f[0],0xffff);assert_eq!(f[1],0);assert_eq!(f[300],0);assert_eq!(f[119999],0xffff);assert_eq!(b.frames,1);cmd(&mut b,0x10,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));cmd(&mut b,0x11,&[]);assert_eq!(b.display().unwrap().2[0],0xffff);cmd(&mut b,0x2a,&[0,0]);cmd(&mut b,0x2c,&[0]);assert_eq!(b.at,0);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([38,21,39,255,255,255],true)));}}
