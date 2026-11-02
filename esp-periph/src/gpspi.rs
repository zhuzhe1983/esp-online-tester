//! GP-SPI2/3 master. The same register block on the S3, C3 and C6.
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;

/// Store `len` received bytes from `rx` in W registers `base..16`, padding absent input with
/// ones. A macro so the GP-SPI completion path keeps exactly this inline code.
#[macro_export]
macro_rules! fill_spi_w {
    ($w:expr, $base:expr, $len:expr, $rx:expr) => {{
        for k in $base..16 { $w[k] = 0xffff_ffff; }
        let capacity = (16 - $base) * 4;
        for i in 0..$len.min(capacity) {
            let b = $rx.get(i).copied().unwrap_or(0xff);
            let word = $base + i / 4;
            let shift = 8 * (i % 4);
            $w[word] = ($w[word] & !(0xff << shift)) | ((b as u32) << shift);
        }
    }};
}

/// One complete transfer waiting for the board attached to a GP-SPI host.
pub struct GpSpiTransfer {
    pub tx: Vec<u8>,
    pub rx_len: usize,
    data_len: usize,
    rx_word_base: usize,
}

/// General-purpose SPI master as the Arduino HAL / IDF `spi_master` use it. The CPU fills
/// W0..W15 (or, with `DMA_CONF.DMA_TX_ENA`, points a GDMA out-channel at the data), sets the phase
/// enables and lengths, writes CMD.UPDATE then CMD.USR, and waits for the transfer. The chip bus
/// supplies any DMA data, asks its board for the MISO response, then finishes the transfer.
pub struct GpSpi { pub regs: RegRam, pub w: [u32; 16], pub int_raw: u32, pub int_ena: u32, pub transfers: u64, pub log: bool,
                   /// the controller's number in log lines: 2 (SPI2 / FSPI, the default) or 3 (the S3's SPI3 / HSPI)
                   pub host: u8,
                   /// bit length of a MOSI phase that DMA must supply, until the bus completes it
                   pub dma_tx_pending: Option<u32>, pending: Option<GpSpiTransfer> }
impl Default for GpSpi { fn default() -> Self { Self::new() } }

