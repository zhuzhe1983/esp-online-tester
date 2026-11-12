//! 2D-DMA (DMA2D): four TX ("out") and three RX ("in") channels with 2D-capable descriptors,
//! memory-to-memory and the PPA's data paths. Same IP on the ESP32-P4 (0x5008_8000) and the
//! ESP32-S31 (0x2034_6000).
//!
//! Sources: ESP32-P4 TRM v0.5 chapter "2D Direct Memory Access Controller (2D-DMA)" (descriptor
//! format, 2D block mode, mem-to-mem, peripheral selection); register layout from IDF master
//! components/soc/esp32s31/register/soc/dma2d_{reg,struct}.h, descriptor from
//! components/esp_hal_dma/include/hal/dma2d_types.h (`dma2d_descriptor_align8_t`), peri_sel ids
//! from components/soc/esp32s31/include/soc/dma2d_channel.h, channel reset sequence from
//! components/esp_hal_dma/dma2d_hal.c.
//!
//! Model: a transfer completes synchronously when everything it needs has been started (both
//! channels of an M2M pair; the PPA plus its channels). The FIFOs, burst lengths, arbitration,
//! reorder and macro-block sizes only shape timing and are kept as storage. Colour-space
//! conversion and scrambling in the channels are not modelled: a channel configured for them is
//! refused (nothing moves) rather than copied unconverted.
use crate::device::{Device, WriteEffect};
use crate::ppa::{Ppa, PpaOp};
use crate::sdmmc::SdmmcMemory;
use emu_core::Fault;

pub const DMA2D_OUT_CHANNELS: usize = 4;
pub const DMA2D_IN_CHANNELS: usize = 3;
const IN_BASE: u32 = 0x500;
const STRIDE: u32 = 0x100;
const COMMON: u32 = 0xa00;
/// Descriptors walked per transfer before the chain is treated as looping.
const STEP_BUDGET: usize = 1 << 16;

/// dma2d_channel.h: PPA SRM uses peri_sel 1 on both sides; blending reads background on TX 2,
/// foreground on TX 3, writes on RX 2. M2M uses any of TX 4-7 / RX 3-7 (dma2d_ll.h masks 0xF0/0xF8).
pub const PERI_PPA_SRM: u32 = 1;
pub const PERI_PPA_BLEND_RX: u32 = 2;
pub const PERI_PPA_BLEND_BG_TX: u32 = 2;
pub const PERI_PPA_BLEND_FG_TX: u32 = 3;

// Interrupt bits (dma2d_struct.h out/in_int_raw).
const OUT_DONE: u32 = 1 << 0; const OUT_EOF: u32 = 1 << 1; const OUT_DSCR_ERR: u32 = 1 << 2; const OUT_TOTAL_EOF: u32 = 1 << 3;
const IN_DONE: u32 = 1 << 0; const IN_SUC_EOF: u32 = 1 << 1; const IN_DSCR_ERR: u32 = 1 << 3; const IN_DSCR_EMPTY: u32 = 1 << 10;

#[derive(Clone, Copy)]
pub struct Dma2dCh {
    /// Register words of the channel's 0x100 window (storage for what is not interpreted).
    pub regs: [u32; 64],
    pub running: bool,
    /// Descriptor the channel will fetch next.
    pub desc: u32,
}
impl Dma2dCh {
    fn new(input: bool) -> Self {
        let mut regs = [0; 64];
        // COLOR_CONVERT reset: INPUT_SEL 7 (disabled); OUTPUT_SEL default per direction.
        regs[if input { 0x4c } else { 0x48 } / 4] = 7 << 3;
        Dma2dCh { regs, running: false, desc: 0 }
    }
    fn conf0(&self) -> u32 { self.regs[0] }
    pub fn int_raw(&self) -> u32 { self.regs[1] }
    fn int_ena(&self) -> u32 { self.regs[2] }
    pub fn irq(&self) -> bool { self.int_raw() & self.int_ena() != 0 }
    fn raise(&mut self, bits: u32) { self.regs[1] |= bits; }
}

/// What a service pass refused: register offset (DMA2D block, or PPA block when `ppa`) that asked for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal { pub ppa: bool, pub off: u32 }

