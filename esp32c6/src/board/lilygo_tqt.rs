//! T-QT-C6 MCU V1.2, GC9107 glass: manufacturer 667396e6 and its pinned
//! Arduino_GFX 00dcd684. Source receipts/scope: lilygo-c6-profiles-2026-11-22.json.
//! SPI RGB565 uses the manufacturer's four rotation/offset configurations.
//! This is a visible-window digital subset, not a claim about undocumented GRAM.
use esp_periph::i2c::I2cDevice;
use esp_soc::{BoardModel,board::{BoardEdge,SpiPins},devices::{DcsPanel,i2c_bitbang::I2cBitBang},pins::PinRoutes};
use std::sync::{Arc,Mutex};

struct Gc9107 {panel:DcsPanel,command:u8,args:[u8;4],count:usize,valid:[bool;2],changes:u64,rejected:u64}
impl Gc9107 {
    fn new()->Self {Self{panel:DcsPanel::new(128,128),command:0,args:[0;4],count:0,valid:[false;2],changes:0,rejected:0}}
    fn reset(&mut self) {let version=self.version();self.panel.reset();self.command=0;self.count=0;self.valid=[false;2];self.changes=version.wrapping_add(1);}
    fn offsets(&self)->Option<(u16,u16)> {
        // Arduino_TFT::setRotation + T-QT-C6 pin_config.h, not a larger invented GRAM.
        match self.panel.madctl&0xe0 {0xc0=>Some((2,1)),0xa0=>Some((1,0)),0x00=>Some((0,0)),0x60=>Some((0,2)),_=>None}
    }
    fn byte(&mut self,data:bool,value:u8) {
        if !data {
            self.command=value;self.count=0;
            if value==0x01 {self.reset();return;}
            if value==0x2a || value==0x2b {self.valid[usize::from(value==0x2b)]=false;}
            self.panel.dc=false;self.panel.byte(value);
            if matches!(value,0x10|0x11|0x20|0x21|0x28|0x29) {self.changes=self.changes.wrapping_add(1);}
            return;
        }
        self.panel.dc=true;
        match self.command {
            0x2a|0x2b=>{
                if self.count>=4 {return;}
                self.args[self.count]=value;self.count+=1;
                if self.count==4 {
                    let index=usize::from(self.command==0x2b);
                    let Some((x,y))=self.offsets() else {self.rejected+=1;return;};let offset=if index==0 {x}else{y};
                    let start=u16::from_be_bytes([self.args[0],self.args[1]]).checked_sub(offset);
                    let end=u16::from_be_bytes([self.args[2],self.args[3]]).checked_sub(offset);
                    if let (Some(start),Some(end))=(start,end) {
                        if start<=end && end<128 {self.valid[index]=true;for b in start.to_be_bytes().into_iter().chain(end.to_be_bytes()) {self.panel.byte(b);}return;}
                    }
                    self.rejected+=1;
                }
            }
            0x36=>{if self.count==0 {self.panel.byte(value);self.valid=[false;2];self.changes=self.changes.wrapping_add(1);}self.count=self.count.saturating_add(1);}
            0x3a=>{if self.count==0 {self.panel.byte(value);}self.count=self.count.saturating_add(1);}
            0x2c if self.valid==[true;2] && self.offsets().is_some() && self.panel.colmod&7==5=>self.panel.byte(value),
            _=>{}
        }
    }
    fn pixels(&self,brightness:u8,reset_high:bool)->Vec<u16> {
        if !reset_high || brightness==0 || self.panel.sleeping || !self.panel.on {return vec![0;128*128];}
        // Glass mounting makes the official rotation0 (MADCTL=C8) upright. The
        // source selects IPS/BGR; INVON is its normal polarity, not a host effect.
        self.panel.gram.iter().rev().map(|&raw| {
            let raw=if self.panel.madctl&8==0 {(raw&0x07e0)|((raw&31)<<11)|(raw>>11)}else{raw};
            let raw=if self.panel.inverted {raw}else{!raw};let brightness=u32::from(brightness);
            let r=((u32::from(raw)>>11)&31)*brightness/255;let g=((u32::from(raw)>>5)&63)*brightness/255;let b=(u32::from(raw)&31)*brightness/255;
            ((r<<11)|(g<<5)|b) as u16
        }).collect()
    }
    fn version(&self)->u64 {self.changes.wrapping_add(self.panel.pixels_written)}
}

