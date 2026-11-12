//! The AHB GDMA register layout of the C3, the C6 and the P4's AHB-DMA, translated onto the
//! shared (S3) channel model. Each channel block starts at 0x70 + 0xC0 * n and orders its
//! registers conf0, conf1, fifo status, pop/push, link, state, suc_eof, err_eof, dscr, bf0,
//! bf1, pri, peri_sel (IN), with OUT the same at +0x60. The interrupt registers sit in front
//! of the channel blocks and are where the chips differ.
use super::Gdma;
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AhbLayout {
    /// One interrupt block per channel at 0x10 * n with IN and OUT bits interleaved (`IN`/`OUT`
    /// below), MISC_CONF at 0x44 (IDF v5.5.4 soc/esp32c3/register/soc/gdma_reg.h)
    C3,
    /// IN interrupts at 0x10 * n, OUT at 0x30 + 0x10 * n, each in the shared bit order,
    /// MISC_CONF at 0x64 (soc/esp32c6/register/soc/gdma_reg.h; the P4 AHB-DMA the same)
    C6,
}

/// The shared model behind one of the AHB layouts. Registers the model does not hold read
/// back what was written.
pub struct GdmaAhb { pub gdma: Gdma, layout: AhbLayout, ram: RegRam }

/// The C3's interrupt bit of each shared IN and OUT interrupt bit
const IN: [u32; 7] = [0, 1, 2, 5, 7, 9, 10];
const OUT: [u32; 6] = [3, 4, 6, 8, 11, 12];
fn pack(value: u32, bits: &[u32]) -> u32 {
    bits.iter().enumerate().fold(0, |out, (i, bit)| out | (((value >> i) & 1) << bit))
}
fn unpack(value: u32, bits: &[u32]) -> u32 {
    bits.iter().enumerate().fold(0, |out, (i, bit)| out | (((value >> bit) & 1) << i))
}

impl GdmaAhb {
    /// `addr_base` supplies the high bits of descriptor addresses the LINK registers carry 20 of.
    pub fn new(layout: AhbLayout, addr_base: u32) -> Self {
        let mut gdma = Gdma::new();
        gdma.addr_base = addr_base;
        GdmaAhb { gdma, layout, ram: RegRam::new() }
    }
    /// The shared model's offset of a channel-block register (three channels).
    pub fn channel(off: u32) -> Option<u32> {
        // the shared (S3) layout per channel: IN conf0 0x00, conf1 0x04, [int 0x08..0x14], fifo 0x18, pop 0x1c,
        // link 0x20, state 0x24, suc_eof 0x28, err_eof 0x2c, dscr 0x30, bf0 0x34, bf1 0x38, pri 0x44, peri_sel 0x48;
        // OUT the same at +0x60
        const T: [u32; 13] = [0x00, 0x04, 0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x34, 0x38, 0x44, 0x48];
        if !(0x70..0x70 + 3 * 0xc0).contains(&off) { return None; }
        let rel = off - 0x70; let (n, k) = (rel / 0xc0, rel % 0xc0);
        if k <= 0x30 { return Some(n * 0xc0 + T[(k / 4) as usize]); }
        if (0x60..=0x90).contains(&k) { return Some(n * 0xc0 + 0x60 + T[((k - 0x60) / 4) as usize]); }
        None
    }
    /// The shared model's offset of a C6-layout register (the P4 AHB-DMA's too); None for the
    /// registers the model does not hold.
    pub fn map_c6(off: u32) -> Option<u32> {
        if off < 0x30 { return Some((off / 0x10) * 0xc0 + 0x08 + off % 0x10); }
        if off < 0x60 { let o = off - 0x30; return Some((o / 0x10) * 0xc0 + 0x68 + o % 0x10); }
        if off == 0x64 { return Some(0x3c8); }
        Self::channel(off)
    }
    /// The C3's interrupt block is handled by `read`/`write` before this.
    fn map(&self, off: u32) -> Option<u32> {
        match self.layout {
            AhbLayout::C3 => if off == 0x44 { Some(0x3c8) } else { Self::channel(off) },
            AhbLayout::C6 => Self::map_c6(off),
        }
    }
}