pub struct Dma2d {
    pub out: [Dma2dCh; DMA2D_OUT_CHANNELS],
    pub inp: [Dma2dCh; DMA2D_IN_CHANNELS],
    common: [u32; 12],
    pub dbg: bool,
    /// Completed transfers, for tests and the stop report: (m2m, ppa).
    pub transfers: (u64, u64),
}
impl Default for Dma2d { fn default() -> Self { Self::new() } }

/// One `dma2d_descriptor_align8_t` (24 bytes, 8-byte aligned).
#[derive(Clone, Copy, Debug)]
struct Desc { w0: u32, w1: u32, w2: u32, buf: u32, next: u32 }
impl Desc {
    fn read(mem: &mut impl SdmmcMemory, addr: u32) -> Result<Self, Fault> {
        let mut b = [0u8; 20];
        mem.read(addr, &mut b)?;
        let w = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        Ok(Desc { w0: w(0), w1: w(4), w2: w(8), buf: w(12), next: w(16) })
    }
    fn vb(&self) -> usize { (self.w0 & 0x3fff) as usize }
    fn hb(&self) -> usize { (self.w0 >> 14 & 0x3fff) as usize }
    fn d2(&self) -> bool { self.w0 >> 29 & 1 != 0 }
    fn suc_eof(&self) -> bool { self.w0 >> 30 & 1 != 0 }
    fn owner(&self) -> bool { self.w0 >> 31 != 0 }
    fn va(&self) -> usize { (self.w1 & 0x3fff) as usize }
    fn ha(&self) -> usize { (self.w1 >> 14 & 0x3fff) as usize }
    fn pbyte(&self) -> u32 { self.w1 >> 28 }
    fn y(&self) -> usize { (self.w2 & 0x3fff) as usize }
    fn x(&self) -> usize { (self.w2 >> 14 & 0x3fff) as usize }
    fn multiple(&self) -> bool { self.w2 >> 28 & 1 != 0 }
    /// 1D: buffer length (TX bytes to send / RX bytes received) = hb_length | ha_length << 14.
    fn length(&self) -> usize { self.hb() | self.ha() << 14 }
    /// 1D: buffer size (RX capacity) = vb_size | va_size << 14.
    fn size(&self) -> usize { self.vb() | self.va() << 14 }
    /// The rows a 2D descriptor covers: (first byte address, bytes per row, row stride, rows).
    /// Single mode is the hb x vb block at (x, y) of the ha-wide picture. Multiple mode walks
    /// hb x vb blocks until the whole ha x va picture is done; the result in memory is the whole
    /// picture, which this model moves in raster order.
    fn rows(&self) -> Option<(u32, usize, usize, usize)> {
        // pbyte: 0 = 0.5 B/px, 1 = 1, 2 = 1.5, 3 = 2, 4 = 3, 5 = 4 (dma2d_types.h).
        let half = match self.pbyte() { 0 => 1, 1 => 2, 2 => 3, 3 => 4, 4 => 6, 5 => 8, _ => return None };
        let stride = self.ha() * half / 2;
        let (x, y, w, h) = if self.multiple() { (0, 0, self.ha(), self.va()) } else { (self.x(), self.y(), self.hb(), self.vb()) };
        let start = self.buf.wrapping_add(((y * self.ha() + x) * half / 2) as u32);
        Some((start, w * half / 2, stride, h))
    }
}

/// How a TX chain ended.
struct Gathered { data: Vec<u8>, eof_desc: u32, next: u32 }

impl Dma2d {
    pub fn new() -> Self {
        Dma2d { out: [Dma2dCh::new(false); DMA2D_OUT_CHANNELS], inp: [Dma2dCh::new(true); DMA2D_IN_CHANNELS], common: [0; 12], dbg: false, transfers: (0, 0) }
    }
    /// Interrupt sources, bit i = the chip table's order: IN ch0-2 then OUT ch0-3.
    pub fn irq_bits(&self) -> u64 {
        let mut v = 0;
        for (i, c) in self.inp.iter().enumerate() { if c.irq() { v |= 1 << i; } }
        for (i, c) in self.out.iter().enumerate() { if c.irq() { v |= 1 << (DMA2D_IN_CHANNELS + i); } }
        v
    }
    fn channel(&mut self, off: u32) -> Option<(bool, usize, u32)> {
        if off < STRIDE * DMA2D_OUT_CHANNELS as u32 { return Some((false, (off / STRIDE) as usize, off % STRIDE)); }
        if (IN_BASE..IN_BASE + STRIDE * DMA2D_IN_CHANNELS as u32).contains(&off) { return Some((true, ((off - IN_BASE) / STRIDE) as usize, off % STRIDE)); }
        None
    }

