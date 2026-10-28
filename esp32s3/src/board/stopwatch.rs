use std::sync::{Arc, Mutex};
use super::unit_pir::UnitPir;

use esp_periph::i2c::m5::{M5ioe1, M5ioe1State, M5pm1, M5pm1State};
use esp_periph::i2c::bmi270::{Bmi270, Bmi270Sample};
use esp_periph::i2c::env4::{Env4, Environment};
use esp_periph::i2c::bh1750::Bh1750;
use esp_soc::board::{BoardEdge, BoardModel, VirtualCycle};
use esp_soc::devices::Ws2812Chain;
use esp_soc::devices::co5300::{Co5300, Co5300Scan};
use esp_soc::devices::i2c_bitbang::I2cBitBang;
use esp_periph::i2c::Reg8Device;

use crate::i2c::{Cst820, I2cDevice, TouchState};

pub const PIN_STOPWATCH_TE: u8 = 38;
pub const PIN_STOPWATCH_BUTTON_A: u8 = 2;
pub const PIN_STOPWATCH_BUTTON_B: u8 = 1;

struct UnitRgb {chain:Ws2812Chain,connected:bool,powered:bool}
struct UnitAngle {source:esp_periph::analog::AnalogStream,connected:bool,value:f32,volts:f32}

struct UnitButton {pressed:bool,level:bool,dirty:bool,cycle:VirtualCycle}

struct PanelState {
    panel: Co5300,
    active: bool,
    version: u64,
    unit_button: Option<UnitButton>,
    unit_angle: Option<Box<UnitAngle>>,
    unit_env4: Option<Box<Env4>>,
    unit_dlight: Option<Box<Bh1750>>,
    unit_pir: Option<Box<UnitPir>>,
}

struct PowerWiring {
    panel: Mutex<PanelState>,
    pmic: Arc<Mutex<M5pm1State>>,
    expander: Arc<Mutex<M5ioe1State>>,
    touch: Arc<Mutex<TouchState>>,
}

impl PowerWiring {
    fn sync(&self) {
        let (off,grove_power) = {let pmic=self.pmic.lock().expect("StopWatch PMIC state mutex poisoned");(pmic.power_off_requested,pmic.grove_5v_enabled)};
        let expander = *self.expander.lock().expect("StopWatch expander state mutex poisoned");
        let active = !off && expander.oled_rail && expander.oled_reset_high;
        let mut panel = self.panel.lock().expect("StopWatch panel mutex poisoned");
        if panel.active && !active {
            panel.panel.reset();
            panel.version += 1;
        }
        panel.active = active;
        if let Some(button)=&mut panel.unit_button {
            // U027 V1.0: white J1.1 -> G11, R1=10k to Grove5V, R2=20k to GND.
            // Digital steady state only: no RC delay, bounce or analog contention.
            let level=grove_power && !button.pressed;
            if level!=button.level {button.level=level;button.dirty=true;}
        }
        if let Some(angle)=&mut panel.unit_angle {
            // U005 manufacturer's R1=10k published curve: 5*Rp/(Rp+10k), Rp/R2=value.
            // Other official drawing says 4.7k; do not claim an identified physical revision.
            let volts=if grove_power && angle.connected {5.0*angle.value/(1.0+angle.value)}else{0.0};
            if volts!=angle.volts {angle.volts=volts;let _=angle.source.replace_current(volts);}
        }
        if let Some(env)=&mut panel.unit_env4 {env.set_power(grove_power);}
        if let Some(light)=&mut panel.unit_dlight {light.set_power(grove_power);}
        if let Some(pir)=&mut panel.unit_pir {pir.set_power(grove_power);}
        if off || !expander.touch_reset_high {
            *self.touch.lock().expect("StopWatch touch mutex poisoned") = TouchState::default();
        }
    }
}

struct PowerDevice {
    device: Box<dyn I2cDevice>,
    wiring: Arc<PowerWiring>,
}

impl I2cDevice for PowerDevice {
    fn start(&mut self, read: bool) -> bool { self.device.start(read) }
    fn write(&mut self, byte: u8) -> bool {
        let ack = self.device.write(byte);
        self.wiring.sync();
        ack
    }
    fn read(&mut self) -> u8 { self.device.read() }
    fn stop(&mut self) { self.device.stop(); }
}

