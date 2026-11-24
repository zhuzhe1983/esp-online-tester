//! ESP32-S3-Touch-AMOLED-1.64 PCB Rev1.1 only. The official v2 repository
//! names its driver SH8601; physical overview identifies CO5300. Keep that
//! distinction: shared CO5300 RGB565 subset, gap20, no invented die readback.
//! Schematic R34=NC/R35=0 means touch reset follows LCD_RESET21, NOT GPIO8.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,qspi_dcs::QspiDcsPanel,ft3168::{Ft3168,Ft3168State},qmi8658::{Qmi8658,Qmi8658Port},imu_motion::ImuMotion}};
const HZ:u64=240_000_000;
// Factory touch_Init writes DEV_MODE00h=00h before reading02..06. Only this
// source-proven normal-mode command is added; other report writes stay NACK.
struct NormalMode{inner:Ft3168,first:bool,reg:Option<u8>,written:bool}
impl I2cDevice for NormalMode{
 fn start(&mut self,r:bool)->bool{if !r{self.first=true;self.reg=None;self.written=false;}self.inner.start(r)}
 fn write(&mut self,v:u8)->bool{if self.first{self.first=false;self.reg=Some(v);return self.inner.write(v);}
 if self.reg==Some(0)&&v==0&&!self.written{self.written=true;return true;}self.inner.write(v)}
 fn read(&mut self)->u8{self.inner.read()}
 fn stop(&mut self){self.inner.stop();}
}
pub struct WaveshareAmoled164{panel:QspiDcsPanel,touch:Arc<Mutex<Ft3168State>>,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,i2c:I2cBitBang,attached:bool,cycle:u64,reset:bool,selected:bool,sd:Option<SpiSdCard>,present:bool,native_select:bool,levels:[bool;3],edges:Vec<BoardEdge>,sd_rejected:u64}
impl Default for WaveshareAmoled164{fn default()->Self{Self::new()}}
impl WaveshareAmoled164{
 pub fn new()->Self{let motion=Arc::new(ImuMotion::default());Self{panel:QspiDcsPanel::co5300(280,456,20,0),touch:Arc::new(Mutex::new(Ft3168State::new(HZ))),qmi:Rc::new(RefCell::new(Qmi8658::new(motion.clone(),HZ))),motion,i2c:I2cBitBang::new(),attached:false,cycle:0,reset:false,selected:false,sd:None,present:false,native_select:true,levels:[true;3],edges:Vec::new(),sd_rejected:0}}
 fn sync_irq(&mut self){let high=self.touch.lock().expect("AM164 FT3168").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:18,level:high});}}
}
impl BoardModel for WaveshareAmoled164{
 fn name(&self)->&'static str{"waveshare-s3-touch-amoled164-rev11"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let reset=r.software_output(21)==Some(true);if self.reset&&!reset{self.panel.reset();}self.reset=reset;self.touch.lock().expect("AM164 FT3168").hardware_at(c,true,reset);let selected=r.software_output(46)==Some(false);if self.selected&&!selected{self.panel.deselect();}self.selected=selected;let scl=r.software_output(48).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(47).unwrap_or(true));for(i,(pin,level))in[(47,sda),(48,scl)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}self.native_select=r.software_output(38).unwrap_or(true);if r.software_output(38)==Some(true){if let Some(s)=&mut self.sd{s.deselect();}}self.sync_irq();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{
  let lcd=p.cs&(1<<46)!=0;let card=p.cs&(1<<38)!=0;
  if lcd&&card{self.panel.reject_until_deselect();self.sd_rejected+=1;if let Some(s)=&mut self.sd{s.deselect();}return vec![255;rx];}
  if card||(!lcd&&p.sclk&(1<<41)!=0&&p.data[0]&(1<<39)!=0){let valid=self.present&&!p.unsupported_mode&&!p.sio&&!p.clock_idle_high&&!p.clock_output_edge&&p.dummy_cycles==0&&p.sclk&(1<<41)!=0&&p.data[0]&(1<<39)!=0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|w|*w<=1)&&p.command_bits.is_multiple_of(8)&&p.address_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8)&&p.read_bits.is_multiple_of(8);if !valid{self.sd_rejected+=1;return vec![255;rx];}
 if let Some(s)=&mut self.sd{let reply=if p.full_duplex||tx.is_empty()||rx==0{s.transfer(card,tx,rx)}else{s.transfer(card,tx,0);s.transfer(card,&[],rx)};if !p.keep_cs&&p.software_cs&(1<<38)==0{s.deselect();}return if p.input[1]==Some(40){reply}else{vec![255;rx]};}return vec![255;rx];}
  if !lcd{return vec![255;rx];}let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let routed=p.sclk&(1<<10)!=0&&p.data[0]&(1<<11)!=0&&(!quad||[12,13,14].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));if !routed||!self.reset||rx!=0{self.panel.reject_until_deselect();}else{self.panel.transfer(p,tx);}
 if !p.keep_cs&&p.software_cs&(1<<46)==0{self.panel.deselect();}vec![255;rx]
 }
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x38,Box::new(NormalMode{inner:Ft3168::new(self.touch.clone()),first:true,reg:None,written:false}));self.i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((47,48))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.motion.cycle.store(c,Ordering::Relaxed);self.touch.lock().expect("AM164 FT3168").advance_to(c);self.sync_irq();}
 fn next_deadline(&self)->Option<u64>{self.touch.lock().expect("AM164 FT3168").next_deadline()}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(47,self.levels[0]),(48,self.levels[1]),(18,self.levels[2])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.touch.lock().expect("AM164 FT3168").touch_at(self.cycle,y.min(455),x.min(279),down);self.sync_irq();}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,c:SdCardConfig)->Result<String,String>{let sd=SpiSdCard::new(image,c)?;let description=sd.describe();self.sd=Some(sd);self.present=true;Ok(description)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([41,39,40,255,255,255],self.present&&self.native_select))}
 fn set_sd_card_present(&mut self,v:bool){if self.present!=v{if let Some(s)=&mut self.sd{s.power_cycle();}}self.present=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((280,456,if self.reset{self.panel.frame()}else{vec![0;280*456]},self.display_version()))}
 fn display_version(&self)->u64{self.panel.version()}
 fn display_frames(&self)->u64{self.panel.frames()}
 fn report(&self)->String{format!("AMOLED1.64 Rev1.1 CO5300 overview / SH8601 BSP class; 280x456 xgap20 CLK10 D11..14 CS46 RESET21sharedFT (R34NC/R35zero), FT3168@38 SDA47SCL48IRQ18, rawXY=(hostY,hostX); QMI8658@6B sample subset; BOOT0; TF native1bit41/39/40 (DAT3=38pulledhigh) or explicit SPI attachment CLK41 MOSI39 MISO40 CS38 {}; pixels={} SDreject={}. EarlierPCB CS9 is NOT accepted. No physical die ID, analog battery/charger, QMI interrupts/calibration, touch gesture/cadence, TE45, vendor power/gamma or complete physical validation.",self.sd.as_ref().map_or_else(||"unattached".into(),SpiSdCard::report),self.panel.pixels_written(),self.sd_rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareAmoled164,on:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();g.enable=1<<21;if on{g.out=1<<21;}m.write(4+4*21,1<<12);b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn packet(c:u8,d:&[u8])->(SpiQspiPins,Vec<u8>){let quad=c==0x2c;let mut b=vec![if quad{0x32}else{2},0,c,0];b.extend(d);(SpiQspiPins{sclk:1<<10,data:[1<<11,1<<12,1<<13,1<<14],cs:1<<46,command_bits:8,address_bits:24,data_bits:(d.len()*8)as u32,command_width:1,address_width:1,data_width:if d.is_empty(){0}else if quad{4}else{1},..Default::default()},b)}
 fn cmd(b:&mut WaveshareAmoled164,c:u8,d:&[u8]){let(p,v)=packet(c,d);b.spi_transfer_qspi_pins(2,p,&v,0);}
 #[test]fn new_revision_shared_reset_gap_and_swapped_raw_touch(){let mut b=WaveshareAmoled164::new();route(&mut b,true);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x51,&[255]);cmd(&mut b,0x2a,&[0,20,0,20]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);let n=b.panel.pixels_written();let(mut p,v)=packet(0x2c,&[0,0]);p.cs=1<<9;b.spi_transfer_qspi_pins(2,p,&v,0);assert_eq!(b.panel.pixels_written(),n);let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert_eq!(d.pins(),Some((47,48)));assert!(d.start(false));assert!(d.write(0));assert!(d.write(0));assert!(!d.write(1));b.advance_to(HZ/10);b.touch(123,345,true);assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[1,1,89,0,123]);route(&mut b,false);assert!(!d.start(false));assert!(b.display().unwrap().2.iter().all(|v|*v==0));}
 #[test]fn physical_tf_spi_accepts_only_its_clock_data_and_miso(){let mut b=WaveshareAmoled164::new();b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();let mut p=SpiQspiPins{sclk:1<<41,data:[1<<39,0,0,0],cs:1<<38,input:[None,Some(40),None,None],data_width:1,data_bits:48,read_width:1,read_bits:64,..Default::default()};b.spi_transfer_qspi_pins(3,SpiQspiPins{cs:0,data_bits:80,read_bits:0,read_width:0,..p},&[255;10],0);let reply=b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8);assert!(reply.contains(&1),"CMD0 enters SPI idle over real TF pins");p.input[1]=Some(37);let reply=b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8);assert_eq!(reply,vec![255;8]);p.sclk=1<<10;b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8);assert_eq!(b.sd_rejected,1);assert_eq!(b.sd_card_slot(),Some(([41,39,40,255,255,255],true)),"Rev1.1 same TF pins also provide the original BIN native1bit route; D1/D2 strapped high, not fabricated four-bit data lanes");}
}
