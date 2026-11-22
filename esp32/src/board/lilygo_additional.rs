//! Individually sourced classic ESP32 LILYGO board subsets, newest first.
//! Exact revisions, source hashes, memory limits and unmodeled peripherals:
//! docs/platform/research/lilygo-classic-additional-profiles-2026-11-22.json.
//! A shared transport does not make radio/modem/PMIC firmware interchangeable.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::board::{Board,BoardModel,BoardEdge,SpiPins,VirtualCycle};
use esp_soc::devices::{DcsPanel,Ws2812Chain,i2c_bitbang::I2cBitBang,oled_mono::{OledMono,OledI2c,OledKind}};
use esp_soc::pins::PinRoutes;
#[path="lilygo_classic_epaper.rs"] mod epaper;

#[derive(Clone,Copy)]
struct Oled {pins:(u8,u8),reset:Option<u8>,spi:Option<OledSpi>,height:u16}
#[derive(Clone,Copy)]
struct OledSpi {wires:[u8;3],dc:u8,enable:[u8;2]}
#[derive(Default)]
struct OledShift {clock:bool,bits:u8,byte:u8,dc:Option<bool>,chunk:Vec<u8>}
impl OledShift {
    fn flush(&mut self,state:&mut OledMono) {if let Some(dc)=self.dc {if !self.chunk.is_empty(){state.write_spi(dc,&self.chunk);self.chunk.clear();}}self.bits=0;self.byte=0;}
    fn drive(&mut self,state:&mut OledMono,selected:bool,clock:Option<bool>,mosi:Option<bool>,dc:Option<bool>) {
        if !selected||clock.is_none()||mosi.is_none()||dc.is_none() {self.flush(state);self.clock=clock.unwrap_or(false);self.dc=None;return;}
        if self.dc!=dc {self.flush(state);self.dc=dc;}
        let clock=clock.unwrap_or(false);
        if !self.clock&&clock {self.byte=(self.byte<<1)|u8::from(mosi==Some(true));self.bits+=1;if self.bits==8 {self.chunk.push(self.byte);self.bits=0;self.byte=0;if self.chunk.len()==1024 {self.flush(state);}}}
        self.clock=clock;
    }
}
#[derive(Clone,Copy)]
struct Lcd {spi:[u8;3],dc:u8,reset:Option<u8>,light:u8,enable:Option<u8>,glass:[usize;4],bgr:bool,round:bool}
struct Profile {
    name:&'static str,buttons:&'static [(&'static str,u8)],encoder:Option<(u8,u8)>,
    oled:Option<Oled>,lcd:Option<Lcd>,rgb:Option<(u8,usize)>,
    indicators:&'static [(u8,bool,[u8;3])],relays:&'static [u8],
}
const EMPTY:Profile=Profile{name:"",buttons:&[],encoder:None,oled:None,lcd:None,rgb:None,indicators:&[],relays:&[]};
const OLED:Oled=Oled{pins:(21,22),reset:None,spi:None,height:64};
const BOOT:&[(&str,u8)]=&[("boot",0)];
const BEAM:Profile=Profile{name:"lilygo-t-beam-meshtastic",buttons:&[("user",38)],oled:Some(OLED),..EMPTY};
const DUAL:Profile=Profile{name:"lilygo-t-lora-dual",buttons:BOOT,rgb:Some((5,1)),..EMPTY};
const LORA13:Profile=Profile{name:"lilygo-lora-v1-3",oled:Some(OLED),..EMPTY};
const SOFTRF:Profile=Profile{name:"lilygo-t-beam-softrf",..BEAM};
const KEYBOARD:Profile=Profile{name:"lilygo-t-display-keyboard",buttons:&[("boot",0),("user",35)],lcd:Some(Lcd{spi:[18,19,5],dc:16,reset:Some(23),light:4,enable:None,glass:[52,40,135,240],bgr:true,round:false}),..EMPTY};
const CAN485:Profile=Profile{name:"lilygo-t-can485",buttons:BOOT,rgb:Some((4,1)),..EMPTY};
const RELAY8:Profile=Profile{name:"lilygo-t-relay-5v-8-channel-relay",indicators:&[(25,true,[0,0,255])],relays:&[33,32,13,12,21,19,18,5],..EMPTY};
const W5500:Profile=Profile{name:"lilygo-t-lite-w5500",oled:Some(OLED),..EMPTY};
// The selected current storefront is the two-WS2812 PICO-V3-02 revision.
// The older discrete RGB example and the unrelated GC9A01 wiki are not applied.
const ENCODER:Profile=Profile{name:"lilygo-t-encoder-esp32",buttons:&[("encoder-press",38)],encoder:Some((36,37)),rgb:Some((25,2)),..EMPTY};
const RELAY4:Profile=Profile{name:"lilygo-t-relay",indicators:&[(25,true,[0,0,255])],relays:&[21,19,18,5],..EMPTY};
const LORA3:Profile=Profile{name:"lilygo-lora3",oled:Some(OLED),indicators:&[(25,true,[0,0,255])],..EMPTY};
const TCXO:Profile=Profile{name:"lilygo-t3-tcxo",buttons:BOOT,oled:Some(Oled{reset:Some(4),..OLED}),indicators:&[(25,true,[0,0,255])],..EMPTY};
const BAO:Profile=Profile{name:"lilygo-t-bao",..EMPTY};
const TTV:Profile=Profile{name:"lilygo-t-tv",oled:Some(Oled{reset:Some(4),spi:Some(OledSpi{wires:[18,23,5],dc:19,enable:[2,33]}),..OLED}),..EMPTY};
const POE:Profile=Profile{name:"lilygo-t-poe-pro",buttons:BOOT,..EMPTY};
const HELIUM:Profile=Profile{name:"lilygo-helium-t-beam",indicators:&[(4,false,[255,0,0])],..BEAM};
const A7608:Profile=Profile{name:"lilygo-t-a7608e-h",buttons:BOOT,..EMPTY};
const A7670:Profile=Profile{name:"lilygo-t-sim-a7670e",buttons:BOOT,..EMPTY};
const T7:Profile=Profile{name:"lilygo-t7-v1-3-mini-32-esp32",buttons:BOOT,indicators:&[(22,false,[0,255,0])],..EMPTY};
const WEIGH:Profile=Profile{name:"lilygo-t-weigh",buttons:BOOT,..EMPTY};
const WATCH2021:Profile=Profile{name:"lilygo-t-watch-2021",buttons:&[("boot",0),("button-2",35),("button-3",34)],lcd:Some(Lcd{spi:[14,13,15],dc:19,reset:Some(27),light:21,enable:Some(5),glass:[0,0,240,240],bgr:true,round:true}),..EMPTY};
const PCIE:Profile=Profile{name:"lilygo-a-t-pcie",indicators:&[(12,false,[0,0,255])],..EMPTY};
const SIM7600EL1:Profile=Profile{name:"lilygo-t-sim7600e-l1",indicators:&[(12,true,[0,0,255])],..EMPTY};
const SIM7600:Profile=Profile{name:"lilygo-t-sim7600",..SIM7600EL1};
const HIGROW:Profile=Profile{name:"lilygo-t-higrow",buttons:&[("boot",0),("user",35)],..EMPTY};
const SIM7070:Profile=Profile{name:"lilygo-t-sim7070g",indicators:&[(12,false,[0,0,255])],..EMPTY};
const SIM7000:Profile=Profile{name:"lilygo-t-sim7000g",..SIM7070};
const TCALL:Profile=Profile{name:"lilygo-t-call-v1-4",indicators:&[(13,true,[0,0,255])],..EMPTY};
const SIM7000E:Profile=Profile{name:"lilygo-t-sim7600e",..EMPTY};
const LION:Profile=Profile{name:"lilygo-t-lion",oled:Some(OLED),buttons:&[("up",32),("down",33),("center",34),("left",36),("right",39)],..EMPTY};
const JOURNAL:Profile=Profile{name:"lilygo-t-journal",oled:Some(Oled{pins:(14,13),height:32,..OLED}),buttons:&[("user",32)],..EMPTY};
const T96:Profile=Profile{name:"lilygo-esp32-oled-v3-0",oled:Some(OLED),..EMPTY};
const INTERNET:Profile=Profile{name:"lilygo-t-internet-com",rgb:Some((12,1)),..EMPTY};
const LORA10:Profile=Profile{name:"lilygo-lora-v1-0-kit",oled:Some(OLED),..EMPTY};
const PROFILES:&[&Profile]=&[&BEAM,&DUAL,&TCXO,&BAO,&LORA13,&TTV,&POE,&SOFTRF,&HELIUM,&KEYBOARD,&A7608,&A7670,&CAN485,&SIM7000E,&T7,&WEIGH,&WATCH2021,&PCIE,&RELAY8,&W5500,&LION,&JOURNAL,&T96,&SIM7600EL1,&SIM7600,&HIGROW,&ENCODER,&INTERNET,&LORA10,&SIM7070,&RELAY4,&SIM7000,&TCALL,&LORA3];

pub fn make_board(name:&str)->Option<Board> {PROFILES.iter().find(|p|p.name==name).map(|&p|Box::new(LilygoClassic::new(p)) as Board).or_else(||epaper::make_board(name))}
struct LilygoClassic {
    profile:&'static Profile,oled:Option<Arc<Mutex<OledMono>>>,oled_shift:OledShift,i2c:I2cBitBang,
    levels:[bool;2],edges:Vec<BoardEdge>,panel:Option<DcsPanel>,dc:Option<bool>,
    reset:bool,supplied:bool,light:u32,gpio_light:u32,pwm:Option<u32>,
    rgb:Option<Ws2812Chain>,leds:Vec<[u8;3]>,relays:Vec<bool>,version:u64,updates:u64,transactions:u64,
}
impl LilygoClassic {
    fn new(profile:&'static Profile)->Self {
        Self{profile,oled:profile.oled.map(|w|Arc::new(Mutex::new(OledMono::new(if w.spi.is_some(){OledKind::Ssd1309}else{OledKind::Ssd1306},128,w.height,0)))),oled_shift:OledShift::default(),i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),panel:profile.lcd.map(|w|if w.round{DcsPanel::new(240,240)}else{DcsPanel::st7789()}),dc:None,reset:false,supplied:false,light:0,gpio_light:0,pwm:None,rgb:profile.rgb.map(|(_,count)|Ws2812Chain::new(count)),leds:vec![[0;3];profile.relays.len()+profile.indicators.len()],relays:vec![false;profile.relays.len()],version:0,updates:0,transactions:0}
    }
    fn pixels(&self)->Option<Vec<u16>> {
        if let Some(oled)=&self.oled {return Some(oled.lock().expect("LILYGO OLED state").frame());}
        let (Some(panel),Some(lcd))=(&self.panel,self.profile.lcd)else{return None;};
        let [x0,y0,w,h]=lcd.glass;let lit=self.supplied&&self.reset&&self.light!=0&&panel.on&&!panel.sleeping;
        let exchange=(panel.madctl&8!=0)!=lcd.bgr;let mut out=vec![0;w*h];
        if lit {for y in 0..h {for x in 0..w {if lcd.round {let(dx,dy)=(2*x as i32+1-w as i32,2*y as i32+1-h as i32);if dx*dx+dy*dy>(w*w)as i32 {continue;}}let mut p=panel.gram[(y+y0)*panel.cols+x+x0];if exchange {p=(p&0x07e0)|((p&0xf800)>>11)|((p&31)<<11);}
            if self.light!=65535 {p=(((((p>>11)&31)as u32*self.light/65535)<<11)|((((p>>5)&63)as u32*self.light/65535)<<5)|((p&31)as u32*self.light/65535))as u16;}out[y*w+x]=p;}}}
        Some(out)
    }
}
impl BoardModel for LilygoClassic {
    fn name(&self)->&'static str {self.profile.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        let mut changed=false;
        for (i,&pin)in self.profile.relays.iter().enumerate() {let on=routes.software_output(pin)==Some(true);changed|=self.relays[i]!=on;self.relays[i]=on;self.leds[i]=if on{[0,0,255]}else{[0;3]};}
        for (i,&(pin,active,color))in self.profile.indicators.iter().enumerate() {let rgb=if routes.software_output(pin)==Some(active){color}else{[0;3]};let at=self.profile.relays.len()+i;changed|=self.leds[at]!=rgb;self.leds[at]=rgb;}
        if changed {self.updates=self.updates.wrapping_add(1);}
        if let (Some(w),Some(state))=(self.profile.oled,&self.oled) {
            if let Some(pin)=w.reset {state.lock().expect("LILYGO OLED state").set_reset(routes.software_output(pin)==Some(true));}
            if let Some(spi)=w.spi {
                self.reset=w.reset.is_none_or(|pin|routes.software_output(pin)==Some(true));self.supplied=spi.enable.iter().all(|&pin|routes.software_output(pin)==Some(true));self.dc=routes.software_output(spi.dc);
                let mut state=state.lock().expect("LILYGO OLED state");state.set_power(self.supplied);
                if self.reset&&self.supplied {let [clk,mosi,cs]=spi.wires;self.oled_shift.drive(&mut state,routes.software_output(cs)==Some(false),routes.software_output(clk),routes.software_output(mosi),self.dc);}else {self.oled_shift=OledShift::default();}
            }else {
                let (sda,scl)=w.pins;let scl_high=routes.software_output(scl)!=Some(false);let sda_high=self.i2c.drive(scl_high,routes.software_output(sda)!=Some(false));
                for (i,(pin,level))in[(sda,sda_high),(scl,scl_high)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
            }
        }
        if let (Some(lcd),Some(panel))=(self.profile.lcd,&mut self.panel) {
            let before=(self.reset,self.supplied,self.light);
            self.reset=lcd.reset.is_none_or(|pin|routes.software_output(pin)==Some(true));self.supplied=lcd.enable.is_none_or(|pin|routes.software_output(pin)==Some(true));
            self.dc=routes.software_output(lcd.dc);self.gpio_light=if routes.software_output(lcd.light)==Some(true){65535}else{0};self.light=self.pwm.unwrap_or(self.gpio_light);
            if before.0&&!self.reset {panel.reset();}
            if before!=(self.reset,self.supplied,self.light) {self.version=self.version.wrapping_add(1);}
        }
    }
    fn uses_gpio_pwm(&self)->bool {self.profile.lcd.is_some()}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        if let Some(lcd)=self.profile.lcd {self.pwm=pwm(lcd.light).map(|(_,duty)|duty);let light=self.pwm.unwrap_or(self.gpio_light);if light!=self.light {self.light=light;self.version=self.version.wrapping_add(1);}}
    }
    fn uses_spi_pins(&self)->bool {self.profile.lcd.is_some()||self.profile.oled.is_some_and(|w|w.spi.is_some())}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if let (Some(w),Some(state))=(self.profile.oled,&self.oled) {if let (Some(spi),Some(dc))=(w.spi,self.dc){let[sclk,mosi,cs]=spi.wires;if self.supplied&&self.reset&&pins.sclk&(1<<sclk)!=0&&pins.mosi&(1<<mosi)!=0&&pins.cs&(1<<cs)!=0 {state.lock().expect("LILYGO OLED state").write_spi(dc,tx);}}}
        if let (Some(lcd),Some(panel),Some(dc))=(self.profile.lcd,&mut self.panel,self.dc) {
            let [sclk,mosi,cs]=lcd.spi;
            if self.supplied&&self.reset&&pins.sclk&(1<<sclk)!=0&&pins.mosi&(1<<mosi)!=0&&pins.cs&(1<<cs)!=0 {
                panel.dc=dc;for &byte in tx {panel.byte(byte);}self.version=self.version.wrapping_add(1);self.transactions=self.transactions.wrapping_add(1);
            }
        }
        vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();let (Some(w),Some(state))=(self.profile.oled,&self.oled)else{return Vec::new();};
        if w.spi.is_some(){return Vec::new();}
        let _=self.i2c.attach(0x3c,Box::new(OledI2c::new(state.clone(),w.pins)));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some(w.pins)).into_iter().map(move|(address,device)|(bus,address,device))).collect()
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if self.profile.rgb.is_some_and(|(data,_)|data==pin){if let Some(rgb)=&mut self.rgb {rgb.from_bits(bits);}}}
    fn input_levels(&self)->Vec<(u8,bool)> {let mut pins:Vec<_>=self.profile.buttons.iter().map(|(_,p)|(*p,true)).collect();if let Some((a,b))=self.profile.encoder {pins.extend([(a,true),(b,true)]);}
        if let Some(w)=self.profile.oled {if w.spi.is_none(){pins.extend([(w.pins.0,self.levels[0]),(w.pins.1,self.levels[1])]);}}pins}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn named_pin(&self,name:&str)->Option<u8> {self.profile.buttons.iter().find(|(id,_)|*id==name).map(|(_,pin)|*pin)}
    fn encoder(&self)->Option<(u8,u8)> {self.profile.encoder}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {if let Some(rgb)=&self.rgb {Some((&rgb.leds,rgb.updates))}else if self.leds.is_empty(){None}else{Some((&self.leds,self.updates))}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let (w,h)=if let Some(state)=&self.oled {state.lock().expect("LILYGO OLED state").dimensions()}else{let lcd=self.profile.lcd?;(lcd.glass[2]as u32,lcd.glass[3]as u32)};Some((w,h,self.pixels()?,self.display_version()))}
    fn display_version(&self)->u64 {self.oled.as_ref().map_or(self.version,|s|s.lock().expect("LILYGO OLED state").version())}
    fn display_frames(&self)->u64 {self.oled.as_ref().map_or_else(||self.panel.as_ref().map_or(0,|p|p.frames),|s|s.lock().expect("LILYGO OLED state").frames())}
    fn hardware_diagnostics(&self)->Option<String> {(!self.relays.is_empty()).then(||format!("{{\"relayEnergized\":{:?},\"relayPins\":{:?},\"physicalContactDynamics\":false}}",self.relays,self.profile.relays))}
    fn report(&self)->String {format!("[{}] source-backed GPIO/LED/display subset; relays={:?} SPI={} OLED={}; RF/modem/PMIC/SD/network/audio/analog/factory-boot=unmodeled",self.profile.name,self.relays,self.transactions,self.oled.as_ref().map_or_else(||"none".into(),|s|s.lock().expect("LILYGO OLED state").report()))}
}
