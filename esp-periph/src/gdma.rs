use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;
mod receive;
pub use receive::{DescriptorWalk, DmaDescriptorFault, DmaDescriptorWord, GDMA_DESCRIPTOR_STEP_BUDGET};
mod transmit;
pub use transmit::{gather_out, OutMemory, SramOut};

// ------------------------------------------------------------------ GDMA (out/TX channels only for now) + I2S0 TX
pub const GDMA_CHANNELS: usize = 5;
pub const GDMA_CH_STRIDE: u32 = 0xC0;
pub const DMA_ADDR_BASE: u32 = 0x3FC0_0000;

#[derive(Clone, Copy, Default)]
pub struct GdmaOutCh {
    pub conf0: u32, pub conf1: u32, pub int_raw: u32, pub int_ena: u32, pub link: u32, pub peri_sel: u32, pub pri: u32,
    pub desc: u32,            // current descriptor address (0 = none)
    pub buf_pos: u32,         // bytes consumed from the current descriptor
    pub running: bool,
    pub eof_desc: u32,
}
impl GdmaOutCh {
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }
}

/// GDMA receive (IN) channel: peripheral -> memory through a descriptor chain.
#[derive(Clone, Copy, Default)]
pub struct GdmaInCh {
    pub conf0: u32, pub conf1: u32, pub int_raw: u32, pub int_ena: u32, pub link: u32, pub peri_sel: u32, pub pri: u32,
    pub desc: u32, pub eof_desc: u32, pub running: bool,
    pub buf_pos: u32,         // bytes filled in the current descriptor
    pub rx_eof_pos: u32,      // peripheral bytes since the last receive EOF
}
impl GdmaInCh { pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 } }

pub struct Gdma { pub out: [GdmaOutCh; GDMA_CHANNELS], pub inp: [GdmaInCh; GDMA_CHANNELS], ram: RegRam, pub misc: u32, pub dbg: bool,
                  /// the high bits of a descriptor address: the LINK registers carry 20 (S3 DRAM at 0x3FC0_0000, C6 SRAM at 0x4080_0000)
                  pub addr_base: u32 }
impl Gdma {
    pub fn new() -> Self { Gdma { out: [GdmaOutCh::default(); GDMA_CHANNELS], inp: [GdmaInCh::default(); GDMA_CHANNELS], ram: RegRam::new(), misc: 0, dbg: false, addr_base: DMA_ADDR_BASE } }
    pub fn read(&self, off: u32) -> u32 {
        if off < GDMA_CH_STRIDE * GDMA_CHANNELS as u32 {
            let ch = (off / GDMA_CH_STRIDE) as usize; let o = off % GDMA_CH_STRIDE; let c = &self.out[ch]; let r = &self.inp[ch];
            return match o {
                0x00 => r.conf0, 0x04 => r.conf1, 0x08 => r.int_raw, 0x0c => r.int_raw & r.int_ena, 0x10 => r.int_ena,
                0x18 => 1 << 1 | 0x1f,                       // INFIFO_STATUS: empty
                0x20 => r.link & 0xF_FFFF,
                0x24 => if r.running { (r.desc & 0x3ffff) | (1 << 20) } else { 0 },
                0x28 => r.eof_desc, 0x2c => r.eof_desc, 0x30 => r.desc, 0x44 => r.pri, 0x48 => r.peri_sel,
                0x60 => c.conf0, 0x64 => c.conf1, 0x68 => c.int_raw, 0x6c => c.int_raw & c.int_ena, 0x70 => c.int_ena,
                // OUTFIFO_STATUS (components/soc/esp32c3/register/soc/gdma_reg.h, IDF v5.5.4: FULL bit 0, EMPTY
                // bit 1, CNT bits 7:2): a channel holding a descriptor has its data in the FIFO, so it is not
                // empty (a C3 rev 0.4 read 0x14 for 5 bytes, idle 0x07800002; the count is not modelled).
                // esp-hal's `start_transfer` waits for EMPTY to clear before starting the peripheral.
                0x78 => if c.running && c.desc != 0 { 0 } else { 0x1f | (1 << 1) },
                0x80 => c.link & 0xF_FFFF,
                0x84 => if c.running { (c.desc & 0x3ffff) | (1 << 20) } else { 0 },   // OUT_STATE: dscr addr + state
                0x88 => c.eof_desc, 0x8c => c.eof_desc, 0x90 => c.desc, 0xa4 => c.pri, 0xa8 => c.peri_sel,
                _ => self.ram.read(off),
            };
        }
        match off { 0x3c8 => self.misc, 0x40c => 0x2008250, _ => self.ram.read(off) }
    }
    pub fn write(&mut self, off: u32, v: u32) {
        if off < GDMA_CH_STRIDE * GDMA_CHANNELS as u32 {
            let ch = (off / GDMA_CH_STRIDE) as usize; let o = off % GDMA_CH_STRIDE;
            if o < 0x60 {
                let r = &mut self.inp[ch];
                match o {
                    0x00 => { r.conf0 = v & !1; if v & 1 != 0 { r.running = false; r.desc = 0; r.buf_pos = 0; r.rx_eof_pos = 0; } }   // IN_RST self-clears
                    0x04 => r.conf1 = v, 0x10 => r.int_ena = v, 0x14 => r.int_raw &= !v,
                    0x20 => {
                        r.link = v & 0xF_FFFF;
                        if v & (1 << 22) != 0 || v & (1 << 23) != 0 { r.desc = self.addr_base | (v & 0xF_FFFF); r.buf_pos = 0; r.rx_eof_pos = 0; r.running = true; }   // START / RESTART
                        if v & (1 << 21) != 0 { r.running = false; }                                                                 // STOP
                    }
                    0x44 => r.pri = v, 0x48 => r.peri_sel = v & 0x3f,
                    _ => self.ram.write(off, v),
                }
                return;
            }
            let c = &mut self.out[ch];
            match o {
                0x60 => { c.conf0 = v & !1; if v & 1 != 0 { c.running = false; c.desc = 0; c.buf_pos = 0; } }   // OUT_RST self-clears
                0x64 => c.conf1 = v, 0x70 => c.int_ena = v, 0x74 => c.int_raw &= !v,
                0x80 => {
                    c.link = v & 0xF_FFFF;
                    if v & (1 << 21) != 0 || v & (1 << 22) != 0 { c.desc = self.addr_base | (v & 0xF_FFFF); c.buf_pos = 0; c.running = true; if c.peri_sel == 5 && self.dbg { eprintln!("[lcd] gdma out link {} at {:#010x}", if v & (1 << 22) != 0 { "RESTART" } else { "START" }, c.desc); } }   // START / RESTART
                    if v & (1 << 20) != 0 { c.running = false; }                                                                                 // STOP
                }
                0xa4 => c.pri = v, 0xa8 => c.peri_sel = v & 0x3f,
                _ => self.ram.write(off, v),
            }
            return;
        }
        match off { 0x3c8 => self.misc = v, _ => self.ram.write(off, v) }
    }
    /// Find the out channel bound to peripheral `peri` (GDMA_TRIG_PERIPH_*).
    pub fn out_channel_for(&self, peri: u32) -> Option<usize> { (0..GDMA_CHANNELS).find(|&i| self.out[i].running && self.out[i].peri_sel == peri) }
    /// An armed receive channel is passive. Each producer added here must also
    /// provide an active-cadence term or a deadline in the owning SoC scheduler.
    pub fn in_channel_for(&self, peri: u32) -> Option<usize> { (0..GDMA_CHANNELS).find(|&i| self.inp[i].running && self.inp[i].peri_sel == peri) }
}

