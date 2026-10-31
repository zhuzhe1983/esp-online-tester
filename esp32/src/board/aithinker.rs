//! Fixed AI-Thinker ESP32-CAM wiring from Arduino-ESP32 2.0.17
//! CameraWebServer/camera_pins.h (CAMERA_MODEL_AI_THINKER).
//! This is a host frame source, not an I2S/DVP capture engine.
use esp_periph::i2c::I2cDevice;
use esp_soc::{board::BoardModel, camera::{Ov2640, Ov2640State, HostFrameSource}, picture::{Jpeg, JpegError, Picture, PixelFormat}};
use std::sync::{Arc, Mutex};

use esp_soc::board::BoardEdge;

pub const PIN_VSYNC: u8 = 25;
/// Approximate 10Hz frame cadence, with a 1ms active-high blanking pulse.
/// Not a sensor/PCLK timing claim. In particular the driver's 1us ISR check
/// must still see the pulse (esp32-camera v2.1.4 ll_cam.c:212-242).
pub const FRAME_CYCLES: u64 = crate::periph::CPU_HZ / 10;
pub const VSYNC_CYCLES: u64 = crate::periph::CPU_HZ / 1000;
/// Approximate frame-start back porch; not an OV2640/PCLK timing claim.
pub const ARM_WINDOW: u64 = VSYNC_CYCLES;

pub const PIN_PWDN: u8 = 32;
pub const PIN_XCLK: u8 = 0;
/// The module has no reset wire. Never interpret -1 as GPIO 255 or GPIO 0.
pub const PIN_RESET: Option<u8> = None;

/// esp32-camera v2.1.4 ov2640_regs.h: IMAGE_MODE_JPEG_EN (0x10), optionally with
/// IMAGE_MODE_HREF_VSYNC (0x02); ov2640_settings_jpeg3 writes 0x12.
fn jpeg_mode(image_mode: u8) -> bool { image_mode & !0x02 == 0x10 }

pub struct AiThinkerCam {
    sensor: Arc<Mutex<Ov2640State>>,
    source: HostFrameSource,
    powered_down: Arc<Mutex<bool>>,
    now: u64,
    next: Option<u64>,
    vsync: bool,
    edges: Vec<BoardEdge>,
    frame_start: Option<u64>,
    unsupported_reported: bool,
    jpeg_reported: bool,
}
impl Default for AiThinkerCam { fn default() -> Self { Self { sensor: Default::default(), source: Default::default(), powered_down: Arc::new(Mutex::new(true)), now: 0, next: None, vsync: false, edges: Vec::new(), frame_start: None, unsupported_reported: false, jpeg_reported: false } } }

struct Sccb { sensor: Arc<Mutex<Ov2640>>, powered_down: Arc<Mutex<bool>> }
// Power is shared with the SCCB device so a powered-down sensor does not ACK.
// The board's output callbacks maintain this independently of frame requests.
// One sensor sits on GPIO26/27: either I2C controller routed there reaches the
// same registers. SCCB_Init uses CONFIG_SCCB_HARDWARE_I2C_PORT (default I2C1);
// the C8 fixture sets PORT0 in sdkconfig.defaults.
impl I2cDevice for Sccb {
    fn pins(&self) -> Option<(u8, u8)> { Some((26, 27)) }
    fn start(&mut self, read: bool) -> bool { !*self.powered_down.lock().unwrap() && self.sensor.lock().unwrap().start(read) }
    fn write(&mut self, byte: u8) -> bool { self.sensor.lock().unwrap().write(byte) }
    fn read(&mut self) -> u8 { self.sensor.lock().unwrap().read() }
    fn stop(&mut self) { self.sensor.lock().unwrap().stop(); }
}

