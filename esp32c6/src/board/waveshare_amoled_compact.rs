//! Manufacturer-source exact C6 AMOLED1.8 V1/V2 and AMOLED1.32 digital subsets.
//! Physical controller identities are separate from copied BSP class names.
use esp_periph::{i2c::{I2cDevice,cst816s::Cst816State},sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::{BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{qspi_dcs::QspiDcsPanel,
 ft3168::{Ft3168,Ft3168State},axp2101::{Axp2101,Axp2101Power,PowerRail},tca9554::{Tca9554,Tca9554State},
 qmi8658::Qmi8658,imu_motion::ImuMotion,pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311}};
use super::waveshare_lcd_family::CstReport;
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Variant{Touch18V1,Touch18V2,Touch132}
impl Variant{
 fn name(self)->&'static str{match self{Self::Touch18V1=>"waveshare-c6-touch-amoled18-v1",Self::Touch18V2=>"waveshare-c6-touch-amoled18",Self::Touch132=>"waveshare-c6-touch-amoled132"}}
 fn size(self)->(u16,u16){if self==Self::Touch132{(466,466)}else{(368,448)}}
 fn bus(self)->(u8,[u8;4],u8){if self==Self::Touch132{(18,[19,20,10,11],22)}else{(0,[1,2,3,4],5)}}
 fn irq(self)->Option<u8>{(self!=Self::Touch132).then_some(15)}
}
type Device=Rc<RefCell<Box<dyn I2cDevice>>>;
struct Routed{device:Device,power:Option<Arc<Mutex<Axp2101Power>>>}
impl Routed{fn active(&self)->bool{self.power.as_ref().is_none_or(|s|s.lock().expect("AXP2101").output_mv(PowerRail::Dcdc1)==Some(3300))}}
impl I2cDevice for Routed{
 fn pins(&self)->Option<(u8,u8)>{Some((8,7))}
 fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,h:Option<u32>)->bool{self.active()&&self.device.borrow_mut().start_address_with_clock(c,a,r,h)}
 fn write(&mut self,b:u8)->bool{self.active()&&self.device.borrow_mut().write(b)}
 fn read(&mut self)->u8{if self.active(){self.device.borrow_mut().read()}else{255}}
 fn stop(&mut self){self.device.borrow_mut().stop();}
}
pub struct CompactAmoled{variant:Variant,panel:QspiDcsPanel,devices:Vec<(u8,Device)>,power:Option<Arc<Mutex<Axp2101Power>>>,io:Option<Arc<Mutex<Tca9554State>>>,
 ft:Option<Arc<Mutex<Ft3168State>>>,cst:Option<Arc<Mutex<Cst816State>>>,motion:Option<Arc<ImuMotion>>,rtc:Option<Arc<Mutex<Pcf85063State>>>,
 reset:bool,active:bool,software_cs:bool,irq_high:bool,cycle:u64,rejected:u64,edges:Vec<BoardEdge>,sd:Option<SpiSdCard>}
