//! Boards around the SoC. The SoC model emits generic events (GPIO edges, RMT symbol streams,
//! SPI bytes, LCD frames, camera requests); a `BoardModel` interprets them as the devices wired to
//! the pins and offers what the UI and the scripts need back.
use esp_periph::i2c::I2cDevice;

pub type VirtualCycle = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardEdge {
    pub cycle: VirtualCycle,
    pub pin: u8,
    pub level: bool,
}

/// Physical routes for a single-lane SPI transaction. Output masks allow mirrored routes.
/// `cs` contains asserted active-low hardware selects and low, enabled software GPIO selects.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpiPins {
    pub sclk: u64,
    pub mosi: u64,
    pub miso: Option<u8>,
    pub cs: u64,
}

/// GPIO matrix masks at an LCD_CAM i80 transaction. Each data bit may drive several pads.
/// `dc_high` is the physical D/C level after the controller's phase polarity,
/// independent of the callback's logical command/data phase.
#[derive(Clone, Copy, Debug, Default)]
pub struct LcdI80Pins { pub data: [u64; 16], pub clock: u64, pub select: u64, pub width: u8, pub dc: u64, pub dc_high: bool }

/// What a board does with the SoC's pin-level activity.
pub trait BoardModel {
    fn name(&self) -> &'static str;
    /// GPIO output level changes, in order.
    fn gpio_changes(&mut self, _changes: &[(u8, bool)]) {}
    /// Output changes at the SoC bus cycle. The drive masks also expose output-enable
    /// transitions (including releasing a low pin), needed by open-drain pulse protocols.
    /// Called immediately after a GPIO output/enable write on chips with pin transport.
    fn gpio_output_at(&mut self, _cycle: VirtualCycle, changes: &[(u8, bool)], _enabled: u64, _output: u64) {
        if !changes.is_empty() { self.gpio_changes(changes); }
    }
    /// A completed RMT transmission, decoded to bits by the peripheral model, with the pin the
    /// GPIO matrix has that channel routed to. Drivers that take a fresh channel per refresh
    /// (the Arduino NeoPixel one does) make the channel meaningless; the pin names the strip.
    fn rmt_frame(&mut self, _pin: u8, _bits: &[bool]) {}
    /// Bytes a GP-SPI master (`host` = 2 or 3) shifted out on MOSI.
    fn spi_tx(&mut self, _host: u8, _data: &[u8]) {}
    /// One complete GP-SPI transaction. The default preserves transmit-only boards and models an
    /// unattached MISO line.
    fn spi_transfer(&mut self, host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        self.spi_tx(host, tx);
        vec![0xff; rx_len]
    }
    /// Opt in to physical SPI routes. Fixed boards avoid route decoding.
    fn uses_spi_pins(&self) -> bool { false }
    /// Pin-aware boards select their devices using these routes and return MISO bytes.
    /// The default preserves the controller-based callback.
    fn spi_transfer_pins(&mut self, host: u8, _pins: SpiPins, tx: &[u8], rx_len: usize) -> Vec<u8> {
        self.spi_transfer(host, tx, rx_len)
    }
    /// Opt in to UART route decoding and receive polling. Cached by `attach_board_devices`.
    fn uses_uart_pins(&self) -> bool { false }
    /// A completed UART byte, with the routing and baud at the FIFO write. Console output is
    /// independent. Inverted signals and bit-level serial timing are not modelled here.
    fn uart_tx(&mut self, _cycle: VirtualCycle, _route: crate::uart::UartRoute, _byte: u8) {}
    /// Completed device-to-chip bytes to deliver at the next device tick. Pin numbers name
    /// chip GPIOs. The callback receives the current bus cycle for pacing. Unrouted input is dropped; a baud mismatch raises UART FRM_ERR.
    fn uart_rx(&mut self, _cycle: VirtualCycle) -> Vec<crate::uart::UartInput> { Vec::new() }
    fn gpio_events(&self) -> u64 { 0 }
    /// Devices on the I2C buses: (bus, 7-bit address, device).
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> { Vec::new() }
    /// Give the board's camera a picture to look at (RGB888).
    fn set_camera_picture(&mut self, _p: crate::picture::Picture) -> Result<(), &'static str> { Err("no camera on this board") }
    /// Next camera frame in the sensor's register-selected wire format, with its size.
    /// Boards requiring an external clock use `camera_frame_with_clock` instead.
    fn camera_frame(&mut self) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)> { None }

    /// Small RGB preview of what the camera is looking at (for the UI), if a picture is loaded.
    fn camera_preview(&self, _w: u32, _h: u32) -> Option<Vec<u8>> { None }
    /// A complete frame from the LCD_CAM RGB interface (RGB565 little-endian, `w`x`h`).
    fn lcd_frame(&mut self, _w: u32, _h: u32, _rgb565: &[u8]) {}
    /// The board's display for the UI/PNG: (width, height, RGB565 pixels, change counter).
    fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> { None }
    /// Completed display frames (for the UI's statistics line).
    fn display_frames(&self) -> u64 { 0 }
    /// Cheap change counter of the display (`display().3` without building the frame).
    fn display_version(&self) -> u64 { 0 }
    /// Prefer waiting one push interval for a quiet pixel stream. The UI still publishes on
    /// the next opportunity during continuous changes so animation cannot starve.
    fn display_quiet_push(&self) -> bool { false }
    /// Page pushes (display, audio, input poll) per emulated second. A panel that is drawn on
    /// interactively wants more than the default; one that rescans a large frame does not.
    fn display_push_hz(&self) -> u64 { 50 }
    /// Opt in to interactive host publication before execution. Returns false if unsupported.
    fn set_smooth_display(&mut self, _on: bool) -> bool { false }
    /// Select a scripted IMU motion (0 = still) before execution. Returns false if unsupported.
    fn set_imu_motion(&mut self, _mode: u32) -> bool { false }
    /// The scripted IMU motion selected by `set_imu_motion` (0 when unsupported).
    fn imu_motion(&self) -> u32 { 0 }
    /// External IMU sample in the sensor frame (g and degrees/second), without advancing time.
    /// Returns false on boards without a host-fed sensor.
    fn set_imu_sample(&mut self, _accel_g: [f64; 3], _gyro_dps: [f64; 3]) -> bool { false }
    /// Explicit digital LoRa peer input. A supporting radio must match its guest
    /// configuration, receive state and physical power/reset/antenna gates.
    fn lora_receive_at(&mut self, _cycle: VirtualCycle, _packet: &crate::devices::sx1262::LoRaInput) -> bool { false }
    /// Cold observation, not a per-tick or RF/network callback.
    fn lora_snapshot(&self) -> Option<String> { None }
    /// Raw display memory for a debug PNG: (pixels, columns, rows).
    fn gram(&self) -> Option<(Vec<u16>, usize, usize)> { None }
    /// LED ring / strip: colours and a change counter.
    fn leds(&self) -> Option<(&[[u8; 3]], u64)> { None }
    /// Addressable LED modules besides `leds()`, each with the port it sits in and a change
    /// counter: (id, colours, updates). The UI draws one square grid per entry.
    fn led_grids(&self) -> Vec<(&'static str, &[[u8; 3]], u64)> { Vec::new() }
    /// Touch input from the UI (panel coordinates).
    fn touch(&mut self, _x: u16, _y: u16, _down: bool) {}
    /// Touch input observed at a specific bus horizon. Untimed boards use the ordinary input path.
    fn touch_at(&mut self, _cycle: VirtualCycle, x: u16, y: u16, down: bool) { self.touch(x, y, down); }
    /// Physical keyboard position, independent of UART text or host key symbols.
    /// Board mapping and scan hardware determine which guest key event is produced.
    fn matrix_key_at(&mut self, _cycle: VirtualCycle, _row: u8, _column: u8, _pressed: bool) -> bool { false }
    /// Current board-driven GPIO input levels, used to reconnect a persistent board after reset.
    fn input_levels(&self) -> Vec<(u8, bool)> { Vec::new() }
    /// Whether GPIO edge polling is needed. Cached by `attach_board_devices`.
    fn uses_gpio_edges(&self) -> bool { true }
    /// Use instruction-sized, unmodeled quanta for GPIO pulse decoding, overriding timing models.
    fn uses_gpio_waveform(&self) -> bool { false }
    /// Earliest autonomous transition strictly after the board's current cycle.
    fn next_deadline(&self) -> Option<VirtualCycle> { None }
    /// Advance monotonically through every board transition due by `cycle`.
    fn advance_to(&mut self, _cycle: VirtualCycle) {}
    /// Host GPIO drives to release after advancing the board.
    fn released_inputs(&mut self) -> Vec<u8> { Vec::new() }
    /// GPIO edges due by the last advance, including synchronous output responses.
    /// GPIO reads drain these even without elapsed cycles; retain future edges until due.
    fn take_edges(&mut self) -> Vec<BoardEdge> { Vec::new() }
    /// A pin by the name scripts and the UI use (`btn1`, `sw`, ...).
    fn named_pin(&self, _name: &str) -> Option<u8> { None }
    /// The rotary encoder's (CLK, DT) pins, if there is one.
    fn encoder(&self) -> Option<(u8, u8)> { None }
    /// Lines for the end-of-run report.
    fn report(&self) -> String { String::new() }
    /// Opt-in software drive after GPIO/IO_MUX writes and board attachment (including reset).
    /// Unlike raw latch changes, this observes route, mux and enable-only transitions.
    /// `uses_gpio_routes` enables this callback; `uses_gpio_waveform` additionally
    /// requires instruction-sized scheduling for pulse-width protocols.
    /// Peripheral-generated waveforms remain on their existing callbacks.
    fn gpio_waveform_at(&mut self, _cycle: VirtualCycle, _routes: crate::pins::PinRoutes<'_>) {}
    /// Lazy physical clock query; called by the capture adapter, never polled per tick.
    /// The callback returns routed PWM frequency and duty (0..=65535) at a GPIO.
    /// Appended to preserve the existing board vtable's hot-call offsets.
    fn camera_frame_with_clock(&mut self, _clock: &dyn Fn(u8) -> Option<(f64, u32)>) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)> { self.camera_frame() }
    /// Advance scheduled inputs, querying a physical clock only on boards that need it.
    /// This replaces the existing advance call, not an additional producer poll.
    fn advance_inputs_with_clock(&mut self, cycle: VirtualCycle, _clock: &dyn Fn(u8) -> Option<(f64, u32)>) { self.advance_to(cycle); }
    /// Parallel camera wiring: D0..D7, PCLK, HREF, VSYNC. None on camera-free boards.
    fn camera_dvp_pins(&self) -> Option<[u8; 11]> { None }
    /// Most recent unclaimed scheduled camera falling edge, never a forced-low edge.
    /// Claiming prevents restart from replaying the same physical frame.
    fn camera_frame_start(&mut self, _claim: bool) -> Option<VirtualCycle> { None }
    /// Physical route changes invalidate the fresh-frame opportunity.
    fn invalidate_camera_frame_start(&mut self) {}
    /// Observe routed GPIO writes without requiring pulse-width/instruction-sized scheduling.
    /// Appended to preserve existing vtable slots. Waveform boards keep their old behavior.
    fn uses_gpio_routes(&self) -> bool { self.uses_gpio_waveform() }
    /// Native SD socket: CLK,CMD,D0..D3 and current supply state. No socket by default.
    /// Appended host-only query: not polled in the CPU/tick hot path.
    fn sd_card_slot(&self) -> Option<([u8; 6], bool)> { None }
    /// Physical card insertion switch, independent of the host controller's card detect signal.
    fn set_sd_card_present(&mut self, _present: bool) {}
    /// Cold host input for a source-backed button unit on a board-owned physical connector.
    /// Returns the routed pin and its digital drive; None drive disconnects it.
    fn accessory_button_at(&mut self, _cycle: VirtualCycle, _model: &str, _port: &str, _connected: bool, _pressed: bool) -> Option<(u8, Option<bool>)> { None }
    /// Cold connection change for a board-owned output unit; never accepts user-selected pins.
    fn accessory_connection_at(&mut self, _cycle: VirtualCycle, _model: &str, _port: &str, _connected: bool) -> bool { false }
    /// Cold physical potentiometer input. Board owns the pin, transfer curve and power gate.
    fn accessory_analog_at(&mut self, _cycle: VirtualCycle, _model: &str, _port: &str, _connected: bool, _value: f32) -> Option<(u8, esp_periph::AnalogSource)> { None }
    /// Cold semantic input for board-owned controls behind expanders or a touch
    /// controller. The board owns physical wiring; no caller-selected pin/register.
    /// Appended to preserve existing hot-call vtable offsets.
    fn button_at(&mut self, _cycle: VirtualCycle, _id: &str, _pressed: bool) -> bool { false }
    /// Cold completed i80 phase, in wire word order; not a framebuffer or a tick callback.
    /// Appended to preserve hot vtable offsets. No callback on a failed DMA gather.
    fn lcd_i80(&mut self, _pins: LcdI80Pins, _data_phase: bool, _words: &[u16]) {}
    /// Opt in to lazy PWM snapshots before host display publication/capture, never per tick.
    fn uses_gpio_pwm(&self) -> bool { false }
    /// Observe the already-latched physical PWM duty without advancing peripheral time.
    /// Only derived visible output state may change here; bump display_version when it does.
    fn gpio_pwm_at(&mut self, _cycle: VirtualCycle, _pwm: &dyn Fn(u8) -> Option<(f64, u32)>) {}

}

pub type Board = Box<dyn BoardModel>;

/// A bare module: nothing on the pins, console only.
pub struct NoBoard;
impl BoardModel for NoBoard {
    fn name(&self) -> &'static str { "none" }
    fn uses_gpio_edges(&self) -> bool { false }
}
