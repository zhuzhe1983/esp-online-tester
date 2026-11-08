//! UART: TX to the host console, RX from it through the 128-byte receive FIFO; the transmit
//! side reads as idle (its FIFO count is 0 and TXFIFO_EMPTY/TX_DONE stay raised). The register
//! map differs a little between the chips — field widths, where rxfifo_rst sits — so the chip
//! crate picks a [`UartLayout`].
use std::collections::VecDeque;
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;

const RX_FIFO_SIZE: usize = 128;
const INT_RXFIFO_FULL: u32 = 1 << 0;
const INT_TXFIFO_EMPTY: u32 = 1 << 1;
pub const INT_FRM_ERR: u32 = 1 << 3;
const INT_RXFIFO_OVF: u32 = 1 << 4;
/// RXFIFO_TOUT (bit 8 on the S3/C3/C6 maps): silicon raises it once the line has been idle for
/// rx_tout_thrhd symbols with bytes still in the FIFO. The IDF / Arduino drivers rely on it to
/// pick up short messages below rxfifo_full_thrhd (a one-character command). Modelled as a level
/// with no idle delay: raised while the FIFO holds bytes, so the driver's ISR drains them.
const INT_RXFIFO_TOUT: u32 = 1 << 8;
const INT_TX_DONE: u32 = 1 << 14;
/// TXFIFO_EMPTY and TX_DONE: always true here, so INT_CLR cannot take them down.
const INT_ALWAYS: u32 = INT_TXFIFO_EMPTY | INT_TX_DONE;

/// What differs between the chips' UART register maps (IDF `uart_reg.h` per chip).
#[derive(Clone, Copy, Debug)]
pub struct UartLayout {
    /// CONF1 rxfifo_full_thrhd: mask of the field at bit 0; txfifo_empty_thrhd starts right above it
    pub thrhd_mask: u32,
    /// CONF0 (CONF0_SYNC on the C6) rxfifo_rst bit
    pub rxfifo_rst: u32,
    /// STATUS rxfifo_cnt: mask of the field at bit 0
    pub rxfifo_cnt_mask: u32,
    /// CONF0 autobaud_en bit, or 0 where the autobaud detector is not modelled. The mask ROM's UART download mode
    /// polls RXD_CNT (0x30) for edges and derives the divisor from LOWPULSE/HIGHPULSE (0x28/0x2c).
    pub autobaud_en: u32,
    /// Offset of LOWPULSE; HIGHPULSE and RXD_CNT follow at +4 and +8 (0x28 on S3/C3, 0x7c on C6).
    pub autobaud_regs: u32,
    /// UART core clock the ROM runs the detector on when it is not in CLK_CONF (0x78), as on the C6 (PCR); 0 = CLK_CONF.
    pub autobaud_clock_hz: u32,
}
impl UartLayout {
    pub const S3: UartLayout = UartLayout { thrhd_mask: 0x3ff, rxfifo_rst: 1 << 17, rxfifo_cnt_mask: 0x3ff, autobaud_en: 1 << 27, autobaud_regs: 0x28, autobaud_clock_hz: 0 };
    pub const C3: UartLayout = UartLayout { thrhd_mask: 0x1ff, rxfifo_rst: 1 << 17, rxfifo_cnt_mask: 0x3ff, autobaud_en: 1 << 27, autobaud_regs: 0x28, autobaud_clock_hz: 0 };
    pub const C6: UartLayout = UartLayout { thrhd_mask: 0xff, rxfifo_rst: 1 << 22, rxfifo_cnt_mask: 0xff, autobaud_en: 1 << 19, autobaud_regs: 0x7c, autobaud_clock_hz: 40_000_000 };
}

