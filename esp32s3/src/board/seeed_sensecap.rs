//! SenseCAP Watcher W1-B digital display/input subset. Manufacturer BSP 8e37f7cc
//! and Espressif esp-iot-solution f721a40c define the SPD2010 wire protocol.
//! USB-powered profile with a gated nominal battery divider. HX6538/camera,
//! audio signal paths, battery charging/discharge dynamics and SD are absent.
//! Sources and exact limitations: docs/platform/seeed-sensecap-runtime.md.
use std::sync::{Arc,Mutex,atomic::{AtomicU64,Ordering}};
use esp_periph::i2c::{I2cDevice,tca9555::{Tca9555,Tca9555State}};
use esp_soc::{board::{BoardModel,BoardEdge,SpiPins,VirtualCycle},devices::DcsPanel,pins::PinRoutes};
use esp_soc::devices::bm8563::{Bm8563,Bm8563State};

// The shared calendar implements the PCF8563-compatible BCD/STOP/burst subset.
// NXP PCF8563 rev11.1 table27 differs from BM8563 at TF and CLKOUT: fix those
// power-up registers once. Undefined calendar bits use zero with VL set, not a
// made-up host date. Alarm, timer IRQ, CLKOUT waveform and backup analog absent.
fn rtc_state()->Arc<Mutex<Bm8563State>> {
    let state=Arc::new(Mutex::new(Bm8563State::new(crate::periph::CPU_HZ)));let mut chip=Bm8563::new(state.clone());
    for (reg,value) in [(1,0),(0x0d,0x80)] {chip.start(false);chip.write(reg);chip.write(value);chip.stop();}state
}
struct WatcherRtc {chip:Bm8563}
impl I2cDevice for WatcherRtc {
    fn pins(&self)->Option<(u8,u8)> {Some((47,48))}
    fn start(&mut self,read:bool)->bool {self.chip.start(read)}
    fn write(&mut self,b:u8)->bool {self.chip.write(b)}
    fn read(&mut self)->u8 {self.chip.read()}
    fn stop(&mut self) {self.chip.stop();}
}

#[derive(Clone)]
pub(super) struct Expander {
    pub state:Arc<Mutex<Tca9555State>>,pub version:Arc<AtomicU64>,pub reset:Arc<AtomicU64>,pub select_epoch:Arc<AtomicU64>,gate:u8,pins:(u8,u8),
}
impl Expander {
    pub fn new(gate:u8,pins:(u8,u8))->Self {Self{state:Default::default(),version:Default::default(),reset:Default::default(),select_epoch:Default::default(),gate,pins}}
    pub fn driven(&self,pin:u8)->bool {self.state.lock().expect("SenseCAP expander poisoned").driven_high()&(1<<pin)!=0}
    pub fn powered(&self)->bool {self.driven(self.gate)}
    pub fn version(&self)->u64 {self.version.load(Ordering::Relaxed)}
    pub fn reset_version(&self)->u64 {self.reset.load(Ordering::Relaxed)}
    pub fn device(&self)->Box<dyn I2cDevice> {Box::new(ExpanderDevice{chip:Tca9555::new(self.state.clone()),shared:self.clone()})}
}
struct ExpanderDevice {chip:Tca9555,shared:Expander}
impl I2cDevice for ExpanderDevice {
    fn pins(&self)->Option<(u8,u8)> {Some(self.shared.pins)}
    fn start(&mut self,read:bool)->bool {self.chip.start(read)}
    fn write(&mut self,b:u8)->bool {
        let old=self.shared.state.lock().expect("SenseCAP expander poisoned").driven_high();let ack=self.chip.write(b);
        let new=self.shared.state.lock().expect("SenseCAP expander poisoned").driven_high();
        if old!=new {self.shared.version.fetch_add(1,Ordering::Relaxed);}
        if old&16==0 && new&16!=0 {self.shared.select_epoch.fetch_add(1,Ordering::Relaxed);}
        if old&(1<<self.shared.gate)!=0 && new&(1<<self.shared.gate)==0 {self.shared.reset.fetch_add(1,Ordering::Relaxed);}
        ack
    }
    fn read(&mut self)->u8 {self.chip.read()}
}

