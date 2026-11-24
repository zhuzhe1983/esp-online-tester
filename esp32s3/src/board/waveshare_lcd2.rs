//! Exact Waveshare LCD2 / Touch-LCD2: ST7789, shared SPI TF and CST816D.
//! Eighth source manifest: original Arduino01_factory, V2 schematic and CST BSP.
//! R16 is NC in the shipped reset option: LCD/TP use the shared RC reset node;
//! GPIO0 is BOOT and is not invented as a populated reset connection.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::{i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}},sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,i2c_bitbang::I2cBitBang,qmi8658::{Qmi8658,Qmi8658Port},imu_motion::ImuMotion}};
const HZ:u64=240_000_000;
// Manufacturer explicitly identifies CST816D and reads A7=B6. No firmware ID,
// gesture, calibration or reserved-register success is fabricated.
struct Cst816D{inner:Cst816s,first:bool,ptr:Option<u8>}
impl I2cDevice for Cst816D{
 fn start(&mut self,r:bool)->bool{if !r{self.first=true;self.ptr=None;}self.inner.start(r)}
 fn write(&mut self,v:u8)->bool{if self.first{self.first=false;self.ptr=matches!(v,0..=8|0xa5|0xa7..=0xa9).then_some(v);return self.ptr.is_some()&&self.inner.write(v);}
 if self.ptr==Some(0xa5)&&v==3{self.ptr=Some(0xa6);return self.inner.write(v);}false}
 fn read(&mut self)->u8{let value=self.inner.read();let p=self.ptr;self.ptr=p.map(|p|p.wrapping_add(1));if p==Some(0xa7){0xb6}else{value}}
 fn stop(&mut self){self.inner.stop();}
}
pub struct WaveshareLcd2{touch_variant:bool,panel:DcsPanel,cmd:u8,dc:Option<bool>,light:u32,gpio_light:u32,pwm:Option<u32>,version:u64,rejected:u64,cycle:u64,touch:Arc<Mutex<Cst816State>>,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,i2c:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,card:Option<SpiSdCard>,present:bool,sd_rejected:u64}
impl WaveshareLcd2{
 pub fn new(touch_variant:bool)->Self{let mut touch=Cst816State::default();touch.powered=touch_variant;let motion=Arc::new(ImuMotion::default());Self{touch_variant,panel:DcsPanel::st7789(),cmd:0,dc:None,light:0,gpio_light:0,pwm:None,version:0,rejected:0,cycle:0,touch:Arc::new(Mutex::new(touch)),qmi:Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(),HZ))),motion,i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:vec![],card:None,present:false,sd_rejected:0}}
 fn sync(&mut self){let light=self.pwm.unwrap_or(self.gpio_light);if self.light!=light{self.light=light;self.version+=1;}
 if self.touch_variant{let high=self.touch.lock().expect("CST816D").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:46,level:high});}}}
}
impl BoardModel for WaveshareLcd2{
 fn name(&self)->&'static str{if self.touch_variant{"waveshare-s3-touch-lcd2"}else{"waveshare-s3-lcd2"}}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);self.dc=r.software_output(42);self.gpio_light=if r.software_output(1)==Some(true){65535}else{0};if r.software_output(41)==Some(true){if let Some(card)=&mut self.card{card.deselect();}}let scl=r.software_output(47).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(48).unwrap_or(true));for(i,(pin,level))in[(48,sda),(47,scl)].into_iter().enumerate(){if level!=self.levels[i]{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.advance_to(c);self.pwm=f(1).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{let lcd=p.cs&(1<<45)!=0;let card=p.cs&(1<<41)!=0;let wired=p.sclk&(1<<39)!=0&&p.data[0]&(1<<38)!=0;let serial=!p.unsupported_mode&&!p.sio&&!p.clock_output_edge&&p.dummy_cycles==0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|w|*w<=1)&&p.command_bits.is_multiple_of(8)&&p.address_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8)&&p.read_bits.is_multiple_of(8);
 if lcd&&card{self.rejected+=1;self.sd_rejected+=1;if let Some(s)=&mut self.card{s.deselect();}return vec![255;rx];}
 if lcd{if !wired||!serial||p.address_bits!=0||self.dc.is_none(){self.rejected+=1;return vec![255;rx];}self.panel.dc=self.dc==Some(true);if self.panel.dc&&matches!(self.cmd,0x2c|0x3c)&&self.panel.colmod&7!=5{self.rejected+=1;return vec![255;rx];}for &v in tx{if !self.panel.dc{self.cmd=v;}self.panel.byte(v);}self.version+=1;return vec![255;rx];}
 if card||wired{if !wired||!serial||p.clock_idle_high||!self.present{self.sd_rejected+=u64::from(card);return vec![255;rx];}
 if let Some(s)=&mut self.card{let reply=if p.full_duplex||tx.is_empty()||rx==0{s.transfer(card,tx,rx)}else{s.transfer(card,tx,0);s.transfer(card,&[],rx)};if !p.keep_cs&&p.software_cs&(1<<41)==0{s.deselect();}return if p.input[1]==Some(40){reply}else{vec![255;rx]};}}
 vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));if self.touch_variant{self.i2c.attach(0x15,Box::new(Cst816D{inner:Cst816s::new(self.touch.clone()),first:true,ptr:None}));}self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((48,47))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.motion.cycle.store(c,Ordering::Relaxed);self.sync();}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut v=vec![(0,true),(48,self.levels[0]),(47,self.levels[1])];if self.touch_variant{v.push((46,self.levels[2]));}v}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn touch(&mut self,x:u16,y:u16,down:bool){if self.touch_variant{self.touch.lock().expect("CST816D").update(x.min(239),y.min(319),down);self.sync();}}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn attach_spi_sd_card(&mut self,s:Box<dyn SdStore>,c:SdCardConfig)->Result<String,String>{let card=SpiSdCard::new(s,c)?;let description=card.describe();self.card=Some(card);self.present=true;Ok(description)}
 fn set_sd_card_present(&mut self,v:bool){if v!=self.present{if let Some(c)=&mut self.card{c.power_cycle();}}self.present=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let mut frame=vec![0;240*320];if self.light>0&&self.panel.on&&!self.panel.sleeping{for(dst,&p)in frame.iter_mut().zip(&self.panel.gram){let p=if self.panel.madctl&8!=0{(p&0x7e0)|(p>>11)|((p&31)<<11)}else{p};let v=u32::from(p);let d=self.light;*dst=((((v>>11)*d/65535)<<11)|((((v>>5)&63)*d/65535)<<5)|((v&31)*d/65535))as u16;}}Some((240,320,frame,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("{} ST7789 physical240x320 shared SPI CLK39/MOSI38/MISO40 LCDCS45/DC42, RC reset(R16NC), BL1high. CST816D@15={} SDA48/SCL47/IRQ46 rawphysicalXY; factory rotation is performed by guest MADCTL/touch transform. QMI8658C@6B; SPI TFCS41 {}. pixels={} reject={} SDreject={}. No OV5640 camera, analog BAT_ADC5/charging, QMIinterrupt/calibration, touch gestures/firmware-ID, physical glass calibration or full application validation.",self.name(),self.touch_variant,self.card.as_ref().map_or_else(||"unattached".into(),SpiSdCard::report),self.panel.pixels_written,self.rejected,self.sd_rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareLcd2,dc:bool,bl:bool,boot:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(p,v)in[(42u8,dc),(1,bl),(0,boot)]{g.enable|=1<<p;if v{g.out|=1<<p;}m.write(4+4*u32::from(p),1<<12);}b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins(n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<39,data:[1<<38,0,0,0],cs:1<<45,data_width:1,data_bits:(n*8)as u32,..Default::default()}}
 fn cmd(b:&mut WaveshareLcd2,c:u8,a:&[u8]){route(b,false,true,true);b.spi_transfer_qspi_pins(2,pins(1),&[c],0);route(b,true,true,true);b.spi_transfer_qspi_pins(2,pins(a.len()),a,0);}
 #[test]fn real_gram_rotation_and_default_nc_reset(){for touch in[false,true]{let mut b=WaveshareLcd2::new(touch);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[5]);cmd(&mut b,0x36,&[0x60]);cmd(&mut b,0x2a,&[0,17,0,17]);cmd(&mut b,0x2b,&[0,5,0,5]);cmd(&mut b,0x2c,&[]);let mut p=pins(2);p.full_duplex=true;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[0xf8,0],2),[255;2]);assert_eq!(b.display().unwrap().2[17*240+234],0xf800);route(&mut b,true,true,false);assert_eq!(b.display().unwrap().2[17*240+234],0xf800,"BOOT has no populated R16 reset link");p.data[0]=1<<37;b.spi_transfer_qspi_pins(2,p,&[0,0],2);assert_eq!(b.panel.pixels_written,1);route(&mut b,true,false,true);assert!(b.display().unwrap().2.iter().all(|v|*v==0));b.gpio_pwm_at(0,&|p|(p==1).then_some((1000.0,32768)));let frame=b.display().unwrap().2;assert_ne!(frame[17*240+234],0);b.gpio_pwm_at(0,&|p|(p==1).then_some((9000.0,32768)));assert_eq!(b.display().unwrap().2,frame);}}
 #[test]fn touch_identity_physical_axes_and_non_touch_absence(){let mut b=WaveshareLcd2::new(true);let mut devs=b.i2c_devices();let d=devs.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x15).unwrap().2.as_mut();assert_eq!(d.pins(),Some((48,47)));assert!(d.start(false));assert!(d.write(0xa7));assert!(d.start(true));assert_eq!(d.read(),0xb6);b.touch(70,250,true);assert!(!b.input_levels().iter().find(|(p,_)|*p==46).unwrap().1);d.start(false);d.write(2);d.start(true);assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[1,0,70,0,250]);b.touch(70,250,false);d.start(false);d.write(2);d.start(true);assert_eq!(d.read(),0);d.start(false);assert!(!d.write(0x70));let mut n=WaveshareLcd2::new(false);assert!(n.i2c_devices().iter().all(|(_,a,_)|*a!=0x15));}
 #[test]fn spi_sd_cmd0_and_wiring_isolation(){let mut b=WaveshareLcd2::new(false);b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();let mut p=SpiQspiPins{sclk:1<<39,data:[1<<38,0,0,0],input:[None,Some(40),None,None],cs:0,data_width:1,data_bits:80,..Default::default()};b.spi_transfer_qspi_pins(2,p,&[255;10],0);p.cs=1<<41;p.data_bits=48;p.read_bits=64;p.read_width=1;assert!(b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],8).contains(&1));p.input[1]=Some(37);assert_eq!(b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],8),[255;8]);p.cs|=1<<45;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],8),[255;8]);b.set_sd_card_present(false);p.cs=1<<41;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[0x40,0,0,0,0,0x95],8),[255;8]);assert!(b.sd_card_slot().is_none(),"Public attachment selects this real SPI transport, not an unimplemented native host");}
}
