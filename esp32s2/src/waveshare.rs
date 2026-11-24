//! Waveshare ESP32-S2-Pico and ESP32-S2-LCD-0.96, 4MiB Flash/8MiB PSRAM.
//! Official ESP32-S2-Pico_Code LCD096.{h,cpp}, Pico/LCD schematics and
//! product pages are retained under docs/research/waveshare-classic-s2-2026-11-24.
//! S2 GPIO software-SPI path, not S3 hardware registers or a factory boot claim.
//! LCD096 driver: ST7735, RGB565, MADCTL A8, CASET+1/RASET+26; no touch.
use esp_soc::{Board,BoardModel,devices::DcsPanel,pins::PinRoutes};
pub const PICO:&str="waveshare-s2-pico";
pub const LCD:&str="waveshare-s2-lcd096";
pub fn memory_mib(name:&str)->Option<(usize,usize)> {matches!(name,PICO|LCD).then_some((4,8))}
pub fn make_board(name:&str)->Option<Board> {let name=match name {PICO=>PICO,LCD=>LCD,_=>return None};Some(Box::new(Pico{name,panel:(name==LCD).then(DcsPanel::st7735),shift:Default::default(),reset:false,light:false,command:0,version:0,bytes:0}))}
#[derive(Default)]
struct Shift {clock:Option<bool>,dc:Option<bool>,value:u8,bits:u8}
struct Pico {name:&'static str,panel:Option<DcsPanel>,shift:Shift,reset:bool,light:bool,command:u8,version:u64,bytes:u64}
impl BoardModel for Pico {
    fn name(&self)->&'static str {self.name}
    fn uses_gpio_routes(&self)->bool {self.panel.is_some()}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {
        let Some(panel)=&mut self.panel else{return;};
        let reset=routes.software_output(21)==Some(true);let light=routes.software_output(45)==Some(true);
        if (reset,light)!=(self.reset,self.light) {self.version=self.version.wrapping_add(1);}
        if self.reset&&!reset {panel.reset();self.command=0;}
        self.reset=reset;self.light=light;
        let clock=routes.software_output(10);let mosi=routes.software_output(11);let dc=routes.software_output(18);
        if !reset||routes.software_output(9)!=Some(false)||clock.is_none()||mosi.is_none()||dc.is_none() {self.shift=Shift::default();return;}
        if self.shift.dc!=dc {self.shift.value=0;self.shift.bits=0;}
        if self.shift.clock==Some(false)&&clock==Some(true) {
            self.shift.value=(self.shift.value<<1)|u8::from(mosi==Some(true));self.shift.bits+=1;
            if self.shift.bits==8 {
                let data=dc==Some(true);let value=self.shift.value;
                if !data||self.command!=0x2c||panel.colmod&7==5 {panel.dc=data;panel.byte(value);if !data {self.command=value;}self.bytes+=1;self.version=self.version.wrapping_add(1);}
                self.shift.bits=0;self.shift.value=0;
            }
        }
        self.shift.clock=clock;self.shift.dc=dc;
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {(id=="boot").then_some(0)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let p=self.panel.as_ref()?;let mut pixels=vec![0;160*80];
        if self.reset&&self.light&&p.on&&!p.sleeping {
            // Physical ST7735 GRAM cols26..105,rows1..160, viewed landscape.
            // A8 maps source driver (x+1,y+26) to physical(y+26,160-x).
            for y in 0..80 {for x in 0..160 {let mut v=p.gram[(160-x)*132+26+y];if p.madctl&8==0 {v=(v&0x07e0)|((v&0xf800)>>11)|((v&31)<<11);}pixels[y*160+x]=v;}}
        }
        Some((160,80,pixels,self.version))
    }
    fn display_version(&self)->u64 {self.version}
    fn display_frames(&self)->u64 {self.panel.as_ref().map_or(0,|p|p.frames)}
    fn report(&self)->String {format!("[{}]4MiB Flash staging/8MiB data-PSRAM; BOOT0;{}; no S3 alias, ROM/USB/IDF boot/hardware-SPI/LEDC/radio/charger unimplemented",self.name,if self.panel.is_some(){format!("ST7735 160x80 GPIO-SPI10/11/9 DC18 RESET21 BL45; accepted{}bytes; gamma/inversion analog not calibrated",self.bytes)}else{"no onboard display; Pico RGB output not implemented".into()})}
}
