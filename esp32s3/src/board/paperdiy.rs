//! K159-B parallel ET047TC1, manufacturer schematic and M5GFX 02107b828acb.
//! A digital black/white endpoint, not calibrated 16-gray electrophoretic physics.
use std::{rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,m5::{M5pm1,M5pm1State}};
use esp_periph::sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,LcdI80Pins,SpiPins};
use esp_soc::devices::{i2c_bitbang::I2cBitBang,et047tc1::Et047tc1};

struct TfSlot {card:Option<Box<SpiSdCard>>,powered:bool}
impl TfSlot {
    fn set_power(&mut self,on:bool) {if self.powered&&!on {if let Some(card)=&mut self.card {card.power_cycle();}}self.powered=on;}
}
// K159-B schematic 2026-09-28: J2 VDD=3V3_L2, U5 EN=PY_MPWR_EN,
// driven by M5PM1 DCDC3V3_EN_PP. The mechanical detect switch is G48.
struct PmicWithTf {inner:M5pm1,state:Arc<Mutex<M5pm1State>>,slot:Rc<Mutex<TfSlot>>}
impl I2cDevice for PmicWithTf {
    fn start(&mut self,read:bool)->bool {self.inner.start(read)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,read:bool,hz:Option<u32>)->bool {self.inner.start_address_with_clock(c,a,read,hz)}
    fn write(&mut self,b:u8)->bool {let ack=self.inner.write(b);let p=self.state.lock().expect("PaperDIY PMIC poisoned");self.slot.lock().expect("PaperDIY TF poisoned").set_power(p.dcdc_enabled&&!p.power_off_requested);ack}
    fn read(&mut self)->u8 {self.inner.read()}
    fn stop(&mut self) {self.inner.stop();}
}
pub struct PaperDiy {pmic:Arc<Mutex<M5pm1State>>,sd:Rc<Mutex<TfSlot>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,output:u64,enabled:u64,panel:Et047tc1,gpio_events:u64}
impl PaperDiy {
    pub fn new()->Self {Self{pmic:Default::default(),sd:Rc::new(Mutex::new(TfSlot{card:None,powered:true})),i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),output:0,enabled:0,panel:Et047tc1::new(),gpio_events:0}}
    fn powered(&self)->bool {let p=self.pmic.lock().expect("PaperDIY PMIC mutex poisoned");!p.power_off_requested && p.driven_high&4!=0 && self.enabled&self.output&(1<<46)!=0}

}
impl Default for PaperDiy {fn default()->Self {Self::new()}}
impl BoardModel for PaperDiy {
    fn name(&self)->&'static str {"paperdiy"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        let previous=self.output;if previous&(1<<47)==0 && output&(1<<47)!=0 {if let Some(sd)=&mut self.sd.lock().expect("PaperDIY TF poisoned").card {sd.deselect();}}
        self.output=output;self.enabled=enabled;self.gpio_events+=changes.len() as u64;
        let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=high(42);let sda=self.i2c.drive(scl,high(41));for (i,(pin,level)) in [(41,sda),(42,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        if !self.powered() {self.panel.stop_scan();return;}
        // SPV low is the gate-driver frame start. The driver's three CKV preamble pulses
        // carry no source data and do not create rows. Latch each actual source line at XLE.
        if previous&(1<<17)!=0 && output&(1<<17)==0 {self.panel.begin_scan();}
        if previous&(1<<15)==0 && output&(1<<15)!=0 {self.panel.latch();}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x6e,Box::new(PmicWithTf{inner:M5pm1::new(self.pmic.clone()),state:self.pmic.clone(),slot:self.sd.clone()}));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((41,42))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(3,true),(4,true),(41,self.levels[0]),(42,self.levels[1]),(48,self.sd.lock().expect("PaperDIY TF poisoned").card.is_none())]}
    fn uses_spi_pins(&self)->bool {true}
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {let card=SpiSdCard::new(image,config)?;let description=card.describe();let mut slot=self.sd.lock().expect("PaperDIY TF poisoned");if slot.card.is_none(){self.edges.push(BoardEdge{cycle:0,pin:48,level:false});}slot.card=Some(Box::new(card));Ok(description)}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        // M5PaperDIY-UserDemo 60a26f0 hal_tf_card.cpp: independent SPI routes,
        // not the unrelated FPC CS/G21 or the native SDMMC controller.
        if pins.sclk&(1<<39)==0 || pins.mosi&(1<<38)==0 {return vec![255;rx_len];}
        let mut slot=self.sd.lock().expect("PaperDIY TF poisoned");if !slot.powered {return vec![255;rx_len];}
        let Some(card)=&mut slot.card else {return vec![255;rx_len]};let result=card.transfer(pins.cs&(1<<47)!=0,tx,rx_len);if pins.miso==Some(40){result}else{vec![255;rx_len]}
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn lcd_i80(&mut self,pins:LcdI80Pins,data_phase:bool,words:&[u16]) {
        let data=[6,14,7,12,9,11,8,10];
        if data_phase && pins.width==8 && pins.clock&(1<<16)!=0 && pins.select&(1<<13)!=0 && data.iter().enumerate().all(|(i,p)|pins.data[i]&(1<<p)!=0) && self.powered() && self.panel.scanning && self.output&(1<<15)==0 && self.output&(1<<18)!=0 {
            self.panel.line(words);
        }
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((540,960,self.panel.portrait(),self.panel.version))}
    fn display_version(&self)->u64 {self.panel.version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn display_quiet_push(&self)->bool {true}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button-a"|"btnA"=>Some(4),"button-b"|"btnB"=>Some(3),_=>None}}
    fn report(&self)->String {let powered=self.powered();let slot=self.sd.lock().expect("PaperDIY TF poisoned");format!("[paperdiy] ET047TC1 bw frames={} row={} powered={} gray/analog/network=unverified SPI-SD={} TF-power={}",self.panel.frames,self.panel.row,powered,slot.card.as_ref().map_or_else(||"absent".into(),|sd|sd.report()),slot.powered)}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tf_routes_detect_power_and_reinitialization() {
        let mut b=PaperDiy::new();let pins=SpiPins{sclk:1<<39,mosi:1<<38,miso:Some(40),cs:1<<47};let cmd=[0x40,0,0,0,0,0x95];
        assert_eq!(b.input_levels().last(),Some(&(48,true)));
        b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
        assert_eq!(b.input_levels().last(),Some(&(48,false)));assert!(b.take_edges().iter().any(|e|e.pin==48&&!e.level),"insertion reaches the already-attached GPIO input");assert!(b.sd_card_slot().is_none());
        b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);
        for bad in [SpiPins{sclk:1<<38,..pins},SpiPins{mosi:1<<39,..pins},SpiPins{cs:1<<21,..pins}] {b.spi_transfer_pins(2,bad,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255]);}
        b.spi_transfer_pins(3,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(3,SpiPins{miso:Some(39),..pins},&[],2),[255,255]);
        b.spi_transfer_pins(3,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(3,pins,&[],2),[255,1]);
        let mut devices=b.i2c_devices();let pm=&mut devices.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x6e).unwrap().2;
        pm.start(false);pm.write(6);pm.write(0x15);pm.stop();assert!(!b.sd.lock().unwrap().powered);
        b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255]);assert_eq!(b.input_levels().last(),Some(&(48,false)),"mechanical switch still closed without supply");
        pm.start(false);pm.write(6);pm.write(0x17);pm.stop();assert!(b.sd.lock().unwrap().powered);
        b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255],"power loss clears SPI initialization");
        b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,1]);
        b.i2c_devices();assert!(b.sd.lock().unwrap().card.is_some(),"controller reattachment preserves nonvolatile media");
    }
    #[test]
    fn power_routes_frame_start_and_latch_control_retained_pixels() {
        let mut b=PaperDiy::new();let mut pins=LcdI80Pins{width:8,clock:1<<16,select:1<<13,..Default::default()};
        for (i,p) in [6,14,7,12,9,11,8,10].into_iter().enumerate(){pins.data[i]=1<<p;}
        let mask=(1<<46)|(1<<18)|(1<<17)|(1<<15);b.gpio_output_at(1,&[],mask,(1<<46)|(1<<17));
        b.lcd_i80(pins,true,&[0x55;248]);assert!(b.panel.pending.is_empty(),"no PMIC rail");
        b.pmic.lock().unwrap().driven_high=4;b.gpio_output_at(2,&[],mask,(1<<46)|(1<<18));
        let wrong=LcdI80Pins{clock:1<<5,..pins};b.lcd_i80(wrong,true,&[0x55;248]);assert!(b.panel.pending.is_empty());
        b.lcd_i80(pins,true,&[0x55;248]);assert_eq!(b.panel.frame[0],0xffff,"source latch not yet strobed");
        b.gpio_output_at(3,&[],mask,(1<<46)|(1<<15));assert_eq!(b.panel.frame[..960],[0;960]);assert_eq!(b.panel.row,1);
        b.gpio_output_at(4,&[],mask,0);assert_eq!(b.panel.frame[0],0,"e-paper retains optical state without power");assert!(!b.panel.scanning);
        assert_eq!(b.named_pin("button-a"),Some(4));assert_eq!(b.named_pin("button-b"),Some(3));assert_eq!(b.named_pin("power"),None);
        assert_eq!(b.input_levels().last(),Some(&(48,true)),"SPI TF card absent");
    }
}
