//! e-Paper ESP32 Driver Board V3, SKU15823. This SKU contains a controller and
//! FFC adapter, not a display panel. Official fixed wiring:13CLK,14DIN,15CS,
//! 27DC,26RST,25BUSY. GPIO0 is BOOT; RESET drives EN, not a fabricated GPIO.
//! Only MCU GPIO and a bounded output trace are observed; no panel protocol,
//! busy response, image, high-voltage waveform or refresh completion is invented.
use esp_soc::{BoardModel,board::SpiPins,pins::PinRoutes};
pub const NAME:&str="waveshare-epaper-esp32-driver-v3";
pub const MEMORY_MIB:(usize,usize)=(4,0);
pub struct EpaperDriver {dc:Option<bool>,reset:Option<bool>,commands:u64,data_bytes:u64,last_command:Option<u8>}
impl Default for EpaperDriver {fn default()->Self {Self::new()}}
impl EpaperDriver {pub fn new()->Self {Self{dc:None,reset:None,commands:0,data_bytes:0,last_command:None}}}
impl BoardModel for EpaperDriver {
    fn name(&self)->&'static str {NAME}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {(id=="boot").then_some(0)}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:u64,routes:PinRoutes<'_>) {self.dc=routes.software_output(27);self.reset=routes.software_output(26);}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if pins.sclk&(1<<13)!=0&&pins.mosi&(1<<14)!=0&&pins.cs&(1<<15)!=0&&self.reset==Some(true) {
            if let Some(dc)=self.dc {if dc {self.data_bytes=self.data_bytes.saturating_add(tx.len()as u64);}else{self.commands=self.commands.saturating_add(tx.len()as u64);if let Some(&b)=tx.last(){self.last_command=Some(b);}}}
        }
        vec![0xff;rx_len] // No attached endpoint, hence no fabricated read response.
    }
    fn hardware_diagnostics(&self)->Option<String> {
        Some(format!("{{\"attachedPanel\":false,\"spiOutputCommandBytes\":{},\"spiOutputDataBytes\":{},\"lastCommand\":{},\"busySynthesized\":false}}",self.commands,self.data_bytes,self.last_command.map_or_else(||"null".into(),|v|v.to_string())))
    }
    fn report(&self)->String {format!("[{NAME}] BOOT0; no onboard display; selected connector SPI output command={} data={}; GPIO25 BUSY not driven without an attached panel; EPD waveform/radio/factory boot unmodeled",self.commands,self.data_bytes)}
}
