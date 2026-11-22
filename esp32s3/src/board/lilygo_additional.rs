//! Independently sourced LilyGO sales/hardware targets, not sibling aliases.
//! LilyGoLib 9116bf41 docs/hardware/lilygo-t-deck.md explicitly establishes the
//! original/Plus main GPIO mapping; T-Deck 9aac147a supplies the same pin header
//! and shared firmware. Plus GNSS stays unmodeled and its Grove is unavailable.
//! Pro selects only the manufacturer's documented Meshtastic-capable V1.0.
//! Additional OLED/SD/BOOT profiles use the exact LoRa-Series target headers;
//! BPF schematic page 2 establishes DC1 OLED and ALDO2 card supply routing.
//! See lilygo-additional-profiles-2026-11-22.json for receipts and limitations.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,VirtualCycle,SpiPins},pins::PinRoutes};
use super::{tdeck::TDeck,lilygo_epaper::LilygoEpaper};
use super::lilygo_display::{LilygoDisplay,Profile,TouchKind,TouchWiring};
use std::sync::{Arc,Mutex};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,oled_mono::{OledMono,OledI2c,OledKind}};

pub fn make_board(name:&str)->Option<Board> {
    let (name,tft):(&'static str,bool)=match name {
        "lilygo-t-deck-pro-meshtastic"=>("lilygo-t-deck-pro-meshtastic",false),
        "lilygo-t-deck-meshtastic"=>("lilygo-t-deck-meshtastic",true),
        "lilygo-t-deck-plus-meshtastic"=>("lilygo-t-deck-plus-meshtastic",true),
        "lilygo-t-deck-plus-1"=>("lilygo-t-deck-plus-1",true),
        _=>return WIRED.iter().find(|p|p.name==name).map(|p|Box::new(WiredBoard::new(p)) as Board)
            .or_else(||SCREENS.iter().find(|p|p.display.name==name).map(|p|Box::new(ScreenBoard{profile:p,inner:LilygoDisplay::new(&p.display)}) as Board)),
    };
    let inner:Board=if tft {Box::new(TDeck::new())}else{Box::new(LilygoEpaper::t_deck_pro())};
    Some(Box::new(AdditionalDeck{name,inner,tft,card:None,card_present:true,powered:false,dc:None,gpio_events:0}))
}

