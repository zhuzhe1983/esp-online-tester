//! Exact 1.9-inch ST7789V2 variants and 1.3-inch ST7789VW board.
//! Sources: seventh-source-manifest.json / factory main.c and schematics.
//! Touch1.9 R31 is NC: GPIO17 does not reset the always-3V3 touch device.
//! Bare1.9 has TWO serial WS2812B-0807 at GPIO15; touch1.9 has neither.
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,Ws2812Chain,i2c_bitbang::I2cBitBang,imu_motion::ImuMotion,qmi8658::{Qmi8658,Qmi8658Port}}};
const HZ:u64=240_000_000;
struct Wiring{name:&'static str,touch:bool,leds:bool,clock:u8,mosi:u8,cs:u8,dc:u8,reset:u8,backlight:u8,active_low:bool,width:usize,height:usize,gap:(usize,usize),reference:u8,sd:[u8;6]}
static CONFIGS:&[Wiring]=&[
 Wiring{name:"waveshare-s3-lcd19",touch:false,leds:true,clock:10,mosi:13,cs:12,dc:11,reset:9,backlight:14,active_low:true,width:170,height:320,gap:(35,0),reference:0,sd:[41,39,40,255,255,255]},
 Wiring{name:"waveshare-s3-touch-lcd19",touch:true,leds:false,clock:10,mosi:13,cs:12,dc:11,reset:9,backlight:14,active_low:true,width:170,height:320,gap:(35,0),reference:0,sd:[41,39,40,255,255,255]},
 Wiring{name:"waveshare-s3-lcd13",touch:false,leds:false,clock:40,mosi:41,cs:39,dc:38,reset:42,backlight:4,active_low:false,width:240,height:240,gap:(0,80),reference:0xc0,sd:[21,18,16,255,255,255]},
];
pub fn make_board(name:&str)->Option<Board>{CONFIGS.iter().find(|w|w.name==name).map(|w|Box::new(WaveshareSmallSpi::new(w))as Board)}
struct WaveshareSmallSpi{w:&'static Wiring,panel:DcsPanel,command:u8,dc:Option<bool>,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,version:u64,cycle:u64,rejected:u64,spi_modes:[u64;4],full_duplex_transfers:u64,read_transfers:u64,card:bool,i2c:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,touch:Arc<Mutex<Cst816State>>,motion:Arc<ImuMotion>,qmi:Rc<RefCell<Qmi8658>>,led:Ws2812Chain}
impl WaveshareSmallSpi{
 fn new(w:&'static Wiring)->Self{let mut t=Cst816State::default();t.powered=w.touch;let motion=Arc::new(ImuMotion::default());let qmi=Rc::new(RefCell::new(Qmi8658::new(motion.clone(),HZ)));Self{w,panel:DcsPanel::st7789(),command:0,dc:None,reset:false,light:0,gpio_light:0,pwm:None,version:0,cycle:0,rejected:0,spi_modes:[0;4],full_duplex_transfers:0,read_transfers:0,card:false,i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:Vec::new(),touch:Arc::new(Mutex::new(t)),motion,qmi,led:Ws2812Chain::new(2)}}
 fn sync(&mut self){let light=self.pwm.unwrap_or(self.gpio_light);if self.light!=light{self.light=light;self.version+=1;}
 if self.w.touch{let high=self.touch.lock().expect("CST816").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:21,level:high});}}}
}
impl BoardModel for WaveshareSmallSpi{
 fn name(&self)->&'static str{self.w.name}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let reset=r.software_output(self.w.reset)==Some(true);if self.reset&&!reset{self.panel.reset();self.command=0;}
 if self.reset!=reset{self.version+=1;}self.reset=reset;self.dc=r.software_output(self.w.dc);self.gpio_light=if r.software_output(self.w.backlight)==Some(!self.w.active_low){65535}else{0};let clock=r.software_output(48).unwrap_or(true);let data=self.i2c.drive(clock,r.software_output(47).unwrap_or(true));for(i,(pin,level))in[(47,data),(48,clock)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,f:&dyn Fn(u8)->Option<(f64,u32)>){self.advance_to(c);self.pwm=f(self.w.backlight).map(|(_,d)|if self.w.active_low{65535-d.min(65535)}else{d.min(65535)});self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 // ST7789V2 serial protocol samples SDA on rising SCL, with either idle level.
 // IDF v5.5.1 S3 spi_ll_master_set_mode maps modes0/3 to ck_out_edge=0.
 // Full-duplex/rx clocks do not disconnect MOSI; unsupported readback floats FF.
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<self.w.cs)==0{return vec![255;rx];}let mode=match(p.clock_idle_high,p.clock_output_edge){(false,false)=>0,(false,true)=>1,(true,true)=>2,(true,false)=>3};self.spi_modes[mode]+=1;self.full_duplex_transfers+=u64::from(p.full_duplex);self.read_transfers+=u64::from(rx>0);let valid=self.reset&&self.dc.is_some()&&!p.sio&&!p.unsupported_mode&&!p.clock_output_edge&&p.dummy_cycles==0&&p.address_bits==0&&p.sclk&(1<<self.w.clock)!=0&&p.data[0]&(1<<self.w.mosi)!=0&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|v|*v<=1)&&p.command_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8);if !valid{self.rejected+=1;return vec![255;rx];}self.panel.dc=self.dc==Some(true);if self.panel.dc&&matches!(self.command,0x2c|0x3c)&&self.panel.colmod&7!=5{self.rejected+=1;return vec![255;rx];}for &v in tx{if !self.panel.dc{self.command=v;}self.panel.byte(v);}self.version+=1;vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x6b,Box::new(Qmi8658Port::new(self.qmi.clone())));if self.w.touch{self.i2c.attach(0x15,Box::new(Cst816s::new(self.touch.clone())));}self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((47,48))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.motion.cycle.store(c,Ordering::Relaxed);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{let mut v=vec![(0,true),(47,self.levels[0]),(48,self.levels[1])];if self.w.touch{v.push((21,self.levels[2]));}v}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot"&&self.w.name!="waveshare-s3-lcd13").then_some(0)}
 fn touch(&mut self,x:u16,y:u16,down:bool){if self.w.touch{self.touch.lock().expect("CST816").update(x.min(169),y.min(319),down);self.sync();}}
 fn set_imu_motion(&mut self,m:u32)->bool{self.qmi.borrow_mut().set_motion_mode(m)}
 fn set_imu_sample(&mut self,a:[f64;3],g:[f64;3])->bool{self.qmi.borrow_mut().set_host_sample(a,g)}
 fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some((self.w.sd,self.card))}
 fn set_sd_card_present(&mut self,p:bool){self.card=p;}
 fn rmt_frame(&mut self,p:u8,b:&[bool]){if self.w.leds&&p==15{self.led.from_bits(b);}}
 fn leds(&self)->Option<(&[[u8;3]],u64)>{self.w.leds.then_some((&self.led.leds,self.led.updates))}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let(w,h)=(self.w.width,self.w.height);let mut frame=vec![0;w*h];if self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping{for y in 0..h{for x in 0..w{let mut col=x+self.w.gap.0;let mut row=y+self.w.gap.1;if self.w.reference&0x40!=0{col=239-col;}
 if self.w.reference&0x80!=0{row=319-row;}let mut p=self.panel.gram[row*240+col];if (self.panel.madctl^self.w.reference)&8!=0{p=(p&0x7e0)|(p>>11)|((p&31)<<11);}let d=self.light;let p=u32::from(p);frame[y*w+x]=((((p>>11)*d/65535)<<11)|((((p>>5)&63)*d/65535)<<5)|((p&31)*d/65535))as u16;}}}Some((w as u32,h as u32,frame,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("{} ST7789 {}x{} visible gap{:?}/referenceMADCTL{:02x}, SPI{}/{}/CS{}/DC{}/RST{} BL{}{}; I2C47/48 QMI8658@6B, CST816@15={} touch1.9 R31NC so GPIO17 disconnected; RGB15 two LEDs={}; SDMMC1bit{:?}; {}pixels rejected={}, reset={} light={} on={} sleeping={} format={:02x} madctl={:02x} nonzeroGRAM={} SPI modes0/1/2/3={:?} fullDuplex={} rxTransfers={}. No radio/analog calibration, CST identity or gestures, prism optics, vendor analog/gamma, complete app or physical validation.",self.w.name,self.w.width,self.w.height,self.w.gap,self.w.reference,self.w.clock,self.w.mosi,self.w.cs,self.w.dc,self.w.reset,self.w.backlight,if self.w.active_low{"low"}else{"high"},self.w.touch,self.w.leds,self.w.sd,self.panel.pixels_written,self.rejected,self.reset,self.light,self.panel.on,self.panel.sleeping,self.panel.colmod,self.panel.madctl,self.panel.gram.iter().filter(|&&p|p!=0).count(),self.spi_modes,self.full_duplex_transfers,self.read_transfers)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareSmallSpi,dc:bool,on:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(pin,v)in[(b.w.reset,on),(b.w.backlight,on^b.w.active_low),(b.w.dc,dc),(17,false)]{g.enable|=1<<pin;if v{g.out|=1<<pin;}m.write(4+4*u32::from(pin),1<<12);}b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));b.gpio_pwm_at(b.cycle,&|_|None);}
 fn pins(b:&WaveshareSmallSpi,n:usize)->SpiQspiPins{SpiQspiPins{sclk:1<<b.w.clock,data:[1<<b.w.mosi,0,0,0],cs:1<<b.w.cs,data_width:1,data_bits:n as u32*8,..Default::default()}}
 fn cmd(b:&mut WaveshareSmallSpi,c:u8,a:&[u8]){route(b,false,true);b.spi_transfer_qspi_pins(2,pins(b,1),&[c],0);route(b,true,true);b.spi_transfer_qspi_pins(2,pins(b,a.len()),a,0);}
 #[test]fn exact_windows_distinct_touch_leds_and_native_sd(){for w in CONFIGS{let mut b=WaveshareSmallSpi::new(w);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x36,&[w.reference]);cmd(&mut b,0x3a,&[5]);let(x,y)=(w.gap.0 as u8,w.gap.1 as u8+5);cmd(&mut b,0x2a,&[0,x,0,x]);cmd(&mut b,0x2b,&[0,y,0,y]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[5*w.width],0xf800);let n=b.panel.pixels_written;let mut p=pins(&b,2);p.data[0]=1<<37;b.spi_transfer_qspi_pins(2,p,&[0,0],0);assert_eq!(b.panel.pixels_written,n);let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((47,48))));if w.touch{b.touch(30,200,true);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x15).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(0));assert!(d.start(true));assert_eq!((0..7).map(|_|d.read()).collect::<Vec<_>>(),[0,0,1,0,30,0,200]);assert!(b.leds().is_none());}else{assert!(ds.iter().all(|(_,a,_)|*a!=0x15));}
 if w.leds{let bits=[0x123456_u32,0xabcdef].into_iter().flat_map(|v|(0..24).rev().map(move|shift|(v>>shift)&1!=0)).collect::<Vec<_>>();b.rmt_frame(38,&bits);assert_eq!(b.leds().unwrap().0,[[0;3];2]);b.rmt_frame(15,&bits);assert_eq!(b.leds().unwrap().0,[[0x34,0x12,0x56],[0xcd,0xab,0xef]]);}b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some((w.sd,true)));route(&mut b,true,false);assert!(b.display().unwrap().2.iter().all(|p|*p==0));}}
 #[test]fn low_and_high_backlights_use_duty_not_frequency(){for w in CONFIGS{let mut b=WaveshareSmallSpi::new(w);route(&mut b,true,true);b.gpio_pwm_at(0,&|p|(p==w.backlight).then_some((5000.0,16384)));let light=b.light;assert_eq!(light,if w.active_low{49151}else{16384});b.gpio_pwm_at(0,&|p|(p==w.backlight).then_some((19000.0,16384)));assert_eq!(b.light,light);b.gpio_pwm_at(0,&|_|None);assert_eq!(b.light,65535);}}
 #[test]fn full_duplex_mosi_and_mode3_preserve_real_pixels(){for w in CONFIGS{for idle in[false,true]{let mut b=WaveshareSmallSpi::new(w);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x36,&[w.reference]);cmd(&mut b,0x3a,&[5]);let(x,y)=(w.gap.0 as u8,w.gap.1 as u8+7);cmd(&mut b,0x2a,&[0,x,0,x]);cmd(&mut b,0x2b,&[0,y,0,y]);route(&mut b,false,true);let mut p=pins(&b,1);p.clock_idle_high=idle;p.full_duplex=true;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[0x2c],1),[255]);route(&mut b,true,true);p=pins(&b,2);p.clock_idle_high=idle;p.full_duplex=true;assert_eq!(b.spi_transfer_qspi_pins(2,p,&[0x07,0xe0],2),[255;2]);assert_eq!(b.display().unwrap().2[7*w.width],0x07e0);let n=b.panel.pixels_written;
 for (idle,edge) in[(false,true),(true,true)]{let mut wrong=p;wrong.clock_idle_high=idle;wrong.clock_output_edge=edge;b.spi_transfer_qspi_pins(2,wrong,&[0xf8,0],2);assert_eq!(b.panel.pixels_written,n);}
 for pin in[36,37]{let mut wrong=p;wrong.data[0]=1<<pin;b.spi_transfer_qspi_pins(2,wrong,&[0xf8,0],2);assert_eq!(b.panel.pixels_written,n);}let mut wrong=p;wrong.cs=1<<43;b.spi_transfer_qspi_pins(2,wrong,&[0xf8,0],2);assert_eq!(b.panel.pixels_written,n);assert_eq!(b.display().unwrap().2[7*w.width],0x07e0);}}}

}
