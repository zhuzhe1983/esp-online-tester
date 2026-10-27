use super::*;

pub(super) fn floating_point_guard_proof() -> u32 {
    use Op::*;
    let mut tests = 0;
    // Ineligible helper-containing blocks still test the emitter's continuation
    // machinery. A helper may disable or enable CP before the scalar instruction.
    for op in [Wsr, Xsr] {
        for enabled in [0, 1] {
            for entry in 0..4 {
                for budget in [1, 2, 4] {
                    let mut block = [insn(Add), insn(op), insn(AddS), insn(Rfr)];
                    block[1].insn.imm = crate::state::sr::CPENABLE as i32;
                    compare(&mut block, Case { entry, budget, fast: true, ..Case::default() }, |c| {
                        c.cpenable = enabled; c.set_ar(5, enabled ^ 1);
                        c.fr[3] = 2f32.to_bits(); c.fr[4] = 3f32.to_bits(); c.fr[5] = 4f32.to_bits();
                    });
                    tests += 1;
                }
            }
        }
    }
    // Disabled CP must not prevent a prefix store or cause a trap when a budget
    // cut or taken branch exits before the FP instruction.
    for prefix in [S32i, Bt] {
        for enabled in [0, 1] {
            for entry in 0..3 {
                for budget in [1, 3] {
                    let mut block = [insn(prefix), insn(AddS), insn(Rfr)];
                    if prefix == Bt { block[0].insn.imm = (BASE + 48) as i32; }
                    compare(&mut block, Case { entry, budget, addr: Some(BASE + 512), fast: true, ..Case::default() }, |c| {
                        c.cpenable = enabled; c.br = 1 << 4;
                    });
                    tests += 1;
                }
            }
        }
    }
    tests
}

pub(super) fn floating_point_part(part: usize) -> u32 {
    use Op::*;
    let ops = [AddS, SubS, MulS, MaddS, MsubS, MovS, AbsS, NegS, Rfr, Wfr, ConstS,
        FloatS, UfloatS, RoundS, TruncS, FloorS, CeilS, UtruncS, UnS, OeqS, UeqS, OltS,
        UltS, OleS, UleS, MoveqzS, MovnezS, MovltzS, MovgezS, MovfS, MovtS,
        MaddnS, DivnS, Div0S, Nexp01S, Recip0S, Rsqrt0S, Sqrt0S, AddexpS,
        MkdadjS, MksadjS, AddexpmS, Movf, Movt, Bf, Bt];
    let values = [0, 0x8000_0000, 1, 0x007f_ffff, 0x0080_0000, 0x3f80_0001,
        0xbf80_0000, 0x3fc0_0000, 0xc020_0000, 0x4eff_ffff, 0x4f00_0000,
        0x4f7f_ffff, 0x4f80_0000, 0xcf00_0000, 0x7f7f_ffff, 0x7f80_0000,
        0xff80_0000, 0x7fc1_2345, 0xffc5_4321, 0x7f81_2345];
    let mut tests = 0;
    for (index, op) in ops.into_iter().enumerate() {
        if index % 4 != part { continue; }
        assert!(super::emitter::supported_opcode(op, true));
        for (n, &bits) in values.iter().enumerate() {
            for entry in 0..3 {
                for budget in [1, 3] {
                    let mut block = [insn(Add), insn(op), insn(Xor)];
                    block[1].insn.imm = if matches!(op, Bf | Bt) { (BASE + 48) as i32 } else { (n % 16) as i32 };
                    compare(&mut block, Case { seed: n as u32, entry, budget, fast: true, loop_end: n % 3 == 0, overflow: n % 7 == 0, ..Case::default() }, |c| {
                        c.cpenable = if n % 9 == 0 { 0 } else { 1 };
                        c.br = if n % 2 == 0 { 0xaaaa } else { 0x5555 };
                        c.fr[3] = 0xbf80_0000;
                        c.fr[4] = bits;
                        c.fr[5] = values[(n + 5) % values.len()];
                    });
                    tests += 1;
                }
            }
        }
    }
    if part != 0 { return tests; }
    // A drawing-like bundle connects float, boolean and integer state through a
    // conversion/coverage decision, including partial execution and aliased operands.
    let raster_ops = [Wfr, FloatS, SubS, MulS, MaddS, OltS, MovtS, TruncS, Movf, Bt];
    for entry in 0..raster_ops.len() as u32 {
        for budget in [1, 4, 12] {
            let mut block: Vec<_> = raster_ops.into_iter().map(insn).collect();
            for bi in &mut block {
                if matches!(bi.insn.op, MovtS | Movf) { bi.insn.t = 3; }
                if bi.insn.op == Bt { bi.insn.s = 3; bi.insn.imm = (BASE + 48) as i32; }
                bi.max_ar = crate::exec::max_ar(&bi.insn);
            }
            compare(&mut block, Case { seed: 15, entry, budget, fast: true, ..Case::default() }, |c| {
                c.cpenable = 1; c.fr[3] = 0.5f32.to_bits();
                c.fr[4] = 2.25f32.to_bits(); c.fr[5] = 3.5f32.to_bits();
            });
            tests += 1;
        }
    }
    // Cancellation distinguishes one fused rounding from multiply followed by add.
    for op in [MaddS, MsubS] {
        compare(&mut [insn(op), insn(Rfr)], Case { budget: 2, fast: true, ..Case::default() }, |c| {
            c.cpenable = 1; c.fr[3] = (-1f32).to_bits();
            c.fr[4] = if op == MaddS { 0x3f80_0001 } else { 0xbf80_0001 };
            c.fr[5] = 0x3f7f_fffe;
        });
        tests += 1;
    }
    // Both native TLB accesses and helper paths must preserve raw FP bits and code versions.
    for op in [Lsi, Ssi] {
        for addr in [BASE + 512, BASE + 513, BASE + 65536] {
            for fast in [false, true] {
                for readonly in [false, true] {
                    for enabled in [0, 1] {
                        compare(&mut [insn(Add), insn(op), insn(Xor)], Case { seed: 1, budget: 3, addr: Some(addr), fast, readonly, ..Case::default() }, |c| {
                                c.cpenable = enabled; c.fr[5] = 0x7f81_2345;
                            });
                        tests += 1;
                    }
                }
            }
        }
    }
    tests
}

