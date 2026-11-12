//! PPA (pixel processing accelerator): the blending engine (blend two layers, fill, bypass) fed
//! by 2D-DMA. Same IP on the ESP32-P4 (0x5008_7000) and the ESP32-S31 (0x2034_5000).
//!
//! Sources: ESP32-P4 TRM v0.5 chapter "Pixel-Processing Accelerator (PPA)" (blending formula,
//! fill, colour keying, alpha update modes); register layout from IDF master
//! components/soc/esp32s31/register/soc/ppa_{reg,struct}.h; field encodings (colour modes,
//! alpha modes, trans-mode sequence) from components/esp_hal_ppa/esp32s31/include/hal/ppa_ll.h and
//! the drivers components/esp_driver_ppa/src/ppa_{blend,fill}.c.
//!
//! Modelled: blend / fill / bypass with ARGB8888, RGB888, RGB565, L8/L4 (CLUT), A8/A4 (fg),
//! GRAY8 inputs and ARGB8888, RGB888, RGB565, GRAY8 outputs; byte and R/B swap; alpha fix /
//! scale / invert; colour keying. Not modelled (refused, nothing written): YUV formats and the
//! scale-rotate-mirror (SRM) engine.
use crate::device::{Device, WriteEffect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PpaOp { Fill, Blend, Bypass, Srm }

const REG_WORDS: usize = 0x104 / 4;
const CLUT_ENTRIES: usize = 256;

pub struct Ppa {
    regs: [u32; REG_WORDS],
    clut: [[u32; CLUT_ENTRIES]; 2],
    clut_ptr: [usize; 2],
    blend: Option<PpaOp>,
    srm: bool,
    pub dbg: bool,
}
impl Default for Ppa { fn default() -> Self { Self::new() } }

/// A pixel as the blender sees it: alpha, red, green, blue (8 bits each).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Argb { pub a: u8, pub r: u8, pub g: u8, pub b: u8 }

fn expand5(v: u16) -> u8 { ((v << 3) | (v >> 2)) as u8 }
fn expand6(v: u16) -> u8 { ((v << 2) | (v >> 4)) as u8 }

/// Blend colour-mode field values (ppa_ll.h `ppa_ll_blend_set_*_color_mode`).
const CM_ARGB8888: u32 = 0; const CM_RGB888: u32 = 1; const CM_RGB565: u32 = 2; const CM_L8: u32 = 4; const CM_L4: u32 = 5;
const CM_A8: u32 = 6; const CM_A4: u32 = 7; const CM_GRAY8: u32 = 12;

/// Bits per pixel of a blend colour mode, or None for what is not modelled (YUV).
fn bits(cm: u32) -> Option<usize> {
    match cm { CM_ARGB8888 => Some(32), CM_RGB888 => Some(24), CM_RGB565 => Some(16), CM_L8 | CM_A8 | CM_GRAY8 => Some(8), CM_L4 | CM_A4 => Some(4), _ => None }
}

/// Decode `n` pixels of colour mode `cm` from a DMA stream (missing bytes read as zero).
/// Memory order is little-endian: ARGB8888 = B,G,R,A; RGB888 = B,G,R; RGB565 = one LE u16.
/// 4-bit modes: the first pixel is the low nibble (assumption, see the module notes).
pub fn decode(cm: u32, data: &[u8], n: usize, clut: &[u32; CLUT_ENTRIES], fix_rgb: u32, byte_swap: bool, rgb_swap: bool) -> Option<Vec<Argb>> {
    let byte = |i: usize| data.get(i).copied().unwrap_or(0);
    let nib = |i: usize| (byte(i / 2) >> (i % 2 * 4)) & 15;
    let from_word = |w: u32| Argb { a: (w >> 24) as u8, r: (w >> 16) as u8, g: (w >> 8) as u8, b: w as u8 };
    let fixed = |a: u8| Argb { a, r: (fix_rgb >> 16) as u8, g: (fix_rgb >> 8) as u8, b: fix_rgb as u8 };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut p = match cm {
            CM_ARGB8888 => {
                let mut b = [byte(4 * i), byte(4 * i + 1), byte(4 * i + 2), byte(4 * i + 3)];
                if byte_swap { b.reverse(); }
                Argb { a: b[3], r: b[2], g: b[1], b: b[0] }
            }
            CM_RGB888 => Argb { a: 255, r: byte(3 * i + 2), g: byte(3 * i + 1), b: byte(3 * i) },
            CM_RGB565 => {
                let mut v = u16::from_le_bytes([byte(2 * i), byte(2 * i + 1)]);
                if byte_swap { v = v.swap_bytes(); }
                Argb { a: 255, r: expand5(v >> 11), g: expand6(v >> 5 & 63), b: expand5(v & 31) }
            }
            CM_L8 => from_word(clut[byte(i) as usize]),
            CM_L4 => from_word(clut[nib(i) as usize]),
            CM_A8 => fixed(byte(i)),
            CM_A4 => fixed(nib(i) * 17),
            CM_GRAY8 => { let v = byte(i); Argb { a: 255, r: v, g: v, b: v } }
            _ => return None,
        };
        if rgb_swap { std::mem::swap(&mut p.r, &mut p.b); }
        out.push(p);
    }
    Some(out)
}

