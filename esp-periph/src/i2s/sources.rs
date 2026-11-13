// ESP-IDF v5.5.5 components/soc/esp32s3/include/soc/gpio_sig_map.h:56-71;
// esp32c3:38-43 and esp32c6:34-39. Select/invert fields in register/soc/gpio_reg.h:
// S3:2672-2678,7804-7810; C3:1325-1331,3897-3903; C6:2466-2473,4998-5005.
// Selection compares the data input, BCLK output and WS output including inversion bits.
// Routing is a matrix-level model; pad enable, IO_MUX and serial edges are not simulated here.
use crate::gpio::Gpio;
use crate::clocked_queue::ClockedQueue;

/// Physical microphone wiring. PDM supplies already converted PCM, not raw bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcmPins {
    I2s { bclk: u8, ws: u8, data: u8 },
    Pdm { clk: u8, data: u8 },
}

/// A host-clocked stereo PCM16 source. Duplicate mono samples into both lanes.
/// Time advances even when no receiver or DMA channel is running, evaluated on access.
pub struct PcmSource {
    pins: Option<PcmPins>,
    queue: ClockedQueue<[i16; 2]>,
    tone: Option<(f64, f64, f64)>,
    clip: Option<Box<Clip>>,
}
/// A whole recording played once at its own rate (`play`), then silence.
struct Clip { frames: Vec<[i16; 2]>, rate: u32, pos: f64 }
impl Default for PcmSource {
    fn default() -> Self { Self { pins: None, queue: ClockedQueue::new(32768, [0; 2], false).unwrap(), tone: None, clip: None } }
}
impl PcmSource {
    pub fn new(rate: u32, pins: PcmPins) -> Result<Self, &'static str> {
        if !(8000..=96000).contains(&rate) { return Err("PCM source rate must be 8000..96000 Hz"); }
        let valid = match pins {
            PcmPins::I2s { bclk, ws, data } => bclk < 49 && ws < 49 && data < 49 && bclk != ws && bclk != data && ws != data,
            PcmPins::Pdm { clk, data } => clk < 49 && data < 49 && clk != data,
        };
        if !valid { return Err("PCM source needs distinct GPIOs in 0..49"); }
        Ok(Self { pins: Some(pins), queue: ClockedQueue::new(rate, [0; 2], false)?, tone: None, clip: None })
    }
    pub fn queued_frames(&self) -> usize { self.queue.frames.len() }
    /// Routed sources retain the newest two seconds; per-port input accepts up to 65536 frames.
    pub fn push(&mut self, frames: &[[i16; 2]]) -> usize {
        self.tone = None;
        self.clip = None;
        let n = if self.pins.is_none() { frames.len().min(65536 - self.queue.frames.len()) } else { frames.len() };
        self.queue.push(frames[..n].iter().copied());
        n
    }
    /// Per-controller input only: a routed source samples its queue by absolute time.
    pub fn tone(&mut self, hz: f64, amplitude: f64) -> Result<(), &'static str> {
        if self.pins.is_some() { return Err("tone is only available on per-controller input; push frames to a routed source"); }
        if !hz.is_finite() || !(0.0..=192000.0).contains(&hz) || !amplitude.is_finite() || !(0.0..=1.0).contains(&amplitude) {
            return Err("I2S tone needs frequency 0..192000 Hz and amplitude 0..1");
        }
        self.queue.frames.clear();
        self.clip = None;
        self.tone = Some((hz, amplitude, 0.0));
        Ok(())
    }
    /// Per-controller input only: play `frames` recorded at `rate` Hz once, from the first frame
    /// the receiver takes, then silence. Resampled to the firmware's RX rate by zero-order hold
    /// (nearest earlier sample, no filter). Not bounded by the push queue limit.
    pub fn play(&mut self, frames: Vec<[i16; 2]>, rate: u32) -> Result<(), &'static str> {
        if self.pins.is_some() { return Err("play is only available on per-controller input; push frames to a routed source"); }
        if !(1000..=192000).contains(&rate) { return Err("recording rate must be 1000..192000 Hz"); }
        self.queue.frames.clear();
        self.tone = None;
        self.clip = Some(Box::new(Clip { frames, rate, pos: 0.0 }));
        Ok(())
    }
    pub fn clear(&mut self) { self.queue.frames.clear(); self.tone = None; self.clip = None; }
    pub(super) fn next(&mut self, rate: u32) -> [i16; 2] {
        if let Some(clip) = &mut self.clip {
            let frame = clip.frames.get(clip.pos as usize).copied().unwrap_or([0; 2]);
            clip.pos += f64::from(clip.rate) / f64::from(rate.max(1));
            return frame;
        }
        if let Some((hz, amplitude, phase)) = &mut self.tone {
            let sample = (phase.sin() * *amplitude * 32767.0).round() as i16;
            *phase = (*phase + std::f64::consts::TAU * *hz / rate as f64) % std::f64::consts::TAU;
            [sample; 2]
        } else {
            self.queue.advance(1, u64::from(self.queue.rate));
            self.queue.current
        }
    }
    fn sample(&self, cycles: u64, cpu_hz: u64) -> [i16; 2] { self.queue.sample(cycles, cpu_hz) }
    fn advance(&mut self, cycles: u64, cpu_hz: u64) { self.queue.advance(cycles, cpu_hz); }
}

