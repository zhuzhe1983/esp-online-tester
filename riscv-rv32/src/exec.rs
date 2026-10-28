//! The interpreter. One instruction per `step()`; 1 instruction = 1 cycle, as in the Xtensa core.

use crate::bus::{Bus, Fault};
use crate::decode::{decode_for, Insn, Op};
use crate::state::{csr, exc, ext_ill, Cpu};
use emu_core::{CacheOperation, ControlEvent, ControlEventKind, StepKind, StepOutcome};

pub use emu_core::Trap;

macro_rules! ld {
    ($cpu:expr, $bus:expr, $f:ident, $addr:expr, $pc:expr) => {
        match $bus.$f($addr) {
            Ok(v) => v,
            Err(_) => { $cpu.trap(exc::LOAD_ACCESS_FAULT, $addr, $pc); return Err(Trap::Exception(exc::LOAD_ACCESS_FAULT)); }
        }
    };
}
macro_rules! st {
    ($cpu:expr, $bus:expr, $f:ident, $addr:expr, $v:expr, $pc:expr) => {
        if $bus.$f($addr, $v).is_err() {
            $cpu.trap(exc::STORE_ACCESS_FAULT, $addr, $pc);
            return Err(Trap::Exception(exc::STORE_ACCESS_FAULT));
        }
    };
}

/// Execute one instruction.
pub fn step<B: Bus>(cpu: &mut Cpu, bus: &mut B) -> Result<(), Trap> { step_outcome(cpu, bus).result() }

/// A CLIC candidate as the SoC presents it (`Cpu::clic`): the CPU line (CLIC ID - 16) in bits
/// 4:0, the 8-bit level in bits 15:8, the SHV (hardware-vectored) attribute in bit 16. On the
/// P4 rev < 3 the SoC has applied the threshold; the core applies its own level and, with the
/// specification's CLIC (`Cpu::clic_std`), `mintthresh`.
pub const CLIC_IRQ_LEVEL_SHIFT: u32 = 8;
pub const CLIC_IRQ_SHV: u32 = 1 << 16;
/// IDF's CPU lines 0-31 are CLIC IDs 16-47 (ESP-IDF v5.5.3 riscv/include/esp_private/interrupt_clic.h:40-41).
pub const CLIC_EXT_OFFSET: u32 = 16;

/// Take a CLIC interrupt (CLIC specification, "CLIC Interrupt Operation"), once the caller has
/// seen it is strictly above the current level with MIE set and `mtvec` in mode 3. mcause gets
/// the ID and the previous level, the level becomes the interrupt's, and an SHV interrupt jumps
/// through `mtvt + 4*id`, a memory read made with MINHV set; the rest enter at the 64-byte-
/// aligned `mtvec` base. A fault on that read is taken as an instruction access fault at the
/// table entry with MINHV left set; the specification's resumption of the table fetch by `mret`
/// is not modelled (IDF's table is in mapped RAM).
#[cold]
#[inline(never)]
fn clic_take<B: Bus>(cpu: &mut Cpu, bus: &mut B, irq: u32, pc: u32) -> StepOutcome {
    use crate::state::{mcause, mstatus};
    let level = (irq >> CLIC_IRQ_LEVEL_SHIFT) & 0xff;
    let (line, id) = (irq & 0x1f, (irq & 0x1f) + CLIC_EXT_OFFSET);
    cpu.insn_count += 1; cpu.cycle_count += 1;
    cpu.mepc = pc;
    cpu.mtval = 0;
    cpu.reservation = None;
    let mpie = if cpu.mstatus & mstatus::MIE != 0 { mstatus::MPIE } else { 0 };
    cpu.mstatus = (cpu.mstatus & !(mstatus::MIE | mstatus::MPIE)) | mpie | mstatus::MPP;
    cpu.mcause = mcause::INTERRUPT | cpu.mil << mcause::MPIL_SHIFT | id;
    cpu.mil = level;
    if irq & CLIC_IRQ_SHV != 0 {
        let entry = cpu.mtvt.wrapping_add(4 * id);
        match bus.read32(entry) {
            Ok(target) => cpu.pc = target & !1,
            Err(_) => {
                cpu.trap(exc::INSN_ACCESS_FAULT, entry, entry);
                cpu.mcause |= mcause::MINHV;
                return StepOutcome { pc, next_pc: cpu.pc, bytes: None, length: 0, kind: StepKind::TrapBefore(Trap::Exception(exc::INSN_ACCESS_FAULT)), control: None };
            }
        }
    } else {
        cpu.pc = cpu.mtvec & !0x3f;
    }
    StepOutcome { pc, next_pc: cpu.pc, bytes: None, length: 0, kind: StepKind::TrapBefore(Trap::Interrupt(line)), control: None }
}