/// Encode pixels into output colour mode `cm` (ARGB8888, RGB888, RGB565, GRAY8).
pub fn encode(cm: u32, px: &[Argb], gray: (u32, u32, u32)) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(px.len() * 4);
    for p in px {
        match cm {
            CM_ARGB8888 => out.extend_from_slice(&[p.b, p.g, p.r, p.a]),
            CM_RGB888 => out.extend_from_slice(&[p.b, p.g, p.r]),
            CM_RGB565 => out.extend_from_slice(&((u16::from(p.r) >> 3) << 11 | (u16::from(p.g) >> 2) << 5 | u16::from(p.b) >> 3).to_le_bytes()),
            // RGB2GRAY coefficients sum to 256 (ppa_ll_set_rgb2gray_coeff).
            CM_GRAY8 => out.push(((u32::from(p.r) * gray.0 + u32::from(p.g) * gray.1 + u32::from(p.b) * gray.2) >> 8).min(255) as u8),
            _ => return None,
        }
    }
    Some(out)
}

/// Alpha update (ppa_ll_blend_configure_rx_*_alpha): mode 1 replaces alpha with the fixed
/// value, mode 2 scales it by value/256; INV inverts it.
fn update_alpha(a: u8, mode: u32, fix: u32, inv: bool) -> u8 {
    let a = match mode { 1 => fix as u8, 2 => ((u32::from(a) * fix) >> 8) as u8, _ => a };
    if inv { 255 - a } else { a }
}

/// Porter-Duff "source over" as in the P4 TRM blending section:
/// A = Af + Ab(1 - Af), C = (Cf Af + Cb Ab (1 - Af)) / A, with alphas normalised to 255.
pub fn over(fg: Argb, bg: Argb) -> Argb {
    let (af, ab) = (u32::from(fg.a), u32::from(bg.a));
    let wb = ab * (255 - af); // background weight * 255
    let a255 = af * 255 + wb;  // output alpha * 255
    if a255 == 0 { return Argb::default(); }
    let c = |f: u8, b: u8| ((u32::from(f) * af * 255 + u32::from(b) * wb + a255 / 2) / a255) as u8;
    Argb { a: ((a255 + 127) / 255) as u8, r: c(fg.r, bg.r), g: c(fg.g, bg.g), b: c(fg.b, bg.b) }
}