    /// Read a TX chain into one byte stream, as the channel would push it into its FIFO.
    fn gather(&mut self, ch: usize, mem: &mut impl SdmmcMemory) -> Result<Gathered, Fault> {
        let c = self.out[ch];
        let (wrback, check_owner) = (c.conf0() & 1 != 0, c.conf0() >> 4 & 1 != 0);
        let mut data = Vec::new();
        let mut addr = c.desc;
        for _ in 0..STEP_BUDGET {
            if addr == 0 { return Err(Fault::Unmapped); }
            let d = Desc::read(mem, addr)?;
            if check_owner && !d.owner() { return Err(Fault::Prohibited); }
            if d.d2() {
                let (start, row, stride, rows) = d.rows().ok_or(Fault::Unmapped)?;
                for r in 0..rows {
                    let at = data.len(); data.resize(at + row, 0);
                    mem.read(start.wrapping_add((r * stride) as u32), &mut data[at..])?;
                }
            } else {
                let at = data.len(); data.resize(at + d.length(), 0);
                mem.read(d.buf, &mut data[at..])?;
            }
            if wrback { mem.write(addr, &(d.w0 & !(1 << 31)).to_le_bytes())?; }
            if self.dbg { eprintln!("[dma2d] out{ch} desc {addr:#010x} w0={:#010x} w1={:#010x} w2={:#010x} buf={:#010x} -> {} bytes", d.w0, d.w1, d.w2, d.buf, data.len()); }
            if d.suc_eof() || d.next == 0 { return Ok(Gathered { data, eof_desc: addr, next: d.next }); }
            addr = d.next;
        }
        Err(Fault::Unmapped)
    }

    /// Write a byte stream down an RX chain. The descriptor holding the last byte gets SUC_EOF
    /// and (1D) the received length; owner is handed back to the CPU on each one used.
    fn scatter(&mut self, ch: usize, data: &[u8], mem: &mut impl SdmmcMemory) -> Result<(), Fault> {
        let mut addr = self.inp[ch].desc;
        let mut pos = 0;
        for _ in 0..STEP_BUDGET {
            if addr == 0 { break; }
            let d = Desc::read(mem, addr)?;
            if self.inp[ch].conf0() >> 4 & 1 != 0 && !d.owner() { return Err(Fault::Prohibited); }
            let mut w0 = d.w0 & !(1 << 31);
            if d.d2() {
                let (start, row, stride, rows) = d.rows().ok_or(Fault::Unmapped)?;
                for r in 0..rows {
                    if pos >= data.len() { break; }
                    let n = row.min(data.len() - pos);
                    mem.write(start.wrapping_add((r * stride) as u32), &data[pos..pos + n])?;
                    pos += n;
                }
                mem.write(addr, &(w0 | if pos >= data.len() { 1 << 30 } else { 0 }).to_le_bytes())?;
            } else {
                let n = d.size().min(data.len() - pos);
                mem.write(d.buf, &data[pos..pos + n])?;
                pos += n;
                w0 = (w0 & !(0x3fff << 14)) | ((n as u32 & 0x3fff) << 14);
                if pos >= data.len() { w0 |= 1 << 30; }
                mem.write(addr, &w0.to_le_bytes())?;
                mem.write(addr + 4, &((d.w1 & !(0x3fff << 14)) | ((n as u32 >> 14 & 0x3fff) << 14)).to_le_bytes())?;
            }
            if self.dbg { eprintln!("[dma2d] in{ch} desc {addr:#010x} w0={:#010x} w1={:#010x} w2={:#010x} buf={:#010x} <- {pos}/{} bytes", d.w0, d.w1, d.w2, d.buf, data.len()); }
            if pos >= data.len() {
                let c = &mut self.inp[ch];
                c.regs[0x28 / 4] = addr; c.desc = d.next; c.running = false;
                c.raise(IN_DONE | IN_SUC_EOF);
                return Ok(());
            }
            addr = d.next;
        }
        // The chain ran out before the data did: descriptor-empty, the rest is lost.
        let c = &mut self.inp[ch];
        c.running = false; c.raise(IN_DSCR_EMPTY);
        Ok(())
    }

