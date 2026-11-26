//! K162 ToughC5, M5GFX28d47949 esp32c5 generated wiring + pmic_ops.
//! ILI9342C CS25/DC26/MOSI7/MISO8/SCLK9; CHSC6540 2Eh on SDA2/SCL3.
//! PM1 IO2 touch reset; IOE1 IO4 LCD reset/IO5 rail/IO10 PWM4 backlight.
use esp_soc::{BoardModel,devices::DcsPanel,board::SpiQspiPins,pins::PinRoutes};
use esp_periph::i2c::{I2cDevice,m5::{M5pm1,M5pm1State,M5ioe1,M5ioe1State}};
use std::{sync::{Arc,Mutex},rc::Rc,cell::RefCell};
struct TouchState {x:u16,y:u16,down:bool}
struct Touch {s:Arc<Mutex<TouchState>>,pm:Arc<Mutex<M5pm1State>>,ptr:Option<u8>,first:bool,packet:[u8;5]}
impl I2cDevice for Touch {
 fn start(&mut self,read:bool)->bool {if self.pm.lock().unwrap().driven_high&4==0 {return false;}
  if !read {self.first=true;}else{let s=self.s.lock().unwrap();self.packet=[u8::from(s.down),(s.x>>8)as u8,s.x as u8,(s.y>>8)as u8,s.y as u8];}true}
 fn write(&mut self,v:u8)->bool {if self.first {self.first=false;self.ptr=matches!(v,2..=6|0x5a).then_some(v);return self.ptr.is_some();}self.ptr==Some(0x5a)&&v==0x5a}
 fn read(&mut self)->u8 {let Some(p)=self.ptr else{return 255;};self.ptr=p.checked_add(1);p.checked_sub(2).and_then(|i|self.packet.get(i as usize)).copied().unwrap_or(255)}
}
type Persistent=Rc<RefCell<Box<dyn I2cDevice>>>;
struct Routed(Persistent);
impl I2cDevice for Routed {
 fn pins(&self)->Option<(u8,u8)> {Some((2,3))}
 fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,hz:Option<u32>)->bool {self.0.borrow_mut().start_address_with_clock(c,a,r,hz)}
 fn write(&mut self,v:u8)->bool {self.0.borrow_mut().write(v)}fn read(&mut self)->u8 {self.0.borrow_mut().read()}fn stop(&mut self) {self.0.borrow_mut().stop();}
}
pub struct ToughC5 {panel:DcsPanel,pm:Arc<Mutex<M5pm1State>>,ioe:Arc<Mutex<M5ioe1State>>,touch:Arc<Mutex<TouchState>>,devices:Vec<(u8,Persistent)>,dc:Option<bool>,ready:bool,version:u64,command:u8,rejected:u64}
impl Default for ToughC5 {fn default()->Self {Self::new()}}
impl ToughC5 {
 pub fn new()->Self {
  let pm=Arc::new(Mutex::new(M5pm1State{vin_mv:5000,..Default::default()}));let ioe=Arc::new(Mutex::new(M5ioe1State::default()));let touch=Arc::new(Mutex::new(TouchState{x:0,y:0,down:false}));
  let devices:Vec<(u8,Box<dyn I2cDevice>)>=vec![(0x6e,Box::new(M5pm1::new(pm.clone()))),(0x4f,Box::new(M5ioe1::new(ioe.clone(),0))),(0x2e,Box::new(Touch{s:touch.clone(),pm:pm.clone(),ptr:None,first:true,packet:[0;5]}))];
  Self{panel:DcsPanel::new(320,240),pm,ioe,touch,devices:devices.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),dc:None,ready:false,version:1,command:0,rejected:0}
 }
 fn sync(&mut self) {let pm=self.pm.lock().unwrap();let ioe=self.ioe.lock().unwrap();let ready=pm.ldo3v3_enabled&&ioe.driven_high&0x18==0x18;if self.ready&&!ready {self.panel.reset();self.version=self.version.wrapping_add(1);}self.ready=ready;if pm.driven_high&4==0 {self.touch.lock().unwrap().down=false;}}
}
impl BoardModel for ToughC5 {
 fn name(&self)->&'static str {"m5-tough-c5"}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,_c:u64,r:PinRoutes<'_>) {self.dc=r.software_output(26);self.sync();}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {self.devices.iter().map(|(a,d)|(0,*a,Box::new(Routed(d.clone()))as Box<dyn I2cDevice>)).collect()}
 fn advance_to(&mut self,_c:u64) {self.sync();}
 fn touch_at(&mut self,_c:u64,x:u16,y:u16,down:bool) {self.sync();if self.pm.lock().unwrap().driven_high&4!=0 {let mut s=self.touch.lock().unwrap();s.x=x.min(319);s.y=y.min(239);s.down=down;}}
 fn touch(&mut self,x:u16,y:u16,down:bool) {self.touch_at(0,x,y,down);}
 fn uses_qspi_pins(&self)->bool {true}
 fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
  self.sync();let floating=vec![255;rx_len];
  if !self.ready||host!=2||p.unsupported_mode||p.full_duplex||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|w|*w>1)||p.sclk&(1<<9)==0||p.cs&(1<<25)==0||(!tx.is_empty()&&p.data[0]&(1<<7)==0)||self.dc.is_none() {self.rejected=self.rejected.wrapping_add(1);return floating;}
  self.panel.dc=self.dc==Some(true);if self.panel.dc&&self.command==0x2c&&self.panel.colmod!=0x55 {return floating;}
  for b in tx {if !self.panel.dc {self.command= *b;}self.panel.byte(*b);}if !tx.is_empty(){self.version=self.version.wrapping_add(1);}
  if rx_len!=0&&p.input[1]!=Some(8)&&!(p.sio&&p.input[0]==Some(7)) {return floating;}
  (0..rx_len).map(|i|match self.command {0x04=>[0xe3,0,0].get(i).copied().unwrap_or(0),0xda=>0xe3,_=>255}).collect()
 }
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
  let ioe=self.ioe.lock().unwrap();let duty=ioe.pwm[3];let visible=self.ready&&duty!=0&&self.panel.on&&!self.panel.sleeping;
  let pixels=self.panel.gram.iter().map(|p|{if !visible {return 0;}let mut v=if self.panel.inverted {*p}else{!*p};if self.panel.madctl&8!=0 {v=(v&0x7e0)|((v&0xf800)>>11)|((v&31)<<11);}let d=u32::from(duty);((((u32::from(v>>11)*d/4095)<<11)|((u32::from((v>>5)&63)*d/4095)<<5))|(u32::from(v&31)*d/4095))as u16}).collect();
  Some((320,240,pixels,self.version.wrapping_add(u64::from(ioe.driven_high)).wrapping_add(u64::from(duty))))
 }
 fn display_version(&self)->u64 {let s=self.ioe.lock().unwrap();self.version.wrapping_add(u64::from(s.driven_high)).wrapping_add(u64::from(s.pwm[3]))}
 fn display_frames(&self)->u64 {self.panel.frames}
 fn report(&self)->String {format!("[m5-tough-c5] C5HR8 16MiB/8MiB; ILI9342C320x240 GPIO25/26/7/8/9 ready={} pixels={} rejected={}; PM1/IOE1+CHSC6540 SDA2/SCL3 digital subset; guest HP-I2C route supported (factory uses LP-I2C); TF image prepared but transport, RTC, buzzer, analog power and radio unimplemented",self.ready,self.panel.pixels_written,self.rejected)}
}
