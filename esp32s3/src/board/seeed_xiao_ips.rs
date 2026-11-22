//! Seeed XIAO ESP32-S3 Plus IPS 1.47/1.14/0.96: routed SPI LCD, buttons and AXS5106L touch.
//! Seeed_GFX2 556df7cb and Display-Gadgets ba7f69f2 give the board wiring and
//! controller command profiles; see docs/platform/seeed-xiao-ips-runtime.md.
//! USB-powered digital subset. No IMU, microphone, SD, battery or sleep/wake model.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::board::{BoardModel,BoardEdge,SpiPins,VirtualCycle};
use esp_soc::devices::{DcsPanel,SpiBitBang};
use esp_soc::pins::PinRoutes;

#[derive(Clone,Copy,PartialEq)]
pub enum IpsSize {Touch147,Lcd114,Lcd096}
impl IpsSize {
    fn name(self)->&'static str {match self {Self::Touch147=>"seeed-xiao-ips-147",Self::Lcd114=>"seeed-xiao-ips-114",Self::Lcd096=>"seeed-xiao-ips-096"}}
    fn dimensions(self)->(usize,usize) {match self {Self::Touch147=>(172,320),Self::Lcd114=>(135,240),Self::Lcd096=>(80,160)}}
    fn buttons(self)->&'static [u8] {match self {Self::Touch147=>&[11,42],Self::Lcd114=>&[43,44,11],Self::Lcd096=>&[43,44]}}
}

struct TouchState {powered:bool,down:bool,x:u16,y:u16}
impl Default for TouchState {fn default()->Self {Self{powered:true,down:false,x:0,y:0}}}
struct Axs5106l {state:Arc<Mutex<TouchState>>,register:u8,select:bool,at:usize,report:[u8;14]}
impl Axs5106l {fn new(state:Arc<Mutex<TouchState>>)->Self {Self{state,register:0,select:false,at:0,report:[0;14]}}}
impl I2cDevice for Axs5106l {
    fn pins(&self)->Option<(u8,u8)> {Some((5,6))}
    fn start(&mut self,read:bool)->bool {
        let s=self.state.lock().expect("XIAO touch mutex poisoned");if !s.powered {return false;}
        self.at=0;self.select= !read;
        if read {self.report=[0;14];if s.down {self.report[1]=1;self.report[2]=(s.x>>8) as u8&15;self.report[3]=s.x as u8;self.report[4]=(s.y>>8) as u8&15;self.report[5]=s.y as u8;}}
        true
    }
    fn write(&mut self,b:u8)->bool {if !self.state.lock().expect("XIAO touch mutex poisoned").powered || !self.select {return false;}self.register=b;self.select=false;true}
    fn read(&mut self)->u8 {let value=if self.register==1 && self.state.lock().expect("XIAO touch mutex poisoned").powered {self.report.get(self.at).copied().unwrap_or(0xff)}else{0xff};self.at=self.at.saturating_add(1);value}
}