impl BoardModel for AiThinkerCam {
    fn name(&self) -> &'static str { "ai-thinker-cam" }
    fn input_levels(&self) -> Vec<(u8, bool)> { vec![(PIN_VSYNC, self.vsync)] }
    fn next_deadline(&self) -> Option<u64> { self.next }
    fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn camera_dvp_pins(&self) -> Option<[u8; 11]> { Some([5, 18, 19, 21, 36, 39, 34, 35, 22, 23, 25]) }
    fn camera_frame_start(&mut self, claim: bool) -> Option<u64> {
        if claim { self.frame_start.take() } else { self.frame_start }
    }
    fn invalidate_camera_frame_start(&mut self) { self.frame_start = None; }
    fn advance_to(&mut self, cycle: u64) {
        if cycle < self.now { return; }
        while let Some(at) = self.next.filter(|&at| at <= cycle) {
            self.vsync = !self.vsync;
            if !self.vsync { self.frame_start = Some(at); }
            self.edges.push(BoardEdge { cycle: at, pin: PIN_VSYNC, level: self.vsync });
            self.next = at.checked_add(if self.vsync { VSYNC_CYCLES } else { FRAME_CYCLES - VSYNC_CYCLES });
        }
        self.now = cycle;
    }
    fn advance_inputs_with_clock(&mut self, cycle: u64, clock: &dyn Fn(u8) -> Option<(f64, u32)>) {
        if cycle < self.now { return; }
        let streaming = self.sensor.lock().unwrap().streaming;
        let live = clock(PIN_XCLK).is_some_and(|(hz, duty)| hz.is_finite() && hz > 0.0 && duty > 0 && duty < 65535);
        if !*self.powered_down.lock().unwrap() && streaming && live {
            if self.next.is_none() { self.next = cycle.checked_add(FRAME_CYCLES); }
            self.advance_to(cycle);
        } else {
            self.next = None;
            self.frame_start = None;
            if self.vsync {
                self.vsync = false;
                self.edges.push(BoardEdge { cycle, pin: PIN_VSYNC, level: false });
            }
            self.now = cycle;
        }
    }
    fn gpio_output_at(&mut self, _: u64, _: &[(u8, bool)], enabled: u64, output: u64) {
        // A floating/unconfigured PWDN is conservatively inactive. The guest must
        // explicitly drive it low; no reset GPIO is attached on this board.
        *self.powered_down.lock().unwrap() = enabled & (1 << PIN_PWDN) == 0 || output & (1 << PIN_PWDN) != 0;
    }
    fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
        let sensor = Arc::new(Mutex::new(Ov2640::new(self.sensor.clone())));
        (0..2).map(|port| (port, 0x30, Box::new(Sccb { sensor: sensor.clone(), powered_down: self.powered_down.clone() }) as Box<dyn I2cDevice>)).collect()
    }
    fn set_camera_picture(&mut self, p: Picture) -> Result<(), &'static str> { self.source.set_camera_picture(p) }
    fn camera_preview(&self, w: u32, h: u32) -> Option<Vec<u8>> { self.source.camera_preview(w, h) }
    fn camera_frame_with_clock(&mut self, clock: &dyn Fn(u8) -> Option<(f64, u32)>) -> Option<(u32, u32, Arc<Vec<u8>>)> {
        let config = *self.sensor.lock().unwrap();
        let (hz, duty) = clock(PIN_XCLK)?;
        if *self.powered_down.lock().unwrap() || !config.streaming || !hz.is_finite() || hz <= 0.0 || duty == 0 || duty >= 65535 { return None; }
        // esp32-camera v2.1.4 ov2640_settings.h: grayscale requests use
        // ov2640_settings_yuv422, exactly the same two-byte wire format as YUV.
        let format = match config.image_mode {
            0 => PixelFormat::Yuyv,
            8 => PixelFormat::Rgb565,
            mode if jpeg_mode(mode) => {
                // No encoder: only host JPEG of exactly the configured size is a frame.
                let frame = self.source.jpeg_frame(config.width, config.height);
                if frame.is_none() && !self.jpeg_reported {
                    eprintln!("[camera] JPEG output {}x{} has no host JPEG of that size", config.width, config.height);
                    self.jpeg_reported = true;
                }
                return frame;
            }
            _ => {
                if !self.unsupported_reported {
                    eprintln!("[camera] unsupported sensor output: IMAGE_MODE {:#x}; classic ESP32-CAM supports raw RGB565/YUV422 and host-encoded JPEG", config.image_mode);
                    self.unsupported_reported = true;
                }
                return None;
            }
        };
        self.source.frame(config.width, config.height, format, None)
    }
    fn set_camera_jpeg(&mut self, jpeg: Jpeg) -> Result<(), JpegError> {
        let config = *self.sensor.lock().unwrap();
        self.source.set_camera_jpeg(jpeg, (config.width, config.height))
    }
    fn camera_frame_is_jpeg(&self) -> bool { jpeg_mode(self.sensor.lock().unwrap().image_mode) }
}