impl GpSpi {
    pub fn new() -> Self { GpSpi { regs: RegRam::new(), w: [0; 16], int_raw: 0, int_ena: 0, transfers: 0, log: false, host: 2, dma_tx_pending: None, pending: None } }
    /// The same block as GP-SPI `host` (2 or 3), for its log lines.
    pub fn with_host(host: u8) -> Self { GpSpi { host, ..Self::new() } }
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x00 => (self.regs.read(0) & !((1 << 23) | (1 << 24)))
                | if self.pending.is_some() { 1 << 24 } else { 0 }, // USR stays busy until completion
            0x34 => self.int_ena, 0x3c => self.int_raw, 0x40 => self.int_raw & self.int_ena,
            0x98..=0xd4 => self.w[((off - 0x98) / 4) as usize],
            0xf0 => 0x2101_0100,
            _ => self.regs.read(off),
        }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x00 => { self.regs.write(0, v & !((1 << 23) | (1 << 24))); if v & (1 << 24) != 0 && self.pending.is_none() { self.transfer(); } }
            0x34 => self.int_ena = v, 0x38 => self.int_raw &= !v,
            // SPI_DMA_INT_SET_REG (0x44, bit 12 SPI_TRANS_DONE_INT_SET: "the software set bit for
            // SPI_TRANS_DONE_INT"; components/soc/esp32s3/register/soc/spi_reg.h, IDF v5.5.2) raises
            // raw status. The IDF driver depends on it: spi_hal_init forces TRANS_DONE through
            // spi_ll_set_int_stat (components/hal/esp32s3/include/hal/spi_ll.h,
            // `hw->dma_int_set.trans_done_int_set = 1`) with the interrupt allocated disabled, and
            // spi_device_queue_trans only enqueues and lets the bus lock enable the interrupt; the ISR
            // then starts the transaction (spi_new_trans -> spi_hal_user_start,
            // components/esp_driver_spi/src/gpspi/spi_master.c, IDF v5.5.2). Without the set bit the
            // ISR never runs and the first queued transfer never starts; polling transfers
            // (spi_device_polling_transmit, Arduino's SPI) set USR themselves and never noticed.
            0x44 => self.int_raw |= v,
            0x98..=0xd4 => self.w[((off - 0x98) / 4) as usize] = v,
            _ => self.regs.write(off, v),
        }
    }
    fn transfer(&mut self) {
        let user = self.regs.read(0x10); let user1 = self.regs.read(0x14); let user2 = self.regs.read(0x18);
        let data_bits = (self.regs.read(0x1c) & 0x3ffff) + 1;
        self.dma_tx_pending = None;
        let mut tx = Vec::new();
        if user & (1 << 31) != 0 {                                        // command phase, LSB byte first
            let n = (((user2 >> 28) & 0xf) + 1).div_ceil(8); let c = user2 & 0xffff;
            for i in 0..n { tx.push((c >> (8 * i)) as u8); }
        }
        if user & (1 << 30) != 0 {                                        // address phase, MSB first from the top of ADDR
            let bits = (user1 >> 27) + 1; let n = bits.div_ceil(8); let a = self.regs.read(0x04);
            for i in 0..n { tx.push((a >> (24 - 8 * i)) as u8); }
        }
        let mut data_len = 0;
        if user & (1 << 27) != 0 {                                        // MOSI data phase from W0.. (or W8.. with HIGHPART)
            let n = data_bits.div_ceil(8) as usize;
            data_len = n;
            if self.regs.read(0x30) & (1 << 28) != 0 {                    // DMA_TX_ENA: the bus fetches the data through GDMA
                self.dma_tx_pending = Some(data_bits);
                if self.log { eprintln!("[spi{}] transfer {} header bytes, {} data bits by DMA", self.host, tx.len(), data_bits); }
            } else {
                let base = if user & (1 << 25) != 0 { 8 } else { 0 };
                for i in 0..n.min((16 - base) * 4) { tx.push((self.w[base + i / 4] >> (8 * (i % 4))) as u8); }
            }
        }
        let rx_len = if user & (1 << 28) != 0 { data_bits.div_ceil(8) as usize } else { 0 };
        let rx_word_base = if user & (1 << 24) != 0 { 8 } else { 0 };
        self.pending = Some(GpSpiTransfer { tx, rx_len, data_len, rx_word_base });
    }

    pub fn has_pending_transfer(&self) -> bool { self.pending.is_some() }

    /// S3 SPI_CLOCK, SPI_USER and SPI_CTRL wire clocks at the SPI source frequency.
    /// Normal SDR command/address/data phases only; this is a wire-time floor,
    /// without DMA setup, memory contention, CS setup/hold or source-clock changes.
    pub fn wire_source_cycles(&self) -> u64 {
        let user = self.regs.read(0x10);
        let ctrl = self.regs.read(0x08);
        let clock = self.regs.read(0x0c);
        let divider = if clock & (1 << 31) != 0 { 1 } else {
            (((clock >> 18) & 15) + 1) * (((clock >> 12) & 63) + 1)
        };
        let lanes = |oct, quad, dual| if oct { 8u64 } else if quad { 4 } else if dual { 2 } else { 1 };
        let mut clocks = 0;
        if user & (1 << 31) != 0 {
            clocks += u64::from(((self.regs.read(0x18) >> 28) & 15) + 1)
                .div_ceil(lanes(ctrl & (1 << 10) != 0, ctrl & (1 << 9) != 0, ctrl & (1 << 8) != 0));
        }
        if user & (1 << 30) != 0 {
            clocks += u64::from((self.regs.read(0x14) >> 27) + 1)
                .div_ceil(lanes(ctrl & (1 << 7) != 0, ctrl & (1 << 6) != 0, ctrl & (1 << 5) != 0));
        }
        if user & (1 << 29) != 0 { clocks += u64::from((self.regs.read(0x14) & 255) + 1); }
        let data_bits = u64::from((self.regs.read(0x1c) & 0x3ffff) + 1);
        let mosi = if user & (1 << 27) != 0 {
            data_bits.div_ceil(lanes(user & (1 << 14) != 0, user & (1 << 13) != 0, user & (1 << 12) != 0))
        } else { 0 };
        let miso = if user & (1 << 28) != 0 {
            data_bits.div_ceil(lanes(ctrl & (1 << 16) != 0, ctrl & (1 << 15) != 0, ctrl & (1 << 14) != 0))
        } else { 0 };
        // DOUTDIN overlaps the data phases; otherwise MOSI precedes MISO.
        clocks += if user & 1 != 0 { mosi.max(miso) } else { mosi + miso };
        (clocks * u64::from(divider)).max(1)
    }

    pub fn take_transfer(&mut self) -> Option<GpSpiTransfer> {
        if self.dma_tx_pending.is_some() {
            return None;
        }
        self.pending.take()
    }

    /// Cancel a transfer that the chip bus could not complete.
    pub fn abort_transfer(&mut self) {
        self.dma_tx_pending = None;
        self.pending = None;
    }

    /// Finish a transaction whose DMA source faulted. Hardware still completes the SPI command,
    /// allowing the driver to observe both TRANS_DONE and the GDMA descriptor error.
    pub fn fail_dma_tx(&mut self) {
        self.abort_transfer();
        self.int_raw |= 1 << 12;
    }

    /// The bus delivered the DMA data phase.
    pub fn complete_dma_tx(&mut self, data: &[u8]) {
        self.dma_tx_pending = None;
        if let Some(transfer) = self.pending.as_mut() {
            transfer.tx.extend_from_slice(&data[..data.len().min(transfer.data_len)]);
        }
        if self.log { eprintln!("[spi{}] dma data {} bytes: {:02x?}", self.host, data.len(), &data[..data.len().min(16)]); }
    }

    /// Complete a transfer with the board's MISO response.
    pub fn finish_transfer(&mut self, transfer: GpSpiTransfer, rx: &[u8]) {
        if transfer.rx_len != 0 {
            fill_spi_w!(self.w, transfer.rx_word_base, transfer.rx_len, rx);
        }
        if self.log { eprintln!("[spi{}] transfer tx={} rx={}: {:02x?}", self.host, transfer.tx.len(), transfer.rx_len, &transfer.tx[..transfer.tx.len().min(16)]); }
        self.transfers += 1;
        self.int_raw |= 1 << 12;                                          // TRANS_DONE
    }
}

