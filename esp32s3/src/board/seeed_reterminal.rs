//! Seeed reTerminal digital subsets. Wiring/protocol sources are fixed in
//! docs/platform/seeed-onboarding.md. Seeed_GFX2 556df7cb0b4a097d9b81026a26fd5511f9c934d6:
//! reTerminal_ePaper_Board_Configs.h, Driver_{SSD1677,UC8179,ED2208}.cpp.
//! Sticky selects the SSD1677 production variant; SSD2677 is not emulated.
//! E1001 is monochrome, E1002 has ideal six-color pixels. No optical calibration.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,gt911::{Gt911,Gt911State}};
use esp_periph::sdmmc::{SdStore,SdCardConfig,card::spi::SpiSdCard};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{ssd1677::Ssd1677,uc8179::Uc8179,ed2208::Ed2208,t133a01::T133a01,it8951::It8951,i2c_bitbang::I2cBitBang};

#[derive(Clone,Copy,PartialEq)]
pub enum Variant {Sticky,E1001,E1002,E1003,E1004}
enum Panel {Sticky(Ssd1677),Mono(Uc8179),Color(Ed2208),Large(T133a01),Tcon(It8951)}
impl Panel {
    fn reset(&mut self){match self{Self::Sticky(p)=>p.reset(),Self::Mono(p)=>p.reset(),Self::Color(p)=>p.reset(),Self::Large(p)=>p.reset(),Self::Tcon(p)=>p.reset()}}
    fn write(&mut self,selected:u8,d:bool,b:&[u8]){match self{Self::Sticky(p)=>p.write(d,b),Self::Mono(p)=>p.write(d,b),Self::Color(p)=>p.write(d,b),Self::Large(p)=>p.write(selected,d,b),Self::Tcon(p)=>{p.transfer(b,0);}}}
    fn advance_to(&mut self,c:u64){match self{Self::Sticky(p)=>p.advance_to(c),Self::Mono(p)=>p.advance_to(c),Self::Color(p)=>p.advance_to(c),Self::Large(p)=>p.advance_to(c),Self::Tcon(p)=>p.advance_to(c)}}
    fn busy_high(&self)->bool{match self{Self::Sticky(p)=>p.busy(),Self::Mono(p)=>p.busy_high(),Self::Color(p)=>p.busy_high(),Self::Large(p)=>p.busy_high(),Self::Tcon(p)=>p.busy_high()}}
    fn next_deadline(&self)->Option<u64>{match self{Self::Sticky(p)=>p.next_deadline(),Self::Mono(p)=>p.next_deadline(),Self::Color(p)=>p.next_deadline(),Self::Large(p)=>p.next_deadline(),Self::Tcon(p)=>p.next_deadline()}}
    fn edges(&mut self)->Vec<BoardEdge>{match self{Self::Sticky(p)=>p.take_edges(),Self::Mono(p)=>p.take_edges(),Self::Color(p)=>p.take_edges(),Self::Large(p)=>p.take_edges(),Self::Tcon(p)=>p.take_edges()}}
    fn frame(&self)->(&[u16],u64,u64,u64){match self{Self::Sticky(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Mono(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Color(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Large(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Tcon(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written)}}
}
// GT911 samples INT at reset release to choose 0x14/0x5d. Only that endpoint
// acknowledges; registering two adapters does not make both addresses valid.
struct StrappedTouch {device:Gt911,address:u8,selected:Arc<Mutex<u8>>}
impl I2cDevice for StrappedTouch {
    fn start(&mut self,r:bool)->bool {*self.selected.lock().expect("Seeed touch address poisoned")==self.address&&self.device.start(r)}
    fn write(&mut self,b:u8)->bool {self.device.write(b)}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self){self.device.stop();}
}
pub struct SeeedReTerminal {
    variant:Variant,panel:Panel,output:u64,enabled:u64,cycle:u64,active:bool,
    reset_high:bool,dc:bool,gpio_events:u64,edges:Vec<BoardEdge>,
    i2c:I2cBitBang,levels:[bool;2],touch:Arc<Mutex<Gt911State>>,touch_address:Arc<Mutex<u8>>,touch_reset:bool,irq:bool,
    sd:Option<Box<SpiSdCard>>,sd_power:bool,
    battery:esp_periph::analog::AnalogStream,battery_enabled:bool,
}
impl SeeedReTerminal {
    pub fn new(variant:Variant)->Self {
        let ms=crate::periph::CPU_HZ/1000;
        let panel=match variant {Variant::Sticky=>Panel::Sticky(Ssd1677::new(800,480,18,ms)),Variant::E1001=>Panel::Mono(Uc8179::new(13,ms)),Variant::E1002=>Panel::Color(Ed2208::e1002(13,ms)),Variant::E1004=>Panel::Large(T133a01::new(13,ms)),Variant::E1003=>Panel::Tcon(It8951::new(13,ms))};
        let touch=Arc::new(Mutex::new(Gt911State::new(480,800,crate::periph::CPU_HZ/200)));touch.lock().expect("Seeed GT911 poisoned").powered=false;
        Self{variant,panel,output:0,enabled:0,cycle:0,active:matches!(variant,Variant::E1001|Variant::E1002),reset_high:true,dc:false,gpio_events:0,edges:Vec::new(),i2c:I2cBitBang::new(),levels:[true;2],touch,touch_address:Arc::new(Mutex::new(0x5d)),touch_reset:true,irq:true,sd:None,sd_power:false,battery:esp_periph::analog::AnalogStream::new(1,0.0,0).expect("constant battery source"),battery_enabled:false}
    }
    fn sticky(&self)->bool {self.variant==Variant::Sticky}
    fn large(&self)->bool {self.variant==Variant::E1004}
    fn tcon(&self)->bool {self.variant==Variant::E1003}
    fn high(&self,p:u8)->bool {self.enabled&(1u64<<p)==0||self.output&(1u64<<p)!=0}
    fn driven_high(&self,p:u8)->bool {self.enabled&self.output&(1u64<<p)!=0}
    fn sd_pins(&self)->(u8,u8,u8) {if self.sticky(){(8,10,11)}else if self.tcon(){(14,39,15)}else{(14,16,15)}}
    fn bus_pins(&self)->(u8,u8,u8,u8) {if self.sticky(){(13,14,12,15)}else{(7,9,8,10)}}
    fn sync_touch(&mut self) {
        if !self.sticky(){return;}
        let powered=self.driven_high(42)&&self.high(41);let mut t=self.touch.lock().expect("Seeed GT911 poisoned");
        if !powered {t.ready=false;t.down=false;}t.powered=powered;t.advance_to(self.cycle);let level=!t.ready;
        if level!=self.irq {self.irq=level;self.edges.push(BoardEdge{cycle:self.cycle,pin:21,level});}
    }
}
impl BoardModel for SeeedReTerminal {
    fn name(&self)->&'static str {match self.variant{Variant::Sticky=>"seeed-reterminal-sticky",Variant::E1001=>"seeed-reterminal-e1001",Variant::E1002=>"seeed-reterminal-e1002",Variant::E1004=>"seeed-reterminal-e1004",Variant::E1003=>"seeed-reterminal-e1003"}}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.cycle=cycle;self.output=output;self.enabled=enabled;self.gpio_events+=changes.len() as u64;
        if matches!(self.variant,Variant::E1001|Variant::E1002|Variant::E1004) {
            let on=self.driven_high(21);if on!=self.battery_enabled{self.battery.replace_current(if on{2.0}else{0.0}).expect("constant battery voltage");self.battery_enabled=on;}
        }
        let reset=self.high(if self.sticky(){17}else if self.large(){38}else{12});let active=reset&&if self.sticky(){self.driven_high(47)}else if self.large(){self.driven_high(12)}else if self.tcon(){self.driven_high(11)&&self.driven_high(21)}else{true};
        if self.active&&!active||self.reset_high&&!reset {self.panel.reset();}self.reset_high=reset;self.active=active;self.dc=self.driven_high(if self.sticky(){16}else{11});
        if self.driven_high(10){if let Panel::Tcon(p)=&mut self.panel{p.end_transaction();}}
        let (cs,en,_)=self.sd_pins();let powered=self.driven_high(en);
        if let Some(sd)=&mut self.sd {if self.sd_power&&!powered {sd.power_cycle();}
            if output&(1u64<<cs)!=0 {sd.deselect();}}self.sd_power=powered;
        if self.sticky() {
            let reset=self.high(41);if !self.touch_reset&&reset {*self.touch_address.lock().expect("Seeed touch address poisoned")=if self.driven_high(21){0x14}else{0x5d};}self.touch_reset=reset;
            self.sync_touch();let scl=self.high(2);let sda=self.i2c.drive(scl,self.high(3));
            for (i,(pin,level)) in [(3,sda),(2,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        }
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        let (sck,mosi,miso,cs)=self.bus_pins();if pins.sclk&(1u64<<sck)==0||pins.mosi&(1u64<<mosi)==0{return vec![255;rx_len];}
        let (sd_cs,_,_)=self.sd_pins();let selected=u8::from(pins.cs&(1u64<<cs)!=0)|if self.large()&&pins.cs&(1<<2)!=0{2}else{0};let panel_selected=selected!=0;let sd_selected=pins.cs&(1u64<<sd_cs)!=0;
        // Reject a real bus collision instead of delivering the same bytes twice.
        if panel_selected&&sd_selected&&self.sd_power{return vec![255;rx_len];}
        if panel_selected&&self.active {
            if let Panel::Tcon(p)=&mut self.panel{let r=p.transfer(tx,rx_len);return if pins.miso==Some(miso){r}else{vec![255;rx_len]};}
            self.panel.write(selected,self.dc,tx);
        }
        if self.sd_power {if let Some(sd)=&mut self.sd {let r=sd.transfer(sd_selected,tx,rx_len);if sd_selected&&pins.miso==Some(miso){return r;}}}
        // 0xff for Sticky's 0x70 read selects the supported SSD1677 path in
        // Driver_Sticky_Auto. It deliberately never reports SSD2677's 0x07 ID.
        vec![255;rx_len]
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        let sd=SpiSdCard::new(image,config)?;let description=sd.describe();if self.sd.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.sd_pins().2,level:false});}self.sd=Some(Box::new(sd));Ok(description)
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        if !self.sticky(){return vec![];}self.i2c.clear();
        for address in [0x14,0x5d] {self.i2c.attach(address,Box::new(StrappedTouch{device:Gt911::new(self.touch.clone()),address,selected:self.touch_address.clone()}));}
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((3,2))).into_iter().map(move |(a,d)|(bus,a,d))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {
        let mut pins=vec![(self.sd_pins().2,self.sd.is_none()),(if self.sticky(){18}else{13},self.panel.busy_high())];
        pins.extend(if self.sticky(){vec![(4,true),(5,true),(6,true),(3,self.levels[0]),(2,self.levels[1]),(21,self.irq)]}else{vec![(3,true),(4,true),(5,true)]});pins
    }
    fn default_analog_inputs(&self)->Vec<(u8,esp_periph::AnalogSource)> {
        // Official SenseCraft_HMI 8e03627d41f63b4133e606cb25c5c81b9d251494:
        // boards/e100x/config.h BAT_ADC_PIN=1, ADC_EN=21 and common/battery_gauge.cpp
        // readVoltage() multiplies the measured pad voltage by 2.0. Model a nominal
        // 4.0V battery through that gated divider; no discharge/charger/PMIC claims.
        // Explicit host voltage/raw sources remain authoritative via set_default.
        if matches!(self.variant,Variant::E1001|Variant::E1002|Variant::E1004){vec![(1,esp_periph::AnalogSource::Stream(self.battery.clone()))]}else{vec![]}
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        if !self.sticky(){return;}self.sync_touch();{let mut t=self.touch.lock().expect("Seeed GT911 poisoned");if !t.powered{return;}
        // Inverse of the official Sticky Arduino example's portrait-to-landscape
        // swap + 180-degree transform. Its inclusive scaling can differ by 1px.
        let raw_x=((u32::from(479-y.min(479))*480+239)/479).min(479) as u16;
        let raw_y=((u32::from(799-x.min(799))*800+399)/799).min(799) as u16;t.update(raw_x,raw_y,down);}self.sync_touch();
    }
    fn named_pin(&self,id:&str)->Option<u8> {if self.sticky(){match id{"ai"=>Some(4),"up"=>Some(5),"down"=>Some(6),_=>None}}else{match id{"key0"=>Some(3),"key1"=>Some(4),"key2"=>Some(5),_=>None}}}
    fn advance_to(&mut self,cycle:VirtualCycle){self.cycle=cycle;self.panel.advance_to(cycle);self.sync_touch();}
    fn next_deadline(&self)->Option<VirtualCycle>{self.panel.next_deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge>{self.sync_touch();let mut edges=std::mem::take(&mut self.edges);edges.extend(self.panel.edges());edges.sort_by_key(|e|e.cycle);edges}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)>{let (frame,version,_,_)=self.panel.frame();
        // Seeed_GFX 24c5094c Tcon.cpp reverses row words and X windows for this
        // mounted panel. Native controller X runs opposite the physical front;
        // mirror that axis, then rotate into the catalog's portrait mounting.
        if self.tcon(){let mut portrait=vec![0;frame.len()];for y in 0..1404{for x in 0..1872{portrait[(1871-x)*1404+1403-y]=frame[y*1872+x];}}return Some((1404,1872,portrait,version));}
        let (w,h)=if self.large(){(1200,1600)}else{(800,480)};Some((w,h,frame.to_vec(),version))
    }
    fn display_version(&self)->u64{self.panel.frame().1}
    fn display_frames(&self)->u64{self.panel.frame().2}
    fn display_quiet_push(&self)->bool{true}
    fn report(&self)->String{format!("[{}] frames={} RAM_bytes={} panel_enabled={} SPI-SD={} SD-power={} touch={} optical/RTC/sensors/audio/power-management=unverified {}",self.name(),self.panel.frame().2,self.panel.frame().3,self.active,self.sd.as_ref().map_or_else(||"absent".into(),|s|s.report()),self.sd_power,if self.sticky(){"GT911-single-contact"}else{"none"},if let Panel::Tcon(p)=&self.panel{p.report()}else{String::new()})}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(b:&mut SeeedReTerminal,c:u8,data:&[u8]) {let (sck,mosi,_,cs)=b.bus_pins();let pins=SpiPins{sclk:1<<sck,mosi:1<<mosi,cs:1<<cs,..Default::default()};b.dc=false;b.spi_transfer_pins(2,pins,&[c],0);b.dc=true;b.spi_transfer_pins(2,pins,data,0);}
    #[test]
    fn separate_panels_require_correct_routes_gate_and_refresh() {
        for variant in [Variant::Sticky,Variant::E1001,Variant::E1002] {
            let mut b=SeeedReTerminal::new(variant);let (sck,mosi,_,cs)=b.bus_pins();
            if variant==Variant::Sticky {send(&mut b,0x24,&[0]);assert_eq!(b.panel.frame().3,0);b.gpio_output_at(0,&[],1<<47,1<<47);}
            let data_cmd=match variant{Variant::Sticky=>0x24,Variant::E1001=>0x13,Variant::E1002|Variant::E1003|Variant::E1004=>0x10};
            send(&mut b,data_cmd,&[]);let before=b.panel.frame().3;b.spi_transfer_pins(2,SpiPins{sclk:1<<(sck+1),mosi:1<<mosi,cs:1<<cs,..Default::default()},&[0],0);assert_eq!(b.panel.frame().3,before);
            send(&mut b,data_cmd,&[0]);assert_eq!(b.display_frames(),0);
            if variant==Variant::Sticky {send(&mut b,0x22,&[0xc4]);send(&mut b,0x20,&[]);}else {send(&mut b,0x61,&[3,32,1,224]);send(&mut b,0x04,&[]);b.advance_to(crate::periph::CPU_HZ/1000);send(&mut b,0x12,&[0]);}
            b.advance_to(crate::periph::CPU_HZ/10);assert_eq!(b.display_frames(),1);assert_eq!(b.display().unwrap().2.iter().filter(|p|**p==0).count(),if variant==Variant::E1002{2}else{8});
            if variant==Variant::Sticky {b.gpio_output_at(crate::periph::CPU_HZ/10,&[],1<<47,0);assert_eq!(b.display_frames(),1);assert_eq!(b.display().unwrap().2.iter().filter(|p|**p==0).count(),8);}
        }
    }
    #[test]
    fn e1004_power_and_dual_select_reach_distinct_portrait_halves() {
        let mut b=SeeedReTerminal::new(Variant::E1004);let pins=SpiPins{sclk:1<<7,mosi:1<<9,cs:(1<<10)|(1<<2),..Default::default()};
        send(&mut b,0x10,&[0]);assert_eq!(b.panel.frame().3,0);b.gpio_output_at(0,&[],(1<<12)|(1<<38),(1<<12)|(1<<38));
        for (cmd,data) in [(0x61,&[4,0xb0,3,0x20][..]),(0x04,&[][..]),(0x10,&[0x23][..]),(0x12,&[1][..])]{b.dc=false;b.spi_transfer_pins(2,pins,&[cmd],0);b.dc=true;b.spi_transfer_pins(2,pins,data,0);}
        b.advance_to(crate::periph::CPU_HZ/10);let (w,h,frame,_)=b.display().unwrap();assert_eq!((w,h),(1200,1600));assert_eq!(&frame[..2],&[0xffe0,0xf800]);assert_eq!(&frame[600..602],&[0xffe0,0xf800]);assert_eq!(b.display_frames(),1);
        b.gpio_output_at(crate::periph::CPU_HZ/10,&[],(1<<12)|(1<<38),1<<38);send(&mut b,0x10,&[0]);assert_eq!(b.panel.frame().3,2);assert_eq!(b.display().unwrap().2[0],0xffe0);
    }
    #[test]
    fn e1003_tcon_requires_both_supplies_route_miso_and_continuous_select() {
        let mut b=SeeedReTerminal::new(Variant::E1003);let pins=SpiPins{sclk:1<<7,mosi:1<<9,miso:Some(8),cs:1<<10,..Default::default()};
        b.spi_transfer_pins(2,pins,&[0x60,0,3,2],0);assert_eq!(b.spi_transfer_pins(2,pins,&[0x10,0],6),[255;6]);
        b.gpio_output_at(0,&[],(1<<11)|(1<<21),1<<11);assert_eq!(b.spi_transfer_pins(2,pins,&[0x10,0],6),[255;6]);
        let enabled=(1<<10)|(1<<11)|(1<<21);b.gpio_output_at(1,&[],enabled,enabled);b.gpio_output_at(2,&[],enabled,enabled&!(1<<10));
        b.spi_transfer_pins(2,pins,&[0x60,0,3,2],0);b.gpio_output_at(3,&[],enabled,enabled);b.gpio_output_at(4,&[],enabled,enabled&!(1<<10));
        b.spi_transfer_pins(2,pins,&[0x10,0],0);assert_eq!(b.spi_transfer_pins(2,pins,&[],6),[0,0,7,80,5,124]);
        b.gpio_output_at(5,&[],enabled,enabled);assert_eq!(b.spi_transfer_pins(2,pins,&[],6),[255;6]);
        assert_eq!(b.sd_pins(),(14,39,15));assert_eq!((b.display().unwrap().0,b.display().unwrap().1),(1404,1872));
    }
    #[test]
    fn sd_needs_power_select_and_miso_and_does_not_share_display_select() {
        let mut b=SeeedReTerminal::new(Variant::Sticky);let pins=SpiPins {sclk:1<<13,mosi:1<<14,miso:Some(12),cs:1<<8,keep_cs:false};let cmd=[0x40,0,0,0,0,0x95];
        b.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();assert!(b.input_levels().contains(&(11,false)));
        b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255]);
        b.gpio_output_at(1,&[],1<<10,1<<10);b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);b.spi_transfer_pins(2,pins,&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,1]);
        b.spi_transfer_pins(2,SpiPins{cs:(1<<8)|(1<<15),..pins},&cmd,0);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255]);
        b.gpio_output_at(2,&[],1<<10,0);b.gpio_output_at(3,&[],1<<10,1<<10);assert_eq!(b.spi_transfer_pins(2,pins,&[],2),[255,255]);
    }
    #[test]
    fn sticky_touch_obeys_power_reset_address_and_coordinate_transform() {
        let mut b=SeeedReTerminal::new(Variant::Sticky);let mut devices=b.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((3,2))));
        b.touch(200,100,true);assert!(!b.touch.lock().unwrap().down);b.gpio_output_at(0,&[],(1<<42)|(1<<41),1<<42);b.gpio_output_at(1,&[],(1<<42)|(1<<41),(1<<42)|(1<<41));
        b.touch(200,100,true);let t=b.touch.lock().unwrap();assert_eq!((t.x,t.y,t.down),(380,600,true));drop(t);
        let d=&mut devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x5d).unwrap().2;assert!(d.start(false));d.write(0x81);d.write(0x4e);d.start(true);d.read();d.stop();
        assert!(!devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x14).unwrap().2.start(false));
        b.gpio_output_at(2,&[],(1<<42)|(1<<41),0);assert!(!b.touch.lock().unwrap().down);assert!(b.irq);
        assert_eq!(b.named_pin("ai"),Some(4));assert_eq!(b.named_pin("power"),None);let e=SeeedReTerminal::new(Variant::E1001);assert_eq!(e.named_pin("key0"),Some(3));assert!(e.sd_card_slot().is_none());
    }
    #[test]
    fn battery_divider_is_gated_and_host_overrides_remain_authoritative() {
        let mut b=SeeedReTerminal::new(Variant::E1001);let mut pads=esp_periph::AnalogInputs::new(crate::periph::CPU_HZ);for (pin,source) in b.default_analog_inputs(){pads.set_default(pin,source);}
        assert_eq!(pads.volts(1,0),0.0);b.gpio_output_at(0,&[],1<<21,1<<21);assert_eq!(pads.volts(1,0),2.0);assert_eq!(pads.volts(2,0),0.0);
        b.gpio_output_at(1,&[],1<<21,0);assert_eq!(pads.volts(1,1),0.0);pads.set(1,esp_periph::AnalogSource::Const(1.6));b.gpio_output_at(2,&[],1<<21,1<<21);
        for (pin,source) in b.default_analog_inputs(){pads.set_default(pin,source);}assert_eq!(pads.volts(1,2),1.6);assert!(SeeedReTerminal::new(Variant::Sticky).default_analog_inputs().is_empty());
    }
}
