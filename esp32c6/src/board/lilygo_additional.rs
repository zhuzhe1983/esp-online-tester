//! Exact T-LoRa-C6 V1.0 and T-Pico2350 V1.2 ESP32-C6 subsystem.
//! Manufacturer schematics/examples and byte receipts:
//! docs/platform/research/lilygo-riscv-additional-profiles-2026-11-22.json.
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins},pins::PinRoutes};
use esp_soc::devices::sx1262::{Sx1262,LoRaInput};

pub struct TLoRaC6 {
    radio:Sx1262,cycle:u64,reset_high:bool,rx_path:bool,tx_path:bool,selected:bool,
    boot_high:bool,levels:[bool;2],led:[[u8;3];1],updates:u64,edges:Vec<BoardEdge>,gpio_events:u64,
}
impl TLoRaC6 {
    pub fn new()->Self {
        let mut s=Self{radio:Sx1262::new(crate::periph::CPU_HZ),cycle:0,reset_high:false,rx_path:false,tx_path:false,selected:false,boot_high:true,levels:[false;2],led:[[0;3]],updates:0,edges:Vec::new(),gpio_events:0};s.sync();s
    }
    fn sync(&mut self) {
        // RadioLib setRfSwitchPins(RX15,TX14): RX=high/low, TX=low/high.
        // Antenna switching affects packets, never SPI discovery/configuration.
        let antenna=if self.radio.is_receiving() {self.rx_path}else{self.tx_path};
        self.radio.controls(true,self.reset_high,antenna);
        for (index,(pin,level)) in [(22,self.radio.busy()),(23,self.radio.irq())].into_iter().enumerate() {
            if self.levels[index]!=level {self.levels[index]=level;self.edges.push(BoardEdge{cycle:self.cycle,pin,level});}
        }
    }
}
impl Default for TLoRaC6 {fn default()->Self {Self::new()}}
impl BoardModel for TLoRaC6 {
    fn name(&self)->&'static str {"lilygo-t-lora-c6"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,_cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.gpio_events+=changes.len() as u64;}
    fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>) {
        self.advance_to(cycle);self.reset_high=routes.software_output(21)==Some(true);
        self.rx_path=routes.software_output(15)==Some(true) && routes.software_output(14)==Some(false);
        self.tx_path=routes.software_output(14)==Some(true) && routes.software_output(15)==Some(false);
        let color=if routes.software_output(7)==Some(false) {[0,255,0]}else{[0;3]};
        if self.led[0]!=color {self.led[0]=color;self.updates=self.updates.wrapping_add(1);}
        self.sync();self.selected=routes.software_output(18)==Some(false);self.radio.select(self.selected);self.sync();
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.sync();if host!=2 || pins.sclk&(1<<6)==0 || pins.mosi&1==0 || pins.cs&(1<<18)==0 {return vec![0xff;rx_len];}
        if !self.selected {self.radio.select(true);}
        let rx=self.radio.transfer(tx,rx_len);if !self.selected {self.radio.select(false);}self.sync();
        if rx_len==0 || pins.miso==Some(1) {rx}else{vec![0xff;rx_len]}
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(8,true),(9,self.boot_high),(22,self.levels[0]),(23,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"button"|"btnA"=>Some(9),_=>None}}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        if self.named_pin(id)!=Some(9) {return false;}self.advance_to(cycle);let high=!pressed;
        if self.boot_high!=high {self.boot_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:9,level:high});}true
    }
    fn advance_to(&mut self,cycle:u64) {
        let end=self.cycle.max(cycle);self.sync();
        while let Some(at)=self.radio.next_deadline().filter(|&at|at<=end) {self.radio.advance_to(at);self.cycle=at;self.sync();}
        self.radio.advance_to(end);self.cycle=end;self.sync();
    }
    fn next_deadline(&self)->Option<u64> {self.radio.next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.led,self.updates))}
    fn lora_receive_at(&mut self,cycle:u64,packet:&LoRaInput)->bool {self.advance_to(cycle);self.radio.receive(&packet.config,&packet.payload,packet.rssi_dbm,packet.snr_quarter_db,packet.crc_error)}
    fn lora_snapshot(&self)->Option<String> {Some(self.radio.snapshot())}
    fn report(&self)->String {format!("[lilygo-t-lora-c6] GPIO9 BOOT, GPIO7 low-active green LED; SX1262 commands={} tx={} rx={} dropped={} rejected={}; RX15/TX14 switch gates, NRST21, BUSY22, DIO1=23; digital packets only, no RF propagation, antenna gain or Wi-Fi/BLE/802.15.4 network",self.radio.stats.commands,self.radio.stats.tx,self.radio.stats.rx,self.radio.stats.dropped,self.radio.stats.rejected)}
}

/// Only the ESP32-C6 coprocessor is simulated. RP2350 owns the main display,
/// touch, TF, HDMI, PMU, expander and GPIO23 user key; none is remapped to C6.
pub struct TPicoC6 {boot_high:bool,edges:Vec<BoardEdge>}
impl TPicoC6 {pub fn new()->Self {Self{boot_high:true,edges:Vec::new()}}}
impl Default for TPicoC6 {fn default()->Self {Self::new()}}
impl BoardModel for TPicoC6 {
    fn name(&self)->&'static str {"lilygo-t-pico-c6"}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(8,true),(9,self.boot_high)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"button"|"btnA"=>Some(9),_=>None}}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        if self.named_pin(id)!=Some(9) {return false;}let high=!pressed;
        if self.boot_high!=high {self.boot_high=high;self.edges.push(BoardEdge{cycle,pin:9,level:high});}true
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn report(&self)->String {"[lilygo-t-pico-c6] T-Pico2350 V1.2 ESP32-C6 coprocessor only, physical RP2350 + ESP32-C6; SW6 GPIO9 BOOT; RP2350 execution/display/touch/TF/HDMI, AT bridge, power and wireless unmodeled".into()}
}
