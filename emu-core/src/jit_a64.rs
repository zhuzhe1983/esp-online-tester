//! A tiny AArch64 encoder: only the instructions the block compiler emits. Every encoding is
//! checked against clang's assembler: `tests::encodings_match_fixture` compares with the fixture
//! `tests::external_encodings_match_clang` regenerates.
#![allow(dead_code)]

pub type Reg = u32;
pub const ZR: Reg = 31;
pub const SP: Reg = 31;

/// Condition codes (the `cond` field).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cond { Eq = 0, Ne = 1, Hs = 2, Lo = 3, Mi = 4, Pl = 5, Vs = 6, Vc = 7, Hi = 8, Ls = 9, Ge = 10, Lt = 11, Gt = 12, Le = 13, Al = 14 }
impl Cond {
    pub fn invert(self) -> Cond {
        use Cond::*;
        match self { Eq => Ne, Ne => Eq, Hs => Lo, Lo => Hs, Mi => Pl, Pl => Mi, Vs => Vc, Vc => Vs, Hi => Ls, Ls => Hi, Ge => Lt, Lt => Ge, Gt => Le, Le => Gt, Al => Al }
    }
}

#[derive(Clone, Copy)]
pub struct Label(pub usize);

enum Fix { B26, B19, B14, Adr21 }

pub struct Asm {
    pub code: Vec<u32>,
    labels: Vec<Option<usize>>,
    fixups: Vec<(usize, usize, Fix)>,
}

#[allow(clippy::new_without_default, reason = "assembler construction reserves its expected code capacity")]
impl Asm {
    pub fn new() -> Self { Asm { code: Vec::with_capacity(256), labels: Vec::new(), fixups: Vec::new() } }
    #[allow(clippy::len_without_is_empty, reason = "the length is exposed as a code offset, not as a collection API")]
    pub fn len(&self) -> usize { self.code.len() }
    pub fn here(&self) -> usize { self.code.len() }
    fn e(&mut self, w: u32) { self.code.push(w); }

    pub fn label(&mut self) -> Label { self.labels.push(None); Label(self.labels.len() - 1) }
    pub fn bind(&mut self, l: Label) { self.labels[l.0] = Some(self.code.len()); }
    pub fn is_bound(&self, l: Label) -> bool { self.labels[l.0].is_some() }

    /// Resolve every branch to its label; every label must be bound.
    pub fn finish(mut self) -> Vec<u32> {
        for (at, l, kind) in std::mem::take(&mut self.fixups) {
            let target = self.labels[l].expect("unbound label");
            let off = target as i64 - at as i64;
            match kind {
                Fix::B26 => { assert!((-(1 << 25)..(1 << 25)).contains(&off)); self.code[at] |= (off as u32) & 0x03ff_ffff; }
                Fix::B19 => { assert!((-(1 << 18)..(1 << 18)).contains(&off)); self.code[at] |= ((off as u32) & 0x7ffff) << 5; }
                Fix::B14 => { assert!((-(1 << 13)..(1 << 13)).contains(&off)); self.code[at] |= ((off as u32) & 0x3fff) << 5; }
                Fix::Adr21 => { let b = off * 4; assert!((-(1 << 20)..(1 << 20)).contains(&b)); let b = b as u32; self.code[at] |= (b & 3) << 29 | ((b >> 2) & 0x7ffff) << 5; }
            }
        }
        self.code
    }