    fn finish_out(&mut self, ch: usize, g: &Gathered) {
        let c = &mut self.out[ch];
        c.regs[0x28 / 4] = g.eof_desc; c.desc = g.next; c.running = false;
        c.raise(OUT_DONE | OUT_EOF | OUT_TOTAL_EOF);
    }
    fn out_error(&mut self, ch: usize) { let c = &mut self.out[ch]; c.running = false; c.raise(OUT_DSCR_ERR); }
    fn in_error(&mut self, ch: usize) { let c = &mut self.inp[ch]; c.running = false; c.raise(IN_DSCR_ERR); }

    fn csc_enabled(&self, input: bool, ch: usize) -> bool {
        if input { self.inp[ch].regs[0x4c / 4] >> 3 & 7 != 7 } else { self.out[ch].regs[0x48 / 4] >> 3 & 7 != 7 }
    }
    fn running_out(&self, peri: impl Fn(u32) -> bool) -> Option<usize> {
        (0..DMA2D_OUT_CHANNELS).find(|&i| self.out[i].running && peri(self.out[i].regs[0x38 / 4] & 7))
    }
    fn running_in(&self, m2m: bool, peri: u32) -> Option<usize> {
        (0..DMA2D_IN_CHANNELS).find(|&i| { let c = &self.inp[i]; c.running && (c.conf0() & 1 != 0) == m2m && (m2m || c.regs[0x3c / 4] & 7 == peri) })
    }

    /// Run every transfer whose channels (and PPA engine) are all started. Returns what was refused.
    pub fn service(&mut self, ppa: &mut Ppa, mem: &mut impl SdmmcMemory) -> Vec<Refusal> {
        let mut refused = Vec::new();
        // Memory-to-memory: RX channel with IN_MEM_TRANS_EN takes the stream of a started M2M TX
        // channel (peri_sel 4-7). The pairing the silicon uses for differing ids is not documented
        // in the IDF sources; the same channel number is preferred, otherwise the lowest started one.
        while let Some(rx) = self.running_in(true, 0) {
            let tx = if self.out[rx].running && self.out[rx].regs[0x38 / 4] & 7 >= 4 { Some(rx) } else { self.running_out(|p| p >= 4) };
            let Some(tx) = tx else { break };
            if self.csc_enabled(false, tx) || self.csc_enabled(true, rx) {
                refused.push(Refusal { ppa: false, off: STRIDE * tx as u32 + 0x48 });
                self.out[tx].running = false; self.inp[rx].running = false;
                continue;
            }
            match self.gather(tx, mem) {
                Ok(g) => {
                    self.finish_out(tx, &g);
                    if self.scatter(rx, &g.data, mem).is_err() { self.in_error(rx); }
                    self.transfers.0 += 1;
                }
                Err(_) => { self.out_error(tx); self.inp[rx].running = false; }
            }
        }
        // PPA operations wait for their channels.
        for op in ppa.pending() {
            let rx = self.running_in(false, if op == PpaOp::Srm { PERI_PPA_SRM } else { PERI_PPA_BLEND_RX });
            let need: &[u32] = match op { PpaOp::Fill => &[], PpaOp::Bypass => &[PERI_PPA_BLEND_BG_TX], PpaOp::Blend => &[PERI_PPA_BLEND_BG_TX, PERI_PPA_BLEND_FG_TX], PpaOp::Srm => &[PERI_PPA_SRM] };
            let txs: Vec<Option<usize>> = need.iter().map(|&p| self.running_out(|q| q == p)).collect();
            if let (Some(rx), true) = (rx, txs.iter().all(Option::is_some)) {
                let txs: Vec<usize> = txs.into_iter().flatten().collect();
                let mut inputs = Vec::new();
                let mut ok = true;
                for &tx in &txs {
                    if self.csc_enabled(false, tx) { refused.push(Refusal { ppa: false, off: STRIDE * tx as u32 + 0x48 }); ok = false; break; }
                    match self.gather(tx, mem) {
                        Ok(g) => { self.finish_out(tx, &g); inputs.push(g.data); }
                        Err(_) => { self.out_error(tx); ok = false; break; }
                    }
                }
                if ok && self.csc_enabled(true, rx) { refused.push(Refusal { ppa: false, off: IN_BASE + STRIDE * rx as u32 + 0x4c }); ok = false; }
                if ok {
                    match ppa.run(op, &inputs) {
                        Ok(out) => {
                            if self.scatter(rx, &out, mem).is_err() { self.in_error(rx); }
                            self.transfers.1 += 1;
                        }
                        Err(off) => { refused.push(Refusal { ppa: true, off }); self.inp[rx].running = false; }
                    }
                } else { self.inp[rx].running = false; ppa.cancel(op); }
            }
        }
        refused
    }
}

