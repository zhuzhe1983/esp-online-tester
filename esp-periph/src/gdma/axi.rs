//! The AXI-GDMA register layout over the shared GDMA transfer model.
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;
use super::Gdma;

/// The AXI-GDMA (ESP32-P4 0x5008A000, ESP32-S31 0x20348000; the S31's IDF master axi_dma_reg.h
/// has the same layout): three channel pairs, the IN channels' blocks at 0x68 * n, the OUT
/// channels' at 0x138 + 0x68 * n, each register in the order conf/interrupts first
/// (INT_RAW, INT_ST, INT_ENA, INT_CLR, CONF0, CONF1, FIFO status, POP/PUSH, LINK1, LINK2,
/// STATE, EOF addresses, DSCR, DSCR_BF0/1, PRI, PERI_SEL), the CRC registers after them not
/// modelled (IDF v5.5.4 register/hw_ver3/soc/axi_dma_reg.h). The transfers are the shared
/// GDMA's: this maps the layout onto it. Two differences: a descriptor address is a whole word
/// (LINK2), not 20 bits, and the start/stop bits are in LINK1 (IN: AUTO_RET 0, STOP 1, START 2,
/// RESTART 3, PARK 4; OUT: STOP 0, START 1, RESTART 2, PARK 3). The interrupt bits are the
/// shared model's (OUT_DONE 0, OUT_EOF 1, OUT_DSCR_ERR 2, OUT_TOTAL_EOF 3). GPSPI2, LCD0 and
/// PARLIO are its peripherals (include/soc/gdma_channel.h: SPI2 is trigger 1).
pub struct AxiDma { pub gdma: Gdma, link2: [[u32; 3]; 2], ram: RegRam }
impl Default for AxiDma { fn default() -> Self { Self::new() } }
impl AxiDma {
    pub const CHANNELS: usize = 3;
    pub fn new() -> Self { AxiDma { gdma: Gdma::new(), link2: [[0; 3]; 2], ram: RegRam::new() } }
    /// (out, channel, register within its block) of an offset in the channel area
    fn locate(off: u32) -> Option<(bool, usize, u32)> {
        let (out, rel) = if off < 0x138 { (false, off) } else if off < 0x270 { (true, off - 0x138) } else { return None };
        Some((out, (rel / 0x68) as usize, rel % 0x68))
    }
    /// The shared model's offset for an AXI register other than LINK1/LINK2.
    fn shared(out: bool, ch: usize, k: u32) -> Option<u32> {
        let t = match k {
            0x00 => 0x08, 0x04 => 0x0c, 0x08 => 0x10, 0x0c => 0x14, 0x10 => 0x00, 0x14 => 0x04, 0x18 => 0x18, 0x1c => 0x1c,
            0x28 => 0x24, 0x2c => 0x28, 0x30 => 0x2c, 0x34 => 0x30, 0x38 => 0x34, 0x3c => 0x38, 0x40 => 0x44, 0x44 => 0x48,
            _ => return None,
        };
        Some(ch as u32 * 0xc0 + if out { 0x60 } else { 0 } + t)
    }
}
impl Device for AxiDma {
    fn read(&mut self, off: u32) -> u32 {
        if let Some((out, ch, k)) = Self::locate(off) {
            let running = if out { self.gdma.out[ch].running } else { self.gdma.inp[ch].running };
            return match k {
                0x20 => if running { 0 } else if out { 1 << 3 } else { 1 << 4 },     // PARK: the link is idle
                0x24 => self.link2[out as usize][ch],
                _ => match Self::shared(out, ch, k) { Some(o) => self.gdma.read(o), None => self.ram.read(off) },
            };
        }
        match off { 0x28c..=0x2a0 => 1, _ => self.ram.read(off) }                   // IN/OUT_RESET_AVAIL: a reset is allowed
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let Some((out, ch, k)) = Self::locate(off) else { self.ram.write(off, v); return WriteEffect::NONE };
        match k {
            0x24 => self.link2[out as usize][ch] = v,
            0x20 => {
                let (stop, start, restart) = if out { (v & 1, v >> 1 & 1, v >> 2 & 1) } else { (v >> 1 & 1, v >> 2 & 1, v >> 3 & 1) };
                let addr = self.link2[out as usize][ch];
                self.gdma.addr_base = addr & 0xfff0_0000;
                // the shared LINK: address 19:0, then OUT STOP 20 / START 21 / RESTART 22, IN STOP 21 / START 22 / RESTART 23
                let s = if out { 20 } else { 21 };
                let word = (addr & 0xf_ffff) | stop << s | start << (s + 1) | restart << (s + 2);
                self.gdma.write(ch as u32 * 0xc0 + if out { 0x80 } else { 0x20 }, word);
            }
            _ => match Self::shared(out, ch, k) { Some(o) => self.gdma.write(o, v), None => self.ram.write(off, v) },
        }
        WriteEffect::NONE
    }
    /// bits 0..3 the OUT channels, 5..8 the IN channels, as the shared model numbers them
    fn irq_sources(&self) -> u64 { Device::irq_sources(&self.gdma) }
    fn debug(&mut self, on: bool) { self.gdma.dbg = on; }
}

