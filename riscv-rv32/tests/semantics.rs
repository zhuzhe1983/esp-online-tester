//! Hermetic tests of the RV32IMC core on a flat RAM: arithmetic and memory through a loop,
//! `step` against `Core::run`, vectored interrupts through the C3's `mtvec` mode, `ecall`,
//! `ebreak`, `mret`; RV32A on the C6 core and RV32F with its lazy-FPU traps on the P4 core.
//! Programs assembled with `riscv32-esp-elf-as -march=rv32imc_zicsr` (`rv32imac_zicsr`,
//! `rv32imafc_zicsr` for the A and F programs).
use emu_core::Core;
use riscv_rv32::state::{csr, mstatus, MISA_RV32IMAFC};
use riscv_rv32::{decode, decode_for, disasm, step, Cpu, FlatRam, Trap};

const BASE: u32 = 0x4038_0000;

/// objdump prints each RISC-V instruction as one little-endian word (or half-word) in hex.
fn asm(listing: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for g in listing.split_whitespace() { let w = u32::from_str_radix(g, 16).unwrap(); out.extend_from_slice(&w.to_le_bytes()[..g.len() / 2]); }
    out
}

fn machine(prog: &[u8], first: &str) -> (Cpu, FlatRam) { machine_on(Cpu::new(), prog, first) }

fn machine_on(mut cpu: Cpu, prog: &[u8], first: &str) -> (Cpu, FlatRam) {
    let mut ram = FlatRam::new(BASE, 64 * 1024);
    ram.mem[..prog.len()].copy_from_slice(prog);
    cpu.pc = BASE;
    let mut b = [0u8; 4]; b.copy_from_slice(&ram.mem[..4]);
    assert_eq!(disasm::format_for(cpu.misa, &decode_for(cpu.misa, BASE, b)).replace('\t', " "), first, "the pasted program does not start with the expected instruction");
    (cpu, ram)
}

const MIX: &str = "40380537 10050513 4581 2bc00613 c10c 4114 059d 068a 00b6c733 00e51223 00455783 00f50423 00854803 98c2 fec591e3 006000ef a019 02a5 8082 a001";

#[test]
fn step_and_run_agree_and_the_loop_computes() {
    let (mut a, mut ra) = machine(&asm(MIX), "lui a0,0x40380");
    for _ in 0..1500 { step(&mut a, &mut ra).unwrap(); }
    let (mut b, mut rb) = machine(&asm(MIX), "lui a0,0x40380");
    let mut left = 1500;
    while left > 0 { let (used, t) = b.run(&mut rb, left); assert_eq!(t, None); left -= used; }
    assert_eq!(a.x, b.x); assert_eq!(a.pc, b.pc); assert_eq!(a.insn_count, b.insn_count); assert_eq!(a.retired_count, b.retired_count); assert_eq!(ra.mem[0x100..0x110], rb.mem[0x100..0x110]);
    assert_eq!(a.x[5], 9, "the call after the loop ran (t0 += 9)");
    assert_eq!(a.x[11], 700, "a1 counted 100 iterations of 7");
    assert_eq!(a.pc, BASE + asm(MIX).len() as u32 - 2, "parked on the final j");
}

/// A pending line the SoC hands over is taken through `mtvec` vectored mode at `base + 4*line`,
/// with mcause/mepc/mstatus as the handler expects; `mret` returns with MIE restored.
#[test]
fn vectored_interrupt_and_mret() {
    let (mut cpu, mut ram) = machine(&asm("403802b7 20028293 0012e293 30529073 4321 30431073 30046073 4501 0505 bffd"), "lui t0,0x40380");
    let handler = asm("342023f3 34102e73 0e11 341e1073 30200073");   // csrr t2,mcause; csrr t3,mepc; addi t3,t3,4; csrw mepc,t3; mret
    let vector = 0x200 + 4 * 3;
    ram.mem[vector..vector + handler.len()].copy_from_slice(&handler);
    for _ in 0..9 { step(&mut cpu, &mut ram).unwrap(); }
    assert!(cpu.mie_enabled());
    assert!(!cpu.irq_pending());
    cpu.set_irq(Some(3));
    assert!(cpu.irq_pending());
    let pc_before = cpu.pc;
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Interrupt(3)));
    assert_eq!(cpu.pc, BASE + vector as u32);
    assert_eq!(cpu.mcause, 0x8000_0003); assert_eq!(cpu.mepc, pc_before);
    assert!(!cpu.mie_enabled(), "MIE cleared on entry");
    cpu.set_irq(None);                                     // the handler acknowledged the source
    for _ in 0..5 { step(&mut cpu, &mut ram).unwrap(); }   // ... mret
    assert_eq!(cpu.x[7], 0x8000_0003, "the handler read mcause");
    assert_eq!(cpu.pc, pc_before + 4);
    assert!(cpu.mie_enabled(), "MIE restored by mret");
    assert_eq!(cpu.read_csr(csr::MSTATUS) & (1 << 7), 1 << 7, "MPIE set");
}

#[test]
fn ecall_and_ebreak_trap() {
    let (mut cpu, mut ram) = machine(&asm("00000073 00100073"), "ecall");
    let mut mt = FlatRam::new(0, 16); let _ = &mut mt;
    cpu.write_csr(csr::MTVEC, BASE + 0x100);
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(11)));
    assert_eq!(cpu.mcause, 11); assert_eq!(cpu.mepc, BASE); assert_eq!(cpu.pc, BASE + 0x100);
    cpu.pc = BASE + 4;
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Ebreak(BASE + 4)));
    assert_eq!(cpu.mcause, 3);
}