impl Device for Dma2d {
    fn read(&mut self, off: u32) -> u32 {
        if let Some((input, ch, o)) = self.channel(off) {
            let c = if input { &self.inp[ch] } else { &self.out[ch] };
            return match o {
                0x0c => c.int_raw() & c.int_ena(),
                0x10 => 0,
                // FIFO status: L1/L2/L3 empty (bits 1, 18, 25), nothing in flight in this model.
                0x14 => 1 << 1 | 1 << 18 | 1 << 25,
                // LINK_CONF: start/restart self-clear, PARK reads 1 while the channel is idle.
                0x1c => { let park = if input { 1 << 24 } else { 1 << 23 }; let pulse = if input { 3 << 22 } else { 3 << 21 }; (c.regs[7] & !pulse & !park) | if c.running { 0 } else { park } }
                // STATE: descriptor address low 18 bits, state, RESET_AVAIL (out bit 24, in bit 23):
                // transfers complete synchronously, so a channel is always safe to reset.
                0x24 => (c.desc & 0x3ffff) | if input { 1 << 23 } else { 1 << 24 },
                0x2c if !input => c.desc, 0x30 if input => c.desc,
                _ => c.regs[(o / 4) as usize],
            };
        }
        if (COMMON..COMMON + 0x30).contains(&off) {
            // DATE (IDF master dma2d_reg.h reset value).
            if off == 0xa2c { return self.common[11].max(0x2306_0100); }
            return self.common[((off - COMMON) / 4) as usize];
        }
        0
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if let Some((input, ch, o)) = self.channel(off) {
            let dbg = self.dbg;
            let c = if input { &mut self.inp[ch] } else { &mut self.out[ch] };
            if dbg { eprintln!("[dma2d] {}{ch} +{o:#04x} <- {v:#010x}", if input { "in" } else { "out" }); }
            match o {
                0x00 => {
                    c.regs[0] = v;
                    // RST (bit 24) resets the channel's state machine and FIFO pointers;
                    // CMD_DISABLE (bit 25) aborts the running transfer (dma2d_hal.c reset sequence).
                    if v & (1 << 24 | 1 << 25) != 0 { c.running = false; }
                }
                0x04 | 0x0c | 0x14 | 0x24 => {}
                0x10 => c.regs[1] &= !v,
                0x1c => {
                    let (stop, start, restart) = if input { (21, 22, 23) } else { (20, 21, 22) };
                    c.regs[7] = v;
                    if v >> stop & 1 != 0 { c.running = false; }
                    if v >> start & 1 != 0 { c.desc = c.regs[8]; c.running = true; }
                    if v >> restart & 1 != 0 { c.running = true; }
                }
                _ => c.regs[(o / 4) as usize] = v,
            }
            return WriteEffect::NONE;
        }
        if (COMMON..COMMON + 0x2c).contains(&off) { self.common[((off - COMMON) / 4) as usize] = v; }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { self.irq_bits() }
    fn debug(&mut self, on: bool) { self.dbg = on; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ppa::Ppa;
    struct Ram(Vec<u8>);
    impl SdmmcMemory for Ram {
        fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Fault> { let a = addr as usize; buf.copy_from_slice(self.0.get(a..a + buf.len()).ok_or(Fault::Unmapped)?); Ok(()) }
        fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Fault> { let a = addr as usize; self.0.get_mut(a..a + data.len()).ok_or(Fault::Unmapped)?.copy_from_slice(data); Ok(()) }
    }
    fn desc(m: &mut Ram, at: u32, w0: u32, w1: u32, w2: u32, buf: u32, next: u32) {
        for (i, w) in [w0, w1, w2, buf, next].into_iter().enumerate() { m.write(at + 4 * i as u32, &w.to_le_bytes()).unwrap(); }
    }
    fn d2(vb: u32, hb: u32, va: u32, ha: u32, pbyte: u32, x: u32, y: u32) -> (u32, u32, u32) {
        (vb | hb << 14 | 1 << 29 | 1 << 30 | 1 << 31, va | ha << 14 | pbyte << 28, y | x << 14)
    }
    fn start(d: &mut Dma2d, input: bool, ch: u32, desc: u32, peri: u32, m2m: bool) {
        let base = if input { IN_BASE } else { 0 } + STRIDE * ch;
        if input && m2m { d.write(base, 1); }
        d.write(base + 0x08, 0x3);
        d.write(base + if input { 0x3c } else { 0x38 }, peri);
        d.write(base + 0x20, desc);
        d.write(base + 0x1c, 1 << if input { 22 } else { 21 });
    }

    #[test]
    fn reset_sequence_sees_reset_avail() {
        let mut d = Dma2d::new();
        d.write(0x100, 1 << 25);
        assert_ne!(d.read(0x124) & (1 << 24), 0);
        d.write(0x100, 1 << 24 | 1 << 25); d.write(0x100, 1 << 25); d.write(0x100, 0);
        assert_ne!(d.read(0x624) & (1 << 23), 0);
    }

    #[test]
    fn m2m_1d_copy_sets_eof_and_length() {
        let mut m = Ram(vec![0; 0x4000]);
        for i in 0..100u32 { m.0[0x1000 + i as usize] = i as u8 ^ 0x5a; }
        desc(&mut m, 0x100, 100 << 14 | 1 << 30 | 1 << 31, 0, 0, 0x1000, 0);
        desc(&mut m, 0x200, 128 | 1 << 31, 0, 0, 0x2000, 0);
        let (mut d, mut p) = (Dma2d::new(), Ppa::new());
        start(&mut d, false, 1, 0x100, 4, false);
        assert!(d.service(&mut p, &mut m).is_empty());
        assert!(d.out[1].running, "TX waits for its RX partner");
        start(&mut d, true, 1, 0x200, 3, true);
        assert!(d.service(&mut p, &mut m).is_empty());
        assert_eq!(&m.0[0x2000..0x2064], &m.0[0x1000..0x1064].to_vec()[..]);
        assert_eq!(m.0[0x2064], 0);
        assert_eq!(d.out[1].int_raw() & OUT_EOF, OUT_EOF);
        assert_eq!(d.inp[1].int_raw() & IN_SUC_EOF, IN_SUC_EOF);
        assert_eq!(d.read(0x628), 0x200);
        let w0 = u32::from_le_bytes(m.0[0x200..0x204].try_into().unwrap());
        assert_eq!((w0 >> 14 & 0x3fff, w0 >> 30), (100, 1), "received length and SUC_EOF, owner back to CPU");
        assert_eq!(d.irq_bits(), 1 << 1 | 1 << 4);
        d.write(0x110, 0xff); d.write(0x610, 0xff); assert_eq!(d.irq_bits(), 0);
    }

    #[test]
    fn m2m_2d_block_copies_a_window_between_pictures() {
        // 4x3 RGB565 block at (2,1) of an 8x4 source into (1,2) of a 6x6 destination.
        let mut m = Ram(vec![0; 0x4000]);
        for i in 0..64u32 { m.0[0x1000 + i as usize] = i as u8; }
        let (w0, w1, w2) = d2(3, 4, 4, 8, 3, 2, 1); desc(&mut m, 0x100, w0, w1, w2, 0x1000, 0);
        let (w0, w1, w2) = d2(3, 4, 6, 6, 3, 1, 2); desc(&mut m, 0x200, w0, w1, w2, 0x2000, 0);
        let (mut d, mut p) = (Dma2d::new(), Ppa::new());
        start(&mut d, false, 0, 0x100, 4, false); start(&mut d, true, 0, 0x200, 3, true);
        d.service(&mut p, &mut m);
        for r in 0..3 {
            let src = 0x1000 + ((1 + r) * 8 + 2) * 2; let dst = 0x2000 + ((2 + r) * 6 + 1) * 2;
            let want = m.0[src..src + 8].to_vec(); assert_eq!(&m.0[dst..dst + 8], &want[..]);
            assert_eq!(m.0[dst - 1], 0); assert_eq!(m.0[dst + 8], 0);
        }
    }

    #[test]
    fn ppa_fill_writes_the_block_through_rx_channel() {
        let mut m = Ram(vec![0; 0x4000]);
        // 2x2 RGB565 block at (1,1) of a 4x3 picture.
        let (w0, w1, w2) = d2(2, 2, 3, 4, 3, 1, 1); desc(&mut m, 0x200, w0, w1, w2, 0x2000, 0);
        let (mut d, mut p) = (Dma2d::new(), Ppa::new());
        start(&mut d, true, 2, 0x200, PERI_PPA_BLEND_RX, false);
        p.write(0x24, 2 << 8); p.write(0x3c, 2 | 2 << 14); p.write(0x4c, 0xf800);
        p.write(0x34, 1 | 1 << 2 | 1 << 3);
        assert!(d.service(&mut p, &mut m).is_empty());
        let px = |x: usize, y: usize| u16::from_le_bytes([m.0[0x2000 + (y * 4 + x) * 2], m.0[0x2001 + (y * 4 + x) * 2]]);
        assert_eq!([px(1, 1), px(2, 1), px(1, 2), px(2, 2)], [0xf800; 4]);
        assert_eq!([px(0, 1), px(3, 1), px(1, 0)], [0; 3]);
        assert_eq!(p.read(0x10) & 2, 2);
        assert_eq!(d.inp[2].int_raw() & IN_SUC_EOF, IN_SUC_EOF);
    }

    #[test]
    fn ppa_blend_argb_over_rgb565_into_rgb565() {
        let mut m = Ram(vec![0; 0x4000]);
        // bg: 2 RGB565 px (pure blue, pure red); fg: ARGB8888 white @ alpha 0x80, then alpha 0.
        m.0[0x1000..0x1004].copy_from_slice(&[0x1f, 0x00, 0x00, 0xf8]);
        m.0[0x1100..0x1108].copy_from_slice(&[0xff, 0xff, 0xff, 0x80, 0xff, 0xff, 0xff, 0x00]);
        let (w0, w1, w2) = d2(1, 2, 1, 2, 3, 0, 0); desc(&mut m, 0x100, w0, w1, w2, 0x1000, 0);
        let (w0, w1, w2) = d2(1, 2, 1, 2, 5, 0, 0); desc(&mut m, 0x140, w0, w1, w2, 0x1100, 0);
        let (w0, w1, w2) = d2(1, 2, 1, 2, 3, 0, 0); desc(&mut m, 0x200, w0, w1, w2, 0x2000, 0);
        let (mut d, mut p) = (Dma2d::new(), Ppa::new());
        start(&mut d, false, 0, 0x100, PERI_PPA_BLEND_BG_TX, false);
        start(&mut d, false, 1, 0x140, PERI_PPA_BLEND_FG_TX, false);
        start(&mut d, true, 0, 0x200, PERI_PPA_BLEND_RX, false);
        p.write(0x3c, 2 | 1 << 14); p.write(0x24, 2 | 2 << 8); // bg RGB565, fg ARGB8888 (0), out RGB565
        p.write(0x50, 0xffffff); p.write(0x54, 0); p.write(0x58, 0xffffff); p.write(0x5c, 0);
        p.write(0x34, 1 | 1 << 3);
        assert!(d.service(&mut p, &mut m).is_empty());
        let px = |i: usize| u16::from_le_bytes([m.0[0x2000 + 2 * i], m.0[0x2001 + 2 * i]]);
        // 0x80/255 white over blue: r = g = 128, b = 255 -> RGB565 (16, 32, 31).
        assert_eq!(px(0), 16 << 11 | 32 << 5 | 31);
        assert_eq!(px(1), 0xf800, "transparent foreground leaves the background");
        assert_eq!(d.transfers.1, 1);
    }

    #[test]
    fn csc_on_a_channel_is_refused_not_copied() {
        let mut m = Ram(vec![0; 0x4000]);
        m.0[0x1000] = 9;
        desc(&mut m, 0x100, 4 << 14 | 1 << 30 | 1 << 31, 0, 0, 0x1000, 0);
        desc(&mut m, 0x200, 4 | 1 << 31, 0, 0, 0x2000, 0);
        let (mut d, mut p) = (Dma2d::new(), Ppa::new());
        d.write(0x48, 0); // input_sel 0: RGB565 -> RGB888
        start(&mut d, false, 0, 0x100, 4, false); start(&mut d, true, 0, 0x200, 3, true);
        assert_eq!(d.service(&mut p, &mut m), vec![Refusal { ppa: false, off: 0x48 }]);
        assert_eq!(m.0[0x2000], 0);
    }
}
