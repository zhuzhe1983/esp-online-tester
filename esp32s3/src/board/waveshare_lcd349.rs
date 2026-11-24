//! ESP32-S3-Touch-LCD-3.49 V2/PCB1.1. Source-specific AXS15231B mode3
//! QSPI, native172x640, two independent I2C pad pairs and TCA9554 controls.
//! The storefront introduction's172x320 typo is not used as geometry.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{BoardModel,BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{DcsPanel,i2c_bitbang::I2cBitBang,tca9554::{Tca9554,Tca9554State},pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210}};
const HZ:u64=240_000_000;
#[derive(Default)]struct Touch{on:bool,down:bool,x:u16,y:u16}
struct AxsTouch{state:Arc<Mutex<Touch>>,query:[u8;11],length:usize,at:usize}
impl I2cDevice for AxsTouch{
 fn start(&mut self,read:bool)->bool{if !self.state.lock().expect("AXS touch").on{return false;}
 if !read{self.length=0;}else{self.at=0;}true}
 fn write(&mut self,b:u8)->bool{if self.length<11{self.query[self.length]=b;}self.length=self.length.saturating_add(1);true}
 fn read(&mut self)->u8{let t=self.state.lock().expect("AXS touch");let mut q=self.query;let len=q[7];q[7]=0;let valid=t.on&&self.length==11&&matches!(len,8|14)&&q==[0xb5,0xab,0xa5,0x5a,0,0,0,0,0,0,0];let report=[0,u8::from(t.down),((t.x>>8)as u8&15)|if t.down{0x80}else{0},t.x as u8,(t.y>>8)as u8&15,t.y as u8,0,0,0,0,0,0,0,0];let v=if valid&&self.at<usize::from(len){report[self.at]}else{255};self.at=self.at.saturating_add(1);v}
}
pub struct WaveshareLcd349{panel:DcsPanel,header:[u8;4],header_len:usize,command:u8,opaque:bool,discard:bool,selected:bool,version:u64,cycle:u64,reset:bool,light:u32,gpio_light:u32,pwm:Option<u32>,touch:Arc<Mutex<Touch>>,expander:Arc<Mutex<Tca9554State>>,rtc:Arc<Mutex<Pcf85063State>>,buses:[I2cBitBang;2],attached:bool,levels:[bool;5],edges:Vec<BoardEdge>,irq_until:u64,card:bool,rejected:u64}
impl Default for WaveshareLcd349{fn default()->Self{Self::new()}}
impl WaveshareLcd349{
 pub fn new()->Self{Self{panel:DcsPanel::new(172,640),header:[0;4],header_len:0,command:0,opaque:true,discard:false,selected:false,version:0,cycle:0,reset:false,light:0,gpio_light:0,pwm:None,touch:Default::default(),expander:Default::default(),rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),buses:[I2cBitBang::new(),I2cBitBang::new()],attached:false,levels:[true;5],edges:Vec::new(),irq_until:0,card:false,rejected:0}}
 fn deselect(&mut self){self.header_len=0;self.discard=false;}
 fn sync(&mut self){let mut e=self.expander.lock().expect("LCD349 expander");let reset=e.driven(5)==Some(true);let enabled=e.driven(1)==Some(true);if self.reset&&!reset{self.panel.reset();self.header_len=0;self.discard=false;self.version+=1;}self.reset=reset;let mut t=self.touch.lock().expect("AXS touch");if t.on&&!reset{t.down=false;}t.on=reset;e.set_inputs(1,u8::from(!reset||self.cycle>=self.irq_until));let irq=e.irq_high();if irq!=self.levels[4]{self.levels[4]=irq;self.edges.push(BoardEdge{cycle:self.cycle,pin:8,level:irq});}let light=if enabled{self.pwm.unwrap_or(self.gpio_light)}else{0};if self.light!=light{self.light=light;self.version+=1;}}
 fn phase(&mut self,mut data:&[u8],width:u8)->bool{if self.header_len<4{if width!=1{return false;}let n=(4-self.header_len).min(data.len());self.header[self.header_len..self.header_len+n].copy_from_slice(&data[..n]);self.header_len+=n;data=&data[n..];if self.header_len==4{let h=self.header;if h[1]!=0||h[3]!=0||!matches!(h[0],2|0x32)||h[0]==0x32&&!matches!(h[2],0x2c|0x3c){return false;}self.command=h[2];self.opaque=!matches!(h[2],1|0x10|0x11|0x20|0x21|0x28|0x29|0x2a|0x2b|0x2c|0x3c|0x36|0x3a)||h[0]==2&&matches!(h[2],0x2c|0x3c);if !self.opaque{self.panel.dc=false;self.panel.byte(self.command);self.panel.dc=true;self.version+=1;}}}
 if data.is_empty(){return true;}
 if self.header_len!=4||width!=if self.header[0]==0x32{4}else{1}{return false;}
 if self.opaque{return true;}
 if matches!(self.command,0x2c|0x3c)&&(self.panel.colmod&7!=5||self.panel.madctl&0x20!=0){return false;}for &v in data{self.panel.byte(v);}self.version+=1;true}
}
impl BoardModel for WaveshareLcd349{
 fn name(&self)->&'static str{"waveshare-s3-touch-lcd349-v2"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.advance_to(c);let selected=r.software_output(9)==Some(false);if self.selected&&!selected{self.deselect();}self.selected=selected;self.gpio_light=if r.software_output(42)==Some(true){65535}else{0};for(i,(sda,scl))in[(47,48),(17,18)].into_iter().enumerate(){let clock=r.software_output(scl).unwrap_or(true);let data=self.buses[i].drive(clock,r.software_output(sda).unwrap_or(true));for(j,(pin,high))in[(sda,data),(scl,clock)].into_iter().enumerate(){let index=i*2+j;if high!=self.levels[index]{self.levels[index]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}}self.sync();}
 fn uses_gpio_pwm(&self)->bool{true}
 fn gpio_pwm_at(&mut self,c:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>){self.cycle=c;self.pwm=pwm(42).map(|(_,d)|d.min(65535));self.sync();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<9)==0{return vec![255;rx];}self.sync();let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let pads=p.sclk&(1<<10)!=0&&p.data[0]&(1<<11)!=0&&(!quad||[12,13,14].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));let bits=u64::from(p.command_bits)+u64::from(p.address_bits)+u64::from(p.data_bits);let valid=self.reset&&pads&&rx==0&&!p.unsupported_mode&&!p.full_duplex&&!p.sio&&p.dummy_cycles==0&&p.clock_idle_high&&!p.clock_output_edge&&p.read_bits==0&&p.read_width==0&&p.command_bits.is_multiple_of(8)&&p.address_bits.is_multiple_of(8)&&p.data_bits.is_multiple_of(8)&&bits==b.len()as u64*8&&(p.command_bits==0)==(p.command_width==0)&&(p.address_bits==0)==(p.address_width==0)&&(p.data_bits==0)==(p.data_width==0);if !valid{self.discard=true;self.rejected+=1;}
 if !self.discard{let mut at=0;for(n,width)in[(usize::from(p.command_bits)/8,p.command_width),(usize::from(p.address_bits)/8,p.address_width),(p.data_bits as usize/8,p.data_width)]{if n==0{continue;}
 if !self.phase(&b[at..at+n],width){self.discard=true;self.rejected+=1;break;}at+=n;}}
  if !p.keep_cs&&p.software_cs&(1<<9)==0{self.deselect();}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.buses[0].attach(0x20,Box::new(Tca9554::new(self.expander.clone())));self.buses[0].attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));self.buses[0].attach(0x18,Box::new(Es8311::new()));self.buses[0].attach(0x40,Box::new(Es7210::new()));self.buses[1].attach(0x3b,Box::new(AxsTouch{state:self.touch.clone(),query:[0;11],length:0,at:0}));self.attached=true;}let mut devices=Vec::new();for(bus,pins)in[(0,(47,48)),(1,(17,18))]{for controller in 0..2{devices.extend(self.buses[bus].controller_devices(Some(pins)).into_iter().map(|(a,d)|(controller,a,d)));}}devices}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.rtc.lock().expect("LCD349 RTC").advance_to(c);self.sync();}
 fn next_deadline(&self)->Option<u64>{(self.cycle<self.irq_until).then_some(self.irq_until)}
 fn touch(&mut self,x:u16,y:u16,down:bool){self.sync();let mut t=self.touch.lock().expect("AXS touch");if !t.on{return;}t.x=640-y.min(639);t.y=x.min(171);t.down=down;self.irq_until=self.cycle.saturating_add(HZ/1000);drop(t);self.sync();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(47,self.levels[0]),(48,self.levels[1]),(17,self.levels[2]),(18,self.levels[3]),(8,self.levels[4])]}
 fn named_pin(&self,n:&str)->Option<u8>{(n=="boot").then_some(0)}
 fn sd_card_slot(&self)->Option<([u8;6],bool)>{Some(([41,39,40,255,255,255],self.card))}
 fn set_sd_card_present(&mut self,v:bool){self.card=v;}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let on=self.reset&&self.light>0&&self.panel.on&&!self.panel.sleeping&&self.panel.madctl&0x20==0;let frame=if on{self.panel.gram.iter().map(|p|if self.panel.madctl&8!=0{(p&0x07e0)|(p>>11)|((p&31)<<11)}else{*p}).collect()}else{vec![0;172*640]};Some((172,640,frame,self.version))}
 fn display_version(&self)->u64{self.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn report(&self)->String{format!("LCD3.49 currentV2/PCB1.1 AXS15231B172x640 QSPI mode3 CLK10/[11,12,13,14]/CS9, TCA9554@20 SDA47SCL48 EX5resetEX1BL+GPIO42PWM; touch@3B separateSDA17SCL18 report8/14 and factoryraw=(640-y,x), EX0touchIRQ -> GPIO8 (explicit1msdigitalpulse); RTC51/ES18+40 digitalcontrol; SDMMC1bit41/39/40. BOOT0. pixels={} rejected={}. RGB565+RAMWR/RAMWRC subset; no vendor voltage/gamma, TE21 cadence, rectangularMV, ADC/latch/analogbattery/PCM/IMU, complete factory or physical acceptance. V1 swapped TE/reset and BL/EXIO are not accepted.",self.panel.pixels_written,self.rejected)}
}
#[cfg(test)]mod tests{use super::*;
 fn write(d:&mut dyn I2cDevice,r:u8,b:u8){assert!(d.start(false));assert!(d.write(r));assert!(d.write(b));d.stop();}
 fn route(b:&mut WaveshareLcd349,light:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();g.enable|=1<<42;if light{g.out|=1<<42;}m.write(4+4*42,1<<12);b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn packet(c:u8,d:&[u8])->(SpiQspiPins,Vec<u8>){let quad=matches!(c,0x2c|0x3c);let mut b=vec![if quad{0x32}else{2},0,c,0];b.extend(d);(SpiQspiPins{sclk:1<<10,data:[1<<11,1<<12,1<<13,1<<14],cs:1<<9,command_bits:8,address_bits:24,data_bits:(d.len()*8)as u32,command_width:1,address_width:1,data_width:if d.is_empty(){0}else if quad{4}else{1},clock_idle_high:true,..Default::default()},b)}
 fn cmd(b:&mut WaveshareLcd349,c:u8,d:&[u8]){let(p,v)=packet(c,d);b.spi_transfer_qspi_pins(3,p,&v,0);}
 #[test]fn v2_expander_gate_mode3_streaming_and_separate_touch_bus(){let mut b=WaveshareLcd349::new();let mut ds=b.i2c_devices();let ex=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();assert_eq!(ex.pins(),Some((47,48)));write(ex,3,0xdd);write(ex,1,0x22);route(&mut b,true);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x3c,&[0,0x1f,0xff,0xff]);let f=b.display().unwrap().2;assert_eq!(&f[..2],&[0xf800,0x07e0]);assert_eq!(&f[172..174],&[0x1f,0xffff]);let n=b.panel.pixels_written;let(mut p,v)=packet(0x3c,&[0,0]);p.clock_idle_high=false;b.spi_transfer_qspi_pins(3,p,&v,0);assert_eq!(b.panel.pixels_written,n);let(mut p,v)=packet(0x3c,&[0,0]);p.data[3]=1<<15;b.spi_transfer_qspi_pins(3,p,&v,0);assert_eq!(b.panel.pixels_written,n);b.touch(100,200,true);let d=ds.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x3b).unwrap().2.as_mut();assert_eq!(d.pins(),Some((17,18)));assert!(d.start(false));for v in[0xb5,0xab,0xa5,0x5a,0,0,0,14,0,0,0]{assert!(d.write(v));}assert!(d.start(true));assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(),[0,1,0x81,184,0,100]);assert!(b.input_levels().contains(&(8,false)));let ex=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x20).unwrap().2.as_mut();write(ex,1,0x20);route(&mut b,true);assert!(b.display().unwrap().2.iter().all(|p|*p==0));write(ex,1,0x22);route(&mut b,true);assert_eq!(b.display().unwrap().2[0],0xf800);write(ex,1,2);route(&mut b,true);assert!(!b.reset);b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([41,39,40,255,255,255],true)));}
}