struct EnvDevice {wiring:Arc<PowerWiring>,address:u8}
impl I2cDevice for EnvDevice {
    // StopWatch J3: yellow G10 SDA, white G11 SCL. The existing S3 GPIO-matrix
    // decoder checks both master output and input routes before addressing.
    fn pins(&self)->Option<(u8,u8)> {Some((10,11))}
    fn start_address(&mut self,configured:u8,address:u8,read:bool)->bool {
        let general=self.address==0x44&&address==0&&!read;
        if !general&&configured!=address {return false;}self.wiring.sync();
        self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_env4.as_mut().is_some_and(|env|env.start(self.address,read,general))
    }
    fn start(&mut self,read:bool)->bool {self.start_address(self.address,self.address,read)}
    fn write(&mut self,b:u8)->bool {self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_env4.as_mut().is_some_and(|env|env.write(self.address,b))}
    fn read(&mut self)->u8 {self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_env4.as_mut().map_or(0xff,|env|env.read(self.address))}
}

struct LightDevice {wiring:Arc<PowerWiring>}
impl I2cDevice for LightDevice {
    fn pins(&self)->Option<(u8,u8)> {Some((10,11))}
    fn start(&mut self,read:bool)->bool {self.wiring.sync();self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_dlight.as_mut().is_some_and(|light|light.start(read))}
    fn write(&mut self,b:u8)->bool {self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_dlight.as_mut().is_some_and(|light|light.write(b))}
    fn read(&mut self)->u8 {self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_dlight.as_mut().map_or(0xff,|light|light.read())}
    fn stop(&mut self) {if let Some(light)=&mut self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_dlight {light.stop();}}
}

pub struct StopWatch {
    wiring: Arc<PowerWiring>,
    gpio_events: u64,
    cs: Option<bool>,
    imu: Arc<Mutex<Bmi270Sample>>,
    unit_rgb: Option<Box<UnitRgb>>,
    i2c: I2cBitBang,
    i2c_levels: [bool; 2],
    i2c_edges: Vec<BoardEdge>,
}

impl StopWatch {
    pub const WIDTH: usize = 468;
    pub const HEIGHT: usize = 466;
    pub const X_OFFSET: u16 = 6;

    pub fn new() -> Self {
        let us = crate::periph::CPU_HZ / 1_000_000;
        Self {
            wiring: Arc::new(PowerWiring {
                panel: Mutex::new(PanelState {
                    panel: Co5300::new(Self::WIDTH, Self::HEIGHT, Self::X_OFFSET).with_scan(Co5300Scan {
                        lines: 500, period: 15_000 * us, pulse_width: 120 * us, pin: PIN_STOPWATCH_TE,
                    }),
                    active: false,
                    version: 0,
                    unit_button: None,
                    unit_angle: None,
                    unit_env4: None,
                    unit_dlight: None,
                    unit_pir: None,
                }),
                pmic: Default::default(),
                expander: Default::default(),
                touch: Default::default(),
            }),
            gpio_events: 0,
            cs: None,
            imu: Default::default(),
            unit_rgb: None,
            i2c: I2cBitBang::new(),
            i2c_levels: [true, true],
            i2c_edges: Vec::new(),
        }
    }
    fn sync_rgb_power(&mut self) {
        if let Some(rgb)=&mut self.unit_rgb {
            let powered=rgb.connected && self.wiring.pmic.lock().expect("StopWatch PMIC state mutex poisoned").grove_5v_enabled;
            if powered!=rgb.powered {rgb.powered=powered;rgb.chain.leds.fill([0;3]);rgb.chain.updates=rgb.chain.updates.wrapping_add(1);}
        }
    }
}

impl Default for StopWatch { fn default() -> Self { Self::new() } }