// SPD2010 page FF/20/10 selects a register bank. Initialization banks reuse
// standard DCS opcodes: their 10h/11h/28h/29h must never change display state.
struct Spd2010 {panel:DcsPanel,page:u8,revision:u64,header:[u8;4],header_count:usize,command:u8,args:[u8;3],arg_count:usize,valid:bool,color:bool}
impl Spd2010 {
    fn new()->Self {Self{panel:DcsPanel::new(412,412),page:0,revision:0,header:[0;4],header_count:0,command:0,args:[0;3],arg_count:0,valid:true,color:false}}
    fn begin(&mut self) {self.header_count=0;self.arg_count=0;self.command=0;self.valid=true;self.color=false;}
    fn reset(&mut self) {self.panel.reset();self.page=0;self.revision+=1;self.begin();}
    fn stream(&mut self,mut data:&[u8],quad_routed:bool)->bool {
        if !self.valid {return false;}
        if self.header_count<4 {
            let n=data.len().min(4-self.header_count);self.header[self.header_count..self.header_count+n].copy_from_slice(&data[..n]);self.header_count+=n;data=&data[n..];
            if self.header_count<4 {return true;}
            self.valid=matches!(self.header[0],0x02|0x32)&&self.header[1]==0&&self.header[3]==0;if !self.valid{return false;}
            self.command=self.header[2];self.color=self.header[0]==0x32;
            if self.command!=0xff && self.page==0 {
                if self.command==1 {self.panel.reset();self.page=0;self.revision+=1;}
                else if matches!(self.command,0x10|0x11|0x20|0x21|0x28|0x29|0x2a|0x2b|0x2c|0x36|0x3a) {
                    self.panel.dc=false;self.panel.byte(self.command);self.panel.dc=true;self.revision+=1;
                }
            }
        }
        if self.command==0xff {
            for &b in data {if self.arg_count<3{self.args[self.arg_count]=b;self.arg_count+=1;}}
            if self.arg_count==3&&self.args[..2]==[0x20,0x10]{self.page=self.args[2];}return true;
        }
        if self.page!=0 {return true;}
        if self.command==0x2c && (self.panel.colmod&7!=5 || (self.color&&!quad_routed)) {return data.is_empty();}
        if !matches!(self.command,0x2a|0x2b|0x2c|0x36|0x3a) {return true;}
        for &byte in data {
            if !matches!(self.command,0x36|0x3a)||self.arg_count==0 {
                // SPD2010 MX/MY are bits1/0; the common DCS walker uses6/7.
                self.panel.byte(if self.command==0x36 {(byte&8)|((byte&2)<<5)|((byte&1)<<7)}else{byte});
            }
            self.arg_count=self.arg_count.saturating_add(1);
        }
        self.revision+=1;true
    }
}

#[derive(Default)]
struct Contact {down:bool,x:u16,y:u16}
struct WatcherTouch {io:Expander,contact:Arc<Mutex<Contact>>,register:u16,count:u8,value:u16,at:usize,phase:u8,epoch:u64,report:[u8;18]}
impl WatcherTouch {
    fn new(io:Expander,contact:Arc<Mutex<Contact>>)->Self {Self{io,contact,register:0,count:0,value:0,at:0,phase:0,epoch:0,report:[0;18]}}
}

