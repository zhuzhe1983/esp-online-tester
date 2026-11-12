//! Boards around the ESP32-S31. One so far: Espressif's ESP-Mosaico (board revision v1.2), wiring
//! from esp-mosaico-bsp (`esp_mosaico.h`) and the Wokwi board-esp-mosaico docs: a CO5300 480 x 480
//! AMOLED on GP-SPI2 in QSPI mode (CLK 42, CS 50, D0-D3 36/51/35/9, TE 43), its RESET on GPIO 44
//! (shared with the touch controller's), and the display supply enabled by GPIO 60 driven low.
//! Modelled: the panel and its reset and power gates, the CST9220 touch on I2C0 (0x5A, SDA 56,
//! SCL 3, INT 6), the top button on GPIO 7 and the Boot key on GPIO 61, the BQ27220 fuel gauge (0x55), the BMI270 IMU (0x69) and two BMM150 magnetometers (0x11, 0x12) on I2C0, and the panel's TE output on GPIO 43: a 60 Hz frame with a 1 ms high blanking pulse.
//! Audio: the ES8311 codec (0x19) on I2C0 as a register file with its chip ID, serving the factory
//! firmware's `audio_dac` and `audio_adc` both (no ES7210 is addressed), full duplex on I2S0 (the factory firmware logs BCLK 37, WS 49, DOUT 52, DIN 40) with the
//! speaker amplifier enabled by GPIO 45; what I2S0 transmits is the run's audio (`--wav`).
use esp_soc::devices::co5300::Co5300;
use esp_soc::{Board, BoardModel, NoBoard};

pub fn make_board(name: &str) -> Option<Board> {
    match name {
        "none" | "bare" => Some(Box::new(NoBoard)),
        "mosaico" | "esp-mosaico" => Some(Box::new(Mosaico::new())),
        _ => None,
    }
}

pub const PIN_LCD_RESET: u8 = 44;
pub const PIN_LCD_POWER_N: u8 = 60;
pub const PIN_LCD_TE: u8 = 43;
/// The speaker amplifier's enable (the factory firmware's `gpio_pa_control`), active high.
pub const PIN_PA: u8 = 45;
/// The ES8311 DAC on I2C0: the factory firmware addresses it as 0x32 (8-bit), CE high.
/// Chip ID registers 0xFD/0xFE read 0x83/0x11 as on the other boards' ES8311.
pub const ES8311_ADDRESS: u8 = 0x19;
/// CST9220 INT, active low: low while a finger is down, and for 1 ms after it lifts so the
/// driver reads the release report.
pub const PIN_TOUCH_INT: u8 = 6;
/// The top button: the factory firmware's board manager calls it `button_power` (gpio_config_t mask
/// bit 7; single click sleep/back, long press chat). Active low with the board's pull-up.
pub const PIN_AI_BUTTON: u8 = 7;
/// The Boot key: `button_boot` in the same table (mask bit 61), active low.
pub const PIN_BOOT_BUTTON: u8 = 61;
/// The BMI270 IMU (`imu_sensor`) on I2C0: the factory firmware's board-manager entry
/// {"imu_sensor", "custom", "bmi270", addr 0x69, "i2c_master"} (SDO high).
pub const BMI270_ADDRESS: u8 = esp_soc::devices::BMI270_ADDRESS;
/// Two BMM150 magnetometers on I2C0, directly on the bus (not behind the BMI270's AUX
/// interface): `magnetometer_1` and `magnetometer_2` list 8-bit addresses 0x22 and 0x24, which
/// the firmware logs as yaml_addr and shifts to the 7-bit i2c_addr.
pub const BMM150_ADDRESSES: [u8; 2] = [0x11, 0x12];
const TOUCH_RELEASE_NS: u64 = 1_000_000;
/// CO5300 TE: one frame every 1/60 s, high for the first 1 ms (the blanking interval).
const TE_PERIOD_NS: u64 = 16_666_667;
const TE_HIGH_NS: u64 = 1_000_000;

