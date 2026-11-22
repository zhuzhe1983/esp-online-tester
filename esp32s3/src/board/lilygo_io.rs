//! Exact LilyGO communication/IO boards, ordered by the dated source audit.
//! Sources, represented revisions and unresolved BOM conflicts are retained in
//! docs/platform/research/lilygo-s3-io-profiles-2026-11-22.json.
//! No modem, Ethernet, camera capture, RS232/485/CAN or analog power emulation.
use std::sync::{Arc,Mutex};
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins},pins::PinRoutes,devices::{DcsPanel,Ws2812Chain,oled_mono::{OledKind,OledMono,OledI2c}}};

#[derive(Clone,Copy,PartialEq,Eq)]
enum Product {Connect,EthElite,Energy,Sim7670,Rs,EthLite,Relay,Sim7080,Camera,Simcam,Keyboard,Micro,Panel,CameraPlus,Vending,Track,Fpga,T7}

/// Four APA102 LED frames. Start=32 zero bits, LED=111/brightness/B/G/R,
/// followed by >=N/2 clocks. Pin-level propagation is collapsed at frame end;
/// oscillator/PWM timing and PCB spatial LED order are not calibrated.
struct Apa102 {zeroes:u8,collecting:bool,word:u32,bits:u8,index:usize,tail:u8,pending:[[u8;3];4],leds:[[u8;3];4],updates:u64,clock:Option<bool>}
impl Apa102 {
    fn new()->Self {Self{zeroes:0,collecting:false,word:0,bits:0,index:0,tail:0,pending:[[0;3];4],leds:[[0;3];4],updates:0,clock:None}}
    fn bit(&mut self,high:bool) {
        self.zeroes=if high {0}else{self.zeroes.saturating_add(1)};
        if self.zeroes>=32 {self.collecting=true;self.word=0;self.bits=0;self.index=0;self.tail=0;return;}
        if !self.collecting {return;}
        if self.index==4 {self.tail+=1;if self.tail==2 {if self.leds!=self.pending {self.leds=self.pending;self.updates=self.updates.wrapping_add(1);}self.collecting=false;}return;}
        self.word=(self.word<<1)|u32::from(high);self.bits+=1;
        if self.bits==32 {
            if self.word>>29!=7 {self.collecting=false;return;}
            let brightness=(self.word>>24)&31;let scale=|c:u32|((c&255)*brightness/31) as u8;
            self.pending[self.index]=[scale(self.word),scale(self.word>>8),scale(self.word>>16)];
            self.index+=1;self.word=0;self.bits=0;
        }
    }
    fn gpio(&mut self,clock:Option<bool>,data:Option<bool>) {
        if clock.is_none() || data.is_none() {self.collecting=false;self.zeroes=0;}
        if self.clock==Some(false) && clock==Some(true) {if let Some(data)=data {self.bit(data);}}
        self.clock=clock;
    }
    fn bytes(&mut self,bytes:&[u8]) {for &b in bytes {for bit in (0..8).rev() {self.bit(b&(1<<bit)!=0);}}}
}

/// T-Relay S3 V1.1: /OE4, SHCP5, STCP6, DS7; /SRCLR tied high.
/// QA..QF drive six relay coils, QG/QH the green/red LEDs. Only digital
/// coil drive is represented; contacts, load/current and switching delay are absent.
struct Shift595 {shift:u8,latch:u8,clock:Option<bool>,strobe:Option<bool>,drive:u8,leds:[[u8;3];2],updates:u64}
impl Shift595 {
    fn new()->Self {Self{shift:0,latch:0,clock:None,strobe:None,drive:0,leds:[[0;3];2],updates:0}}
    fn gpio(&mut self,routes:&PinRoutes<'_>) {
        let clock=routes.software_output(5);let strobe=routes.software_output(6);
        if self.clock==Some(false) && clock==Some(true) {if let Some(data)=routes.software_output(7) {self.shift=(self.shift<<1)|u8::from(data);}}
        if self.strobe==Some(false) && strobe==Some(true) {self.latch=self.shift;}
        self.clock=clock;self.strobe=strobe;
        let drive=if routes.software_output(4)==Some(false) {self.latch}else{0};
        let leds=[if drive&64!=0 {[0,255,0]}else{[0;3]},if drive&128!=0 {[255,0,0]}else{[0;3]}];
        if self.leds!=leds {self.leds=leds;self.updates=self.updates.wrapping_add(1);}self.drive=drive;
    }
}

/// AXP2101 explicitly bounded register subset. Board-programmed EFUSE rail
/// defaults are unknown: ALDO3 stays unavailable until a guest writes its
/// enable and supported 3.3V setting. This is a diagnostic starting policy,
/// not a claim about factory PMIC defaults. No invented battery/USB readings.
struct AxpState {adc:u8,ldos:u8,aldo3:u8,enable_written:bool,voltage_written:bool}
impl AxpState {
    fn new()->Self {Self{adc:3,ldos:0,aldo3:0,enable_written:false,voltage_written:false}}
    fn sd_power(&self)->bool {self.enable_written&&self.voltage_written&&self.ldos&4!=0&&self.aldo3==28}
    fn readable(reg:u8)->bool {matches!(reg,0x03|0x30|0x90|0x94)}
    fn read(&self,reg:u8)->u8 {match reg {0x03=>0x4a,0x30=>self.adc,0x90=>self.ldos,0x94=>self.aldo3,_=>0xff}}
    fn write(&mut self,reg:u8,value:u8)->bool {match reg {
        0x30=>{self.adc=value&0x3f;true},
        0x90=>{self.ldos=value;self.enable_written=true;true},
        0x94=>{self.aldo3=value&31;self.voltage_written=true;true},
        _=>false,
    }}
}
struct AxpSubset {state:Arc<Mutex<AxpState>>,pins:(u8,u8),pointer:u8,address:bool}
impl I2cDevice for AxpSubset {
    fn pins(&self)->Option<(u8,u8)> {Some(self.pins)}
    fn start(&mut self,read:bool)->bool {if read {AxpState::readable(self.pointer)}else{self.address=true;true}}
    fn write(&mut self,byte:u8)->bool {
        if self.address {self.pointer=byte;self.address=false;return AxpState::readable(byte);}
        let accepted=self.state.lock().expect("LilyGO AXP mutex poisoned").write(self.pointer,byte);self.pointer=self.pointer.wrapping_add(1);accepted
    }
    fn read(&mut self)->u8 {let b=self.state.lock().expect("LilyGO AXP mutex poisoned").read(self.pointer);self.pointer=self.pointer.wrapping_add(1);b}
}

