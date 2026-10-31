//! ESP32-S31 `device_set!` table (BASE 0), same shape as P4/C3.
//!
//! Shared esp-periph IP mounted here: UART0, Systimer (P4-style /2.5 wrap), USB-Serial/JTAG,
//! TimerGroup (with local MWDT stages for the WDT half), and S31 Efuse (ReadImage protocol —
//! shared `Efuse` is RegRam-only and cannot replace it).
//!
//! Still local outside this table:
//! - Pads / GPIO / IO_MUX — S31 offsets and LP/HP hold regs do not match shared `Gpio` (S3 map).
//! - Interrupt matrix — S31 CLIC + 169 sources / dual-core map (same choice as S3/P4 local matrix).

use emu_core::{ClockDomain, ClockTree};
use esp_periph::{
    device_set, mmio, Device, DeviceSet, Misc, WriteEffect, NO_SOURCE, Systimer,
    TimerGroup, Uart, UartLayout, UsbSerialJtag,
};
use emu_core::Fault;

use crate::efuse::Efuse;
use crate::mwdt::Mwdt;

pub const CPU_HZ: u64 = 40_000_000;

/// PCR clock words that gate IP in this set (HP_SYS_CLKRST).
pub const SYSTIMER_CLOCK: u32 = 0x2058_7120;
pub const USB_CLOCK: u32 = 0x2058_7144;
pub const TIMG0_CLOCK: u32 = 0x2058_7114;
pub const TIMG1_CLOCK: u32 = 0x2058_711c;

pub mod src {
    pub const USB_SERIAL_JTAG: usize = 2;
    pub const UART0: usize = 9;
    pub const TG0_WDT: usize = 27;
    pub const TG1_WDT: usize = 30;
    pub const SYSTIMER_TARGET0: usize = 33;
    pub const SYSTIMER_TARGET1: usize = 34;
    pub const SYSTIMER_TARGET2: usize = 35;
    // EFUSE_SOURCE = 152 is outside DeviceSet's 128-bit source_status; gathered via Irqs.
}

// ── Systimer: shared IP + S31 PCR gate + XTAL/2.5 (P4-style) ────────────────

#[derive(Default)]
pub struct SystimerS31 {
    pub st: Systimer,
    frac: u64,
    denominator: u64,
    pub clock: u32,
}

impl SystimerS31 {
    pub fn clock_on(&self) -> bool {
        self.clock & 3 == 1
    }

    fn counter_on(&self) -> bool {
        self.clock & 0x12 == 0x10
    }

    pub fn source_rc_fast(&self) -> bool {
        self.clock & 8 != 0
    }

    pub fn irqs(&self) -> [bool; 3] {
        std::array::from_fn(|t| self.st.irq(t))
    }

    pub fn clock_read(&self) -> u32 {
        self.clock
    }

    pub fn clock_write(&mut self, value: u32) -> Result<(), Fault> {
        if value & 4 != 0 {
            return Err(Fault::Prohibited);
        }
        if value & 2 != 0 {
            *self = Self { clock: value & 0x1b, ..Self::default() };
            return Ok(());
        }
        self.clock = value & 0x1b;
        Ok(())
    }

    pub fn advance_ticks(&mut self, ticks: u64, stalled: [bool; 2]) {
        if ticks == 0 || !self.counter_on() || stalled == [true, true] {
            return;
        }
        self.st.tick(ticks);
    }

    pub fn advance_cpu(&mut self, cycles: u64, cpu_hz: u64, rc_fast_hz: u64, stalled: [bool; 2]) {
        if cycles == 0 || !self.counter_on() || cpu_hz == 0 || stalled == [true, true] {
            return;
        }
        let source = if self.source_rc_fast() { rc_fast_hz } else { 40_000_000 };
        if source == 0 {
            return;
        }
        let denominator = cpu_hz.saturating_mul(5);
        if self.denominator != 0 && self.denominator != denominator {
            self.frac = self.frac * denominator / self.denominator;
        }
        self.denominator = denominator;
        let total = self.frac + cycles * source * 2;
        self.frac = total % denominator;
        let ticks = total / denominator;
        if ticks != 0 {
            self.st.tick(ticks);
        }
    }