#[test]
fn compressed_and_full_lengths() {
    assert_eq!(Cpu::insn_len([0x21, 0x43, 0, 0]), 2);          // c.li
    assert_eq!(Cpu::insn_len([0xb7, 0x02, 0x38, 0x40]), 4);    // lui
}

/// RV32A (the C6 core): lr/sc with a matching and a stale reservation, the read-modify-write
/// AMOs including the signed/unsigned min-max pair, and the aq/rl suffixes in the disassembly.
/// Assembled with `-march=rv32imac_zicsr`.
const ATOMICS: &str = "40380537 10050513 4595 c10c 1005262f 061d 18c526af 18c5272f 478d 00f5282f 57fd a0f528af e0f522af 0eb5232f 00052383 64f52e2f a001";

#[test]
fn atomics_reserve_swap_and_combine() {
    let (mut cpu, mut ram) = machine(&asm(ATOMICS), "lui a0,0x40380");
    for _ in 0..16 { step(&mut cpu, &mut ram).unwrap(); }
    let word = |ram: &FlatRam| u32::from_le_bytes(ram.mem[0x100..0x104].try_into().unwrap());
    assert_eq!(cpu.x[12], 12, "lr.w read the stored 5, then +7");
    assert_eq!(cpu.x[13], 0, "sc.w with a live reservation succeeds");
    assert_eq!(cpu.x[14], 1, "a second sc.w finds no reservation and fails");
    assert_eq!(cpu.x[16], 12, "amoadd.w returns the old value");
    assert_eq!(cpu.x[17], 15, "amomax.w keeps 15 over -1");
    assert_eq!(cpu.x[5], 15, "amomaxu.w returns the old 15");
    assert_eq!(cpu.x[6], 0xffff_ffff, "amomaxu.w stored 0xffffffff, which amoswap.w read back");
    assert_eq!(cpu.x[7], 5, "the swap stored a1 = 5");
    assert_eq!(cpu.x[28], 5, "amoand.w returns the old value");
    assert_eq!(word(&ram), 5, "5 & -1 = 5 stays in memory");
    assert_eq!(cpu.pc, BASE + asm(ATOMICS).len() as u32 - 2, "parked on the final j");
    let d = |w: u32| disasm::format(&decode(0, w.to_le_bytes())).replace('\t', " ");
    assert_eq!(d(0x0eb5232f), "amoswap.w.aqrl t1,a1,(a0)");
    assert_eq!(d(0x64f52e2f), "amoand.w.aq t3,a5,(a0)");
    assert_eq!(d(0x1005262f), "lr.w a2,(a0)");
    assert_eq!(d(0x18c526af), "sc.w a3,a2,(a0)");
    assert_eq!(cpu.read_csr(csr::MISA), 0x4000_1104, "the default core still reports RV32IMC");
    assert_eq!(riscv_rv32::Cpu::new_rv32imac().read_csr(csr::MISA), 0x4000_1105, "the C6 core reports RV32IMAC");
}

/// RV32F on the P4 core, through instructions: static and dynamic rounding, accrued flags, the
/// canonical NaN, -0 in fmin/fmax, conversions that clip, quiet and signaling compares, fclass,
/// the fused multiply-adds (the n-forms negate the product, so -(+0) + +0 is +0), FS = Dirty.
const FARITH: &str = "000022b7 3002a073 3f800537 f0050553 00300593 d005f5d3 18b57653 18b516d3 18b53753 00215073 18b577d3 00102673 00101073 \
    f0000053 180570d3 18007153 e00106d3 00102773 200011d3 28300253 280192d3 c000f7d3 c0117853 c00638d3 00101073 a0212953 001029f3 \
    a0a11a53 00102af3 e0019b53 00200313 d0037353 50b37443 50b374c7 50b3794b 50b379cf 00205073 00a07a43 00a07acb 00a07b4f 5805fbd3 \
    20352c53 c01c1bd3 00302c73 30002cf3 0000006f";