/// Visible-window RGB565 subsets. Keyboard uses the exact legacy GC9107
/// offsets; Camera Plus uses its zero-offset 240x240 window. JD9613 FEh pages
/// are decoded before DCS, so vendor gamma data never turns into RAM pixels.
/// Unknown RAM outside the documented glass is not invented.
struct IoPanel {kind:Product,panel:DcsPanel,command:u8,args:[u8;4],count:usize,valid:[bool;2],page:u8,brightness:u8,brightness_control:u8,changes:u64,rejected:u64}
impl IoPanel {
    fn new(kind:Product)->Self {let (w,h)=match kind {Product::Keyboard=>(128,128),Product::Track=>(126,294),_=>(240,240)};Self{kind,panel:DcsPanel::new(w,h),command:0,args:[0;4],count:0,valid:[false;2],page:0,brightness:if kind==Product::Track {0}else{255},brightness_control:0,changes:0,rejected:0}}
    fn version(&self)->u64 {self.changes.wrapping_add(self.panel.pixels_written)}
    fn reset(&mut self) {self.changes=self.version().wrapping_add(1);self.panel.reset();self.command=0;self.count=0;self.valid=[false;2];self.page=0;self.brightness=if self.kind==Product::Track {0}else{255};self.brightness_control=0;}
    fn offsets(&self)->Option<(u16,u16)> {if self.kind!=Product::Keyboard {return Some((0,0));}match self.panel.madctl&0xe0 {0xc0=>Some((2,1)),0xa0=>Some((1,0)),0=>Some((0,0)),0x60=>Some((0,2)),_=>None}}
    fn byte(&mut self,data:bool,value:u8) {
        if !data {
            self.command=value;self.count=0;
            if self.kind==Product::Track && (value==0xfe || self.page!=0) {return;}
            if value==1 {self.reset();return;}
            if matches!(value,0x2a|0x2b) {self.valid[usize::from(value==0x2b)]=false;}
            if value==0x2c && (self.valid!=[true;2] || self.panel.colmod&7!=5) {self.rejected+=1;return;}
            self.panel.dc=false;self.panel.byte(value);if value==0x10&&self.kind==Product::Track {self.brightness=0;}
            if matches!(value,0x10|0x11|0x20|0x21|0x28|0x29) {self.changes=self.changes.wrapping_add(1);}return;
        }
        if self.kind==Product::Track {if self.command==0xfe {self.page=value;return;}
            if self.page!=0 {return;}}
        self.panel.dc=true;
        match self.command {
            0x2a|0x2b=>{
                if self.count>=4 {return;}self.args[self.count]=value;self.count+=1;
                if self.count==4 {
                    let index=usize::from(self.command==0x2b);let Some((x,y))=self.offsets() else{self.rejected+=1;return;};let offset=if index==0 {x}else{y};
                    let start=u16::from_be_bytes([self.args[0],self.args[1]]).checked_sub(offset);let end=u16::from_be_bytes([self.args[2],self.args[3]]).checked_sub(offset);
                    let limit=if (index==0) ^ (self.panel.madctl&0x20!=0) {self.panel.cols}else{self.panel.rows};
                    if let (Some(start),Some(end))=(start,end) {if start<=end && usize::from(end)<limit {self.valid[index]=true;for b in start.to_be_bytes().into_iter().chain(end.to_be_bytes()) {self.panel.byte(b);}return;}}
                    self.rejected+=1;
                }
            }
            0x36=>{if self.count==0 {self.panel.byte(value);self.valid=[false;2];self.changes=self.changes.wrapping_add(1);}self.count=self.count.saturating_add(1);}
            0x3a=>{if self.count==0 {self.panel.byte(value);}self.count=self.count.saturating_add(1);}
            0x51 if self.kind==Product::Track=>{if self.count==0 {self.brightness=value;self.changes=self.changes.wrapping_add(1);}self.count=self.count.saturating_add(1);}
            0x53 if self.kind==Product::Track=>{if self.count==0 {self.brightness_control=value&0x28;self.changes=self.changes.wrapping_add(1);}self.count=self.count.saturating_add(1);}
            0x2c if self.valid==[true;2] && self.offsets().is_some() && self.panel.colmod&7==5=>self.panel.byte(value),
            _=>{}
        }
    }
    fn pixels(&self,light:u8,powered:bool)->Vec<u16> {
        let (w,h)=if self.kind==Product::Track {(294,126)}else{(self.panel.cols,self.panel.rows)};let mut pixels=vec![0;w*h];
        if !powered || light==0 || self.panel.sleeping || !self.panel.on || (self.kind==Product::Track&&self.brightness_control&0x20==0) {return pixels;}
        let brightness=u32::from(light)*u32::from(self.brightness)/255;
        for y in 0..h {for x in 0..w {
            let at=match self.kind {Product::Keyboard=>(127-y)*128+127-x,Product::Track=>x*126+(125-y),_=>y*w+x};let mut raw=self.panel.gram[at];
            let bgr_glass=self.kind==Product::Keyboard;
            if (self.panel.madctl&8!=0)!=bgr_glass {raw=swap_rgb(raw);}
            let normal_inverted=self.kind!=Product::Track;if self.panel.inverted!=normal_inverted {raw= !raw;}
            pixels[y*w+x]=scale_rgb(raw,brightness);
        }}pixels
    }
}
fn swap_rgb(raw:u16)->u16 {(raw&0x07e0)|((raw&31)<<11)|(raw>>11)}
fn scale_rgb(raw:u16,brightness:u32)->u16 {let raw=u32::from(raw);((((raw>>11)&31)*brightness/255)<<11 | (((raw>>5)&63)*brightness/255)<<5 | ((raw&31)*brightness/255)) as u16}