// CST816T application note AN-CST816T-v1: 01h..06h contact report, E5h=03h
// sleep (NOT the CST816S A5h). EnChange FAh bit5 emits EDh*0.1ms IRQ pulses.
// Only contact-change IRQ mode is implemented; gesture/periodic/test modes,
// automatic standby and unknown identity registers are not fabricated.
struct Touch {reset_high:bool,sleeping:bool,x:u16,y:u16,down:bool,irq_ctl:u8,irq_width:u8,irq_end:Option<u64>}
impl Touch {
    fn new()->Self {Self{reset_high:true,sleeping:false,x:0,y:0,down:false,irq_ctl:0,irq_width:10,irq_end:None}}
    fn active(&self)->bool {self.reset_high && !self.sleeping}
    fn irq_high(&self)->bool {!self.active() || self.irq_end.is_none()}
    fn reset(&mut self,high:bool) {if self.reset_high && !high {*self=Self::new();self.reset_high=false;}else{self.reset_high=high;}}
    fn update(&mut self,cycle:u64,x:u16,y:u16,down:bool) {
        if !self.active() {return;}
        let (x,y)=(x.min(127),y.min(127));let changed=(self.x,self.y,self.down)!=(x,y,down);self.x=x;self.y=y;self.down=down;
        if changed && self.irq_ctl&0x20!=0 {self.irq_end=Some(cycle.saturating_add(crate::periph::CPU_HZ/10_000*u64::from(self.irq_width)));}
    }
}
struct Cst816t {state:Arc<Mutex<Touch>>,pointer:u8,first:bool}
impl Cst816t {fn new(state:Arc<Mutex<Touch>>)->Self {Self{state,pointer:0,first:true}}}
impl I2cDevice for Cst816t {
    fn pins(&self)->Option<(u8,u8)> {Some((21,22))}
    fn start(&mut self,read:bool)->bool {
        if !self.state.lock().expect("CST816T state").active() {return false;}
        if read {matches!(self.pointer,1..=6|0xe5|0xed|0xfa)}else{self.first=true;true}
    }
    fn write(&mut self,value:u8)->bool {
        let mut state=self.state.lock().expect("CST816T state");if !state.active() {return false;}
        if self.first {self.pointer=value;self.first=false;return true;}
        match self.pointer {
            0xe5 if value==3=>{state.sleeping=true;state.down=false;state.irq_end=None;}
            0xed if (1..=200).contains(&value)=>state.irq_width=value,
            0xfa=>{state.irq_ctl=value;if value&0x20==0 {state.irq_end=None;}}
            _=>return false,
        }
        self.pointer=self.pointer.wrapping_add(1);true
    }
    fn read(&mut self)->u8 {
        let state=self.state.lock().expect("CST816T state");
        let value=if !state.active() {0xff}else{match self.pointer {1=>0,2=>u8::from(state.down),3=>(state.x>>8) as u8&15,4=>state.x as u8,5=>(state.y>>8) as u8&15,6=>state.y as u8,0xe5=>0,0xed=>state.irq_width,0xfa=>state.irq_ctl,_=>0xff}};
        self.pointer=self.pointer.wrapping_add(1);value
    }
}

