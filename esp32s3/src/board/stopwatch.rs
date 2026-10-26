use std::sync::{Arc, Mutex};

use esp_periph::i2c::m5::{M5ioe1, M5ioe1State, M5pm1, M5pm1State};
use esp_periph::i2c::bmi270::{Bmi270, Bmi270Sample};
use esp_soc::board::{BoardEdge, BoardModel, VirtualCycle};
use esp_soc::devices::co5300::{Co5300, Co5300Scan};

use crate::i2c::{Cst820, I2cDevice, TouchState};

pub const PIN_STOPWATCH_TE: u8 = 38;
pub const PIN_STOPWATCH_BUTTON_A: u8 = 2;
pub const PIN_STOPWATCH_BUTTON_B: u8 = 1;

struct PanelState {
    panel: Co5300,
    active: bool,
    version: u64,
}

struct PowerWiring {
    panel: Mutex<PanelState>,
    pmic: Arc<Mutex<M5pm1State>>,
    expander: Arc<Mutex<M5ioe1State>>,
    touch: Arc<Mutex<TouchState>>,
}

impl PowerWiring {
    fn sync(&self) {
        let off = self.pmic.lock().expect("StopWatch PMIC state mutex poisoned").power_off_requested;
        let expander = *self.expander.lock().expect("StopWatch expander state mutex poisoned");
        let active = !off && expander.oled_rail && expander.oled_reset_high;
        let mut panel = self.panel.lock().expect("StopWatch panel mutex poisoned");
        if panel.active && !active {
            panel.panel.reset();
            panel.version += 1;
        }
        panel.active = active;
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
        vec![state.panel.tear_input().expect("StopWatch scan must be configured"),
            (PIN_STOPWATCH_BUTTON_A, true), (PIN_STOPWATCH_BUTTON_B, true)]
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
        self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").panel.advance_to(cycle);
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> {
        self.wiring.panel.lock().expect("StopWatch panel mutex poisoned").panel.take_edges()
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
