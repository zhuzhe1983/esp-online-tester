//! M5Stack AtomS3R C126, ST7735 batch since 2026-05-14. See docs/platform/atoms3r-runtime.md.
//! M5GFX 65c816a3: 132x132 memory, offsets (2,1), rotation 2. LP5562 W at 0x30.
//! K147 GC9107 batch: same fixed M5GFX revision, ID 00/91/07, 128x160 GRAM with y=32 visible origin.
//! See docs/platform/atoms3r-chatbot-runtime.md; Atomic Voice Base audio is not emulated.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_periph::i2c::lp5562::{Lp5562,Lp5562State};
use esp_periph::i2c::bmi270::{Bmi270,Bmi270Sample};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};

pub struct AtomS3R {gc9107:bool,panel:DcsPanel,command:u8,read_at:usize,reset_high:bool,display_changes:u64,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,imu:Arc<Mutex<Bmi270Sample>>,light:Arc<Mutex<Lp5562State>>,gpio_events:u64}
impl AtomS3R {
    pub fn new()->Self {Self::with_gc9107(false)}
    /// K147 documented GC9107 batch; the audio base is not a simulated codec.
    pub fn chatbot()->Self {Self::with_gc9107(true)}
    fn with_gc9107(gc9107:bool)->Self {Self{gc9107,panel:if gc9107 {DcsPanel::new(128,160)} else {DcsPanel::new(132,132)},command:0,read_at:0,reset_high:true,display_changes:0,i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),imu:Default::default(),light:Default::default(),gpio_events:0}}}