// ------------------------------------------------------------------ UART
pub struct Uart { pub tx_out: Vec<u8>, pub int_raw: u32, pub int_ena: u32, layout: UartLayout, rx: VecDeque<u8>, ram: RegRam, edges: u32, queue: VecDeque<u8>, credit: u64 }
impl Uart {
    pub fn new(layout: UartLayout) -> Self { Uart { tx_out: Vec::new(), int_raw: INT_ALWAYS, int_ena: 0, layout, rx: VecDeque::new(), ram: RegRam::new(), edges: 0, queue: VecDeque::new(), credit: 0 } }
    /// Bytes from the host into the receive FIFO; what does not fit is dropped and flagged RXFIFO_OVF.
    pub fn host_input(&mut self, data: &[u8]) {
        if self.autobaud_on() { self.edges = self.edges.saturating_add(9 * data.len() as u32).min(0x3ff); }
        for &b in data {
            if self.rx.len() >= RX_FIFO_SIZE { self.int_raw |= INT_RXFIFO_OVF; break; }
            self.rx.push_back(b);
        }
        self.refresh_rx_full();
    }
    /// Bytes injected by the host (a console, esptool through a bridge). The host has no line rate and can send far more
    /// than the 128-byte FIFO holds (an esptool stub upload is 6 KiB per packet), and firmware sized its buffers for bytes
    /// arriving at the baud rate. What fits goes into the FIFO now; the rest waits and `advance` releases it at the line
    /// rate, instead of being dropped with RXFIFO_OVF as `host_input` (a real wire) does. Chips that call this must call
    /// `advance` from their device tick (`pace_input`) and count `input_deadline` in their idle deadline.
    pub fn host_input_queued(&mut self, data: &[u8]) {
        if self.autobaud_on() { self.edges = self.edges.saturating_add(9 * data.len() as u32).min(0x3ff); }
        self.queue.extend(data);
        self.pump(usize::MAX);
    }
    /// Bytes from a board's wire (pin-driven input, already at line rate): straight into the FIFO, overflowing as on
    /// silicon. Only while host bytes are still queued do they wait behind them, so the order the two sources sent in
    /// is kept (the overflow check then happens when they are released).
    pub fn wire_input(&mut self, data: &[u8]) {
        if self.queue.is_empty() { self.host_input(data); } else { self.queue.extend(data); }
    }
    /// Host bytes waiting for room in the FIFO: the only case in which `advance` has work.
    pub fn has_queued(&self) -> bool { !self.queue.is_empty() }
    fn pump(&mut self, max: usize) -> usize {
        let mut moved = 0;
        while moved < max && self.rx.len() < RX_FIFO_SIZE {
            let Some(b) = self.queue.pop_front() else { break };
            self.rx.push_back(b);
            moved += 1;
        }
        if moved > 0 { self.refresh_rx_full(); }
        moved
    }
    /// CPU cycles per byte at `baud` (10 bits per byte; 115200 when none is configured).
    fn cycles_per_byte(cpu_hz: u64, baud: Option<u32>) -> u64 {
        let baud = baud.filter(|b| *b > 0).unwrap_or(115_200) as u64;
        (cpu_hz * 10 / baud).max(1)
    }
    /// Release queued host bytes at 10 bits per byte. `baud` is the configured rate (115200 when none is set). Credit
    /// only accrues while a byte could move, so a full FIFO holds the line and no burst follows when it drains.
    /// Returns whether any byte moved (the receive interrupt level may have changed).
    pub fn advance(&mut self, cycles: u64, cpu_hz: u64, baud: Option<u32>) -> bool {
        if self.queue.is_empty() || self.rx.len() >= RX_FIFO_SIZE { self.credit = 0; return false; }
        let per_byte = Self::cycles_per_byte(cpu_hz, baud);
        self.credit += cycles;
        let due = (self.credit / per_byte) as usize;
        if due == 0 { return false; }
        let moved = self.pump(due);
        self.credit = if moved < due { 0 } else { self.credit - moved as u64 * per_byte };
        moved > 0
    }
    /// CPU cycles until `advance` would release the next queued byte, or None when nothing can move (no queue, or a full
    /// FIFO that only a read can open). An idle core must not be fast-forwarded past it.
    pub fn input_deadline(&self, cpu_hz: u64, baud: Option<u32>) -> Option<u64> {
        if self.queue.is_empty() || self.rx.len() >= RX_FIFO_SIZE { return None; }
        Some(Self::cycles_per_byte(cpu_hz, baud).saturating_sub(self.credit).max(1))
    }
    /// CONF1 rxfifo_full_thrhd (the layout says how wide); the silicon reset value is 0x60, a
    /// driver that wants every byte sets 1. RXFIFO_FULL is a level here: it stays raised while the count is at or
    /// over the threshold, so a driver that clears it before draining is woken again.
    fn rx_full_threshold(&self) -> usize { ((self.ram.read(0x24) & self.layout.thrhd_mask) as usize).max(1) }
    /// RXFIFO_FULL follows the FIFO level on classic silicon: it falls when a read leaves fewer bytes than the threshold,
    /// without software writing INT_CLR. The mask ROM's UART interrupt handler relies on it: when the other UART of the
    /// pair interrupts, it only reads that UART's FIFO and never clears the interrupt. (`refresh_rx_full` still re-raises it
    /// on the next arrival or when INT_CLR leaves the count at or above the threshold.)
    fn lower_rx_full_when_drained(&mut self) {
        if self.rx.len() < self.rx_full_threshold() { self.int_raw &= !INT_RXFIFO_FULL; }
    }
    fn refresh_rx_full(&mut self) {
        if self.rx.len() >= self.rx_full_threshold() { self.int_raw |= INT_RXFIFO_FULL; }
        if !self.rx.is_empty() { self.int_raw |= INT_RXFIFO_TOUT; }
    }
    /// Configured baud, including the fractional UART and source-clock dividers.
    /// `clock` is CLK_CONF on S3/C3 or PCR_UARTn_SCLK_CONF on C6.
    pub fn baud(&self, clock: u32, rc_hz: u32) -> Option<u32> {
        let source = match (clock >> 20) & 3 { 1 => 80_000_000u64, 2 => rc_hz as u64, 3 => 40_000_000, _ => return None };
        let div = self.ram.read(0x14);
        let uart_div = (div & 0xfff) as u64 * 16 + ((div >> 20) & 15) as u64;
        if uart_div == 0 { return None; }
        let denominator = (clock & 63) as u64;
        let numerator = ((clock >> 6) & 63) as u64;
        let integer = ((clock >> 12) & 255) as u64 + 1;
        let (n, d) = if denominator == 0 { (integer, 1) } else { (integer * denominator + numerator, denominator) };
        Some((source * 16 * d / (n * uart_div)) as u32)
    }
    pub fn clock_config(&self) -> u32 { self.ram.read(0x78) }
    pub fn rx_pending(&self) -> usize { self.rx.len() }
    /// Classic ESP32 has a 20-bit divider and selects APB or the 1 MHz reference tick.
    /// ESP-IDF v5.5.4: components/soc/esp32/register/soc/uart_reg.h
    /// (UART_CLKDIV, UART_CLKDIV_FRAG, UART_TICK_REF_ALWAYS_ON) and
    /// components/soc/esp32/include/soc/soc.h (APB_CLK_FREQ, REF_CLK_FREQ).
    /// These fields match v4.4.8, where uart_reg.h is under include/soc/.
    /// Uses the nominal 80 MHz APB clock.
    pub fn classic_baud(&self) -> Option<u32> {
        let div = self.ram.read(0x14);
        let divisor = u64::from(div & 0xfffff) * 16 + u64::from((div >> 20) & 15);
        let source = if self.ram.read(0x20) & (1 << 27) != 0 { 80_000_000u64 } else { 1_000_000 };
        (divisor != 0).then(|| (source * 16 / divisor) as u32)
    }
    fn autobaud_on(&self) -> bool { self.layout.autobaud_en != 0 && self.ram.read(0x20) & self.layout.autobaud_en != 0 }
    /// One bit time of the sender, in UART core-clock cycles, less one: the ROM's `((low + high) << 3) + 16` is then
    /// 16 * clock / baud. Host bytes have no line rate, so this reports esptool's sync rate, 115200.
    fn autobaud_pulse(&self) -> u32 {
        let core = if self.layout.autobaud_clock_hz != 0 { self.layout.autobaud_clock_hz } else {
            let clock = self.clock_config();
            let src = match (clock >> 20) & 3 { 2 => 17_500_000u32, 3 => 40_000_000, _ => 80_000_000 };
            src / (((clock >> 12) & 255) + 1)
        };
        (core / 115_200).saturating_sub(1).min(0xfff)
    }
    pub fn read(&mut self, off: u32) -> u32 {
        match off {
            o if self.layout.autobaud_en != 0 && o >= self.layout.autobaud_regs && o <= self.layout.autobaud_regs + 8 => {
                if o == self.layout.autobaud_regs + 8 { self.edges }                            // RXD_CNT
                else if self.autobaud_on() || self.edges > 0 { self.autobaud_pulse() }          // LOWPULSE / HIGHPULSE
                else { self.ram.read(o) }
            }
            0x0 => {
                let b = self.rx.pop_front().map(|b| b as u32).unwrap_or(0);
                self.lower_rx_full_when_drained();
                b
            }
            0x4 => self.int_raw,
            0x8 => self.int_raw & self.int_ena,
            0xc => self.int_ena,
            0x1c => 0xe000_c000 | (self.rx.len() as u32 & self.layout.rxfifo_cnt_mask),   // STATUS: rxfifo_cnt, tx count 0, TXD/RTSN/DSRN idle levels as on silicon
            0x98 => 0,                              // REG_UPDATE (C6 and later): the driver sets it and spins until hardware clears it
            _ => self.ram.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x0 => self.tx_out.push(v as u8),
            0xc => self.int_ena = v,
            0x10 => { self.int_raw &= !v | INT_ALWAYS; self.refresh_rx_full(); }
            0x20 => {
                if v & self.layout.rxfifo_rst != 0 { self.rx.clear(); }
                let ab = self.layout.autobaud_en;
                if ab != 0 && v & ab != 0 && self.ram.read(off) & ab == 0 { self.edges = 0; }   // detector restarts
                self.ram.write(off, v);
            }   // CONF0 rxfifo_rst
            0x24 => { self.ram.write(off, v); self.refresh_rx_full(); }
            _ => self.ram.write(off, v),
        }
    }
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
}
impl Device for Uart {
    fn read(&mut self, off: u32) -> u32 { Uart::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Uart::write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
}

/// Release queued host input on every UART (`Uart::advance`), each at its own baud. Returns whether a receive
/// interrupt level may have changed. One call per device tick, only while `Uart::has_queued` holds for some port.
pub fn pace_input<'a>(ports: impl IntoIterator<Item = (&'a mut Uart, Option<u32>)>, cycles: u64, cpu_hz: u64) -> bool {
    ports.into_iter().fold(false, |moved, (u, baud)| u.advance(cycles, cpu_hz, baud) | moved)
}
/// The earliest `Uart::input_deadline` of any port, for the chip's idle deadline.
pub fn input_deadline<'a>(ports: impl IntoIterator<Item = (&'a Uart, Option<u32>)>, cpu_hz: u64) -> Option<u64> {
    ports.into_iter().filter_map(|(u, baud)| u.input_deadline(cpu_hz, baud)).min()
}

