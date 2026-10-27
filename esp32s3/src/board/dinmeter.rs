//! Original M5Stack DinMeter K134, not K134-V11. DIN_Meter_v1.0 schematic,
//! M5DinMeter-UserDemo d2fdfe44 and M5GFX 65c816a3: LCD DC4/MOSI5/CLK6/CS7/RST8,
//! encoder A41/B40 and press42. USB-powered digital subset; PWM/RTC/audio omitted.
use esp_soc::board::{BoardModel,SpiPins};
use esp_soc::devices::DcsPanel;

pub struct DinMeter {panel:DcsPanel,command:u8,read_at:usize,reset_high:bool,display_changes:u64,gpio_events:u64}
impl DinMeter {pub fn new()->Self {Self{panel:DcsPanel::st7789(),command:0,read_at:0,reset_high:true,display_changes:0,gpio_events:0}}}
impl Default for DinMeter {fn default()->Self {Self::new()}}
impl BoardModel for DinMeter {
    fn name(&self)->&'static str {"dinmeter"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {
        self.gpio_events+=changes.len() as u64;
        for &(pin,high) in changes {match pin {4=>self.panel.dc=high,8=>{if self.reset_high && !high {self.panel.reset();self.command=0;self.read_at=0;self.display_changes+=1;}self.reset_high=high;},_=>{}}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if !self.reset_high || pins.sclk&(1<<6)==0 || pins.cs&(1<<7)==0 || (pins.mosi&(1<<5)==0 && pins.miso!=Some(5)) {return vec![0xff;rx_len];}
        for &b in tx {if !self.panel.dc {self.command=b;self.read_at=0;self.display_changes+=1;}self.panel.byte(b);}
        // ST7789V2 v1.0 RDDID/ID1/ID2/ID3 default bytes; SPI dummy phase is
        // consumed by GP-SPI. These identify the panel, never the board revision.
        let data:&[u8]=match self.command {0x04=>&[0x85,0x85,0x52],0xda|0xdb=>&[0x85],0xdc=>&[0x52],_=>&[]};
        (0..rx_len).map(|_|{let value=data.get(self.read_at).copied().unwrap_or(0);self.read_at=self.read_at.saturating_add(1);value}).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(42,true),(40,true),(41,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"btnA"|"button"|"sw"|"encoder-button"=>Some(42),"boot"=>Some(0),"clk"=>Some(41),"dt"=>Some(40),_=>None}}
    fn encoder(&self)->Option<(u8,u8)> {Some((41,40))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        // Fixed glass orientation, independent of guest MADCTL. Official factory
        // setRotation(3) maps (x,y) to GRAM column52+y,row279-x. M5GFX's newer
        // offset_rotation=2 reaches the same physical landscape with rotation1.
        let mut frame=vec![0;240*135];
        if self.panel.on && !self.panel.sleeping {for y in 0..135 {for x in 0..240 {frame[y*240+x]=self.panel.gram[(279-x)*240+52+y];}}}
        Some((240,135,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[dinmeter] ST7789V2 frames={} pixels={} madctl={:#x} encoder=A41/B40; PWM/RTC/audio/power unmodeled",self.panel.frames,self.panel.pixels_written,self.panel.madctl)}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<6,mosi:1<<5,cs:1<<7,..Default::default()}}
    fn cmd(b:&mut DinMeter,c:u8,data:&[u8]) {b.gpio_changes(&[(4,false)]);b.spi_transfer_pins(2,pins(),&[c],0);b.gpio_changes(&[(4,true)]);b.spi_transfer_pins(2,pins(),data,0);}
    #[test]
    fn source_routed_id_and_input_pins() {
        let mut b=DinMeter::new();assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0x85,0x85,0x52]);
        assert_eq!(b.spi_transfer_pins(2,SpiPins{sclk:1<<36,..pins()},&[4],3),[0xff;3]);assert_eq!(b.spi_transfer_pins(2,SpiPins{cs:1<<37,..pins()},&[4],3),[0xff;3]);
        assert_eq!(b.encoder(),Some((41,40)));assert_eq!(b.named_pin("encoder-button"),Some(42));assert!(b.input_levels().contains(&(42,true)));assert!(b.i2c_devices().is_empty(),"unmodeled RTC must NACK");
    }
    #[test]
    fn factory_landscape_four_corners_and_display_gates() {
        let mut b=DinMeter::new();cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0xb0]);
        for (x,y,color) in [(0u16,0u16,0xf800u16),(239,0,0x07e0),(0,134,0x001f),(239,134,0xffff)] {
            let x=(40+x).to_be_bytes();let y=(52+y).to_be_bytes();cmd(&mut b,0x2a,&[x[0],x[1],x[0],x[1]]);cmd(&mut b,0x2b,&[y[0],y[1],y[0],y[1]]);cmd(&mut b,0x2c,&color.to_be_bytes());
        }
        let frame=b.display().unwrap();assert_eq!((frame.0,frame.1),(240,135));assert_eq!([frame.2[0],frame.2[239],frame.2[134*240],frame.2[135*240-1]],[0xf800,0x07e0,0x001f,0xffff]);
        let version=b.display_version();cmd(&mut b,0x28,&[]);assert!(b.display_version()>version);assert!(b.display().unwrap().2.iter().all(|p|*p==0));cmd(&mut b,0x29,&[]);cmd(&mut b,0x10,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        b.gpio_changes(&[(8,false)]);assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0xff;3]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
    }
}
