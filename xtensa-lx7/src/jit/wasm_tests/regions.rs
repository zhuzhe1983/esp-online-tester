use super::*;

/// Run `program` through the block scheduler with regions and compare against
/// instruction-by-instruction execution under varying credit, probes, code changes
/// and timer deadlines. Returns the largest single-call retirement, which proves that
/// a region ran past its head block.
fn region_program(name: &str, program: &[u8], expected: &[(u32, Op, u32)], data: &[u8], head_len: u32, interior: u32, setup: impl Fn(&mut Cpu), turns: usize) -> u32 {
    region_program_on(name, program, expected, data, head_len, interior, false, false, setup, turns)
}
fn region_program_on(name: &str, program: &[u8], expected: &[(u32, Op, u32)], data: &[u8], head_len: u32, interior: u32, readonly: bool, unwatched: bool, setup: impl Fn(&mut Cpu), turns: usize) -> u32 {
    let (mut a, mut b) = (cpu(3), cpu(3));
    let (mut ra, mut rb) = (Ram::new(true, readonly), Ram::new(true, readonly));
    for r in [&mut ra, &mut rb] {
        r.ram.mem[..program.len()].copy_from_slice(program);
        r.ram.mem[0x1000..0x1000 + data.len()].copy_from_slice(data);
        // store-s1: a mapping no decoded code depends on, even after the region forms.
        if unwatched { r.unwatch(); r.pinned = true; }
    }
    CONTEXT.with(|c| *c.borrow_mut() = format!("region program {name}"));
    for &(off, op, target) in expected {
        let i = crate::decode::decode(BASE + off, ra.fetch(BASE + off).unwrap());
        assert_eq!(i.op, op, "{name}: encoding at +{off}: {}", crate::disasm::format(&i));
        if target != 0 { assert_eq!(i.imm as u32, BASE + target, "{name}: target at +{off}: {}", crate::disasm::format(&i)); }
    }
    for c in [&mut a, &mut b] {
        c.pc = BASE;
        c.ps = 0;
        setup(c);
    }
    let mut max_done = 0;
    for turn in 0..turns {
        let budget = 1 + (turn * 7) as u32 % 71;
        let head_probe = (40..45).contains(&(turn % 50));
        for c in [&mut a, &mut b] {
            // A probe on an interior chunk head must stop the region without a flush;
            // a probe on the head itself must stop internal backedges to it.
            if turn % 50 == 25 { c.boundary_bloom = emu_core::core::pc_bit(BASE + interior); }
            if turn % 50 == 35 { c.boundary_bloom = 0; }
            if turn % 50 == 40 { c.boundary_bloom = emu_core::core::pc_bit(BASE); }
            if turn % 50 == 45 { c.boundary_bloom = 0; }
            // A timer deadline inside the region must land on the same instruction.
            if turn % 90 == 60 { c.ccompare[0] = c.ccount.wrapping_add(1 + (turn % 13) as u32); c.refresh_event(); c.intenable = 1 << 6; }
        }
        // rename-s1: once warm, start the driver with no caller frame live: the wrapper's RETW must
        // take WINDOW_UF8 although its nested leaf returned inline.
        if name == "leaf-calls-underflow" && turn >= 600 {
            for c in [&mut a, &mut b] {
                c.pc = BASE + 256;
                c.ps = ps::WOE;
                c.windowbase = 0;
                c.windowstart = 0;
            }
        }
        let start = b.pc;
        CONTEXT.with(|c| *c.borrow_mut() = format!("region program {name} turn {turn} start {start:x} budget {budget}"));
        let (done, trap) = crate::block::run_block(&mut b, &mut rb, budget);
        assert!(done <= budget, "{name}: {done} > budget {budget}");
        if head_probe && start == BASE { assert!(done <= head_len, "{name}: region ran through a probed head ({done})"); }
        max_done = max_done.max(done);
        let mut oracle = None;
        for _ in 0..done {
            ra.note_pc(a.pc);
            if let Err(t) = crate::step(&mut a, &mut ra) { oracle = Some(t); break; }
        }
        assert_eq!(trap, oracle, "{name}: turn {turn} budget {budget}");
        same(&a, &b);
        assert_eq!(ra.ram.mem, rb.ram.mem, "{name}: memory after turn {turn}");
        assert_eq!(ra.versions, rb.versions, "{name}: versions after turn {turn}");
        if done > 0 && trap.is_none() { assert_eq!(ra.noted, rb.noted, "{name}: noted PC after turn {turn}"); }
        if let Some(Trap::Interrupt(_)) = trap {
            // Return from the interrupt by hand: both continue at the interrupted PC.
            for c in [&mut a, &mut b] { c.pc = c.epc[1]; c.ps &= !ps::EXCM; c.interrupt = 0; c.ccompare[0] = 0; c.refresh_event(); }
        }
    }
    max_done
}

