//! Thermal Camera ESP32 Module (45 and 90 degree optics), ESP32-S3 16F/8P.
//! The manufacturer Web Demo selects S3MINI_C: discrete active-low RGB on
//! GPIO1/38/2, BOOT0, SPI2 CLK6/MISO7/MOSI15, sensor CS16, calibration CS5,
//! SYSRST17 and power-down3. Retained original source receipts are in
//! docs/platform/research/waveshare-thermal-20261124.
//! MI0802 register semantics, calibration storage and thermal radiometry are
//! not implemented: no fabricated ID, temperature, ready interrupt or frame.
use esp_soc::{board::{BoardModel, SpiPins}, pins::PinRoutes};

#[derive(Default)]
pub struct WaveshareThermal {
    led: [[u8; 3]; 1],
    version: u64,
    controls: [Option<bool>; 4],
    sensor_transactions: u64,
    calibration_transactions: u64,
}

impl BoardModel for WaveshareThermal {
    fn name(&self) -> &'static str { "waveshare-s3-thermal-camera" }
    fn uses_gpio_edges(&self) -> bool { false }
    fn uses_gpio_routes(&self) -> bool { true }
    fn gpio_waveform_at(&mut self, _cycle: u64, routes: PinRoutes<'_>) {
        let next = [1, 38, 2].map(|pin| if routes.software_output(pin) == Some(false) { 255 } else { 0 });
        if self.led[0] != next {
            self.led[0] = next;
            self.version = self.version.wrapping_add(1);
        }
        self.controls = [16, 5, 17, 3].map(|pin| routes.software_output(pin));
    }
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { Some((&self.led, self.version)) }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(0, true)] }
    fn named_pin(&self, id: &str) -> Option<u8> { matches!(id, "boot" | "button").then_some(0) }
    fn uses_spi_pins(&self) -> bool { true }
    fn spi_transfer_pins(&mut self, host: u8, pins: SpiPins, _tx: &[u8], rx_len: usize) -> Vec<u8> {
        if host == 2 && pins.sclk & (1 << 6) != 0 && pins.mosi & (1 << 15) != 0 && pins.miso == Some(7) {
            match pins.cs & ((1 << 16) | (1 << 5)) {
                value if value == 1 << 16 => self.sensor_transactions += 1,
                value if value == 1 << 5 => self.calibration_transactions += 1,
                _ => {}
            }
        }
        // Explicit unsupported device response, never a synthetic successful ID.
        vec![0xff; rx_len]
    }
    fn report(&self) -> String {
        format!("[{}] RGB(discrete active-low G1/G38/G2)={:?} updates={} CS16/CS5/RST17/PD3={:?}; observed SPI attempts sensor={} calibration={}; MI0802/calibration/temperature/DATA_AV4/battery/USB/RF=unmodeled", self.name(), self.led[0], self.version, self.controls, self.sensor_transactions, self.calibration_transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_soc::pins::ChipPins;

    #[test]
    fn discrete_channels_require_physical_output_and_never_decode_ws2812() {
        let mut b = WaveshareThermal::default();
        let mut gpio = esp_periph::Gpio::new();
        let mut mux = esp_periph::RegRam::new();
        for pin in [1, 38, 2] { mux.write(4 + 4 * pin, 1 << 12); gpio.enable |= 1 << pin; }
        gpio.out = 1 << 38;
        b.gpio_waveform_at(0, ChipPins::S3.routes(&gpio, &mux));
        assert_eq!(b.led[0], [255, 0, 255]);
        gpio.enable &= !(1 << 1);
        mux.write(4 + 4 * 2, 0);
        b.gpio_waveform_at(1, ChipPins::S3.routes(&gpio, &mux));
        assert_eq!(b.led[0], [0; 3]);
        b.rmt_frame(1, &[true; 24]);
        assert_eq!(b.led[0], [0; 3]);
        assert!(b.display().is_none());
        assert!(b.camera_frame().is_none());
        assert_eq!(b.input_levels(), [(0, true)], "no invented sensor-ready input");
    }

    #[test]
    fn thermal_and_calibration_are_observed_but_unsupported_and_never_ack_with_ids() {
        let mut b = WaveshareThermal::default();
        let mut pins = SpiPins { sclk: 1 << 6, mosi: 1 << 15, miso: Some(7), cs: 1 << 16, keep_cs: false };
        assert_eq!(b.spi_transfer_pins(2, pins, &[0, 0], 2), [0xff; 2]);
        assert_eq!(b.sensor_transactions, 1);
        pins.cs |= 1 << 5;
        b.spi_transfer_pins(2, pins, &[0, 0], 2);
        assert_eq!(b.sensor_transactions, 1, "simultaneous selections are not prioritized");
        pins.cs = 1 << 5;
        pins.miso = Some(8);
        b.spi_transfer_pins(2, pins, &[0x9f], 3);
        assert_eq!(b.calibration_transactions, 0);
        pins.miso = Some(7);
        assert_eq!(b.spi_transfer_pins(2, pins, &[0x9f], 3), [0xff; 3]);
        assert_eq!(b.calibration_transactions, 1);
    }

    #[test]
    fn actual_lx7_guest_mmio_controls_discrete_rgb_and_reads_boot() {
        use emu_core::Core;
        use esp_soc::SocBus as _;
        let mut m = crate::machine([0; 6]);
        m.bus.board = Box::<WaveshareThermal>::default();
        m.bus.attach_board_devices();
        let entry = crate::bus::IRAM_LOW;
        m.bus.load_bytes(entry, &[0x32, 0x64, 0, 0x52, 0x24, 0]).unwrap();
        fn put(m: &mut crate::Machine, address: u32, value: u32) {
            let cpu = &mut m.cores[0];
            cpu.pc = crate::bus::IRAM_LOW;
            cpu.set_ar(4, address);
            cpu.set_ar(3, value);
            assert!(cpu.step(&mut m.bus).trap().is_none());
        }
        for pin in [1, 2, 38] {
            put(&mut m, 0x6000_9004 + 4 * pin, 1 << 12);
            put(&mut m, 0x6000_4554 + 4 * pin, 256);
        }
        put(&mut m, 0x6000_4024, (1 << 1) | (1 << 2));
        put(&mut m, 0x6000_4030, 1 << 6);
        put(&mut m, 0x6000_4014, 1 << 6);
        m.bus.refresh_board_outputs();
        assert_eq!(m.bus.board.leds().unwrap().0, [[255, 0, 255]]);
        put(&mut m, 0x6000_4028, 1 << 1);
        put(&mut m, 0x6000_900c, 0);
        m.bus.refresh_board_outputs();
        assert_eq!(m.bus.board.leds().unwrap().0, [[0; 3]], "released and unmuxed channels are off");
        put(&mut m, 0x6000_9004, (1 << 12) | (1 << 9));
        for high in [true, false, true] {
            m.bus.gpio_set_input(0, high);
            let cpu = &mut m.cores[0];
            cpu.pc = entry + 3;
            cpu.set_ar(4, 0x6000_403c);
            assert!(cpu.step(&mut m.bus).trap().is_none());
            assert_eq!(cpu.get_ar(5) & 1 != 0, high);
        }
    }
}
