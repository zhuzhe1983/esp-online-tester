use std::sync::{Arc, Mutex};

use esp_periph::i2c::m5::{M5ioe1, M5ioe1State, M5pm1, M5pm1State};
use esp_periph::i2c::bmi270::{Bmi270, Bmi270Sample};
use esp_soc::board::{BoardEdge, BoardModel, VirtualCycle};
use esp_soc::devices::co5300::{Co5300, Co5300Scan};

use crate::i2c::{Cst820, I2cDevice, TouchState};

pub const PIN_STOPWATCH_TE: u8 = 38;
pub const PIN_STOPWATCH_BUTTON_A: u8 = 2;
pub const PIN_STOPWATCH_BUTTON_B: u8 = 1;

struct UnitButton {pressed:bool,level:bool,dirty:bool,cycle:VirtualCycle}

struct PanelState {
    panel: Co5300,
    active: bool,
    version: u64,
    unit_button: Option<UnitButton>,
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

pub struct StopWatch {
    wiring: Arc<PowerWiring>,
    gpio_events: u64,
    cs: Option<bool>,
    imu: Arc<Mutex<Bmi270Sample>>,
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
                }),
                pmic: Default::default(),
                expander: Default::default(),
                touch: Default::default(),
            }),
            gpio_events: 0,
            cs: None,
            imu: Default::default(),
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
        vec![
            (0, M5pm1::ADDRESS, Box::new(PowerDevice {
                device: Box::new(M5pm1::new(self.wiring.pmic.clone())), wiring: self.wiring.clone(),
            })),
            (0, M5ioe1::ADDRESS, Box::new(PowerDevice {
                device: Box::new(M5ioe1::new(self.wiring.expander.clone(), 0)), wiring: self.wiring.clone(),
            })),
            (0, 0x15, Box::new(Cst820::new(self.wiring.touch.clone()))),
            (0, Bmi270::ADDRESS, Box::new(Bmi270::new(self.imu.clone()))),
        ]
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
            (PIN_STOPWATCH_BUTTON_A, true), (PIN_STOPWATCH_BUTTON_B, true)];
        if let Some(button)=&state.unit_button {levels.push((11,button.level));}levels
    }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        self.wiring.sync();
        if self.wiring.pmic.lock().expect("StopWatch PMIC state mutex poisoned").power_off_requested
            || !self.wiring.expander.lock().expect("StopWatch expander state mutex poisoned").touch_reset_high { return; }
        self.wiring.touch.lock().expect("StopWatch touch mutex poisoned")
            .update(x.min(Self::WIDTH as u16 - 1), y.min(Self::HEIGHT as u16 - 1), down);
    }
    fn next_deadline(&self) -> Option<VirtualCycle> {
        self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").panel.next_deadline()
    }
    fn advance_to(&mut self, cycle: VirtualCycle) {
        self.wiring.sync();
        let mut state=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");
        state.panel.advance_to(cycle);if let Some(button)=&mut state.unit_button {button.cycle=cycle;}
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> {
        let mut state=self.wiring.panel.lock().expect("StopWatch panel mutex poisoned");let mut edges=state.panel.take_edges();
        if let Some(button)=&mut state.unit_button {if button.dirty {button.dirty=false;edges.push(BoardEdge{cycle:button.cycle,pin:11,level:button.level});}}edges
    }
    fn accessory_button_at(&mut self,cycle:VirtualCycle,model:&str,port:&str,connected:bool,pressed:bool)->Option<(u8,Option<bool>)> {
        if model!="gpio-button-u027" || port!="grove" {return None;}
        let level=self.wiring.pmic.lock().expect("StopWatch PMIC state mutex poisoned").grove_5v_enabled && !pressed;
        self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").unit_button=connected.then_some(UnitButton{pressed,level,dirty:false,cycle});
        Some((11,connected.then_some(level)))
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
