//! Independently sourced T-Deck sales/hardware variants, not sibling aliases.
//! LilyGoLib 9116bf41 docs/hardware/lilygo-t-deck.md explicitly establishes the
//! original/Plus main GPIO mapping; T-Deck 9aac147a supplies the same pin header
//! and shared firmware. Plus GNSS stays unmodeled and its Grove is unavailable.
//! Pro selects only the manufacturer's documented Meshtastic-capable V1.0.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard}};
use esp_soc::{board::{Board,BoardModel,BoardEdge,VirtualCycle,SpiPins},pins::PinRoutes};
use super::{tdeck::TDeck,lilygo_epaper::LilygoEpaper};

pub fn make_board(name:&str)->Option<Board> {
    let (name,tft):(&'static str,bool)=match name {
        "lilygo-t-deck-pro-meshtastic"=>("lilygo-t-deck-pro-meshtastic",false),
        "lilygo-t-deck-meshtastic"=>("lilygo-t-deck-meshtastic",true),
        "lilygo-t-deck-plus-meshtastic"=>("lilygo-t-deck-plus-meshtastic",true),
        "lilygo-t-deck-plus-1"=>("lilygo-t-deck-plus-1",true),
        _=>return None,
    };
    let inner:Board=if tft {Box::new(TDeck::new())}else{Box::new(LilygoEpaper::t_deck_pro())};
    Some(Box::new(AdditionalDeck{name,inner,tft,card:None,card_present:true,powered:false,dc:None,gpio_events:0}))
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
