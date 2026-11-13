//! The ESP32-S31's AHB-GDMA layout (SOC_AHB_GDMA_VERSION 2) over the shared GDMA transfer model.
//!
//! IDF master components/soc/esp32s31/register/soc/ahb_dma_reg.h: five channel pairs; the IN
//! interrupt blocks (INT_RAW, INT_ST, INT_ENA, INT_CLR) at 0x10 * n, the OUT ones at
//! 0x50 + 0x10 * n, MISC_CONF at 0xA4; each IN channel's block at 0x100 + 0x100 * n (CONF0,
//! CONF1, FIFO status, POP, LINK, LINK_ADDR, STATE, SUC_EOF, ERR_EOF, DONE, DSCR, DSCR_BF0/1,
//! PRI, PERI_SEL), the OUT block the same at +0x80. As on the AXI-GDMA the descriptor address
//! is a whole word (LINK_ADDR) and LINK holds the control bits (IN: AUTO_RET 0, STOP 1,
//! START 2, RESTART 3, PARK 4; OUT: STOP 0, START 1, RESTART 2, PARK 3). The interrupt bits are
//! the shared model's (IN DONE 0, SUC_EOF 1, ERR_EOF 2, DSCR_ERR 3; OUT DONE 0, EOF 1,
//! DSCR_ERR 2, TOTAL_EOF 3). Peripherals (esp_hal_dma/esp32s31/include/hal/gdma_channel.h):
//! UHCI0 0, I2S0 1-4, I2S1 5-8, RMT0 10.
use super::{Gdma, GDMA_CHANNELS};
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;

