//! ESP32-S3-Touch-AMOLED-1.32: overview identifies CO5300/CST820; fixed
//! factory source uses an SH8601-named class with RGB565, Xgap6 and common
//! CO5300 commands. We retain that evidence difference, not infer chip ID.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use esp_soc::{board::{BoardEdge,BoardModel,SpiQspiPins},pins::PinRoutes,devices::{i2c_bitbang::I2cBitBang,qspi_dcs::QspiDcsPanel,es8311::Es8311}};
pub struct WaveshareAmoled132{panel:QspiDcsPanel,touch:Arc<Mutex<Cst816State>>,i2c:I2cBitBang,attached:bool,reset:bool,selected:bool,cycle:u64,levels:[bool;3],edges:Vec<BoardEdge>}
impl Default for WaveshareAmoled132{fn default()->Self{Self::new()}}
impl WaveshareAmoled132{
 pub fn new()->Self{Self{panel:QspiDcsPanel::co5300(466,466,6,0),touch:Default::default(),i2c:I2cBitBang::new(),attached:false,reset:false,selected:false,cycle:0,levels:[true;3],edges:Vec::new()}}
 fn irq(&mut self){let high=self.touch.lock().expect("AMOLED132 touch").irq_high();if high!=self.levels[2]{self.levels[2]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:6,level:high});}}
}
impl BoardModel for WaveshareAmoled132{
 fn name(&self)->&'static str{"waveshare-s3-touch-amoled132"}
 fn uses_gpio_routes(&self)->bool{true}
 fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>){self.cycle=c;let reset=r.software_output(8)==Some(true);if self.reset&&!reset{self.panel.reset();}self.reset=reset;let selected=r.software_output(10)==Some(false);if self.selected&&!selected{self.panel.deselect();}self.selected=selected;{let mut t=self.touch.lock().expect("AMOLED132 touch");let on=r.software_output(7)==Some(true);if t.powered&&!on{t.reset();}t.powered=on;}let scl=r.software_output(48).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(47).unwrap_or(true));for(i,(pin,high))in[(47,sda),(48,scl)].into_iter().enumerate(){if high!=self.levels[i]{self.levels[i]=high;self.edges.push(BoardEdge{cycle:c,pin,level:high});}}self.irq();}
 fn uses_qspi_pins(&self)->bool{true}
 fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,b:&[u8],rx:usize)->Vec<u8>{if p.cs&(1<<10)==0{return vec![255;rx];}let quad=[p.command_width,p.address_width,p.data_width].contains(&4);let routed=p.sclk&(1<<11)!=0&&p.data[0]&(1<<12)!=0&&(!quad||[13,14,15].into_iter().enumerate().all(|(i,pin)|p.data[i+1]&(1<<pin)!=0));if !routed||!self.reset||rx!=0{self.panel.reject_until_deselect();}else{self.panel.transfer(p,b);}
  if !p.keep_cs&&p.software_cs&(1<<10)==0{self.panel.deselect();}vec![255;rx]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{if !self.attached{self.i2c.attach(0x15,Box::new(Cst816s::new(self.touch.clone())));self.i2c.attach(0x18,Box::new(Es8311::new()));self.attached=true;}(0..2).flat_map(|b|self.i2c.controller_devices(Some((47,48))).into_iter().map(move|(a,d)|(b,a,d))).collect()}
 fn advance_to(&mut self,c:u64){self.cycle=c;self.irq();}
 fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(17,true),(47,self.levels[0]),(48,self.levels[1]),(6,self.levels[2])]}
 fn named_pin(&self,n:&str)->Option<u8>{match n{"boot"=>Some(0),"power"=>Some(17),_=>None}}
 fn touch(&mut self,x:u16,y:u16,d:bool){self.touch.lock().expect("AMOLED132 touch").update(466-x.min(465),466-y.min(465),d);self.irq();}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((466,466,if self.reset{self.panel.frame()}else{vec![0;466*466]},self.display_version()))}
 fn display_version(&self)->u64{self.panel.version()}
 fn display_frames(&self)->u64{self.panel.frames()}
 fn report(&self)->String{format!("ESP32-S3-AMOLED1.32 CO5300 overview/SH8601 driver-class discrepancy retained; QSPI11/[12,13,14,15]CS10RST8 gap6, RGB565466sq; CST820 common02..06 report @15 SDA47SCL48RST7IRQ6 unknown chip ID, source raw=(466-x,466-y); ES8311control@18, BOOT0/PWR17 digital inputs. No TF socket declared. SYS18 battery latch/ADC, TE9, PCM, full application, optical pose and physical validation absent. pixels={}",self.panel.pixels_written())}
}
#[cfg(test)]mod tests{use super::*;
 fn route(b:&mut WaveshareAmoled132,on:bool){let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();for pin in[7,8]{g.enable|=1<<pin;if on{g.out|=1<<pin;}m.write(4+4*pin,1<<12);}b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));}
 fn packet(c:u8,d:&[u8])->(SpiQspiPins,Vec<u8>){let quad=c==0x2c;let mut b=vec![if quad{0x32}else{2},0,c,0];b.extend(d);(SpiQspiPins{sclk:1<<11,data:[1<<12,1<<13,1<<14,1<<15],cs:1<<10,command_bits:8,address_bits:24,data_bits:(d.len()*8)as u32,command_width:1,address_width:1,data_width:if d.is_empty(){0}else if quad{4}else{1},..Default::default()},b)}
 fn cmd(b:&mut WaveshareAmoled132,c:u8,d:&[u8]){let(p,v)=packet(c,d);b.spi_transfer_qspi_pins(2,p,&v,0);}
 #[test]fn exact_x_gap_rgb565_touch_coordinates_reset_and_wrong_lane(){let mut b=WaveshareAmoled132::new();route(&mut b,true);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x51,&[255]);cmd(&mut b,0x2a,&[0,6,0,6]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);let n=b.panel.pixels_written();let(mut p,v)=packet(0x2c,&[0,0]);p.data[2]=1<<16;b.spi_transfer_qspi_pins(2,p,&v,0);assert_eq!(b.panel.pixels_written(),n);let mut ds=b.i2c_devices();b.touch(123,234,true);let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x15).unwrap().2.as_mut();assert!(d.start(false));assert!(d.write(2));assert!(d.start(true));let v:Vec<_>=(0..5).map(|_|d.read()).collect();assert_eq!(v,[1,1,87,0,232]);route(&mut b,false);assert!(!d.start(false));assert!(b.display().unwrap().2.iter().all(|v|*v==0));assert!(b.sd_card_slot().is_none());}}
