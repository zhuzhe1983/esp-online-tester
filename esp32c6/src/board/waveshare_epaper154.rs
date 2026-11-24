//! ESP32-C6-ePaper-1.54, pinned manufacturer 30d5082567da2fa300fee762f06d9bc6fd027873.
//! Official 1.54-inch V2 data sheet does not identify the controller die.
//! Reuse only the evidenced200x200 SSD16xx-compatible monochrome command subset.
//! TCA9554 EX0 supplies the panel; ink persists when that supply is disabled.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::{BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{
    ssd1681::Ssd1681,tca9554::{Tca9554,Tca9554State},pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311}};
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex}};
type PersistentDevice=Rc<RefCell<Box<dyn I2cDevice>>>;
struct RoutedDevice{device:PersistentDevice,audio_power:Option<Arc<Mutex<Tca9554State>>>}
impl RoutedDevice{fn active(&self)->bool{self.audio_power.as_ref().is_none_or(|s|s.lock().expect("TCA9554 state").driven(1)==Some(true))}}
impl I2cDevice for RoutedDevice{
    fn pins(&self)->Option<(u8,u8)>{Some((18,8))}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,h:Option<u32>)->bool{self.active()&&self.device.borrow_mut().start_address_with_clock(c,a,r,h)}
    fn write(&mut self,b:u8)->bool{self.active()&&self.device.borrow_mut().write(b)}
    fn read(&mut self)->u8{if self.active(){self.device.borrow_mut().read()}else{255}}
    fn stop(&mut self){self.device.borrow_mut().stop();}
}
pub struct Epaper154{
    panel:Ssd1681,expander:Arc<Mutex<Tca9554State>>,rtc:Arc<Mutex<Pcf85063State>>,devices:Vec<(u8,PersistentDevice)>,
    dc:Option<bool>,powered:bool,reset_high:bool,reset_pin:bool,cycle:u64,rejected:u64,sd:Option<SpiSdCard>,
}
impl Default for Epaper154{fn default()->Self{Self::new()}}
impl Epaper154{
    pub fn new()->Self{
        let expander=Arc::new(Mutex::new(Tca9554State::default()));let rtc=Arc::new(Mutex::new(Pcf85063State::new(crate::periph::CPU_HZ)));
        let parts:Vec<(u8,Box<dyn I2cDevice>)>=vec![(0x20,Box::new(Tca9554::new(expander.clone()))),(0x51,Box::new(Pcf85063::new(rtc.clone()))),(0x18,Box::new(Es8311::new()))];
        Self{panel:Ssd1681::new(10,crate::periph::CPU_HZ/1000),expander,rtc,devices:parts.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),dc:None,powered:false,reset_high:false,reset_pin:false,cycle:0,rejected:0,sd:None}
    }
    fn sync_power(&mut self){let powered=self.expander.lock().expect("TCA9554 state").driven(0)==Some(true);let active=powered&&self.reset_pin;
        if self.powered&&!powered{self.panel.power_loss();}else if self.reset_high&&!active{self.panel.reset();}
        self.powered=powered;self.reset_high=active;
    }
}
impl BoardModel for Epaper154{
    fn name(&self)->&'static str{"waveshare-c6-epaper154"}
    fn named_pin(&self,name:&str)->Option<u8>{match name{"boot"|"button"=>Some(9),"power"=>Some(2),_=>None}}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(9,true),(2,true),(10,self.powered&&self.panel.busy())]}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{self.devices.iter().map(|(a,d)|(0,*a,Box::new(RoutedDevice{device:d.clone(),audio_power:(*a==0x18).then(||self.expander.clone())})as Box<dyn I2cDevice>)).collect()}
    fn advance_to(&mut self,cycle:u64){self.cycle=cycle;self.panel.advance_to(cycle);self.rtc.lock().expect("PCF85063 state").advance_to(cycle);self.sync_power();}
    fn next_deadline(&self)->Option<u64>{self.panel.next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge>{self.panel.take_edges()}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>){self.advance_to(cycle);self.dc=r.software_output(15);self.reset_pin=r.software_output(11)==Some(true);self.sync_power();
        let gpio=r.software_output(3);let hardware=[68,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==3));if gpio==Some(true)||(gpio.is_none()&&!hardware){if let Some(sd)=&mut self.sd{sd.deselect();}}}
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        self.sync_power();let floating=vec![255;rx_len];
        if host!=2||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge||p.sclk&(1<<6)==0||p.data[0]&(1<<5)==0
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||p.dummy_cycles!=0{self.rejected+=1;return floating;}
        let epd=p.cs&(1<<7)!=0;let card=p.cs&(1<<3)!=0;
        if epd&&card{self.rejected+=1;if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
        if let Some(sd)=&mut self.sd{let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(card,tx,rx_len)}else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx_len)};
            if card{if p.hardware_cs&(1<<3)!=0&&!p.keep_cs&&p.software_cs&(1<<3)==0{sd.deselect();}return if p.input[1]==Some(4){rx}else{floating};}}
        if epd&&self.reset_high{if rx_len!=0||p.full_duplex{self.rejected+=1;return floating;}
        if let Some(dc)=self.dc{self.panel.write(dc,tx);}}
        floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{let sd=SpiSdCard::new(image,config)?;let description=sd.describe();self.sd=Some(sd);Ok(description)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{Some((200,200,self.panel.frame.clone(),self.panel.version))}
    fn display_version(&self)->u64{self.panel.version}
    fn display_frames(&self)->u64{self.panel.frames}
    fn display_quiet_push(&self)->bool{true}
    fn report(&self)->String{format!("[waveshare-c6-epaper154] 200x200 V2 monochrome/SSD16xx-compatible digital subset; controller die not identified; frames={} RAM_bytes={} powered={} reset={} rejected={}; TCA955420 EX0=EPD EX1=audio; RTC51 codec18; BOOT9 PWR2; SD {}; SHTC3/ADC/audio/optical LUT/temperature/ghosting/orientation/RF unverified",self.panel.frames,self.panel.ram_bytes_written,self.powered,self.reset_high,self.rejected,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}
#[cfg(test)]mod tests{
    use super::*;use esp_periph::{Gpio,RegRam};use esp_soc::pins::ChipPins;
    fn write(d:&mut dyn I2cDevice,a:u8,bytes:&[u8]){assert!(d.start_address_with_clock(a,a,false,None));for v in bytes{assert!(d.write(*v));}d.stop();}
    fn exp(b:&mut Epaper154,r:u8,v:u8){let mut d=b.i2c_devices().remove(0).2;write(&mut *d,0x20,&[r,v]);b.advance_to(b.cycle);}
    fn route(b:&mut Epaper154,dc:bool,rst:bool){let mut g=Gpio::new();let mut m=RegRam::new();for (pin,high)in[(15,dc),(11,rst)]{g.func_out_sel[pin]=128;g.enable|=1<<pin;if high{g.out|=1<<pin;}m.write(4+pin as u32*4,1<<12|1<<9);}b.gpio_waveform_at(b.cycle,ChipPins::C6.routes(&g,&m));}
    fn pins()->SpiQspiPins{SpiQspiPins{sclk:1<<6,data:[1<<5,0,0,0],cs:1<<7,data_width:1,data_bits:8,..Default::default()}}
    fn send(b:&mut Epaper154,c:u8,d:&[u8]){route(b,false,true);b.spi_transfer_qspi_pins(2,pins(),&[c],0);route(b,true,true);b.spi_transfer_qspi_pins(2,SpiQspiPins{data_bits:d.len()as u32*8,..pins()},d,0);}
    #[test]fn waveshare_epaper_expander_power_busy_commit_and_ink_retention(){let mut b=Epaper154::new();assert_eq!(b.named_pin("power"),Some(2));assert!(b.i2c_devices().iter().all(|(_,_,d)|d.pins()==Some((18,8))));
        send(&mut b,0x24,&[0]);assert_eq!(b.panel.ram_bytes_written,0);exp(&mut b,1,1);exp(&mut b,3,0xfe);
        send(&mut b,0x44,&[0,24]);send(&mut b,0x45,&[0,0,199,0]);send(&mut b,0x24,&[0x7f]);assert_eq!(b.display().unwrap().2[0],65535);
        send(&mut b,0x22,&[0xc7]);send(&mut b,0x20,&[]);assert!(b.input_levels().contains(&(10,true)));let deadline=b.next_deadline().unwrap();b.advance_to(deadline-1);assert_eq!(b.display_frames(),0);b.advance_to(deadline);assert_eq!(b.display().unwrap().2[0],0);assert_eq!(b.display_frames(),1);
        exp(&mut b,1,0);assert_eq!(b.display().unwrap().2[0],0,"bistable optical frame survives power loss");assert!(!b.input_levels().contains(&(10,true)));let before=b.panel.ram_bytes_written;send(&mut b,0x24,&[255]);assert_eq!(b.panel.ram_bytes_written,before);
    }
    #[test]fn waveshare_epaper_audio_supply_and_wrong_spi_routes_do_not_ack(){let mut b=Epaper154::new();let mut ds=b.i2c_devices();let d=&mut ds.iter_mut().find(|(_,a,_)|*a==0x18).unwrap().2;assert!(!d.start_address_with_clock(0x18,0x18,false,None));exp(&mut b,1,3);exp(&mut b,3,0xfc);assert!(d.start_address_with_clock(0x18,0x18,false,None));
        send(&mut b,0x24,&[]);let before=b.panel.ram_bytes_written;for wrong in [SpiQspiPins{sclk:1<<5,..pins()},SpiQspiPins{cs:1<<7|1<<3,..pins()},SpiQspiPins{data_width:4,..pins()}]{b.spi_transfer_qspi_pins(2,wrong,&[0],0);}assert_eq!(b.panel.ram_bytes_written,before);
        route(&mut b,true,false);b.spi_transfer_qspi_pins(2,pins(),&[0],0);assert_eq!(b.panel.ram_bytes_written,before);
    }
}
