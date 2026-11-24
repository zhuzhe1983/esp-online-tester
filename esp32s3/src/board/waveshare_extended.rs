//! Exact S3 ETH, Tiny and six-relay SKUs. Primary schematic/BSP identities live
//! in batch-03 research. Shared silicon does not add absent board peripherals.
use esp_periph::sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard};
use esp_soc::{board::{Board,BoardModel,SpiQspiPins},devices::Ws2812Chain,pins::PinRoutes};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Profile {Eth,EthCam,EthPoe,EthPoeCam,EthM,EthCamM,EthPoeM,EthPoeCamM,Tiny,TinyKit,TinyN8R8,TinyN8R8Kit,Relay6}
impl Profile {
    pub const ALL:[Self;13]=[Self::Eth,Self::EthCam,Self::EthPoe,Self::EthPoeCam,Self::EthM,Self::EthCamM,Self::EthPoeM,Self::EthPoeCamM,Self::Tiny,Self::TinyKit,Self::TinyN8R8,Self::TinyN8R8Kit,Self::Relay6];
    pub fn name(self)->&'static str {match self {Self::Eth=>"waveshare-s3-eth",Self::EthCam=>"waveshare-s3-eth-cam-kit",Self::EthPoe=>"waveshare-s3-eth-poe",Self::EthPoeCam=>"waveshare-s3-eth-poe-cam-kit",Self::EthM=>"waveshare-s3-eth-m",Self::EthCamM=>"waveshare-s3-eth-cam-kit-m",Self::EthPoeM=>"waveshare-s3-eth-poe-m",Self::EthPoeCamM=>"waveshare-s3-eth-poe-cam-kit-m",Self::Tiny=>"waveshare-s3-tiny",Self::TinyKit=>"waveshare-s3-tiny-kit",Self::TinyN8R8=>"waveshare-s3-tiny-n8r8",Self::TinyN8R8Kit=>"waveshare-s3-tiny-n8r8-kit",Self::Relay6=>"waveshare-s3-relay-6ch"}}
    pub fn tiny(self)->bool {matches!(self,Self::Tiny|Self::TinyKit|Self::TinyN8R8|Self::TinyN8R8Kit)}
    pub fn eth(self)->bool {!self.tiny()&&self!=Self::Relay6}
    pub fn camera(self)->bool {matches!(self,Self::EthCam|Self::EthPoeCam|Self::EthCamM|Self::EthPoeCamM)}
    pub fn led_pin(self)->u8 {if self.eth(){21}else{38}}
    pub fn boot(self)->bool {!matches!(self,Self::Tiny|Self::TinyN8R8)}
    pub fn memory(self)->(usize,usize) {match self {Self::Tiny|Self::TinyKit=>(4,2),Self::TinyN8R8|Self::TinyN8R8Kit=>(8,8),Self::Relay6=>(16,0),_=>(16,8)}}
}
pub fn profile(name:&str)->Option<Profile>{Profile::ALL.into_iter().find(|p|p.name()==name)}
pub fn memory_mib(name:&str)->Option<(usize,usize)>{profile(name).map(Profile::memory)}
pub fn make_board(name:&str)->Option<Board>{profile(name).map(|p|Box::new(Extended::new(p))as Board)}
pub struct Extended {p:Profile,chain:Ws2812Chain,led:[[u8;3];1],outputs:[Option<bool>;9],sd:Option<Box<SpiSdCard>>,present:bool,cs:Option<bool>,ethernet_bytes:u64}
impl Extended {
    pub fn new(p:Profile)->Self {Self{p,chain:Ws2812Chain::new(1).with_gpio_clock(240_000_000),led:[[0;3]],outputs:[None;9],sd:None,present:false,cs:None,ethernet_bytes:0}}
    fn sync_led(&mut self) {let[r,g,b]=self.chain.leds[0];self.led[0]=if self.p==Profile::Relay6{[g,r,b]}else{[r,g,b]};}
}
impl BoardModel for Extended {
    fn name(&self)->&'static str {self.p.name()}
    fn uses_gpio_edges(&self)->bool {false}
    fn uses_gpio_routes(&self)->bool {true}
    fn uses_gpio_waveform(&self)->bool {true}
    fn gpio_waveform_at(&mut self,c:u64,r:PinRoutes<'_>) {
        let level=r.software_output(self.p.led_pin());self.chain.gpio_drive(c,level.is_some(),level.unwrap_or(false));self.sync_led();
        if self.p==Profile::Relay6 {for(i,pin)in[1,2,41,42,45,46,21].into_iter().enumerate(){self.outputs[i]=r.software_output(pin);}}
        if self.p.eth(){self.outputs[7]=r.software_output(9);self.outputs[8]=if self.p.camera(){r.software_output(8)}else{None};let next=r.software_output(4);if self.cs==Some(false)&&next!=Some(false){if let Some(sd)=&mut self.sd{sd.deselect();}}self.cs=next;}
    }
    fn next_deadline(&self)->Option<u64>{self.chain.gpio_deadline()}
    fn advance_to(&mut self,c:u64){self.chain.advance_gpio(c);self.sync_led();}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==self.p.led_pin(){self.chain.from_bits(bits);self.sync_led();}}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led,self.chain.updates))}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(0,true)]} // also the bare Tiny's strap pad; no physical key is added
    fn named_pin(&self,id:&str)->Option<u8>{if id=="boot"&&self.p.boot(){Some(0)}else{None}}
    fn uses_qspi_pins(&self)->bool {self.p.eth()}
    fn spi_transfer_qspi_pins(&mut self,_host:u8,p:SpiQspiPins,tx:&[u8],rx:usize)->Vec<u8>{
        let valid=!p.unsupported_mode&&!p.sio&&!p.clock_idle_high&&!p.clock_output_edge&&p.command_bits==0&&p.address_bits==0&&p.dummy_cycles==0&&p.data_bits.is_multiple_of(8)&&p.read_bits.is_multiple_of(8)&&[p.command_width,p.address_width,p.data_width,p.read_width].iter().all(|w|*w<=1);
        if !valid{return vec![255;rx];}
        // Ethernet transport is deliberately observation-only: no fabricated
        // W5500 ID, socket state, packets or successful network connection.
        if p.sclk&(1<<13)!=0&&p.data[0]&(1<<11)!=0&&p.cs&(1<<14)!=0&&self.outputs[7]!=Some(false){self.ethernet_bytes=self.ethernet_bytes.saturating_add(tx.len()as u64);}
        if p.sclk&(1<<7)==0||p.data[0]&(1<<6)==0||!self.present{return vec![255;rx];}
        if let Some(sd)=&mut self.sd{let answer=sd.transfer(p.cs&(1<<4)!=0,tx,rx);if !p.keep_cs&&p.software_cs&(1<<4)==0{sd.deselect();}
            if p.input[1]==Some(5){return answer;}}
        vec![255;rx]
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{if !self.p.eth(){return Err("This exact SKU has no TF socket".into());}let card=SpiSdCard::new(image,config)?;let description=card.describe();self.sd=Some(Box::new(card));self.present=true;Ok(description)}
    fn set_sd_card_present(&mut self,present:bool){if present!=self.present{if let Some(sd)=&mut self.sd{sd.power_cycle();}}self.present=present&&self.p.eth();}
    fn hardware_diagnostics(&self)->Option<String>{let b=|v:Option<bool>|v.map_or("null",|v|if v{"true"}else{"false"});Some(format!("{{\"relayCoilDrive\":[{}],\"buzzerGpioDrive\":{},\"w5500ResetHigh\":{},\"cameraEnableHigh\":{},\"w5500SpiBytesObserved\":{},\"ethernetImplemented\":false,\"cameraDvpImplemented\":false,\"loadPhysicsImplemented\":false}}",self.outputs[..6].iter().map(|v|b(*v)).collect::<Vec<_>>().join(","),b(self.outputs[6]),b(self.outputs[7]),b(self.outputs[8]),self.ethernet_bytes))}
    fn report(&self)->String{format!("{} memory={:?} MiB. ETH one GRB WS2812 GPIO21 and SPI SD7/6/5/4; W5500 SPI13/11/12 CS14 RESET9 INT10 observation only; optional exact CAM kits enable8, OV5640 SCCB/DVP unimplemented. Tiny GPIO38 RGB follows documented Arduino2.0.11 neopixelWrite GRB transport; physical BOOT/USB belong to adapter kits. Relay6 GPIO1/2/41/42/45/46 coils, GPIO21 buzzer drive, GPIO38 RGB wire transport; N16 has zero PSRAM. No RTC/IMU/LCD, network services, acoustic/relay-contact/power/USB/analog or radio physics. Factory boot unverified.",self.name(),self.p.memory())}
}
