//! Waveshare P4 DSI products: sourced GPIO and touch-control subset, no fabricated DSI pixels.
//! Sources: waveshare_boards 9be477dc7edf81b4b61b0163470214da857ef481,
//! exact product schematics/READMEs and waveshare/esp_lcd_touch_cst9217 2.0.0.
//! Receipts: docs/research/waveshare-p4-s31-2026-11-24. GT9271 hardware is kept
//! distinct from GT911 while implementing the documented compatible report subset.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State}};
use esp_soc::{Board,BoardModel,board::BoardEdge,pins::PinRoutes,devices::{es8311::Es8311,pcf85063::{Pcf85063,Pcf85063State},qmi8658::{Qmi8658,Qmi8658Port},imu_motion::ImuMotion}};
#[derive(Clone,Copy,PartialEq)]
pub enum TouchKind{Gt911,Gt9271,Cst9217}
#[derive(Clone,Copy)]
pub struct Profile{pub name:&'static str,pub width:u16,pub height:u16,pub reset:Option<u8>,pub touch:TouchKind,pub revision:u16,pub sd_power:Option<(u8,bool)>,pub relay:bool}
const fn lcd(name:&'static str,width:u16,height:u16,reset:Option<u8>,revision:u16)->Profile{Profile{name,width,height,reset,touch:TouchKind::Gt911,revision,sd_power:Some((45,false)),relay:false}}
pub const PROFILES:&[Profile]=&[
    Profile{touch:TouchKind::Cst9217,sd_power:None,..lcd("waveshare-p4-knob-amoled175",466,466,Some(27),301)},
    lcd("waveshare-p4-lcd5-v13",720,1280,Some(23),100),lcd("waveshare-p4-lcd5-v32",720,1280,Some(23),301),
    lcd("waveshare-p4-lcd43-v13",480,800,Some(23),100),lcd("waveshare-p4-lcd43-v32",480,800,Some(23),301),
    Profile{touch:TouchKind::Gt9271,sd_power:None,..lcd("waveshare-p4-lcd7",720,1280,None,301)},
    Profile{touch:TouchKind::Gt9271,sd_power:None,..lcd("waveshare-p4-lcd8",800,1280,None,301)},
    Profile{touch:TouchKind::Gt9271,sd_power:None,..lcd("waveshare-p4-lcd101",800,1280,None,301)},
    lcd("waveshare-p4-lcd7b",1024,600,Some(23),301),
    lcd("waveshare-p4-lcd4b-v13",720,720,Some(23),100),lcd("waveshare-p4-lcd4b-v32",720,720,Some(23),301),
    Profile{relay:true,..lcd("waveshare-p4-panel-eth2ro-v13",720,720,Some(23),100)},
    Profile{relay:true,..lcd("waveshare-p4-panel-eth2ro-v32",720,720,Some(23),301)},
    Profile{touch:TouchKind::Gt9271,..lcd("waveshare-p4-lcd34c",800,800,Some(23),301)},
    Profile{touch:TouchKind::Gt9271,..lcd("waveshare-p4-lcd4c",720,720,Some(23),301)},
];
pub fn profile(name:&str)->Option<Profile>{PROFILES.iter().copied().find(|p|p.name==name)}
pub fn make_board(name:&str)->Option<Board>{profile(name).map(|p|Box::new(Screen::new(p))as Board)}
/// Exact 7/10.1-DSI-TOUCH-A kit attachments: GT9271 on SDA7/SCL8,
/// no wired touch RESET/INT. Bare carriers never call this constructor.
/// The shared I2C backlight controller and MIPI-DSI output remain unmodeled.
pub(crate) fn external_panel_kit(name:&'static str,width:u16,height:u16,sd_power:Option<(u8,bool)>)->Board{
    Box::new(Screen::new(Profile{name,width,height,reset:None,touch:TouchKind::Gt9271,revision:301,sd_power,relay:false}))
}
struct Wired(Box<dyn I2cDevice>);
impl I2cDevice for Wired{
    fn pins(&self)->Option<(u8,u8)>{Some((7,8))}
    fn start(&mut self,r:bool)->bool{self.0.start(r)}fn write(&mut self,b:u8)->bool{self.0.write(b)}fn read(&mut self)->u8{self.0.read()}fn stop(&mut self){self.0.stop();}
}
struct SharedCodec(Arc<Mutex<Es8311>>);
impl I2cDevice for SharedCodec{
    fn start(&mut self,r:bool)->bool{self.0.lock().unwrap().start(r)}fn write(&mut self,b:u8)->bool{self.0.lock().unwrap().write(b)}fn read(&mut self)->u8{self.0.lock().unwrap().read()}fn stop(&mut self){self.0.lock().unwrap().stop();}
}
struct Goodix{inner:Gt911,kind:TouchKind,address:u16,phase:u8}
impl I2cDevice for Goodix{
    fn start(&mut self,r:bool)->bool{if !r{self.phase=0;}self.inner.start(r)}
    fn write(&mut self,b:u8)->bool{match self.phase{0=>{self.address=u16::from(b)<<8;self.phase=1;},1=>{self.address|=u16::from(b);self.phase=2;},_=>self.address=self.address.wrapping_add(1)}self.inner.write(b)}
    fn read(&mut self)->u8{let a=self.address;self.address=self.address.wrapping_add(1);let b=self.inner.read();if self.kind==TouchKind::Gt9271&&(0x8140..0x8144).contains(&a){b"9271"[(a-0x8140)as usize]}else{b}}
    fn stop(&mut self){self.inner.stop();}
}
#[derive(Default)]
struct CstState{powered:bool,x:u16,y:u16,down:bool}
struct Cst9217{state:Arc<Mutex<CstState>>,reg:u16,phase:u8,pos:usize}
impl I2cDevice for Cst9217{
    fn start(&mut self,r:bool)->bool{if !r{self.phase=0;}self.pos=0;self.state.lock().unwrap().powered&&(!r||matches!(self.reg,0xd000|0xd1f8))}
    fn write(&mut self,b:u8)->bool{match self.phase{0=>{self.reg=u16::from(b)<<8;self.phase=1;},1=>{self.reg|=u16::from(b);self.phase=2;},_=>{}}true}
    fn read(&mut self)->u8{let s=self.state.lock().unwrap();let data=if self.reg==0xd1f8{[0xd2,1,0xd2,1,0,0,0,0,0,0]}else{[if s.down{6}else{0},(s.x>>4)as u8,(s.y>>4)as u8,((s.x&15)<<4|(s.y&15))as u8,0,1,0xab,0,0,0]};let b=data.get(self.pos).copied().unwrap_or(0);self.pos=self.pos.saturating_add(1);b}
}
struct Screen{
    p:Profile,cycle:u64,boot:bool,power:bool,cw:bool,ccw:bool,edges:Vec<BoardEdge>,
    goodix:Arc<Mutex<Gt911State>>,cst:Arc<Mutex<CstState>>,irq:bool,
    rtc:Arc<Mutex<Pcf85063State>>,imu:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,codec:Arc<Mutex<Es8311>>,motor:[bool;2],relay:[bool;2],
}
impl Screen{
    fn new(p:Profile)->Self{let goodix=Arc::new(Mutex::new(Gt911State::new(p.width,p.height,crate::periph::CPU_HZ/100)));goodix.lock().unwrap().powered=p.reset.is_none();let imu=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new_c(imu.clone(),crate::periph::CPU_HZ)));Self{p,cycle:0,boot:true,power:false,cw:true,ccw:true,edges:Vec::new(),goodix,cst:Arc::new(Mutex::new(CstState::default())),irq:true,rtc:Arc::new(Mutex::new(Pcf85063State::new(crate::periph::CPU_HZ))),imu,qmi,codec:Arc::new(Mutex::new(Es8311::new())),motor:[false;2],relay:[false;2]}}
    fn knob(&self)->bool{self.p.touch==TouchKind::Cst9217}
}
impl BoardModel for Screen{
    fn name(&self)->&'static str{self.p.name}
    fn named_pin(&self,id:&str)->Option<u8>{match id{"boot"=>Some(35),"power" if self.knob()=>Some(2),"cw" if self.knob()=>Some(20),"ccw" if self.knob()=>Some(21),_=>None}}
    fn input_levels(&self)->Vec<(u8,bool)>{let mut p=vec![(35,self.boot)];if self.knob(){p.extend([(2,self.power),(20,self.cw),(21,self.ccw),(26,self.irq)]);}p}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool{let Some(pin)=self.named_pin(id)else{return false;};let high=if id=="power"{pressed}else{!pressed};let state=match id{"boot"=>&mut self.boot,"power"=>&mut self.power,"cw"=>&mut self.cw,_=>&mut self.ccw};if *state!=high{*state=high;self.edges.push(BoardEdge{cycle,pin,level:high});}true}
    fn uses_gpio_edges(&self)->bool{true}fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>){self.cycle=cycle;let powered=self.p.reset.is_none_or(|p|routes.software_output(p)==Some(true));if self.knob(){let mut s=self.cst.lock().unwrap();s.powered=powered;if !powered{s.down=false;}self.motor=[routes.software_output(22)==Some(true),routes.software_output(23)==Some(true)];}else{let mut s=self.goodix.lock().unwrap();s.powered=powered;if !powered{s.ready=false;}}
        if self.p.relay{self.relay=[routes.software_output(32)==Some(true),routes.software_output(46)==Some(true)];}}
    fn advance_to(&mut self,cycle:u64){self.cycle=cycle;if self.knob(){self.imu.cycle.store(cycle,std::sync::atomic::Ordering::Relaxed);self.rtc.lock().unwrap().advance_to(cycle);}else{self.goodix.lock().unwrap().advance_to(cycle);}}
    fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
    fn touch(&mut self,x:u16,y:u16,down:bool){if self.knob(){let mut s=self.cst.lock().unwrap();if !s.powered{return;}s.x=465-x.min(465);s.y=465-y.min(465);s.down=down;let irq=!down;if self.irq!=irq{self.irq=irq;self.edges.push(BoardEdge{cycle:self.cycle,pin:26,level:irq});}}else{let mut s=self.goodix.lock().unwrap();if s.powered{s.update(x,y,down);}}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{let mut out=Vec::new();for bus in [0,1]{
        if self.knob(){out.push((bus,0x5a,Box::new(Wired(Box::new(Cst9217{state:self.cst.clone(),reg:0,phase:0,pos:0})))as Box<dyn I2cDevice>));out.push((bus,0x51,Box::new(Wired(Box::new(Pcf85063::new(self.rtc.clone()))))));out.push((bus,0x6b,Box::new(Wired(Box::new(Qmi8658Port::new(self.qmi.clone()))))));}
        else{out.push((bus,0x5d,Box::new(Wired(Box::new(Goodix{inner:Gt911::new(self.goodix.clone()),kind:self.p.touch,address:0,phase:0})))as Box<dyn I2cDevice>));out.push((bus,0x18,Box::new(Wired(Box::new(SharedCodec(self.codec.clone()))))));}
    }out}
    fn set_imu_motion(&mut self,mode:u32)->bool{self.knob()&&self.qmi.borrow_mut().set_motion_mode(mode)}
    fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.knob()&&self.qmi.borrow_mut().set_host_sample(a,g)}
    fn report(&self)->String{format!("[{}] {}x{} physical panel; DSI/DPI framebuffer unimplemented. I2C7/8 BOOT35; touch single-contact subset, Goodix strap selects only5d; CST project/checkcode metadata unsupported. Knob direction switches CW20/CCW21, power2 HIGH, motor22/23={:?}; relay32/46={:?}. No analog/audio/USB/CSI/radio/Ethernet/factory acceptance.",self.name(),self.p.width,self.p.height,self.motor,self.relay)}
}
#[cfg(test)]
mod tests{
    use super::*;
    fn read(d:&mut dyn I2cDevice,reg:u16,n:usize)->Vec<u8>{assert!(d.start(false));for b in reg.to_be_bytes(){assert!(d.write(b));}assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    #[test]
    fn every_profile_uses_exact_input_geometry_without_a_synthetic_dsi_frame(){
        for p in PROFILES{let mut b=Screen::new(*p);assert!(b.display().is_none());assert!(b.button_at(1,"boot",true));assert!(b.input_levels().contains(&(35,false)));let d=b.i2c_devices();assert!(d.iter().all(|(_,_,v)|v.pins()==Some((7,8))));assert!(!d.iter().any(|(_,a,_)|*a==0x14));assert_eq!(d.iter().filter(|(_,a,_)|*a==0x5a).count(),if b.knob(){2}else{0});}
    }
    #[test]
    fn knob_direction_switches_and_cst9217_reports_do_not_emulate_quadrature(){
        let mut b=Screen::new(PROFILES[0]);assert!(b.encoder().is_none());assert!(b.button_at(1,"cw",true));assert!(b.input_levels().contains(&(20,false)));assert!(b.input_levels().contains(&(21,true)));assert!(b.button_at(2,"cw",false));assert!(b.button_at(3,"ccw",true));assert!(b.button_at(4,"power",true));assert!(b.input_levels().contains(&(2,true)));let mut d=b.i2c_devices();assert!(!d[0].2.start(true));b.cst.lock().unwrap().powered=true;b.touch(100,200,true);let a=read(d[0].2.as_mut(),0xd000,10);assert_eq!((a[0],a[5],a[6]),(6,1,0xab));assert_eq!(((u16::from(a[1])<<4)|u16::from(a[3]>>4),(u16::from(a[2])<<4)|u16::from(a[3]&15)),(365,265));assert_eq!(read(d[0].2.as_mut(),0xd1f8,4),[0xd2,1,0xd2,1]);assert!(d[0].2.start(false));d[0].2.write(0xd2);d[0].2.write(4);assert!(!d[0].2.start(true),"unknown project identity must not be fabricated");
    }
    #[test]
    fn goodix_variants_preserve_identity_ack_and_bounded_touch_coordinates(){
        for name in ["waveshare-p4-lcd5-v32","waveshare-p4-lcd34c"]{let p=profile(name).unwrap();let mut b=Screen::new(p);let mut d=b.i2c_devices();assert!(!d[0].2.start(true));b.goodix.lock().unwrap().powered=true;assert_eq!(read(d[0].2.as_mut(),0x8140,4),if p.touch==TouchKind::Gt9271{b"9271"}else{b"911\0"});b.touch(u16::MAX,u16::MAX,true);b.advance_to(1);let a=read(d[0].2.as_mut(),0x814e,6);assert_eq!(a[0],0x81);assert_eq!(u16::from_le_bytes([a[2],a[3]]),p.width-1);assert_eq!(u16::from_le_bytes([a[4],a[5]]),p.height-1);assert!(d[0].2.start(false));for v in [0x81,0x4e,0]{d[0].2.write(v);}assert_eq!(read(d[0].2.as_mut(),0x814e,1),[0]);}
    }
}
