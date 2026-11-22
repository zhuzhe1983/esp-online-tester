//! Exact SPI e-paper subsets for T-Deck Pro V1.0 and Mini E-Paper S3.
//! Source revisions, pin maps, protocol scope and uncertainties are recorded in
//! docs/platform/research/lilygo-epaper-profiles-2026-11-22.json. These are digital RAM/
//! refresh models: no optical waveform, grayscale, ghosting or factory-boot claim.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,tca8418::{Tca8418,Tca8418State}};
use esp_periph::sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::i2c_bitbang::I2cBitBang;
use esp_soc::pins::PinRoutes;
use crate::periph::CPU_HZ;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
enum Controller {Uc8253,Uc8175}
impl Controller {
    fn dimensions(self)->(usize,usize) {match self {Self::Uc8253=>(240,320),Self::Uc8175=>(80,128)}}
    fn window_bytes(self)->usize {match self {Self::Uc8253=>7,Self::Uc8175=>5}}
    fn refresh_ms(self,partial:bool)->u64 {match (self,partial) {(Self::Uc8253,false)=>1100,(Self::Uc8253,true)=>700,(Self::Uc8175,false)=>2000,(Self::Uc8175,true)=>700}}
}
#[derive(Clone,Copy)]
struct Window {x0:usize,x1:usize,y0:usize,y1:usize}
impl Window {fn bytes(self)->usize {(self.x1-self.x0+1)*(self.y1-self.y0+1)}}
struct Pending {at:u64,image:Option<Vec<u8>>}
struct Epaper {
    controller:Controller,current:Vec<u8>,previous:Vec<u8>,visible:Vec<u8>,
    command:Option<u8>,parameters:[u8;7],parameter_count:usize,cursor:usize,
    window:Option<Window>,partial:bool,configured:bool,powered:bool,sleeping:bool,
    pending:Option<Pending>,frames:u64,ram_bytes:u64,unsupported_commands:u64,resets:u64,
}
impl Epaper {
    fn new(controller:Controller)->Self {
        let (w,h)=controller.dimensions();let bytes=w*h/8;
        Self{controller,current:vec![255;bytes],previous:vec![255;bytes],visible:vec![255;bytes],command:None,parameters:[0;7],parameter_count:0,cursor:0,window:Some(Window{x0:0,x1:w/8-1,y0:0,y1:h-1}),partial:false,configured:false,powered:false,sleeping:false,pending:None,frames:0,ram_bytes:0,unsupported_commands:0,resets:0}
    }
    fn reset(&mut self,clear_ram:bool) {
        let (w,h)=self.controller.dimensions();self.command=None;self.parameter_count=0;self.cursor=0;self.window=Some(Window{x0:0,x1:w/8-1,y0:0,y1:h-1});
        self.partial=false;self.configured=false;self.powered=false;self.sleeping=false;self.pending=None;self.resets+=1;
        if clear_ram {self.current.fill(255);self.previous.fill(255);}
        // An e-paper image persists without power. Reset never invents an optical refresh.
    }
    fn busy_high(&self)->bool {self.pending.is_none()}
    fn next_deadline(&self)->Option<u64> {self.pending.as_ref().map(|p|p.at)}
    fn advance_to(&mut self,cycle:u64)->Option<u64> {
        if self.pending.as_ref().is_some_and(|p|p.at<=cycle) {
            let pending=self.pending.take().expect("due e-paper operation");
            if let Some(image)=pending.image {self.visible=image;self.frames+=1;}
            Some(pending.at)
        } else {None}
    }
    fn start_busy(&mut self,cycle:u64,milliseconds:u64,image:Option<Vec<u8>>) {self.pending=Some(Pending{at:cycle.saturating_add((CPU_HZ/1000).saturating_mul(milliseconds).max(1)),image});}
    fn area(&self)->Option<Window> {
        if self.partial {self.window} else {let (w,h)=self.controller.dimensions();Some(Window{x0:0,x1:w/8-1,y0:0,y1:h-1})}
    }
    fn command(&mut self,cycle:u64,command:u8) {
        if self.sleeping || self.pending.is_some() {return;}
        self.command=Some(command);self.parameter_count=0;self.cursor=0;
        match command {
            0x02=>{self.powered=false;self.start_busy(cycle,50,None);}
            0x04=>{self.powered=true;self.start_busy(cycle,50,None);}
            0x12=>{
                if !self.configured || !self.powered {return;}
                let Some(area)=self.area() else {return;};let (w,_)=self.controller.dimensions();
                let mut image=self.visible.clone();for y in area.y0..=area.y1 {let a=y*w/8+area.x0;let b=y*w/8+area.x1+1;image[a..b].copy_from_slice(&self.current[a..b]);}
                self.start_busy(cycle,self.controller.refresh_ms(self.partial),Some(image));
            }
            0x90=>self.window=None,
            0x91=>self.partial=true,
            0x92=>self.partial=false,
            0x00|0x07|0x10|0x13|0x50=>{},
            0xe0|0xe5 if self.controller==Controller::Uc8253=>{},
            // The Mini's official partial-update setup programs these analog
            // parameters. They do not implement optical waveforms in this model.
            0x01|0x06|0x23|0x24|0x2a|0x30|0x60|0x82|0xd2|0xe3 if self.controller==Controller::Uc8175=>{},
            _=>{self.unsupported_commands+=1;self.command=None;}
        }
    }
    fn data(&mut self,value:u8) {
        if self.sleeping || self.pending.is_some() {return;}
        let Some(command)=self.command else {return;};
        if command==0x10 || command==0x13 {
            if !self.configured {return;}
            if let Some(area)=self.area() {
                if self.cursor<area.bytes() {let (w,_)=self.controller.dimensions();let row_bytes=area.x1-area.x0+1;let index=(area.y0+self.cursor/row_bytes)*(w/8)+area.x0+self.cursor%row_bytes;
                    if command==0x13 {self.current[index]=value;}else{self.previous[index]=value;}self.ram_bytes+=1;self.cursor+=1;
                }
            }
            return;
        }
        if self.parameter_count<self.parameters.len() {self.parameters[self.parameter_count]=value;}
        self.parameter_count=self.parameter_count.saturating_add(1);
        match command {
            0x00=>{
                if self.controller==Controller::Uc8253 {
                    if self.parameter_count==1 && value==0x1e {self.reset(false);self.command=Some(0x00);self.parameters[0]=value;self.parameter_count=1;}
                    if self.parameter_count==2 {self.configured=self.parameters[..2]==[0x1f,0x0d];}
                } else if self.parameter_count==1 {self.configured=matches!(value,0x5f|0x6f);}
            }
            0x07 if self.parameter_count==1 && value==0xa5=>{self.sleeping=true;self.powered=false;}
            0x90 if self.parameter_count==self.controller.window_bytes()=>{
                let (w,h)=self.controller.dimensions();let p=self.parameters;
                let (y0,y1)=if self.controller==Controller::Uc8253 {(usize::from(u16::from_be_bytes([p[2],p[3]])),usize::from(u16::from_be_bytes([p[4],p[5]])))}else{(usize::from(p[2]),usize::from(p[3]))};
                let x0=usize::from(p[0])/8;let x1=usize::from(p[1])/8;
                self.window=(x0<=x1 && x1<w/8 && y0<=y1 && y1<h).then_some(Window{x0,x1,y0,y1});
            }
            _=>{}
        }
    }
    fn pixels(&self)->Vec<u16> {self.visible.iter().flat_map(|byte|(0..8).rev().map(move |bit|if byte&(1<<bit)==0 {0}else{0xffff})).collect()}
}