    // ---------------------------------------------------------------- moves and immediates
    pub fn movz(&mut self, rd: Reg, imm16: u32, shift: u32) { self.e(0x5280_0000 | (shift / 16) << 21 | (imm16 & 0xffff) << 5 | rd); }
    pub fn movk(&mut self, rd: Reg, imm16: u32, shift: u32) { self.e(0x7280_0000 | (shift / 16) << 21 | (imm16 & 0xffff) << 5 | rd); }
    pub fn movn(&mut self, rd: Reg, imm16: u32, shift: u32) { self.e(0x1280_0000 | (shift / 16) << 21 | (imm16 & 0xffff) << 5 | rd); }
    pub fn movz_x(&mut self, rd: Reg, imm16: u32, shift: u32) { self.e(0xd280_0000 | (shift / 16) << 21 | (imm16 & 0xffff) << 5 | rd); }
    pub fn movk_x(&mut self, rd: Reg, imm16: u32, shift: u32) { self.e(0xf280_0000 | (shift / 16) << 21 | (imm16 & 0xffff) << 5 | rd); }
    /// 32-bit immediate in one or two instructions.
    pub fn mov32(&mut self, rd: Reg, imm: u32) {
        if imm & 0xffff_0000 == 0 { self.movz(rd, imm, 0); }
        else if imm & 0xffff == 0 { self.movz(rd, imm >> 16, 16); }
        else if imm & 0xffff_0000 == 0xffff_0000 { self.movn(rd, !imm & 0xffff, 0); }
        else { self.movz(rd, imm & 0xffff, 0); self.movk(rd, imm >> 16, 16); }
    }
    /// 64-bit immediate (addresses).
    pub fn mov64(&mut self, rd: Reg, imm: u64) {
        self.movz_x(rd, (imm & 0xffff) as u32, 0);
        for sh in [16u32, 32, 48] { let part = ((imm >> sh) & 0xffff) as u32; if part != 0 { self.movk_x(rd, part, sh); } }
    }
    pub fn mov(&mut self, rd: Reg, rm: Reg) { self.e(0x2a00_03e0 | rm << 16 | rd); }
    pub fn mov_x(&mut self, rd: Reg, rm: Reg) { self.e(0xaa00_03e0 | rm << 16 | rd); }

