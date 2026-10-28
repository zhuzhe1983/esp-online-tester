use std::collections::VecDeque;

/// A host sample clock with a two-second drop-old buffer and zero-order hold.
pub(crate) struct ClockedQueue<T> {
    pub(crate) rate: u32,
    pub(crate) frames: VecDeque<T>,
    phase: u64,
    pub(crate) current: T,
    hold: bool,
    silence: T,
}
impl<T: Copy> ClockedQueue<T> {
    pub(crate) fn new(rate: u32, initial: T, hold: bool) -> Result<Self, &'static str> {
        if !(1..=96000).contains(&rate) { return Err("sample rate must be 1..96000 Hz"); }
        Ok(Self { rate, frames: VecDeque::new(), phase: 0, current: initial, hold, silence: initial })
    }
    pub(crate) fn push(&mut self, samples: impl ExactSizeIterator<Item = T>) {
        let capacity = self.rate as usize * 2;
        let skip = samples.len().saturating_sub(capacity);
        let discard = (self.frames.len() + samples.len().min(capacity)).saturating_sub(capacity);
        self.frames.drain(..discard);
        self.frames.extend(samples.skip(skip));
    }
    pub(crate) fn sample(&self, cycles: u64, cpu_hz: u64) -> T {
        let due = (u128::from(self.phase) + u128::from(cycles) * u128::from(self.rate)) / u128::from(cpu_hz);
        if due == 0 { self.current } else {
            self.frames.get(usize::try_from(due - 1).unwrap_or(usize::MAX)).copied()
                .unwrap_or_else(|| if self.hold { self.frames.back().copied().unwrap_or(self.current) } else { self.silence })
        }
    }
    pub(crate) fn advance(&mut self, cycles: u64, cpu_hz: u64) -> (u64, usize) {
        self.current = self.sample(cycles, cpu_hz);
        let phase = u128::from(self.phase) + u128::from(cycles) * u128::from(self.rate);
        let due = phase / u128::from(cpu_hz);
        let consumed = due.min(self.frames.len() as u128) as usize;
        self.frames.drain(..consumed);
        self.phase = (phase % u128::from(cpu_hz)) as u64;
        (due.min(u128::from(u64::MAX)) as u64, consumed)
    }
}