// CST328 single-contact report from T-Deck-Pro e3246046, hyn_cst3xx.c.
// Unknown identity/configuration registers NACK: no fabricated firmware IDs.
struct TouchState {reset_high:bool,awake:bool,pending:bool,packet:[u8;7]}
impl TouchState {
    fn new()->Self {Self{reset_high:true,awake:true,pending:false,packet:[0xab,0,0,0,0,0,0xab]}}
    fn irq_high(&self)->bool {!self.reset_high || !self.awake || !self.pending}
    fn reset(&mut self,high:bool) {if self.reset_high && !high {*self=Self::new();self.reset_high=false;}else{self.reset_high=high;}}
    fn update(&mut self,x:u16,y:u16,down:bool) {
        if !self.reset_high || !self.awake {return;}
        let x=x.min(239);let y=y.min(319);
        self.packet=[if down {6}else{0},(x>>4) as u8,(y>>4) as u8,((x&15)<<4 | y&15) as u8,if down {1}else{0},u8::from(down),0xab];self.pending=true;
    }
}
struct Cst328 {state:Arc<Mutex<TouchState>>,pointer:u16,pointer_bytes:u8}
impl Cst328 {fn new(state:Arc<Mutex<TouchState>>)->Self {Self{state,pointer:0,pointer_bytes:0}}}
impl I2cDevice for Cst328 {
    fn pins(&self)->Option<(u8,u8)> {Some((13,14))}
    fn start(&mut self,read:bool)->bool {
        if !self.state.lock().expect("CST328 state").reset_high {return false;}
        if read {(0xd000..=0xd006).contains(&self.pointer)}else{self.pointer_bytes=0;true}
    }
    fn write(&mut self,value:u8)->bool {
        let mut state=self.state.lock().expect("CST328 state");if !state.reset_high {return false;}
        if self.pointer_bytes<2 {
            self.pointer=if self.pointer_bytes==0 {u16::from(value)<<8}else{self.pointer|u16::from(value)};self.pointer_bytes+=1;
            if self.pointer_bytes==2 {match self.pointer {0xd109=>state.awake=true,0xd105=>{state.awake=false;state.pending=false;},_=>{}}}
            return true;
        }
        if self.pointer==0xd000 && value==0xab {state.pending=false;state.packet[0]=0xab;state.packet[5]=0;self.pointer=self.pointer.wrapping_add(1);true}else{false}
    }
    fn read(&mut self)->u8 {
        let state=self.state.lock().expect("CST328 state");let value=if state.reset_high && state.awake && (0xd000..=0xd006).contains(&self.pointer) {state.packet[usize::from(self.pointer-0xd000)]}else{0xff};
        self.pointer=self.pointer.wrapping_add(1);value
    }
}