#[derive(Clone,Copy)]
struct OledWiring {kind:OledKind,sda:u8,scl:u8,address:u8}
#[derive(Clone,Copy)]
struct CardWiring {pins:[u8;4],other_selects:u64}
#[derive(Clone,Copy)]
struct PowerWiring {pins:(u8,u8),oled_ldo:Option<u8>,card_ldo:u8}
struct WiredProfile {name:&'static str,oled:Option<OledWiring>,buttons:&'static [(&'static str,u8)],led:Option<(u8,bool)>,card:Option<CardWiring>,power:Option<PowerWiring>,scope:&'static str}
const BOOT:&[(&str,u8)]=&[("boot",0)];
struct ScreenProfile {display:Profile,other_selects:u64,scope:&'static str}
const SCREENS:&[ScreenProfile]=&[
    ScreenProfile{display:Profile{name:"lilygo-t-connect-pro",controller:"ST7796",gram:[320,480],glass:[49,0,222,480],spi:[12,11,21],dc:Some(41),reset:None,light:Some((46,true)),light_pwm:true,enable:None,quad:false,buttons:BOOT,encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst226Se,address:0x5a,sda:39,scl:40,irq:3,reset:Some(47)}),expander:false},other_selects:(1<<10)|(1<<14),scope:"V1.0 with screen and SX1262; ST7796/CST226SE single-point touch/BOOT only; W5500/LoRa/relay/CAN/RS485/RS232 absent; no SD on this selected variant"},
    ScreenProfile{display:Profile{name:"lilygo-t-circle-s3",controller:"GC9D01N",gram:[160,160],glass:[0,0,160,160],spi:[15,17,13],dc:Some(16),reset:None,light:Some((18,false)),light_pwm:true,enable:None,quad:false,buttons:BOOT,encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst816D,address:0x15,sda:11,scl:14,irq:12,reset:None}),expander:false},other_selects:0,scope:"V1.1 base board; GC9D01N/CST816D single-point touch/BOOT; APA102/audio/microphone absent; infrared expansion excluded"},
    ScreenProfile{display:Profile{name:"lilygo-t-encoder-pro",controller:"CO5300",gram:[390,390],glass:[0,0,390,390],spi:[12,11,10],dc:None,reset:Some(4),light:None,light_pwm:false,enable:Some(3),quad:true,buttons:&[("encoder-press",0)],encoder:Some((1,2)),touch:Some(TouchWiring{kind:TouchKind::Cst816D,address:0x15,sda:5,scl:6,irq:9,reset:Some(8)}),expander:false},other_selects:0,scope:"TFD12MASBCTB4_V0_07 CO5300/CST816D panel; QSPI display and encoder; original DXQ120MYB2416A SH8601/CHSC5816 panel excluded; buzzer absent"},
];
/// Only the protocol implementation is shared. Each constructor above declares
/// its own exact pin map, selected panel, touch type and shared-bus exclusions.
struct ScreenBoard {profile:&'static ScreenProfile,inner:LilygoDisplay}
impl BoardModel for ScreenBoard {
    fn name(&self)->&'static str {self.profile.display.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {self.inner.gpio_waveform_at(cycle,routes);}
    fn uses_gpio_pwm(&self)->bool {self.inner.uses_gpio_pwm()}
    fn gpio_pwm_at(&mut self,cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {self.inner.gpio_pwm_at(cycle,pwm);}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {if pins.cs&self.profile.other_selects!=0 {return vec![255;rx];}self.inner.spi_transfer_pins(host,pins,tx,rx)}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {self.inner.i2c_devices()}
    fn input_levels(&self)->Vec<(u8,bool)> {self.inner.input_levels()}
    fn named_pin(&self,name:&str)->Option<u8> {self.inner.named_pin(name).or_else(||(name=="boot").then_some(0))}
    fn encoder(&self)->Option<(u8,u8)> {self.inner.encoder()}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.inner.touch(x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)> {self.inner.touch_irq()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.inner.take_edges()}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.inner.display()}
    fn display_version(&self)->u64 {self.inner.display_version()}
    fn display_frames(&self)->u64 {self.inner.display_frames()}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {self.inner.gram()}
    fn report(&self)->String {format!("[{}] {}; factory-boot=unverified; touch IRQ is held level, no periodic pulses/calibration/gestures; {}",self.name(),self.profile.scope,self.inner.report())}
}
const T3_OLED:OledWiring=OledWiring{kind:OledKind::Ssd1306,sda:18,scl:17,address:0x3c};
const T3_CARD:CardWiring=CardWiring{pins:[14,11,2,13],other_selects:0};
const SUPREME_OLED:OledWiring=OledWiring{kind:OledKind::Sh1106,sda:17,scl:18,address:0x3c};
const SUPREME_CARD:CardWiring=CardWiring{pins:[36,35,37,47],other_selects:1<<34};
const SUPREME_POWER:PowerWiring=PowerWiring{pins:(42,41),oled_ldo:Some(0),card_ldo:4};
// Exact separate catalog identities; current sources explicitly establish these
// selected hardware targets, including V1.2/V1.3 main-board pin equality.
const WIRED:&[WiredProfile]=&[
    WiredProfile{name:"lilygo-t-beam-bpf",oled:Some(OledWiring{kind:OledKind::Sh1106,sda:8,scl:9,address:0x3c}),buttons:&[("boot",0),("user",3)],led:None,card:Some(CardWiring{pins:[12,11,13,10],other_selects:1<<1}),power:Some(PowerWiring{pins:(8,9),oled_ldo:None,card_ldo:1}),scope:"BPF Q438 SX1278 hardware; SH1106/DC1 and SD/ALDO2 digital rails; RF/GNSS/PMU-key absent"},
    WiredProfile{name:"lilygo-t3-s3-v1-3",oled:Some(T3_OLED),buttons:BOOT,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"T3-S3 V1.3 main-board OLED/LED/BOOT/SD; optional radio types absent"},
    WiredProfile{name:"lilygo-t-2can",oled:None,buttons:BOOT,led:None,card:None,power:None,scope:"T-2Can V1.0 ESP_BOOT0 input diagnostic only; MCP2515/TWAI/CAN PHY absent; not T-2Can-FD"},
    WiredProfile{name:"lilygo-t-beam-supreme-meshtastic",oled:Some(SUPREME_OLED),buttons:BOOT,led:None,card:Some(SUPREME_CARD),power:Some(SUPREME_POWER),scope:"Supreme V3.0 QMC6310U/SH1106 0x3c target; ALDO1 OLED/BLDO1 SD; Meshtastic boot unverified"},
    WiredProfile{name:"lilygo-t-beam-1w",oled:Some(OledWiring{kind:OledKind::Sh1106,sda:8,scl:9,address:0x3c}),buttons:&[("boot",0),("user",17)],led:Some((18,true)),card:Some(CardWiring{pins:[13,11,12,10],other_selects:1<<15}),power:None,scope:"T-Beam 1W V1.0 SX1262 target; OLED/LED/buttons/SD only; RF PA/GNSS/fan/ADC absent; no inferred PMU"},
    WiredProfile{name:"lilygo-t3-s3-lr1121",oled:Some(T3_OLED),buttons:BOOT,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"T3-S3 LR1121 V1.3 SSD1306 target; radio/RTC absent; no SH1106 sibling substitution"},
    WiredProfile{name:"lilygo-t-beam-supreme",oled:Some(SUPREME_OLED),buttons:BOOT,led:None,card:Some(SUPREME_CARD),power:Some(SUPREME_POWER),scope:"Supreme V3.0 QMC6310U/SH1106 0x3c target; ALDO1 OLED/BLDO1 SD; RF/GNSS/sensors absent"},
    WiredProfile{name:"lilygo-t3s3-v1-0",oled:Some(T3_OLED),buttons:BOOT,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"Current T3 S3 storefront selects V1.2 main-board wiring; legacy v1-0 slug is not proof of V1.0 hardware"},
    WiredProfile{name:"lilygo-t3-s3-meshtastic",oled:Some(T3_OLED),buttons:BOOT,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"Explicit manufacturer-linked T3 S3 SX1262 Meshtastic target; OLED/LED/BOOT/SD only, Meshtastic boot unverified"},
];

/// AXP2101 rail-only subset, sourced from the pinned vendor XPowersLib. This
/// deliberately does not duplicate T-Beam's key/IRQ/ADC model or invent sensors.
struct Rails {regs:[u8;0x9b],wiring:PowerWiring,panel:Arc<Mutex<OledMono>>,card_epoch:u64}
impl Rails {
    fn new(wiring:PowerWiring,panel:Arc<Mutex<OledMono>>)->Self {let mut regs=[0;0x9b];for (a,v) in [(0,0x20),(1,0x10),(3,0x4a),(0x80,1),(0x82,18)] {regs[a]=v;}let s=Self{regs,wiring,panel,card_epoch:0};s.sync_panel();s}
    fn supplied(&self)->bool {self.regs[0x80]&1!=0&&self.regs[0x10]&1==0}
    fn ldo(&self,bit:u8)->bool {self.supplied()&&self.regs[0x90]&(1<<bit)!=0}
    fn card_power(&self)->bool {self.ldo(self.wiring.card_ldo)}
    fn sync_panel(&self) {let on=self.wiring.oled_ldo.map_or_else(||self.supplied(),|b|self.ldo(b));self.panel.lock().expect("OLED mutex poisoned").set_power(on);}
    fn register(address:u8)->bool {matches!(address,0|1|3|0x10|0x80..=0x86|0x90..=0x9a)}
    fn write(&mut self,address:u8,value:u8)->bool {if !Self::register(address) {return false;}let was=self.card_power();if !matches!(address,0|1|3) {self.regs[usize::from(address)]=value;}
        if was&&!self.card_power() {self.card_epoch=self.card_epoch.wrapping_add(1);}self.sync_panel();true}
}
struct RailDevice {state:Arc<Mutex<Rails>>,pointer:u8,first:bool,pins:(u8,u8)}
impl I2cDevice for RailDevice {
    fn pins(&self)->Option<(u8,u8)> {Some(self.pins)}
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}!read||Rails::register(self.pointer)}
    fn write(&mut self,b:u8)->bool {if self.first {self.pointer=b;self.first=false;return Rails::register(b);}let ok=self.state.lock().expect("AXP rails mutex poisoned").write(self.pointer,b);self.pointer=self.pointer.wrapping_add(1);ok}
    fn read(&mut self)->u8 {let value=if Rails::register(self.pointer) {self.state.lock().expect("AXP rails mutex poisoned").regs[usize::from(self.pointer)]}else{0xff};self.pointer=self.pointer.wrapping_add(1);value}
}
struct WireBus {pins:(u8,u8),bus:I2cBitBang,levels:[bool;2]}
struct WiredBoard {profile:&'static WiredProfile,panel:Option<Arc<Mutex<OledMono>>>,rails:Option<Arc<Mutex<Rails>>>,wires:Vec<WireBus>,edges:Vec<BoardEdge>,card:Option<Box<SpiSdCard>>,card_present:bool,card_epoch:u64,leds:[[u8;3];1],led_version:u64}
impl WiredBoard {
    fn new(profile:&'static WiredProfile)->Self {
        let panel=profile.oled.map(|w|Arc::new(Mutex::new(OledMono::new(w.kind,128,64,if w.kind==OledKind::Sh1106{2}else{0}))));
        let rails=profile.power.zip(panel.as_ref()).map(|(w,p)|Arc::new(Mutex::new(Rails::new(w,p.clone()))));
        Self{profile,panel,rails,wires:Vec::new(),edges:Vec::new(),card:None,card_present:true,card_epoch:0,leds:[[0;3]],led_version:0}
    }
}
impl BoardModel for WiredBoard {
    fn name(&self)->&'static str {self.profile.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        if let Some((pin,on))=self.profile.led {let value=if routes.software_output(pin)==Some(on){[255;3]}else{[0;3]};if self.leds[0]!=value {self.leds[0]=value;self.led_version=self.led_version.wrapping_add(1);}}
        for w in &mut self.wires {let (sda,scl)=w.pins;let high_scl=routes.software_output(scl)!=Some(false);let high_sda=w.bus.drive(high_scl,routes.software_output(sda)!=Some(false));for (i,(pin,level)) in [(sda,high_sda),(scl,high_scl)].into_iter().enumerate() {if w.levels[i]!=level {w.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.wires.clear();
        if let (Some(p),Some(w))=(&self.panel,self.profile.oled) {let mut bus=I2cBitBang::new();bus.attach(w.address,Box::new(OledI2c::new(p.clone(),(w.sda,w.scl))));self.wires.push(WireBus{pins:(w.sda,w.scl),bus,levels:[true;2]});}
        if let (Some(p),Some(w))=(&self.rails,self.profile.power) {let index=if let Some(i)=self.wires.iter().position(|bus|bus.pins==w.pins) {i}else{self.wires.push(WireBus{pins:w.pins,bus:I2cBitBang::new(),levels:[true;2]});self.wires.len()-1};self.wires[index].bus.attach(0x34,Box::new(RailDevice{state:p.clone(),pointer:0,first:true,pins:w.pins}));}
        self.wires.iter().flat_map(|w|(0..2).flat_map(move |controller|w.bus.controller_devices(Some(w.pins)).into_iter().map(move |(a,d)|(controller,a,d)))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {self.profile.buttons.iter().map(|(_,pin)|(*pin,true)).chain(self.wires.iter().flat_map(|w|[(w.pins.0,w.levels[0]),(w.pins.1,w.levels[1])])).collect()}
    fn named_pin(&self,name:&str)->Option<u8> {self.profile.buttons.iter().find_map(|(id,pin)|(*id==name).then_some(*pin))}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn uses_spi_pins(&self)->bool {self.profile.card.is_some()}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        let Some(w)=self.profile.card else{return vec![255;rx_len];};let [clk,mosi,miso,cs]=w.pins;
        if let Some(rails)=&self.rails {let r=rails.lock().expect("AXP rails mutex poisoned");if r.card_epoch!=self.card_epoch {if let Some(card)=&mut self.card {card.power_cycle();}self.card_epoch=r.card_epoch;}
            if !r.card_power(){return vec![255;rx_len];}}
        if !self.card_present||pins.sclk&(1<<clk)==0||pins.mosi&(1<<mosi)==0 {return vec![255;rx_len];}let Some(card)=&mut self.card else{return vec![255;rx_len];};
        let selected=pins.cs&((1<<cs)|w.other_selects)==1<<cs;let rx=card.transfer(selected,tx,rx_len);if selected&&pins.miso==Some(miso) {rx}else{vec![255;rx_len]}
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {if self.profile.card.is_none() {return Err("No documented SD socket on selected board".into());}let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(Box::new(card));Ok(description)}
    fn set_sd_card_present(&mut self,present:bool) {if self.card_present!=present {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_present=present;}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.panel.as_ref().map(|p|{let p=p.lock().expect("OLED mutex poisoned");let (w,h)=p.dimensions();(w,h,p.frame(),p.version())})}
    fn display_version(&self)->u64 {self.panel.as_ref().map_or(0,|p|p.lock().expect("OLED mutex poisoned").version())}
    fn display_frames(&self)->u64 {self.panel.as_ref().map_or(0,|p|p.lock().expect("OLED mutex poisoned").frames())}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {self.profile.led.map(|_|(&self.leds[..],self.led_version))}
    fn report(&self)->String {format!("[{}] {}; SD-SPI={} rail-only-PMU={} radio/GNSS/ADC/IRQ/charger/analog=unmodeled factory-boot=unverified {}",self.profile.name,self.profile.scope,self.card.is_some()&&self.card_present,self.rails.is_some(),self.panel.as_ref().map_or(String::new(),|p|p.lock().expect("OLED mutex poisoned").report()))}
}

struct AdditionalDeck {
    name:&'static str,inner:Board,tft:bool,card:Option<Box<SpiSdCard>>,card_present:bool,
    powered:bool,dc:Option<bool>,gpio_events:u64,
}
impl BoardModel for AdditionalDeck {
    fn name(&self)->&'static str {self.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_events+=changes.len() as u64;
        if !self.tft {self.inner.gpio_output_at(cycle,changes,enabled,output);}
    }
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        if !self.tft {self.inner.gpio_waveform_at(cycle,routes);return;}
        // Feed the established T-Deck device models only routed software GPIO.
        // A GPIO latch alone cannot power the board, clock I2C, or select D/C.
        let mut enabled=0;let mut output=0;
        for pin in [10,11,42,9,17,8,18] {if let Some(high)=routes.software_output(pin) {enabled|=1<<pin;if high {output|=1<<pin;}}}
        let powered=enabled&output&(1<<10)!=0;
        if self.powered && !powered {if let Some(card)=&mut self.card {card.power_cycle();}}
        self.powered=powered;self.dc=routes.software_output(11);
        self.inner.gpio_output_at(cycle,&[],enabled,output);
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if self.tft {
            if !self.powered {return vec![0xff;rx_len];}
            let selects=pins.cs&((1<<9)|(1<<12)|(1<<39));
            if pins.sclk&(1<<40)!=0 && pins.mosi&(1<<41)!=0 && self.card_present {
                if let Some(card)=&mut self.card {
                    let selected=selects==1<<39;let rx=card.transfer(selected,tx,rx_len);
                    if selected {return if pins.miso==Some(38) {rx}else{vec![0xff;rx_len]};}
                }
            }
            if selects==1<<12 && self.dc.is_none() {return vec![0xff;rx_len];}
        }
        self.inner.spi_transfer_pins(host,pins,tx,rx_len)
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        if !self.tft {return self.inner.attach_spi_sd_card(image,config);}
        let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(Box::new(card));Ok(description)
    }
    fn set_sd_card_present(&mut self,present:bool) {
        if !self.tft {self.inner.set_sd_card_present(present);return;}
        if self.card_present!=present {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_present=present;
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {self.inner.i2c_devices()}
    fn input_levels(&self)->Vec<(u8,bool)> {self.inner.input_levels()}
    fn named_pin(&self,name:&str)->Option<u8> {self.inner.named_pin(if self.tft {name.strip_prefix("trackball-").unwrap_or(name)}else{name})}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {self.inner.button_at(cycle,id,pressed)}
    fn matrix_key_at(&mut self,cycle:VirtualCycle,row:u8,column:u8,pressed:bool)->bool {self.inner.matrix_key_at(cycle,row,column,pressed)}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.inner.touch(x,y,down);}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.inner.touch_at(cycle,x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)> {self.inner.touch_irq()}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.inner.advance_to(cycle);}
    fn next_deadline(&self)->Option<VirtualCycle> {self.inner.next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.inner.take_edges()}
    fn released_inputs(&mut self)->Vec<u8> {self.inner.released_inputs()}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.inner.display()}
    fn display_version(&self)->u64 {self.inner.display_version()}
    fn display_frames(&self)->u64 {self.inner.display_frames()}
    fn display_push_hz(&self)->u64 {self.inner.display_push_hz()}
    fn display_quiet_push(&self)->bool {self.inner.display_quiet_push()}
    fn lora_receive_at(&mut self,cycle:VirtualCycle,packet:&esp_soc::devices::sx1262::LoRaInput)->bool {self.inner.lora_receive_at(cycle,packet)}
    fn lora_snapshot(&self)->Option<String> {self.inner.lora_snapshot()}
    fn report(&self)->String {
        let detail=if self.tft {format!("SD-SPI={} {}",self.card.is_some()&&self.card_present,self.inner.report().replace("audio/SD/GNSS=absent","audio/GNSS=absent"))}else{self.inner.report()};
        format!("[{}] selected={} GNSS/audio/charger/battery=unmodeled factory/Meshtastic-boot=unverified; {}",self.name,if self.tft {"original/Plus SX1262 main-board wiring"}else{"Pro V1.0 only; V1.1 excluded"},detail)
    }
}