pub struct SeeedXiaoIps {
    size:IpsSize,panel:DcsPanel,software_spi:SpiBitBang,software_levels:[Option<bool>;3],dc_valid:bool,
    reset_high:bool,backlight_high:bool,pwm:u32,changes:u64,gpio_events:u64,spi_bytes:u64,
    touch:Arc<Mutex<TouchState>>,touch_irq:bool,cycle:VirtualCycle,edges:Vec<BoardEdge>,
}
impl SeeedXiaoIps {
    pub fn new(size:IpsSize)->Self {Self{size,panel:DcsPanel::st7789(),software_spi:SpiBitBang::new(7,9,3),software_levels:[None;3],dc_valid:false,reset_high:true,backlight_high:false,pwm:0,changes:0,gpio_events:0,spi_bytes:0,touch:Default::default(),touch_irq:true,cycle:0,edges:Vec::new()}}
    fn byte(&mut self,b:u8) {if !self.panel.dc {self.changes+=1;}self.panel.byte(b);self.spi_bytes+=1;}
    fn backlight(&self)->u32 {if self.backlight_high {65535}else{self.pwm}}
    fn set_irq(&mut self,level:bool) {if self.touch_irq!=level {self.touch_irq=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:44,level});}}
}
impl BoardModel for SeeedXiaoIps {
    fn name(&self)->&'static str {self.size.name()}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.cycle=cycle;self.gpio_events+=changes.len() as u64;}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;
        // R9 joins LCD_RST and CTP_RST; the optional R7 pull-up is DNP. Treat
        // an undriven reset as deasserted in this USB-powered digital profile,
        // inferred from the factory's I2C probe before it configures GPIO13.
        let reset=routes.software_output(13)!=Some(false);let backlight=routes.software_output(12)==Some(true);
        if self.reset_high && !reset {self.panel.reset();self.changes+=1;self.set_irq(true);}
        if self.backlight_high!=backlight {self.changes+=1;}
        self.reset_high=reset;self.backlight_high=backlight;
        {let mut touch=self.touch.lock().expect("XIAO touch mutex poisoned");touch.powered=reset;if !reset {touch.down=false;}}
        let dc=routes.software_output(4);self.dc_valid=dc.is_some();self.panel.dc=dc.unwrap_or(false);
        let levels=[routes.software_output(3),routes.software_output(9),routes.software_output(7)];
        if !reset || !self.dc_valid || levels.iter().any(Option::is_none) {
            self.software_spi=SpiBitBang::new(7,9,3);self.software_levels=[None;3];return;
        }
        if self.software_levels.iter().any(Option::is_none) {
            self.software_spi.pin(3,true);self.software_spi.pin(9,levels[1].unwrap_or(false));self.software_spi.pin(7,levels[2].unwrap_or(false));self.software_spi.pin(3,levels[0].unwrap_or(true));self.software_levels=levels;return;
        }
        // Sample CS and data before clock. Losing a software pin route discards
        // partial bytes, and establishing a route at a high clock is not an edge.
        for (i,pin) in [3,9,7].into_iter().enumerate() {
            if levels[i]!=self.software_levels[i] {
                let byte=self.software_spi.pin(pin,levels[i].unwrap_or(false));
                if let Some(b)=byte {self.byte(b);}
            }
        }
        self.software_levels=levels;
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {let duty=pwm(12).map_or(0,|(_,d)|d.min(65535));if self.pwm!=duty {self.pwm=duty;self.changes+=1;}}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if !self.reset_high || !self.dc_valid || pins.sclk&(1<<7)==0 || pins.mosi&(1<<9)==0 || pins.cs&(1<<3)==0 {return vec![0xff;rx_len];}
        for &b in tx {self.byte(b);}
        // The LCD is write-only on this PCB. SD MISO8 is not an LCD read line.
        vec![0xff;rx_len]
    }
    fn input_levels(&self)->Vec<(u8,bool)> {let mut levels=vec![(0,true),(5,true),(6,true)];levels.extend(self.size.buttons().iter().map(|&p|(p,true)));if self.size==IpsSize::Touch147 {levels.push((44,self.touch_irq));}levels}
    fn named_pin(&self,name:&str)->Option<u8> {let index=match name {"btnA"|"usr1"|"button1"|"button-a"=>0,"btnB"|"usr2"|"button2"|"button-b"=>1,"btnC"|"usr3"|"button3"|"button-c"=>2,"boot"=>return Some(0),_=>return None};self.size.buttons().get(index).copied()}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {if self.size!=IpsSize::Touch147 {return Vec::new();}(0..2).map(|bus|(bus,0x63,Box::new(Axs5106l::new(self.touch.clone())) as Box<dyn I2cDevice>)).collect()}
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        if self.size!=IpsSize::Touch147 || !self.reset_high {return;}
        // Seeed's native JD9853A orientation is MX. The matching official touch
        // driver mirrors raw X; expose the raw report, never firmware coordinates.
        {let mut s=self.touch.lock().expect("XIAO touch mutex poisoned");s.x=171-x.min(171);s.y=y.min(319);s.down=down;}
        // One falling-edge notification per host report, including movement.
        // The waveform/scan period is not calibrated; no synthetic gestures.
        if down {self.set_irq(true);}self.set_irq(!down);
    }
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.cycle=cycle;self.touch(x,y,down);}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let (w,h)=self.size.dimensions();let mut frame=vec![0;w*h];let duty=self.backlight();
        if self.reset_high && duty!=0 && self.panel.on && !self.panel.sleeping {
            for y in 0..h {for x in 0..w {
                // Fixed glass window/orientation. 0.96 is conservatively scoped
                // to the official rotation-2 profile; other scan windows are not
                // inferred from the inconsistent 24/0 offsets in its driver.
                let (col,row)=match self.size {IpsSize::Touch147=>(205-x,y),IpsSize::Lcd114=>(52+x,40+y),IpsSize::Lcd096=>(215-x,319-y)};
                let mut p=self.panel.gram[row*240+col];
                let native_bgr=self.size!=IpsSize::Lcd114;if (self.panel.madctl&8!=0)!=native_bgr {p=(p&0x07e0)|((p&0x1f)<<11)|(p>>11);}
                let native_invert=self.size==IpsSize::Lcd114;if self.panel.inverted!=native_invert {p= !p;}
                let p=p as u32;frame[y*w+x]=(((((p>>11)&31)*duty/65535)<<11)|((((p>>5)&63)*duty/65535)<<5)|((p&31)*duty/65535)) as u16;
            }}
        }
        Some((w as u32,h as u32,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.changes}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[{}] SPI bytes={} frames={} pixels={} madctl={:#x} backlight={} touch={} partial; IMU/PDM/SD/battery unmodeled",self.name(),self.spi_bytes,self.panel.frames,self.panel.pixels_written,self.panel.madctl,self.backlight(),if self.size==IpsSize::Touch147 {"AXS5106L-single-contact"}else{"absent"})}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<7,mosi:1<<9,cs:1<<3,..Default::default()}}
    fn drive(b:&mut SeeedXiaoIps,output:u64,enabled:u64) {let mut gpio=esp_periph::Gpio::new();gpio.enable=enabled;gpio.out=output;let mut mux=esp_periph::RegRam::new();for pin in [3,4,7,9,12,13] {mux.write(4+4*pin,1<<12);}b.gpio_waveform_at(1,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));}
    fn cmd(b:&mut SeeedXiaoIps,c:u8,data:&[u8]) {drive(b,(1<<12)|(1<<13),(1<<4)|(1<<12)|(1<<13));b.spi_transfer_pins(2,pins(),&[c],0);drive(b,(1<<4)|(1<<12)|(1<<13),(1<<4)|(1<<12)|(1<<13));b.spi_transfer_pins(2,pins(),data,0);}
    fn init(b:&mut SeeedXiaoIps) {cmd(b,0x11,&[]);cmd(b,0x29,&[]);cmd(b,0x3a,&[0x55]);let madctl=match b.size {IpsSize::Touch147=>0x48,IpsSize::Lcd114=>0,IpsSize::Lcd096=>0xc8};cmd(b,0x36,&[madctl]);cmd(b,if b.size==IpsSize::Lcd114 {0x21}else{0x20},&[]);}
    fn pixel(b:&mut SeeedXiaoIps,x:u16,y:u16,color:u16) {let x=x.to_be_bytes();let y=y.to_be_bytes();cmd(b,0x2a,&[x[0],x[1],x[0],x[1]]);cmd(b,0x2b,&[y[0],y[1],y[0],y[1]]);cmd(b,0x2c,&color.to_be_bytes());}
    #[test]
    fn official_native_profiles_draw_four_distinct_corners_on_each_glass() {
        for (size,offset) in [(IpsSize::Touch147,(34,0)),(IpsSize::Lcd114,(52,40)),(IpsSize::Lcd096,(24,0))] {
            let mut b=SeeedXiaoIps::new(size);init(&mut b);let (w,h)=size.dimensions();
            for (x,y,color) in [(0,0,0xf800),(w-1,0,0x07e0),(0,h-1,0x001f),(w-1,h-1,0xffff)] {pixel(&mut b,(x+offset.0) as u16,(y+offset.1) as u16,color);}
            let frame=b.display().unwrap();assert_eq!((frame.0,frame.1),(w as u32,h as u32));assert_eq!([frame.2[0],frame.2[w-1],frame.2[(h-1)*w],frame.2[w*h-1]],[0xf800,0x07e0,0x001f,0xffff],"{}",b.name());
            cmd(&mut b,0x28,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));cmd(&mut b,0x29,&[]);cmd(&mut b,0x10,&[]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        }
    }
    #[test]
    fn spi_requires_every_physical_route_and_gpio_reset_dc_and_backlight() {
        let mut b=SeeedXiaoIps::new(IpsSize::Touch147);drive(&mut b,(1<<12)|(1<<13),(1<<4)|(1<<12)|(1<<13));
        for p in [SpiPins{sclk:1<<8,..pins()},SpiPins{mosi:1<<8,..pins()},SpiPins{cs:1<<43,..pins()}] {b.spi_transfer_pins(2,p,&[0x11],0);assert!(b.panel.sleeping);}
        drive(&mut b,1<<13,1<<13);b.spi_transfer_pins(2,pins(),&[0x11],0);assert!(b.panel.sleeping,"DC is not driven");
        init(&mut b);pixel(&mut b,34,0,0xffff);assert_eq!(b.display().unwrap().2[0],0xffff);
        drive(&mut b,1<<13,(1<<4)|(1<<12)|(1<<13));assert_eq!(b.display().unwrap().2[0],0);
        b.gpio_pwm_at(2,&|p|if p==12 {Some((5000.0,32768))}else{None});assert_eq!(b.display().unwrap().2[0],0x7bef);
        drive(&mut b,0,(1<<4)|(1<<12)|(1<<13));b.spi_transfer_pins(2,pins(),&[0x11],0);assert!(b.panel.sleeping);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
    }
    #[test]
    fn software_spi_and_hardware_spi_reach_the_same_lcd_commands() {
        let mut b=SeeedXiaoIps::new(IpsSize::Touch147);let enabled=(1<<3)|(1<<4)|(1<<7)|(1<<9)|(1<<12)|(1<<13);let base=(1<<12)|(1<<13);
        drive(&mut b,base,enabled);for bit in (0..8).rev() {let value=base|if 0x11&(1<<bit)!=0 {1<<9}else{0};drive(&mut b,value,enabled);drive(&mut b,value|(1<<7),enabled);drive(&mut b,value,enabled);}
        assert!(!b.panel.sleeping);assert_eq!(b.spi_bytes,1);assert!(!b.uses_gpio_waveform(),"no instruction-sized scheduler for software SPI");
    }
    #[test]
    fn touch_reports_raw_mirrored_coordinates_release_and_reset_without_fake_sensors() {
        let mut b=SeeedXiaoIps::new(IpsSize::Touch147);let mut devices=b.i2c_devices();assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,addr,d)|*addr==0x63 && d.pins()==Some((5,6))));
        let d=devices[0].2.as_mut();assert!(d.start(false),"factory probes before configuring reset");drive(&mut b,0,1<<13);assert!(!d.start(false));init(&mut b);b.touch_at(100,20,301,true);assert!(b.input_levels().contains(&(44,false)));
        assert!(d.start(false));assert!(d.write(1));d.stop();assert!(d.start(true));let report=(0..14).map(|_|d.read()).collect::<Vec<_>>();assert_eq!(report,&[0,1,0,151,1,45,0,0,0,0,0,0,0,0]);assert_eq!(d.read(),0xff);
        b.take_edges();b.touch_at(200,21,302,true);assert_eq!(b.take_edges(),[BoardEdge{cycle:200,pin:44,level:true},BoardEdge{cycle:200,pin:44,level:false}]);
        b.touch(20,301,false);assert!(b.input_levels().contains(&(44,true)));d.start(false);d.write(1);d.start(true);assert_eq!((d.read(),d.read()),(0,0));
        b.touch(u16::MAX,u16::MAX,true);d.start(false);d.write(1);d.start(true);assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(),[0,1,0,0,1,63]);
        drive(&mut b,0,(1<<4)|(1<<12)|(1<<13));assert!(!d.start(false));assert!(b.input_levels().contains(&(44,true)));
        for size in [IpsSize::Lcd114,IpsSize::Lcd096] {assert!(SeeedXiaoIps::new(size).i2c_devices().is_empty());}
    }
    #[test]
    fn product_ids_select_distinct_button_maps_and_panels() {
        for (id,pins,dimensions) in [("seeed-xiao-ips-147",vec![11,42],(172,320)),("seeed-xiao-ips-114",vec![43,44,11],(135,240)),("seeed-xiao-ips-096",vec![43,44],(80,160))] {
            let b=super::super::make_board(id).expect("registered XIAO board");assert_eq!(b.name(),id);
            for controls in [["btnA","btnB","btnC"],["usr1","usr2","usr3"],["button-a","button-b","button-c"]] {for (i,control) in controls.into_iter().enumerate() {assert_eq!(b.named_pin(control),pins.get(i).copied());}}
            let frame=b.display().unwrap();assert_eq!((frame.0,frame.1),dimensions);assert!(pins.iter().all(|p|b.input_levels().contains(&(*p,true))));
        }
    }
}
