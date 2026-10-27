use super::*;

pub(super) fn integer_ops_part(part: usize) -> u32 {
    use Op::*;
    let mut tests = 0;
    let values = [0, 1, 0xffff_ffff, 0x8000_0000, 0x7fff_ffff, 0xa5a5_5a5a, 0x0001_8000];
    // coverage-s3: MUL16 sign/zero extension and all four mask-branch outcomes need operand pairs.
    for (index, op) in [Abs, Sra, Src, Muluh, Mulsh, Mul16u, Mul16s, Bany, Bnone, Ball, Bnall].into_iter().enumerate() {
        if index % 3 != part { continue; }
        // Production admission is separate from queue(), used by the differential harness.
        let mut admitted = [insn(Nop), insn(op), insn(Xor)];
        let mut cc = CodeCache::new(0).unwrap();
        assert!(compile(&mut cc, &mut admitted, BASE, false).is_some());
        for dest in [3, 4, 5] {
            let mut arithmetic = insn(op);
            arithmetic.insn.r = dest;
            arithmetic.max_ar = crate::exec::max_ar(&arithmetic.insn);
            let pairs: Vec<(u32, u32)> = if matches!(op, Muluh | Mulsh | Mul16u | Mul16s | Bany | Bnone | Ball | Bnall) {
                values.iter().flat_map(|&left| values.iter().map(move |&right| (left, right))).collect()
            } else {
                // Distinct halves expose reversed SRC concatenation and extension errors.
                values.iter().map(|&right| (!right, right)).collect()
            };
            for (left, right) in pairs {
                let counts: Vec<u32> = if matches!(op, Sra | Src) {
                    (0..=64).chain([127, u32::MAX]).collect()
                } else { vec![0] };
                for sar in counts {
                    compare(&mut [insn(Nop), arithmetic, insn(Nop)], Case { seed: 15, budget: 3, ..Case::default() }, |c| {
                            c.sar = sar;
                            c.set_ar(4, left);
                            c.set_ar(5, right);
                        });
                    tests += 1;
                }
            }
        }
        // A low-register prefix completes before an overflow at the new instruction.
        let mut prefix = insn(Movi);
        prefix.insn.t = 1;
        prefix.max_ar = crate::exec::max_ar(&prefix.insn);
        // Cover both whole-block and checked execution, including pre-instruction traps.
        for entry in 0..3 {
            for budget in 0..=3 {
                for overflow in [false, true] {
                    for loop_end in [false, true] {
                        compare(&mut [prefix, insn(op), insn(Nop)], Case { seed: 15, entry, budget, loop_end, overflow, ..Case::default() }, |c| {
                                c.sar = 32;
                                c.set_ar(4, 0x8000_0000);
                                c.set_ar(5, 0xffff_ffff);
                            });
                        tests += 1;
                    }
                }
            }
        }
    }
    if part != 0 { return tests; }
    // This opcode remains deliberately outside production admission.
    let mut cc = CodeCache::new(0).unwrap();
    assert!(compile(&mut cc, &mut [insn(Add), insn(Nsa), insn(Xor)], BASE, false).is_none());
    tests
}

pub(super) fn basic_ops() -> u32 {
    use Op::*;
    let ops = [
        Nop, NopN, Memw, Extw, Movi, MoviN, Mov, MovN, Add, AddN, Sub, And, Or, Xor, Mull, Salt,
        Saltu, Addi, AddiN, Addmi, Addx2, Addx4, Addx8, Subx2, Subx4, Subx8, Neg, Slli, Srli, Srai,
        Sll, Srl, Extui, Sext, Ssr, Ssl, Ssa8l, Ssa8b, Ssai, Abs, Sra, Src, Muluh, Mulsh, Nsa, Min, Max, Minu, Maxu, Moveqz, Movnez,
        Movltz, Movgez, Nsau, J, Jx, Beqz, BeqzN, Bnez, BnezN, Bltz, Bgez, Beqi, Bnei, Blti, Bgei,
        Bltui, Bgeui, Beq, Bne, Blt, Bge, Bltu, Bgeu, Bbci, Bbsi, Bbc, Bbs,
        Mul16u, Mul16s, Bany, Bnone, Ball, Bnall,
    ];
    let mut tests = 0;
    for op in ops {
        for seed in [0, 1, 15, 0xffff_ffff] {
            for entry in 0..3 {
                for budget in 1..=3 {
                    let mut block = [insn(Add), insn(op), insn(Xor)];
                    compare(&mut block, Case { seed, entry, budget, ..Case::default() }, |_| {});
                    tests += 1;
                }
            }
        }
    }
    tests
}

pub(super) fn division() -> u32 {
    use Op::*;
    let mut tests = 0;
    // Division: ordinary operands, zero divisors (the interpreter raises DIVIDE_BY_ZERO), and
    // INT_MIN by -1, which wraps where wasm's i32.div_s would trap.
    for op in [Quou, Quos, Remu, Rems] {
        for (dividend, divisor) in [(7, 2), (0xffff_fff9, 2), (5, 0), (0, 0), (0x8000_0000, 0xffff_ffff), (0x8000_0000, 1), (u32::MAX, u32::MAX), (0x7fff_ffff, 0xffff_fffe)] {
            for budget in 1..=3 {
                let mut block = [insn(Add), insn(op), insn(Xor)];
                compare(&mut block, Case { seed: 15, budget, ..Case::default() }, |c| {
                    c.set_ar(4, dividend);
                    c.set_ar(5, divisor);
                });
                tests += 1;
            }
        }
    }
    tests
}
