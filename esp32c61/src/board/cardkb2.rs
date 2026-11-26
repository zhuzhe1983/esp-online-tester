//! U215 CardKB2, ESP32-C61HF4 / Sch_Unit_CardKB2. Factory bsp_keyboard.c
//! rows0..3; columns4,5,6,7,8,29,10,11,22,23,24. Two empty intersections.
//! Espressif button_matrix.c drives one row high and reads pulled-down columns.
//! Host edges close switches only; the guest performs scanning and key mapping.
use esp_soc::{BoardModel,board::BoardEdge,pins::PinRoutes};
pub const COLUMNS:[u8;11]=[4,5,6,7,8,29,10,11,22,23,24];
pub struct CardKb2 {keys:[[bool;11];4],rows:[Option<bool>;4],columns:[bool;11],edges:Vec<BoardEdge>,cycle:u64,leds:[[u8;3];1],updates:u64}
impl Default for CardKb2 {fn default()->Self {Self::new()}}
impl CardKb2 {
 pub fn new()->Self {Self{keys:[[false;11];4],rows:[None;4],columns:[false;11],edges:Vec::new(),cycle:0,leds:[[0;3];1],updates:0}}
 fn resolve(&mut self) {for (c,pin) in COLUMNS.iter().enumerate() {let high=(0..4).any(|r|self.keys[r][c]&&self.rows[r]==Some(true));let low=(0..4).any(|r|self.keys[r][c]&&self.rows[r]==Some(false));let level=high&&!low;if level!=self.columns[c] {self.columns[c]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:*pin,level});}}}
}
impl BoardModel for CardKb2 {
 fn name(&self)->&'static str {"m5-cardkb2"}
 fn uses_gpio_routes(&self)->bool {true}
 fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>) {
  self.cycle=cycle;self.rows=std::array::from_fn(|r|routes.software_output(r as u8));self.resolve();
  let next=[[27,28,9].map(|p|if routes.software_output(p)==Some(false) {255}else{0})];if next!=self.leds {self.leds=next;self.updates=self.updates.wrapping_add(1);}
 }
 fn matrix_key_at(&mut self,cycle:u64,row:u8,column:u8,pressed:bool)->bool {
  if row>=4||column>=11||(column==10&&matches!(row,0|3)) {return false;}self.cycle=cycle;self.keys[row as usize][column as usize]=pressed;self.resolve();true
 }
 fn input_levels(&self)->Vec<(u8,bool)> {COLUMNS.iter().copied().zip(self.columns).chain([(9,true)]).collect()}
 fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
 fn named_pin(&self,id:&str)->Option<u8> {(id=="boot").then_some(9)}
 fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.updates))}
 fn report(&self)->String {format!("[m5-cardkb2] C61HF4 4MiB physical 4x11/42-switch GPIO matrix; rows={:?}, columns={:?}, RGB cathodes27/28/9={:?}; guest scans/maps keys; simultaneous opposing row drives resolve low (no analog contention/ghost calibration); no UART-text key injection, I2C-slave firmware, USB HID/BLE/RF model",self.rows,self.columns,self.leds[0])}
}