pub(super) fn regions() -> u32 {
    use Op::*;
    let mut cases = 0;
    // A PS-writing leaf exits the region so interrupt/window proofs are rebuilt.
    // Count only region lowerings: hot standalone blocks cannot satisfy this check.
    for (op, word) in [(Rsil, 0x006030u32), (Wsr, 0x130000 | (crate::state::sr::PS << 8) | 0x30),
        (Xsr, 0x610000 | (crate::state::sr::PS << 8) | 0x30)] {
        let mut p = asm::addi_n(2, 2, 1);
        p.extend(asm::bz(1, BASE + 2, 2, BASE + 8));
        p.extend(asm::j(BASE + 5, BASE + 11));
        p.extend([word as u8, (word >> 8) as u8, (word >> 16) as u8]);
        p.extend(asm::movi_n(2, 0));
        p.extend(asm::j(BASE + 13, BASE));
        for flags in [0, ps::WOE] {
            let before = PS_REGION_TAKEN.load(std::sync::atomic::Ordering::Relaxed);
            region_program("PS-terminal-leaf", &p,
                &[(0, AddiN, 0), (2, Bnez, 8), (5, J, 11), (8, op, 0), (11, MoviN, 0), (13, J, 0)],
                &[], 2, 8, |c| {
                    c.ps = flags;
                    c.windowstart = 1 << c.windowbase;
                    c.set_ar(2, 0);
                    c.set_ar(3, flags);
                }, 600);
            assert!(PS_REGION_TAKEN.load(std::sync::atomic::Ordering::Relaxed) > before,
                "{op:?} PS={flags:x} must execute inline inside a region");
            cases += 1;
        }
    }
    // Forty non-contiguous chunks exercise a large br_table and five version pages.
    // Enter every chunk with both short credit and hundreds of instructions of credit.
    let mut large = vec![0; 40 * 32];
    for k in 0..40 {
        let mut chunk = Vec::new();
        for _ in 0..6 { chunk.extend(asm::addi_n(2, 2, 1)); }
        chunk.extend(asm::s8i(3, 4, 0));
        chunk.extend(asm::j(BASE + (k * 32 + 15) as u32, BASE + ((k + 1) % 40 * 32) as u32));
        large[k * 32..k * 32 + chunk.len()].copy_from_slice(&chunk);
    }
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..large.len()].copy_from_slice(&large);
        let c = cpu(0);
        let head: Vec<BlockInsn> = (0..8).scan(BASE, |pc, _| {
            let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap());
            *pc += i.len as u32;
            Some(BlockInsn { insn: i, max_ar: crate::exec::max_ar(&i), straddle: false, off: 0 })
        }).collect();
        let formed = emitter::region::form(&c, &mut ram, BASE, &head, true).expect("large region");
        assert_eq!(formed.chunks.len(), 40);
        assert_eq!(formed.chunks.iter().map(|c| c.instructions.len()).sum::<usize>(), 320);
        assert_eq!(formed.pages.len(), 5);
        // tails-s1: a counting region, guarded copies of every chunk, and of every other chunk.
        let all: Vec<usize> = (0..40).collect();
        let even: Vec<usize> = (0..40).step_by(2).collect();
        for copies in [None, Some(&all[..]), Some(&even[..])] {
        let (bytes, sites) = emitter::region::generate(&formed.chunks, &formed.pages, &formed.loops, &formed.leaves, true, copies);
        let slot = unsafe { host_jit_compile(bytes.as_ptr(), bytes.len()) };
        assert_ne!(slot, 0, "large region module with {} exit sites", sites.len());
        type Run = extern "C" fn(*mut Cpu, *mut Ram, *const Helpers, u32, u32, *const TlbEntry, *mut u32) -> u32;
        let f: Run = unsafe { std::mem::transmute(slot as usize) };
        for (entry, chunk) in formed.chunks.iter().enumerate() {
            // Budgets 9..15 cut a copy of the next chunk at every one of its indices; 8 and 16
            // land exactly on a chunk end.
            for budget in [8, 9, 10, 11, 12, 13, 14, 15, 16, 23, 63, 300, 511] {
                for dst in [BASE + 0x2000, BASE + 4 * 256 + 31] {
                    let (mut a, mut b) = (cpu(0), cpu(0));
                    let (mut ra, mut rb) = (Ram::new(true, false), Ram::new(true, false));
                    for r in [&mut ra, &mut rb] { r.ram.mem[..large.len()].copy_from_slice(&large); }
                    for c in [&mut a, &mut b] {
                        c.pc = chunk.pc; c.ps = 0;
                        c.set_ar(3, 0x42); c.set_ar(4, dst);
                    }
                    CONTEXT.with(|c| *c.borrow_mut() = format!("large region entry {entry} budget {budget} dst {dst:x} copies {:?}", copies.map(|c| c.len())));
                    let fm = rb.fast_mem().unwrap();
                    let result = f(&mut b, &mut rb, &Helpers::new::<Ram>(), budget, entry as u32, fm.tlb, fm.page_ver);
                    let done = result & 0xffff;
                    assert!(done > 0 && done <= budget);
                    if dst == BASE + 0x2000 && budget >= 300 { assert!(done >= 296, "large DONE credit: {done}"); }
                    // No store lands in the region's own pages: with every chunk copied the whole
                    // credit is spent, and a counting region reports each unspent exit as SHORT.
                    if dst == BASE + 0x2000 && copies.is_some_and(|c| c.len() == 40) { assert_eq!(done, budget); }
                    if dst == BASE + 0x2000 && copies.is_none() && done < budget { assert_eq!((result >> 16) & 7, CODE_SHORT); }
                    for _ in 0..done {
                        let i = crate::decode::decode(a.pc, ra.fetch(a.pc).unwrap());
                        exec_insn(&mut a, &mut ra, &i).unwrap();
                    }
                    same(&a, &b);
                    assert_eq!(ra.ram.mem, rb.ram.mem);
                    assert_eq!(ra.versions, rb.versions);
                    cases += 1;
                }
            }
        }
        // tails-s2: resume at every index of every copied chunk. The entry parameter carries the
        // copy's dispatch index and the index to start at; no store lands in the region's own
        // pages, so with every chunk copied the whole credit is spent inside one call.
        for (entry, copy) in copies.map(|c| emitter::region::copy_indices(&formed.chunks, c)).unwrap_or_default().into_iter().enumerate() {
            let Some(copy) = copy else { continue };
            let mut pc = formed.chunks[entry].pc;
            for (idx, bi) in formed.chunks[entry].instructions.iter().enumerate() {
                for budget in [1u32, 2, 7, 63].into_iter().filter(|_| idx > 0) {
                    let (mut a, mut b) = (cpu(0), cpu(0));
                    let (mut ra, mut rb) = (Ram::new(true, false), Ram::new(true, false));
                    for r in [&mut ra, &mut rb] { r.ram.mem[..large.len()].copy_from_slice(&large); }
                    for c in [&mut a, &mut b] { c.pc = pc; c.ps = 0; c.set_ar(3, 0x42); c.set_ar(4, BASE + 0x2000); }
                    CONTEXT.with(|c| *c.borrow_mut() = format!("large region resume {entry}:{idx} budget {budget}"));
                    let fm = rb.fast_mem().unwrap();
                    let result = f(&mut b, &mut rb, &Helpers::new::<Ram>(), budget, copy | ((idx as u32) << 16), fm.tlb, fm.page_ver);
                    let done = result & 0xffff;
                    assert!(done > 0 && done <= budget);
                    if copies.is_some_and(|c| c.len() == 40) { assert_eq!(done, budget); }
                    for _ in 0..done {
                        let i = crate::decode::decode(a.pc, ra.fetch(a.pc).unwrap());
                        exec_insn(&mut a, &mut ra, &i).unwrap();
                    }
                    same(&a, &b);
                    assert_eq!(ra.ram.mem, rb.ram.mem);
                    assert_eq!(ra.versions, rb.versions);
                    cases += 1;
                }
                pc = pc.wrapping_add(bi.insn.len as u32);
            }
        }
        unsafe { host_jit_release(slot) };
        }
    }
    for dst in [BASE + 0x2000, BASE + 4 * 256 + 31] {
        let max = region_program("large-region-dispatch", &large, &[], &[], 8, 32, |c| {
            c.set_ar(3, 0x42); c.set_ar(4, dst);
        }, 1200);
        assert!(dst != BASE + 0x2000 || max > 8, "large region never passed its head");
        cases += 1;
    }
    // Put LEND inside the second chunk of the same large graph: formation must split
    // it and preserve the backedge even when many dispatch targets precede the split.
    let mut loop_large = large.clone();
    let mut prefix = asm::movi_n(10, 3);
    prefix.extend(asm::lp(9, BASE + 2, 10, BASE + 38));
    for _ in 0..6 { prefix.extend(asm::addi_n(2, 2, 1)); }
    prefix.extend(asm::s8i(3, 4, 0));
    prefix.extend(asm::j(BASE + 20, BASE + 32));
    loop_large[..prefix.len()].copy_from_slice(&prefix);
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..loop_large.len()].copy_from_slice(&loop_large);
        let head: Vec<BlockInsn> = (0..2).scan(BASE, |pc, _| {
            let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32;
            Some(BlockInsn { insn: i, max_ar: crate::exec::max_ar(&i), straddle: false, off: 0 })
        }).collect();
        let formed = emitter::region::form(&cpu(0), &mut ram, BASE, &head, true).expect("large loop region");
        assert!(formed.chunks.len() > 40);
        assert!(formed.chunks.iter().any(|c| c.pc == BASE + 38));
        assert_eq!(formed.loops, vec![(BASE + 38, BASE + 5)]);
        assert_eq!(formed.pages.len(), 5);
    }
    let max = region_program("large-loop-region", &loop_large, &[], &[], 2, 5, |c| {
        c.set_ar(3, 0x42); c.set_ar(4, BASE + 0x2000);
    }, 1200);
    assert!(max > 8, "large loop region never passed its head");
    cases += 1;
    // The ROM memmove byte loop: a 6-instruction body ending in J, a one-instruction
    // BNE block branching back, then a boundary the region cannot cross.
    let mut p = Vec::new();
    p.extend(asm::add_n(9, 3, 8));       // 0  a9 = src + i
    p.extend(asm::l8ui(10, 9, 0));       // 2
    p.extend(asm::add_n(9, 2, 8));       // 5  a9 = dst + i
    p.extend(asm::s8i(10, 9, 0));        // 7
    p.extend(asm::addi_n(8, 8, 1));      // 10
    p.extend(asm::j(BASE + 12, BASE + 17)); // 12
    p.extend(asm::movi_n(8, 0));         // 15  (never executed)
    p.extend(asm::bcc(9, BASE + 17, 4, 8, BASE)); // 17 bne a4, a8, 0
    p.extend(asm::rsr(13, 234));         // 20  rsr a13, ccount: a block boundary
    p.extend(asm::movi_n(8, 0));         // 23  restart the copy
    p.extend(asm::j(BASE + 25, BASE));   // 25
    let memmove = [(0, AddN, 0), (2, L8ui, 0), (5, AddN, 0), (7, S8i, 0), (10, AddiN, 0), (12, J, 17), (17, Bne, 0), (20, Rsr, 0), (23, MoviN, 0), (25, J, 0)];
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let c = cpu(0);
        let head: Vec<BlockInsn> = (0..6).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&c, &mut ram, BASE, &head, true).expect("memmove region");
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len())).collect::<Vec<_>>(), vec![(0, 6), (17, 1)]);
        let (bytes, sites) = emitter::region::generate(&formed.chunks, &formed.pages, &formed.loops, &formed.leaves, true, Some(&[]));
        let slot = unsafe { host_jit_compile(bytes.as_ptr(), bytes.len()) };
        assert!(slot != 0, "memmove region module must compile ({} bytes, {} sites)", bytes.len(), sites.len());
        unsafe { host_jit_release(slot) };
    }
    for (src, dst, whole) in [
        (BASE + 0x1000, BASE + 0x2000, true),
        // Copying the program onto itself and over its own tail bumps the code page the
        // region was decoded from: every store sets DIRTY, so the region leaves at each
        // edge, and the dispatcher must re-validate and rebuild exactly.
        (BASE, BASE, false),
        (BASE + 0x1000, BASE + 0x10, false),
        // Slow loads and stores leave generated code through the helper in a successor chunk.
        (SLOW, BASE + 0x2000, false),
        (BASE + 0x1000, SLOW, false),
    ] {
        let max = region_program("memmove", &p, &memmove, &[], 6, 17, |c| {
            c.set_ar(2, dst); c.set_ar(3, src); c.set_ar(4, 40); c.set_ar(8, 0);
        }, 700);
        let stats: Vec<u32> = REGION_STATS.iter().map(|s| s.load(std::sync::atomic::Ordering::Relaxed)).collect();
        assert!(!whole || max >= 40, "memmove region retired at most {max} per call; formed/failed/maxdone/exits[END,LEFT,TRAP,CUT,PRE,REJ,_,_]/maxbudget {stats:?}");
        cases += 1;
    }
    // A tile scan: two chunks in a loop with a conditional exit to a third. The second
    // chunk rewrites the first immediate of the chunk it falls into, so the region
    // must leave at that edge and the rebuilt block must see the new instruction.
    let mut tp = Vec::new();
    tp.extend(asm::l16ui(11, 8, 0));                 // 0
    tp.extend(asm::bcc(9, BASE + 3, 11, 6, BASE + 35)); // 3  bne a11, a6, 35
    tp.extend(asm::addi_n(8, 8, 2));                 // 6
    tp.extend(asm::addi_n(10, 10, -1));              // 8
    tp.extend(asm::bz(1, BASE + 10, 10, BASE));      // 10 bnez a10, 0
    tp.extend(asm::movi_n(10, 8));                   // 13 reset: the immediate byte at 14 gets toggled 8 <-> 9
    tp.extend(asm::mov_n(8, 12));                    // 15
    tp.extend(asm::l8ui(14, 15, 0));                 // 17
    tp.extend(asm::xor(14, 14, 5));                  // 20
    tp.extend(asm::s8i(14, 15, 0));                  // 23 a store into the region's own code page
    tp.extend(asm::j(BASE + 26, BASE + 43));         // 26 j 43: forward, non-contiguous
    tp.extend(asm::rsr(13, 234));                    // 29 (never executed)
    tp.extend(asm::rsr(13, 234));                    // 32 (never executed)
    tp.extend(asm::bcc(1, BASE + 35, 6, 7, BASE + 41)); // 35 beq a6, a7, 41 (always taken)
    tp.extend(asm::rsr(13, 234));                    // 38 (never executed)
    tp.extend(asm::addi_n(8, 8, 2));                 // 41 skip the odd halfword
    tp.extend(asm::j(BASE + 43, BASE));              // 43 j 0
    let tile = [(0, L16ui, 0), (3, Bne, 35), (6, AddiN, 0), (8, AddiN, 0), (10, Bnez, 0), (13, MoviN, 0), (15, MovN, 0), (17, L8ui, 0), (20, Xor, 0), (23, S8i, 0), (26, J, 43), (35, Beq, 41), (38, Rsr, 0), (41, AddiN, 0), (43, J, 0)];
    // A zero buffer with one odd halfword: the scan matches until it reaches it.
    let mut data = [0u8; 32];
    data[14] = 0x34; data[15] = 0x12;
    let stat = |i: usize| REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
    let (formed0, dropped0, covered0) = (stat(0), stat(9), stat(10));
    let max = region_program("tile", &tp, &tile, &data, 2, 6, |c| {
        c.set_ar(12, BASE + 0x1000); c.set_ar(8, BASE + 0x1000); c.set_ar(10, 8);
        c.set_ar(6, 0); c.set_ar(7, 0); c.set_ar(15, BASE + 14); c.set_ar(5, 0x10);
    }, 900);
    assert!(max >= 30, "tile region retired at most {max} per call");
    // Four heads of this loop become hot, but a head inside a live region gets no region
    // of its own: only the first, plus one re-formation per code change.
    assert!(stat(10) > covered0, "no covered head was skipped");
    assert!(stat(0) - formed0 <= stat(9) - dropped0 + 1, "overlapping regions: formed {} dropped {}", stat(0) - formed0, stat(9) - dropped0);
    cases += 1;
    // A hardware loop with a branch inside its body: the LOOPNEZ, the body, the skip
    // target and the loop exit are all region chunks; the backedge is an internal edge,
    // and a credit cut inside the loop re-enters with the loop active.
    let mut p = Vec::new();
    p.extend(asm::mov_n(8, 14));                    // 0  a8 = buffer
    p.extend(asm::movi_n(10, 12));                  // 2
    p.extend(asm::lp(9, BASE + 4, 10, BASE + 17));  // 4  loopnez a10, 17
    p.extend(asm::l8ui(11, 8, 0));                  // 7
    p.extend(asm::bz(0, BASE + 10, 11, BASE + 15)); // 10 beqz a11, 15
    p.extend(asm::addi_n(9, 9, 1));                 // 13
    p.extend(asm::addi_n(8, 8, 1));                 // 15 (ends at LEND)
    p.extend(asm::addi_n(12, 12, 1));               // 17 loop exit
    p.extend(asm::rsr(13, 234));                    // 19
    p.extend(asm::j(BASE + 22, BASE));              // 22
    let hwloop = [(0, MovN, 0), (2, MoviN, 0), (4, Loopnez, 17), (7, L8ui, 0), (10, Beqz, 15), (13, AddiN, 0), (15, AddiN, 0), (17, AddiN, 0), (19, Rsr, 0), (22, J, 0)];
    let data: Vec<u8> = (0..16u8).map(|i| i & 1).collect();
    let formed_before = REGION_STATS[0].load(std::sync::atomic::Ordering::Relaxed);
    let max = region_program("hwloop", &p, &hwloop, &data, 3, 7, |c| {
        c.set_ar(14, BASE + 0x1000); c.set_ar(9, 0); c.set_ar(12, 0);
    }, 900);
    assert!(max >= 40, "hardware-loop region retired at most {max} per call");
    assert!(REGION_STATS[0].load(std::sync::atomic::Ordering::Relaxed) > formed_before, "hwloop formed no region");
    cases += 1;
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let c = cpu(0);
        let head: Vec<BlockInsn> = (0..3).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&c, &mut ram, BASE, &head, true).expect("hwloop region");
        assert_eq!(formed.loops, vec![(BASE + 17, BASE + 7)]);
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len())).collect::<Vec<_>>(),
            vec![(0, 3), (7, 2), (17, 1), (13, 2), (15, 1)]);
        cases += 1;
    }
    // Calls and returns end chunks and leave; a function entry heads a region whose
    // window proof is redone after ENTRY; RETW.N uses its guarded inline path.
    let mut p = Vec::new();
    p.extend(asm::call8(BASE, BASE + 12));          // 0  call8 F
    p.extend(asm::addi_n(2, 2, 1));                 // 3  (return address)
    p.extend(asm::addi_n(3, 3, 1));                 // 5
    p.extend(asm::j(BASE + 7, BASE));               // 7
    p.extend(asm::nop_n());                         // 10
    p.extend(asm::entry(1, 32));                    // 12 F: entry a1, 32
    p.extend(asm::movi_n(3, 5));                    // 15
    p.extend(asm::add_n(2, 2, 3));                  // 17
    p.extend(asm::bz(2, BASE + 19, 2, BASE + 24));  // 19 bltz a2, 24
    p.extend(asm::retw_n());                        // 22
    p.extend(asm::movi_n(2, 0));                    // 24
    p.extend(asm::retw_n());                        // 26
    let calls = [(0, Call8, 12), (3, AddiN, 0), (5, AddiN, 0), (7, J, 0), (10, NopN, 0), (12, Entry, 0), (15, MoviN, 0), (17, AddN, 0), (19, Bltz, 24), (22, RetwN, 0), (24, MoviN, 0), (26, RetwN, 0)];
    let formed_before = REGION_STATS[0].load(std::sync::atomic::Ordering::Relaxed);
    let max = region_program("calls", &p, &calls, &[], 1, 12, |c| {
        c.ps = ps::WOE; c.windowstart = 1 << c.windowbase;
        c.set_ar(10, (-100i32) as u32); c.set_ar(2, 0); c.set_ar(3, 0);
    }, 900);
    assert!(max >= 4, "call region retired at most {max} per call");
    assert!(REGION_STATS[0].load(std::sync::atomic::Ordering::Relaxed) >= formed_before + 2, "call/entry regions did not form");
    cases += 1;
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let c = cpu(0);
        let head: Vec<BlockInsn> = (0..4).scan(BASE + 12, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&c, &mut ram, BASE + 12, &head, true).expect("entry region");
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len())).collect::<Vec<_>>(),
            vec![(12, 4), (22, 1), (24, 2)]);
        assert!(emitter::region::form(&c, &mut ram, BASE, &head[..1], true).is_none(), "a lone call is not a region");
        cases += 1;
    }
    // A malformed `entry a4` heading a region: its own operand must still be proved free
    // before the interpreter helper runs it, so an occupied frame raises the overflow.
    let mut p = Vec::new();
    p.extend(asm::entry(4, 32));                    // 0
    p.extend(asm::bz(1, BASE + 3, 2, BASE + 9));    // 3  bnez a2, 9
    p.extend(asm::j(BASE + 6, BASE));               // 6
    p.extend(asm::addi_n(2, 2, -1));                // 9
    p.extend(asm::j(BASE + 11, BASE));              // 11
    let bad = [(0, Entry, 0), (3, Bnez, 9), (6, J, 0), (9, AddiN, 0), (11, J, 0)];
    for occupied in [false, true] {
        region_program("entry-a4", &p, &bad, &[], 1, 9, |c| {
            c.ps = ps::WOE | (2 << ps::CALLINC_SHIFT);
            c.windowstart = (1 << c.windowbase) | if occupied { 1 << ((c.windowbase + 1) % 16) } else { 0 };
            c.set_ar(2, 3); c.set_ar(4, BASE + 0x4000);
        }, 60);
        cases += 1;
    }
    // An ENTRY-headed region entered at an interior chunk that touches a8, with the
    // frame a8 lives in occupied: the module must reject, so the block path raises the
    // overflow exception. Called directly: the ENTRY block itself never gets hot here.
    let mut p = Vec::new();
    p.extend(asm::entry(1, 32));                    // 0
    p.extend(asm::addi_n(8, 8, 1));                 // 3
    p.extend(asm::addi_n(2, 2, -1));                // 5
    p.extend(asm::bz(1, BASE + 7, 2, BASE + 3));    // 7  bnez a2, 3
    p.extend(asm::j(BASE + 10, BASE + 3));          // 10 j 3
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let c0 = cpu(0);
        let head: Vec<BlockInsn> = (0..4).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: crate::exec::max_ar(&i), straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&c0, &mut ram, BASE, &head, true).expect("entry-interior region");
        let interior = formed.chunks.iter().position(|c| c.pc == BASE + 3).expect("interior chunk") as u32;
        let (bytes, _) = emitter::region::generate(&formed.chunks, &formed.pages, &formed.loops, &formed.leaves, true, Some(&[]));
        let slot = unsafe { host_jit_compile(bytes.as_ptr(), bytes.len()) };
        assert!(slot != 0);
        type Run = extern "C" fn(*mut Cpu, *mut Ram, *const Helpers, u32, u32, *const TlbEntry, *mut u32) -> u32;
        let f: Run = unsafe { std::mem::transmute(slot as usize) };
        for occupied in [false, true] {
            let mut c = cpu(3);
            c.pc = BASE + 3;
            c.ps = ps::WOE;
            c.windowstart = (1 << c.windowbase) | if occupied { 1 << ((c.windowbase + 2) % 16) } else { 0 };
            c.set_ar(2, 5);
            let before = c.ar;
            let fm = ram.fast_mem().unwrap();
            let result = f(&mut c, &mut ram, &Helpers::new::<Ram>(), 64, interior, fm.tlb, fm.page_ver);
            if occupied {
                assert_eq!(result >> 16, CODE_REJECT, "interior entry over an occupied frame must reject");
                assert_eq!(c.ar, before);
            } else {
                assert_eq!((result >> 16) & 7, CODE_LEFT);
                assert!(result & 0xffff >= 3, "interior entry ran {} instructions", result & 0xffff);
            }
            cases += 1;
        }
        unsafe { host_jit_release(slot) };
    }
    // A hardware loop whose body is exactly one ENTRY: when the post-ENTRY window proof
    // fails, the side exit must still take the backedge that ends at ENTRY's successor.
    let mut p = Vec::new();
    p.extend(asm::movi_n(10, 3));                   // 0
    p.extend(asm::lp(9, BASE + 2, 10, BASE + 8));   // 2  loopnez a10, 8
    p.extend(asm::entry(1, 32));                    // 5  (ends at LEND)
    p.extend(asm::addi_n(8, 8, 1));                 // 8
    p.extend(asm::j(BASE + 10, BASE));              // 10
    let loop_entry = [(0, MoviN, 0), (2, Loopnez, 8), (5, Entry, 0), (8, AddiN, 0), (10, J, 0)];
    for occupied in [false, true] {
        region_program("loop-entry", &p, &loop_entry, &[], 2, 5, |c| {
            c.ps = ps::WOE | (2 << ps::CALLINC_SHIFT);
            c.windowstart = (1 << c.windowbase) | if occupied { 1 << ((c.windowbase + 3) % 16) } else { 0 };
            c.set_ar(1, BASE + 0x4000);
        }, 60);
        cases += 1;
    }
    // The TinyDraw tile-uniform kernel shape: Q0 filled from a register, a 128-bit load
    // and lane compare, a LOOPNEZ over load/compare/and, then a store and scalar reduce.
    // Every PIE instruction is emitted on WASM SIMD and the loop is a region.
    use crate::pie::Role::*;
    let mut p = Vec::new();
    for sel in 0..4 { p.extend(asm::pie("ee.movi.32.q", &[(Qu, 0), (As, 7), (Sel, sel)])); } // 0,3,6,9
    p.extend(asm::mov_n(8, 12));                                             // 12
    p.extend(asm::pie("ee.vld.128.ip", &[(Qu, 1), (As, 8), (Imm, 16)]));     // 14
    p.extend(asm::pie("ee.vcmp.eq.s16", &[(Qa, 3), (Qx, 1), (Qy, 0)]));      // 17
    p.extend(asm::movi_n(10, 3));                                            // 20
    p.extend(asm::lp(9, BASE + 22, 10, BASE + 34));                          // 22 loopnez a10, 34
    p.extend(asm::pie("ee.vld.128.ip", &[(Qu, 1), (As, 8), (Imm, 16)]));     // 25
    p.extend(asm::pie("ee.vcmp.eq.s16", &[(Qa, 2), (Qx, 1), (Qy, 0)]));      // 28
    p.extend(asm::pie("ee.andq", &[(Qa, 3), (Qx, 3), (Qy, 2)]));             // 31
    p.extend(asm::mov_n(10, 13));                                            // 34
    p.extend(asm::pie("ee.vst.128.ip", &[(Qv, 3), (As, 10), (Imm, 16)]));    // 36
    p.extend(asm::l32i_n(10, 13, 0));                                        // 39
    p.extend(asm::l32i_n(11, 13, 4));                                        // 41
    p.extend(asm::and(10, 10, 11));                                          // 43
    p.extend(asm::s32i_n(10, 13, 8));                                        // 46
    p.extend(asm::j(BASE + 48, BASE + 12));                                  // 48: an internal edge after the stores
    let uniform = [(0, Pie, 0), (3, Pie, 0), (12, MovN, 0), (14, Pie, 0), (17, Pie, 0), (20, MoviN, 0), (22, Loopnez, 34), (25, Pie, 0), (28, Pie, 0), (31, Pie, 0), (34, MovN, 0), (36, Pie, 0), (39, L32iN, 0), (41, L32iN, 0), (43, And, 0), (46, S32iN, 0), (48, J, 12)];
    let mut data = [0x42u8, 0x00].repeat(40);
    data[50] = 0x43;
    // Variants: the plain kernel; CP3 disabled; the store landing in the last 16 bytes of
    // the mapping; loads running off the end of the mapping (slow, then a fault); a
    // read-only mapping (every fast store misses); the store into the region's own code
    // page followed by the internal edge back into the loop; the slow window; and an
    // occupied AR frame together with CP3 disabled, where the window overflow must win.
    let last16 = BASE + 65536 - 16;
    for (label, cp3, src, dst, readonly, occupied, turns, whole) in [
        ("uniform", 8, BASE + 0x1000, BASE + 0x2000, false, false, 600, true),
        ("uniform-cp3-off", 0, BASE + 0x1000, BASE + 0x2000, false, false, 40, false),
        ("uniform-last16", 8, BASE + 0x1000, last16, false, false, 300, true),
        ("uniform-off-end", 8, BASE + 65536 - 32, BASE + 0x2000, false, false, 40, false),
        ("uniform-readonly", 8, BASE + 0x1000, BASE + 0x2000, true, false, 40, false),
        // The 128-bit store lands in the region's own code page (past the program), so the
        // J edge after it must leave and the dispatcher must re-validate.
        ("uniform-self-modify", 8, BASE + 0x1000, BASE + 64, false, false, 300, false),
        ("uniform-slow", 8, SLOW, BASE + 0x2000, false, false, 300, false),
        ("uniform-overflow", 0, BASE + 0x1000, BASE + 0x2000, false, true, 40, false),
    ] {
        let max = region_program_on(label, &p, &uniform, &data, 9, 25, readonly, false, |c| {
            c.cpenable = cp3;
            if occupied { c.ps = ps::WOE; c.windowstart = (1 << c.windowbase) | (1 << ((c.windowbase + 2) % 16)); }
            c.set_ar(7, 0x0042_0042); c.set_ar(12, src); c.set_ar(13, dst);
        }, turns);
        assert!(!whole || max >= 20, "{label}: region retired at most {max} per call");
        cases += 1;
    }
    // The dot-product kernel of on-device inference (pocket-tank's 4-bit matmul): the ACCX reset,
    // 128-bit loads, signed 8- and 16-bit multiply-accumulate with and without their load, and the
    // RUR of ACCX that follows each dot product (without it the block would stay interpreted), in
    // a LOOPNEZ with an internal edge back. Variants: mixed lanes; lanes that drive ACCX into its
    // upper and lower saturation bound (starting near it, past the reset); CP3 disabled; a
    // read-only mapping, where loads stay on the fast path; loads running off the mapping; and
    // the slow window, where every load misses and re-executes in the interpreter.
    let mut dp = Vec::new();
    dp.extend(asm::pie("ee.zero.accx", &[]));                                                          // 0
    dp.extend(asm::movi_n(10, 3));                                                                     // 3
    dp.extend(asm::lp(9, BASE + 5, 10, BASE + 34));                                                    // 5 loopnez a10, 34
    dp.extend(asm::pie("ee.vld.128.ip", &[(Qu, 0), (As, 8), (Imm, 16)]));                              // 8
    dp.extend(asm::pie("ee.vld.128.ip", &[(Qu, 4), (As, 9), (Imm, 16)]));                              // 11
    dp.extend(asm::pie("ee.vmulas.s8.accx.ld.ip", &[(Qu, 5), (As, 9), (Imm, 16), (Qx, 0), (Qy, 4)]));  // 14
    dp.extend(asm::pie("ee.vmulas.s8.accx", &[(Qx, 0), (Qy, 5)]));                                     // 18
    dp.extend(asm::rur(11, 0));                                                                        // 21 rur.accx_0 a11
    dp.extend(asm::pie("ee.vmulas.s16.accx", &[(Qx, 4), (Qy, 5)]));                                    // 24
    dp.extend(asm::rur(15, 1));                                                                        // 27 rur.accx_1 a15
    dp.extend(asm::pie("ee.vmulas.s16.accx.ld.ip", &[(Qu, 1), (As, 8), (Imm, 16), (Qx, 0), (Qy, 5)])); // 30
    dp.extend(asm::mov_n(8, 12));                                                                      // 34
    dp.extend(asm::mov_n(9, 13));                                                                      // 36
    dp.extend(asm::j(BASE + 38, BASE + 3));                                                            // 38
    let dot = [(0, Pie, 0), (3, MoviN, 0), (5, Loopnez, 34), (8, Pie, 0), (11, Pie, 0), (14, Pie, 0), (18, Pie, 0),
               (21, Rur, 0), (24, Pie, 0), (27, Rur, 0), (30, Pie, 0), (34, MovN, 0), (36, MovN, 0), (38, J, 3)];
    let mixed: Vec<u8> = (0..0x100u32).map(|i| (i.wrapping_mul(0x9e37_79b9) >> 24) as u8).collect();
    let positive = vec![0x80u8; 0x100];                          // every product positive
    let negative = [vec![0x7fu8; 0x80], vec![0x80u8; 0x80]].concat(); // the a8 stream against the a9 stream: net negative
    let (near_high, near_low) = ((1i64 << 39) - 1 - 5_000_000, -(1i64 << 39) + 5_000_000);
    for (label, cp3, data, accx, src, readonly, turns, whole) in [
        ("dot", 8, &mixed, 0i64, BASE + 0x1000, false, 600, true),
        ("dot-saturate-high", 8, &positive, near_high, BASE + 0x1000, false, 300, true),
        ("dot-saturate-low", 8, &negative, near_low, BASE + 0x1000, false, 300, true),
        ("dot-cp3-off", 0, &mixed, 0, BASE + 0x1000, false, 40, false),
        ("dot-readonly", 8, &mixed, 0, BASE + 0x1000, true, 300, true),
        ("dot-off-end", 8, &mixed, 0, BASE + 65536 - 48, false, 40, false),
        ("dot-slow", 8, &mixed, 0, SLOW, false, 300, false),
    ] {
        let max = region_program_on(label, &dp, &dot, data, 3, 8, readonly, false, |c| {
            c.cpenable = cp3;
            c.accx = [accx as u32, ((accx >> 32) & 0xff) as u32];
            if accx != 0 { c.pc = BASE + 3; } // past the reset, so ACCX starts near its bound
            c.set_ar(8, src); c.set_ar(9, src + 0x80); c.set_ar(12, src); c.set_ar(13, src + 0x80);
        }, turns);
        assert!(!whole || max >= 20, "{label}: region retired at most {max} per call");
        cases += 1;
    }
    // EX178: the reset and final accumulate share a chunk ending at LEND.
    // The conditional backedge's spill must not suppress the fall-through spill.
    let mut held = Vec::new();
    held.extend(asm::movi_n(10, 3));
    held.extend(asm::lp(9, BASE + 2, 10, BASE + 12));
    held.extend(asm::pie("ee.zero.accx", &[]));
    held.extend(asm::pie("ee.vmulas.s8.accx.ld.ip", &[(Qu, 0), (As, 8), (Imm, 16), (Qx, 0), (Qy, 0)]));
    held.extend(asm::rur(11, 0));
    held.extend(asm::mov_n(8, 12));
    held.extend(asm::j(BASE + 17, BASE));
    let expected = [(0, MoviN, 0), (2, Loopnez, 12), (5, Pie, 0), (8, Pie, 0), (12, Rur, 0), (15, MovN, 0), (17, J, 0)];
    let max = region_program("held-accx-lend", &held, &expected, &mixed, 2, 5, |c| {
        c.cpenable = 8;
        c.set_ar(8, BASE + 0x1000); c.set_ar(12, BASE + 0x1000);
    }, 600);
    assert!(max > 2, "held ACCX loop never passed its head");
    cases += 1;
    // Enter the compiled body directly to exercise each false backedge predicate:
    // exhausted LCOUNT and an unrelated active loop with a different LEND.
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..held.len()].copy_from_slice(&held);
        let head: Vec<BlockInsn> = (0..2).scan(BASE, |pc, _| {
            let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap());
            *pc += i.len as u32;
            Some(BlockInsn { insn: i, max_ar: crate::exec::max_ar(&i), straddle: false, off: 0 })
        }).collect();
        let formed = emitter::region::form(&cpu(0), &mut ram, BASE, &head, true).expect("held ACCX region");
        let entry = formed.chunks.iter().position(|c| c.pc == BASE + 5).expect("held ACCX body") as u32;
        let (bytes, _) = emitter::region::generate(&formed.chunks, &formed.pages, &formed.loops, &formed.leaves, true, Some(&[]));
        let slot = unsafe { host_jit_compile(bytes.as_ptr(), bytes.len()) };
        assert_ne!(slot, 0);
        type Run = extern "C" fn(*mut Cpu, *mut Ram, *const Helpers, u32, u32, *const TlbEntry, *mut u32) -> u32;
        let f: Run = unsafe { std::mem::transmute(slot as usize) };
        for (lend, lcount) in [(BASE + 12, 0), (BASE + 100, 2)] {
            let (mut a, mut b) = (cpu(3), cpu(3));
            let (mut ra, mut rb) = (Ram::new(true, false), Ram::new(true, false));
            for r in [&mut ra, &mut rb] {
                r.ram.mem[..held.len()].copy_from_slice(&held);
                r.ram.mem[0x1000..0x1100].copy_from_slice(&mixed);
            }
            for c in [&mut a, &mut b] {
                c.pc = BASE + 5; c.ps = 0; c.cpenable = 8;
                c.lbeg = BASE + 5; c.lend = lend; c.lcount = lcount;
                c.accx = [123456, 0];
                c.set_ar(8, BASE + 0x1000);
            }
            CONTEXT.with(|c| *c.borrow_mut() = format!("held ACCX exit lend {lend:x} lcount {lcount}"));
            let fm = rb.fast_mem().unwrap();
            let result = f(&mut b, &mut rb, &Helpers::new::<Ram>(), 2, entry, fm.tlb, fm.page_ver);
            assert_eq!(result & 0xffff, 2, "held ACCX body must execute");
            for _ in 0..2 {
                let i = crate::decode::decode(a.pc, ra.fetch(a.pc).unwrap());
                exec_insn(&mut a, &mut ra, &i).unwrap();
            }
            same(&a, &b);
            assert_eq!(ra.ram.mem, rb.ram.mem);
            cases += 1;
        }
        unsafe { host_jit_release(slot) };
    }
    // EX178: pocket-tank's q4 matmul group, the shape that lets the emitter hold ACCX in a
    // local: `ee.zero.accx` dominating a straight-line run of accumulates in one chunk, a
    // 128-bit store and an `ee.vld.128.ip` inside the run (neither reads the accumulator),
    // the s16 accumulate with its larger per-step bound, and the `rur.accx_0` that ends the
    // run by forcing the spill. Variants drive an exit from every position of the run: a
    // load fault, a slow-window miss that re-executes in the interpreter, a read-only
    // mapping, a store into the region's own code page (DIRTY), and CP3 disabled.
    // `a7` counts iterations into lane 0 of Q0, an accumulate operand, so every pass
    // produces a different ACCX: a run that wrongly kept the local across `rur.accx_0`,
    // a chunk edge or a miss would read the previous pass's value and be caught.
    // Twenty-seven NOPs put instruction 32 (the chunk length limit) in the middle of the
    // run, so the region's internal edge cuts it and the held accumulator has to be
    // written back on an edge that does not leave.
    let mut q4 = Vec::new();
    q4.extend(asm::addi_n(7, 7, 1));                                                                   // 0
    for _ in 0..27 { q4.extend(asm::nop_n()); }                                                        // 2..56
    q4.extend(asm::pie("ee.movi.32.q", &[(Qu, 0), (As, 7), (Sel, 0)]));                                // 56
    q4.extend(asm::pie("ee.zero.accx", &[]));                                                          // 59
    q4.extend(asm::pie("ee.vld.128.ip", &[(Qu, 4), (As, 8), (Imm, 16)]));                              // 62
    q4.extend(asm::pie("ee.vmulas.s8.accx.ld.ip", &[(Qu, 5), (As, 8), (Imm, 16), (Qx, 0), (Qy, 4)]));  // 65
    q4.extend(asm::pie("ee.vmulas.s8.accx.ld.ip", &[(Qu, 4), (As, 8), (Imm, 16), (Qx, 1), (Qy, 5)]));  // 69 chunk 1 head
    q4.extend(asm::pie("ee.vld.128.ip", &[(Qu, 7), (As, 8), (Imm, 16)]));                              // 73
    q4.extend(asm::pie("ee.vmulas.s8.accx.ld.ip", &[(Qu, 5), (As, 8), (Imm, 16), (Qx, 2), (Qy, 7)]));  // 76
    q4.extend(asm::pie("ee.vmulas.s8.accx", &[(Qx, 2), (Qy, 4)]));                                     // 80
    q4.extend(asm::pie("ee.vst.128.ip", &[(Qv, 3), (As, 10), (Imm, 16)]));                             // 83
    q4.extend(asm::pie("ee.vmulas.s16.accx", &[(Qx, 0), (Qy, 4)]));                                    // 86
    q4.extend(asm::rur(11, 0));                                                                        // 89 rur.accx_0 a11
    q4.extend(asm::pie("ee.zero.accx", &[]));                                                          // 92
    q4.extend(asm::pie("ee.vmulas.s16.accx.ld.ip", &[(Qu, 6), (As, 9), (Imm, 16), (Qx, 0), (Qy, 1)])); // 95
    q4.extend(asm::rur(15, 1));                                                                        // 99 rur.accx_1 a15
    q4.extend(asm::mov_n(8, 12));                                                                      // 102
    q4.extend(asm::mov_n(9, 13));                                                                      // 104
    q4.extend(asm::mov_n(10, 14));                                                                     // 106
    q4.extend(asm::j(BASE + 108, BASE));                                                               // 108
    let group = [(0, AddiN, 0), (56, Pie, 0), (59, Pie, 0), (62, Pie, 0), (65, Pie, 0), (69, Pie, 0), (73, Pie, 0),
                 (76, Pie, 0), (80, Pie, 0), (83, Pie, 0), (86, Pie, 0), (89, Rur, 0), (92, Pie, 0), (95, Pie, 0),
                 (99, Rur, 0), (102, MovN, 0), (108, J, 0)];
    for (label, cp3, data, src, dst, readonly, turns, whole) in [
        ("q4", 8, &mixed, BASE + 0x1000, BASE + 0x2000, false, 600, true),
        ("q4-extreme", 8, &positive, BASE + 0x1000, BASE + 0x2000, false, 600, true),
        ("q4-cp3-off", 0, &mixed, BASE + 0x1000, BASE + 0x2000, false, 40, false),
        ("q4-readonly", 8, &mixed, BASE + 0x1000, BASE + 0x2000, true, 300, false),
        // The coalesced range leaves the mapping: the shared probe fails and the
        // per-access copy must fault on exactly the access that leaves it.
        ("q4-off-end", 8, &mixed, BASE + 65536 - 48, BASE + 0x2000, false, 40, false),
        ("q4-off-end2", 8, &mixed, BASE + 65536 - 64, BASE + 0x2000, false, 40, false),
        // The base is not 16-aligned: every address is masked but the post-increments
        // are not, so the coalesced copy must keep the low bits of the base register.
        ("q4-unaligned", 8, &mixed, BASE + 0x1000 + 5, BASE + 0x2000, false, 300, false),
        ("q4-slow", 8, &mixed, SLOW, BASE + 0x2000, false, 300, false),
        // Past the 111-byte program but inside its version page: DIRTY, no rewritten code.
        ("q4-self-modify", 8, &mixed, BASE + 0x1000, BASE + 128, false, 300, false),
    ] {
        let max = region_program_on(label, &q4, &group, data, 32, 69, readonly, false, |c| {
            c.cpenable = cp3;
            c.set_ar(7, 0x0100_0000);
            c.set_ar(8, src); c.set_ar(9, src + 0x80); c.set_ar(10, dst);
            c.set_ar(12, src); c.set_ar(13, src + 0x80); c.set_ar(14, dst);
        }, turns);
        assert!(!whole || max >= 40, "{label}: region retired at most {max} per call");
        cases += 1;
    }
    // EX155: the 4-bit weight unpack of pocket-tank's matmul: WUR/RUR SAR_BYTE, the byte shift
    // across two Q registers (every count 0..15, with and without QUP, destination aliasing either
    // source), 32-bit lane shifts for SAR 0..32, and saturating/min/max lane arithmetic.
    let mut up = Vec::new();
    up.extend(asm::pie("ee.vld.128.ip", &[(Qu, 0), (As, 8), (Imm, 16)]));   // 0
    up.extend(asm::pie("ee.vld.128.ip", &[(Qu, 1), (As, 8), (Imm, 16)]));   // 3
    up.extend(asm::addi_n(11, 11, 1));                                      // 6
    up.extend(asm::wur(11, 13));                                            // 8
    up.extend(asm::shift_setup(1, 11));                                     // 11 ssl: SAR 1..32
    up.extend(asm::pie("ee.src.q", &[(Qa, 2), (Qs0, 0), (Qs1, 1)]));        // 14
    up.extend(asm::pie("ee.src.q.qup", &[(Qa, 3), (Qs0, 0), (Qs1, 1)]));    // 17
    up.extend(asm::pie("ee.vsr.32", &[(Qa, 4), (Qs, 2)]));                  // 20
    up.extend(asm::pie("ee.vsl.32", &[(Qa, 5), (Qs, 3)]));                  // 23
    up.extend(asm::shift_setup(0, 11));                                     // 26 ssr: SAR 0..31
    up.extend(asm::pie("ee.vsr.32", &[(Qa, 6), (Qs, 2)]));                  // 29
    up.extend(asm::pie("ee.vsl.32", &[(Qa, 7), (Qs, 3)]));                  // 32
    up.extend(asm::pie("ee.vsubs.s8", &[(Qa, 4), (Qx, 4), (Qy, 5)]));       // 35
    up.extend(asm::pie("ee.vadds.s8", &[(Qa, 5), (Qx, 6), (Qy, 2)]));       // 38
    up.extend(asm::pie("ee.vsubs.s16", &[(Qa, 6), (Qx, 6), (Qy, 3)]));      // 41
    up.extend(asm::pie("ee.vadds.s16", &[(Qa, 7), (Qx, 7), (Qy, 2)]));      // 44
    up.extend(asm::pie("ee.vmin.s8", &[(Qa, 2), (Qx, 4), (Qy, 5)]));        // 47
    up.extend(asm::pie("ee.vmax.s8", &[(Qa, 3), (Qx, 4), (Qy, 5)]));        // 50
    up.extend(asm::pie("ee.vmin.s16", &[(Qa, 4), (Qx, 6), (Qy, 7)]));       // 53
    up.extend(asm::pie("ee.vmax.s16", &[(Qa, 5), (Qx, 6), (Qy, 7)]));       // 56
    up.extend(asm::pie("ee.vmin.s32", &[(Qa, 6), (Qx, 2), (Qy, 3)]));       // 59
    up.extend(asm::pie("ee.vmax.s32", &[(Qa, 7), (Qx, 2), (Qy, 3)]));       // 62
    up.extend(asm::pie("ee.src.q.qup", &[(Qa, 0), (Qs0, 0), (Qs1, 1)]));    // 65 Qa is Qs0
    up.extend(asm::pie("ee.src.q.qup", &[(Qa, 1), (Qs0, 0), (Qs1, 1)]));    // 68 Qa is Qs1
    up.extend(asm::pie("ee.src.q", &[(Qa, 1), (Qs0, 1), (Qs1, 0)]));        // 71
    up.extend(asm::rur(14, 13));                                            // 74
    up.extend(asm::mov_n(8, 12));                                           // 77
    up.extend(asm::j(BASE + 79, BASE));                                     // 79
    let unpack = [(0, Pie, 0), (6, AddiN, 0), (8, Wur, 0), (11, Ssl, 0), (14, Pie, 0), (17, Pie, 0), (26, Ssr, 0), (35, Pie, 0), (65, Pie, 0),
                  (74, Rur, 0), (77, MovN, 0), (79, J, 0)];
    let edges: Vec<u8> = (0..0x100u32).map(|i| [0x80u8, 0x7f, 0xff, 0x00, 0x01, 0x81][(i as usize * 7 + i as usize / 16) % 6]).collect();
    for (label, cp3, data, turns) in [("unpack", 8, &mixed, 900), ("unpack-edges", 8, &edges, 900), ("unpack-cp3-off", 0, &mixed, 40)] {
        let max = region_program_on(label, &up, &unpack, data, 28, 14, false, false, |c| {
            c.cpenable = cp3;
            c.set_ar(8, BASE + 0x1000); c.set_ar(12, BASE + 0x1000); c.set_ar(11, 0xffff_fff0);
        }, turns);
        assert!(cp3 == 0 || max >= 20, "{label}: region retired at most {max} per call");
        cases += 1;
    }
    // Region formation itself for the tile scan: every static successor, nothing past RSR.
    let mut ram = Ram::new(true, false);
    ram.ram.mem[..tp.len()].copy_from_slice(&tp);
    let c = cpu(0);
    let head: Vec<BlockInsn> = (0..2).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
    let formed = emitter::region::form(&c, &mut ram, BASE, &head, true).expect("tile region");
    assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len())).collect::<Vec<_>>(),
        vec![(0, 2), (6, 3), (35, 1), (13, 6), (41, 2), (43, 1)]);
    assert_eq!(formed.pages, vec![(0, 0)]);
    assert!(emitter::region::form(&c, &mut ram, BASE + 38, &head, true).is_none(), "RSR head");
    // tails-s1: the region programs above choose guarded copies and run through them.
    assert!(REGION_STATS[12].load(std::sync::atomic::Ordering::Relaxed) > 20, "too few regions chose guarded copies");
    // tails-s2: and resume inside them.
    assert!(REGION_STATS[13].load(std::sync::atomic::Ordering::Relaxed) > 200, "too few resumes into guarded copies: {}", REGION_STATS[13].load(std::sync::atomic::Ordering::Relaxed));
    cases + 2 + prev_page_store() + forward_edges() + self_loops() + outside_loops() + jx_literal() + deferred_in_guarded_copy() + leaf_calls()
        + resumed_head_copy() + head_recovery_long_pie() + uncovered_verdict() + resume_memo() + named_tail_resume() + looped_resumes() + store_runs()
}