#[test]
fn f_arithmetic_rounds_flags_and_canonicalises() {
    let n = asm(FARITH).len() / 4 - 1;
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &asm(FARITH), "lui t0,0x2");
    for _ in 0..n { step(&mut cpu, &mut ram).unwrap(); }
    let (f, x) = (cpu.f, cpu.x);
    assert_eq!(cpu.pc, BASE + 4 * n as u32, "parked on the final j");
    assert_eq!((f[12], f[13], f[14], f[15]), (0x3eaa_aaab, 0x3eaa_aaaa, 0x3eaa_aaab, 0x3eaa_aaaa), "1/3 in dyn (RNE), rtz, rup, dyn (RDN)");
    assert_eq!(x[12], 0x01, "1/3 is inexact");
    assert_eq!((f[1], f[2], x[13], x[14]), (0x7f80_0000, 0x7fc0_0000, 0x7fc0_0000, 0x18), "1/0 = +inf (DZ), 0/0 = the canonical NaN (NV)");
    assert_eq!((f[3], f[4], f[5]), (0x8000_0000, 0x8000_0000, 0), "fneg, fmin(+0,-0) = -0, fmax(-0,+0) = +0");
    assert_eq!((x[15], x[16], x[17]), (0x7fff_ffff, 0xffff_ffff, 1), "fcvt.w.s(+inf), fcvt.wu.s(NaN), fcvt.w.s(1/3, rup)");
    assert_eq!((x[18], x[19], x[20], x[21]), (0, 0, 0, 0x10), "feq of a quiet NaN raises nothing, flt raises NV");
    assert_eq!(x[22], 1 << 3, "fclass(-0)");
    assert_eq!((f[8], f[9], f[18], f[19]), (0x40e0_0000, 0x40a0_0000, 0xc0a0_0000, 0xc0e0_0000), "fmadd 7, fmsub 5, fnmsub -5, fnmadd -7");
    assert_eq!((f[20], f[21], f[22]), (0, 0, 0x8000_0000), "+0 sums: fmadd +0, fnmsub +0, fnmadd -0");
    assert_eq!((f[23], f[24], x[23]), (0x3fdd_b3d7, 0xbf80_0000, 0), "sqrt 3, fsgnjx(1,-0) = -1, fcvt.wu.s(-1) clips to 0");
    assert_eq!(x[24], 0x11, "fcsr: frm RNE, NV from flt and fcvt.wu.s, NX from sqrt");
    assert_eq!(x[25], 0x8000_7800, "mstatus: SD, FS = Dirty, MPP = M");
    // the same program through Core::run leaves the same state
    let (mut b, mut rb) = machine_on(Cpu::new_esp32p4(), &asm(FARITH), "lui t0,0x2");
    let mut left = n as u32;
    while left > 0 { let (used, t) = b.run(&mut rb, left); assert_eq!(t, None); left -= used; }
    assert_eq!((b.x, b.f, b.fcsr, b.mstatus, b.pc), (cpu.x, cpu.f, cpu.fcsr, cpu.mstatus, cpu.pc));
}

/// On the P4 rev >= 3 core EXT_ILL flags every FP instruction taken with FS = Off: FLW, FSW, the FP
/// CSRs, C.FLW, C.FSW, C.FLWSP and C.FSWSP as well as the computational ones (FMADD among them),
/// which is all IDF's rev >= 3 `vectors.S` looks at ("The FPU_EXT_ILL bug was fixed on revision
/// 3.0 and higher", ESP-IDF v5.5.4 components/riscv/vectors.S:279-280). An IDF v5.5.4 sdmmc
/// example built for rev >= 3 panics in `sdmmc_card_init` at its first C.FLW without it.
#[test]
fn rev3_ext_ill_flags_fp_loads_stores_and_csrs() {
    let prog = asm("00c5f553 e00505d3 00052507 003025f3 7f001673 e15c 6022 00a52027 6108 e02a 68c58543");
    for (core, ld_st) in [(Cpu::new_esp32p4_rev3(), 1), (Cpu::new_esp32p4(), 0)] {
        let (mut cpu, mut ram) = machine_on(core, &prog, "fadd.s fa0,fa1,fa2");
        cpu.mtvec = BASE + 0x400;
        cpu.x[10] = BASE + 0x800; cpu.x[2] = BASE + 0x900;
        for (off, raw, ext) in [(0, 0x00c5_f553u32, 1), (4, 0xe005_05d3, 1), (8, 0x0005_2507, ld_st), (12, 0x0030_25f3, ld_st), (20, 0xe15c, ld_st), (22, 0x6022, ld_st),
            (24, 0x00a5_2027, ld_st), (28, 0x6108, ld_st), (30, 0xe02a, ld_st), (32, 0x68c5_8543, 1)] {
            cpu.ext_ill = 0;
            cpu.pc = BASE + off;
            assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(2)), "{raw:08x}");
            assert_eq!((cpu.mtval, cpu.ext_ill), (raw, ext), "EXT_ILL after {raw:08x}, rev >= 3: {}", cpu.ext_ill_all_fp);
        }
        cpu.reset();
        assert_eq!(cpu.ext_ill_all_fp, ld_st == 1, "a reset keeps the revision's EXT_ILL");
    }
}

/// The lazy FPU contract ESP-IDF v5.5.3 relies on (`components/riscv/vectors.S`, `_panic_handler`):
/// with FS = Off every F instruction and FP CSR access is an illegal instruction with mtval = its
/// bits; EXT_ILL (0x7F0) bit 0 is set for the computational instructions only, and reads and
/// clears with `csrrw a2,0x7f0,zero`.
#[test]
fn fs_off_traps_with_the_instruction_and_its_ext_ill_reason() {
    let prog = asm("00c5f553 e00505d3 00052507 003025f3 7f001673 e15c 6022");
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &prog, "fadd.s fa0,fa1,fa2");
    assert_eq!(cpu.mstatus & mstatus::FS, 0, "FS is Off at reset");
    cpu.mtvec = BASE + 0x400;
    cpu.x[10] = BASE + 0x800; cpu.x[2] = BASE + 0x900;
    // (offset, raw, EXT_ILL after the trap)
    let traps = [(0, 0x00c5_f553, 1), (8, 0x0005_2507, 0), (12, 0x0030_25f3, 0), (20, 0xe15c, 0), (22, 0x6022, 0), (4, 0xe005_05d3, 1)];
    for (k, &(off, raw, ext)) in traps.iter().enumerate() {
        if k == 1 {
            cpu.pc = BASE + 16;
            step(&mut cpu, &mut ram).unwrap();                     // csrrw a2,0x7f0,zero needs no FP state
            assert_eq!((cpu.x[12], cpu.ext_ill), (1, 0), "EXT_ILL read and cleared");
        }
        cpu.pc = BASE + off;
        assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(2)), "{raw:08x}");
        assert_eq!((cpu.mcause, cpu.mtval, cpu.mepc, cpu.pc), (2, raw, BASE + off, BASE + 0x400), "{raw:08x}");
        assert_eq!(cpu.ext_ill, ext, "EXT_ILL after {raw:08x}");
    }
    assert_eq!((cpu.f, cpu.fcsr, cpu.mstatus & (mstatus::FS | mstatus::SD)), ([0; 32], 0, 0), "a trapped F instruction changes no FP state");
    // with the unit on, the same instructions run
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::FS_INITIAL);
    for off in [0, 4, 8, 12] { cpu.pc = BASE + off; step(&mut cpu, &mut ram).unwrap(); }
}