impl Device for GdmaAhb {
    fn read(&mut self, off: u32) -> u32 {
        if self.layout == AhbLayout::C3 && off < 0x30 {
            let (inp, out) = (&self.gdma.inp[(off / 0x10) as usize], &self.gdma.out[(off / 0x10) as usize]);
            let raw = pack(inp.int_raw, &IN) | pack(out.int_raw, &OUT);
            let ena = pack(inp.int_ena, &IN) | pack(out.int_ena, &OUT);
            return match off % 0x10 { 0 => raw, 4 => raw & ena, 8 => ena, _ => 0 };
        }
        match self.map(off) { Some(o) => self.gdma.read(o), None => self.ram.read(off) }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if self.layout == AhbLayout::C3 && off < 0x30 {
            let n = (off / 0x10) as usize;
            let (input, output) = (unpack(v, &IN), unpack(v, &OUT));
            let g = &mut self.gdma;
            match off % 0x10 {
                8 => { g.inp[n].int_ena = input; g.out[n].int_ena = output; }
                0 | 12 => { g.inp[n].int_raw &= !input; g.out[n].int_raw &= !output; }
                _ => {}
            }
            return WriteEffect::NONE;
        }
        match self.map(off) { Some(o) => self.gdma.write(o, v), None => self.ram.write(off, v) }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { Device::irq_sources(&self.gdma) }
    fn debug(&mut self, on: bool) { self.gdma.dbg = on; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c3_packs_interrupts_and_c6_splits_them() {
        let mut c3 = GdmaAhb::new(AhbLayout::C3, 0x3fc0_0000);
        c3.write(0x18, (1 << 1) | (1 << 4));                   // INT_ENA_CH1: IN_SUC_EOF, OUT_EOF
        assert_eq!((c3.gdma.inp[1].int_ena, c3.gdma.out[1].int_ena), (1 << 1, 1 << 1));
        c3.gdma.out[1].int_raw = 1 << 1;
        assert_eq!((c3.read(0x10), c3.read(0x14)), (1 << 4, 1 << 4));
        c3.write(0x1c, 1 << 4);                                 // INT_CLR_CH1
        assert_eq!(c3.gdma.out[1].int_raw, 0);
        c3.write(0x44, 7);                                      // MISC_CONF
        assert_eq!(c3.gdma.misc, 7);

        let mut c6 = GdmaAhb::new(AhbLayout::C6, 0x4080_0000);
        c6.write(0x18, 1 << 1);                                 // IN_INT_ENA_CH1
        c6.write(0x48, 1 << 1);                                 // OUT_INT_ENA_CH1
        assert_eq!((c6.gdma.inp[1].int_ena, c6.gdma.out[1].int_ena), (1 << 1, 1 << 1));
        c6.write(0x64, 7);                                      // MISC_CONF
        assert_eq!(c6.gdma.misc, 7);
        // OUT_LINK_CH2 START: the descriptor address takes the layout's high bits
        c6.write(0x70 + 2 * 0xc0 + 0x70, (1 << 21) | 0x1_2340);
        assert_eq!((c6.gdma.out[2].running, c6.gdma.out[2].desc), (true, 0x4081_2340));
    }

    /// A register outside the model reads back on its own: before the shared adapter the C3
    /// kept it in the model's spare storage, where it aliased IN_DSCR_BF0_CH0.
    #[test]
    fn unmapped_registers_do_not_alias_channel_registers() {
        for layout in [AhbLayout::C3, AhbLayout::C6] {
            let mut g = GdmaAhb::new(layout, 0);
            g.write(0x34, 0xdead_beef);
            g.write(0x6c, 0x1234_5678);
            assert_eq!(g.read(0x94), 0, "IN_DSCR_BF0_CH0");
            assert_eq!(g.read(0x6c), 0x1234_5678);
            if layout == AhbLayout::C3 { assert_eq!(g.read(0x34), 0xdead_beef); }
        }
    }
}
