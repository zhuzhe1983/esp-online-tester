//! A subset of Xespv2p2, Espressif's PIE v2.2 SIMD extension, as on the ESP32-S31 (and the
//! ESP32-P4: "PIE Extension support is compliant to Espressif's PIE V2.2.0 version"). Semantics
//! cite the ESP32-P4 Technical Reference Manual (v1.x, chapter 4 "Processor Instruction
//! Extensions (PIE)", the per-instruction section 4.7.2.n, and chapter 2.7.2 for enabling).
//!
//! Q registers are `u128`, lane 0 in the low bits, loaded little-endian. Memory accesses are
//! force-aligned (the low bits of the address cleared) unless CFG.mis_ld / CFG.mis_st selects the
//! hardware misaligned mode (TRM 4.4.3).

use crate::bus::Bus;
use crate::decode::{pie_q, pie_qw, pie_rm, pie_sat, Insn, Op};
use crate::state::{exc, ext_ill, Cpu};
use emu_core::Trap;

const CFG_MIS_ST: u32 = 1 << 0;
const CFG_MIS_LD: u32 = 1 << 1;
const CFG_VXSAT: u32 = 1 << 2;
const CFG_RM_EXC: u32 = 1 << 3;
const CFG_VXSAT_EN: u32 = 1 << 8;
/// TRM Table 4.4-3: rm = 7 takes CFG.vxrm; vxrm = 7 too means UNNECESSARY (FLOOR, rm_exc on an
/// inexact shift).
const RM_DYN: u32 = 7;

fn lanes16(v: u128) -> [u32; 8] { std::array::from_fn(|k| (v >> (16 * k)) as u32 & 0xffff) }
fn from16(l: [u32; 8]) -> u128 { l.iter().enumerate().fold(0, |v, (k, &x)| v | ((x & 0xffff) as u128) << (16 * k)) }
fn bytes(v: u128) -> [u8; 16] { v.to_le_bytes() }

/// `p >> s` rounded per TRM Table 4.4-3 (unsigned, so UP = CEILING and DOWN = FLOOR). Returns the
/// value and whether the shift was inexact.
pub fn round_shr(p: u64, s: u32, rm: u32) -> (u64, bool) {
    if s == 0 { return (p, false); }
    let (p, s) = (p as u128, s.min(127));
    let (q, rem, half) = (p >> s, p & ((1u128 << s) - 1), 1u128 << (s - 1));
    let up = match rm {
        1 | 2 => rem != 0,                                // CEILING, UP (away from zero)
        4 => rem >= half,                                 // HALF_UP (ties away from zero)
        5 => rem > half,                                  // HALF_DOWN (ties toward zero)
        6 => rem > half || (rem == half && q & 1 == 1),   // HALF_EVEN
        _ => false,                                       // FLOOR, DOWN, UNNECESSARY
    };
    ((q + up as u128) as u64, rem != 0)
}

/// Load 16 bytes at `ad`: from `ad & !15` in force-alignment mode, else as the bytes are.
fn load128<B: Bus>(cpu: &Cpu, bus: &mut B, ad: u32) -> Result<u128, u32> {
    let mut b = [0u8; 16];
    if cpu.pie_cfg & CFG_MIS_LD == 0 {
        let base = ad & !15;
        for k in 0..4u32 { let a = base + 4 * k; b[4 * k as usize..][..4].copy_from_slice(&bus.read32(a).map_err(|_| a)?.to_le_bytes()); }
    } else {
        for (k, x) in b.iter_mut().enumerate() { let a = ad.wrapping_add(k as u32); *x = bus.read8(a).map_err(|_| a)?; }
    }
    Ok(u128::from_le_bytes(b))
}

fn store128<B: Bus>(cpu: &Cpu, bus: &mut B, ad: u32, v: u128) -> Result<(), u32> {
    let b = v.to_le_bytes();
    if cpu.pie_cfg & CFG_MIS_ST == 0 {
        let base = ad & !15;
        for k in 0..4u32 { let a = base + 4 * k; bus.write32(a, u32::from_le_bytes(b[4 * k as usize..][..4].try_into().unwrap())).map_err(|_| a)?; }
    } else {
        for (k, &x) in b.iter().enumerate() { let a = ad.wrapping_add(k as u32); bus.write8(a, x).map_err(|_| a)?; }
    }
    Ok(())
}

