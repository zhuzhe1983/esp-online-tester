//! M5Stack AtomS3 C123, GC9107 variant. Pinmap: docs.m5stack.com/en/core/AtomS3.
//! M5GFX 65c816a3: 128x160 GRAM, y offset 32, ID 00 91 07. M5Unified
//! 368ad608: active-low G41 button, MPU6886 at 0x68 on SDA38/SCL39.
//! Sources, measured scope and exclusions: docs/platform/atoms3-runtime.md.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;
use esp_periph::i2c::mpu6886::{Mpu6886,Mpu6886Sample};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang};

pub struct AtomS3 {panel:DcsPanel,command:u8,read_at:usize,reset_high:bool,display_changes:u64,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,imu:Arc<Mutex<Mpu6886Sample>>,gpio_events:u64}
impl AtomS3 {pub fn new()->Self {Self{panel:DcsPanel::new(128,160),command:0,read_at:0,reset_high:true,display_changes:0,i2c:I2cBitBang::new(),levels:[true,true],edges:Vec::new(),imu:Default::default(),gpio_events:0}}}
impl Default for AtomS3 {fn default()->Self {Self::new()}}
impl BoardModel for AtomS3 {
    fn name(&self)->&'static str {"atoms3"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {
        self.gpio_events+=changes.len() as u64;
        for &(pin,high) in changes {match pin {33=>self.panel.dc=high,34=>{if self.reset_high && !high {self.panel.reset();self.command=0;self.read_at=0;self.display_changes+=1;}self.reset_high=high;},_=>{}}}
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_changes(changes);let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(39);let sda=self.i2c.drive(scl,released(38));
        for (index,(pin,level)) in [(38,sda),(39,scl)].into_iter().enumerate() {if level!=self.levels[index] {self.levels[index]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        if !self.reset_high || pins.sclk&(1<<17)==0 || pins.cs&(1<<15)==0 || (pins.mosi&(1<<21)==0 && pins.miso!=Some(21)) {return vec![0xff;rx_len];}
        for &b in tx {if !self.panel.dc {self.command=b;self.read_at=0;self.display_changes+=1;}self.panel.byte(b);}
        (0..rx_len).map(|_| {let value=match self.command {0x04=>[0x00,0x91,0x07].get(self.read_at).copied().unwrap_or(0),0xda=>0,0xdb=>0x91,0xdc=>7,_=>0xff};self.read_at+=1;value}).collect()
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x68,Box::new(Mpu6886::new(self.imu.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((38,39))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(41,true),(38,self.levels[0]),(39,self.levels[1])]}
    fn named_pin(&self,name:&str)->Option<u8> {match name {"btnA"|"button"|"g41"=>Some(41),"boot"|"g0"=>Some(0),_=>None}}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let pixels=if self.panel.on && !self.panel.sleeping {self.panel.gram[32*128..160*128].to_vec()} else {vec![0;128*128]};
        Some((128,128,pixels,self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written+self.panel.resets+self.display_changes}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),128,160))}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        let mut sample=self.imu.lock().expect("AtomS3 IMU sample mutex poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn report(&self)->String {format!("[atoms3] GC9107 frames={} pixels={} MPU6886=polled-subset",self.panel.frames,self.panel.pixels_written)}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<17,mosi:1<<21,cs:1<<15,..Default::default()}}
    fn cmd(b:&mut AtomS3,c:u8,data:&[u8]) {b.gpio_changes(&[(33,false)]);b.spi_transfer_pins(3,pins(),&[c],0);b.gpio_changes(&[(33,true)]);b.spi_transfer_pins(3,pins(),data,0);}
    #[test]
    fn gc9107_physical_route_id_window_rotation_and_reset() {
        let mut b=AtomS3::new();assert_eq!(b.spi_transfer_pins(2,pins(),&[4],3),[0,0x91,7]);assert_eq!(b.spi_transfer_pins(3,SpiPins{sclk:1<<18,..pins()},&[4],3),[0xff;3]);
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x2a,&[0,0,0,1]);cmd(&mut b,0x2b,&[0,32,0,32]);cmd(&mut b,0x2c,&[0xf8,0,0x07,0xe0]);
        assert_eq!(&b.display().unwrap().2[..2],&[0xf800,0x07e0]);
        cmd(&mut b,0x36,&[0x60]);cmd(&mut b,0x2a,&[0,32,0,32]);cmd(&mut b,0x2b,&[0,0,0,0]);cmd(&mut b,0x2c,&[0,0x1f]);assert_eq!(b.display().unwrap().2[127],0x001f);
        let version=b.display_version();cmd(&mut b,0x28,&[]);assert!(b.display_version()>version);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        b.gpio_changes(&[(34,false)]);assert_eq!(b.spi_transfer_pins(3,pins(),&[4],3),[0xff;3]);b.gpio_changes(&[(34,true),(33,false)]);assert_eq!(b.spi_transfer_pins(3,pins(),&[4],3),[0,0x91,7]);
    }
    #[test]
    fn button_and_shared_imu_are_on_documented_pins() {
        let mut b=AtomS3::new();assert_eq!(b.named_pin("btnA"),Some(41));assert!(b.input_levels().contains(&(41,true)));
        let mut devices=b.i2c_devices();assert_eq!(devices.len(),2);assert!(devices.iter().all(|(_,a,d)|*a==0x68 && d.pins()==Some((38,39))));
        let d=devices[0].2.as_mut();d.start(false);d.write(0x6b);d.write(1);assert!(b.set_imu_sample([1.0,-1.0,0.0],[0.0;3]));d.start(false);d.write(0x3b);d.start(true);assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(),[0x40,0,0xc0,0,0,0]);
        let d=devices[1].2.as_mut();d.start(false);d.write(0x6b);d.start(true);assert_eq!(d.read(),1,"both controllers share sensor state");assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));
    }
}