/// A RIFF/WAVE file's PCM16 frames (mono duplicated into both lanes) and its sample rate.
/// Accepts WAVE_FORMAT_PCM or WAVE_FORMAT_EXTENSIBLE, 16-bit, one or two channels.
pub fn parse_wav(bytes: &[u8]) -> Result<(u32, Vec<[i16; 2]>), String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" { return Err("not a RIFF/WAVE file".into()); }
    let (mut fmt, mut pos) = (None, 12);
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = &bytes[pos + 8..(pos + 8).saturating_add(len).min(bytes.len())];
        if id == b"fmt " {
            if body.len() < 16 { return Err("short fmt chunk".into()); }
            let word = |o: usize| u16::from_le_bytes([body[o], body[o + 1]]);
            let (format, channels, rate, bits) = (word(0), word(2), u32::from_le_bytes(body[4..8].try_into().unwrap()), word(14));
            if format != 1 && format != 0xfffe { return Err(format!("WAV format {format:#x} is not integer PCM")); }
            if bits != 16 { return Err(format!("{bits}-bit WAV; only 16-bit PCM is supported")); }
            if !(1..=2).contains(&channels) { return Err(format!("{channels}-channel WAV; only mono or stereo")); }
            fmt = Some((channels as usize, rate));
        } else if id == b"data" {
            let (channels, rate) = fmt.ok_or("data chunk before fmt chunk")?;
            let frames = body.chunks_exact(2 * channels).map(|f| {
                let l = i16::from_le_bytes([f[0], f[1]]);
                [l, if channels == 2 { i16::from_le_bytes([f[2], f[3]]) } else { l }]
            }).collect();
            return Ok((rate, frames));
        }
        pos = pos + 8 + len + (len & 1);
    }
    Err("no data chunk".into())
}

/// Shared by all RX ports. A receiver whose matrix route matches a slot reads that slot;
/// otherwise it keeps its per-controller input. Slots, audio and clock phase survive
/// chip reset. Lowest matching slot wins. Only the RX BCK/WS output signals are matched.
#[derive(Default)]
pub struct PcmSources {
    pub inputs: Vec<Option<PcmSource>>,
    now: u64,
}
impl PcmSources {
    /// Synchronize once before reading/pushing or sampling an RX interval.
    pub fn advance_to(&mut self, now: u64, cpu_hz: u64) {
        self.advance(now.saturating_sub(self.now), cpu_hz);
        self.now = self.now.max(now);
    }

