//! Exact older S3 SPI boards, not aliases of the later 2.8B/C RGB PCBs.
//! Pinned manufacturer BSP/schematics and version notices are retained under
//! docs/platform/research/waveshare-s3-tail-first-five-sources. Touch2.8 changed CST328
//! to CST3530 on 2026-06-05; both revisions retain independent board identities.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,
    devices::{DcsPanel,i2c_bitbang::I2cBitBang,imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port},
    pcf85063::{Pcf85063,Pcf85063State},waveshare_touch_parts::{Cst3530,Cst3530State}}};
const HZ:u64=240_000_000;
#[derive(Clone,Copy,PartialEq,Eq)]enum Touch{None,Cst328,Cst3530,Cst816t}
struct Wiring{name:&'static str,spi:[u8;3],dc:u8,reset:u8,light:u8,height:usize,gap:usize,
    touch:Touch,touch_pads:(u8,u8),touch_reset:u8,touch_irq:u8,power:u8,sd:Option<[u8;6]>}
const LCD28:Wiring=Wiring{name:"waveshare-s3-lcd28",spi:[40,45,42],dc:41,reset:39,light:5,height:320,gap:0,
    touch:Touch::None,touch_pads:(1,3),touch_reset:2,touch_irq:4,power:6,sd:Some([14,17,16,255,255,255])};
static PROFILES:&[Wiring]=&[LCD28,Wiring{name:"waveshare-s3-touch-lcd28-v1",touch:Touch::Cst328,..LCD28},
    Wiring{name:"waveshare-s3-touch-lcd28-v2",touch:Touch::Cst3530,..LCD28},
    Wiring{name:"waveshare-s3-touch-lcd169",spi:[6,7,5],dc:4,reset:8,light:15,height:280,gap:20,
        touch:Touch::Cst816t,touch_pads:(11,10),touch_reset:13,touch_irq:14,power:40,sd:None}];
pub fn make_board(name:&str)->Option<Board>{PROFILES.iter().find(|p|p.name==name).map(|p|Box::new(WaveshareTailSpi::new(p))as Board)}

// BSP-defined CST328 report subset. Unknown identity/checksum/gesture fields
// are never replaced with a magic value just to pass an original app probe.
#[derive(Default)]struct Cst328State{powered:bool,x:u16,y:u16,down:bool,pending:bool,sequence:u64,reset:u64}
struct Cst328{state:Arc<Mutex<Cst328State>>,ptr:u16,bytes:u8,snapshot:Option<u64>,reset:u64}
impl Cst328{fn new(state:Arc<Mutex<Cst328State>>)->Self{Self{state,ptr:0,bytes:0,snapshot:None,reset:0}}}
impl I2cDevice for Cst328{
 fn start(&mut self,read:bool)->bool{let s=self.state.lock().expect("CST328");if self.reset!=s.reset{self.reset=s.reset;self.bytes=0;self.snapshot=None;}
 if !s.powered{return false;}
 if read{if self.bytes!=2{return false;}self.snapshot=Some(s.sequence);}else{self.ptr=0;self.bytes=0;}true}
 fn write(&mut self,v:u8)->bool{if self.bytes<2{self.ptr=(self.ptr<<8)|u16::from(v);self.bytes+=1;return self.bytes<2||matches!(self.ptr,0xd000..=0xd006);}
  if self.ptr==0xd005&&v==0{let mut s=self.state.lock().expect("CST328");if self.snapshot==Some(s.sequence){s.pending=false;}self.snapshot=None;self.ptr+=1;true}else{false}}
 fn read(&mut self)->u8{let s=self.state.lock().expect("CST328");if !s.powered{return 255;}let value=match self.ptr{0xd000=>if s.down{6}else{0},0xd001=>(s.x>>4)as u8,0xd002=>(s.y>>4)as u8,0xd003=>((s.x&15)<<4|(s.y&15))as u8,0xd004=>0,0xd005=>u8::from(s.down),0xd006=>0,_=>255};self.ptr=self.ptr.wrapping_add(1);value}
}
struct WaveshareTailSpi{p:&'static Wiring,panel:DcsPanel,lcm:u8,command:u8,dc:Option<bool>,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,version:u64,cycle:u64,rejected:u64,last_rejected:Option<SpiQspiPins>,card:bool,
    buses:[I2cBitBang;2],attached:bool,levels:[bool;5],edges:Vec<BoardEdge>,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,rtc:Arc<Mutex<Pcf85063State>>,
    c328:Arc<Mutex<Cst328State>>,c3530:Arc<Mutex<Cst3530State>>,c816:Arc<Mutex<Cst816State>>}
