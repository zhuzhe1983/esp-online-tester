//! Exact Waveshare LR1121 screen-board digital subsets. Pin and memory facts
//! come from the individual 2026-11-23 manufacturer snapshots retained with
//! the S3 source manifests. RF, fuel gauges and physical power-key behaviour
//! are deliberately absent; these are not interchangeable LoRa emulators.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins},pins::PinRoutes,
    devices::{Ws2812Chain,es7210::Es7210,es8311::Es8311,i2c_bitbang::I2cBitBang,
        oled_mono::{OledI2c,OledKind,OledMono},pcf85063::{Pcf85063,Pcf85063State},
        ssd16xx_mono::{Ssd16xxMono,Variant as EpaperVariant}}};
const HZ:u64=240_000_000;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Variant {Oled154,Epaper29}
impl Variant {
    pub fn name(self)->&'static str {match self {Self::Oled154=>"waveshare-s3-lr1121-oled154",Self::Epaper29=>"waveshare-s3-lr1121-epaper29"}}
    pub fn memory_mib(self)->(usize,usize) {(16,8)}
    fn buttons(self)->&'static [(&'static str,u8)] {match self {Self::Oled154=>&[("boot",0),("power",4),("menu",5),("ptt",6)],Self::Epaper29=>&[("boot",0),("key",1)]}}
}
pub fn variant(name:&str)->Option<Variant> {[Variant::Oled154,Variant::Epaper29].into_iter().find(|v|v.name()==name)}
pub fn make_board(name:&str)->Option<Board> {variant(name).map(|v|Box::new(WaveshareLr::new(v))as Board)}

