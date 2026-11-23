//! M5StickC K016-C with the ESP32-PICO-D4 and an 80×160 ST7735SV LCD.
//! Wiring and orientation follow the StickC product page and M5StickC
//! 990489188ec40c2bd4611cc3f3f2ac7c3817024a (In_eSPI_Setup.h, ST7735_Rotation.h).
//! Internal I2C SDA21/SCL22: AXP192 0x34, MPU6886 0x68, BM8563 0x51.
//! This configuration covers the MPU6886 hardware revision.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,axp192::{Axp192,Axp192State},mpu6886::{Mpu6886,Mpu6886Sample}};
use esp_soc::board::{BoardModel,SpiPins};
use esp_soc::devices::{DcsPanel,i2c_bitbang::I2cBitBang,bm8563::{Bm8563,Bm8563State}};

pub struct StickC {
    panel: DcsPanel,
    pub pmic: Arc<Mutex<Axp192State>>,
    imu: Arc<Mutex<Mpu6886Sample>>,
    rtc: Arc<Mutex<Bm8563State>>,
    cycle: u64,
    cs_high: bool,
    changes: u64,
    gpio_events: u64,
    led: [[u8;3];1],
    led_updates: u64,
    spi_bytes: u64,
    dropped: u64,
}
impl Default for StickC {fn default()->Self {Self::new()}}
impl StickC {
    pub fn new() -> Self {
        Self {
            panel: DcsPanel::st7735(),
            pmic: Arc::new(Mutex::new(Axp192State::stickc())),
            imu: Default::default(),
            rtc: Arc::new(Mutex::new(Bm8563State::new(crate::periph::CPU_HZ))),
            cycle: 0,
            cs_high: true,
            changes: 0,
            gpio_events: 0,
            led: [[0;3]],
            led_updates: 0,
            spi_bytes: 0,
            dropped: 0,
        }
    }
}
impl BoardModel for StickC {
    fn name(&self)->&'static str {"stickc"}
    fn uses_spi_pins(&self)->bool {true}
    fn gpio_changes(&mut self, changes: &[(u8,bool)]) {
        for &(pin, high) in changes {
            self.gpio_events += 1;
            match pin {
                5 => self.cs_high = high,
                23 => self.panel.dc = high,
                18 if !high => {
                    self.panel.reset();
                    self.changes = self.changes.wrapping_add(1);
                }
                10 => {
                    let color = if high { [0;3] } else { [255,0,0] };
                    if self.led[0] != color {
                        self.led[0] = color;
                        self.led_updates = self.led_updates.wrapping_add(1);
                    }
                }
                _ => {}
            }
        }
    }
    fn spi_transfer_pins(&mut self, _host: u8, pins: SpiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if !self.cs_high
            && pins.sclk & (1 << 13) != 0
            && pins.mosi & (1 << 15) != 0
            && self.pmic.lock().expect("StickC PMIC poisoned").ldo_enabled(3)
        {
            let before = (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets);
            for &byte in tx { self.panel.byte(byte); }
            self.spi_bytes += tx.len() as u64;
            if before != (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets) {
                self.changes = self.changes.wrapping_add(1);
            }
        } else {
            self.dropped += tx.len() as u64;
        }
        vec![0xff;rx_len]
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(37,true),(39,true),(21,true),(22,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button-a"|"btnA"|"a"=>Some(37),"button-b"|"btnB"|"b"=>Some(39),_=>None}}
    fn button_at(&mut self,_cycle:u64,id:&str,pressed:bool)->bool {if id!="power" {return false;}self.pmic.lock().expect("StickC PMIC poisoned").set_key(pressed);true}
    fn i2c_devices(&mut self) -> Vec<(u8,u8,Box<dyn I2cDevice>)> {
        let mut shared = I2cBitBang::new();
        shared.attach(0x34, Box::new(Axp192::new(self.pmic.clone())));
        shared.attach(0x68, Box::new(Mpu6886::new(self.imu.clone())));
        shared.attach(0x51, Box::new(Bm8563::new(self.rtc.clone())));
        (0..2).flat_map(|bus| shared.controller_devices(Some((21,22)))
            .into_iter().map(move |(address, device)| (bus, address, device))).collect()
    }
    fn next_deadline(&self)->Option<u64> {Some(self.cycle.saturating_add(crate::periph::CPU_HZ))}
    fn advance_to(&mut self,cycle:u64) {self.cycle=cycle;self.rtc.lock().expect("StickC RTC poisoned").advance_to(cycle);self.pmic.lock().expect("StickC PMIC poisoned").advance_to_us(cycle/(crate::periph::CPU_HZ/1_000_000));}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}let mut s=self.imu.lock().expect("StickC IMU poisoned");s.accel_g=accel_g;s.gyro_dps=gyro_dps;s.version=s.version.wrapping_add(1);true}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.led,self.led_updates))}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {
        let panel = &self.panel;
        let power = self.pmic.lock().expect("StickC PMIC poisoned");
        let blank = !panel.on || panel.sleeping || !power.ldo_enabled(3) || !power.ldo_enabled(2);
        let exchange = panel.madctl & 8 == 0;
        let mut pixels = Vec::with_capacity(80 * 160);
        // GREENTAB160x80: glass RAM columns 26..105, rows 1..160.
        // Native upright rotation0 uses MX|MY|MH|BGR (0xc8/0xcc).
        for y in 0..160 {
            for x in 0..80 {
                let mut color = panel.gram[(160 - y) * panel.cols + 105 - x];
                if exchange {
                    color = (color & 0x07e0) | ((color & 0xf800) >> 11) | ((color & 0x001f) << 11);
                }
                pixels.push(if blank { 0 } else { color });
            }
        }
        drop(power);
        Some((80, 160, pixels, self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.changes).wrapping_add(self.pmic.lock().expect("StickC PMIC poisoned").version)}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),self.panel.cols,self.panel.rows))}
    fn report(&self)->String {format!("[stickc] ST7735SV frames={} pixels={} spi={} dropped={} AXP192=polled MPU6886=polled BM8563=calendar; mic=converted-PCM; IR/raw-PDM/SH200Q/analog-power/long-PEK unsupported",self.panel.frames,self.panel.pixels_written,self.spi_bytes,self.dropped)}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn routes()->SpiPins {SpiPins{sclk:1<<13,mosi:1<<15,miso:None,cs:1<<5,keep_cs:false}}
    fn cmd(b:&mut StickC,c:u8,data:&[u8]) {b.gpio_changes(&[(5,false),(23,false)]);b.spi_transfer_pins(2,routes(),&[c],0);b.gpio_changes(&[(23,true)]);b.spi_transfer_pins(2,routes(),data,0);b.gpio_changes(&[(5,true)]);}
    fn wr(d:&mut dyn I2cDevice,r:u8,v:u8) {d.start(false);d.write(r);d.write(v);d.stop();}
    #[test]
    fn real_spi_commands_reach_cropped_glass_and_ldo_controls_output() {
        let mut b=StickC::new();cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[5]);cmd(&mut b,0x36,&[0xc8]);
        cmd(&mut b,0x2a,&[0,26,0,27]);cmd(&mut b,0x2b,&[0,1,0,1]);cmd(&mut b,0x2c,&[0xf8,0,7,0xe0]);let(w,h,p,_)=b.display().unwrap();assert_eq!((w,h),(80,160));assert_eq!(&p[..2],&[0xf800,0x07e0]);assert_eq!(p.iter().filter(|&&v|v!=0).count(),2);
        let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==0 && *a==0x34).unwrap().2.as_mut();wr(d,0x12,0x49);assert!(b.display().unwrap().2.iter().all(|&v|v==0));wr(d,0x12,0x4d);assert_eq!(b.display().unwrap().2[0],0xf800);
        // Wrong route and CS high are rejected, not rendered from frontend state.
        b.gpio_changes(&[(5,false)]);let mut bad=routes();bad.mosi=1<<19;b.spi_transfer_pins(2,bad,&[0xff,0xff],0);b.gpio_changes(&[(5,true)]);b.spi_transfer_pins(2,routes(),&[0xff,0xff],0);assert_eq!(b.display().unwrap().2[0],0xf800);assert_eq!(b.dropped,4);
    }
    #[test]
    fn guest_cycles_drive_pmic_sampling_gate_and_powered_fixture_reset() {
        let mut b=StickC::new();let tick=crate::periph::CPU_HZ/1_000_000;
        let mut ds=b.i2c_devices();let d=ds.iter_mut().find(|(bus,a,_)|*bus==1 && *a==0x34).unwrap().2.as_mut();
        let read=|d:&mut dyn I2cDevice,r:u8| {d.start(false);d.write(r);d.start(true);let v=d.read();d.stop();v};
        // Default 25 Hz conversion is driven only by guest cycles, including floor-to-us.
        b.advance_to(40_000*tick-1);assert_eq!(read(d,0x5e),0);assert_eq!(read(d,0x78),0);
        b.advance_to(40_000*tick);assert_eq!((read(d,0x5e),read(d,0x5f)),(0x6a,1));assert_eq!((read(d,0x78),read(d,0x79)),(0xe8,15));
        wr(d,0x83,0);b.pmic.lock().unwrap().set_chip_temperature_mc(0);b.advance_to(80_000*tick);
        assert_eq!((read(d,0x5e),read(d,0x5f)),(0x6a,1));
        wr(d,0x83,0x80);b.advance_to(120_000*tick);assert_eq!((read(d,0x5e),read(d,0x5f)),(0x5a,7));
        b.pmic.lock().unwrap().reset_powered_fixture();b.advance_to(160_000*tick-1);assert_eq!(read(d,0x5e),0);
        b.advance_to(160_000*tick);assert_eq!((read(d,0x5e),read(d,0x5f)),(0x5a,7));
    }
    #[test]
    fn factory_tilt_vectors_are_shared_coherent_and_do_not_require_gyro_irq() {
        let mut b=StickC::new();let mut ds=b.i2c_devices();
        let d=ds.iter_mut().find(|(bus,a,_)|*bus==0 && *a==0x68).unwrap().2.as_mut();
        for (r,v) in [(0x6b,1),(0x1c,0x10),(0x1b,0x18)] {wr(d,r,v);}
        for (g,expected) in [([0.0,0.0,1.0],[0,0,4096]),([0.5,0.5,0.5_f64.sqrt()],[2048,2048,2896]),([-0.5,0.25,0.6875_f64.sqrt()],[-2048,1024,3396])] {
            assert!(b.set_imu_sample(g,[0.0;3]));
            let d=ds.iter_mut().find(|(bus,a,_)|*bus==1 && *a==0x68).unwrap().2.as_mut();
            d.start(false);d.write(0x3b);d.start(true);let raw:Vec<_>=(0..6).map(|_|d.read()).collect();d.stop();
            let xyz:Vec<_>=raw.as_chunks::<2>().0.iter().map(|b|i16::from_be_bytes(*b)).collect();assert_eq!(xyz,expected);
        }
    }
    #[test]
    fn internal_i2c_and_controls_are_sku_specific() {
        let mut b=StickC::new();let mut ds=b.i2c_devices();assert_eq!(ds.len(),6);assert!(ds.iter().all(|(_,_,d)|d.pins()==Some((21,22))));assert_eq!(b.named_pin("button-a"),Some(37));assert_eq!(b.named_pin("button-b"),Some(39));assert_eq!(b.named_pin("power"),None);assert!(!b.button_at(0,"fake",true));assert!(b.button_at(0,"power",true));b.button_at(1,"power",false);
        let d=ds.iter_mut().find(|(bus,a,_)|*bus==1 && *a==0x34).unwrap().2.as_mut();d.start(false);d.write(0x46);d.start(true);assert_eq!(d.read(),2);
        b.gpio_changes(&[(10,false)]);assert_eq!(b.leds().unwrap().0,[[255,0,0]]);b.gpio_changes(&[(10,true)]);assert_eq!(b.leds().unwrap().0,[[0;3]]);assert!(b.set_imu_sample([0.0;3],[1.0;3]));assert!(!b.set_imu_sample([f64::NAN;3],[0.0;3]));
    }
}