/// ESP codec-dev initialization register readback only. The official BSP uses
/// ES8311 8-bit address30h (7-bit18h) and an ES7243E fallback at14h. No identity,
/// PLL lock, ADC samples or acoustic output is fabricated; unwritten reads FFh.
/// Source: esp-adf 8d74459b components/esp_codec_dev/{platform,device}.
struct CodecRegisters {regs:[Option<u8>;256],ptr:u8,first:bool,adc:bool}
impl CodecRegisters {fn new(adc:bool)->Self {Self{regs:[None;256],ptr:0,first:true,adc}}}
impl I2cDevice for CodecRegisters {
    fn pins(&self)->Option<(u8,u8)> {Some((47,48))}
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}true}
    fn write(&mut self,b:u8)->bool {if self.first{self.ptr=b;self.first=false;}else{if if self.adc {self.ptr<=0x21||matches!(self.ptr,0xf7|0xf9)}else{self.ptr<=0x45}{self.regs[usize::from(self.ptr)]=Some(b);}self.ptr=self.ptr.wrapping_add(1);}true}
    fn read(&mut self)->u8 {let value=self.regs[usize::from(self.ptr)].unwrap_or(0xff);self.ptr=self.ptr.wrapping_add(1);value}
}
impl I2cDevice for WatcherTouch {
    fn pins(&self)->Option<(u8,u8)> {Some((39,38))}
    fn start(&mut self,read:bool)->bool {
        if !self.io.powered() {return false;}
        let epoch=self.io.reset_version();if epoch!=self.epoch {self.phase=0;self.epoch=epoch;self.contact.lock().expect("Watcher touch poisoned").down=false;}
        self.at=0;
        if !read {self.count=0;self.value=0;return true;}
        let s=self.contact.lock().expect("Watcher touch poisoned");self.report.fill(0);
        match self.register {
            0x0020=>{self.report[1]=match self.phase {0=>0x40,1=>0x20,_=>8};if self.phase==2 && s.down {self.report[0]=1;self.report[2]=10;}},
            0x0300 if self.phase==2 && s.down=>{self.report[5]=s.x as u8;self.report[6]=s.y as u8;self.report[7]=(((s.x>>8) as u8)<<4)|((s.y>>8) as u8&15);self.report[8]=1;},
            0x02fc=>self.report[5]=0x82,
            // Firmware revision/PID remain unknown (zero), only the chip name is identified.
            0x0026=>self.report[10..18].copy_from_slice(b"0102DPS\0"),
            _=>{}
        }
        true
    }
    fn write(&mut self,b:u8)->bool {
        if !self.io.powered() {return false;}
        match self.count {0=>self.register=u16::from(b),1=>self.register|=u16::from(b)<<8,2=>self.value=u16::from(b),3=>{
            self.value|=u16::from(b)<<8;
            match (self.register,self.value) { (4,1)=>self.phase=1,(0x46,0) if self.phase==1=>self.phase=2,(2,1)=>self.io.state.lock().expect("SenseCAP expander poisoned").set_input(5,true),_=>{} }
        },_=>{}}
        self.count=self.count.saturating_add(1);true
    }
    fn read(&mut self)->u8 {let b=if self.io.powered(){self.report.get(self.at).copied().unwrap_or(0xff)}else{0xff};self.at=self.at.saturating_add(1);b}
}


