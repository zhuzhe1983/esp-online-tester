//! M5Stack Tough K034, classic ESP32. Wiring: Tough main schematic and
//! docs/platform/evidence/tough/sources.json. ILI9342C on GPIO18/23/38
//! with CS5/DC15; AXP192 LDO2 feeds the panel, GPIO4 releases the shared
//! LCD/touch reset and LDO3 powers the backlight. ToughC5 is a separate board.
use crate::periph::CPU_HZ;
use esp_periph::i2c::{axp192::{AXP192_ADDRESS, Axp192, Axp192State}, I2cDevice};
use esp_soc::{
    BoardModel,
    board::{BoardEdge, SpiPins},
    devices::{DcsPanel, bm8563::{Bm8563, Bm8563State}, i2c_bitbang::I2cBitBang},
};
use std::sync::{Arc, Mutex};

const WIDTH: u16 = 320;
const HEIGHT: u16 = 240;
const LCD_CLK: u8 = 18;
const LCD_MOSI: u8 = 23;
const LCD_MISO: u8 = 38;
const LCD_CS: u8 = 5;
const LCD_DC: u8 = 15;
const INTERNAL_I2C: (u8, u8) = (21, 22);
const TOUCH_ADDR: u8 = 0x2e;
const TOUCH_IRQ: u8 = 39;
const RTC_ADDR: u8 = 0x51;
const KEYS: [(&str, u16); 3] = [("btn-a", 54), ("btn-b", 160), ("btn-c", 266)];

#[derive(Default)]
struct Touch {
    screen: Option<(u16, u16)>,
    keys: [bool; 3],
    irq_mode: bool,
    reset_count: u64,
}

impl Touch {
    fn contact(&self) -> Option<(u16, u16)> {
        // Centers of the lower touch regions in M5Tough M5Button.h (2dc8ad31).
        self.screen.or_else(|| self.keys.iter().position(|pressed| *pressed).map(|i| (KEYS[i].1, 228)))
    }

    fn sync_reset(&mut self, count: u64) {
        if self.reset_count != count {
            self.reset_count = count;
            self.irq_mode = false;
        }
    }
}

struct Chsc6540 {
    state: Arc<Mutex<Touch>>,
    pmic: Arc<Mutex<Axp192State>>,
    pointer: u8,
    address: bool,
    packet: [u8; 5],
}

impl Chsc6540 {
    fn new(state: Arc<Mutex<Touch>>, pmic: Arc<Mutex<Axp192State>>) -> Self {
        Self { state, pmic, pointer: 0, address: true, packet: [0; 5] }
    }
}

impl I2cDevice for Chsc6540 {
    fn start(&mut self, read: bool) -> bool {
        let pmic = self.pmic.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        state.sync_reset(pmic.panel_reset_count());
        if !pmic.panel_ready() { return false; }

        if read {
            self.packet = match state.contact() {
                Some((x, y)) => {
                    let [xh, xl] = x.to_be_bytes();
                    let [yh, yl] = y.to_be_bytes();
                    [1, xh & 15, xl, yh & 15, yl]
                }
                None => [0; 5],
            };
        } else {
            self.address = true;
        }
        true
    }

    fn write(&mut self, value: u8) -> bool {
        if self.address {
            self.pointer = value;
            self.address = false;
        } else {
            // M5GFX Touch_CHSC6540.inl (28d47949), interrupt-mode command.
            if self.pointer == 0x5a && value == 0x5a { self.state.lock().unwrap().irq_mode = true; }
            self.pointer = self.pointer.wrapping_add(1);
        }
        true
    }

    fn read(&mut self) -> u8 {
        // 0x02 = report count, 0x03..=0x06 = the first point.
        let value = self.pointer.checked_sub(0x02)
            .and_then(|index| self.packet.get(usize::from(index))).copied().unwrap_or(0);
        self.pointer = self.pointer.wrapping_add(1);
        value
    }
}

pub struct M5Tough {
    panel: DcsPanel,
    panel_reset_count: u64,
    panel_command: u8,
    read_index: u8,
    read_at: usize,
    read_started: bool,
    video_version: u64,
    gpio_events: u64,
    spi_bytes: u64,
    spi_dropped: u64,
    pmic: Arc<Mutex<Axp192State>>,
    touch: Arc<Mutex<Touch>>,
    touch_active: bool,
    irq_high: bool,
    edges: Vec<BoardEdge>,
    rtc: Arc<Mutex<Bm8563State>>,
    rtc_last: u64,
}

