//! Seeed XIAO ePaper EE02/03/04/05: independent bare adapters and fixed glass kits.
//! Seeed_GFX2 556df7cb0b4a097d9b81026a26fd5511f9c934d6,
//! XIAO_ePaper_Board_Configs.h / Seeed_Panel_Configs.h; hardware/source receipts
//! and the EE05 ADC_EN39 vs firmware ADC_EN6 discrepancy: seeed-ee-runtime.md.
use esp_periph::analog::AnalogStream;
use esp_soc::board::{Board,BoardModel,BoardEdge,SpiPins,VirtualCycle};
use esp_soc::devices::{ssd1681::Ssd1681,ssd1677::Ssd1677,uc8179::Uc8179,ed2208::Ed2208,t133a01::T133a01,it8951::It8951};
use esp_soc::devices::{ssd16xx_mono::{Ssd16xxMono,Variant as MonoVariant},jd796xx::{Jd796xx,Variant as QuadVariant},uc8151::{Uc8151,Variant as FlexVariant}};
use esp_soc::pins::PinRoutes;

#[derive(Clone,Copy,PartialEq)]
enum Adapter {Ee02,Ee03,Ee04,Ee05}
#[derive(Clone,Copy)]
enum Glass {Mono154,Mono213,Mono290,Mono420,Mono426,Mono583,Mono750,Quad213,Quad290,Flex213,Flex290,Color730,Color1330,Tcon1030}
impl Glass {
    fn geometry(self)->(usize,usize){match self{Self::Mono154=>(200,200),Self::Mono213|Self::Quad213=>(122,250),Self::Mono290|Self::Quad290|Self::Flex290=>(128,296),Self::Flex213=>(104,212),Self::Mono420=>(400,300),Self::Mono583=>(648,480),Self::Mono426|Self::Mono750|Self::Color730=>(800,480),Self::Color1330=>(1200,1600),Self::Tcon1030=>(1872,1404)}}
}
// Keep the bare product distinct: None means no attached panel, not a default glass.
const CONFIGS:&[(&str,Adapter,Option<Glass>)]=&[
    ("seeed-xiao-epaper-ee02",Adapter::Ee02,None),
    ("seeed-xiao-epaper-ee03",Adapter::Ee03,None),
    ("seeed-xiao-epaper-ee04",Adapter::Ee04,None),
    ("seeed-xiao-epaper-ee05",Adapter::Ee05,None),
    ("seeed-xiao-epaper-ee02-p133-sp6",Adapter::Ee02,Some(Glass::Color1330)),
    ("seeed-xiao-epaper-ee03-p103-mono",Adapter::Ee03,Some(Glass::Tcon1030)),
    ("seeed-xiao-epaper-ee04-p073-sp6",Adapter::Ee04,Some(Glass::Color730)),
    ("seeed-xiao-epaper-ee04-p075-mono",Adapter::Ee04,Some(Glass::Mono750)),
    ("seeed-xiao-epaper-ee05-p075-mono",Adapter::Ee05,Some(Glass::Mono750)),
    ("seeed-xiao-epaper-ee04-p0426-mono",Adapter::Ee04,Some(Glass::Mono426)),
    ("seeed-xiao-epaper-ee05-p0426-mono",Adapter::Ee05,Some(Glass::Mono426)),
    ("seeed-xiao-epaper-ee04-p0154-mono",Adapter::Ee04,Some(Glass::Mono154)),
    ("seeed-xiao-epaper-ee05-p0154-mono",Adapter::Ee05,Some(Glass::Mono154)),
    ("seeed-xiao-epaper-ee04-p0213-mono",Adapter::Ee04,Some(Glass::Mono213)),
    ("seeed-xiao-epaper-ee05-p0213-mono",Adapter::Ee05,Some(Glass::Mono213)),
    ("seeed-xiao-epaper-ee04-p029-mono",Adapter::Ee04,Some(Glass::Mono290)),
    ("seeed-xiao-epaper-ee05-p029-mono",Adapter::Ee05,Some(Glass::Mono290)),
    ("seeed-xiao-epaper-ee04-p042-mono",Adapter::Ee04,Some(Glass::Mono420)),
    ("seeed-xiao-epaper-ee05-p042-mono",Adapter::Ee05,Some(Glass::Mono420)),
    ("seeed-xiao-epaper-ee04-p0583-mono",Adapter::Ee04,Some(Glass::Mono583)),
    ("seeed-xiao-epaper-ee05-p0583-mono",Adapter::Ee05,Some(Glass::Mono583)),
    ("seeed-xiao-epaper-ee04-p0213-quad",Adapter::Ee04,Some(Glass::Quad213)),
    ("seeed-xiao-epaper-ee05-p0213-quad",Adapter::Ee05,Some(Glass::Quad213)),
    ("seeed-xiao-epaper-ee04-p029-quad",Adapter::Ee04,Some(Glass::Quad290)),
    ("seeed-xiao-epaper-ee05-p029-quad",Adapter::Ee05,Some(Glass::Quad290)),
    ("seeed-xiao-epaper-ee04-p0213-flex",Adapter::Ee04,Some(Glass::Flex213)),
    ("seeed-xiao-epaper-ee05-p0213-flex",Adapter::Ee05,Some(Glass::Flex213)),
    ("seeed-xiao-epaper-ee04-p029-flex",Adapter::Ee04,Some(Glass::Flex290)),
    ("seeed-xiao-epaper-ee05-p029-flex",Adapter::Ee05,Some(Glass::Flex290)),
];
enum Panel {Small(Ssd1681),Ssd(Ssd1677),SmallMono(Ssd16xxMono),Quad(Jd796xx),Flex(Uc8151),Mono(Uc8179),Color(Ed2208),Large(T133a01),Tcon(It8951)}
impl Panel {
    fn new(glass:Glass,ms:u64)->Self {match glass {
        Glass::Mono154=>Self::Small(Ssd1681::new(4,ms)),Glass::Mono426=>Self::Ssd(Ssd1677::new(800,480,4,ms)),Glass::Mono750=>Self::Mono(Uc8179::new(4,ms)),Glass::Mono583=>Self::Mono(Uc8179::seeed_583(4,ms)),
        Glass::Mono213=>Self::SmallMono(Ssd16xxMono::new(MonoVariant::Ssd1680_213,4,ms)),Glass::Mono290=>Self::SmallMono(Ssd16xxMono::new(MonoVariant::Ssd1680_290,4,ms)),Glass::Mono420=>Self::SmallMono(Ssd16xxMono::new(MonoVariant::Ssd1683_420,4,ms)),
        Glass::Quad213=>Self::Quad(Jd796xx::new(QuadVariant::Jd79676,4,ms)),Glass::Quad290=>Self::Quad(Jd796xx::new(QuadVariant::Jd79667,4,ms)),Glass::Flex213=>Self::Flex(Uc8151::new(FlexVariant::Flex213,4,ms)),Glass::Flex290=>Self::Flex(Uc8151::new(FlexVariant::Flex290,4,ms)),
        Glass::Color730=>Self::Color(Ed2208::e1002(4,ms)),Glass::Color1330=>Self::Large(T133a01::new(4,ms)),Glass::Tcon1030=>Self::Tcon(It8951::new(4,ms))}}
    fn reset(&mut self){match self{Self::Small(p)=>p.reset(),Self::Ssd(p)=>p.reset(),Self::SmallMono(p)=>p.reset(),Self::Quad(p)=>p.reset(),Self::Flex(p)=>p.reset(),Self::Mono(p)=>p.reset(),Self::Color(p)=>p.reset(),Self::Large(p)=>p.reset(),Self::Tcon(p)=>p.reset()}}
    fn power_loss(&mut self){match self{Self::Small(p)=>p.power_loss(),Self::Ssd(p)=>p.power_loss(),Self::SmallMono(p)=>p.power_loss(),Self::Quad(p)=>p.power_loss(),Self::Flex(p)=>p.power_loss(),Self::Mono(p)=>p.power_loss(),_=>self.reset()}}
    fn write(&mut self,selected:u8,dc:bool,data:&[u8]){match self{Self::Small(p)=>p.write(dc,data),Self::Ssd(p)=>p.write(dc,data),Self::SmallMono(p)=>p.write(dc,data),Self::Quad(p)=>p.write(dc,data),Self::Flex(p)=>p.write(dc,data),Self::Mono(p)=>p.write(dc,data),Self::Color(p)=>p.write(dc,data),Self::Large(p)=>p.write(selected,dc,data),Self::Tcon(_)=>{}}}
    fn advance_to(&mut self,c:u64){match self{Self::Small(p)=>p.advance_to(c),Self::Ssd(p)=>p.advance_to(c),Self::SmallMono(p)=>p.advance_to(c),Self::Quad(p)=>p.advance_to(c),Self::Flex(p)=>p.advance_to(c),Self::Mono(p)=>p.advance_to(c),Self::Color(p)=>p.advance_to(c),Self::Large(p)=>p.advance_to(c),Self::Tcon(p)=>p.advance_to(c)}}
    fn busy_high(&self)->bool{match self{Self::Small(p)=>p.busy(),Self::Ssd(p)=>p.busy(),Self::SmallMono(p)=>p.busy(),Self::Quad(p)=>p.busy_high(),Self::Flex(p)=>p.busy_high(),Self::Mono(p)=>p.busy_high(),Self::Color(p)=>p.busy_high(),Self::Large(p)=>p.busy_high(),Self::Tcon(p)=>p.busy_high()}}
    fn next_deadline(&self)->Option<u64>{match self{Self::Small(p)=>p.next_deadline(),Self::Ssd(p)=>p.next_deadline(),Self::SmallMono(p)=>p.next_deadline(),Self::Quad(p)=>p.next_deadline(),Self::Flex(p)=>p.next_deadline(),Self::Mono(p)=>p.next_deadline(),Self::Color(p)=>p.next_deadline(),Self::Large(p)=>p.next_deadline(),Self::Tcon(p)=>p.next_deadline()}}
    fn edges(&mut self)->Vec<BoardEdge>{match self{Self::Small(p)=>p.take_edges(),Self::Ssd(p)=>p.take_edges(),Self::SmallMono(p)=>p.take_edges(),Self::Quad(p)=>p.take_edges(),Self::Flex(p)=>p.take_edges(),Self::Mono(p)=>p.take_edges(),Self::Color(p)=>p.take_edges(),Self::Large(p)=>p.take_edges(),Self::Tcon(p)=>p.take_edges()}}
    fn frame(&self)->(&[u16],u64,u64,u64){match self{Self::Small(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Ssd(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::SmallMono(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Quad(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Flex(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Mono(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Color(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Large(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written),Self::Tcon(p)=>(&p.frame,p.version,p.frames,p.ram_bytes_written)}}
}
pub fn make_board(name:&str)->Option<Board> {CONFIGS.iter().find(|c|c.0==name).map(|&(id,adapter,glass)|Box::new(SeeedEe::new(id,adapter,glass)) as Board)}
pub struct SeeedEe {
    id:&'static str,adapter:Adapter,glass:Option<Glass>,panel:Option<Panel>,cycle:u64,events:u64,
    powered:bool,active:bool,dc:Option<bool>,software_cs:bool,selected:bool,spi_host:u8,
    battery:AnalogStream,battery_enabled:bool,led:[[u8;3];1],led_version:u64,
}
impl SeeedEe {
    fn new(id:&'static str,adapter:Adapter,glass:Option<Glass>)->Self {Self{id,adapter,glass,panel:glass.map(|p|Panel::new(p,crate::periph::CPU_HZ/1000)),cycle:0,events:0,powered:false,active:false,dc:None,software_cs:false,selected:false,spi_host:0,battery:AnalogStream::new(1,0.0,0).expect("constant EE battery source"),battery_enabled:false,led:[[0;3]],led_version:0}}
    fn third_key(&self)->u8 {if self.adapter==Adapter::Ee05{8}else{5}}
    fn battery_enable_pin(&self)->u8 {if self.adapter==Adapter::Ee05{39}else{6}}
}
impl BoardModel for SeeedEe {
    fn name(&self)->&'static str {self.id}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,_cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64){self.events+=changes.len() as u64;}
    fn gpio_events(&self)->u64 {self.events}
    fn gpio_waveform_at(&mut self,cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.cycle=cycle;
        let battery=routes.software_output(self.battery_enable_pin())==Some(true);
        if battery!=self.battery_enabled {self.battery.replace_current(if battery{2.0}else{0.0}).expect("constant EE divider voltage");self.battery_enabled=battery;}
        let led=if routes.software_output(21)==Some(false){[0,255,0]}else{[0;3]};if self.led[0]!=led{self.led[0]=led;self.led_version+=1;}
        let Some(panel)=&mut self.panel else{return;};panel.advance_to(cycle);
        // PWR_EN43 is the same pad as native UART0 TX. IDF v4.4.7 S3
        // gpio_sig_map.h/io_mux_reg.h: U0TXD signal12/function0; idle is HIGH.
        // Only its steady level is represented, not power ripple from UART bits.
        let power=routes.software_output(43)==Some(true)||routes.function(43,0)||routes.matrix_output(43,12);
        let active=power&&routes.software_output(38)!=Some(false);
        if self.powered&&!power{panel.power_loss();}else if self.active&&!active{panel.reset();}
        self.powered=power;self.active=active;self.dc=routes.software_output(10);
        let software_cs=routes.software_output(44)==Some(false);
        if self.software_cs&&!software_cs||!active{if let Panel::Tcon(p)=panel{p.end_transaction();}self.selected=false;}
        self.software_cs=software_cs;
    }
    fn uses_spi_pins(&self)->bool {self.panel.is_some()}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        let Some(panel)=&mut self.panel else{return vec![255;rx_len];};
        let selected=u8::from(pins.cs&(1<<44)!=0)|if self.adapter==Adapter::Ee02&&pins.cs&(1<<41)!=0{2}else{0};
        let valid=self.active&&matches!(host,2|3)&&pins.sclk&(1<<7)!=0&&pins.mosi&(1<<9)!=0&&selected!=0;
        if let Panel::Tcon(p)=panel {
            if !self.selected||self.spi_host!=host {p.end_transaction();}self.spi_host=host;
            let r=if valid{p.transfer(tx,rx_len)}else{vec![255;rx_len]};self.selected=valid&&(pins.keep_cs||self.software_cs);
            if !self.selected{p.end_transaction();}
            return if pins.miso==Some(8){r}else{vec![255;rx_len]};
        }
        if valid{if let Some(dc)=self.dc{panel.write(selected,dc,tx);}}
        // Every other kit has no MISO conductor. A bare adapter has no endpoint.
        vec![255;rx_len]
    }
    fn input_levels(&self)->Vec<(u8,bool)> {
        let mut pins=vec![(0,true),(2,true),(3,true),(self.third_key(),true)];
        if let Some(panel)=&self.panel{pins.push((4,panel.busy_high()));}pins
    }
    fn named_pin(&self,name:&str)->Option<u8>{match name{"boot"=>Some(0),"key1"=>Some(2),"key2"=>Some(3),"key3"=>Some(self.third_key()),_=>None}}
    fn default_analog_inputs(&self)->Vec<(u8,esp_periph::AnalogSource)> {
        // Official schematics use equal 10k divider resistors through TPS22916:
        // 4.0V nominal battery -> 2.0V BAT_ADC1 only while ADC_EN is HIGH.
        // EE05 SCH/PCB V1.11 routes ADC_EN to D12/GPIO39; do not compensate for
        // SenseCraft_HMI 8e03627d's common ADC_EN6 definition. Host overrides win.
        vec![(1,esp_periph::AnalogSource::Stream(self.battery.clone()))]
    }
    fn leds(&self)->Option<(&[[u8;3]],u64)>{Some((&self.led,self.led_version))}
    fn advance_to(&mut self,cycle:VirtualCycle){self.cycle=cycle;if let Some(p)=&mut self.panel{p.advance_to(cycle);}}
    fn next_deadline(&self)->Option<VirtualCycle>{self.panel.as_ref().and_then(Panel::next_deadline)}
    fn take_edges(&mut self)->Vec<BoardEdge>{self.panel.as_mut().map_or_else(Vec::new,Panel::edges)}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let panel=self.panel.as_ref()?;let (frame,version,_,_)=panel.frame();let (w,h)=self.glass?.geometry();let mut pixels=frame.to_vec();
        // Only 4.26-inch glass has Config::mirror=true. The small-panel
        // breakout-specific mirror flag does not apply to EE adapter boards.
        // IT8951's official 1bpp path reverses X windows and row words; map
        // its physical front to native landscape, not E1003's portrait mounting.
        if matches!(panel,Panel::Ssd(_)|Panel::Tcon(_)){for row in pixels.chunks_exact_mut(w){row.reverse();}}
        Some((w as u32,h as u32,pixels,version))
    }
    fn display_version(&self)->u64{self.panel.as_ref().map_or(0,|p|p.frame().1)}
    fn display_frames(&self)->u64{self.panel.as_ref().map_or(0,|p|p.frame().2)}
    fn display_quiet_push(&self)->bool{self.panel.is_some()}
    fn report(&self)->String {
        let (frames,bytes)=self.panel.as_ref().map_or((0,0),|p|{let f=p.frame();(f.2,f.3)});
        format!("[{}] panel={} frames={} RAM_bytes={} panel_enabled={} battery_gate={} partial; nominal-battery/digital-display only, PMIC/radio/sleep-wake unverified",self.id,self.panel.is_some(),frames,bytes,self.active,self.battery_enable_pin())
    }
}