/// Original_Test STM32G030 firmware e35ec481, protocol version 02h.
/// Its address callback consumes the PREVIOUS receive buffer on the next write
/// address phase. Reads return that snapshot; selecting twice is intentional.
/// This bounded CS/key/version protocol is not execution of the STM32 CPU.
struct KeyboardState {selected:u8,keys:u8,pending:Option<(u8,u8)>,response:u8,response_valid:bool}
impl KeyboardState {fn new()->Self {Self{selected:0,keys:0,pending:None,response:0,response_valid:false}}}
struct KeyboardI2c {state:Arc<Mutex<KeyboardState>>,first:bool,command:u8,writes:u8}
impl I2cDevice for KeyboardI2c {
    fn pins(&self)->Option<(u8,u8)> {Some((42,2))}
    fn start(&mut self,read:bool)->bool {
        let mut s=self.state.lock().expect("Keyboard STM32 state");if read {return s.response_valid;}
        if let Some((cmd,value))=s.pending.take() {match cmd {1=>s.selected=value&31,2=>{s.response=s.keys;s.response_valid=true;},0x10=>{s.response=2;s.response_valid=true;},_=>{}}}
        self.first=true;self.writes=0;true
    }
    fn write(&mut self,b:u8)->bool {
        if self.first {self.first=false;self.command=b;if !matches!(b,1|2|0x10) {return false;}self.state.lock().expect("Keyboard STM32 state").pending=Some((b,0));return true;}
        if !matches!(self.command,1|2|0x10) || self.writes!=0 {return false;}self.writes=1;self.state.lock().expect("Keyboard STM32 state").pending=Some((self.command,b));true
    }
    fn read(&mut self)->u8 {let s=self.state.lock().expect("Keyboard STM32 state");if s.response_valid {s.response}else{255}}
}
struct SpiScreens {kind:Product,panels:Vec<IoPanel>,keyboard:Option<Arc<Mutex<KeyboardState>>>,dc:Option<bool>,reset_high:bool,powered:bool,light_gpio:Option<bool>,brightness:u8,changes:u64,transactions:u64}
impl SpiScreens {
    fn new(kind:Product)->Self {Self{kind,panels:(0..if kind==Product::Keyboard {4}else{1}).map(|_|IoPanel::new(kind)).collect(),keyboard:(kind==Product::Keyboard).then(||Arc::new(Mutex::new(KeyboardState::new()))),dc:None,reset_high:true,powered:kind!=Product::Track,light_gpio:None,brightness:if kind==Product::Track {255}else{0},changes:0,transactions:0}}
    fn gpio(&mut self,routes:&PinRoutes<'_>) {
        let (dc,reset,backlight)=match self.kind {Product::Keyboard=>(39,Some(38),Some((1,false))),Product::Track=>(7,Some(8),None),_=>(45,None,Some((46,true)))};
        self.dc=routes.software_output(dc);let reset_high=reset.is_none_or(|pin|routes.software_output(pin)!=Some(false));let powered=self.kind!=Product::Track || routes.software_output(4)==Some(true);
        if (self.reset_high&&!reset_high)||(self.powered&&!powered) {for panel in &mut self.panels {panel.reset();}}
        if (self.reset_high,self.powered)!=(reset_high,powered) {self.changes=self.changes.wrapping_add(1);}self.reset_high=reset_high;self.powered=powered;
        if let Some((pin,active))=backlight {self.light_gpio=routes.software_output(pin);self.light(if self.light_gpio==Some(active) {255}else{0});}
    }
    fn light(&mut self,value:u8) {if self.brightness!=value {self.brightness=value;self.changes=self.changes.wrapping_add(1);}}
    fn pwm(&mut self,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        let (pin,active)=match self.kind {Product::Keyboard=>(1,false),Product::CameraPlus=>(46,true),_=>return};
        let value=pwm(pin).map(|(_,d)|if active {d.min(65535)}else{65535-d.min(65535)}).map(|d|(d*255/65535) as u8).unwrap_or(if self.light_gpio==Some(active) {255}else{0});self.light(value);
    }
    fn spi(&mut self,pins:SpiPins,bytes:&[u8]) {
        if !self.powered || !self.reset_high {return;}let Some(dc)=self.dc else{return;};
        let (sclk,mosi,cs)=match self.kind {Product::Keyboard=>(41,40,None),Product::Track=>(5,6,Some(9)),_=>(35,34,Some(36))};
        if pins.sclk&(1<<sclk)==0 || pins.mosi&(1<<mosi)==0 || cs.is_some_and(|p|pins.cs&(1<<p)==0) {return;}
        let selected=self.keyboard.as_ref().map_or(31,|s|s.lock().expect("Keyboard STM32 state").selected);
        for (i,panel) in self.panels.iter_mut().enumerate() {if self.kind!=Product::Keyboard || selected&(1<<(4-i))!=0 {for &b in bytes {panel.byte(dc,b);}self.transactions=self.transactions.wrapping_add(1);}}
    }
    fn version(&self)->u64 {self.panels.iter().fold(self.changes,|a,p|a.wrapping_add(p.version()))}
    fn frames(&self)->u64 {self.panels.iter().map(|p|p.panel.frames).sum()}
    fn display(&self)->(u32,u32,Vec<u16>,u64) {
        let (w,h)=match self.kind {Product::Keyboard=>(512,128),Product::Track=>(294,126),_=>(240,240)};let mut frame=vec![0;w*h];
        for (i,panel) in self.panels.iter().enumerate() {let pixels=panel.pixels(self.brightness,self.powered&&self.reset_high);let pw=if self.kind==Product::Keyboard {128}else{w};for y in 0..h {frame[y*w+i*pw..y*w+(i+1)*pw].copy_from_slice(&pixels[y*pw..(y+1)*pw]);}}
        (w as u32,h as u32,frame,self.version())
    }
}

