//! K136 VAMeter-Firmware5e8f847b hal_config/hal_display: MOSI35/SCK36/CS37/DC34/RST33/BL38.
//! A3/B1/press2,BOOT0. Real display/input subset; INA226,physical V/I,relay,buzzer absent.
use esp_soc::{BoardModel,board::{SpiPins,VirtualCycle},pins::PinRoutes,devices::DcsPanel};
pub struct VAMeter {panel:DcsPanel,dc:Option<bool>,reset:bool,gpio_light:bool,pwm:Option<bool>,changes:u64}
impl Default for VAMeter {fn default()->Self {Self::new()}}
impl VAMeter {pub fn new()->Self {Self{panel:DcsPanel::st7789(),dc:None,reset:false,gpio_light:false,pwm:None,changes:0}}fn lit(&self)->bool {self.pwm.unwrap_or(self.gpio_light)}}
impl BoardModel for VAMeter {
 fn name(&self)->&'static str {"vameter"}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,_c:VirtualCycle,r:PinRoutes<'_>) {let previous=(self.reset,self.lit());self.dc=r.software_output(34);let reset=r.software_output(33)==Some(true);if self.reset&&!reset {self.panel.reset();}self.reset=reset;self.gpio_light=r.software_output(38)==Some(true);if previous!=(self.reset,self.lit()){self.changes=self.changes.wrapping_add(1);}}
 fn uses_gpio_pwm(&self)->bool {true}
 fn gpio_pwm_at(&mut self,_c:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {let old=self.lit();self.pwm=pwm(38).map(|(_,d)|d!=0);if old!=self.lit(){self.changes=self.changes.wrapping_add(1);}}
 fn uses_spi_pins(&self)->bool {true}
 fn spi_transfer_pins(&mut self,h:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {if matches!(h,2|3)&&self.reset&&p.sclk&(1<<36)!=0&&p.mosi&(1<<35)!=0&&p.cs&(1<<37)!=0 {if let Some(dc)=self.dc {self.panel.dc=dc;for &b in tx{self.panel.byte(b);}self.changes=self.changes.wrapping_add(1);}}vec![255;rx]}
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(1,true),(2,true),(3,true)]}
 fn named_pin(&self,n:&str)->Option<u8> {match n{"encoder-press"=>Some(2),"boot"=>Some(0),_=>None}}
 fn encoder(&self)->Option<(u8,u8)> {Some((3,1))}
 fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
  let mut f=vec![0;240*240];if self.reset&&self.lit()&&self.panel.on&&!self.panel.sleeping {
   // Zero-offset240-square source HAL endpoint. MY selects lower240 rows of
   // the320-row RAM after DCS mirroring; physical mounting is uncalibrated.
   let y=if self.panel.madctl&0x80!=0 {80}else{0};for row in 0..240 {f[row*240..(row+1)*240].copy_from_slice(&self.panel.gram[(row+y)*240..(row+y+1)*240]);}
  }Some((240,240,f,self.display_version()))
 }
 fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.changes)}
 fn display_frames(&self)->u64 {self.panel.frames}
 fn report(&self)->String {format!("[vameter] realST7789 SPI RGB565 frames={} pixels={} BL38={} encoderA3/B1/press2; INA2260x40/41,relay,buzzer,physical voltage/current circuit and calibrated orientation unimplemented",self.panel.frames,self.panel.pixels_written,self.lit())}
}