/// Execute one slow-path event and retain its exact fetch window and trap timing.
pub fn step_outcome<B: Bus>(cpu: &mut Cpu, bus: &mut B) -> StepOutcome {
    let pc = cpu.pc;
    // The SoC's INTC decides enable/priority and hands us the line (`Core::set_irq`); the CPU
    // only gates on mstatus.MIE, or with a CLIC on MIE and its own level too.
    if cpu.waiting || cpu.mie_enabled() {
        if let Some(line) = cpu.irq {
            if cpu.clic {
                // In mtvec modes 0 and 1 (CLINT mode) interrupts are mie/mip's, not the CLIC's,
                // and nothing raises mip on this chip; the ROM runs there with mie = 0.
                if cpu.clic_mode() {
                    // any presented interrupt ends a WFI: the privileged ISA lets WFI resume for
                    // any reason, and a core left asleep could not be woken by a lower one
                    cpu.waiting = false;
                    if cpu.mie_enabled() && (line >> CLIC_IRQ_LEVEL_SHIFT) & 0xff > cpu.mil.max(cpu.mintthresh) { return clic_take(cpu, bus, line, pc); }
                }
            } else {
                cpu.waiting = false;
                if cpu.mie_enabled() {
                    cpu.insn_count += 1; cpu.cycle_count += 1;
                    cpu.trap(0x8000_0000 | line, 0, pc);
                    return StepOutcome { pc, next_pc: cpu.pc, bytes: None, length: 0, kind: StepKind::TrapBefore(Trap::Interrupt(line)), control: None };
                }
            }
        }
    }
    if cpu.waiting {
        cpu.insn_count += 1; cpu.cycle_count += 1;
        return StepOutcome { pc, next_pc: pc, bytes: None, length: 0, kind: StepKind::Idle, control: None };
    }

    bus.note_pc(pc);
    let bytes = match bus.fetch(pc) {
        Ok(b) => b,
        Err(_) => {
            cpu.insn_count += 1; cpu.cycle_count += 1;
            cpu.trap(exc::INSN_ACCESS_FAULT, pc, pc);
            return StepOutcome { pc, next_pc: cpu.pc, bytes: None, length: 0, kind: StepKind::TrapBefore(Trap::Exception(exc::INSN_ACCESS_FAULT)), control: None };
        }
    };
    let i = decode_for(cpu.misa, pc, bytes);
    cpu.insn_count += 1; cpu.cycle_count += 1;
    let control = if i.op == Op::FenceI {
        Some(ControlEvent { kind: ControlEventKind::Cache(CacheOperation::FenceInstruction), address: 0 })
    } else { None };
    let result = exec_insn(cpu, bus, &i, pc);
    let kind = match result {
        Ok(()) => { cpu.retired_count += 1; StepKind::Retired }
        Err(trap) => StepKind::TrapDuring(trap),
    };
    StepOutcome { pc, next_pc: cpu.pc, bytes: Some(bytes), length: i.len, kind, control }
}

