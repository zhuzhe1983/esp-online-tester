//! Independently sourced LilyGO sales/hardware targets, not sibling aliases.
//! LilyGoLib 9116bf41 docs/hardware/lilygo-t-deck.md explicitly establishes the
//! original/Plus main GPIO mapping; T-Deck 9aac147a supplies the same pin header
//! and shared firmware. Plus GNSS stays unmodeled and its Grove is unavailable.
//! Pro selects only the manufacturer's documented Meshtastic-capable V1.0.
//! Additional OLED/SD/BOOT profiles use the exact LoRa-Series target headers;
//! BPF schematic page 2 establishes DC1 OLED and ALDO2 card supply routing.
//! See lilygo-additional-profiles-2026-11-22.json for receipts and limitations.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,VirtualCycle,SpiPins,LcdI80Pins},pins::PinRoutes};
use super::{tdeck::TDeck,lilygo_epaper::LilygoEpaper};
use super::lilygo_display::{LilygoDisplay,Profile,TouchKind,TouchWiring,TouchState,TouchDevice};
use std::sync::{Arc,Mutex};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,oled_mono::{OledMono,OledI2c,OledKind}};

pub fn make_board(name:&str)->Option<Board> {
    if name=="lilygo-ts-s3-epaper" {return Some(Box::new(TsEpaper::new()));}
    if name=="lilygo-t-touch-bar" {return Some(Box::new(TouchBar::new()));}
    if name=="lilygo-t-hmi" {return Some(Box::new(Hmi::new()));}
    if name=="lilygo-t5-e-paper-s3-pro" {return Some(Box::new(RawEpaper::new(true)));}
    if name=="lilygo-t5-4-7-inch-e-paper-v2-3" {return Some(Box::new(RawEpaper::new(false)));}
    let (name,tft):(&'static str,bool)=match name {
        "lilygo-t-deck-pro-meshtastic"=>("lilygo-t-deck-pro-meshtastic",false),
        "lilygo-t-deck-meshtastic"=>("lilygo-t-deck-meshtastic",true),
        "lilygo-t-deck-plus-meshtastic"=>("lilygo-t-deck-plus-meshtastic",true),
        "lilygo-t-deck-plus-1"=>("lilygo-t-deck-plus-1",true),
        _=>return WIRED.iter().find(|p|p.name==name).map(|p|Box::new(WiredBoard::new(p)) as Board)
            .or_else(||SCREENS.iter().find(|p|p.display.name==name).map(|p|Box::new(ScreenBoard{profile:p,inner:LilygoDisplay::new(&p.display),card:None,card_present:true}) as Board)),
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
struct WiredProfile {name:&'static str,oled:Option<OledWiring>,buttons:&'static [(&'static str,u8)],encoder:Option<(u8,u8)>,emission_enable:Option<u8>,led:Option<(u8,bool)>,card:Option<CardWiring>,power:Option<PowerWiring>,scope:&'static str}
const BOOT:&[(&str,u8)]=&[("boot",0)];
struct ScreenProfile {display:Profile,other_selects:u64,card:Option<CardWiring>,scope:&'static str}
const SCREENS:&[ScreenProfile]=&[
    ScreenProfile{display:Profile{name:"lilygo-t-glass",controller:"JD9613",gram:[128,294],glass:[0,168,126,126],spi:[15,16,17],dc:Some(21),reset:Some(18),light:None,light_pwm:false,enable:None,quad:false,buttons:BOOT,encoder:None,touch:None,expander:false},other_selects:0,card:None,scope:"T-Glass V2 bottom 126x126 prism window; JD9613 native126x294 with padded address canvas128x294; vendor pages and native0/180 rotation; capacitive pad14/optical mirror/IMU/RTC/audio unmodeled"},
    ScreenProfile{display:Profile{name:"lilygo-t-display-s3-long",controller:"AXS15231B",gram:[180,640],glass:[0,0,180,640],spi:[17,13,12],dc:None,reset:Some(16),light:Some((1,false)),light_pwm:true,enable:None,quad:true,buttons:BOOT,encoder:None,touch:Some(TouchWiring{kind:TouchKind::Axs15231,address:0x3b,sda:15,scl:10,irq:11,reset:Some(16)}),expander:false},other_selects:0,card:Some(CardWiring{pins:[40,39,41,38],other_selects:0}),scope:"New AXS15231B display/integrated-touch assembly with shared reset16; DCS RGB565/QSPI/active-low backlight/BOOT/SPI-SD; old CST3530 assembly, fuel gauge/charger/RTC unmodeled"},
    ScreenProfile{display:Profile{name:"lilygo-t-connect-pro",controller:"ST7796",gram:[320,480],glass:[49,0,222,480],spi:[12,11,21],dc:Some(41),reset:None,light:Some((46,true)),light_pwm:true,enable:None,quad:false,buttons:BOOT,encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst226Se,address:0x5a,sda:39,scl:40,irq:3,reset:Some(47)}),expander:false},other_selects:(1<<10)|(1<<14),card:None,scope:"V1.0 with screen and SX1262; ST7796/CST226SE single-point touch/BOOT only; W5500/LoRa/relay/CAN/RS485/RS232 absent; no SD on this selected variant"},
    ScreenProfile{display:Profile{name:"lilygo-t-circle-s3",controller:"GC9D01N",gram:[160,160],glass:[0,0,160,160],spi:[15,17,13],dc:Some(16),reset:None,light:Some((18,false)),light_pwm:true,enable:None,quad:false,buttons:BOOT,encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst816D,address:0x15,sda:11,scl:14,irq:12,reset:None}),expander:false},other_selects:0,card:None,scope:"V1.1 base board; GC9D01N/CST816D single-point touch/BOOT; APA102/audio/microphone absent; infrared expansion excluded"},
    ScreenProfile{display:Profile{name:"lilygo-t-encoder-pro",controller:"CO5300",gram:[390,390],glass:[0,0,390,390],spi:[12,11,10],dc:None,reset:Some(4),light:None,light_pwm:false,enable:Some(3),quad:true,buttons:&[("encoder-press",0)],encoder:Some((1,2)),touch:Some(TouchWiring{kind:TouchKind::Cst816D,address:0x15,sda:5,scl:6,irq:9,reset:Some(8)}),expander:false},other_selects:0,card:None,scope:"TFD12MASBCTB4_V0_07 CO5300/CST816D panel; QSPI display and encoder; original DXQ120MYB2416A SH8601/CHSC5816 panel excluded; buzzer absent"},
];
/// Only the protocol implementation is shared. Each constructor above declares
/// its own exact pin map, selected panel, touch type and shared-bus exclusions.
struct ScreenBoard {profile:&'static ScreenProfile,inner:LilygoDisplay,card:Option<Box<SpiSdCard>>,card_present:bool}
impl BoardModel for ScreenBoard {
    fn name(&self)->&'static str {self.profile.display.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {self.inner.gpio_waveform_at(cycle,routes);}
    fn uses_gpio_pwm(&self)->bool {self.inner.uses_gpio_pwm()}
    fn gpio_pwm_at(&mut self,cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {self.inner.gpio_pwm_at(cycle,pwm);}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        if let (Some(w),Some(card))=(self.profile.card,&mut self.card) {let [clk,mosi,miso,cs]=w.pins;
            if self.card_present&&pins.sclk&(1<<clk)!=0&&pins.mosi&(1<<mosi)!=0 {let selected=pins.cs&((1<<cs)|w.other_selects)==1<<cs;let bytes=card.transfer(selected,tx,rx);if selected{return if pins.miso==Some(miso){bytes}else{vec![255;rx]};}}
        }
        if pins.cs&self.profile.other_selects!=0 {return vec![255;rx];}self.inner.spi_transfer_pins(host,pins,tx,rx)
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {if self.profile.card.is_none(){return Err("No SD socket on selected screen target".into());}let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(Box::new(card));Ok(description)}
    fn set_sd_card_present(&mut self,present:bool) {if self.card_present!=present {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_present=present;}
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
    WiredProfile{name:"lilygo-t-beam-bpf",oled:Some(OledWiring{kind:OledKind::Sh1106,sda:8,scl:9,address:0x3c}),buttons:&[("boot",0),("user",3)],encoder:None,emission_enable:None,led:None,card:Some(CardWiring{pins:[12,11,13,10],other_selects:1<<1}),power:Some(PowerWiring{pins:(8,9),oled_ldo:None,card_ldo:1}),scope:"BPF Q438 SX1278 hardware; SH1106/DC1 and SD/ALDO2 digital rails; RF/GNSS/PMU-key absent"},
    WiredProfile{name:"lilygo-t3-s3-v1-3",oled:Some(T3_OLED),buttons:BOOT,encoder:None,emission_enable:None,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"T3-S3 V1.3 main-board OLED/LED/BOOT/SD; optional radio types absent"},
    WiredProfile{name:"lilygo-t-2can",oled:None,buttons:BOOT,encoder:None,emission_enable:None,led:None,card:None,power:None,scope:"T-2Can V1.0 ESP_BOOT0 input diagnostic only; MCP2515/TWAI/CAN PHY absent; not T-2Can-FD"},
    WiredProfile{name:"lilygo-t3-s3-mvsr",oled:Some(T3_OLED),buttons:BOOT,encoder:None,emission_enable:None,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"Complete T3-S3 V1.2 + MVSR V1.0 kit from exact storefront, not CPU-less backplate alone; SSD1306/BOOT/LED/SD; audio/vibration/RTC/radio absent"},
    WiredProfile{name:"lilygo-t-twr-rev2-1",oled:Some(OledWiring{kind:OledKind::Sh1106,sda:8,scl:9,address:0x3c}),buttons:&[("boot",0),("ptt",3),("encoder-press",21)],encoder:Some((47,46)),emission_enable:None,led:None,card:Some(CardWiring{pins:[12,11,13,10],other_selects:1<<14}),power:Some(PowerWiring{pins:(8,9),oled_ldo:None,card_ldo:1}),scope:"TWR Rev2.1 VHF OLED 0x3c/DC1, encoder/PTT/BOOT and SD/ALDO2; SA868/WS2812/GNSS/microphone/PMU-key/version-detect absent; UHF OLED 0x3d excluded"},
    WiredProfile{name:"lilygo-t-beam-supreme-meshtastic",oled:Some(SUPREME_OLED),buttons:BOOT,encoder:None,emission_enable:None,led:None,card:Some(SUPREME_CARD),power:Some(SUPREME_POWER),scope:"Supreme V3.0 QMC6310U/SH1106 0x3c target; ALDO1 OLED/BLDO1 SD; Meshtastic boot unverified"},
    WiredProfile{name:"lilygo-t-beam-1w",oled:Some(OledWiring{kind:OledKind::Sh1106,sda:8,scl:9,address:0x3c}),buttons:&[("boot",0),("user",17)],encoder:None,emission_enable:None,led:Some((18,true)),card:Some(CardWiring{pins:[13,11,12,10],other_selects:1<<15}),power:None,scope:"T-Beam 1W V1.0 SX1262 target; OLED/LED/buttons/SD only; RF PA/GNSS/fan/ADC absent; no inferred PMU"},
    WiredProfile{name:"lilygo-t3-s3-lr1121",oled:Some(T3_OLED),buttons:BOOT,encoder:None,emission_enable:None,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"T3-S3 LR1121 V1.3 SSD1306 target; radio/RTC absent; no SH1106 sibling substitution"},
    WiredProfile{name:"lilygo-t-beam-supreme",oled:Some(SUPREME_OLED),buttons:BOOT,encoder:None,emission_enable:None,led:None,card:Some(SUPREME_CARD),power:Some(SUPREME_POWER),scope:"Supreme V3.0 QMC6310U/SH1106 0x3c target; ALDO1 OLED/BLDO1 SD; RF/GNSS/sensors absent"},
    WiredProfile{name:"lilygo-t-twr-plus",oled:Some(OledWiring{kind:OledKind::Sh1106,sda:8,scl:9,address:0x3c}),buttons:&[("boot",0),("ptt",3),("encoder-press",21)],encoder:Some((47,46)),emission_enable:None,led:None,card:Some(CardWiring{pins:[12,11,13,10],other_selects:1<<14}),power:Some(PowerWiring{pins:(8,9),oled_ldo:None,card_ldo:1}),scope:"Exact README links TWR Rev2.0 to t-twr-plus; SH1106 0x3c/DC1, encoder/PTT/BOOT, SD/ALDO2; SA868/audio/GNSS/WS2812/PMU-key absent"},
    WiredProfile{name:"lilygo-t-twr",oled:Some(OledWiring{kind:OledKind::Ssd1306,sda:13,scl:14,address:0x3c}),buttons:&[("boot",0),("user",3),("ptt",38),("encoder-press",7)],encoder:Some((9,5)),emission_enable:Some(21),led:None,card:None,power:None,scope:"Original TWR Rev1.x SSD1306; G21 gates OLED VBAT emission, VDD/I2C/RAM remain powered; exact original buttons/encoder; no Plus/Rev2 or SD/PMU/radio/ADC substitution"},
    WiredProfile{name:"lilygo-t3s3-v1-0",oled:Some(T3_OLED),buttons:BOOT,encoder:None,emission_enable:None,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"Current T3 S3 storefront selects V1.2 main-board wiring; legacy v1-0 slug is not proof of V1.0 hardware"},
    WiredProfile{name:"lilygo-t3-s3-meshtastic",oled:Some(T3_OLED),buttons:BOOT,encoder:None,emission_enable:None,led:Some((37,true)),card:Some(T3_CARD),power:None,scope:"Explicit manufacturer-linked T3 S3 SX1262 Meshtastic target; OLED/LED/BOOT/SD only, Meshtastic boot unverified"},
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
struct WiredBoard {profile:&'static WiredProfile,panel:Option<Arc<Mutex<OledMono>>>,rails:Option<Arc<Mutex<Rails>>>,wires:Vec<WireBus>,edges:Vec<BoardEdge>,card:Option<Box<SpiSdCard>>,card_present:bool,card_epoch:u64,leds:[[u8;3];1],led_version:u64,emission:bool,emission_version:u64}
impl WiredBoard {
    fn new(profile:&'static WiredProfile)->Self {
        let panel=profile.oled.map(|w|Arc::new(Mutex::new(OledMono::new(w.kind,128,64,if w.kind==OledKind::Sh1106{2}else{0}))));
        let rails=profile.power.zip(panel.as_ref()).map(|(w,p)|Arc::new(Mutex::new(Rails::new(w,p.clone()))));
        Self{profile,panel,rails,wires:Vec::new(),edges:Vec::new(),card:None,card_present:true,card_epoch:0,leds:[[0;3]],led_version:0,emission:profile.emission_enable.is_none(),emission_version:0}
    }
}
impl BoardModel for WiredBoard {
    fn name(&self)->&'static str {self.profile.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        // Original TWR schematic: GPIO21 gates OLED VBAT through Q3/Q2;
        // VDD remains on 3V3. RAM/I2C stay live while emission is disabled.
        if let Some(pin)=self.profile.emission_enable {let on=routes.software_output(pin)==Some(true);if on!=self.emission {self.emission=on;self.emission_version=self.emission_version.wrapping_add(1);}}
        if let Some((pin,on))=self.profile.led {let value=if routes.software_output(pin)==Some(on){[255;3]}else{[0;3]};if self.leds[0]!=value {self.leds[0]=value;self.led_version=self.led_version.wrapping_add(1);}}
        for w in &mut self.wires {let (sda,scl)=w.pins;let high_scl=routes.software_output(scl)!=Some(false);let high_sda=w.bus.drive(high_scl,routes.software_output(sda)!=Some(false));for (i,(pin,level)) in [(sda,high_sda),(scl,high_scl)].into_iter().enumerate() {if w.levels[i]!=level {w.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.wires.clear();
        if let (Some(p),Some(w))=(&self.panel,self.profile.oled) {let mut bus=I2cBitBang::new();bus.attach(w.address,Box::new(OledI2c::new(p.clone(),(w.sda,w.scl))));self.wires.push(WireBus{pins:(w.sda,w.scl),bus,levels:[true;2]});}
        if let (Some(p),Some(w))=(&self.rails,self.profile.power) {let index=if let Some(i)=self.wires.iter().position(|bus|bus.pins==w.pins) {i}else{self.wires.push(WireBus{pins:w.pins,bus:I2cBitBang::new(),levels:[true;2]});self.wires.len()-1};self.wires[index].bus.attach(0x34,Box::new(RailDevice{state:p.clone(),pointer:0,first:true,pins:w.pins}));}
        self.wires.iter().flat_map(|w|(0..2).flat_map(move |controller|w.bus.controller_devices(Some(w.pins)).into_iter().map(move |(a,d)|(controller,a,d)))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {self.profile.buttons.iter().map(|(_,pin)|(*pin,true)).chain(self.profile.encoder.into_iter().flat_map(|(a,b)|[(a,true),(b,true)])).chain(self.wires.iter().flat_map(|w|[(w.pins.0,w.levels[0]),(w.pins.1,w.levels[1])])).collect()}
    fn named_pin(&self,name:&str)->Option<u8> {self.profile.buttons.iter().find_map(|(id,pin)|(*id==name).then_some(*pin)).or(match (name,self.profile.encoder) {("clk",Some((a,_)))=>Some(a),("dt",Some((_,b)))=>Some(b),_=>None})}
    fn encoder(&self)->Option<(u8,u8)> {self.profile.encoder}
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
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.panel.as_ref().map(|p|{let p=p.lock().expect("OLED mutex poisoned");let (w,h)=p.dimensions();let frame=if self.emission {p.frame()}else{vec![0;w as usize*h as usize]};(w,h,frame,p.version().wrapping_add(self.emission_version))})}
    fn display_version(&self)->u64 {self.panel.as_ref().map_or(0,|p|p.lock().expect("OLED mutex poisoned").version()).wrapping_add(self.emission_version)}
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

// DEPG0213BN is SSD1680, not the older SSD1681 or UC81xx family. Its real
// 176x296 RAM, 128-wide firmware canvas and 122x250 glass are distinct.
const TS_IO:WiredProfile=WiredProfile{name:"lilygo-ts-s3-epaper",oled:None,buttons:BOOT,encoder:None,emission_enable:None,led:None,card:Some(CardWiring{pins:[14,11,2,13],other_selects:1<<15}),power:None,scope:"TS-S3 exact SD/BOOT wires"};
struct TsEpaper {inner:WiredBoard,panel:esp_soc::devices::ssd1680::Ssd1680,reset:bool,dc:Option<bool>,cs:Option<bool>}
impl TsEpaper {fn new()->Self {let mut panel=esp_soc::devices::ssd1680::Ssd1680::new(48,240_000_000);panel.set_timing_ms(4000,800,100,150);Self{inner:WiredBoard::new(&TS_IO),panel,reset:false,dc:None,cs:None}}}
impl BoardModel for TsEpaper {
    fn name(&self)->&'static str {TS_IO.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {self.panel.advance_to(cycle);let reset=routes.software_output(47)==Some(true);if self.reset&&!reset{self.panel.reset(true);}self.reset=reset;self.dc=routes.software_output(16);self.cs=routes.software_output(15);}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        if self.reset&&self.cs!=Some(true)&&pins.sclk&(1<<14)!=0&&pins.mosi&(1<<11)!=0&&pins.cs&((1<<15)|(1<<13))==1<<15 {if let Some(dc)=self.dc{self.panel.write(dc,tx);}}
        self.inner.spi_transfer_pins(host,pins,tx,rx)
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(48,self.panel.busy())]}
    fn named_pin(&self,name:&str)->Option<u8> {self.inner.named_pin(name)}
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {self.inner.attach_spi_sd_card(image,config)}
    fn set_sd_card_present(&mut self,present:bool) {self.inner.set_sd_card_present(present);}
    fn next_deadline(&self)->Option<VirtualCycle> {self.panel.next_deadline()}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.panel.advance_to(cycle);}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.panel.take_edges()}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((122,250,self.panel.frame.clone(),self.panel.version))}
    fn display_version(&self)->u64 {self.panel.version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn report(&self)->String {format!("[{}] DEPG0213BN/SSD1680 visible122x250 RAM176x296; origin0 exact GxDEPG0213BN; 4000/800ms modeled full/partial driver deadlines; bytes={} frames={} factory-boot=unverified analog/LUT/temperature/RF unmodeled",self.name(),self.panel.ram_bytes_written,self.panel.frames)}
}

// Two real JD9613 controllers share MOSI/SCLK/DC/reset; both CS low is the
// manufacturer's broadcast initialization, not an erroneous bus-contention case.
const TOUCHBAR_LEFT:Profile=Profile{name:"touchbar-left-jd9613",controller:"JD9613",gram:[126,294],glass:[0,0,126,294],spi:[17,18,13],dc:Some(16),reset:Some(15),light:None,light_pwm:false,enable:None,quad:false,buttons:&[],encoder:None,touch:None,expander:false};
const TOUCHBAR_RIGHT:Profile=Profile{name:"touchbar-right-jd9613",spi:[17,18,14],..TOUCHBAR_LEFT};
struct TouchBar {panels:[LilygoDisplay;2],touch:Arc<Mutex<TouchState>>,wire:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,cycle:VirtualCycle}
impl TouchBar {
    fn new()->Self {Self{panels:[LilygoDisplay::new(&TOUCHBAR_LEFT),LilygoDisplay::new(&TOUCHBAR_RIGHT)],touch:Arc::new(Mutex::new(TouchState{powered:false,asleep:false,down:false,x:0,y:0,width:127,height:621})),wire:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),cycle:0}}
    fn irq(&self)->bool {let t=self.touch.lock().expect("Touch Bar CST820");!t.powered||t.asleep||!t.down}
}
impl BoardModel for TouchBar {
    fn name(&self)->&'static str {"lilygo-t-touch-bar"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;for p in &mut self.panels {p.observe_routes(cycle,&routes);}
        let old=self.irq();{let mut t=self.touch.lock().expect("Touch Bar CST820");t.powered=routes.software_output(9)==Some(true);if !t.powered{t.clear();}}
        let irq=self.irq();if irq!=old{self.edges.push(BoardEdge{cycle,pin:12,level:irq});}
        let scl=routes.software_output(11)!=Some(false);let sda=self.wire.drive(scl,routes.software_output(10)!=Some(false));for(i,(pin,level))in[(10,sda),(11,scl)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {for p in &mut self.panels {p.spi_transfer_pins(host,pins,tx,0);}vec![255;rx]}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {self.wire.clear();self.wire.attach(0x15,Box::new(TouchDevice::new(TouchKind::Cst820,self.touch.clone())));(0..2).flat_map(|bus|self.wire.controller_devices(Some((10,11))).into_iter().map(move |(a,d)|(bus,a,d))).collect()}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(10,self.levels[0]),(11,self.levels[1]),(12,self.irq())]}
    fn touch(&mut self,x:u16,y:u16,down:bool) {let old=self.irq();{let mut t=self.touch.lock().expect("Touch Bar CST820");if !t.powered||t.asleep{return;}let x=x.min(587);t.x=126-y.min(125);t.y=x+if x>=294{32}else{0};t.down=down;}
        let irq=self.irq();if irq!=old{self.edges.push(BoardEdge{cycle:self.cycle,pin:12,level:irq});}}
    fn touch_irq(&self)->Option<(u8,bool)> {Some((12,self.irq()))}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let mut composite=vec![0;588*126];for(i,p)in self.panels.iter().enumerate(){let (_,_,native,_)=p.display()?;
        // Invert the vendor software placement x_native=126-(y+h), y_native=x.
        for y in 0..126{for x in 0..294{composite[y*588+i*294+x]=native[x*126+125-y];}}}
        Some((588,126,composite,self.display_version()))}
    fn display_version(&self)->u64 {self.panels.iter().map(BoardModel::display_version).fold(0,u64::wrapping_add)}
    fn display_frames(&self)->u64 {self.panels.iter().map(BoardModel::display_frames).sum()}
    fn report(&self)->String {format!("[{}] AMOLED H621 variant; two JD9613 native126x294 panels, vendor software90deg rotation horizontally composed588x126; CST820 held-level single-point; 32-coordinate physical touch gap omitted from preview; LED/LCD H612 variant excluded; no factory-boot or calibration claim",self.name())}
}

const HMI_DATA:[u8;8]=[48,47,39,40,41,42,45,46];
struct Hmi {panel:esp_soc::devices::DcsPanel,power:bool,light:bool,wr:Option<bool>,version:u64,present:bool,bytes:u64}
impl Hmi {fn new()->Self {Self{panel:esp_soc::devices::DcsPanel::st7789(),power:false,light:false,wr:None,version:0,present:true,bytes:0}}}
impl BoardModel for Hmi {
    fn name(&self)->&'static str {"lilygo-t-hmi"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:VirtualCycle,routes:PinRoutes<'_>) {
        let power=routes.software_output(10)==Some(true);let light=routes.software_output(38)==Some(true);if self.power&&!power{self.panel.reset();}
        if(self.power,self.light)!=(power,light){self.version=self.version.wrapping_add(1);}self.power=power;self.light=light;
        let wr=routes.software_output(8);if self.wr==Some(false)&&wr==Some(true)&&power&&routes.software_output(6)==Some(false) {if let Some(dc)=routes.software_output(7) {let mut word=0u8;let mut valid=true;for(i,p)in HMI_DATA.into_iter().enumerate(){if let Some(high)=routes.software_output(p){word|=u8::from(high)<<i;}else{valid=false;}}if valid{self.panel.dc=dc;self.panel.byte(word);self.bytes+=1;if !dc{self.version=self.version.wrapping_add(1);}}}}
        self.wr=wr;
    }
    fn lcd_i80(&mut self,pins:LcdI80Pins,_data_phase:bool,words:&[u16]) {if !self.power||pins.width!=8||pins.clock&(1<<8)==0||pins.select&(1<<6)==0||pins.dc&(1<<7)==0||!HMI_DATA.iter().enumerate().all(|(i,p)|pins.data[i]&(1<<p)!=0){return;}
        self.panel.dc=pins.dc_high;for &word in words{self.panel.byte(word as u8);self.bytes+=1;}if !pins.dc_high{self.version=self.version.wrapping_add(1);}}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(21,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name{"boot"=>Some(0),"user"=>Some(21),_=>None}}
    fn sd_card_slot(&self)->Option<([u8;6],bool)> {Some(([12,11,13,255,255,255],self.power&&self.present))}
    fn set_sd_card_present(&mut self,present:bool) {self.present=present;}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((240,320,if self.power&&self.light&&self.panel.on&&!self.panel.sleeping{self.panel.gram.clone()}else{vec![0;240*320]},self.display_version()))}
    fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.version)}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[{}] ST7789 240x320 exact i80 data48/47/39/40/41/42/45/46 WR8 CS6 DC7 power10 BL38; bytes={} BOOT0/user21 and native1-bit SDMMC12/11/13; XPT2046 physical touch unmodeled; USB-present digital subset, battery latch14/ADC/backlight dimming/factory-boot unverified",self.name(),self.bytes)}
}

