//! Exact C6 LCD boards, wired from the individually retained official schematics.
//! See docs/platform/waveshare-c6-lcd-family.md for pinned BSPs, conflicts and scope.
//! SPI is opt-in and phase/route checked; legacy waveshare-c6-lcd147 is unchanged.
use esp_soc::{BoardModel,board::{BoardEdge,SpiQspiPins},pins::PinRoutes,devices::{
    DcsPanel,Ws2812Chain,axs5106::{Axs5106,Axs5106State},imu_motion::ImuMotion,
    qmi8658::Qmi8658,pcf85063::{Pcf85063,Pcf85063State},es8311::Es8311,es7210::Es7210,
    tca9554::{Tca9554,Tca9554State},axp2101::{Axp2101,Axp2101Power,PowerRail}}};
use esp_periph::{i2c::{I2cDevice,cst816s::{Cst816s,Cst816State}},sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use std::{cell::RefCell,rc::Rc,sync::{Arc,Mutex,atomic::Ordering}};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum LcdVariant {Lcd085,Lcd19,Touch19,Touch169,Lcd169,Touch147,Lcd128,Touch128,Lcd154,Touch154,GeekV1,GeekV2,Lcd13,Touch183}
#[derive(Clone,Copy)]
struct Wiring {name:&'static str,flash:usize,width:usize,height:usize,gap:(usize,usize),sclk:u8,mosi:u8,cs:u8,dc:u8,rst:u8,reset_pullup:bool,bl:u8,bl_low:bool,i2c:(u8,u8),sd:Option<(u8,u8)>,touch_reset:Option<u8>,touch_irq:Option<u8>}
impl LcdVariant {
    fn wiring(self)->Wiring {match self {
        Self::Lcd085=>Wiring{name:"waveshare-c6-lcd085",flash:8,width:128,height:128,gap:(2,3),sclk:1,mosi:2,cs:5,dc:3,rst:4,reset_pullup:false,bl:6,bl_low:false,i2c:(8,7),sd:Some((16,17)),touch_reset:None,touch_irq:None},
        Self::Lcd19|Self::Touch19=>Wiring{name:if self==Self::Lcd19{"waveshare-c6-lcd19"}else{"waveshare-c6-touch-lcd19"},flash:8,width:170,height:320,gap:(35,0),sclk:5,mosi:4,cs:7,dc:6,rst:14,reset_pullup:false,bl:15,bl_low:true,i2c:(18,8),sd:Some((19,20)),touch_reset:None,touch_irq:None},
        Self::Touch169=>Wiring{name:"waveshare-c6-touch-lcd169",flash:16,width:240,height:280,gap:(0,20),sclk:1,mosi:2,cs:5,dc:3,rst:4,reset_pullup:true,bl:6,bl_low:false,i2c:(8,7),sd:None,touch_reset:Some(4),touch_irq:Some(11)},
        Self::Lcd169=>Wiring{name:"waveshare-c6-lcd169",flash:16,width:240,height:280,gap:(0,20),sclk:1,mosi:2,cs:5,dc:3,rst:4,reset_pullup:true,bl:6,bl_low:false,i2c:(8,7),sd:None,touch_reset:None,touch_irq:None},
        Self::Touch147=>Wiring{name:"waveshare-c6-touch-lcd147",flash:8,width:172,height:320,gap:(34,0),sclk:1,mosi:2,cs:14,dc:15,rst:22,reset_pullup:false,bl:23,bl_low:false,i2c:(18,19),sd:Some((3,4)),touch_reset:Some(20),touch_irq:Some(21)},
        Self::Lcd128|Self::Touch128=>Wiring{name:if self==Self::Lcd128{"waveshare-c6-lcd128"}else{"waveshare-c6-touch-lcd128"},flash:16,width:240,height:240,gap:(0,0),sclk:11,mosi:21,cs:10,dc:19,rst:20,reset_pullup:false,bl:7,bl_low:false,i2c:(18,8),sd:Some((22,15)),touch_reset:if self==Self::Touch128{Some(23)}else{None},touch_irq:if self==Self::Touch128{Some(4)}else{None}},
        Self::Lcd154|Self::Touch154=>Wiring{name:if self==Self::Lcd154{"waveshare-c6-lcd154"}else{"waveshare-c6-touch-lcd154"},flash:16,width:240,height:240,gap:(0,0),sclk:1,mosi:2,cs:11,dc:3,rst:4,reset_pullup:false,bl:6,bl_low:false,i2c:(8,7),sd:Some((16,17)),touch_reset:if self==Self::Touch154{Some(4)}else{None},touch_irq:if self==Self::Touch154{Some(5)}else{None}},
        Self::GeekV1|Self::GeekV2=>Wiring{name:if self==Self::GeekV1{"waveshare-c6-geek-v1"}else{"waveshare-c6-geek"},flash:16,width:135,height:240,gap:(52,40),sclk:1,mosi:2,cs:5,dc:3,rst:4,reset_pullup:false,bl:6,bl_low:false,i2c:(7,8),sd:Some((20,23)),touch_reset:None,touch_irq:None},
        Self::Lcd13=>Wiring{name:"waveshare-c6-lcd13",flash:4,width:240,height:240,gap:(0,0),sclk:7,mosi:6,cs:14,dc:15,rst:21,reset_pullup:false,bl:22,bl_low:false,i2c:(0,0),sd:Some((5,4)),touch_reset:None,touch_irq:None},
        Self::Touch183=>Wiring{name:"waveshare-c6-touch-lcd183",flash:16,width:240,height:284,gap:(0,0),sclk:1,mosi:2,cs:5,dc:3,rst:4,reset_pullup:false,bl:6,bl_low:false,i2c:(7,8),sd:Some((16,17)),touch_reset:Some(4),touch_irq:Some(11)},
    }}
    pub fn from_name(name:&str)->Option<Self> {match name {"waveshare-c6-geek"=>Some(Self::GeekV2),"waveshare-c6-geek-v1"=>Some(Self::GeekV1),"waveshare-c6-lcd13"=>Some(Self::Lcd13),"waveshare-c6-touch-lcd183"=>Some(Self::Touch183),"waveshare-c6-lcd128"=>Some(Self::Lcd128),"waveshare-c6-touch-lcd128"=>Some(Self::Touch128),"waveshare-c6-lcd154"=>Some(Self::Lcd154),"waveshare-c6-touch-lcd154"=>Some(Self::Touch154),"waveshare-c6-lcd085"=>Some(Self::Lcd085),"waveshare-c6-lcd19"=>Some(Self::Lcd19),"waveshare-c6-touch-lcd19"=>Some(Self::Touch19),"waveshare-c6-touch-lcd169"=>Some(Self::Touch169),"waveshare-c6-lcd169"=>Some(Self::Lcd169),"waveshare-c6-touch-lcd147"=>Some(Self::Touch147),_=>None}}
    pub fn flash_mb(self)->usize {self.wiring().flash}
}
type PersistentDevice=Rc<RefCell<Box<dyn I2cDevice>>>;
struct RoutedDevice {device:PersistentDevice,pins:(u8,u8)}
impl I2cDevice for RoutedDevice {
    fn pins(&self)->Option<(u8,u8)>{Some(self.pins)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,hz:Option<u32>)->bool{self.device.borrow_mut().start_address_with_clock(c,a,r,hz)}
    fn write(&mut self,v:u8)->bool{self.device.borrow_mut().write(v)}
    fn read(&mut self)->u8{self.device.borrow_mut().read()}
    fn stop(&mut self){self.device.borrow_mut().stop();}
}
// The two BSPs consume the common CST816 report, not a fabricated variant ID.
// LCD1.9 writes 00h=00h; no speculative configuration register writes are ACKed.
pub(super) struct CstReport {inner:Cst816s,first:bool,selector:Option<u8>,normal_written:bool}
impl CstReport {pub(super) fn new(state:Arc<Mutex<Cst816State>>)->Self{Self{inner:Cst816s::new(state),first:true,selector:None,normal_written:false}}}
impl I2cDevice for CstReport {
    fn start(&mut self,read:bool)->bool{if !read{self.first=true;self.normal_written=false;}(!read||self.selector.is_some())&&self.inner.start(read)}
    fn write(&mut self,v:u8)->bool{
        if self.first {self.first=false;self.selector=matches!(v,0..=6|0xa7..=0xa9).then_some(v);return self.selector.is_some()&&self.inner.write(v);}
        if self.selector==Some(0)&&v==0&&!self.normal_written {self.normal_written=true;return self.inner.write(v);}false
    }
    fn read(&mut self)->u8{if self.selector.is_some(){self.inner.read()}else{0xff}}
    fn stop(&mut self){self.inner.stop();}
}

pub struct WaveshareLcd {
    variant:LcdVariant,w:Wiring,panel:DcsPanel,dc:Option<bool>,reset_high:bool,backlight:u8,command:u8,generation:u64,rejected:u64,
    power:Option<Arc<Mutex<Axp2101Power>>>,power_ok:bool,devices:Vec<(u8,PersistentDevice)>,cst:Option<Arc<Mutex<Cst816State>>>,axs:Option<Arc<Mutex<Axs5106State>>>,
    motion:Option<Arc<ImuMotion>>,rtc:Option<Arc<Mutex<Pcf85063State>>>,led:Option<Ws2812Chain>,sd:Option<SpiSdCard>,cycle:u64,irq_high:bool,
}
impl WaveshareLcd {
    pub fn new(variant:LcdVariant)->Self {
        let w=variant.wiring();let mut parts:Vec<(u8,Box<dyn I2cDevice>)>=vec![];
        let cst=matches!(variant,LcdVariant::Touch19|LcdVariant::Touch169|LcdVariant::Touch128|LcdVariant::Touch154|LcdVariant::Touch183).then(||{let mut state=Cst816State::default();state.powered=true;Arc::new(Mutex::new(state))});
        if let Some(s)=&cst{parts.push((0x15,Box::new(CstReport::new(s.clone()))));}
        let axs=(variant==LcdVariant::Touch147).then(||Arc::new(Mutex::new(Axs5106State::default())));
        if let Some(s)=&axs{parts.push((0x63,Box::new(Axs5106::new(s.clone()))));}
        let motion=(!matches!(variant,LcdVariant::Lcd085|LcdVariant::GeekV1|LcdVariant::GeekV2|LcdVariant::Lcd13)).then(||Arc::new(ImuMotion::default()));
        if let Some(m)=&motion{parts.push((0x6b,Box::new(if matches!(variant,LcdVariant::Lcd19|LcdVariant::Touch19|LcdVariant::Lcd154|LcdVariant::Touch154|LcdVariant::Touch183){Qmi8658::new_c(m.clone(),crate::periph::CPU_HZ)}else{Qmi8658::new(m.clone(),crate::periph::CPU_HZ)})));}
        if matches!(variant,LcdVariant::Lcd19|LcdVariant::Touch19){parts.push((0x20,Box::new(Tca9554::new(Arc::new(Mutex::new(Tca9554State::default()))))));}
        let rtc=matches!(variant,LcdVariant::Touch183|LcdVariant::Touch169|LcdVariant::Lcd169|LcdVariant::Lcd128|LcdVariant::Touch128).then(||Arc::new(Mutex::new(Pcf85063State::new(crate::periph::CPU_HZ))));
        if let Some(s)=&rtc{parts.push((0x51,Box::new(Pcf85063::new(s.clone()))));}
        if matches!(variant,LcdVariant::Touch183|LcdVariant::Lcd085|LcdVariant::Touch169|LcdVariant::Lcd169|LcdVariant::Lcd154|LcdVariant::Touch154){parts.push((0x18,Box::new(Es8311::new())));}
        // ES7210 address is the board's AD1/AD0 straps, not an audio sample source.
        if matches!(variant,LcdVariant::Touch183|LcdVariant::Lcd085|LcdVariant::Lcd154|LcdVariant::Touch154){parts.push((0x40,Box::new(Es7210::new())));}
        let power=(variant==LcdVariant::Touch183).then(||Arc::new(Mutex::new(Axp2101Power::default())));
        if let Some(s)=&power{parts.push((0x34,Box::new(Axp2101::new(s.clone()))));}
        let panel=match variant{LcdVariant::Lcd128|LcdVariant::Touch128=>DcsPanel::new(240,240),LcdVariant::Lcd085=>DcsPanel::st7735(),LcdVariant::Touch147=>DcsPanel::new(240,320),_=>DcsPanel::st7789()};
        Self{variant,w,panel,power,power_ok:true,dc:None,reset_high:w.reset_pullup,backlight:0,command:0,generation:1,rejected:0,
            devices:parts.into_iter().map(|(a,d)|(a,Rc::new(RefCell::new(d)))).collect(),cst,axs,motion,rtc,
            led:match variant{LcdVariant::Lcd085=>Some(Ws2812Chain::new(8)),LcdVariant::Lcd13=>Some(Ws2812Chain::new(1)),_=>None},sd:None,cycle:0,irq_high:true}
    }
    fn touch_level(&self)->bool{if let Some(s)=&self.cst{s.lock().expect("CST816 state").irq_high()}else if let Some(s)=&self.axs{s.lock().expect("AXS5106 state").irq_high()}else{true}}
    fn set_backlight(&mut self,duty:u8){if self.backlight!=duty{self.backlight=duty;self.generation=self.generation.wrapping_add(1);}}
}
impl BoardModel for WaveshareLcd {
    fn name(&self)->&'static str{self.w.name}
    fn named_pin(&self,name:&str)->Option<u8>{match name{"boot"|"button"=>Some(9),"user" if matches!(self.variant,LcdVariant::Lcd154|LcdVariant::Touch154|LcdVariant::Touch169|LcdVariant::Lcd169)=>Some(18),"power" if matches!(self.variant,LcdVariant::Touch169|LcdVariant::Lcd169)=>Some(18),_=>None}}
    fn input_levels(&self)->Vec<(u8,bool)>{let mut pins=vec![(9,true)];if matches!(self.variant,LcdVariant::Lcd154|LcdVariant::Touch154|LcdVariant::Touch169|LcdVariant::Lcd169){pins.push((18,true));}
        if let Some(pin)=self.w.touch_irq{pins.push((pin,self.touch_level()));}pins}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)>{self.devices.iter().map(|(a,d)|(0,*a,Box::new(RoutedDevice{device:d.clone(),pins:self.w.i2c})as Box<dyn I2cDevice>)).collect()}
    fn advance_to(&mut self,cycle:u64){self.cycle=cycle;if let Some(s)=&self.motion{s.cycle.store(cycle,Ordering::Relaxed);}
        if let Some(s)=&self.rtc{s.lock().expect("PCF85063 state").advance_to(cycle);}
        let on=self.power.as_ref().is_none_or(|s|s.lock().expect("AXP2101 state").output_mv(PowerRail::Dcdc1)==Some(3300));
        if on!=self.power_ok{if !on{self.panel.reset();}self.power_ok=on;self.generation+=1;}
    }
    fn touch(&mut self,x:u16,y:u16,down:bool){if let Some(s)=&self.cst{s.lock().expect("CST816 state").update(x.min(self.w.width as u16-1),y.min(self.w.height as u16-1),down);}
        if let Some(s)=&self.axs{s.lock().expect("AXS5106 state").update(x,y,down);}}
    fn touch_at(&mut self,cycle:u64,x:u16,y:u16,down:bool){self.advance_to(cycle);self.touch(x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)>{self.w.touch_irq.map(|pin|(pin,self.touch_level()))}
    fn take_edges(&mut self)->Vec<BoardEdge>{let high=self.touch_level();if high==self.irq_high{return vec![];}self.irq_high=high;self.w.touch_irq.map_or_else(Vec::new,|pin|vec![BoardEdge{cycle:self.cycle,pin,level:high}])}
    fn set_imu_motion(&mut self,mode:u32)->bool{if let Some(s)=&self.motion{s.mode.store(mode,Ordering::Relaxed);true}else{false}}
    fn imu_motion(&self)->u32{self.motion.as_ref().map_or(0,|s|s.mode.load(Ordering::Relaxed))}
    fn uses_gpio_routes(&self)->bool{true}
    fn gpio_waveform_at(&mut self,cycle:u64,r:PinRoutes<'_>){
        self.advance_to(cycle);self.dc=r.software_output(self.w.dc);
        let reset=r.software_output(self.w.rst).unwrap_or(self.w.reset_pullup);
        if self.reset_high!=reset{if !reset{self.panel.reset();self.command=0;}self.reset_high=reset;self.generation=self.generation.wrapping_add(1);}
        if let Some(pin)=self.w.touch_reset{
            let high=r.software_output(pin).unwrap_or(self.w.reset_pullup&&pin==self.w.rst);
            if let Some(s)=&self.cst{let mut s=s.lock().expect("CST816 state");if s.powered&&!high{s.reset();}s.powered=high;}
            if let Some(s)=&self.axs{s.lock().expect("AXS5106 state").set_reset(high);}
        }
        if let Some(high)=r.software_output(self.w.bl){self.set_backlight(if high!=self.w.bl_low{255}else{0});}
        if let Some((_,cs))=self.w.sd{let gpio=r.software_output(cs);let hardware=[68,101,102,103,104,105].into_iter().any(|s|r.output_pins(s).any(|p|p==cs));
            if gpio==Some(true)||(gpio.is_none()&&!hardware){if let Some(sd)=&mut self.sd{sd.deselect();}}}
    }
    fn uses_gpio_pwm(&self)->bool{true}
    fn gpio_pwm_at(&mut self,cycle:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>){self.advance_to(cycle);if let Some((high,_hz))=pwm(self.w.bl){let duty=if self.w.bl_low{1.0-high}else{high};self.set_backlight((duty.clamp(0.0,1.0)*255.0).round()as u8);}}
    fn uses_qspi_pins(&self)->bool{true}
    fn spi_transfer_qspi_pins(&mut self,host:u8,p:SpiQspiPins,tx:&[u8],rx_len:usize)->Vec<u8>{
        let floating=vec![0xff;rx_len];

        let card=self.w.sd.is_some_and(|(_,cs)|p.cs&(1<<cs)!=0);
        let (clk,mosi)=if self.variant==LcdVariant::GeekV1&&(card||p.cs==0){(19,18)}else{(self.w.sclk,self.w.mosi)};
        if host!=2||p.unsupported_mode||p.sio||p.clock_output_edge
            ||[p.command_width,p.address_width,p.data_width,p.read_width].iter().any(|&w|w>1)
            ||p.sclk&(1<<clk)==0||p.data[0]&(1<<mosi)==0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)
            ||!p.data_bits.is_multiple_of(8)||!p.read_bits.is_multiple_of(8)||p.dummy_cycles!=0{self.rejected+=1;return floating;}
        let lcd=p.cs&(1<<self.w.cs)!=0;let card=self.w.sd.is_some_and(|(_,cs)|p.cs&(1<<cs)!=0);
        if lcd&&card{self.rejected+=1;if let Some(sd)=&mut self.sd{sd.deselect();}return floating;}
        if let Some(sd)=&mut self.sd{
            let rx=if p.full_duplex||tx.is_empty()||rx_len==0{sd.transfer(card,tx,rx_len)}else{sd.transfer(card,tx,0);sd.transfer(card,&[],rx_len)};
            if card{let(miso,cs)=self.w.sd.expect("attached SD has physical pins");if p.hardware_cs&(1<<cs)!=0&&!p.keep_cs&&p.software_cs&(1<<cs)==0{sd.deselect();}return if p.input[1]==Some(miso){rx}else{floating};}
        }
        if lcd&&self.reset_high&&self.power_ok&&self.dc.is_some(){
            // A TX/RX SPI transaction still clocks valid MOSI into this write-only
            // glass. Its unconnected MISO returns the floating bytes above.
            if rx_len!=0&&!p.full_duplex{self.rejected+=1;return floating;}self.panel.dc=self.dc==Some(true);
            let color16=self.panel.colmod==0x55||(matches!(self.variant,LcdVariant::Lcd085|LcdVariant::Touch147|LcdVariant::Lcd128|LcdVariant::Touch128|LcdVariant::GeekV1|LcdVariant::GeekV2|LcdVariant::Lcd13|LcdVariant::Touch183)&&self.panel.colmod==5);
            if self.panel.dc&&self.command==0x2c&&!color16{self.rejected+=1;return floating;}
            for &b in tx{if !self.panel.dc{self.command=b;}self.panel.byte(b);}if !tx.is_empty(){self.generation=self.generation.wrapping_add(1);}
        }floating
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String>{if self.w.sd.is_none(){return Err("this exact board has no TF socket".into());}let sd=SpiSdCard::new(image,config)?;let description=sd.describe();self.sd=Some(sd);Ok(description)}
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]){if pin==if self.variant==LcdVariant::Lcd13{8}else{14}{if let Some(led)=&mut self.led{led.from_bits(bits);}}}
    fn leds(&self)->Option<(&[[u8;3]],u64)>{self.led.as_ref().map(|led|(led.leds.as_slice(),led.updates))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{
        let p=&self.panel;let visible=self.power_ok&&self.reset_high&&self.backlight!=0&&p.on&&!p.sleeping;let mut pixels=Vec::with_capacity(self.w.width*self.w.height);
        for y in 0..self.w.height{for x in 0..self.w.width{let mut value=if visible{p.gram[(y+self.w.gap.1)*p.cols+if self.variant==LcdVariant::Lcd13{239-x}else{x+self.w.gap.0}]}else{0};
            if p.madctl&8!=0{value=(value&0x7e0)|((value&0xf800)>>11)|((value&31)<<11);}
            // Linear virtual brightness only; LCD polarity/gamma are not calibrated.
            let d=u32::from(self.backlight);value=(((u32::from(value>>11)*d/255)<<11)|((u32::from((value>>5)&63)*d/255)<<5)|(u32::from(value&31)*d/255))as u16;pixels.push(value);}}
        Some((self.w.width as u32,self.w.height as u32,pixels,self.generation))
    }
    fn display_version(&self)->u64{self.generation}
    fn display_frames(&self)->u64{self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)>{Some((self.panel.gram.clone(),self.panel.cols,self.panel.rows))}
    fn report(&self)->String{format!("[{}] {}x{} pixels={} RAMWR={} duty={}/255 rejected={}; BOOT9; SD {}; digital control subset; RF/audio/power/analog color unverified",self.w.name,self.w.width,self.w.height,self.panel.pixels_written,self.panel.frames,self.backlight,self.rejected,self.sd.as_ref().map_or_else(||"absent".into(),SpiSdCard::report))}
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_periph::{Gpio,RegRam};
    use esp_soc::pins::ChipPins;
    fn gpio(g:&mut Gpio,m:&mut RegRam,pin:u8,high:bool){g.func_out_sel[usize::from(pin)]=128;g.enable|=1<<pin;if high{g.out|=1<<pin;}else{g.out&=!(1<<pin);}m.write(4+u32::from(pin)*4,1<<12|1<<9);}
    fn activate(b:&mut WaveshareLcd)->(Gpio,RegRam){let mut g=Gpio::new();let mut m=RegRam::new();gpio(&mut g,&mut m,b.w.rst,true);gpio(&mut g,&mut m,b.w.bl,!b.w.bl_low);gpio(&mut g,&mut m,b.w.dc,false);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));(g,m)}
    fn command(b:&mut WaveshareLcd,g:&mut Gpio,m:&mut RegRam,c:u8,args:&[u8]){
        let mut p=SpiQspiPins{sclk:1<<b.w.sclk,data:[1<<b.w.mosi,0,0,0],cs:1<<b.w.cs,data_width:1,data_bits:8,..Default::default()};
        gpio(g,m,b.w.dc,false);b.gpio_waveform_at(0,ChipPins::C6.routes(g,m));b.spi_transfer_qspi_pins(2,p,&[c],0);
        if !args.is_empty(){gpio(g,m,b.w.dc,true);b.gpio_waveform_at(0,ChipPins::C6.routes(g,m));p.data_bits=args.len()as u32*8;b.spi_transfer_qspi_pins(2,p,args,0);}
    }
    #[test]
    fn waveshare_jd9853_uses_sourced_column34_window_and_retains_hidden_gram(){
        // Pinned factory/main.c sets gap(34,0); JD9853 draw_bitmap adds it
        // before CASET. The active172 columns are34..205 of240-column RAM.
        let mut b=WaveshareLcd::new(LcdVariant::Touch147);
        b.reset_high=true;b.power_ok=true;b.backlight=255;
        b.panel.on=true;b.panel.sleeping=false;b.panel.colmod=0x55;
        fn write(panel:&mut DcsPanel,x:u16,value:u16){
            panel.dc=false;panel.byte(0x2a);panel.dc=true;
            for v in [(x>>8)as u8,x as u8,(x>>8)as u8,x as u8]{panel.byte(v);}
            panel.dc=false;panel.byte(0x2b);panel.dc=true;
            for v in [0,0,0,0]{panel.byte(v);}
            panel.dc=false;panel.byte(0x2c);panel.dc=true;
            for v in value.to_be_bytes(){panel.byte(v);}
        }
        write(&mut b.panel,0,0xffff);write(&mut b.panel,34,0xf800);write(&mut b.panel,205,0x001f);
        let pixels=b.display().unwrap().2;assert_eq!(pixels[0],0xf800);assert_eq!(pixels[171],0x001f);
        assert_eq!(b.panel.gram[0],0xffff);assert!(!pixels.contains(&0xffff));
        assert_eq!(b.panel.cols,240);assert_eq!(b.panel.rows,320);
    }
    #[test]
    fn exact_five_boards_have_distinct_devices_and_real_boot_net(){
        for (variant,addresses,size,sd) in [
            (LcdVariant::Lcd085,vec![0x18,0x40],(128,128),true),
            (LcdVariant::Lcd19,vec![0x6b,0x20],(170,320),true),
            (LcdVariant::Touch19,vec![0x15,0x6b,0x20],(170,320),true),
            (LcdVariant::Touch169,vec![0x15,0x6b,0x51,0x18],(240,280),false),
            (LcdVariant::Lcd169,vec![0x6b,0x51,0x18],(240,280),false),
            (LcdVariant::Touch147,vec![0x63,0x6b],(172,320),true),
        ]{
            let mut b=WaveshareLcd::new(variant);let devices=b.i2c_devices();assert_eq!(devices.iter().map(|(_,a,_)|*a).collect::<Vec<_>>(),addresses);
            assert!(devices.iter().all(|(_,_,d)|d.pins()==Some(b.w.i2c)));assert_eq!(b.named_pin("boot"),Some(9));assert_eq!((b.w.width,b.w.height),size);assert_eq!(b.w.sd.is_some(),sd);
            assert_eq!(b.led.is_some(),variant==LcdVariant::Lcd085,"do not invent IMU or LED parts from copied example files");
        }
    }
    #[test]
    fn schematic_wires_reset_phase_and_viewport_gate_lcd_pixels(){
        for variant in [LcdVariant::Lcd085,LcdVariant::Lcd19,LcdVariant::Touch19,LcdVariant::Touch169,LcdVariant::Lcd169,LcdVariant::Touch147]{
            let mut b=WaveshareLcd::new(variant);let(mut g,mut m)=activate(&mut b);let(x,y)=(b.w.gap.0 as u8,b.w.gap.1 as u8);
            command(&mut b,&mut g,&mut m,0x11,&[]);command(&mut b,&mut g,&mut m,0x29,&[]);command(&mut b,&mut g,&mut m,0x3a,&[0x55]);
            command(&mut b,&mut g,&mut m,0x2a,&[0,x,0,x]);command(&mut b,&mut g,&mut m,0x2b,&[0,y,0,y]);command(&mut b,&mut g,&mut m,0x2c,&[0xf8,0]);
            assert_eq!(b.display().unwrap().2[0],0xf800);assert_eq!(b.panel.pixels_written,1);
            let mut wrong=SpiQspiPins{sclk:1<<b.w.mosi,data:[1<<b.w.sclk,0,0,0],cs:1<<b.w.cs,data_width:1,data_bits:16,..Default::default()};
            b.spi_transfer_qspi_pins(2,wrong,&[0,0x1f],0);assert_eq!(b.panel.pixels_written,1,"copied 0.85 BSP swap is not a second valid wiring");
            wrong.sclk=1<<b.w.sclk;wrong.data[0]=1<<b.w.mosi;wrong.data_width=4;b.spi_transfer_qspi_pins(2,wrong,&[0,0x1f],0);assert_eq!(b.panel.pixels_written,1,"single-lane panel must reject QSPI");
            gpio(&mut g,&mut m,b.w.rst,false);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert!(b.display().unwrap().2.iter().all(|&p|p==0));assert_eq!(b.panel.resets,1);
        }
    }
    #[test]
    fn cst_polling_board_has_no_soldered_irq_and_169_shares_display_reset(){
        let mut b=WaveshareLcd::new(LcdVariant::Touch19);b.touch(169,319,true);assert_eq!(b.touch_irq(),None);assert!(b.take_edges().is_empty());
        let mut d=b.i2c_devices().remove(0).2;assert!(d.start_address_with_clock(0x15,0x15,false,Some(400000)));assert!(d.write(0));assert!(d.write(0));d.stop();assert!(d.start_address_with_clock(0x15,0x15,false,Some(400000)));assert!(d.write(0));assert!(d.start_address_with_clock(0x15,0x15,true,Some(400000)));
        let report:Vec<_>=(0..7).map(|_|d.read()).collect();assert_eq!(report,[0,0,1,0,169,1,63]);
        assert!(d.start_address_with_clock(0x15,0x15,false,None));assert!(!d.write(0xff));
        let mut b=WaveshareLcd::new(LcdVariant::Touch169);let(mut g,mut m)=activate(&mut b);b.touch(10,20,true);assert_eq!(b.touch_irq(),Some((11,false)));
        gpio(&mut g,&mut m,4,false);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert_eq!(b.touch_irq(),Some((11,true)));assert!(!b.cst.as_ref().unwrap().lock().unwrap().powered);
        gpio(&mut g,&mut m,4,true);b.gpio_waveform_at(0,ChipPins::C6.routes(&g,&m));assert!(!b.cst.as_ref().unwrap().lock().unwrap().down);
    }
    #[test]
    fn waveshare_169_non_touch_has_power_key_and_does_not_create_touch_or_sd(){
        use esp_periph::sdmmc::card::MemStore;
        let mut b=WaveshareLcd::new(LcdVariant::Lcd169);
        assert_eq!(b.named_pin("power"),Some(18));assert_eq!(b.named_pin("boot"),Some(9));
        b.touch(120,140,true);assert_eq!(b.touch_irq(),None);assert!(b.take_edges().is_empty());
        assert!(b.i2c_devices().iter().all(|(_,a,_)|*a!=0x15));
        assert!(b.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),SdCardConfig::default()).is_err());
        assert_eq!(LcdVariant::Lcd169.flash_mb(),16);
        assert_eq!(WaveshareLcd::new(LcdVariant::Touch169).named_pin("power"),Some(18));
    }
}
