use super::*;

pub(super) fn special_register_blocks() -> u32 {
    use crate::state::sr;
    let mut tests = 0;
    let exact = [sr::PS, sr::PRID, sr::SCOMPARE1, sr::INTENABLE, sr::VECBASE,
        sr::CPENABLE, sr::EXCCAUSE, sr::EXCVADDR, sr::DEPC];
    for number in exact.into_iter().chain(177..=183).chain(194..=199).chain(209..=215).chain(244..=247) {
        let mut block = [insn(Op::Add), insn(Op::Rsr), insn(Op::Xor)];
        block[1].insn.imm = number as i32;
        let mut cc = CodeCache::new(0).unwrap();
        assert!(compile(&mut cc, &mut block, BASE, false).is_some());
        for entry in 0..3 {
            for budget in 1..=3 {
                compare(&mut block, Case { entry, budget, ..Case::default() }, |c| {
                        c.write_sr(number, 0xab00_0000 | number).unwrap();
                        c.prid = 0x1234_5678;
                    });
                tests += 1;
            }
        }
    }
    // Time-accounted registers must still start their own interpreter block.
    for number in [sr::CCOUNT, sr::INTERRUPT, sr::ICOUNT, 240, 241, 242] {
        let mut block = [insn(Op::Add), insn(Op::Rsr)];
        block[1].insn.imm = number as i32;
        let mut cc = CodeCache::new(0).unwrap();
        assert!(compile(&mut cc, &mut block, BASE, false).is_none());
        assert!(crate::block::must_start_block(&block[1].insn));
        tests += 1;
    }
    for op in [Op::Wsr, Op::Xsr] {
        for number in [sr::PS, sr::INTENABLE, sr::WINDOWBASE, sr::WINDOWSTART,
            sr::LBEG, sr::LEND, sr::LCOUNT, sr::SAR, sr::CPENABLE, sr::VECBASE,
            sr::SCOMPARE1, 177, 194, 209, 244, 255] {
            let mut block = [insn(Op::Add), insn(Op::MovN), insn(op)];
            block[2].insn.imm = number as i32;
            for wb in [0, 15] {
                for entry in 0..3 {
                    for budget in [1, 3, 12] {
                        compare(&mut block, Case { seed: wb, entry, budget, loop_end: true, ..Case::default() }, |c| {
                                let value = if number == sr::LEND { BASE + 9 } else { 9 };
                                c.set_ar(4, value);
                                c.set_ar(5, value);
                            });
                        tests += 1;
                    }
                }
            }
        }
    }
    for op in [Op::Rsil, Op::Rsync, Op::Esync, Op::Dsync] {
        let mut block = [insn(Op::Add), insn(Op::MovN), insn(op)];
        for entry in 0..3 {
            for budget in 1..=3 {
                compare(&mut block, Case { seed: 15, entry, budget, loop_end: true, ..Case::default() }, |_| {});
                tests += 1;
            }
        }
    }
    tests
}

/// helpers-s1: the compiled RSIL / WSR PS / XSR PS terminals. Sweeps the PS write mask, the old
/// value returned in AR[t], and a hardware loop whose end is the terminal's own fall-through, so
/// the backedge has to come out of generated code instead of the helper's `exec_insn`.
pub(super) fn ps_terminals() -> u32 {
    use Op::*;
    let mut tests = 0;
    for op in [Rsil, Wsr, Xsr] {
        for (ps0, value) in [(0u32, 0u32), (0x1f, 9), (ps::WOE | 3, 0x0007_ff3f), (0x1f, 0xffff_ffff), (ps::WOE, ps::WOE | 15)] {
            for &level in if op == Rsil { &[0, 3, 15][..] } else { &[0][..] } {
                let mut block = [insn(Add), insn(MovN), insn(op)];
                block[2].insn.imm = if op == Rsil { level } else { crate::state::sr::PS as i32 };
                assert!(emitter::supported_insn(&block[2].insn, false), "{op:?} must be admitted");
                for lend in [0, BASE + 6, BASE + 9] {
                    for entry in 0..3 {
                        for budget in [1, 3] {
                            let configure = |c: &mut Cpu| {
                                c.ps = ps0;
                                c.windowstart = 1 << c.windowbase;
                                c.set_ar(4, value);
                                c.set_ar(5, value);
                                c.lbeg = BASE;
                                c.lend = lend;
                                c.lcount = 2 * u32::from(lend != 0);
                            };
                            let case = Case { seed: 15, entry, budget, ..Case::default() };
                            for hint in [lend, 0] {
                                let before = PS_INLINE_TAKEN.load(std::sync::atomic::Ordering::Relaxed);
                                compare_hinted(&mut block, case, &configure, hint);
                                if entry == 2 {
                                    assert!(PS_INLINE_TAKEN.load(std::sync::atomic::Ordering::Relaxed) > before,
                                        "{op:?} PS={ps0:x} hint={hint:x} must execute inline");
                                }
                            }
                            tests += 2;
                        }
                    }
                }
            }
        }
    }
    tests
}

