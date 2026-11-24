//! ESP32-S3-Touch-LCD-1.85B: exact manufacturer BSP routes, bounded ST77916
//! QSPI RGB565 command path, CST816S single-contact reports and native SDMMC.
//! Pinned sources: docs/research/waveshare-s3-20261123/source-manifest.json.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,
 devices::{DcsPanel,i2c_bitbang::I2cBitBang,pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210}};
const HZ:u64=240_000_000;
pub struct WaveshareLcd185b {
 panel:DcsPanel,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,selected:bool,
 command:u8,header:[u8;4],header_len:usize,rejected:bool,version:u64,cycle:u64,
 touch:Arc<Mutex<Cst816State>>,rtc:Arc<Mutex<Pcf85063State>>,i2c:I2cBitBang,attached:bool,
 levels:[bool;3],edges:Vec<BoardEdge>,card:bool,
}
impl Default for WaveshareLcd185b {fn default()->Self{Self::new()}}
impl WaveshareLcd185b {
 pub fn new()->Self {Self{panel:DcsPanel::new(360,360),reset:false,light:0,gpio_light:0,pwm:None,selected:false,command:0,header:[0;4],header_len:0,rejected:false,version:0,cycle:0,touch:Default::default(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:Vec::new(),card:false}}
 fn deselect(&mut self){self.header_len=0;self.rejected=false;}
 fn irq(&mut self){let high=self.touch.lock().expect("LCD185B touch").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:4,level:high});}}
}
impl BoardModel for WaveshareLcd185b {
 fn name(&self)->&'static str{"waveshare-s3-touch-lcd185b"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let reset=r.software_output(3)==Some(true);if self.reset&&!reset{self.panel.reset();self.deselect();self.version+=1;}self.reset=reset;let sel=r.software_output(21)==Some(false);if self.selected&&!sel{self.deselect();}self.selected=sel;
 self.gpio_light=if r.software_output(5)==Some(true){65535}else{0};let light=self.pwm.unwrap_or(self.gpio_light);if light!=self.light{self.light=light;self.version+=1;}
 let mut t=self.touch.lock().expect("LCD185B touch");let tr=r.software_output(1)==Some(true);if t.powered&&!tr{t.reset();}t.powered=tr;drop(t);
 let scl=r.software_output(10).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(11).unwrap_or(true));for(i,(pin,level))in[(11,sda),(10,scl)].into_iter().enumerate(){if level!=self.levels[i]{self.levels[i]=level;self.edges.push(BoardEdge{cycle:c,pin,level});}}self.irq();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,_c:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>){self.pwm=pwm(5).map(|(_,d)|d.min(65535));let v=self.pwm.unwrap_or(self.gpio_light);if v!=self.light{self.light=v;self.version+=1;}}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,mut b:&[u8],rx:usize)->Vec<u8>{
 if p.cs&(1<<21)==0{return vec![255;rx];}
 let quad=[p.command_width,p.address_width,p.data_width].contains(&4);
 let wired=p.sclk&(1<<40)!=0&&p.data[0]&(1<<46)!=0&&(!quad||[45,42,41].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));
 let valid=wired&&self.reset&&rx==0&&!p.unsupported_mode&&!p.full_duplex&&!p.sio&&p.dummy_cycles==0&&!p.clock_idle_high&&!p.clock_output_edge&&p.command_width<=1&&p.address_width<=1;
 if !valid{self.rejected=true;}
 if !self.rejected {
  if self.header_len<4 {let n=b.len().min(4-self.header_len);self.header[self.header_len..self.header_len+n].copy_from_slice(&b[..n]);self.header_len+=n;b=&b[n..];if self.header_len==4{let h=self.header;if h[1]!=0||h[3]!=0||!matches!(h[0],0x02|0x32)||h[0]==0x32&&!matches!(h[2],0x2c|0x3c){self.rejected=true;}else{self.command=h[2];if self.command!=0x3c{self.panel.dc=false;self.panel.byte(self.command);self.version+=1;}self.panel.dc=true;}}}
  if self.header_len==4&&!self.rejected{let pixel=matches!(self.command,0x2c|0x3c);let phase_ok=if pixel{self.header[0]==0x32&&p.data_width==4&&self.panel.colmod&7==5}else{self.header[0]==0x02&&p.data_width<=1};if !b.is_empty()&&!phase_ok{self.rejected=true;}else{for &v in b{self.panel.byte(v);}if !b.is_empty(){self.version+=1;}}}
 }
 if !p.keep_cs&&p.software_cs&(1<<21)==0{self.deselect();}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x15,Box::new(Cst816s::new(self.touch.clone())));self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));self.attached=true;}(0..2).flat_map(|bus|self.i2c.controller_devices(Some((11,10))).into_iter().map(move|(a,d)|(bus,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.rtc.lock().expect("LCD185B RTC").advance_to(c);self.irq();}
 fn touch(&mut self,x:u16,y:u16,d:bool){self.touch.lock().expect("LCD185B touch").update(x.min(359),y.min(359),d);self.irq();}
 fn touch_at(&mut self,c:u64,x:u16,y:u16,d:bool){self.advance_to(c);self.touch(x,y,d);}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(11,self.levels[0]),(10,self.levels[1]),(4,self.levels[2])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([15,14,16,17,12,13],self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let on=self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping;Some((360,360,if on{self.panel.gram.clone()}else{vec![0;360*360]},self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("{} 16F8P ST77916 QSPI40/[46,45,42,41]/CS21 RST3 BL5 RGB565 subset pixels={} frames={}; CST816S@15 SDA11/SCL10 RST1 IRQ4 ID unknown; RTC51 ES8311/ES7210 register controls SDMMC15/14/16/17/12/13; QMI8658/BQ27220/audio PCM/analog charge/TE/vendor gamma and complete factory acceptance absent",self.name(),self.panel.pixels_written,self.panel.frames)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareLcd185b,reset:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(pin,v)in[(3,reset),(5,true),(1,true)]{g.enable|=1<<pin;if v{g.out|=1<<pin;}m.write(4+4*pin,1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn pins(quad:bool)->SpiQspiPins{SpiQspiPins{sclk:1<<40,data:[1<<46,1<<45,1<<42,1<<41],cs:1<<21,command_width:1,address_width:1,data_width:if quad{4}else{1},..Default::default()}}
 fn cmd(b:&mut WaveshareLcd185b,c:u8,a:&[u8]){let mut d=vec![if c==0x2c{0x32}else{0x02},0,c,0];d.extend(a);b.spi_transfer_qspi_pins(2,pins(c==0x2c),&d,0);}
 #[test]fn source_routes_rgb565_pixels_touch_and_wrong_lane_rejection(){let mut b=WaveshareLcd185b::new();route(&mut b,true);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);let count=b.panel.pixels_written;let mut p=pins(true);p.data[2]=1<<43;b.spi_transfer_qspi_pins(2,p,&[0x32,0,0x2c,0,0,0],0);assert_eq!(b.panel.pixels_written,count);
 let mut ds=b.i2c_devices();assert!(!ds.iter().any(|(_,a,_)|*a==0x55));b.touch(320,210,true);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x15).unwrap().2.as_mut();assert_eq!(d.pins(),Some((11,10)));assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));let report:Vec<_>=(0..5).map(|_|d.read()).collect();assert_eq!(report,[1,1,64,0,210]);route(&mut b,false);assert!(b.display().unwrap().2.iter().all(|p|*p==0));}
}