pub struct WaveshareLr {
    variant:Variant,oled:Option<Arc<Mutex<OledMono>>>,epaper:Option<Ssd16xxMono>,
    rtc:Arc<Mutex<Pcf85063State>>,i2c:I2cBitBang,attached:bool,levels:[bool;2],edges:Vec<BoardEdge>,
    cycle:u64,power:bool,reset:bool,dc:Option<bool>,card_present:bool,
    chain:Ws2812Chain,led:[[u8;3];1],led_version:u64,
}
impl WaveshareLr {
    pub fn new(variant:Variant)->Self {
        Self{variant,oled:(variant==Variant::Oled154).then(||Arc::new(Mutex::new(OledMono::new(OledKind::Ssd1309,128,64,0)))),
            epaper:(variant==Variant::Epaper29).then(||Ssd16xxMono::new(EpaperVariant::Ssd1680_290,8,HZ/1000)),
            rtc:Arc::new(Mutex::new(Pcf85063State::new(HZ))),i2c:I2cBitBang::new(),attached:false,levels:[true;2],edges:Vec::new(),
            cycle:0,power:variant==Variant::Oled154,reset:false,dc:None,card_present:false,
            chain:Ws2812Chain::new(1).with_gpio_clock(HZ as u32),led:[[0;3]],led_version:0}
    }
    fn sync_led(&mut self) {let next=if self.power{self.chain.leds[0]}else{[0;3]};if self.led[0]!=next{self.led[0]=next;self.led_version+=1;}}
}
impl BoardModel for WaveshareLr {
    fn name(&self)->&'static str {self.variant.name()}
    fn uses_gpio_routes(&self)->bool {true}
    fn uses_gpio_waveform(&self)->bool {self.epaper.is_some()}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>) {
        self.advance_to(cycle);
        if let Some(panel)=&mut self.epaper {
            let power=r.software_output(38)==Some(true);let reset=r.software_output(9)==Some(true);
            if self.power&&!power {panel.power_loss();self.chain=Ws2812Chain::new(1).with_gpio_clock(HZ as u32);}
            if self.reset&&!reset {panel.reset();}self.power=power;self.reset=reset;self.dc=r.software_output(17);
            let level=r.software_output(21);self.chain.gpio_drive(cycle,power&&level.is_some(),level.unwrap_or(false));self.sync_led();
        }
        let scl=r.software_output(14).unwrap_or(true);let sda=self.i2c.drive(scl,r.software_output(13).unwrap_or(true));
        for (i,(pin,level))in[(13,sda),(14,scl)].into_iter().enumerate(){if level!=self.levels[i]{self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        if !self.attached {
            self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));
            if let Some(panel)=&self.oled {
                self.i2c.attach(0x3c,Box::new(OledI2c::new(panel.clone(),(13,14))));
                self.i2c.attach(0x18,Box::new(Es8311::new()));self.i2c.attach(0x40,Box::new(Es7210::new()));
            }
            self.attached=true;
        }
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((13,14))).into_iter().map(move|(a,d)|(bus,a,d))).collect()
    }
    fn uses_spi_pins(&self)->bool {self.epaper.is_some()}
    fn spi_transfer_pins(&mut self,_host:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        if let Some(panel)=&mut self.epaper {
            // EPD and LR1121 share SCK/MOSI. Simultaneous selection is not a
            // valid display-only transfer; unknown radio input never ACKs.
            if self.power&&self.reset&&p.sclk&(1<<40)!=0&&p.mosi&(1<<45)!=0
                &&p.cs&((1<<18)|(1<<42))==1<<18&&rx==0 {
                if let Some(dc)=self.dc {panel.write(dc,tx);}
            }
        }
        vec![255;rx]
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if self.epaper.is_some()&&self.power&&pin==21 {self.chain.from_bits(bits);self.sync_led();}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {self.epaper.as_ref().map(|_|(&self.led[..],self.led_version))}
    fn input_levels(&self)->Vec<(u8,bool)> {
        let mut pins:Vec<_>=self.variant.buttons().iter().map(|(_,pin)|(*pin,true)).collect();
        pins.extend([(13,self.levels[0]),(14,self.levels[1])]);if let Some(p)=&self.epaper {pins.push((8,p.busy()));}pins
    }
    fn named_pin(&self,name:&str)->Option<u8> {self.variant.buttons().iter().find_map(|(id,pin)|(*id==name).then_some(*pin))}
    fn advance_to(&mut self,cycle:u64) {
        self.cycle=cycle;self.rtc.lock().expect("LR RTC").advance_to(cycle);
        if let Some(p)=&mut self.epaper {p.advance_to(cycle);self.chain.advance_gpio(cycle);self.sync_led();}
    }
    fn next_deadline(&self)->Option<u64> {self.epaper.as_ref().and_then(|p|[p.next_deadline(),self.chain.gpio_deadline()].into_iter().flatten().min())}
    fn take_edges(&mut self)->Vec<BoardEdge> {let mut e=std::mem::take(&mut self.edges);if let Some(p)=&mut self.epaper {e.extend(p.take_edges());}e.sort_by_key(|v|v.cycle);e}
    fn sd_card_slot(&self)->Option<([u8;6],bool)> {Some(([12,10,11,255,255,255],self.card_present&&self.power))}
    fn set_sd_card_present(&mut self,present:bool) {self.card_present=present;}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        if let Some(p)=&self.oled {let p=p.lock().expect("LR OLED");return Some((128,64,p.frame(),p.version()));}
        self.epaper.as_ref().map(|p|(128,296,p.frame.clone(),p.version))
    }
    fn display_version(&self)->u64 {self.oled.as_ref().map_or_else(||self.epaper.as_ref().map_or(0,|p|p.version),|p|p.lock().expect("LR OLED").version())}
    fn display_frames(&self)->u64 {self.oled.as_ref().map_or_else(||self.epaper.as_ref().map_or(0,|p|p.frames),|p|p.lock().expect("LR OLED").frames())}
    fn display_push_hz(&self)->u64 {if self.epaper.is_some(){5}else{30}}
    fn report(&self)->String {
        format!("{} 16F8P I2C SDA13/SCL14 RTC51; SDMMC1bit CLK12 CMD10 D0=11 present={} supply={}; {}; LR1121 RF/IRQ/BUSY and fuel gauge absent; no analog charging, PWR long-press/poweroff, RTC alarm or audio PCM. ePaper uses only mono RAM24/26/refresh22+20 with deterministic digital BUSY, not physical timing/4-gray LUTs.",self.name(),self.card_present,self.power,
            self.oled.as_ref().map_or_else(||format!("SSD1680-compatible 128x296 SPI40/45 CS18 DC17 RESET9 BUSY8 sharedPower38 frames={}",self.display_frames()),|p|p.lock().expect("LR OLED").report()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d:&mut dyn I2cDevice,b:&[u8]) {assert!(d.start(false));for x in b {assert!(d.write(*x));}d.stop();}
    fn route(b:&mut WaveshareLr,power:bool,reset:bool,dc:Option<bool>) {
        let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();
        for(pin,v)in[(38,Some(power)),(9,Some(reset)),(17,dc)] {if let Some(v)=v {g.enable|=1<<pin;if v{g.out|=1<<pin;}m.write(4+4*pin,1<<12);}}
        b.gpio_waveform_at(b.cycle,esp_soc::pins::ChipPins::S3.routes(&g,&m));
    }
    fn pins()->SpiPins {SpiPins{sclk:1<<40,mosi:1<<45,cs:1<<18,..Default::default()}}
    fn cmd(b:&mut WaveshareLr,c:u8,data:&[u8]) {route(b,true,true,Some(false));b.spi_transfer_pins(2,pins(),&[c],0);route(b,true,true,Some(true));b.spi_transfer_pins(2,pins(),data,0);}
    #[test]
    fn oled_bytes_reach_only_source_controller_and_unknown_fuel_radio_stay_absent() {
        let mut b=WaveshareLr::new(Variant::Oled154);let mut ds=b.i2c_devices();
        assert_eq!(ds.len(),8);assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((13,14))));
        assert!(!ds.iter().any(|(_,a,_)|matches!(a,0x55|0x62)));
        let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x3c).unwrap().2.as_mut();
        write(d,&[0x00,0xaf,0x20,0,0x21,0,127,0x22,0,7]);write(d,&[0x40,1,0]);
        let(_,_,p,_)=b.display().unwrap();assert_eq!((p[0],p[1],p[128]),(65535,0,0));assert_eq!(b.display_frames(),1);
        assert!(!d.start(true));assert_eq!(b.spi_transfer(2,&[0,0],2),[255;2]);
        b.set_sd_card_present(true);assert_eq!(b.sd_card_slot(),Some(([12,10,11,255,255,255],true)));
        assert_eq!(b.named_pin("ptt"),Some(6));assert_eq!(b.named_pin("power"),Some(4));
    }
    #[test]
    fn epaper_pins_shared_selects_busy_and_supply_preserve_only_optical_image() {
        let mut b=WaveshareLr::new(Variant::Epaper29);cmd(&mut b,0x24,&[0x7f]);
        let bytes=b.epaper.as_ref().unwrap().ram_bytes_written;assert_eq!(bytes,1);
        for p in [SpiPins{cs:1<<42,..pins()},SpiPins{cs:(1<<18)|(1<<42),..pins()},SpiPins{sclk:1<<39,..pins()},SpiPins{mosi:1<<46,..pins()}] {b.spi_transfer_pins(2,p,&[0],0);}
        route(&mut b,true,true,None);b.spi_transfer_pins(2,pins(),&[0],0);assert_eq!(b.epaper.as_ref().unwrap().ram_bytes_written,bytes);
        cmd(&mut b,0x22,&[0xf7]);cmd(&mut b,0x20,&[]);let end=b.next_deadline().unwrap();
        assert!(b.input_levels().contains(&(8,true)));assert_eq!(b.display_frames(),0);b.advance_to(end);assert_eq!(b.display_frames(),1);
        let image=b.display().unwrap().2;assert_eq!((image[0],image[1]),(0,65535));assert!(b.take_edges().contains(&BoardEdge{cycle:end,pin:8,level:false}));
        b.set_sd_card_present(true);route(&mut b,false,true,Some(true));assert!(!b.sd_card_slot().unwrap().1);assert_eq!(b.display().unwrap().2,image);
        b.spi_transfer_pins(2,pins(),&[0],0);assert_eq!(b.epaper.as_ref().unwrap().ram_bytes_written,bytes);
    }
}
