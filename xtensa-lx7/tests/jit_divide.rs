//! Native code divides (QUOU, QUOS, REMU, REMS) without the interpreter, with the interpreter's
//! result: quotients and remainders, i32::MIN / -1, the zero-divisor exception and the loop back-edge.
#![cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]

use xtensa_lx7::block::run_block;
use xtensa_lx7::state::exc;
use xtensa_lx7::{decode, Bus, Cpu, Fault, FlatRam, Op, Trap};

const CODE: u32 = 0x4037_0000;

// quou/quos/remu/rems a3, a4, a5 (r = 3, s = 4, t = 5) and the same into a5 (r = t)
const CASES: [(Op, [u8; 3]); 8] = [
    (Op::Quou, [0x50, 0x34, 0xc2]), (Op::Quos, [0x50, 0x34, 0xd2]),
    (Op::Remu, [0x50, 0x34, 0xe2]), (Op::Rems, [0x50, 0x34, 0xf2]),
    (Op::Quou, [0x50, 0x54, 0xc2]), (Op::Quos, [0x50, 0x54, 0xd2]),
    (Op::Remu, [0x50, 0x54, 0xe2]), (Op::Rems, [0x50, 0x54, 0xf2]),
];

// (dividend, divisor): signs, i32::MIN / -1, a divisor above the dividend, a dividend of 0.
const OPERANDS: [(u32, u32); 9] = [
    (100, 7), (0xffff_ff9c, 7), (100, 0xffff_fff9), (0xffff_ff9c, 0xffff_fff9),
    (0x8000_0000, 0xffff_ffff), (0x8000_0000, 1), (5, 0x8000_0000), (0, 3), (0xffff_ffff, 0x10),
];

struct CodeBus { code: FlatRam, versions: [u32; 2], interpreted: u32 }