    pub(crate) fn active(&self) -> bool {
        self.inputs.iter().any(Option::is_some)
    }
    fn advance(&mut self, cycles: u64, cpu_hz: u64) {
        for source in self.inputs.iter_mut().flatten() {
            source.advance(cycles, cpu_hz);
        }
    }
    pub(crate) fn select(&self, gpio: &Gpio, signals: RxSignals, pdm: bool) -> Option<usize> {
        let RxSignals {
            data,
            clock,
            input_select_bit,
            output_mask,
        } = signals;
        self.inputs.iter().position(|source| {
            let Some(source) = source else {
                return false;
            };
            let (din, bclk, ws) = match (source.pins, pdm) {
                (Some(PcmPins::I2s { bclk, ws, data }), false) => (data, bclk, Some(ws)),
                (Some(PcmPins::Pdm { clk, data }), true) => (data, clk, None),
                _ => return false,
            };
            gpio.func_in_sel[data] & ((1 << (input_select_bit + 1)) - 1)
                == u32::from(din) | (1 << input_select_bit)
                && gpio.func_out_sel[bclk as usize] & output_mask
                    == clock[usize::from(pdm)] as u32
                && ws.is_none_or(|ws| {
                    gpio.func_out_sel[ws as usize] & output_mask == clock[1] as u32
                })
        })
    }
    pub(crate) fn sample(&mut self, id: usize, cycles: u64, cpu_hz: u64) -> [i16; 2] {
        let source = self.inputs[id].as_mut().unwrap();
        // Zero-order resampling; no anti-alias filter.
        source.sample(cycles, cpu_hz)
    }
}

/// Chip-specific GPIO matrix encoding for an RX controller.
#[derive(Clone, Copy)]
pub struct RxSignals {
    pub data: usize,
    /// Output signal indices [BCLK, WS]; PDM uses the WS signal.
    pub clock: [usize; 2],
    pub input_select_bit: u32,
    pub output_mask: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wav(fmt: u16, channels: u16, bits: u16, rate: u32, data: &[u8]) -> Vec<u8> {
        let mut v = b"RIFF\0\0\0\0WAVE".to_vec();
        v.extend(b"LIST"); v.extend(3u32.to_le_bytes()); v.extend([1, 2, 3, 0]);   // odd chunk, padded
        v.extend(b"fmt "); v.extend(16u32.to_le_bytes());
        for w in [fmt, channels] { v.extend(w.to_le_bytes()); }
        v.extend(rate.to_le_bytes()); v.extend((rate * u32::from(channels) * 2).to_le_bytes());
        for w in [channels * 2, bits] { v.extend(w.to_le_bytes()); }
        v.extend(b"data"); v.extend((data.len() as u32).to_le_bytes()); v.extend(data);
        v
    }
    #[test]
    fn wav_parsing_accepts_pcm16_mono_and_stereo_only() {
        let pcm = [1u8, 0, 2, 0, 0xff, 0xff, 4, 0];
        assert_eq!(parse_wav(&wav(1, 1, 16, 16000, &pcm)), Ok((16000, vec![[1, 1], [2, 2], [-1, -1], [4, 4]])));
        assert_eq!(parse_wav(&wav(0xfffe, 2, 16, 48000, &pcm)), Ok((48000, vec![[1, 2], [-1, 4]])));
        assert!(parse_wav(&wav(3, 1, 16, 16000, &pcm)).unwrap_err().contains("not integer PCM"));
        assert!(parse_wav(&wav(1, 1, 24, 16000, &pcm)).unwrap_err().contains("16-bit"));
        assert!(parse_wav(&wav(1, 3, 16, 16000, &pcm)).unwrap_err().contains("mono or stereo"));
        assert!(parse_wav(b"RIFX....WAVE").is_err());
        assert_eq!(parse_wav(&wav(1, 1, 16, 16000, &pcm)[..48]).unwrap_err(), "no data chunk");
    }
    #[test]
    fn played_recording_is_resampled_once_then_silent() {
        let mut input = PcmSource::default();
        input.play(vec![[1; 2], [2; 2], [3; 2], [4; 2]], 16000).unwrap();
        // 8 kHz receiver: every other sample (zero-order hold), then silence.
        assert_eq!((0..3).map(|_| input.next(8000)[0]).collect::<Vec<_>>(), [1, 3, 0]);
        input.play(vec![[1; 2], [2; 2]], 8000).unwrap();
        assert_eq!((0..5).map(|_| input.next(16000)[0]).collect::<Vec<_>>(), [1, 1, 2, 2, 0]);
        assert!(input.play(Vec::new(), 10).is_err());
        let mut routed = PcmSource::new(8000, PcmPins::Pdm { clk: 1, data: 2 }).unwrap();
        assert!(routed.play(vec![[1; 2]], 8000).is_err());
        input.play(vec![[9; 2]], 8000).unwrap();
        input.clear();
        assert_eq!(input.next(8000), [0; 2]);
    }
    #[test]
    fn absolute_time_is_lazy_monotonic_and_handles_long_stalls() {
        let mut sources = PcmSources::default();
        sources.inputs.resize_with(16, || None);
        sources.advance_to(80000, 8000);
        let mut source = PcmSource::new(8000, PcmPins::Pdm { clk: 1, data: 2 }).unwrap();
        source.push(&[[1; 2], [2; 2], [3; 2]]);
        sources.inputs[0] = Some(source);
        sources.advance_to(80001, 8000);
        assert_eq!(sources.inputs[0].as_ref().unwrap().queue.current, [1; 2]);
        sources.advance_to(0, 8000);
        sources.advance_to(80002, 8000);
        assert_eq!(sources.inputs[0].as_ref().unwrap().queue.current, [2; 2]);
        sources.advance_to(u64::MAX, 8000);
        let source = sources.inputs[0].as_ref().unwrap();
        assert_eq!(source.queued_frames(), 0);
        assert_eq!(source.queue.current, [0; 2]);
    }