/// lane-s1: a dispatch at the PC a quantum-ending region exit left enters that region directly.
/// Whatever could change that entry in between (a store into its code page, a flush, a probe, a
/// hardware loop, a timer deadline inside the quantum, a pending interrupt) must leave the result
/// exactly the interpreter's; the store, flush, probe and loop must send it the ordinary way.
/// lane-s2b: the same through the prepared entry a round batch takes first (`run_memo`, then
/// `run_block` when it declines).
fn resume_memo() -> u32 {
    resume_memo_on(false) + resume_memo_on(true)
}

fn resume_memo_on(prepared: bool) -> u32 {
    let mut p = Vec::new();
    for _ in 0..3 { p.extend(asm::addi_n(3, 3, 1)); }  // 0 2 4   chunk 0
    p.extend(asm::j(BASE + 6, BASE + 9));             // 6
    for _ in 0..3 { p.extend(asm::addi_n(5, 5, 1)); }  // 9 11 13 chunk 1
    p.extend(asm::j(BASE + 15, BASE));                // 15
    let turn = |a: &mut Cpu, b: &mut Cpu, ra: &mut Ram, rb: &mut Ram, budget: u32| {
        let (done, trap) = if prepared { crate::block::run_memo(b, rb, budget) } else { None }
            .unwrap_or_else(|| crate::block::run_block(b, rb, budget));
        let mut oracle = None;
        for _ in 0..done {
            ra.note_pc(a.pc);
            if let Err(t) = crate::step(a, ra) { oracle = Some(t); break; }
        }
        assert_eq!(trap, oracle, "memo: budget {budget}");
        same(a, b);
        if let Some(Trap::Interrupt(_)) = trap {
            for c in [a, b] { c.pc = c.epc[1]; c.ps &= !ps::EXCM; c.interrupt = 0; c.ccompare[0] = 0; c.refresh_event(); c.intenable = 0; }
        }
    };
    let stat = |i: usize| REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
    let (mut a, mut b) = (cpu(5), cpu(5));
    let (mut ra, mut rb) = (Ram::new(true, false), Ram::new(true, false));
    for r in [&mut ra, &mut rb] { r.ram.mem[..p.len()].copy_from_slice(&p); }
    CONTEXT.with(|c| *c.borrow_mut() = format!("resume memo (prepared {prepared})"));
    let (hits, tuned) = (stat(14), stat(12));
    // Mixed credit: exits at chunk heads choose guarded copies, whose cuts then end quanta mid-chunk.
    for t in 0..300 { turn(&mut a, &mut b, &mut ra, &mut rb, [64, 61, 50, 64, 23, 64][t % 6]); }
    assert!(stat(12) > tuned, "memo: the region never chose a copy");
    assert!(stat(14) > hits + 100, "memo: too few direct entries ({})", stat(14) - hits);
    for kind in 0..7 {
        let mut armed = false;
        for t in 0..40 {
            turn(&mut a, &mut b, &mut ra, &mut rb, [61, 64, 50][t % 3]);
            // the loop case needs a chunk-head entry, the one kind that may admit a loop
            // memo-s2: the replayed hints case needs a guarded copy's cut
            if b.blocks.memo.0 == b.pc && (kind != 3 || b.blocks.memo.2 >> 16 == 0) && (kind != 6 || b.blocks.memo.2 >> 16 != 0) { armed = true; break; }
        }
        assert!(armed, "memo: kind {kind} never armed");
        let declined = stat(15);
        match kind {
            // addi.n a3,a3,1 at +2 becomes addi.n a3,a3,2: a new version of the region's page
            0 => for r in [&mut ra, &mut rb] { r.write8(BASE + 2, 0x2b).unwrap(); },
            // memo-s2: a flush forgets the memo outright (and its unwritten hints)
            1 => { b.blocks.flush(); assert_eq!(b.blocks.memo.0, 1, "memo: a flush kept the memo"); }
            2 => b.boundary_bloom = emu_core::core::pc_bit(b.pc),
            // an active hardware loop, not the region's own, ending inside it after a fall-through
            3 => for c in [&mut a, &mut b] { c.lcount = 1; c.lbeg = BASE; c.lend = BASE + 4; },
            4 => for c in [&mut a, &mut b] { c.ccompare[0] = c.ccount.wrapping_add(3); c.refresh_event(); c.intenable = 1 << 6; },
            5 => for c in [&mut a, &mut b] { c.intenable = 1 << 6; c.interrupt = 1 << 6; },
            // memo-s2: a copy cut's memo declined (a foreign loop) must still find the resume the
            // exit would have named, not decode a new block in mid-chunk
            _ => for c in [&mut a, &mut b] { c.lcount = 1; c.lbeg = BASE; c.lend = BASE + 4; },
        }
        let builds = b.blocks.builds;
        turn(&mut a, &mut b, &mut ra, &mut rb, 64);
        if kind == 6 { assert_eq!(b.blocks.builds, builds, "memo: a declined memo decoded a new block"); }
        for _ in 0..2 { turn(&mut a, &mut b, &mut ra, &mut rb, 64); }
        if matches!(kind, 0 | 2 | 3 | 6) { assert!(stat(15) > declined, "memo: kind {kind} was not declined"); }
        b.boundary_bloom = 0;
        for c in [&mut a, &mut b] { c.lcount = 0; }
    }
    // lane-s1l: a quantum that ends exactly at the head of a loop body the region set up itself
    // resumes there directly with that loop active, as the slow path admits it.
    let mut p = asm::movi_n(4, 60);                   // 0
    p.extend(asm::lp(8, BASE + 2, 4, BASE + 11));     // 2  loop a4, body 5..11
    for r in [3, 5, 6] { p.extend(asm::addi_n(r, r, 1)); } // 5 7 9
    p.extend(asm::j(BASE + 11, BASE));                // 11
    let (mut a, mut b) = (cpu(6), cpu(6));
    let (mut ra, mut rb) = (Ram::new(true, false), Ram::new(true, false));
    for r in [&mut ra, &mut rb] { r.ram.mem[..p.len()].copy_from_slice(&p); }
    CONTEXT.with(|c| *c.borrow_mut() = format!("resume memo in a loop (prepared {prepared})"));
    let mut looped = 0;
    for _ in 0..400 {
        let (armed, hits) = (b.lcount != 0 && b.blocks.memo.0 == b.pc, stat(14));
        turn(&mut a, &mut b, &mut ra, &mut rb, 63);
        if armed && stat(14) > hits { looped += 1; }
    }
    assert!(looped > 20, "memo: too few direct entries inside the region's own loop ({looped})");
    2
}

