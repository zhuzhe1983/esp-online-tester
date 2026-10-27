//! K131 / K131-V11 carrier: Sch_AirQ_v1.0, M5Stack AirQUserDemo a2abd67f.
//! USB-powered digital panel and explicit virtual environmental samples.
use std::sync::{Arc,Mutex};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{ssd1681::Ssd1681,i2c_bitbang::I2cBitBang};
use esp_periph::i2c::{I2cDevice,air_quality::{AirSensor,AirSensorState,Kind}};
pub struct AirQuality {name:&'static str,panel:Ssd1681,dc:bool,reset:bool,cycle:VirtualCycle,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,sensors:[Arc<Mutex<AirSensorState>>;2],gpio_events:u64}
impl AirQuality {pub fn new()->Self{Self::with_revision(false)} pub fn v11()->Self{Self::with_revision(true)} fn with_revision(v11:bool)->Self{let scd=Arc::new(Mutex::new(AirSensorState::new(Kind::Scd40)));let mut sen=AirSensorState::new(Kind::Sen55);sen.powered=false;Self{name:if v11{"airquality-v11"}else{"airquality"},panel:Ssd1681::new(1,240_000),dc:false,reset:true,cycle:0,i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),sensors:[scd,Arc::new(Mutex::new(sen))],gpio_events:0}}}
impl Default for AirQuality{fn default()->Self{Self::new()}}
impl BoardModel for AirQuality {
 fn name(&self)->&'static str{self.name}
 fn gpio_changes(&mut self,changes:&[(u8,bool)]){self.gpio_events+=changes.len() as u64;for &(p,h) in changes{match p{3=>self.dc=h,2=>{if self.reset&&!h{self.panel.reset();}self.reset=h;},_=>{}}}}
 fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64){self.advance_to(cycle);self.gpio_changes(changes);let high=|pin:u8|enabled&(1u64<<pin)==0||output&(1u64<<pin)!=0;let scl=high(12);let sda=self.i2c.drive(scl,high(11));for(i,(pin,level))in[(11,sda),(12,scl)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}self.sensors[1].lock().expect("AirQ SEN55 mutex poisoned").power(enabled&(1<<10)!=0&&output&(1<<10)==0);}
 fn gpio_events(&self)->u64{self.gpio_events}
 fn uses_spi_pins(&self)->bool{true}
 fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8>{if !self.reset||pins.cs&(1<<4)==0||pins.sclk&(1<<5)==0||(pins.mosi&(1<<6)==0&&pins.miso!=Some(6)){return vec![255;rx];}self.panel.write(self.dc,tx);(0..rx).map(|_|self.panel.read()).collect()}
 fn advance_to(&mut self,cycle:VirtualCycle){self.cycle=cycle;self.panel.advance_to(cycle);for s in &self.sensors{s.lock().expect("AirQ sensor mutex poisoned").cycle=cycle;}}
 fn next_deadline(&self)->Option<VirtualCycle>{self.panel.next_deadline()}
 fn take_edges(&mut self)->Vec<BoardEdge>{let mut e=std::mem::take(&mut self.edges);e.extend(self.panel.take_edges());e.sort_by_key(|x|x.cycle);e}
 fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true),(8,true),(42,true),(1,self.panel.busy()),(11,self.levels[0]),(12,self.levels[1])]}
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{self.i2c.clear();for(addr,s)in[0x62,0x69].into_iter().zip(&self.sensors){self.i2c.attach(addr,Box::new(AirSensor::new(s.clone())));}(0..2).flat_map(|bus|self.i2c.controller_devices(Some((11,12))).into_iter().map(move|(addr,d)|(bus,addr,d))).collect()}
 fn named_pin(&self,id:&str)->Option<u8>{match id{"button-a"=>Some(0),"button-b"=>Some(8),_=>None}}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((200,200,self.panel.frame.clone(),self.panel.version))}
 fn display_version(&self)->u64{self.panel.version}
 fn display_frames(&self)->u64{self.panel.frames}
 fn display_quiet_push(&self)->bool{true}
 fn report(&self)->String{let s=self.sensors[0].lock().expect("AirQ SCD40 mutex poisoned");let n=self.sensors[1].lock().expect("AirQ SEN55 mutex poisoned");format!("[{}] SSD1681 frames={} bytes={} busy={} SCD40={} SEN55={} powered={} unsupported={}/{} virtual samples=420ppm/25C/50%/PM5,7,9,10; RTC/battery/audio/network/Stamp RGB unmodeled",self.name,self.panel.frames,self.panel.ram_bytes_written,self.panel.busy(),s.commands,n.commands,n.powered,s.unsupported,n.unsupported)}
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn revision_identity_keeps_the_documented_carrier_routes(){let a=AirQuality::new();let v=AirQuality::v11();assert_ne!(a.name(),v.name());assert_eq!(v.named_pin("button-a"),Some(0));assert_eq!(v.named_pin("button-b"),Some(8));assert_eq!(v.input_levels(),a.input_levels());assert!(v.report().contains("Stamp RGB unmodeled"));}
 #[test]fn source_routes_and_sensor_power_are_not_interchangeable(){let mut b=AirQuality::new();let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((11,12))));assert_eq!(b.named_pin("button-b"),Some(8));assert_eq!(b.named_pin("power"),None);assert!(!ds[1].2.start(false));b.gpio_output_at(0,&[],1<<10,0);b.advance_to(240_000_000);assert!(ds[1].2.start(false));b.gpio_output_at(240_000_001,&[],1<<10,1<<10);assert!(!ds[1].2.start(false));let pins=SpiPins{sclk:1<<5,mosi:1<<6,cs:1<<4,..Default::default()};assert_eq!(b.spi_transfer_pins(2,pins,&[0x2f],1),[1]);assert_eq!(b.spi_transfer_pins(2,SpiPins{cs:1<<17,..pins},&[0x2f],1),[255]);b.gpio_changes(&[(2,false)]);assert_eq!(b.spi_transfer_pins(2,pins,&[0x2f],1),[255]);}
}