impl Bus for CodeBus {
    fn note_code_page(&mut self, _vidx: u32) {} // No guest stores: the code never changes.
    fn read8(&mut self, _: u32) -> Result<u8, Fault> { Err(Fault::Unmapped) }
    fn read16(&mut self, _: u32) -> Result<u16, Fault> { Err(Fault::Unmapped) }
    fn read32(&mut self, _: u32) -> Result<u32, Fault> { Err(Fault::Unmapped) }
    fn write8(&mut self, _: u32, _: u8) -> Result<(), Fault> { Err(Fault::Unmapped) }
    fn write16(&mut self, _: u32, _: u16) -> Result<(), Fault> { Err(Fault::Unmapped) }
    fn write32(&mut self, _: u32, _: u32) -> Result<(), Fault> { Err(Fault::Unmapped) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> { self.code.fetch(pc) }
    fn page_versions(&self) -> &[u32] { &self.versions }
    // The interpreter helper reports its pc; compiled code does not.
    fn note_pc(&mut self, _pc: u32) { self.interpreted += 1; }
}

struct Run { result: (u32, Option<Trap>), cpu: Cpu, bus: CodeBus }

/// One instruction then `j self`, with a4 = `dividend`, a5 = `divisor`, and a loop whose end is
/// the next instruction when `looped`.
fn run(op: Op, bytes: [u8; 3], jit: bool, dividend: u32, divisor: u32, looped: bool) -> Run {
    let mut bus = CodeBus { code: FlatRam::new(CODE, 16), versions: [0; 2], interpreted: 0 };
    bus.code.mem[..3].copy_from_slice(&bytes);
    bus.code.mem[3..6].copy_from_slice(&[0x06, 0xff, 0xff]);
    let insn = decode(CODE, bus.code.fetch(CODE).unwrap());
    assert_eq!((insn.op, insn.s, insn.t), (op, 4, 5));
    let mut cpu = Cpu::new(0);
    cpu.ps = 0;
    cpu.blocks.jit_enabled = jit;
    cpu.set_ar(3, 0x1234_5678);
    cpu.set_ar(4, dividend);
    cpu.set_ar(5, divisor);
    if looped { cpu.lbeg = CODE + 0x40; cpu.lend = CODE + 3; cpu.lcount = 2; }
    cpu.pc = CODE;
    let result = run_block(&mut cpu, &mut bus, 1);
    if jit { assert!(cpu.blocks.jit_instructions > 0, "{op:?}: must exercise compiled code"); } else { assert_eq!(cpu.blocks.jit_instructions, 0); }
    Run { result, cpu, bus }
}

/// Retired count, trap, pc, a0..a15, LCOUNT, EPC1, EXCCAUSE, PS.
type State = (u32, Option<Trap>, u32, [u32; 16], u32, u32, u32, u32);

fn state(r: &Run) -> State {
    (r.result.0, r.result.1, r.cpu.pc, std::array::from_fn(|i| r.cpu.get_ar(i as u8)), r.cpu.lcount, r.cpu.epc[1], r.cpu.exccause, r.cpu.ps)
}

fn expected(op: Op, a: u32, b: u32) -> u32 {
    match op {
        Op::Quou => a / b,
        Op::Remu => a % b,
        Op::Quos => (a as i32).wrapping_div(b as i32) as u32,
        _ => (a as i32).wrapping_rem(b as i32) as u32,
    }
}

#[test]
fn divides_compile_and_match_the_interpreter() {
    for (op, bytes) in CASES {
        let r = bytes[1] >> 4;
        for (a, b) in OPERANDS {
            let (n, i) = (run(op, bytes, true, a, b, false), run(op, bytes, false, a, b, false));
            assert_eq!(state(&n), state(&i), "{op:?} a{r} {a:#x} {b:#x}");
            assert_eq!(n.result, (1, None), "{op:?} {a:#x} {b:#x}");
            assert_eq!(n.bus.interpreted, 0, "{op:?} {a:#x} {b:#x}: must not call the interpreter");
            assert_eq!(n.cpu.get_ar(r), expected(op, a, b), "{op:?} a{r} {a:#x} {b:#x}");
            assert_eq!(n.cpu.pc, CODE + 3, "{op:?}");
        }
    }
}

#[test]
fn divides_take_the_loop_back_edge() {
    for (op, bytes) in CASES {
        let (n, i) = (run(op, bytes, true, 100, 7, true), run(op, bytes, false, 100, 7, true));
        assert_eq!(state(&n), state(&i), "{op:?}");
        assert_eq!((n.cpu.pc, n.cpu.lcount), (CODE + 0x40, 1), "{op:?}");
        assert_eq!(n.bus.interpreted, 0, "{op:?}");
    }
}

#[test]
fn mid_block_divides_match_the_interpreter_in_rotated_windows() {
    for (op, bytes) in CASES {
        for windowbase in [0, 3] {
            for divisor in [0, 7, (-7i32) as u32] {
                let execute = |jit| {
                    let mut bus = CodeBus { code: FlatRam::new(CODE, 16), versions: [0; 2], interpreted: 0 };
                    bus.code.mem[..2].copy_from_slice(&[0x1b, 0x66]); // addi.n a6, a6, 1
                    bus.code.mem[2..5].copy_from_slice(&bytes);
                    bus.code.mem[5..8].copy_from_slice(&[0x06, 0xff, 0xff]);
                    assert_eq!(decode(CODE, bus.code.fetch(CODE).unwrap()).op, Op::AddiN);
                    let mut cpu = Cpu::new(0);
                    cpu.ps = 0; cpu.windowbase = windowbase; cpu.blocks.jit_enabled = jit;
                    cpu.set_ar(3, 0x1234_5678); cpu.set_ar(4, 100); cpu.set_ar(5, divisor); cpu.set_ar(6, 42);
                    cpu.pc = CODE;
                    let result = run_block(&mut cpu, &mut bus, 2);
                    if jit { assert!(cpu.blocks.jit_instructions > 0); }
                    Run { result, cpu, bus }
                };
                let (n, i) = (execute(true), execute(false));
                assert_eq!(state(&n), state(&i), "{op:?} windowbase {windowbase} divisor {divisor:#x}");
                assert_eq!(n.cpu.get_ar(6), 43);
                if divisor == 0 { assert_eq!(n.cpu.epc[1], CODE + 2); }
            }
        }
    }
}

#[test]
fn a_zero_divisor_raises_through_the_interpreter() {
    for (op, bytes) in CASES {
        for looped in [false, true] {
            let (n, i) = (run(op, bytes, true, 100, 0, looped), run(op, bytes, false, 100, 0, looped));
            assert_eq!(state(&n), state(&i), "{op:?} looped {looped}");
            assert_eq!(n.result, (1, Some(Trap::Exception(exc::DIVIDE_BY_ZERO))), "{op:?}");
            assert_eq!((n.cpu.epc[1], n.cpu.exccause), (CODE, exc::DIVIDE_BY_ZERO), "{op:?}");
            assert_eq!((n.cpu.get_ar(3), n.cpu.get_ar(4), n.cpu.get_ar(5)), (0x1234_5678, 100, 0), "{op:?}: nothing may change");
            assert_eq!(n.bus.interpreted, 1, "{op:?}: the interpreter raises the exception");
        }
    }
}
