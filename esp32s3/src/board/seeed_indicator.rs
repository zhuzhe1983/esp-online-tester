//! SenseCAP Indicator D1 RGB display/input, explicit GX(ST7701S) and DX profiles.
//! Seeed-Solution/SenseCAP_Indicator_ESP32 77edb8d2, bsp sensecap_indicator_board.c,
//! lcd_panel_config.c and ft5x06.c. No RP2040 sensor readings or LoRa are invented.
use std::sync::{Arc,Mutex,atomic::Ordering};
use esp_periph::i2c::{I2cDevice,ft6336::{Ft6336g,Ft6336State}};
use esp_soc::{board::{BoardModel,LcdRgbPins,VirtualCycle},pins::PinRoutes};
use super::seeed_sensecap::Expander;

#[derive(Clone,Copy,PartialEq)]
pub enum IndicatorPanel {Gx,Dx}
struct IndicatorTouch {io:Expander,state:Arc<Mutex<Ft6336State>>,device:Ft6336g}
impl I2cDevice for IndicatorTouch {
    fn pins(&self)->Option<(u8,u8)> {Some((39,40))}
    fn start(&mut self,read:bool)->bool {let powered=self.io.driven(7);let mut s=self.state.lock().expect("Indicator touch poisoned");s.powered=powered;if !powered{s.reset();}drop(s);self.device.start(read)}
    fn write(&mut self,b:u8)->bool {self.device.write(b)}
    fn read(&mut self)->u8 {self.device.read()}
    fn stop(&mut self) {self.device.stop();}
}