/// `rv_utils_enable_fpu` (ESP-IDF v5.5.3 `riscv/include/riscv/rv_utils.h`) sets FS = Initial,
/// writes fcsr and clears bit 13, then requires FS = Clean; FreeRTOS reads bit 13 as the dirty bit.
/// A store reads FP state and leaves it Clean, a load makes it Dirty, clearing both bits turns it Off.
#[test]
fn fs_follows_the_idf_enable_sequence() {
    let prog = asm("000027b7 3007a073 00100713 00371073 3007b073 30002573 00a12027 300025f3 00012587 30002673 000066b7 3006b073 300026f3 0000006f");
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &prog, "lui a5,0x2");
    cpu.x[2] = BASE + 0x800;
    for _ in 0..13 { step(&mut cpu, &mut ram).unwrap(); }
    assert_eq!(cpu.x[10], 0x5800, "FS = Clean after the enable sequence, SD clear");
    assert_eq!(cpu.fcsr, 1, "the fcsr write landed");
    assert_eq!(cpu.x[11], 0x5800, "fsw leaves FS Clean");
    assert_eq!(cpu.x[12], 0x8000_7800, "flw makes it Dirty, with SD");
    assert_eq!(cpu.x[13], 0x1800, "csrc mstatus,0x6000 turns it Off and clears SD");
    cpu.write_csr(csr::MSTATUS, 0x8000_1800);
    assert_eq!(cpu.read_csr(csr::MSTATUS), 0x1800, "SD is read-only");
}

/// C.FLW, C.FSW, C.FLWSP and C.FSWSP move bits unchanged, a signaling NaN included, and f0 is a
/// valid C.FLWSP destination.
#[test]
fn compressed_fp_loads_and_stores_move_bits() {
    let prog = asm("615c fdfc 7ffe e07e 6012 a001");
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &prog, "flw fa5,4(a0)");
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::FS_INITIAL);
    cpu.x[10] = BASE + 0x800; cpu.x[11] = BASE + 0x900; cpu.x[2] = BASE + 0xa00;
    let put = |ram: &mut FlatRam, off: usize, v: u32| ram.mem[off..off + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut ram, 0x804, 0x7f80_0001); put(&mut ram, 0xafc, 0xdead_beef); put(&mut ram, 0xa04, 0x3f80_0000);
    for _ in 0..5 { step(&mut cpu, &mut ram).unwrap(); }
    let get = |ram: &FlatRam, off: usize| u32::from_le_bytes(ram.mem[off..off + 4].try_into().unwrap());
    assert_eq!((cpu.f[15], get(&ram, 0x900 + 124)), (0x7f80_0001, 0x7f80_0001), "c.flw/c.fsw keep a signaling NaN's bits");
    assert_eq!((cpu.f[31], get(&ram, 0xa00)), (0xdead_beef, 0xdead_beef), "c.flwsp 252(sp), c.fswsp 0(sp)");
    assert_eq!(cpu.f[0], 0x3f80_0000, "c.flwsp into f0");
    assert_eq!(cpu.pc, BASE + 10);
    assert_eq!(cpu.fcsr, 0, "moves raise no flags");
}

/// Reserved rounding modes trap as illegal instructions, static (5, 6) or through frm (5, 6, 7),
/// and are not an FPU-off reason: EXT_ILL stays clear, so ESP-IDF panics instead of re-enabling the
/// FPU and retrying forever.
#[test]
fn reserved_rounding_modes_are_illegal() {
    let prog = asm("00c5d553 5805e553 0022d073 00c5f553 0023d073 d0057553 00225073 00c5f553");
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &prog, "fadd.s fa0,fa1,fa2,unknown");
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::FS_DIRTY);
    cpu.mtvec = BASE + 0x400;
    for (off, raw) in [(0, 0x00c5_d553), (4, 0x5805_e553), (12, 0x00c5_f553), (20, 0xd005_7553)] {
        if off >= 12 { cpu.pc = BASE + off - 4; step(&mut cpu, &mut ram).unwrap(); }   // fsrmi 5, then 7
        cpu.pc = BASE + off;
        assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(2)), "{raw:08x}");
        assert_eq!((cpu.mtval, cpu.mepc, cpu.ext_ill), (raw, BASE + off, 0), "{raw:08x}");
    }
    cpu.pc = BASE + 24;
    step(&mut cpu, &mut ram).unwrap();                             // fsrmi 4: RMM is valid
    step(&mut cpu, &mut ram).unwrap();
    assert_eq!(cpu.pc, BASE + 32);
}

