//! Exact manufacturer ePaper1.54G four-color and ePaper3.97 monochrome subsets.
//! Pins and transactions are retained in waveshare-s3-20261123 source manifest.
//! 1.54G is a separate 2-bit stream, never an SSD1680 monochrome alias. Its
//! controller silicon identity/analog waveforms remain unasserted.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,spi_bitbang::SpiBitBang,ssd1677::Ssd1677,pcf85063::{Pcf85063,Pcf85063State}}};
const HZ:u64=240_000_000;
struct FourColor{ram:Vec<u8>,frame:Vec<u16>,at:usize,command:u8,params:[u8;4],count:usize,awake:bool,power:bool,resolution:bool,cycle:u64,deadline:Option<u64>,refresh:bool,frames:u64,version:u64,edges:Vec<BoardEdge>}
impl FourColor{
 fn new()->Self{Self{ram:vec![0x55;10000],frame:vec![0xffff;40000],at:0,command:0,params:[0;4],count:0,awake:true,power:false,resolution:false,cycle:0,deadline:None,refresh:false,frames:0,version:0,edges:Vec::new()}}
 fn reset(&mut self,loss:bool){self.command=0;self.count=0;self.at=0;self.awake=true;self.power=false;self.resolution=false;self.refresh=false;if loss{self.ram.fill(0x55);}
  if self.deadline.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:8,level:true});}}
 fn busy_for(&mut self,cycles:u64){if self.deadline.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:8,level:false});}self.deadline=Some(self.cycle.saturating_add(cycles.max(1)));}
 fn advance(&mut self,c:u64){self.cycle=c;if let Some(end)=self.deadline.filter(|t|*t<=c){self.deadline=None;if self.refresh{for(i,b)in self.ram.iter().enumerate(){for p in 0..4{self.frame[i*4+p]=[0,0xffff,0xffe0,0xf800][usize::from((b>>(6-p*2))&3)];}}self.frames+=1;self.version+=1;self.refresh=false;}self.edges.push(BoardEdge{cycle:end,pin:8,level:true});}}
 fn write(&mut self,data:bool,bytes:&[u8]){for &b in bytes{if !self.awake{continue;}
  if !data{self.command=b;self.count=0;match b{0x04=>{self.power=true;self.busy_for(HZ/1000);},0x02=>{self.power=false;self.busy_for(HZ/1000);},0x10=>self.at=0,0x12 if self.power&&self.resolution&&self.at==self.ram.len()=>{self.refresh=true;self.busy_for(HZ*20);},_=>{}}}else{if self.command==0x10{if self.at<self.ram.len(){self.ram[self.at]=b;self.at+=1;}continue;}
  if self.count<4{self.params[self.count]=b;}self.count=self.count.saturating_add(1);match(self.command,self.count){(0x61,4)=>self.resolution=self.params==[0,200,0,200],(0x07,1)if b==0xa5=>{self.awake=false;self.power=false;},_=>{}}}}}
}
#[derive(Clone,Copy)]enum Kind{Color154,Mono397}
struct Config{kind:Kind,name:&'static str,spi:[u8;5],busy:u8,power:Option<u8>,i2c:(u8,u8),sd:[u8;6],buttons:&'static[(&'static str,u8)]}
static CONFIGS:&[Config]=&[Config{kind:Kind::Color154,name:"waveshare-s3-epaper154g",spi:[12,13,11,10,9],busy:8,power:Some(6),i2c:(47,48),sd:[39,41,40,255,255,255],buttons:&[("boot",0),("power",18)]},Config{kind:Kind::Mono397,name:"waveshare-s3-epaper397",spi:[11,12,10,9,46],busy:3,power:None,i2c:(41,42),sd:[16,17,15,255,255,255],buttons:&[("boot",0)]}];
pub fn make_board(name:&str)->Option<Board>{CONFIGS.iter().find(|p|p.name==name).map(|p|Box::new(WaveshareEpaper::new(p))as Board)}
enum Panel{Color(FourColor),Mono(Box<Ssd1677>)}
impl Panel{
 fn write(&mut self,dc:bool,b:&[u8]){match self{Self::Color(p)=>p.write(dc,b),Self::Mono(p)=>p.write(dc,b)}}
 fn reset(&mut self,loss:bool){match self{Self::Color(p)=>p.reset(loss),Self::Mono(p)=>if loss{p.power_loss()}else{p.reset()}}}
 fn advance(&mut self,c:u64){match self{Self::Color(p)=>p.advance(c),Self::Mono(p)=>p.advance_to(c)}}
 fn deadline(&self)->Option<u64>{match self{Self::Color(p)=>p.deadline,Self::Mono(p)=>p.next_deadline()}}
 fn edges(&mut self)->Vec<BoardEdge>{match self{Self::Color(p)=>std::mem::take(&mut p.edges),Self::Mono(p)=>p.take_edges()}}
 fn high(&self)->bool{match self{Self::Color(p)=>p.deadline.is_none(),Self::Mono(p)=>p.busy()}}
 fn display(&self)->(u32,u32,Vec<u16>,u64){match self{Self::Color(p)=>(200,200,p.frame.clone(),p.version),Self::Mono(p)=>(800,480,p.frame.clone(),p.version)}}
 fn frames(&self)->u64{match self{Self::Color(p)=>p.frames,Self::Mono(p)=>p.frames}}
}
struct WaveshareEpaper{cfg:&'static Config,panel:Panel,spi:SpiBitBang,i2c:I2cBitBang,rtc:Arc<Mutex<Pcf85063State>>,attached:bool,cycle:u64,power:bool,reset:bool,dc:Option<bool>,levels:[bool;2],edges:Vec<BoardEdge>,card:bool}
impl WaveshareEpaper{
 fn new(cfg:&'static Config)->Self{Self{cfg,panel:match cfg.kind{Kind::Color154=>Panel::Color(FourColor::new()),Kind::Mono397=>Panel::Mono(Box::new(Ssd1677::new(800,480,cfg.busy,HZ/1000)))},spi:SpiBitBang::new(cfg.spi[0],cfg.spi[1],cfg.spi[2]),i2c:I2cBitBang::new(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),attached:false,cycle:0,power:cfg.power.is_none(),reset:false,dc:None,levels:[true;2],edges:Vec::new(),card:false}}
}
impl BoardModel for WaveshareEpaper{
 fn name(&self)->&'static str{self.cfg.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn uses_gpio_waveform(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let pins=self.cfg.spi;let power=self.cfg.power.is_none_or(|p|r.software_output(p)==Some(true));let reset=r.software_output(pins[4])==Some(true);if self.power&&!power{self.panel.reset(true);}
  if self.reset&&!reset{self.panel.reset(false);}self.power=power;self.reset=reset;self.dc=r.software_output(pins[3]);let clk=r.software_output(pins[0]);let mosi=r.software_output(pins[1]);let cs=r.software_output(pins[2]);if let(true,true,Some(dc),Some(clk),Some(mosi),Some(cs))=(power,reset,self.dc,clk,mosi,cs){self.spi.pin(pins[1],mosi);self.spi.pin(pins[2],cs);if let Some(b)=self.spi.pin(pins[0],clk){self.panel.write(dc,&[b]);}}else{self.spi=SpiBitBang::new(pins[0],pins[1],pins[2]);}let(sda,scl)=self.cfg.i2c;let clock=r.software_output(scl).unwrap_or(true);let data=self.i2c.drive(clock,r.software_output(sda).unwrap_or(true));for(i,(pin,high))in[(sda,data),(scl,clock)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}}
 fn uses_spi_pins(&self)->bool{true}
 fn spi_transfer_pins(&mut self,_host:u8,p:SpiPins,b:&[u8],rx:usize)->Vec<u8>{let pins=self.cfg.spi;if self.power&&self.reset&&p.sclk&(1<<pins[0])!=0&&p.mosi&(1<<pins[1])!=0&&p.cs&(1<<pins[2])!=0&&rx==0{if let Some(dc)=self.dc{self.panel.write(dc,b);}}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some(self.cfg.i2c)).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.panel.advance(c);self.rtc.lock().expect("EPD RTC").advance_to(c);}
 fn next_deadline(&self)->Option<u64>{self.panel.deadline()}
 fn take_edges(&mut self)->Vec<BoardEdge>{let mut e=std::mem::take(&mut self.edges);e.extend(self.panel.edges());e.sort_by_key(|x|x.cycle);e}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut p=self.cfg.buttons.iter().map(|(_,p)|(*p,true)).collect::<Vec<_>>();p.extend([(self.cfg.i2c.0,self.levels[0]),(self.cfg.i2c.1,self.levels[1]),(self.cfg.busy,self.panel.high())]);p}
 fn named_pin(&self,n:&str)->Option<u8>{self.cfg.buttons.iter().find_map(|(name,p)|(*name==n).then_some(*p))}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some((self.cfg.sd,self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some(self.panel.display())}
 fn display_version(&self)->u64{match &self.panel{Panel::Color(p)=>p.version,Panel::Mono(p)=>p.version}}
 fn display_frames(&self)->u64{self.panel.frames()}
 fn display_push_hz(&self)->u64{5}
 fn report(&self)->String{format!("{} source SPI+GPIO bitbang, PCF85063, exact1bitSDMMC; 1.54G full200x200 packed2bpp palette00black01white10yellow11red, active-lowBUSY and nominal20s full-refresh policy; 3.97 SSD1677 mono RAM24/26+22/20. No4gray/LUT optical calibration, temperatures/ghosting, codec PCM, sensors, ADC battery, physical power latch or hardware acceptance. frames={}",self.cfg.name,self.panel.frames())}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareEpaper,power:bool,reset:bool,dc:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();let mut pins=vec![(b.cfg.spi[3],dc),(b.cfg.spi[4],reset)];if let Some(p)=b.cfg.power{pins.push((p,power));}for(p,v)in pins{g.enable|=1<<p;if v{g.out|=1<<p;}m.write(4+4*u32::from(p),1<<12);}b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins(b:&WaveshareEpaper)->SpiPins{SpiPins{sclk:1<<b.cfg.spi[0],mosi:1<<b.cfg.spi[1],cs:1<<b.cfg.spi[2],..Default::default()}}
 fn cmd(b:&mut WaveshareEpaper,c:u8,bytes:&[u8]){route(b,true,true,false);b.spi_transfer_pins(2,pins(b),&[c],0);route(b,true,true,true);b.spi_transfer_pins(2,pins(b),bytes,0);}
 #[test]fn color154_requires_full_wire_frame_refresh_and_preserves_optical_power_loss(){let mut b=WaveshareEpaper::new(&CONFIGS[0]);cmd(&mut b,0x61,&[0,200,0,200]);cmd(&mut b,0x04,&[]);b.advance_to(b.next_deadline().unwrap());let mut ram=vec![0x55;10000];ram[0]=0x1b;cmd(&mut b,0x10,&ram);assert_eq!(b.display_frames(),0);cmd(&mut b,0x12,&[0]);let end=b.next_deadline().unwrap();assert_eq!(end-b.cycle,20*HZ);b.advance_to(end-1);assert_eq!(b.display_frames(),0);b.advance_to(end);assert_eq!(&b.display().unwrap().2[..4],&[0,0xffff,0xffe0,0xf800]);let old=b.display().unwrap().2;route(&mut b,false,true,true);b.spi_transfer_pins(2,pins(&b),&[0;8],0);assert_eq!(b.display().unwrap().2,old);cmd(&mut b,0x61,&[0,200,0,200]);cmd(&mut b,0x04,&[]);b.advance_to(b.next_deadline().unwrap());cmd(&mut b,0x10,&[0]);cmd(&mut b,0x12,&[0]);assert!(b.next_deadline().is_none());assert_eq!(b.display_frames(),1);}
 #[test]fn mono397_uses_pixel_x_address_and_rejects_wrong_cs(){let mut b=WaveshareEpaper::new(&CONFIGS[1]);cmd(&mut b,0x11,&[3]);cmd(&mut b,0x44,&[0,0,0x1f,3]);cmd(&mut b,0x45,&[0,0,0xdf,1]);cmd(&mut b,0x4e,&[8,0]);cmd(&mut b,0x4f,&[1,0]);cmd(&mut b,0x24,&[0x7f]);let mut wrong=pins(&b);wrong.cs=1<<11;b.spi_transfer_pins(2,wrong,&[0],0);cmd(&mut b,0x22,&[0xf7]);cmd(&mut b,0x20,&[]);b.advance_to(b.next_deadline().unwrap());let f=b.display().unwrap().2;let row=(480-1-1)*800;assert_eq!(f[row+8],0);assert_eq!(f[row+9],0xffff);assert_eq!(f[row+16],0xffff);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([16,17,15,255,255,255],true)));assert_eq!(b.i2c_devices()[0].2.pins(),Some((41,42)));}
}