impl Default for M5Tough { fn default() -> Self { Self::new() } }

impl M5Tough {
    pub fn new() -> Self {
        Self {
            panel: DcsPanel::new(usize::from(WIDTH), usize::from(HEIGHT)),
            panel_reset_count: 0, panel_command: 0, read_index: 0, read_at: 0, read_started: false,
            video_version: 0, gpio_events: 0, spi_bytes: 0, spi_dropped: 0,
            pmic: Arc::new(Mutex::new(Axp192State::new(CPU_HZ))),
            touch: Arc::new(Mutex::new(Touch::default())),
            touch_active: false, irq_high: true, edges: Vec::new(),
            rtc: Arc::new(Mutex::new(Bm8563State::new(CPU_HZ))), rtc_last: 0,
        }
    }

    fn touch_irq_high(&self) -> bool {
        let pmic = self.pmic.lock().unwrap();
        let mut touch = self.touch.lock().unwrap();
        touch.sync_reset(pmic.panel_reset_count());
        !(pmic.panel_ready() && touch.irq_mode && touch.contact().is_some())
    }

    fn update_irq(&mut self, cycle: u64) {
        let high = self.touch_irq_high();
        if self.irq_high != high {
            self.irq_high = high;
            self.edges.push(BoardEdge { cycle, pin: TOUCH_IRQ, level: high });
        }
    }

    fn sync_panel(&mut self) -> bool {
        let pmic = self.pmic.lock().unwrap();
        let count = pmic.panel_reset_count();
        if self.panel_reset_count != count {
            self.panel.reset();
            self.panel_reset_count = count;
            self.panel_command = 0;
            self.read_index = 0;
            self.read_at = 0;
            self.read_started = false;
            self.video_version = self.video_version.wrapping_add(1);
        }
        pmic.panel_ready()
    }

    fn visible(&self) -> bool {
        let pmic = self.pmic.lock().unwrap();
        pmic.panel_ready() && pmic.rails().ldo3 && pmic.panel_reset_count() == self.panel_reset_count
            && self.panel.on && !self.panel.sleeping
    }
}

