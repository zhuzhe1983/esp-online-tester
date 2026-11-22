//! Source-backed LILYGO display/control subsets. Each profile names its own board;
//! sharing a transport never makes firmware, radios or hardware revisions equivalent.
//! Pin/GRAM references and unimplemented peripherals are recorded in
//! docs/platform/research/lilygo-runtime-profiles-2026-11-22.json.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,tca9555::{Tca9555,Tca9555State}};
use esp_soc::board::{Board,BoardModel,BoardEdge,SpiPins,VirtualCycle};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};
use esp_soc::pins::PinRoutes;

#[derive(Clone,Copy,PartialEq)]
enum TouchKind {Cst816,Cst226,Ft6336}
#[derive(Clone,Copy)]
struct TouchWiring {kind:TouchKind,address:u8,sda:u8,scl:u8,irq:u8,reset:Option<u8>}
struct Profile {
    name:&'static str,controller:&'static str,gram:[usize;2],glass:[usize;4],
    spi:[u8;3],dc:Option<u8>,reset:Option<u8>,light:Option<(u8,bool)>,light_pwm:bool,enable:Option<u8>,quad:bool,
    buttons:&'static [(&'static str,u8)],encoder:Option<(u8,u8)>,touch:Option<TouchWiring>,expander:bool,
}
const BOOT:&[(&str,u8)]=&[("boot",0)];
const PRO:Profile=Profile{name:"lilygo-t-display-s3-pro",controller:"ST7796",gram:[320,480],glass:[49,0,222,480],spi:[18,17,39],dc:Some(9),reset:Some(47),light:Some((48,true)),light_pwm:true,enable:None,quad:false,buttons:&[("boot",0),("button-2",12),("button-3",16)],encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst226,address:0x5a,sda:5,scl:6,irq:21,reset:Some(13)}),expander:false};
const PRO_EXTERNAL:Profile=Profile{name:"lilygo-t-display-s3-pro-external",..PRO};
const PRO_LR1121:Profile=Profile{name:"lilygo-t-display-s3-pro-lr1121",..PRO};
const BAR:Profile=Profile{name:"lilygo-t-display-bar",controller:"ST7789",gram:[240,320],glass:[82,18,76,284],spi:[7,6,8],dc:Some(5),reset:Some(40),light:Some((15,false)),light_pwm:true,enable:None,quad:false,buttons:&[("boot",0),("button-1",38)],encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst816,address:0x15,sda:2,scl:3,irq:21,reset:Some(1)}),expander:false};
const ULTRA:Profile=Profile{name:"lilygo-t-watch-ultra",controller:"CO5300",gram:[454,502],glass:[22,0,410,502],spi:[40,38,41],dc:None,reset:Some(37),light:None,light_pwm:false,enable:None,quad:true,buttons:BOOT,encoder:None,touch:None,expander:true};
const PAGER:Profile=Profile{name:"lilygo-t-lora-pager",controller:"ST7796",gram:[320,480],glass:[49,0,222,480],spi:[35,34,38],dc:Some(37),reset:None,light:Some((42,true)),light_pwm:false,enable:None,quad:false,buttons:&[("boot",0),("encoder-press",7)],encoder:Some((40,41)),touch:None,expander:false};
const PAGER_MESHTASTIC:Profile=Profile{name:"lilygo-t-lora-pager-meshtastic",..PAGER};
const WATCH:Profile=Profile{name:"lilygo-t-watch-s3",controller:"ST7789",gram:[240,320],glass:[0,0,240,240],spi:[18,13,12],dc:Some(38),reset:None,light:Some((45,true)),light_pwm:true,enable:None,quad:false,buttons:&[],encoder:None,touch:Some(TouchWiring{kind:TouchKind::Ft6336,address:0x38,sda:39,scl:40,irq:16,reset:None}),expander:false};
const WATCH_PLUS:Profile=Profile{name:"lilygo-t-watch-s3-plus",buttons:BOOT,..WATCH};
const AMOLED:Profile=Profile{name:"lilygo-t-display-s3-amoled",controller:"RM67162",gram:[240,536],glass:[0,0,240,536],spi:[47,18,6],dc:None,reset:Some(17),light:None,light_pwm:false,enable:None,quad:true,buttons:BOOT,encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst816,address:0x15,sda:3,scl:2,irq:21,reset:None}),expander:false};
const AMOLED_PLUS:Profile=Profile{name:"lilygo-t-display-s3-amoled-plus",dc:Some(7),enable:Some(38),quad:false,..AMOLED};
const AMOLED_LITE:Profile=Profile{name:"lilygo-t-display-amoled-lite",controller:"SH8501",gram:[368,194],glass:[0,0,368,194],spi:[5,7,4],dc:None,reset:Some(40),light:None,light_pwm:false,enable:None,quad:true,buttons:&[("boot",0),("user",21)],encoder:None,touch:None,expander:false};
// RM690B0 library adds 16 address columns on both portrait orientations. The
// padded coordinate canvas is a model of that documented visible window, not a
// claim about the controller's physical storage capacity.
const T4:Profile=Profile{name:"lilygo-t4-s3",controller:"RM690B0",gram:[482,600],glass:[16,0,450,600],spi:[15,14,11],dc:None,reset:Some(13),light:None,light_pwm:false,enable:Some(9),quad:true,buttons:BOOT,encoder:None,touch:Some(TouchWiring{kind:TouchKind::Cst226,address:0x5a,sda:6,scl:7,irq:8,reset:Some(17)}),expander:false};
const EMBED_PLUS:Profile=Profile{name:"lilygo-t-embed-cc1101-plus",controller:"ST7789",gram:[240,320],glass:[35,0,170,320],spi:[11,9,41],dc:Some(16),reset:None,light:Some((21,true)),light_pwm:false,enable:None,quad:false,buttons:&[("boot",0),("encoder-press",0),("user",6)],encoder:Some((4,5)),touch:None,expander:false};
const EMBED:Profile=Profile{name:"lilygo-t-embed",controller:"ST7789",gram:[240,320],glass:[35,0,170,320],spi:[12,11,10],dc:Some(13),reset:Some(9),light:Some((15,true)),light_pwm:false,enable:Some(46),quad:false,buttons:&[("boot",0),("encoder-press",0)],encoder:Some((2,1)),touch:None,expander:false};
const EMBED_SI4732:Profile=Profile{name:"lilygo-t-embed-si4732",..EMBED};
const DONGLE:Profile=Profile{name:"lilygo-t-dongle-s3",controller:"ST7735",gram:[132,162],glass:[26,1,80,160],spi:[5,3,4],dc:Some(2),reset:Some(1),light:Some((38,false)),light_pwm:true,enable:None,quad:false,buttons:BOOT,encoder:None,touch:None,expander:false};
// The small GC9A01 module's current vendor rotation table selects [2,1] at
// rotation 0 and [2,3] at rotation 2. This is a padded command-coordinate canvas,
// not a statement of the controller's physical memory size or old-panel support.
const QT:Profile=Profile{name:"lilygo-t-qt-pro",controller:"GC9A01",gram:[132,132],glass:[2,1,128,128],spi:[3,2,5],dc:Some(6),reset:Some(1),light:Some((10,false)),light_pwm:true,enable:None,quad:false,buttons:&[("boot",0),("user",47)],encoder:None,touch:None,expander:false};
const AMOLED164:Profile=Profile{name:"lilygo-t-display-s3-amoled-1-64",controller:"CO5300",gram:[320,456],glass:[20,0,280,456],spi:[12,11,10],dc:None,reset:Some(17),light:None,light_pwm:false,enable:Some(16),quad:true,buttons:BOOT,encoder:None,touch:None,expander:false};
const PROFILES:&[&Profile]=&[&PRO_EXTERNAL,&PAGER_MESHTASTIC,&EMBED_PLUS,&ULTRA,&BAR,&EMBED_SI4732,&PAGER,&PRO_LR1121,&WATCH_PLUS,&AMOLED_PLUS,&AMOLED164,&T4,&AMOLED_LITE,&PRO,&AMOLED,&WATCH,&DONGLE,&EMBED,&QT];

