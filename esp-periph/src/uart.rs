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
}
impl UartLayout {
    pub const S3: UartLayout = UartLayout { thrhd_mask: 0x3ff, rxfifo_rst: 1 << 17, rxfifo_cnt_mask: 0x3ff };
    pub const C3: UartLayout = UartLayout { thrhd_mask: 0x1ff, rxfifo_rst: 1 << 17, rxfifo_cnt_mask: 0x3ff };
    pub const C6: UartLayout = UartLayout { thrhd_mask: 0xff, rxfifo_rst: 1 << 22, rxfifo_cnt_mask: 0xff };
}

// ------------------------------------------------------------------ UART
pub struct Uart { pub tx_out: Vec<u8>, pub int_raw: u32, pub int_ena: u32, layout: UartLayout, rx: VecDeque<u8>, ram: RegRam }
impl Uart {
    pub fn new(layout: UartLayout) -> Self { Uart { tx_out: Vec::new(), int_raw: INT_ALWAYS, int_ena: 0, layout, rx: VecDeque::new(), ram: RegRam::new() } }
    /// Bytes from the host into the receive FIFO; what does not fit is dropped and flagged RXFIFO_OVF.
    pub fn host_input(&mut self, data: &[u8]) {
        for &b in data {
            if self.rx.len() >= RX_FIFO_SIZE { self.int_raw |= INT_RXFIFO_OVF; break; }
            self.rx.push_back(b);
        }
        self.refresh_rx_full();
    }
    /// CONF1 rxfifo_full_thrhd (the layout says how wide); the silicon reset value is 0x60, a
    /// driver that wants every byte sets 1. RXFIFO_FULL is a level here: it stays raised while the count is at or
    /// over the threshold, so a driver that clears it before draining is woken again.
    fn rx_full_threshold(&self) -> usize { ((self.ram.read(0x24) & self.layout.thrhd_mask) as usize).max(1) }
    /// RXFIFO_FULL follows the FIFO level on classic silicon: it falls when a read leaves fewer bytes than the threshold,
    /// without software writing INT_CLR. The mask ROM's UART interrupt handler relies on it: when the other UART of the
    /// pair interrupts, it only reads that UART's FIFO and never clears the interrupt. (`refresh_rx_full` still re-raises it
    /// on the next arrival or when INT_CLR leaves the count at or above the threshold.)
    pub fn lower_rx_full_when_drained(&mut self) {
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
    pub fn read(&mut self, off: u32) -> u32 {
        match off {
            0x0 => self.rx.pop_front().map(|b| b as u32).unwrap_or(0),
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
            0x20 => { if v & self.layout.rxfifo_rst != 0 { self.rx.clear(); } self.ram.write(off, v); }   // CONF0 rxfifo_rst
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