/// alias-s1: a copy cut names its head's decoded block and index, so the next dispatch resumes
/// there with no alias lookup and no build. A flush, or a rewrite of the code, between the cut and
/// the arrival makes that resume invalid, and the arrival runs the current code.
fn named_tail_resume() -> u32 {
    let mut p = Vec::new();
    p.extend(asm::addi_n(3, 3, 1));        // 0  chunk 0
    p.extend(asm::addi_n(5, 5, 1));        // 2
    p.extend(asm::addi_n(6, 6, 1));        // 4
    p.extend(asm::j(BASE + 6, BASE + 9));  // 6
    p.extend(asm::addi_n(3, 3, 1));        // 9  chunk 1
    p.extend(asm::addi_n(5, 5, 1));        // 11
    p.extend(asm::addi_n(6, 6, 1));        // 13 the cut
    p.extend(asm::j(BASE + 15, BASE));     // 15
    let mut c = cpu(7);
    let mut ram = Ram::new(true, false);
    ram.ram.mem[..p.len()].copy_from_slice(&p);
    let stat = |i: usize| REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
    let prepare = |c: &mut Cpu, ram: &mut Ram| {
        let (formed, tuned) = (stat(0), stat(12));
        for _ in 0..60 { c.pc = BASE; crate::block::run_block(c, ram, 64); }
        assert!(stat(0) > formed, "named-tail: no region");
        for _ in 0..20 { c.pc = BASE; crate::block::run_block(c, ram, 6); }
        assert!(stat(12) > tuned, "named-tail: chunk 1 got no copy");
    };
    // Two instructions into chunk 1's copy, with chunk 1's head decoded.
    let cut = |c: &mut Cpu, ram: &mut Ram| {
        c.pc = BASE + 9;
        assert_eq!(crate::block::run_block(c, ram, 1), (1, None));
        c.pc = BASE;
        assert_eq!(crate::block::run_block(c, ram, 6), (6, None));
        // memo-s2: the hints wait in the memo
        assert_eq!((c.pc, c.blocks.memo.0, c.blocks.memo.4), (BASE + 13, BASE + 13, BASE + 9), "named-tail: no copy cut");
        (c.blocks.alias_hits, c.blocks.builds)
    };
    let arrive = |c: &mut Cpu, ram: &mut Ram| {
        let mut direct = c.clone();
        assert!(crate::exec::step(&mut direct, ram).is_ok());
        assert_eq!(crate::block::run_block(c, ram, 1), (1, None));
        same(c, &direct);
        assert_eq!(c.pc, BASE + 15);
    };
    prepare(&mut c, &mut ram);
    let (hits, builds) = cut(&mut c, &mut ram);
    arrive(&mut c, &mut ram);
    assert_eq!((c.blocks.alias_hits, c.blocks.builds), (hits, builds), "named-tail: the arrival must resume, not alias or build");
    let (hits, builds) = cut(&mut c, &mut ram);
    let a6 = c.get_ar(6);
    for (i, b) in asm::addi_n(6, 6, 2).into_iter().enumerate() { ram.write8(BASE + 13 + i as u32, b).unwrap(); }
    arrive(&mut c, &mut ram);
    assert_eq!(c.get_ar(6), a6.wrapping_add(2), "named-tail: the arrival must run the rewritten code");
    assert_eq!((c.blocks.alias_hits, c.blocks.builds), (hits + 1, builds + 1), "named-tail: a rewrite re-decodes the head and aliases into it");
    prepare(&mut c, &mut ram);
    let (hits, builds) = cut(&mut c, &mut ram);
    c.blocks.flush();
    arrive(&mut c, &mut ram);
    assert_eq!((c.blocks.alias_hits, c.blocks.builds), (hits, builds + 1), "named-tail: after a flush the arrival is a new head");
    3
}


