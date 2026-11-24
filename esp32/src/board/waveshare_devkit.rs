//! Exact Waveshare ESP32-DEV-KIT-XX module variants, with/without headers.
//! Official schematic: WS2812B-0807 on27; BOOT0 with10k pull-up; RESET is EN.
//! Store SKUs and module suffixes establish N4/0, N16/0 and N16R8 memories.
//! Header options change the assembly, not the electrical simulation profile.
use esp_soc::{Board,BoardModel,devices::Ws2812Chain};

pub const PROFILES:[(&str,usize,usize);4]=[
    ("waveshare-esp32-devkit-wroom-32e-n4",4,0),
    ("waveshare-esp32-devkit-wroom-32ue-n16",16,0),
    ("waveshare-esp32-devkit-wrover-e-n16r8",16,8),
    ("waveshare-esp32-devkit-wrover-ie-n16r8",16,8),
];
pub fn memory_mib(name:&str)->Option<(usize,usize)> {PROFILES.iter().find(|p|p.0==name).map(|p|(p.1,p.2))}
pub fn make_board(name:&str)->Option<Board> {PROFILES.iter().find(|p|p.0==name).map(|p|Box::new(DevKit{name:p.0,rgb:Ws2812Chain::new(1)}) as Board)}
struct DevKit {name:&'static str,rgb:Ws2812Chain}
impl BoardModel for DevKit {
    fn name(&self)->&'static str {self.name}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {(id=="boot").then_some(0)}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {format!("[{}] exact module memory {:?}MiB; BOOT0, WS2812B27 updates={}; USB bridge, antenna RF and battery charging not modeled",self.name,memory_mib(self.name),self.rgb.updates)}
}