/// EX170: one compiled [MaddS, MsubS] block run over `n` operand triples against the helper's
/// arithmetic (`f32::mul_add`, which is libm `fmaf` on wasm32). Returns the mismatch count and logs
/// how many triples were in the halfway class that still calls the helper. Operands mix uniform
/// random bits with adversarial classes: subnormals, exponent extremes, NaN payloads, infinities,
/// signed zeros, and sums constructed to land on or next to the f32 halfway pattern.
pub fn fma_sweep(seed: u64, n: u32, report: &mut dyn FnMut(String)) -> (u32, u32) {
    use Op::*;
    let mut block = [insn(MaddS), insn(MsubS)];
    block[1].insn.r = 6;
    let mut ram = Ram::new(true, false);
    let mut cc = CodeCache::new(0).unwrap();
    let code = queue(&mut cc, &mut block, BASE, true);
    for _ in 0..HOT { ready(&cc, code, 0); }
    assert!(ready(&cc, code, 0), "compiled module must execute");
    let mut c = cpu(1);
    c.cpenable = 1;
    let fm = ram.fast_mem();
    let helpers = Helpers::new::<Ram>();
    let mut state = seed | 1;
    let mut next = move || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };
    const SPECIAL: [u32; 24] = [0, 0x8000_0000, 1, 0x8000_0001, 2, 0x007f_ffff, 0x0080_0000, 0x0080_0001, 0x7f7f_ffff, 0xff7f_ffff,
        0x7f80_0000, 0xff80_0000, 0x7fc0_0000, 0xffc0_0000, 0x7f80_0001, 0xff81_2345, 0x7fc1_2345, 0xffff_ffff,
        0x3f80_0000, 0x3f80_0001, 0x3f7f_ffff, 0xbf80_0001, 0x3400_0000, 0x3380_0000];
    let (mut bad, mut halfway) = (0u32, 0u32);
    for _ in 0..n {
        let w = next();
        let pick = |v: u64, w: u64| -> u32 {
            match w & 15 {
                0..=5 => v as u32,                                                              // any bits
                6..=8 => (v as u32 & 0x807f_ffff) | (((v >> 32) as u32 % 60 + 97) << 23),       // mid exponents: products and addends interact
                9 => v as u32 & 0x807f_ffff,                                                    // subnormal or zero
                10 => (v as u32 & 0x80ff_ffff) | if v >> 63 != 0 { 0x7f00_0000 } else { 0 },    // exponent extremes: tiny, huge, inf, NaN
                11 => (v as u32 & 0x8000_0fff) | ((v >> 32) as u32 & 0x7f80_0000),              // few significant bits: exact sums and ties
                12 => (v as u32 & 0xff80_0000) | 0x007f_f000 | ((v >> 32) as u32 & 0xfff),      // long runs of ones
                _ => SPECIAL[(v >> 20) as usize % SPECIAL.len()],
            }
        };
        let (mut s, mut t) = (pick(next(), w), pick(next(), w >> 4));
        let mut z = pick(next(), w >> 8);
        if w >> 12 & 3 == 0 {
            // The halfway class on purpose: z = M * 2^k, s * t = +-2^(k-1) * (1 - a^2 * 2^-46), so the f64
            // sum rounds onto M + 1/2 exactly while the true sum sits just beside the tie. With a = 0
            // it is an exact tie. k reaches down into subnormal results and up to overflow.
            let v = next();
            let m = (v as u32 & 0x00ff_ffff) | if v >> 24 & 1 == 0 { 0x0080_0000 } else { 0 };
            let a = if v >> 25 & 3 == 0 { 0 } else { (v >> 27) as u32 & 0x3ff };
            let k = (v >> 40) as i32 % 140 - if v >> 39 & 1 == 0 { 150 } else { 40 };
            let scale = |x: f64, e: i32| x * f64::from_bits(((1023 + e.clamp(-1022, 1023)) as u64) << 52);
            let k1 = k / 2;
            z = (scale(f64::from(m), k) as f32).to_bits() | ((v >> 38) as u32 & 1) << 31;
            s = (scale(1.0 + f64::from(a) / 8388608.0, k1) as f32).to_bits() | ((v >> 37) as u32 & 1) << 31;
            t = (scale(1.0 - f64::from(a) / 8388608.0, k - 1 - k1) as f32).to_bits();
            if v >> 36 & 1 != 0 { z = z.wrapping_add(((v >> 34) & 3) as u32).wrapping_sub(1); }
        }
        c.fr[4] = s; c.fr[5] = t; c.fr[3] = z; c.fr[6] = z;
        c.pc = BASE;
        let result = unsafe { run(&cc, code, &mut c, &mut ram, &helpers, 2, 0, fm) };
        assert_eq!(result & 0xffff, 2, "both instructions must retire");
        let (fs, ft, fz) = (f32::from_bits(s), f32::from_bits(t), f32::from_bits(z));
        let want = [fs.mul_add(ft, fz).to_bits(), (-fs).mul_add(ft, fz).to_bits()];
        let wide = (f64::from(fs) * f64::from(ft) + f64::from(fz)).to_bits();
        if wide & 0x1fff_ffff == 0x1000_0000 { halfway += 1; }
        if [c.fr[3], c.fr[6]] != want {
            bad += 1;
            if bad <= 8 { report(format!("fma mismatch s={s:#010x} t={t:#010x} z={z:#010x} jit={:#010x},{:#010x} helper={:#010x},{:#010x}", c.fr[3], c.fr[6], want[0], want[1])); }
        }
    }
    (bad, halfway)
}