// ED047TC1 and ET047TC1 are not renamed as each other. The existing two-bit
// optical endpoint is reused because both pinned ED047TC1 drivers explicitly
// send 0x55 for black and 0xaa for white. Analog integration/gray levels remain
// absent; no RGB framebuffer bypasses the actual source/gate drive symbols.
const T5PRO_IO:WiredProfile=WiredProfile{name:"lilygo-t5-e-paper-s3-pro",oled:None,buttons:BOOT,encoder:None,emission_enable:None,led:None,card:Some(CardWiring{pins:[14,13,21,12],other_selects:1<<46}),power:None,scope:"T5S3-4.7-PRO H752-01 BOOT/SPI-SD"};
const EPD47_IO:WiredProfile=WiredProfile{name:"lilygo-t5-4-7-inch-e-paper-v2-3",oled:None,buttons:&[("boot",0),("user",21)],encoder:None,emission_enable:None,led:None,card:Some(CardWiring{pins:[11,15,16,42],other_selects:0}),power:None,scope:"Manufacturer current T5-ePaper-S3 linked by exact storefront slug; shift-register rails/SPI-SD"};
struct RawEpaper {pro:bool,inner:WiredBoard,panel:esp_soc::devices::et047tc1::Et047tc1,expander:Arc<Mutex<esp_periph::i2c::tca9555::Tca9555State>>,wire:I2cBitBang,levels:[bool;2],irq:bool,edges:Vec<BoardEdge>,cycle:VirtualCycle,stv:Option<bool>,clk:Option<bool>,strobe:Option<bool>,shift:u8,bits:u8,config:u8,rgb_routes:bool,rgb_scan:bool}
impl RawEpaper {
    fn new(pro:bool)->Self {let expander=Arc::new(Mutex::new(esp_periph::i2c::tca9555::Tca9555State::default()));
        // TPS power-good is deliberately low while the TPS analog supply is
        // unmodeled. We do not fake a factory power-on success or its ID/PG.
        expander.lock().expect("T5 PCA9535").set_input(14,false);
        Self{pro,inner:WiredBoard::new(if pro{&T5PRO_IO}else{&EPD47_IO}),panel:Default::default(),expander,wire:I2cBitBang::new(),levels:[true;2],irq:true,edges:Vec::new(),cycle:0,stv:None,clk:None,strobe:None,shift:0,bits:0,config:2,rgb_routes:false,rgb_scan:false}}
    fn powered(&self)->bool {if self.pro {let mask=(1<<8)|(1<<11)|(1<<12)|(1<<13);self.expander.lock().expect("T5 PCA9535").driven_high()&mask==mask}else{self.config&0x2e==0x2c}}
    fn sync_irq(&mut self) {if self.pro {let level=self.expander.lock().expect("T5 PCA9535").irq_high();if level!=self.irq{self.irq=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:38,level});}}}
    fn apply_config(&mut self,value:u8) {let previous=self.config;self.config=value;if !self.powered(){self.panel.stop_scan();return;}
        if previous&16!=0&&value&16==0{self.panel.begin_scan();}
        if previous&1==0&&value&1!=0&&value&0xc0==0xc0{self.panel.latch();}}
    fn unshuffle(v:u8)->u16 {u16::from((v>>6)|((v>>2)&12)|((v<<2)&48)|(v<<6))}
}
impl BoardModel for RawEpaper {
    fn name(&self)->&'static str {self.inner.name()}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;
        if self.pro {
            let scl=routes.software_output(40)!=Some(false);let sda=self.wire.drive(scl,routes.software_output(39)!=Some(false));for(i,(pin,level))in[(39,sda),(40,scl)].into_iter().enumerate(){if self.levels[i]!=level{self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
            self.rgb_routes=routes.matrix_output(4,154)&&routes.matrix_output(42,151)&&routes.matrix_output(41,150)&&[18,8,16,17,7,15,5,6].iter().enumerate().all(|(i,&p)|routes.matrix_output(p,141+i as u32));
            let stv=routes.software_output(45);if self.stv==Some(true)&&stv==Some(false)&&self.powered(){self.panel.begin_scan();self.rgb_scan=false;}self.stv=stv;
            if !self.powered(){self.panel.stop_scan();self.rgb_scan=false;}self.sync_irq();
        }else {
            let clk=routes.software_output(12);let strobe=routes.software_output(0);if self.strobe==Some(true)&&strobe==Some(false){self.shift=0;self.bits=0;}
            if self.clk==Some(false)&&clk==Some(true){if let Some(bit)=routes.software_output(13){self.shift=(self.shift<<1)|u8::from(bit);self.bits=self.bits.saturating_add(1);}else{self.bits=9;}}
            if self.strobe==Some(false)&&strobe==Some(true)&&self.bits==8{self.apply_config(self.shift);self.bits=0;}self.clk=clk;self.strobe=strobe;
        }
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {if !self.pro{return Vec::new();}self.wire.clear();self.wire.attach(0x20,Box::new(esp_periph::i2c::tca9555::Tca9555::new(self.expander.clone())));(0..2).flat_map(|bus|self.wire.controller_devices(Some((39,40))).into_iter().map(move|(a,d)|(bus,a,d))).collect()}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {if !self.pro||id!="function"{return false;}self.cycle=cycle;self.expander.lock().expect("T5 PCA9535").set_input(10,!pressed);self.sync_irq();true}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut levels=self.inner.input_levels();if self.pro{levels.extend([(39,self.levels[0]),(40,self.levels[1]),(38,self.expander.lock().expect("T5 PCA9535").irq_high())]);}levels}
    fn named_pin(&self,name:&str)->Option<u8> {self.inner.named_pin(name)}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_irq();std::mem::take(&mut self.edges)}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {self.inner.spi_transfer_pins(host,pins,tx,rx)}
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {self.inner.attach_spi_sd_card(image,config)}
    fn set_sd_card_present(&mut self,present:bool) {self.inner.set_sd_card_present(present);}
    fn lcd_i80(&mut self,pins:LcdI80Pins,data:bool,words:&[u16]) {
        if self.pro||!self.powered()||self.config&0xc1!=0xc0||!self.panel.scanning||!data||pins.width!=8||pins.clock&(1<<41)==0||pins.select!=0||pins.dc&(1<<40)==0||pins.dc_high||words.len()<240||![6,7,4,5,2,3,8,1].iter().enumerate().all(|(i,p)|pins.data[i]&(1<<p)!=0){return;}
        let line:Vec<_>=words.iter().take(240).map(|&v|Self::unshuffle(v as u8)).collect();self.panel.line(&line);
    }
    fn lcd_rgb_start(&mut self,_cycle:VirtualCycle,w:u32,h:u32,bpp:u32,clock:&dyn Fn(u8)->Option<(u64,u32)>) {self.rgb_scan=self.pro&&self.rgb_routes&&self.powered()&&self.panel.scanning&&self.panel.row==0&&w==241&&h==544&&bpp==1&&clock(48).is_some_and(|(period,pulses)|period!=0&&pulses>=h);}
    fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]) {let scan=std::mem::take(&mut self.rgb_scan);if !scan||!self.rgb_routes||!self.powered()||!self.panel.scanning||w!=241||h!=544||bytes.len()!=241*544{return;}
        let mut line=[0u16;240];for row in bytes.as_chunks::<241>().0.iter().take(540){for(word,&v)in line.iter_mut().zip(&row[1..]){*word=Self::unshuffle(v);}self.panel.line(&line);self.panel.latch();}self.panel.stop_scan();}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((960,540,self.panel.frame.clone(),self.panel.version))}
    fn display_version(&self)->u64 {self.panel.version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn display_quiet_push(&self)->bool {true}
    fn report(&self)->String {format!("[{}] ED047TC1 960x540 two-bit black/white drive endpoint; {} ; pixels retained on rail-off; frames={} powered={} factory-boot=unverified gray/analog/temperature/touch/GNSS/RF absent",self.name(),if self.pro{"H752-01 PCA9535 digital rails; TPS651851 PG/charger/RTC absent; RGB241x544 plus finite CKV48"}else{"exact T5-ePaper-S3 8-bit shift rails/8-bit i80; latch advances row; CKV pulse integration unmodeled"},self.panel.frames,self.powered())}
}
