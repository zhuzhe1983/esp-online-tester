//! LILYGO T-Display-S3 non-touch: ST7789V 170x320 over LCD_CAM i80.
//! Manufacturer 1cc41eb1 examples/factory/pin_config.h and factory.ino:
//! D0..7=39/40/41/42/45/46/47/48, WR8/DC7/CS6/RST5/RD9, BL38,
//! power15 and active-low buttons0/14. USB-powered digital subset;
//! battery/charger and pulse brightness levels are unmodelled; LEDC uses its latched average duty.
use esp_soc::board::{BoardModel,LcdI80Pins,VirtualCycle};
use esp_soc::devices::DcsPanel;
use esp_soc::pins::PinRoutes;

const DATA:[u8;8]=[39,40,41,42,45,46,47,48];
pub struct TDisplayS3 {panel:DcsPanel,output:u64,enabled:u64,changes:u64,gpio_events:u64,transactions:u64,wr:Option<bool>,gpio_bytes:u64,pwm:u32}
impl TDisplayS3 {
    pub fn new()->Self {Self{panel:DcsPanel::st7789(),output:0,enabled:0,changes:0,gpio_events:0,transactions:0,wr:None,gpio_bytes:0,pwm:0}}
    fn backlight(&self)->u32 {if self.high(38) {65535} else {self.pwm}}
    fn high(&self,pin:u8)->bool {self.enabled&self.output&(1u64<<pin)!=0}
}
impl Default for TDisplayS3 {fn default()->Self {Self::new()}}
impl BoardModel for TDisplayS3 {
    fn name(&self)->&'static str {"tdisplay-s3"}
    fn gpio_output_at(&mut self,_cycle:VirtualCycle,changes:&[(u8,bool)],_enabled:u64,_output:u64) {self.gpio_events+=changes.len() as u64;}
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:VirtualCycle,routes:PinRoutes<'_>) {
        let reset=self.high(5);let light=self.high(38);self.enabled=0;self.output=0;
        for pin in [5,6,7,8,9,15,38,39,40,41,42,45,46,47,48] {if let Some(level)=routes.software_output(pin) {self.enabled|=1<<pin;if level {self.output|=1<<pin;}}}
        if reset && !self.high(5) {self.panel.reset();self.changes+=1;}
        if light!=self.high(38) {self.changes+=1;}
        let wr=routes.software_output(8);
        // Manufacturer bundled TFT_eSPI ESP32_S3.h tft_Write_8/16: put D0..7
        // on the bus with WR low, latch on WR high. Route/enable changes alone
        // cannot create a write from an unknown previous WR level.
        if self.wr==Some(false) && wr==Some(true) && self.high(5) && self.high(9)
            && routes.software_output(6)==Some(false) && self.enabled&(1<<7)!=0
            && DATA.iter().all(|&p|self.enabled&(1<<p)!=0) {
            self.panel.dc=self.high(7);if !self.panel.dc {self.changes+=1;}
            let byte=DATA.iter().enumerate().fold(0u8,|v,(i,&p)|v|((self.high(p) as u8)<<i));
            self.panel.byte(byte);self.gpio_bytes+=1;
        }
        self.wr=wr;
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        let duty=pwm(38).map_or(0,|(_,d)|d.min(65535));if duty!=self.pwm {self.pwm=duty;self.changes+=1;}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn lcd_i80(&mut self,pins:LcdI80Pins,_data_phase:bool,words:&[u16]) {
        if pins.width!=8 || pins.clock&(1<<8)==0 || pins.select&(1<<6)==0 || pins.dc&(1<<7)==0
            || !DATA.iter().enumerate().all(|(i,p)|pins.data[i]&(1<<p)!=0) || !self.high(5) || !self.high(9) {return;}
        self.panel.dc=pins.dc_high;self.transactions+=1;
        if !pins.dc_high {self.changes+=1;}
        for &word in words {self.panel.byte(word as u8);}
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(14,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"boot"|"btn1"|"button1"=>Some(0),"user"|"btn2"|"button2"=>Some(14),_=>None}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let mut frame=vec![0;170*320];
        // Fixed physical glass: GRAM columns35..204, rows0..319. Factory MV|MY
        // and gap(0,35) draw landscape; the calibrated CAD pane rotates -90deg.
        // Preserve the native portrait framebuffer rather than rotating both layers.
        let duty=self.backlight();
        if self.high(5) && duty!=0 && self.panel.on && !self.panel.sleeping {
            for (row,line) in frame.chunks_mut(170).enumerate() {line.copy_from_slice(&self.panel.gram[row*240+35..row*240+205]);}
        }
        if duty<65535 {for pixel in &mut frame {let p=*pixel as u32;*pixel=(((((p>>11)&31)*duty/65535)<<11)|((((p>>5)&63)*duty/65535)<<5)|((p&31)*duty/65535)) as u16;}}
        Some((170,320,frame,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.changes}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn report(&self)->String {format!("[tdisplay-s3] ST7789V i80={} gpio_bytes={} frames={} pixels={} madctl={:#x} on={} sleeping={} reset={} rd={} backlight_snapshot={} power15={} USB-powered; no touch/battery/pulse-dimming",self.transactions,self.gpio_bytes,self.panel.frames,self.panel.pixels_written,self.panel.madctl,self.panel.on,self.panel.sleeping,self.high(5),self.high(9),self.backlight(),self.high(15))}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->LcdI80Pins {LcdI80Pins{data:std::array::from_fn(|i|if i<8{1<<DATA[i]}else{0}),clock:1<<8,select:1<<6,dc:1<<7,width:8,..Default::default()}}
    fn drive(b:&mut TDisplayS3,enabled:u64,output:u64) {
        let mut gpio=esp_periph::Gpio::new();gpio.enable=enabled;gpio.out=output;let mut mux=esp_periph::RegRam::new();
        for p in 0..49 {mux.write(4+4*p,1<<12);}
        b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&gpio,&mux));
    }
    fn drive_state(b:&mut TDisplayS3,disable:u64,mask:u64) {drive(b,b.enabled&!disable,b.output&mask);}
    fn power(b:&mut TDisplayS3) {let mask=(1<<5)|(1<<9)|(1<<15)|(1<<38);drive(b,mask,mask);}
    fn cmd(b:&mut TDisplayS3,c:u8,data:&[u8]) {b.lcd_i80(pins(),false,&[c as u16]);b.lcd_i80(LcdI80Pins{dc_high:true,..pins()},true,&data.iter().map(|&v|v as u16).collect::<Vec<_>>());}
    #[test]
    fn factory_four_corners_keep_physical_portrait_glass() {
        let mut b=TDisplayS3::new();power(&mut b);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x36,&[0xa0]);cmd(&mut b,0x3a,&[0x55]);
        for (x,y,color) in [(0u16,0u16,0xf800u16),(319,0,0x07e0),(0,169,0x001f),(319,169,0xffff)] {
            let x=x.to_be_bytes();let y=(35+y).to_be_bytes();cmd(&mut b,0x2a,&[x[0],x[1],x[0],x[1]]);cmd(&mut b,0x2b,&[y[0],y[1],y[0],y[1]]);cmd(&mut b,0x2c,&color.to_be_bytes());
        }
        let (w,h,pixels,_)=b.display().unwrap();assert_eq!((w,h),(170,320));assert_eq!([pixels[319*170],pixels[0],pixels[320*170-1],pixels[169]],[0xf800,0x07e0,0x001f,0xffff]);
        cmd(&mut b,0x28,&[]);assert!(b.display().unwrap().2.iter().all(|&p|p==0));cmd(&mut b,0x29,&[]);cmd(&mut b,0x10,&[]);assert!(b.display().unwrap().2.iter().all(|&p|p==0));
    }
    #[test]
    fn wrong_routes_and_dc_polarity_cannot_initialize_the_panel() {
        let mut b=TDisplayS3::new();power(&mut b);
        for route in [LcdI80Pins{clock:1<<10,..pins()},LcdI80Pins{select:1<<10,..pins()},LcdI80Pins{dc:1<<10,..pins()},LcdI80Pins{width:16,..pins()},LcdI80Pins{data:[0;16],..pins()},LcdI80Pins{dc_high:true,..pins()}] {b.lcd_i80(route,false,&[0x11]);assert!(b.panel.sleeping);}
        cmd(&mut b,0x11,&[]);assert!(!b.panel.sleeping);
        drive_state(&mut b,0,!(1<<5));b.lcd_i80(pins(),false,&[0x11]);assert!(b.panel.sleeping);
        power(&mut b);drive_state(&mut b,0,!(1<<9));b.lcd_i80(pins(),false,&[0x11]);assert!(b.panel.sleeping);
        assert_eq!(b.input_levels(),[(0,true),(14,true)]);assert_eq!(b.named_pin("user"),Some(14));assert!(b.i2c_devices().is_empty());
    }
    #[test]
    fn backlight_is_driven_and_usb_supply_does_not_depend_on_battery_gate() {
        let mut b=TDisplayS3::new();power(&mut b);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);b.panel.gram[35]=0xffff;
        assert_eq!(b.display().unwrap().2[0],0xffff);let version=b.display_version();drive_state(&mut b,0,!(1<<38));assert!(b.display_version()>version);assert_eq!(b.display().unwrap().2[0],0);
        power(&mut b);drive_state(&mut b,1<<38,u64::MAX);assert_eq!(b.display().unwrap().2[0],0,"a floating output latch is not backlight drive");
        power(&mut b);drive_state(&mut b,0,!(1<<15));assert_eq!(b.display().unwrap().2[0],0xffff,"manufacturer pin note: G15 gates battery supply, USB is present here");
    }
    #[test]
    fn gpio_i80_requires_wr_edge_selected_bus_and_each_software_route() {
        let mut b=TDisplayS3::new();let mut gpio=esp_periph::Gpio::new();let mut mux=esp_periph::RegRam::new();
        for p in [5,6,7,8,9,15,38,39,40,41,42,45,46,47,48] {gpio.enable|=1<<p;mux.write(4+4*p,1<<12);}
        gpio.out=(1<<5)|(1<<9)|(1<<15)|(1<<38)|(1<<39)|(1<<45); // command 0x11
        let sample=|b:&mut TDisplayS3,g:&esp_periph::Gpio,m:&esp_periph::RegRam|b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(g,m));
        sample(&mut b,&gpio,&mux);assert!(b.panel.sleeping,"WR low is not a transaction");
        gpio.out|=1<<8;sample(&mut b,&gpio,&mux);assert!(!b.panel.sleeping);assert_eq!(b.gpio_bytes,1);
        sample(&mut b,&gpio,&mux);assert_eq!(b.gpio_bytes,1,"WR held high is not another byte");
        for bad in 0..6 {
            b.panel.reset();gpio.out&=!((1<<8)|(1<<6));gpio.enable|=(1<<45)|(1<<9);gpio.func_out_sel[45]=256;mux.write(4+4*45,1<<12);
            match bad {0=>gpio.out|=1<<6,1=>gpio.enable&=!(1<<45),2=>gpio.func_out_sel[45]=138,3=>mux.write(4+4*45,0),4=>gpio.enable&=!(1<<9),_=>gpio.out|=1<<7}
            sample(&mut b,&gpio,&mux);gpio.out|=1<<8;sample(&mut b,&gpio,&mux);assert!(b.panel.sleeping,"bad route/gate {bad}");
        }
        assert!(!b.uses_gpio_waveform(),"write edges use existing callbacks, no instruction-sized scheduler");
    }

}
