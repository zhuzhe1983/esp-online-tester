//! Analog pad voltages the SAR ADC samples, set from the host: a constant (`adc <gpio> <volts>`)
//! or a waveform played against emulated time (`adcwave <gpio> <file> <rate_hz>`), so a recorded
//! or synthetic signal (EMG, a sensor trace) reaches the firmware's analogRead() sample by sample.
//! The ADC model that reads these is the chip's business (the S3's SENS one-shot path in
//! `rtc_cntl.rs`). Raw inputs bypass the voltage transfer curve; observations track completed samples.
use std::collections::HashMap;
use std::sync::Arc;
mod stream;
pub use stream::AnalogStream;

#[derive(Clone)]
pub enum AnalogSource {
    Const(f32),
    /// `samples` volts at `rate_hz`, starting at `start_cycles`; before the start it reads the first
    /// sample, after the end it holds the last one.
    Wave { samples: Arc<Vec<f32>>, rate_hz: f64, start_cycles: u64 },
    /// Host voltage samples played on its own clock. Clones share the bounded queue.
    Stream(AnalogStream),
    RawStream(AnalogStream<u16>),
}
impl std::fmt::Debug for AnalogSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalogSource::Const(v) => write!(f, "Const({v} V)"),
            AnalogSource::Wave { samples, rate_hz, .. } => write!(f, "Wave({} samples @ {rate_hz} Hz)", samples.len()),
            AnalogSource::RawStream(_) => write!(f, "Stream(raw counts)"),
            AnalogSource::Stream(_) => write!(f, "Stream(volts)"),
        }
    }
}

/// Last completed conversion. Generation starts at zero and wraps at u64::MAX.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdcObservation { pub generation: u64, pub raw: u16 }

#[derive(Default)]
pub struct AnalogInputs { pins: HashMap<u8, AnalogSource>, raw: HashMap<u8, u16>, observed: HashMap<u8, AdcObservation>, pub cpu_hz: u64 }
impl AnalogInputs {
    pub fn new(cpu_hz: u64) -> Self { AnalogInputs { cpu_hz, ..Default::default() } }
    pub fn set(&mut self, pin: u8, src: AnalogSource) { self.raw.remove(&pin); self.pins.insert(pin, src); }
    /// Attach a board's nominal source only while the pad has no explicit source.
    /// Reattachment and reboot must preserve host voltages, streams and raw counts (including zero).
    /// This does not sample the ADC or change its observation generation.
    pub fn set_default(&mut self, pin: u8, src: AnalogSource) {
        if !self.raw.contains_key(&pin) { self.pins.entry(pin).or_insert(src); }
    }

    /// Replace the voltage source with a post-attenuation 12-bit count. Invalid counts leave it intact.
    pub fn set_raw(&mut self, pin: u8, raw: u16) -> bool {
        if raw > 4095 { return false; }
        self.pins.remove(&pin); self.raw.insert(pin, raw); true
    }
    pub fn observation(&self, pin: u8) -> AdcObservation { self.observed.get(&pin).copied().unwrap_or_default() }
    /// Record that the chip's peripheral clock reads `bus_cycles - offset`. Conversions keep
    /// sampling at peripheral time; attached streams translate it to host bus cycles.
    pub fn set_stream_clock_offset(&mut self, offset: u64) {
        for source in self.pins.values_mut() {
            match source {
                AnalogSource::Stream(stream) => stream.clock_offset = offset,
                AnalogSource::RawStream(stream) => stream.clock_offset = offset,
                AnalogSource::Const(_) | AnalogSource::Wave { .. } => {}
            }
        }
    }
    /// Complete one sample. Reading or replacing a source never advances its generation.
    pub fn convert(&mut self, pin: u8, now: u64, code: impl FnOnce(f32) -> u32) -> u32 {
        let raw = self.raw.get(&pin).copied().unwrap_or_else(|| match self.pins.get(&pin) {
            Some(AnalogSource::RawStream(stream)) => stream.sample(now, self.cpu_hz),
            source => code(self.source_volts(source, now)) as u16,
        });
        let sample = self.observed.entry(pin).or_default();
        sample.generation = sample.generation.wrapping_add(1); sample.raw = raw;
        u32::from(raw)
    }
    /// Volts on `pin` at `now_cycles` (0 V for undriven pads or raw-count sources).
    pub fn volts(&self, pin: u8, now_cycles: u64) -> f32 {
        self.source_volts(self.pins.get(&pin), now_cycles)
    }
    fn source_volts(&self, source: Option<&AnalogSource>, now_cycles: u64) -> f32 {
        match source {
            None | Some(AnalogSource::RawStream(_)) => 0.0,
            Some(AnalogSource::Const(v)) => *v,
            Some(AnalogSource::Stream(stream)) => stream.sample(now_cycles, self.cpu_hz),
            Some(AnalogSource::Wave { samples, rate_hz, start_cycles }) => {
                if samples.is_empty() { return 0.0; }
                let dt = now_cycles.saturating_sub(*start_cycles) as f64 / self.cpu_hz.max(1) as f64;
                samples[((dt * rate_hz) as usize).min(samples.len() - 1)]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wave_follows_emulated_time_and_holds_the_ends() {
        let mut a = AnalogInputs::new(1_000);
        a.set(4, AnalogSource::Wave { samples: Arc::new(vec![0.1, 0.2, 0.3]), rate_hz: 10.0, start_cycles: 500 });
        assert_eq!(a.volts(4, 0), 0.1);        // before the start: first sample
        assert_eq!(a.volts(4, 600), 0.2);      // 0.1 s after the start at 10 Hz
        assert_eq!(a.volts(4, 99_000), 0.3);   // after the end: last sample
        assert_eq!(a.volts(5, 600), 0.0);      // nobody drives GPIO5
    }
    #[test]
    fn board_defaults_preserve_explicit_zero_raw_voltage_and_observation() {
        let mut a=AnalogInputs::new(1_000);a.set_default(3,AnalogSource::Const(2.0));
        assert_eq!(a.volts(3,0),2.0);assert_eq!(a.observation(3).generation,0);
        a.set(3,AnalogSource::Const(0.0));a.set_default(3,AnalogSource::Const(2.0));assert_eq!(a.volts(3,0),0.0);
        assert!(a.set_raw(3,0));a.set_default(3,AnalogSource::Const(2.0));
        assert_eq!(a.convert(3,0,|_|4095),0);let sample=a.observation(3);
        a.set_default(3,AnalogSource::Const(2.0));assert_eq!(a.observation(3),sample);
        assert_eq!(a.volts(4,0),0.0,"no source added to neighbouring pads");
    }

}