#[inline]
pub fn exec_insn<B: Bus>(cpu: &mut Cpu, bus: &mut B, i: &Insn, pc: u32) -> Result<(), Trap> {
    use Op::*;
    let next = pc.wrapping_add(i.len as u32);
    let a = cpu.get(i.rs1);
    let b = cpu.get(i.rs2);
    let imm = i.imm;
    let immu = imm as u32;
    let mut new_pc = next;

    match i.op {
        Illegal => { cpu.trap(exc::ILLEGAL_INSN, i.raw, pc); return Err(Trap::Exception(exc::ILLEGAL_INSN)); }

        Lui => cpu.set(i.rd, immu),
        Auipc => cpu.set(i.rd, pc.wrapping_add(immu)),
        Jal => { cpu.set(i.rd, next); new_pc = immu; }
        Jalr => { let t = a.wrapping_add(immu) & !1; cpu.set(i.rd, next); new_pc = t; }

        Beq => if a == b { new_pc = immu },
        Bne => if a != b { new_pc = immu },
        Blt => if (a as i32) < (b as i32) { new_pc = immu },
        Bge => if (a as i32) >= (b as i32) { new_pc = immu },
        Bltu => if a < b { new_pc = immu },
        Bgeu => if a >= b { new_pc = immu },

        Lb => { let ad = a.wrapping_add(immu); let v = ld!(cpu, bus, read8, ad, pc); cpu.set(i.rd, v as i8 as i32 as u32); }
        Lh => { let ad = a.wrapping_add(immu); let v = ld!(cpu, bus, read16, ad, pc); cpu.set(i.rd, v as i16 as i32 as u32); }
        Lw => { let ad = a.wrapping_add(immu); let v = ld!(cpu, bus, read32, ad, pc); cpu.set(i.rd, v); }
        Lbu => { let ad = a.wrapping_add(immu); let v = ld!(cpu, bus, read8, ad, pc); cpu.set(i.rd, v as u32); }
        Lhu => { let ad = a.wrapping_add(immu); let v = ld!(cpu, bus, read16, ad, pc); cpu.set(i.rd, v as u32); }
        Sb => { let ad = a.wrapping_add(immu); st!(cpu, bus, write8, ad, b as u8, pc); }
        Sh => { let ad = a.wrapping_add(immu); st!(cpu, bus, write16, ad, b as u16, pc); }
        Sw => { let ad = a.wrapping_add(immu); st!(cpu, bus, write32, ad, b, pc); }

        Addi => cpu.set(i.rd, a.wrapping_add(immu)),
        Slti => cpu.set(i.rd, ((a as i32) < imm) as u32),
        Sltiu => cpu.set(i.rd, (a < immu) as u32),
        Xori => cpu.set(i.rd, a ^ immu),
        Ori => cpu.set(i.rd, a | immu),
        Andi => cpu.set(i.rd, a & immu),
        Slli => cpu.set(i.rd, a << (immu & 31)),
        Srli => cpu.set(i.rd, a >> (immu & 31)),
        Srai => cpu.set(i.rd, ((a as i32) >> (immu & 31)) as u32),

        Add => cpu.set(i.rd, a.wrapping_add(b)),
        Sub => cpu.set(i.rd, a.wrapping_sub(b)),
        Sll => cpu.set(i.rd, a << (b & 31)),
        Slt => cpu.set(i.rd, ((a as i32) < (b as i32)) as u32),
        Sltu => cpu.set(i.rd, (a < b) as u32),
        Xor => cpu.set(i.rd, a ^ b),
        Srl => cpu.set(i.rd, a >> (b & 31)),
        Sra => cpu.set(i.rd, ((a as i32) >> (b & 31)) as u32),
        Or => cpu.set(i.rd, a | b),
        And => cpu.set(i.rd, a & b),

        Mul => cpu.set(i.rd, a.wrapping_mul(b)),
        Mulh => cpu.set(i.rd, (((a as i32 as i64) * (b as i32 as i64)) >> 32) as u32),
        Mulhsu => cpu.set(i.rd, (((a as i32 as i64) * (b as u64 as i64)) >> 32) as u32),
        Mulhu => cpu.set(i.rd, (((a as u64) * (b as u64)) >> 32) as u32),
        // RISC-V defines division by zero and the signed overflow case, rather than trapping
        Div => cpu.set(i.rd, if b == 0 { u32::MAX } else if a == 0x8000_0000 && b == u32::MAX { a } else { ((a as i32).wrapping_div(b as i32)) as u32 }),
        Divu => { #[allow(clippy::manual_checked_ops, reason = "RISC-V defines division by zero as an architectural result")] let v = if b == 0 { u32::MAX } else { a / b }; cpu.set(i.rd, v); }
        Rem => cpu.set(i.rd, if b == 0 { a } else if a == 0x8000_0000 && b == u32::MAX { 0 } else { ((a as i32).wrapping_rem(b as i32)) as u32 }),
        Remu => cpu.set(i.rd, if b == 0 { a } else { a % b }),

        Fence | FenceI => {}

        // RV32A. The core keeps its own reservation; the bus says whether another hart's store
        // has broken it (one hart: never).
        LrW => { let v = ld!(cpu, bus, read32, a, pc); cpu.reservation = Some(a); bus.reserve(a); cpu.set(i.rd, v); }
        ScW => {
            let held = bus.reservation_held(a);
            if cpu.reservation == Some(a) && held { st!(cpu, bus, write32, a, b, pc); cpu.set(i.rd, 0); } else { cpu.set(i.rd, 1); }
            cpu.reservation = None;
        }
        AmoSwapW | AmoAddW | AmoXorW | AmoAndW | AmoOrW | AmoMinW | AmoMaxW | AmoMinuW | AmoMaxuW => {
            let old = ld!(cpu, bus, read32, a, pc);
            let new = match i.op {
                AmoSwapW => b, AmoAddW => old.wrapping_add(b), AmoXorW => old ^ b, AmoAndW => old & b, AmoOrW => old | b,
                AmoMinW => if (old as i32) < (b as i32) { old } else { b },
                AmoMaxW => if (old as i32) > (b as i32) { old } else { b },
                AmoMinuW => old.min(b), _ => old.max(b),
            };
            st!(cpu, bus, write32, a, new, pc);
            cpu.set(i.rd, old);
        }

        Ecall => { cpu.trap(exc::ECALL_M, 0, pc); return Err(Trap::Exception(exc::ECALL_M)); }
        Ebreak => { cpu.trap(exc::BREAKPOINT, pc, pc); return Err(Trap::Ebreak(pc)); }
        Mret => { cpu.mret(); return Ok(()); }
        Wfi => { cpu.waiting = true; }

        Csrrw | Csrrs | Csrrc | Csrrwi | Csrrsi | Csrrci => {
            let n = immu;
            // fflags/frm/fcsr are FP state: illegal with FS = Off. EXT_ILL does not flag these on the
            // P4 rev < 3 (ESP-IDF v5.5.3 `components/riscv/vectors.S` decodes mtval for them
            // instead); on rev >= 3 it does (`Cpu::ext_ill_all_fp`).
            if n.wrapping_sub(csr::FFLAGS) < 3 && cpu.has_f() && cpu.fs_off() {
                if cpu.has_ext_ill && cpu.ext_ill_all_fp { cpu.ext_ill |= ext_ill::FPU; }
                cpu.trap(exc::ILLEGAL_INSN, i.raw, pc);
                return Err(Trap::Exception(exc::ILLEGAL_INSN));
            }
            let src = if matches!(i.op, Csrrwi | Csrrsi | Csrrci) { i.rs1 as u32 } else { a };
            // A read is only skipped for csrrw with rd = x0; a write only for set/clear with a
            // zero source operand — which is how firmware reads a CSR without disturbing it.
            let old = if matches!(i.op, Csrrw | Csrrwi) && i.rd == 0 { 0 } else { cpu.read_csr(n) };
            let write = match i.op {
                Csrrw | Csrrwi => Some(src),
                Csrrs | Csrrsi => if i.rs1 != 0 { Some(old | src) } else { None },
                _ => if i.rs1 != 0 { Some(old & !src) } else { None },
            };
            if let Some(v) = write { cpu.write_csr(n, v); }
            cpu.set(i.rd, old);
        }

        Flw | Fsw | FmaddS | FmsubS | FnmsubS | FnmaddS | FaddS | FsubS | FmulS | FdivS | FsqrtS
        | FsgnjS | FsgnjnS | FsgnjxS | FminS | FmaxS | FcvtWS | FcvtWuS | FcvtSW | FcvtSWu | FmvXW | FmvWX
        | FeqS | FltS | FleS | FclassS => return crate::fpu::exec(cpu, bus, i, pc),
    }
    cpu.pc = new_pc;
    Ok(())
}

/// So the SoC can report a bus error without knowing the trap encoding.
pub fn fault_cause(f: Fault, store: bool) -> u32 {
    match f {
        Fault::Misaligned => if store { exc::STORE_MISALIGNED } else { exc::LOAD_MISALIGNED },
        _ => if store { exc::STORE_ACCESS_FAULT } else { exc::LOAD_ACCESS_FAULT },
    }
}