/// The C3 and C6 cores have no F: its encodings, compressed ones included, stay illegal with mtval
/// = the instruction, and mstatus, fcsr's number and 0x7F0 keep behaving as plain CSRs, as before
/// F was added.
#[test]
fn cores_without_f_treat_f_as_illegal() {
    for core in [Cpu::new(), Cpu::new_rv32imac()] {
        let prog = asm("00c5f553 615c 00052507 003025f3 00371073 003025f3 7f001673");
        let (mut cpu, mut ram) = machine_on(core, &prog, "unimp");
        cpu.mtvec = BASE + 0x400;
        cpu.x[10] = BASE + 0x800;
        for (off, raw) in [(0, 0x00c5_f553), (4, 0x615c), (6, 0x0005_2507)] {
            cpu.pc = BASE + off;
            assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(2)), "{raw:08x}");
            assert_eq!((cpu.mcause, cpu.mtval, cpu.mepc), (2, raw, BASE + off), "{raw:08x}");
        }
        cpu.pc = BASE + 10;
        cpu.x[14] = 0x1234;
        for _ in 0..4 { step(&mut cpu, &mut ram).unwrap(); }
        assert_eq!((cpu.x[11], cpu.x[12]), (0x1234, 0), "fcsr's number reads back what was written, 0x7F0 is unmodelled");
        assert_eq!((cpu.f, cpu.fcsr, cpu.ext_ill), ([0; 32], 0, 0));
        cpu.write_csr(csr::MSTATUS, 0x8000_7800);
        assert_eq!(cpu.read_csr(csr::MSTATUS), 0x8000_7800, "mstatus keeps whatever was written");
        let d = |w: u32| cpu.disasm(BASE, w.to_le_bytes());
        assert_eq!((d(0x00c5_f553), d(0x615c), d(0x0030_25f3)), ("unimp".into(), "unimp".into(), "csrr a1,fcsr".into()));
    }
    let p4 = Cpu::new_esp32p4();
    let d = |w: u32| p4.disasm(BASE, w.to_le_bytes());
    assert_eq!((d(0x00c5_f553), d(0x615c), d(0x0030_25f3)), ("fadd.s fa0,fa1,fa2".into(), "flw fa5,4(a0)".into(), "frcsr a1".into()));
}

/// A chip reset keeps the core's ISA and EXT_ILL, and resets the FP state with FS = Off.
#[test]
fn reset_keeps_the_p4_configuration() {
    let mut cpu = Cpu::new_esp32p4();
    cpu.write_csr(csr::MSTATUS, mstatus::FS_DIRTY);
    cpu.f[3] = 5; cpu.fcsr = 0x1f; cpu.ext_ill = 1;
    cpu.reset();
    assert_eq!((cpu.read_csr(csr::MISA), cpu.misa, cpu.has_ext_ill), (0x4000_1125, MISA_RV32IMAFC, true));
    assert_eq!((cpu.mstatus, cpu.f[3], cpu.fcsr, cpu.ext_ill), (0x1800, 0, 0, 0));
    let mut c3 = Cpu::new();
    c3.reset();
    assert_eq!((c3.misa, c3.has_ext_ill), (0x4000_1104, false));
}

/// The P4 core's `cycle`/`instret` (and their high halves) read the same counters as
/// mcycle/minstret, including a guest write to mcycle; on the C3/C6 they stay unmodelled CSRs.
#[test]
fn p4_user_counters_shadow_the_machine_counters() {
    // csrr a0,cycle; csrr a1,instret; csrr a2,cycleh; csrr a3,instreth; csrr a4,mcycle
    let prog = asm("c0002573 c02025f3 c8002673 c82026f3 b0002773");
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &prog, "rdcycle a0");
    cpu.advance_cycles(100);
    cpu.write_csr(csr::MCYCLEH, 7);
    for _ in 0..5 { step(&mut cpu, &mut ram).unwrap(); }
    // an instruction counts its own cycle before it reads the counter
    assert_eq!((cpu.x[10], cpu.x[11], cpu.x[12], cpu.x[13]), (101, 1, 7, 0));
    assert_eq!(cpu.x[14], 105);
    cpu.write_csr(csr::CYCLE, 0);                               // read-only: ignored
    assert_eq!(cpu.read_csr(csr::CYCLE), 105);
    cpu.reset();
    assert!(cpu.user_counters);
    let mut c6 = Cpu::new_rv32imac();
    c6.advance_cycles(100);
    assert_eq!(c6.read_csr(csr::CYCLE), 0);
    c6.write_csr(csr::CYCLE, 5);
    assert_eq!(c6.read_csr(csr::CYCLE), 5, "an unmodelled CSR reads back what was written");
}