    #[test]
    fn lowest_matching_source_wins_and_unmatched_receivers_keep_controller_input() {
        use crate::i2s::I2s;
        let mut sources = PcmSources::default();
        sources.inputs.resize_with(16, || None);
        let mut gpio = Gpio::new();
        gpio.func_in_sel[25] = 6 | (1 << 7);
        gpio.func_out_sel[4] = 26;
        gpio.func_out_sel[5] = 27;
        let signals = RxSignals { data: 25, clock: [26, 27], input_select_bit: 7, output_mask: 0x3ff };
        for id in [0, 15] {
            sources.inputs[id] = Some(PcmSource::new(8000, PcmPins::I2s { bclk: 4, ws: 5, data: 6 }).unwrap());
        }
        assert_eq!(sources.select(&gpio, signals, false), Some(0));
        sources.inputs[0] = None;
        assert_eq!(sources.select(&gpio, signals, false), Some(15));
        let mut rx = I2s::new(160_000_000);
        rx.write(0x30, (1 << 26) | (2 << 27) | 25);
        rx.write(0x28, (24 << 7) | (15 << 13) | (15 << 18));
        rx.write(0x50, (1 << 16) | 3);
        rx.write(0x20, 4);
        rx.rx_input().push(&[[12, 34]]);
        // A matched source that underruns is silent; the controller input stays queued.
        assert_eq!(rx.receive(20000, 20000, false, &gpio, signals, Some(&mut sources)), [0; 4]);
        assert_eq!(rx.rx_source(), Some(15));
        sources.inputs[15] = None;
        assert_eq!(rx.receive(20000, 40000, false, &gpio, signals, Some(&mut sources)), [12, 0, 34, 0]);
        assert_eq!(rx.rx_source(), None);
        rx.rx_input().tone(1000.0, 1.0).unwrap();
        sources.inputs[3] = Some(PcmSource::new(8000, PcmPins::I2s { bclk: 7, ws: 8, data: 9 }).unwrap());
        let tone: Vec<_> = (0..3).flat_map(|n| rx.receive(20000, 60000 + 20000 * n, false, &gpio, signals, Some(&mut sources))).collect();
        assert_eq!(tone, [0, 0, 0, 0, 0x82, 0x5a, 0x82, 0x5a, 0xff, 0x7f, 0xff, 0x7f]);

    }