pub(super) fn terminal_helpers() -> u32 {
    use Op::*;
    let mut tests = 0;
    for op in [Call0, Call4, Call8, Call12, Callx0, Callx4, Callx8, Callx12, Ret, RetN, Retw, RetwN] {
        let mut block = [insn(Add), insn(MovN), insn(op)];
        if matches!(op, Callx0 | Callx4 | Callx8 | Callx12) {
            block[2].insn.s = match op { Callx0 => 0, Callx4 => 4, Callx8 => 8, _ => 12 };
            block[2].max_ar = crate::exec::max_ar(&block[2].insn);
        }
        // Dirty the implicit return register: the helper must see its spilled value,
        // and helper writes/window rotations must not be overwritten after it returns.
        block[1].insn.t = 0;
        block[1].max_ar = crate::exec::max_ar(&block[1].insn);
        let mut cc = CodeCache::new(0).unwrap();
        assert!(compile(&mut cc, &mut block, BASE, false).is_some());
        assert!(compile(&mut cc, &mut [insn(op), insn(Add)], BASE, false).is_none());
        assert!(compile(&mut cc, &mut [insn(op)], BASE, false).is_none());
        for wb in [0, 7, 15] {
            for flags in [0, ps::WOE, ps::WOE | ps::EXCM] {
                for windows in [0, 1 << 2, 0xffff] {
                    for inc in 0..4 {
                        for entry in 0..3 {
                            for budget in 1..=3 {
                                compare(&mut block, Case { seed: wb, entry, budget, ..Case::default() }, |c| {
                                        c.ps = flags;
                                        c.windowbase = wb;
                                        c.windowstart = windows;
                                        let ret = (inc << 30) | ((BASE + 0x400) & 0x3fff_ffff);
                                        c.set_ar(0, ret);
                                        c.set_ar(4, ret);
                                    });
                                tests += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    tests
}

/// helpers-s2: the guarded inline RETW / RETW.N. Sweeps every call increment against a window
/// where only the returned-into frame is live, plus WOE clear, A0 without an increment and the
/// underflow, and a hardware loop ending on the return, which a taken transfer must not take.
pub(super) fn windowed_return() -> u32 {
    use Op::*;
    let mut tests = 0;
    for op in [Retw, RetwN] {
        let mut block = [insn(Add), insn(MovN), insn(op)];
        // Dirty A0 before the return reads it: the spill must use the pre-rotation window.
        block[1].insn.t = 0;
        block[1].max_ar = crate::exec::max_ar(&block[1].insn);
        assert!(emitter::supported_insn(&block[2].insn, false), "{op:?} must be admitted");
        for wb in [0, 7, 15] {
            for flags in [0, ps::WOE, ps::WOE | ps::EXCM] {
                for inc in 0..4u32 {
                    for windows in [0, 0xffff, 1 << ((wb + 16 - inc) % 16)] {
                        for lend in [0, BASE + 9] {
                            for entry in 0..3 {
                                for budget in [1, 3] {
                                    let configure = |c: &mut Cpu| {
                                        c.ps = flags;
                                        c.windowbase = wb;
                                        c.windowstart = windows;
                                        let ret = (inc << 30) | ((BASE + 0x400) & 0x3fff_ffff);
                                        c.set_ar(0, ret);
                                        c.set_ar(4, ret);
                                        c.lbeg = BASE;
                                        c.lend = lend;
                                        c.lcount = 2 * u32::from(lend != 0);
                                    };
                                    let case = Case { seed: wb, entry, budget, ..Case::default() };
                                    let before = RETW_INLINE_TAKEN.load(std::sync::atomic::Ordering::Relaxed);
                                    compare_hinted(&mut block, case, &configure, lend);
                                    if entry == 2 {
                                        let inline = flags & ps::WOE != 0 && inc != 0
                                            && windows & (1 << ((wb + 16 - inc) % 16)) != 0;
                                        assert_eq!(RETW_INLINE_TAKEN.load(std::sync::atomic::Ordering::Relaxed) > before, inline,
                                            "{op:?} flags={flags:x} inc={inc} windows={windows:x}: inline guard outcome");
                                    }
                                    tests += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    tests
}

pub(super) fn whole_block_guards() -> u32 {
    let mut tests = 0;
    for offset in [-3i32, 0, 1, 3, 4, 6, 9, 10, 12] {
        for count in [0, 1, 0xffff_ffff] {
            for flags in [0, ps::WOE, ps::WOE | ps::EXCM] {
                for windows in [0, 0xffff] {
                    for entry in 0..3 {
                        for budget in 1..=3 {
                            compare(&mut [insn(Op::Add), insn(Op::MovN), insn(Op::Xor)], Case { seed: 15, entry, budget, ..Case::default() }, |c| {
                                    c.lend = BASE.wrapping_add(offset as u32);
                                    c.lbeg = BASE + 0x100;
                                    c.lcount = count;
                                    c.ps = flags;
                                    c.windowstart = windows;
                                });
                            tests += 1;
                        }
                    }
                }
            }
        }
    }
    tests
}

pub(super) fn window_masks() -> u32 {
    let mut cc = CodeCache::new(0).unwrap();
    let mut cases = 0;
    for high in [3, 7, 11, 15] {
        let mut low = insn(Op::Movi);
        low.insn.t = 1;
        low.max_ar = 1;
        let mut upper = insn(Op::Add);
        upper.insn.r = high;
        upper.insn.s = 2;
        upper.insn.t = 3;
        upper.max_ar = crate::exec::max_ar(&upper.insn);
        let mut block = [low, upper];
        let id = queue(&mut cc, &mut block, BASE, false);
        for _ in 0..HOT {
            ready(&cc, id, 0);
        }
        for wb in 0..16 {
            for frame in 1..=3 {
                for status in [0, ps::WOE, ps::WOE | ps::EXCM] {
                    for entry in 0..2 {
                        for budget in 1..=2 {
                            let (mut a, mut b) = (cpu(wb), cpu(wb));
                            for c in [&mut a, &mut b] {
                                c.pc = BASE + entry * 3;
                                c.ps = status;
                                c.windowstart = 1 << ((wb + frame) & 15);
                            }
                            let (mut ra, mut rb) = (Ram::new(false, false), Ram::new(false, false));
                            let actual = unsafe {
                                run(
                                    &cc,
                                    id,
                                    &mut b,
                                    &mut rb,
                                    &Helpers::new::<Ram>(),
                                    budget,
                                    entry,
                                    None,
                                )
                            };
                            let (mut done, mut trap) = (0, None);
                            for bi in block.iter().skip(entry as usize).take(budget as usize) {
                                if let Some(t) = a.check_overflow(bi.max_ar) {
                                    trap = Some(t);
                                    break;
                                }
                                exec_insn(&mut a, &mut ra, &bi.insn).unwrap();
                                done += 1;
                            }
                            assert_eq!(actual & 0xffff, done);
                            assert_eq!(trap, b.jit_trap.take());
                            same(&a, &b);
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    cases
}

pub(super) fn entry_and_shifts_part(part: usize) -> u32 {
    use Op::*;
    let mut tests = 0;
    for (index, op) in [Sll, Srl].into_iter().enumerate() {
        if index != part { continue; }
        for sar in (0..=64).chain([127, u32::MAX]) {
            for value in [0, 1, 0x8000_0000, 0xffff_ffff, 0xa5a5_5a5a] {
                for alias in [false, true] {
                    let mut shift = insn(op);
                    if alias { shift.insn.r = if op == Sll { 4 } else { 5 }; }
                    shift.max_ar = crate::exec::max_ar(&shift.insn);
                    let mut block = [insn(Nop), shift, insn(Xor)];
                    let mut cc = CodeCache::new(0).unwrap();
                    assert!(compile(&mut cc, &mut block, BASE, false).is_some());
                    compare(&mut block, Case { seed: 15, budget: 3, ..Case::default() }, |c| {
                            c.sar = sar;
                            c.set_ar(4, value);
                            c.set_ar(5, value);
                        });
                    tests += 1;
                }
            }
        }
    }
    for (wi, wb) in [0, 14, 15].into_iter().enumerate() {
        for (fi, flags) in [0, ps::WOE, ps::WOE | ps::EXCM].into_iter().enumerate() {
            if 2 + wi * 3 + fi != part { continue; }
            // The +4 frame is outside the initial a15 guard, but can collide
            // after ENTRY rotates. This catches reuse of the whole-block proof.
            for windows in [0, 1 << ((wb + 4) & 15), 0xffff] {
                for inc in 0..4 {
                    for s in [0, 1, 3, 4] {
                        let mut prefix = insn(Movi);
                        prefix.insn.t = 1;
                        prefix.insn.imm = -1;
                        let mut enter = insn(Entry);
                        enter.insn.s = s;
                        enter.insn.imm = 32;
                        let mut upper = insn(Add);
                        upper.insn.r = 15;
                        let mut block = [prefix, enter, upper, enter, insn(Xor)];
                        for bi in &mut block { bi.max_ar = crate::exec::max_ar(&bi.insn); }
                        let mut cc = CodeCache::new(0).unwrap();
                        assert!(compile(&mut cc, &mut block, BASE, false).is_some());
                        for entry in 0..5 {
                            for budget in 0..=5 {
                                compare(&mut block, Case { seed: wb, entry, budget, ..Case::default() }, |c| {
                                        c.ps = flags | (inc << ps::CALLINC_SHIFT);
                                        c.windowstart = windows;
                                        // Alternate active loop ends directly after ENTRY.
                                        c.lcount = inc & 1;
                                        c.lend = BASE + 6;
                                        c.lbeg = BASE;
                                    });
                                tests += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    tests
}

pub(super) fn helper_continuation() -> u32 {
    use Op::*;
    let mut tests = 0;
    for overflow in [false, true] {
        for entry in 0..3 {
            // Keep an unsupported opcode here to exercise helper continuation.
            compare(&mut [insn(Add), insn(Nsa), insn(Xor)],
                Case { seed: 15, entry, budget: 3, loop_end: true, overflow, ..Case::default() }, |_| {});
            tests += 1;
        }
    }
    tests
}

// Assert the generated path itself, so silently falling back cannot pass this oracle.
pub(super) fn guarded_loop_sites() -> u32 {
    let mut tests = 0;
    for site in [1, 2, 3] {
        for entry in 0..3 {
            for budget in [1, 2, 3, 8] {
                let mut block = [insn(Op::Add), insn(Op::Xor), insn(Op::Add)];
                let lend = BASE + site * 3;
                let configure = |c: &mut Cpu| { c.ps = 0; c.lend = lend; c.lbeg = BASE; c.lcount = 2; };
                let case = Case { entry, budget, ..Case::default() };
                assert!(compare_hinted(&mut block, case, &configure, lend), "guarded site={site} entry={entry} budget={budget}");
                assert!(!compare_hinted(&mut block, case, &configure, 0), "checked site={site} entry={entry} budget={budget}");
                tests += 2;
            }
        }
    }
    tests
}

pub(super) fn pie_wide_shifts() -> u32 {
    use crate::pie::Role::{Qa, Qs};
    let mut tests = 0;
    for name in ["ee.vsr.32", "ee.vsl.32"] {
        let bytes = asm::pie(name, &[(Qa, 1), (Qs, 0)]);
        let raw = bytes[0] as u32 | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16);
        let mut shift = insn(Op::Pie);
        shift.insn = crate::decode::decode(BASE + 3, raw.to_le_bytes());
        for sar in 33..64 {
            let mut block = [insn(Op::Nop), shift, insn(Op::Xor)];
            for entry in 0..=1 {
                for budget in [1, 3] {
                    compare(&mut block, Case { entry, budget, ..Case::default() }, |c| {
                        c.ps = 0; c.cpenable = 8;
                        c.write_sr(crate::state::sr::SAR, sar).unwrap();
                        c.qr[0] = u128::MAX;
                    });
                    tests += 1;
                }
            }
        }
    }
    tests
}

/// Exercise admission and the bridge itself, so falling back cannot hide missing coverage.
pub(super) fn interpreted_bridges() -> u32 {
    use Op::*;
    let mut cases = 0;
    for op in [Entry, Loop, Loopnez, Loopgtz, Ret, RetN, Retw, RetwN, Rsr, Add, Quos] {
        for trap in [false, true] {
            let mut ops = [insn(Mul16u), insn(op), insn(Add)];
            for b in &mut ops { b.insn.r = 1; b.insn.s = 2; b.insn.t = 3; }
            if op == Rsr { ops[1].insn.imm = crate::state::sr::PS as i32; }
            if op == Entry { ops[1].insn.s = 1; ops[1].insn.imm = 16; }
            if trap && op == Add { ops[1].insn.r = 8; }
            for b in &mut ops { b.max_ar = crate::exec::max_ar(&b.insn); }
            let mut a = cpu(0);
            a.ps = ps::WOE | (1 << ps::CALLINC_SHIFT);
            a.windowstart = if trap && op == Add { 1 | (1 << 2) } else { 0xffff };
            // No overflow in the ordinary cases; RETW underflow is selected independently.
            if op != Add || !trap { a.windowstart = if matches!(op, Retw | RetwN) && trap { 1 } else { 1 | (1 << 15) }; }
            a.set_ar(0, (1 << 30) | ((BASE + 0x100) & 0x3fff_ffff));
            a.set_ar(1, BASE + 0x200);
            a.set_ar(2, 3);
            a.set_ar(3, if trap && op == Quos { 0 } else { 2 });
            let mut b = a.clone();
            let (mut ra, mut rb) = (Ram::new(false, false), Ram::new(false, false));
            b.blocks.install_test_bridge(BASE, &ops);
            let (start, n) = b.blocks.bridge_target(BASE, rb.page_versions(), 3).unwrap();
            let result = crate::block::bridge(&mut b, &mut rb, start, n);
            a.blocks.install_test_bridge(BASE, &ops);
            a.blocks.jit_enabled = false;
            a.blocks.observed = true; // one decoded block, without continuation
            let (iterations, trap_ref) = crate::block::run_block(&mut a, &mut ra, 3);
            let done = result & 0xffff;
            let exit = result >> 16;
            let pre = exit == CODE_TRAP_PRE;
            assert_eq!(exit, if trap_ref.is_none() { CODE_END } else if pre { CODE_TRAP_PRE } else { CODE_TRAP });
            assert_eq!(iterations, done + u32::from(pre), "bridge {op:?} trap={trap}");
            assert_eq!(b.jit_trap, trap_ref);
            assert_eq!(b.blocks.bridged, done);
            b.insn_count += done as u64;
            b.advance_ccount(done * b.approximate_cpi);
            same(&a, &b);
            assert_eq!(ra.noted, rb.noted);
            if op == Add { assert_eq!(result, if trap { 1 | CODE_TRAP_PRE << 16 } else { 3 | CODE_END << 16 }); }
            if op == Quos && trap { assert_eq!(result, 2 | CODE_TRAP << 16); }
            if matches!(op, Retw | RetwN) && trap { assert_eq!(result, 2 | CODE_TRAP << 16); }
            cases += 1;
        }
    }
    cases
}

pub(super) fn bridge_classes() -> u32 {
    use Op::*;
    // Every currently supported word_access opcode, including indexed floating
    // accesses and atomic/synchronized forms, plus dispatcher-visible controls.
    let memory = [L32i, L32iN, L32ai, S32i, S32iN, S32ri, S32nb, L32e,
        S32e, S32c1i, Lsi, Lsip, Ssi, Ssip, Lsx, Lsxp, Ssx, Ssxp, L32r];
    let c = cpu(0);
    for op in memory {
        let i = insn(op).insn;
        assert!(crate::exec::word_access(&c, &i).is_some(), "{op:?}");
        let class = crate::block::bridge_class(&i);
        assert!(class == 0 || class == 3, "{op:?}: {class}");
        assert!(class == 0 || class > crate::block::BRIDGE_CLASS, "memory admitted: {op:?}");
    }
    for op in [Wsr, Xsr, Rsil] {
        assert_eq!(crate::block::bridge_class(&insn(op).insn), 0, "{op:?}");
    }
    22
}

/// gen-s1: an own module reloading and spilling every register quad, at every window position
/// (WINDOWBASE 13..15 wrap the 64-register file inside the window), whole, resumed and cut.
pub(super) fn window_quads() -> u32 {
    let mut block = [insn(Op::Add), insn(Op::Xor), insn(Op::Sub), insn(Op::Add)];
    for (bi, (r, s, t)) in block.iter_mut().zip([(15, 0, 4), (9, 14, 1), (3, 12, 7), (0, 8, 15)]) {
        (bi.insn.r, bi.insn.s, bi.insn.t) = (r, s, t);
        bi.max_ar = crate::exec::max_ar(&bi.insn);
    }
    let mut cases = 0;
    for wb in 0..16 {
        for entry in 0..3 {
            for budget in [1, 2, 4, 9] {
                compare(&mut block, Case { seed: 5, entry, budget, ..Case::default() }, |c| { c.windowbase = wb; c.windowstart = 1 << wb; });
                cases += 1;
            }
        }
    }
    cases
}
