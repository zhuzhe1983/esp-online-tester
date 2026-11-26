//! U120 internal ESP32-PICO controller (exact package suffix unverified),
//! NOT an I2C display attached to its own ESP32.
//! M5Unit-LCD-Internal-FW 8c123afa3e9c command_processor.cpp: SPI13/15,
//! CS5 DC23 RST18 BL4 active-low, ST7789 135x240 at RAM52/40; external SDA21/SCL22
//! are the ESP32's slave interface (0x3e), not a pretend local I2C device.
use esp_soc::{BoardModel,board::SpiPins,devices::DcsPanel};
pub struct UnitLcd {panel:DcsPanel,reset:bool,brightness:u32,changes:u64,accepted:u64,dropped:u64}
impl UnitLcd {pub fn new()->Self {Self{panel:DcsPanel::st7789(),reset:true,brightness:0,changes:0,accepted:0,dropped:0}}}
impl Default for UnitLcd {fn default()->Self {Self::new()}}
impl BoardModel for UnitLcd {
    fn name(&self)->&'static str {"unit-lcd"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {for &(pin,level) in changes {match pin {
        23=>self.panel.dc=level,
        18=>{if self.reset && !level {self.panel.reset();self.changes=self.changes.wrapping_add(1);}self.reset=level;},
        4=>{let brightness=if level{0}else{65535};if self.brightness!=brightness {self.brightness=brightness;self.changes=self.changes.wrapping_add(1);}},_=>{}
    }}}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        if self.reset && p.sclk&(1<<13)!=0 && p.mosi&(1<<15)!=0 && p.cs&(1<<5)!=0 {
            let before=(self.panel.on,self.panel.sleeping,self.panel.madctl,self.panel.resets);
            for &b in tx {self.panel.byte(b);}self.accepted+=tx.len()as u64;
            if before!=(self.panel.on,self.panel.sleeping,self.panel.madctl,self.panel.resets) {self.changes=self.changes.wrapping_add(1);}
        } else {self.dropped+=tx.len()as u64;}vec![255;rx]
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {if let Some((_,duty))=pwm(4) {let brightness=65535-duty.min(65535);if self.brightness!=brightness {self.brightness=brightness;self.changes=self.changes.wrapping_add(1);}}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let mut pixels=super::tdisplay::lcd_visible(&self.panel);if self.brightness!=65535 {for p in &mut pixels {let r=(u32::from(*p>>11)*self.brightness/65535)as u16;let g=(u32::from((*p>>5)&63)*self.brightness/65535)as u16;let b=(u32::from(*p&31)*self.brightness/65535)as u16;*p=(r<<11)|(g<<5)|b;}}
        Some((135,240,pixels,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.changes)}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[unit-lcd] U120 ST7789 pixels={} frames={} spi={} dropped={} BL={}; external SDA21/SCL22 I2C slave0x3e requires guest slave transport",self.panel.pixels_written,self.panel.frames,self.accepted,self.dropped,self.brightness)}
}
