//! K140 SwitchC6 official GPIO table: key7, relay OFF14 / ON15, EH_12V ADC6.
//! Digital latching-coil subset; energy-harvesting supply and AC loads unmodeled.
use esp_soc::{BoardModel,pins::PinRoutes};
pub struct SwitchC6 {closed:bool,coils:(bool,bool),leds:[[u8;3];1],updates:u64,conflicts:u64}
impl Default for SwitchC6 {fn default()->Self {Self::new()}}
impl SwitchC6 {pub fn new()->Self {Self{closed:false,coils:(false,false),leds:[[0;3];1],updates:0,conflicts:0}}}
impl BoardModel for SwitchC6 {
 fn name(&self)->&'static str {"m5-switchc6"}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,_cycle:u64,r:PinRoutes<'_>) {
  let next=(r.software_output(14)==Some(true),r.software_output(15)==Some(true));
  if next==(true,true)&&self.coils!=next {self.conflicts=self.conflicts.wrapping_add(1);}
  if next.0&&!next.1&&!self.coils.0 {self.closed=false;}
  if next.1&&!next.0&&!self.coils.1 {self.closed=true;}
  self.coils=next;let color=if self.closed {[0,255,0]}else{[0;3]};if self.leds[0]!=color {self.leds[0]=color;self.updates=self.updates.wrapping_add(1);}
 }
 fn input_levels(&self)->Vec<(u8,bool)> {vec![(7,true)]}
 fn named_pin(&self,id:&str)->Option<u8> {(id=="button").then_some(7)}
 fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.updates))}
 fn report(&self)->String {format!("[m5-switchc6] relay digital latch closed={} OFF14/ON15={:?} simultaneous_coil_conflicts={}; GPIO7 key; no calibrated pulse-width, energy storage/ADC6 source, mains load or RF model; physical Flash unspecified (4MiB virtual); latch retains state across MCU reset",self.closed,self.coils,self.conflicts)}
}
