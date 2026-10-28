//! Host-only StopWatch presentation. Firmware still writes the original GRAM.
//! Official V0.5 draws a screen as eight adjacent full-width windows (60 rows,
//! then 46). Keep the preceding picture until a top-to-bottom strip sequence
//! completes. This is not TE, scanout, or a promise about hardware timing.
use esp_soc::board::VirtualCycle;

type Window = [u16; 4];
const LAST_ROW: u16 = super::StopWatch::HEIGHT as u16 - 1;
// One existing UI push interval without pixels ends an abandoned/partial redraw.
// Checked only at host capture, never by a new guest tick/deadline callback.
const IDLE_LIMIT: VirtualCycle = crate::periph::CPU_HZ / 50;

#[derive(Default)]
pub(super) struct Presentation {
    frozen: Option<(Vec<u16>, u64)>,
    primed: Option<Window>,
    batch: Option<Batch>,
    last_write: VirtualCycle,
}

struct Batch { window: Window, pixels: u64 }
impl Batch {
    fn complete(&self) -> bool { self.pixels >= u64::from(self.window[1] - self.window[0] + 1) * u64::from(self.window[3] - self.window[2] + 1) }
}
fn full_width(w: Window) -> bool { w[0] == super::StopWatch::X_OFFSET && w[1] == w[0] + super::StopWatch::WIDTH as u16 - 1 && w[2] <= w[3] && w[3] <= LAST_ROW }
fn first_strip(w: Window) -> bool { full_width(w) && w[2] == 0 }

impl Presentation {
    pub(super) fn reset(&mut self) { *self = Self::default(); }
    pub(super) fn before(&mut self, window: Window, frame: &[u16], version: u64) {
        // CASET/RASET complete before RAMWR, even when its header and pixels share
        // a transfer. Save before the first write, including split CS headers.
        if self.batch.is_none() {
            if first_strip(window) {
                if self.primed != Some(window) || self.frozen.as_ref().is_none_or(|(_, v)| *v != version) {
                    self.frozen = Some((frame.to_vec(), version));
                    self.primed = Some(window);
                }
            } else { self.frozen = None; self.primed = None; }
        }
    }
    pub(super) fn after(&mut self, window: Window, old_frames: u64, frames: u64, old_pixels: u64, pixels: u64, cycle: VirtualCycle) {
        if frames < old_frames || pixels < old_pixels { self.reset(); return; }
        if frames != old_frames {
            if let Some(batch) = &self.batch {
                if first_strip(window) || (batch.complete() && full_width(window) && window[2] == batch.window[3] + 1) {
                    self.batch = Some(Batch { window, pixels: 0 });
                } else { self.reset(); }
            } else if first_strip(window) && self.primed == Some(window) {
                self.batch = Some(Batch { window, pixels: 0 });
            }
            if self.batch.is_some() { self.last_write = cycle; }
        }
        if pixels != old_pixels {
            self.last_write = cycle;
            if let Some(batch) = &mut self.batch {
                if batch.window != window { self.reset(); return; }
                batch.pixels = batch.pixels.saturating_add(pixels - old_pixels);
                if batch.complete() && window[3] == LAST_ROW { self.reset(); }
            }
        }
    }
    pub(super) fn snapshot(&mut self, frame: &[u16], version: u64, cycle: VirtualCycle) -> (Vec<u16>, u64) {
        if self.batch.is_some() {
            if cycle.saturating_sub(self.last_write) >= IDLE_LIMIT { self.reset(); }
            else if let Some((pixels, published)) = &self.frozen { return (pixels.clone(), *published); }
        }
        (frame.to_vec(), version)
    }
}