#[derive(Clone,Copy)]
struct Wiring {name:&'static str,controller:Controller,sclk:u8,mosi:u8,cs:u8,dc:u8,busy:u8,reset:Option<u8>,enable:Option<u8>,sd:[u8;4],select_mask:u64}
const DECK:Wiring=Wiring{name:"lilygo-t-deck-pro",controller:Controller::Uc8253,sclk:36,mosi:33,cs:34,dc:35,busy:37,reset:None,enable:None,sd:[36,33,47,48],select_mask:(1<<34)|(1<<48)|(1<<3)};
const MINI:Wiring=Wiring{name:"lilygo-mini-epaper-s3",controller:Controller::Uc8175,sclk:14,mosi:15,cs:13,dc:12,busy:10,reset:Some(11),enable:Some(42),sd:[41,39,38,40],select_mask:1<<13};

pub struct LilygoEpaper {
    wiring:Wiring,panel:Epaper,cycle:u64,dc:Option<bool>,reset_high:bool,supply:bool,busy_high:bool,
    i2c:I2cBitBang,levels:[bool;2],touch:Arc<Mutex<TouchState>>,keyboard:Arc<Mutex<Tca8418State>>,irqs:[bool;2],
    card:Option<Box<SpiSdCard>>,card_present:bool,buttons:[bool;3],edges:Vec<BoardEdge>,gpio_events:u64,spi_seen:u64,spi_matched:u64,
}
impl LilygoEpaper {
    pub fn t_deck_pro()->Self {Self::new(DECK)}
    pub fn mini_epaper_s3()->Self {Self::new(MINI)}
    fn new(wiring:Wiring)->Self {Self{wiring,panel:Epaper::new(wiring.controller),cycle:0,dc:None,reset_high:true,supply:wiring.enable.is_none(),busy_high:true,i2c:I2cBitBang::new(),levels:[true;2],touch:Arc::new(Mutex::new(TouchState::new())),keyboard:Arc::new(Mutex::new(Tca8418State::new(CPU_HZ))),irqs:[true;2],card:None,card_present:true,buttons:[true;3],edges:Vec::new(),gpio_events:0,spi_seen:0,spi_matched:0}}
    fn is_deck(&self)->bool {self.wiring.controller==Controller::Uc8253}
    fn sync(&mut self) {
        let high=self.panel.busy_high();if high!=self.busy_high {self.busy_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:self.wiring.busy,level:high});}
        if self.is_deck() {let states=[self.touch.lock().expect("CST328 state").irq_high(),self.keyboard.lock().expect("TCA8418 state").irq_high()];for (index,high) in states.into_iter().enumerate() {if self.irqs[index]!=high {self.irqs[index]=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:[12,15][index],level:high});}}}
    }
}
impl BoardModel for LilygoEpaper {
    fn name(&self)->&'static str {self.wiring.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.advance_to(cycle);self.gpio_events+=changes.len() as u64;}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.advance_to(cycle);self.dc=routes.software_output(self.wiring.dc);
        let supply=self.wiring.enable.is_none_or(|pin|routes.software_output(pin)==Some(true));
        let reset_high=self.wiring.reset.is_none_or(|pin|routes.software_output(pin)!=Some(false));
        if (self.supply && !supply) || (self.reset_high && !reset_high) {self.panel.reset(true);}
        self.supply=supply;self.reset_high=reset_high;
        if self.is_deck() {
            self.touch.lock().expect("CST328 state").reset(routes.software_output(45)!=Some(false));
            let scl=routes.software_output(14)!=Some(false);let sda=self.i2c.drive(scl,routes.software_output(13)!=Some(false));
            for (index,(pin,level)) in [(13,sda),(14,scl)].into_iter().enumerate() {if self.levels[index]!=level {self.levels[index]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}}
        }
        self.sync();
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.spi_seen+=1;let w=self.wiring;let is_deck=self.is_deck();let selects=pins.cs&w.select_mask;
        let panel_route=pins.sclk&(1<<w.sclk)!=0 && pins.mosi&(1<<w.mosi)!=0;
        if panel_route && selects==1<<w.cs && self.supply && self.reset_high {
            if let Some(dc)=self.dc {self.spi_matched+=1;for &value in tx {if dc {self.panel.data(value);}else{self.panel.command(self.cycle,value);}}self.sync();}
        }
        let [sclk,mosi,miso,cs]=w.sd;
        if pins.sclk&(1<<sclk)!=0 && pins.mosi&(1<<mosi)!=0 && self.card_present {
            if let Some(card)=&mut self.card {
                let selected=pins.cs&(1<<cs)!=0 && (!is_deck || selects==1<<cs);
                let result=card.transfer(selected,tx,rx_len);if pins.miso==Some(miso) {return result;}
            }
        }
        vec![0xff;rx_len]
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(Box::new(card));Ok(description)}
    fn set_sd_card_present(&mut self,present:bool) {if self.card_present!=present {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_present=present;}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();if !self.is_deck() {return Vec::new();}
        self.i2c.attach(0x1a,Box::new(Cst328::new(self.touch.clone())));self.i2c.attach(Tca8418::ADDRESS,Box::new(Tca8418::new(self.keyboard.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((13,14))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn matrix_key_at(&mut self,cycle:VirtualCycle,row:u8,column:u8,pressed:bool)->bool {
        if !self.is_deck() || row>=4 || column>=10 || (row==3 && column<5) {return false;}
        self.advance_to(cycle);
        // Factory peri_keypad.cpp: event index -> row=index/10, column=9-index%10.
        // The five blank positions in its fourth row are not physical switches.
        self.keyboard.lock().expect("TCA8418 state").key(self.cycle,row,9-column,pressed)
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {if self.is_deck() {self.touch.lock().expect("CST328 state").update(x,y,down);self.sync();}}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.advance_to(cycle);self.touch(x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)> {self.is_deck().then(||(12,self.touch.lock().expect("CST328 state").irq_high()))}
    fn next_deadline(&self)->Option<VirtualCycle> {self.panel.next_deadline().into_iter().chain(if self.is_deck() {self.keyboard.lock().expect("TCA8418 state").next_deadline()}else{None}).min()}
    fn advance_to(&mut self,cycle:VirtualCycle) {
        let cycle=self.cycle.max(cycle);self.sync();
        if let Some(at)=self.panel.advance_to(cycle) {self.busy_high=true;self.edges.push(BoardEdge{cycle:at,pin:self.wiring.busy,level:true});}
        if self.is_deck() {for (at,level) in self.keyboard.lock().expect("TCA8418 state").advance_to(cycle) {self.irqs[1]=level;self.edges.push(BoardEdge{cycle:at,pin:15,level});}}
        self.cycle=cycle;self.sync();
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync();let mut edges=std::mem::take(&mut self.edges);edges.sort_by_key(|edge|edge.cycle);edges}
    fn input_levels(&self)->Vec<(u8,bool)> {
        let mut levels=vec![(0,self.buttons[0]),(self.wiring.busy,self.panel.busy_high())];
        if self.is_deck() {levels.extend([(13,self.levels[0]),(14,self.levels[1]),(12,self.touch.lock().expect("CST328 state").irq_high()),(15,self.keyboard.lock().expect("TCA8418 state").irq_high())]);}
        else {levels.extend([(3,self.buttons[1]),(4,self.buttons[2])]);}levels
    }
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"g0"=>Some(0),"right"|"btn1" if !self.is_deck()=>Some(3),"left"|"btn2" if !self.is_deck()=>Some(4),_=>None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        let Some(pin)=self.named_pin(id) else {return false;};self.advance_to(cycle);let index=match pin {3=>1,4=>2,_=>0};let level=!pressed;
        if self.buttons[index]!=level {self.buttons[index]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}true
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let (w,h)=self.wiring.controller.dimensions();Some((w as u32,h as u32,self.panel.pixels(),self.panel.frames))}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn display_version(&self)->u64 {self.panel.frames}
    fn display_push_hz(&self)->u64 {8}
    fn report(&self)->String {format!("[{}] {:?} digital-monochrome frames={} RAM-bytes={} spi={}/{} busy={} powered={} reset-high={} unsupported-commands={} SD-SPI={} touch={} keyboard={} analog-EPD/PMIC/RF/GNSS/IMU/audio=unmodeled factory-boot=unverified",self.name(),self.wiring.controller,self.panel.frames,self.panel.ram_bytes,self.spi_matched,self.spi_seen,!self.panel.busy_high(),self.supply,self.reset_high,self.panel.unsupported_commands,self.card.is_some()&&self.card_present,if self.is_deck(){"CST328-report-only"}else{"absent"},if self.is_deck(){"TCA8418"}else{"GPIO"})}
}