/// CLIC mode on the P4 core (CLIC specification, "CLIC Interrupt Operation"; ESP-IDF v5.5.3
/// riscv/vectors_clic.S): a hardware-vectored interrupt jumps through `mtvt + 4*id`, mcause
/// carries the ID and the previous level, mintstatus (0x346) the new one; a candidate at or
/// below the current level waits, a higher one nests, and `mret` restores the level from MPIL.
#[test]
fn clic_vectors_nests_by_level_and_mret_restores_it() {
    use riscv_rv32::exec::{CLIC_IRQ_LEVEL_SHIFT, CLIC_IRQ_SHV};
    let nops = asm("0001 0001 0001 0001 0001 0001 0001 0001");   // c.nop
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &nops, "nop");
    let (table, handler, base) = (BASE + 0x400, BASE + 0x800, BASE + 0x200);
    for id in 16..48u32 { ram.mem[(table - BASE + 4 * id) as usize..][..4].copy_from_slice(&(handler + 4 * id).to_le_bytes()); }
    for w in (0x800..0x900).step_by(2) { ram.mem[w..w + 2].copy_from_slice(&0x0001u16.to_le_bytes()); }
    ram.mem[0x900..0x904].copy_from_slice(&0x3020_0073u32.to_le_bytes());   // mret
    cpu.write_csr(csr::MTVEC, base | 3);
    cpu.write_csr(csr::MTVT, table | 0x3f);
    assert_eq!(cpu.read_csr(csr::MTVT), table, "mtvt is 64-byte aligned");
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
    let irq = |line: u32, level: u32, shv: bool| Some(line | level << CLIC_IRQ_LEVEL_SHIFT | if shv { CLIC_IRQ_SHV } else { 0 });
    cpu.set_irq(irq(2, 0x3f, true));                                  // CLIC ID 18, level 1
    let at = cpu.pc;
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Interrupt(2)));
    assert_eq!((cpu.pc, cpu.mepc), (handler + 4 * 18, at));
    assert_eq!(cpu.read_csr(csr::MCAUSE), 0x8000_0000 | 3 << 28 | 1 << 27 | 18, "MPP = M, MPIE = 1, MPIL = 0, ID 18");
    assert_eq!(cpu.read_csr(csr::MINTSTATUS_P4), 0x3f << 24);
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);          // the handler enables nesting
    cpu.set_irq(irq(1, 0x3f, true));                                  // the same level waits
    assert_eq!(step(&mut cpu, &mut ram), Ok(()));
    cpu.set_irq(irq(12, 0x7f, false));                                // level 3, not vectored: the base
    let inner = cpu.pc;
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Interrupt(12)));
    assert_eq!((cpu.pc, cpu.mepc, cpu.read_csr(csr::MINTSTATUS_P4) >> 24), (base, inner, 0x7f));
    assert_eq!(cpu.read_csr(csr::MCAUSE) & 0x80ff_0fff, 0x8000_0000 | 0x3f << 16 | 28, "MPIL is the interrupted level");
    cpu.set_irq(None);
    cpu.pc = BASE + 0x900;                                            // mret from the nested one
    step(&mut cpu, &mut ram).unwrap();
    assert_eq!((cpu.pc, cpu.mil), (inner, 0x3f));
    assert!(cpu.mie_enabled());
    // an exception keeps the level, enters at the base and records the level in MPIL
    cpu.write_csr(csr::MSTATUS, cpu.mstatus & !mstatus::MIE);
    ram.mem[(cpu.pc - BASE) as usize..][..4].copy_from_slice(&0x0000_0073u32.to_le_bytes());   // ecall
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(11)));
    assert_eq!((cpu.pc, cpu.mil, cpu.read_csr(csr::MCAUSE) & 0x80ff_0fff), (base, 0x3f, 0x3f << 16 | 11));
    // writing mcause sets MPIL and, through the alias, mstatus.MPIE
    cpu.write_csr(csr::MCAUSE, 0x8000_0012);
    assert_eq!(cpu.mstatus & mstatus::MPIE, 0);
    cpu.pc = BASE + 0x900;
    step(&mut cpu, &mut ram).unwrap();
    assert_eq!((cpu.mil, cpu.mie_enabled()), (0, false));
}

/// The ROM's `mtvec` mode 1 keeps taking exceptions at its base on the P4 core, and the C6 core
/// has no CLIC: 0x307 and 0x346 are plain CSRs there and a presented line is taken as before.
#[test]
fn clic_mode_is_disabled_on_the_c6_core() {
    let (mut p4, mut ram) = machine_on(Cpu::new_esp32p4(), &asm("00000073"), "ecall");
    p4.write_csr(csr::MTVEC, BASE + 0x101);
    assert_eq!(step(&mut p4, &mut ram), Err(Trap::Exception(11)));
    assert_eq!((p4.pc, p4.mcause, p4.read_csr(csr::MCAUSE)), (BASE + 0x100, 11, 0xb), "mcause reads the bare cause outside CLIC mode");
    let mut c6 = Cpu::new_rv32imac();
    c6.write_csr(csr::MTVT, 0x1234);
    c6.write_csr(csr::MINTSTATUS_P4, 0x55);
    assert_eq!((c6.read_csr(csr::MTVT), c6.read_csr(csr::MINTSTATUS_P4)), (0x1234, 0x55));
    assert!(!c6.clic);
}