/// Interrupt-delivery latency after a UART INT_ENA write. On silicon the interrupt matrix and the CPU's controller
/// take a few cycles; the mask ROMs' `uart_buff_switch` enables a UART's interrupt and stores which UART its handler
/// serves in the next instructions, so an interrupt taken in between runs the handler for the previous UART and faults.
/// The bus arms this on the write and presents the new level once `CYCLES` have passed, counted from the tick after
/// the one that ends the block holding the write: that tick can carry cycles spent before the write, so it does not
/// count, which makes the delay independent of where blocks and quanta end.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnaLatency(Option<(bool, u64)>);
impl EnaLatency {
    pub const CYCLES: u64 = 16;
    pub fn arm(&mut self) { self.0 = Some((false, Self::CYCLES)); }
    /// An INT_ENA write from `old` to `new`. Only newly enabled bits wait for the latency (the ROM's case); a write that
    /// only disables or keeps sources is presented at once, so a driver that masks an interrupt takes no late one.
    /// Returns whether the bus should re-derive its interrupt lines now.
    pub fn on_write(&mut self, old: u32, new: u32) -> bool {
        if new & !old != 0 { self.arm(); false } else { true }
    }
    pub fn pending(&self) -> bool { self.0.is_some() }
    /// Advance by a tick of `cycles`; true when the level is due now (the bus re-derives its interrupt lines).
    pub fn tick(&mut self, cycles: u64) -> bool {
        match self.0 {
            None => false,
            Some((false, left)) => { self.0 = Some((true, left)); false }
            Some((true, left)) if cycles >= left => { self.0 = None; true }
            Some((true, left)) => { self.0 = Some((true, left - cycles)); false }
        }
    }
    /// Cycles an idle core may skip before the level is due.
    pub fn deadline(&self) -> Option<u64> { self.0.map(|(_, left)| left.max(1)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_baud_uses_its_divider_and_clock_source() {
        let mut u = Uart::new(UartLayout::S3);
        u.write(0x14, 0);
        assert_eq!(u.classic_baud(), None);
        u.write(0x20, 1 << 27);
        u.write(0x14, 694);
        assert_eq!(u.classic_baud(), Some(115273));
        u.write(0x20, 0);
        u.write(0x14, (2 << 20) | 104);
        assert_eq!(u.classic_baud(), Some(9603));
        u.write(0x20, 1 << 27);
        u.write(0x14, 1 << 16);
        assert_eq!(u.classic_baud(), Some(1220));
    }

    /// The Linux esp32_uart driver's receive path: threshold 1, RXFIFO_FULL enabled, count from
    /// STATUS, pop the FIFO, then INT_CLR — and the line must drop only once the FIFO is empty.
    #[test]
    fn receive_fifo_drives_rxfifo_full_as_a_level() {
        let mut u = Uart::new(UartLayout::S3);
        u.write(0x24, 1); u.write(0xc, INT_RXFIFO_FULL);
        assert!(!u.irq());
        u.host_input(b"ro");
        assert!(u.irq()); assert_eq!(u.read(0x1c) & 0x3ff, 2);
        u.write(0x10, INT_RXFIFO_FULL);          // cleared early: still two bytes waiting
        assert!(u.irq());
        assert_eq!((u.read(0x0), u.read(0x0)), (b'r' as u32, b'o' as u32));
        u.write(0x10, INT_RXFIFO_FULL);
        assert!(!u.irq()); assert_eq!(u.read(0x1c) & 0x3ff, 0); assert_eq!(u.read(0x0), 0);
        assert_eq!(u.read(0x4) & INT_ALWAYS, INT_ALWAYS);
    }
    #[test]
    fn queued_host_input_is_released_at_the_line_rate_instead_of_overflowing() {
        let mut u = Uart::new(UartLayout::C3);
        let data: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        u.host_input_queued(&data);
        assert_eq!(u.read(0x1c) & 0x3ff, RX_FIFO_SIZE as u32);
        assert_eq!(u.read(0x4) & INT_RXFIFO_OVF, 0);
        let per_byte = 160_000_000 * 10 / 115_200;
        assert!(!u.advance(per_byte * 5, 160_000_000, None), "a full FIFO holds the line");
        let mut got: Vec<u8> = (0..10).map(|_| u.read(0) as u8).collect();
        assert!(!u.advance(per_byte - 1, 160_000_000, None));
        assert!(u.advance(1, 160_000_000, None));
        assert_eq!(u.read(0x1c) & 0x3ff, RX_FIFO_SIZE as u32 - 9, "exactly one byte per byte time, nothing banked");
        while u.has_queued() || u.read(0x1c) & 0x3ff != 0 {
            while u.read(0x1c) & 0x3ff != 0 { got.push(u.read(0) as u8); }
            u.advance(per_byte * 200, 160_000_000, None);
        }
        assert_eq!(got, data);
    }

    #[test]
    fn input_deadline_is_the_next_byte_time_and_wire_input_keeps_order() {
        let mut u = Uart::new(UartLayout::C3);
        let per_byte = 160_000_000 * 10 / 115_200;
        assert_eq!(u.input_deadline(160_000_000, None), None);
        u.host_input_queued(&[1; 130]);
        assert_eq!(u.input_deadline(160_000_000, None), None, "full FIFO: only a read can make room");
        u.wire_input(&[2]);
        assert_eq!(u.read(0x4) & INT_RXFIFO_OVF, 0, "behind the queue, not overflowing it");
        let _ = u.read(0);
        assert_eq!(u.input_deadline(160_000_000, None), Some(per_byte));
        u.advance(per_byte - 10, 160_000_000, None);
        assert_eq!(u.input_deadline(160_000_000, None), Some(10));
        while u.rx_pending() > 0 { let _ = u.read(0); }
        for _ in 0..3 { u.advance(per_byte, 160_000_000, None); }
        let tail: Vec<u32> = (0..3).map(|_| u.read(0)).collect();
        assert_eq!(tail, [1, 1, 2], "the wire byte arrives after the host bytes queued before it");
        u.wire_input(&[3; RX_FIFO_SIZE + 1]);
        assert_ne!(u.read(0x4) & INT_RXFIFO_OVF, 0, "with nothing queued, wire input overflows as on silicon");
    }

    #[test]
    fn ena_latency_does_not_count_the_tick_that_ends_the_writing_block() {
        let mut l = EnaLatency::default();
        assert!(!l.tick(1_000));
        l.arm();
        assert!(!l.tick(1_000), "the block that wrote INT_ENA may have spent these cycles before the write");
        for _ in 0..EnaLatency::CYCLES - 1 { assert!(!l.tick(1)); }
        assert!(l.tick(1));
        assert!(!l.pending());
    }

    #[test]
    fn autobaud_counts_edges_and_reports_a_115200_bit_time() {
        let mut u = Uart::new(UartLayout::C3);
        u.write(0x78, 1 << 20);                 // CLK_CONF: APB (80 MHz), divider 1
        u.write(0x20, 1 << 27);                 // AUTOBAUD_EN
        assert_eq!(u.read(0x30), 0);
        u.host_input_queued(&[0x07, 0x07, 0x12, 0x20].repeat(4));
        assert!(u.read(0x30) > 127, "the ROM's connection check needs more than 127 edges");
        let div = ((u.read(0x28) + u.read(0x2c)) << 3) + 16;   // uart_baudrate_detect
        assert_eq!(div, 16 * 80_000_000 / 115_200 / 16 * 16);
        u.write(0x20, 0);
        u.write(0x20, 1 << 27);                 // re-enabled: the detector starts over
        assert_eq!(u.read(0x30), 0);
        let mut c6 = Uart::new(UartLayout::C6);
        c6.write(0x20, 1 << 19);
        c6.host_input_queued(&[0x55; 16]);
        assert!(c6.read(0x84) > 127);
        assert_eq!(((c6.read(0x7c) + c6.read(0x80)) << 3) + 16, 16 * (40_000_000 / 115_200));
    }

    #[test]
    fn receive_fifo_overflow_is_flagged_and_reset_by_conf0() {
        let mut u = Uart::new(UartLayout::S3);
        u.host_input(&[b'x'; RX_FIFO_SIZE + 3]);
        assert_eq!(u.read(0x1c) & 0x3ff, RX_FIFO_SIZE as u32);
        assert_ne!(u.read(0x4) & INT_RXFIFO_OVF, 0);
        u.write(0x20, 1 << 17);
        assert_eq!(u.read(0x1c) & 0x3ff, 0);
    }
    /// The C6 map: the IDF driver's default txfifo_empty_thrhd of 10 sits in bits 15:8 of CONF1,
    /// right above an 8-bit rxfifo_full_thrhd, and rxfifo_rst is bit 22 of CONF0_SYNC.
    #[test]
    fn c6_layout_reads_its_own_fields() {
        let mut u = Uart::new(UartLayout::C6);
        u.write(0x24, (10 << 8) | 1); u.write(0xc, INT_RXFIFO_FULL);
        u.host_input(b"x");
        assert!(u.irq());
        u.write(0x20, 1 << 17);   // the S3's rxfifo_rst bit means nothing here
        assert_eq!(u.read(0x1c) & 0xff, 1);
        u.write(0x20, 1 << 22);
        assert_eq!(u.read(0x1c) & 0xff, 0);
    }

    /// A one-character command (below rxfifo_full_thrhd) still wakes the IDF / Arduino driver:
    /// RXFIFO_TOUT is up while bytes wait, and drops once the FIFO is drained and cleared.
    #[test]
    fn short_message_raises_rx_timeout() {
        let mut u = Uart::new(UartLayout::S3);
        u.write(0x24, 120); u.write(0xc, INT_RXFIFO_TOUT);
        u.host_input(b"?");
        assert!(u.irq() && u.read(0x4) & INT_RXFIFO_FULL == 0);
        let _ = u.read(0x0);                      // the driver drains the byte
        u.write(0x10, INT_RXFIFO_TOUT);
        assert!(!u.irq());
    }
}