pub fn make_board(name:&str)->Option<Board> {PROFILES.iter().find(|p|p.name==name).map(|&p|Box::new(LilygoDisplay::new(p)) as Board)}

struct TouchState {powered:bool,asleep:bool,down:bool,x:u16,y:u16,width:u16,height:u16}
impl TouchState {fn clear(&mut self) {self.down=false;self.asleep=false;}}
/// Digital report/command subsets derived from the pinned manufacturer's SensorLib.
/// No firmware/project/calibration identifiers, pressure or gestures are invented.
struct TouchDevice {kind:TouchKind,state:Arc<Mutex<TouchState>>,registers:[u8;256],ptr:u8,address_byte:bool,prefix:Option<u8>,word:u16,read_at:usize}
impl TouchDevice {
    fn new(kind:TouchKind,state:Arc<Mutex<TouchState>>)->Self {Self{kind,state,registers:[0;256],ptr:0,address_byte:true,prefix:None,word:0,read_at:0}}
}
impl I2cDevice for TouchDevice {
    fn start(&mut self,read:bool)->bool {let state=self.state.lock().expect("LILYGO touch state mutex poisoned");if !state.powered || state.asleep {return false;}drop(state);if !read {self.address_byte=true;self.prefix=None;}true}
    fn write(&mut self,b:u8)->bool {
        if self.address_byte {self.address_byte=false;self.prefix=if self.kind==TouchKind::Cst226 && matches!(b,0xd1|0xd2) {Some(b)} else {None};self.word=u16::from(b);self.ptr=b;self.read_at=0;return true;}
        if let Some(prefix)=self.prefix.take() {self.word=(u16::from(prefix)<<8)|u16::from(b);self.read_at=0;if self.word==0xd105 {self.state.lock().expect("LILYGO touch state mutex poisoned").asleep=true;}return true;}
        let reg=self.ptr;
        if matches!(reg,0xa5|0xe5) && b==3 {let mut s=self.state.lock().expect("LILYGO touch state mutex poisoned");s.down=false;s.asleep=true;}
        if !matches!(reg,0x01..=0x08|0xa3|0xa6..=0xa9) {self.registers[usize::from(reg)]=b;}
        self.ptr=self.ptr.wrapping_add(1);true
    }
    fn read(&mut self)->u8 {
        let s=self.state.lock().expect("LILYGO touch state mutex poisoned");
        if self.kind==TouchKind::Cst226 {
            let report=[u8::from(s.down)*6,(s.x>>4) as u8,(s.y>>4) as u8,((s.x&15)<<4) as u8|(s.y&15) as u8,0,u8::from(s.down),0xab];
            let value=match self.word {0xd1fc=>[0,0,0xca,0xca].get(self.read_at).copied(),0xd1f8=>[s.width as u8,(s.width>>8) as u8,s.height as u8,(s.height>>8) as u8].get(self.read_at).copied(),0xd204|0xd208=>Some(0),0=>report.get(self.read_at).copied(),_=>None}.unwrap_or(0);
            self.read_at=self.read_at.saturating_add(1);return value;
        }
        self.registers[2]=u8::from(s.down);
        self.registers[3]=((s.x>>8) as u8&15)|if self.kind==TouchKind::Ft6336 {0x80}else{0};self.registers[4]=s.x as u8;
        self.registers[5]=(s.y>>8) as u8&15;self.registers[6]=s.y as u8;
        // The family's documented CST816S model identifier, not a measured die
        // revision. Firmware version stays zero; other CST816 variants are unverified.
        if self.kind==TouchKind::Cst816 {self.registers[0xa7]=0xb4;}
        if self.kind==TouchKind::Ft6336 {self.registers[0xa8]=0x11;}
        let value=self.registers[usize::from(self.ptr)];self.ptr=self.ptr.wrapping_add(1);value
    }
}

