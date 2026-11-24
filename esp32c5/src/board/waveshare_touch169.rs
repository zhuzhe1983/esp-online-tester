//! ESP32-C5-Touch-LCD-1.69 @ 136f2a8c68b3808ce4a9dbea218e9c3a4928212f.
//! 16MiB flash/8MiB package PSRAM, ST7789V2 240x280, CST816S and TCA9554.
//! Schematic page1: LCD_RST is pulled high; EX1 resets touch ONLY. The BSP's
//! "Shared with LCD reset" comment is inconsistent with the actual netlist.
use esp_soc::{BoardModel,board::{BoardEdge,SpiQspiPins},devices::{DcsPanel,
    imu_motion::ImuMotion,tca9554::{Tca9554,Tca9554State},qmi8658::Qmi8658,
    pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311},pins::PinRoutes};
use esp_periph::i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}};
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};

struct TouchReset{port:Arc<Mutex<Tca9554State>>,touch:Arc<Mutex<Cst816State>>,falling:Mutex<u64>}
impl TouchReset{
    fn sync(&self){
        let p=self.port.lock().expect("TCA9554 state");let(high,falling)=(p.high(1),p.falling_edges(1));drop(p);
        let mut last=self.falling.lock().expect("touch reset edge");let mut t=self.touch.lock().expect("CST816S state");
        if !high||*last!=falling{t.reset();}*last=falling;t.powered=high;
    }
}
// Keep the peripheral itself across controller reattachment. An ESP restart is
// not a power-cycle of its externally supplied RTC, codec, touch or IMU.
type PersistentDevice=Rc<RefCell<Box<dyn I2cDevice>>>;
struct RoutedDevice{device:PersistentDevice,reset:Arc<TouchReset>}
impl I2cDevice for RoutedDevice{
    fn pins(&self)->Option<(u8,u8)>{Some((8,9))}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,hz:Option<u32>)->bool{self.reset.sync();self.device.borrow_mut().start_address_with_clock(c,a,r,hz)}
    fn write(&mut self,v:u8)->bool{let ack=self.device.borrow_mut().write(v);self.reset.sync();ack}
    fn read(&mut self)->u8{self.reset.sync();self.device.borrow_mut().read()}
    fn stop(&mut self){self.device.borrow_mut().stop();self.reset.sync();}
}
// Only the documented one-contact report, unknown identity readback, and sleep
// command of the existing CST816S subset are exposed. No generic config ACK.
struct TouchReport{inner:Cst816s,pointer:Option<u8>,first:bool}
impl I2cDevice for TouchReport{
    fn start(&mut self,read:bool)->bool{if !read{self.first=true;}(!read||self.pointer.is_some())&&self.inner.start(read)}
    fn write(&mut self,v:u8)->bool{
        if self.first{self.first=false;self.pointer=matches!(v,2..=6|0xa5|0xa7..=0xa9).then_some(v);return self.pointer.is_some()&&self.inner.write(v);}
        let Some(p)=self.pointer else{return false;};if p!=0xa5||v!=3{return false;}
        self.pointer=Some(p.wrapping_add(1));self.inner.write(v)
    }
    fn read(&mut self)->u8{let Some(p)=self.pointer else{return 0xff;};if !matches!(p,2..=6|0xa7..=0xa9){return 0xff;}self.pointer=Some(p.wrapping_add(1));self.inner.read()}
}