impl CompactAmoled{
 pub fn new(variant:Variant)->Self{
  let(w,h)=variant.size();let full=variant!=Variant::Touch132;
  let power=full.then(||Arc::new(Mutex::new(Axp2101Power::default())));let io=full.then(||Arc::new(Mutex::new(Tca9554State::default())));
  let ft=(variant==Variant::Touch18V1).then(||Arc::new(Mutex::new(Ft3168State::new(crate::periph::CPU_HZ))));
  let cst=(variant!=Variant::Touch18V1).then(||{let mut s=Cst816State::default();s.powered=true;Arc::new(Mutex::new(s))});
  let motion=full.then(||Arc::new(ImuMotion::default()));let rtc=full.then(||Arc::new(Mutex::new(Pcf85063State::new(crate::periph::CPU_HZ))));
  let mut devices:Vec<(u8,Box<dyn I2cDevice>)>=vec![(0x18,Box::new(Es8311::new()))];
  if let Some(s)=&power{devices.push((0x34,Box::new(Axp2101::new(s.clone()))));}
  if let Some(s)=&io{devices.push((0x20,Box::new(Tca9554::new(s.clone()))));}
  if let Some(s)=&ft{devices.push((0x38,Box::new(Ft3168::new(s.clone()))));}
  if let Some(s)=&cst{devices.push((0x15,Box::new(CstReport::new(s.clone()))));}
  if let Some(s)=&motion{devices.push((0x6b,Box::new(Qmi8658::new(s.clone(),crate::periph::CPU_HZ))));}
  if let Some(s)=&rtc{devices.push((0x51,Box::new(Pcf85063::new(s.clone()))));}
  let panel=match variant{Variant::Touch18V1=>QspiDcsPanel::sh8601_bsp_rgb565(w as usize,h as usize,0,0),Variant::Touch18V2=>QspiDcsPanel::co5300(w as usize,h as usize,16,0),Variant::Touch132=>QspiDcsPanel::co5300(w as usize,h as usize,6,0xc0)};
  Self{variant,panel,devices:devices.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),power,io,ft,cst,motion,rtc,reset:false,active:false,software_cs:false,irq_high:true,cycle:0,rejected:0,edges:vec![],sd:None}
 }
 fn sync(&mut self){
  let powered=self.power.as_ref().is_none_or(|s|s.lock().expect("AXP2101").output_mv(PowerRail::Dcdc1)==Some(3300));
  let reset=self.io.as_ref().map_or(self.reset,|s|s.lock().expect("TCA9554").high(4));
  let touch_reset=self.io.as_ref().is_none_or(|s|s.lock().expect("TCA9554").high(5));
  let active=powered&&reset;if self.active!=active{if !active{self.panel.reset();}self.active=active;self.panel.changes+=1;}
  if let Some(s)=&self.ft{s.lock().expect("FT compatible").hardware_at(self.cycle,powered,touch_reset);}
  if let Some(s)=&self.cst{let mut s=s.lock().expect("CST820-compatible");let on=powered&&touch_reset;if s.powered&&!on{s.reset();}s.powered=on;}
  self.sync_irq();
 }
 fn high(&self)->bool{self.ft.as_ref().map_or_else(||self.cst.as_ref().is_none_or(|s|s.lock().expect("CST820-compatible").irq_high()),|s|s.lock().expect("FT compatible").irq_high())}
 fn sync_irq(&mut self){let high=self.high();if high!=self.irq_high{self.irq_high=high;if let Some(pin)=self.variant.irq(){self.edges.push(BoardEdge{cycle:self.cycle,pin,level:high});}}}
}
impl BoardModel for CompactAmoled{
 fn name(&self)->&'static str{self.variant.name()}
 fn named_pin(&self,name:&str)->Option<u8>{match name{"boot"|"button"=>Some(9),"power"|"user" if self.variant==Variant::Touch132=>Some(1),_=>None}}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut p=vec![(9,true)];if self.variant==Variant::Touch132{p.push((1,true));}
  if let Some(irq)=self.variant.irq(){p.push((irq,self.high()));}p}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{self.devices.iter().map(|(a,d)|(0,*a,Box::new(Routed{device:d.clone(),power:if *a==0x34{None}else{self.power.clone()}})as Box<dyn I2cDevice>)).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;if let Some(s)=&self.motion{s.cycle.store(c,Ordering::Relaxed);}
  if let Some(s)=&self.rtc{s.lock().expect("PCF85063").advance_to(c);}self.sync();}
 fn next_deadline(&self)->Option<u64>{self.ft.as_ref().and_then(|s|s.lock().expect("FT compatible").next_deadline())}
 fn take_edges(&mut self)->Vec<BoardEdge>{self.sync_irq();std::mem::take(&mut self.edges)}
 fn touch_irq(&self)->Option<(u8,bool)>{self.variant.irq().map(|p|(p,self.high()))}
 fn touch(&mut self,x:u16,y:u16,down:bool){let(w,h)=self.variant.size();let(x,y)=(x.min(w-1),y.min(h-1));if let Some(s)=&self.ft{s.lock().expect("FT compatible").touch_at(self.cycle,x,y,down);}
  if let Some(s)=&self.cst{s.lock().expect("CST820-compatible").update(x,y,down);}self.sync_irq();}
 fn touch_at(&mut self,c:u64,x:u16,y:u16,down:bool){self.advance_to(c);self.touch(x,y,down);}
 fn set_imu_motion(&mut self,m:u32)->bool{self.motion.as_ref().is_some_and(|s|{s.mode.store(m,Ordering::Relaxed);true})}
 fn imu_motion(&self)->u32{self.motion.as_ref().map_or(0,|s|s.mode.load(Ordering::Relaxed))}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);if self.variant==Variant::Touch132{self.reset=r.software_output(21)==Some(true);}self.sync();let(_,_,cs)=self.variant.bus();let sel=r.software_output(cs)==Some(false);if self.software_cs&&!sel{self.panel.deselect();}self.software_cs=sel;
  if self.variant!=Variant::Touch132{let gpio=r.software_output(6);let hardware=[68,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==6));if gpio==Some(true)||(gpio.is_none()&&!hardware){if let Some(sd)=&mut self.sd{sd.deselect();}}}}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{self.sync();let(clk,data,cs)=self.variant.bus();let floating=vec![255;rx_len];let lcd=p.cs&(1<<cs)!=0;let card=self.variant!=Variant::Touch132&&p.cs&(1<<6)!=0;
  if lcd&&card{self.rejected+=1;self.panel.reject_until_deselect();if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
  if card||!lcd{
   let routed=host==2&&p.sclk&(1<<11)!=0&&p.data[0]&(1<<10)!=0;
   if !routed||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge||p.dummy_cycles!=0||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8){self.rejected+=1;return floating;}
   if let Some(sd)=&mut self.sd{let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(card,tx,rx_len)}else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx_len)};if card{if p.hardware_cs&(1<<6)!=0&&!p.keep_cs&&p.software_cs&(1<<6)==0{sd.deselect();}return if p.input[1]==Some(18){rx}else{floating};}}return floating;
  }
  let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let routed=host==2&&p.sclk&(1<<clk)!=0&&p.data[0]&(1<<data[0])!=0&&(!quad||data[1..].iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));
  if !routed||!self.active||rx_len!=0{self.panel.reject_until_deselect();self.rejected+=1;}else{self.panel.transfer(p,tx);}
  if !p.keep_cs&&p.software_cs&(1<<cs)==0{self.panel.deselect();}floating
 }
 fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{if self.variant==Variant::Touch132{return Err("AMOLED1.32 has no TF socket".into());}let sd=SpiSdCard::new(image,config)?;let d=sd.describe();self.sd=Some(sd);Ok(d)}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let(w,h)=self.variant.size();Some((w as u32,h as u32,if self.active{self.panel.frame()}else{vec![0;w as usize*h as usize]},self.panel.version()))}
 fn display_version(&self)->u64{self.panel.version()}
 fn display_frames(&self)->u64{self.panel.frames()}
 fn report(&self)->String{format!("[{}] {} {}x{} RAMWR={} pixels={} active={} MADCTL={:02x} rejected={}/{}; source-specific digital touch/control; raw contact coordinates; SD {}; RF/audio/analog rails/private ID/TE/physical touch registration unverified",self.name(),self.panel.controller(),self.panel.width(),self.panel.height(),self.panel.frames(),self.panel.pixels_written(),self.active,self.panel.madctl,self.rejected,self.panel.rejected,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}
#[cfg(test)]mod tests{
 use super::*;use esp_periph::{Gpio,RegRam};use esp_soc::pins::ChipPins;
 fn write(d:&mut dyn I2cDevice,a:u8,b:&[u8]){assert!(d.start_address_with_clock(a,a,false,None));for v in b{assert!(d.write(*v));}d.stop();}
 fn pins(b:&CompactAmoled,n:usize,w:u8,keep:bool)->SpiQspiPins{let(c,d,s)=b.variant.bus();SpiQspiPins{sclk:1<<c,data:d.map(|p|1<<p),cs:1<<s,hardware_cs:1<<s,data_width:w,data_bits:n as u32*8,keep_cs:keep,..Default::default()}}
 fn command(b:&mut CompactAmoled,c:u8,p:&[u8]){let r=pins(b,4,1,!p.is_empty());b.spi_transfer_qspi_pins(2,r,&[2,0,c,0],0);if !p.is_empty(){let r=pins(b,p.len(),1,false);b.spi_transfer_qspi_pins(2,r,p,0);}}
 #[test]fn waveshare_compact_amoled_revision_devices_reset_and_quad_pixels(){for v in[Variant::Touch18V1,Variant::Touch18V2,Variant::Touch132]{
  let mut b=CompactAmoled::new(v);let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((8,7))));let addresses=ds.iter().map(|(_,a,_)|*a).collect::<Vec<_>>();assert_eq!(addresses,match v{Variant::Touch18V1=>vec![0x18,0x34,0x20,0x38,0x6b,0x51],Variant::Touch18V2=>vec![0x18,0x34,0x20,0x15,0x6b,0x51],Variant::Touch132=>vec![0x18,0x15]});
  if v==Variant::Touch132{let mut g=Gpio::new();let mut m=RegRam::new();g.func_out_sel[21]=128;g.enable=1<<21;g.out=g.enable;m.write(4+21*4,1<<12|1<<9);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));}
  else{let d=&mut ds.iter_mut().find(|(_,a,_)|*a==0x20).unwrap().2;write(&mut **d,0x20,&[3,0xcf]);write(&mut **d,0x20,&[1,0x30]);b.advance_to(0);}
  let(gap,mad)=match v{Variant::Touch18V1=>(0,0),Variant::Touch18V2=>(16,0),Variant::Touch132=>(6,0xc0)};
  command(&mut b,0x36,&[mad]);command(&mut b,0x3a,&[0x55]);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);command(&mut b,0x53,&[0x20]);command(&mut b,0x51,&[255]);command(&mut b,0x2a,&[0,gap,0,gap]);command(&mut b,0x2b,&[0,0,0,0]);let p=pins(&b,4,1,true);b.spi_transfer_qspi_pins(2,p,&[0x32,0,0x2c,0],0);let p=pins(&b,2,4,false);b.spi_transfer_qspi_pins(2,p,&[0xf8,0],0);assert_eq!(b.display().unwrap().2[0],0xf800);
  b.advance_to(crate::periph::CPU_HZ/10);b.touch(999,999,true);let address=if v==Variant::Touch18V1{0x38}else{0x15};let d=&mut ds.iter_mut().find(|(_,a,_)|*a==address).unwrap().2;write(&mut **d,address,&[2]);assert!(d.start_address_with_clock(address,address,true,None));let report=(0..5).map(|_|d.read()).collect::<Vec<_>>();let(w,h)=v.size();assert_eq!(report,[1,((w-1)>>8)as u8,(w-1)as u8,((h-1)>>8)as u8,(h-1)as u8]);d.stop();assert_eq!(b.touch_irq().map(|p|p.0),v.irq());
  if v!=Variant::Touch132{let d=&mut ds.iter_mut().find(|(_,a,_)|*a==0x20).unwrap().2;write(&mut **d,0x20,&[1,0]);b.advance_to(crate::periph::CPU_HZ/10+1);assert!(b.display().unwrap().2.iter().all(|p|*p==0));let d=&mut ds.iter_mut().find(|(_,a,_)|*a==address).unwrap().2;assert!(!d.start_address_with_clock(address,address,false,None));}
  else{assert_eq!(b.named_pin("power"),Some(1));assert!(!b.set_imu_motion(1),"no invented IMU on1.32");}
 }}
 #[test]fn waveshare_amoled18_separate_sd_bus_rejects_display_clock(){use esp_periph::sdmmc::card::MemStore;
  let mut b=CompactAmoled::new(Variant::Touch18V2);b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).unwrap();let mut p=SpiQspiPins{sclk:1<<11,data:[1<<10,0,0,0],data_width:1,read_width:1,data_bits:80,..Default::default()};b.spi_transfer_qspi_pins(2,p,&[255;10],0);p.cs=1<<6;p.software_cs=p.cs;p.input[1]=Some(18);p.data_bits=48;b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],0);p.data_bits=0;p.read_bits=16;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,1]);p.sclk=1;p.data[0]=2;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[],2),[255,255]);
 }
}
