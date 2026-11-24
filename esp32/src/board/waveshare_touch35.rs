//! Classic ESP32-Touch-LCD-3.5, 16 MiB Flash / 2 MiB PSRAM.
//! Waveshare 51f354d2aeb2245fd7b748c9f3ea93cea1c527ae factory BSP and
//! ST7796 / FT6336 drivers; retained sources: docs/research/waveshare-classic-touch35-2026-11-24.
//! Digital subset: SPI RGB565, TCA9554 resets, FT6x36 single contact, PCF85063
//! calendar, ES8311 control, AXP2101 reference registers, BOOT and SYS_OUT.
//! No audio samples, PMIC rail-to-MCU reset, charging, touch gestures or complete factory boot claim.
use std::{cell::{Cell,RefCell},rc::Rc,sync::{Arc,Mutex}};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State}};
use esp_periph::sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard};
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins,VirtualCycle},pins::PinRoutes};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang,tca9554::{Tca9554,Tca9554State},
    pcf85063::{Pcf85063,Pcf85063State},axp2101::{Axp2101,Axp2101Power},es8311::Es8311};

pub const NAME:&str="waveshare-esp32-touch35";
pub const MEMORY_MIB:(usize,usize)=(16,2);
const WIDTH:usize=320;
const HEIGHT:usize=480;

struct Expander {
    device:Tca9554,state:Arc<Mutex<Tca9554State>>,panel:Rc<RefCell<DcsPanel>>,
    touch:Arc<Mutex<Ft6336State>>,touch_resets:Rc<Cell<u64>>,version:Rc<Cell<u64>>,
}
impl I2cDevice for Expander {
    fn start(&mut self,read:bool)->bool {self.device.start(read)}
    fn write(&mut self,value:u8)->bool {
        let before=self.state.lock().unwrap().levels();let ack=self.device.write(value);
        let after=self.state.lock().unwrap().levels();
        if before!=after {self.version.set(self.version.get().wrapping_add(1));}
        if before&1!=0&&after&1==0 {self.panel.borrow_mut().reset();}
        if before&2!=0&&after&2==0 {
            self.touch.lock().unwrap().reset();
            self.touch_resets.set(self.touch_resets.get().wrapping_add(1));
        }
        ack
    }
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}
struct Touch {
    device:Ft6336g,state:Arc<Mutex<Ft6336State>>,expander:Arc<Mutex<Tca9554State>>,
    resets:Rc<Cell<u64>>,observed:u64,
}
impl I2cDevice for Touch {
    fn start(&mut self,read:bool)->bool {
        if self.observed!=self.resets.get() {self.observed=self.resets.get();self.device=Ft6336g::new(self.state.clone());}
        self.state.lock().unwrap().powered=self.expander.lock().unwrap().high(1);
        self.device.start(read)
    }
    fn write(&mut self,value:u8)->bool {self.device.write(value)}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}
struct Codec(Rc<RefCell<Es8311>>);
impl I2cDevice for Codec {
    fn start(&mut self,read:bool)->bool {self.0.borrow_mut().start(read)}
    fn write(&mut self,value:u8)->bool {self.0.borrow_mut().write(value)}
    fn read(&mut self)->u8 {self.0.borrow_mut().read()}
    fn stop(&mut self) {self.0.borrow_mut().stop();}
}