    pub fn ticks_until_irq(&self, stalled: [bool; 2]) -> Option<u64> {
        if !self.counter_on() || stalled == [true, true] {
            return None;
        }
        if self.irqs().iter().any(|&b| b) {
            return Some(0);
        }
        self.st.next_deadline()
    }
}

impl Device for SystimerS31 {
    fn read(&mut self, off: u32) -> u32 {
        if !self.clock_on() {
            return 0;
        }
        self.st.read(off)
    }

    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if !self.clock_on() {
            return WriteEffect::NONE;
        }
        // IDF port_systick: PERIOD_MODE after one-shot COMP_LOAD without a second load.
        if matches!(off, 0x34 | 0x38 | 0x3c) {
            let t = ((off - 0x34) / 4) as usize;
            let old = self.st.target_conf[t];
            self.st.write(off, v);
            if v & (1 << 30) != 0 && old & (1 << 30) == 0 && v & 0x03ff_ffff != 0 {
                self.st.write(0x50 + t as u32 * 4, 1);
            }
            return WriteEffect::NONE;
        }
        self.st.write(off, v);
        WriteEffect::NONE
    }

    fn irq_sources(&self) -> u64 {
        Device::irq_sources(&self.st)
    }

    fn clock(&self) -> Option<ClockDomain> {
        Some(ClockDomain::Systimer)
    }

    fn tick(&mut self, ticks: u64) {
        if self.counter_on() {
            self.st.tick(ticks);
        }
    }

    fn has_deadline(&self) -> bool {
        true
    }

    fn next_deadline(&self) -> Option<u64> {
        if !self.counter_on() {
            return None;
        }
        self.st.next_deadline()
    }
}

// ── TIMG: shared TimerGroup + local MWDT stages ─────────────────────────────

#[derive(Default)]
pub struct TimgS31 {
    pub timers: TimerGroup,
    pub wdt: Mwdt,
}

impl TimgS31 {
    pub fn irq_wdt(&self) -> bool {
        self.wdt.irq()
    }

    pub fn advance_xtal(&mut self, ticks: u64) -> Result<(), Fault> {
        self.timers.tick(ticks);
        self.wdt.advance_xtal(ticks)
    }
}

impl Device for TimgS31 {
    fn read(&mut self, off: u32) -> u32 {
        if off < 0x48 {
            self.timers.read(off)
        } else {
            self.wdt.read(off).unwrap_or(0)
        }
    }

    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if off < 0x48 {
            Device::write(&mut self.timers, off, v)
        } else {
            let _ = self.wdt.write(off, v);
            WriteEffect::NONE
        }
    }

    fn irq_sources(&self) -> u64 {
        u64::from(self.wdt.irq())
    }
}

// ── USB Serial/JTAG: shared IP + S31 PCR gate ───────────────────────────────

pub struct UsbS31 {
    pub usj: UsbSerialJtag,
    pub clock: u32,
    attached: bool,
}

impl Default for UsbS31 {
    fn default() -> Self {
        let mut usj = UsbSerialJtag::new(CPU_HZ);
        usj.connected = false;
        Self { usj, clock: 1, attached: false }
    }
}

impl UsbS31 {
    pub fn clock_on(&self) -> bool {
        self.clock & 1 != 0
    }

    pub fn clock_read(&self) -> u32 {
        self.clock
    }

    pub fn clock_write(&mut self, value: u32) {
        self.clock = value & 1;
    }

    pub fn irq(&self) -> bool {
        self.clock_on() && self.usj.irq()
    }

    pub fn attach_host(&mut self, on: bool) {
        self.attached = on;
        self.usj.connected = on;
    }

    pub fn host_send(&mut self, data: &[u8]) -> Result<(), Fault> {
        if !self.attached || !self.clock_on() {
            return Err(Fault::Prohibited);
        }
        if data.is_empty() || data.len() > 64 || !self.usj.rx.is_empty() {
            return Err(Fault::Prohibited);
        }
        self.usj.host_input(data);
        Ok(())
    }