pub struct TQtC6 {panel:Gc9107,dc:Option<bool>,reset_high:bool,backlight_gpio:Option<bool>,brightness:u8,led_gpio:Option<bool>,leds:[[u8;3];1],led_updates:u64,boot_high:bool,cycle:u64,touch:Arc<Mutex<Touch>>,i2c:I2cBitBang,levels:[bool;2],irq_high:bool,edges:Vec<BoardEdge>,gpio_events:u64,spi_matched:u64}
impl Default for TQtC6 {fn default()->Self {Self::new()}}
impl TQtC6 {
    pub fn new()->Self {Self{panel:Gc9107::new(),dc:None,reset_high:true,backlight_gpio:None,brightness:0,led_gpio:None,leds:[[0;3];1],led_updates:0,boot_high:true,cycle:0,touch:Arc::new(Mutex::new(Touch::new())),i2c:I2cBitBang::new(),levels:[true;2],irq_high:true,edges:Vec::new(),gpio_events:0,spi_matched:0}}
    fn backlight(&mut self,brightness:u8) {if self.brightness!=brightness {self.brightness=brightness;self.panel.changes=self.panel.changes.wrapping_add(1);}}
    fn led(&mut self,brightness:u8) {let color=[brightness;3];if self.leds[0]!=color {self.leds[0]=color;self.led_updates=self.led_updates.wrapping_add(1);}}
    fn sync_irq(&mut self,cycle:u64) {let high=self.touch.lock().expect("CST816T state").irq_high();if high!=self.irq_high {self.irq_high=high;self.edges.push(BoardEdge{cycle,pin:7,level:high});}}
}
impl BoardModel for TQtC6 {
    fn name(&self)->&'static str {"lilygo-t-qt-c6"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,cycle:u64,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.advance_to(cycle);self.gpio_events+=changes.len() as u64;}
    fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>) {
        self.advance_to(cycle);self.dc=routes.software_output(19);let high=routes.software_output(20)!=Some(false);
        if self.reset_high && !high {self.panel.reset();}self.reset_high=high;
        self.backlight_gpio=routes.software_output(2);self.backlight(if self.backlight_gpio==Some(false) {255}else{0});
        self.led_gpio=routes.software_output(9);self.led(if self.led_gpio==Some(false) {255}else{0});
        self.touch.lock().expect("CST816T state").reset(routes.software_output(23)!=Some(false));
        let scl=routes.software_output(22)!=Some(false);let sda=self.i2c.drive(scl,routes.software_output(21)!=Some(false));
        for (index,(pin,level)) in [(21,sda),(22,scl)].into_iter().enumerate() {if self.levels[index]!=level {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        self.sync_irq(cycle);
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:u64,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        let low=|pin,software|pwm(pin).map(|(_,duty)|((65535-duty.min(65535))*255/65535) as u8).unwrap_or(if software==Some(false) {255}else{0});
        self.backlight(low(2,self.backlight_gpio));self.led(low(9,self.led_gpio));
    }
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if self.reset_high && pins.sclk&(1<<18)!=0 && pins.mosi&(1<<15)!=0 && pins.cs&(1<<14)!=0 {
            if let Some(dc)=self.dc {self.spi_matched+=1;for &value in tx {self.panel.byte(dc,value);}}
        }
        vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x15,Box::new(Cst816t::new(self.touch.clone())));
        self.i2c.controller_devices(Some((21,22))).into_iter().map(|(address,device)|(0,address,device)).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(8,true),(9,self.boot_high),(7,self.touch.lock().expect("CST816T state").irq_high()),(21,self.levels[0]),(22,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"button"|"btnA"=>Some(9),_=>None}}
    fn button_at(&mut self,cycle:u64,id:&str,pressed:bool)->bool {
        if self.named_pin(id)!=Some(9) {return false;}self.advance_to(cycle);
        let high=!pressed;if self.boot_high!=high {self.boot_high=high;self.edges.push(BoardEdge{cycle,pin:9,level:high});}true
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.touch.lock().expect("CST816T state").update(self.cycle,x,y,down);self.sync_irq(self.cycle);}
    fn touch_at(&mut self,cycle:u64,x:u16,y:u16,down:bool) {self.advance_to(cycle);self.touch(x,y,down);}
    fn touch_irq(&self)->Option<(u8,bool)> {Some((7,self.touch.lock().expect("CST816T state").irq_high()))}
    fn next_deadline(&self)->Option<u64> {self.touch.lock().expect("CST816T state").irq_end}
    fn advance_to(&mut self,cycle:u64) {
        self.cycle=cycle;let due={let mut touch=self.touch.lock().expect("CST816T state");if touch.irq_end.is_some_and(|end|end<=cycle) {touch.irq_end.take()}else{None}};
        self.sync_irq(due.unwrap_or(cycle));
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {self.sync_irq(self.cycle);std::mem::take(&mut self.edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((128,128,self.panel.pixels(self.brightness,self.reset_high),self.display_version()))}
    fn display_version(&self)->u64 {self.panel.version()}
    fn display_frames(&self)->u64 {self.panel.panel.frames}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.leds,self.led_updates))}
    fn report(&self)->String {format!("[lilygo-t-qt-c6] GC9107 128x128 pixels={} spi={} rejected_windows={} brightness={}; CST816T polled contact/change IRQ only; gesture IRQ, PMIC, IMU and battery unmodeled",self.panel.panel.pixels_written,self.spi_matched,self.panel.rejected,self.brightness)}
}