/// The ESP-Mosaico v1.2.
pub struct Mosaico {
    /// the CO5300; its frame is what the glass shows (480 x 480, no column offset in the BSP)
    pub panel: Co5300,
    /// the supply enable (GPIO 60, active low) and the reset line (GPIO 44, active low)
    pub powered: bool,
    pub in_reset: bool,
    /// the speaker amplifier's enable (GPIO 45)
    pub amp: bool,
    pub gpio_events: u64,
    /// transactions the panel could not take: unpowered or held in reset
    pub dropped: u64,
    /// the finger the CST9220 reports
    pub touch: std::sync::Arc<std::sync::Mutex<crate::cst9220::Touch>>,
    /// the storage NAND on GP-SPI3 (the BSP's MOSAICO_NAND)
    pub nand: crate::spi_nand::SpiNand,
    /// TE: the level now and the edges not yet delivered (`advance_to` takes emulated nanoseconds)
    te_level: bool,
    te_now: u64,
    /// INT level and, after a release, when it goes back high
    touch_int_low: bool,
    touch_release_at: Option<u64>,
    te_edges: Vec<esp_soc::board::BoardEdge>,
    /// Scripted motion for the BMI270 (clock: emulated nanoseconds, stored by `advance_to`)
    pub imu: std::sync::Arc<esp_soc::devices::imu_motion::ImuMotion>,
}
impl Default for Mosaico { fn default() -> Self { Self::new() } }
impl Mosaico {
    pub const WIDTH: usize = 480;
    pub const HEIGHT: usize = 480;
    pub fn new() -> Self {
        Mosaico { panel: Co5300::new(Self::WIDTH, Self::HEIGHT, 0), powered: false, in_reset: false, amp: false, gpio_events: 0, dropped: 0, touch: Default::default(), nand: crate::spi_nand::SpiNand::new(), te_level: false, te_now: 0, touch_int_low: false, touch_release_at: None, te_edges: Vec::new(), imu: Default::default() }
    }
}
impl BoardModel for Mosaico {
    fn name(&self) -> &'static str { "mosaico" }
    fn uses_gpio_edges(&self) -> bool { false }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, level) in changes {
            self.gpio_events += 1;
            match pin {
                PIN_LCD_POWER_N => { if level && self.powered { self.panel.reset(); } self.powered = !level; }
                PIN_LCD_RESET => { if !level { self.panel.reset(); } self.in_reset = !level; }
                PIN_PA => self.amp = level,
                _ => {}
            }
        }
    }
    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_LCD_TE, self.te_level), (PIN_TOUCH_INT, !self.touch_int_low), (PIN_AI_BUTTON, true), (PIN_BOOT_BUTTON, true)] }
    /// The S31 machine passes emulated time in nanoseconds. TE runs only while the panel is powered
    /// and out of reset; a jump over several frames yields the level at `now` (one edge pair at most).
    fn advance_to(&mut self, now: esp_soc::board::VirtualCycle) {
        self.te_now = now;
        self.imu.cycle.store(now, std::sync::atomic::Ordering::Relaxed);
        if self.touch_release_at.is_some_and(|t| now >= t) {
            self.touch_release_at = None; self.touch_int_low = false;
            self.te_edges.push(esp_soc::board::BoardEdge { cycle: now, pin: PIN_TOUCH_INT, level: true });
        }
        let level = self.powered && !self.in_reset && now % TE_PERIOD_NS < TE_HIGH_NS;
        if level != self.te_level {
            self.te_level = level;
            self.te_edges.push(esp_soc::board::BoardEdge { cycle: now, pin: PIN_LCD_TE, level });
        }
    }
    fn named_pin(&self, name: &str) -> Option<u8> {
        match name { "ai" | "power" | "btn1" | "button" => Some(PIN_AI_BUTTON), "boot" | "btn2" => Some(PIN_BOOT_BUTTON), _ => None }
    }
    fn take_edges(&mut self) -> Vec<esp_soc::board::BoardEdge> { std::mem::take(&mut self.te_edges) }
    /// The next TE edge, in the same emulated nanoseconds `advance_to` takes, so an idle machine
    /// wakes for it instead of jumping over the 1 ms blanking pulse.
    fn next_deadline(&self) -> Option<esp_soc::board::VirtualCycle> {
        if !self.powered || self.in_reset { return None; }
        let now = self.te_now;
        let phase = now % TE_PERIOD_NS;
        let te = now - phase + if phase < TE_HIGH_NS { TE_HIGH_NS } else { TE_PERIOD_NS };
        Some(self.touch_release_at.map_or(te, |t| t.min(te)))
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn esp_periph::i2c::I2cDevice>)> {
        vec![(0, crate::cst9220::ADDRESS, Box::new(crate::cst9220::Cst9220::new(self.touch.clone(), Self::WIDTH as u16, Self::HEIGHT as u16))),
             (0, crate::bq27220::ADDRESS, Box::new(crate::bq27220::Bq27220::new())),
             (0, ES8311_ADDRESS, Box::new(esp_periph::i2c::Reg8Device::new("es8311", &[(0xfd, 0x83), (0xfe, 0x11)]))),
             (0, BMI270_ADDRESS, Box::new(esp_soc::devices::Bmi270::new(self.imu.clone(), 1_000_000_000))),
             (0, BMM150_ADDRESSES[0], Box::new(esp_soc::devices::Bmm150::new(Default::default()))),
             (0, BMM150_ADDRESSES[1], Box::new(esp_soc::devices::Bmm150::new(Default::default())))]
    }
    fn set_imu_motion(&mut self, mode: u32) -> bool { self.imu.mode.store(mode, std::sync::atomic::Ordering::Relaxed); true }
    fn imu_motion(&self) -> u32 { self.imu.mode.load(std::sync::atomic::Ordering::Relaxed) }
    fn touch(&mut self, x: u16, y: u16, down: bool) {
        *self.touch.lock().unwrap() = crate::cst9220::Touch { down, x, y };
        // A new report: INT falls (again, for a move while down) and, on release, rises 1 ms later.
        if self.touch_int_low { self.te_edges.push(esp_soc::board::BoardEdge { cycle: self.te_now, pin: PIN_TOUCH_INT, level: true }); }
        self.touch_int_low = true;
        self.te_edges.push(esp_soc::board::BoardEdge { cycle: self.te_now, pin: PIN_TOUCH_INT, level: false });
        self.touch_release_at = (!down).then_some(self.te_now + TOUCH_RELEASE_NS);
    }
    fn spi_transfer(&mut self, host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if host == 3 { return self.nand.transaction(tx, rx_len); }
        if host == 2 {
            if self.powered && !self.in_reset { self.panel.transaction(tx); } else { self.dropped += 1; }
        }
        vec![0xff; rx_len]
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        Some((Self::WIDTH as u32, Self::HEIGHT as u32, self.panel.frame.clone(), self.panel.pixels_written))
    }
    fn display_version(&self) -> u64 { self.panel.pixels_written }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn report(&self) -> String {
        let n = self.nand.stats;
        format!("[emu] mosaico: CO5300 {} RAMWR, {} pixels; power {}, reset {}; {} transactions dropped; gpio events {}; speaker amp {}\n[emu] mosaico: NAND {} page reads, {} programs, {} erases, {} refused",
                self.panel.frames, self.panel.pixels_written, if self.powered { "on" } else { "off" },
                if self.in_reset { "held" } else { "released" }, self.dropped, self.gpio_events, if self.amp { "on" } else { "off" }, n.reads, n.programs, n.erases, n.refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panel_takes_commands_only_when_powered_and_out_of_reset() {
        let mut b = Mosaico::new();
        let ramwr = [0x32, 0, 0x2c, 0, 0x12, 0x34];
        b.spi_transfer(2, &ramwr, 0);
        assert_eq!(b.panel.pixels_written, 0);
        assert_eq!(b.dropped, 1);
        b.gpio_changes(&[(PIN_LCD_POWER_N, false), (PIN_LCD_RESET, false)]);
        b.spi_transfer(2, &ramwr, 0);
        assert_eq!(b.panel.pixels_written, 0, "held in reset");
        b.gpio_changes(&[(PIN_LCD_RESET, true)]);
        b.spi_transfer(2, &ramwr, 0);
        assert_eq!(b.panel.frame[0], 0x1234);
    }
}
