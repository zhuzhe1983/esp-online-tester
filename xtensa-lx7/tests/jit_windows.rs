//! Native window helpers against the independent block interpreter: links, rotation, traps,
//! loop tails, aliased CALLX8 targets, budget cuts and timer delivery.
#![cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]

use xtensa_lx7::block::run_block;
use xtensa_lx7::state::{exc, ps, vec};
use xtensa_lx7::{decode, Bus, Cpu, Fault, FlatRam, Op, Trap};

const CODE: u32 = 0x4037_0000;

struct CodeBus { code: FlatRam, versions: [u32; 2], stop: bool, noted_pc: u32 }
impl Bus for CodeBus {
    fn note_code_page(&mut self, _: u32) {}
    fn read8(&mut self, _: u32) -> Result<u8, Fault> { Err(Fault::Unmapped) }
    fn read16(&mut self, _: u32) -> Result<u16, Fault> { Err(Fault::Unmapped) }
    fn read32(&mut self, _: u32) -> Result<u32, Fault> { Err(Fault::Unmapped) }
    fn write8(&mut self, _: u32, _: u8) -> Result<(), Fault> { Err(Fault::Unmapped) }
    fn write16(&mut self, _: u32, _: u16) -> Result<(), Fault> { Err(Fault::Unmapped) }
    fn write32(&mut self, _: u32, _: u32) -> Result<(), Fault> { Err(Fault::Unmapped) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> { self.code.fetch(pc) }
    fn page_versions(&self) -> &[u32] { &self.versions }
    fn note_pc(&mut self, pc: u32) { self.noted_pc = pc; }
    fn block_break(&self) -> bool { self.stop && self.noted_pc == CODE + 2 }
}

#[derive(Debug, PartialEq, Eq)]
struct State {
    ar: [u32; 64], pc: u32, ps: u32, wb: u32, ws: u32, epc: [u32; 8], depc: u32,
    cause: u32, waiting: bool, lcount: u32, insns: u64, ccount: u32, interrupt: u32,
}
fn state(c: &Cpu) -> State {
    State { ar: c.ar, pc: c.pc, ps: c.ps, wb: c.windowbase, ws: c.windowstart, epc: c.epc, depc: c.depc,
        cause: c.exccause, waiting: c.waiting, lcount: c.lcount, insns: c.insn_count, ccount: c.ccount, interrupt: c.interrupt }
}
fn bus(bytes: &[u8]) -> CodeBus {
    let mut b = CodeBus { code: FlatRam::new(CODE, 64), versions: [0; 2], stop: false, noted_pc: 0 };
    b.code.mem[..bytes.len()].copy_from_slice(bytes);
    b.code.mem[bytes.len()..bytes.len()+3].copy_from_slice(&[0x06, 0xff, 0xff]); // j self
    b
}
fn setup(c: &mut Cpu, wb: u32, status: u32, ws: u32, n: u32, looped: bool) {
    c.ar = std::array::from_fn(|i| 0xffff_0000 | i as u32);
    c.pc = CODE; c.ps = status; c.windowbase = wb; c.windowstart = ws;
    c.set_ar(0, (n << 30) | ((CODE + 0x20) & 0x3fff_ffff));
    c.epc = [0; 8]; c.depc = 0; c.exccause = 123; c.vecbase = CODE + 0x1000;
    c.waiting = false; c.lbeg = CODE + 0x30; c.lend = if looped { CODE + 3 } else { 0 }; c.lcount = 2;
    c.insn_count = 0; c.ccount = 0; c.interrupt = 0; c.intenable = 0; c.jit_trap = None;
}

#[test]
fn window_helpers_match_the_interpreter_across_windows_and_exceptions() {
    let mut cases = vec![(Op::Call8, vec![0x25, 0, 0]), (Op::Retw, vec![0x90, 0, 0]), (Op::RetwN, vec![0x1d, 0xf0])];
    for s in [0, 1, 3, 8, 15] { cases.push((Op::Callx8, vec![0xe0, s, 0])); }
    for s in [0, 1, 3, 4] { cases.push((Op::Entry, vec![0x36, 0x40 | s, 0])); } // entry aS,32; s4 is illegal
    let mut checked = 0;
    for (op, bytes) in cases {
        let (mut nb, mut ib) = (bus(&bytes), bus(&bytes));
        assert_eq!(decode(CODE, nb.code.fetch(CODE).unwrap()).op, op);
        let (mut native, mut oracle) = (Cpu::new(0), Cpu::new(0));
        native.blocks.jit_enabled = true; oracle.blocks.jit_enabled = false;
        for wb in 0u32..16 {
            for status in [0, ps::EXCM, ps::WOE, ps::WOE | ps::EXCM, ps::WOE | ps::UM] {
                for inc in 0..4 {
                    for n in 0..4 {
                        let returned = wb.wrapping_sub(n) & 15;
                        for ws in [0, 1 << wb, 1 << returned, 0xffff] {
                            for looped in [false, true] {
                                let status = status | (inc << ps::CALLINC_SHIFT) | (5 << ps::OWB_SHIFT);
                                setup(&mut native, wb, status, ws, n, looped);
                                setup(&mut oracle, wb, status, ws, n, looped);
                                if looped { native.lend = CODE + bytes.len() as u32; oracle.lend = native.lend; }
                                if op == Op::Entry {
                                    let s = decode(CODE, nb.code.fetch(CODE).unwrap()).s;
                                    native.set_ar(s, 16); oracle.set_ar(s, 16); // stack subtraction wraps
                                }
                                let compiled = native.blocks.jit_instructions;
                                let nr = run_block(&mut native, &mut nb, 1);
                                let ir = run_block(&mut oracle, &mut ib, 1);
                                assert_eq!(nr, ir, "{op:?} wb{wb} ps{status:x} ws{ws:x} ret{n}");
                                assert_eq!(state(&native), state(&oracle), "{op:?} wb{wb} ps{status:x} ws{ws:x} ret{n}");
                                assert_eq!(native.blocks.jit_instructions - compiled, native.insn_count, "native path must actually run {op:?}");
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 122_880);
}

#[test]
fn entry_reloads_the_rotated_window_and_preserves_budget_and_bus_exits() {
    for stop in [false, true] {
        for budgets in [[1, 2], [2, 1], [3, 0]] {
            let execute = |jit| {
                let mut b = bus(&[0x1b, 0x66, 0x36, 0x41, 0x00, 0x1b, 0x66]); // addi.n a6; entry a1,32; addi.n a6
                b.stop = stop;
                let mut c = Cpu::new(0); c.blocks.jit_enabled = jit;
                setup(&mut c, 15, ps::WOE | (2 << ps::CALLINC_SHIFT), 1 << 15, 2, false);
                let mut results = Vec::new();
                for budget in budgets { if budget > 0 { results.push(run_block(&mut c, &mut b, budget)); } }
                (state(&c), results)
            };
            assert_eq!(execute(true), execute(false), "bus stop{stop} budgets{budgets:?}");
        }
    }
}

#[test]
fn window_overflow_and_timer_traps_keep_their_original_instruction_boundary() {
    for jit in [false, true] {
        let mut b = bus(&[0x25, 0, 0]);
        let mut c = Cpu::new(0); c.blocks.jit_enabled = jit;
        setup(&mut c, 15, ps::WOE, (1 << 15) | 1, 2, false);
        // Trap-before consumes one scheduler step without retiring a guest instruction.
        assert_eq!(run_block(&mut c, &mut b, 1), (1, Some(Trap::Exception(0x203))));
        assert_eq!((c.epc[1], c.pc, c.insn_count), (CODE, c.vecbase + vec::WINDOW_OF12, 0));

        let mut b = bus(&[0x36, 0x41, 0, 0x1b, 0x66]);
        let mut c = Cpu::new(0); c.blocks.jit_enabled = jit;
        setup(&mut c, 0, ps::WOE | (2 << ps::CALLINC_SHIFT), 1, 2, false);
        c.intenable = 1 << 6; c.ccompare[0] = 1;
        assert_eq!(run_block(&mut c, &mut b, 2), (1, None));
        assert_eq!((c.pc, c.ccount, c.windowbase), (CODE + 3, 1, 2));
        assert_eq!(run_block(&mut c, &mut b, 1), (1, Some(Trap::Interrupt(6))));
        assert_eq!((c.epc[1], c.exccause), (CODE + 3, exc::LEVEL1_INTERRUPT));
    }
}