pub struct SeeedWatcher {
    io:Expander,panel:Spd2010,contact:Arc<Mutex<Contact>>,reset_seen:u64,quad_routes:[bool;2],backlight:bool,pwm:u32,
    changes:u64,events:u64,spi_bytes:u64,rejected:u64,cycle:VirtualCycle,irq:bool,edges:Vec<BoardEdge>,selected:bool,software_cs:bool,spi_host:u8,
    rtc:Arc<Mutex<Bm8563State>>,
    battery:esp_periph::analog::AnalogStream,battery_enabled:bool,
}
impl Default for SeeedWatcher {fn default()->Self {Self::new()}}
impl SeeedWatcher {
    pub fn new()->Self {Self{io:Expander::new(9,(47,48)),panel:Spd2010::new(),contact:Default::default(),reset_seen:0,quad_routes:[false;2],backlight:false,pwm:0,changes:0,events:0,spi_bytes:0,rejected:0,cycle:0,irq:true,edges:Vec::new(),selected:false,software_cs:false,spi_host:0,rtc:rtc_state(),battery:esp_periph::analog::AnalogStream::new(1,0.0,0).expect("constant battery source"),battery_enabled:false}}
    fn sync(&mut self) {
        let epoch=self.io.reset_version();if epoch!=self.reset_seen {self.panel.reset();self.reset_seen=epoch;self.selected=false;}
        let on=self.io.driven(15);if on!=self.battery_enabled {self.battery.replace_current(if on{0.98}else{0.0}).expect("constant battery pad voltage");self.battery_enabled=on;}
    }
    fn visible(&self)->bool {self.io.powered() && self.reset_seen==self.io.reset_version() && (self.backlight || self.pwm>0) && self.panel.panel.on && !self.panel.panel.sleeping}
    fn update_irq(&mut self) {let high=self.io.state.lock().expect("SenseCAP expander poisoned").irq_high();if high!=self.irq {self.irq=high;self.edges.push(BoardEdge{cycle:self.cycle,pin:2,level:high});}}
}
impl BoardModel for SeeedWatcher {
    fn name(&self)->&'static str {"seeed-sensecap-watcher"}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;self.sync();let light=routes.software_output(8)==Some(true);if light!=self.backlight {self.backlight=light;self.changes+=1;}
        let software_cs=routes.software_output(45)==Some(false);if self.software_cs&&!software_cs {self.selected=false;}self.software_cs=software_cs;
        // IDF v5.2.1 gpio_sig_map.h: SPI3 Q/D/HD/WP=67/68/69/70,
        // FSPI Q/D/HD/WP=102/103/104/105. IO2=WP, IO3=HD.
        self.quad_routes=[routes.matrix_output(1,102)&&routes.matrix_output(14,105)&&routes.matrix_output(13,104),routes.matrix_output(1,67)&&routes.matrix_output(14,70)&&routes.matrix_output(13,69)];
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.cycle=cycle;self.events+=changes.len() as u64;}
    fn gpio_events(&self)->u64 {self.events}
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {let duty=pwm(8).map_or(0,|(_,d)|d.min(65535));if duty!=self.pwm {self.pwm=duty;self.changes+=1;}}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.sync();
        let valid=matches!(host,2|3)&&self.io.powered()&&pins.cs&(1<<45)!=0&&pins.sclk&(1<<7)!=0&&pins.mosi&(1<<9)!=0;
        // CS continuity, not a pixel-byte heuristic, identifies a split header
        // and data. A data chunk starting 02 00 2C 00 remains pixel data.
        if !self.selected||self.spi_host!=host {self.panel.begin();}self.spi_host=host;
        if valid&&self.panel.stream(tx,self.quad_routes[usize::from(host==3)]) {self.spi_bytes+=tx.len() as u64;}else{self.rejected+=1;}
        self.selected=valid&&(pins.keep_cs||self.software_cs);
        vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        (0..2).flat_map(|bus|[(bus,0x21,self.io.device()),(bus,0x53,Box::new(WatcherTouch::new(self.io.clone(),self.contact.clone())) as Box<dyn I2cDevice>),(bus,0x18,Box::new(CodecRegisters::new(false)) as Box<dyn I2cDevice>),(bus,0x14,Box::new(CodecRegisters::new(true)) as Box<dyn I2cDevice>),(bus,0x51,Box::new(WatcherRtc{chip:Bm8563::new(self.rtc.clone())}) as Box<dyn I2cDevice>)]).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(2,self.irq),(41,true),(42,true),(47,true),(48,true),(39,true),(38,true)]}
    fn default_analog_inputs(&self)->Vec<(u8,esp_periph::AnalogSource)> {
        // Official BSP 8e37f7cc: ADC1_CH2(GPIO3), divider82/20, power P1.7.
        // Nominal ~4.0V battery gives0.98V at the pad only while its gate is
        // driven high. No discharge/charger simulation; explicit host overrides
        // keep precedence through AnalogInputs::set_default.
        vec![(3,esp_periph::AnalogSource::Stream(self.battery.clone()))]
    }
    fn encoder(&self)->Option<(u8,u8)> {Some((41,42))}
    // espressif/knob 1.0.1 at esp-iot-solution 63e9a518, Kconfig and
    // iot_knob.c: default 3ms polling and two debounce samples. A 2ms
    // host phase can disappear between samples. 20ms is our slow-turn
    // stimulus, not a claimed physical detent period or a firmware patch.
    fn encoder_phase_ms(&self)->u8 {20}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"=>Some(0),_=>None}}
    fn button_at(&mut self,cycle:VirtualCycle,id:&str,pressed:bool)->bool {
        if id!="knob" {return false;}self.cycle=cycle;self.io.state.lock().expect("SenseCAP expander poisoned").set_input(3,!pressed);self.update_irq();true
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {if !self.io.powered(){return;}*self.contact.lock().expect("Watcher touch poisoned")=Contact{down,x:x.min(411),y:y.min(411)};self.io.state.lock().expect("SenseCAP expander poisoned").set_input(5,!down);self.update_irq();}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.cycle=cycle;self.touch(x,y,down);}
    fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;self.sync();self.update_irq();self.rtc.lock().expect("Watcher RTC poisoned").advance_to(cycle);}
    fn take_edges(&mut self)->Vec<BoardEdge> {self.update_irq();std::mem::take(&mut self.edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((412,412,if self.visible(){self.panel.panel.gram.clone()}else{vec![0;412*412]},self.display_version()))}
    fn display_version(&self)->u64 {self.panel.panel.pixels_written+self.panel.revision+self.changes+self.io.version()}
    fn display_frames(&self)->u64 {self.panel.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.panel.gram.clone(),412,412))}
    fn report(&self)->String {format!("[seeed-sensecap-watcher] SPD2010 pixels={} frames={} bytes={} rejected={} powered={} visible={} encoder=A41/B42 press=P03; PCF8563=calendar-only codec=registers-only camera/audio/SD absent",self.panel.panel.pixels_written,self.panel.panel.frames,self.spi_bytes,self.rejected,self.io.powered(),self.visible())}
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_soc::pins::ChipPins;
    fn write(d:&mut dyn I2cDevice,r:u8,values:&[u8]) {assert!(d.start(false));assert!(d.write(r));for &v in values {assert!(d.write(v));}}
    fn read(d:&mut dyn I2cDevice,r:u8,n:usize)->Vec<u8> {write(d,r,&[]);assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    fn power(b:&mut SeeedWatcher,on:bool) {let mut d=b.io.device();write(d.as_mut(),2,&[0,if on {2}else{0}]);write(d.as_mut(),6,&[255,0xfd]);}
    fn routes(b:&mut SeeedWatcher,bad:Option<u8>,backlight:bool) {
        let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
        for (pin,signal) in [(1,67),(14,70),(13,69),(8,256)] {mux.write(4+4*pin,1<<12);gpio.func_out_sel[pin as usize]=if bad==Some(pin as u8){256}else{signal};gpio.enable|=1<<pin;}
        if backlight {gpio.out|=1<<8;}b.gpio_waveform_at(0,ChipPins::S3.routes(&gpio,&mux));
    }
    fn pins()->SpiPins {SpiPins{sclk:1<<7,mosi:1<<9,cs:1<<45,..Default::default()}}
    fn command(b:&mut SeeedWatcher,c:u8,data:&[u8]) {let mut tx=vec![if c==0x2c{0x32}else{0x02},0,c,0];tx.extend(data);b.spi_transfer_pins(3,pins(),&tx,0);}
    fn pixel(b:&mut SeeedWatcher,color:u16) {command(b,0x2a,&[0,0,0,0]);command(b,0x2b,&[0,0,0,0]);command(b,0x2c,&color.to_be_bytes());}
    #[test]
    fn watcher_display_requires_power_physical_select_quad_lanes_and_backlight() {
        let mut b=SeeedWatcher::new();routes(&mut b,None,true);command(&mut b,0x3a,&[0x55]);pixel(&mut b,0xf800);assert_eq!(b.panel.panel.pixels_written,0);
        power(&mut b,true);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);command(&mut b,0x3a,&[0x55]);pixel(&mut b,0xf800);assert_eq!(b.display().unwrap().2[0],0xf800);
        for pin in [1,13,14] {routes(&mut b,Some(pin),true);pixel(&mut b,0x1f);assert_eq!(b.panel.panel.gram[0],0xf800);}
        routes(&mut b,None,true);
        for broken in [SpiPins{sclk:1<<6,..pins()},SpiPins{mosi:1<<8,..pins()},SpiPins{cs:0,..pins()}] {b.spi_transfer_pins(3,broken,&[0x32,0,0x2c,0,0,0x1f],0);assert_eq!(b.panel.panel.gram[0],0xf800);}
        routes(&mut b,None,false);assert_eq!(b.display().unwrap().2[0],0);assert_eq!(b.gram().unwrap().0[0],0xf800);
        b.gpio_pwm_at(0,&|p|if p==8{Some((5000.,32768))}else{None});assert_eq!(b.display().unwrap().2[0],0xf800);
        power(&mut b,false);assert_eq!(b.display().unwrap().2[0],0);power(&mut b,true);b.advance_to(1);assert!(!b.visible(),"power cycle must clear display/sleep commands");
    }
    #[test]
    fn watcher_vendor_banks_and_formats_cannot_impersonate_dcs_commands() {
        let mut b=SeeedWatcher::new();power(&mut b,true);routes(&mut b,None,true);
        command(&mut b,0xff,&[0x20,0x10,0x11]);command(&mut b,0x11,&[0]);command(&mut b,0x29,&[0]);assert!(b.panel.panel.sleeping);assert!(!b.panel.panel.on);
        command(&mut b,0xff,&[0x20,0x10,0]);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);pixel(&mut b,0xf800);assert_eq!(b.panel.panel.pixels_written,0,"RGB666 is outside the declared subset");
        command(&mut b,0x3a,&[0x55]);command(&mut b,0x36,&[3]);pixel(&mut b,0xf800);assert_eq!(b.panel.panel.gram[412*412-1],0xf800);
        command(&mut b,1,&[]);assert!(b.panel.panel.sleeping);assert!(!b.panel.panel.on);
    }
    #[test]
    fn watcher_kept_cs_stream_preserves_split_headers_and_header_like_pixels() {
        let mut b=SeeedWatcher::new();power(&mut b,true);routes(&mut b,None,true);
        let held=SpiPins{keep_cs:true,..pins()};
        b.spi_transfer_pins(3,held,&[2,0],0);b.spi_transfer_pins(3,held,&[0x3a,0],0);b.spi_transfer_pins(3,pins(),&[0x55],0);assert_eq!(b.panel.panel.colmod,0x55);
        command(&mut b,0x2a,&[0,0,0,1]);command(&mut b,0x2b,&[0,0,0,0]);
        b.spi_transfer_pins(3,held,&[0x32,0,0x2c,0],0);b.spi_transfer_pins(3,pins(),&[2,0,0x2c,0],0);
        assert_eq!(&b.panel.panel.gram[..2],&[0x0200,0x2c00]);
        // New hardware CS discards a partial command rather than borrowing the
        // next transaction's bytes. The missing parameter remains unapplied.
        b.spi_transfer_pins(3,pins(),&[2,0,0x3a,0],0);b.spi_transfer_pins(3,pins(),&[0x66],0);assert_eq!(b.panel.panel.colmod,0x55);
        b.spi_transfer_pins(3,held,&[2,0,0xff,0],0);b.spi_transfer_pins(3,pins(),&[0x20,0x10,0x11],0);assert_eq!(b.panel.page,0x11);
    }
    fn touch_write(d:&mut dyn I2cDevice,bytes:&[u8]) {assert!(d.start(false));for &b in bytes{assert!(d.write(b));}}
    fn touch_read(d:&mut dyn I2cDevice,register:u16,n:usize)->Vec<u8> {touch_write(d,&register.to_le_bytes());assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    #[test]
    fn watcher_touch_cpu_handshake_coordinates_release_and_expander_knob_are_physical() {
        let mut b=SeeedWatcher::new();let mut d=WatcherTouch::new(b.io.clone(),b.contact.clone());assert_eq!(d.pins(),Some((39,38)));assert!(!d.start(false));power(&mut b,true);
        assert_eq!(touch_read(&mut d,0x20,4),[0,0x40,0,0]);touch_write(&mut d,&[4,0,1,0]);assert_eq!(touch_read(&mut d,0x20,4),[0,0x20,0,0]);touch_write(&mut d,&[0x50,0,0,0]);touch_write(&mut d,&[0x46,0,0,0]);
        b.touch_at(100,400,300,true);assert_eq!(touch_read(&mut d,0x20,4),[1,8,10,0]);assert_eq!(touch_read(&mut d,0x300,10),[0,0,0,0,0,144,44,0x11,1,0]);assert_eq!(touch_read(&mut d,0x2fc,8)[5],0x82);
        touch_write(&mut d,&[2,0,1,0]);assert!(b.io.state.lock().unwrap().levels()&(1<<5)!=0);
        b.touch(400,300,false);assert_eq!(touch_read(&mut d,0x20,4),[0,8,0,0]);
        let mut exp=b.io.device();assert_eq!(exp.pins(),Some((47,48)));assert!(b.button_at(200,"knob",true));assert!(b.take_edges().iter().any(|e|e.pin==2&&!e.level));assert_eq!(read(exp.as_mut(),0,1)[0]&8,0);
        assert!(b.button_at(201,"knob",false));assert_eq!(read(exp.as_mut(),0,1)[0]&8,8);assert_eq!(b.named_pin("knob"),None);assert_eq!(b.encoder(),Some((41,42)));
        power(&mut b,false);assert!(!d.start(true));assert!(!b.button_at(202,"invalid",true));
    }
    #[test]
    fn watcher_rtc_uses_shared_guest_time_and_pcf_defaults_without_fabricated_codec_ids() {
        let mut b=SeeedWatcher::new();let mut devices=b.i2c_devices();
        let rtc0=devices.iter().position(|(bus,a,_)|*bus==0&&*a==0x51).unwrap();
        let rtc1=devices.iter().position(|(bus,a,_)|*bus==1&&*a==0x51).unwrap();
        assert_eq!(devices[rtc0].2.pins(),Some((47,48)));
        assert_eq!(read(devices[rtc0].2.as_mut(),0,3),[8,0,0x80]);devices[rtc0].2.stop();
        assert_eq!(read(devices[rtc1].2.as_mut(),0x0d,2),[0x80,3]);devices[rtc1].2.stop();
        write(devices[rtc0].2.as_mut(),2,&[0x59,0x59,0x23,0x28,3,2,0x24]);devices[rtc0].2.stop();
        b.advance_to(crate::periph::CPU_HZ);assert_eq!(read(devices[rtc1].2.as_mut(),2,7),[0,0,0,0x29,4,2,0x24]);devices[rtc1].2.stop();
        let codec=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x18).unwrap().2.as_mut();assert_eq!(codec.pins(),Some((47,48)));
        assert_eq!(read(codec,0xfd,3),[0xff;3]);write(codec,1,&[0x3f]);assert_eq!(read(codec,1,1),[0x3f]);
        assert!(!devices.iter().any(|(_,a,_)|*a==0x30),"BSP's codec-dev address30h is eight-bit, not an extra device");
    }
    #[test]
    fn watcher_nominal_battery_needs_its_expander_output_gate_and_preserves_host_override() {
        let mut b=SeeedWatcher::new();let mut pads=esp_periph::AnalogInputs::new(crate::periph::CPU_HZ);
        for (pin,source) in b.default_analog_inputs(){pads.set_default(pin,source);}assert_eq!(pads.volts(3,0),0.0);
        let mut io=b.io.device();write(io.as_mut(),2,&[255,255]);b.advance_to(1);assert_eq!(pads.volts(3,1),0.0,"input pull-up is not divider power");
        write(io.as_mut(),6,&[255,0x7f]);b.advance_to(2);assert_eq!(pads.volts(3,2),0.98);assert_eq!(pads.volts(2,2),0.0);
        write(io.as_mut(),3,&[0x7f]);b.advance_to(3);assert_eq!(pads.volts(3,3),0.0);
        pads.set(3,esp_periph::AnalogSource::Const(0.8));write(io.as_mut(),3,&[255]);b.advance_to(4);
        for (pin,source) in b.default_analog_inputs(){pads.set_default(pin,source);}assert_eq!(pads.volts(3,4),0.8);
    }
}
