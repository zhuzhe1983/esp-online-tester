//! M5Stack StickT2 K016-T2 board wiring from the official pin map and M5-StickT a502cb40.
//! The shared AXP192 starts with LCD power enabled in this board's fixture. Lepton CCI
//! and VoSPI supply synthetic Raw14 counts. I2C encoder 0x30, microphone and RTC are absent.
pub mod lepton;
use std::sync::{Arc, Mutex};
use esp_periph::i2c::{I2cDevice, axp192::{Axp192, Axp192State, AXP192_ADDRESS}, mpu6886::{Mpu6886, Mpu6886Sample}};
use esp_soc::board::{BoardEdge, BoardModel, SpiPins, VirtualCycle};
use esp_soc::devices::{DcsPanel, i2c_bitbang::I2cBitBang};

const BUS_PINS: (u8, u8) = (21, 22);
const CCI_ADDRESS: u8 = 0x2a;
const IMU_ADDRESS: u8 = 0x68;
const VSYNC_PIN: u8 = 38;

pub struct StickT2 {
    pmic: Arc<Mutex<Axp192State>>,
    pub thermal: Arc<Mutex<lepton::Lepton>>,
    panel: DcsPanel,
    cs_high: bool,
    reset_high: bool,
    changes: u64,
    gpio_events: u64,
    accepted: u64,
    dropped: u64,
    i2c: I2cBitBang,
    levels: [bool; 2],
    edges: Vec<BoardEdge>,
    imu: Arc<Mutex<Mpu6886Sample>>,
}
impl StickT2 {
    pub fn new() -> Self {
        Self {
            pmic: Arc::new(Mutex::new(Axp192State::stickt2())),
            thermal: Arc::new(Mutex::new(lepton::Lepton::new())),
            panel: DcsPanel::new(240, 320),
            cs_high: true,
            reset_high: true,
            changes: 0,
            gpio_events: 0,
            accepted: 0,
            dropped: 0,
            i2c: I2cBitBang::new(),
            levels: [true; 2],
            edges: Vec::new(),
            imu: Default::default(),
        }
    }
}
impl Default for StickT2 { fn default() -> Self { Self::new() } }
impl BoardModel for StickT2 {
    fn name(&self) -> &'static str { "stickt2" }
    fn thermal_environment_at(&mut self, cycle: VirtualCycle, [background, hotspot, x, y, radius]: [u16; 5]) -> bool {
        self.advance_to(cycle);
        self.thermal.lock().expect("Lepton poisoned").set_scene(lepton::Raw14Scene { background, hotspot, center: [x, y], radius })
    }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        self.gpio_events+=changes.len() as u64;
        for &(pin, high) in changes {
            match pin {
                26 => self.thermal.lock().expect("Lepton poisoned").reset_line(high),
                0 => self.thermal.lock().expect("Lepton poisoned").cs(high),
                5 => self.cs_high = high,
                23 => self.panel.dc = high,
                18 => {
                    if self.reset_high && !high {
                        self.panel.reset();
                        self.changes = self.changes.wrapping_add(1);
                    }
                    self.reset_high = high;
                }
                _ => {}
            }
        }
    }
    fn gpio_output_at(&mut self, cycle: VirtualCycle, changes: &[(u8, bool)], enabled: u64, output: u64) {
        self.advance_to(cycle);
        self.gpio_changes(changes);
        let released = |pin| enabled & (1u64 << pin) == 0 || output & (1u64 << pin) != 0;
        let scl = released(BUS_PINS.1);
        let sda = self.i2c.drive(scl, released(BUS_PINS.0));
        for (i, (pin, level)) in [(BUS_PINS.0, sda), (BUS_PINS.1, scl)].into_iter().enumerate() {
            if level != self.levels[i] {
                self.levels[i] = level;
                self.edges.push(BoardEdge { cycle, pin, level });
            }
        }
    }
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self, _host: u8, pins: SpiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if pins.sclk & (1 << 2) != 0 && pins.miso == Some(25) && pins.cs & 1 != 0 {
            return self.thermal.lock().expect("Lepton poisoned").receive(rx_len);
        }
        if self.reset_high && !self.cs_high && pins.sclk & (1 << 13) != 0 && pins.mosi & (1 << 15) != 0
            && pins.cs & (1 << 5) != 0 && self.pmic.lock().expect("StickT2 PMIC poisoned").ldo_enabled(3)
        {
            let before = (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets);
            for &byte in tx { self.panel.byte(byte); }
            if before != (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets) {
                self.changes = self.changes.wrapping_add(1);
            }
            self.accepted += tx.len() as u64;
        } else {
            self.dropped += tx.len() as u64;
        }
        vec![0xff; rx_len]
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        self.i2c.clear();
        self.i2c.attach(CCI_ADDRESS, Box::new(lepton::Cci::new(self.thermal.clone())));
        self.i2c.attach(AXP192_ADDRESS, Box::new(Axp192::new(self.pmic.clone())));
        self.i2c.attach(IMU_ADDRESS, Box::new(Mpu6886::new(self.imu.clone())));
        (0..2).flat_map(|bus| self.i2c.controller_devices(Some(BUS_PINS)).into_iter()
            .map(move |(address, device)| (bus, address, device))).collect()
    }
    fn advance_to(&mut self, cycle: VirtualCycle) {
        for (cycle, level) in self.thermal.lock().expect("Lepton poisoned").advance(cycle) {
            self.edges.push(BoardEdge { cycle, pin: VSYNC_PIN, level });
        }
    }
    fn next_deadline(&self)->Option<VirtualCycle> {self.thermal.lock().expect("Lepton poisoned").deadline()}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn input_levels(&self) -> Vec<(u8, bool)> {
        let vsync = self.thermal.lock().expect("Lepton poisoned").vsync;
        vec![(37, true), (39, true), (VSYNC_PIN, vsync), (BUS_PINS.0, self.levels[0]), (BUS_PINS.1, self.levels[1])]
    }
    fn named_pin(&self,id:&str)->Option<u8> {match id {"btnA"|"button-a"=>Some(37),"btnB"|"button-b"=>Some(39),_=>None}}
    fn button_at(&mut self, _cycle: VirtualCycle, id: &str, pressed: bool) -> bool {
        if id != "power" { return false; }
        self.pmic.lock().expect("StickT2 PMIC poisoned").set_key(pressed);
        true
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        // Manufacturer ST7789_Rotation.h: (52,40), (40,53), (53,40), (40,52).
        let power = self.pmic.lock().expect("StickT2 PMIC poisoned");
        let lit = power.ldo_enabled(2) && power.ldo_enabled(3);
        drop(power);
        Some((135, 240, if lit { super::tdisplay::lcd_visible(&self.panel) } else { vec![0; 135 * 240] }, self.display_version()))
    }
    fn display_version(&self)->u64 {self.panel.pixels_written.wrapping_add(self.panel.resets).wrapping_add(self.changes).wrapping_add(self.pmic.lock().expect("StickT2 PMIC poisoned").version)}
    fn display_frames(&self)->u64 {self.panel.frames}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {Some((self.panel.gram.clone(),240,320))}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {
        if !accel_g.iter().chain(gyro_dps.iter()).all(|v|v.is_finite()) {return false;}
        let mut sample=self.imu.lock().expect("StickT2 IMU sample mutex poisoned");sample.accel_g=accel_g;sample.gyro_dps=gyro_dps;sample.version=sample.version.wrapping_add(1);true
    }
    fn set_camera_picture(&mut self, _p: esp_soc::picture::Picture) -> Result<(), &'static str> {
        Err("StickT2 uses synthetic Raw14 thermal input through the thermal environment")
    }
    fn report(&self) -> String {
        let thermal = self.thermal.lock().expect("Lepton poisoned");
        format!("[stickt2] ST7789 frames={} pixels={} spi={} dropped={}; MPU6886=polled; AXP192=LDO2/3-PEK-short; \
            Lepton3.0=synthetic-Raw14 cci_reads={} cci_writes={} commands={} unsupported={} vospi_reads={} packets={} mode={} phase={}; \
            encoder-0x30/microphone/RTC/long-power-key/analog-power=unmodeled", self.panel.frames,self.panel.pixels_written,
            self.accepted,self.dropped,thermal.cci_reads,thermal.cci_writes,thermal.commands,thermal.unsupported,thermal.spi_reads,
            thermal.packets,thermal.gpio_mode,thermal.phase)
    }
}
#[cfg(test)] mod tests {
    use super::*;
    fn pins()->SpiPins {SpiPins{sclk:1<<13,mosi:1<<15,cs:1<<5,..Default::default()}}
    fn cmd(b:&mut StickT2,c:u8,data:&[u8]) {b.gpio_changes(&[(5,false),(23,false)]);b.spi_transfer_pins(2,pins(),&[c],0);b.gpio_changes(&[(23,true)]);b.spi_transfer_pins(2,pins(),data,0);b.gpio_changes(&[(5,true)]);}
    fn init(b:&mut StickT2) {cmd(b,0x11,&[]);cmd(b,0x29,&[]);cmd(b,0x3a,&[0x55]);cmd(b,0x36,&[0x08]);}
    fn window(b:&mut StickT2,x:u16,y:u16) {let [xh,xl]=x.to_be_bytes();let [yh,yl]=y.to_be_bytes();cmd(b,0x2a,&[xh,xl,xh,xl]);cmd(b,0x2b,&[yh,yl,yh,yl]);}
    #[test] fn stickt2_panel_routes_pixels_rotation_sleep_reset() {
        let mut b=StickT2::new();init(&mut b);window(&mut b,52,40);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);
        cmd(&mut b,0x36,&[0x68]);window(&mut b,40,53);cmd(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.display().unwrap().2[134],0x07e0);
        let v=b.display_version();cmd(&mut b,0x10,&[]);assert!(b.display_version()>v);assert!(b.display().unwrap().2.iter().all(|p|*p==0));cmd(&mut b,0x11,&[]);
        b.gpio_changes(&[(5,false),(23,true)]);let count=b.panel.pixels_written;
        for route in [SpiPins::default(),SpiPins{sclk:1<<18,..pins()},SpiPins{mosi:1<<19,..pins()},SpiPins{cs:1<<4,..pins()}] {assert_eq!(b.spi_transfer_pins(3,route,&[0xff,0xff],2),[0xff;2]);}assert_eq!(b.panel.pixels_written,count);
        b.gpio_changes(&[(5,true)]);b.spi_transfer_pins(3,pins(),&[0xff,0xff],0);assert_eq!(b.panel.pixels_written,count);
        b.gpio_changes(&[(18,false),(5,false)]);let resets=b.panel.resets;b.gpio_changes(&[(18,false)]);assert_eq!(b.panel.resets,resets);b.spi_transfer_pins(3,pins(),&[0xff,0xff],0);assert_eq!(b.panel.pixels_written,0);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
    }
    #[test] fn stickt2_pins_and_imu_no_fake_encoder_or_rgb_camera() {
        let mut b=StickT2::new();assert_eq!(b.named_pin("btnA"),Some(37));assert_eq!(b.named_pin("btnB"),Some(39));assert!(b.input_levels().contains(&(39,true)));
        for id in ["power","encoder-press","PA2","PA3","PA4","clk","dt"] {assert_eq!(b.named_pin(id),None);}assert_eq!(b.encoder(),None);assert!(b.button_at(0,"power",true));assert!(b.button_at(1,"power",false));assert!(!b.button_at(2,"fake",true));assert!(b.camera_frame().is_none());
        let mut ds=b.i2c_devices();assert_eq!(ds.len(),6);assert!(ds.iter().all(|(_,a,d)|[0x2a,0x34,0x68].contains(a)&&d.pins()==Some((21,22))),"unmodeled 0x30 encoder must NACK");
        let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x68).unwrap().2.as_mut();d.start(false);d.write(0x6b);d.write(1);assert!(b.set_imu_sample([1.0,-1.0,0.0],[0.0;3]));d.start(false);d.write(0x3b);d.start(true);assert_eq!((0..6).map(|_|d.read()).collect::<Vec<_>>(),[0x40,0,0xc0,0,0,0]);
        let d=ds.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x68).unwrap().2.as_mut();d.start(false);d.write(0x6b);d.start(true);assert_eq!(d.read(),1);assert!(!b.set_imu_sample([f64::NAN,0.0,0.0],[0.0;3]));
    }
    #[test] fn stickt2_buttons_reach_classic_input_registers() {
        use emu_core::Bus;use esp_soc::SocBus as _;let mut b=crate::bus::SocBus::new(4*1024*1024,[2,0,0,0,0,1]);b.board=Box::new(StickT2::new());b.attach_board_devices();
        // IDF v5.5.4 io_mux_reg.h: input enable bit 9; SENSOR_CAPP/G37 +8, SENSOR_VN/G39 +0x10.
        b.write32(0x3ff49008,1<<9).unwrap();b.write32(0x3ff49010,1<<9).unwrap();
        let address=0x3ff44040;assert_eq!(b.read32(address).unwrap()&0xa0,0xa0);
        b.gpio_set_input(37,false);assert_eq!(b.read32(address).unwrap()&0xa0,0x80);b.gpio_set_input(39,false);assert_eq!(b.read32(address).unwrap()&0xa0,0);
        b.gpio_set_input(37,true);b.gpio_set_input(39,true);assert_eq!(b.read32(address).unwrap()&0xa0,0xa0);
    }
    #[test] fn stickt2_spi_mmio_and_gpio_matrix_reach_the_glass() {
        use emu_core::Bus;
        const GPIO:u32=0x3ff44000;const MUX:u32=0x3ff49000;const SPI:u32=0x3ff65000;
        let mut b=crate::bus::SocBus::new(4<<20,[2,0,0,0,0,1]);b.board=Box::new(StickT2::new());b.attach_board_devices();
        // IDF v5.5.4 io_mux_reg.h / gpio_sig_map.h: MCU_SEL2, VSPICLK63, VSPID65.
        for off in [0x6c,0x38,0x3c,0x70,0x8c] {b.write32(MUX+off,2<<12).unwrap();}
        for (pin,signal) in [(5,256),(13,63),(15,65),(18,256),(23,256)] {b.write32(GPIO+0x530+4*pin,signal).unwrap();}
        b.write32(GPIO+8,(1<<5)|(1<<18)).unwrap();b.write32(GPIO+0x24,(1<<5)|(1<<13)|(1<<15)|(1<<18)|(1<<23)).unwrap();
        let xfer=|b:&mut crate::bus::SocBus,dc:bool,bytes:&[u8]| {
            b.write32(GPIO+if dc {8}else{12},1<<23).unwrap();b.write32(GPIO+12,1<<5).unwrap();
            for (i,chunk) in bytes.chunks(4).enumerate() {let mut word=0u32;for (k,v) in chunk.iter().enumerate() {word|=u32::from(*v)<<(8*k);}b.write32(SPI+0x80+4*i as u32,word).unwrap();}
            b.write32(SPI+0x1c,1<<27).unwrap();b.write32(SPI+0x28,bytes.len() as u32*8-1).unwrap();b.write32(SPI,1<<18).unwrap();b.write32(GPIO+8,1<<5).unwrap();
        };
        let cmd=|b:&mut crate::bus::SocBus,c:u8,p:&[u8]| {xfer(b,false,&[c]);if !p.is_empty() {xfer(b,true,p);}};
        cmd(&mut b,0x11,&[]);cmd(&mut b,0x29,&[]);cmd(&mut b,0x3a,&[0x55]);cmd(&mut b,0x36,&[8]);
        cmd(&mut b,0x2a,&[0,52,0,52]);cmd(&mut b,0x2b,&[0,40,0,40]);cmd(&mut b,0x2c,&[0xf8,0]);
        assert_eq!(b.board.display().unwrap().2[0],0xf800,"real SPI MMIO, not a host framebuffer injection");
        b.write32(GPIO+0x530+4*13,256).unwrap();cmd(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"disconnecting SCLK must not change GRAM");
    }

    #[test] fn stickt2_shared_axp192_ldos_and_pek_are_not_gpio() {
        let mut b=StickT2::new();init(&mut b);window(&mut b,52,40);cmd(&mut b,0x2c,&[0xf8,0]);assert_eq!(b.display().unwrap().2[0],0xf800);
        let mut ds=b.i2c_devices();
        let wr=|ds:&mut Vec<(u8,u8,Box<dyn I2cDevice>)>,r:u8,v:u8| {let d=ds.iter_mut().find(|(bus,a,_)|*bus==0&&*a==0x34).unwrap().2.as_mut();d.start(false);d.write(r);d.write(v);d.stop();};
        let rd=|ds:&mut Vec<(u8,u8,Box<dyn I2cDevice>)>,r:u8| {let d=ds.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x34).unwrap().2.as_mut();d.start(false);d.write(r);d.start(true);d.read()};
        let version=b.display_version();wr(&mut ds,0x12,0x49);assert!(b.display_version()>version);assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        wr(&mut ds,0x12,0x4d);assert_eq!(b.display().unwrap().2[0],0xf800,"backlight-only gate preserves guest GRAM");
        wr(&mut ds,0x12,0x45);let written=b.panel.pixels_written;cmd(&mut b,0x2c,&[0x07,0xe0]);assert_eq!(b.panel.pixels_written,written,"TFT IC rail gates SPI writes");assert!(b.display().unwrap().2.iter().all(|p|*p==0));
        assert_eq!(b.named_pin("power"),None);assert!(b.button_at(0,"power",true));assert_eq!(rd(&mut ds,0x46)&2,0);b.button_at(1,"power",false);assert_eq!(rd(&mut ds,0x46)&2,2);assert_eq!(rd(&mut ds,0x46)&2,2);wr(&mut ds,0x46,2);assert_eq!(rd(&mut ds,0x46)&2,0);
    }

    #[test] fn stickt2_lepton_cci_gpio_and_spi_route_without_framebuffer_injection() {
        let mut b=StickT2::new(); let mut devices=b.i2c_devices();
        let cci=devices.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x2a).unwrap().2.as_mut();
        assert!(cci.start(false));for byte in [0,6,0,1,0,5] {assert!(cci.write(byte));}cci.stop();
        cci.start(false);for byte in [0,4,0x48,0x55] {assert!(cci.write(byte));}cci.stop();
        cci.start(false);cci.write(0);cci.write(2);cci.start(true);assert_eq!([cci.read(),cci.read()],[0,7]);cci.stop();
        let at=b.next_deadline().unwrap();b.advance_to(at);assert!(b.take_edges().iter().any(|e|e.pin==38&&e.level));
        b.gpio_changes(&[(0,false)]);
        let pins=SpiPins{sclk:1<<2,miso:Some(25),cs:1,..Default::default()};
        let before=b.thermal.lock().unwrap().spi_reads;
        assert_eq!(b.spi_transfer_pins(2,SpiPins{miso:Some(24),..pins},&[],4),[255;4]);assert_eq!(b.thermal.lock().unwrap().spi_reads,before);
        assert_eq!(&b.spi_transfer_pins(2,pins,&[],164)[..6],&[0,0,0xf9,0x3c,0x1f,0x40]);
        b.gpio_changes(&[(0,true)]);assert_eq!(b.spi_transfer_pins(2,pins,&[],4),[255;4]);
        b.gpio_changes(&[(26,false)]);b.advance_to(at);assert!(b.take_edges().iter().any(|e|e.pin==38&&!e.level));assert!(!cci.start(false));
        assert!(b.display().unwrap().2.iter().all(|p|*p==0));
    }

    #[test] fn stickt2_web_environment_changes_protocol_only_and_rejects_invalid_counts() {
        let mut board=StickT2::new();let state=board.thermal.clone();let mut devices=board.i2c_devices();
        let cci=devices.iter_mut().find(|(bus,addr,_)|*bus==0&&*addr==0x2a).unwrap().2.as_mut();
        cci.start(false);for v in [0,6,0,1,0,5] {cci.write(v);}cci.stop();cci.start(false);for v in [0,4,0x48,0x55] {cci.write(v);}cci.stop();
        cci.start(false);cci.write(0);cci.write(2);cci.start(true);cci.read();cci.read();cci.stop();
        let mut m=crate::machine([0;6],4<<20);m.bus.board=Box::new(board);let web=esp_soc::web::WebServer::queued();m.web=Some(web.clone());
        web.push_incoming(r#"{"t":"thermal-environment","background":1234,"hotspot":4321,"centerX":0,"centerY":0,"radius":0}"#.into());m.run(0);
        let at=m.bus.board.next_deadline().unwrap();m.bus.board.advance_to(at);m.bus.board.gpio_changes(&[(0,false)]);
        let bytes=state.lock().unwrap().receive(164);assert_eq!(&bytes[4..6],&4321u16.to_be_bytes());
        web.push_incoming(r#"{"t":"thermal-environment","background":0,"hotspot":12,"centerX":0,"centerY":0,"radius":0}"#.into());m.run(0);
        let bytes=state.lock().unwrap().receive(164);assert_eq!(&bytes[4..6],&1234u16.to_be_bytes());assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));
    }

}