    #[test]
    fn selected_source_query_tracks_ports_routes_stop_reset_and_legacy() {
        use crate::i2s::I2s;
        let sig = [
            RxSignals { data: 25, clock: [26, 27], input_select_bit: 7, output_mask: 0x3ff },
            RxSignals { data: 30, clock: [31, 32], input_select_bit: 7, output_mask: 0x3ff },
        ];
        let mut bank = PcmSources::default();
        bank.inputs.resize_with(16, || None);
        for (id, bclk, ws, data, sample) in [(15, 4, 5, 6, 123i16), (2, 7, 8, 9, -321)] {
            let mut source = PcmSource::new(8000, PcmPins::I2s { bclk, ws, data }).unwrap();
            source.push(&[[sample; 2]; 4]);
            bank.inputs[id] = Some(source);
        }
        let mut gpio = Gpio::new();
        gpio.func_in_sel[25] = 6 | (1 << 7);
        gpio.func_out_sel[4] = 26;
        gpio.func_out_sel[5] = 27;
        gpio.func_in_sel[30] = 9 | (1 << 7);
        gpio.func_out_sel[7] = 31;
        gpio.func_out_sel[8] = 32;
        let mut ports = [I2s::new(160_000_000), I2s::new(160_000_000)];
        for (port, signals) in ports.iter_mut().zip(sig) {
            assert_eq!(port.rx_source(), None);
            port.write(0x30, (1 << 26) | (2 << 27) | 25);
            port.write(0x28, (24 << 7) | (15 << 13) | (15 << 18));
            port.write(0x50, (1 << 16) | 3);
            port.write(0x20, 4);
            let sample: i16 = if signals.data == 25 { 123 } else { -321 };
            let expected: Vec<_> = [sample; 2].into_iter().flat_map(i16::to_le_bytes).collect();
            assert_eq!(port.rx_rate(), Some(8000));
            assert_eq!(port.receive(20000, 20000, false, &gpio, signals, Some(&mut bank)), expected);
        }
        assert_eq!([ports[0].rx_source(), ports[1].rx_source()], [Some(15), Some(2)]);
        let before = (bank.now, bank.inputs[15].as_ref().unwrap().queued_frames());
        for _ in 0..3 { assert_eq!(ports[0].rx_source(), Some(15)); }
        assert_eq!((bank.now, bank.inputs[15].as_ref().unwrap().queued_frames()), before);
        // Inspection is last-interval attribution, not a new route poll.
        gpio.func_in_sel[25] = 9 | (1 << 7);
        gpio.func_out_sel[7] = 26;
        gpio.func_out_sel[8] = 27;
        assert_eq!(ports[0].rx_source(), Some(15));
        ports[0].receive(0, 20000, false, &gpio, sig[0], Some(&mut bank));
        assert_eq!(ports[0].rx_source(), Some(2));
        ports[1].receive(0, 20000, false, &gpio, sig[1], Some(&mut bank));
        assert_eq!(ports[1].rx_source(), None); // clocks moved to port 0
        gpio.func_out_sel[8] ^= 1 << 9; // inversion is part of routing
        ports[0].receive(0, 20000, false, &gpio, sig[0], Some(&mut bank));
        assert_eq!(ports[0].rx_source(), None);
        gpio.func_out_sel[8] ^= 1 << 9;
        ports[0].receive(0, 20000, false, &gpio, sig[0], Some(&mut bank));
        assert_eq!(ports[0].rx_source(), Some(2));
        ports[0].write(0x20, 0);
        assert_eq!(ports[0].rx_source(), None);
        ports[0].write(0x20, 1);
        ports[0].write(0x20, 4);
        assert_eq!(ports[0].rx_source(), None);
        ports[0].receive(0, 20000, false, &gpio, sig[0], Some(&mut bank));
        let mut rebuilt = I2s::new(160_000_000);
        rebuilt.keep_rx_input(&mut ports[0]);
        rebuilt.write(0x20, 4);
        assert_eq!(rebuilt.rx_source(), None);
        assert_eq!(ports[0].rx_source(), None);
        ports[1].receive(0, 20000, false, &gpio, sig[0], Some(&mut bank));
        assert_eq!(ports[1].rx_source(), Some(2));
        ports[1].receive(0, 20000, false, &gpio, sig[0], None);
        assert_eq!(ports[1].rx_source(), None);
        bank.inputs[2] = None;
        ports[1].receive(0, 20000, false, &gpio, sig[0], Some(&mut bank));
        assert_eq!(ports[1].rx_source(), None);
    }

