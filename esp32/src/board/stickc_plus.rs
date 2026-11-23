//! M5Stack StickC-Plus K016-P (ESP32-PICO-D4).
//! Wiring: https://docs.m5stack.com/en/core/StickC-Plus
//! LCD offsets/colour order: M5StickC-Plus src/utility/ST7789_Rotation.h.
//! AXP192 LDO2/LDO3 supply integration is deliberately explicit; there is no GPIO backlight.
use esp_periph::i2c::{
    axp192::{Axp192, Axp192State},
    mpu6886::{Mpu6886, Mpu6886Sample},
    I2cDevice,
};
use esp_soc::{
    board::{BoardEdge, SpiPins, VirtualCycle},
    devices::{
        bm8563::{Bm8563, Bm8563State},
        i2c_bitbang::I2cBitBang,
        DcsPanel,
    },
    BoardModel,
};
use std::sync::{Arc, Mutex};

pub const PIN_LCD_MOSI: u8 = 15;
pub const PIN_LCD_SCLK: u8 = 13;
pub const PIN_LCD_CS: u8 = 5;
pub const PIN_LCD_DC: u8 = 23;
pub const PIN_LCD_RST: u8 = 18;
pub const PIN_BUTTON_A: u8 = 37;
pub const PIN_BUTTON_B: u8 = 39;
pub const PIN_BUZZER: u8 = 2;
pub const PIN_LED: u8 = 10;
pub const PIN_I2C_SDA: u8 = 21;
pub const PIN_I2C_SCL: u8 = 22;
const CPU_HZ: u64 = 240_000_000;
const AUDIO_HZ: u32 = 16_000;

/// PMIC-observed digital rails. Initial values follow the shared powered-board
/// fixture, not AXP192 silicon reset or an analog battery simulation.
#[derive(Default)]
pub struct StickCPlusSupply {
    pub lcd_ic: bool,
    pub lcd_backlight: bool,
    reset_pending: bool,
}

/// Observe every PMIC byte write, including off/on writes before a board tick.
struct PowerDevice {
    inner: Axp192,
    state: Arc<Mutex<Axp192State>>,
    supply: Arc<Mutex<StickCPlusSupply>>,
}
impl I2cDevice for PowerDevice {
    fn start(&mut self, read: bool) -> bool {
        self.inner.start(read)
    }
    fn write(&mut self, byte: u8) -> bool {
        let ack = self.inner.write(byte);
        let state = self.state.lock().unwrap();
        let mut rails = self.supply.lock().unwrap();
        let ic = state.ldo_enabled(3) && state.ldo_mv(3) > 0;
        if rails.lcd_ic && !ic {
            rails.reset_pending = true;
        }
        rails.lcd_ic = ic;
        rails.lcd_backlight = state.ldo_enabled(2) && state.ldo_mv(2) > 0;
        ack
    }
    fn read(&mut self) -> u8 {
        self.inner.read()
    }
    fn stop(&mut self) {
        self.inner.stop();
    }
}