/// loop-s1: quanta end inside a LOOPNEZ body that is one region chunk, so dispatches start there
/// with the loop active: fresh entries at the body head and resumes inside its copy continue the
/// loop in the region. LCOUNT 0, 1 and n; LBEG moved (as by WSR) and LEND moved mid-chunk must be
/// refused; a window collision rejects at entry. Every turn is compared with the interpreter.
fn looped_resumes() -> u32 {
    let stat = |i: usize| REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
    let mut cases = 0;
    for (n, perturb, windows) in [(0, false, 0), (1, false, 0), (2, false, 0), (7, false, 0), (40, false, 0), (40, true, 0), (40, false, 0b101)] {
        let mut p = Vec::new();
        p.extend(asm::addi_n(3, 3, 1));               // 0  head
        p.extend(asm::movi_n(7, n));                  // 2
        p.extend(asm::lp(9, BASE + 4, 7, BASE + 13)); // 4  loopnez a7, 13
        p.extend(asm::addi_n(5, 5, 1));               // 7  LBEG: the body is one chunk
        p.extend(asm::addi_n(9, 9, 1));               // 9
        p.extend(asm::addi_n(4, 4, 1));               // 11 ends at LEND
        p.extend(asm::j(BASE + 13, BASE));            // 13
        let (mut a, mut b) = (cpu(3), cpu(3));
        let (mut ra, mut rb) = (Ram::new(true, false), Ram::new(true, false));
        for r in [&mut ra, &mut rb] { r.ram.mem[..p.len()].copy_from_slice(&p); }
        for c in [&mut a, &mut b] {
            c.pc = BASE;
            c.ps = if windows != 0 { ps::WOE } else { 0 };
        }
        let (looped, resumes) = (stat(16), stat(13));
        let mut memo_looped = 0;
        for turn in 0..1500 {
            let budget = 1 + (turn * 7) as u32 % 71;
            for c in [&mut a, &mut b] {
                // Occupied frames above: a9 overflows until the "handler" below frees them.
                if windows != 0 && turn % 100 == 0 { c.windowstart = (1 << 3) | (windows << 4); }
                if perturb && turn % 30 == 10 && c.lcount != 0 { c.lbeg = BASE + 9; }
                if perturb && turn % 30 == 20 && c.lcount != 0 { c.lend = BASE + 11; }
            }
            CONTEXT.with(|c| *c.borrow_mut() = format!("looped-resumes n {n} perturb {perturb} turn {turn} pc {:x} budget {budget}", b.pc));
            // integrate: a memo'd copy resume with the chunk's own loop active (lane-s1c x loop-s1)
            let (armed, hits) = (b.lcount != 0 && b.blocks.memo.0 == b.pc && b.blocks.memo.2 >> 16 != 0, stat(14));
            let (done, trap) = crate::block::run_block(&mut b, &mut rb, budget);
            if armed && stat(14) > hits { memo_looped += 1; }
            assert!(done <= budget);
            let mut oracle = None;
            for _ in 0..done {
                ra.note_pc(a.pc);
                if let Err(t) = crate::step(&mut a, &mut ra) { oracle = Some(t); break; }
            }
            assert_eq!(trap, oracle, "looped-resumes n {n}: turn {turn}");
            same(&a, &b);
            if done > 0 && trap.is_none() { assert_eq!(ra.noted, rb.noted, "looped-resumes n {n}: noted PC after turn {turn}"); }
            if trap.is_some() {
                for c in [&mut a, &mut b] { c.pc = c.epc[1]; c.ps = ps::WOE; c.windowbase = 3; c.windowstart = 1 << 3; }
            }
            cases += 1;
        }
        if n >= 7 && windows == 0 {
            assert!(stat(16) > looped + 100, "looped-resumes n {n}: too few region entries inside the active loop ({})", stat(16) - looped);
            assert!(stat(13) > resumes + 50, "looped-resumes n {n}: too few resumes into the looped copy ({})", stat(13) - resumes);
            assert!(perturb || memo_looped > 20, "looped-resumes n {n}: too few memo'd copy resumes inside the active loop ({memo_looped})");
        }
    }
    cases
}

