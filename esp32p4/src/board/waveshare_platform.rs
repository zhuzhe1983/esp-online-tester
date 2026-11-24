//! Waveshare P4 bare modules and carriers, without an implicitly attached DSI panel.
//! Exact schematics, BSP 2.0.1, DB BSPs and original factory manifests are recorded
//! in docs/evidence/waveshare-p4-platform-2026-11-23. Physical I2C7/8 and BOOT35;
//! the Core-DEV-KIT alone has WS2812B-0807 on GPIO34, and lacks the carrier codec/TF.
//! The codec is a control-register subset. Ethernet, C5/C6 radios, DSI/CSI and
//! analog power are not synthesized by this board adapter.
use std::sync::{Arc, Mutex};
use esp_periph::i2c::I2cDevice;
use esp_soc::{Board, BoardModel, board::BoardEdge, devices::{es8311::Es8311, Ws2812Chain}, pins::PinRoutes};

#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub name: &'static str,
    pub flash_mib: usize,
    pub psram_mib: usize,
    pub boot: Option<u8>,
    pub codec: bool,
    pub sd: bool,
    pub sd_power: Option<(u8, bool)>,
    pub rgb: Option<u8>,
}
const fn module(name: &'static str, flash_mib: usize) -> Profile {
    Profile { name, flash_mib, psram_mib: 32, boot: None, codec: false, sd: false, sd_power: None, rgb: None }
}
const fn carrier(name: &'static str, flash_mib: usize) -> Profile {
    Profile { name, flash_mib, psram_mib: 32, boot: Some(35), codec: true, sd: true, sd_power: None, rgb: None }
}
pub const PROFILES: &[Profile] = &[
    module("waveshare-p4-core", 16),
    module("waveshare-p4-core-32mb", 32),
    module("waveshare-p4-module-16mb", 16),
    module("waveshare-p4-module-32mb", 32),
    module("waveshare-p4-module-16mb-ipex", 16),
    module("waveshare-p4-module-32mb-ipex", 32),
    Profile { boot: Some(35), rgb: Some(34), ..module("waveshare-p4-core-dev-kit", 32) },
    carrier("waveshare-p4-nano", 16),
    carrier("waveshare-p4-module-dev-kit", 16),
    carrier("waveshare-p4-wifi6-dev-kit", 16),
    carrier("waveshare-p4-wifi6", 32),
    carrier("waveshare-p4-eth", 32),
    carrier("waveshare-p4-poe-eth", 32),
    carrier("waveshare-p4-pico", 32),
    carrier("waveshare-p4-wifi6-poe-eth", 32),
    Profile { sd_power: Some((45, false)), ..carrier("waveshare-p4-wifi6-db", 32) },
    Profile { sd_power: Some((45, false)), ..carrier("waveshare-p4-nano-wifi6-db", 16) },
];
pub fn profile(name: &str) -> Option<Profile> { PROFILES.iter().copied().find(|p| p.name == name) }
pub fn make_board(name: &str) -> Option<Board> { profile(name).map(|p| Box::new(WavesharePlatform::new(p)) as Board) }

