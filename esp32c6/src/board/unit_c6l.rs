//! U202 Unit C6L, M5Stack official pin map fetched 2026-11-26.
//! S012 radio module is retained, OLED CS6/MOSI21/SCLK20/DC18/RESET15,
//! user key on PI4IOE5V6408 P0, direct G2 WS2812. No buzzer/USB/RF physics.
use super::stamp_c6lora::StampC6LoRa;
use esp_periph::i2c::I2cDevice;
use esp_soc::{BoardModel,board::{BoardEdge,VirtualCycle,SpiPins},pins::PinRoutes,devices::{Ws2812Chain,oled_mono::{OledMono,OledKind},sx1262::LoRaInput}};
pub struct UnitC6L {radio:StampC6LoRa,oled:OledMono,rgb:Ws2812Chain,dc:Option<bool>,reset:bool,clock:bool,bits:u8,byte:u8,shift_dc:Option<bool>,chunk:Vec<u8>}
impl Default for UnitC6L {fn default()->Self {Self::new()}}
impl UnitC6L {
 pub fn new()->Self {let mut radio=StampC6LoRa::new();radio.set_aux_input(0,true);Self{radio,oled:OledMono::new(OledKind::Ssd1306,64,48,32),rgb:Ws2812Chain::new(1),dc:None,reset:false,clock:false,bits:0,byte:0,shift_dc:None,chunk:Vec::new()}}
 fn flush(&mut self) {if let Some(dc)=self.shift_dc {if !self.chunk.is_empty(){self.oled.write_spi(dc,&self.chunk);self.chunk.clear();}}self.bits=0;self.byte=0;}
}
impl BoardModel for UnitC6L {
 fn name(&self)->&'static str {"unit-c6l"}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_output_at(&mut self,c:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {self.radio.gpio_output_at(c,changes,enabled,output);}
 fn gpio_events(&self)->u64 {self.radio.gpio_events()}
 fn gpio_waveform_at(&mut self,c:VirtualCycle,routes:PinRoutes<'_>) {
  self.dc=routes.software_output(18);self.reset=routes.software_output(15)==Some(true);self.oled.set_reset(self.reset);
  let clk=routes.software_output(20);let data=routes.software_output(21);let selected=routes.software_output(6)==Some(false);
  self.radio.gpio_waveform_at(c,routes);
  if !self.reset||!selected||clk.is_none()||data.is_none()||self.dc.is_none(){self.flush();self.clock=clk.unwrap_or(false);self.shift_dc=None;return;}
  if self.shift_dc!=self.dc {self.flush();self.shift_dc=self.dc;}
  if !self.clock&&clk==Some(true){self.byte=(self.byte<<1)|u8::from(data==Some(true));self.bits+=1;if self.bits==8{self.chunk.push(self.byte);self.bits=0;self.byte=0;if self.chunk.len()==1024{self.flush();}}}self.clock=clk==Some(true);
 }
 fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {self.radio.i2c_devices()}
 fn input_levels(&self)->Vec<(u8,bool)> {self.radio.input_levels()}
 fn take_edges(&mut self)->Vec<BoardEdge> {self.radio.take_edges()}
 fn advance_to(&mut self,c:VirtualCycle) {self.radio.advance_to(c);}
 fn next_deadline(&self)->Option<VirtualCycle> {self.radio.next_deadline()}
 fn button_at(&mut self,_c:VirtualCycle,id:&str,pressed:bool)->bool {if id!="button"{return false;}self.radio.set_aux_input(0,!pressed);true}
 fn uses_spi_pins(&self)->bool {true}
 fn spi_transfer_pins(&mut self,h:u8,p:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
  if h==2&&p.sclk&(1<<20)!=0&&p.mosi&(1<<21)!=0&&p.cs&(1<<6)!=0 {if let Some(dc)=self.dc {self.flush();self.oled.write_spi(dc,tx);}return vec![0xff;rx_len];}
  self.radio.spi_transfer_pins(h,p,tx,rx_len)
 }
 fn rmt_frame(&mut self,p:u8,bits:&[bool]) {if p==2 {self.rgb.from_bits(bits);}}
 fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let(w,h)=self.oled.dimensions();Some((w,h,self.oled.frame(),self.oled.version()))}
 fn display_frames(&self)->u64 {self.oled.frames()}
 fn display_version(&self)->u64 {self.oled.version()}
 fn lora_receive_at(&mut self,c:VirtualCycle,p:&LoRaInput)->bool {self.radio.lora_receive_at(c,p)}
 fn lora_snapshot(&self)->Option<String> {self.radio.lora_snapshot()}
 fn report(&self)->String {format!("[unit-c6l] {}; {}; P0 semantic key, RMT RGB2; buzzer, USB, analog optics and radio propagation unmodeled",self.oled.report(),self.radio.report())}
}
#[cfg(test)]mod tests {
 use super::*;
 fn pins()->SpiPins {SpiPins{sclk:1<<20,mosi:1<<21,miso:None,cs:1<<6,keep_cs:false}}
 #[test]fn oled_requires_actual_route_reset_and_guest_commands() {
  let mut b=UnitC6L::new();b.dc=Some(false);b.spi_transfer_pins(2,pins(),&[0xaf],0);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
  b.oled.set_reset(true);b.spi_transfer_pins(2,pins(),&[0xaf,0x20,0,0x21,32,95,0x22,0,5],0);b.dc=Some(true);
  let before=b.display_version();b.spi_transfer_pins(2,SpiPins{mosi:1<<22,..pins()},&[255],0);assert_eq!(b.display_version(),before);
  b.spi_transfer_pins(2,pins(),&[1,2],0);let frame=b.display().unwrap();assert_eq!((frame.0,frame.1),(64,48));assert_eq!(frame.2[0],0xffff);assert_eq!(frame.2[65],0xffff);assert_eq!(frame.2.iter().filter(|p|**p!=0).count(),2);
  b.oled.set_reset(false);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
 }
 #[test]fn p0_key_reaches_expander_and_rgb_uses_only_g2() {
  let mut b=UnitC6L::new();let mut d=b.i2c_devices().pop().unwrap().2;
  let read=|d:&mut dyn I2cDevice|{assert!(d.start(false));assert!(d.write(15));assert!(d.start(true));d.read()};
  assert_eq!(read(d.as_mut())&1,1);assert!(b.button_at(0,"button",true));assert_eq!(read(d.as_mut())&1,0);assert!(b.button_at(1,"button",false));assert_eq!(read(d.as_mut())&1,1);assert!(!b.button_at(2,"power",true));
  let bits=vec![true;24];b.rmt_frame(8,&bits);assert_eq!(b.leds().unwrap().1,0);b.rmt_frame(2,&bits);assert_eq!(b.leds().unwrap().0,[[255;3]]);
 }
}
