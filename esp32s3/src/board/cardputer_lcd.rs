//! Shared ST7789V2 display wired identically on the original and Adv Cardputers.
use esp_soc::devices::DcsPanel;
use esp_soc::board::SpiPins;
pub(super) struct CardputerLcd {pub panel:DcsPanel,command:u8,read_at:usize,reset_high:bool,spi_seen:u64,spi_matched:u64,lcd_route:SpiPins}
impl CardputerLcd {
    pub fn new()->Self {Self{panel:DcsPanel::st7789(),command:0,read_at:0,reset_high:true,spi_seen:0,spi_matched:0,lcd_route:SpiPins::default()}}
    pub fn gpio(&mut self,changes:&[(u8,bool)]) {for &(pin,level) in changes {match pin {34=>self.panel.dc=level,33=>{if self.reset_high && !level {self.panel.reset();}self.reset_high=level;},_=>{}}}}
    pub fn spi(&mut self,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.spi_seen+=1;if pins.sclk&(1<<36)!=0 {self.lcd_route=pins;}
        if pins.sclk&(1<<36)!=0 && (pins.mosi&(1<<35)!=0 || pins.miso==Some(35)) && pins.cs&(1<<37)!=0 {self.spi_matched+=1;self.transfer(tx,rx_len)} else {vec![0xff;rx_len]}
    }
    pub(super) fn transfer(&mut self,tx:&[u8],rx_len:usize)->Vec<u8> {
        if !self.reset_high {return vec![0xff;rx_len];}
        for &byte in tx {if !self.panel.dc {self.command=byte;self.read_at=0;}self.panel.byte(byte);}
        // Sitronix ST7789V2 v1.0 §§9.1.3/9.1.46–48: default ID 85 85 52.
        // GP-SPI already accounts for the programmed dummy phase before these bytes.
        let data:&[u8]=match self.command {0x04=>&[0x85,0x85,0x52],0xda|0xdb=>&[0x85],0xdc=>&[0x52],_=>&[]};
        (0..rx_len).map(|_|{let value=data.get(self.read_at).copied().unwrap_or(0);self.read_at=self.read_at.saturating_add(1);value}).collect()
    }
    pub fn display(&self)->(u32,u32,Vec<u16>,u64) {
        let mut frame=Vec::with_capacity(240*135);for y in 0..135 {for x in 0..240 {let mut px=self.panel.gram[(40+x)*240+(52+134-y)];if !self.panel.on || self.panel.sleeping {px=0;}frame.push(px);}}
        (240,135,frame,self.version())
    }
    pub fn version(&self)->u64 {self.panel.pixels_written+self.panel.resets}
    pub fn report(&self)->String {format!("frames={} pixels={} madctl={:#x} colmod={:#x} on={} spi={}/{} lcd_route={:?} reset={} dc={}",self.panel.frames,self.panel.pixels_written,self.panel.madctl,self.panel.colmod,self.panel.on,self.spi_matched,self.spi_seen,self.lcd_route,self.reset_high,self.panel.dc)}
}