impl Ppa {
    pub fn new() -> Self {
        let mut regs = [0; REG_WORDS];
        regs[0x98 / 4] = 77 << 16 | 150 << 8 | 29; // RGB2GRAY reset (ppa_reg.h defaults)
        regs[0x100 / 4] = 0x2405_0300;              // DATE
        Ppa { regs, clut: [[0; CLUT_ENTRIES]; 2], clut_ptr: [0; 2], blend: None, srm: false, dbg: false }
    }
    fn reg(&self, off: u32) -> u32 { self.regs[(off / 4) as usize] }
    pub fn irq(&self) -> bool { self.reg(0x10) & self.reg(0x18) != 0 }
    /// Started operations, waiting for their 2D-DMA channels.
    pub fn pending(&self) -> Vec<PpaOp> { self.blend.into_iter().chain(self.srm.then_some(PpaOp::Srm)).collect() }
    pub fn cancel(&mut self, op: PpaOp) { if op == PpaOp::Srm { self.srm = false; } else { self.blend = None; } }

    /// Produce the RX stream of `op` from its TX streams (blend: [bg, fg]; bypass: [bg]; fill: []).
    /// On success the engine's EOF interrupt is raised; Err is the register that asked for what is not modelled.
    pub fn run(&mut self, op: PpaOp, inputs: &[Vec<u8>]) -> Result<Vec<u8>, u32> {
        self.cancel(op);
        if op == PpaOp::Srm { return Err(0x64); }
        let size = self.reg(0x3c);
        let n = (size & 0x3fff) as usize * (size >> 14 & 0x3fff) as usize;
        let cm = self.reg(0x24);
        let (bg_cm, fg_cm, tx_cm) = (cm & 15, cm >> 4 & 15, cm >> 8 & 15);
        let g = self.reg(0x98);
        let gray = (g >> 16 & 0xff, g >> 8 & 0xff, g & 0xff);
        if bits(tx_cm).is_none() || tx_cm == CM_L8 || tx_cm == CM_L4 || tx_cm == CM_A8 || tx_cm == CM_A4 { return Err(0x24); }
        let out = if op == PpaOp::Fill {
            // Fill: BLEND_FIX_PIXEL in the output format, repeated (ppa_ll_blend_configure_filling_block_color).
            let v = self.reg(0x4c).to_le_bytes();
            let per = bits(tx_cm).unwrap_or(8) / 8;
            let mut out = Vec::with_capacity(n * per);
            for _ in 0..n { out.extend_from_slice(&v[..per]); }
            out
        } else {
            let order = self.reg(0x2c);
            let alpha = self.reg(0x40);
            let bg = decode(bg_cm, inputs.first().map_or(&[][..], |v| v), n, &self.clut[0], 0, order & 1 != 0, order & 4 != 0).ok_or(0x24u32)?;
            let bg: Vec<Argb> = bg.into_iter().map(|p| Argb { a: update_alpha(p.a, alpha >> 16 & 3, alpha & 0xff, alpha >> 20 & 1 != 0), ..p }).collect();
            if op == PpaOp::Bypass { encode(tx_cm, &bg, gray).ok_or(0x24u32)? } else {
                if matches!(bg_cm, CM_A8 | CM_A4) { return Err(0x24); }
                let fg = decode(fg_cm, inputs.get(1).map_or(&[][..], |v| v), n, &self.clut[1], self.reg(0x48), order & 2 != 0, order & 8 != 0).ok_or(0x24u32)?;
                let in_range = |p: Argb, lo: u32, hi: u32| {
                    let ch = |s: u32, v: u8| (lo >> s & 0xff) as u8 <= v && v <= (hi >> s & 0xff) as u8;
                    ch(16, p.r) && ch(8, p.g) && ch(0, p.b)
                };
                let (fg_lo, fg_hi, bg_lo, bg_hi, def) = (self.reg(0x50), self.reg(0x54), self.reg(0x58), self.reg(0x5c), self.reg(0x60));
                let px: Vec<Argb> = fg.into_iter().zip(bg).map(|(f, b)| {
                    let f = Argb { a: update_alpha(f.a, alpha >> 18 & 3, alpha >> 8 & 0xff, alpha >> 21 & 1 != 0), ..f };
                    // Colour keying (TRM PPA "Color key"): a keyed layer is transparent; both keyed
                    // gives the default colour, or the foreground when FG_BG_REVERSE is set.
                    match (in_range(f, fg_lo, fg_hi), in_range(b, bg_lo, bg_hi)) {
                        (true, true) if def >> 24 & 1 != 0 => f,
                        (true, true) => Argb { a: 255, r: (def >> 16) as u8, g: (def >> 8) as u8, b: def as u8 },
                        (true, false) => b,
                        (false, true) => over(f, Argb { a: 0, ..b }),
                        (false, false) => over(f, b),
                    }
                }).collect();
                encode(tx_cm, &px, gray).ok_or(0x24u32)?
            }
        };
        self.regs[0x10 / 4] |= 2; // BLEND_EOF
        if self.dbg { eprintln!("[ppa] {op:?} {}x{} cm={cm:#x} -> {} bytes", size & 0x3fff, size >> 14 & 0x3fff, out.len()); }
        Ok(out)
    }
}