impl Default for AtomS3R {fn default()->Self {Self::new()}}
impl BoardModel for AtomS3R {
    fn name(&self)->&'static str {if self.gc9107 {"atoms3r-chatbot"} else {"atoms3r"}}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {
        self.gpio_events+=changes.len() as u64;
        for &(pin,high) in changes {match pin {42=>self.panel.dc=high,48=>{if self.reset_high && !high {self.panel.reset();self.command=0;self.read_at=0;self.display_changes+=1;}self.reset_high=high;},_=>{}}}
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(0);let sda=self.i2c.drive(scl,released(45));
        for (index,(pin,level)) in [(45,sda),(0,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if !self.reset_high || pins.sclk&(1<<15)==0 || pins.cs&(1<<14)==0 || (pins.mosi&(1<<21)==0 && pins.miso!=Some(21)) {return vec![0xff;rx_len];}
        for &b in tx {if !self.panel.dc {self.command=b;self.read_at=0;self.display_changes+=1;}self.panel.byte(b);}
        let id=if self.gc9107 {[0x00,0x91,0x07]} else {[0x83,0x76,0x00]};
        (0..rx_len).map(|_| {let value=match self.command {0x04=>id.get(self.read_at).copied().unwrap_or(0),0xda=>id[0],0xdb=>id[1],0xdc=>id[2],_=>0xff};self.read_at+=1;value}).collect()
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x68,Box::new(Bmi270::new(self.imu.clone())));self.i2c.attach(0x30,Box::new(Lp5562::new(self.light.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((45,0))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(41,true),(45,self.levels[0]),(0,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"btnA"|"button"|"g41"=>Some(41),"boot"|"g0"=>Some(0),_=>None}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let pwm=self.light.lock().expect("AtomS3R backlight mutex poisoned").white_pwm() as u32;
        let mut pixels=vec![0;128*128];
        if self.panel.on && !self.panel.sleeping && pwm!=0 {for y in 0..128 {for x in 0..128 {
            let p=self.panel.gram[if self.gc9107 {(y+32)*128+x} else {(128-y)*132+129-x}] as u32;
            pixels[y*128+x]=((((p>>11)&31)*pwm/255)<<11 | (((p>>5)&63)*pwm/255)<<5 | ((p&31)*pwm/255)) as u16;
        }}}
        Some((128,128,pixels,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes+self.light.lock().expect("AtomS3R backlight mutex poisoned").version}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),if self.gc9107 {128} else {132},if self.gc9107 {160} else {132}))}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        let mut sample=self.imu.lock().expect("AtomS3R IMU sample mutex poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn report(&self)->String {if self.gc9107 {format!("[atoms3r-chatbot] GC9107 frames={} pixels={} BMI270=polled-subset audio=unimplemented",self.panel.frames,self.panel.pixels_written)} else {format!("[atoms3r] ST7735 frames={} pixels={} BMI270=polled-subset",self.panel.frames,self.panel.pixels_written)}}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<15,mosi:1<<21,cs:1<<14,..Default::default()}}
    fn cmd(b:&mut AtomS3R,c:u8,data:&[u8]) {b.gpio_changes(&[(42,false)]);b.spi_transfer_pins(3,pins(),&[c],0);b.gpio_changes(&[(42,true)]);b.spi_transfer_pins(3,pins(),data,0);}
    fn wr(d:&mut dyn I2cDevice,r:u8,v:u8) {d.start(false);d.write(r);d.write(v);}
    #[test]
    fn st7735_window_reset_and_i2c_backlight() {
        let mut b=AtomS3R::new();assert_eq!(b.spi_transfer_pins(2,pins(),&[4],4),[0x83,0x76,0,0]);
        assert_eq!(b.spi_transfer_pins(3,SpiPins{sclk:1<<17,..pins()},&[4],3),[0xff;3]);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[0xdc]);
        cmd(&mut b,0x2a,&[0,2,0,3]);cmd(&mut b,0x2b,&[0,3,0,3]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);
        assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        let mut devices=b.i2c_devices();let d=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x30).unwrap().2.as_mut();
        for (r,v) in [(0,0x40),(8,1),(0x70,0),(14,255)] {wr(d,r,v);}
        assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);
        let before=b.display_version();wr(d,14,128);assert!(b.display_version()>before);assert_eq!(&b.display().unwrap().2[..2],&[0x7800,0x03e0]);
        wr(d,0,0);assert!(b.display().unwrap().2.iter().all(|p|*p==0));wr(d,0,0x40);
        b.gpio_changes(&[(48,false)]);assert_eq!(b.spi_transfer_pins(3,pins(),&[4],3),[0xff;3]);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
    }
    #[test]
    fn chatbot_gc9107_uses_its_own_window_and_no_audio_ack() {
        let mut b=AtomS3R::chatbot();assert_eq!(b.name(),"atoms3r-chatbot");
        assert_eq!(b.spi_transfer_pins(3,pins(),&[4],3),[0,0x91,7]);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);
        cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,32,0,32]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);
        let mut devices=b.i2c_devices();assert!(devices.iter().all(|(_,a,_)|*a!=0x18));
        let d=devices.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x30).unwrap().2.as_mut();
        for (r,v) in [(0,0x40),(8,1),(0x70,0),(14,255)] {wr(d,r,v);}
        assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);
        assert_eq!(b.gram().unwrap().1,128);assert_eq!(b.gram().unwrap().2,160);
        let mut efuse=esp_periph::Efuse::new([0;6]);crate::board::configure_chip_identity(b.name(),&mut efuse);assert_eq!((efuse.ram.read(0x50)>>21)&7,1);
    }
    #[test]
    fn distinct_pins_bmi270_and_package_identity() {
        let mut b=AtomS3R::new();assert_eq!(b.named_pin("button"),Some(41));assert!(b.input_levels().contains(&(41,true)));
        let devices=b.i2c_devices();assert_eq!(devices.len(),4);assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((45,0))));assert!(devices.iter().all(|(_,a,_)|*a!=0x10),"BMM150 is behind the unimplemented AUX bridge");
        assert!(b.set_imu_sample([1.0,0.0,0.0],[125.0,0.0,0.0]));assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));
        let mut efuse=esp_periph::Efuse::new([0;6]);let before=efuse.ram.read(0x50);crate::board::configure_chip_identity("atoms3",&mut efuse);assert_eq!(efuse.ram.read(0x50),before);
        crate::board::configure_chip_identity(b.name(),&mut efuse);assert_eq!((efuse.ram.read(0x50)>>21)&7,1);assert_eq!(efuse.ram.read(0x50)&!(7<<21),before&!(7<<21));
    }
}
