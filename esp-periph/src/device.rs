//! What a register-level peripheral model looks like to the chip that mounts it.
use emu_core::ClockDomain;

/// What a register write may have changed beyond the device's own state, for the SoC to act on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WriteEffect(pub u8);
impl WriteEffect {
    pub const NONE: WriteEffect = WriteEffect(0);
    /// a SPI flash command is pending: the SoC must run it against the flash array now
    pub const SPI_EXEC: WriteEffect = WriteEffect(1);
    /// the interrupt source → line mapping changed
    pub const INTMAP: WriteEffect = WriteEffect(2);
    /// an I2S receiver's RX_CONF was written: SoCs that cache pending work re-read `rx_running`
    pub const RX_CONF: WriteEffect = WriteEffect(4);
    /// a clock/reset controller pulsed a peripheral's reset bit: the SoC rebuilds that peripheral
    pub const PERIPH_RESET: WriteEffect = WriteEffect(8);
    pub fn contains(self, o: WriteEffect) -> bool { self.0 & o.0 != 0 }
}
impl std::ops::BitOr for WriteEffect { type Output = WriteEffect; fn bitor(self, o: WriteEffect) -> WriteEffect { WriteEffect(self.0 | o.0) } }

/// A peripheral: a 4 KiB register block (or a slice of one), optionally clocked, optionally an
/// interrupt source. The chip's `DeviceSet` table says where it sits and what its sources are
/// numbered; the model itself knows nothing about where it is mounted.
pub trait Device {
    fn read(&mut self, off: u32) -> u32;
    fn write(&mut self, off: u32, v: u32) -> WriteEffect;
    /// Bit i set = the device's i-th interrupt source (in the order the chip's table lists them)
    /// is asserted right now.
    fn irq_sources(&self) -> u64 { 0 }
    /// The clock whose ticks `tick` wants, if the device keeps time.
    fn clock(&self) -> Option<ClockDomain> { None }
    fn tick(&mut self, _ticks: u64) {}
    /// True if the device implements `next_deadline` (so the SoC only asks the timers).
    fn has_deadline(&self) -> bool { false }
    /// Ticks of its clock until the device could next raise an interrupt, if it can say. The SoC
    /// uses the minimum to decide how long device time may be deferred (conservative: it may be
    /// early, never late).
    fn next_deadline(&self) -> Option<u64> { None }
    /// Print what the device does (`--debug <name>`).
    fn debug(&mut self, _on: bool) {}
}

impl<T: Device> Device for Box<T> {
    fn read(&mut self, off: u32) -> u32 { (**self).read(off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { (**self).write(off, v) }
    fn irq_sources(&self) -> u64 { (**self).irq_sources() }
    fn clock(&self) -> Option<ClockDomain> { (**self).clock() }
    fn tick(&mut self, ticks: u64) { (**self).tick(ticks); }
    fn has_deadline(&self) -> bool { (**self).has_deadline() }
    fn next_deadline(&self) -> Option<u64> { (**self).next_deadline() }
    fn debug(&mut self, on: bool) { (**self).debug(on); }
}
