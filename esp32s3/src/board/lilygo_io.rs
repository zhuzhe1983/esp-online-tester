//! Exact LilyGO communication/IO boards, ordered by the dated source audit.
//! Sources, represented revisions and unresolved BOM conflicts are retained in
//! docs/platform/research/lilygo-s3-io-profiles-2026-11-22.json.
//! No modem, Ethernet, camera capture, RS232/485/CAN or analog power emulation.
use std::sync::{Arc,Mutex};
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins},pins::PinRoutes,devices::{Ws2812Chain,oled_mono::{OledKind,OledMono,OledI2c}}};

#[derive(Clone,Copy,PartialEq,Eq)]
enum Product {Connect,EthElite,Energy,Sim7670,Rs,EthLite,Relay,Sim7080,Camera,Simcam}

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

pub struct LilygoIo {
    product:Product,buttons:[bool;2],edges:Vec<BoardEdge>,events:u64,plain_led:[[u8;3];1],plain_updates:u64,
    apa:Option<Apa102>,shift:Option<Shift595>,rgb:Option<Ws2812Chain>,
    card:Option<Box<SpiSdCard>>,card_present:bool,card_selected:bool,card_power:bool,
    axp:Option<Arc<Mutex<AxpState>>>,oled:Option<Arc<Mutex<OledMono>>>,
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
    fn new(product:Product)->Self {Self{product,buttons:[true;2],edges:Vec::new(),events:0,plain_led:[[0;3]],plain_updates:0,
        apa:(product==Product::Connect).then(Apa102::new),shift:(product==Product::Relay).then(Shift595::new),rgb:(product==Product::Rs).then(||Ws2812Chain::new(1)),
        card:None,card_present:true,card_selected:false,card_power:product!=Product::Simcam,
        axp:matches!(product,Product::Sim7080|Product::Camera).then(||Arc::new(Mutex::new(AxpState::new()))),
        oled:(product==Product::Camera).then(||Arc::new(Mutex::new(OledMono::new(OledKind::Ssd1306,128,64,0)))),
    }}
    // SCLK,MOSI,MISO,CS: exact board branches, never inferred from a sibling.
    fn sd_spi_pins(&self)->Option<[u8;4]> {match self.product {Product::EthElite=>Some([10,11,9,12]),Product::Sim7670=>Some([21,14,47,13]),Product::EthLite=>Some([7,6,5,42]),Product::Simcam=>Some([39,38,40,47]),_=>None}}
}
impl BoardModel for LilygoIo {
    fn name(&self)->&'static str {match self.product {Product::Connect=>"lilygo-t-connect",Product::EthElite=>"lilygo-t-eth-elite",Product::Energy=>"lilygo-t-energy-s3",Product::Sim7670=>"lilygo-t-sim7670g-s3",Product::Rs=>"lilygo-t-rs-s3",Product::EthLite=>"lilygo-t-eth-lite",Product::Relay=>"lilygo-t-relay-s3",Product::Sim7080=>"lilygo-t-sim7080-s3",Product::Camera=>"lilygo-t-camera-s3",Product::Simcam=>"lilygo-t-simcam"}}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_events(&self)->u64 {self.events}
    fn gpio_output_at(&mut self,_cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.events=self.events.wrapping_add(changes.len() as u64);}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {
        if let Some(apa)=&mut self.apa {apa.gpio(routes.software_output(3),routes.software_output(8));}
        if let Some(shift)=&mut self.shift {shift.gpio(&routes);}
        let color=match self.product {Product::EthElite if routes.software_output(38)==Some(true)=>[0,255,0],Product::Sim7670 if routes.software_output(12)==Some(false)=>[0,0,255],_=>[0;3]};
        if self.plain_led[0]!=color {self.plain_led[0]=color;self.plain_updates=self.plain_updates.wrapping_add(1);}
        if let Some([_,_,_,cs])=self.sd_spi_pins() {
            let powered=self.product!=Product::Simcam || routes.software_output(1)==Some(true);
            if self.card_power&&!powered {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_power=powered;
            let selected=powered&&routes.software_output(cs)==Some(false);
            if self.card_selected&&!selected {if let Some(card)=&mut self.card {card.deselect();}}self.card_selected=selected;
        }
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==1 {if let Some(rgb)=&mut self.rgb {rgb.from_bits(bits);}}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {
        if let Some(apa)=&self.apa {return Some((&apa.leds,apa.updates));}
        if let Some(shift)=&self.shift {return Some((&shift.leds,shift.updates));}
        if let Some(rgb)=&self.rgb {return Some((&rgb.leds,rgb.updates));}
        matches!(self.product,Product::EthElite|Product::Sim7670).then_some((&self.plain_led,self.plain_updates))
    }
    fn input_levels(&self)->Vec<(u8,bool)> {let mut v=vec![(0,self.buttons[0])];if self.product==Product::Rs {v.push((5,self.buttons[1]));}v}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"btnA"=>Some(0),"button" if self.product!=Product::Rs=>Some(0),"user"|"button"|"btnB" if self.product==Product::Rs=>Some(5),_=>None}}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        let Some(pin)=self.named_pin(id) else{return false;};let index=usize::from(pin!=0);let high=!pressed;
        if self.buttons[index]!=high {self.buttons[index]=high;self.edges.push(BoardEdge{cycle,pin,level:high});}true
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn uses_spi_pins(&self)->bool {self.apa.is_some()||self.sd_spi_pins().is_some()}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if matches!(host,2|3) {
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
            if let Some(state)=&self.axp {let pins=if self.product==Product::Sim7080 {(15,7)}else{(7,6)};v.push((controller,0x34,Box::new(AxpSubset{state:state.clone(),pins,pointer:0,address:true})));}
            if let Some(state)=&self.oled {v.push((controller,0x3c,Box::new(OledI2c::new(state.clone(),(7,6)))));}
        }v
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let state=self.oled.as_ref()?.lock().expect("LilyGO OLED mutex poisoned");Some((128,64,state.frame(),state.version()))}
    fn display_version(&self)->u64 {self.oled.as_ref().map_or(0,|p|p.lock().expect("LilyGO OLED mutex poisoned").version())}
    fn display_frames(&self)->u64 {self.oled.as_ref().map_or(0,|p|p.lock().expect("LilyGO OLED mutex poisoned").frames())}
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
        };format!("[{}] BOOT0; {}; SPI-SD={}; RF/network unmodeled",self.name(),io,self.card.as_ref().map_or_else(||"not attached".into(),|c|c.report()))
    }
}