    // ---------------------------------------------------------------- arithmetic
    pub fn add_imm(&mut self, rd: Reg, rn: Reg, imm: u32) { debug_assert!(imm < 4096); self.e(0x1100_0000 | imm << 10 | rn << 5 | rd); }
    pub fn sub_imm(&mut self, rd: Reg, rn: Reg, imm: u32) { debug_assert!(imm < 4096); self.e(0x5100_0000 | imm << 10 | rn << 5 | rd); }
    pub fn subs_imm(&mut self, rd: Reg, rn: Reg, imm: u32) { debug_assert!(imm < 4096); self.e(0x7100_0000 | imm << 10 | rn << 5 | rd); }
    pub fn add_imm_x(&mut self, rd: Reg, rn: Reg, imm: u32) { debug_assert!(imm < 4096); self.e(0x9100_0000 | imm << 10 | rn << 5 | rd); }
    pub fn cmp_imm(&mut self, rn: Reg, imm: u32) { self.subs_imm(ZR, rn, imm); }
    /// add/sub with a 32-bit immediate of any size (may use a scratch register).
    pub fn add_imm32(&mut self, rd: Reg, rn: Reg, imm: u32, scratch: Reg) {
        if imm < 4096 { self.add_imm(rd, rn, imm); }
        else if imm.wrapping_neg() < 4096 { self.sub_imm(rd, rn, imm.wrapping_neg()); }
        else { self.mov32(scratch, imm); self.add(rd, rn, scratch); }
    }
    pub fn add(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x0b00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn add_x(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x8b00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn add_lsl(&mut self, rd: Reg, rn: Reg, rm: Reg, sh: u32) { self.e(0x0b00_0000 | rm << 16 | sh << 10 | rn << 5 | rd); }
    pub fn add_lsr(&mut self, rd: Reg, rn: Reg, rm: Reg, sh: u32) { self.e(0x0b40_0000 | rm << 16 | sh << 10 | rn << 5 | rd); }
    pub fn add_x_lsl(&mut self, rd: Reg, rn: Reg, rm: Reg, sh: u32) { self.e(0x8b00_0000 | rm << 16 | sh << 10 | rn << 5 | rd); }
    pub fn eor_lsr(&mut self, rd: Reg, rn: Reg, rm: Reg, sh: u32) { self.e(0x4a40_0000 | rm << 16 | sh << 10 | rn << 5 | rd); }
    pub fn sub(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x4b00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn sub_lsl(&mut self, rd: Reg, rn: Reg, rm: Reg, sh: u32) { self.e(0x4b00_0000 | rm << 16 | sh << 10 | rn << 5 | rd); }
    pub fn subs(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x6b00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn cmp(&mut self, rn: Reg, rm: Reg) { self.subs(ZR, rn, rm); }
    pub fn neg(&mut self, rd: Reg, rm: Reg) { self.sub(rd, ZR, rm); }
    pub fn and(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x0a00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn orr(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x2a00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn orr_lsl(&mut self, rd: Reg, rn: Reg, rm: Reg, sh: u32) { self.e(0x2a00_0000 | rm << 16 | sh << 10 | rn << 5 | rd); }
    pub fn orr_x(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0xaa00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn eor(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x4a00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn bic(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x0a20_0000 | rm << 16 | rn << 5 | rd); }
    pub fn ands(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x6a00_0000 | rm << 16 | rn << 5 | rd); }
    pub fn tst(&mut self, rn: Reg, rm: Reg) { self.ands(ZR, rn, rm); }
    /// `and` with a contiguous mask `((1 << ones) - 1) << shift` (32-bit).
    pub fn and_mask(&mut self, rd: Reg, rn: Reg, ones: u32, shift: u32) {
        debug_assert!((1..=31).contains(&ones) && ones + shift <= 32);
        self.e(0x1200_0000 | ((32 - shift) & 31) << 16 | (ones - 1) << 10 | rn << 5 | rd);
    }
    pub fn tst_mask(&mut self, rn: Reg, ones: u32, shift: u32) {
        debug_assert!((1..=31).contains(&ones) && ones + shift <= 32);
        self.e(0x7200_0000 | ((32 - shift) & 31) << 16 | (ones - 1) << 10 | rn << 5 | ZR);
    }
    pub fn lsl_imm(&mut self, rd: Reg, rn: Reg, sh: u32) { let sh = sh & 31; self.e(0x5300_0000 | ((32 - sh) & 31) << 16 | (31 - sh) << 10 | rn << 5 | rd); }
    pub fn lsr_imm(&mut self, rd: Reg, rn: Reg, sh: u32) { let sh = sh & 31; self.e(0x5300_0000 | sh << 16 | 31 << 10 | rn << 5 | rd); }
    pub fn asr_imm(&mut self, rd: Reg, rn: Reg, sh: u32) { let sh = sh & 31; self.e(0x1300_0000 | sh << 16 | 31 << 10 | rn << 5 | rd); }
    pub fn lsr_imm_x(&mut self, rd: Reg, rn: Reg, sh: u32) { self.e(0xd340_0000 | sh << 16 | 63 << 10 | rn << 5 | rd); }
    pub fn asr_imm_x(&mut self, rd: Reg, rn: Reg, sh: u32) { self.e(0x9340_0000 | sh << 16 | 63 << 10 | rn << 5 | rd); }
    pub fn lsl_imm_x(&mut self, rd: Reg, rn: Reg, sh: u32) { self.e(0xd340_0000 | ((64 - sh) & 63) << 16 | (63 - sh) << 10 | rn << 5 | rd); }
    pub fn ubfx(&mut self, rd: Reg, rn: Reg, lsb: u32, width: u32) { self.e(0x5300_0000 | lsb << 16 | (lsb + width - 1) << 10 | rn << 5 | rd); }
    pub fn sbfx(&mut self, rd: Reg, rn: Reg, lsb: u32, width: u32) { self.e(0x1300_0000 | lsb << 16 | (lsb + width - 1) << 10 | rn << 5 | rd); }
    pub fn sxth(&mut self, rd: Reg, rn: Reg) { self.sbfx(rd, rn, 0, 16); }
    pub fn uxth(&mut self, rd: Reg, rn: Reg) { self.ubfx(rd, rn, 0, 16); }
    pub fn lslv(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x1ac0_2000 | rm << 16 | rn << 5 | rd); }
    pub fn lsrv(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x1ac0_2400 | rm << 16 | rn << 5 | rd); }
    pub fn asrv(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x1ac0_2800 | rm << 16 | rn << 5 | rd); }
    pub fn lsrv_x(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x9ac0_2400 | rm << 16 | rn << 5 | rd); }
    pub fn mul(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x1b00_7c00 | rm << 16 | rn << 5 | rd); }
    pub fn umull(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x9ba0_7c00 | rm << 16 | rn << 5 | rd); }
    pub fn smull(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x9b20_7c00 | rm << 16 | rn << 5 | rd); }
    pub fn udiv(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x1ac0_0800 | rm << 16 | rn << 5 | rd); }
    pub fn sdiv(&mut self, rd: Reg, rn: Reg, rm: Reg) { self.e(0x1ac0_0c00 | rm << 16 | rn << 5 | rd); }
    /// rd = ra - rn * rm
    pub fn msub(&mut self, rd: Reg, rn: Reg, rm: Reg, ra: Reg) { self.e(0x1b00_8000 | rm << 16 | ra << 10 | rn << 5 | rd); }
    pub fn clz(&mut self, rd: Reg, rn: Reg) { self.e(0x5ac0_1000 | rn << 5 | rd); }
    pub fn csel(&mut self, rd: Reg, rn: Reg, rm: Reg, c: Cond) { self.e(0x1a80_0000 | rm << 16 | (c as u32) << 12 | rn << 5 | rd); }
    pub fn csinc(&mut self, rd: Reg, rn: Reg, rm: Reg, c: Cond) { self.e(0x1a80_0400 | rm << 16 | (c as u32) << 12 | rn << 5 | rd); }
    pub fn csneg(&mut self, rd: Reg, rn: Reg, rm: Reg, c: Cond) { self.e(0x5a80_0400 | rm << 16 | (c as u32) << 12 | rn << 5 | rd); }
    pub fn cset(&mut self, rd: Reg, c: Cond) { self.csinc(rd, ZR, ZR, c.invert()); }
    pub fn cneg(&mut self, rd: Reg, rn: Reg, c: Cond) { self.csneg(rd, rn, rn, c.invert()); }

    // ---------------------------------------------------------------- memory
    pub fn ldr(&mut self, rt: Reg, rn: Reg, off: u32) { debug_assert!(off.is_multiple_of(4) && off < 16384); self.e(0xb940_0000 | (off / 4) << 10 | rn << 5 | rt); }
    pub fn str(&mut self, rt: Reg, rn: Reg, off: u32) { debug_assert!(off.is_multiple_of(4) && off < 16384); self.e(0xb900_0000 | (off / 4) << 10 | rn << 5 | rt); }
    pub fn ldrh(&mut self, rt: Reg, rn: Reg, off: u32) { debug_assert!(off.is_multiple_of(2) && off < 8192); self.e(0x7940_0000 | (off / 2) << 10 | rn << 5 | rt); }
    pub fn ldr_x(&mut self, rt: Reg, rn: Reg, off: u32) { debug_assert!(off.is_multiple_of(8) && off < 32768); self.e(0xf940_0000 | (off / 8) << 10 | rn << 5 | rt); }
    pub fn str_x(&mut self, rt: Reg, rn: Reg, off: u32) { debug_assert!(off.is_multiple_of(8) && off < 32768); self.e(0xf900_0000 | (off / 8) << 10 | rn << 5 | rt); }
    /// `ldr wt, [xn, wm, uxtw #2]`
    pub fn ldr_idx(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0xb860_0800 | wm << 16 | 0b010 << 13 | 1 << 12 | rn << 5 | rt); }
    /// `str wt, [xn, wm, uxtw #2]`
    pub fn str_idx(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0xb820_0800 | wm << 16 | 0b010 << 13 | 1 << 12 | rn << 5 | rt); }
    /// Byte-offset register forms `[xn, wm, uxtw]` (no scaling): loads zero-extend, `ldrsh`/`ldrsb` sign-extend to 32 bits.
    pub fn ldr_u(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0xb860_4800 | wm << 16 | rn << 5 | rt); }
    pub fn ldrh_u(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0x7860_4800 | wm << 16 | rn << 5 | rt); }
    pub fn ldrb_u(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0x3860_4800 | wm << 16 | rn << 5 | rt); }
    pub fn ldrsh_u(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0x78e0_4800 | wm << 16 | rn << 5 | rt); }
    pub fn str_u(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0xb820_4800 | wm << 16 | rn << 5 | rt); }
    pub fn strh_u(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0x7820_4800 | wm << 16 | rn << 5 | rt); }
    pub fn strb_u(&mut self, rt: Reg, rn: Reg, wm: Reg) { self.e(0x3820_4800 | wm << 16 | rn << 5 | rt); }
    /// `stp xt1, xt2, [sp, #-imm]!`
    pub fn stp_pre(&mut self, rt1: Reg, rt2: Reg, rn: Reg, off: i32) { self.e(0xa980_0000 | (((off / 8) as u32) & 0x7f) << 15 | rt2 << 10 | rn << 5 | rt1); }
    pub fn stp(&mut self, rt1: Reg, rt2: Reg, rn: Reg, off: i32) { self.e(0xa900_0000 | (((off / 8) as u32) & 0x7f) << 15 | rt2 << 10 | rn << 5 | rt1); }
    pub fn ldp(&mut self, rt1: Reg, rt2: Reg, rn: Reg, off: i32) { self.e(0xa940_0000 | (((off / 8) as u32) & 0x7f) << 15 | rt2 << 10 | rn << 5 | rt1); }
    /// `ldp xt1, xt2, [sp], #imm`
    pub fn ldp_post(&mut self, rt1: Reg, rt2: Reg, rn: Reg, off: i32) { self.e(0xa8c0_0000 | (((off / 8) as u32) & 0x7f) << 15 | rt2 << 10 | rn << 5 | rt1); }

    // ---------------------------------------------------------------- control
    pub fn b(&mut self, l: Label) { self.fixups.push((self.code.len(), l.0, Fix::B26)); self.e(0x1400_0000); }
    pub fn b_cond(&mut self, c: Cond, l: Label) { self.fixups.push((self.code.len(), l.0, Fix::B19)); self.e(0x5400_0000 | c as u32); }
    pub fn cbz(&mut self, rt: Reg, l: Label) { self.fixups.push((self.code.len(), l.0, Fix::B19)); self.e(0x3400_0000 | rt); }
    pub fn cbnz(&mut self, rt: Reg, l: Label) { self.fixups.push((self.code.len(), l.0, Fix::B19)); self.e(0x3500_0000 | rt); }
    pub fn cbnz_x(&mut self, rt: Reg, l: Label) { self.fixups.push((self.code.len(), l.0, Fix::B19)); self.e(0xb500_0000 | rt); }
    /// `tbz xt, #bit, label` (bit may be up to 63 — the test is on the 64-bit register)
    pub fn tbz(&mut self, rt: Reg, bit: u32, l: Label) { let at = self.code.len(); self.e(0x3600_0000 | (bit >> 5) << 31 | (bit & 31) << 19 | rt); self.tb_fix(at, l); }
    pub fn tbnz(&mut self, rt: Reg, bit: u32, l: Label) { let at = self.code.len(); self.e(0x3700_0000 | (bit >> 5) << 31 | (bit & 31) << 19 | rt); self.tb_fix(at, l); }
    fn tb_fix(&mut self, at: usize, l: Label) { self.fixups.push((at, l.0, Fix::B14)); }
    pub fn blr(&mut self, rn: Reg) { self.e(0xd63f_0000 | rn << 5); }
    pub fn br(&mut self, rn: Reg) { self.e(0xd61f_0000 | rn << 5); }
    pub fn ret(&mut self) { self.e(0xd65f_03c0); }
    /// `adr xd, label`
    pub fn adr(&mut self, rd: Reg, l: Label) { self.fixups.push((self.code.len(), l.0, Fix::Adr21)); self.e(0x1000_0000 | rd); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference program: assembler text and our encoding of it, one instruction per line.
    fn program() -> (String, Vec<u32>) {
        let mut a = Asm::new();
        let l_end = a.label();
        let mut text = String::new();
        macro_rules! t { ($txt:expr, $e:expr) => { text.push_str($txt); text.push('\n'); $e; } }
        t!("movz w9, #0x1234", a.movz(9, 0x1234, 0));
        t!("movk w9, #0x5678, lsl #16", a.movk(9, 0x5678, 16));
        t!("movn w10, #7", a.movn(10, 7, 0));
        t!("movz x11, #0xabcd, lsl #32", a.movz_x(11, 0xabcd, 32));
        t!("movk x11, #1, lsl #48", a.movk_x(11, 1, 48));
        t!("mov w12, w13", a.mov(12, 13));
        t!("mov x14, x15", a.mov_x(14, 15));
        t!("add w9, w10, #100", a.add_imm(9, 10, 100));
        t!("sub w9, w10, #4095", a.sub_imm(9, 10, 4095));
        t!("subs w9, w10, #7", a.subs_imm(9, 10, 7));
        t!("add x21, x19, #864", a.add_imm_x(21, 19, 864));
        t!("cmp w9, #32", a.cmp_imm(9, 32));
        t!("add w9, w10, w11", a.add(9, 10, 11));
        t!("add x9, x10, x11", a.add_x(9, 10, 11));
        t!("add w9, w10, w11, lsl #3", a.add_lsl(9, 10, 11, 3));
        t!("add w9, w10, w11, lsr #8", a.add_lsr(9, 10, 11, 8));
        t!("add x9, x24, x9, lsl #5", a.add_x_lsl(9, 24, 9, 5));
        t!("eor w9, w9, w1, lsr #24", a.eor_lsr(9, 9, 1, 24));
        t!("sub w9, w10, w11", a.sub(9, 10, 11));
        t!("sub w9, w10, w11, lsl #2", a.sub_lsl(9, 10, 11, 2));
        t!("cmp w9, w10", a.cmp(9, 10));
        t!("neg w9, w10", a.neg(9, 10));
        t!("and w9, w10, w11", a.and(9, 10, 11));
        t!("orr w9, w10, w11", a.orr(9, 10, 11));
        t!("orr w0, w9, w0, lsl #16", a.orr_lsl(0, 9, 0, 16));
        t!("orr x9, x10, x11", a.orr_x(9, 10, 11));
        t!("eor w9, w10, w11", a.eor(9, 10, 11));
        t!("bic w9, w10, w11", a.bic(9, 10, 11));
        t!("tst w9, w10", a.tst(9, 10));
        t!("and w9, w10, #0x3f", a.and_mask(9, 10, 6, 0));
        t!("and w9, w10, #0x1f", a.and_mask(9, 10, 5, 0));
        t!("and w9, w10, #0xffff", a.and_mask(9, 10, 16, 0));
        t!("and w9, w10, #0x18", a.and_mask(9, 10, 2, 3));
        t!("tst w9, #7", a.tst_mask(9, 3, 0));
        t!("lsl w9, w10, #5", a.lsl_imm(9, 10, 5));
        t!("lsl w9, w10, #0", a.lsl_imm(9, 10, 0));
        t!("lsr w9, w10, #5", a.lsr_imm(9, 10, 5));
        t!("asr w9, w10, #31", a.asr_imm(9, 10, 31));
        t!("lsr x9, x10, #32", a.lsr_imm_x(9, 10, 32));
        t!("asr x9, x10, #32", a.asr_imm_x(9, 10, 32));
        t!("lsl x9, x10, #32", a.lsl_imm_x(9, 10, 32));
        t!("ubfx w9, w10, #3, #7", a.ubfx(9, 10, 3, 7));
        t!("sbfx w9, w10, #0, #12", a.sbfx(9, 10, 0, 12));
        t!("sxth w9, w10", a.sxth(9, 10));
        t!("uxth w9, w10", a.uxth(9, 10));
        t!("lsl w9, w10, w11", a.lslv(9, 10, 11));
        t!("lsr w9, w10, w11", a.lsrv(9, 10, 11));
        t!("asr w9, w10, w11", a.asrv(9, 10, 11));
        t!("lsr x9, x10, x11", a.lsrv_x(9, 10, 11));
        t!("mul w9, w10, w11", a.mul(9, 10, 11));
        t!("umull x9, w10, w11", a.umull(9, 10, 11));
        t!("smull x9, w10, w11", a.smull(9, 10, 11));
        t!("udiv w11, w9, w10", a.udiv(11, 9, 10));
        t!("sdiv w11, w9, w10", a.sdiv(11, 9, 10));
        t!("msub w12, w11, w10, w9", a.msub(12, 11, 10, 9));
        t!("clz w9, w10", a.clz(9, 10));
        t!("csel w9, w10, w11, lt", a.csel(9, 10, 11, Cond::Lt));
        t!("csinc w9, w10, w11, hs", a.csinc(9, 10, 11, Cond::Hs));
        t!("csneg w9, w10, w11, pl", a.csneg(9, 10, 11, Cond::Pl));
        t!("cset w9, lo", a.cset(9, Cond::Lo));
        t!("cneg w9, w10, mi", a.cneg(9, 10, Cond::Mi));
        t!("ldr w9, [x19, #128]", a.ldr(9, 19, 128));
        t!("str w9, [x19, #4]", a.str(9, 19, 4));
        t!("ldr x9, [x25, #16]", a.ldr_x(9, 25, 16));
        t!("str x9, [x25, #8]", a.str_x(9, 25, 8));
        t!("ldr w10, [x21, w9, uxtw #2]", a.ldr_idx(10, 21, 9));
        t!("str w10, [x21, w9, uxtw #2]", a.str_idx(10, 21, 9));
        t!("ldr w0, [x12, w10, uxtw]", a.ldr_u(0, 12, 10));
        t!("ldrh w0, [x12, w10, uxtw]", a.ldrh_u(0, 12, 10));
        t!("ldrh w11, [x9, #20]", a.ldrh(11, 9, 20));
        t!("ldrb w0, [x12, w10, uxtw]", a.ldrb_u(0, 12, 10));
        t!("ldrsh w0, [x12, w10, uxtw]", a.ldrsh_u(0, 12, 10));
        t!("str w2, [x12, w10, uxtw]", a.str_u(2, 12, 10));
        t!("strh w2, [x12, w10, uxtw]", a.strh_u(2, 12, 10));
        t!("strb w2, [x12, w10, uxtw]", a.strb_u(2, 12, 10));
        t!("str w3, [sp, #96]", a.str(3, SP, 96));
        t!("ldr w9, [sp, #96]", a.ldr(9, SP, 96));
        t!("stp x29, x30, [sp, #-96]!", a.stp_pre(29, 30, SP, -96));
        t!("stp x19, x20, [sp, #16]", a.stp(19, 20, SP, 16));
        t!("ldp x19, x20, [sp, #16]", a.ldp(19, 20, SP, 16));
        t!("ldp x29, x30, [sp], #96", a.ldp_post(29, 30, SP, 96));
        t!("b 1f", a.b(l_end));
        t!("b.ne 1f", a.b_cond(Cond::Ne, l_end));
        t!("cbz w9, 1f", a.cbz(9, l_end));
        t!("cbnz w9, 1f", a.cbnz(9, l_end));
        t!("cbnz x9, 1f", a.cbnz_x(9, l_end));
        t!("tbz w9, #5, 1f", a.tbz(9, 5, l_end));
        t!("tbnz x9, #33, 1f", a.tbnz(9, 33, l_end));
        t!("adr x9, 1f", a.adr(9, l_end));
        t!("blr x9", a.blr(9));
        t!("br x9", a.br(9));
        t!("ret", a.ret());
        text.push_str("1:\n");
        a.bind(l_end);
        (text, a.finish())
    }

    /// Hermetic: every word must match `tests/fixtures/a64_expected.hex`, which `external_encodings_match_clang`
    /// produced from clang's assembler (run it with `UPDATE_FIXTURE=1` after adding an instruction).
    #[test]
    fn encodings_match_fixture() {
        let (text, ours) = program();
        let want: Vec<u32> = include_str!("../tests/fixtures/a64_expected.hex").lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(|l| u32::from_str_radix(l.trim(), 16).unwrap()).collect();
        assert_eq!(want.len(), ours.len(), "instruction count differs from the fixture; regenerate it with clang");
        for (i, ((w, o), line)) in want.iter().zip(ours.iter()).zip(text.lines()).enumerate() { assert_eq!(*w, *o, "#{} {:08x} (fixture) != {:08x} (ours): {}", i, w, o, line); }
    }

    /// Assemble the reference text with clang and compare word for word with our encoder.
    #[test]
    #[ignore = "needs Apple's clang and llvm-objdump; regenerates the fixture with UPDATE_FIXTURE=1"]
    fn external_encodings_match_clang() {
        let clang = "/Library/Developer/CommandLineTools/usr/bin/clang";
        let objdump = "/Library/Developer/CommandLineTools/usr/bin/llvm-objdump";
        assert!(std::path::Path::new(clang).exists(), "{} not found", clang);
        let (text, ours) = program();
        let dir = std::env::temp_dir().join(format!("a64enc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.s"), &text).unwrap();
        let st = std::process::Command::new(clang).args(["-c", "-arch", "arm64", "-x", "assembler", "-o"]).arg(dir.join("t.o")).arg(dir.join("t.s")).status().unwrap();
        assert!(st.success(), "clang failed");
        let out = std::process::Command::new(objdump).arg("-d").arg(dir.join("t.o")).output().unwrap();
        let dis = String::from_utf8_lossy(&out.stdout);
        let mut theirs = Vec::new();
        for line in dis.lines() {
            let l = line.trim();
            // "       0: 52824689     mov w9, #0x1234"
            let mut it = l.splitn(3, |c: char| c.is_whitespace());
            let (Some(addr), Some(word)) = (it.next(), it.next()) else { continue };
            if !addr.ends_with(':') { continue; }
            if let Ok(w) = u32::from_str_radix(word.trim(), 16) { theirs.push((w, l.to_string())); }
        }
        assert_eq!(theirs.len(), ours.len(), "instruction count differs\n{}", dis);
        for (i, ((w, line), o)) in theirs.iter().zip(ours.iter()).enumerate() {
            assert_eq!(*w, *o, "#{} {:08x} (clang) != {:08x} (ours): {}", i, w, o, line);
        }
        let _ = std::fs::remove_dir_all(&dir);
        if std::env::var("UPDATE_FIXTURE").is_ok() {
            let mut f = String::from("# AArch64 encodings of the reference program in jit_a64.rs tests, as clang assembled them.\n");
            for w in &ours { f += &format!("{:08x}\n", w); }
            std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/a64_expected.hex"), f).unwrap();
        }
    }
}
