//! M5Stack Tab5 C145, ILI9881C + GT911 production batch. Manufacturer
//! M5Tab5-UserDemo fe8aa051 BSP: SDA31/SCL32, PI4IO 43/44, LCD_RST=P4,
//! TP_RST=P5, BL22, touch INT23. ST7121/ST7123 batches are distinct and are
//! not silently identified as this board. Analog charging, C6, camera/audio,
//! RTC/IMU/current monitors, USB host and card wiring remain outside this subset.
use esp_periph::i2c::{I2cDevice, gt911::{Gt911, Gt911State}, pi4ioe5v6408::{Pi4ioe5v6408, Pi4ioState}};
use esp_soc::{BoardModel, board::BoardEdge, pins::PinRoutes};
use std::sync::{Arc, Mutex};
const W: usize = 720;
const H: usize = 1280;
type Io = Arc<Mutex<Pi4ioState>>;
fn reset_released(s: &Pi4ioState, pin: u8) -> bool { s.powered && s.drive(pin).unwrap_or_else(|| s.input() & (1 << pin) != 0) }
struct Wired { inner: Box<dyn I2cDevice> }
impl I2cDevice for Wired {
    fn pins(&self) -> Option<(u8,u8)> { Some((31,32)) }
    fn start(&mut self, r:bool) -> bool { self.inner.start(r) }
    fn write(&mut self, b:u8) -> bool { self.inner.write(b) }
    fn read(&mut self) -> u8 { self.inner.read() }
    fn stop(&mut self) { self.inner.stop(); }
}
struct Touch { inner:Gt911, state:Arc<Mutex<Gt911State>>, io:Io, low_edges:u64 }
impl I2cDevice for Touch {
    fn start(&mut self, r:bool) -> bool {
        let io = self.io.lock().unwrap(); let mut t = self.state.lock().unwrap();
        if self.low_edges != io.low_edges[5] { t.ready = false; self.low_edges = io.low_edges[5]; }
        t.powered = reset_released(&io,5); if !t.powered { t.ready = false; }
        drop(t); drop(io); self.inner.start(r)
    }
    fn write(&mut self,b:u8)->bool {self.inner.write(b)}
    fn read(&mut self)->u8 {self.inner.read()}
    fn stop(&mut self) {self.inner.stop();}
}
pub struct Tab5 {
    pub io1:Io, pub io2:Io, pub touch_state:Arc<Mutex<Gt911State>>,
    pixels:Vec<u16>, frames:u64, version:u64, page:u8, sleeping:bool, on:bool,
    colmod:u8, madctl:u8, reset:bool, reset_edges:u64, link:bool, backlight:bool, fresh:bool,
    cycle:u64, irq:bool, edges:Vec<BoardEdge>,
}
impl Default for Tab5 {fn default()->Self {Self::new()}}
impl Tab5 {
    pub fn new()->Self {
        let touch_state = Arc::new(Mutex::new(Gt911State::new(W as u16,H as u16,crate::periph::CPU_HZ/100)));
        touch_state.lock().unwrap().powered = false;
        Self {io1:Arc::new(Mutex::new(Pi4ioState::default())),io2:Arc::new(Mutex::new(Pi4ioState::default())),touch_state,
            pixels:vec![0;W*H],frames:0,version:0,page:0,sleeping:true,on:false,colmod:0x66,madctl:0,reset:true,reset_edges:0,link:false,backlight:false,fresh:false,cycle:0,irq:true,edges:Vec::new()}
    }
    fn visible(&self)->bool {self.on && !self.sleeping && !self.reset && self.link && self.backlight && self.fresh}
    fn reset_panel(&mut self) {self.page=0;self.sleeping=true;self.on=false;self.colmod=0x66;self.madctl=0;self.fresh=false;self.version=self.version.wrapping_add(1);}
    fn sync_resets(&mut self) {
        let io=self.io1.lock().unwrap();let reset=!reset_released(&io,4);let low=io.low_edges[4];let tp=reset_released(&io,5);drop(io);
        if low!=self.reset_edges || reset && !self.reset {self.reset_panel();}
        if reset!=self.reset {self.version=self.version.wrapping_add(1);}
        self.reset=reset;self.reset_edges=low;
        let mut t=self.touch_state.lock().unwrap();if !tp{t.ready=false;}t.powered=tp;
    }
    fn update_backlight(&mut self,on:bool) {if self.backlight!=on{self.backlight=on;self.version=self.version.wrapping_add(1);}}
}
impl BoardModel for Tab5 {
    fn name(&self)->&'static str {"m5stack-tab5"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:u64,routes:PinRoutes<'_>) {self.cycle=cycle;self.sync_resets();self.update_backlight(routes.software_output(22)==Some(true));}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        // SDA/SCL may be routed to either hardware controller, but the physical
        // pins must match on every address phase. They are not two separate buses.
        let mut out=Vec::new();
        for bus in [0,1] {
            let entries:Vec<(u8,Box<dyn I2cDevice>)>=vec![
                (0x43,Box::new(Pi4ioe5v6408::new(self.io1.clone()))),
                (0x44,Box::new(Pi4ioe5v6408::new(self.io2.clone()))),
                (0x14,Box::new(Touch{inner:Gt911::new(self.touch_state.clone()),state:self.touch_state.clone(),io:self.io1.clone(),low_edges:self.io1.lock().unwrap().low_edges[5]}))];
            for (addr,inner) in entries {out.push((bus,addr,Box::new(Wired{inner}) as Box<dyn I2cDevice>));}
        }
        out
    }
    fn advance_to(&mut self,cycle:u64) {
        self.cycle=cycle;self.sync_resets();let mut t=self.touch_state.lock().unwrap();t.advance_to(cycle);
        let irq=!t.powered || !t.ready;drop(t);if self.irq!=irq{self.irq=irq;self.edges.push(BoardEdge{cycle,pin:23,level:irq});}
    }
    fn advance_inputs_with_clock(&mut self,cycle:u64,clock:&dyn Fn(u8)->Option<(f64,u32)>) {self.advance_to(cycle);if let Some((hz,duty))=clock(22){self.update_backlight(hz>0.0 && duty!=0);}}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(23,self.irq)]}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.touch_state.lock().unwrap().update(x,y,down);}
    fn dsi_packet(&mut self,vc:u8,kind:u8,payload:&[u8])->Vec<u8> {
        self.sync_resets();if vc!=0 || self.reset || payload.is_empty(){return Vec::new();}
        let cmd=payload[0];let args=&payload[1..];
        if kind==0x06 {return match cmd{0x0a=>vec![if self.on{4}else{0}|if !self.sleeping{0x10}else{0}],0x0b=>vec![self.madctl],0x0c=>vec![self.colmod],_=>Vec::new()};}
        if !matches!(kind,0x05|0x15|0x39){return Vec::new();}
        // ILI9881C vendor pages contain command numbers overlapping base DCS.
        if cmd==0xff && args.len()==3 && args[0..2]==[0x98,0x81]{self.page=args[2];return Vec::new();}
        if self.page!=0{return Vec::new();}
        match cmd {0x01=>self.reset_panel(),0x10=>self.sleeping=true,0x11=>self.sleeping=false,0x28=>self.on=false,0x29=>self.on=true,
            0x36=>if let Some(&v)=args.first(){self.madctl=v;},0x3a=>if let Some(&v)=args.first(){self.colmod=v;},_=>{}}
        self.version=self.version.wrapping_add(1);Vec::new()
    }
    fn dsi_link(&mut self,enabled:bool) {self.sync_resets();if !enabled{self.fresh=false;}
        if self.link!=enabled{self.link=enabled;self.version=self.version.wrapping_add(1);}}
    fn lcd_frame(&mut self,w:u32,h:u32,bytes:&[u8]) {
        self.sync_resets();if self.reset || !self.link || w as usize!=W || h as usize!=H || bytes.len()!=W*H*2 || self.colmod&0x70!=0x50{return;}
        for (i,b) in bytes.as_chunks::<2>().0.iter().enumerate() {
            let (mut x,mut y)=(i%W,i/W);if self.madctl&0x40!=0{x=W-1-x;}
            if self.madctl&0x80!=0{y=H-1-y;}
            let mut px=u16::from_le_bytes([b[0],b[1]]);if self.madctl&8!=0{px=px&0x7e0|(px&31)<<11|(px>>11&31);}self.pixels[y*W+x]=px;
        }
        self.fresh=true;self.frames=self.frames.wrapping_add(1);self.version=self.version.wrapping_add(1);
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((W as u32,H as u32,if self.visible(){self.pixels.clone()}else{vec![0;W*H]},self.version))}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.pixels.clone(),W,H))}
    fn display_version(&self)->u64 {self.version}
    fn display_frames(&self)->u64 {self.frames}
    fn report(&self)->String {format!("[tab5] ILI9881C+GT911 batch, {} DMA frames; DSI={} LCDreset={} on={} sleep={} BL={}. ST7121/ST7123 variants, analog power/charging, C6/radio, CSI/audio, RTC/IMU/INA226, SD/USB host unverified or unimplemented; no factory/hardware acceptance",self.frames,self.link,self.reset,self.on,self.sleeping,self.backlight)}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    fn link_recovery_requires_a_complete_fresh_frame_and_backlight_does_not_create_one() {
        let mut b=Tab5::new();let mut io=Pi4ioe5v6408::new(b.io1.clone());
        for (reg,value) in [(13,0x30),(11,0x30),(5,0x20),(7,0),(3,0x20)] {assert!(io.start(false));assert!(io.write(reg));assert!(io.write(value));}
        for (cmd,args) in [(0x3a,vec![0x55]),(0x11,vec![]),(0x29,vec![])] {let mut p=vec![cmd];p.extend(args);b.dsi_packet(0,if p.len()==1{5}else{0x15},&p);}
        b.update_backlight(true);b.dsi_link(true);let mut pixels=vec![0;W*H*2];for p in pixels.as_chunks_mut::<2>().0 {*p=[0x1f,0];}
        b.lcd_frame(W as u32,H as u32,&pixels);assert_eq!(b.display().unwrap().2[721],0x001f);assert_eq!(b.display_frames(),1);
        b.dsi_link(false);b.dsi_link(true);b.update_backlight(false);b.update_backlight(true);b.lcd_frame(W as u32,H as u32,&pixels[..pixels.len()-2]);
        assert!(b.display().unwrap().2.iter().all(|p|*p==0),"restored link and invalid DMA length cannot revive retained pixels");assert_eq!(b.display_frames(),1);
        for p in pixels.as_chunks_mut::<2>().0 {*p=[0,0xf8];}b.lcd_frame(W as u32,H as u32,&pixels);assert_eq!(b.display().unwrap().2[721],0xf800);assert_eq!(b.display_frames(),2);
        b.update_backlight(false);b.update_backlight(true);assert_eq!(b.display().unwrap().2[721],0xf800,"backlight alone reveals the last complete active-link frame");
    }
}
