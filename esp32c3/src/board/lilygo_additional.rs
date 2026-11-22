//! Exact T-01C3, T8-C3 V1.1 and T-Zigbee V1.0 C3 wiring.
//! docs/platform/research/lilygo-riscv-additional-profiles-2026-11-22.json.
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins},pins::PinRoutes};
use esp_periph::sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard};

#[derive(Clone,Copy,PartialEq,Eq)]
enum Product {T01,T8,Zigbee}
pub struct LilygoAdditionalC3 {
    product:Product,button_high:bool,led:[[u8;3];1],updates:u64,edges:Vec<BoardEdge>,gpio_events:u64,
    card:Option<Box<SpiSdCard>>,card_present:bool,card_selected:bool,
}
impl LilygoAdditionalC3 {
    pub fn t01_c3()->Self {Self::new(Product::T01)}
    pub fn t8_c3()->Self {Self::new(Product::T8)}
    pub fn t_zigbee()->Self {Self::new(Product::Zigbee)}
    fn new(product:Product)->Self {Self{product,button_high:true,led:[[0;3]],updates:0,edges:Vec::new(),gpio_events:0,card:None,card_present:true,card_selected:false}}
}
impl BoardModel for LilygoAdditionalC3 {
    fn name(&self)->&'static str {match self.product {Product::T01=>"lilygo-t-01c3",Product::T8=>"lilygo-t8-c3",Product::Zigbee=>"lilygo-t-zigbee"}}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,_cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.gpio_events+=changes.len() as u64;}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {
        let color=if routes.software_output(3)!=Some(true) {[0;3]}else if self.product==Product::Zigbee {[0,0,255]}else{[0,255,0]};
        if self.led[0]!=color {self.led[0]=color;self.updates=self.updates.wrapping_add(1);}
        if self.product==Product::T8 {
            let selected=routes.software_output(6)==Some(false);
            if self.card_selected && !selected {if let Some(card)=&mut self.card {card.deselect();}}self.card_selected=selected;
        }
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.led,self.updates))}
    fn input_levels(&self)->Vec<(u8,bool)> {match self.product {Product::T01=>Vec::new(),Product::T8=>vec![(8,true),(9,self.button_high)],Product::Zigbee=>vec![(2,self.button_high),(8,true),(9,true)]}}
    fn named_pin(&self,name:&str)->Option<u8> {match (self.product,name) {(Product::T8,"boot"|"button"|"btnA")=>Some(9),(Product::Zigbee,"button"|"btnA"|"user")=>Some(2),_=>None}}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        let Some(pin)=self.named_pin(id) else {return false;};let high=!pressed;
        if self.button_high!=high {self.button_high=high;self.edges.push(BoardEdge{cycle,pin,level:high});}true
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn uses_spi_pins(&self)->bool {self.product==Product::T8}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if self.product==Product::T8 && host==2 && self.card_present && pins.sclk&(1<<4)!=0 && pins.mosi&(1<<7)!=0 {
            if let Some(card)=&mut self.card {
                let result=card.transfer(pins.cs&(1<<6)!=0,tx,rx_len);
                // GPIO CS spans command/data chunks; peripheral CS rises after
                // this transaction, including reads with an incorrect MISO route.
                if !self.card_selected {card.deselect();}
                if pins.miso==Some(5) {return result;}
            }
        }
        vec![0xff;rx_len]
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        if self.product!=Product::T8 {return Err("this exact LilyGO C3 board has no TF socket".into());}
        let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(Box::new(card));Ok(description)
    }
    fn set_sd_card_present(&mut self,present:bool) {if self.card_present!=present {if let Some(card)=&mut self.card {card.power_cycle();}}self.card_present=present;}
    fn report(&self)->String {match self.product {
        Product::T01=>"[lilygo-t-01c3] GPIO3 high-active green LED; GPIO9 is a header download pad, not an onboard button; external relay, analog power and wireless unmodeled".into(),
        Product::T8=>format!("[lilygo-t8-c3] V1.1 GPIO3 green LED, GPIO9 BOOT, SPI TF CLK4/MISO5/MOSI7/CS6; card={}; analog battery/TP4054 charger and wireless unmodeled",self.card.as_ref().map_or_else(||"not attached".into(),|card|card.report())),
        Product::Zigbee=>"[lilygo-t-zigbee] ESP32-C3 subsystem only, GPIO3 high-active blue LED and GPIO2 user button; TLSR8258 MCU/HCI/red-green LEDs and RF/network unmodeled".into(),
    }}
}
