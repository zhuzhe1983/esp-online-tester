//! K117 AtomU, independently mapped from docs.m5stack.com/en/core/AtomU.
//! ESP32-PICO-D4 (4MiB/no PSRAM), G39 key, one G27 SK6812; SPM1423
//! PDM CLK5/DATA19 and IR12 are reserved, not an emulated PCM microphone.
use esp_soc::{BoardModel,devices::Ws2812Chain};
pub struct AtomU {rgb:Ws2812Chain}
impl AtomU {pub fn new()->Self {Self{rgb:Ws2812Chain::new(1)}}}
impl Default for AtomU {fn default()->Self {Self::new()}}
impl BoardModel for AtomU {
    fn name(&self)->&'static str {"atomu"}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(39,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"|"g39"=>Some(39),_=>None}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[atomu] K117 SK6812/G27 updates={} rgb={:?}; SPM1423 PDM/IR/physical USB unmodeled",self.rgb.updates,self.rgb.leds)}
}
