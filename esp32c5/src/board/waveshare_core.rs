//! Exact Waveshare C5 core boards. Official pinned sources and conflicting
//! header-count statements are retained in their product knowledge records.
//! Zero: 0dc2567a87741639be311ff0d4a60f588f63b58f, GPIO27 WS2812 GRB.
//! Pico: cd413f3b76723516c8c264034f462381fe0ac0ac; Mini Kit:
//! 9eb18f4282e2e464b34b911ed6ba63cf28f22c9f; their RGB example explicitly
//! selects LED_COLOR_ORDER_RGB. WIFI6 KIT's ESP-IDF demo uses default GRB.
//! BOOT is GPIO28 active-low on all four. Pico's TCA9554 is 20h on G23/G24.
//! No RF, battery ADC, charger, USB bridge or PSRAM boot acceptance is implied.
use esp_soc::{BoardModel, devices::{Ws2812Chain, tca9554::{Tca9554, Tca9554State}}};
use esp_periph::i2c::I2cDevice;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreVariant { Zero, Pico, Mini, Wifi16, Wifi32 }
impl CoreVariant {
    pub fn from_name(name: &str) -> Option<Self> { Some(match name {
        "waveshare-c5-zero" => Self::Zero, "waveshare-c5-pico" => Self::Pico,
        "waveshare-c5-mini-kit" => Self::Mini, "waveshare-c5-wifi6-n16r8" => Self::Wifi16,
        "waveshare-c5-wifi6-n32r8" => Self::Wifi32, _ => return None,
    }) }
    pub fn name(self) -> &'static str { match self {
        Self::Zero => "waveshare-c5-zero", Self::Pico => "waveshare-c5-pico",
        Self::Mini => "waveshare-c5-mini-kit", Self::Wifi16 => "waveshare-c5-wifi6-n16r8",
        Self::Wifi32 => "waveshare-c5-wifi6-n32r8",
    } }
    pub fn memory_mib(self) -> (usize, usize) { match self { Self::Wifi16 => (16, 8), Self::Wifi32 => (32, 8), _ => (4, 0) } }
}

struct PicoExpander(Tca9554);
impl I2cDevice for PicoExpander {
    fn pins(&self) -> Option<(u8, u8)> { Some((23, 24)) }
    fn start(&mut self, read: bool) -> bool { self.0.start(read) }
    fn write(&mut self, value: u8) -> bool { self.0.write(value) }
    fn read(&mut self) -> u8 { self.0.read() }
    fn stop(&mut self) { self.0.stop(); }
}

pub struct WaveshareCore { variant: CoreVariant, led: Ws2812Chain, port: Arc<Mutex<Tca9554State>> }
impl WaveshareCore {
    pub fn new(variant: CoreVariant) -> Self {
        Self { variant, led: Ws2812Chain::new(1), port: Arc::new(Mutex::new(Tca9554State::default())) }
    }
}
impl BoardModel for WaveshareCore {
    fn name(&self) -> &'static str { self.variant.name() }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(28, true)] }
    fn named_pin(&self, name: &str) -> Option<u8> { matches!(name, "boot" | "button").then_some(28) }
    fn rmt_frame(&mut self, pin: u8, bits: &[bool]) {
        if pin != 27 || bits.len() < 24 { return; }
        self.led.from_bits(bits);
        if matches!(self.variant, CoreVariant::Pico | CoreVariant::Mini) { self.led.leds[0].swap(0, 1); }
    }
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { Some((&self.led.leds, self.led.updates.wrapping_add(1))) }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if self.variant == CoreVariant::Pico { vec![(0, 0x20, Box::new(PicoExpander(Tca9554::new(self.port.clone()))))] } else { vec![] }
    }
    fn report(&self) -> String {
        format!("[{}] BOOT G28; RGB G27 RMT {:?}, frames={}; {}. C5 SRAM subset only; ROM/Flash boot, RF, analog battery, charging and USB bridge unavailable",
            self.name(), self.led.leds[0], self.led.updates,
            if self.variant == CoreVariant::Pico { "TCA9554 20h G23/G24; six header IOs documented by official example; INT not connected to MCU" } else { "no onboard I2C peripheral" })
    }
}
