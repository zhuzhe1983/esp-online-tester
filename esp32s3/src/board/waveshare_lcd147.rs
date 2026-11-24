//! Two distinct 1.47-inch S3 PCBs, from their own official demo archives.
//! LCD1.47B: ST7789T, physical RAM window x34..205. Touch1.47: JD9853,
//! factory main.c explicitly applies set_gap(34,0) for default portrait,
//! beyond bsp_display.c initialization. Both use RAM columns34..205;
//! JD9853 remains a common-DCS subset without ST7789-only RAMCTRL behavior.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,Ws2812Chain,i2c_bitbang::I2cBitBang,axs5106::{Axs5106,Axs5106State},imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};
const HZ:u64=240_000_000;
struct Wiring{name:&'static str,touch:bool,clock:u8,mosi:u8,cs:u8,dc:u8,reset:u8,i2c:(u8,u8),sd:[u8;6]}
static CONFIGS:&[Wiring]=&[
 Wiring{name:"waveshare-s3-lcd147b",touch:false,clock:40,mosi:45,cs:42,dc:41,reset:39,i2c:(48,47),sd:[14,15,16,18,17,21]},
 Wiring{name:"waveshare-s3-touch-lcd147",touch:true,clock:38,mosi:39,cs:21,dc:45,reset:40,i2c:(42,41),sd:[16,15,17,18,13,14]},
];
pub fn make_board(name:&str)->Option<Board>{CONFIGS.iter().find(|w|w.name==name).map(|w|Box::new(WaveshareLcd147::new(w))as Board)}
struct WaveshareLcd147{w:&'static Wiring,panel:DcsPanel,command:u8,dc:Option<bool>,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,version:u64,cycle:u64,rejected:u64,card:bool,i2c:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,touch:Arc<Mutex<Axs5106State>>,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,led:Ws2812Chain}
impl WaveshareLcd147{
 fn new(w:&'static Wiring)->Self{let motion=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new(motion.clone(),HZ)));Self{w,panel:if w.touch{DcsPanel::new(240,320)}else{DcsPanel::st7789()},command:0,dc:None,reset:false,light:0,gpio_light:0,pwm:None,version:0,cycle:0,rejected:0,card:false,i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:Vec::new(),touch:Default::default(),motion,qmi,led:Ws2812Chain::new(1)}}
 fn sync(&mut self){let light=self.pwm.unwrap_or(self.gpio_light);if self.light!=light{self.light=light;self.version+=1;}
 if self.w.touch{let high=self.touch.lock().expect("AXS5106").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:47,level:high});}}}
}
impl BoardModel for WaveshareLcd147{
 fn name(&self)->&'static str{self.w.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let reset=r.software_output(self.w.reset)==Some(true);if self.reset&&!reset{self.panel.reset();self.command=0;}
 if self.reset!=reset{self.version+=1;}self.reset=reset;self.dc=r.software_output(self.w.dc);self.gpio_light=if r.software_output(46)==Some(true){65535}else{0};if self.w.touch{self.touch.lock().expect("AXS5106").set_reset(r.software_output(48)==Some(true));}let(sda,scl)=self.w.i2c;let clock=r.software_output(scl).unwrap_or(true);let data=self.i2c.drive(clock,r.software_output(sda).unwrap_or(true));for(i,(pin,level))in[(sda,data),(scl,clock)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.advance_to(c);self.pwm=f(46).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<self.w.cs)==0{return vec![255;rx];}let valid=self.reset&&self.dc.is_some()&&rx==0&&!p.full_duplex&&!p.sio&&!p.unsupported_mode&&!p.clock_idle_high&&!p.clock_output_edge&&p.dummy_cycles==0&&p.address_bits==0&&p.sclk&(1<<self.w.clock)!=0&&p.data[0]&(1<<self.w.mosi)!=0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|v|*v<=1)&&p.command_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8);if !valid{self.rejected+=1;return vec![255;rx];}self.panel.dc=self.dc==Some(true);if self.panel.dc&&matches!(self.command,0x2c|0x3c)&&self.panel.colmod&7!=5{self.rejected+=1;return vec![255;rx];}for &v in tx{if !self.panel.dc{self.command=v;}self.panel.byte(v);}self.version+=1;vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));if self.w.touch{self.i2c.attach(0x63,Box::new(Axs5106::new(self.touch.clone())));}self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some(self.w.i2c)).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.motion.cycle.store(c,Ordering::Relaxed);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut v=vec![(0,true),(self.w.i2c.0,self.levels[0]),(self.w.i2c.1,self.levels[1])];if self.w.touch{v.push((47,self.levels[2]));}v}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn touch(&mut self,x:u16,y:u16,down:bool){if self.w.touch{self.touch.lock().expect("AXS5106").update(171-x.min(171),y.min(319),down);self.sync();}}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some((self.w.sd,self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn rmt_frame(&mut self,p:u8,b:&[bool]){if !self.w.touch&&p==38{self.led.from_bits(b);}}
 fn leds(&self)->Option<(&[[u8;3]],u64)>{(!self.w.touch).then_some((&self.led.leds,self.led.updates))}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let mut frame=vec![0;172*320];if self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping{for y in 0..320{for x in 0..172{let col=if self.w.touch{x+34}else{239-(x+34)};let mut p=self.panel.gram[y*self.panel.cols+col];if self.panel.madctl&8!=0{p=(p&0x7e0)|(p>>11)|((p&31)<<11);}let d=self.light;let p=u32::from(p);frame[y*172+x]=((((p>>11)*d/65535)<<11)|((((p>>5)&63)*d/65535)<<5)|((p&31)*d/65535))as u16;}}}Some((172,320,frame,self.display_version()))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("{} {}172x320 CLK{}/MOSI{}/CS{}/DC{}/RST{} BL46high; I2C{:?} QMI8658@6B digital sample subset, AXS5106L@63={}, SDMMC4bit{:?}; {}pixels rejected={}. ST7789B xgap34/referenceMX follows exact factory; JD9853 xgap34 from factory main.c set_gap call, touch mirroredX follows BSP flags. No analog battery/charge/RF, physical QMI suffix/calibration, TE/gamma/vendor bank behavior, touch gestures/multitouch/scan cadence or complete hardware acceptance.",self.w.name,if self.w.touch{"JD9853"}else{"ST7789T"},self.w.clock,self.w.mosi,self.w.cs,self.w.dc,self.w.reset,self.w.i2c,self.w.touch,self.w.sd,self.panel.pixels_written,self.rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareLcd147,dc:bool,on:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(pin,v)in[(b.w.reset,on),(46,on),(b.w.dc,dc),(48,on)]{if pin==48&&!b.w.touch{continue;}g.enable|=1<<pin;if v{g.out|=1<<pin;}m.write(4+4*u32::from(pin),1<<12);}b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));b.gpio_pwm_at(b.cycle,&|_|None);}
 fn pins(b:&WaveshareLcd147,n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<b.w.clock,data:[1<<b.w.mosi,0,0,0],cs:1<<b.w.cs,data_width:1,data_bits:n as u32*8,..Default::default()}}
 fn cmd(b:&mut WaveshareLcd147,c:u8,a:&[u8]){route(b,false,true);b.spi_transfer_qspi_pins(2,pins(b,1),&[c],0);route(b,true,true);b.spi_transfer_qspi_pins(2,pins(b,a.len()),a,0);}
 #[test]fn distinct_dies_windows_routes_touch_and_sd(){for w in CONFIGS{let mut b=WaveshareLcd147::new(w);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x36,&[if w.touch{0}else{0x40}]);cmd(&mut b,0x3a,&[5]);let x=34;cmd(&mut b,0x2a,&[0,x,0,x]);cmd(&mut b,0x2b,&[0,5,0,5]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[5*172],0xf800);cmd(&mut b,0x2a,&[0,0,0,0]);cmd(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.display().unwrap().2[5*172],0xf800,"RAM column0 is outside the published x34..205 window");let n=b.panel.pixels_written;let mut p=pins(&b,2);p.data[0]=1<<37;b.spi_transfer_qspi_pins(2,p,&[0,0],0);assert_eq!(b.panel.pixels_written,n);cmd(&mut b,0xb0,&[0,0xe8]);assert_eq!(b.panel.little_endian,!w.touch);let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,_,d)|d.pins()==Some(w.i2c)));if w.touch{b.touch(30,200,true);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x63).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(1));assert!(d.start(true));assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(),[0,1,0,141,0,200]);}else{assert!(ds.iter().all(|(_,a,_)|*a!=0x63));let bits=(0..24).rev().map(|shift|(0x123456_u32>>shift)&1!=0).collect::<Vec<_>>();b.rmt_frame(37,&bits);assert_eq!(b.leds().unwrap().0[0],[0,0,0]);b.rmt_frame(38,&bits);assert_eq!(b.leds().unwrap().0[0],[0x34,0x12,0x56]);}b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some((w.sd,true)));route(&mut b,true,false);assert!(b.display().unwrap().2.iter().all(|p|*p==0));}}
}