    #[test]
    fn selected_pdm_source_uses_ws_clock_and_query_does_not_attach_state() {
        use crate::i2s::I2s;
        let signals = RxSignals { data: 15, clock: [16, 17], input_select_bit: 6, output_mask: 0x1ff };
        let mut bank = PcmSources::default();
        bank.inputs.resize_with(16, || None);
        let mut source = PcmSource::new(8000, PcmPins::Pdm { clk: 4, data: 6 }).unwrap();
        source.push(&[[11, 22]]);
        bank.inputs[15] = Some(source);
        let mut gpio = Gpio::new();
        gpio.func_in_sel[15] = 6 | (1 << 6);
        gpio.func_out_sel[4] = 17;
        let mut rx = I2s::new(160_000_000);
        assert_eq!(rx.rx_source(), None);
        assert!(rx.pcm_bank_ref().is_none());
        // A routed but disabled receiver must not allocate or attribute a source.
        assert!(rx.receive(0, 0, true, &gpio, signals, Some(&mut bank)).is_empty());
        assert_eq!(rx.rx_source(), None);
        assert!(rx.rx.is_none());
        rx.write(0x30, (1 << 26) | (2 << 27) | 25);
        rx.write(0x28, (24 << 7) | (15 << 13));
        rx.write(0x50, 3);
        rx.write(0x20, 4 | (1 << 20) | (1 << 21));
        assert_eq!(rx.rx_rate(), Some(4000));
        assert_eq!(rx.receive(40000, 40000, true, &gpio, signals, Some(&mut bank)), [0; 4]);
        assert_eq!(rx.rx_source(), Some(15));
        gpio.func_out_sel[4] = 16; // TX-like BCLK instead of RX WS/PDM clock
        rx.receive(0, 40000, true, &gpio, signals, Some(&mut bank));
        assert_eq!(rx.rx_source(), None);
    }

    #[test]
    fn source_clock_bound_and_underrun() {
        let pins = PcmPins::I2s {
            bclk: 4,
            ws: 5,
            data: 6,
        };
        assert!(PcmSource::new(0, pins).is_err());
        assert!(PcmSource::new(7999, pins).is_err());
        assert!(PcmSource::new(96001, pins).is_err());
        assert!(PcmSource::new(96000, pins).is_ok());
        assert!(PcmSource::new(8000, PcmPins::Pdm { clk: 49, data: 1 }).is_err());
        assert!(PcmSource::new(8000, PcmPins::Pdm { clk: 1, data: 1 }).is_err());
        let mut source = PcmSource::new(8000, pins).unwrap();
        source.push(&[[1; 2], [2; 2], [3; 2]]);
        assert_eq!(source.sample(9999, 80_000_000), [0; 2]);
        assert_eq!(source.sample(10000, 80_000_000), [1; 2]);
        source.advance(15000, 80_000_000);
        assert_eq!(source.queued_frames(), 2);
        assert_eq!(source.sample(4999, 80_000_000), [1; 2]);
        assert_eq!(source.sample(5000, 80_000_000), [2; 2]);
        source.advance(25000, 80_000_000);
        assert_eq!(source.sample(0, 80_000_000), [0; 2]);
        source.push(&vec![[11; 2]; 16000]);
        source.push(&[[22; 2]]);
        assert_eq!(source.queued_frames(), 16000);
        assert_eq!(source.sample(16000 * 10000, 80_000_000), [22; 2]);
        source.push(&vec![[33; 2]; 16001]);
        assert_eq!(source.queued_frames(), 16000);
        assert_eq!(source.sample(10000, 80_000_000), [33; 2]);
        source.advance(80_000_000 * 3, 80_000_000);
        assert_eq!(source.queued_frames(), 0);
        assert_eq!(source.queue.current, [0; 2]);
    }