/// CST816D contact report supported by the exact Camera Plus driver. This
/// exposes 01h..06h only; identity, sleep/gesture/IRQ timing are not guessed.
#[derive(Default)]
struct CameraTouch {x:u16,y:u16,down:bool}
struct CameraTouchI2c {state:Arc<Mutex<CameraTouch>>,pointer:u8,first:bool}
impl I2cDevice for CameraTouchI2c {
    fn pins(&self)->Option<(u8,u8)> {Some((33,37))}
    fn start(&mut self,read:bool)->bool {if read {(1..=6).contains(&self.pointer)}else{self.first=true;true}}
    fn write(&mut self,b:u8)->bool {if !self.first {return false;}self.first=false;self.pointer=b;(1..=6).contains(&b)}
    fn read(&mut self)->u8 {let s=self.state.lock().expect("Camera Plus touch state");let b=match self.pointer {1=>0,2=>u8::from(s.down),3=>(s.x>>8) as u8&15,4=>s.x as u8,5=>(s.y>>8) as u8&15,6=>s.y as u8,_=>255};self.pointer=self.pointer.wrapping_add(1);b}
}

/// T-Panel V1.2: RGB565 pixels have no controller GRAM. Only completed,
/// correctly routed LCD_CAM DMA scans become the displayed frame. XL9535
/// supplies 9-bit serial command/DC on P15/P16/P17 and LCD reset on P05.
struct RgbPanel {out:u16,polarity:u16,config:u16,clock:Option<bool>,bits:u8,word:u16,reset_high:bool,command:u8,count:u8,colmod:u8,madctl:u8,inverted:bool,sleeping:bool,on:bool,pixels:Vec<u16>,frames:u64,changes:u64,brightness:u8,light_gpio:Option<bool>,routes:bool,started:bool,commands:u64}
impl RgbPanel {
    fn new()->Self {Self{out:0xffff,polarity:0,config:0xffff,clock:None,bits:0,word:0,reset_high:false,command:0,count:0,colmod:0x70,madctl:0,inverted:false,sleeping:true,on:false,pixels:vec![0;480*480],frames:0,changes:0,brightness:0,light_gpio:None,routes:false,started:false,commands:0}}
    fn reset(&mut self) {self.bits=0;self.word=0;self.command=0;self.count=0;self.colmod=0x70;self.madctl=0;self.inverted=false;self.sleeping=true;self.on=false;self.started=false;self.changes=self.changes.wrapping_add(1);}
    fn output(&self,pin:u8)->Option<bool> {(self.config&(1<<pin)==0).then_some(self.out&(1<<pin)!=0)}
    fn lines(&mut self) {
        // Unknown reset input at startup is deliberately held unavailable until
        // the guest configures the expander output; no factory rail default.
        let reset_high=self.output(5)==Some(true);if self.reset_high&&!reset_high {self.reset();}
        if self.reset_high!=reset_high {self.changes=self.changes.wrapping_add(1);}self.reset_high=reset_high;
        let clock=self.output(13);let data=self.output(14);let selected=self.output(15)==Some(false);
        if !reset_high || !selected || clock.is_none() || data.is_none() {self.bits=0;self.word=0;}
        else if self.clock==Some(false) && clock==Some(true) {self.word=(self.word<<1)|u16::from(data==Some(true));self.bits+=1;if self.bits==9 {self.byte(self.word&0x100!=0,self.word as u8);self.bits=0;self.word=0;}}
        self.clock=clock;
    }
    fn byte(&mut self,data:bool,value:u8) {
        if !data {self.command=value;self.count=0;self.commands=self.commands.wrapping_add(1);match value {1=>self.reset(),0x10=>self.sleeping=true,0x11=>self.sleeping=false,0x20=>self.inverted=false,0x21=>self.inverted=true,0x28=>self.on=false,0x29=>self.on=true,_=>{}}self.changes=self.changes.wrapping_add(1);return;}
        if self.count==0 {match self.command {0x36=>{self.madctl=value;self.changes=self.changes.wrapping_add(1);},0x3a=>{self.colmod=value;self.changes=self.changes.wrapping_add(1);},_=>{}}}self.count=self.count.saturating_add(1);
    }
    fn write(&mut self,reg:u8,value:u8) {let shift=(reg&1)*8;let target=match reg&6 {2=>Some(&mut self.out),4=>Some(&mut self.polarity),6=>Some(&mut self.config),_=>None};if let Some(target)=target {*target=(*target&!(255<<shift))|(u16::from(value)<<shift);}self.lines();}
    fn read(&self,reg:u8)->u8 {let value=match reg&6 {0=>(self.out|self.config)^(self.polarity&self.config),2=>self.out,4=>self.polarity,_=>self.config};(value>>((reg&1)*8)) as u8}
    fn light(&mut self,value:u8) {if self.brightness!=value {self.brightness=value;self.changes=self.changes.wrapping_add(1);}}
    fn gpio(&mut self,routes:&PinRoutes<'_>) {
        self.light_gpio=routes.software_output(14);self.light(if self.light_gpio==Some(true) {255}else{0});
        self.routes=routes.matrix_output(41,154)&&routes.matrix_output(39,151)&&routes.matrix_output(40,152)&&[1,2,3,4,5,6,7,8,9,10,11,12,13,42,46,45].iter().enumerate().all(|(i,&p)|routes.matrix_output(p,133+i as u32));
    }
    fn frame(&mut self,w:u32,h:u32,bytes:&[u8]) {if !self.started || !self.routes || !self.reset_high || (w,h)!=(480,480) || bytes.len()!=480*480*2 {return;}for (out,bytes) in self.pixels.iter_mut().zip(bytes.as_chunks::<2>().0) {*out=u16::from_le_bytes(*bytes);}self.frames=self.frames.wrapping_add(1);self.changes=self.changes.wrapping_add(1);}
    fn display(&self)->Vec<u16> {
        let mut frame=vec![0;480*480];if !self.reset_high || self.sleeping || !self.on || self.brightness==0 || (self.colmod>>4)&7!=5 {return frame;}
        for y in 0..480 {for x in 0..480 {let row=if self.madctl&0x10!=0 {479-y}else{y};let mut raw=self.pixels[row*480+x];if self.madctl&8!=0 {raw=swap_rgb(raw);}
            if self.inverted {raw= !raw;}frame[y*480+x]=scale_rgb(raw,u32::from(self.brightness));}}frame
    }
}
struct PanelExpander {state:Arc<Mutex<RgbPanel>>,pointer:u8,first:bool}
impl I2cDevice for PanelExpander {
    fn pins(&self)->Option<(u8,u8)> {Some((17,18))}
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}true}
    fn write(&mut self,b:u8)->bool {if self.first {self.first=false;self.pointer=b;return b<8;}
        if self.pointer>=8 {return false;}self.state.lock().expect("T-Panel XL9535 state").write(self.pointer,b);self.pointer^=1;true}
    fn read(&mut self)->u8 {if self.pointer>=8 {return 255;}let b=self.state.lock().expect("T-Panel XL9535 state").read(self.pointer);self.pointer^=1;b}
}

