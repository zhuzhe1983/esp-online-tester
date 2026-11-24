//! ESP32-S3-Touch-LCD-3.5 V2 ST7796 / FT6336, not 3.5B's AXS15231B.
//! R35 pulls LCD CS permanently low; TCA EX1 resets LCD+touch, EX6 PWR high.
//! Official schematic and fixed BSP are retained in seventh-source-manifest.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,i2c_bitbang::I2cBitBang,tca9554::{Tca9554,Tca9554State},pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,axp2101::{Axp2101,Axp2101Power},imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};
const HZ:u64=240_000_000;
pub struct WaveshareLcd35{panel:DcsPanel,dc:Option<bool>,command:u8,version:u64,cycle:u64,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,touch:Arc<Mutex<Ft6336State>>,expander:Arc<Mutex<Tca9554State>>,rtc:Arc<Mutex<Pcf85063State>>,bus:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,power_pressed:bool,power:Arc<Mutex<Axp2101Power>>,card:bool,rejected:u64,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>}
impl Default for WaveshareLcd35{fn default()->Self{Self::new()}}
impl WaveshareLcd35{
 pub fn new()->Self{let motion=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new(motion.clone(),HZ)));Self{panel:DcsPanel::new(320,480),dc:None,command:0,version:0,cycle:0,reset:false,light:0,gpio_light:0,pwm:None,touch:Default::default(),expander:Default::default(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),bus:I2cBitBang::new(),attached:false,levels:[true;3],edges:Vec::new(),power_pressed:false,power:Default::default(),card:false,rejected:0,motion,qmi}}
 fn sync(&mut self){let mut e=self.expander.lock().expect("LCD35 expander");e.set_inputs(1<<6,u8::from(self.power_pressed)<<6);let reset=e.driven(1)==Some(true);if self.reset&&!reset{self.panel.reset();self.command=0;self.version+=1;}self.reset=reset;let high=e.irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:4,level:high});}let mut t=self.touch.lock().expect("FT6336");if t.powered&&!reset{t.reset();}t.powered=reset;let light=self.pwm.unwrap_or(self.gpio_light);if self.light!=light{self.light=light;self.version+=1;}}
}
impl BoardModel for WaveshareLcd35{
 fn name(&self)->&'static str{"waveshare-s3-touch-lcd35-v2"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);self.dc=r.software_output(3);self.gpio_light=if r.software_output(6)==Some(true){65535}else{0};let clock=r.software_output(7).unwrap_or(true);let data=self.bus.drive(clock,r.software_output(8).unwrap_or(true));for(i,(pin,high))in[(8,data),(7,clock)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.cycle=c;self.pwm=f(6).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{self.sync();let valid=self.reset&&self.dc.is_some()&&p.sclk&(1<<5)!=0&&p.data[0]&(1<<1)!=0&&rx==0&&!p.unsupported_mode&&!p.full_duplex&&!p.sio&&p.dummy_cycles==0&&!p.clock_idle_high&&!p.clock_output_edge&&p.address_bits==0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|v|*v<=1)&&p.command_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8);if !valid{self.rejected+=1;return vec![255;rx];}self.panel.dc=self.dc==Some(true);if self.panel.dc&&matches!(self.command,0x2c|0x3c)&&self.panel.colmod&7!=5{self.rejected+=1;return vec![255;rx];}for &v in b{if !self.panel.dc{self.command=v;}self.panel.byte(v);}self.version+=1;vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.bus.attach(0x20,Box::new(Tca9554::new(self.expander.clone())));self.bus.attach(0x34,Box::new(Axp2101::new(self.power.clone())));self.bus.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.bus.attach(0x18,Box::new(Es8311::new()));self.bus.attach(0x38,Box::new(Ft6336g::unknown_identity(self.touch.clone())));self.bus.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));self.attached=true;}(0..2).flat_map(|b|self.bus.controller_devices(Some((8,7))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.rtc.lock().expect("LCD35 RTC").advance_to(c);self.motion.cycle.store(c,Ordering::Relaxed);self.sync();}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.sync();let mut t=self.touch.lock().expect("FT6336");if t.powered{t.update(x.min(319),y.min(479),down);}}
 fn button_at(&mut self,c:u64,id:&str,down:bool)->bool{if id!="power"{return false;}self.cycle=c;self.power_pressed=down;self.sync();true}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(8,self.levels[0]),(7,self.levels[1]),(4,self.levels[2])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([11,10,9,255,255,255],self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let mut f=vec![0;320*480];if self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping{for y in 0..480{for x in 0..320{let mut p=self.panel.gram[y*320+319-x];if (self.panel.madctl^0x48)&8!=0{p=(p&0x7e0)|(p>>11)|((p&31)<<11);}let p=u32::from(p);let d=self.light;f[y*320+x]=((((p>>11)*d/65535)<<11)|((((p>>5)&63)*d/65535)<<5)|((p&31)*d/65535))as u16;}}}Some((320,480,f,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("TouchLCD3.5 V2 ST7796 320x480 SPI5/1/DC3, CS physical R35 ground (no MCU CS), TCA20 EX1 shared reset/EX6 PWR-high/INT4; I2C8/7 FT6336@38 common report with UNKNOWN identities, AXP34 RTC51 ES8311@18 QMI6B. BL6high SDMMC1bit11/10/9 BOOT0. pixels={} rejected={}. Source BSP MADCTL48 reference is digital presentation, not glass calibration. No FT interrupt cadence, SD detect mux, OV5640 camera, PCM, analog PMU rails/calibration, full application or physical validation.",self.panel.pixels_written,self.rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn write(d:&mut dyn I2cDevice,r:u8,v:u8){assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));d.stop();}
 fn route(b:&mut WaveshareLcd35,dc:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(pin,v)in[(3,dc),(6,true)]{g.enable|=1<<pin;if v{g.out|=1<<pin;}m.write(4+4*pin,1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn packet(n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<5,data:[1<<1,0,0,0],data_width:1,data_bits:n as u32*8,..Default::default()}}
 fn cmd(b:&mut WaveshareLcd35,c:u8,a:&[u8]){route(b,false);b.spi_transfer_qspi_pins(2,packet(1),&[c],0);route(b,true);b.spi_transfer_qspi_pins(2,packet(a.len()),a,0);}
 #[test]fn no_mcu_cs_st7796_and_ft6336_are_distinct_from_axs35b(){let mut b=WaveshareLcd35::new();let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((8,7))));assert!(ds.iter().all(|(_,a,_)|*a!=0x3b&&*a!=0x40));let ex=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();write(ex,3,0xfd);write(ex,1,2);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x36,&[0x48]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);let n=b.panel.pixels_written;let mut p=packet(2);p.data[0]=1<<2;b.spi_transfer_qspi_pins(2,p,&[0,0],0);assert_eq!(b.panel.pixels_written,n);b.touch(123,345,true);let tp=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert!(tp.start(false));tp.write(2);assert!(tp.start(true));assert_eq!((0..5).map(|_|tp.read()).collect::<Vec<_>>(),[1,0x80,123,1,89]);assert!(b.button_at(1,"power",true));let ex=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();assert!(ex.start(false));ex.write(0);ex.start(true);assert_ne!(ex.read()&64,0);write(ex,1,0);b.advance_to(2);assert!(b.display().unwrap().2.iter().all(|v|*v==0));b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([11,10,9,255,255,255],true)));}
}
