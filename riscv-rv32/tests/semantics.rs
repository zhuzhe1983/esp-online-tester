//! Hermetic tests of the RV32IMC core on a flat RAM: arithmetic and memory through a loop,
//! `step` against `Core::run`, vectored interrupts through the C3's `mtvec` mode, `ecall`,
//! `ebreak`, `mret`; RV32A on the C6 core and RV32F with its lazy-FPU traps on the P4 core.
//! Programs assembled with `riscv32-esp-elf-as -march=rv32imc_zicsr` (`rv32imac_zicsr`,
//! `rv32imafc_zicsr` for the A and F programs, `rv32imafc_zicsr_zba_zbb_zbs_zcb_zcmp` for the S31
//! bit-manipulation and Zc ones).
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

/// In mtvec modes 0 and 1 the P4 core still takes a CLIC candidate above its level with MIE set (the mask ROM's UART
/// download mode depends on it): at the mtvec base, not vectored, with the CLIC ID as mcause and its level in service
/// until `mret`. mcause reads as written there (no MPP/MPIE alias outside CLIC mode).
#[test]
fn clint_mode_takes_a_clic_candidate_at_the_base() {
    let (mut cpu, mut ram) = machine_on(Cpu::new_esp32p4(), &asm("0001 0001 10500073 0001"), "nop");
    cpu.write_csr(csr::MTVEC, BASE + 0x101);
    cpu.set_irq(clic_irq(2, 0x3f, false));
    assert!(cpu.irq_pending());
    assert_eq!(step(&mut cpu, &mut ram), Ok(()), "MIE clear: not taken");
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
    let pc = cpu.pc;
    let _ = Core::step(&mut cpu, &mut ram);
    assert_eq!((cpu.pc, cpu.mepc), (BASE + 0x100, pc));
    assert_eq!(cpu.read_csr(csr::MCAUSE), 0x8000_0000 | (2 + 16), "the plain CLIC ID: no MPIL bits in CLINT mode");
    assert_eq!(cpu.mil, 0x3f);
    cpu.write_csr(csr::MSTATUS, cpu.mstatus | mstatus::MIE);
    let again = Core::step(&mut cpu, &mut ram);
    assert!(!matches!(again.kind, emu_core::StepKind::TrapBefore(Trap::Interrupt(_))), "the same level does not re-enter");
    // That step ran the (illegal) word at the base: return from that exception first, back at the handler's level.
    cpu.mret();
    assert_eq!(cpu.mil, 0x3f);
    // An exception inside the handler, and its mret, leave the level in service alone; mcause is plain.
    cpu.trap(11, 0, cpu.pc);
    assert_eq!((cpu.read_csr(csr::MCAUSE), cpu.mil), (11, 0x3f));
    cpu.mret();
    assert_eq!(cpu.mil, 0x3f, "the exception's mret restores the handler's level, not 0");
    // Software may overwrite mcause (the ROM does not save it around nesting): the interrupt's mret still restores 0.
    cpu.write_csr(csr::MCAUSE, 0x8000_0000 | 0xff << 16 | 0x12);
    cpu.mret();
    assert_eq!(cpu.mil, 0, "the level comes from hidden state, not mcause");
    cpu.write_csr(csr::MCAUSE, 0x8000_0000 | 3 << 28 | 0x12);
    assert_eq!(cpu.read_csr(csr::MCAUSE), 0x8000_0000 | 3 << 28 | 0x12, "no MPP/MPIE alias outside CLIC mode");
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

/// One instruction (objdump hex) on a fresh S31 core with `regs` set; the trap, if any, and the core.
fn s31_one(hex: &str, regs: &[(usize, u32)]) -> (Cpu, FlatRam, Result<(), Trap>) {
    let mut cpu = Cpu::new_esp32s31();
    let mut ram = FlatRam::new(BASE, 64 * 1024);
    let p = asm(hex); ram.mem[..p.len()].copy_from_slice(&p);
    cpu.pc = BASE; cpu.mtvec = BASE + 0x100;
    for &(r, v) in regs { cpu.x[r] = v; }
    let t = step(&mut cpu, &mut ram);
    (cpu, ram, t)
}

/// `a0 = op(a1, a2)` for each Zba/Zbb/Zbs instruction (bit-manipulation v1.0.0), edge cases included.
#[test]
fn s31_zba_zbb_zbs_compute() {
    const A0: usize = 10;
    let cases: &[(&str, u32, u32, u32)] = &[
        ("20c5a533", 3, 10, 16), ("20c5c533", 3, 10, 22), ("20c5e533", 3, 10, 34),            // sh1add sh2add sh3add
        ("20c5e533", 0x8000_0001, 1, 9),                                                         // the shift drops bit 31
        ("40c5f533", 0xff00_ff0f, 0x0f0f_0f0f, 0xf000_f000), ("40c5e533", 0, 0xffff_0000, 0x0000_ffff), ("40c5c533", 0xf0f0, 0xff00, 0xffff_f00f), // andn orn xnor
        ("60059513", 0, 0, 32), ("60059513", 1, 0, 31), ("60059513", 0x8000_0000, 0, 0),        // clz
        ("60159513", 0, 0, 32), ("60159513", 0x8000_0000, 0, 31), ("60159513", 6, 0, 1),        // ctz
        ("60259513", 0, 0, 0), ("60259513", u32::MAX, 0, 32), ("60259513", 0x8000_0101, 0, 3), // cpop
        ("0ac5e533", (-5i32) as u32, 3, 3), ("0ac5f533", (-5i32) as u32, 3, (-5i32) as u32),    // max maxu
        ("0ac5c533", (-5i32) as u32, 3, (-5i32) as u32), ("0ac5d533", (-5i32) as u32, 3, 3),    // min minu
        ("60459513", 0x1234_5680, 0, 0xffff_ff80), ("60459513", 0x7f, 0, 0x7f),                 // sext.b
        ("60559513", 0x1234_8000, 0, 0xffff_8000), ("60559513", 0xffff_7fff, 0, 0x7fff),        // sext.h
        ("0805c533", 0xffff_8001, 0, 0x8001),                                                    // zext.h
        ("60c59533", 0x8000_0001, 33, 3), ("60c5d533", 0x8000_0001, 1, 0xc000_0000),            // rol ror (shift mod 32)
        ("6005d513", 0x1234_5678, 0, 0x1234_5678), ("6045d513", 0x1234_5678, 0, 0x8123_4567),  // rori by 0 and 4
        ("2875d513", 0x0010_8000, 0, 0x00ff_ff00), ("2875d513", 0, 0, 0),                       // orc.b
        ("6985d513", 0x1234_5678, 0, 0x7856_3412),                                               // rev8
        ("48c59533", u32::MAX, 31, 0x7fff_ffff), ("49f59513", u32::MAX, 0, 0x7fff_ffff),        // bclr bclri
        ("48c5d533", 0x8000_0000, 63, 1), ("49f5d513", 0x8000_0000, 0, 1), ("49f5d513", 0x7fff_ffff, 0, 0), // bext bexti, bit 31
        ("68c59533", 0, 4, 16), ("68059513", 1, 0, 0),                                           // binv binvi
        ("28c59533", 0, 31, 0x8000_0000), ("29f59513", 1, 0, 0x8000_0001),                       // bset bseti
    ];
    for &(hex, a, b, want) in cases {
        let (cpu, _, t) = s31_one(hex, &[(11, a), (12, b)]);
        assert_eq!(t, Ok(()), "{hex}");
        assert_eq!(cpu.x[A0], want, "{hex} a1={a:#x} a2={b:#x}");
        assert_eq!(cpu.pc, BASE + 4);
    }
}

/// Zcb: the byte and half-word loads and stores, the unary forms and c.mul.
#[test]
fn s31_zcb_compute_and_move_memory() {
    let mut cpu = Cpu::new_esp32s31();
    let mut ram = FlatRam::new(BASE, 64 * 1024);
    let p = asm("81e8 85a8 85e8 89c8 8da8"); ram.mem[..p.len()].copy_from_slice(&p);
    ram.mem[0x200..0x204].copy_from_slice(&[0x11, 0x22, 0x80, 0xf0]);
    cpu.pc = BASE; cpu.x[11] = BASE + 0x200;
    step(&mut cpu, &mut ram).unwrap(); assert_eq!(cpu.x[10], 0xf0);         // c.lbu a0,3(a1)
    step(&mut cpu, &mut ram).unwrap(); assert_eq!(cpu.x[10], 0xf080);       // c.lhu a0,2(a1)
    step(&mut cpu, &mut ram).unwrap(); assert_eq!(cpu.x[10], 0xffff_f080);  // c.lh a0,2(a1)
    cpu.x[10] = 0x1234_56ab;
    step(&mut cpu, &mut ram).unwrap(); assert_eq!(ram.mem[0x200..0x204], [0x11, 0xab, 0x80, 0xf0]); // c.sb a0,1(a1)
    step(&mut cpu, &mut ram).unwrap(); assert_eq!(ram.mem[0x200..0x204], [0x11, 0xab, 0xab, 0x56]); // c.sh a0,2(a1)
    assert_eq!(cpu.pc, BASE + 10);
    for (hex, a, want) in [("9d61", 0x1234_5680u32, 0x80u32), ("9d65", 0x1234_5680, 0xffff_ff80), ("9d69", 0xffff_8001, 0x8001),
                           ("9d6d", 0x1234_8000, 0xffff_8000), ("9d75", 0x0f0f_0000, 0xf0f0_ffff)] {
        let (cpu, _, t) = s31_one(hex, &[(10, a)]);
        assert_eq!((t, cpu.x[10], cpu.pc), (Ok(()), want, BASE + 2), "{hex}");
    }
    let (cpu, _, _) = s31_one("9d4d", &[(10, 0x1_0001), (11, 0x1_0003)]);   // c.mul a0,a1
    assert_eq!(cpu.x[10], 0x1_0001u32.wrapping_mul(0x1_0003));
}

/// cm.push then cm.pop for every rlist and stack adjustment (Zc v1.0.0): ra at sp-4, s0.. below
/// it, sp moved by the rounded size plus spimm*16, and every register restored.
#[test]
fn s31_zcmp_push_pop_round_trip_every_rlist() {
    use riscv_rv32::decode::{rlist_reg, rlist_regs};
    for rlist in 4u32..16 {
        for spimm in 0u32..4 {
            let push = 0xb802 | rlist << 4 | spimm << 2;
            let pop = 0xba02 | rlist << 4 | spimm << 2;
            let mut cpu = Cpu::new_esp32s31();
            let mut ram = FlatRam::new(BASE, 64 * 1024);
            ram.mem[..4].copy_from_slice(&[(push & 0xff) as u8, (push >> 8) as u8, (pop & 0xff) as u8, (pop >> 8) as u8]);
            cpu.pc = BASE;
            let sp = BASE + 0x8000;
            cpu.x[2] = sp;
            for r in 1..32 { if r != 2 { cpu.x[r] = 0x1000 * r as u32 + 7; } }
            let saved = cpu.x;
            let n = rlist_regs(rlist);
            let adj = ((n * 4 + 15) & !15) + spimm * 16;
            step(&mut cpu, &mut ram).unwrap();
            assert_eq!(cpu.x[2], sp - adj, "push rlist {rlist} spimm {spimm}");
            for k in 0..n {
                let off = (sp - 4 * (k + 1) - BASE) as usize;
                let v = u32::from_le_bytes(ram.mem[off..off + 4].try_into().unwrap());
                assert_eq!(v, saved[rlist_reg(k) as usize], "slot {k} of rlist {rlist}");
            }
            let below = (sp - 4 * (n + 1) - BASE) as usize;
            assert_eq!(ram.mem[below..below + 4], [0; 4], "nothing stored past the list");
            for k in 0..n { cpu.x[rlist_reg(k) as usize] = 0xdead; }
            step(&mut cpu, &mut ram).unwrap();
            assert_eq!(cpu.x, saved, "pop rlist {rlist} spimm {spimm}");
            assert_eq!(cpu.pc, BASE + 4);
        }
    }
    assert_eq!(rlist_regs(15), 13, "rlist 15 is ra, s0-s11: s10 never without s11");
}

/// cm.popret returns through the restored ra; cm.popretz also zeroes a0. mvsa01/mva01s move a0/a1.
#[test]
fn s31_zcmp_popret_popretz_and_moves() {
    for (hex, a0) in [("be42", 0x55u32), ("bc42", 0)] {
        let sp = BASE + 0x1000;
        let mut cpu = Cpu::new_esp32s31();
        let mut ram = FlatRam::new(BASE, 64 * 1024);
        let p = asm(hex); ram.mem[..2].copy_from_slice(&p);
        ram.mem[0x1000 + 16 - 4..0x1000 + 16].copy_from_slice(&(BASE + 0x301).to_le_bytes());   // ra at new sp - 4
        cpu.pc = BASE; cpu.x[2] = sp; cpu.x[10] = 0x55;
        step(&mut cpu, &mut ram).unwrap();
        assert_eq!((cpu.pc, cpu.x[1], cpu.x[2], cpu.x[10]), (BASE + 0x300, BASE + 0x301, sp + 16, a0), "{hex}");
    }
    let (cpu, _, t) = s31_one("acaa", &[(10, 1), (11, 2)]);                // cm.mvsa01 s1,s2
    assert_eq!((t, cpu.x[9], cpu.x[18]), (Ok(()), 1, 2));
    let (cpu, _, t) = s31_one("ac62", &[(8, 9)]);                          // cm.mva01s s0,s0
    assert_eq!((t, cpu.x[10], cpu.x[11]), (Ok(()), 9, 9));
}

/// A push or pop that faults leaves sp and the registers as they were, so it can be re-executed.
#[test]
fn s31_zcmp_faults_are_precise() {
    // cm.push {ra,s0-s11},-64 with only 8 bytes of RAM below sp: the third store faults
    let (cpu, _, t) = s31_one("b8f2", &[(2, BASE + 8), (1, 0x11)]);
    assert_eq!(t, Err(Trap::Exception(7)));
    assert_eq!((cpu.x[2], cpu.mepc, cpu.mtval, cpu.x[1]), (BASE + 8, BASE, BASE - 4, 0x11));
    // cm.pop {ra,s0-s11},64 reaching past the end of RAM: nothing is written
    let (cpu, _, t) = s31_one("baf2", &[(2, BASE + 0x1_0000 - 16), (1, 0x11), (8, 0x22)]);
    assert_eq!(t, Err(Trap::Exception(5)));
    assert_eq!((cpu.x[2], cpu.x[1], cpu.x[8], cpu.mepc), (BASE + 0x1_0000 - 16, 0x11, 0x22, BASE));
}

/// Reserved encodings are illegal on the S31; with the extensions off (C3/C6/P4) every new
/// encoding, the C.FLD/C.FSD/C.FLDSP/C.FSDSP slots included, stays illegal.
#[test]
fn s31_reserved_and_flag_off_encodings_are_illegal() {
    // rlist 0-3 for push/pop/popretz/popret, cm.mvsa01 s0,s0, c.lh/c.sh holes, c.zext.w (RV64)
    for hex in ["b802", "ba12", "bc22", "be32", "ac22", "8c40", "8c60", "9c71"] {
        let (cpu, _, t) = s31_one(hex, &[]);
        assert_eq!(t, Err(Trap::Exception(2)), "{hex}");
        assert_eq!(cpu.mtval, u32::from_str_radix(hex, 16).unwrap(), "{hex}");
    }
    let p4 = Cpu::new_esp32p4();
    for hex in ["b876", "ba76", "be42", "bc42", "acaa", "ac62", "81e8", "85a8", "85e8", "89c8", "8da8", "9d61", "9d65", "9d69", "9d6d", "9d75", "9d4d",
                "20c5a533", "40c5f533", "60059513", "0ac5e533", "0805c533", "60c59533", "6005d513", "2875d513", "6985d513", "48c5d533", "29f59513",
                "2000", "a000", "3002", "b002"] {                         // c.fld, c.fsd, c.fldsp, c.fsdsp
        for misa in [p4.misa, Cpu::new().misa, Cpu::new_rv32imac().misa] {
            assert!(decode_for(misa, BASE, asm(hex).try_into().unwrap_or_else(|v: Vec<u8>| [v[0], v[1], 0, 0])).is_illegal(), "{hex} misa {misa:#x}");
        }
    }
    // the C.FLD and C.FLDSP slots stay illegal on the S31 too (no D, and Zcmp uses only quadrant 2 funct3 101)
    for hex in ["2000", "a000", "3002"] { assert_eq!(s31_one(hex, &[]).2, Err(Trap::Exception(2)), "{hex}"); }
    assert!(decode(BASE, [0x6e, 0xb8, 0, 0]).is_illegal());
}

// ---- Xespv2p2 (PIE v2.2) on the S31 core. Encodings from riscv32-esp-elf-as-xespv2p2
// (esp-14.2.0_20260121); expected values from the ESP32-P4 TRM chapter 4 operation listings.

const DATA: u32 = BASE + 0x1000;

fn q16(l: [u16; 8]) -> u128 { l.iter().enumerate().fold(0, |v, (k, &x)| v | (x as u128) << (16 * k)) }
fn l16(v: u128) -> [u16; 8] { std::array::from_fn(|k| (v >> (16 * k)) as u16) }
fn qb(f: impl Fn(usize) -> u8) -> u128 { u128::from_le_bytes(std::array::from_fn(f)) }

/// One PIE instruction (objdump hex) on an S31 core with PIE enabled (mext_pie_status = Initial),
/// after `setup`; 64 bytes 0, 1, 2, ... sit at DATA.
fn pie_one(hex: &str, setup: impl FnOnce(&mut Cpu)) -> (Cpu, FlatRam, Result<(), Trap>) {
    let mut cpu = Cpu::new_esp32s31();
    cpu.write_csr(csr::MEXT_PIE_STATUS, 1);
    let mut ram = FlatRam::new(BASE, 64 * 1024);
    let p = asm(hex); ram.mem[..p.len()].copy_from_slice(&p);
    for k in 0..64 { ram.mem[0x1000 + k] = k as u8; }
    cpu.pc = BASE; cpu.mtvec = BASE + 0x100;
    setup(&mut cpu);
    let t = step(&mut cpu, &mut ram);
    (cpu, ram, t)
}

const MX: [u16; 8] = [0xffff, 300, 8, 40, 1, 0, 0x1234, 3];
const MY: [u16; 8] = [0xffff, 7, 3, 1, 1, 9, 0x10, 5];
fn mul_setup(cpu: &mut Cpu) { cpu.q[0] = q16(MX); cpu.q[1] = q16(MY); cpu.pie_sar = 4; }

/// esp.vmul.u16 (TRM 4.7.2.155, 4.4.5): products 0xfffe0001 2100 24 40 1 0 0x12340 15 shifted
/// right by SAR = 4, rounded by rm, saturated with sat, truncated without.
#[test]
fn pie_vmul_u16_shifts_rounds_and_saturates() {
    let cases = [
        ("07c5991f", [0xffff, 131, 1, 2, 0, 0, 0x1234, 0]),   // sat,dyn with vxrm = 0: FLOOR
        ("07c5911f", [0xffff, 131, 2, 2, 0, 0, 0x1234, 1]),   // sat,rne: 1.5 -> 2, 2.5 -> 2, 0.9375 -> 1
        ("07c4011f", [0xe000, 131, 1, 2, 0, 0, 0x1234, 0]),   // trunc,rdn: the low 16 bits of 0x0fffe000
        ("07c5091f", [0xffff, 132, 2, 3, 1, 0, 0x1234, 1]),   // sat,rup: CEILING
    ];
    for (hex, want) in cases {
        let (cpu, _, t) = pie_one(hex, mul_setup);
        assert_eq!(t, Ok(()), "{hex}");
        assert_eq!(l16(cpu.q[2]), want, "{hex}");
        assert_eq!(cpu.pc, BASE + 4);
    }
    // dyn with CFG.vxrm = HALF_EVEN (6) rounds as rne; vxrm = 7 is UNNECESSARY: FLOOR and rm_exc
    let (cpu, _, _) = pie_one("07c5991f", |c| { mul_setup(c); c.pie_cfg = 6 << 4; });
    assert_eq!(l16(cpu.q[2]), [0xffff, 131, 2, 2, 0, 0, 0x1234, 1]);
    let (cpu, _, _) = pie_one("07c5991f", |c| { mul_setup(c); c.pie_cfg = 7 << 4; });
    assert_eq!((l16(cpu.q[2])[2], cpu.pie_cfg & 8), (1, 8));
    // a saturating lane sets vxsat only with vxsat_en, and reading CFG clears it
    let (mut cpu, _, _) = pie_one("07c5991f", mul_setup);
    assert_eq!(cpu.pie_cfg & 4, 0);
    let (cpu2, _, _) = pie_one("07c5991f", |c| { mul_setup(c); c.pie_cfg = 1 << 8; });
    assert_eq!(cpu2.pie_cfg & 4, 4);
    cpu.pie_cfg = cpu2.pie_cfg;
    assert_eq!(cpu.read_pie_cfg() & 4, 4);
    assert_eq!(cpu.read_pie_cfg() & 4, 0);
}

/// esp.vmul.u16.ld.incp (TRM 4.7.2.156): the product, and qu loaded from rs1 force-aligned, rs1 += 16.
#[test]
fn pie_vmul_u16_ld_incp_loads_and_increments() {
    let (cpu, _, t) = pie_one("0579b15f", |c| { mul_setup(c); c.x[11] = DATA + 4; });
    assert_eq!(t, Ok(()));
    assert_eq!(l16(cpu.q[2]), [0xffff, 131, 1, 2, 0, 0, 0x1234, 0]);
    assert_eq!((cpu.q[4], cpu.x[11]), (qb(|k| k as u8), DATA + 20));
    // trunc,rhaz: 2.5 rounds away from zero, lane 0 keeps its low bits
    let (cpu, _, _) = pie_one("0521b15f", |c| { mul_setup(c); c.x[11] = DATA; });
    assert_eq!(l16(cpu.q[2]), [0xe000, 131, 2, 3, 0, 0, 0x1234, 1]);
    // a faulting load changes nothing
    let (cpu, _, t) = pie_one("0579b15f", |c| { mul_setup(c); c.x[11] = 0x1000; c.q[2] = 7; });
    assert_eq!(t, Err(Trap::Exception(5)));
    assert_eq!((cpu.q[2], cpu.x[11], cpu.mtval), (7, 0x1000, 0x1000));
}

/// esp.vadd.u16 / esp.vsub.u16 / esp.vsub.u8 (TRM 4.7.2.101, 4.7.2.188, 4.7.2.194) with and
/// without saturation, and esp.vadd.u16.ld.incp (4.7.2.102).
#[test]
fn pie_add_sub_saturate_or_wrap() {
    let set = |c: &mut Cpu| { c.q[0] = q16([0xfff0, 1, 5, 0x8000, 0, 0xffff, 100, 7]); c.q[1] = q16([0x20, 2, 7, 1, 0, 0xffff, 100, 0]); };
    assert_eq!(l16(pie_one("06a5231b", set).0.q[2]), [0xffff, 3, 12, 0x8001, 0, 0xffff, 200, 7]);
    assert_eq!(l16(pie_one("06a4231b", set).0.q[2]), [0x0010, 3, 12, 0x8001, 0, 0xfffe, 200, 7]);
    assert_eq!(l16(pie_one("06a5271b", set).0.q[2]), [0xffd0, 0, 0, 0x7fff, 0, 0, 0, 7]);
    assert_eq!(l16(pie_one("06a4271b", set).0.q[2]), [0xffd0, 0xffff, 0xfffe, 0x7fff, 0, 0, 0, 7]);
    let set8 = |c: &mut Cpu| { c.q[0] = qb(|k| (k * 10) as u8); c.q[1] = qb(|_| 50); };
    assert_eq!(pie_one("06a1271b", set8).0.q[2], qb(|k| (k * 10).saturating_sub(50) as u8));
    assert_eq!(pie_one("06a0271b", set8).0.q[2], qb(|k| (k as u8 * 10).wrapping_sub(50)));
    let (cpu, _, _) = pie_one("0529d11f", |c| { set(c); c.x[11] = DATA + 0x10; });
    assert_eq!(l16(cpu.q[2]), [0xffff, 3, 12, 0x8001, 0, 0xffff, 200, 7]);
    assert_eq!((cpu.q[4], cpu.x[11]), (qb(|k| 16 + k as u8), DATA + 0x20));
}

/// esp.vmin.u16, esp.vcmp.eq.u16, esp.andq/orq/xorq, esp.zero.q (TRM 4.7.2.138, 4.7.2.209,
/// 4.7.2.202/204/205, 4.7.2.262).
#[test]
fn pie_min_compare_logic_zero() {
    let set = |c: &mut Cpu| { c.q[0] = q16([1, 9, 0xffff, 4, 0, 7, 8, 0x8000]); c.q[1] = q16([2, 9, 0, 4, 1, 6, 8, 0x7fff]); c.q[7] = !0; };
    assert_eq!(l16(pie_one("0744991f", set).0.q[2]), [1, 9, 0, 4, 0, 6, 8, 0x7fff]);
    assert_eq!(l16(pie_one("0741011f", set).0.q[2]), [0, 0xffff, 0, 0xffff, 0, 0, 0xffff, 0]);
    let (a, b) = (q16([1, 9, 0xffff, 4, 0, 7, 8, 0x8000]), q16([2, 9, 0, 4, 1, 6, 8, 0x7fff]));
    assert_eq!(pie_one("0580011f", set).0.q[2], a & b);
    assert_eq!(pie_one("0500011f", set).0.q[2], a | b);
    assert_eq!(pie_one("0504011f", set).0.q[2], a ^ b);
    assert_eq!(pie_one("0000139b", set).0.q[7], 0);
}

/// esp.vext.u8 q4,q5,q6 (TRM 4.7.2.251): bytes 0-7 widened into q4, 8-15 into q5.
#[test]
fn pie_vext_u8_widens_both_halves() {
    let (cpu, _, _) = pie_one("025c221b", |c| c.q[6] = qb(|k| 0xf0 + k as u8));
    assert_eq!(l16(cpu.q[4]), [0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7]);
    assert_eq!(l16(cpu.q[5]), [0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff]);
}

/// esp.vunzip.8 q0,q1 (TRM 4.7.2.254): the even bytes of q1:q0 to q0, the odd to q1.
#[test]
fn pie_vunzip_8_splits_even_and_odd_bytes() {
    let (cpu, _, _) = pie_one("0684201b", |c| { c.q[0] = qb(|k| k as u8); c.q[1] = qb(|k| 16 + k as u8); });
    assert_eq!((cpu.q[0], cpu.q[1]), (qb(|k| 2 * k as u8), qb(|k| 2 * k as u8 + 1)));
}

/// esp.vzipt.8 / esp.vunzipt.8 q0,q1,q2 (TRM 4.7.2.261, 4.7.2.256): a three-way byte interleave
/// x0 y0 w0 x1 ... across q0, q1, q2, and its inverse.
#[test]
fn pie_vzipt_vunzipt_8_interleave_three_ways() {
    let set = |c: &mut Cpu| { c.q[0] = qb(|k| k as u8); c.q[1] = qb(|k| 0x40 + k as u8); c.q[2] = qb(|k| 0x80 + k as u8); };
    let (zipped, _, _) = pie_one("0640211b", set);
    let stream: Vec<u8> = (0..16u8).flat_map(|k| [k, 0x40 + k, 0x80 + k]).collect();
    for n in 0..3 { assert_eq!(zipped.q[n], u128::from_le_bytes(stream[16 * n..16 * n + 16].try_into().unwrap()), "q{n}"); }
    assert_eq!(&zipped.q[0].to_le_bytes()[..6], &[0, 0x40, 0x80, 1, 0x41, 0x81]);
    let (back, _, _) = pie_one("0640251b", |c| c.q[..3].copy_from_slice(&zipped.q[..3]));
    assert_eq!((back.q[0], back.q[1], back.q[2]), (qb(|k| k as u8), qb(|k| 0x40 + k as u8), qb(|k| 0x80 + k as u8)));
}

/// esp.movi.32.a (TRM 4.7.2.230), esp.movx.w/r.sar (4.7.2.244/237), esp.movx.w/r.cfg
/// (4.7.2.241/234, Register 4.1: only vxsat_en, vxrm, mis_ld and mis_st are writable).
#[test]
fn pie_moves_to_and_from_general_registers() {
    let (cpu, _, _) = pie_one("0ce0211b", |c| c.q[3] = 0x4444_4444_3333_3333_2222_2222_1111_1111);
    assert_eq!(cpu.x[10], 0x3333_3333);
    let (cpu, _, _) = pie_one("90c2201b", |c| c.x[12] = 0xffff_ffc7);
    assert_eq!(cpu.pie_sar, 7);
    let (cpu, _, _) = pie_one("80c0229b", |c| c.pie_sar = 0x2a);
    assert_eq!(cpu.x[13], 0x2a);
    let (cpu, _, _) = pie_one("9082201b", |c| c.x[12] = 0xffff_ffff);
    assert_eq!(cpu.pie_cfg, 0x1f3);
    let (cpu, _, _) = pie_one("8080229b", |c| c.pie_cfg = 0x1ff);
    assert_eq!((cpu.x[13], cpu.pie_cfg), (0x1ff, 0x1fb), "reading CFG clears vxsat");
}

/// The .ip loads and stores (TRM 4.7.2.312, 4.7.2.318, 4.7.2.285, 4.7.2.274): force-aligned
/// addresses (TRM 4.4.3.1), rs1 post-incremented by the signed immediate.
#[test]
fn pie_ip_loads_and_stores_align_and_post_increment() {
    let (cpu, _, _) = pie_one("0311851f", |c| c.x[11] = DATA + 0x24);
    assert_eq!((cpu.q[1], cpu.x[11]), (qb(|k| 0x20 + k as u8), DATA + 0x44));
    let (cpu, ram, _) = pie_one("ff11879f", |c| { c.x[11] = DATA + 0x18; c.q[1] = qb(|k| 0xa0 + k as u8); });
    assert_eq!(cpu.x[11], DATA + 8);
    assert_eq!(&ram.mem[0x1010..0x1020], &qb(|k| 0xa0 + k as u8).to_le_bytes());
    assert_eq!((ram.mem[0x100f], ram.mem[0x1020]), (0x0f, 0x20));
    let (cpu, _, _) = pie_one("ff910f1f", |c| c.x[10] = DATA + 3);
    assert_eq!((l16(cpu.q[3]), cpu.x[10]), ([0x0302; 8], DATA + 1));
    let (cpu, _, _) = pie_one("03a1811f", |c| c.x[11] = DATA + 0x13);
    assert_eq!((cpu.q[0], cpu.pie_sar_byte, cpu.x[11]), (qb(|k| 0x10 + k as u8), 3, DATA + 0x23));
    // CFG.mis_ld: the hardware misaligned mode reads the bytes where they are (TRM 4.4.3.2)
    let (cpu, _, _) = pie_one("0311851f", |c| { c.x[11] = DATA + 0x24; c.pie_cfg = 2; });
    assert_eq!(cpu.q[1], qb(|k| 0x24 + k as u8));
}

/// The TRM 4.4.3.1 misaligned-load sequence: ld.128.usar.ip, vld.128.ip, src.q (TRM 4.7.2.326)
/// rebuild the 16 bytes at an address that is not 16-byte aligned.
#[test]
fn pie_usar_and_src_q_rebuild_a_misaligned_load() {
    let (mut cpu, mut ram, t) = pie_one("03a1811f 0311849f 8400411b", |c| c.x[11] = DATA + 5);
    assert_eq!(t, Ok(()));
    for _ in 0..2 { step(&mut cpu, &mut ram).unwrap(); }
    assert_eq!((cpu.q[2], cpu.x[11]), (qb(|k| 5 + k as u8), DATA + 0x25));
    // SAR_BYTE = 0 leaves qw
    let (cpu, _, _) = pie_one("8400411b", |c| { c.q[0] = 1; c.q[1] = 2; });
    assert_eq!(cpu.q[2], 1);
}

/// PIE starts Off: the instruction is illegal and EXT_ILL.PIE says why (TRM 2.7.2.3); once
/// enabled, an executed PIE instruction makes the state Dirty and mstatus.XS/SD follow. The P4
/// core, without `ISA_PIE`, decodes the same word as an ordinary illegal instruction.
#[test]
fn pie_is_lazily_enabled_like_the_fpu() {
    let mut cpu = Cpu::new_esp32s31();
    let mut ram = FlatRam::new(BASE, 64 * 1024);
    ram.mem[..4].copy_from_slice(&asm("0000139b"));
    cpu.pc = BASE; cpu.mtvec = BASE + 0x100;
    assert_eq!(step(&mut cpu, &mut ram), Err(Trap::Exception(2)));
    assert_eq!((cpu.mcause, cpu.mtval, cpu.ext_ill & 4), (2, 0x0000_139b, 4));
    assert_eq!(cpu.read_csr(csr::MSTATUS) & mstatus::XS, 0);
    cpu.write_csr(csr::MEXT_PIE_STATUS, 1);
    assert_eq!(cpu.read_csr(csr::MSTATUS) & (mstatus::XS | mstatus::SD), 1 << 15);
    cpu.pc = BASE; cpu.q[7] = 5;
    assert_eq!(step(&mut cpu, &mut ram), Ok(()));
    assert_eq!((cpu.q[7], cpu.read_csr(csr::MEXT_PIE_STATUS)), (0, 3));
    assert_eq!(cpu.read_csr(csr::MSTATUS) & (mstatus::XS | mstatus::SD), mstatus::XS | mstatus::SD);
    // XS is read-only
    let m = cpu.read_csr(csr::MSTATUS); cpu.write_csr(csr::MSTATUS, m & !mstatus::XS);
    assert_eq!(cpu.read_csr(csr::MSTATUS) & mstatus::XS, mstatus::XS);
    cpu.write_csr(csr::MEXT_PIE_STATUS, 2);
    assert_eq!(cpu.read_csr(csr::MSTATUS) & (mstatus::XS | mstatus::SD), 2 << 15);
    // a reset turns PIE off again and clears its registers
    cpu.reset();
    assert_eq!((cpu.pie_state, cpu.q, cpu.has_pie()), (0, [0; 8], true));

    let mut p4 = Cpu::new_esp32p4_rev3();
    p4.pc = BASE; p4.mtvec = BASE + 0x100;
    assert_eq!(step(&mut p4, &mut ram), Err(Trap::Exception(2)));
    assert_eq!(p4.ext_ill & 4, 0);
    assert_eq!(disasm::format_for(p4.misa, &decode_for(p4.isa(), BASE, [0x9b, 0x13, 0, 0])), "unimp");
    assert_eq!(disasm::format_for(cpu.misa, &decode_for(cpu.isa(), BASE, [0x9b, 0x13, 0, 0])), "esp.zero.q\tq7");
}
