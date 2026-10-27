use crate::clocked_queue::ClockedQueue;
use std::sync::{Arc, Mutex};

/// A shared host stream. Host timestamps use the attached chip's bus cycle count.
/// Voltage samples are volts; raw samples are post-attenuation 12-bit counts.
/// An attached copy also records the bus cycle count at which the chip's peripheral clock
/// restarted, because conversions sample at peripheral time.
#[derive(Clone)]
pub struct AnalogStream<T = f32> { state: Arc<Mutex<State<T>>>, pub(super) clock_offset: u64 }
struct State<T> { queue: ClockedQueue<T>, now: u64 }
impl<T: Copy> AnalogStream<T> {
    fn create(rate: u32, initial: T, now: u64) -> Result<Self, &'static str> {
        Ok(Self { state: Arc::new(Mutex::new(State { queue: ClockedQueue::new(rate, initial, true)?, now })), clock_offset: 0 })
    }
    fn append(&self, samples: &[T], now: u64, cpu_hz: u64) {
        let mut state = self.state.lock().unwrap();
        state.advance(now, cpu_hz);
        state.queue.push(samples.iter().copied());
    }
    pub fn queued_samples(&self, now: u64, cpu_hz: u64) -> usize {
        let mut state = self.state.lock().unwrap();
        state.advance(now, cpu_hz);
        state.queue.frames.len()
    }
    /// Sample at peripheral-clock time `now`.
    pub(super) fn sample(&self, now: u64, cpu_hz: u64) -> T {
        let mut state = self.state.lock().unwrap();
        state.advance(now.saturating_add(self.clock_offset), cpu_hz);
        state.queue.current
    }
}
impl AnalogStream<f32> {
    /// Cold steady-state voltage replacement. Shared clones see the new level on
    /// the next real ADC conversion; discard queued samples rather than replaying stale power.
    pub fn replace_current(&self, volts: f32) -> Result<(), &'static str> {
        if !volts.is_finite() { return Err("voltage must be finite"); }
        let mut state=self.state.lock().unwrap();state.queue.frames.clear();state.queue.current=volts;Ok(())
    }
    pub fn new(rate: u32, bias: f32, now: u64) -> Result<Self, &'static str> {
        if !bias.is_finite() { return Err("voltage bias must be finite"); }
        Self::create(rate, bias, now)
    }
    /// Append volts, keeping the newest two seconds. Invalid samples leave state intact.
    pub fn push(&self, samples: &[f32], now: u64, cpu_hz: u64) -> Result<(), &'static str> {
        if cpu_hz == 0 || samples.iter().any(|v| !v.is_finite()) { return Err("voltage samples must be finite and cycle clock nonzero"); }
        self.append(samples, now, cpu_hz);
        Ok(())
    }
}
impl AnalogStream<u16> {
    pub fn new_raw(rate: u32, bias: u16, now: u64) -> Result<Self, &'static str> {
        if bias > 4095 { return Err("raw ADC bias must be in 0..=4095"); }
        Self::create(rate, bias, now)
    }
    pub fn push_raw(&self, samples: &[u16], now: u64, cpu_hz: u64) -> Result<(), &'static str> {
        if cpu_hz == 0 || samples.iter().any(|&v| v > 4095) { return Err("raw counts must be in 0..=4095 and cycle clock nonzero"); }
        self.append(samples, now, cpu_hz);
        Ok(())
    }
}
impl<T: Copy> State<T> {
    fn advance(&mut self, now: u64, cpu_hz: u64) {
        self.queue.advance(now.saturating_sub(self.now), cpu_hz.max(1));
        self.now = self.now.max(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalogInputs, AnalogSource};
    #[test]
    fn cold_voltage_replacement_clears_queued_samples_without_completing_a_conversion() {
        let source=AnalogStream::new(100,0.0,0).unwrap();let mut pads=AnalogInputs::new(1000);
        pads.set(11,AnalogSource::Stream(source.clone()));source.push(&[2.0,2.5],0,1000).unwrap();source.replace_current(1.25).unwrap();
        assert_eq!(pads.volts(11,100),1.25);assert_eq!(source.queued_samples(100,1000),0);assert_eq!(pads.observation(11).generation,0);
        assert!(source.replace_current(f32::NAN).is_err());assert_eq!(pads.volts(11,200),1.25);
        assert_eq!(pads.convert(11,200,|v|(v*1000.0)as u32),1250);assert_eq!(pads.observation(11).generation,1);
        source.replace_current(0.0).unwrap();pads.set_stream_clock_offset(200);assert_eq!(pads.volts(11,0),0.0);
    }
    #[test]
    fn stream_clock_bound_hold_and_shared_handle() {
        assert!(AnalogStream::new(0, 1.0, 0).is_err());
        assert!(AnalogStream::new(96001, 1.0, 0).is_err());
        // Slow sensor streams: 1 Hz holds each sample for a full second.
        let slow = AnalogStream::new(1, 0.0, 0).unwrap();
        slow.push(&[0.5, 0.25], 0, 1000).unwrap();
        assert_eq!((slow.sample(999, 1000), slow.sample(1000, 1000), slow.sample(2000, 1000)), (0.0, 0.5, 0.25));
        assert!(AnalogStream::new(8000, f32::NAN, 0).is_err());
        let stream = AnalogStream::new(8000, 1.0, 0).unwrap();
        let mut pads = AnalogInputs::new(80_000_000);
        pads.set(1, AnalogSource::Stream(stream.clone()));
        assert!(stream.push(&[f32::NAN], 100000, 80_000_000).is_err());
        assert!(stream.push(&[1.0], 0, 0).is_err());
        stream.push(&[1.25, 0.75, 1.125], 0, pads.cpu_hz).unwrap();
        assert_eq!(pads.volts(1, 9999), 1.0);
        assert_eq!(pads.volts(1, 10000), 1.25);
        assert_eq!(pads.volts(1, 19999), 1.25);
        assert_eq!(pads.volts(1, 20000), 0.75);
        assert_eq!(pads.volts(1, 90000), 1.125);
        stream.push(&[0.5], 95000, pads.cpu_hz).unwrap();
        assert_eq!(pads.volts(1, 99999), 1.125);
        assert_eq!(pads.volts(1, 100000), 0.5);
        stream.push(&vec![1.0; 16000], 100000, pads.cpu_hz).unwrap();
        stream.push(&[2.0], 100000, pads.cpu_hz).unwrap();
        assert_eq!(stream.queued_samples(100000, pads.cpu_hz), 16000);
        assert_eq!(pads.volts(1, 160100000), 2.0);
        stream.push(&vec![3.0; 24000], 160100000, pads.cpu_hz).unwrap();
        assert_eq!(stream.queued_samples(160100000, pads.cpu_hz), 16000);
        assert_eq!(pads.volts(1, u64::MAX), 3.0);
    }
    #[test]
    fn raw_stream_counts_and_validation() {
        assert!(AnalogStream::new_raw(8000, 4096, 0).is_err());
        let stream = AnalogStream::new_raw(8000, 777, 0).unwrap();
        let mut pads = AnalogInputs::new(8000);
        pads.set(1, AnalogSource::RawStream(stream.clone()));
        assert_eq!(pads.convert(1, 0, |_| panic!("raw calibration")), 777);
        assert!(stream.push_raw(&[1], 0, 0).is_err());
        stream.push_raw(&[0, 1, 1024, 2048, 4095], 0, pads.cpu_hz).unwrap();
        assert!(stream.push_raw(&[5, 4096], 100, pads.cpu_hz).is_err());
        assert_eq!(stream.queued_samples(0, pads.cpu_hz), 5);
        for (n, raw) in [0, 1, 1024, 2048, 4095].into_iter().enumerate() {
            assert_eq!(pads.convert(1, n as u64 + 1, |_| panic!("raw calibration")), raw);
        }
        assert_eq!(pads.convert(1, 100, |_| panic!("raw calibration")), 4095);
        stream.push_raw(&vec![23; 16000], 100, pads.cpu_hz).unwrap();
        stream.push_raw(&[42], 100, pads.cpu_hz).unwrap();
        assert_eq!(stream.queued_samples(100, pads.cpu_hz), 16000);
        assert_eq!(pads.convert(1, 16100, |_| panic!("raw calibration")), 42);
    }
    #[test]
    fn old_timestamps_do_not_replay_and_sources_replace_each_other() {
        let stream = AnalogStream::new_raw(8000, 777, 100).unwrap();
        stream.push_raw(&[10, 20, 30], 100, 8000).unwrap();
        let mut pads = AnalogInputs::new(8000);
        pads.set_raw(1, 999);
        pads.set(1, AnalogSource::RawStream(stream.clone()));
        assert_eq!(pads.convert(1, 101, |_| panic!("raw conversion")), 10);
        assert_eq!(pads.convert(1, 0, |_| panic!("raw conversion")), 10);
        assert_eq!(pads.convert(1, 102, |_| panic!("raw conversion")), 20);
        assert_eq!(pads.observation(1).generation, 3);
        stream.queued_samples(103, pads.cpu_hz);
        assert_eq!(pads.observation(1).generation, 3);
        pads.set_raw(1, 888);
        assert_eq!(pads.convert(1, 104, |_| panic!("raw conversion")), 888);
        pads.set(1, AnalogSource::Const(1.25));
        assert_eq!(pads.convert(1, 105, |v| (v * 100.0) as u32), 125);
    }
    #[test]
    fn stream_fractional_clock_is_read_independent() {
        let make = || {
            let stream = AnalogStream::new(44100, 0.0, 0).unwrap();
            stream.push(&(0..2000).map(|x| x as f32).collect::<Vec<_>>(), 0, 160_000_000).unwrap();
            stream
        };
        let a = make(); let b = make();
        for now in (0..5_000_000).step_by(7919) { a.sample(now, 160_000_000); }
        assert_eq!(a.sample(5_000_000, 160_000_000), b.sample(5_000_000, 160_000_000));
        assert_eq!(a.queued_samples(5_000_000, 160_000_000), b.queued_samples(5_000_000, 160_000_000));
    }
}