pub struct AhbDmaV2 { pub gdma: Gdma, link_addr: [[u32; GDMA_CHANNELS]; 2], ram: RegRam }
impl Default for AhbDmaV2 { fn default() -> Self { Self::new() } }
impl AhbDmaV2 {
    pub fn new() -> Self { AhbDmaV2 { gdma: Gdma::new(), link_addr: [[0; GDMA_CHANNELS]; 2], ram: RegRam::new() } }
    /// (out, channel, register within its half-block) of an offset in the channel area
    fn locate(off: u32) -> Option<(bool, usize, u32)> {
        if !(0x100..0x100 + 0x100 * GDMA_CHANNELS as u32).contains(&off) { return None; }
        let rel = off - 0x100;
        let (n, k) = ((rel / 0x100) as usize, rel % 0x100);
        Some((k >= 0x80, n, k % 0x80))
    }
    /// The shared model's offset of a register other than LINK/LINK_ADDR; None for the ones it does not hold.
    fn shared(off: u32) -> Option<u32> {
        if off < 0x50 { return Some((off / 0x10) * 0xc0 + 0x08 + off % 0x10); }
        if off < 0xa0 { let o = off - 0x50; return Some((o / 0x10) * 0xc0 + 0x68 + o % 0x10); }
        if off == 0xa4 { return Some(0x3c8); }
        let (out, ch, k) = Self::locate(off)?;
        let t = match k {
            0x00 => 0x00, 0x04 => 0x04, 0x08 => 0x18, 0x0c => 0x1c, 0x18 => 0x24, 0x1c => 0x28, 0x20 => 0x2c,
            0x28 => 0x30, 0x2c => 0x34, 0x30 => 0x38, 0x34 => 0x44, 0x38 => 0x48,
            _ => return None,
        };
        Some(ch as u32 * 0xc0 + if out { 0x60 } else { 0 } + t)
    }
}
impl Device for AhbDmaV2 {
    fn read(&mut self, off: u32) -> u32 {
        if let Some((out, ch, k)) = Self::locate(off) {
            let running = if out { self.gdma.out[ch].running } else { self.gdma.inp[ch].running };
            match k {
                0x10 => return if running { 0 } else if out { 1 << 3 } else { 1 << 4 },   // PARK: the link is idle
                0x14 => return self.link_addr[out as usize][ch],
                _ => {}
            }
        }
        match Self::shared(off) { Some(o) => self.gdma.read(o), None => self.ram.read(off) }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if let Some((out, ch, k)) = Self::locate(off) {
            match k {
                0x14 => { self.link_addr[out as usize][ch] = v; return WriteEffect::NONE; }
                0x10 => {
                    let (stop, start, restart) = if out { (v & 1, v >> 1 & 1, v >> 2 & 1) } else { (v >> 1 & 1, v >> 2 & 1, v >> 3 & 1) };
                    let addr = self.link_addr[out as usize][ch];
                    self.gdma.addr_base = addr & 0xfff0_0000;
                    // the shared LINK: address 19:0, then OUT STOP 20 / START 21 / RESTART 22, IN STOP 21 / START 22 / RESTART 23
                    let s = if out { 20 } else { 21 };
                    self.gdma.write(ch as u32 * 0xc0 + if out { 0x80 } else { 0x20 }, (addr & 0xf_ffff) | stop << s | start << (s + 1) | restart << (s + 2));
                    return WriteEffect::NONE;
                }
                _ => {}
            }
        }
        match Self::shared(off) { Some(o) => self.gdma.write(o, v), None => self.ram.write(off, v) }
        WriteEffect::NONE
    }
    /// bits 0..5 the OUT channels, 5..10 the IN channels, as the shared model numbers them
    fn irq_sources(&self) -> u64 { Device::irq_sources(&self.gdma) }
    fn debug(&mut self, on: bool) { self.gdma.dbg = on; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s31_layout_reaches_the_shared_channels() {
        let mut d = AhbDmaV2::new();
        d.write(0x58 + 0x10 * 3, 1 << 3);                              // OUT_INT_ENA_CH3: TOTAL_EOF
        d.write(0x18, 1 << 1);                                          // IN_INT_ENA_CH1: SUC_EOF
        assert_eq!((d.gdma.out[3].int_ena, d.gdma.inp[1].int_ena), (1 << 3, 1 << 1));
        d.write(0xa4, 5);
        assert_eq!(d.gdma.misc, 5);
        let out4 = 0x100 + 0x100 * 4 + 0x80;
        d.write(out4 + 0x38, 1);                                        // OUT_PERI_SEL_CH4: I2S0
        d.write(out4, 1 << 2);                                          // OUT_CONF0: AUTO_WRBACK
        d.write(out4 + 0x14, 0x2f01_2340);                              // OUT_LINK_ADDR
        assert_ne!(d.read(out4 + 0x10) & (1 << 3), 0, "parked");
        d.write(out4 + 0x10, 1 << 1);                                   // OUT_LINK: START
        assert_eq!((d.gdma.out[4].running, d.gdma.out[4].desc, d.gdma.out[4].conf0), (true, 0x2f01_2340, 1 << 2));
        assert_eq!(d.gdma.out_channel_for(1), Some(4));
        assert_eq!(d.read(out4 + 0x14), 0x2f01_2340);
        d.write(0x58 + 0x10 * 4, 1 << 3);                              // OUT_INT_ENA_CH4: TOTAL_EOF
        d.gdma.out[4].int_raw = 1 << 3;
        assert_eq!(d.read(0x54 + 0x10 * 4), 1 << 3, "OUT_INT_ST_CH4");
        assert!(d.irq_sources() & (1 << 4) != 0);
        d.write(0x5c + 0x10 * 4, 1 << 3);                               // OUT_INT_CLR_CH4
        assert_eq!(d.gdma.out[4].int_raw, 0);
        let in2 = 0x100 + 0x100 * 2;
        d.write(in2 + 0x38, 5);                                         // IN_PERI_SEL_CH2: I2S1
        d.write(in2 + 0x14, 0x2f00_1000);
        d.write(in2 + 0x10, 1 << 2);                                    // IN_LINK: START
        assert_eq!(d.gdma.in_channel_for(5), Some(2));
        assert_eq!(d.gdma.inp[2].desc, 0x2f00_1000);
    }
}