    pub fn host_receive(&mut self) -> Option<Vec<u8>> {
        if !self.attached || !self.clock_on() {
            return None;
        }
        if self.usj.tx_out.is_empty() && self.usj.tx_fifo.is_empty() {
            return None;
        }
        // Prefer flushed bytes; if guest only filled fifo without WR_DONE yet, leave it.
        if self.usj.tx_out.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.usj.tx_out))
    }
}

impl Device for UsbS31 {
    fn read(&mut self, off: u32) -> u32 {
        if !self.clock_on() {
            return 0;
        }
        self.usj.read(off)
    }

    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if !self.clock_on() {
            return WriteEffect::NONE;
        }
        self.usj.write(off, v);
        WriteEffect::NONE
    }

    fn irq_sources(&self) -> u64 {
        u64::from(self.irq())
    }

    fn clock(&self) -> Option<ClockDomain> {
        Some(ClockDomain::Cpu)
    }

    fn tick(&mut self, cycles: u64) {
        if self.clock_on() {
            self.usj.tick(cycles);
        }
    }
}

// ── Efuse: S31 ReadImage protocol as Device (shared Efuse is RegRam-only) ───

impl Device for Efuse {
    fn read(&mut self, off: u32) -> u32 {
        Efuse::read(self, off).unwrap_or(0)
    }

    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let _ = Efuse::write(self, off, v);
        WriteEffect::NONE
    }

    fn irq_sources(&self) -> u64 {
        u64::from(Efuse::irq(self))
    }
}

fn new_uart() -> Uart {
    let mut uart = Uart::new(UartLayout {
        thrhd_mask: 0xff,
        rxfifo_rst: 1 << 22,
        rxfifo_cnt_mask: 0xff,
    });
    // S31 v6.1 uart_reg.h defaults.
    for (off, value) in [
        (0x14, 694),
        (0x20, 0x1c),
        (0x24, 0x6060),
        (0x88, 3 << 24),
        (0x8c, 36_774_432),
        (0x9c, 1280),
    ] {
        uart.write(off, value);
    }
    uart
}

pub struct Peripherals {
    pub uart0: Uart,
    pub usb: UsbS31,
    pub systimer: SystimerS31,
    pub timg: [TimgS31; 2],
    pub efuse: Efuse,
    pub misc: Misc,
    pub clock: ClockTree<{ Peripherals::CLOCKS.len() }>,
}

impl Default for Peripherals {
    fn default() -> Self {
        Self::new()
    }
}

impl Peripherals {
    pub fn new() -> Self {
        Peripherals {
            uart0: new_uart(),
            usb: UsbS31::default(),
            systimer: SystimerS31::default(),
            timg: [TimgS31::default(), TimgS31::default()],
            efuse: Efuse::default(),
            misc: Misc::new(),
            clock: Self::new_clock(),
        }
    }

    pub fn block_name(block: u32) -> &'static str {
        match block {
            0x2038a => "UART0",
            0x20391 => "USB_SERIAL_JTAG",
            0x20399 => "SYSTIMER",
            0x20580 => "TIMG0",
            0x20581 => "TIMG1",
            0x20715 => "EFUSE",
            _ => "?",
        }
    }

    /// True when `addr` belongs to a block in this DeviceSet (not PCR clock words).
    pub fn owns(addr: u32) -> bool {
        matches!(
            addr >> 12,
            0x2038a | 0x20391 | 0x20399 | 0x20580 | 0x20581 | 0x20715
        )
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        mmio::read32(self, addr)
    }

    pub fn write32(&mut self, addr: u32, v: u32) -> WriteEffect {
        mmio::write32(self, addr, v)
    }
}

device_set! { Peripherals; clock: (clock) CPU_HZ, [(ClockDomain::Xtal, 1), (ClockDomain::Apb, 1), (ClockDomain::Systimer, 1), (ClockDomain::Cpu, 1)];
    0x2038a "UART0" (uart0) => [src::UART0];
    0x20391 "USB_SERIAL_JTAG" (usb) => [src::USB_SERIAL_JTAG];
    0x20399 "SYSTIMER" (systimer) => [src::SYSTIMER_TARGET0, src::SYSTIMER_TARGET1, src::SYSTIMER_TARGET2];
    0x20580 "TIMG0" (timg[0]) => [src::TG0_WDT];
    0x20581 "TIMG1" (timg[1]) => [src::TG1_WDT];
    0x20715 "EFUSE" (efuse) => [NO_SOURCE];
}