    #[test]
    fn resampling_is_tick_independent_and_shared_by_receivers() {
        use crate::i2s::I2s;
        let run = |ticks: &[u64]| {
            let mut sources = PcmSources::default();
            sources.inputs.resize_with(16, || None);
            let mut input = PcmSource::new(
                16000,
                PcmPins::I2s {
                    bclk: 4,
                    ws: 5,
                    data: 6,
                },
            )
            .unwrap();
            input.push(&(1..=16).map(|sample| [sample; 2]).collect::<Vec<_>>());
            sources.inputs[0] = Some(input);
            let mut gpio = Gpio::new();
            gpio.func_in_sel[25] = 6 | (1 << 7);
            gpio.func_out_sel[4] = 26;
            gpio.func_out_sel[5] = 27;
            let signals = RxSignals {
                data: 25,
                clock: [26, 27],
                input_select_bit: 7,
                output_mask: 0x3ff,
            };
            let mut receivers = [I2s::new(160_000_000), I2s::new(160_000_000)];
            for rx in &mut receivers {
                rx.write(0x30, (1 << 26) | (2 << 27) | 25);
                rx.write(0x28, (24 << 7) | (15 << 13) | (15 << 18));
                rx.write(0x50, (1 << 16) | 3);
                rx.write(0x20, 4);
            }
            let mut output = [Vec::new(), Vec::new()];
            let mut now = 0;
            for &cycles in ticks {
                now += cycles;
                for (rx, output) in receivers.iter_mut().zip(&mut output) {
                    output.extend(rx.receive(cycles, now, false, &gpio, signals, Some(&mut sources)));
                }
                sources.advance_to(now, 160_000_000);
            }
            assert_eq!(output[0], output[1]);
            let source = sources.inputs[0].as_ref().unwrap();
            assert_eq!(source.queued_frames(), 8);

            output[0].clone()
        };
        let expected: Vec<_> = [2i16, 4, 6, 8]
            .into_iter()
            .flat_map(|n| [n, n].into_iter().flat_map(i16::to_le_bytes))
            .collect();
        assert_eq!(run(&[80000]), expected);
        assert_eq!(run(&[1, 9998, 20002, 17000, 33000 - 1]), expected);
    }

    #[test]
    fn routing_checks_every_wire_and_inversion() {
        for (data, bit, mask) in [
            (25, 7, 0x3ff),
            (30, 7, 0x3ff),
            (15, 6, 0x1ff),
            (15, 7, 0x1ff),
        ] {
            let signals = RxSignals {
                data,
                clock: [data + 1, data + 2],
                input_select_bit: bit,
                output_mask: mask,
            };
            let mut sources = PcmSources::default();
            sources.inputs.resize_with(16, || None);
            sources.inputs[15] = Some(
                PcmSource::new(
                    8000,
                    PcmPins::I2s {
                        bclk: 4,
                        ws: 5,
                        data: 6,
                    },
                )
                .unwrap(),
            );
            let mut gpio = Gpio::new();
            gpio.func_in_sel[data] = 6 | (1 << bit);
            gpio.func_out_sel[4] = data as u32 + 1;
            gpio.func_out_sel[5] = data as u32 + 2;
            assert_eq!(sources.select(&gpio, signals, false), Some(15));
            assert_eq!(sources.select(&gpio, signals, true), None);
            for pin in [4, 5] {
                gpio.func_out_sel[pin] ^= 1;
                assert_eq!(sources.select(&gpio, signals, false), None);
                gpio.func_out_sel[pin] ^= 1;
            }
            gpio.func_in_sel[data] ^= 1 << (bit - 1);
            assert_eq!(sources.select(&gpio, signals, false), None);
            gpio.func_in_sel[data] ^= 1 << (bit - 1);
            gpio.func_in_sel[data] ^= 1 << bit;
            assert_eq!(sources.select(&gpio, signals, false), None);
            gpio.func_in_sel[data] ^= 1 << bit;
            sources.inputs[15] =
                Some(PcmSource::new(8000, PcmPins::Pdm { clk: 4, data: 6 }).unwrap());
            gpio.func_out_sel[4] = data as u32 + 2;
            gpio.func_out_sel[5] = 0;
            assert_eq!(sources.select(&gpio, signals, true), Some(15));
            assert_eq!(sources.select(&gpio, signals, false), None);
        }
    }

    #[test]
    fn tone_is_rejected_on_routed_sources() {
        let mut routed = PcmSource::new(8000, PcmPins::Pdm { clk: 1, data: 2 }).unwrap();
        routed.push(&[[5; 2]]);
        assert!(routed.tone(1000.0, 0.5).is_err());
        assert_eq!((routed.queued_frames(), routed.sample(10000, 80_000_000)), (1, [5; 2]));
        assert!(PcmSource::default().tone(1000.0, 0.5).is_ok());
    }
}
