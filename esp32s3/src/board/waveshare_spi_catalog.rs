//! Individually reviewed SPI glass/capacity families. Manufacturer pin sources
//! and their exact commercial SKU mapping live in the S3 integration fragments.
//! Shared DCS code does not inherit another vendor's board identity or wiring.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,es8311::Es8311,es7210::Es7210,pcf85063::{Pcf85063,Pcf85063State},axp2101::{Axp2101,Axp2101Power}}};
use super::lilygo_display::{Profile,LilygoDisplay};
const KEYS:&[(&str,u8)]=&[("boot",0),("volume-up",4),("power",5)];
static LCD154:Profile=Profile{name:"waveshare-s3-lcd154",controller:"ST7789",gram:[240,320],glass:[0,0,240,240],spi:[38,39,21],dc:Some(45),reset:Some(40),light:Some((46,true)),light_pwm:true,enable:None,quad:false,buttons:KEYS,encoder:None,touch:None,expander:false};
static TOUCH154:Profile=Profile{name:"waveshare-s3-touch-lcd154",..LCD154};
// GC9107 vendor/Xiaozhi driver adds X=2,Y=1 for128x128 glass. This is a
// padded DCS coordinate canvas, not a measurement of controller RAM capacity.
static LCD085:Profile=Profile{name:"waveshare-s3-lcd085",controller:"GC9107",gram:[132,162],glass:[2,1,128,128],..LCD154};
static LCD183:Profile=Profile{name:"waveshare-s3-touch-lcd183",controller:"ST7789P",gram:[240,320],glass:[0,0,240,284],spi:[6,7,5],dc:Some(4),reset:Some(38),light:Some((40,true)),buttons:&[("boot",0)],..LCD154};
#[derive(Clone,Copy)]struct Config{panel:&'static Profile,touch:bool,touch_pins:(u8,u8),i2c:(u8,u8),rtc:bool,pmu:bool,codecs:bool,sd:Option<[u8;6]>}
const COMMON:Config=Config{panel:&LCD154,touch:false,touch_pins:(47,48),i2c:(42,41),rtc:false,pmu:false,codecs:true,sd:Some([16,15,17,18,13,14])};
static CONFIGS:&[Config]=&[COMMON,Config{panel:&TOUCH154,touch:true,..COMMON},Config{panel:&LCD085,..COMMON},Config{panel:&LCD183,touch:true,touch_pins:(39,13),i2c:(15,14),rtc:true,pmu:true,sd:None,..COMMON}];
pub fn make_board(name:&str)->Option<Board>{CONFIGS.iter().find(|p|p.panel.name==name).map(|p|Box::new(SourcedSpi::new(p))as Board)}
struct SourcedSpi{cfg:&'static Config,display:LilygoDisplay,touch:Arc<Mutex<Cst816State>>,i2c:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,cycle:u64,card:bool,rtc:Arc<Mutex<Pcf85063State>>,power:Arc<Mutex<Axp2101Power>>}
impl SourcedSpi{
 fn new(cfg:&'static Config)->Self{Self{cfg,display:LilygoDisplay::new(cfg.panel),touch:Default::default(),i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:Vec::new(),cycle:0,card:false,rtc:Arc::new(Mutex::new(Pcf85063State::new(240_000_000))),power:Default::default()}}
 fn irq(&mut self){if self.cfg.touch{let h=self.touch.lock().expect("Waveshare touch").irq_high();if h!=self.levels[2]{self.levels[2]=h;self.edges.push(BoardEdge{cycle:self.cycle,pin:self.cfg.touch_pins.1,level:h});}}}
}
impl BoardModel for SourcedSpi{
 fn name(&self)->&'static str{self.cfg.panel.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;self.display.observe_routes(c,&r);if self.cfg.touch{let mut t=self.touch.lock().expect("Waveshare touch");let on=r.software_output(self.cfg.touch_pins.0)==Some(true);if t.powered&&!on{t.reset();}t.powered=on;}let(sda_pin,scl_pin)=self.cfg.i2c;let scl=r.software_output(scl_pin).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(sda_pin).unwrap_or(true));for(i,(pin,h))in[(sda_pin,sda),(scl_pin,scl)].into_iter().enumerate(){if h!=self.levels[i]{self.levels[i]=h;self.edges.push(BoardEdge{cycle:c,pin,level:h});}}self.irq();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,p:&dyn Fn(u8)->Option<(f64,u32)>){self.display.gpio_pwm_at(c,p);}
 fn uses_spi_pins(&self)->bool{true}
 fn spi_transfer_pins(&mut self,h:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8>{if rx>0{return vec![255;rx];}self.display.spi_transfer_pins(h,p,tx,rx)}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{if self.cfg.touch{self.i2c.attach(0x15,Box::new(Cst816s::new(self.touch.clone())));}
  if self.cfg.codecs{self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));}
  if self.cfg.rtc{self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));}
  if self.cfg.pmu{self.i2c.attach(0x34,Box::new(Axp2101::new(self.power.clone())));}self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some(self.cfg.i2c)).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;if self.cfg.rtc{self.rtc.lock().expect("SPI RTC").advance_to(c);}self.irq();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut x:Vec<_>=self.cfg.panel.buttons.iter().map(|(_,p)|(*p,true)).collect();x.extend([(self.cfg.i2c.0,self.levels[0]),(self.cfg.i2c.1,self.levels[1])]);if self.cfg.touch{x.push((self.cfg.touch_pins.1,self.levels[2]));}x}
 fn named_pin(&self,n:&str)->Option<u8>{self.cfg.panel.buttons.iter().find_map(|(id,p)|(*id==n).then_some(*p))}
 fn touch(&mut self,x:u16,y:u16,d:bool){if self.cfg.touch{self.touch.lock().expect("Waveshare touch").update(x.min((self.cfg.panel.glass[2]-1)as u16),y.min((self.cfg.panel.glass[3]-1)as u16),d);self.irq();}}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{self.cfg.sd.map(|pins|(pins,self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{self.display.display()}
 fn display_version(&self)->u64{self.display.display_version()}
 fn display_frames(&self)->u64{self.display.display_frames()}
 fn gram(&self)->Option<(Vec<u16>,usize,usize)>{self.display.gram()}
 fn report(&self)->String{format!("{} source-specific SPI{:?} DC{:?} RST{:?} BL{:?}; I2C{:?} CST816-compatible single-report={} reset/IRQ{:?} ID unknown; RTC={} AXP2101control={} SDMMC{:?}. No ADC/charging/physical PWR latch or PMU analog rail topology, audio PCM, RF, complete factory app or physical acceptance. GC9107 padded canvas is not a physical RAM-capacity measurement. {}",self.name(),self.cfg.panel.spi,self.cfg.panel.dc,self.cfg.panel.reset,self.cfg.panel.light,self.cfg.i2c,self.cfg.touch,self.cfg.touch_pins,self.cfg.rtc,self.cfg.pmu,self.cfg.sd,self.display.report())}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut SourcedSpi,dc:bool,reset:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(pin,hi)in[(b.cfg.panel.dc.unwrap(),dc),(b.cfg.panel.reset.unwrap(),reset),(b.cfg.panel.light.unwrap().0,true),(b.cfg.touch_pins.0,true)]{g.enable|=1<<pin;if hi{g.out|=1<<pin;}m.write(4+4*u32::from(pin),1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins(b:&SourcedSpi)->SpiPins{SpiPins{sclk:1<<b.cfg.panel.spi[0],mosi:1<<b.cfg.panel.spi[1],cs:1<<b.cfg.panel.spi[2],..Default::default()}}
 fn cmd(b:&mut SourcedSpi,c:u8,a:&[u8]){route(b,false,true);b.spi_transfer_pins(2,pins(b),&[c],0);route(b,true,true);b.spi_transfer_pins(2,pins(b),a,0);}
 #[test]fn each_glass_writes_real_corner_pixels_and_rejects_wrong_pins(){for c in CONFIGS{let mut b=SourcedSpi::new(c);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);let[x,y,w,h]=c.panel.glass;cmd(&mut b,0x2a,&[(x>>8)as u8,x as u8,(x>>8)as u8,x as u8]);cmd(&mut b,0x2b,&[(y>>8)as u8,y as u8,(y>>8)as u8,y as u8]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);let count=b.display_version();b.spi_transfer_pins(2,SpiPins{cs:1<<22,..pins(&b)},&[0,0],0);assert_eq!(b.display_version(),count);assert_eq!((b.display().unwrap().0,b.display().unwrap().1),(w as u32,h as u32));let mut ds=b.i2c_devices();assert_eq!(ds.iter().any(|(_,a,_)|*a==0x15),c.touch);if c.touch{b.touch(200,270,true);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x15).unwrap().2.as_mut();assert_eq!(d.pins(),Some(c.i2c));assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));let bytes:Vec<_>=(0..5).map(|_|d.read()).collect();let raw_y=(u16::from(bytes[3])<<8)|u16::from(bytes[4]);assert_eq!(raw_y,270.min((h-1)as u16));}b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),c.sd.map(|p|(p,true)));route(&mut b,true,false);assert!(b.display().unwrap().2.iter().all(|p|*p==0));}}
}