impl DeviceSet for Peripherals {
    const BASE: u32 = 0;
    fn block_name(block: u32) -> &'static str {
        Peripherals::block_name(block)
    }
    fn misc(&self) -> &Misc {
        &self.misc
    }
    fn misc_mut(&mut self) -> &mut Misc {
        &mut self.misc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::Fault;

    fn systimer_on() -> SystimerS31 {
        let mut t = SystimerS31::default();
        t.clock_write(0x11).unwrap();
        t
    }

    #[test]
    fn systimer_pcr_gate_and_reset() {
        let mut t = systimer_on();
        // Default CONF already enables unit0; advance while counter_on.
        t.advance_ticks(5, [false; 2]);
        assert_eq!(t.st.unit[0], 5);
        // APB gate clear (0x10): MMIO off, but counter_on still true — counters keep running.
        t.clock_write(0x10).unwrap();
        assert!(!t.clock_on());
        t.advance_ticks(3, [false; 2]);
        assert_eq!(t.st.unit[0], 8);
        // Soft reset (bit1) clears IP state while latching clock word.
        t.clock_write(0x13).unwrap();
        assert_eq!(t.st.unit[0], 0);
        assert_eq!(t.clock_write(0x15), Err(Fault::Prohibited));
        // Counter clock bit clear stops ticks.
        t.clock_write(0x11).unwrap();
        t.advance_ticks(2, [false; 2]);
        assert_eq!(t.st.unit[0], 2);
        t.clock_write(0x01).unwrap();
        t.advance_ticks(10, [false; 2]);
        assert_eq!(t.st.unit[0], 2);
    }

    #[test]
    fn systimer_period_mode_enter_reloads_like_comp_load() {
        // IDF port_systick: one-shot COMP_LOAD then PERIOD_MODE without a second load.
        let mut t = systimer_on();
        // unit0 + target0 WORK_EN
        Device::write(&mut t, 0, 0x4100_0000);
        Device::write(&mut t, 0x20, 4); // target0 value low
        Device::write(&mut t, 0x50, 1); // COMP_LOAD
        t.advance_ticks(4, [false; 2]);
        assert_eq!(t.st.int_raw & 1, 1);
        Device::write(&mut t, 0x6c, 1); // clear
        // Enter period mode — wrap auto COMP_LOAD from current count
        Device::write(&mut t, 0x34, (1 << 30) | 4);
        t.advance_ticks(12, [false; 2]);
        assert_ne!(t.st.int_raw & 1, 0);
    }

    #[test]
    fn usb_shared_packet_and_clock_gate() {
        let mut u = UsbS31::default();
        u.attach_host(true);
        // fill 64-byte packet then WR_DONE via write path that flushes at 64
        for b in 0..64u32 {
            Device::write(&mut u, 0, b);
        }
        assert_eq!(u.host_receive(), Some((0..64).map(|x| x as u8).collect()));
        Device::write(&mut u, 0, 65);
        Device::write(&mut u, 4, 1); // WR_DONE
        assert_eq!(u.host_receive(), Some(vec![65]));
        u.clock_write(0);
        assert_eq!(u.host_send(b"x"), Err(Fault::Prohibited));
        assert_eq!(Device::read(&mut u, 4), 0);
    }

    #[test]
    fn device_set_owns_expected_blocks() {
        assert!(Peripherals::owns(0x2038_a000));
        assert!(Peripherals::owns(0x2039_1000));
        assert!(Peripherals::owns(0x2039_9000));
        assert!(Peripherals::owns(0x2058_0000));
        assert!(Peripherals::owns(0x2058_1000));
        assert!(Peripherals::owns(0x2071_5000));
        assert!(!Peripherals::owns(SYSTIMER_CLOCK));
        assert!(!Peripherals::owns(0x2058_3000)); // GPIO stays local
    }
}