impl BoardModel for StopWatch {
    fn name(&self) -> &'static str { "stopwatch" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        self.gpio_events += changes.len() as u64;
        // Official StopWatch HAL / M5GFX 0.2.19: GPIO39 is software-controlled CS.
        for &(pin, level) in changes {
            if pin == 39 {
                if !level { self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").panel.begin_transaction(); }
                self.cs = Some(level);
            }
        }
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {
        // U003 V1.0 J2.2 yellow -> Port A G10; 3 SK6812 RGB, GRB bytes.
        // Completed RMT frames only. No software pulse timing or daisy-chain output.
        if pin!=10 {return;}self.sync_rgb_power();
        if let Some(rgb)=&mut self.unit_rgb {if rgb.powered {rgb.chain.from_bits(bits);}}
    }
    fn led_grids(&self)->Vec<(&'static str,&[[u8;3]],u64)> {
        self.unit_rgb.as_ref().map_or_else(Vec::new,|rgb|vec![("grove-u003",rgb.chain.leds.as_slice(),rgb.chain.updates)])
    }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn spi_transfer(&mut self, host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if host == 2 {
            self.wiring.sync();
            let mut state = self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");
            if state.active && self.cs != Some(true) {
                let before = state.panel.pixels_written;
                if self.cs.is_some() { state.panel.transaction_stream(tx); } else { state.panel.transaction(tx); }
                if state.panel.pixels_written != before || tx.get(..4) == Some(&[0x02, 0, 0x01, 0]) {
                    state.version += 1;
                }
            }
        }
        vec![0xff; rx_len]
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        self.i2c.clear();
        let devices:Vec<(u8,u8,Box<dyn I2cDevice>)>=vec![
            (0, M5pm1::ADDRESS, Box::new(PowerDevice {
                device: Box::new(M5pm1::new(self.wiring.pmic.clone())), wiring: self.wiring.clone(),
            })),
            (0, M5ioe1::ADDRESS, Box::new(PowerDevice {
                device: Box::new(M5ioe1::new(self.wiring.expander.clone(), 0)), wiring: self.wiring.clone(),
            })),
            (0, 0x15, Box::new(Cst820::new(self.wiring.touch.clone()))),
            (0, Bmi270::ADDRESS, Box::new(Bmi270::new(self.imu.clone()))),
            // RX8130 register RAM for board probing/settings, as on PaperMono.
            // Calendar progression, alarms and battery-domain retention are not modeled.
            (0, 0x32, Box::new(Reg8Device::new("rx8130-register-ram", &[]))),
        ];
        for (_,address,device) in devices {self.i2c.attach(address,device);}
        // StopWatch schematic: internal SDA47/SCL48. M5GFX probes with GPIO,
        // then uses either controller; all paths access the same physical chips.
        let mut routed:Vec<(u8,u8,Box<dyn I2cDevice>)>=(0..2).flat_map(|bus|self.i2c.controller_devices(Some((47,48))).into_iter().map(move |(address,device)|(bus,address,device))).collect();
        for bus in [0,1] {for address in [0x44,0x76] {routed.push((bus,address,Box::new(EnvDevice{wiring:self.wiring.clone(),address})));}routed.push((bus,0x23,Box::new(LightDevice{wiring:self.wiring.clone()})));}routed
    }
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        self.gpio_changes(changes);
        let released=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=released(48);let sda=self.i2c.drive(scl,released(47));
        for (index,(pin,level)) in [(47,sda),(48,scl)].into_iter().enumerate() {
            if self.i2c_levels[index]!=level {self.i2c_levels[index]=level;self.i2c_edges.push(BoardEdge{cycle,pin,level});}
        }
    }
    fn set_imu_sample(&mut self, accel_g: [f64; 3], gyro_dps: [f64; 3]) -> bool {
        if accel_g.iter().chain(gyro_dps.iter()).any(|v| !v.is_finite()) { return false; }
        let mut sample = self.imu.lock().expect("StopWatch IMU sample mutex poisoned");
        sample.accel_g = accel_g.map(|v| v.clamp(-16.0, 16.0));
        sample.gyro_dps = gyro_dps.map(|v| v.clamp(-2000.0, 2000.0));
        sample.version = sample.version.wrapping_add(1); true
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let state = self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");
        Some((Self::WIDTH as u32, Self::HEIGHT as u32, state.panel.frame.clone(), state.version))
    }
    fn display_version(&self) -> u64 { self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").version }
    fn display_frames(&self) -> u64 { self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").panel.frames }
    fn input_levels(&self) -> Vec<(u8, bool)> {
        let state = self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");
        let mut levels=vec![state.panel.tear_input().expect("StopWatch scan must be configured"),
            (PIN_STOPWATCH_BUTTON_A, true), (PIN_STOPWATCH_BUTTON_B, true), (47,self.i2c_levels[0]), (48,self.i2c_levels[1])];
        if let Some(button)=&state.unit_button {levels.push((11,button.level));}
        if let Some(pir)=&state.unit_pir {if let Some(level)=pir.level() {levels.push((11,level));}}levels
    }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        self.wiring.sync();
        if self.wiring.pmic.lock().expect("StopWatch PMIC state mutex poisoned").power_off_requested
            || !self.wiring.expander.lock().expect("StopWatch expander state mutex poisoned").touch_reset_high { return; }
        self.wiring.touch.lock().expect("StopWatch touch mutex poisoned")
            .update(x.min(Self::WIDTH as u16 - 1), y.min(Self::HEIGHT as u16 - 1), down);
    }
    fn next_deadline(&self) -> Option<VirtualCycle> {
        let state=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");state.panel.next_deadline().into_iter().chain(state.unit_pir.as_ref().and_then(|pir|pir.deadline())).min()
    }
    fn advance_to(&mut self, cycle: VirtualCycle) {
        self.wiring.sync();self.sync_rgb_power();
        let mut state=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");
        state.panel.advance_to(cycle);if let Some(button)=&mut state.unit_button {button.cycle=cycle;}
        if let Some(env)=&mut state.unit_env4 {env.advance_to(cycle);}
        if let Some(light)=&mut state.unit_dlight {light.advance_to(cycle);}
        if let Some(pir)=&mut state.unit_pir {pir.advance_to(cycle);}
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> {
        let mut state=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");let mut edges=state.panel.take_edges();
        edges.append(&mut self.i2c_edges);
        if let Some(button)=&mut state.unit_button {if button.dirty {button.dirty=false;edges.push(BoardEdge{cycle:button.cycle,pin:11,level:button.level});}}
        if let Some(pir)=&mut state.unit_pir {edges.extend(pir.take_edges());}edges
    }
    fn released_inputs(&mut self)->Vec<u8> {self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_pir.as_mut().map_or_else(Vec::new,|pir|pir.released_inputs())}
    fn accessory_motion_at(&mut self,cycle:VirtualCycle,model:&str,port:&str,connected:bool,motion:bool)->bool {
        if model!="pir-u004-m5-2s"||port!="grove" {return false;}
        {let mut state=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");let pir=state.unit_pir.get_or_insert_with(||Box::new(UnitPir::new(crate::periph::CPU_HZ)));pir.input_at(cycle,connected,motion);}
        self.wiring.sync();true
    }
    fn accessory_button_at(&mut self,cycle:VirtualCycle,model:&str,port:&str,connected:bool,pressed:bool)->Option<(u8,Option<bool>)> {
        if model!="gpio-button-u027" || port!="grove" {return None;}
        let level=self.wiring.pmic.lock().expect("StopWatch PMIC state mutex poisoned").grove_5v_enabled && !pressed;
        self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_button=connected.then_some(UnitButton{pressed,level,dirty:false,cycle});
        Some((11,connected.then_some(level)))
    }
    fn accessory_connection_at(&mut self,_cycle:VirtualCycle,model:&str,port:&str,connected:bool)->bool {
        if model!="sk6812-u003" || port!="grove" {return false;}
        let rgb=self.unit_rgb.get_or_insert_with(||Box::new(UnitRgb{chain:Ws2812Chain::new(3),connected:false,powered:false}));
        if rgb.connected!=connected {rgb.connected=connected;rgb.chain.leds.fill([0;3]);rgb.chain.updates=rgb.chain.updates.wrapping_add(1);}
        self.sync_rgb_power();true
    }
    fn accessory_analog_at(&mut self,cycle:VirtualCycle,model:&str,port:&str,connected:bool,value:f32)->Option<(u8,esp_periph::AnalogSource)> {
        if model!="potentiometer-u005-10k-curve" || port!="grove" || !value.is_finite() || !(0.0..=1.0).contains(&value) {return None;}
        let source={let mut panel=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");
            if panel.unit_angle.is_none() {panel.unit_angle=Some(Box::new(UnitAngle{source:esp_periph::analog::AnalogStream::new(1,0.0,cycle).ok()?,connected:false,value:0.0,volts:0.0}));}
            let angle=panel.unit_angle.as_mut()?;angle.connected=connected;angle.value=value;angle.source.clone()};
        self.wiring.sync();Some((11,esp_periph::AnalogSource::Stream(source)))
    }
    fn accessory_environment_at(&mut self,cycle:VirtualCycle,model:&str,port:&str,connected:bool,sample:Environment)->bool {
        if model!="sht40-bmp280-u001d"||port!="grove"||!sample.valid() {return false;}
        {let mut panel=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");let env=panel.unit_env4.get_or_insert_with(||Box::new(Env4::new(crate::periph::CPU_HZ)));env.update(cycle,connected,sample);}
        self.wiring.sync();true
    }
    fn accessory_illuminance_at(&mut self,cycle:VirtualCycle,model:&str,port:&str,connected:bool,lux:f64)->bool {
        if model!="bh1750fvi-u136"||port!="grove"||!Bh1750::valid(lux) {return false;}
        {let mut panel=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");let light=panel.unit_dlight.get_or_insert_with(||Box::new(Bh1750::new(crate::periph::CPU_HZ)));light.update(cycle,connected,lux);}
        self.wiring.sync();true
    }
    fn named_pin(&self, name: &str) -> Option<u8> {
        match name {
            "a" | "btn_a" => Some(PIN_STOPWATCH_BUTTON_A),
            "b" | "btn_b" => Some(PIN_STOPWATCH_BUTTON_B),
            _ => None,
        }
    }
    fn report(&self) -> String {
        let off = self.wiring.pmic.lock().expect("StopWatch PMIC state mutex poisoned").power_off_requested;
        format!("[stopwatch] power_off_requested={off}")
    }
}

#[cfg(test)]
mod unit_button_tests {
    use super::*;
    fn write(device:&mut dyn I2cDevice,reg:u8,value:u8) {assert!(device.start(false));assert!(device.write(reg));assert!(device.write(value));device.stop();}
    #[test]
    fn internal_i2c_gpio_probe_and_both_controllers_share_state() {
        fn drive(b:&mut StopWatch,scl:bool,sda:bool)->bool {
            b.gpio_output_at(1,&[],(1<<47)|(1<<48),(u64::from(scl)<<48)|(u64::from(sda)<<47));
            b.input_levels().into_iter().find(|(pin,_)|*pin==47).unwrap().1
        }
        fn start(b:&mut StopWatch) {drive(b,false,true);drive(b,true,true);drive(b,true,false);}
        fn stop(b:&mut StopWatch) {drive(b,false,false);drive(b,true,false);assert!(drive(b,true,true));}
        fn send(b:&mut StopWatch,value:u8)->bool {for bit in (0..8).rev() {drive(b,false,value&(1<<bit)!=0);drive(b,true,value&(1<<bit)!=0);}drive(b,false,true);let ack=!drive(b,true,true);drive(b,false,true);ack}
        let mut b=StopWatch::new();let mut devices=b.i2c_devices();
        for address in [0x6e,0x4f,0x68,0x15,0x32] {start(&mut b);assert!(send(&mut b,address<<1));stop(&mut b);}
        start(&mut b);assert!(!send(&mut b,0x50<<1));stop(&mut b); // StopWatch has no PaperMono NFC.
        start(&mut b);assert!(send(&mut b,0x6e<<1));assert!(send(&mut b,6));assert!(send(&mut b,0x1f));stop(&mut b);
        for bus in 0..2 {let d=devices.iter_mut().find(|(port,a,_)|*port==bus&&*a==0x6e).unwrap().2.as_mut();assert_eq!(d.pins(),Some((47,48)));assert!(d.start(false));assert!(d.write(6));assert!(d.start(true));assert_eq!(d.read(),0x1f);d.stop();}
        assert!(b.wiring.pmic.lock().unwrap().grove_5v_enabled);
        assert!(b.take_edges().iter().all(|e|e.pin==47||e.pin==48));
        assert!(b.take_edges().is_empty());
        // Reinitializing the board restores real device reset state, not the prior probe.
        let mut reset=b.i2c_devices();let d=reset.iter_mut().find(|(bus,a,_)|*bus==1&&*a==0x6e).unwrap().2.as_mut();d.start(false);d.write(6);d.start(true);assert_eq!(d.read(),0x17);
    }
    #[test]
    fn u027_routes_only_white_pin_and_follows_grove_power_and_hold() {
        let mut board=StopWatch::new();let mut devices=board.i2c_devices();let pmic=devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2.as_mut();
        assert_eq!(board.accessory_button_at(0,"gpio-button-u027","grove",true,false),Some((11,Some(false))));
        assert_eq!(board.accessory_button_at(0,"gpio-button-u027","internal-i2c",true,true),None);
        assert_eq!(board.accessory_button_at(0,"arbitrary","grove",true,true),None);
        write(pmic,6,0x1f);board.advance_to(10);assert!(board.take_edges().contains(&BoardEdge{cycle:10,pin:11,level:true}));
        assert!(!board.input_levels().iter().any(|(pin,_)|*pin==10),"yellow wire is NC");
        assert_eq!(board.accessory_button_at(11,"gpio-button-u027","grove",true,true),Some((11,Some(false))));
        write(pmic,6,0x17);board.advance_to(12);assert!(board.take_edges().is_empty());
        assert_eq!(board.accessory_button_at(13,"gpio-button-u027","grove",true,false),Some((11,Some(false))));
        write(pmic,6,0x1f);write(pmic,7,0x40);write(pmic,0x0c,0xa1);board.advance_to(14);assert!(board.take_edges().contains(&BoardEdge{cycle:14,pin:11,level:true}));
        write(pmic,7,0);board.advance_to(15);assert!(board.take_edges().contains(&BoardEdge{cycle:15,pin:11,level:false}));
        assert_eq!(board.accessory_button_at(16,"gpio-button-u027","grove",false,true),Some((11,None)));
        assert!(!board.input_levels().iter().any(|(pin,_)|*pin==11));write(pmic,7,0x40);board.advance_to(17);assert!(board.take_edges().is_empty(),"detached unit cannot drive after a power write");
    }
    #[test]
    fn u027_detach_discards_pending_edges_and_guest_reset_preserves_physical_hold() {
        let mut board=StopWatch::new();board.accessory_button_at(0,"gpio-button-u027","grove",true,false);
        let mut devices=board.i2c_devices();let pmic=devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2.as_mut();write(pmic,6,0x1f);
        board.accessory_button_at(1,"gpio-button-u027","grove",false,false);assert!(board.take_edges().is_empty());
        board.accessory_button_at(2,"gpio-button-u027","grove",true,true);drop(devices);let mut devices=board.i2c_devices();
        write(devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2.as_mut(),6,0x1f);board.advance_to(3);
        assert!(board.input_levels().contains(&(11,false)),"guest restart does not physically release a held switch");
        assert_eq!(board.accessory_button_at(4,"gpio-button-u027","grove",true,false),Some((11,Some(true))));
    }
}

#[cfg(test)]
mod unit_rgb_tests {
    use super::*;
    fn write(device:&mut dyn I2cDevice,reg:u8,value:u8){assert!(device.start(false));assert!(device.write(reg));assert!(device.write(value));device.stop();}
    fn bits()->Vec<bool>{[0u8,255,0,128,0,0,0,0,64].into_iter().flat_map(|v|(0..8).map(move|i|v&(0x80>>i)!=0)).collect()}
    #[test]
    fn u003_real_rmt_route_power_disconnect_and_reset() {
        let mut board=StopWatch::new();assert!(board.led_grids().is_empty());board.rmt_frame(10,&bits());assert!(board.led_grids().is_empty());
        assert!(!board.accessory_connection_at(0,"arbitrary","grove",true));assert!(!board.accessory_connection_at(0,"sk6812-u003","other",true));
        assert!(board.accessory_connection_at(0,"sk6812-u003","grove",true));let mut devices=board.i2c_devices();let pmic=devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2.as_mut();
        board.rmt_frame(10,&bits());assert_eq!(board.led_grids()[0].1,[[0;3];3]);write(pmic,6,0x1f);
        board.rmt_frame(11,&bits());board.advance_to(1);assert_eq!(board.led_grids()[0].1,[[0;3];3]);
        board.rmt_frame(10,&bits());assert_eq!(board.led_grids()[0].1,[[255,0,0],[0,128,0],[0,0,64]]);
        write(pmic,6,0x17);board.advance_to(2);assert_eq!(board.led_grids()[0].1,[[0;3];3]);write(pmic,6,0x1f);board.advance_to(3);assert_eq!(board.led_grids()[0].1,[[0;3];3]);
        board.rmt_frame(10,&bits());board.accessory_connection_at(4,"sk6812-u003","grove",false);board.rmt_frame(10,&bits());assert_eq!(board.led_grids()[0].1,[[0;3];3]);
        board.accessory_connection_at(5,"sk6812-u003","grove",true);assert_eq!(board.led_grids()[0].1,[[0;3];3]);board.rmt_frame(10,&bits());
        write(pmic,7,0x40);write(pmic,0x0c,0xa1);board.advance_to(6);assert_eq!(board.led_grids()[0].1[0],[255,0,0]);write(pmic,7,0);board.advance_to(7);assert_eq!(board.led_grids()[0].1,[[0;3];3]);
        drop(devices);board.i2c_devices();board.advance_to(8);assert_eq!(board.led_grids()[0].1,[[0;3];3]);
    }
}

#[cfg(test)]
mod unit_angle_tests {
    use super::*;
    fn write(device:&mut dyn I2cDevice,reg:u8,value:u8){assert!(device.start(false));assert!(device.write(reg));assert!(device.write(value));device.stop();}
    #[test]
    fn u005_voltage_reaches_only_white_adc_pad_and_tracks_power_and_connection() {
        let mut board=StopWatch::new();assert!(board.accessory_analog_at(0,"other","grove",true,0.5).is_none());assert!(board.accessory_analog_at(0,"potentiometer-u005-10k-curve","wrong",true,0.5).is_none());
        let (pin,source)=board.accessory_analog_at(0,"potentiometer-u005-10k-curve","grove",true,0.5).unwrap();assert_eq!(pin,11);
        let mut pads=esp_periph::AnalogInputs::new(240_000_000);pads.set(pin,source);let mut devices=board.i2c_devices();let pmic=devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2.as_mut();
        assert_eq!(pads.volts(11,0),0.0);write(pmic,6,0x1f);assert!((pads.volts(11,1)-5.0/3.0).abs()<0.00001);assert_eq!(pads.volts(10,1),0.0);
        for value in [f32::NAN,-0.1,1.1] {assert!(board.accessory_analog_at(2,"potentiometer-u005-10k-curve","grove",true,value).is_none());}
        assert!((pads.volts(11,2)-5.0/3.0).abs()<0.00001);
        board.accessory_analog_at(3,"potentiometer-u005-10k-curve","grove",true,1.0);assert_eq!(pads.volts(11,3),2.5);
        write(pmic,6,0x17);assert_eq!(pads.volts(11,4),0.0);write(pmic,6,0x1f);assert_eq!(pads.volts(11,5),2.5);
        board.accessory_analog_at(6,"potentiometer-u005-10k-curve","grove",false,1.0);assert_eq!(pads.volts(11,6),0.0);
        board.accessory_analog_at(7,"potentiometer-u005-10k-curve","grove",true,0.25);assert_eq!(pads.volts(11,7),1.0);
        write(pmic,7,0x40);write(pmic,0x0c,0xa1);assert_eq!(pads.volts(11,8),1.0);write(pmic,7,0);assert_eq!(pads.volts(11,9),0.0);
        drop(devices);board.i2c_devices();assert_eq!(pads.volts(11,10),0.0);
    }
}

#[cfg(test)]
mod unit_env4_tests {
    use super::*;
    #[test]
    fn environment_endpoints_need_attachment_supply_and_exact_model() {
        let mut board=StopWatch::new();let mut devices=board.i2c_devices();
        for (_,address,device) in &mut devices {if [0x44,0x76].contains(address) {assert_eq!(device.pins(),Some((10,11)));assert!(!device.start(false));}}
        assert!(!board.accessory_environment_at(0,"sht30-qmp6988","grove",true,Environment::default()));
        assert!(!board.accessory_environment_at(0,"sht40-bmp280-u001d","wrong-port",true,Environment::default()));
        assert!(board.accessory_environment_at(0,"sht40-bmp280-u001d","grove",true,Environment::default()));
        assert!(!devices.iter_mut().find(|(_,a,_)|*a==0x44).unwrap().2.start(false));
        {let pmic=&mut devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2;assert!(pmic.start(false));pmic.write(6);pmic.write(0x1f);pmic.stop();}
        board.advance_to(crate::periph::CPU_HZ/100);
        {let sensor=&mut devices.iter_mut().find(|(_,a,_)|*a==0x76).unwrap().2;assert!(sensor.start(false));assert!(sensor.write(0xd0));assert!(sensor.start(true));assert_eq!(sensor.read(),0x58);}
        assert!(board.accessory_environment_at(crate::periph::CPU_HZ/100,"sht40-bmp280-u001d","grove",false,Environment::default()));
        assert!(!devices.iter_mut().find(|(_,a,_)|*a==0x76).unwrap().2.start(false));
        assert!(board.accessory_environment_at(crate::periph::CPU_HZ/100,"sht40-bmp280-u001d","grove",true,Environment::default()));
        {let pmic=&mut devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2;pmic.start(false);pmic.write(6);pmic.write(0x17);pmic.stop();}
        assert!(!devices.iter_mut().find(|(_,a,_)|*a==0x44).unwrap().2.start(false));
    }
}

#[cfg(test)]
mod unit_dlight_tests {
    use super::*;
    #[test]
    fn light_device_is_physical_powered_and_independent_of_environment() {
        let mut board=StopWatch::new();let mut devices=board.i2c_devices();
        assert!(!board.accessory_illuminance_at(0,"bh1750-other-sku","grove",true,1000.0));
        assert!(!board.accessory_illuminance_at(0,"bh1750fvi-u136","wrong",true,1000.0));
        assert!(board.accessory_illuminance_at(0,"bh1750fvi-u136","grove",true,1000.0));
        assert!(!devices.iter_mut().find(|(_,a,_)|*a==0x23).unwrap().2.start(false));
        {let pmic=&mut devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2;pmic.start(false);pmic.write(6);pmic.write(0x1f);pmic.stop();}
        {let light=&mut devices.iter_mut().find(|(_,a,_)|*a==0x23).unwrap().2;assert_eq!(light.pins(),Some((10,11)));assert!(light.start(false));assert!(light.write(0x20));light.stop();}
        board.advance_to(crate::periph::CPU_HZ/5);
        {let light=&mut devices.iter_mut().find(|(_,a,_)|*a==0x23).unwrap().2;assert!(light.start(true));assert_eq!([light.read(),light.read()],[4,176]);light.stop();}
        assert!(!devices.iter_mut().find(|(_,a,_)|*a==0x44).unwrap().2.start(false));
        assert!(board.accessory_environment_at(crate::periph::CPU_HZ/5,"sht40-bmp280-u001d","grove",true,Environment::default()));
        assert!(board.accessory_illuminance_at(crate::periph::CPU_HZ/5,"bh1750fvi-u136","grove",false,1000.0));
        assert!(!devices.iter_mut().find(|(_,a,_)|*a==0x23).unwrap().2.start(false));
        assert!(devices.iter_mut().find(|(_,a,_)|*a==0x76).unwrap().2.start(false));
    }
}

#[cfg(test)]
mod unit_pir_tests {
    use super::*;
    fn write(device:&mut dyn I2cDevice,reg:u8,value:u8){assert!(device.start(false));assert!(device.write(reg));assert!(device.write(value));device.stop();}
    #[test] fn u004_deadline_pin_power_hold_and_disconnected_edges() {
        let mut b=StopWatch::new();let mut devices=b.i2c_devices();let pmic=devices.iter_mut().find(|(_,a,_)|*a==0x6e).unwrap().2.as_mut();
        assert!(!b.accessory_motion_at(0,"arbitrary","grove",true,true));assert!(!b.accessory_motion_at(0,"pir-u004-m5-2s","internal-i2c",true,true));
        assert!(b.accessory_motion_at(0,"pir-u004-m5-2s","grove",true,true));assert!(!b.input_levels().iter().any(|(pin,_)|*pin==11));
        write(pmic,6,0x1f);b.advance_to(10);assert_eq!(b.take_edges(),[BoardEdge{cycle:0,pin:11,level:true}]);assert!(!b.input_levels().iter().any(|(pin,_)|*pin==10));
        b.accessory_motion_at(10,"pir-u004-m5-2s","grove",true,false);let due=10+crate::periph::CPU_HZ*2;assert_eq!(b.next_deadline(),Some(due));b.advance_to(due);assert!(b.take_edges().contains(&BoardEdge{cycle:due,pin:11,level:false}));
        b.accessory_motion_at(due,"pir-u004-m5-2s","grove",true,true);b.take_edges();write(pmic,7,0x40);write(pmic,0x0c,0xa1);assert!(b.input_levels().contains(&(11,true)));
        write(pmic,7,0);assert_eq!(b.released_inputs(),[11]);assert!(!b.input_levels().iter().any(|(pin,_)|*pin==11));
        b.accessory_motion_at(due,"pir-u004-m5-2s","grove",false,true);write(pmic,7,0x40);assert!(b.take_edges().is_empty());assert!(!b.input_levels().iter().any(|(pin,_)|*pin==11));
    }
}