pub struct Touch169{
    panel:DcsPanel,dc:Option<bool>,gpio_backlight:Option<bool>,backlight:u32,cmd:u8,generation:u64,rejected:u64,
    reset:Arc<TouchReset>,devices:Vec<(u8,PersistentDevice)>,motion:Arc<ImuMotion>,rtc:Arc<Mutex<Pcf85063State>>,cycle:u64,irq_high:bool,
}
impl Default for Touch169{fn default()->Self{Self::new()}}
impl Touch169{
    pub fn new()->Self{
        let port=Arc::new(Mutex::new(Tca9554State::default()));let touch=Arc::new(Mutex::new(Cst816State::default()));
        let reset=Arc::new(TouchReset{port:port.clone(),touch:touch.clone(),falling:Mutex::new(0)});reset.sync();
        let motion=Arc::new(ImuMotion::default());let rtc=Arc::new(Mutex::new(Pcf85063State::new(crate::bus::CPU_HZ)));
        let parts:Vec<(u8,Box<dyn I2cDevice>)>=vec![
            (0x20,Box::new(Tca9554::new(port))),
            (0x15,Box::new(TouchReport{inner:Cst816s::new(touch),pointer:None,first:true})),
            // SDO/SA0 is grounded on the schematic, hence 6Bh, not a 6Ah alias.
            // The product resource page links the QMI8658C data sheet.
            (0x6b,Box::new(Qmi8658::new_c(motion.clone(),crate::bus::CPU_HZ))),
            (0x51,Box::new(Pcf85063::new(rtc.clone()))),(0x18,Box::new(Es8311::new())),
        ];
        Self{panel:DcsPanel::st7789(),dc:None,gpio_backlight:None,backlight:0,cmd:0,generation:1,rejected:0,
            reset,devices:parts.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),motion,rtc,cycle:0,irq_high:true}
    }
    fn brightness(&mut self,duty:u32){if self.backlight!=duty{self.backlight=duty;self.generation=self.generation.wrapping_add(1);}}
}
impl BoardModel for Touch169{
    fn name(&self)->&'static str{"waveshare-c5-touch169"}
    fn named_pin(&self,name:&str)->Option<u8>{matches!(name,"boot"|"button").then_some(28)}
    fn input_levels(&self)->Vec<(u8,bool)>{vec![(28,true),(5,self.reset.touch.lock().expect("CST816S state").irq_high())]}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{
        self.devices.iter().map(|(a,d)|(0,*a,Box::new(RoutedDevice{device:d.clone(),reset:self.reset.clone()})as Box<dyn I2cDevice>)).collect()
    }
    fn advance_to(&mut self,cycle:u64){self.cycle=cycle;self.motion.cycle.store(cycle,Ordering::Relaxed);self.rtc.lock().expect("PCF85063 state").advance_to(cycle);self.reset.sync();}
    fn touch(&mut self,x:u16,y:u16,down:bool){self.reset.sync();self.reset.touch.lock().expect("CST816S state").update(x.min(239),y.min(279),down);}
    fn touch_at(&mut self,cycle:u64,x:u16,y:u16,down:bool){self.advance_to(cycle);self.touch(x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)>{Some((5,self.reset.touch.lock().expect("CST816S state").irq_high()))}
    fn take_edges(&mut self)->Vec<BoardEdge>{
        self.reset.sync();let high=self.reset.touch.lock().expect("CST816S state").irq_high();
        if high==self.irq_high{return vec![];}self.irq_high=high;vec![BoardEdge{cycle:self.cycle,pin:5,level:high}]
    }
    fn set_imu_motion(&mut self,mode:u32)->bool{self.motion.mode.store(mode,Ordering::Relaxed);true}
    fn imu_motion(&self)->u32{self.motion.mode.load(Ordering::Relaxed)}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>){self.advance_to(cycle);self.dc=r.software_output(24);self.gpio_backlight=r.software_output(26);if let Some(on)=self.gpio_backlight{self.brightness(if on{65535}else{0});}}
    fn advance_inputs_with_clock(&mut self,_cycle:u64,clock:&dyn Fn(u8)->Option<(f64,u32)>){let duty=self.gpio_backlight.map_or_else(||clock(26).map_or(0,|(_,d)|d),|on|if on{65535}else{0});self.brightness(duty);}
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        let floating=vec![0xff;rx_len];
        if host!=2||p.unsupported_mode||p.sio||p.full_duplex||p.clock_idle_high||p.clock_output_edge||rx_len!=0
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||p.sclk&(1<<23)==0||p.data[0]&(1<<7)==0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)
            ||!p.data_bits.is_multiple_of(8)||p.dummy_cycles!=0{self.rejected+=1;return floating;}
        if p.cs&(1<<27)!=0&&self.dc.is_some(){
            self.panel.dc=self.dc==Some(true);
            if self.panel.dc&&self.cmd==0x2c&&self.panel.colmod!=0x55{self.rejected+=1;return floating;}
            for &b in tx{if !self.panel.dc{self.cmd=b;}self.panel.byte(b);}if !tx.is_empty(){self.generation=self.generation.wrapping_add(1);}
        }floating
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{
        let p=&self.panel;let visible=self.backlight!=0&&p.on&&!p.sleeping;let mut out=Vec::with_capacity(240*280);
        for y in 20..300{for x in 0..240{let mut v=if visible{p.gram[y*240+x]}else{0};
            if visible{if p.madctl&8!=0{v=(v&0x7e0)|((v&0xf800)>>11)|((v&31)<<11);}
                if !p.inverted{v= !v;}
                let d=self.backlight;v=((((u32::from(v>>11)*d/65535)<<11)|((u32::from((v>>5)&63)*d/65535)<<5))|(u32::from(v&31)*d/65535))as u16;}
            out.push(v);
        }}Some((240,280,out,self.generation))
    }
    fn display_version(&self)->u64{self.generation}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String{format!("[waveshare-c5-touch169] ST7789 240x280 gap0,20 pixels={} RAMWR={} duty={} rejected={}; CST816S15 reset=TCA20.EX1 irq5; QMI8658C6b PCF85063A51 ES831118; GPIO28 BOOT; no SD; virtual USB supply; factory/RF/audio/battery/power-key unverified",self.panel.pixels_written,self.panel.frames,self.backlight,self.rejected)}
}
