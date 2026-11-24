//! Non-touch C5 LCD2.73, official revision418de818. Physical32F8P; ILI9488
//! serial RGB666, CH32V00616-bit firmware, SD and three real GPIO keys.
use esp_periph::{i2c::I2cDevice,sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use esp_soc::{BoardModel,board::SpiQspiPins,devices::{imu_motion::ImuMotion,
    qmi8658::Qmi8658,pcf85063::{Pcf85063,Pcf85063State}},pins::PinRoutes};
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use super::{waveshare_ch32v006::{Ch32V006,Ch32V006Port},waveshare_ili9488::Ili9488};
type PersistentDevice=Rc<RefCell<Box<dyn I2cDevice>>>;
struct RoutedDevice(PersistentDevice);
impl I2cDevice for RoutedDevice {
    fn pins(&self)->Option<(u8,u8)>{Some((27,26))}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,h:Option<u32>)->bool{self.0.borrow_mut().start_address_with_clock(c,a,r,h)}
    fn write(&mut self,b:u8)->bool{self.0.borrow_mut().write(b)}
    fn read(&mut self)->u8{self.0.borrow_mut().read()}
    fn stop(&mut self){self.0.borrow_mut().stop();}
}
pub struct Lcd273 {
    panel:Ili9488,port:Arc<Mutex<Ch32V006Port>>,dc:Option<bool>,backlight:u8,lcd_reset_high:bool,lcd_falling:u64,
    generation:u64,rejected:u64,devices:Vec<(u8,PersistentDevice)>,motion:Arc<ImuMotion>,rtc:Arc<Mutex<Pcf85063State>>,sd:Option<SpiSdCard>,
}
impl Default for Lcd273{fn default()->Self{Self::new()}}
impl Lcd273 {
    pub fn new()->Self {
        let port=Arc::new(Mutex::new(Ch32V006Port::new()));
        // Actual reset pullupR36, quiet IMU/RTC IRQ inputs; CHG_DET high in
        // this explicit USB-powered/no-battery scene. Header inputs are low.
        port.lock().unwrap().inputs=0x0f;
        let motion=Arc::new(ImuMotion::default());let rtc=Arc::new(Mutex::new(Pcf85063State::new(crate::bus::CPU_HZ)));
        let parts:Vec<(u8,Box<dyn I2cDevice>)>=vec![
            (0x24,Box::new(Ch32V006::new(port.clone()))),
            // Canonical resources linkQMI8658C.pdf; schematic SA0=GND,6B.
            (0x6b,Box::new(Qmi8658::new_c(motion.clone(),crate::bus::CPU_HZ))),
            (0x51,Box::new(Pcf85063::new(rtc.clone()))),
        ];
        Self{panel:Ili9488::new(),port,dc:None,backlight:0,lcd_reset_high:false,lcd_falling:0,generation:1,rejected:0,
            devices:parts.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),motion,rtc,sd:None}
    }
    fn sync_ports(&mut self){
        let p=self.port.lock().expect("CH32V006 state");let(high,falling,brightness)=(p.level(0)==Some(true),p.falling_edges(0),p.pwm.unwrap_or(0));drop(p);
        if self.lcd_falling!=falling||self.lcd_reset_high&&!high{self.panel.reset();self.generation=self.generation.wrapping_add(1);}
        if self.lcd_reset_high!=high||self.backlight!=brightness{self.generation=self.generation.wrapping_add(1);}
        self.lcd_falling=falling;self.lcd_reset_high=high;self.backlight=brightness;
    }
}
impl BoardModel for Lcd273 {
    fn name(&self)->&'static str{"waveshare-c5-lcd273"}
    fn named_pin(&self,name:&str)->Option<u8>{match name{"boot"|"button"=>Some(28),"prev"=>Some(2),"next"=>Some(3),_=>None}}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(28,true),(2,true),(3,true)]}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{self.devices.iter().map(|(a,d)|(0,*a,Box::new(RoutedDevice(d.clone()))as Box<dyn I2cDevice>)).collect()}
    fn advance_to(&mut self,cycle:u64){self.motion.cycle.store(cycle,Ordering::Relaxed);self.rtc.lock().expect("PCF85063 state").advance_to(cycle);self.sync_ports();}
    fn set_imu_motion(&mut self,mode:u32)->bool{self.motion.mode.store(mode,Ordering::Relaxed);true}
    fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>){
        self.advance_to(cycle);self.dc=r.software_output(4);
        let gpio=r.software_output(9);let hardware=[61,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==9));
        if gpio==Some(true)||(gpio.is_none()&&!hardware){if let Some(sd)=&mut self.sd{sd.deselect();}}
    }
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        self.sync_ports();let floating=vec![0xff;rx_len];
        if host!=2||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||p.sclk&(1<<6)==0||p.data[0]&(1<<7)==0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)
            ||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||p.dummy_cycles!=0{self.rejected+=1;return floating;}
        let lcd=p.cs&(1<<8)!=0;let card=p.cs&(1<<9)!=0;
        if lcd&&card{self.rejected+=1;if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
        if let Some(sd)=&mut self.sd{
            let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(card,tx,rx_len)}else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx_len)};
            if card{if p.hardware_cs&(1<<9)!=0&&!p.keep_cs&&p.software_cs&(1<<9)==0{sd.deselect();}return if p.input[1]==Some(5){rx}else{floating};}
        }
        if lcd&&self.lcd_reset_high&&self.dc.is_some(){
            if rx_len!=0||p.full_duplex{self.rejected+=1;return floating;}
            for &byte in tx{if !self.panel.byte(self.dc==Some(true),byte){self.rejected+=1;}}
            if !tx.is_empty(){self.generation=self.generation.wrapping_add(1);}
        }floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{let sd=SpiSdCard::new(image,config)?;let result=sd.describe();self.sd=Some(sd);Ok(result)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{
        let p=&self.panel;let visible=self.lcd_reset_high&&self.backlight!=0&&p.on&&!p.sleeping;
        // Source driver's mirror(false,false) actually sets MX; no set_gap is
        // used. The visible glass is320rows of the320x480RAM, starting at0.
        let pixels=(0..320).flat_map(|y|(0..320).map(move|x|{
            if !visible{return 0;}let mut v=Ili9488::rgb565(p.gram[y*320+(319-x)]);
            if p.madctl&8==0{v=(v&0x7e0)|((v&0xf800)>>11)|((v&31)<<11);}
            if !p.inverted{v= !v;}
            let d=u32::from(self.backlight);((((u32::from(v>>11)*d/255)<<11)|((u32::from((v>>5)&63)*d/255)<<5))|(u32::from(v&31)*d/255))as u16
        })).collect();Some((320,320,pixels,self.generation))
    }
    fn display_version(&self)->u64{self.generation}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.iter().map(|&v|Ili9488::rgb565(v)).collect(),320,480))}
    fn report(&self)->String{format!("[waveshare-c5-lcd273] ILI9488 DBI-RGB666 320x320/GRAM320x480 pixels={} RAMWR={} duty={}/255 rejected={}; CH32V00624 EX0=LCD; QMI8658C6b PCF85063A51; PREV2 NEXT3 BOOT28; SD {}; virtual USB3V3/no-battery; factory/RF/ADC/SHTC3/power-key/TE unverified",self.panel.pixels_written,self.panel.frames,self.backlight,self.rejected,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}