impl Device for GpSpi {
    fn read(&mut self, off: u32) -> u32 { GpSpi::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { GpSpi::write(self, off, v); WriteEffect::NONE }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
    fn debug(&mut self, on: bool) { self.log = on; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_time_uses_read_lanes_and_duplex_phase_order() {
        let mut spi = GpSpi::new();
        spi.write(0x0c, 1 << 31);
        spi.write(0x1c, 31);
        for (user, ctrl, expected) in [
            ((1 << 28) | (1 << 13), 0, 32), // FWRITE must not shorten MISO
            (1 << 28, 1 << 15, 8),
            (1 << 28, 1 << 14, 16),
            ((1 << 27) | (1 << 28), 0, 64), // sequential half duplex
            ((1 << 27) | (1 << 28) | 1, 0, 32), // concurrent full duplex
            ((1 << 27) | (1 << 28) | (1 << 13), 1 << 14, 24),
        ] {
            spi.write(0x10, user);
            spi.write(0x08, ctrl);
            assert_eq!(spi.wire_source_cycles(), expected, "USER={user:#x} CTRL={ctrl:#x}");
        }
    }

    #[test]
    fn wire_time_uses_divider_and_quad_data_lanes() {
        let mut spi = GpSpi::new();
        spi.write(0x0c, 1 << 12); // 80 MHz / 2
        spi.write(0x10, (1 << 31) | (1 << 30) | (1 << 27) | (1 << 13));
        spi.write(0x18, 7 << 28); // eight command bits
        spi.write(0x14, 23 << 27); // 24 address bits, still single lane
        spi.write(0x1c, 32768 * 8 - 1);
        assert_eq!(spi.wire_source_cycles(), (8 + 24 + 32768 * 2) * 2);
        spi.write(0x00, 1 << 24);
        assert_ne!(spi.read(0) & (1 << 24), 0);
        let transfer = spi.take_transfer().unwrap();
        spi.finish_transfer(transfer, &[]);
        assert_eq!(spi.read(0) & (1 << 24), 0);
    }

    #[test]
    fn splits_phases_and_writes_the_board_response_to_miso_words() {
        let mut spi = GpSpi::new();
        let user = (1 << 31) | (1 << 30) | (1 << 28) | (1 << 27) | (1 << 24);
        spi.write(0x10, user);
        spi.write(0x14, 23 << 27);
        spi.write(0x18, (7 << 28) | 0x02);
        spi.write(0x04, 0x0400_0000);
        spi.write(0x1c, 23);
        spi.write(0x20, 0x3e);
        spi.write(0x98, 0x4433_2211);

        spi.write(0x00, 1 << 24);
        let transfer = spi.take_transfer().expect("USR must issue one board transaction");
        assert_eq!(transfer.tx, [0x02, 0x04, 0x00, 0x00, 0x11, 0x22, 0x33]);
        assert_eq!(transfer.rx_len, 3);
        assert_eq!(spi.transfers, 0);

        spi.finish_transfer(transfer, &[0xa5, 0x5a]);
        assert_eq!(spi.w[8], 0xffff_5aa5);
        assert_ne!(spi.int_raw & (1 << 12), 0);
        assert_eq!(spi.transfers, 1);
    }

    #[test]
    fn dma_transfer_is_not_visible_until_the_bus_supplies_its_data() {
        let mut spi = GpSpi::new();
        spi.write(0x30, 1 << 28);
        spi.write(0x10, 1 << 27);
        spi.write(0x1c, 15);
        spi.write(0x00, 1 << 24);

        assert!(spi.take_transfer().is_none());
        spi.complete_dma_tx(&[0x12, 0x34]);
        let transfer = spi.take_transfer().expect("DMA completion makes the transfer visible");
        assert_eq!(transfer.tx, [0x12, 0x34]);
        spi.finish_transfer(transfer, &[]);
        assert_eq!(spi.transfers, 1);
    }

    #[test]
    fn cpu_transfer_requires_aborting_an_active_dma_wait() {
        let mut spi = GpSpi::new();
        spi.write(0x30, 1 << 28);
        spi.write(0x10, 1 << 27);
        spi.write(0x1c, 7);
        spi.write(0x00, 1 << 24);
        assert!(spi.take_transfer().is_none());

        spi.abort_transfer();
        spi.write(0x30, 0);
        spi.write(0x98, 0x5a);
        spi.write(0x00, 1 << 24);

        let transfer = spi.take_transfer().expect("CPU transaction may start after abort");
        assert_eq!(transfer.tx, [0x5a]);
    }
    /// SPI_DMA_INT_SET_REG (0x44): the ISR kick ESP-IDF's spi_master relies on. The driver sets
    /// TRANS_DONE here at init while its interrupt is disabled, so enabling the interrupt later
    /// enters the ISR that starts the first queued transaction.
    #[test]
    fn dma_int_set_raises_raw_status_and_the_interrupt() {
        let mut spi = GpSpi::new();
        spi.write(0x34, 1 << 12);
        assert_eq!(spi.read(0x3c), 0);
        assert!(!spi.irq());
        spi.write(0x44, 1 << 12);
        assert_eq!(spi.read(0x3c) & (1 << 12), 1 << 12, "INT_RAW shows the set bit");
        assert_eq!(spi.read(0x40) & (1 << 12), 1 << 12, "INT_ST shows it while enabled");
        assert!(spi.irq());
        spi.write(0x38, 1 << 12);
        assert_eq!(spi.read(0x3c), 0, "INT_CLR clears it again");
        assert!(!spi.irq());
    }
}

#[cfg(test)]
mod busy_rewrite_tests {
    use super::*;
    #[test]
    fn command_read_modify_write_preserves_pending_transfer() {
        let mut spi = GpSpi::new();
        spi.write(0x10, 1 << 27);
        spi.write(0x1c, 7);
        spi.write(0x98, 0x42);
        spi.write(0, 1 << 24);
        spi.write(0x98, 0x99);
        spi.write(0, spi.read(0));
        assert_eq!(spi.take_transfer().unwrap().tx, [0x42]);
    }
}

#[cfg(test)]
mod octal_timing_tests {
    use super::*;
    #[test]
    fn octal_data_uses_eight_lanes() {
        for read in [false, true] {
            let mut spi = GpSpi::new();
            spi.write(0x0c, 1 << 31);
            spi.write(0x10, if read { 1 << 28 } else { (1 << 27) | (1 << 14) });
            spi.write(0x08, if read { 1 << 16 } else { 0 });
            spi.write(0x1c, 64 - 1);
            assert_eq!(spi.wire_source_cycles(), 8);
        }
    }
}