struct LilygoDisplay {
    profile:&'static Profile,panel:DcsPanel,dc:Option<bool>,reset:bool,enabled:bool,light:u32,gpio_light:u32,pwm:Option<u32>,
    cs:Option<bool>,header:[u8;4],header_len:usize,command:u8,brightness:u8,
    version:u64,transactions:u64,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,cycle:VirtualCycle,
    touch:Arc<Mutex<TouchState>>,expander:Arc<Mutex<Tca9555State>>,
}
impl LilygoDisplay {
    fn new(profile:&'static Profile)->Self {
        let panel=if profile.controller=="ST7789" {DcsPanel::st7789()} else {DcsPanel::new(profile.gram[0],profile.gram[1])};
        Self{profile,panel,dc:None,reset:profile.reset.is_none(),enabled:profile.enable.is_none(),light:if profile.light.is_none(){65535}else{0},gpio_light:0,pwm:None,cs:None,header:[0;4],header_len:0,command:0,brightness:255,version:0,transactions:0,i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),cycle:0,
            touch:Arc::new(Mutex::new(TouchState{powered:profile.touch.is_some_and(|w|w.reset.is_none()),asleep:false,down:false,x:0,y:0,width:profile.glass[2] as u16,height:profile.glass[3] as u16})),expander:Default::default()}
    }
    fn supplied(&self)->bool {self.enabled && (!self.profile.expander || self.expander.lock().expect("LILYGO expander mutex poisoned").driven_high()&(1<<7)!=0)}
    fn command(&mut self,c:u8) {
        if c==0x3c && self.command==0x2c {return;}
        self.command=c;self.panel.dc=false;self.panel.byte(c);self.panel.dc=true;self.version=self.version.wrapping_add(1);
        if c==1 {self.brightness=255;}
    }
    fn data(&mut self,data:&[u8]) {if self.command==0x2c && self.panel.colmod&7!=5 {return;}self.panel.dc=true;for &b in data {if self.command==0x51 {if self.brightness!=b {self.version=self.version.wrapping_add(1);}self.brightness=b;}self.panel.byte(b);}}
    fn valid_header(data:&[u8])->bool {matches!(data[0],0x02|0x32) && data[1]==0 && data[3]==0}
    fn quad(&mut self,mut data:&[u8]) {
        if self.cs==Some(false) {
            if self.header_len<4 {let n=data.len().min(4-self.header_len);self.header[self.header_len..self.header_len+n].copy_from_slice(&data[..n]);self.header_len+=n;data=&data[n..];if self.header_len<4 {return;}
        if !Self::valid_header(&self.header) {return;}self.command(self.header[2]);}
            if Self::valid_header(&self.header) {self.data(data);}
        } else if data.len()>=4 && Self::valid_header(&data[..4]) {self.command(data[2]);self.data(&data[4..]);}
    }
    fn i2c_pins(&self)->Option<(u8,u8)> {self.profile.touch.map(|w|(w.sda,w.scl)).or(if self.profile.expander {Some((3,2))}else{None})}
    fn touch_level(&self)->bool {let s=self.touch.lock().expect("LILYGO touch state mutex poisoned");!s.powered || s.asleep || !s.down}
}
impl BoardModel for LilygoDisplay {
    fn name(&self)->&'static str {self.profile.name}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;let before=(self.reset,self.enabled,self.light);
        self.reset=self.profile.reset.is_none_or(|p|routes.software_output(p)==Some(true));
        self.enabled=self.profile.enable.is_none_or(|p|routes.software_output(p)==Some(true));
        self.dc=self.profile.dc.and_then(|p|routes.software_output(p));
        self.gpio_light=self.profile.light.map_or(65535,|(p,active)|if routes.software_output(p)==Some(active){65535}else{0});
        self.light=self.pwm.unwrap_or(self.gpio_light);
        if (before.0 && !self.reset) || (before.1 && !self.enabled) {self.panel.reset();self.header_len=0;self.command=0;self.brightness=255;}
        if before!=(self.reset,self.enabled,self.light) {self.version=self.version.wrapping_add(1);}
        let cs=routes.software_output(self.profile.spi[2]);if cs==Some(false) && self.cs!=Some(false) {self.header_len=0;self.header=[0;4];}self.cs=cs;
        if let Some(w)=self.profile.touch {let old=self.touch_level();let mut s=self.touch.lock().expect("LILYGO touch state mutex poisoned");let power=w.reset.is_none_or(|p|routes.software_output(p)==Some(true));if !power {s.clear();}s.powered=power;drop(s);let level=self.touch_level();if old!=level {self.edges.push(BoardEdge{cycle,pin:w.irq,level});}}
        if let Some((sda,scl))=self.i2c_pins() {let high_scl=routes.software_output(scl)!=Some(false);let high_sda=self.i2c.drive(high_scl,routes.software_output(sda)!=Some(false));for (i,(pin,level)) in [(sda,high_sda),(scl,high_scl)].into_iter().enumerate() {if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}}
    }
    fn uses_gpio_pwm(&self)->bool {self.profile.light_pwm}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {if let Some((pin,active))=self.profile.light {let duty=pwm(pin).map(|(_,d)|if active {d.min(65535)}else{65535-d.min(65535)});if duty!=self.pwm {self.pwm=duty;self.light=duty.unwrap_or(self.gpio_light);self.version=self.version.wrapping_add(1);}}}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        let [clock,mosi,cs]=self.profile.spi;
        if !self.reset || !self.supplied() || self.cs==Some(true) || pins.sclk&(1u64<<clock)==0 || pins.mosi&(1u64<<mosi)==0 || pins.cs&(1u64<<cs)==0 {return vec![0xff;rx];}
        if self.profile.quad {self.quad(tx);}else if let Some(dc)=self.dc {if dc {self.data(tx);}else {for &c in tx {self.command(c);}}}else{return vec![0xff;rx];}
        self.transactions=self.transactions.wrapping_add(1);vec![0xff;rx]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();if let Some(w)=self.profile.touch {self.i2c.attach(w.address,Box::new(TouchDevice::new(w.kind,self.touch.clone())));}
        if self.profile.expander {self.i2c.attach(0x20,Box::new(Tca9555::new(self.expander.clone())));}
        let pins=self.i2c_pins();(0..2).flat_map(|bus|self.i2c.controller_devices(pins).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {let mut result:Vec<_>=self.profile.buttons.iter().map(|(_,p)|(*p,true)).collect();if let Some((a,b))=self.profile.encoder {result.extend([(a,true),(b,true)]);}
        if let Some(w)=self.profile.touch {result.push((w.irq,self.touch_level()));}
        if let Some((sda,scl))=self.i2c_pins() {result.extend([(sda,self.levels[0]),(scl,self.levels[1])]);}result}
    fn named_pin(&self,name:&str)->Option<u8> {self.profile.buttons.iter().find(|(n,_)|*n==name).map(|(_,p)|*p).or(match (name,self.profile.encoder) {("clk",Some((a,_)))=>Some(a),("dt",Some((_,b)))=>Some(b),_=>None})}
    fn encoder(&self)->Option<(u8,u8)> {self.profile.encoder}
    fn touch(&mut self,x:u16,y:u16,down:bool) {let Some(w)=self.profile.touch else{return;};let old=self.touch_level();let mut s=self.touch.lock().expect("LILYGO touch state mutex poisoned");if !s.powered||s.asleep {return;}s.x=x.min(s.width-1);s.y=y.min(s.height-1);s.down=down;drop(s);let level=self.touch_level();if level!=old {self.edges.push(BoardEdge{cycle:self.cycle,pin:w.irq,level});}}
    fn touch_irq(&self)->Option<(u8,bool)> {self.profile.touch.map(|w|(w.irq,self.touch_level()))}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let [x,y,w,h]=self.profile.glass;let mut frame=vec![0;w*h];
        if self.reset&&self.supplied()&&self.light!=0&&self.brightness!=0&&self.panel.on&&!self.panel.sleeping {for row in 0..h {frame[row*w..(row+1)*w].copy_from_slice(&self.panel.gram[(y+row)*self.panel.cols+x..(y+row)*self.panel.cols+x+w]);}}
        Some((w as u32,h as u32,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.version).wrapping_add(u64::from(self.supplied()))}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),self.panel.cols,self.panel.rows))}
    fn report(&self)->String {format!("[{}] {} display/control subset: spi={} pixels={} frames={} reset={} supply={} light={}; no PMIC/battery/audio/radio/SD/IMU/physical calibration; QSPI D1-D3 and TE timing unverified",self.profile.name,self.profile.controller,self.transactions,self.panel.pixels_written,self.panel.frames,self.reset,self.supplied(),self.light)}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn drive(b:&mut LilygoDisplay,extra:u64,low:u64) {let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();for p in 0..49 {mux.write(4+4*p,1<<12);}for p in [b.profile.reset,b.profile.enable,b.profile.dc,b.profile.light.map(|p|p.0),b.profile.touch.and_then(|w|w.reset)].into_iter().flatten() {gpio.enable|=1<<p;gpio.out|=1<<p;}gpio.enable|=extra;gpio.out|=extra;gpio.out&=!low;b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));}
    fn pins(b:&LilygoDisplay)->SpiPins {let [sclk,mosi,cs]=b.profile.spi;SpiPins{sclk:1<<sclk,mosi:1<<mosi,cs:1<<cs,..Default::default()}}
    fn cmd(b:&mut LilygoDisplay,c:u8,data:&[u8]) {let p=pins(b);if b.profile.quad {let mut bytes=vec![2,0,c,0];bytes.extend(data);b.spi_transfer_pins(2,p,&bytes,0);}else{b.dc=Some(false);b.spi_transfer_pins(2,p,&[c],0);b.dc=Some(true);b.spi_transfer_pins(2,p,data,0);}}
    fn setup(b:&mut LilygoDisplay) {drive(b,0,b.profile.light.filter(|p|!p.1).map_or(0,|p|1<<p.0));cmd(b,0x11,&[]);cmd(b,0x29,&[]);cmd(b,0x3a,&[0x55]);}
    fn pixel(b:&mut LilygoDisplay,x:u16,y:u16,color:u16) {let x=x.to_be_bytes();let y=y.to_be_bytes();cmd(b,0x2a,&[x[0],x[1],x[0],x[1]]);cmd(b,0x2b,&[y[0],y[1],y[0],y[1]]);cmd(b,0x2c,&color.to_be_bytes());}
    fn read(d:&mut dyn I2cDevice,register:&[u8],n:usize)->Vec<u8> {assert!(d.start(false));for &b in register {assert!(d.write(b));}assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    #[test]
    fn pro_external_exact_spi_window_buttons_and_wrong_routes() {
        let mut b=LilygoDisplay::new(&PRO_EXTERNAL);setup(&mut b);pixel(&mut b,49,0,0xf800);pixel(&mut b,270,479,0x07e0);
        let (w,h,pixels,_)=b.display().unwrap();assert_eq!((w,h),(222,480));assert_eq!(pixels[0],0xf800);assert_eq!(pixels[pixels.len()-1],0x07e0);
        let n=b.panel.pixels_written;let p=pins(&b);for route in [SpiPins{cs:1<<38,..p},SpiPins{sclk:1<<7,..p},SpiPins{mosi:1<<6,..p}] {b.spi_transfer_pins(2,route,&[0xff;8],0);}assert_eq!(b.panel.pixels_written,n);
        assert_eq!(b.named_pin("button-2"),Some(12));assert_eq!(b.named_pin("button-3"),Some(16));assert_eq!(b.named_pin("power"),None);
        cmd(&mut b,0x28,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));cmd(&mut b,0x29,&[]);drive(&mut b,0,1<<47);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
    }
    #[test]
    fn bar_active_low_backlight_crop_and_sleep_preserve_gram() {
        let mut b=LilygoDisplay::new(&BAR);setup(&mut b);pixel(&mut b,82,18,0x001f);pixel(&mut b,157,301,0xffff);let pixels=b.display().unwrap().2;assert_eq!((pixels[0],pixels[76*284-1]),(0x001f,0xffff));
        drive(&mut b,0,0);assert!(b.display().unwrap().2.iter().all(|p|*p==0));drive(&mut b,0,1<<15);assert_eq!(b.display().unwrap().2,pixels);cmd(&mut b,0x10,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));assert_eq!(b.panel.gram[18*240+82],0x001f);
    }
    #[test]
    fn quad_headers_continuation_reset_and_power_gates() {
        let mut b=LilygoDisplay::new(&AMOLED);setup(&mut b);pixel(&mut b,239,535,0xf81f);assert_eq!(b.display().unwrap().2[240*536-1],0xf81f);
        cmd(&mut b,0x51,&[0]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));cmd(&mut b,0x51,&[127]);assert_eq!(b.display().unwrap().2[240*536-1],0xf81f);
        let mut plus=LilygoDisplay::new(&AMOLED_PLUS);setup(&mut plus);pixel(&mut plus,0,0,0xffff);assert_eq!(plus.display().unwrap().2[0],0xffff);drive(&mut plus,0,1<<38);assert_eq!(plus.display().unwrap().2[0],0);
        let mut ultra=LilygoDisplay::new(&ULTRA);setup(&mut ultra);assert_eq!(ultra.transactions,0,"XL9555 supply must be explicitly enabled");let mut ds=ultra.i2c_devices();let d=ds[0].2.as_mut();assert!(d.start(false));d.write(6);d.write(0x7f);d.write(0xff);setup(&mut ultra);pixel(&mut ultra,22,0,0x07e0);assert_eq!(ultra.display().unwrap().2[0],0x07e0);
    }
    #[test]
    fn source_touch_protocols_pins_clamping_release_and_reset() {
        let mut b=LilygoDisplay::new(&PRO);setup(&mut b);let mut ds=b.i2c_devices();assert!(ds.iter().all(|(_,a,d)|*a==0x5a&&d.pins()==Some((5,6))));b.touch(221,479,true);assert_eq!(b.touch_irq(),Some((21,false)));let d=ds[0].2.as_mut();assert_eq!(read(d,&[0],7),[6,13,29,0xdf,0,1,0xab]);assert_eq!(read(d,&[0xd1,0xf8],4),[222,0,224,1]);b.touch(999,999,false);assert_eq!(read(d,&[0],7)[5],0);drive(&mut b,0,1<<13);assert!(!d.start(false));
        let mut bar=LilygoDisplay::new(&BAR);setup(&mut bar);let mut ds=bar.i2c_devices();bar.touch(999,999,true);assert_eq!(read(ds[0].2.as_mut(),&[2],5),[1,0,75,1,27]);bar.touch(0,0,false);assert_eq!(read(ds[0].2.as_mut(),&[2],1),[0]);
        let mut watch=LilygoDisplay::new(&WATCH_PLUS);let mut ds=watch.i2c_devices();watch.touch(123,234,true);assert_eq!(read(ds[0].2.as_mut(),&[2],5),[1,0x80,123,0,234]);assert!(ds.iter().all(|(_,a,d)|*a==0x38&&d.pins()==Some((39,40))));
    }
    #[test]
    fn every_profile_is_separate_and_does_not_invent_secondary_peripherals() {
        for profile in PROFILES {let mut b=make_board(profile.name).unwrap();assert_eq!(b.name(),profile.name);assert_eq!(b.named_pin("boot").is_some(),profile.name!="lilygo-t-watch-s3");assert!(!b.uses_gpio_waveform());assert!(b.sd_card_slot().is_none());assert!(b.lora_snapshot().is_none());assert!(b.leds().is_none());assert!(b.display().is_some());assert!(!b.set_imu_sample([0.0;3],[0.0;3]));}
        assert!(make_board("lilygo-unknown").is_none());
    }
}