struct Codec(Arc<Mutex<Es8311>>);
impl I2cDevice for Codec {
    fn pins(&self) -> Option<(u8, u8)> { Some((7, 8)) }
    fn start(&mut self, read: bool) -> bool { self.0.lock().unwrap_or_else(|e| e.into_inner()).start(read) }
    fn write(&mut self, byte: u8) -> bool { self.0.lock().unwrap_or_else(|e| e.into_inner()).write(byte) }
    fn read(&mut self) -> u8 { self.0.lock().unwrap_or_else(|e| e.into_inner()).read() }
    fn stop(&mut self) { self.0.lock().unwrap_or_else(|e| e.into_inner()).stop(); }
}
pub struct WavesharePlatform {
    profile: Profile,
    boot_high: bool,
    codec: Arc<Mutex<Es8311>>,
    rgb: Ws2812Chain,
    amp: bool,
    edges: Vec<BoardEdge>,
}
impl WavesharePlatform {
    pub fn new(profile: Profile) -> Self {
        Self { profile, boot_high: true, codec: Arc::new(Mutex::new(Es8311::new())),
            rgb: Ws2812Chain::new(1).with_gpio_clock(crate::periph::CPU_HZ as u32), amp: false, edges: Vec::new() }
    }
}
impl BoardModel for WavesharePlatform {
    fn name(&self) -> &'static str { self.profile.name }
    fn input_levels(&self) -> Vec<(u8, bool)> { self.profile.boot.into_iter().map(|pin| (pin, self.boot_high)).collect() }
    fn named_pin(&self, name: &str) -> Option<u8> { matches!(name, "boot" | "btn" | "btn1").then_some(self.profile.boot).flatten() }
    fn button_at(&mut self, cycle: u64, id: &str, pressed: bool) -> bool {
        let Some(pin) = self.named_pin(id) else { return false };
        let high = !pressed;
        if high != self.boot_high { self.boot_high = high; self.edges.push(BoardEdge { cycle, pin, level: high }); }
        true
    }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn uses_gpio_edges(&self) -> bool { self.profile.boot.is_some() || self.profile.rgb.is_some() }
    fn uses_gpio_routes(&self) -> bool { self.profile.codec || self.profile.rgb.is_some() }
    fn uses_gpio_waveform(&self) -> bool { self.profile.rgb.is_some() }
    fn gpio_waveform_at(&mut self, cycle: u64, routes: PinRoutes<'_>) {
        if self.profile.codec { self.amp = routes.software_output(53) == Some(true); }
        if let Some(pin) = self.profile.rgb {
            let level = routes.software_output(pin);
            self.rgb.gpio_drive(cycle, level.is_some(), level.unwrap_or(false));
        }
    }
    fn advance_to(&mut self, cycle: u64) { if self.profile.rgb.is_some() { self.rgb.advance_gpio(cycle); } }
    fn next_deadline(&self) -> Option<u64> { self.profile.rgb.and_then(|_| self.rgb.gpio_deadline()) }
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { self.profile.rgb.map(|_| (&self.rgb.leds[..], self.rgb.updates)) }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        if !self.profile.codec { return Vec::new(); }
        // Both masters can reach the same physical device, only when routed to 7/8.
        // A P4 reset does not recreate the external codec's control registers.
        [0, 1].into_iter().map(|bus| (bus, 0x18, Box::new(Codec(self.codec.clone())) as Box<dyn I2cDevice>)).collect()
    }
    fn report(&self) -> String {
        format!("[{}] {}F/{}P boot={:?} ES8311-control={} amp={} TF={} TF-power={:?} rgb={:?} ledUpdates={}; DSI/CSI/RMT/radio/ethernet/analog unmodeled",
            self.name(), self.profile.flash_mib, self.profile.psram_mib, self.profile.boot, self.profile.codec,
            self.amp, self.profile.sd, self.profile.sd_power, self.profile.rgb, self.rgb.updates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bare_modules_and_core_carrier_do_not_inherit_external_panels_or_codecs() {
        for p in PROFILES {
            let mut b = WavesharePlatform::new(*p);
            assert!(b.display().is_none());
            assert_eq!(b.i2c_devices().len(), if p.codec { 2 } else { 0 });
            assert_eq!(b.named_pin("boot"), p.boot);
            assert!(!b.button_at(0, "reset", true));
            assert_eq!(b.leds().is_some(), p.rgb.is_some());
        }
        for name in ["waveshare-p4-core", "waveshare-p4-core-32mb", "waveshare-p4-core-dev-kit"] {
            let p = profile(name).unwrap(); assert!(!p.sd && !p.codec);
        }
        for name in ["waveshare-p4-module-16mb", "waveshare-p4-module-32mb", "waveshare-p4-module-16mb-ipex", "waveshare-p4-module-32mb-ipex"] {
            let p = profile(name).unwrap(); assert!(!p.sd && !p.codec && p.boot.is_none());
        }
        assert!(profile("waveshare-p4-module").is_none(), "the ambiguous module name does not silently choose a capacity or antenna");
    }
    #[test]
    fn boot_and_shared_codec_state_survive_device_reattachment() {
        let mut b = WavesharePlatform::new(profile("waveshare-p4-nano").unwrap());
        assert!(b.button_at(25, "boot", true)); assert_eq!(b.input_levels(), [(35, false)]);
        let mut devices = b.i2c_devices();
        assert_eq!(devices[0].2.pins(), Some((7, 8)));
        assert!(devices[0].2.start(false)); assert!(devices[0].2.write(0x32)); assert!(devices[0].2.write(0x77)); devices[0].2.stop();
        assert!(devices[1].2.start(false)); assert!(devices[1].2.write(0x32)); assert!(devices[1].2.start(true)); assert_eq!(devices[1].2.read(), 0x77);
        let mut reattached = b.i2c_devices(); assert!(reattached[0].2.start(false)); assert!(reattached[0].2.write(0x32)); assert!(reattached[0].2.start(true)); assert_eq!(reattached[0].2.read(), 0x77);
        assert_eq!(b.input_levels(), [(35, false)]); assert!(b.button_at(50, "boot", false));
        assert_eq!(b.take_edges().iter().map(|e| (e.cycle, e.pin, e.level)).collect::<Vec<_>>(), [(25, 35, false), (50, 35, true)]);
    }
    #[test]
    fn core_dev_kit_led_requires_physical_gpio34_pulses_and_a_latch() {
        let mut b = WavesharePlatform::new(profile("waveshare-p4-core-dev-kit").unwrap());
        let mut g = esp_periph::Gpio::with_pins(57);
        let mut mux = esp_periph::RegRam::new();
        mux.write(4 + 4 * 34, 1 << 12); g.enable = 1 << 34;
        let mut cycle = 0;
        for byte in [0x12u8, 0x34, 0x56] {
            for shift in (0..8).rev() {
                let high = if byte & (1 << shift) != 0 { 288 } else { 144 };
                g.out = 1 << 34; b.gpio_waveform_at(cycle, crate::periph::GPIO_PINS.routes(&g, &mux));
                cycle += high;
                g.out = 0; b.gpio_waveform_at(cycle, crate::periph::GPIO_PINS.routes(&g, &mux));
                cycle += 450 - high;
            }
        }
        let due = b.next_deadline().unwrap();
        b.advance_to(due - 1); assert_eq!(b.rgb.updates, 0);
        b.advance_to(due); assert_eq!(b.leds(), Some((&[[0x34, 0x12, 0x56]][..], 1)));
        // This adapter does not turn an unimplemented P4 RMT transaction into a pixel.
        b.rmt_frame(34, &[true; 24]); assert_eq!(b.rgb.updates, 1);
        g.out = 1 << 34; b.gpio_waveform_at(due + 100, crate::periph::GPIO_PINS.routes(&g, &mux));
        g.enable = 0; b.gpio_waveform_at(due + 200, crate::periph::GPIO_PINS.routes(&g, &mux));
        assert!(b.next_deadline().is_none());
    }
}