impl Default for Gdma { fn default() -> Self { Self::new() } }

/// One DMA descriptor (dma_descriptor_t): dw0 = size[11:0] length[23:12] suc_eof[30] owner[31]; dw1 = buffer; dw2 = next
pub struct DmaDesc { pub addr: u32, pub size: u32, pub length: u32, pub eof: bool, pub owner_dma: bool, pub buf: u32, pub next: u32 }
pub fn read_desc(mem: &dyn Fn(u32) -> u32, addr: u32) -> DmaDesc {
    let dw0 = mem(addr);
    DmaDesc::decode(addr, dw0, mem(addr + 4), mem(addr + 8))
}
impl DmaDesc {
    fn decode(addr: u32, dw0: u32, buf: u32, next: u32) -> Self {
    Self { addr, size: dw0 & 0xfff, length: (dw0 >> 12) & 0xfff, eof: dw0 & (1 << 30) != 0, owner_dma: dw0 & (1 << 31) != 0, buf, next }
    }
}
impl Device for Gdma {
    fn read(&mut self, off: u32) -> u32 { Gdma::read(self, off) }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { Gdma::write(self, off, v); WriteEffect::NONE }
    /// bits 0..5 = out channels, bits 5..10 = in channels
    fn debug(&mut self, on: bool) { self.dbg = on; }
    fn irq_sources(&self) -> u64 { (0..GDMA_CHANNELS).fold(0, |m, i| m | ((self.out[i].irq() as u64) << i) | ((self.inp[i].irq() as u64) << (GDMA_CHANNELS + i))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OUTFIFO_STATUS reads empty while idle and not empty once OUTLINK_START runs a descriptor.
    #[test]
    fn out_fifo_is_not_empty_while_a_channel_runs() {
        let mut g = Gdma::new();
        assert_ne!(g.read(0x78) & (1 << 1), 0, "idle: empty");
        g.write(0x80, (1 << 21) | 0x1000);                              // OUT_LINK: START
        assert_eq!(g.read(0x78) & (1 << 1), 0, "running: prefetched");
        g.write(0x80, 1 << 20);                                         // OUT_LINK: STOP
        assert_ne!(g.read(0x78) & (1 << 1), 0, "stopped: empty");
    }
}
