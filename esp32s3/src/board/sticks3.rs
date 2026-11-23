//! M5Stack StickS3 K150 with ESP32-S3-PICO-1-N8R8.
//! Official StickS3 pinmap and M5GFX 65c816a3, lines 2995..3063.
//! LCD gate is PM1 GPIO2 (L3B). See docs/platform/sticks3-runtime.md.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,Reg8Device,bmi270::{Bmi270,Bmi270Sample},m5::{M5pm1,M5pm1State}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};

struct Lcd {panel:DcsPanel,powered:bool,reset:bool,backlight:bool,pwm:u32,changes:u64}
struct Wiring {lcd:Mutex<Lcd>,pmic:Arc<Mutex<M5pm1State>>}
impl Wiring {
    fn sync(&self) {
        let powered={let p=self.pmic.lock().expect("StickS3 PM1 poisoned");!p.power_off_requested && p.driven_high&4!=0};
        let mut l=self.lcd.lock().expect("StickS3 LCD poisoned");
        if l.powered!=powered {if !powered {l.panel.reset();l.panel.gram.fill(0);}l.powered=powered;l.changes+=1;}
    }
}
struct PowerDevice {base:M5pm1,wiring:Arc<Wiring>}
impl I2cDevice for PowerDevice {
    fn start(&mut self,read:bool)->bool {self.base.start(read)}
    fn start_address_with_clock(&mut self,c:u8,a:u8,r:bool,hz:Option<u32>)->bool {self.base.start_address_with_clock(c,a,r,hz)}
    fn write(&mut self,b:u8)->bool {let ack=self.base.write(b);self.wiring.sync();ack}
    fn read(&mut self)->u8 {self.base.read()}
    fn stop(&mut self) {self.base.stop();}
}
pub struct StickS3 {wiring:Arc<Wiring>,imu:Arc<Mutex<Bmi270Sample>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,gpio_events:u64}
impl StickS3 {
    pub fn new()->Self {Self{wiring:Arc::new(Wiring{lcd:Mutex::new(Lcd{panel:DcsPanel::st7789(),powered:false,reset:false,backlight:false,pwm:0,changes:0}),pmic:Default::default()}),imu:Default::default(),i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),gpio_events:0}}
}
impl Default for StickS3 {fn default()->Self {Self::new()}}
impl BoardModel for StickS3 {
    fn name(&self)->&'static str {"sticks3"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_events+=changes.len() as u64;
        let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(48);let sda=self.i2c.drive(scl,released(47));
        for (i,(pin,level)) in [(47,sda),(48,scl)].into_iter().enumerate() {if level!=self.levels[i] {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn uses_gpio_routes(&self)->bool {true}
    fn gpio_waveform_at(&mut self,_cycle:VirtualCycle,routes:esp_soc::pins::PinRoutes<'_>) {
        let mut l=self.wiring.lcd.lock().expect("StickS3 LCD poisoned");
        let reset=routes.software_output(21)==Some(true);let light=routes.software_output(38)==Some(true);
        if l.reset && !reset {l.panel.reset();l.changes+=1;}
        if l.backlight!=light {l.changes+=1;}l.reset=reset;l.backlight=light;l.panel.dc=routes.software_output(45)==Some(true);
    }
    fn uses_gpio_pwm(&self)->bool {true}
    fn gpio_pwm_at(&mut self,_cycle:VirtualCycle,pwm:&dyn Fn(u8)->Option<(f64,u32)>) {
        let duty=pwm(38).map_or(0,|(_,d)|d.min(65535));let mut l=self.wiring.lcd.lock().expect("StickS3 LCD poisoned");if l.pwm!=duty {l.pwm=duty;l.changes+=1;}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        self.wiring.sync();let mut l=self.wiring.lcd.lock().expect("StickS3 LCD poisoned");
        if l.powered && l.reset && pins.sclk&(1<<40)!=0 && pins.cs&(1<<41)!=0 && (pins.mosi&(1<<39)!=0 || pins.miso==Some(39)) {
            // D/C distinguishes commands and data across both held-CS and separate transfers.
            for &b in tx {l.panel.byte(b);}l.changes+=tx.len() as u64;
        }
        // No verified panel readback capture: do not invent a batch-specific chip ID.
        vec![0xff;rx_len]
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x6e,Box::new(PowerDevice{base:M5pm1::new(self.wiring.pmic.clone()),wiring:self.wiring.clone()}));
        self.i2c.attach(0x68,Box::new(Bmi270::new(self.imu.clone())));
        self.i2c.attach(0x18,Box::new(Reg8Device::new("es8311-initialization-subset",&[(0xfd,0x83),(0xfe,0x11)])));
        self.wiring.sync();
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((47,48))).into_iter().map(move |(a,d)|(bus,a,d))).collect()
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(11,true),(12,true),(47,self.levels[0]),(48,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"btnA"|"key1"|"g11"=>Some(11),"btnB"|"key2"|"g12"=>Some(12),_=>None}}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        let mut s=self.imu.lock().expect("StickS3 IMU poisoned");s.accel_g=accel_g;s.gyro_dps=gyro_dps;s.version=s.version.wrapping_add(1);true
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let l=self.wiring.lcd.lock().expect("StickS3 LCD poisoned");let mut frame=vec![0;135*240];let duty=if l.backlight {65535}else{l.pwm};
        // M5GFX panel 135x240, offset (52,40), rotation 0. DCS owns MADCTL.
        if l.powered && l.reset && l.panel.on && !l.panel.sleeping && duty!=0 {
            for (y,line) in frame.chunks_mut(135).enumerate() {line.copy_from_slice(&l.panel.gram[(y+40)*240+52..(y+40)*240+187]);}
            if duty<65535 {for p in &mut frame {let v=*p as u32;*p=(((((v>>11)&31)*duty/65535)<<11)|((((v>>5)&63)*duty/65535)<<5)|((v&31)*duty/65535)) as u16;}}
        }
        Some((135,240,frame,l.changes))
    }
    fn display_version(&self)->u64 {let l=self.wiring.lcd.lock().expect("StickS3 LCD poisoned");l.changes}
    fn display_frames(&self)->u64 {self.wiring.lcd.lock().expect("StickS3 LCD poisoned").panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.wiring.lcd.lock().expect("StickS3 LCD poisoned").panel.gram.clone(),240,320))}
    fn report(&self)->String {let l=self.wiring.lcd.lock().expect("StickS3 LCD poisoned");format!("[sticks3] ST7789P3 frames={} pixels={} L3B={} reset={} BMI270=polled-subset ES8311=register-init-only audio-route/PM1-power-key/IR=unimplemented",l.panel.frames,l.panel.pixels_written,l.powered,l.reset)}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<40,mosi:1<<39,cs:1<<41,..Default::default()}}
    fn gpio(b:&mut StickS3,dc:bool,light:bool,reset:bool) {
        let mut g=esp_periph::Gpio::new();let mut m=esp_periph::RegRam::new();
        for (p,v) in [(21,reset),(38,light),(45,dc)] {g.enable|=1<<p;if v {g.out|=1<<p;}m.write(4+4*p,1<<12);}
        b.gpio_waveform_at(0,esp_soc::pins::ChipPins::S3.routes(&g,&m));
    }
    fn cmd(b:&mut StickS3,c:u8,data:&[u8]) {
        gpio(b,false,true,true);b.spi_transfer_pins(3,SpiPins{keep_cs:!data.is_empty(),..pins()},&[c],0);
        if !data.is_empty() {gpio(b,true,true,true);b.spi_transfer_pins(3,pins(),data,0);}
    }
    fn wr(d:&mut dyn I2cDevice,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));d.stop();}
    fn rd(d:&mut dyn I2cDevice,r:u8)->u8 {assert!(d.start(false));d.write(r);assert!(d.start(true));d.read()}
    fn power(b:&mut StickS3)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let mut ds=b.i2c_devices();let pm=ds.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x6e).unwrap().2.as_mut();
        for (r,v) in [(0x16,0),(0x10,4),(0x13,0x1b),(0x11,4)] {wr(pm,r,v);}gpio(b,false,true,true);ds
    }
    #[test]
    fn physical_window_corners_wrong_spi_and_reset() {
        let mut b=StickS3::new();let _ds=power(&mut b);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);
        for (x,y,c) in [(52u16,40u16,0xf800u16),(186,40,0x07e0),(52,279,0x001f),(186,279,0xffff)] {let x=x.to_be_bytes();let y=y.to_be_bytes();cmd(&mut b,0x2a,&[x[0],x[1],x[0],x[1]]);cmd(&mut b,0x2b,&[y[0],y[1],y[0],y[1]]);cmd(&mut b,0x2c,&c.to_be_bytes());}
        let (w,h,p,_) = b.display().unwrap();assert_eq!((w,h),(135,240));assert_eq!([p[0],p[134],p[239*135],p[135*240-1]],[0xf800,0x07e0,0x001f,0xffff]);
        for bad in [SpiPins{sclk:1<<15,..pins()},SpiPins{mosi:1<<35,..pins()},SpiPins{cs:1<<37,..pins()}] {gpio(&mut b,false,true,true);b.spi_transfer_pins(2,bad,&[0x28],0);assert!(b.wiring.lcd.lock().unwrap().panel.on);}
        cmd(&mut b,0x28,&[]);assert!(b.display().unwrap().2.iter().all(|&p|p==0));cmd(&mut b,0x29,&[]);cmd(&mut b,0x10,&[]);assert!(b.display().unwrap().2.iter().all(|&p|p==0));cmd(&mut b,0x11,&[]);
        gpio(&mut b,false,true,false);assert!(b.wiring.lcd.lock().unwrap().panel.sleeping);
    }
    #[test]
    fn pm1_gate_backlight_and_reset_do_not_replay_old_gram() {
        let mut b=StickS3::new();cmd(&mut b,0x11,&[]);assert!(b.wiring.lcd.lock().unwrap().panel.sleeping,"PM1 GPIO2 defaults off");
        let mut ds=power(&mut b);cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);b.wiring.lcd.lock().unwrap().panel.gram[40*240+52]=0xffff;
        assert_eq!(b.display().unwrap().2[0],0xffff);gpio(&mut b,true,false,true);assert_eq!(b.display().unwrap().2[0],0);b.gpio_pwm_at(0,&|p|if p==38{Some((1000.0,32768))}else{None});assert_eq!(b.display().unwrap().2[0],0x7bef);
        let pm=ds.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x6e).unwrap().2.as_mut();let before=b.display_version();wr(pm,0x11,0);assert!(b.display_version()>before);assert!(b.display().unwrap().2.iter().all(|&p|p==0));wr(pm,0x11,4);assert!(b.wiring.lcd.lock().unwrap().panel.sleeping);assert!(b.wiring.lcd.lock().unwrap().panel.gram.iter().all(|&p|p==0));
    }
    #[test]
    fn only_sticks3_devices_routes_buttons_and_imu_registers() {
        let mut b=StickS3::new();let mut ds=power(&mut b);assert_eq!(ds.len(),6);assert!(ds.iter().all(|(_,a,d)|[0x18,0x68,0x6e].contains(a)&&d.pins()==Some((47,48))));
        assert_eq!(b.named_pin("btnA"),Some(11));assert_eq!(b.named_pin("btnB"),Some(12));assert_eq!(b.named_pin("power"),None);assert!(b.input_levels().contains(&(11,true)));assert!(b.input_levels().contains(&(12,true)));
        assert!(b.set_imu_sample([1.0,0.0,0.0],[0.0;3]));assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));
        let imu=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x68).unwrap().2.as_mut();assert_eq!(rd(imu,0),0x24);assert_eq!(rd(imu,0x0d),0,"disabled until full config");
        wr(imu,0x59,0);imu.start(false);imu.write(0x5e);for _ in 0..8192 {imu.write(0);}imu.stop();wr(imu,0x59,1);wr(imu,0x7d,4);assert_eq!(rd(imu,0x0d),0x10,"1 g at default 8 g full scale");
        wr(imu,0x7d,0);assert_eq!(rd(imu,0x0d),0);
        let codec=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x18).unwrap().2.as_mut();assert_eq!(rd(codec,0xfd),0x83);assert_eq!(rd(codec,0xfe),0x11);
        assert_eq!(b.audio(),(&[][..],0));let old=b.display_version();b.touch_at(0,1,1,true);assert_eq!(b.display_version(),old);
    }
}