/// edge-s1: a head chunk no internal edge reaches. Its quanta end only in dispatches short of its
/// credit, so only the own-module resumes there can choose its copy; later resumes enter the copy.
fn resumed_head_copy() -> u32 {
    let mut p = Vec::new();
    p.extend(asm::addi_n(3, 3, 1));        // 0  chunk 0
    p.extend(asm::addi_n(5, 5, 1));        // 2
    p.extend(asm::addi_n(6, 6, 1));        // 4
    p.extend(asm::j(BASE + 6, BASE + 9));  // 6
    p.extend(asm::addi_n(3, 3, 1));        // 9  chunk 1
    p.extend(asm::addi_n(5, 5, 1));        // 11
    p.extend(asm::jx(7));                  // 13 back to the head through the dispatcher
    let expected = [(0, Op::AddiN, 0), (6, Op::J, 9), (9, Op::AddiN, 0), (13, Op::Jx, 0)];
    region_program("resumed-head", &p, &expected, &[], 4, 9, |c| c.set_ar(7, BASE), 400);
    let stat = |i: usize| REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
    let mut c = cpu(7);
    let mut ram = Ram::new(true, false);
    ram.ram.mem[..p.len()].copy_from_slice(&p);
    c.set_ar(7, BASE);
    let formed = stat(0);
    // Whole passes only: no quantum ends at the internal edge, and none resumes.
    for _ in 0..60 { c.pc = BASE; crate::block::run_block(&mut c, &mut ram, 64); }
    assert!(stat(0) > formed, "resumed-head: no region");
    let (tuned, resumes) = (stat(12), stat(13));
    for _ in 0..2 {
        c.pc = BASE;
        assert_eq!(crate::block::run_block(&mut c, &mut ram, 2), (2, None));
        assert_eq!(crate::block::run_block(&mut c, &mut ram, 1), (1, None));
    }
    assert_eq!(stat(12), tuned + 1, "resumed-head: resumes alone must choose the head's copy");
    c.pc = BASE;
    let a3 = c.get_ar(3);
    assert_eq!(crate::block::run_block(&mut c, &mut ram, 2), (2, None));
    // lane-s1c: the own module's cut leaves the copy entry for the next dispatch
    let (armed, hits) = (c.blocks.memo.0 == c.pc, stat(14));
    assert_eq!(crate::block::run_block(&mut c, &mut ram, 64), (64, None));
    assert_eq!(stat(13), resumes + 1, "resumed-head: the resume must enter the head's copy");
    assert!(armed && stat(14) == hits + 1, "resumed-head: the resume after a cut must use the memo");
    // 66 instructions from the head: 9 passes of 7, then three more (a3 counts twice per pass).
    assert_eq!((c.pc, c.get_ar(3)), (BASE + 6, a3.wrapping_add(19)));
    // edge-s1r: a second window of resumes, now into chunk 1, adds its copy to the head's. Chunk 1
    // is reached inside the region only, so its own module is compiled first (no resume yet).
    for _ in 0..40 { c.pc = BASE + 9; crate::block::run_block(&mut c, &mut ram, 1); }
    assert_eq!(stat(12), tuned + 1, "resumed-head: dispatches that are not resumes count nothing");
    for _ in 0..2 {
        c.pc = BASE + 9;
        assert_eq!(crate::block::run_block(&mut c, &mut ram, 1), (1, None));
        assert_eq!(crate::block::run_block(&mut c, &mut ram, 1), (1, None));
    }
    assert_eq!(stat(12), tuned + 2, "resumed-head: a second window must add chunk 1's copy");
    for (start, first) in [(BASE + 9, 1), (BASE, 2)] {
        let resumes = stat(13);
        c.pc = start;
        assert_eq!(crate::block::run_block(&mut c, &mut ram, first), (first, None));
        assert_eq!(crate::block::run_block(&mut c, &mut ram, 64), (64, None));
        assert_eq!(stat(13), resumes + 1, "resumed-head: the resume at {start:x} must enter its copy");
    }
    2
}

/// tails-s1 (from EX182 s1): a helper fallback inside a guarded copy. Short credit first makes
/// the region count its exits at chunk 1 and regenerate with that chunk copied; then a
/// credit-short edge enters the copy two instructions short and the slow store there is
/// refused under `defer_armed`: the refused instruction is counted and subtracted exactly
/// as from an own-module body, and its PC is the continuation.
fn deferred_in_guarded_copy() -> u32 {
    let mut p = Vec::new();
    p.extend(asm::addi_n(3, 3, 1));        // 0  chunk 0
    p.extend(asm::addi_n(5, 5, 1));        // 2
    p.extend(asm::addi_n(6, 6, 1));        // 4
    p.extend(asm::j(BASE + 6, BASE + 9));  // 6
    p.extend(asm::addi_n(3, 3, 1));        // 9  chunk 1
    p.extend(asm::s32i_n(3, 4, 0));        // 11 refused while deferring
    p.extend(asm::addi_n(5, 5, 1));        // 13
    p.extend(asm::j(BASE + 15, BASE));     // 15
    let mut c = cpu(7);
    let mut ram = Ram::new(true, false);
    ram.ram.mem[..p.len()].copy_from_slice(&p);
    c.set_ar(4, SLOW);
    let stat = |i: usize| REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
    let (formed, tuned) = (stat(0), stat(12));
    for _ in 0..60 { c.pc = BASE; crate::block::run_block(&mut c, &mut ram, 64); }
    assert!(stat(0) > formed, "deferred-guarded: no region");
    for _ in 0..20 { c.pc = BASE; crate::block::run_block(&mut c, &mut ram, 6); }
    assert_eq!(stat(12), tuned + 1, "deferred-guarded: the region must choose its copy exactly once");
    c.pc = BASE;
    ram.defer_armed = true;
    let (retired, writes) = (c.insn_count, ram.slow_writes);
    let (a3, a5, a6) = (c.get_ar(3), c.get_ar(5), c.get_ar(6));
    assert_eq!(crate::block::run_block(&mut c, &mut ram, 6), (5, None));
    assert!(ram.deferred);
    assert_eq!(c.pc, BASE + 11, "the refused store is the continuation");
    assert_eq!(c.insn_count, retired + 5);
    assert_eq!(ram.slow_writes, writes);
    assert_eq!((c.get_ar(3), c.get_ar(5), c.get_ar(6)), (a3.wrapping_add(2), a5.wrapping_add(1), a6.wrapping_add(1)));
    // The store runs on the next dispatch, from the interior PC the cut left behind.
    ram.defer_armed = false;
    ram.deferred = false;
    assert_eq!(crate::block::run_block(&mut c, &mut ram, 1), (1, None));
    assert_eq!(ram.slow_writes, writes + 1);
    assert_eq!(&ram.slow[..4], &a3.wrapping_add(2).to_le_bytes());
    // A copy cut two instructions into chunk 1 names its head. With the head block gone from
    // the decoded cache, the resume decodes that head and aliases into it, not a new head.
    c.set_ar(4, BASE + 0x1000);
    c.pc = BASE;
    assert_eq!(crate::block::run_block(&mut c, &mut ram, 6), (6, None));
    let hint = (BASE + 13, BASE + 9);
    // memo-s2: the memo'd cut keeps its hints (the head) in the memo for a dispatch that declines it
    assert_eq!((c.pc, c.blocks.memo.0, c.blocks.memo.4), (BASE + 13, BASE + 13, hint.1));
    let at = c.clone();
    c.blocks.flush();
    c.blocks.alias_pc = c.pc;
    c.blocks.alias_head = hint;
    let (hits, builds) = (c.blocks.alias_hits, c.blocks.builds);
    assert_eq!(crate::block::run_block(&mut c, &mut ram, 1), (1, None));
    assert_eq!((c.blocks.alias_hits, c.blocks.builds, c.pc), (hits + 1, builds + 1, BASE + 15));
    assert_eq!(c.blocks.alias_head, (1, 1), "a head hint is used once");
    // Review T1: the head no longer fetches (a mapping ending there) while the cut PC still does.
    // Recovery gives up without an exception and the arrival runs exactly as a direct step at it.
    // An old hint does not survive the decoder flush; a hint that fails is consumed.
    let mut direct = at.clone();
    assert!(crate::exec::step(&mut direct, &mut ram).is_ok());
    for (fault, rearm) in [(Some(BASE + 9), true), (None, false)] {
        c = at.clone();
        c.blocks.alias_head = hint;
        c.blocks.flush();
        assert_eq!(c.blocks.alias_head, (1, 1), "flush forgets the head hint");
        c.blocks.alias_pc = c.pc;
        if rearm { c.blocks.alias_head = hint; }
        ram.fetch_fault = fault;
        let (hits, builds) = (c.blocks.alias_hits, c.blocks.builds);
        assert_eq!(crate::block::run_block(&mut c, &mut ram, 1), (1, None));
        same(&c, &direct);
        assert_eq!((c.blocks.alias_hits, c.blocks.builds, c.blocks.alias_head), (hits, builds + 1, (1, 1)),
                   "no head recovery (fault {fault:x?}): only the arrival's own block is built");
    }
    ram.fetch_fault = None;
    2
}