/// S31 v6.1 selects standard CLIC CSRs, while the existing P4 rev < 3 keeps 0x346.
#[test]
fn s31_standard_clic_status_and_threshold_are_separate_from_old_p4() {
    let mut s31 = Cpu::new_esp32s31();
    s31.mil = 0x7f;
    assert_eq!(s31.read_csr(csr::MINTSTATUS), 0x7f00_0000);
    assert_eq!(s31.read_csr(csr::MINTSTATUS_P4), 0);
    s31.write_csr(csr::MINTSTATUS, 0); assert_eq!(s31.read_csr(csr::MINTSTATUS), 0x7f00_0000);
    s31.write_csr(csr::MINTTHRESH, 0xffff_ff3f); assert_eq!(s31.read_csr(csr::MINTTHRESH), 0x3f);
    s31.reset(); assert!(s31.clic && s31.clic_std);
    assert_eq!((s31.mil, s31.read_csr(csr::MINTTHRESH)), (0, 0));
    let mut p4 = Cpu::new_esp32p4(); p4.mil = 0x3f;
    assert_eq!(p4.read_csr(csr::MINTSTATUS_P4), 0x3f00_0000);
    assert_eq!(p4.read_csr(csr::MINTSTATUS), 0); assert!(!p4.clic_std);
}

/// mhartid reads the core's index, read-only, and a reset keeps it with the rest of the P4
/// configuration.
#[test]
fn mhartid_survives_reset() {
    let mut c = Cpu::new_esp32p4();
    c.hartid = 1;
    c.write_csr(csr::MHARTID, 5);
    c.reset();
    assert_eq!((c.read_csr(csr::MHARTID), c.clic, c.user_counters), (1, true, true));
    assert_eq!(Cpu::new_rv32imac().read_csr(csr::MHARTID), 0);
}

/// The P4 rev >= 3 core's CLIC is the specification's (ESP-IDF v5.5.4
/// riscv/include/esp_private/interrupt_clic.h:60-66): an interrupt is taken only above both
/// mintthresh (0x347) and the current level, mintstatus reads at 0xFB1, and 0x346 is a plain CSR
/// there. On the rev < 3 core the threshold is the SoC's: a write to 0x347 masks nothing.
#[test]
fn rev3_clic_takes_only_above_mintthresh() {
    let nops = asm("0001 0001 0001 0001");
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4_rev3(), &nops, "nop");
    cpu.write_csr(csr::MTVEC, BASE + 0x203);
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
    cpu.write_csr(csr::MINTTHRESH, 0x13f);
    assert_eq!(cpu.read_csr(csr::MINTTHRESH), 0x3f, "the threshold is 8 bits");
    cpu.set_irq(clic_irq(2, 0x3f, false));                          // level 1 at threshold 1 waits
    assert_eq!(step(&mut cpu, &mut ram), Ok(()));
    cpu.set_irq(clic_irq(2, 0x7f, false));                          // level 3 is taken
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Interrupt(2)));
    assert_eq!(cpu.read_csr(csr::MINTSTATUS), 0x7f << 24);
    cpu.write_csr(csr::MINTSTATUS_P4, 0x55);
    assert_eq!(cpu.read_csr(csr::MINTSTATUS_P4), 0x55, "0x346 is not mintstatus on rev >= 3");
    cpu.reset();
    assert!(cpu.clic_std);
    assert_eq!(cpu.read_csr(csr::MINTTHRESH), 0, "a reset clears the threshold");
    let (mut old, mut ram) = machine_on(Cpu::new_esp32p4(), &nops, "nop");
    old.write_csr(csr::MTVEC, BASE + 0x203);
    old.write_csr(csr::MSTATUS, old.mstatus | mstatus::MIE);
    old.write_csr(csr::MINTTHRESH, 0xff);
    old.set_irq(clic_irq(2, 0x3f, false));
    assert_eq!(step(&mut old, &mut ram), Err(Trap::Interrupt(2)));
}

fn clic_irq(line: u32, level: u32, shv: bool) -> Option<u32> {
    use riscv_rv32::exec::{CLIC_IRQ_LEVEL_SHIFT, CLIC_IRQ_SHV};
    Some(line | level << CLIC_IRQ_LEVEL_SHIFT | if shv { CLIC_IRQ_SHV } else { 0 })
}

/// In mtvec modes 0 and 1 the P4 core takes no CLIC candidate, even with MIE set, does not wake
/// from WFI for it and does not count it as pending; mcause reads as written there.
#[test]
fn clint_mode_takes_no_clic_candidate() {
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &asm("0001 0001 10500073 0001"), "nop");
    cpu.write_csr(csr::MTVEC, BASE + 0x101);
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
    cpu.set_irq(clic_irq(2, 0x3f, false));
    assert!(!cpu.irq_pending());
    for _ in 0..3 { assert_eq!(step(&mut cpu, &mut ram), Ok(())); }   // nop, nop, wfi
    assert!(cpu.waiting);
    assert_eq!(Core::step(&mut cpu, &mut ram).kind, emu_core::StepKind::Idle, "the WFI goes on");
    cpu.write_csr(csr::MCAUSE, 0x8000_0000 | 3 << 28 | 0x12);
    assert_eq!(cpu.read_csr(csr::MCAUSE), 0x8000_0000 | 3 << 28 | 0x12, "no MPP/MPIE alias outside CLIC mode");
    cpu.write_csr(csr::MTVEC, BASE + 0x103);
    assert!(cpu.irq_pending());
}