pub struct SeeedIndicator {
    kind:IndicatorPanel,io:Expander,touch:Arc<Mutex<Ft6336State>>,frame:Vec<u16>,frames:u64,changes:u64,events:u64,
    rgb_valid:bool,rgb_swap_bytes:bool,backlight:bool,pwm:u32,reset_seen:u64,select_seen:u64,clock:Option<bool>,bits:u8,word:u16,
    page:u8,command:u8,args:[u8;5],count:usize,sleeping:bool,on:bool,commands:u64,rejected:u64,
}
impl SeeedIndicator {
    pub fn new(kind:IndicatorPanel)->Self {Self{kind,io:Expander::new(5,(39,40)),touch:Default::default(),frame:vec![0;480*480],frames:0,changes:0,events:0,rgb_valid:false,rgb_swap_bytes:false,backlight:false,pwm:0,reset_seen:0,select_seen:0,clock:None,bits:0,word:0,page:0,command:0,args:[0;5],count:0,sleeping:true,on:false,commands:0,rejected:0}}
    pub(super) fn expander(&self)->Expander {self.io.clone()}
    pub(super) fn rgb_diagnostics(&self)->String {format!("RGB565 frames={} commands={} rejected={} routed={} swap_bytes={} reset_high={} visible={}",self.frames,self.commands,self.rejected,self.rgb_valid,self.rgb_swap_bytes,self.io.powered(),self.visible())}
    fn rgb_mapping(&mut self,direct:bool,swapped:bool) {let valid=direct||swapped;let swap=valid&&!direct;if (valid,swap)!=(self.rgb_valid,self.rgb_swap_bytes){self.rgb_valid=valid;self.rgb_swap_bytes=swap;self.changes+=1;}}
    fn sync(&mut self) {let epoch=self.io.reset_version();if self.reset_seen!=epoch {self.reset_seen=epoch;self.sleeping=true;self.on=false;self.page=0;self.bits=0;self.clock=None;self.changes+=1;}}
    fn visible(&self)->bool {self.io.powered()&&self.io.reset_version()==self.reset_seen&&self.rgb_valid&&(self.backlight||self.pwm>0)&&(self.kind==IndicatorPanel::Dx||(!self.sleeping&&self.on))}
    fn serial_word(&mut self,word:u16) {
        if word&0x100==0 {
            self.command=word as u8;self.count=0;self.commands+=1;
            if self.page==0 {match self.command {1=>{self.sleeping=true;self.on=false;},0x10=>self.sleeping=true,0x11=>self.sleeping=false,0x28=>self.on=false,0x29=>self.on=true,_=>{}}self.changes+=1;}
        }else if self.command==0xff {
            if self.count<self.args.len(){self.args[self.count]=word as u8;self.count+=1;}
            if self.count==5 && self.args[..4]==[0x77,1,0,0] {self.page=self.args[4];}
        }
    }
}
impl BoardModel for SeeedIndicator {
    fn name(&self)->&'static str {match self.kind {IndicatorPanel::Gx=>"seeed-sensecap-indicator-gx",IndicatorPanel::Dx=>"seeed-sensecap-indicator-dx"}}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_output_at(&mut self,_cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.events+=changes.len() as u64;}
    fn gpio_events(&self)->u64 {self.events}
    fn gpio_waveform_at(&mut self,_cycle:VirtualCycle,routes:PinRoutes<'_>) {
        self.sync();let light=routes.software_output(45)==Some(true);if light!=self.backlight {self.backlight=light;self.changes+=1;}
        // S3 IDF v4.4.3/v5.2.1 gpio_sig_map.h: DATA0..15=133..148,
        // DE150, HSYNC151, VSYNC152, PCLK154. No S31 route numbering is used.
        let controls=routes.matrix_output(21,154)&&routes.matrix_output(16,151)&&routes.matrix_output(17,152)&&routes.matrix_output(18,150);
        // LovyanGFX 894e4340 src/lgfx/v1/platforms/esp32s3/Bus_RGB.cpp
        // rgb565sig_tbl swaps DATA0..7 with DATA8..15. Decode the actual pad
        // mapping, preserving the Seeed IDF BSP's direct 16-lane route.
        self.rgb_mapping(controls&&(0..16).all(|i|routes.matrix_output(15-i,133+i as u32)),controls&&(0..16).all(|i|routes.matrix_output(15-i,133+(i^8) as u32)));
        let select_epoch=self.io.select_epoch.load(Ordering::Relaxed);
        if select_epoch!=self.select_seen {self.select_seen=select_epoch;self.bits=0;self.word=0;self.clock=None;}
        let clk=routes.software_output(41);let data=routes.software_output(48);
        if self.kind==IndicatorPanel::Dx || !self.io.powered() || self.io.driven(4) || clk.is_none() || data.is_none() {self.bits=0;self.word=0;self.clock=None;return;}
        if self.clock==Some(false)&&clk==Some(true) {
            self.word=(self.word<<1)|u16::from(data==Some(true));self.bits+=1;
            if self.bits==9 {self.serial_word(self.word);self.bits=0;self.word=0;}
        }
        self.clock=clk;
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {let duty=pwm(45).map_or(0,|(_,d)|d.min(65535));if duty!=self.pwm {self.pwm=duty;self.changes+=1;}}
    fn lcd_rgb_pins(&mut self,p:LcdRgbPins) {
        let controls=p.width==16&&p.clock&(1<<21)!=0&&p.hsync&(1<<16)!=0&&p.vsync&(1<<17)!=0&&p.enable&(1<<18)!=0;
        self.rgb_mapping(controls&&(0..16).all(|i|p.data[i]&(1<<(15-i))!=0),controls&&(0..16).all(|i|p.data[i^8]&(1<<(15-i))!=0));
    }
    fn lcd_frame(&mut self,w:u32,h:u32,data:&[u8]) {
        self.sync();if !self.rgb_valid||!self.io.powered()||(w,h)!=(480,480)||data.len()!=480*480*2 {self.rejected+=1;return;}
        for (dst,src) in self.frame.iter_mut().zip(data.as_chunks::<2>().0) {let pixel=u16::from_le_bytes(*src);*dst=if self.rgb_swap_bytes{pixel.swap_bytes()}else{pixel};}self.frames+=1;
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let address=if self.kind==IndicatorPanel::Gx{0x48}else{0x38};
        (0..2).flat_map(|bus|[(bus,0x20,self.io.device()),(bus,address,Box::new(IndicatorTouch{io:self.io.clone(),state:self.touch.clone(),device:Ft6336g::new(self.touch.clone())}) as Box<dyn I2cDevice>)]).collect()
    }
    fn touch(&mut self,x:u16,y:u16,down:bool) {
        // Native touch axes oppose RGB scan coordinates: official v1.0.0
        // ed94f161 examples/terminal_demo/main/lv_port.c mirrors both axes.
        // Preserve zero-based 0..479 raw reports; that firmware's WIDTH-raw
        // formula leaves its original one-pixel edge discrepancy intact.
        if self.io.driven(7) {self.touch.lock().expect("Indicator touch poisoned").update(479-x.min(479),479-y.min(479),down);}
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(38,true),(39,true),(40,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"button"|"user"=>Some(38),_=>None}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((480,480,if self.visible(){self.frame.clone()}else{vec![0;480*480]},self.display_version()))}
    fn display_version(&self)->u64 {self.frames+self.changes+self.io.version()}
    fn display_frames(&self)->u64 {self.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.frame.clone(),480,480))}
    fn report(&self)->String {format!("[{}] {} touch=FT5x06; RP2040/sensors/LoRa absent",self.name(),self.rgb_diagnostics())}
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_soc::pins::ChipPins;
    fn write(d:&mut dyn I2cDevice,r:u8,values:&[u8]) {assert!(d.start(false));d.write(r);for &v in values{d.write(v);}}
    fn read(d:&mut dyn I2cDevice,r:u8,n:usize)->Vec<u8> {write(d,r,&[]);assert!(d.start(true));(0..n).map(|_|d.read()).collect()}
    fn output(b:&mut SeeedIndicator,mask:u8) {let mut d=b.io.device();write(d.as_mut(),2,&[mask,0]);write(d.as_mut(),6,&[0x4f,0xff]);}
    fn route(b:&mut SeeedIndicator,clock:bool,data:bool,light:bool) {let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();for (p,signal) in (0..16).map(|i|(15-i,133+i)).chain([(21,154),(16,151),(17,152),(18,150),(41,256),(48,256),(45,256)]){mux.write(4+4*p,1<<12);gpio.func_out_sel[p as usize]=signal;gpio.enable|=1<<p;}gpio.out=if clock{1<<41}else{0}|if data{1<<48}else{0}|if light{1<<45}else{0};b.gpio_waveform_at(0,ChipPins::S3.routes(&gpio,&mux));}
    fn word(b:&mut SeeedIndicator,w:u16) {output(b,0xb0);route(b,false,false,true);output(b,0xa0);route(b,false,false,true);for i in (0..9).rev(){route(b,false,w&(1<<i)!=0,true);route(b,true,w&(1<<i)!=0,true);}output(b,0xb0);}
    fn command(b:&mut SeeedIndicator,c:u8,args:&[u8]) {word(b,u16::from(c));for &a in args{word(b,0x100|u16::from(a));}}
    fn rgb()->LcdRgbPins {LcdRgbPins{clock:1<<21,hsync:1<<16,vsync:1<<17,enable:1<<18,width:16,data:std::array::from_fn(|i|1<<(15-i))}}
    #[test]
    fn indicator_gx_requires_nine_bit_init_reset_backlight_and_every_rgb_lane() {
        let mut b=SeeedIndicator::new(IndicatorPanel::Gx);let mut frame=vec![0;480*480*2];frame[..2].copy_from_slice(&0xf800u16.to_le_bytes());b.lcd_rgb_pins(rgb());b.lcd_frame(480,480,&frame);assert_eq!(b.frames,0);
        output(&mut b,0xb0);route(&mut b,false,false,true);b.lcd_frame(480,480,&frame);assert_eq!(b.display().unwrap().2[0],0);
        command(&mut b,0xff,&[0x77,1,0,0,0x11]);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);assert!(!b.visible(),"bank11h vendor registers are not sleep/display opcodes");
        command(&mut b,0xff,&[0x77,1,0,0,0]);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);assert_eq!(b.display().unwrap().2[0],0xf800);
        for i in 0..20 {let mut pins=rgb();match i{0..=15=>pins.data[i]=0,16=>pins.clock=0,17=>pins.hsync=0,18=>pins.vsync=0,_=>pins.enable=0};b.lcd_rgb_pins(pins);b.lcd_frame(480,480,&frame);assert!(!b.visible());assert_eq!(b.frames,1);}
        b.lcd_rgb_pins(rgb());route(&mut b,false,false,false);assert_eq!(b.display().unwrap().2[0],0);b.gpio_pwm_at(0,&|p|if p==45{Some((5000.,1))}else{None});assert!(b.visible());
        output(&mut b,0x90);assert!(!b.visible());output(&mut b,0xb0);route(&mut b,false,false,true);assert!(!b.visible(),"reset requires fresh GX init");
    }
    #[test]
    fn indicator_revisions_expose_only_their_real_touch_address_and_reset_gate() {
        for kind in [IndicatorPanel::Gx,IndicatorPanel::Dx] {
            let mut b=SeeedIndicator::new(kind);let address=if kind==IndicatorPanel::Gx{0x48}else{0x38};let mut devices=b.i2c_devices();assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((39,40))));let touch=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==address).unwrap().2.as_mut();assert!(!touch.start(true));
            output(&mut b,0xb0);b.touch(100,470,true);assert_eq!(read(touch,2,5),[1,0x81,123,0,9]);assert_eq!(read(touch,2,1),[1]);b.touch(100,470,false);assert_eq!(read(touch,2,1),[0]);
            output(&mut b,0x30);assert!(!touch.start(true));assert_eq!(b.named_pin("button"),Some(38));assert_eq!(b.named_pin("boot"),None);
        }
    }
    #[test]
    fn dx_rgb_profile_needs_reset_and_routing_without_inventing_a_spi_controller() {
        let mut b=SeeedIndicator::new(IndicatorPanel::Dx);output(&mut b,0xb0);route(&mut b,false,false,true);b.lcd_rgb_pins(rgb());let frame=vec![255;480*480*2];b.lcd_frame(480,480,&frame);assert_eq!(b.display().unwrap().2[0],0xffff);assert_eq!(b.commands,0);b.lcd_frame(1,1,&[0,0]);assert_eq!(b.frames,1);
    }
    #[test]
    fn indicator_rgb565_follows_direct_and_lovyangfx_gpio_lane_mappings() {
        for swapped in [false,true] {
            let mut b=SeeedIndicator::new(IndicatorPanel::Gx);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);
            let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
            for (pin,signal) in (0..16).map(|i|(15-i,133+if swapped{i^8}else{i})).chain([(21,154),(16,151),(17,152),(18,150),(45,256)]) {gpio.enable|=1<<pin;gpio.func_out_sel[pin as usize]=signal;mux.write(4+4*pin,1<<12);}
            gpio.out=1<<45;b.gpio_waveform_at(0,ChipPins::S3.routes(&gpio,&mux));assert_eq!(b.rgb_swap_bytes,swapped);
            let mut data=vec![0;480*480*2];for bit in 0..16 {let value=1u16<<bit;data[2*bit..2*bit+2].copy_from_slice(&if swapped{value.to_be_bytes()}else{value.to_le_bytes()});}
            b.lcd_frame(480,480,&data);let shown=b.display().unwrap().2;assert_eq!(b.frames,1);for (bit,value) in shown.iter().take(16).enumerate(){assert_eq!(*value,1<<bit);}
            // A missing physical lane must never silently select either mapping.
            gpio.func_out_sel[4]=256;b.gpio_waveform_at(0,ChipPins::S3.routes(&gpio,&mux));b.lcd_frame(480,480,&data);assert_eq!(b.frames,1);assert!(!b.visible());assert_eq!(b.gram().unwrap().0,shown);
        }
    }
    #[test]
    fn indicator_swapped_rgb_rejects_wrong_dimensions_lengths_and_each_missing_lane() {
        let mut b=SeeedIndicator::new(IndicatorPanel::Gx);command(&mut b,0x11,&[]);command(&mut b,0x29,&[]);let mut pins=rgb();pins.data=std::array::from_fn(|i|1<<(15-(i^8)));b.lcd_rgb_pins(pins);
        let mut data=vec![0;480*480*2];data[..2].copy_from_slice(&0xf81fu16.to_be_bytes());b.lcd_frame(480,480,&data);assert_eq!(b.display().unwrap().2[0],0xf81f);
        b.lcd_frame(800,480,&vec![0;800*480*2]);b.lcd_frame(480,800,&vec![0;480*800*2]);b.lcd_frame(480,480,&data[..data.len()-2]);assert_eq!(b.frames,1);assert_eq!(b.gram().unwrap().0[0],0xf81f);
        for lane in 0..21 {let mut bad=pins;match lane{0..=15=>bad.data[lane]=0,16=>bad.clock=0,17=>bad.hsync=0,18=>bad.vsync=0,19=>bad.enable=0,_=>bad.width=8};b.lcd_rgb_pins(bad);b.lcd_frame(480,480,&data);assert_eq!(b.frames,1);assert!(!b.visible());}
        b.lcd_rgb_pins(pins);b.lcd_frame(480,480,&data);assert_eq!(b.frames,2);assert_eq!(b.display().unwrap().2[0],0xf81f);
    }
}
