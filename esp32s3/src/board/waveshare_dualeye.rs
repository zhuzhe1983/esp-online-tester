//! Intact ESP32-S3-DualEye(-Touch)-LCD-1.28, manufacturer c0eebfcb.
//! Both GC9A01 controllers own independent RAM/reset/backlight and CS. The
//! current public display/touch API presents only controller1; never alias2.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,es8311::Es8311,es7210::Es7210}};
use super::lilygo_display::{Profile,LilygoDisplay};
static EYE1:Profile=Profile{name:"waveshare-s3-dualeye-lcd128",controller:"GC9A01",gram:[240,240],glass:[0,0,240,240],spi:[41,42,47],dc:Some(45),reset:Some(48),light:Some((46,true)),light_pwm:true,enable:None,quad:false,buttons:&[("boot",0)],encoder:None,touch:None,expander:false};
static EYE2:Profile=Profile{name:"waveshare-s3-dualeye-lcd128-secondary",spi:[41,42,38],reset:Some(8),light:Some((39,true)),..EYE1};
const TOUCH_WIRES:[(u8,u8,u8,u8);2]=[(11,10,4,5),(3,2,6,7)];
pub struct WaveshareDualEye{touch_enabled:bool,displays:[LilygoDisplay;2],touches:[Arc<Mutex<Cst816State>>;2],i2c:[I2cBitBang;2],attached:bool,levels:[bool;6],edges:Vec<BoardEdge>,cycle:u64,card:bool}
impl WaveshareDualEye{
 pub fn new(touch_enabled:bool)->Self{Self{touch_enabled,displays:[LilygoDisplay::new(&EYE1),LilygoDisplay::new(&EYE2)],touches:std::array::from_fn(|_|Default::default()),i2c:std::array::from_fn(|_|I2cBitBang::new()),attached:false,levels:[true;6],edges:Vec::new(),cycle:0,card:false}}
 fn irq(&mut self){if self.touch_enabled{for(i,(_,_,_,irq))in TOUCH_WIRES.into_iter().enumerate(){let high=self.touches[i].lock().expect("DualEye touch").irq_high();if high!=self.levels[3*i+2]{self.levels[3*i+2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:irq,level:high});}}}}
}
impl BoardModel for WaveshareDualEye{
 fn name(&self)->&'static str{if self.touch_enabled{"waveshare-s3-dualeye-touch-lcd128"}else{"waveshare-s3-dualeye-lcd128"}}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;for display in &mut self.displays{display.observe_routes(c,&r);}for(i,(sda,scl,rst,_))in TOUCH_WIRES.into_iter().enumerate(){if self.touch_enabled{let mut t=self.touches[i].lock().expect("DualEye touch");let high=r.software_output(rst)==Some(true);if t.powered&&!high{t.reset();}t.powered=high;}let clock=r.software_output(scl).unwrap_or(true);let data=self.i2c[i].drive(clock,r.software_output(sda).unwrap_or(true));for(j,(pin,high))in[(sda,data),(scl,clock)].into_iter().enumerate(){if high!=self.levels[3*i+j]{self.levels[3*i+j]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}}self.irq();}
 fn uses_spi_pins(&self)->bool{true}
 fn spi_transfer_pins(&mut self,h:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8>{if rx==0{for d in &mut self.displays{d.spi_transfer_pins(h,p,tx,0);}}vec![255;rx]}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,p:&dyn Fn(u8)->Option<(f64,u32)>){for d in &mut self.displays{d.gpio_pwm_at(c,p);}}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{if self.touch_enabled{for i in 0..2{self.i2c[i].attach(0x15,Box::new(Cst816s::new(self.touches[i].clone())));}}self.i2c[0].attach(0x18,Box::new(Es8311::new()));self.i2c[0].attach(0x40,Box::new(Es7210::new()));self.attached=true;}let mut out=Vec::new();for bus in 0..2{for(i,(sda,scl,_,_))in TOUCH_WIRES.into_iter().enumerate(){out.extend(self.i2c[i].controller_devices(Some((sda,scl))).into_iter().map(|(a,d)|(bus,a,d)));}}out}
 fn touch(&mut self,x:u16,y:u16,down:bool){if self.touch_enabled{self.touches[0].lock().expect("DualEye touch").update(x.min(239),y.min(239),down);self.irq();}}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.irq();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut out=vec![(0,true)];for(i,(sda,scl,_,irq))in TOUCH_WIRES.into_iter().enumerate(){out.extend([(sda,self.levels[3*i]),(scl,self.levels[3*i+1])]);if self.touch_enabled{out.push((irq,self.levels[3*i+2]));}}out}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([17,21,18,255,255,255],self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{self.displays[0].display()}
 fn display_version(&self)->u64{self.displays[0].display_version()}
 fn display_frames(&self)->u64{self.displays[0].display_frames()}
 fn gram(&self)->Option<(Vec<u16>,usize,usize)>{self.displays[0].gram()}
 fn report(&self)->String{format!("{} intact board; independent240sq GC9A01 SPI41/42 DC45 CS47/38 RST48/8 BL46/39. Primary-only host framebuffer/touch; second controller RAM exists but no public secondary display/input API. CST816 common report unknown ID on11/10RST4IRQ5 and3/2RST6IRQ7 touchSKU only; SDMMC17/21/18. No cut-board/FPC resolder modes, third screen, ADC/battery, PCM, full factory or physical validation. primary={} secondary={}",self.name(),self.displays[0].report(),self.displays[1].report())}
}
#[cfg(test)]mod tests{use super::*;
 fn drive(b:&mut WaveshareDualEye,dc:bool,low:u64){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for p in[48,8,46,39,4,6,45]{g.enable|=1<<p;g.out|=1<<p;m.write(4+4*p,1<<12);}if !dc{g.out&=!(1<<45);}g.out&=!low;b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn cmd(b:&mut WaveshareDualEye,cs:u8,c:u8,data:&[u8]){let p=SpiPins{sclk:1<<41,mosi:1<<42,cs:1<<cs,..Default::default()};drive(b,false,0);b.spi_transfer_pins(2,p,&[c],0);drive(b,true,0);b.spi_transfer_pins(2,p,data,0);}
 #[test]fn separate_chip_selects_ram_reset_and_primary_presentation(){let mut b=WaveshareDualEye::new(false);for(cs,color)in[(47,[0xf8,0]),(38,[0x07,0xe0])]{for c in[0x11,0x29]{cmd(&mut b,cs,c,&[]);}cmd(&mut b,cs,0x3a,&[0x55]);cmd(&mut b,cs,0x2a,&[0,0,0,0]);cmd(&mut b,cs,0x2b,&[0,0,0,0]);cmd(&mut b,cs,0x2c,&color);}assert_eq!(b.display().unwrap().2[0],0xf800);assert_eq!(b.displays[1].display().unwrap().2[0],0x07e0);let before=b.displays[1].display_version();cmd(&mut b,37,0x2c,&[255,255]);assert_eq!(b.displays[1].display_version(),before);drive(&mut b,true,1<<48);assert_eq!(b.display().unwrap().2[0],0);assert_eq!(b.displays[1].display().unwrap().2[0],0x07e0);assert!(b.i2c_devices().iter().all(|(_,a,_)|*a!=0x15));}
 #[test]fn two_touch_buses_keep_independent_contacts_and_resets(){let mut b=WaveshareDualEye::new(true);drive(&mut b,true,0);let mut ds=b.i2c_devices();b.touch(120,160,true);for(pins,expected)in[((11,10),1),((3,2),0)]{let d=ds.iter_mut().find(|(bus,a,d)|*bus==0&&*a==0x15&&d.pins()==Some(pins)).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));assert_eq!(d.read(),expected);}let mut hw=esp_periph::i2c::I2c::new();for(bus,a,d)in ds{if bus==0{hw.attach(a,d);}}assert!(hw.has_pinned_devices());assert!(!b.input_levels().iter().find(|(p,_)|*p==5).unwrap().1);assert!(b.input_levels().iter().find(|(p,_)|*p==7).unwrap().1);drive(&mut b,true,1<<4);assert!(b.touches[1].lock().unwrap().powered);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([17,21,18,255,255,255],true)));}
}
