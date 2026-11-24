//! Plain1.46/B/C SPD2010 and plain1.85 ST77916 share the documented TCA
//! reset/SD outputs, but have different touch controllers and physical buses.
//! Ninth source manifest retains exact factory sources, schematics and revisions.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,i2c_bitbang::I2cBitBang,tca9554::{Tca9554,Tca9554State},pcf85063::{Pcf85063,Pcf85063State},imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};
const HZ:u64=240_000_000;
struct RoundPanel{memory:DcsPanel,spd:bool,page:u8,header:[u8;4],at:usize,command:u8,parameters:usize,args:[u8;4],normal:bool,bad:bool,version:u64,rejected:u64}
impl RoundPanel{
 fn new(spd:bool)->Self{let w=if spd{412}else{360};Self{memory:DcsPanel::new(w,w),spd,page:0,header:[0;4],at:0,command:0,parameters:0,args:[0;4],normal:false,bad:false,version:0,rejected:0}}
 fn reset(&mut self){let(f,p,v,r)=(self.memory.frames,self.memory.pixels_written,self.version,self.rejected);*self=Self::new(self.spd);self.memory.frames=f;self.memory.pixels_written=p;self.version=v.wrapping_add(1);self.rejected=r;}
 fn deselect(&mut self){self.at=0;self.parameters=0;self.normal=false;self.bad=false;}
 fn reject(&mut self){self.bad=true;self.rejected+=1;}
 fn phase(&mut self,mut b:&[u8],width:u8)->bool{if self.at<4{if width!=1{return false;}let n=(4-self.at).min(b.len());self.header[self.at..self.at+n].copy_from_slice(&b[..n]);self.at+=n;b=&b[n..];if self.at==4{let[op,z,c,last]=self.header;if !matches!(op,2|0x32)||z!=0||last!=0||op==0x32&&!matches!(c,0x2c|0x3c){return false;}self.command=c;self.normal=self.page==0&&matches!(c,1|0x10|0x11|0x20|0x21|0x28|0x29|0x2a|0x2b|0x2c|0x3c|0x36|0x3a);if self.normal{if c==1{let h=self.header;self.reset();self.header=h;self.at=4;self.command=c;self.normal=true;}else{self.memory.dc=false;self.memory.byte(c);self.memory.dc=true;self.version+=1;}}}}
  if b.is_empty(){return true;}
 if self.at!=4||width!=if self.header[0]==0x32{4}else{1}{return false;}
  if self.spd&&self.command==0xff{if self.parameters+b.len()>3{return false;}for(i,&v)in b.iter().enumerate(){self.args[self.parameters+i]=v;}self.parameters+=b.len();if self.parameters==3&&self.args[..2]==[0x20,0x10]{self.page=self.args[2];}return true;}
  if !self.spd&&self.command==0xf0{if self.parameters+b.len()>1{return false;}self.page=b[0];self.parameters+=b.len();return true;}
  if !self.normal{self.parameters+=b.len();return true;}
  if matches!(self.command,0x2c|0x3c){if self.header[0]!=0x32||self.memory.colmod&7!=5{return false;}}
  else{let needed=match self.command{0x2a|0x2b=>4,0x36|0x3a=>1,_=>1};if self.parameters+b.len()>needed{return false;}}
  for &v in b{self.memory.byte(if self.spd&&self.command==0x36{(v&8)|((v&2)<<5)|((v&1)<<7)}else{v});}self.parameters+=b.len();self.version+=1;true
 }
 fn transfer(&mut self,p:SpiQspiPins,b:&[u8])->bool{if self.bad{return false;}let bits=u64::from(p.command_bits)+u64::from(p.address_bits)+u64::from(p.data_bits);if p.unsupported_mode||p.sio||p.full_duplex||p.dummy_cycles!=0||p.clock_idle_high||p.clock_output_edge||p.read_bits!=0||p.read_width!=0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)||!p.data_bits.is_multiple_of(8)||bits!=b.len()as u64*8||(p.command_bits==0)!=(p.command_width==0)||(p.address_bits==0)!=(p.address_width==0)||(p.data_bits==0)!=(p.data_width==0){self.reject();return false;}let mut at=0;for(n,w)in[(usize::from(p.command_bits)/8,p.command_width),(usize::from(p.address_bits)/8,p.address_width),(p.data_bits as usize/8,p.data_width)]{if n>0{if !self.phase(&b[at..at+n],w){self.reject();return false;}at+=n;}}true}
}
#[derive(Default)]struct SpdState{active:bool,generation:u64,phase:u8,point_mode:bool,x:u16,y:u16,down:bool,pending:bool,reports:u64}
impl SpdState{fn hardware(&mut self,on:bool,reset:bool){if on!=self.active||reset{let gen=self.generation.wrapping_add(1);let reports=self.reports;*self=Self{active:on,generation:gen,reports,..Default::default()};}}}
struct SpdTouch{state:Arc<Mutex<SpdState>>,generation:u64,register:u16,count:usize,value:u16,read_at:usize,report:[u8;18],valid:bool}
impl SpdTouch{
 fn new(state:Arc<Mutex<SpdState>>)->Self{Self{state,generation:0,register:0,count:0,value:0,read_at:0,report:[0;18],valid:false}}
 fn sync(&mut self)->bool{let s=self.state.lock().expect("SPD2010 touch");if self.generation!=s.generation{self.generation=s.generation;self.valid=false;self.count=0;}s.active}
}
impl I2cDevice for SpdTouch{
 fn start(&mut self,read:bool)->bool{if !self.sync(){return false;}
 if !read{self.count=0;self.register=0;self.value=0;self.valid=false;return true;}
 if !self.valid||!matches!(self.register,0x20|0x300|0x2fc){return false;}self.read_at=0;self.report=[0;18];let mut s=self.state.lock().expect("SPD2010 touch");match self.register{0x20=>{self.report[1]=match s.phase{0=>0x40,1=>0x20,_=>8};if s.phase==2&&s.down{self.report[0]=1;self.report[2]=10;}},0x300=>{if s.phase==2&&s.down{self.report[5]=s.x as u8;self.report[6]=s.y as u8;self.report[7]=(((s.x>>8)&15)<<4|(s.y>>8)&15)as u8;self.report[8]=1;s.reports+=1;}},0x2fc=>self.report[5]=0x82,_=>{}}true}
 fn write(&mut self,b:u8)->bool{if !self.sync(){return false;}match self.count{0=>self.register=u16::from(b),1=>{self.register|=u16::from(b)<<8;self.valid=matches!(self.register,2|4|0x46|0x50|0x20|0x300|0x2fc);if !self.valid{return false;}},2=>{if !matches!(self.register,2|4|0x46|0x50){return false;}self.value=u16::from(b);},3=>{self.value|=u16::from(b)<<8;let mut s=self.state.lock().expect("SPD2010 touch");match(self.register,self.value){(4,1)=>s.phase=1,(0x50,0)=>s.point_mode=true,(0x46,0)if s.phase==1&&s.point_mode=>s.phase=2,(2,1)=>s.pending=false,_=>return false,}},_=>return false,}self.count+=1;true}
 fn read(&mut self)->u8{if !self.sync(){return 255;}let v=self.report.get(self.read_at).copied().unwrap_or(255);self.read_at=self.read_at.saturating_add(1);v}
}
// Only measured point/sleep controls. A7/firmware/OTP are not guessed merely
// because the manufacturer's driver logs them; unsupported registers NACK.
struct CstPort{inner:Cst816s,first:bool,ptr:Option<u8>,auto_sleep:Option<u8>}
impl I2cDevice for CstPort{
 fn start(&mut self,r:bool)->bool{if !r{self.first=true;self.ptr=None;}self.inner.start(r)&&(!r||self.ptr.is_some())}
 fn write(&mut self,v:u8)->bool{if self.first{self.first=false;self.ptr=matches!(v,0..=6|0xa5|0xfe).then_some(v);return self.ptr.is_some()&&self.inner.write(v);}
 if self.ptr==Some(0xfe)&&v<=1{self.auto_sleep=Some(v);self.ptr=None;return true;}
 if self.ptr==Some(0xa5)&&v==3{self.ptr=None;return self.inner.write(v);}false}
 fn read(&mut self)->u8{if self.ptr==Some(0xfe){return self.auto_sleep.unwrap_or(255);}let v=self.inner.read();self.ptr=self.ptr.filter(|p|*p<6).map(|p|p+1);v}
 fn stop(&mut self){self.inner.stop();}
}
pub struct WaveshareRoundTca{spd:bool,touch_variant:bool,panel:RoundPanel,io:Arc<Mutex<Tca9554State>>,rtc:Arc<Mutex<Pcf85063State>>,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,spd_touch:Arc<Mutex<SpdState>>,cst:Arc<Mutex<Cst816State>>,main_i2c:I2cBitBang,touch_i2c:I2cBitBang,attached:bool,cycle:u64,reset:bool,falls:[u64;2],selected:bool,light:u32,gpio_light:u32,pwm:Option<u32>,levels:[bool;5],edges:Vec<BoardEdge>,card:bool}
impl WaveshareRoundTca{
 pub fn new(spd:bool,touch_variant:bool)->Self{let motion=Arc::new(ImuMotion::default());Self{spd,touch_variant,panel:RoundPanel::new(spd),io:Default::default(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),qmi:Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(),HZ))),motion,spd_touch:Default::default(),cst:Default::default(),main_i2c:I2cBitBang::new(),touch_i2c:I2cBitBang::new(),attached:false,cycle:0,reset:false,falls:[0;2],selected:false,light:0,gpio_light:0,pwm:None,levels:[true;5],edges:vec![],card:false}}
 fn sync(&mut self){let io=self.io.lock().expect("round TCA");let falls=[io.falling_edges(1),io.falling_edges(0)];let reset=io.high(1);let tp=io.high(0)&&self.touch_variant;drop(io);if falls[0]!=self.falls[0]||self.reset&&!reset{self.panel.reset();}
 if self.reset!=reset{self.panel.version+=1;}self.reset=reset;
  if self.spd{self.spd_touch.lock().expect("SPD2010 touch").hardware(tp,falls[1]!=self.falls[1]);}else{let mut s=self.cst.lock().expect("CST816");if s.powered!=tp||falls[1]!=self.falls[1]{s.reset();}s.powered=tp;}self.falls=falls;
  let light=self.pwm.unwrap_or(self.gpio_light);if light!=self.light{self.light=light;self.panel.version+=1;}
  let high=if !self.touch_variant{true}else if self.spd{let s=self.spd_touch.lock().expect("SPD2010 touch");!s.active||!s.pending}else{self.cst.lock().expect("CST816").irq_high()};if high!=self.levels[4]{self.levels[4]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:4,level:high});}
 }
}
impl BoardModel for WaveshareRoundTca{
 fn name(&self)->&'static str{if self.spd{"waveshare-s3-touch-lcd146"}else if self.touch_variant{"waveshare-s3-touch-lcd185"}else{"waveshare-s3-lcd185"}}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);self.gpio_light=if r.software_output(5)==Some(true){65535}else{0};let selected=r.software_output(21)==Some(false);if self.selected&&!selected{self.panel.deselect();}self.selected=selected;
  let scl=r.software_output(10).unwrap_or(true);let sda=self.main_i2c.drive(scl,r.software_output(11).unwrap_or(true));let tscl=r.software_output(3).unwrap_or(true);let tsda=if !self.spd&&self.touch_variant{self.touch_i2c.drive(tscl,r.software_output(1).unwrap_or(true))}else{true};for(i,(pin,level))in[(11,sda),(10,scl),(1,tsda),(3,tscl)].into_iter().enumerate(){if level!=self.levels[i]{self.levels[i]=level;if i<2||!self.spd&&self.touch_variant{self.edges.push(BoardEdge{cycle:c,pin,level});}}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.advance_to(c);self.pwm=f(5).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<21)==0{return vec![255;rx];}self.sync();let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let wired=p.sclk&(1<<40)!=0&&p.data[0]&(1<<46)!=0&&(!quad||[45,42,41].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));if !wired||!self.reset||rx!=0{self.panel.reject();}else{self.panel.transfer(p,b);}
 if !p.keep_cs&&p.software_cs&(1<<21)==0{self.panel.deselect();}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.main_i2c.attach(0x20,Box::new(Tca9554::new(self.io.clone())));self.main_i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.main_i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));if self.spd{self.main_i2c.attach(0x53,Box::new(SpdTouch::new(self.spd_touch.clone())));}else if self.touch_variant{self.touch_i2c.attach(0x15,Box::new(CstPort{inner:Cst816s::new(self.cst.clone()),first:true,ptr:None,auto_sleep:None}));}self.attached=true;}
  let mut v:Vec<_>=(0..2).flat_map(|bus|self.main_i2c.controller_devices(Some((11,10))).into_iter().map(move|(a,d)|(bus,a,d))).collect();if !self.spd&&self.touch_variant{for bus in 0..2{v.extend(self.touch_i2c.controller_devices(Some((1,3))).into_iter().map(|(a,d)|(bus,a,d)));}}v}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.motion.cycle.store(c,Ordering::Relaxed);self.rtc.lock().expect("round RTC").advance_to(c);self.sync();}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.sync();if !self.touch_variant{return;}let w=if self.spd{412}else{360};if self.spd{let mut s=self.spd_touch.lock().expect("SPD2010 touch");if s.active{s.x=x.min(w-1);s.y=y.min(w-1);s.down=down;s.pending=true;}}else{self.cst.lock().expect("CST816").update(x.min(w-1),y.min(w-1),down);}self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut v=vec![(0,true),(6,true),(11,self.levels[0]),(10,self.levels[1])];if self.touch_variant{v.push((4,self.levels[4]));if !self.spd{v.extend([(1,self.levels[2]),(3,self.levels[3])]);}}v}
 fn named_pin(&self,n:&str)->Option<u8>{match n{"boot"=>Some(0),"power"=>Some(6),_=>None}}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([14,17,16,255,255,255],self.card&&self.io.lock().expect("round TCA").high(2)))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let w=if self.spd{412}else{360};let mut f=vec![0;w*w];if self.reset&&self.light>0&&self.panel.memory.on&&!self.panel.memory.sleeping{for(p,&v)in f.iter_mut().zip(&self.panel.memory.gram){let v=if self.panel.memory.madctl&8!=0{(v&0x7e0)|(v>>11)|((v&31)<<11)}else{v};let v=u32::from(v);let d=self.light;*p=((((v>>11)*d/65535)<<11)|((((v>>5)&63)*d/65535)<<5)|((v&31)*d/65535))as u16;}}Some((w as u32,w as u32,f,self.display_version()))}
 fn display_version(&self)->u64{self.panel.version.wrapping_add(self.panel.memory.pixels_written)}
 fn display_frames(&self)->u64{self.panel.memory.frames}
 fn report(&self)->String{format!("{} {} QSPI40/[46,45,42,41]/CS21 EXIO2reset BL5high; page{} pixels={} frames={} reject={}. TCA20/PCF51/QMI6B SDA11/SCL10; {}. TF native1bit14/17/16 gated by actual EXIO3. BOOT0/PWR6low. No guessed touch firmware/OTP/ID, PCM5101/MIC acoustic engine, analog battery/charger/volume, RTC alarm/IMU IRQ or physical validation. Source1.85V2.1 IRQ GPIO13 populated-option mapping not implemented.",self.name(),if self.spd{"SPD2010 412x412"}else{"ST77916 360x360"},self.panel.page,self.panel.memory.pixels_written,self.panel.memory.frames,self.panel.rejected,if self.spd{"SPD2010@53 shared11/10 little-endian command/status/contact; identity26 unknown NACK"}else if self.touch_variant{"CST816@15 independent SDA1/SCL3 EXIO1reset IRQ4; A7 unknown NACK"}else{"Non-touch SKU: no CST device"})}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareRoundTca,light:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();g.enable=1<<5;if light{g.out=1<<5;}m.write(4+4*5,1<<12);b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins(n:usize,w:u8)->SpiQspiPins{SpiQspiPins{sclk:1<<40,data:[1<<46,1<<45,1<<42,1<<41],cs:1<<21,data_width:w,data_bits:n as u32*8,keep_cs:true,..Default::default()}}
 fn cmd(b:&mut WaveshareRoundTca,c:u8,a:&[u8]){b.spi_transfer_qspi_pins(2,pins(4,1),&[if c==0x2c{0x32}else{2},0,c,0],0);let mut p=pins(a.len(),if c==0x2c{4}else{1});p.keep_cs=false;b.spi_transfer_qspi_pins(2,p,a,0);}
 fn wr(d:&mut dyn I2cDevice,b:&[u8]){assert!(d.start(false));for &v in b{assert!(d.write(v));}d.stop();}
 #[test]fn manufacturer_pages_pixels_tca_reset_and_wrong_lane(){for spd in[false,true]{let mut b=WaveshareRoundTca::new(spd,true);route(&mut b,true);if spd{cmd(&mut b,0xff,&[0x20,0x10,0x11]);}else{cmd(&mut b,0xf0,&[0x10]);}cmd(&mut b,0x29,&[]);assert!(!b.panel.memory.on);if spd{cmd(&mut b,0xff,&[0x20,0x10,0]);}else{cmd(&mut b,0xf0,&[0]);}cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,3,0,4]);cmd(&mut b,0x2b,&[0,9,0,9]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);let w=if spd{412}else{360};assert_eq!(&b.display().unwrap().2[9*w+3..9*w+5],&[0xf800,0x07e0]);let n=b.panel.memory.pixels_written;let mut p=pins(4,1);p.data[0]=1<<38;p.keep_cs=false;b.spi_transfer_qspi_pins(2,p,&[2,0,0x10,0],0);assert_eq!(b.panel.memory.pixels_written,n);assert!(!b.panel.memory.sleeping);let mut io=Tca9554::new(b.io.clone());wr(&mut io,&[3,0]);wr(&mut io,&[1,0xfd]);b.advance_to(1);assert!(b.display().unwrap().2.iter().all(|v|*v==0));assert!(!b.reset);wr(&mut io,&[1,255]);b.advance_to(2);assert!(b.reset);assert!(!b.panel.memory.on);}}
 #[test]fn spd_protocol_has_real_boot_phases_and_contact_unknown_id_nacks(){let mut b=WaveshareRoundTca::new(true,true);route(&mut b,true);let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x53).unwrap().2.as_mut();assert_eq!(d.pins(),Some((11,10)));wr(d,&[0x20,0]);assert!(d.start(true));assert_eq!((0..4).map(|_|d.read()).collect::<Vec<_>>(),[0,0x40,0,0]);wr(d,&[4,0,1,0]);wr(d,&[0x50,0,0,0]);wr(d,&[0x46,0,0,0]);b.touch(300,210,true);wr(d,&[0x20,0]);assert!(d.start(true));assert_eq!((0..4).map(|_|d.read()).collect::<Vec<_>>(),[1,8,10,0]);wr(d,&[0,3]);assert!(d.start(true));let bytes=(0..10).map(|_|d.read()).collect::<Vec<_>>();assert_eq!(&bytes[4..9],&[0,44,210,0x10,1]);wr(d,&[2,0,1,0]);b.advance_to(1);assert!(b.levels[4]);assert!(d.start(false));assert!(d.write(0x26));assert!(!d.write(0));let mut io=Tca9554::new(b.io.clone());wr(&mut io,&[3,0]);wr(&mut io,&[1,254]);b.advance_to(2);assert!(!d.start(false));}
 #[test]fn separate_cst_bus_native_card_gate_and_non_touch_absence(){let mut b=WaveshareRoundTca::new(false,true);route(&mut b,true);let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x15).unwrap().2.as_mut();assert_eq!(d.pins(),Some((1,3)));b.touch(200,301,true);wr(d,&[2]);assert!(d.start(true));assert_eq!((0..5).map(|_|d.read()).collect::<Vec<_>>(),[1,0,200,1,45]);d.start(false);assert!(!d.write(0xa7));b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([14,17,16,255,255,255],true)));let mut io=Tca9554::new(b.io.clone());wr(&mut io,&[3,0]);wr(&mut io,&[1,0xfb]);assert!(!b.sd_card_slot().unwrap().1);let mut n=WaveshareRoundTca::new(false,false);assert!(n.i2c_devices().iter().all(|(_,a,_)|*a!=0x15));b.gpio_pwm_at(1,&|p|(p==5).then_some((5000.0,32768)));let l=b.light;b.gpio_pwm_at(2,&|p|(p==5).then_some((19000.0,32768)));assert_eq!((l,b.light),(32768,32768));}
}
