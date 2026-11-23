//! Exact classic ESP32 e-paper wiring, with retained optical frames.
//! Sources/revision boundaries: lilygo-classic-additional-profiles-2026-11-22.json.
//! The Mini, Wrist and T5 use different controllers, BUSY levels and supply pins.
use esp_soc::{board::{Board,BoardModel,BoardEdge,SpiPins,VirtualCycle},pins::PinRoutes};
use esp_soc::devices::{uc81xx::{Controller,Epaper},ssd1680::Ssd1680,ssd1681::Ssd1681};
use esp_periph::sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard};
use crate::periph::CPU_HZ;
#[path="lilygo_serial_epaper.rs"] mod serial;
use serial::{SerialPaper,Kind as SerialKind};

#[derive(Clone,Copy)] enum Kind {Mini,Wrist,T5,Gray213,T5s}
#[derive(Clone,Copy)] struct Wiring {id:&'static str,kind:Kind,spi:[u8;3],dc:u8,reset:u8,busy:u8,enable:Option<u8>,buttons:&'static[(&'static str,u8)],sd:Option<[u8;4]>}
const MINI:Wiring=Wiring{id:"lilygo-mini-e-paper-core",kind:Kind::Mini,spi:[22,21,5],dc:19,reset:4,busy:34,enable:Some(27),buttons:&[("boot",0),("button-1",36),("button-2",39)],sd:Some([14,15,2,13])};
const WRIST:Wiring=Wiring{id:"lilygo-t-wrist-e-paper-1-54-inch-display",kind:Kind::Wrist,spi:[14,13,15],dc:2,reset:17,busy:16,enable:Some(5),buttons:&[("user",35)],sd:None};
const T5:Wiring=Wiring{id:"lilygo-t5-v2-3-1",kind:Kind::T5,spi:[18,23,5],dc:17,reset:16,busy:4,enable:None,buttons:&[("user",39)],sd:Some([14,15,2,13])};
const GRAY:Wiring=Wiring{id:"lilygo-t5-2-13inch-e-paper",kind:Kind::Gray213,..T5};
const T5S:Wiring=Wiring{id:"lilygo-t5s-2-7inch-e-paper",kind:Kind::T5s,buttons:&[("button-1",37),("button-2",38),("button-3",39)],..T5};
enum Panel {Mini(Epaper),Wrist(Ssd1681),T5(Ssd1680),Serial(SerialPaper)}
impl Panel {
    fn new(w:Wiring)->Self {match w.kind {Kind::Mini=>Self::Mini(Epaper::new(Controller::Uc8175,CPU_HZ)),Kind::Wrist=>Self::Wrist(Ssd1681::new(w.busy,CPU_HZ/1000)),Kind::T5=>Self::T5(Ssd1680::new(w.busy,CPU_HZ)),Kind::Gray213=>Self::Serial(SerialPaper::new(SerialKind::Gray213,CPU_HZ)),Kind::T5s=>Self::Serial(SerialPaper::new(SerialKind::Il91874,CPU_HZ))}}
    fn busy(&self)->bool {match self {Self::Mini(p)=>!p.busy_high(),Self::Wrist(p)=>p.busy(),Self::T5(p)=>p.busy(),Self::Serial(p)=>p.busy()}}
    fn next_deadline(&self)->Option<u64> {match self {Self::Mini(p)=>p.next_deadline(),Self::Wrist(p)=>p.next_deadline(),Self::T5(p)=>p.next_deadline(),Self::Serial(p)=>p.next_deadline()}}
    fn advance_to(&mut self,cycle:u64) {match self {Self::Mini(p)=>{p.advance_to(cycle);},Self::Wrist(p)=>{p.advance_to(cycle);p.take_edges();},Self::T5(p)=>{p.advance_to(cycle);p.take_edges();},Self::Serial(p)=>p.advance_to(cycle)}}
    fn reset(&mut self,cycle:u64,clear_ram:bool) {match self {
        Self::Mini(p)=>p.reset(clear_ram),Self::T5(p)=>{p.reset(clear_ram);p.take_edges();},Self::Serial(p)=>p.reset(clear_ram),
        Self::Wrist(p)=>{p.reset();p.take_edges();if clear_ram {let frame=p.frame.clone();let frames=p.frames;let version=p.version;let bytes=p.ram_bytes_written;*p=Ssd1681::new(WRIST.busy,CPU_HZ/1000);p.frame=frame;p.frames=frames;p.version=version;p.ram_bytes_written=bytes;p.advance_to(cycle);}}
    }}
    fn write(&mut self,cycle:u64,dc:bool,bytes:&[u8]) {if self.busy(){return;}match self {Self::Mini(p)=>{for &b in bytes {if dc{p.data(b);}else{p.command(cycle,b);}}},Self::Wrist(p)=>{p.write(dc,bytes);p.take_edges();},Self::T5(p)=>{p.write(dc,bytes);p.take_edges();},Self::Serial(p)=>p.write(cycle,dc,bytes)}}
    fn frame(&self)->(u32,u32,Vec<u16>,u64) {match self {Self::Mini(p)=>(80,128,p.pixels(),p.frames),Self::Wrist(p)=>(200,200,p.frame.clone(),p.version),Self::T5(p)=>(122,250,p.frame.clone(),p.version),Self::Serial(p)=>{let(w,h)=p.dimensions();(w,h,p.pixels(),p.frames)}}}
    fn frames(&self)->u64 {match self {Self::Mini(p)=>p.frames,Self::Wrist(p)=>p.frames,Self::T5(p)=>p.frames,Self::Serial(p)=>p.frames}}
}
pub fn make_board(id:&str)->Option<Board> {[MINI,WRIST,T5,GRAY,T5S].into_iter().find(|w|w.id==id).map(|w|Box::new(ClassicEpaper::new(w))as Board)}
struct ClassicEpaper {w:Wiring,panel:Panel,cycle:u64,dc:Option<bool>,supply:bool,reset:bool,busy_level:bool,edges:Vec<BoardEdge>,card:Option<Box<SpiSdCard>>,card_present:bool,spi_seen:u64,spi_matched:u64}
impl ClassicEpaper {
    fn new(w:Wiring)->Self {Self{w,panel:Panel::new(w),cycle:0,dc:None,supply:w.enable.is_none(),reset:true,busy_level:matches!(w.kind,Kind::Mini|Kind::Gray213|Kind::T5s),edges:Vec::new(),card:None,card_present:true,spi_seen:0,spi_matched:0}}
    fn busy_level(&self)->bool {self.panel.busy()!=matches!(self.w.kind,Kind::Mini|Kind::Gray213|Kind::T5s)}
    fn sync(&mut self,cycle:u64) {let level=self.busy_level();if level!=self.busy_level {self.busy_level=level;self.edges.push(BoardEdge{cycle,pin:self.w.busy,level});}}
}
impl BoardModel for ClassicEpaper {
    fn name(&self)->&'static str {self.w.id}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,_changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.advance_to(cycle);}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.advance_to(cycle);self.dc=routes.software_output(self.w.dc);
        let supply=self.w.enable.is_none_or(|pin|routes.software_output(pin)==Some(true));let reset=routes.software_output(self.w.reset)!=Some(false);
        if self.supply&&!supply {self.panel.reset(self.cycle,true);}else if self.reset&&!reset {self.panel.reset(self.cycle,false);}
        self.supply=supply;self.reset=reset;self.sync(self.cycle);
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.spi_seen+=1;let[clk,mosi,cs]=self.w.spi;let selection=(1<<cs)|self.w.sd.map_or(0,|w|1<<w[3]);
        if pins.sclk&(1<<clk)!=0&&pins.mosi&(1<<mosi)!=0&&pins.cs&selection==1<<cs&&self.supply&&self.reset {
            if let Some(dc)=self.dc {self.spi_matched+=1;self.panel.write(self.cycle,dc,tx);self.sync(self.cycle);}
        }
        if let (Some([clk,mosi,miso,cs]),Some(card))=(self.w.sd,&mut self.card) {
            if self.card_present&&pins.sclk&(1<<clk)!=0&&pins.mosi&(1<<mosi)!=0 {
                let selected=pins.cs&selection==1<<cs;let result=card.transfer(selected,tx,rx_len);if pins.miso==Some(miso){return result;}
            }
        }vec![255;rx_len]
    }
    fn next_deadline(&self)->Option<VirtualCycle> {self.panel.next_deadline()}
    fn advance_to(&mut self,cycle:VirtualCycle) {let cycle=self.cycle.max(cycle);let due=self.panel.next_deadline().filter(|&d|d<=cycle);self.panel.advance_to(cycle);self.sync(due.unwrap_or(cycle));self.cycle=cycle;}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {self.w.buttons.iter().map(|&(_,pin)|(pin,true)).chain([(self.w.busy,self.busy_level())]).collect()}
    fn named_pin(&self,name:&str)->Option<u8> {self.w.buttons.iter().find(|&&(id,_)|id==name).map(|&(_,pin)|pin)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some(self.panel.frame())}
    fn display_frames(&self)->u64 {self.panel.frames()}
    fn display_version(&self)->u64 {self.panel.frames()}
    fn display_push_hz(&self)->u64 {8}
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {if self.w.sd.is_none(){return Err("No sourced TF socket on this selected board".into());}let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(Box::new(card));Ok(description)}
    fn set_sd_card_present(&mut self,present:bool) {if self.card_present!=present{if let Some(card)=&mut self.card {card.power_cycle();}}self.card_present=present;}
    fn report(&self)->String {format!("[{}] source-backed e-paper digital subset; frames={} busy={} supply={} reset={} SPI={}/{} SD-SPI={}; analog waveform/ghosting/RTC/IMU/audio/factory-boot=unmodeled",self.w.id,self.panel.frames(),self.panel.busy(),self.supply,self.reset,self.spi_matched,self.spi_seen,self.card.is_some()&&self.card_present)}
}