pub struct WaveshareTouch35 {
    panel:Rc<RefCell<DcsPanel>>,expander:Arc<Mutex<Tca9554State>>,touch:Arc<Mutex<Ft6336State>>,
    rtc:Arc<Mutex<Pcf85063State>>,power:Arc<Mutex<Axp2101Power>>,codec:Rc<RefCell<Es8311>>,
    version:Rc<Cell<u64>>,touch_resets:Rc<Cell<u64>>,i2c:I2cBitBang,i2c_levels:[bool;2],
    dc:Option<bool>,command:u8,gpio_light:u32,light:u32,pwm:Option<u32>,
    boot_down:bool,power_down:bool,contact_down:bool,irq_high:bool,cycle:VirtualCycle,rtc_horizon:VirtualCycle,rtc_elapsed:VirtualCycle,
    edges:Vec<BoardEdge>,card:Option<SpiSdCard>,spi_bytes:u64,dropped:u64,
}
impl Default for WaveshareTouch35 {fn default()->Self {Self::new()}}
impl WaveshareTouch35 {
    pub fn new()->Self {Self{
        panel:Rc::new(RefCell::new(DcsPanel::new(WIDTH,HEIGHT))),expander:Default::default(),touch:Default::default(),
        rtc:Arc::new(Mutex::new(Pcf85063State::new(crate::periph::CPU_HZ))),power:Default::default(),
        codec:Rc::new(RefCell::new(Es8311::new())),version:Default::default(),touch_resets:Default::default(),
        i2c:I2cBitBang::new(),i2c_levels:[true;2],dc:None,command:0,gpio_light:0,light:0,pwm:None,
        boot_down:false,power_down:false,contact_down:false,irq_high:true,cycle:0,rtc_horizon:0,rtc_elapsed:0,edges:Vec::new(),card:None,spi_bytes:0,dropped:0,
    }}
    fn changed(&self) {self.version.set(self.version.get().wrapping_add(1));}
    fn irq_level(&self)->bool {!self.contact_down||!self.expander.lock().unwrap().high(1)}
}
impl BoardModel for WaveshareTouch35 {
    fn name(&self)->&'static str {NAME}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;self.dc=routes.software_output(27);
        self.gpio_light=if routes.software_output(25)==Some(true){65535}else{0};
        let light=self.pwm.unwrap_or(self.gpio_light);if self.light!=light {self.light=light;self.changed();}
        let scl=routes.software_output(22)!=Some(false);
        let sda=self.i2c.drive(scl,routes.software_output(21)!=Some(false));
        for (i,(pin,level))in[(21,sda),(22,scl)].into_iter().enumerate() {
            if self.i2c_levels[i]!=level {self.i2c_levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}
        }
        if routes.software_output(15)==Some(true) {if let Some(card)=&mut self.card {card.deselect();}}
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        self.pwm=pwm(25).map(|(_,d)|d);let light=self.pwm.unwrap_or(self.gpio_light);
        if light!=self.light {self.light=light;self.changed();}
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if pins.sclk&(1<<18)==0||(!tx.is_empty()&&pins.mosi&(1<<23)==0) {
            self.dropped+=tx.len()as u64;return vec![0xff;rx_len];
        }
        let lcd=pins.cs&(1<<5)!=0;let sd=pins.cs&(1<<15)!=0;
        // Shared wires: two selected devices are not silently prioritized.
        if lcd&&sd {self.dropped+=tx.len()as u64;return vec![0xff;rx_len];}
        if !lcd {
            if let Some(card)=&mut self.card {
                let reply=card.transfer(sd,tx,rx_len);if pins.miso==Some(19) {return reply;}
            }
            return vec![0xff;rx_len];
        }
        let Some(dc)=self.dc else {return vec![0xff;rx_len];};
        if !self.expander.lock().unwrap().high(0) {return vec![0xff;rx_len];}
        let mut p=self.panel.borrow_mut();p.dc=dc;
        for &byte in tx {
            if !dc {self.command=byte;}
            // ST7796 vendor setup and gamma are retained only as command traffic.
            // Pixel decoding is qualified solely for the official 16-bit path.
            if dc&&self.command==0x2c&&p.colmod&7!=5 {self.dropped+=1;continue;}
            p.byte(byte);self.spi_bytes+=1;
        }
        drop(p);if !tx.is_empty(){self.changed();}vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();
        self.i2c.attach(0x20,Box::new(Expander{device:Tca9554::new(self.expander.clone()),state:self.expander.clone(),panel:self.panel.clone(),touch:self.touch.clone(),touch_resets:self.touch_resets.clone(),version:self.version.clone()}));
        // Shared FT6x36 single-contact format is verified against this exact
        // FT6336 driver. No G/U silicon identity or gesture implementation inferred.
        self.i2c.attach(0x38,Box::new(Touch{device:Ft6336g::new(self.touch.clone()),state:self.touch.clone(),expander:self.expander.clone(),resets:self.touch_resets.clone(),observed:self.touch_resets.get()}));
        self.i2c.attach(0x51,Box::new(Pcf85063::new(self.rtc.clone())));
        self.i2c.attach(0x34,Box::new(Axp2101::new(self.power.clone())));
        self.i2c.attach(0x18,Box::new(Codec(self.codec.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((21,22))).into_iter().map(move|(address,device)|(bus,address,device))).collect()
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        let card=SpiSdCard::new(image,config)?;let description=card.describe();self.card=Some(card);Ok(description)
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,!self.boot_down),(36,self.power_down),(37,self.irq_level()),(21,self.i2c_levels[0]),(22,self.i2c_levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {if name=="boot"{Some(0)}else{None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        let (pin,level)=match id {"boot"=>{self.boot_down=pressed;(0,!pressed)},"power"=>{self.power_down=pressed;(36,pressed)},_=>return false};
        // SYS_OUT is only its documented digital observation, not MCU power-off.
        self.edges.push(BoardEdge{cycle,pin,level});true
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        self.contact_down=down;let enabled=self.expander.lock().unwrap().high(1);
        self.touch.lock().unwrap().update(x.min(319),y.min(479),down&&enabled);
    }
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.cycle=cycle;self.touch(x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)> {Some((37,self.irq_level()))}
    fn advance_to(&mut self,cycle:VirtualCycle) {
        // MCU reboot resets its cycle origin, not the independently powered RTC.
        self.rtc_elapsed=self.rtc_elapsed.saturating_add(cycle.saturating_sub(self.rtc_horizon));
        self.rtc_horizon=cycle;self.cycle=cycle;self.rtc.lock().unwrap().advance_to(self.rtc_elapsed);
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {
        let high=self.irq_level();if high!=self.irq_high {self.irq_high=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:37,level:high});}
        std::mem::take(&mut self.edges)
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let p=self.panel.borrow();let lit=self.light!=0&&self.expander.lock().unwrap().high(0)&&p.on&&!p.sleeping;
        let mut pixels=vec![0;WIDTH*HEIGHT];
        if lit {for (out,&value)in pixels.iter_mut().zip(&p.gram) {
            let value=if p.madctl&8!=0 {value}else{(value&0x07e0)|((value&0xf800)>>11)|((value&31)<<11)};
            *out=if self.light==65535 {value}else{(((((value>>11)&31)as u32*self.light/65535)<<11)|((((value>>5)&63)as u32*self.light/65535)<<5)|((value&31)as u32*self.light/65535))as u16};
        }}
        Some((WIDTH as u32,HEIGHT as u32,pixels,self.display_version()))
    }
    fn display_version(&self)->u64 {self.version.get()}
    fn display_frames(&self)->u64 {self.panel.borrow().frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.borrow().gram.clone(),WIDTH,HEIGHT))}
    fn report(&self)->String {format!("[{}] ST7796 RGB565={} bytes, dropped={}; TCA9554 reset0/1; FT6336 single-contact; PCF85063 calendar; ES8311 control; AXP2101 reference USB/no-cell scenario; SPI SD={}; audio/analog/PMIC-system-power/gestures/full-factory-boot=unverified",NAME,self.spi_bytes,self.dropped,self.card.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}