/// Review T2: a copy cut late in a chunk of four-byte PIE instructions (up to 124 bytes past
/// its head, beyond `alias_lookup`'s backward scan) still recovers into the one head block.
fn head_recovery_long_pie() -> u32 {
    use crate::pie::Role::*;
    let mut p = Vec::new();
    for _ in 0..31 { p.extend(asm::pie("ee.vmulas.s16.accx.ld.ip", &[(Qu, 1), (As, 8), (Imm, 16), (Qx, 0), (Qy, 5)])); }
    p.extend(asm::j(BASE + 124, BASE));
    assert_eq!(p.len(), 127);
    for k in 24..32u32 {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let mut c = cpu(5);
        c.cpenable = 8;
        c.set_ar(8, BASE + 0x1000);
        c.pc = BASE + 4 * k;
        let mut direct = c.clone();
        assert!(crate::exec::step(&mut direct, &mut ram).is_ok());
        c.blocks.alias_pc = c.pc;
        c.blocks.alias_head = (c.pc, BASE);
        let (hits, builds) = (c.blocks.alias_hits, c.blocks.builds);
        assert_eq!(crate::block::run_block(&mut c, &mut ram, 1), (1, None));
        same(&c, &direct);
        assert_eq!((c.blocks.alias_hits, c.blocks.builds), (hits + 1, builds + 1),
                   "long-pie cut at {k}: one build (the head), entered at the cut");
    }
    8
}

/// EX181 s2: the two shapes whose backedge stays inside one chunk — a `bnez` back to the
/// chunk head, and a hardware loop body that is exactly one chunk after the LEND split.
/// Both are wrapped in a WASM loop, so their backedge is a `br` instead of a br_table hop;
/// the harness cuts credit at every index and probes the head.
fn self_loops() -> u32 {
    use Op::*;
    let mut p = Vec::new();
    p.extend(asm::movi_n(3, 5));                 // 0
    p.extend(asm::addi_n(2, 2, 1));              // 2  the self-looping chunk starts here
    p.extend(asm::addi_n(3, 3, -1));             // 4
    p.extend(asm::bz(1, BASE + 6, 3, BASE + 2)); // 6  bnez a3, 2
    p.extend(asm::addi_n(6, 6, 1));              // 9
    p.extend(asm::j(BASE + 11, BASE));           // 11
    let shape = [(0, MoviN, 0), (2, AddiN, 0), (4, AddiN, 0), (6, Bnez, 2), (9, AddiN, 0), (11, J, 0)];
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let head: Vec<BlockInsn> = (0..4).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&cpu(0), &mut ram, BASE, &head, true).expect("bnez self-loop region");
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len())).collect::<Vec<_>>(), vec![(0, 4), (9, 2), (2, 3)]);
    }
    let before = emitter::region::SELF_LOOP_BRANCHES.load(std::sync::atomic::Ordering::Relaxed);
    let max = region_program("bnez-self-loop", &p, &shape, &[], 4, 2, |_| {}, 900);
    assert!(max > 4, "bnez self-loop region never passed its head ({max})");
    assert!(emitter::region::SELF_LOOP_BRANCHES.load(std::sync::atomic::Ordering::Relaxed) > before, "bnez self-loop emitted no direct backedge");

    let mut p = Vec::new();
    p.extend(asm::lp(9, BASE, 3, BASE + 9));        // 0  loopnez a3, 9
    p.extend(asm::addi_n(2, 2, 1));                 // 3  LBEG: the whole body is one chunk
    p.extend(asm::addi_n(4, 4, 1));                 // 5
    p.extend(asm::addi_n(5, 5, 1));                 // 7  ends exactly at LEND
    p.extend(asm::addi_n(6, 6, 1));                 // 9  loop exit
    p.extend(asm::j(BASE + 11, BASE));              // 11
    let shape = [(0, Loopnez, 9), (3, AddiN, 0), (5, AddiN, 0), (7, AddiN, 0), (9, AddiN, 0), (11, J, 0)];
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let head: Vec<BlockInsn> = (0..1).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&cpu(0), &mut ram, BASE, &head, true).expect("hardware self-loop region");
        assert_eq!(formed.loops, vec![(BASE + 9, BASE + 3)]);
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len())).collect::<Vec<_>>(), vec![(0, 1), (3, 3), (9, 2)]);
    }
    // a3 is never written, so the count is the same on every pass; 0 makes LOOPNEZ skip.
    for count in [4, 1, 0] {
        let before = emitter::region::SELF_LOOP_BRANCHES.load(std::sync::atomic::Ordering::Relaxed);
        let max = region_program("hw-self-loop", &p, &shape, &[], 1, 3, move |c| { c.set_ar(3, count); }, 900);
        assert!(max > 1, "hardware self-loop region never passed its head ({max})");
        assert!(emitter::region::SELF_LOOP_BRANCHES.load(std::sync::atomic::Ordering::Relaxed) > before, "hardware self-loop emitted no direct backedge (count {count})");
    }
    2
}

/// coverage-s1: a loop body whose LOOP ran before its region formed. Formation adopts the
/// active (LEND, LBEG), so the backedge stays inside; the count drains to zero and later passes
/// arrive through `j`. With LBEG off every chunk head the backedge leaves the region instead.
fn outside_loops() -> u32 {
    use Op::*;
    let mut p = Vec::new();
    p.extend(asm::addi_n(2, 2, 1));   // 0  LBEG
    p.extend(asm::addi_n(4, 4, 1));   // 2
    p.extend(asm::addi_n(5, 5, 1));   // 4  ends exactly at LEND
    p.extend(asm::addi_n(6, 6, 1));   // 6
    p.extend(asm::j(BASE + 8, BASE)); // 8
    let shape = [(0, AddiN, 0), (2, AddiN, 0), (4, AddiN, 0), (6, AddiN, 0), (8, J, 0)];
    let active = |c: &mut Cpu, lbeg: u32| { c.lcount = 3000; c.lbeg = BASE + lbeg; c.lend = BASE + 6; };
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let head: Vec<BlockInsn> = (0..3).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let mut c = cpu(0);
        assert!(emitter::region::form(&c, &mut ram, BASE, &head, true).is_none(), "one chunk without an active loop");
        active(&mut c, 0);
        let formed = emitter::region::form(&c, &mut ram, BASE, &head, true).expect("outside-loop region");
        assert_eq!(formed.loops, vec![(BASE + 6, BASE)]);
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len())).collect::<Vec<_>>(), vec![(0, 3), (6, 2)]);
    }
    for lbeg in [0, 2] {
        let before = emitter::region::SELF_LOOP_BRANCHES.load(std::sync::atomic::Ordering::Relaxed);
        // A probed head rejects the region; the own module's backedge to it then ends a dispatch
        // and a new one starts there, so a run may legally pass the head: no head bound here.
        let max = region_program("outside-loop", &p, &shape, &[], 71, 6, move |c| active(c, lbeg), 900);
        assert!(max > 3, "outside-loop region never passed its head ({max}, LBEG +{lbeg})");
        if lbeg == 0 {
            assert!(emitter::region::SELF_LOOP_BRANCHES.load(std::sync::atomic::Ordering::Relaxed) > before, "outside loop emitted no direct backedge");
        }
    }
    2
}

/// coverage-s2: `l32r a9; jx a9` with its literal on a page no chunk occupies. The region
/// predicts the literal it saw at formation; every pass flips the literal between two targets,
/// so the guarded edge runs both matched and mismatched without a page-version drop.
fn jx_literal() -> u32 {
    use Op::*;
    let (a, b) = (BASE + 262, BASE + 274);
    let mut p = a.to_le_bytes().to_vec();         // 0    the literal, alone on page 0
    p.resize(256, 0);
    p.extend(asm::l32r(9, BASE + 256, BASE));      // 256  head
    p.extend(asm::jx(9));                          // 259
    for (at, r) in [(262, 2), (274, 3)] {
        p.resize(at, 0);
        p.extend(asm::addi_n(r, r, 1));            // A 262 / B 274
        p.extend(asm::xor(10, 10, 12));            // flip the stored target
        p.extend(asm::s32i_n(10, 11, 0));
        p.extend(asm::j(BASE + at as u32 + 7, BASE + 256));
    }
    let shape = [(256, L32r, 0), (259, Jx, 0), (262, AddiN, 0), (264, Xor, 0), (267, S32iN, 0), (269, J, 256),
                 (274, AddiN, 0), (276, Xor, 0), (279, S32iN, 0), (281, J, 256)];
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let head: Vec<BlockInsn> = (0..2).scan(BASE + 256, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&cpu(0), &mut ram, BASE + 256, &head, true).expect("jx literal region");
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.jx)).collect::<Vec<_>>(), vec![(256, Some(a)), (262, None)]);
        // x6 integrate: a tails guarded copy of the trampoline chunk keeps its own prediction
        // (the copies are emitted after the last chunk, whose target is None).
        let edges = emitter::region::JX_EDGES.load(std::sync::atomic::Ordering::Relaxed);
        emitter::region::generate(&formed.chunks, &formed.pages, &formed.loops, &formed.leaves, true, Some(&[0]));
        assert_eq!(emitter::region::JX_EDGES.load(std::sync::atomic::Ordering::Relaxed), edges + 2, "the guarded copy lost its JX edge");
    }
    let before = emitter::region::JX_EDGES.load(std::sync::atomic::Ordering::Relaxed);
    region_program("jx-literal", &p, &shape, &[], 2, 262, move |c| {
        c.pc = BASE + 256;
        c.set_ar(10, a);
        c.set_ar(11, BASE);
        c.set_ar(12, a ^ b);
    }, 900);
    assert!(emitter::region::JX_EDGES.load(std::sync::atomic::Ordering::Relaxed) > before, "jx literal emitted no guarded edge");
    1
}

/// EX181: a graph whose internal forward edges skip chunks, so the emitted `br` labels are
/// not all zero: chunk 2's taken target is chunk 4 and its fallthrough is chunk 5. Two bits
/// of a counter pick the path, so every edge runs; `j 0` keeps the backward edges on the
/// dispatch table.
fn forward_edges() -> u32 {
    use Op::*;
    let mut p = Vec::new();
    p.extend(asm::addi_n(10, 10, 1));            // 0
    p.extend(asm::and(11, 10, 12));              // 2  a11 = a10 & 1
    p.extend(asm::bz(0, BASE + 5, 11, BASE + 23)); // 5  beqz a11, 23
    p.extend(asm::and(11, 10, 13));              // 8  a11 = a10 & 2
    p.extend(asm::bz(0, BASE + 11, 11, BASE + 32)); // 11 beqz a11, 32
    p.extend(asm::addi_n(4, 4, 1));              // 14
    p.extend(asm::j(BASE + 16, BASE + 46));      // 16
    p.extend(asm::nop_n());                      // 19 (never executed)
    p.extend(asm::nop_n());                      // 21
    p.extend(asm::addi_n(5, 5, 1));              // 23
    p.extend(asm::addi_n(5, 5, 1));              // 25
    p.extend(asm::j(BASE + 27, BASE + 39));      // 27
    p.extend(asm::nop_n());                      // 30
    p.extend(asm::addi_n(6, 6, 1));              // 32
    p.extend(asm::j(BASE + 34, BASE + 46));      // 34
    p.extend(asm::nop_n());                      // 37
    p.extend(asm::addi_n(7, 7, 1));              // 39
    p.extend(asm::j(BASE + 41, BASE));           // 41
    p.extend(asm::nop_n());                      // 44
    p.extend(asm::and(9, 10, 12));               // 46
    p.extend(asm::bz(0, BASE + 49, 9, BASE));    // 49 beqz a9, 0
    p.extend(asm::addi_n(8, 8, 1));              // 52
    p.extend(asm::j(BASE + 54, BASE));           // 54
    let shape = [(0, AddiN, 0), (2, And, 0), (5, Beqz, 23), (8, And, 0), (11, Beqz, 32), (14, AddiN, 0), (16, J, 46),
                 (23, AddiN, 0), (27, J, 39), (32, AddiN, 0), (34, J, 46), (39, AddiN, 0), (41, J, 0),
                 (46, And, 0), (49, Beqz, 0), (52, AddiN, 0), (54, J, 0)];
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let head: Vec<BlockInsn> = (0..3).scan(BASE, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&cpu(0), &mut ram, BASE, &head, true).expect("forward-edge region");
        // Breadth first from the head: fallthrough, then taken target (EX181 s3).
        assert_eq!(formed.chunks.iter().map(|c| c.pc - BASE).collect::<Vec<_>>(), vec![0, 8, 23, 14, 32, 39, 46, 52]);
    }
    let before = emitter::region::FORWARD_BRANCHES.load(std::sync::atomic::Ordering::Relaxed);
    let max = region_program("forward-edges", &p, &shape, &[], 3, 8, |c| {
        c.set_ar(12, 1); c.set_ar(13, 2);
    }, 1200);
    assert!(max > 3, "forward-edge region never passed its head ({max})");
    assert!(emitter::region::FORWARD_BRANCHES.load(std::sync::atomic::Ordering::Relaxed) > before, "forward-edge region emitted no direct forward branch");
    1
}

/// EX180: a region whose code lives entirely in page 0 stores into page 1 at offset zero.
/// The bumped page is outside the region's page range, so `region_store_check` does not
/// see it, but the bus also bumps page 0 — a code page of this very region. Both engines
/// must record the same pages and retire the same instructions.
fn prev_page_store() -> u32 {
    use Op::*;
    let mut p = Vec::new();
    for _ in 0..6 { p.extend(asm::addi_n(2, 2, 1)); }        // 0..12
    p.extend(asm::s8i(3, 4, 0));                             // 12: into page 1, offset 0
    p.extend(asm::j(BASE + 15, BASE + 18));                  // 15: edge to the next chunk
    for _ in 0..4 { p.extend(asm::addi_n(5, 5, 1)); }         // 18..26
    p.extend(asm::j(BASE + 26, BASE));                       // 26: back to the head
    let shape = [(0, AddiN, 0), (12, S8i, 0), (15, J, 18), (18, AddiN, 0), (26, J, 0)];
    let mut cases = 0;
    for off in [0u32, 1, 2, 3] {
        let max = region_program("prev-page-store", &p, &shape, &[], 8, 18, |c| {
            c.set_ar(3, 0x5a);
            c.set_ar(4, BASE + 0x100 + off);
        }, 300);
        assert!(max > 8, "prev-page-store: region never passed its head ({max})");
        cases += 1;
    }
    cases
}