impl BoardModel for M5Tough {
    fn name(&self) -> &'static str { "m5-tough" }

    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        self.gpio_events += changes.len() as u64;
        for &(pin, level) in changes {
            if pin == LCD_DC { self.panel.dc = level; }
        }
    }

    fn gpio_events(&self) -> u64 { self.gpio_events }
    fn uses_spi_pins(&self) -> bool { true }

    fn spi_transfer_pins(&mut self, _host: u8, pins: SpiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if !self.sync_panel() || pins.sclk & (1 << LCD_CLK) == 0 || pins.cs & (1 << LCD_CS) == 0
            || (!tx.is_empty() && pins.mosi & (1 << LCD_MOSI) == 0) {
            self.spi_dropped += tx.len() as u64;
            return vec![0xff; rx_len];
        }

        self.spi_bytes += tx.len() as u64;
        let before = (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets, self.panel.pixels_written);
        for &byte in tx {
            if !self.panel.dc {
                self.panel_command = byte;
                self.read_at = 0;
                self.read_started = false;
            } else if self.panel_command == 0xd9 {
                self.read_index = byte & 15;
            }
            self.panel.byte(byte);
        }
        if before != (self.panel.on, self.panel.sleeping, self.panel.madctl, self.panel.resets, self.panel.pixels_written) {
            self.video_version = self.video_version.wrapping_add(1);
        }
        if pins.miso != Some(LCD_MISO) { return vec![0xff; rx_len]; }

        // ILI9342C RDDID begins with a dummy clock. Legacy M5GFX b9639027
        // submits that clock separately before its 32-bit read. The classic
        // SPI adapter rounds a one-bit receive to one byte.
        if rx_len == 1 && tx.is_empty() && self.panel_command == 0x04 && !self.read_started {
            self.read_started = true;
            return vec![0];
        }
        self.read_started |= rx_len > 0;

        // ILITEK ILI9342C V102 §§8.2.3 and 8.2.46–48, also used by StackChan.
        (0..rx_len).map(|_| {
            let indexed = self.read_at.saturating_add(usize::from(self.read_index.saturating_sub(1)));
            let value = match self.panel_command {
                0x04 => [0xe3, 0, 0].get(self.read_at).copied().unwrap_or(0),
                0xda => 0xe3,
                0xd3 => [0, 0x93, 0x42].get(indexed).copied().unwrap_or(0),
                _ => 0,
            };
            self.read_at = self.read_at.saturating_add(1);
            value
        }).collect()
    }

    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        // M5Tough and M5Unified can use either master. Both route to this same
        // physical pair, with one register pointer per slave.
        let mut bus = I2cBitBang::new();
        bus.attach(AXP192_ADDRESS, Box::new(Axp192::new(self.pmic.clone())));
        bus.attach(TOUCH_ADDR, Box::new(Chsc6540::new(self.touch.clone(), self.pmic.clone())));
        bus.attach(RTC_ADDR, Box::new(Bm8563::new(self.rtc.clone())));
        (0..2).flat_map(|master| bus.controller_devices(Some(INTERNAL_I2C)).into_iter()
            .map(move |(address, dev)| (master, address, dev))).collect()
    }

    fn touch(&mut self, x: u16, y: u16, down: bool) {
        let mut touch = self.touch.lock().unwrap();
        touch.screen = down.then_some((x.min(WIDTH - 1), y.min(HEIGHT - 1)));
        self.touch_active = touch.contact().is_some();
        drop(touch);
        self.irq_high = self.touch_irq_high();
    }

    fn touch_irq(&self) -> Option<(u8, bool)> { Some((TOUCH_IRQ, self.irq_high)) }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(TOUCH_IRQ, self.irq_high)] }

    fn button_at(&mut self, cycle: u64, id: &str, pressed: bool) -> bool {
        let Some(index) = KEYS.iter().position(|(key, _)| *key == id) else { return false; };
        let mut touch = self.touch.lock().unwrap();
        touch.keys[index] = pressed;
        self.touch_active = touch.contact().is_some();
        drop(touch);
        self.update_irq(cycle);
        true
    }

    fn advance_to(&mut self, cycle: u64) {
        if self.touch_active || !self.irq_high { self.update_irq(cycle); }
        if cycle.saturating_sub(self.rtc_last) >= CPU_HZ {
            self.rtc.lock().unwrap().advance_to(cycle);
            self.rtc_last = cycle;
        }
    }

    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }

    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
        let visible = self.visible();
        let pixels = if visible {
            self.panel.gram.clone()
        } else {
            vec![0; usize::from(WIDTH) * usize::from(HEIGHT)]
        };
        let version = self.video_version.wrapping_mul(2) | u64::from(visible);
        Some((u32::from(WIDTH), u32::from(HEIGHT), pixels, version))
    }

    fn display_version(&self) -> u64 { self.video_version.wrapping_mul(2) | u64::from(self.visible()) }
    fn display_frames(&self) -> u64 { self.panel.frames }
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> {
        Some((self.panel.gram.clone(), usize::from(WIDTH), usize::from(HEIGHT)))
    }
    fn report(&self) -> String {
        format!("[emu] m5-tough: ILI9342C {} RAMWR, {} pixels, madctl={:#04x}, lit={}, spi {} received/{} dropped; CHSC6540 GPIO{}={} reset_count={}",
            self.panel.frames, self.panel.pixels_written, self.panel.madctl, self.visible(), self.spi_bytes, self.spi_dropped,
            TOUCH_IRQ, u8::from(self.irq_high), self.panel_reset_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(devices: &mut [(u8, u8, Box<dyn I2cDevice>)], bus: u8, address: u8) -> &mut dyn I2cDevice {
        devices.iter_mut().find(|item| item.0 == bus && item.1 == address).unwrap().2.as_mut()
    }

    fn write(dev: &mut dyn I2cDevice, bytes: &[u8]) {
        assert!(dev.start(false));
        for &byte in bytes { assert!(dev.write(byte)); }
        dev.stop();
    }

    fn read(dev: &mut dyn I2cDevice, reg: u8, n: usize) -> Vec<u8> {
        assert!(dev.start(false));
        assert!(dev.write(reg));
        assert!(dev.start(true));
        let data = (0..n).map(|_| dev.read()).collect();
        dev.stop();
        data
    }

    fn power(dev: &mut dyn I2cDevice) {
        write(dev, &[0x12, 0x0c]);
        write(dev, &[0x95, 0x04, 0x02]);
    }

    fn route() -> SpiPins {
        SpiPins { sclk: 1 << LCD_CLK, mosi: 1 << LCD_MOSI, cs: 1 << LCD_CS, miso: Some(LCD_MISO) }
    }

    fn dcs(board: &mut M5Tough, pins: SpiPins, cmd: u8, params: &[u8], rx: usize) -> Vec<u8> {
        board.gpio_changes(&[(LCD_DC, false)]);
        let result = board.spi_transfer_pins(3, pins, &[cmd], rx);
        board.gpio_changes(&[(LCD_DC, true)]);
        if !params.is_empty() { board.spi_transfer_pins(3, pins, params, 0); }
        result
    }

    #[test]
    fn panel_accepts_only_selected_and_powered_display_and_gates_backlight() {
        let mut board = M5Tough::new();
        let mut devices = board.i2c_devices();
        let pmic = device(&mut devices, 0, AXP192_ADDRESS);
        assert_eq!(dcs(&mut board, route(), 0x04, &[], 3), [0xff; 3]);
        power(pmic);
        assert_eq!(dcs(&mut board, route(), 0x04, &[], 3), [0xe3, 0, 0]);
        dcs(&mut board, route(), 0x04, &[], 0);
        assert_eq!(board.spi_transfer_pins(3, route(), &[], 1), [0]);
        assert_eq!(board.spi_transfer_pins(3, route(), &[], 4), [0xe3, 0, 0, 0]);
        assert_eq!(dcs(&mut board, SpiPins { miso: Some(19), ..route() }, 0x04, &[], 3), [0xff; 3]);
        for pins in [
            SpiPins { cs: 1 << 4, ..route() },
            SpiPins { mosi: 1 << 1, ..route() },
            SpiPins { sclk: 1 << 1, ..route() },
        ] {
            assert_eq!(dcs(&mut board, pins, 0x04, &[], 3), [0xff; 3]);
        }
        dcs(&mut board, route(), 0x11, &[], 0);
        dcs(&mut board, route(), 0x29, &[], 0);
        dcs(&mut board, route(), 0x3a, &[0x55], 0);
        dcs(&mut board, route(), 0x2a, &[0, 17, 0, 17], 0);
        dcs(&mut board, route(), 0x2b, &[0, 6, 0, 6], 0);
        dcs(&mut board, route(), 0x2c, &[0xf8, 0x00], 0);
        let (w, h, pixels, version) = board.display().unwrap();
        assert_eq!((w, h, pixels[6 * 320 + 17]), (320, 240, 0xf800));
        write(pmic, &[0x12, 4]); // LDO2 stays on, LDO3 off.
        assert_ne!(board.display_version(), version);
        assert_eq!(board.display().unwrap().2[6 * 320 + 17], 0);
        write(pmic, &[0x12, 12]);
        assert_eq!(board.display().unwrap().2[6 * 320 + 17], 0xf800);
        write(pmic, &[0x96, 0]);
        write(pmic, &[0x96, 2]);
        assert_eq!(board.display().unwrap().2[6 * 320 + 17], 0);
        dcs(&mut board, route(), 0x29, &[], 0);
        assert!(board.panel.sleeping, "a short reset pulse clears the command state");
    }

    #[test]
    fn touch_register_snapshot_keys_and_release_use_the_same_irq() {
        let mut board = M5Tough::new();
        let mut devices = board.i2c_devices();
        assert!(!device(&mut devices, 0, TOUCH_ADDR).start(false));
        power(device(&mut devices, 1, AXP192_ADDRESS));
        let touch = device(&mut devices, 0, TOUCH_ADDR);
        assert_eq!(touch.pins(), Some(INTERNAL_I2C));
        write(touch, &[0x5a, 0x5a]);
        board.touch(319, 1, true);
        assert_eq!(board.touch_irq(), Some((39, false)));
        assert_eq!(read(touch, 2, 5), [1, 1, 63, 0, 1]);
        board.touch(0, 0, false);
        assert_eq!(board.touch_irq(), Some((39, true)));
        assert!(board.button_at(101, "btn-c", true));
        assert_eq!(board.take_edges(), [BoardEdge { cycle: 101, pin: 39, level: false }]);
        assert_eq!(read(touch, 2, 5), [1, 1, 10, 0, 228]);
        assert!(board.button_at(102, "btn-c", false));
        assert_eq!(board.take_edges(), [BoardEdge { cycle: 102, pin: 39, level: true }]);
        assert_eq!(read(touch, 2, 5), [0; 5]);
        assert!(!board.button_at(103, "power", true));
    }
}