impl WaveshareTailSpi{
 fn new(p:&'static Wiring)->Self{let motion=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new_c(motion.clone(),HZ)));Self{p,panel:DcsPanel::st7789(),lcm:0x2c,command:0,dc:None,reset:false,light:0,gpio_light:0,pwm:None,version:0,cycle:0,rejected:0,last_rejected:None,card:false,buses:[I2cBitBang::new(),I2cBitBang::new()],attached:false,levels:[true;5],edges:Vec::new(),motion,qmi,rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),c328:Default::default(),c3530:Default::default(),c816:Default::default()}}
 fn sync(&mut self){let light=self.pwm.unwrap_or(self.gpio_light);if self.light!=light{self.light=light;self.version+=1;}let irq=match self.p.touch{Touch::None=>return,Touch::Cst328=>{let s=self.c328.lock().expect("CST328");!s.powered||!s.pending},Touch::Cst3530=>self.c3530.lock().expect("CST3530").irq_high(),Touch::Cst816t=>self.c816.lock().expect("CST816T").irq_high()};if irq!=self.levels[4]{self.levels[4]=irq;self.edges.push(BoardEdge{cycle:self.cycle,pin:self.p.touch_irq,level:irq});}}
 fn touch_reset(&mut self,high:bool){match self.p.touch{Touch::None=>{},Touch::Cst328=>{let mut s=self.c328.lock().expect("CST328");if s.powered&&!high{s.down=false;s.pending=false;s.reset=s.reset.wrapping_add(1);}s.powered=high;},Touch::Cst3530=>self.c3530.lock().expect("CST3530").set_reset(high),Touch::Cst816t=>{let mut s=self.c816.lock().expect("CST816T");if s.powered&&!high{s.reset();}s.powered=high;}}}
}
impl BoardModel for WaveshareTailSpi{
 fn name(&self)->&'static str{self.p.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let reset=r.software_output(self.p.reset)==Some(true);if self.reset&&!reset{self.panel.reset();self.lcm=0x2c;self.command=0;}
 if self.reset!=reset{self.version+=1;}self.reset=reset;self.dc=r.software_output(self.p.dc);self.gpio_light=if r.software_output(self.p.light)==Some(true){65535}else{0};self.touch_reset(r.software_output(self.p.touch_reset)==Some(true));
  for (i,(sda,scl))in[(11,10),self.p.touch_pads].into_iter().enumerate(){if i==1&&(self.p.touch==Touch::None||self.p.touch_pads==(11,10)){continue;}let clock=r.software_output(scl).unwrap_or(true);let data=self.buses[i].drive(clock,r.software_output(sda).unwrap_or(true));for(j,(pin,level))in[(sda,data),(scl,clock)].into_iter().enumerate(){let k=2*i+j;if self.levels[k]!=level{self.levels[k]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.advance_to(c);self.pwm=f(self.p.light).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<self.p.spi[2])==0{return vec![255;rx];}let valid=self.reset&&self.dc.is_some()&&!p.sio&&!p.unsupported_mode&&!p.clock_output_edge&&p.dummy_cycles==0&&p.address_bits==0&&p.sclk&(1<<self.p.spi[0])!=0&&p.data[0]&(1<<self.p.spi[1])!=0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|v|*v<=1)&&p.command_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8)&&p.read_bits.is_multiple_of(8);if !valid{self.last_rejected=Some(p);self.rejected+=1;return vec![255;rx];}self.panel.dc=self.dc==Some(true);if self.panel.dc&&matches!(self.command,0x2c|0x3c)&&self.panel.colmod&7!=5{self.rejected+=1;return vec![255;rx];}for &v in tx{if !self.panel.dc{self.command=v;if v==1{self.lcm=0x2c;}}else if self.command==0xc0{self.lcm=v;}self.panel.byte(v);}self.version+=1;vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.buses[0].attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));self.buses[0].attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));let i=usize::from(self.p.touch_pads!=(11,10));match self.p.touch{Touch::None=>{},Touch::Cst328=>{self.buses[i].attach(0x1a,Box::new(Cst328::new(self.c328.clone())));},Touch::Cst3530=>{self.buses[i].attach(0x58,Box::new(Cst3530::new(self.c3530.clone())));},Touch::Cst816t=>{self.buses[i].attach(0x15,Box::new(Cst816s::new(self.c816.clone())));}}self.attached=true;}
  let mut out=Vec::new();for b in 0..2{for (a,d)in self.buses[0].controller_devices(Some((11,10))){out.push((b,a,d));}
 if self.p.touch_pads!=(11,10){for(a,d)in self.buses[1].controller_devices(Some(self.p.touch_pads)){out.push((b,a,d));}}}out}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.motion.cycle.store(c,Ordering::Relaxed);self.rtc.lock().expect("Tail RTC").advance_to(c);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut v=vec![(0,true),(self.p.power,true),(11,self.levels[0]),(10,self.levels[1])];if self.p.touch!=Touch::None{v.push((self.p.touch_irq,self.levels[4]));if self.p.touch_pads!=(11,10){v.extend([(self.p.touch_pads.0,self.levels[2]),(self.p.touch_pads.1,self.levels[3])]);}}v}
 fn named_pin(&self,n:&str)->Option<u8>{match n{"boot"=>Some(0),"power"=>Some(self.p.power),_=>None}}
 fn touch(&mut self,x:u16,y:u16,down:bool){let(x,y)=(x.min(239),y.min((self.p.height-1)as u16));match self.p.touch{Touch::None=>{},Touch::Cst328=>{let mut s=self.c328.lock().expect("CST328");if s.powered{s.x=x;s.y=y;s.down=down;s.pending=true;s.sequence=s.sequence.wrapping_add(1);}},Touch::Cst3530=>self.c3530.lock().expect("CST3530").update(x,y,down),Touch::Cst816t=>self.c816.lock().expect("CST816T").update(x,y,down)}self.sync();}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{self.p.sd.map(|p|(p,self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let mut frame=vec![0;240*self.p.height];if self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping{for y in 0..self.p.height{for x in 0..240{// C0h XMX/XMY alter output scan, separate from MADCTL RAM addressing.
// Native portrait reference follows the exact factory MADCTL=00 / LCMCTRL=2C.
let row=if self.lcm&0x40!=0{319-(y+self.p.gap)}else{y+self.p.gap};let col=if self.lcm&8==0{239-x}else{x};let mut p=self.panel.gram[row*240+col];if self.panel.madctl&8!=0{p=(p&0x7e0)|(p>>11)|((p&31)<<11);}let d=self.light;let p=u32::from(p);frame[y*240+x]=((((p>>11)*d/65535)<<11)|((((p>>5)&63)*d/65535)<<5)|((p&31)*d/65535))as u16;}}}Some((240,self.p.height as u32,frame,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("{}: ST7789 RGB565 240x{} gapY{} exactSPI{:?} DC{} RST{} BL{}; mainI2C11/10 QMI8658C@6B PCF85063@51; touch{}@{:?} reset{} IRQ{}; SD{:?}; pixels={} rejected={} madctl={:02x} lcm={:02x} colmod={:02x} reset={} on={} sleep={} light={} lastRejected={:?}. BOOT and PWR digital input only: no analog battery, charger, power-hold rail switching, buzzer/audio PCM, RF, physical touch/brightness calibration or full application acceptance. LCD transmit is processed while unwired readback floatsFF, including full-duplex host writes; rising-edge SPI modes0/3 only (Espressif CK_OUT_EDGE=0 for both). Portrait reference MADCTL00/LCMCTRL2C; LCM XMX/XMY output scan tracked relative to reset. LCM XMV/XINV/XBGR, analog gamma and physical glass orientation remain uncalibrated. Unknown touch IDs stay unknown; CST328 identity/debug commands are unsupported.",self.p.name,self.p.height,self.p.gap,self.p.spi,self.p.dc,self.p.reset,self.p.light,match self.p.touch{Touch::None=>"none",Touch::Cst328=>"CST328/1A",Touch::Cst3530=>"CST3530/58",Touch::Cst816t=>"CST816T/15 single-contact common protocol"},self.p.touch_pads,self.p.touch_reset,self.p.touch_irq,self.p.sd,self.panel.pixels_written,self.rejected,self.panel.madctl,self.lcm,self.panel.colmod,self.reset,self.panel.on,self.panel.sleeping,self.light,self.last_rejected)}
}

#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareTailSpi,dc:bool,on:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(pin,v)in[(b.p.reset,on),(b.p.light,on),(b.p.dc,dc),(b.p.touch_reset,on)]{g.enable|=1<<pin;if v{g.out|=1<<pin;}m.write(4+4*u32::from(pin),1<<12);}b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));b.gpio_pwm_at(b.cycle,&|_|None);}
 fn pins(b:&WaveshareTailSpi,n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<b.p.spi[0],data:[1<<b.p.spi[1],0,0,0],cs:1<<b.p.spi[2],data_width:1,data_bits:n as u32*8,..Default::default()}}
 fn cmd(b:&mut WaveshareTailSpi,c:u8,a:&[u8]){route(b,false,true);b.spi_transfer_qspi_pins(2,pins(b,1),&[c],0);route(b,true,true);b.spi_transfer_qspi_pins(2,pins(b,a.len()),a,0);}
 fn read(d:&mut dyn I2cDevice,reg:&[u8],n:usize)->Vec<u8>{assert!(d.start(false));for b in reg{assert!(d.write(*b));}assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
 #[test]fn exact_spi_corner_window_reset_backlight_and_route_rejection(){for p in PROFILES{let mut b=WaveshareTailSpi::new(p);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0]);cmd(&mut b,0x2a,&[0,0,0,0]);let y=p.gap as u16;cmd(&mut b,0x2b,&[(y>>8)as u8,y as u8,(y>>8)as u8,y as u8]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);let mut duplex=pins(&b,2);duplex.full_duplex=true;duplex.read_bits=16;duplex.read_width=1;assert_eq!(b.spi_transfer_qspi_pins(2,duplex,&[0xf8,0],2),[255,255]);let n=b.panel.pixels_written;let mut wrong=pins(&b,2);wrong.data_width=4;b.spi_transfer_qspi_pins(2,wrong,&[0,0],0);assert_eq!(n,b.panel.pixels_written);b.gpio_pwm_at(0,&|pin|(pin==p.light).then_some((1000.,0)));assert_eq!(b.display().unwrap().2[0],0);route(&mut b,true,false);assert!(!b.panel.on);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),p.sd.map(|pins|(pins,true)));}}
 #[test]fn spi_mode3_and_lcm_scan_are_independent_of_madctl_ram_addresses(){let mut b=WaveshareTailSpi::new(&PROFILES[0]);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x3a,&[5]);cmd(&mut b,0x36,&[0]);cmd(&mut b,0x2a,&[0,0,0,0]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);let mut mode3=pins(&b,2);mode3.clock_idle_high=true;assert_eq!(b.spi_transfer_qspi_pins(2,mode3,&[0x07,0xe0],0),Vec::<u8>::new());assert_eq!(b.display().unwrap().2[0],0x07e0);let n=b.panel.pixels_written;for idle in[false,true]{let mut wrong=mode3;wrong.clock_idle_high=idle;wrong.clock_output_edge=true;b.spi_transfer_qspi_pins(2,wrong,&[0,31],0);}assert_eq!(b.panel.pixels_written,n,"modes1/2 launch on the wrong edge");cmd(&mut b,0xc0,&[0x24]);assert_eq!(b.display().unwrap().2[239],0x07e0);cmd(&mut b,0xc0,&[0x6c]);assert_eq!(b.display().unwrap().2[319*240],0x07e0);cmd(&mut b,1,&[]);assert_eq!(b.lcm,0x2c);}
 #[test]fn exact_touch_revision_bus_report_release_and_reset(){for p in PROFILES{let mut b=WaveshareTailSpi::new(p);route(&mut b,true,true);let mut ds=b.i2c_devices();for(_,a,d)in &ds{assert_eq!(d.pins(),Some(if matches!(a,0x1a|0x58){(1,3)}else{(11,10)}));}let at=match p.touch{Touch::None=>{assert!(ds.iter().all(|(_,a,_)|matches!(a,0x6b|0x51)));continue},Touch::Cst328=>0x1a,Touch::Cst3530=>0x58,Touch::Cst816t=>0x15};let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==at).unwrap().2.as_mut();b.touch(123,234,true);assert!(b.input_levels().contains(&(p.touch_irq,false)));let a=match p.touch{Touch::Cst328=>read(d,&[0xd0,0],7),Touch::Cst3530=>read(d,&[0xd0,7,0,0],9),Touch::Cst816t=>read(d,&[2],5),Touch::None=>unreachable!()};match p.touch{Touch::Cst328=>assert_eq!(a,[6,7,14,0xba,0,1,0]),Touch::Cst3530=>assert_eq!(a,[0,0,0,1,123,234,0,0,0x20]),Touch::Cst816t=>assert_eq!(a,[1,0,123,0,234]),Touch::None=>unreachable!()}b.touch(123,234,false);route(&mut b,true,false);assert!(!d.start(false));}}
 #[test]fn cst328_unknown_id_rejected_and_late_ack_preserves_new_event(){let p=&PROFILES[1];let mut b=WaveshareTailSpi::new(p);route(&mut b,true,true);let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x1a).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(0xd2));assert!(!d.write(4));b.touch(1,2,true);assert_eq!(read(d,&[0xd0,5],1),[1]);b.touch(2,3,false);assert!(d.start(false));for v in[0xd0,5,0]{assert!(d.write(v));}assert!(b.c328.lock().unwrap().pending);assert_eq!(read(d,&[0xd0,5],1),[0]);assert!(d.start(false));for v in[0xd0,5,0]{assert!(d.write(v));}assert!(!b.c328.lock().unwrap().pending);}
}