pub struct StickCPlus {
    pub panel: DcsPanel,
    pub pmic: Arc<Mutex<Axp192State>>,
    pub supply: Arc<Mutex<StickCPlusSupply>>,
    pub imu: Arc<Mutex<Mpu6886Sample>>,
    pub rtc: Arc<Mutex<Bm8563State>>,
    cs_high: bool,
    changes: u64,
    events: u64,
    spi_bytes: u64,
    spi_dropped: u64,
    i2c: I2cBitBang,
    i2c_levels: [bool; 2],
    edges: Vec<BoardEdge>,
    led: [[u8; 3]; 1],
    led_changes: u64,
    audio: Vec<i16>,
    audio_next: u64,
    audio_cycle: u64,
    tone: Option<(f64, u32)>,
    phase: f64,
}
impl Default for StickCPlus {
    fn default() -> Self {
        Self::new()
    }
}
impl StickCPlus {
    pub fn new() -> Self {
        let pmic = Arc::new(Mutex::new(Axp192State::stickc()));
        let state = pmic.lock().unwrap();
        let supply = Arc::new(Mutex::new(StickCPlusSupply {
            lcd_ic: state.ldo_enabled(3),
            lcd_backlight: state.ldo_enabled(2),
            reset_pending: false,
        }));
        drop(state);
        Self {
            pmic,
            panel: DcsPanel::st7789(),
            supply,
            imu: Arc::new(Mutex::new(Mpu6886Sample::default())),
            rtc: Arc::new(Mutex::new(Bm8563State::new(CPU_HZ))),
            cs_high: true,
            changes: 0,
            events: 0,
            spi_bytes: 0,
            spi_dropped: 0,
            i2c: I2cBitBang::new(),
            i2c_levels: [true; 2],
            edges: Vec::new(),
            led: [[0; 3]],
            led_changes: 0,
            audio: Vec::new(),
            audio_next: 0,
            audio_cycle: 0,
            tone: None,
            phase: 0.0,
        }
    }
    fn sync_power(&mut self) {
        let mut rails = self.supply.lock().unwrap();
        if std::mem::take(&mut rails.reset_pending) {
            self.panel.reset();
            // Power loss makes GRAM undefined; use deterministic cleared RAM,
            // unlike an ordinary DCS/GPIO reset which retains the controller RAM.
            self.panel.gram.fill(0);
            self.changes = self.changes.wrapping_add(1);
        }
    }
    fn capture_to(&mut self, cycle: u64) {
        if cycle < self.audio_cycle {
            return;
        }
        // Bounded cold capture (up to 10 s), no unbounded catch-up after a long idle.
        let step = CPU_HZ / u64::from(AUDIO_HZ);
        self.audio_next = self.audio_next.max(cycle.saturating_sub(CPU_HZ * 10));
        while self.audio_next < cycle {
            let value = if let Some((hz, duty)) = self
                .tone
                .filter(|(h, d)| h.is_finite() && *h > 0.0 && *d > 0 && *d < 65535)
            {
                let v = if self.phase < f64::from(duty) / 65535.0 {
                    4096
                } else {
                    -4096
                };
                self.phase = (self.phase + hz / f64::from(AUDIO_HZ)).fract();
                v
            } else {
                0
            };
            if self.audio.len() < AUDIO_HZ as usize * 10 {
                self.audio.push(value);
            }
            self.audio_next = self.audio_next.saturating_add(step);
        }
        self.audio_cycle = cycle;
    }
}
impl BoardModel for StickCPlus {
    fn name(&self) -> &'static str {
        "stickc-plus"
    }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        for &(pin, level) in changes {
            self.events += 1;
            match pin {
                PIN_LCD_CS => self.cs_high = level,
                PIN_LCD_DC => self.panel.dc = level,
                PIN_LCD_RST if !level => {
                    self.panel.reset();
                    self.changes = self.changes.wrapping_add(1);
                }
                PIN_LED => {
                    self.led = [if level { [0; 3] } else { [255, 0, 0] }];
                    self.led_changes = self.led_changes.wrapping_add(1);
                }
                _ => {}
            }
        }
    }
    fn gpio_output_at(
        &mut self,
        cycle: VirtualCycle,
        changes: &[(u8, bool)],
        enabled: u64,
        output: u64,
    ) {
        self.gpio_changes(changes);
        let released = |pin: u8| enabled & (1u64 << pin) == 0 || output & (1u64 << pin) != 0;
        let scl = released(PIN_I2C_SCL);
        let sda = self.i2c.drive(scl, released(PIN_I2C_SDA));
        for (i, (pin, level)) in [(PIN_I2C_SDA, sda), (PIN_I2C_SCL, scl)]
            .into_iter()
            .enumerate()
        {
            if self.i2c_levels[i] != level {
                self.i2c_levels[i] = level;
                self.edges.push(BoardEdge { cycle, pin, level });
            }
        }
    }
    fn uses_gpio_edges(&self) -> bool {
        true
    }
    fn uses_spi_pins(&self) -> bool {
        true
    }
    fn spi_transfer_pins(&mut self, _host: u8, pins: SpiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        self.sync_power();
        // Require actual panel wires (unrouted/other SPI traffic must not draw).
        let routed = pins.sclk & (1 << PIN_LCD_SCLK) != 0 && pins.mosi & (1 << PIN_LCD_MOSI) != 0;
        if self.supply.lock().unwrap().lcd_ic
            && (!self.cs_high || pins.cs & (1 << PIN_LCD_CS) != 0)
            && routed
        {
            self.spi_bytes += tx.len() as u64;
            for &b in tx {
                self.panel.byte(b);
            }
            self.changes = self.changes.wrapping_add(1);
        } else {
            self.spi_dropped += tx.len() as u64;
        }
        vec![0xff; rx_len]
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        // Attach once: controller 0/1 and software I2C share physical register state.
        if self
            .i2c
            .controller_devices(None)
            .iter()
            .all(|(a, _)| *a != 0x68)
        {
            self.i2c.attach(
                0x34,
                Box::new(PowerDevice {
                    inner: Axp192::new(self.pmic.clone()),
                    state: self.pmic.clone(),
                    supply: self.supply.clone(),
                }),
            );
            self.i2c
                .attach(0x68, Box::new(Mpu6886::new(self.imu.clone())));
            self.i2c
                .attach(0x51, Box::new(Bm8563::new(self.rtc.clone())));
        }
        (0..2)
            .flat_map(|bus| {
                self.i2c
                    .controller_devices(Some((PIN_I2C_SDA, PIN_I2C_SCL)))
                    .into_iter()
                    .map(move |(a, d)| (bus, a, d))
            })
            .collect()
    }
    fn input_levels(&self) -> Vec<(u8, bool)> {
        vec![
            (PIN_BUTTON_A, true),
            (PIN_BUTTON_B, true),
            (PIN_I2C_SDA, self.i2c_levels[0]),
            (PIN_I2C_SCL, self.i2c_levels[1]),
        ]
    }
    fn button_at(&mut self, _cycle: VirtualCycle, id: &str, pressed: bool) -> bool {
        if id != "power" {
            return false;
        }
        self.pmic.lock().unwrap().set_key(pressed);
        true
    }
    fn named_pin(&self, name: &str) -> Option<u8> {
        match name {
            "button-a" | "btn1" | "a" => Some(PIN_BUTTON_A),
            "button-b" | "btn2" | "b" => Some(PIN_BUTTON_B),
            _ => None,
        }
    }
    fn set_imu_sample(&mut self, accel_g: [f64; 3], gyro_dps: [f64; 3]) -> bool {
        if accel_g
            .iter()
            .chain(gyro_dps.iter())
            .any(|x| !x.is_finite())
        {
            return false;
        }
        let mut s = self.imu.lock().unwrap();
        s.accel_g = accel_g.map(|v| v.clamp(-16.0, 16.0));
        s.gyro_dps = gyro_dps.map(|v| v.clamp(-2000.0, 2000.0));
        s.version = s.version.wrapping_add(1);
        true
    }
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let p = self.supply.lock().unwrap();
        let pixels = if p.lcd_ic && p.lcd_backlight && !p.reset_pending {
            super::tdisplay::lcd_visible(&self.panel)
        } else {
            vec![0; 135 * 240]
        };
        drop(p);
        Some((135, 240, pixels, self.display_version()))
    }
    fn display_version(&self) -> u64 {
        let version = self.pmic.lock().unwrap().version;
        let s = self.supply.lock().unwrap();
        self.changes
            .wrapping_mul(4)
            .wrapping_add(u64::from(s.lcd_ic) * 2 + u64::from(s.lcd_backlight))
            .wrapping_add(version.wrapping_mul(8))
    }
    fn display_frames(&self) -> u64 {
        self.panel.frames
    }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> {
        Some((self.panel.gram.clone(), self.panel.cols, self.panel.rows))
    }
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> {
        Some((&self.led, self.led_changes))
    }
    fn gpio_events(&self) -> u64 {
        self.events
    }
    fn power_environment_at(
        &mut self,
        cycle: VirtualCycle,
        input: esp_periph::i2c::axp192::Axp192PowerInput,
    ) -> bool {
        let mut pmic = self.pmic.lock().unwrap();
        pmic.advance_to_us(cycle / (CPU_HZ / 1_000_000));
        pmic.set_power_input(input);
        true
    }
    fn advance_to(&mut self, cycle: VirtualCycle) {
        self.sync_power();
        self.rtc.lock().unwrap().advance_to(cycle);
        self.pmic
            .lock()
            .unwrap()
            .advance_to_us(cycle / (CPU_HZ / 1_000_000));
    }
    fn advance_inputs_with_clock(
        &mut self,
        cycle: VirtualCycle,
        clock: &dyn Fn(u8) -> Option<(f64, u32)>,
    ) {
        self.advance_to(cycle);
        self.capture_to(cycle);
        self.tone = clock(PIN_BUZZER);
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> {
        std::mem::take(&mut self.edges)
    }
    fn audio(&self) -> (&[i16], u32) {
        (&self.audio, AUDIO_HZ)
    }
    fn clear_audio_capture(&mut self) {
        self.audio.clear();
    }
    fn report(&self) -> String {
        let rails = self.supply.lock().unwrap();
        format!(
            "[emu] StickC-Plus K016-P: SPI {} accepted/{} dropped; LCD on={} sleeping={} backlight={} ic={} frames={} pixels={}",
            self.spi_bytes,
            self.spi_dropped,
            self.panel.on,
            self.panel.sleeping,
            rails.lcd_backlight,
            rails.lcd_ic,
            self.panel.frames,
            self.panel.pixels_written,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn routes() -> SpiPins {
        SpiPins {
            sclk: 1 << PIN_LCD_SCLK,
            mosi: 1 << PIN_LCD_MOSI,
            cs: 0,
            miso: None,
            keep_cs: false,
        }
    }
    fn cmd(b: &mut StickCPlus, c: u8, data: &[u8]) {
        b.gpio_changes(&[(PIN_LCD_CS, false), (PIN_LCD_DC, false)]);
        b.spi_transfer_pins(2, routes(), &[c], 0);
        b.gpio_changes(&[(PIN_LCD_DC, true)]);
        b.spi_transfer_pins(2, routes(), data, 0);
        b.gpio_changes(&[(PIN_LCD_CS, true)]);
    }
    fn power(b: &mut StickCPlus) {
        pmic_write(b, 0x12, 0x4d);
    }
    fn pmic_write(b: &mut StickCPlus, reg: u8, value: u8) {
        let mut devices = b.i2c_devices();
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 0 && *a == 0x34)
            .unwrap()
            .2
            .as_mut();
        write(d, reg, &[value]);
        b.advance_to(0);
    }
    fn init(b: &mut StickCPlus) {
        cmd(b, 0x11, &[]);
        cmd(b, 0x29, &[]);
        cmd(b, 0x3a, &[0x55]);
        cmd(b, 0x36, &[8]);
    }
    fn pixel(b: &mut StickCPlus, x: u16, y: u16, value: u16) {
        let x = x.to_be_bytes();
        let y = y.to_be_bytes();
        cmd(b, 0x2a, &[x[0], x[1], x[0], x[1]]);
        cmd(b, 0x2b, &[y[0], y[1], y[0], y[1]]);
        cmd(b, 0x2c, &value.to_be_bytes());
    }
    #[test]
    fn plus_has_its_own_factory_and_no_gpio_power_key() {
        for name in ["stickc-plus", "m5stack-k016-p"] {
            assert_eq!(
                super::super::make_board(name).unwrap().name(),
                "stickc-plus"
            );
        }
        assert!(super::super::make_board("stickc-plus2").is_none());
        let b = StickCPlus::new();
        assert_eq!(b.named_pin("button-a"), Some(37));
        assert_eq!(b.named_pin("button-b"), Some(39));
        assert_eq!(b.named_pin("power"), None);
        assert_eq!(b.named_pin("button-c"), None);
        assert!(b.input_levels().contains(&(37, true)));
        assert_eq!((b.display().unwrap().0, b.display().unwrap().1), (135, 240));
    }
    #[test]
    fn panel_requires_rails_select_and_exact_plus_wires() {
        let mut b = StickCPlus::new();
        pmic_write(&mut b, 0x12, 1);
        init(&mut b);
        assert!(!b.panel.on, "IC rail absent must ignore commands");
        power(&mut b);
        init(&mut b);
        pixel(&mut b, 52, 40, 0xf800);
        pixel(&mut b, 186, 279, 0x07e0);
        let frame = b.display().unwrap().2;
        assert_eq!(frame[0], 0xf800);
        assert_eq!(frame[135 * 240 - 1], 0x07e0);
        let before = b.panel.pixels_written;
        b.gpio_changes(&[(PIN_LCD_DC, true)]);
        b.spi_transfer_pins(2, routes(), &[0xff, 0xff], 0);
        assert_eq!(b.panel.pixels_written, before, "CS high");
        b.gpio_changes(&[(PIN_LCD_CS, false)]);
        b.spi_transfer_pins(
            2,
            SpiPins {
                sclk: 1 << 18,
                mosi: 1 << 19,
                ..routes()
            },
            &[0xff, 0xff],
            0,
        );
        b.spi_transfer_pins(2, SpiPins::default(), &[0xff, 0xff], 0);
        assert_eq!(b.panel.pixels_written, before, "wrong or unknown routes");
        let before = b.panel.pixels_written;
        b.gpio_changes(&[(PIN_LCD_CS, true)]);
        b.spi_transfer_pins(
            3,
            SpiPins {
                cs: 1 << PIN_LCD_CS,
                ..routes()
            },
            &[0xff, 0xff],
            0,
        );
        assert_eq!(
            b.panel.pixels_written,
            before + 1,
            "hardware CS also reaches physical panel"
        );
        pmic_write(&mut b, 0x12, 0x49);
        assert!(b.display().unwrap().2.iter().all(|x| *x == 0));
        pmic_write(&mut b, 0x12, 0x4d);
        assert_eq!(b.display().unwrap().2[0], 0xf800);
        cmd(&mut b, 0x10, &[]);
        assert!(b.display().unwrap().2.iter().all(|x| *x == 0));
    }
    #[test]
    fn panel_rotation_one_matches_official_driver_offsets() {
        let mut b = StickCPlus::new();
        power(&mut b);
        init(&mut b);
        cmd(&mut b, 0x36, &[0x68]);
        pixel(&mut b, 40, 53, 0xffff);
        assert_eq!(b.display().unwrap().2[134], 0xffff);
        pixel(&mut b, 279, 187, 0x07e0);
        assert_eq!(b.display().unwrap().2[239 * 135], 0x07e0);
        b.gpio_changes(&[(PIN_LCD_RST, false)]);
        assert!(b.display().unwrap().2.iter().all(|x| *x == 0));
    }
    #[test]
    fn axp192_ldo3_loss_resets_gram_even_when_restored_before_next_tick() {
        let mut b = StickCPlus::new();
        init(&mut b);
        pixel(&mut b, 52, 40, 0xf800);
        assert_eq!(b.display().unwrap().2[0], 0xf800);
        let resets = b.panel.resets;
        let mut devices = b.i2c_devices();
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 0 && *a == 0x34)
            .unwrap()
            .2
            .as_mut();
        write(d, 0x12, &[0x45]); // LDO3 off; backlight remains enabled
        assert!(b.display().unwrap().2.iter().all(|p| *p == 0));
        write(d, 0x12, &[0x4d]); // restore without an advance between the two I2C writes
        assert!(
            b.display().unwrap().2.iter().all(|p| *p == 0),
            "pending power loss cannot reveal stale pixels"
        );
        b.advance_to(0);
        assert_eq!(b.panel.resets, resets + 1);
        assert!(!b.panel.on);
        init(&mut b);
        assert_eq!(
            b.display().unwrap().2[0],
            0,
            "lost IC rail destroys previous GRAM"
        );
        pixel(&mut b, 52, 40, 0x07e0);
        assert_eq!(b.display().unwrap().2[0], 0x07e0);
        pmic_write(&mut b, 0x28, 0x90);
        let p = b.pmic.lock().unwrap();
        assert_eq!(p.ldo_mv(2), 2700);
        assert_eq!(p.ldo_mv(3), 1800);
    }
    #[test]
    fn power_control_is_axp_pek_release_latch_and_w1c_not_gpio() {
        let mut b = StickCPlus::new();
        let mut devices = b.i2c_devices();
        assert!(b.button_at(0, "power", true));
        assert_eq!(b.pmic.lock().unwrap().register(0x46) & 2, 0);
        assert!(b.button_at(240_000, "power", false));
        assert_eq!(b.pmic.lock().unwrap().register(0x46) & 2, 2);
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 1 && *a == 0x34)
            .unwrap()
            .2
            .as_mut();
        assert_eq!(read(d, 0x46, 1)[0] & 2, 2);
        assert_eq!(read(d, 0x46, 1)[0] & 2, 2);
        write(d, 0x46, &[2]);
        assert_eq!(read(d, 0x46, 1)[0] & 2, 0);
        assert_eq!(b.named_pin("power"), None);
        assert!(!b.button_at(0, "not-a-key", true));
    }
    fn write(d: &mut dyn I2cDevice, reg: u8, data: &[u8]) {
        assert!(d.start(false));
        assert!(d.write(reg));
        for v in data {
            assert!(d.write(*v));
        }
        d.stop();
    }
    fn read(d: &mut dyn I2cDevice, reg: u8, n: usize) -> Vec<u8> {
        d.start(false);
        d.write(reg);
        d.start(true);
        let v = (0..n).map(|_| d.read()).collect();
        d.stop();
        v
    }
    #[test]
    fn imu_registers_and_rtc_are_shared_on_both_routed_controllers() {
        let mut b = StickCPlus::new();
        let mut devices = b.i2c_devices();
        assert_eq!(devices.len(), 6);
        assert!(devices.iter().all(|(_, _, d)| d.pins() == Some((21, 22))));
        assert_eq!(devices.iter().filter(|(_, a, _)| *a == 0x34).count(), 2);
        assert!(b.set_imu_sample([1.0, -1.0, 0.5], [0.0; 3]));
        assert!(!b.set_imu_sample([f64::NAN, 0.0, 0.0], [0.0; 3]));
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 0 && *a == 0x68)
            .unwrap()
            .2
            .as_mut();
        assert_eq!(read(d, 0x75, 1), [0x19]);
        write(d, 0x6b, &[1]);
        assert_eq!(read(d, 0x3b, 6), [0x40, 0, 0xc0, 0, 0x20, 0]);
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 1 && *a == 0x68)
            .unwrap()
            .2
            .as_mut();
        assert_eq!(read(d, 0x6b, 1), [1]);
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 0 && *a == 0x51)
            .unwrap()
            .2
            .as_mut();
        write(d, 0, &[0]);
        write(d, 2, &[0x59, 0x59, 0x23, 0x28, 0, 0x02, 0x24]);
        b.advance_to(CPU_HZ);
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 1 && *a == 0x51)
            .unwrap()
            .2
            .as_mut();
        assert_eq!(read(d, 2, 4), [0, 0, 0, 0x29]);
    }
    #[test]
    fn gpio_buttons_are_active_low_and_led_is_not_plus2_buzzer() {
        use emu_core::Bus;
        use esp_soc::SocBus;
        let mut bus = crate::bus::SocBus::new(4 << 20, [0; 6]);
        bus.board = Box::new(StickCPlus::new());
        bus.attach_board_devices();
        assert_eq!(
            (bus.periph.efuse.ram.read(0x0c) >> 9) & 7,
            5,
            "PICO-D4 package, not default D0WD"
        );
        bus.write32(0x3ff49008, (2 << 12) | (1 << 9)).unwrap();
        bus.write32(0x3ff49010, (2 << 12) | (1 << 9)).unwrap();
        assert_ne!(bus.read32(0x3ff44040).unwrap() & (1 << 5), 0);
        assert_ne!(bus.read32(0x3ff44040).unwrap() & (1 << 7), 0);
        bus.gpio_set_input(37, false);
        assert_eq!(bus.read32(0x3ff44040).unwrap() & (1 << 5), 0);
        bus.gpio_set_input(37, true);
        assert_ne!(bus.read32(0x3ff44040).unwrap() & (1 << 5), 0);
        let mut b = StickCPlus::new();
        b.gpio_changes(&[(10, false)]);
        assert_eq!(b.leds().unwrap().0, [[255, 0, 0]]);
        b.gpio_changes(&[(10, true)]);
        assert_eq!(b.leds().unwrap().0, [[0, 0, 0]]);
    }
    #[test]
    fn buzzer_pcm_comes_from_guest_ledc_g2_and_stops_when_route_is_removed() {
        use emu_core::Bus;
        use esp_soc::SocBus;
        let mut bus = crate::bus::SocBus::new(4 << 20, [0; 6]);
        bus.board = Box::new(StickCPlus::new());
        bus.attach_board_devices();
        for (a, v) in [
            (0x3ff48070, 1 << 27),
            (0x3ff000c0, 1 << 11),
            (0x3ff59140, 8 | (16000 << 5) | (1 << 25)),
            (0x3ff59008, 64 << 4),
            (0x3ff5900c, (1 << 31) | (1 << 30) | (1 << 20) | (1 << 10)),
            (0x3ff59000, 4),
            (0x3ff49040, (2 << 12) | (1 << 9)),
            (0x3ff44538, 71),
        ] {
            bus.write32(a, v).unwrap();
        }
        bus.tick(48_000);
        bus.board.clear_audio_capture();
        bus.tick(240_000);
        let (pcm, rate) = bus.audio();
        assert_eq!(rate, AUDIO_HZ);
        assert!(pcm.contains(&4096) && pcm.contains(&-4096));
        bus.write32(0x3ff44538, 256).unwrap();
        bus.board.clear_audio_capture();
        bus.tick(240_000);
        assert!(bus.audio().0.iter().all(|x| *x == 0));
    }
    #[test]
    fn battery_adc_is_shared_and_runs_on_plus_guest_clock_not_input_receipt() {
        use esp_periph::i2c::axp192::Axp192PowerInput;
        let mut b = StickCPlus::new();
        let mut devices = b.i2c_devices();
        let d = devices
            .iter_mut()
            .find(|(bus, a, _)| *bus == 1 && *a == 0x34)
            .unwrap()
            .2
            .as_mut();
        b.advance_to(CPU_HZ * 39_999 / 1_000_000);
        assert_eq!(read(d, 0x78, 1)[0], 0);
        b.advance_to(CPU_HZ / 25);
        assert_eq!(read(d, 0x78, 1)[0], 0xe8);
        assert_eq!(read(d, 0x79, 1)[0], 15);
        b.pmic
            .lock()
            .unwrap()
            .set_power_input(Axp192PowerInput::LOW_BATTERY_3V0);
        assert_eq!(read(d, 0x78, 1)[0], 0xe8);
        b.advance_to(CPU_HZ * 2 / 25);
        assert_eq!(read(d, 0x78, 1)[0], 0xaa);
        assert_eq!(read(d, 0x79, 1)[0], 7);
        write(d, 0x82, &[0]);
        b.pmic
            .lock()
            .unwrap()
            .set_power_input(Axp192PowerInput::BATTERY_4V1);
        b.advance_to(CPU_HZ);
        assert_eq!(read(d, 0x78, 1)[0], 0xaa);
    }
}
