use super::*;

pub(super) fn priced_cases_part(part: usize) -> u32 {
    use std::sync::atomic::Ordering::Relaxed;
    assert!(!PRICED.swap(true, Relaxed));
    let cases = match part {
        0 => directed_cases(),
        1..=3 => arithmetic::integer_ops_part(part - 1),
        4..=7 => float::floating_point_part(part - 4),
        8 => float::floating_point_guard_proof(),
        9..=19 => control::entry_and_shifts_part(part - 9),
        20 => control::terminal_helpers(),
        21 => control::special_register_blocks(),
        22 => control::ps_terminals(),
        _ => panic!("invalid priced test group"),
    };
    PRICED.store(false, Relaxed);
    cases
}

fn directed_cases() -> u32 {
    use Op::*;
    let mut cases = 0;
    // Shared-table goldens are needed in addition to backend parity: both backends
    // could otherwise agree on the same incorrect readiness calculation.
    let mut producer = insn(AddS); producer.insn.r = 0; producer.insn.s = 1; producer.insn.t = 2;
    let mut overwrite = insn(Wfr); overwrite.insn.r = 0;
    let mut reader = insn(Rfr); reader.insn.s = 0;
    for middle in [overwrite, insn(Quou)] {
        let block = [producer, middle, reader];
        assert_eq!(crate::exec::static_extras(block.iter().map(|b| &b.insn)), vec![0, 0, 0]);
        cases += 1;
    }
    let mut consumer = insn(AddS); consumer.insn.s = 0;
    assert_eq!(crate::exec::static_extras([producer, consumer].iter().map(|b| &b.insn)), vec![0, 3]);
    cases += 1;
    for op in [Quou, Quos, Remu, Rems] {
        for (left, right) in [(7, 2), (0x8000_0000, u32::MAX), (5, 0)] {
            for entry in 0..2 { for budget in 1..=2 {
                compare(&mut [insn(Nop), insn(op)], Case { entry, budget, ..Case::default() }, |c| { c.set_ar(4, left); c.set_ar(5, right); });
                cases += 1;
            } }
        }
    }
    for op in [Loop, Loopnez, Loopgtz, Beqz, Bnez, Bltz, Bgez] {
        for value in [0, 1, u32::MAX] { for loop_end in [false, true] {
            let mut transfer = insn(op); transfer.insn.imm = (BASE + 0x100) as i32;
            compare(&mut [insn(Nop), transfer], Case { budget: 2, loop_end, ..Case::default() }, |c| c.set_ar(4, value));
            cases += 1;
        } }
    }
    // An actual taken transfer can target the fall-through PC; pc!=next alone
    // must not determine whether it gets its branch/alignment price.
    for op in [J, Call0, Beqz] {
        let mut transfer = insn(op); transfer.insn.imm = (BASE + 6) as i32;
        compare(&mut [insn(Nop), transfer], Case { budget: 2, ..Case::default() }, |c| c.set_ar(4, 0));
        cases += 1;
    }
    // Include indirect targets that straddle a fetch word.
    for op in [Jx, Callx0, Callx4, Callx8, Callx12] {
        for flags in [0, ps::WOE] { for offset in [0x100, 0x102, 0x103] {
            compare(&mut [insn(Nop), insn(op)], Case { budget: 2, ..Case::default() }, |c| { c.ps = flags; c.set_ar(4, BASE + offset); });
            cases += 1;
        } }
    }
    for op in [L32i, L32ai, L32e, S32c1i] {
        let mut load = insn(op); load.insn.t = 5; load.insn.imm = 0;
        load.max_ar = crate::exec::max_ar(&load.insn);
        let mut use_loaded = insn(Add); use_loaded.insn.s = 5;
        use_loaded.max_ar = crate::exec::max_ar(&use_loaded.insn);
        assert_eq!(crate::exec::static_extras([load, use_loaded].iter().map(|b| &b.insn)), vec![0, 1]);
        for entry in 0..2 { for budget in 1..=2 {
            compare(&mut [load, use_loaded], Case { entry, budget, fast: true, ..Case::default() }, |c| c.set_ar(4, BASE + 0x1000));
            cases += 1;
        } }
    }
    cases += executed_fetch_batches();
    // A dependency wait must not survive a faulting helper when the interpreter
    // charges extras only for successful instructions.
    let mut load = insn(L32i); load.insn.t = 4; load.insn.imm = 0;
    load.max_ar = crate::exec::max_ar(&load.insn);
    compare(&mut [load, insn(FloatS)], Case { budget: 2, fast: true, ..Case::default() }, |c| { c.set_ar(4, BASE + 0x1000); c.cpenable = 0; });
    cases += 1;
    let mut store = insn(Ssi); store.insn.t = 0; store.insn.imm = 0;
    store.max_ar = crate::exec::max_ar(&store.insn);
    compare(&mut [producer, store], Case { budget: 2, fast: true, readonly: true, ..Case::default() }, |c| { c.set_ar(4, BASE + 0x1000); c.cpenable = 1; });
    cases += 1;

    cases
}

fn executed_fetch_batches() -> u32 {
    use std::sync::atomic::Ordering::Relaxed;
    const FLASH: u32 = 0x4200_0000;
    assert!(!FETCH_RING.swap(true, Relaxed));
    for budget in [1, 12, 64] {
        let mut block = vec![insn(Op::Movi); 16];
        let mut cache = CodeCache::new(0).unwrap();
        let code = queue(&mut cache, &mut block, FLASH, false);
        for _ in 0..HOT { ready(&cache, code, 0); }
        assert!(ready(&cache, code, 0));
        let mut cpu = Cpu::new(0);
        cpu.pc = FLASH;
        cpu.ps = 0;
        cpu.price_control = true;
        cpu.icache_fill = 5;
        cpu.lbeg = FLASH;
        cpu.lend = FLASH + 36;
        cpu.lcount = 10;
        let mut ram = FlatRam::new(FLASH, 4096);
        // SAFETY: the ready code and matching helpers remain live throughout this call.
        let result = unsafe { run(&cache, code, &mut cpu, &mut ram, Helpers::shared::<FlatRam>(), budget, 0, None) };
        assert_eq!(result & 0xffff, budget, "fetch accounting must preserve multi-instruction loop batching");
        assert_eq!(cpu.icache_misses, if budget == 1 { 1 } else { 2 });
        assert_eq!(cpu.timing_extra, if budget == 1 { 5 } else { 10 });
    }
    FETCH_RING.store(false, Relaxed);
    3
}
