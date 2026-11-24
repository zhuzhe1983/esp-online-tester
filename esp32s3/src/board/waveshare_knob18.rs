//! Source-bounded digital model of the S3 half of Waveshare Knob1.8.
//! The bundled driver is named SH8601 but uses F0 register banks on a backlit
//! LCD. That class name is not evidence of the fitted die; no SH8601 identity
//! or AMOLED defaults are asserted. The secondary classic ESP32 is separate.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,i2c_bitbang::I2cBitBang}};
pub struct WaveshareKnob18{panel:DcsPanel,page:u8,header:[u8;4],header_len:usize,command:u8,opaque:bool,discard:bool,selected:bool,version:u64,cycle:u64,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,touch:Arc<Mutex<Cst816State>>,i2c:I2cBitBang,attached:bool,levels:[bool;3],edges:Vec<BoardEdge>,card:bool,rejected:u64}
impl Default for WaveshareKnob18{fn default()->Self{Self::new()}}
impl WaveshareKnob18{
 pub fn new()->Self{Self{panel:DcsPanel::new(360,360),page:0,header:[0;4],header_len:0,command:0,opaque:true,discard:false,selected:false,version:0,cycle:0,reset:false,light:0,gpio_light:0,pwm:None,touch:Default::default(),i2c:I2cBitBang::new(),attached:false,levels:[true;3],edges:Vec::new(),card:false,rejected:0}}
 fn deselect(&mut self){self.header_len=0;self.discard=false;}
 fn reset_panel(&mut self){self.panel.reset();self.panel.gram.fill(0);self.page=0;self.header_len=0;self.discard=false;self.version+=1;}
 fn sync(&mut self){let high=self.touch.lock().expect("Knob touch").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:9,level:high});}let light=self.pwm.unwrap_or(self.gpio_light);if light!=self.light{self.light=light;self.version+=1;}}
 fn phase(&mut self,mut data:&[u8],width:u8)->bool{
  if self.header_len<4{if width!=1{return false;}let n=(4-self.header_len).min(data.len());self.header[self.header_len..self.header_len+n].copy_from_slice(&data[..n]);self.header_len+=n;data=&data[n..];if self.header_len==4{let h=self.header;if h[1]!=0||h[3]!=0||!matches!(h[0],2|0x32)||h[0]==0x32&&!matches!(h[2],0x2c|0x3c){return false;}self.command=h[2];self.opaque=self.page!=0||!matches!(h[2],1|0x10|0x11|0x20|0x21|0x28|0x29|0x2a|0x2b|0x2c|0x3c|0x36|0x3a)||h[0]==2&&matches!(h[2],0x2c|0x3c);if !self.opaque{self.panel.dc=false;self.panel.byte(self.command);self.panel.dc=true;if self.command==1{self.panel.gram.fill(0);}self.version+=1;}}}
  if data.is_empty(){return true;}
  if self.header_len!=4||width!=if self.header[0]==0x32{4}else{1}{return false;}
  if self.command==0xf0&&self.header[0]==2{self.page=data[0];return true;}
  if self.opaque{return true;}
  if matches!(self.command,0x2c|0x3c)&&(self.panel.colmod&7!=5||self.panel.madctl&!0xe8!=0){return false;}for &v in data{self.panel.byte(v);}self.version+=1;true
 }
}
impl BoardModel for WaveshareKnob18{
 fn name(&self)->&'static str{"waveshare-s3-knob-touch-lcd18"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;let reset=r.software_output(21)==Some(true);if self.reset&&!reset{self.reset_panel();}
 if reset!=self.reset{self.version+=1;}self.reset=reset;let selected=r.software_output(14)==Some(false);if self.selected&&!selected{self.deselect();}self.selected=selected;self.gpio_light=if r.software_output(47)==Some(true){65535}else{0};{let mut t=self.touch.lock().expect("Knob touch");let high=r.software_output(10)==Some(true);if t.powered&&!high{t.reset();}t.powered=high;}let clock=r.software_output(12).unwrap_or(true);let data=self.i2c.drive(clock,r.software_output(11).unwrap_or(true));for(i,(pin,high))in[(11,data),(12,clock)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>){self.cycle=c;self.pwm=pwm(47).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<14)==0{return vec![255;rx];}let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let pads=p.sclk&(1<<13)!=0&&p.data[0]&(1<<15)!=0&&(!quad||[16,17,18].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));let bits=u64::from(p.command_bits)+u64::from(p.address_bits)+u64::from(p.data_bits);let valid=self.reset&&pads&&rx==0&&!p.unsupported_mode&&!p.full_duplex&&!p.sio&&p.dummy_cycles==0&&!p.clock_idle_high&&!p.clock_output_edge&&p.read_bits==0&&p.read_width==0&&p.command_bits.is_multiple_of(8)&&p.address_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8)&&bits==b.len()as u64*8&&(p.command_bits==0)==(p.command_width==0)&&(p.address_bits==0)==(p.address_width==0)&&(p.data_bits==0)==(p.data_width==0);if !valid{self.discard=true;self.rejected+=1;}
 if !self.discard{let mut at=0;for(n,width)in[(usize::from(p.command_bits)/8,p.command_width),(usize::from(p.address_bits)/8,p.address_width),(p.data_bits as usize/8,p.data_width)]{if n==0{continue;}
 if !self.phase(&b[at..at+n],width){self.discard=true;self.rejected+=1;break;}at+=n;}}
 if !p.keep_cs&&p.software_cs&(1<<14)==0{self.deselect();}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x15,Box::new(Cst816s::new(self.touch.clone())));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((11,12))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.sync();}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.touch.lock().expect("Knob touch").update(x.min(359),y.min(359),down);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(8,true),(7,true),(11,self.levels[0]),(12,self.levels[1]),(9,self.levels[2])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn encoder(&self)->Option<(u8,u8)>{Some((8,7))}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([4,3,5,6,42,2],self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let on=self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping&&self.panel.madctl&!0xe8==0;let mut frame=if on{self.panel.gram.iter().map(|p|if self.panel.madctl&8!=0{(p&0x07e0)|(p>>11)|((p&31)<<11)}else{*p}).collect()}else{vec![0;360*360]};for y in 0..360{for x in 0..360{let dx=2*x as i64+1-360;let dy=2*y as i64+1-360;if dx*dx+dy*dy>360*360{frame[y*360+x]=0;}}}Some((360,360,frame,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("Knob1.8 S3 only: manufacturer-driver RGB565/QSPI360sq mode0 CS14CLK13D15/16/17/18RST21 BL47; source F0 bank common-command gating; CST816-family common report unknown ID SDA11SCL12 RST10IRQ9; S3 encoder8/7 and SDMMC4bit4/3/5/6/42/2. pixels={} rejected={}. Physical LCD die unknown despite SH8601 driver classname; no fabricated ID. No second ESP32-U4WDH execution/Bluetoothaudio, DRV2605 waveform, batteryADC, microphone/DAC/CH445P/audioPCM, real scan cadence, vendor gamma/voltage/optical inversion, physical glass orientation/detent or full factory acceptance. Reset RAM zero is explicit unknown-state policy.",self.panel.pixels_written,self.rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareKnob18,reset:bool,light:bool,touch:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for(p,v)in[(21,reset),(47,light),(10,touch)]{g.enable|=1<<p;if v{g.out|=1<<p;}m.write(4+4*p,1<<12);}b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn packet(c:u8,d:&[u8])->(SpiQspiPins,Vec<u8>){let quad=matches!(c,0x2c|0x3c);let mut b=vec![if quad{0x32}else{2},0,c,0];b.extend(d);(SpiQspiPins{sclk:1<<13,data:[1<<15,1<<16,1<<17,1<<18],cs:1<<14,command_bits:8,address_bits:24,data_bits:(d.len()*8)as u32,command_width:1,address_width:1,data_width:if d.is_empty(){0}else if quad{4}else{1},..Default::default()},b)}
 fn cmd(b:&mut WaveshareKnob18,c:u8,d:&[u8]){let(p,v)=packet(c,d);b.spi_transfer_qspi_pins(2,p,&v,0);}
 #[test]fn qspi_bank_boundary_pixels_wrong_pads_reset_and_backlight(){let mut b=WaveshareKnob18::new();route(&mut b,true,true,true);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,179,0,180]);cmd(&mut b,0x2b,&[0,179,0,179]);cmd(&mut b,0x2c,&[0xf8,0,7,0xe0]);assert_eq!(&b.display().unwrap().2[179*360+179..179*360+181],&[0xf800,0x7e0]);cmd(&mut b,0xf0,&[1]);cmd(&mut b,0x10,&[]);assert!(!b.panel.sleeping,"bank1 command10 cannot turn off common display");cmd(&mut b,0xf0,&[0]);for bad in 0..3{let(mut p,v)=packet(0x2c,&[255,255]);match bad{0=>p.cs=1<<20,1=>p.data[2]=1<<19,_=>p.clock_idle_high=true}let n=b.panel.pixels_written;b.spi_transfer_qspi_pins(2,p,&v,0);assert_eq!(b.panel.pixels_written,n);}route(&mut b,true,false,true);assert_eq!(b.display().unwrap().2[179*360+179],0);route(&mut b,true,true,true);assert_eq!(b.display().unwrap().2[179*360+179],0xf800);route(&mut b,false,true,true);route(&mut b,true,true,true);for c in[0x11,0x29]{cmd(&mut b,c,&[]);}assert!(b.display().unwrap().2.iter().all(|p|*p==0),"unknown reset RAM is zero policy, not stale framebuffer");}
 #[test]fn touch_report_reaches_independent_irq_and_sd_is_four_bit(){let mut b=WaveshareKnob18::new();route(&mut b,true,true,true);let mut ds=b.i2c_devices();b.touch(123,234,true);assert!(b.input_levels().contains(&(9,false)));let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x15).unwrap().2.as_mut();assert_eq!(d.pins(),Some((11,12)));assert!(d.start(false));assert!(d.write(0));assert!(d.start(true));assert_eq!((0..7).map(|_|d.read()).collect::<Vec<_>>(),[0,0,1,0,123,0,234]);b.advance_to(1);assert!(b.input_levels().contains(&(9,true)));route(&mut b,true,true,false);assert!(!d.start(false));assert_eq!(b.encoder(),Some((8,7)));b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([4,3,5,6,42,2],true)));assert!(b.named_pin("encoder-press").is_none());}
}