impl Device for Ppa {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x00 | 0x04 => { let t = (off / 4) as usize; let v = self.clut[t][self.clut_ptr[t] % CLUT_ENTRIES]; self.clut_ptr[t] += 1; v }
            0x14 => self.reg(0x10) & self.reg(0x18),
            0x1c => 0,
            // TRANS_MODE_UPDATE and BLEND_RST, SCAL_ROTATE_START and RST are self-clearing.
            0x34 => self.reg(0x34) & !(1 << 3 | 1 << 4),
            0x64 => self.reg(0x64) & !(3 << 26),
            0x400..=0x7ff => self.clut[0][((off - 0x400) / 4) as usize],
            0x800..=0xbff => self.clut[1][((off - 0x800) / 4) as usize],
            o if (o as usize) < REG_WORDS * 4 => self.reg(o),
            _ => 0,
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        if self.dbg { eprintln!("[ppa] +{off:#05x} <- {v:#010x}"); }
        match off {
            0x00 | 0x04 => { let t = (off / 4) as usize; self.clut[t][self.clut_ptr[t] % CLUT_ENTRIES] = v; self.clut_ptr[t] += 1; }
            0x0c => {
                self.regs[3] = v & !(0xf << 1);
                if v & (1 << 1 | 1 << 3) != 0 { self.clut_ptr[0] = 0; }
                if v & (1 << 2 | 1 << 4) != 0 { self.clut_ptr[1] = 0; }
            }
            0x10 | 0x14 | 0x70 | 0x74 | 0x78 | 0x7c | 0x100 => {}
            0x1c => self.regs[4] &= !v,
            0x34 => {
                self.regs[0x34 / 4] = v;
                if v & (1 << 4) != 0 { self.blend = None; }
                // ppa_ll_blend_start: BLEND_EN with TRANS_MODE_UPDATE latches bypass / fill / blend.
                else if v & 1 != 0 && v & (1 << 3) != 0 {
                    self.blend = Some(if v & 4 != 0 { PpaOp::Fill } else if v & 2 != 0 { PpaOp::Bypass } else { PpaOp::Blend });
                }
            }
            0x64 => {
                self.regs[0x64 / 4] = v;
                if v & (1 << 26) != 0 { self.srm = false; } else if v & (1 << 27) != 0 { self.srm = true; }
            }
            0x400..=0x7ff => self.clut[0][((off - 0x400) / 4) as usize] = v,
            0x800..=0xbff => self.clut[1][((off - 0x800) / 4) as usize] = v,
            o if (o as usize) < REG_WORDS * 4 => self.regs[(o / 4) as usize] = v,
            _ => {}
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { u64::from(self.irq()) }
    fn debug(&mut self, on: bool) { self.dbg = on; }
}

#[cfg(test)]
mod tests {
    use super::*;
    const W: Argb = Argb { a: 255, r: 255, g: 255, b: 255 };
    #[test]
    fn over_matches_porter_duff_at_the_corners() {
        let blue = Argb { a: 255, r: 0, g: 0, b: 255 };
        assert_eq!(over(Argb { a: 0, ..W }, blue), blue);
        assert_eq!(over(W, blue), W);
        assert_eq!(over(Argb { a: 128, ..W }, blue), Argb { a: 255, r: 128, g: 128, b: 255 });
        assert_eq!(over(Argb { a: 128, ..W }, Argb { a: 0, ..blue }), Argb { a: 128, ..W });
        assert_eq!(over(Argb::default(), Argb::default()), Argb::default());
    }
    #[test]
    fn decode_encode_formats() {
        let clut = [0u32; CLUT_ENTRIES];
        let p = decode(CM_RGB565, &[0xf8, 0x1f], 1, &clut, 0, true, false).unwrap();
        assert_eq!(p[0], Argb { a: 255, r: 0xff, g: 0, b: 0xff }, "byte swap: 0xf81f");
        let p = decode(CM_ARGB8888, &[1, 2, 3, 4], 1, &clut, 0, false, true).unwrap();
        assert_eq!(p[0], Argb { a: 4, r: 1, g: 2, b: 3 }, "R/B swap");
        let p = decode(CM_A4, &[0xf0], 2, &clut, 0x102030, false, false).unwrap();
        assert_eq!((p[0].a, p[1].a, p[1].r, p[1].b), (0, 255, 0x10, 0x30));
        assert_eq!(encode(CM_RGB888, &[Argb { a: 9, r: 1, g: 2, b: 3 }], (77, 150, 29)).unwrap(), vec![3, 2, 1]);
        assert_eq!(encode(CM_GRAY8, &[W], (77, 150, 29)).unwrap(), vec![255]);
        assert_eq!(encode(CM_RGB565, &[W], (0, 0, 0)).unwrap(), vec![0xff, 0xff]);
        assert!(decode(9, &[], 1, &clut, 0, false, false).is_none(), "YUV422 not modelled");
    }
    #[test]
    fn fill_argb_and_yuv_refused() {
        let mut p = Ppa::new();
        p.write(0x24, 0); p.write(0x3c, 3 | 1 << 14); p.write(0x4c, 0x80112233);
        p.write(0x34, 1 | 4 | 8);
        assert_eq!(p.pending(), vec![PpaOp::Fill]);
        assert_eq!(p.run(PpaOp::Fill, &[]).unwrap(), [0x33, 0x22, 0x11, 0x80].repeat(3));
        assert!(p.pending().is_empty());
        p.write(0x24, 9 << 8); p.write(0x34, 1 | 4 | 8);
        assert_eq!(p.run(PpaOp::Fill, &[]), Err(0x24));
        p.write(0x64, 1 << 27); assert_eq!(p.pending(), vec![PpaOp::Srm]); assert_eq!(p.run(PpaOp::Srm, &[]), Err(0x64));
    }
    #[test]
    fn blend_fixed_and_scaled_alpha_and_a8_foreground() {
        let mut p = Ppa::new();
        // bg RGB888 black, fg A8 (alpha 255) with fixed colour red, fg alpha scaled by 128/256.
        p.write(0x24, CM_RGB888 | CM_A8 << 4 | CM_ARGB8888 << 8); p.write(0x3c, 1 | 1 << 14);
        p.write(0x48, 0xff0000); p.write(0x40, 128 << 8 | 2 << 18);
        p.write(0x50, 0xffffff); p.write(0x58, 0xffffff);
        p.write(0x34, 1 | 8);
        let out = p.run(PpaOp::Blend, &[vec![0, 0, 0], vec![255]]).unwrap();
        assert_eq!(out, vec![0, 0, 127, 255]);
        assert_eq!(p.read(0x10), 2);
        p.write(0x18, 2); assert!(p.irq()); p.write(0x1c, 2); assert!(!p.irq());
    }
    #[test]
    fn clut_fifo_and_memory_ports_agree() {
        let mut p = Ppa::new();
        p.write(0x0c, 1 << 3); p.write(0x00, 0xff00ff00); p.write(0x00, 0x11223344);
        assert_eq!(p.read(0x404), 0x11223344);
        p.write(0x804, 0x55); p.write(0x0c, 1 << 4); p.read(0x04); assert_eq!(p.read(0x04), 0x55);
    }
}