/// store-s1: the span-fill and memset loop bodies as region self-loop chunks, from aligned,
/// misaligned and page-straddling pointers and counts that the credit cuts anywhere, in a
/// watched mapping (versions moved in bulk, never over the region's own code pages) or not.
fn store_runs() -> u32 {
    use std::sync::atomic::Ordering::Relaxed;
    use Op::*;
    let mut span = Vec::new();
    span.extend(asm::lp(9, BASE, 3, BASE + 8));     // 0  loopnez a3, 8
    span.extend(asm::s16i(6, 4, 0));                // 3  LBEG
    span.extend(asm::addi_n(4, 4, 2));              // 6  ends at LEND
    span.extend(asm::addi_n(6, 6, 1));              // 8  the next pattern
    span.extend(asm::add_n(4, 7, 2));               // 10 the next start
    span.extend(asm::j(BASE + 12, BASE));           // 12
    let span_shape = [(0, Loopnez, 8), (3, S16i, 0), (6, AddiN, 0), (8, AddiN, 0), (10, AddN, 0), (12, J, 0)];
    let mut memset = Vec::new();
    memset.extend(asm::lp(9, BASE, 3, BASE + 14));  // 0  loopnez a3, 14
    for (at, off) in [(3, 8), (5, 0), (7, 12), (9, 4)] {
        assert_eq!(memset.len(), at);
        memset.extend(asm::s32i_n(6, 4, off));      // 3..9 LBEG
    }
    memset.extend(asm::addi(4, 4, 16));             // 11 ends at LEND
    memset.extend(asm::addi_n(6, 6, 1));            // 14
    memset.extend(asm::add_n(4, 7, 2));             // 16
    memset.extend(asm::j(BASE + 18, BASE));         // 18
    let memset_shape = [(0, Loopnez, 14), (3, S32iN, 0), (11, Addi, 0), (14, AddiN, 0), (16, AddN, 0), (18, J, 0)];
    let mut cases = 0;
    for (name, p, shape) in [("span-run", &span, &span_shape[..]), ("memset-run", &memset, &memset_shape[..])] {
        // Near: the region's own page 0, and page 1, whose first bytes move page 0's version.
        for (count, start, skew, near) in [(37, BASE + 0x1000, 0, false), (37, BASE + 0x1000, 1, false), (5, BASE + 0x10f8, 0, false),
            (200, BASE + 0x1000, 0, false), (1, BASE + 0x1000, 0, false), (37, BASE + 0x2fc, 0, false), (12, BASE + 0x40, 0, true), (12, BASE + 0x100, 0, true)] {
            for unwatched in [true, false] {
                STORE_RUN_TAKEN.store(0, Relaxed);
                let done = STORE_RUN_DONE.load(Relaxed);
                let max = region_program_on(name, p, shape, &[], 1, 3, false, unwatched, move |c| {
                    c.set_ar(3, count); c.set_ar(7, start); c.set_ar(2, skew); c.set_ar(4, start + skew);
                }, 600);
                assert!(max > 3, "{name} region never passed its head ({max})");
                let allowed = skew == 0 && (unwatched || !near);
                let hit = STORE_RUN_TAKEN.load(Relaxed) != 0;
                assert!(!hit || allowed, "{name}: store run taken outside its proof");
                assert!(hit || !allowed || count < 3, "{name}: store run never taken (count {count} start {start:x} unwatched {unwatched})");
                // gen-s2: a run that covers the loop continues at LEND, inside the region.
                assert!(STORE_RUN_DONE.load(Relaxed) > done || !allowed || count < 3, "{name}: no store run finished its loop (count {count})");
                cases += 1;
            }
        }
    }
    cases
}

/// leaf-s1: a driver region calls a wrapper W that calls leaf L (CALL8 twice, nested), then
/// `l32r a8; callx8 a8` into a ROM-style trampoline T (`l32r a9; jx a9`) and leaf D. L and D use
/// the FP coprocessor. Variants: the CALLX literal flips between T and D (guarded mismatch), a
/// store rewrites L's `addi.n` immediate (1 <-> -1) every pass (L declines, so W leaves at its call), an occupied frame past W's window
/// (W's post-ENTRY proof exits), the coprocessor disabled (declines), and probes on L's head.
fn leaf_calls() -> u32 {
    use Op::*;
    let (t, d, w, l) = (BASE + 0x140, BASE + 0x180, BASE + 0x1c0, BASE + 0x300);
    let mut p = t.to_le_bytes().to_vec();              // 0    literal: T (the CALLX target)
    p.extend(d.to_le_bytes());                         // 4    literal: D (T's JX target)
    p.resize(256, 0);
    p.extend(asm::mov_n(10, 4));                       // 256  head
    p.extend(asm::call8(BASE + 258, w));               // 258  call8 W
    p.extend(asm::mov_n(4, 10));                       // 261  <- return site
    p.extend(asm::l32r(8, BASE + 263, BASE));          // 263
    p.extend(asm::callx8(8));                          // 266  callx8 T
    p.extend(asm::add_n(4, 4, 10));                    // 269  <- return site
    p.extend(asm::l32i_n(11, 13, 0));                  // 271
    p.extend(asm::xor(11, 11, 12));                    // 273
    p.extend(asm::s32i_n(11, 13, 0));                  // 276  flip the literal, or L's code
    p.extend(asm::j(BASE + 278, BASE + 256));          // 278
    p.resize(0x140, 0);
    p.extend(asm::l32r(9, t, BASE + 4));               // T
    p.extend(asm::jx(9));
    p.resize(0x180, 0);
    p.extend(asm::entry(1, 32));                       // D
    p.extend(asm::add_n(2, 2, 3));
    p.extend(asm::wfr(0, 2));
    p.extend(asm::rfr(2, 0));
    p.extend(asm::retw_n());
    p.resize(0x1c0, 0);
    p.extend(asm::entry(1, 32));                       // W
    p.extend(asm::mov_n(10, 2));
    p.extend(asm::call8(w + 5, l));
    p.extend(asm::mov_n(2, 10));
    p.extend(asm::retw_n());
    p.resize(0x300, 0);
    p.extend(asm::entry(1, 32));                       // L (its page and the one before are not the region's)
    p.extend(asm::addi_n(2, 2, 1));
    p.extend(asm::wfr(1, 2));
    p.extend(asm::rfr(2, 1));
    p.extend(asm::retw_n());
    let shape = [(256, MovN, 0), (258, Call8, 0x1c0), (261, MovN, 0), (263, L32r, 0), (266, Callx8, 0), (269, AddN, 0),
                 (271, L32iN, 0), (273, Xor, 0), (276, S32iN, 0), (278, J, 256), (0x140, L32r, 0), (0x143, Jx, 0),
                 (0x180, Entry, 0), (0x185, Wfr, 0), (0x188, Rfr, 0), (0x1c5, Call8, 0x300), (0x300, Entry, 0)];
    {
        let mut ram = Ram::new(true, false);
        ram.ram.mem[..p.len()].copy_from_slice(&p);
        let head: Vec<BlockInsn> = (0..2).scan(BASE + 256, |pc, _| { let i = crate::decode::decode(*pc, ram.fetch(*pc).unwrap()); *pc += i.len as u32; Some(BlockInsn { insn: i, max_ar: 0, straddle: false, off: 0 }) }).collect();
        let formed = emitter::region::form(&cpu(0), &mut ram, BASE + 256, &head, true).expect("leaf-call region");
        assert_eq!(formed.chunks.iter().map(|c| (c.pc - BASE, c.instructions.len(), c.leaf)).collect::<Vec<_>>(),
            vec![(256, 2, Some(1)), (261, 3, Some(2)), (269, 5, None)]);
        assert_eq!(formed.leaves.iter().map(|l| (l.pc - BASE, l.count)).collect::<Vec<_>>(), vec![(0x300, 5), (0x1c0, 10), (0x140, 7)]);
        // Leaves are inlined: the region module defines exactly one function, `run`.
        let (module, _) = emitter::region::generate(&formed.chunks, &formed.pages, &formed.loops, &formed.leaves, true, None);
        let mut at = 8;
        let functions = loop {
            let (id, mut size, mut shift) = (module[at], 0usize, 0);
            at += 1;
            loop { let b = module[at]; at += 1; size |= usize::from(b & 127) << shift; shift += 7; if b < 128 { break; } }
            if id == 3 { break &module[at..at + size]; }
            at += size;
        };
        assert_eq!(functions, [1, 0], "leaf-calls: one defined function");
    }
    let mut cases = 1;
    for (label, lit, flip, occupied, cp, turns) in [
        ("leaf-calls", BASE, 0, false, 1, 900),
        ("leaf-calls-callx-flip", BASE, t ^ d, false, 1, 900),
        ("leaf-calls-jx-literal-flip", BASE + 4, d ^ l, false, 1, 900), // T's own JX literal: D <-> L
        ("leaf-calls-underflow", BASE, 0, false, 1, 900),
        ("leaf-calls-code-page", BASE + 0x300, 0x1000_0000, false, 1, 600),
        ("leaf-calls-overflow", BASE, 0, true, 1, 300),
        ("leaf-calls-no-coprocessor", BASE, 0, false, 0, 300),
    ] {
        let before = LEAF_RETURNS.load(std::sync::atomic::Ordering::Relaxed);
        region_program(label, &p, &shape, &[], 2, 0x300, move |c| {
            c.pc = BASE + 256;
            c.ps = ps::WOE;
            c.cpenable = cp;
            c.windowstart = (1 << c.windowbase) | if occupied { 1 << ((c.windowbase + 4) % 16) } else { 0 };
            c.set_ar(1, BASE + 0x4000);
            c.set_ar(12, flip);
            c.set_ar(13, lit);
        }, turns);
        // The occupied frame ends in the overflow vector, whose code is not a leaf's.
        let taken = LEAF_RETURNS.load(std::sync::atomic::Ordering::Relaxed) > before;
        assert!(occupied || taken == (cp != 0), "{label}: inline leaves returned: {taken}");
        cases += 1;
    }
    // gen-s1: whole-window reloads and spills address register quads; every window position,
    // including those where the renamed leaf registers (a16..a31) wrap the 64-register file.
    for wb in 0..16 {
        region_program(&format!("leaf-calls-wb{wb}"), &p, &shape, &[], 2, 0x300, move |c| {
            c.pc = BASE + 256;
            c.ps = ps::WOE;
            c.cpenable = 1;
            c.windowbase = wb;
            c.windowstart = 1 << wb;
            c.set_ar(1, BASE + 0x4000);
            c.set_ar(12, 0);
            c.set_ar(13, BASE);
        }, 200);
        cases += 1;
    }
    cases
}

/// hop-s2: a head that cannot form a region (one chunk ending in JX) takes its own module without
/// the admission facts once its tries are spent; a region formed later at another head that covers
/// it (a coverage insertion) ends that, and the next entry there enters the covering region.
fn uncovered_verdict() -> u32 {
    let mut p = Vec::new();
    p.extend(asm::addi_n(3, 3, 1));        // 0  the uncovered head: one chunk
    p.extend(asm::addi_n(5, 5, 1));        // 2
    p.extend(asm::jx(7));                  // 4
    p.extend([0; 9]);
    p.extend(asm::addi_n(6, 6, 1));        // 16 a later head whose region covers 0
    p.extend(asm::j(BASE + 18, BASE));     // 18
    let mut c = cpu(7);
    let mut ram = Ram::new(true, false);
    ram.ram.mem[..p.len()].copy_from_slice(&p);
    c.set_ar(7, BASE);
    let stat = |i: usize| REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
    let (failed, early) = (stat(1), stat(17));
    for _ in 0..60 { c.pc = BASE; assert_eq!(crate::block::run_block(&mut c, &mut ram, 3), (3, None)); }
    assert!(stat(1) >= failed + REGION_TRIES as u32, "uncovered: formation tries not spent");
    assert!(stat(17) > early + 10, "uncovered: no early own-module entries");
    let (formed, covered) = (stat(0), stat(10));
    for _ in 0..60 { c.pc = BASE + 16; crate::block::run_block(&mut c, &mut ram, 2); }
    assert!(stat(0) > formed, "uncovered: no covering region");
    let early = stat(17);
    c.pc = BASE;
    let a3 = c.get_ar(3);
    assert_eq!(crate::block::run_block(&mut c, &mut ram, 3), (3, None));
    assert_eq!((stat(17), c.get_ar(3)), (early, a3.wrapping_add(1)), "uncovered: a stale verdict ran the own module");
    assert!(stat(10) > covered, "uncovered: the entry did not find its covering region");
    1
}
