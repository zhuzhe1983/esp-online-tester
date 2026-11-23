//! Three distinct LilyGO S2 products. GPIO wiring comes from the exact official
//! 2021 base schematics and T-Dongle-ESP32S2 Setup203 + schematic + factory source.
use esp_soc::{Board, BoardModel, devices::DcsPanel, pins::PinRoutes};

pub const WOOR:&str="lilygo-esp32-s2-woor";
pub const BASE:&str="lilygo-esp32-s2";
pub const DONGLE:&str="lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board";

pub fn make_board(id:&str)->Option<Board> {
    let name=match id {WOOR=>WOOR,BASE=>BASE,DONGLE=>DONGLE,"bare"|"none"=>return Some(Box::new(esp_soc::NoBoard)),_=>return None};
    Some(Box::new(LilygoS2::new(name)))
}
#[derive(Default)]
struct SpiShift {clock:Option<bool>,dc:Option<bool>,value:u8,bits:u8}
pub struct LilygoS2 {
    name:&'static str,panel:Option<DcsPanel>,shift:SpiShift,reset:bool,light:bool,
    command:u8,version:u64,bytes:u64,led:[[u8;3];1],led_version:u64,
}
impl LilygoS2 {
    fn new(name:&'static str)->Self {Self{name,panel:(name==DONGLE).then(DcsPanel::st7789),shift:Default::default(),reset:false,light:false,command:0,version:1,bytes:0,led:[[0;3]],led_version:1}}
    fn drive(&mut self,routes:PinRoutes<'_>) {
        let Some(panel)=self.panel.as_mut() else{return;};
        let reset=routes.software_output(38)==Some(true);
        let light=routes.software_output(33)==Some(true);
        if reset!=self.reset || light!=self.light {self.version=self.version.wrapping_add(1);}
        if self.reset && !reset {panel.reset(); self.command=0;}
        self.reset=reset; self.light=light;
        let color=if routes.software_output(39)==Some(true){[255,0,0]}else{[0;3]};
        if color!=self.led[0] {self.led[0]=color; self.led_version=self.led_version.wrapping_add(1);}
        let clock=routes.software_output(36); let mosi=routes.software_output(35); let dc=routes.software_output(37);
        if !reset || routes.software_output(34)!=Some(false) || clock.is_none() || mosi.is_none() || dc.is_none() {
            self.shift=SpiShift::default(); return;
        }
        if self.shift.dc!=dc {self.shift.value=0;self.shift.bits=0;}
        if self.shift.clock==Some(false) && clock==Some(true) {
            self.shift.value=(self.shift.value<<1)|u8::from(mosi==Some(true)); self.shift.bits+=1;
            if self.shift.bits==8 {
                let data=dc==Some(true); let value=self.shift.value;
                // Shared DCS digital subset is RGB565 only. Do not render an
                // unsupported RGB666 stream as pairs of RGB565 bytes.
                if !data || self.command!=0x2c || panel.colmod&7==5 {
                    panel.dc=data; panel.byte(value);
                    if !data {self.command=value;}
                    self.bytes+=1; self.version=self.version.wrapping_add(1);
                }
                self.shift.bits=0; self.shift.value=0;
            }
        }
        self.shift.clock=clock; self.shift.dc=dc;
    }
}
impl BoardModel for LilygoS2 {
    fn name(&self)->&'static str {self.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {self.drive(routes);}
    fn named_pin(&self,id:&str)->Option<u8> {(id=="boot").then_some(0)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true)]}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {self.panel.as_ref().map(|_|(&self.led[..],self.led_version))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let panel=self.panel.as_ref()?; let mut frame=vec![0;135*240];
        if self.reset && self.light && panel.on && !panel.sleeping {
            // TFT_eSPI rotation 0: visible x=52..186,y=40..279 in 240x320 GRAM.
            // MADCTL applies in the controller; the physical glass stays fixed.
            for y in 0..240 {for x in 0..135 {
                let mut p=panel.gram[(y+40)*240+x+52];
                if panel.madctl&8==0 {p=(p&0x07e0)|((p&0xf800)>>11)|((p&31)<<11);}
                frame[y*135+x]=p;
            }}
        }
        Some((135,240,frame,self.version))
    }
    fn display_version(&self)->u64 {self.version}
    fn display_frames(&self)->u64 {self.panel.as_ref().map_or(0,|p|p.frames)}
    fn report(&self)->String {format!("[{}] BOOT0{}; SRAM diagnostic only; SD/I2C/LEDC/hardware-SPI/USB/radio unimplemented",self.name,if self.panel.is_some(){format!(", red LED39, GPIO software-SPI ST7789 135x240, {} accepted bytes",self.bytes)}else{String::new()})}
}
