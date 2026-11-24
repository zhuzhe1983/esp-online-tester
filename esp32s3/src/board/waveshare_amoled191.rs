//! Plain/touch AMOLED1.91. RM67162 V0.5 pp84/89/98 and exact factory BSP.
//! The manufacturer calls its driver SH8601, but linked panel data sheet is
//! RM67162 and its MADCTR F0/one-byte brightness differ from SH8601. This is
//! explicitly that evidenced 536x240 RGB565 BSP subset, not a fabricated die ID.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,ft3168::{Ft3168,Ft3168State},i2c_bitbang::I2cBitBang,imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};
const HZ:u64=240_000_000;
struct Panel { memory:DcsPanel,madctl:u8,brightness:u8,header:[u8;4],at:usize,command:u8,parameters:usize,args:[u8;4],window:[u16;4],bad:bool,normal:bool,version:u64,rejected:u64 }
impl Panel {
 fn new()->Self{let mut memory=DcsPanel::new(536,240);memory.colmod=0x77;Self{memory,madctl:0,brightness:255,header:[0;4],at:0,command:0,parameters:0,args:[0;4],window:[0,535,0,239],bad:false,normal:false,version:0,rejected:0}}
 fn reset(&mut self){let(v,f,p,r)=(self.version,self.memory.frames,self.memory.pixels_written,self.rejected);*self=Self::new();self.version=v.wrapping_add(1);self.memory.frames=f;self.memory.pixels_written=p;self.rejected=r;}
 fn deselect(&mut self){self.at=0;self.parameters=0;self.bad=false;self.normal=false;}
 fn reject(&mut self){self.bad=true;self.rejected+=1;}
 fn orientation(&self)->bool{self.madctl&0x20!=0&&self.madctl&7==0}
 fn phase(&mut self,mut b:&[u8],width:u8)->bool{
  if self.at<4{if width!=1{return false;}let n=(4-self.at).min(b.len());self.header[self.at..self.at+n].copy_from_slice(&b[..n]);self.at+=n;b=&b[n..];if self.at==4{let[op,z,c,last]=self.header;if !matches!(op,2|0x32)||z!=0||last!=0||op==0x32&&!matches!(c,0x2c|0x3c){return false;}self.command=c;self.normal=matches!(c,1|0x10|0x11|0x20|0x21|0x28|0x29|0x2a|0x2b|0x2c|0x3c|0x36|0x3a|0x51);
   if self.normal{if c==1{let h=self.header;self.reset();self.header=h;self.at=4;self.command=c;self.normal=true;}else{self.memory.dc=false;self.memory.byte(c);self.memory.dc=true;self.version+=1;}}}}
  if b.is_empty(){return true;}
 if self.at!=4||width!=if self.header[0]==0x32{4}else{1}{return false;}
 if !self.normal{self.parameters+=b.len();return true;}
  if matches!(self.command,0x2c|0x3c){let[x0,x1,y0,y1]=self.window;if self.header[0]!=0x32||self.memory.colmod&7!=5||!self.orientation()||x0>x1||y0>y1||x1>=536||y1>=240{return false;}}
  else{let n=match self.command{0x2a|0x2b=>4,0x36|0x3a|0x51=>1,_=>0};if self.parameters+b.len()>n{return false;}for(i,&v)in b.iter().enumerate(){self.args[self.parameters+i]=v;}match self.command{0x36=>self.madctl=b[0],0x51=>self.brightness=b[0],0x2a|0x2b if self.parameters+b.len()==4=>{let i=if self.command==0x2a{0}else{2};self.window[i]=u16::from_be_bytes([self.args[0],self.args[1]]);self.window[i+1]=u16::from_be_bytes([self.args[2],self.args[3]]);},_=>{}}}
  // RM67162 MV has the inverse sense of the common DCS walker. Rectangular
  // exchanged axes and RSMX/RSMY are deliberately outside this bounded subset.
  for &v in b{self.memory.byte(if self.command==0x36{(v&0xc8)|(!v&0x20)}else{v});}self.parameters+=b.len();self.version+=1;true
 }
 fn transfer(&mut self,p:SpiQspiPins,b:&[u8])->bool{if self.bad{return false;}let bits=u64::from(p.command_bits)+u64::from(p.address_bits)+u64::from(p.data_bits);if p.unsupported_mode||p.sio||p.full_duplex||p.dummy_cycles!=0||p.clock_idle_high||p.clock_output_edge||p.read_bits!=0||p.read_width!=0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)||!p.data_bits.is_multiple_of(8)||bits!=b.len()as u64*8||(p.command_bits==0)!=(p.command_width==0)||(p.address_bits==0)!=(p.address_width==0)||(p.data_bits==0)!=(p.data_width==0){self.reject();return false;}let mut at=0;for(n,w)in[(usize::from(p.command_bits)/8,p.command_width),(usize::from(p.address_bits)/8,p.address_width),(p.data_bits as usize/8,p.data_width)]{if n>0{if !self.phase(&b[at..at+n],w){self.reject();return false;}at+=n;}}true}
 fn frame(&self)->Vec<u16>{let mut out=vec![0;536*240];if !self.memory.on||self.memory.sleeping||self.memory.colmod&7!=5||!self.orientation(){return out;}let d=u32::from(self.brightness);for y in 0..240{for x in 0..536{let mut p=self.memory.gram[(239-y)*536+535-x];if self.madctl&8!=0{p=(p&0x7e0)|(p>>11)|((p&31)<<11);}
 if self.memory.inverted{p^=0xffff;}let v=u32::from(p);out[y*536+x]=((((v>>11)*d/255)<<11)|((((v>>5)&63)*d/255)<<5)|((v&31)*d/255))as u16;}}out}
}
struct FtNormal{inner:Ft3168,first:bool,normal:bool,written:bool}
impl I2cDevice for FtNormal {
 fn start(&mut self,r:bool)->bool{if !r{self.first=true;self.normal=false;self.written=false;}self.inner.start(r)}
 fn write(&mut self,v:u8)->bool{if self.first{self.first=false;self.normal=v==0;return self.inner.write(v);}
 if self.normal&&!self.written&&v==0{self.written=true;return true;}self.inner.write(v)}
 fn read(&mut self)->u8{self.inner.read()}
 fn stop(&mut self){self.inner.stop();}
}
pub struct WaveshareAmoled191 { touch_variant:bool,sd_v2:bool,panel:Panel,reset:bool,selected:bool,cycle:u64,ft:Arc<Mutex<Ft3168State>>,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,i2c:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,card_present:bool,card:Option<SpiSdCard>,sd_rejected:u64 }
impl WaveshareAmoled191 {
 pub fn new(touch_variant:bool,sd_v2:bool)->Self{let motion=Arc::new(ImuMotion::default());Self{touch_variant,sd_v2,panel:Panel::new(),reset:false,selected:false,cycle:0,ft:Arc::new(Mutex::new(Ft3168State::new(HZ))),qmi:Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(),HZ))),motion,i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:vec![],card_present:false,card:None,sd_rejected:0}}
 fn irq(&mut self){if self.touch_variant{let high=self.ft.lock().expect("AM191 FT3168").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:41,level:high});}}}
}
impl BoardModel for WaveshareAmoled191 {
 fn name(&self)->&'static str{match(self.touch_variant,self.sd_v2){(false,false)=>"waveshare-s3-amoled191-v1",(false,true)=>"waveshare-s3-amoled191-v2",(true,false)=>"waveshare-s3-touch-amoled191-v1",(true,true)=>"waveshare-s3-touch-amoled191-v2"}}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let reset=r.software_output(17)==Some(true);if self.reset&&!reset{self.panel.reset();}self.reset=reset;self.ft.lock().expect("AM191 FT3168").hardware_at(c,self.touch_variant,reset);let selected=r.software_output(6)==Some(false);if self.selected&&!selected{self.panel.deselect();}self.selected=selected;if r.software_output(9)==Some(true){if let Some(card)=&mut self.card{card.deselect();}}
  let scl=r.software_output(39).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(40).unwrap_or(true));for(i,(pin,level))in[(40,sda),(39,scl)].into_iter().enumerate(){if level!=self.levels[i]{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}self.irq();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{let lcd=p.cs&(1<<6)!=0;let card=p.cs&(1<<9)!=0&&!self.sd_v2;
  if lcd&&card{self.panel.reject();self.sd_rejected+=1;return vec![255;rx];}
  if lcd{let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let wired=p.sclk&(1<<47)!=0&&p.data[0]&(1<<18)!=0&&(!quad||[7,48,5].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));if !wired||!self.reset||rx!=0{self.panel.reject();}else{self.panel.transfer(p,tx);}
 if !p.keep_cs&&p.software_cs&(1<<6)==0{self.panel.deselect();}return vec![255;rx];}
  if !self.sd_v2&&(card||p.sclk&(1<<47)!=0){let serial=p.sclk&(1<<47)!=0&&p.data[0]&(1<<42)!=0&&!p.unsupported_mode&&!p.sio&&!p.clock_idle_high&&!p.clock_output_edge&&p.dummy_cycles==0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|w|*w<=1);if !serial||!self.card_present{self.sd_rejected+=u64::from(card);return vec![255;rx];}
 if let Some(sd)=&mut self.card{let reply=if p.full_duplex||tx.is_empty()||rx==0{sd.transfer(card,tx,rx)}else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx)};if !p.keep_cs&&p.software_cs&(1<<9)==0{sd.deselect();}return if p.input[1]==Some(8){reply}else{vec![255;rx]};}}
  vec![255;rx]
 }
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));if self.touch_variant{self.i2c.attach(0x38,Box::new(FtNormal{inner:Ft3168::new(self.ft.clone()),first:true,normal:false,written:false}));}self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((40,39))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.motion.cycle.store(c,Ordering::Relaxed);self.ft.lock().expect("AM191 FT3168").advance_to(c);self.irq();}
 fn next_deadline(&self)->Option<u64>{self.ft.lock().expect("AM191 FT3168").next_deadline()}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut v=vec![(0,true),(40,self.levels[0]),(39,self.levels[1])];if self.touch_variant{v.push((41,self.levels[2]));}v}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 // The factory maps physical FT X to screenY (240-y), physicalY to screenX.
 fn touch(&mut self,x:u16,y:u16,down:bool){if self.touch_variant{self.ft.lock().expect("AM191 FT3168").touch_at(self.cycle,240-y.min(239),x.min(535),down);self.irq();}}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{self.sd_v2.then_some(([9,42,8,255,255,255],self.card_present))}
 fn attach_spi_sd_card(&mut self,s:Box<dyn SdStore>,c:SdCardConfig)->Result<String,String>{if self.sd_v2{return Err("V2 is physical nativeSDMMC CLK9/CMD42/D08".into());}let card=SpiSdCard::new(s,c)?;let desc=card.describe();self.card=Some(card);self.card_present=true;Ok(desc)}
 fn set_sd_card_present(&mut self,p:bool){if p!=self.card_present{if let Some(c)=&mut self.card{c.power_cycle();}}self.card_present=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((536,240,if self.reset{self.panel.frame()}else{vec![0;536*240]},self.panel.version.wrapping_add(self.panel.memory.pixels_written)))}
 fn display_version(&self)->u64{self.panel.version.wrapping_add(self.panel.memory.pixels_written)}
 fn display_frames(&self)->u64{self.panel.memory.frames}
 fn report(&self)->String{format!("{} RM67162-linked/SH8601-named originalBSP536x240 QSPI47/[18,7,48,5]/CS6 RST17(sharedTP); MADCTR{:02x} relativeF0, bits2/1/0 and exchangedXY unsupported; brightness{} onebyte; FT3168={} I2C40/39 IRQ41; QMI8658C@6B; nativeSD9/42/8={} SPI47/42/8/CS9={}. pixels={} frames={} reject={} SDreject={}. Unknown die identity/readback, exact vendor silicon revision, OTP, unobserved analog/gamma/scan, battery voltage and complete factory/hardware acceptance. Old schematicSPI and active VersionControl_V2 nativeSD remain distinct profiles, no combined invented wiring.",self.name(),self.panel.madctl,self.panel.brightness,self.touch_variant,self.sd_v2,!self.sd_v2,self.panel.memory.pixels_written,self.panel.memory.frames,self.panel.rejected,self.sd_rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareAmoled191,reset:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();g.enable=1<<17;if reset{g.out=1<<17;}m.write(4+4*17,1<<12);b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins(n:usize,w:u8)->SpiQspiPins{SpiQspiPins{sclk:1<<47,data:[1<<18,1<<7,1<<48,1<<5],cs:1<<6,data_width:w,data_bits:n as u32*8,keep_cs:true,..Default::default()}}
 fn cmd(b:&mut WaveshareAmoled191,c:u8,a:&[u8]){b.spi_transfer_qspi_pins(2,pins(4,1),&[if c==0x2c{0x32}else{2},0,c,0],0);let mut p=pins(a.len(),if c==0x2c{4}else{1});p.keep_cs=false;b.spi_transfer_qspi_pins(2,p,a,0);}
 #[test]fn rm_specific_mv_brightness_window_and_wrong_wires(){let mut b=WaveshareAmoled191::new(false,true);route(&mut b,true);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0xf0]);cmd(&mut b,0x2a,&[0,13,0,14]);cmd(&mut b,0x2b,&[0,7,0,7]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);assert_eq!(&b.display().unwrap().2[7*536+13..7*536+15],&[0xf800,0x07e0]);cmd(&mut b,0x51,&[0]);assert!(b.display().unwrap().2.iter().all(|v|*v==0));cmd(&mut b,0x51,&[255]);let n=b.panel.memory.pixels_written;cmd(&mut b,0x36,&[0xc0]);cmd(&mut b,0x2c,&[0,0]);assert_eq!(b.panel.memory.pixels_written,n);cmd(&mut b,0x36,&[0xf0]);let mut p=pins(4,1);p.sclk=1<<40;p.keep_cs=false;b.spi_transfer_qspi_pins(2,p,&[2,0,0x10,0],0);assert!(!b.panel.memory.sleeping);route(&mut b,false);assert!(b.display().unwrap().2.iter().all(|v|*v==0));assert_eq!(b.panel.brightness,255);}
 #[test]fn exact_touch_mapping_reset_and_absent_non_touch(){let mut b=WaveshareAmoled191::new(true,true);route(&mut b,true);b.advance_to(HZ/10);let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x38).unwrap().2.as_mut();assert_eq!(d.pins(),Some((40,39)));b.touch(321,110,true);assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));let p=(0..5).map(|_|d.read()).collect::<Vec<_>>();assert_eq!(p,[1,0,130,1,65]);d.start(false);assert!(!d.write(0xa7));route(&mut b,false);assert!(!d.start(false));let mut n=WaveshareAmoled191::new(false,true);assert!(n.i2c_devices().iter().all(|(_,a,_)|*a!=0x38));}
 #[test]fn source_v1_spi_and_v2_native_sd_are_mutually_exclusive(){let mut v2=WaveshareAmoled191::new(false,true);v2.set_sd_card_present(true);assert_eq!(v2.sd_card_slot(),Some(([9,42,8,255,255,255],true)));assert!(v2.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).is_err());let mut b=WaveshareAmoled191::new(false,false);assert!(b.sd_card_slot().is_none());b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();let mut p=SpiQspiPins{sclk:1<<47,data:[1<<42,0,0,0],input:[None,Some(8),None,None],data_width:1,data_bits:80,..Default::default()};b.spi_transfer_qspi_pins(3,p,&[255;10],0);p.cs=1<<9;p.data_bits=48;p.read_bits=64;p.read_width=1;assert!(b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8).contains(&1));p.input[1]=Some(16);assert_eq!(b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8),[255;8]);p.cs|=1<<6;assert_eq!(b.spi_transfer_qspi_pins(3,p,&[0x40,0,0,0,0,0x95],8),[255;8]);}
}