/// MIE alone blocks a take; a WFI ends for any presented candidate, takeable or not.
#[test]
fn clic_mie_blocks_and_wfi_wakes() {
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &asm("10500073 0001 0001 0001"), "wfi");
    cpu.write_csr(csr::MTVEC, BASE + 0x203);
    cpu.set_irq(clic_irq(1, 0x3f, false));
    let mut core = |cpu: &mut Cpu| Core::step(cpu, &mut ram);
    assert_eq!(core(&mut cpu).kind, emu_core::StepKind::Retired);       // wfi
    assert!(cpu.waiting);
    assert_eq!(core(&mut cpu).kind, emu_core::StepKind::Retired, "MIE = 0: woken, nothing taken, the nop runs");
    assert!(!cpu.waiting);
    cpu.mil = 0x3f;
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
    cpu.pc = BASE;
    cpu.set_irq(None);
    assert_eq!(core(&mut cpu).kind, emu_core::StepKind::Retired);
    assert!(cpu.waiting);
    cpu.set_irq(clic_irq(1, 0x3f, false));                            // at the current level
    assert_eq!(core(&mut cpu).kind, emu_core::StepKind::Retired, "woken, not taken: it resumes after the wfi");
    assert_eq!(cpu.pc, BASE + 6);
    cpu.mil = 0;
    assert_eq!(core(&mut cpu).kind, emu_core::StepKind::TrapBefore(Trap::Interrupt(1)));
    assert_eq!(cpu.pc, BASE + 0x200);
}

/// An interrupt between LR and SC breaks the P4 core's reservation, so the SC cannot overwrite
/// what the handler stored there (the RISC-V unprivileged ISA allows an SC to fail at any time).
/// The C6 core keeps its reservation across a trap, as before.
#[test]
fn a_trap_breaks_the_p4_cores_reservation() {
    // lr.w a0,(a1); sc.w a2,a3,(a1)
    let prog = asm("1005a52f 18d5a62f");
    let handler = asm("c194 30200073");                               // sw a3,0(a1); mret
    for (cpu, sc_result) in [(Cpu::new_esp32p4(), 1), (Cpu::new_rv32imac(), 0)] {
        let (mut cpu, mut ram) = machine_on(cpu, &prog, "lr.w a0,(a1)");
        ram.mem[0x200..0x200 + handler.len()].copy_from_slice(&handler);
        cpu.x[11] = BASE + 0x100; cpu.x[13] = 0x55;
        cpu.write_csr(csr::MTVEC, BASE + 0x200 + if cpu.clic { 3 } else { 0 });
        step(&mut cpu, &mut ram).unwrap();
        cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
        cpu.set_irq(if cpu.clic { clic_irq(4, 0x3f, false) } else { Some(4) });
        assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Interrupt(4)));
        cpu.set_irq(None);
        cpu.x[13] = 0x66;
        step(&mut cpu, &mut ram).unwrap();                            // the handler's store
        step(&mut cpu, &mut ram).unwrap();                            // mret
        cpu.x[13] = 0x77;
        step(&mut cpu, &mut ram).unwrap();                            // sc.w
        assert_eq!(cpu.x[12], sc_result, "SC result on the {} core", if cpu.clic { "P4" } else { "C6" });
        assert_eq!(ram.mem[0x100], if sc_result == 1 { 0x66 } else { 0x77 });
    }
}

/// An exception between LR and SC breaks it too: `Cpu::trap` clears it, not only the CLIC take.
#[test]
fn an_exception_breaks_the_p4_cores_reservation() {
    // lr.w a0,(a1); ecall; sc.w a2,a3,(a1)
    let prog = asm("1005a52f 00000073 18d5a62f");
    let handler = asm("c194 341022f3 0291 34129073 30200073");      // sw a3,0(a1); mepc += 4; mret
    for (cpu, sc_result) in [(Cpu::new_esp32p4(), 1), (Cpu::new_rv32imac(), 0)] {
        let (mut cpu, mut ram) = machine_on(cpu, &prog, "lr.w a0,(a1)");
        ram.mem[0x200..0x200 + handler.len()].copy_from_slice(&handler);
        cpu.x[11] = BASE + 0x100; cpu.x[13] = 0x66;
        cpu.write_csr(csr::MTVEC, BASE + 0x200 + if cpu.clic { 3 } else { 0 });
        step(&mut cpu, &mut ram).unwrap();
        assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(11)));
        for _ in 0..5 { step(&mut cpu, &mut ram).unwrap(); }          // the handler, through mret
        assert_eq!(cpu.pc, BASE + 8);
        cpu.x[13] = 0x77;
        step(&mut cpu, &mut ram).unwrap();                            // sc.w
        assert_eq!(cpu.x[12], sc_result, "SC result on the {} core", if cpu.clic { "P4" } else { "C6" });
        assert_eq!(ram.mem[0x100], if sc_result == 1 { 0x66 } else { 0x77 });
    }
}

/// A fault on the hardware-vectoring table read is an instruction access fault at the table
/// entry with MINHV set; `mret`'s resumption of the fetch is not modelled.
#[test]
fn clic_vector_table_fault_is_an_access_fault() {
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &asm("0001 0001"), "nop");
    cpu.write_csr(csr::MTVEC, BASE + 0x203);
    cpu.write_csr(csr::MTVT, 0x2000_0000);                            // outside the RAM
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
    cpu.set_irq(clic_irq(5, 0x3f, true));
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(1)));
    let entry = 0x2000_0000 + 4 * 21;
    assert_eq!((cpu.pc, cpu.mepc, cpu.mtval), (BASE + 0x200, entry, entry));
    assert_eq!(cpu.read_csr(csr::MCAUSE) & (1 << 30 | 0xfff), 1 << 30 | 1, "MINHV and the access-fault code");
}
