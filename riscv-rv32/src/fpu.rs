//! The F instructions. Only a core whose `misa` has F decodes them, so the C3 and C6 never get here.

use crate::bus::Bus;
use crate::decode::{Insn, Op};
use crate::exec::Trap;
use crate::softfloat::{self as sf, Rm, SIGN};
use crate::state::{exc, ext_ill, Cpu};

fn illegal(cpu: &mut Cpu, i: &Insn, pc: u32) -> Result<(), Trap> {
    cpu.trap(exc::ILLEGAL_INSN, i.raw, pc);
    Err(Trap::Exception(exc::ILLEGAL_INSN))
}

/// The static rm field, or `frm` for DYN; reserved modes (static 5-6, dynamic 5-7) are illegal,
/// which the F chapter allows for a reserved encoding.
fn rounding_mode(cpu: &Cpu, i: &Insn) -> Option<Rm> {
    Rm::from_bits(if i.imm == 7 { (cpu.fcsr >> 5) & 7 } else { i.imm as u32 })
}

#[inline(always)]
fn write_f(cpu: &mut Cpu, rd: u8, v: u32) { cpu.f[rd as usize] = v; cpu.set_fs_dirty(); }

#[inline(never)]
pub(crate) fn exec<B: Bus>(cpu: &mut Cpu, bus: &mut B, i: &Insn, pc: u32) -> Result<(), Trap> {
    use Op::*;
    if cpu.fs_off() {
        // On rev < 3 EXT_ILL flags the computational instructions but not FLW/FSW or their
        // compressed forms (ESP-IDF v5.5.3 `soc_caps.h` SOC_CPU_HAS_FPU_EXT_ILL_BUG; `vectors.S`
        // decodes mtval for those); on rev >= 3 it flags them all (`Cpu::ext_ill_all_fp`).
        if cpu.has_ext_ill && (cpu.ext_ill_all_fp || !matches!(i.op, Flw | Fsw)) { cpu.ext_ill |= ext_ill::FPU; }
        return illegal(cpu, i, pc);
    }
    let (a, b) = (cpu.f[i.rs1 as usize], cpu.f[i.rs2 as usize]);
    match i.op {
        Flw => {
            let ad = cpu.get(i.rs1).wrapping_add(i.imm as u32);
            match bus.read32(ad) {
                Ok(v) => write_f(cpu, i.rd, v),
                Err(_) => { cpu.trap(exc::LOAD_ACCESS_FAULT, ad, pc); return Err(Trap::Exception(exc::LOAD_ACCESS_FAULT)); }
            }
        }
        Fsw => {
            let ad = cpu.get(i.rs1).wrapping_add(i.imm as u32);
            if bus.write32(ad, b).is_err() { cpu.trap(exc::STORE_ACCESS_FAULT, ad, pc); return Err(Trap::Exception(exc::STORE_ACCESS_FAULT)); }
        }
        // Sign injection and the moves copy bits: no flags, NaN payloads kept.
        FsgnjS => write_f(cpu, i.rd, (a & !SIGN) | (b & SIGN)),
        FsgnjnS => write_f(cpu, i.rd, (a & !SIGN) | (!b & SIGN)),
        FsgnjxS => write_f(cpu, i.rd, a ^ (b & SIGN)),
        FmvXW => cpu.set(i.rd, a),
        FmvWX => { let v = cpu.get(i.rs1); write_f(cpu, i.rd, v); }
        FclassS => cpu.set(i.rd, sf::classify(a)),
        FminS | FmaxS => {
            let (r, flags) = if i.op == FminS { sf::min(a, b) } else { sf::max(a, b) };
            cpu.accrue_fflags(flags);
            write_f(cpu, i.rd, r);
        }
        FeqS | FltS | FleS => {
            let (r, flags) = match i.op { FeqS => sf::eq(a, b), FltS => sf::lt(a, b), _ => sf::le(a, b) };
            cpu.accrue_fflags(flags);
            cpu.set(i.rd, r as u32);
        }
        _ => {
            let Some(rm) = rounding_mode(cpu, i) else { return illegal(cpu, i, pc) };
            let c = cpu.f[i.rs3() as usize];
            let x = cpu.get(i.rs1);
            let (r, flags) = match i.op {
                FaddS => sf::add(a, b, rm),
                FsubS => sf::sub(a, b, rm),
                FmulS => sf::mul(a, b, rm),
                FdivS => sf::div(a, b, rm),
                FsqrtS => sf::sqrt(a, rm),
                // fnmsub/fnmadd negate the product, not the sum, which matters for a zero result.
                FmaddS => sf::fma(a, b, c, rm),
                FmsubS => sf::fma(a, b, c ^ SIGN, rm),
                FnmsubS => sf::fma(a ^ SIGN, b, c, rm),
                FnmaddS => sf::fma(a ^ SIGN, b, c ^ SIGN, rm),
                FcvtSW => sf::from_i32(x, rm),
                FcvtSWu => sf::from_u32(x, rm),
                FcvtWS | FcvtWuS => {
                    let (v, flags) = if i.op == FcvtWS { sf::to_i32(a, rm) } else { sf::to_u32(a, rm) };
                    cpu.accrue_fflags(flags);
                    cpu.set(i.rd, v);
                    cpu.pc = pc.wrapping_add(i.len as u32);
                    return Ok(());
                }
                _ => unreachable!("{:?} is not an F instruction", i.op),
            };
            cpu.accrue_fflags(flags);
            write_f(cpu, i.rd, r);
        }
    }
    cpu.pc = pc.wrapping_add(i.len as u32);
    Ok(())
}
