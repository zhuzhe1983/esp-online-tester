//! Current ESP32-S3-Touch-AMOLED-2.41 V2, exact reset/IRQ rewiring.
//! RM690B0 explicitly named BSP digital subset; not the SH8601 class-name alias.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{rm690b0::Rm690b0Bsp,i2c_bitbang::I2cBitBang,tca9554::{Tca9554,Tca9554State},pcf85063::{Pcf85063,Pcf85063State},imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};
const HZ:u64=240_000_000;
pub struct WaveshareAmoled241{panel:Rm690b0Bsp,cycle:u64,reset:bool,selected:bool,touch:Arc<Mutex<Ft6336State>>,expander:Arc<Mutex<Tca9554State>>,rtc:Arc<Mutex<Pcf85063State>>,bus:I2cBitBang,attached:bool,levels:[bool;4],edges:Vec<BoardEdge>,card:bool,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>}
impl Default for WaveshareAmoled241{fn default()->Self{Self::new()}}
impl WaveshareAmoled241{
 pub fn new()->Self{let motion=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new(motion.clone(),HZ)));Self{panel:Rm690b0Bsp::new(),cycle:0,reset:false,selected:false,touch:Default::default(),expander:Default::default(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),bus:I2cBitBang::new(),attached:false,levels:[true;4],edges:Vec::new(),card:false,motion,qmi}}
 fn sync(&mut self){let e=self.expander.lock().expect("AM241 expander");let reset=e.driven(0)==Some(true);if self.reset&&!reset{self.panel.reset();}self.reset=reset;let mut t=self.touch.lock().expect("FT6336U");let tr=e.driven(1)==Some(true);if t.powered&&!tr{t.reset();}t.powered=tr;let touch_high=!t.powered||!t.down;let ex_high=e.irq_high();for(i,(pin,high))in[(3,touch_high),(18,ex_high)].into_iter().enumerate(){if high!=self.levels[i+2]{self.levels[i+2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin,level:high});}}}
}
impl BoardModel for WaveshareAmoled241{
 fn name(&self)->&'static str{"waveshare-s3-touch-amoled241-v2"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let selected=r.software_output(9)==Some(false);if self.selected&&!selected{self.panel.deselect();}self.selected=selected;let clock=r.software_output(48).unwrap_or(true);let data=self.bus.drive(clock,r.software_output(47).unwrap_or(true));for(i,(pin,high))in[(47,data),(48,clock)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<9)==0{return vec![255;rx];}self.sync();let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let pads=p.sclk&(1<<10)!=0&&p.data[0]&(1<<11)!=0&&(!quad||[12,13,14].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));if self.reset&&pads&&rx==0{self.panel.transfer(p,b);}else{self.panel.reject();}
 if !p.keep_cs&&p.software_cs&(1<<9)==0{self.panel.deselect();}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.bus.attach(0x20,Box::new(Tca9554::new(self.expander.clone())));self.bus.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.bus.attach(0x38,Box::new(Ft6336g::unknown_identity(self.touch.clone())));self.bus.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));self.attached=true;}(0..2).flat_map(|b|self.bus.controller_devices(Some((47,48))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.rtc.lock().expect("AM241 RTC").advance_to(c);self.motion.cycle.store(c,Ordering::Relaxed);self.sync();}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.sync();let mut t=self.touch.lock().expect("FT6336U");if t.powered{t.update(y.min(449),599-x.min(599),down);}drop(t);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(15,true),(47,self.levels[0]),(48,self.levels[1]),(3,self.levels[2]),(18,self.levels[3])]}
 fn named_pin(&self,n:&str)->Option<u8>{match n{"boot"=>Some(0),"key"=>Some(15),_=>None}}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([4,5,6,255,255,255],self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((600,450,if self.reset{self.panel.frame_landscape()}else{vec![0;600*450]},self.panel.version()))}
 fn display_version(&self)->u64{self.panel.version()}
 fn display_frames(&self)->u64{self.panel.frames()}
 fn report(&self)->String{format!("AMOLED2.41 V2 RM690B0 RGB565 BSP subset, QSPI10/[11,12,13,14]/CS9, TCA20 bit0OLEDreset/bit1FTreset/bit2RTCIRQ, GPIO3touchIRQ/GPIO18expanderIRQ; I2C47/48 FT6336U@38 UNKNOWN IDs, RTC51 QMI6B. Visible600x450 is factory landscape transpose of physical x16..465/y0..599; MADCTR30h BSP vs sparse datasheet conflict explicit. BOOT0 KEY15 SDMMC1bit4/5/6. pixels={} rejected={} visible={} format={:02x} madctl={:02x} brightness={} control={:02x} nonzeroGRAM={}. FT level IRQ is deterministic held-contact policy; no RTC alarm/IRQ engine, calibrated pulse/scan timing, analog brightness/gamma, battery switch/charge behavior, SPI SD, complete application or physical acceptance; V1 wiring not accepted.",self.panel.pixels_written(),self.panel.rejected,self.panel.visible(),self.panel.pixel_format(),self.panel.madctl(),self.panel.brightness,self.panel.control,self.panel.nonzero_gram())}
}
#[cfg(test)]mod tests{use super::*;
 fn write(d:&mut dyn I2cDevice,r:u8,v:u8){assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));d.stop();}
 fn packet(c:u8,d:&[u8])->(SpiQspiPins,Vec<u8>){let quad=matches!(c,0x2c|0x3c);let mut b=vec![if quad{0x32}else{2},0,c,0];b.extend(d);(SpiQspiPins{sclk:1<<10,data:[1<<11,1<<12,1<<13,1<<14],cs:1<<9,command_bits:8,address_bits:24,data_bits:(d.len()*8)as u32,command_width:1,address_width:1,data_width:if d.is_empty(){0}else if quad{4}else{1},..Default::default()},b)}
 fn cmd(b:&mut WaveshareAmoled241,c:u8,d:&[u8]){let(p,v)=packet(c,d);b.spi_transfer_qspi_pins(2,p,&v,0);}
 #[test]fn v2_tca_resets_display_touch_and_wrong_old_reset_are_distinct(){let mut b=WaveshareAmoled241::new();let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((47,48))));let ex=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();write(ex,3,0xfc);write(ex,1,1);b.advance_to(0);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x51,&[255]);cmd(&mut b,0x36,&[0x30]);cmd(&mut b,0x2a,&[0,17,0,17]);cmd(&mut b,0x2b,&[0,21,0,21]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[5*600+17],0xf800);let tp=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert!(!tp.start(false));let ex=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();write(ex,1,3);b.advance_to(1);b.touch(123,345,true);let tp=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert!(tp.start(false));tp.write(2);tp.start(true);assert_eq!((0..5).map(|_|tp.read()).collect::<Vec<_>>(),[1,0x81,89,1,220]);let n=b.panel.pixels_written();let(mut p,v)=packet(0x2c,&[0,0]);p.data[3]=1<<15;b.spi_transfer_qspi_pins(2,p,&v,0);assert_eq!(b.panel.pixels_written(),n);let ex=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();write(ex,1,2);b.advance_to(2);assert!(b.display().unwrap().2.iter().all(|v|*v==0));b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([4,5,6,255,255,255],true)));assert_eq!(b.named_pin("key"),Some(15));}
}
