//! ESP32-C5-Touch-LCD-2.8 @0196dcb7e601d6faa1c33785a71f6b0c35b532e6.
//! Physical N32R8 module:32MiB flash/8MiB PSRAM; factory layout uses16MiB.
//! The CH32V003 8-bit firmware, LCD and touch are separate physical devices.
use esp_soc::{BoardModel,board::{BoardEdge,SpiQspiPins},devices::{DcsPanel,
    imu_motion::ImuMotion,qmi8658::Qmi8658,pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311},pins::PinRoutes};
use esp_periph::{i2c::I2cDevice,sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};
use super::waveshare_touch28_parts::{Ch32Port,Ch32V003,Cst3530,Cst3530State};

struct TouchReset{port:Arc<Mutex<Ch32Port>>,touch:Arc<Mutex<Cst3530State>>,falling:Mutex<u64>}
impl TouchReset{
    fn sync(&self){
        let p=self.port.lock().expect("CH32V003 state");let(high,falling)=(p.high(0),p.falling_edges(0));drop(p);
        let mut last=self.falling.lock().expect("touch reset edge");let mut t=self.touch.lock().expect("CST3530 state");
        if *last!=falling{t.set_reset(false);}*last=falling;t.set_reset(high);
    }
}
type PersistentDevice=Rc<RefCell<Box<dyn I2cDevice>>>;
struct RoutedDevice{device:PersistentDevice,reset:Arc<TouchReset>}
impl I2cDevice for RoutedDevice{
    fn pins(&self)->Option<(u8,u8)>{Some((0,1))}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,hz:Option<u32>)->bool{self.reset.sync();self.device.borrow_mut().start_address_with_clock(c,a,r,hz)}
    fn write(&mut self,v:u8)->bool{let ack=self.device.borrow_mut().write(v);self.reset.sync();ack}
    fn read(&mut self)->u8{self.reset.sync();self.device.borrow_mut().read()}
    fn stop(&mut self){self.device.borrow_mut().stop();self.reset.sync();}
}
pub struct Touch28{
    panel:DcsPanel,dc:Option<bool>,backlight:u8,lcd_reset_high:bool,lcd_falling:u64,cmd:u8,generation:u64,rejected:u64,
    reset:Arc<TouchReset>,devices:Vec<(u8,PersistentDevice)>,motion:Arc<ImuMotion>,rtc:Arc<Mutex<Pcf85063State>>,cycle:u64,irq_high:bool,sd:Option<SpiSdCard>,
}
impl Default for Touch28{fn default()->Self{Self::new()}}
impl Touch28{
    pub fn new()->Self{
        let port=Arc::new(Mutex::new(Ch32Port::new()));let touch=Arc::new(Mutex::new(Cst3530State::new()));
        let reset=Arc::new(TouchReset{port:port.clone(),touch:touch.clone(),falling:Mutex::new(0)});reset.sync();
        let motion=Arc::new(ImuMotion::default());let rtc=Arc::new(Mutex::new(Pcf85063State::new(crate::bus::CPU_HZ)));
        let parts:Vec<(u8,Box<dyn I2cDevice>)>=vec![
            (0x24,Box::new(Ch32V003::new(port))),(0x58,Box::new(Cst3530::new(touch))),
            // Schematic page1 SA0 is grounded. The official resources link C data sheet.
            (0x6b,Box::new(Qmi8658::new_c(motion.clone(),crate::bus::CPU_HZ))),
            (0x51,Box::new(Pcf85063::new(rtc.clone()))),(0x18,Box::new(Es8311::new())),
        ];
        Self{panel:DcsPanel::st7789(),dc:None,backlight:0,lcd_reset_high:false,lcd_falling:0,cmd:0,generation:1,rejected:0,
            reset,devices:parts.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),motion,rtc,cycle:0,irq_high:true,sd:None}
    }
    fn sync_ports(&mut self){
        self.reset.sync();let p=self.reset.port.lock().expect("CH32V003 state");let(high,falling,brightness)=(p.high(1),p.falling_edges(1),p.pwm.unwrap_or(0));drop(p);
        if self.lcd_falling!=falling||self.lcd_reset_high&&!high{self.panel.reset();self.cmd=0;self.generation=self.generation.wrapping_add(1);}
        if self.lcd_reset_high!=high||self.backlight!=brightness{self.generation=self.generation.wrapping_add(1);}
        self.lcd_falling=falling;self.lcd_reset_high=high;self.backlight=brightness;
    }
}
impl BoardModel for Touch28{
    fn name(&self)->&'static str{"waveshare-c5-touch28"}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(28)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(28,true),(5,self.reset.touch.lock().expect("CST3530 state").irq_high())]}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{
        self.devices.iter().map(|(a,d)|(0,*a,Box::new(RoutedDevice{device:d.clone(),reset:self.reset.clone()})as Box<dyn I2cDevice>)).collect()
    }
    fn advance_to(&mut self,cycle:u64){self.cycle=cycle;self.motion.cycle.store(cycle,Ordering::Relaxed);self.rtc.lock().expect("PCF85063 state").advance_to(cycle);self.sync_ports();}
    fn touch(&mut self,x:u16,y:u16,down:bool){self.reset.sync();self.reset.touch.lock().expect("CST3530 state").update(x,y,down);}
    fn touch_at(&mut self,cycle:u64,x:u16,y:u16,down:bool){self.advance_to(cycle);self.touch(x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)>{Some((5,self.reset.touch.lock().expect("CST3530 state").irq_high()))}
    fn take_edges(&mut self)->Vec<BoardEdge>{
        self.sync_ports();let high=self.reset.touch.lock().expect("CST3530 state").irq_high();
        if high==self.irq_high{return vec![];}self.irq_high=high;vec![BoardEdge{cycle:self.cycle,pin:5,level:high}]
    }
    fn set_imu_motion(&mut self,mode:u32)->bool{self.motion.mode.store(mode,Ordering::Relaxed);true}
    fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>){
        self.advance_to(cycle);self.dc=r.software_output(9);
        let sd_gpio=r.software_output(23);let sd_hardware=[61,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==23));
        if sd_gpio==Some(true)||(sd_gpio.is_none()&&!sd_hardware){if let Some(sd)=&mut self.sd{sd.deselect();}}
    }
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        self.sync_ports();let floating=vec![0xff;rx_len];
        if host!=2||p.unsupported_mode||p.sio||p.clock_idle_high||p.clock_output_edge
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||p.sclk&(1<<6)==0||p.data[0]&(1<<7)==0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)
            ||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||p.dummy_cycles!=0{self.rejected+=1;return floating;}
        let lcd=p.cs&(1<<10)!=0;let card=p.cs&(1<<23)!=0;
        if lcd&&card{self.rejected+=1;if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
        if let Some(sd)=&mut self.sd{
            let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(card,tx,rx_len)}else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx_len)};
            if card{if p.hardware_cs&(1<<23)!=0&&!p.keep_cs&&p.software_cs&(1<<23)==0{sd.deselect();}return if p.input[1]==Some(8){rx}else{floating};}
        }
        if lcd&&self.lcd_reset_high&&self.dc.is_some(){
            if rx_len!=0||p.full_duplex{self.rejected+=1;return floating;}self.panel.dc=self.dc==Some(true);
            if self.panel.dc&&self.cmd==0x2c&&self.panel.colmod!=0x55{self.rejected+=1;return floating;}
            for &b in tx{if !self.panel.dc{self.cmd=b;}self.panel.byte(b);}if !tx.is_empty(){self.generation=self.generation.wrapping_add(1);}
        }floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{let sd=SpiSdCard::new(image,config)?;let result=sd.describe();self.sd=Some(sd);Ok(result)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{
        let p=&self.panel;let visible=self.lcd_reset_high&&self.backlight!=0&&p.on&&!p.sleeping;
        let pixels=p.gram.iter().map(|&word|{if !visible{return 0;}let mut v=word;
            if p.madctl&8!=0{v=(v&0x7e0)|((v&0xf800)>>11)|((v&31)<<11);}
            if !p.inverted{v= !v;}
            let d=u32::from(self.backlight);((((u32::from(v>>11)*d/255)<<11)|((u32::from((v>>5)&63)*d/255)<<5))|(u32::from(v&31)*d/255))as u16
        }).collect();Some((240,320,pixels,self.generation))
    }
    fn display_version(&self)->u64{self.generation}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String{format!("[waveshare-c5-touch28] ST7789 240x320 pixels={} RAMWR={} duty={}/255 rejected={}; CH32V00324 EX1=LCD EX0=CST353058 irq5 reports={}; QMI8658C6b PCF85063A51 ES831118; BOOT28; SD {}; virtual USB supply; factory/RF/audio/ADC/SHTC3/power-key unverified",self.panel.pixels_written,self.panel.frames,self.backlight,self.rejected,self.reset.touch.lock().expect("CST3530 state").reports,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}