/// Directed double-rounding cases exercise the emitted fallback in the ordinary CI suite.
pub(super) fn fma_halfway_fallback() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CALLS: AtomicU32 = AtomicU32::new(0);
    extern "C" fn counted_fused(s: u32, t: u32, r: u32, subtract: u32) -> u32 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        h_fused(s, t, r, subtract)
    }
    for op in [Op::MaddS, Op::MsubS] {
        let mut ram = Ram::new(true, false);
        let mut cc = CodeCache::new(0).unwrap();
        let code = queue(&mut cc, &mut [insn(op)], BASE, true);
        for _ in 0..HOT { ready(&cc, code, 0); }
        assert!(ready(&cc, code, 0));
        let mut c = cpu(1);
        c.pc = BASE;
        c.cpenable = 1;
        c.fr[3] = 0x3f80_0001;
        c.fr[4] = if op == Op::MaddS { 0x3f80_0001 } else { 0xbf80_0001 };
        c.fr[5] = 0x337f_fffe;
        let s = f64::from(f32::from_bits(c.fr[4])) * if op == Op::MsubS { -1.0 } else { 1.0 };
        let wide = s * f64::from(f32::from_bits(c.fr[5])) + f64::from(f32::from_bits(c.fr[3]));
        assert_eq!(wide.to_bits() & 0x1fff_ffff, 0x1000_0000);
        assert_eq!((wide as f32).to_bits(), 0x3f80_0002, "inlining without fallback double-rounds");
        let mut helpers = Helpers::new::<Ram>();
        helpers.fused = counted_fused as *const ();
        CALLS.store(0, Ordering::Relaxed);
        let fm = ram.fast_mem();
        let result = unsafe { run(&cc, code, &mut c, &mut ram, &helpers, 1, 0, fm) };
        assert_eq!(result & 0xffff, 1);
        assert_eq!(CALLS.load(Ordering::Relaxed), 1, "generated code must call the fallback");
        assert_eq!(c.fr[3], 0x3f80_0001, "fallback preserves the helper's single rounding");
    }
    2
}