/// Saturate `v` to `max` when `sat`, else keep the low bits (TRM 4.4.4); a clamp sets CFG.vxsat
/// when CFG.vxsat_en is set.
fn clamp(cpu: &mut Cpu, v: i64, max: i64, sat: bool) -> u32 {
    if !sat { return (v & max) as u32; }
    let c = v.clamp(0, max);
    if c != v && cpu.pie_cfg & CFG_VXSAT_EN != 0 { cpu.pie_cfg |= CFG_VXSAT; }
    c as u32
}

/// esp.vmul.u16 qz,qx,qy,sat,rm (TRM 4.7.2.155): each 32-bit lane product shifted right by
/// SAR[5:0], rounded by rm (TRM 4.4.5), then saturated to 16 bits with sat, else truncated.
fn vmul_u16(cpu: &mut Cpu, x: u128, y: u128, sat: bool, rm: u32) -> u128 {
    let rm = if rm == RM_DYN { (cpu.pie_cfg >> 4) & 0xf } else { rm };
    let (a, b, s) = (lanes16(x), lanes16(y), cpu.pie_sar & 0x3f);
    let mut out = [0u32; 8];
    for k in 0..8 {
        let (v, inexact) = round_shr(a[k] as u64 * b[k] as u64, s, rm);
        // vxrm = 7 (or a reserved 8-15): UNNECESSARY, FLOOR that reports an inexact shift
        if rm >= RM_DYN && inexact { cpu.pie_cfg |= CFG_RM_EXC; }
        out[k] = clamp(cpu, v as i64, 0xffff, sat);
    }
    from16(out)
}

/// esp.vadd.u16 / esp.vsub.u16 (TRM 4.7.2.101, 4.7.2.188): per-lane sum or difference,
/// saturated to [0, 0xffff] with sat, else wrapped.
fn vaddsub_u16(cpu: &mut Cpu, x: u128, y: u128, sub: bool, sat: bool) -> u128 {
    let (a, b) = (lanes16(x), lanes16(y));
    let mut out = [0u32; 8];
    for k in 0..8 { let v = if sub { a[k] as i64 - b[k] as i64 } else { a[k] as i64 + b[k] as i64 }; out[k] = clamp(cpu, v, 0xffff, sat); }
    from16(out)
}

