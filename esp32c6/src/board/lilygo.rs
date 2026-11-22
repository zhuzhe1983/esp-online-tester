//! LILYGO ESP32-C6 boards, pinned manufacturer sources and scope in
//! docs/platform/research/lilygo-c6-profiles-2026-11-22.json.
//! T7-C6 V1.0 has a charger LED, not an MCU RGB LED. T-Knob V1.0 has four
//! low-active green LEDs; its MT6701/TMC6300 haptic mechanism is not quadrature.
use esp_soc::{BoardModel,board::BoardEdge,pins::PinRoutes};

const KNOB_LEDS:[u8;4]=[23,22,21,20];
pub struct LilygoC6 {knob:bool,boot_high:bool,leds:[[u8;3];4],updates:u64,edges:Vec<BoardEdge>,gpio_events:u64}
impl LilygoC6 {
    pub fn t7_c6()->Self {Self::new(false)}
    pub fn t_knob()->Self {Self::new(true)}
    fn new(knob:bool)->Self {Self{knob,boot_high:true,leds:[[0;3];4],updates:0,edges:Vec::new(),gpio_events:0}}
    fn led(&mut self,index:usize,green:u8) {let color=[0,green,0];if self.leds[index]!=color {self.leds[index]=color;self.updates=self.updates.wrapping_add(1);}}
}
impl BoardModel for LilygoC6 {
    fn name(&self)->&'static str {if self.knob {"lilygo-t-knob"}else{"lilygo-t7-c6"}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(8,true),(9,self.boot_high)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"button"|"btnA"=>Some(9),_=>None}}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        if self.named_pin(id)!=Some(9) {return false;}
        let high=!pressed;if self.boot_high!=high {self.boot_high=high;self.edges.push(BoardEdge{cycle,pin:9,level:high});}true
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn uses_gpio_routes(&self)->bool {self.knob}
    fn gpio_output_at(&mut self,_cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.gpio_events+=changes.len() as u64;}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {
        if self.knob {for (index,pin) in KNOB_LEDS.into_iter().enumerate() {self.led(index,if routes.software_output(pin)==Some(false) {255}else{0});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {self.knob.then_some((&self.leds,self.updates))}
    fn report(&self)->String {
        if self.knob {format!("[lilygo-t-knob] green_leds={:?} changes={}; MT6701 angle, TMC6300 motor/haptics and buzzer unmodeled",self.leds,self.updates)}
        else {"[lilygo-t7-c6] GPIO9 BOOT; TP4065 charger/LED and battery ADC unmodeled; no screen or MCU user LED".into()}
    }
}
