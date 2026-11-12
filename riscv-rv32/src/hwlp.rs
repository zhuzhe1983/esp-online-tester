//! Xesploop, Espressif's hardware loops, as on the ESP32-S31 and the ESP32-P4. Semantics cite the
//! ESP32-P4 Technical Reference Manual (v1.x, section 2.7.1 "Hardware Loop": 2.7.1.3 for the
//! nesting and priority, 2.7.1.5 for the constraints, Table 2.7-2 for the instructions, Registers
//! 2.64 and 2.79-2.93 for the CSRs).
//!
//! Each loop has a start, an end (the address of its last instruction) and a count. When the
//! instruction at a loop's end retires with a non-zero count, the count goes down by one and, if
//! it is still non-zero, execution continues at the start; so a count of n runs the body n times
//! and leaves the count 0, which disables the loop. Loop 0 is the inner loop and wins when both
//! end at the same instruction (2.7.1.3); when it falls through there, loop 1 is checked. The
//! state lives in CSRs, so an interrupt taken inside a body saves the next pc in mepc (the start,
//! after a back-edge) and resumes exactly; ESP-IDF saves the CSRs when the state is Dirty
//! (ESP-IDF v5.5.4 freertos/FreeRTOS-Kernel/portable/riscv/portasm.S `hwlp_save_regs`).
//!
//! With mhwloop_state_reg.STATE = Off every HWLP instruction and loop CSR access is illegal and
//! sets EXT_ILL.HWLP (Registers 2.64, 2.85), and no loop runs: IDF turns the state Off while an ISR
//! runs (portasm.S `rtos_int_enter`), with the interrupted task's loop still in the CSRs. Any
//! instruction, CSR write or back-edge makes the state Dirty.

use crate::decode::{hwlp_count_imm, Insn, Op};
use crate::state::{exc, ext_ill, Cpu};
use emu_core::Trap;

/// The loop count register is 24 bits (TRM Registers 2.81, 2.84).
const COUNT_MASK: u32 = 0xff_ffff;

/// Execute a hardware-loop instruction (TRM Table 2.7-2). esp.lp.setup/setupi start the body at
/// the next instruction (pc + 4).
#[cold]
#[inline(never)]
pub fn exec(cpu: &mut Cpu, i: &Insn, pc: u32) -> Result<(), Trap> {
    use Op::*;
    if cpu.hwlp_state & 3 == 0 { return Err(off(cpu, i.raw, pc)); }
    let l = i.rd as usize & 1;
    let next = pc.wrapping_add(4);
    match i.op {
        EspLpSetup => { cpu.hwlp_count[l] = cpu.get(i.rs1) & COUNT_MASK; cpu.hwlp_start[l] = next; cpu.hwlp_end[l] = i.imm as u32; }
        EspLpSetupi => { cpu.hwlp_count[l] = hwlp_count_imm(i.raw); cpu.hwlp_start[l] = next; cpu.hwlp_end[l] = i.imm as u32; }
        EspLpStarti => cpu.hwlp_start[l] = i.imm as u32,
        EspLpEndi => cpu.hwlp_end[l] = i.imm as u32,
        EspLpCount => cpu.hwlp_count[l] = cpu.get(i.rs1) & COUNT_MASK,
        EspLpCounti => cpu.hwlp_count[l] = i.imm as u32,
        _ => unreachable!(),
    }
    cpu.set_hwlp_state(3);
    cpu.pc = next;
    Ok(())
}

/// The illegal-instruction trap of an instruction or CSR access with the hardware loops Off.
#[cold]
pub fn off(cpu: &mut Cpu, raw: u32, pc: u32) -> Trap {
    if cpu.has_ext_ill { cpu.ext_ill |= ext_ill::HWLP; }
    cpu.trap(exc::ILLEGAL_INSN, raw, pc);
    Trap::Exception(exc::ILLEGAL_INSN)
}

/// A loop CSR (mhwloopN_*, uhwloopN_*), which is illegal to access with the state Off.
#[inline(always)]
pub fn is_loop_csr(n: u32) -> bool { matches!(n, 0x7C6..=0x7CB | 0x8C0..=0x8C5) }

/// After the instruction at `pc` retired: take a loop's back-edge if `pc` ends an active loop.
/// The back-edge replaces whatever next pc the instruction chose (a branch or jump as a loop's
/// last instruction is undefined, TRM 2.7.1.5). It retires nothing, so the instruction count
/// stays one per executed instruction.
#[inline(always)]
pub fn retire(cpu: &mut Cpu, pc: u32) {
    if cpu.hwlp_count[0] | cpu.hwlp_count[1] != 0 { back_edge(cpu, pc); }
}

#[inline(never)]
fn back_edge(cpu: &mut Cpu, pc: u32) {
    if cpu.hwlp_state & 3 == 0 { return; }
    for l in 0..2 {
        if cpu.hwlp_count[l] != 0 && pc == cpu.hwlp_end[l] {
            cpu.hwlp_count[l] -= 1;
            if cpu.hwlp_state & 3 != 3 { cpu.set_hwlp_state(3); }
            if cpu.hwlp_count[l] != 0 { cpu.pc = cpu.hwlp_start[l]; return; }
        }
    }
}