/// Execute a PIE instruction. With mext_pie_status.STATE = Off it is illegal and EXT_ILL.PIE is
/// set (TRM 2.7.2.3); otherwise it leaves the state Dirty.
#[cold]
#[inline(never)]
pub fn exec<B: Bus>(cpu: &mut Cpu, bus: &mut B, i: &Insn, pc: u32) -> Result<(), Trap> {
    use Op::*;
    if cpu.pie_state == 0 {
        if cpu.has_ext_ill { cpu.ext_ill |= ext_ill::PIE; }
        cpu.trap(exc::ILLEGAL_INSN, i.raw, pc);
        return Err(Trap::Exception(exc::ILLEGAL_INSN));
    }
    let w = i.raw;
    let qs = cpu.q;
    let q = |n: usize| qs[n];
    let (q7, q10, q20, q26, q29, qw) = (pie_q(w, 7), pie_q(w, 10), pie_q(w, 20), pie_q(w, 26), pie_q(w, 29), pie_qw(w));
    let a = cpu.get(i.rs1);
    macro_rules! load { ($ad:expr) => { match load128(cpu, bus, $ad) { Ok(v) => v, Err(f) => { cpu.trap(exc::LOAD_ACCESS_FAULT, f, pc); return Err(Trap::Exception(exc::LOAD_ACCESS_FAULT)); } } }; }
    match i.op {
        // qz = qx * qy (operand order of the syntax: qz bits 9:7, qx 31:29, qy 28:26)
        EspVmulU16 => { let v = vmul_u16(cpu, q(q29), q(q26), pie_sat(i.op, w), pie_rm(i.op, w)); cpu.q[q7] = v; }
        // TRM 4.7.2.156: the product as vmul.u16, and qu = load128(rs1), rs1 += 16. The sources
        // are read before either write; qu is written last.
        EspVmulU16LdIncp => {
            let (x, y) = (q(q29), q(q26));
            let m = load!(a);
            let v = vmul_u16(cpu, x, y, pie_sat(i.op, w), pie_rm(i.op, w));
            cpu.q[q7] = v; cpu.q[q10] = m; cpu.set(i.rs1, a.wrapping_add(16));
        }
        EspVaddU16 | EspVsubU16 => { let v = vaddsub_u16(cpu, q(q29), q(q26), i.op == EspVsubU16, pie_sat(i.op, w)); cpu.q[q20] = v; }
        // TRM 4.7.2.102: the sum as vadd.u16 into qv, and qu = load128(rs1), rs1 += 16
        EspVaddU16LdIncp => {
            let (x, y) = (q(q29), q(q26));
            let m = load!(a);
            let v = vaddsub_u16(cpu, x, y, false, pie_sat(i.op, w));
            cpu.q[q20] = v; cpu.q[q10] = m; cpu.set(i.rs1, a.wrapping_add(16));
        }
        // TRM 4.7.2.194: per-byte difference, saturated to [0, 0xff] with sat, else wrapped
        EspVsubU8 => {
            let (x, y, sat) = (bytes(q(q29)), bytes(q(q26)), pie_sat(i.op, w));
            let mut o = [0u8; 16];
            for k in 0..16 { o[k] = clamp(cpu, x[k] as i64 - y[k] as i64, 0xff, sat) as u8; }
            cpu.q[q20] = u128::from_le_bytes(o);
        }
        // TRM 4.7.2.138: the smaller unsigned lane
        EspVminU16 => { let (x, y) = (lanes16(q(q29)), lanes16(q(q26))); cpu.q[q7] = from16(std::array::from_fn(|k| x[k].min(y[k]))); }
        // TRM 4.7.2.209: 0xffff where the lanes are equal, else 0
        EspVcmpEqU16 => { let (x, y) = (lanes16(q(q29)), lanes16(q(q26))); cpu.q[q7] = from16(std::array::from_fn(|k| if x[k] == y[k] { 0xffff } else { 0 })); }
        // TRM 4.7.2.202, 4.7.2.204, 4.7.2.205
        EspAndq => cpu.q[q7] = q(q29) & q(q26),
        EspOrq => cpu.q[q7] = q(q29) | q(q26),
        EspXorq => cpu.q[q7] = q(q29) ^ q(q26),
        // TRM 4.7.2.262
        EspZeroQ => cpu.q[q7] = 0,
        // TRM 4.7.2.251 esp.vext.u8 qz,qv,qw: bytes 0-7 of qw zero-extended into qz's lanes,
        // bytes 8-15 into qv's
        EspVextU8 => {
            let b = bytes(q(qw));
            let (lo, hi) = (from16(std::array::from_fn(|k| b[k] as u32)), from16(std::array::from_fn(|k| b[8 + k] as u32)));
            cpu.q[q7] = lo; cpu.q[q20] = hi;
        }
        // TRM 4.7.2.254 esp.vunzip.8 qx,qy: of the 32 bytes qy:qx, the even ones go to qx, the odd to qy
        EspVunzip8 => {
            let (x, y) = (bytes(q(q29)), bytes(q(q26)));
            let all: Vec<u8> = x.iter().chain(y.iter()).copied().collect();
            let (mut e, mut o) = ([0u8; 16], [0u8; 16]);
            for k in 0..16 { e[k] = all[2 * k]; o[k] = all[2 * k + 1]; }
            cpu.q[q29] = u128::from_le_bytes(e); cpu.q[q26] = u128::from_le_bytes(o);
        }
        // TRM 4.7.2.261 esp.vzipt.8 qx,qy,qw: the bytes interleaved x0 y0 w0 x1 y1 w1 ...; the 48
        // bytes go to qx, qy, qw in turn
        EspVzipt8 => {
            let (x, y, z) = (bytes(q(q29)), bytes(q(q26)), bytes(q(qw)));
            let mut s = [0u8; 48];
            for k in 0..16 { s[3 * k] = x[k]; s[3 * k + 1] = y[k]; s[3 * k + 2] = z[k]; }
            let part = |n: usize| u128::from_le_bytes(s[16 * n..16 * n + 16].try_into().unwrap());
            let (t0, t1, t2) = (part(0), part(1), part(2));
            cpu.q[q29] = t0; cpu.q[q26] = t1; cpu.q[qw] = t2;
        }
        // TRM 4.7.2.256 esp.vunzipt.8 qx,qy,qw: the inverse; of the 48 bytes qx, qy, qw, every third
        // from 0 goes to qx, from 1 to qy, from 2 to qw
        EspVunzipt8 => {
            let (x, y, z) = (bytes(q(q29)), bytes(q(q26)), bytes(q(qw)));
            let s: Vec<u8> = x.iter().chain(y.iter()).chain(z.iter()).copied().collect();
            let pick = |r: usize| u128::from_le_bytes(std::array::from_fn(|k| s[3 * k + r]));
            let (t0, t1, t2) = (pick(0), pick(1), pick(2));
            cpu.q[q29] = t0; cpu.q[q26] = t1; cpu.q[qw] = t2;
        }
        // TRM 4.7.2.326 esp.src.q qz,qw,qy: qz = ({qy, qw} >> SAR_BYTE * 8)[127:0]
        EspSrcQ => {
            let (lo, hi, s) = (q(qw), q(q26), 8 * (cpu.pie_sar_byte & 0xf));
            cpu.q[q7] = if s == 0 { lo } else { lo >> s | hi << (128 - s) };
        }
        // TRM 4.7.2.312: qu = load128(rs1), rs1 += imm
        EspVld128Ip => { let v = load!(a); cpu.q[q10] = v; cpu.set(i.rs1, a.wrapping_add(i.imm as u32)); }
        // TRM 4.7.2.318: MEM(rs1) = qu, rs1 += imm
        EspVst128Ip => {
            if let Err(f) = store128(cpu, bus, a, q(q10)) { cpu.trap(exc::STORE_ACCESS_FAULT, f, pc); return Err(Trap::Exception(exc::STORE_ACCESS_FAULT)); }
            cpu.set(i.rs1, a.wrapping_add(i.imm as u32));
        }
        // TRM 4.7.2.285: one halfword (address force-aligned to 2, TRM 4.4.3.1) in all eight lanes,
        // rs1 += imm
        EspVldbc16Ip => {
            let r = if cpu.pie_cfg & CFG_MIS_LD == 0 { bus.read16(a & !1).map_err(|_| a & !1) }
                    else { bus.read8(a).and_then(|l| Ok(l as u16 | (bus.read8(a.wrapping_add(1))? as u16) << 8)).map_err(|_| a) };
            let h = match r { Ok(h) => h, Err(f) => { cpu.trap(exc::LOAD_ACCESS_FAULT, f, pc); return Err(Trap::Exception(exc::LOAD_ACCESS_FAULT)); } };
            cpu.q[q10] = from16([h as u32; 8]);
            cpu.set(i.rs1, a.wrapping_add(i.imm as u32));
        }
        // TRM 4.7.2.274: qu = load128(rs1), SAR_BYTE = rs1[3:0], rs1 += imm
        EspLd128UsarIp => {
            let v = load!(a);
            cpu.q[q10] = v; cpu.pie_sar_byte = a & 0xf; cpu.set(i.rs1, a.wrapping_add(i.imm as u32));
        }
        // TRM 4.7.2.244, 4.7.2.237: SAR[5:0] = rs1[5:0]; rd = SAR zero-extended
        EspMovxWSar => cpu.pie_sar = a & 0x3f,
        EspMovxRSar => { let v = cpu.pie_sar; cpu.set(i.rd, v); }
        // TRM 4.7.2.241, 4.7.2.234 (and Register 4.1 for the writable fields)
        EspMovxWCfg => cpu.write_pie_cfg(a),
        EspMovxRCfg => { let v = cpu.read_pie_cfg(); cpu.set(i.rd, v); }
        // TRM 4.7.2.230: rd = 32-bit lane sel of qy
        EspMovi32A => { let v = (q(q26) >> (32 * i.imm)) as u32; cpu.set(i.rd, v); }
        _ => unreachable!("not a PIE instruction: {:?}", i.op),
    }
    cpu.set_pie_state(3);
    cpu.pc = pc.wrapping_add(4);
    Ok(())
}