pub struct LilygoIo {
    product:Product,buttons:[bool;6],edges:Vec<BoardEdge>,events:u64,plain_led:[[u8;3];1],plain_updates:u64,
    apa:Option<Apa102>,shift:Option<Shift595>,rgb:Option<Ws2812Chain>,
    card:Option<Box<SpiSdCard>>,card_present:bool,card_selected:bool,card_power:bool,
    axp:Option<Arc<Mutex<AxpState>>>,oled:Option<Arc<Mutex<OledMono>>>,screens:Option<SpiScreens>,rgb_panel:Option<Arc<Mutex<RgbPanel>>>,camera_touch:Option<Arc<Mutex<CameraTouch>>>,vending_pulse:Option<bool>,vending_supply:bool,
}
impl LilygoIo {
    pub fn t_connect()->Self {Self::new(Product::Connect)}
    pub fn t_eth_elite()->Self {Self::new(Product::EthElite)}
    pub fn t_energy_s3()->Self {Self::new(Product::Energy)}
    pub fn t_sim7670g_s3()->Self {Self::new(Product::Sim7670)}
    pub fn t_rs_s3()->Self {Self::new(Product::Rs)}
    pub fn t_eth_lite()->Self {Self::new(Product::EthLite)}
    pub fn t_relay_s3()->Self {Self::new(Product::Relay)}
    pub fn t_sim7080_s3()->Self {Self::new(Product::Sim7080)}
    pub fn t_camera_s3()->Self {Self::new(Product::Camera)}
    pub fn t_simcam()->Self {Self::new(Product::Simcam)}
    pub fn t_keyboard_s3_pro()->Self {Self::new(Product::Keyboard)}
    pub fn t_micro32_s3()->Self {Self::new(Product::Micro)}
    pub fn t_panel_s3()->Self {Self::new(Product::Panel)}
    pub fn t_camera_plus_s3()->Self {Self::new(Product::CameraPlus)}
    pub fn t_vending()->Self {Self::new(Product::Vending)}
    pub fn t_track()->Self {Self::new(Product::Track)}
    pub fn t_fpga()->Self {Self::new(Product::Fpga)}
    pub fn t7_s3()->Self {Self::new(Product::T7)}
    fn new(product:Product)->Self {Self{product,buttons:[true;6],edges:Vec::new(),events:0,plain_led:[[0;3]],plain_updates:0,
        apa:(product==Product::Connect).then(Apa102::new),shift:(product==Product::Relay).then(Shift595::new),rgb:(product==Product::Rs).then(||Ws2812Chain::new(1)),
        card:None,card_present:true,card_selected:false,card_power:product!=Product::Simcam,
        axp:matches!(product,Product::Sim7080|Product::Camera|Product::Fpga).then(||Arc::new(Mutex::new(AxpState::new()))),
        oled:(product==Product::Camera).then(||Arc::new(Mutex::new(OledMono::new(OledKind::Ssd1306,128,64,0)))),
        screens:matches!(product,Product::Keyboard|Product::Track|Product::CameraPlus).then(||SpiScreens::new(product)),
        rgb_panel:(product==Product::Panel).then(||Arc::new(Mutex::new(RgbPanel::new()))),camera_touch:(product==Product::CameraPlus).then(||Arc::new(Mutex::new(CameraTouch::default()))),vending_pulse:None,vending_supply:false,
    }}
    // SCLK,MOSI,MISO,CS: exact board branches, never inferred from a sibling.
    fn sd_spi_pins(&self)->Option<[u8;4]> {match self.product {Product::EthElite=>Some([10,11,9,12]),Product::Sim7670=>Some([21,14,47,13]),Product::EthLite=>Some([7,6,5,42]),Product::Simcam=>Some([39,38,40,47]),Product::CameraPlus=>Some([35,34,48,21]),Product::Panel=>Some([36,35,37,38]),_=>None}}
}
impl BoardModel for LilygoIo {
    fn name(&self)->&'static str {match self.product {Product::Connect=>"lilygo-t-connect",Product::EthElite=>"lilygo-t-eth-elite",Product::Energy=>"lilygo-t-energy-s3",Product::Sim7670=>"lilygo-t-sim7670g-s3",Product::Rs=>"lilygo-t-rs-s3",Product::EthLite=>"lilygo-t-eth-lite",Product::Relay=>"lilygo-t-relay-s3",Product::Sim7080=>"lilygo-t-sim7080-s3",Product::Camera=>"lilygo-t-camera-s3",Product::Simcam=>"lilygo-t-simcam",Product::Keyboard=>"lilygo-t-keyboard-s3-pro",Product::Micro=>"lilygo-t-micro32-s3",Product::Panel=>"lilygo-t-panel-s3",Product::CameraPlus=>"lilygo-t-camera-plus-s3",Product::Vending=>"lilygo-t-vending",Product::Track=>"lilygo-t-track",Product::Fpga=>"lilygo-t-fpga",Product::T7=>"lilygo-t7-s3"}}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_events(&self)->u64 {self.events}
    fn gpio_output_at(&mut self,_cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.events=self.events.wrapping_add(changes.len() as u64);}
    fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>) {
        if let Some(apa)=&mut self.apa {apa.gpio(routes.software_output(3),routes.software_output(8));}
        if let Some(shift)=&mut self.shift {shift.gpio(&routes);}
        let color=match self.product {Product::EthElite if routes.software_output(38)==Some(true)=>[0,255,0],Product::Sim7670 if routes.software_output(12)==Some(false)=>[0,0,255],Product::T7 if routes.software_output(17)==Some(true)=>[0,255,0],Product::Fpga if routes.software_output(46)==Some(true)=>[0,0,255],_=>[0;3]};
        if self.plain_led[0]!=color {self.plain_led[0]=color;self.plain_updates=self.plain_updates.wrapping_add(1);}
        if let Some(screens)=&mut self.screens {screens.gpio(&routes);if self.product==Product::Track&&!screens.powered {for (i,pin) in [1,2,3,10].into_iter().enumerate() {if !self.buttons[i+1] {self.buttons[i+1]=true;self.edges.push(BoardEdge{cycle,pin,level:true});}}}}
        if let Some(panel)=&self.rgb_panel {panel.lock().expect("T-Panel XL9535 state").gpio(&routes);}
        if self.product==Product::Vending {self.vending_pulse=routes.software_output(16);self.vending_supply=routes.software_output(4)==Some(true);}
        if let Some([_,_,_,cs])=self.sd_spi_pins() {
            let powered=self.product!=Product::Simcam || routes.software_output(1)==Some(true);
            if self.card_power&&!powered {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_power=powered;
            let selected=powered&&routes.software_output(cs)==Some(false);
            if self.card_selected&&!selected {if let Some(card)=&mut self.card {card.deselect();}}self.card_selected=selected;
        }
    }
    fn uses_gpio_pwm(&self)->bool {self.screens.as_ref().is_some_and(|s|s.kind!=Product::Track)||self.rgb_panel.is_some()}
    fn gpio_pwm_at(&mut self,_cycle:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        if let Some(screens)=&mut self.screens {screens.pwm(pwm);}
        if let Some(panel)=&self.rgb_panel {let mut p=panel.lock().expect("T-Panel XL9535 state");let brightness=pwm(14).map(|(_,d)|(d.min(65535)*255/65535) as u8).unwrap_or(if p.light_gpio==Some(true) {255}else{0});p.light(brightness);}
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==1 {if let Some(rgb)=&mut self.rgb {rgb.from_bits(bits);}}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {
        if let Some(apa)=&self.apa {return Some((&apa.leds,apa.updates));}
        if let Some(shift)=&self.shift {return Some((&shift.leds,shift.updates));}
        if let Some(rgb)=&self.rgb {return Some((&rgb.leds,rgb.updates));}
        matches!(self.product,Product::EthElite|Product::Sim7670|Product::Fpga|Product::T7).then_some((&self.plain_led,self.plain_updates))
    }
    fn input_levels(&self)->Vec<(u8,bool)> {
        if self.product==Product::Micro {return Vec::new();}let mut v=vec![(0,self.buttons[0])];
        match self.product {Product::Rs=>v.push((5,self.buttons[1])),Product::CameraPlus=>{v.push((17,self.buttons[1]));v.push((47,true));},Product::Keyboard=>v.extend([(4,true),(5,true)]),Product::Track=>v.extend([1,2,3,10].into_iter().enumerate().map(|(i,p)|(p,self.buttons[i+1]))),_=>{}}v
    }
    fn named_pin(&self,name:&str)->Option<u8> {
        if self.product==Product::Micro {return None;}
        match name {"boot"|"btnA"=>Some(0),"button" if !matches!(self.product,Product::Rs|Product::CameraPlus)=>Some(0),"user"|"button"|"btnB" if self.product==Product::Rs=>Some(5),"user"|"button"|"btnB" if self.product==Product::CameraPlus=>Some(17),"knob"|"sw"|"key5" if self.product==Product::Keyboard=>Some(0),"clk" if self.product==Product::Keyboard=>Some(4),"dt" if self.product==Product::Keyboard=>Some(5),"track-right" if self.product==Product::Track=>Some(1),"track-up" if self.product==Product::Track=>Some(2),"track-left" if self.product==Product::Track=>Some(3),"track-down" if self.product==Product::Track=>Some(10),_=>None}
    }
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        if self.product==Product::Keyboard {
            if matches!(id,"clk"|"dt") {return false;}
            let bit=match id {"key1"=>Some(16),"key2"=>Some(8),"key3"=>Some(4),"key4"=>Some(2),"key5"|"boot"|"btnA"|"button"|"knob"|"sw"=>Some(1),_=>None};
            if let Some(bit)=bit {let mut state=self.screens.as_ref().expect("keyboard panels").keyboard.as_ref().expect("keyboard STM32").lock().expect("Keyboard STM32 state");state.keys=(state.keys&!bit)|if pressed {bit}else{0};if bit!=1 {return true;}}
        }
        let Some(pin)=self.named_pin(id) else{return false;};let index=if self.product==Product::Track&&pin!=0 {if !self.screens.as_ref().expect("Track display").powered {return false;}[1,2,3,10].iter().position(|&p|p==pin).expect("Track direction pin")+1}else{usize::from(pin!=0)};let high=!pressed;
        if self.buttons[index]!=high {self.buttons[index]=high;self.edges.push(BoardEdge{cycle,pin,level:high});}true
    }
    fn encoder(&self)->Option<(u8,u8)> {(self.product==Product::Keyboard).then_some((4,5))}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn uses_spi_pins(&self)->bool {self.apa.is_some()||self.sd_spi_pins().is_some()||self.screens.is_some()}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if matches!(host,2|3) {
            if let Some(screens)=&mut self.screens {screens.spi(pins,tx);}
            if pins.sclk&(1<<3)!=0&&pins.mosi&(1<<8)!=0 {if let Some(apa)=&mut self.apa {apa.bytes(tx);}}
            if let Some([sclk,mosi,miso,cs])=self.sd_spi_pins() {
                if self.card_present&&self.card_power&&pins.sclk&(1<<sclk)!=0&&pins.mosi&(1<<mosi)!=0 {
                    if let Some(card)=&mut self.card {let response=card.transfer(pins.cs&(1<<cs)!=0,tx,rx_len);if !self.card_selected {card.deselect();}
                        if pins.miso==Some(miso) {return response;}}
                }
            }
        }vec![0xff;rx_len]
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        if self.sd_spi_pins().is_none() {return Err("this exact board has no SPI SD transport; T-SIM7080G uses native SDMMC".into());}
        let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(Box::new(card));Ok(description)
    }
    fn set_sd_card_present(&mut self,present:bool) {if self.card_present!=present {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_present=present;}
    fn sd_card_slot(&self)->Option<([u8;6],bool)> {if self.product!=Product::Sim7080 {return None;}Some(([38,39,40,255,255,255],self.card_present&&self.axp.as_ref().expect("SIM7080 PMIC").lock().expect("LilyGO AXP mutex poisoned").sd_power()))}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let mut v:Vec<(u8,u8,Box<dyn I2cDevice>)>=Vec::new();
        for controller in 0..2 {
            if let Some(state)=&self.axp {let pins=match self.product {Product::Sim7080=>(15,7),Product::Fpga=>(38,39),_=>(7,6)};v.push((controller,0x34,Box::new(AxpSubset{state:state.clone(),pins,pointer:0,address:true})));}
            if let Some(state)=&self.oled {v.push((controller,0x3c,Box::new(OledI2c::new(state.clone(),(7,6)))));}
            if let Some(state)=self.screens.as_ref().and_then(|s|s.keyboard.as_ref()) {v.push((controller,1,Box::new(KeyboardI2c{state:state.clone(),first:true,command:0,writes:0})));}
            if let Some(state)=&self.rgb_panel {v.push((controller,0x20,Box::new(PanelExpander{state:state.clone(),pointer:0,first:true})));}
            if let Some(state)=&self.camera_touch {v.push((controller,0x15,Box::new(CameraTouchI2c{state:state.clone(),pointer:0,first:true})));}
        }v
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {if let Some(state)=&self.camera_touch {let mut s=state.lock().expect("Camera Plus touch state");s.x=x.min(239);s.y=y.min(239);s.down=down;}}
    fn lcd_rgb_start(&mut self,_cycle:u64,w:u32,h:u32,bpp:u32,_clock:&dyn Fn(u8)->Option<(u64,u32)>) {if let Some(panel)=&self.rgb_panel {let mut p=panel.lock().expect("T-Panel XL9535 state");p.started=(w,h,bpp)==(480,480,2)&&p.routes&&p.reset_high;}}
    fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]) {if let Some(panel)=&self.rgb_panel {panel.lock().expect("T-Panel XL9535 state").frame(w,h,bytes);}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        if let Some(screens)=&self.screens {return Some(screens.display());}
        if let Some(panel)=&self.rgb_panel {let p=panel.lock().expect("T-Panel XL9535 state");return Some((480,480,p.display(),p.changes));}
        let state=self.oled.as_ref()?.lock().expect("LilyGO OLED mutex poisoned");Some((128,64,state.frame(),state.version()))
    }
    fn display_version(&self)->u64 {if let Some(s)=&self.screens {return s.version();}
        if let Some(p)=&self.rgb_panel {return p.lock().expect("T-Panel XL9535 state").changes;}self.oled.as_ref().map_or(0,|p|p.lock().expect("LilyGO OLED mutex poisoned").version())}
    fn display_frames(&self)->u64 {if let Some(s)=&self.screens {return s.frames();}
        if let Some(p)=&self.rgb_panel {return p.lock().expect("T-Panel XL9535 state").frames;}self.oled.as_ref().map_or(0,|p|p.lock().expect("LilyGO OLED mutex poisoned").frames())}
    fn report(&self)->String {
        let io=match self.product {
            Product::Connect=>"APA102=4 CLK3 DATA8; RS485/CAN transceivers unmodeled".into(),
            Product::EthElite=>"GPIO38 green LED; W5500 and optional shields unmodeled".into(),
            Product::Energy=>"BOOT0; battery ADC, IP3005A/HX6610S charger and charger LEDs unmodeled".into(),
            Product::Sim7670=>"GPIO12 low-active user LED; SIM7670G UART/cellular/GNSS unmodeled".into(),
            Product::Rs=>"GPIO5 user button, RMT WS2812 GPIO1; RS232/485 transceivers unmodeled".into(),
            Product::EthLite=>"S3 W5500 variant; Ethernet and optional shields unmodeled".into(),
            Product::Relay=>format!("74HC595 /OE4 CLK5 LATCH6 DATA7; relay-drive-mask=0x{:02x}; contact/load/timing/RTC and optional interfaces unmodeled",self.shift.as_ref().expect("relay shift register").drive&63),
            Product::Sim7080=>format!("native 1-bit SD CLK38 CMD39 D0=40 ALDO3-ready={}; AXP2101 register subset only, EFUSE rail defaults unknown; modem/camera/analog PMIC and card brownout reset unmodeled",self.sd_card_slot().expect("native SD").1),
            Product::Camera=>format!("{}; AXP2101 ID/ADC-enable/ALDO3 register subset only; camera/PIR/audio/analog PMIC/whole-board shutdown unmodeled",self.oled.as_ref().expect("camera OLED").lock().expect("LilyGO OLED mutex poisoned").report()),
            Product::Simcam=>"V1.3 SPI SD peripheral supply enabled by GPIO1; PCIe modem/camera/IR/audio and analog power unmodeled".into(),
            Product::Keyboard=>"V1.1 host: STM32 Original_Test protocol02 delayed CS/key/version subset; four independent GC9107 128x128 windows on a 512x128 DEBUG canvas; BOOT/KEY5=0 encoder4/5; STM32 CPU, LED modes, chained boards and alternate ST7735/LovyanGFX offsets unmodeled".into(),
            Product::Micro=>"bare ESP32-S3FH4R2 module 4MB Flash/2MB PSRAM; no invented onboard button, LED, display or SD socket".into(),
            Product::Panel=>format!("V1.2 ST7701S RGB565 480x480, XL9535 9-bit serial init; commands={}; explicit expander reset configuration required; CST3240 touch, H2 CPU, CAN/RS485 and optical timing unmodeled",self.rgb_panel.as_ref().expect("T-Panel RGB").lock().expect("T-Panel XL9535 state").commands),
            Product::CameraPlus=>"V1.2 ST7789 240x240 BL46; shared SPI SD; USER17; CST816D polled contact only; camera/microphone/speaker/haptic/SY6970 charger and touch IRQ/identity unmodeled".into(),
            Product::Vending=>format!("BOOT0; GPIO16 MCU pulse drive={:?}, GPIO4 peripheral supply={}; physical PSRAM present, capacity unknown and disabled in diagnostic configuration; RS485/modem/connector loads unmodeled",self.vending_pulse,self.vending_supply),
            Product::Track=>"V1.2 photo/factory pin map SPI5/6 CS9 DC7 RST8, JD9613 126x294 mounted landscape294x126; GPIO4 power; BOOT0 and four trackball sensor lines; conflicting README SPI pins, LED color and old 4MB schematic explicitly excluded".into(),
            Product::Fpga=>"S3 subsystem: BOOT0 blue LED46; AXP2101 ID/ADC-enable/ALDO3 subset on38/39; Gowin FPGA/Cortex-M3, FPGA memory/BANK supplies and JTAG unmodeled".into(),
            Product::T7=>"T7-S3 V1.2 N16R8: BOOT0 green LED17; TP4054 charge LED, battery ADC and analog power unmodeled".into(),
        };format!("[{}] {}; SPI-SD={}; RF/network unmodeled",self.name(),io,self.card.as_ref().map_or_else(||"not attached".into(),|c|c.report()))
    }
}
