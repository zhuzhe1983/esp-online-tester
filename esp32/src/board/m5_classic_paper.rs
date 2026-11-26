//! Exact M5Paper K049-B and current CoreInk K048/D67 digital subsets.
//! Sources and revision boundary: docs/platform/research/m5-classic-paper-runtime.json.
//! The Paper G23 is EPD power enable in M5EPD, not an IT8951 reset input.
//! CoreInk D67 is SSD1681; legacy M09's 0x10/0x13 protocol is NOT accepted.
use std::sync::{Arc,Mutex};
use esp_periph::{i2c::{I2cDevice,gt911::{Gt911,Gt911State}},sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins,VirtualCycle},pins::PinRoutes,devices::{it8951::It8951,ssd1681::Ssd1681,bm8563::{Bm8563,Bm8563State},i2c_bitbang::I2cBitBang}};
use crate::periph::CPU_HZ;

pub const PAPER:&str="m5paper-v11";
pub const PAPER_ORIGINAL:&str="m5paper-original";
pub const PAPER_COMM:&str="m5paper-comm";
pub const COREINK:&str="coreink-d67";
enum Panel {Paper(It8951),CoreInk(Ssd1681)}
pub fn make_board(name:&str)->Option<Board> {match name {PAPER|PAPER_ORIGINAL|PAPER_COMM=>Some(Box::new(ClassicPaper::with_name(name))),COREINK=>Some(Box::new(ClassicPaper::new(false))),_=>None}}
struct ClassicPaper {touch_irq_high:bool,name:&'static str,touch:Option<Arc<Mutex<Gt911State>>>,sd:Option<Box<SpiSdCard>>,panel:Panel,rtc:Arc<Mutex<Bm8563State>>,cycle:u64,dc:Option<bool>,enabled:bool,reset:bool,selected:bool,spi:u64,dropped:u64,hold:Option<bool>,edges:Vec<BoardEdge>}
impl ClassicPaper {
    fn with_name(name:&str)->Self {let mut board=Self::new(true);board.name=match name{PAPER_ORIGINAL=>PAPER_ORIGINAL,PAPER_COMM=>PAPER_COMM,_=>PAPER};board}
    fn new(paper:bool)->Self {Self{touch_irq_high:true,name:if paper{PAPER}else{COREINK},touch:paper.then(||Arc::new(Mutex::new(Gt911State::new(540,960,CPU_HZ/100)))),sd:None,panel:if paper{Panel::Paper(It8951::m5paper(27,CPU_HZ/1000))}else{Panel::CoreInk(Ssd1681::new(4,CPU_HZ/1000))},rtc:Arc::new(Mutex::new(Bm8563State::new(CPU_HZ))),cycle:0,dc:None,enabled:!paper,reset:true,selected:false,spi:0,dropped:0,hold:None,edges:Vec::new()}}
    fn paper(&self)->bool {matches!(self.panel,Panel::Paper(_))}
    fn sync_touch(&mut self){if let Some((pin,high))=self.touch_irq(){if high!=self.touch_irq_high{self.touch_irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin,level:high});}}}
    fn drain(&mut self) {self.edges.extend(match &mut self.panel{Panel::Paper(p)=>p.take_edges(),Panel::CoreInk(p)=>p.take_edges()});}
    fn busy(&self)->(u8,bool) {match &self.panel {Panel::Paper(p)=>(27,p.busy_high()),Panel::CoreInk(p)=>(4,p.busy())}}
}
impl BoardModel for ClassicPaper {
    fn name(&self)->&'static str {self.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {
        self.advance_to(cycle);if changes.iter().any(|&(pin,level)|pin==4&&level){if let Some(sd)=&mut self.sd{sd.deselect();}}
        if let Panel::Paper(p)=&mut self.panel {if changes.iter().any(|&(pin,level)|pin==15&&level){p.end_transaction();self.selected=false;}}
    }
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.advance_to(cycle);
        self.hold=routes.software_output(if self.paper(){2}else{12});
        match &mut self.panel {
            Panel::Paper(p)=>{
                let enabled=routes.software_output(23)==Some(true);
                if self.enabled&&!enabled {p.reset();}
                let selected=routes.software_output(15)==Some(false);
                if self.selected&&!selected {p.end_transaction();}
                self.enabled=enabled;self.selected=selected;
            },
            Panel::CoreInk(p)=>{self.dc=routes.software_output(15);let reset=routes.software_output(0)!=Some(false);if self.reset&&!reset{p.reset();}self.reset=reset;},
        }self.drain();
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if self.paper()&&pins.cs&((1<<15)|(1<<4))==0&&pins.sclk&(1<<14)!=0&&pins.mosi&(1<<12)!=0{if let Some(sd)=&mut self.sd{sd.transfer(false,tx,rx_len);}}
        if self.paper()&&pins.sclk&(1<<14)!=0&&pins.mosi&(1<<12)!=0&&pins.cs&((1<<15)|(1<<4))==1<<4 {if let Some(sd)=&mut self.sd{let result=sd.transfer(true,tx,rx_len);return if pins.miso==Some(13){result}else{vec![255;rx_len]};}}
        let result=match &mut self.panel {
            Panel::Paper(p) if self.enabled&&pins.sclk&(1<<14)!=0&&pins.mosi&(1<<12)!=0&&pins.cs&((1<<15)|(1<<4))==1<<15=>{
                self.selected=true;self.spi+=tx.len()as u64;let r=p.transfer(tx,rx_len);if pins.miso==Some(13){r}else{vec![255;rx_len]}
            },
            Panel::CoreInk(p) if self.reset&&pins.sclk&(1<<18)!=0&&pins.mosi&(1<<23)!=0&&pins.cs&(1<<9)!=0&&self.dc.is_some()=>{
                self.spi+=tx.len()as u64;if !p.busy(){p.write(self.dc.unwrap(),tx);}vec![255;rx_len]
            },
            _=>{self.dropped+=tx.len()as u64;vec![255;rx_len]}
        };self.drain();result
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let mut bus=I2cBitBang::new();let _=bus.attach(0x51,Box::new(Bm8563::new(self.rtc.clone())));
        if let Some(touch)=&self.touch{bus.attach(0x14,Box::new(Gt911::new(touch.clone())));}
        (0..2).flat_map(|n|bus.controller_devices(Some((21,22))).into_iter().map(move|(a,d)|(n,a,d))).collect()
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{if !self.paper(){return Err("CoreInk has no microSD socket".into());}let sd=SpiSdCard::new(image,config)?;let description=sd.describe();self.sd=Some(Box::new(sd));Ok(description)}
    fn touch(&mut self,x:u16,y:u16,down:bool){if let Some(touch)=&self.touch{let mut state=touch.lock().unwrap();state.update(x,y,down);state.advance_to(self.cycle);}self.sync_touch();}
    fn touch_irq(&self)->Option<(u8,bool)>{self.touch.as_ref().map(|s|(36,!s.lock().unwrap().ready))}
    fn next_deadline(&self)->Option<VirtualCycle> {match &self.panel {Panel::Paper(p)=>p.next_deadline(),Panel::CoreInk(p)=>p.next_deadline()}}
    fn advance_to(&mut self,cycle:VirtualCycle) {let cycle=cycle.max(self.cycle);self.cycle=cycle;self.rtc.lock().unwrap().advance_to(cycle);if let Some(touch)=&self.touch{touch.lock().unwrap().advance_to(cycle);}match &mut self.panel{Panel::Paper(p)=>p.advance_to(cycle),Panel::CoreInk(p)=>p.advance_to(cycle)}self.sync_touch();self.drain();}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut pins=vec![(37,true),(38,true),(39,true),(21,true),(22,true),self.busy()];if self.paper(){pins.push(self.touch_irq().unwrap());}else{pins.extend([(5,true),(27,true)]);}pins}
    fn named_pin(&self,name:&str)->Option<u8> {match (self.paper(),name){(true,"rotary-right")|(false,"dial-up")=>Some(37),(true,"rotary-press")|(false,"dial-press")=>Some(38),(true,"rotary-left")|(false,"dial-down")=>Some(39),(false,"button-user")=>Some(5),(false,"button-power")=>Some(27),_=>None}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some(match &self.panel {
        // Manufacturer M5GFX Paper rotation_offset=3: portrait physical glass.
        Panel::Paper(p)=>{let mut pixels=vec![0xffff;960*540];for y in 0..960{for x in 0..540{pixels[y*540+x]=p.frame[x*960+959-y];}}(540,960,pixels,p.version)},
        Panel::CoreInk(p)=>(200,200,p.frame.clone(),p.version)
    })}
    fn display_version(&self)->u64 {match &self.panel{Panel::Paper(p)=>p.version,Panel::CoreInk(p)=>p.version}}
    fn display_frames(&self)->u64 {match &self.panel{Panel::Paper(p)=>p.frames,Panel::CoreInk(p)=>p.frames}}
    fn display_push_hz(&self)->u64 {8}
    fn report(&self)->String {format!("[{}] exact digital e-paper/key/BM8563 calendar subset; frames={} SPI={} dropped={} BUSY={:?} EPD-power={} hold={:?}; GT911=single-contact; SD=SPI14/12/13/4; SHT30/EEPROM/audio/battery/hold power-off/RTC alarm/wake/analog LUT/factory boot unmodeled; CoreInk supports D67 only",self.name(),self.display_frames(),self.spi,self.dropped,self.busy(),self.enabled,self.hold)}
}
