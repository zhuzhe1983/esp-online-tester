//! Individually sourced CO5300 desk modules. QSPI lanes and reset/touch pins are
//! manufacturer pin_config.h facts; supply policy follows pinned board source.
//! 2.16 native1bit SDMMC is independently confirmed by its manufacturer BSP.
//! The1.75C socket route remains absent until independently established.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{axp2101::{Axp2101,Axp2101Power,PowerRail},cst92xx::{Cst92xx,Cst92State,Cst92Kind},es7210::Es7210,es8311::Es8311,pcf85063::{Pcf85063,Pcf85063State},i2c_bitbang::I2cBitBang,qspi_dcs::QspiDcsPanel}};
const HZ:u64=240_000_000;
struct Config{name:&'static str,w:usize,h:usize,reset:u8,touch_reset:u8,touch_kind:Cst92Kind,sd:Option<[u8;6]>}
static CONFIGS:&[Config]=&[Config{name:"waveshare-s3-touch-amoled216",w:480,h:480,reset:39,touch_reset:40,touch_kind:Cst92Kind::Cst9220,sd:Some([2,1,3,255,255,255])},Config{name:"waveshare-s3-touch-amoled175c",w:466,h:466,reset:1,touch_reset:2,touch_kind:Cst92Kind::Cst9217,sd:None}];
pub fn make_board(name:&str)->Option<Board>{CONFIGS.iter().find(|p|p.name==name).map(|p|Box::new(WaveshareCo5300::new(p))as Board)}
struct Gated{power:Arc<Mutex<Axp2101Power>>,adc:bool,inner:Box<dyn I2cDevice>}
impl Gated{fn on(&self)->bool{let p=self.power.lock().expect("CO5300 power");if self.adc{p.output_mv(PowerRail::Aldo1).is_some_and(|v|(1600..=3600).contains(&v))}else{p.output_mv(PowerRail::Dcdc1)==Some(3300)}}}
impl I2cDevice for Gated{fn start(&mut self,r:bool)->bool{self.on()&&self.inner.start(r)}fn write(&mut self,b:u8)->bool{self.on()&&self.inner.write(b)}fn read(&mut self)->u8{if self.on(){self.inner.read()}else{255}}fn stop(&mut self){self.inner.stop();}}
struct WaveshareCo5300{cfg:&'static Config,panel:QspiDcsPanel,power:Arc<Mutex<Axp2101Power>>,touch:Arc<Mutex<Cst92State>>,rtc:Arc<Mutex<Pcf85063State>>,i2c:I2cBitBang,attached:bool,main:bool,reset:bool,touch_reset:bool,selected:bool,levels:[bool;3],edges:Vec<BoardEdge>,cycle:u64,card:bool}
impl WaveshareCo5300{
 fn new(cfg:&'static Config)->Self{Self{cfg,panel:QspiDcsPanel::co5300(cfg.w,cfg.h,0,0),power:Default::default(),touch:Arc::new(Mutex::new(Cst92State::new(cfg.touch_kind,cfg.w as u16,cfg.h as u16))),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),i2c:I2cBitBang::new(),attached:false,main:true,reset:false,touch_reset:false,selected:false,levels:[true;3],edges:Vec::new(),cycle:0,card:false}}
 fn sync(&mut self){let on=self.power.lock().expect("CO5300 power").output_mv(PowerRail::Dcdc1)==Some(3300);if self.main!=on{self.panel.reset();self.main=on;}self.touch.lock().expect("CST92xx").hardware(on,self.touch_reset);let high=self.touch.lock().expect("CST92xx").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:11,level:high});}}
}
impl BoardModel for WaveshareCo5300{
 fn name(&self)->&'static str{self.cfg.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;let reset=r.software_output(self.cfg.reset)==Some(true);if self.reset&&!reset{self.panel.reset();}self.reset=reset;self.touch_reset=r.software_output(self.cfg.touch_reset)==Some(true);let selected=r.software_output(12)==Some(false);if self.selected&&!selected{self.panel.deselect();}self.selected=selected;let scl=r.software_output(14).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(15).unwrap_or(true));for(i,(pin,hi))in[(15,sda),(14,scl)].into_iter().enumerate(){if hi!=self.levels[i]{self.levels[i]=hi;self.edges.push(BoardEdge{cycle:c,pin,level:hi});}}self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<12)==0{return vec![255;rx];}self.sync();let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let route=p.sclk&(1<<38)!=0&&p.data[0]&(1<<4)!=0&&(!quad||[5,6,7].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));if !route||!self.reset||!self.main||rx!=0{self.panel.reject_until_deselect();}else{self.panel.transfer(p,b);}
  if !p.keep_cs&&p.software_cs&(1<<12)==0{self.panel.deselect();}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x34,Box::new(Axp2101::new(self.power.clone())));self.i2c.attach(0x5a,Box::new(Cst92xx::new(self.touch.clone())));self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.i2c.attach(0x18,Box::new(Gated{power:self.power.clone(),adc:false,inner:Box::new(Es8311::new())}));self.i2c.attach(0x40,Box::new(Gated{power:self.power.clone(),adc:true,inner:Box::new(Es7210::new())}));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((15,14))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.rtc.lock().expect("RTC").advance_to(c);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(15,self.levels[0]),(14,self.levels[1]),(11,self.levels[2])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.sync();self.touch.lock().expect("CST92xx").update(x,y,down);self.sync();}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{self.cfg.sd.map(|pins|(pins,self.main&&self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((self.cfg.w as u32,self.cfg.h as u32,if self.main&&self.reset{self.panel.frame()}else{vec![0;self.cfg.w*self.cfg.h]},self.display_version()))}
 fn display_version(&self)->u64{self.panel.version()}
 fn display_frames(&self)->u64{self.panel.frames()}
 fn report(&self)->String{format!("{} CO5300 {}x{} QSPI38/[4,5,6,7]/CS12 RST{}; CST92xx@5A SDA15/SCL14 RST{}IRQ11; AXP2101 DCDC1=3V3/ALDO1 audio-ADC digital gates RTC51; {}pixels. Chip family IDs source-defined; firmware/project/calibration unknown. 2.16 SDMMC1bit2/1/3 sourced from BSP2.0.1; 1.75C SD unresolved. No QMI8658, analog charging/audio PCM, PMU key poweroff, physical IRQ cadence, complete factory or hardware acceptance.",self.cfg.name,self.cfg.w,self.cfg.h,self.cfg.reset,self.cfg.touch_reset,self.panel.pixels_written())}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareCo5300){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for pin in[b.cfg.reset,b.cfg.touch_reset]{g.enable|=1<<pin;g.out|=1<<pin;m.write(4+4*u32::from(pin),1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn packet(c:u8,a:&[u8],quad:bool)->(SpiQspiPins,Vec<u8>){let mut b=vec![if quad{0x32}else{0x02},0,c,0];b.extend(a);(SpiQspiPins{sclk:1<<38,data:[1<<4,1<<5,1<<6,1<<7],cs:1<<12,command_bits:8,address_bits:24,data_bits:(a.len()*8)as u32,command_width:1,address_width:1,data_width:if a.is_empty(){0}else if quad{4}else{1},..Default::default()},b)}
 fn cmd(b:&mut WaveshareCo5300,c:u8,a:&[u8]){let(p,bytes)=packet(c,a,c==0x2c);b.spi_transfer_qspi_pins(2,p,&bytes,0);}
 #[test]fn exact_qspi_pixels_family_touch_reports_and_wrong_pins(){for cfg in CONFIGS{let mut b=WaveshareCo5300::new(cfg);route(&mut b);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x51,&[255]);cmd(&mut b,0x2a,&[0,0,0,0]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.display().unwrap().2[0],0x07e0);cmd(&mut b,0x36,&[0xa0]);cmd(&mut b,0x2a,&[0,2,0,2]);cmd(&mut b,0x2b,&[0,3,0,3]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[(cfg.h-3)*cfg.w+3],0xf800);let count=b.panel.pixels_written();let(mut p,bytes)=packet(0x2c,&[0xf8,0],true);p.data[3]=1<<8;b.spi_transfer_qspi_pins(2,p,&bytes,0);assert_eq!(b.panel.pixels_written(),count);let mut ds=b.i2c_devices();b.touch(400,401,true);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x5a).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(0xd0));assert!(d.write(0));assert!(d.start(true));let v:Vec<_>=(0..15).map(|_|d.read()).collect();assert_eq!(v[6],0xab);assert_eq!((u16::from(v[1])<<4)|u16::from(v[3]>>4),400);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),cfg.sd.map(|p|(p,true)));}}
}
