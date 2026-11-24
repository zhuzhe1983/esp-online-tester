//! Individually reviewed SPI glass/capacity families. Manufacturer pin sources
//! and their exact commercial SKU mapping live in the S3 integration fragments.
//! Shared DCS code does not inherit another vendor's board identity or wiring.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,es8311::Es8311,es7210::Es7210}};
use super::lilygo_display::{Profile,LilygoDisplay};
const KEYS:&[(&str,u8)]=&[("boot",0),("volume-up",4),("power",5)];
static LCD154:Profile=Profile{name:"waveshare-s3-lcd154",controller:"ST7789",gram:[240,320],glass:[0,0,240,240],spi:[38,39,21],dc:Some(45),reset:Some(40),light:Some((46,true)),light_pwm:true,enable:None,quad:false,buttons:KEYS,encoder:None,touch:None,expander:false};
static TOUCH154:Profile=Profile{name:"waveshare-s3-touch-lcd154",..LCD154};
// GC9107 vendor/Xiaozhi driver adds X=2,Y=1 for128x128 glass. This is a
// padded DCS coordinate canvas, not a measurement of controller RAM capacity.
static LCD085:Profile=Profile{name:"waveshare-s3-lcd085",controller:"GC9107",gram:[132,162],glass:[2,1,128,128],..LCD154};
struct Config{panel:&'static Profile,touch:bool,sd:[u8;6]}
static CONFIGS:&[Config]=&[Config{panel:&LCD154,touch:false,sd:[16,15,17,18,13,14]},Config{panel:&TOUCH154,touch:true,sd:[16,15,17,18,13,14]},Config{panel:&LCD085,touch:false,sd:[16,15,17,18,13,14]}];
pub fn make_board(name:&str)->Option<Board>{CONFIGS.iter().find(|p|p.panel.name==name).map(|p|Box::new(SourcedSpi::new(p))as Board)}
struct SourcedSpi{cfg:&'static Config,display:LilygoDisplay,touch:Arc<Mutex<Cst816State>>,i2c:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,cycle:u64,card:bool}
impl SourcedSpi{
 fn new(cfg:&'static Config)->Self{Self{cfg,display:LilygoDisplay::new(cfg.panel),touch:Default::default(),i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:Vec::new(),cycle:0,card:false}}
 fn irq(&mut self){if self.cfg.touch{let h=self.touch.lock().expect("Waveshare touch").irq_high();if h!=self.levels[2]{self.levels[2]=h;self.edges.push(BoardEdge{cycle:self.cycle,pin:48,level:h});}}}
}
impl BoardModel for SourcedSpi{
 fn name(&self)->&'static str{self.cfg.panel.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;self.display.observe_routes(c,&r);if self.cfg.touch{let mut t=self.touch.lock().expect("Waveshare touch");let on=r.software_output(47)==Some(true);if t.powered&&!on{t.reset();}t.powered=on;}let scl=r.software_output(41).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(42).unwrap_or(true));for(i,(pin,h))in[(42,sda),(41,scl)].into_iter().enumerate(){if h!=self.levels[i]{self.levels[i]=h;self.edges.push(BoardEdge{cycle:c,pin,level:h});}}self.irq();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,p:&dyn Fn(u8)->Option<(f64,u32)>){self.display.gpio_pwm_at(c,p);}
 fn uses_spi_pins(&self)->bool{true}
 fn spi_transfer_pins(&mut self,h:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8>{if rx>0{return vec![255;rx];}self.display.spi_transfer_pins(h,p,tx,rx)}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{if self.cfg.touch{self.i2c.attach(0x15,Box::new(Cst816s::new(self.touch.clone())));}self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((42,41))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.irq();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut x:Vec<_>=self.cfg.panel.buttons.iter().map(|(_,p)|(*p,true)).collect();x.extend([(42,self.levels[0]),(41,self.levels[1])]);if self.cfg.touch{x.push((48,self.levels[2]));}x}
 fn named_pin(&self,n:&str)->Option<u8>{self.cfg.panel.buttons.iter().find_map(|(id,p)|(*id==n).then_some(*p))}
 fn touch(&mut self,x:u16,y:u16,d:bool){if self.cfg.touch{self.touch.lock().expect("Waveshare touch").update(x.min(239),y.min(239),d);self.irq();}}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some((self.cfg.sd,self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{self.display.display()}
 fn display_version(&self)->u64{self.display.display_version()}
 fn display_frames(&self)->u64{self.display.display_frames()}
 fn gram(&self)->Option<(Vec<u16>,usize,usize)>{self.display.gram()}
 fn report(&self)->String{format!("{} exact SPI38/39/21 DC45 RST40 BL46 digital DCS; I2C42/41 codecs18/40 control only; touchCST816={} RST47 IRQ48 ID unknown; SDMMC{:?}. No simulated battery ADC/charging/physical PWR latch/audio PCM, RF, complete application or physical acceptance. GC9107 padded coordinate canvas is not physical RAM-capacity evidence. {}",self.name(),self.cfg.touch,self.cfg.sd,self.display.report())}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut SourcedSpi,dc:bool,reset:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(pin,hi)in[(45,dc),(40,reset),(46,true),(47,true)]{g.enable|=1<<pin;if hi{g.out|=1<<pin;}m.write(4+4*pin,1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins()->SpiPins{SpiPins{sclk:1<<38,mosi:1<<39,cs:1<<21,..Default::default()}}
 fn cmd(b:&mut SourcedSpi,c:u8,a:&[u8]){route(b,false,true);b.spi_transfer_pins(2,pins(),&[c],0);route(b,true,true);b.spi_transfer_pins(2,pins(),a,0);}
 #[test]fn each_glass_writes_real_corner_pixels_and_rejects_wrong_pins(){for c in CONFIGS{let mut b=SourcedSpi::new(c);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);let[x,y,w,h]=c.panel.glass;cmd(&mut b,0x2a,&[0,x as u8,0,x as u8]);cmd(&mut b,0x2b,&[0,y as u8,0,y as u8]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);let count=b.display_version();b.spi_transfer_pins(2,SpiPins{cs:1<<22,..pins()},&[0,0],0);assert_eq!(b.display_version(),count);assert_eq!((b.display().unwrap().0,b.display().unwrap().1),(w as u32,h as u32));let ds=b.i2c_devices();assert_eq!(ds.iter().any(|(_,a,_)|*a==0x15),c.touch);route(&mut b,true,false);assert!(b.display().unwrap().2.iter().all(|p|*p==0));}}
}
