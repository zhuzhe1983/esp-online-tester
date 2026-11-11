//! RV32IMAC decoder (the C3 is IMC, the C6 adds A), plus F and its compressed loads and stores
//! for a core whose `misa` has F. Compressed instructions are expanded to their 32-bit equivalent for
//! execution, but remember which `c.*` form they came from so the disassembler can be checked
//! against `riscv32-esp-elf-objdump` (tests/objdump_diff.rs).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Illegal,
    // RV32I
    Lui, Auipc, Jal, Jalr,
    Beq, Bne, Blt, Bge, Bltu, Bgeu,
    Lb, Lh, Lw, Lbu, Lhu,
    Sb, Sh, Sw,
    Addi, Slti, Sltiu, Xori, Ori, Andi, Slli, Srli, Srai,
    Add, Sub, Sll, Slt, Sltu, Xor, Srl, Sra, Or, And,
    Fence, FenceI,
    Ecall, Ebreak, Mret, Wfi,
    Csrrw, Csrrs, Csrrc, Csrrwi, Csrrsi, Csrrci,
    // RV32M
    Mul, Mulh, Mulhsu, Mulhu, Div, Divu, Rem, Remu,
    // RV32A (the C6); `imm` carries the aq/rl bits for the disassembler
    LrW, ScW, AmoSwapW, AmoAddW, AmoXorW, AmoAndW, AmoOrW, AmoMinW, AmoMaxW, AmoMinuW, AmoMaxuW,
    // Zba, Zbb, Zbs (bit-manipulation v1.0.0), when the core has `ISA_ZB`; shift amounts in `imm`
    Sh1add, Sh2add, Sh3add,
    Andn, Orn, Xnor, Clz, Ctz, Cpop, Max, Maxu, Min, Minu, SextB, SextH, ZextH, Rol, Ror, Rori, OrcB, Rev8,
    Bclr, Bclri, Bext, Bexti, Binv, Binvi, Bset, Bseti,
    // Zcmp (Zc v1.0.0), when the core has `ISA_ZC`: `rs2` = rlist (4-15), `imm` = the stack
    // adjustment in bytes; the moves name s-registers r1s' in `rs1`, r2s' in `rs2`
    CmPush, CmPop, CmPopret, CmPopretz, CmMvsa01, CmMva01s,
    // RV32F; `imm` carries the rm field (funct3) of the computational ones, rs3 stays in `raw`.
    // Kept last: `is_f` compares discriminants.
    Flw, Fsw,
    FmaddS, FmsubS, FnmsubS, FnmaddS,
    FaddS, FsubS, FmulS, FdivS, FsqrtS,
    FsgnjS, FsgnjnS, FsgnjxS, FminS, FmaxS,
    FcvtWS, FcvtWuS, FcvtSW, FcvtSWu, FmvXW, FmvWX,
    FeqS, FltS, FleS, FclassS,
}

impl Op {
    /// An F instruction, which a core reaches only when its `misa` has F.
    #[inline(always)]
    pub fn is_f(self) -> bool { (self as u8) >= (Op::Flw as u8) }
}

/// Which compressed form an instruction was written as (`None` = a real 32-bit instruction).
/// Execution ignores this; only the disassembler cares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comp {
    None,
    Addi4spn, Lw, Sw,
    Nop, Addi, Jal, Li, Addi16sp, Lui, Srli, Srai, Andi, Sub, Xor, Or, And, J, Beqz, Bnez,
    Slli, Lwsp, Jr, Mv, Ebreak, Jalr, Add, Swsp,
    Flw, Fsw, Flwsp, Fswsp,
    // Zcb; Zcmp prints only as itself
    Lbu, Lhu, Lh, Sb, Sh, ZextB, SextB, ZextH, SextH, Not, Mul,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Insn {
    pub op: Op,
    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,
    /// signed immediate; for `jal`/branches the **absolute** target, for CSR ops the CSR number
    pub imm: i32,
    /// 2 for compressed, 4 otherwise
    pub len: u8,
    /// the compressed mnemonic this came from, for the disassembler
    pub comp: Comp,
    pub raw: u32,
}

impl Insn {
    const fn new(op: Op, rd: u8, rs1: u8, rs2: u8, imm: i32, len: u8, raw: u32) -> Insn {
        Insn { op, rd, rs1, rs2, imm, len, comp: Comp::None, raw }
    }
    const fn c(mut self, comp: Comp) -> Insn { self.comp = comp; self.len = 2; self }
    pub fn is_illegal(&self) -> bool { self.op == Op::Illegal }
    /// The third source register of the fused multiply-adds (R4 format, always 32-bit).
    #[inline(always)]
    pub fn rs3(&self) -> u8 { (self.raw >> 27) as u8 }
}

const ILL2: Insn = Insn { op: Op::Illegal, rd: 0, rs1: 0, rs2: 0, imm: 0, len: 2, comp: Comp::None, raw: 0 };

#[inline(always)]
fn sext(v: u32, bits: u32) -> i32 { ((v << (32 - bits)) as i32) >> (32 - bits) }

/// Not `misa` bits (26-29 are reserved zero there): extensions `misa` cannot report, OR-ed into
/// the ISA word `decode_for` takes (`Cpu::isa`). Zba+Zbb+Zbs, and Zcb+Zcmp (the ESP32-S31).
pub const ISA_ZB: u32 = 1 << 26;
pub const ISA_ZC: u32 = 1 << 27;
const MISA_M: u32 = 1 << 12;

/// Decode the instruction at `pc` for a core without F (the C3 and C6). `bytes` is up to 4 bytes
/// little-endian; only the first two are read for a compressed instruction.
pub fn decode(pc: u32, bytes: [u8; 4]) -> Insn { decode_for(0, pc, bytes) }

/// Decode for a core whose ISA word is `isa` (`misa`, plus `ISA_ZB`/`ISA_ZC`): the F encodings,
/// compressed ones included, decode only when it has F and are illegal otherwise, so a core
/// without F decodes exactly as `decode`; likewise Zb* and Zc*. Those are reached only from
/// encodings that are otherwise illegal, so a core without them pays nothing.
#[inline]
pub fn decode_for(isa: u32, pc: u32, bytes: [u8; 4]) -> Insn {
    let raw = u32::from_le_bytes(bytes);
    let f = isa & crate::state::MISA_F != 0;
    if raw & 3 != 3 { return decode_compressed(pc, raw & 0xffff, f, isa); }
    decode32(pc, raw, f, isa & ISA_ZB != 0)
}

// Both decoders inline into `decode_for`, so the step decodes without a call and an Insn
// returned through memory.
#[inline(always)]
fn decode32(pc: u32, w: u32, f: bool, zb: bool) -> Insn {
    let (rd, rs1, rs2) = (((w >> 7) & 0x1f) as u8, ((w >> 15) & 0x1f) as u8, ((w >> 20) & 0x1f) as u8);
    let f3 = (w >> 12) & 7;
    let f7 = w >> 25;
    let i_imm = (w as i32) >> 20;
    let s_imm = (((w & 0xfe00_0000) as i32) >> 20) | ((w >> 7) & 0x1f) as i32;
    let b_off = sext(((w >> 31) << 12) | (((w >> 7) & 1) << 11) | (((w >> 25) & 0x3f) << 5) | (((w >> 8) & 0xf) << 1), 13);
    let j_off = sext(((w >> 31) << 20) | (((w >> 12) & 0xff) << 12) | (((w >> 20) & 1) << 11) | (((w >> 21) & 0x3ff) << 1), 21);
    let mk = |op: Op, imm: i32| Insn::new(op, rd, rs1, rs2, imm, 4, w);

    match w & 0x7f {
        0x37 => mk(Op::Lui, (w & 0xffff_f000) as i32),
        0x17 => mk(Op::Auipc, (w & 0xffff_f000) as i32),
        0x6f => mk(Op::Jal, pc.wrapping_add(j_off as u32) as i32),
        0x67 if f3 == 0 => mk(Op::Jalr, i_imm),
        0x63 => {
            let op = match f3 { 0 => Op::Beq, 1 => Op::Bne, 4 => Op::Blt, 5 => Op::Bge, 6 => Op::Bltu, 7 => Op::Bgeu, _ => Op::Illegal };
            mk(op, pc.wrapping_add(b_off as u32) as i32)
        }
        0x03 => {
            let op = match f3 { 0 => Op::Lb, 1 => Op::Lh, 2 => Op::Lw, 4 => Op::Lbu, 5 => Op::Lhu, _ => Op::Illegal };
            mk(op, i_imm)
        }
        0x23 => {
            let op = match f3 { 0 => Op::Sb, 1 => Op::Sh, 2 => Op::Sw, _ => Op::Illegal };
            mk(op, s_imm)
        }
        0x13 => match f3 {
            0 => mk(Op::Addi, i_imm),
            2 => mk(Op::Slti, i_imm),
            3 => mk(Op::Sltiu, i_imm),
            4 => mk(Op::Xori, i_imm),
            6 => mk(Op::Ori, i_imm),
            7 => mk(Op::Andi, i_imm),
            1 if f7 == 0 => mk(Op::Slli, (rs2 & 0x1f) as i32),
            5 if f7 == 0 => mk(Op::Srli, (rs2 & 0x1f) as i32),
            5 if f7 == 0x20 => mk(Op::Srai, (rs2 & 0x1f) as i32),
            _ if zb => decode_zb(w),
            _ => mk(Op::Illegal, 0),
        },
        0x33 => {
            let op = match (f7, f3) {
                (0x00, 0) => Op::Add, (0x20, 0) => Op::Sub,
                (0x00, 1) => Op::Sll, (0x00, 2) => Op::Slt, (0x00, 3) => Op::Sltu,
                (0x00, 4) => Op::Xor, (0x00, 5) => Op::Srl, (0x20, 5) => Op::Sra,
                (0x00, 6) => Op::Or, (0x00, 7) => Op::And,
                (0x01, 0) => Op::Mul, (0x01, 1) => Op::Mulh, (0x01, 2) => Op::Mulhsu, (0x01, 3) => Op::Mulhu,
                (0x01, 4) => Op::Div, (0x01, 5) => Op::Divu, (0x01, 6) => Op::Rem, (0x01, 7) => Op::Remu,
                _ if zb => return decode_zb(w),
                _ => Op::Illegal,
            };
            mk(op, 0)
        }
        0x0f => match f3 { 0 => mk(Op::Fence, i_imm), 1 => mk(Op::FenceI, 0), _ => mk(Op::Illegal, 0) },
        0x2f if f3 == 2 => {
            let op = match w >> 27 {
                0x02 if rs2 == 0 => Op::LrW, 0x03 => Op::ScW, 0x01 => Op::AmoSwapW, 0x00 => Op::AmoAddW,
                0x04 => Op::AmoXorW, 0x0c => Op::AmoAndW, 0x08 => Op::AmoOrW, 0x10 => Op::AmoMinW,
                0x14 => Op::AmoMaxW, 0x18 => Op::AmoMinuW, 0x1c => Op::AmoMaxuW, _ => Op::Illegal,
            };
            mk(op, ((w >> 25) & 3) as i32)                 // bit 1 = aq, bit 0 = rl
        }
        0x73 => match f3 {
            0 => match w >> 20 {
                0x000 if rd == 0 && rs1 == 0 => mk(Op::Ecall, 0),
                0x001 if rd == 0 && rs1 == 0 => mk(Op::Ebreak, 0),
                0x302 if rd == 0 && rs1 == 0 => mk(Op::Mret, 0),
                0x105 if rd == 0 && rs1 == 0 => mk(Op::Wfi, 0),
                _ => mk(Op::Illegal, 0),
            },
            1 => mk(Op::Csrrw, (w >> 20) as i32), 2 => mk(Op::Csrrs, (w >> 20) as i32), 3 => mk(Op::Csrrc, (w >> 20) as i32),
            5 => mk(Op::Csrrwi, (w >> 20) as i32), 6 => mk(Op::Csrrsi, (w >> 20) as i32), 7 => mk(Op::Csrrci, (w >> 20) as i32),
            _ => mk(Op::Illegal, 0),
        },
        0x07 | 0x27 | 0x43 | 0x47 | 0x4b | 0x4f | 0x53 if f => decode_f(w),
        _ => mk(Op::Illegal, 0),
    }
}

/// Zba/Zbb/Zbs in OP and OP-IMM (RISC-V bit-manipulation v1.0.0, RV32 encodings): only what
/// the base decoder found illegal comes here.
#[inline(never)]
fn decode_zb(w: u32) -> Insn {
    use Op::*;
    let (rd, rs1, rs2) = (((w >> 7) & 0x1f) as u8, ((w >> 15) & 0x1f) as u8, ((w >> 20) & 0x1f) as u8);
    let (f3, f7) = ((w >> 12) & 7, w >> 25);
    let mk = |op: Op, imm: i32| Insn::new(op, rd, rs1, rs2, imm, 4, w);
    let sh = rs2 as i32;
    let op = if w & 0x7f == 0x13 {
        match (f3, f7) {
            (1, 0x30) => return match rs2 { 0 => mk(Clz, 0), 1 => mk(Ctz, 0), 2 => mk(Cpop, 0), 4 => mk(SextB, 0), 5 => mk(SextH, 0), _ => mk(Illegal, 0) },
            (1, 0x24) => return mk(Bclri, sh), (1, 0x34) => return mk(Binvi, sh), (1, 0x14) => return mk(Bseti, sh),
            (5, 0x24) => return mk(Bexti, sh), (5, 0x30) => return mk(Rori, sh),
            (5, 0x14) if rs2 == 7 => OrcB, (5, 0x34) if rs2 == 0x18 => Rev8,
            _ => Illegal,
        }
    } else {
        match (f7, f3) {
            (0x10, 2) => Sh1add, (0x10, 4) => Sh2add, (0x10, 6) => Sh3add,
            (0x20, 7) => Andn, (0x20, 6) => Orn, (0x20, 4) => Xnor,
            (0x05, 4) => Min, (0x05, 5) => Minu, (0x05, 6) => Max, (0x05, 7) => Maxu,
            (0x30, 1) => Rol, (0x30, 5) => Ror, (0x04, 4) if rs2 == 0 => ZextH,
            (0x24, 1) => Bclr, (0x24, 5) => Bext, (0x34, 1) => Binv, (0x14, 1) => Bset,
            _ => Illegal,
        }
    };
    mk(op, 0)
}

/// Zcb (quadrant 0 funct3 100, quadrant 1 funct6 100111) and Zcmp (quadrant 2 funct3 101, the
/// C.FSDSP slot without D) per the Zc v1.0.0 specification. Reserved encodings are illegal;
/// c.mul needs M, and c.sext.b/c.sext.h/c.zext.h need Zbb.
#[inline(never)]
fn decode_zc(c: u32, isa: u32) -> Insn {
    use Op::*;
    let rdp = (((c >> 2) & 7) + 8) as u8;
    let rs1p = (((c >> 7) & 7) + 8) as u8;
    let mk = |op: Op, rd: u8, rs1: u8, rs2: u8, imm: i32, comp: Comp| Insn::new(op, rd, rs1, rs2, imm, 2, c).c(comp);
    let ill = Insn { raw: c, ..ILL2 };
    let zb = isa & ISA_ZB != 0;
    match (c & 3, (c >> 13) & 7) {
        (0, 4) => {
            let (b0, b1) = (((c >> 6) & 1) as i32, ((c >> 5) & 1) as i32);
            match ((c >> 10) & 7, b0) {
                (0, _) => mk(Lbu, rdp, rs1p, 0, b1 << 1 | b0, Comp::Lbu),
                (1, 0) => mk(Lhu, rdp, rs1p, 0, b1 << 1, Comp::Lhu),
                (1, _) => mk(Lh, rdp, rs1p, 0, b1 << 1, Comp::Lh),
                (2, _) => mk(Sb, 0, rs1p, rdp, b1 << 1 | b0, Comp::Sb),
                (3, 0) => mk(Sh, 0, rs1p, rdp, b1 << 1, Comp::Sh),
                _ => ill,
            }
        }
        (1, 4) if (c >> 10) & 7 == 7 => match ((c >> 5) & 3, (c >> 2) & 7) {
            (2, _) if isa & MISA_M != 0 => mk(Mul, rs1p, rs1p, rdp, 0, Comp::Mul),
            (3, 0) => mk(Andi, rs1p, rs1p, 0, 255, Comp::ZextB),
            (3, 1) if zb => mk(SextB, rs1p, rs1p, 0, 0, Comp::SextB),
            (3, 2) if zb => mk(ZextH, rs1p, rs1p, 0, 0, Comp::ZextH),
            (3, 3) if zb => mk(SextH, rs1p, rs1p, 0, 0, Comp::SextH),
            (3, 5) => mk(Xori, rs1p, rs1p, 0, -1, Comp::Not),
            _ => ill,
        },
        (2, 5) => {
            let sreg = |n: u32| (if n < 2 { 8 + n } else { 16 + n }) as u8;
            let rlist = (c >> 4) & 0xf;
            match (c >> 8) & 0x1f {
                0x18 | 0x1a | 0x1c | 0x1e if rlist >= 4 => {
                    let op = match (c >> 9) & 3 { 0 => CmPush, 1 => CmPop, 2 => CmPopretz, _ => CmPopret };
                    let adj = stack_adj_base(rlist) + ((c >> 2) & 3) * 16;
                    mk(op, 0, 0, rlist as u8, adj as i32, Comp::None)
                }
                _ if (c >> 10) & 7 == 3 && (c >> 5) & 1 == 1 => {
                    let (r1, r2) = ((c >> 7) & 7, (c >> 2) & 7);
                    let op = if (c >> 6) & 1 == 0 { CmMvsa01 } else { CmMva01s };
                    if r1 == r2 && op == CmMvsa01 { return ill; }   // two writes to one register: reserved
                    mk(op, 0, sreg(r1), sreg(r2), 0, Comp::None)
                }
                _ => ill,
            }
        }
        _ => ill,
    }
}

/// How many registers rlist 4-15 names: ra, then s0.. (rlist 15 adds s10 and s11 together).
#[inline(always)]
pub fn rlist_regs(rlist: u32) -> u32 { if rlist == 15 { 13 } else { rlist - 3 } }
/// Zc v1.0.0 "stack_adj_base": the saved registers rounded up to 16 bytes (RV32).
#[inline(always)]
fn stack_adj_base(rlist: u32) -> u32 { (rlist_regs(rlist) * 4 + 15) & !15 }
/// The `n`th register (0 = ra) an rlist names: ra, s0, s1, s2-s11.
#[inline(always)]
pub fn rlist_reg(n: u32) -> u8 { match n { 0 => 1, 1 | 2 => (7 + n) as u8, _ => (15 + n) as u8 } }

/// FLW/FSW, the fused multiply-adds and OP-FP. Without D only fmt = S (00) exists, and the
/// encodings objdump does not know (rs2 or funct3 out of range) are illegal. Reserved rm values
/// still decode: whether rm is valid depends on `frm` too, so exec checks it.
#[inline(never)]
fn decode_f(w: u32) -> Insn {
    use Op::*;
    let (rd, rs1, rs2) = (((w >> 7) & 0x1f) as u8, ((w >> 15) & 0x1f) as u8, ((w >> 20) & 0x1f) as u8);
    let rm = (w >> 12) & 7;
    let mk = |op: Op, imm: i32| Insn::new(op, rd, rs1, rs2, imm, 4, w);
    match w & 0x7f {
        0x07 if rm == 2 => mk(Flw, (w as i32) >> 20),
        0x27 if rm == 2 => mk(Fsw, (((w & 0xfe00_0000) as i32) >> 20) | ((w >> 7) & 0x1f) as i32),
        0x43 | 0x47 | 0x4b | 0x4f if (w >> 25) & 3 == 0 => {
            let op = match w & 0x7f { 0x43 => FmaddS, 0x47 => FmsubS, 0x4b => FnmsubS, _ => FnmaddS };
            mk(op, rm as i32)
        }
        0x53 => {
            let op = match (w >> 25, rs2, rm) {
                (0x00, _, _) => FaddS, (0x04, _, _) => FsubS, (0x08, _, _) => FmulS, (0x0c, _, _) => FdivS,
                (0x2c, 0, _) => FsqrtS,
                (0x10, _, 0) => FsgnjS, (0x10, _, 1) => FsgnjnS, (0x10, _, 2) => FsgnjxS,
                (0x14, _, 0) => FminS, (0x14, _, 1) => FmaxS,
                (0x60, 0, _) => FcvtWS, (0x60, 1, _) => FcvtWuS,
                (0x68, 0, _) => FcvtSW, (0x68, 1, _) => FcvtSWu,
                (0x70, 0, 0) => FmvXW, (0x70, 0, 1) => FclassS,
                (0x50, _, 2) => FeqS, (0x50, _, 1) => FltS, (0x50, _, 0) => FleS,
                (0x78, 0, 0) => FmvWX,
                _ => return mk(Illegal, 0),
            };
            mk(op, rm as i32)
        }
        _ => mk(Illegal, 0),
    }
}

#[inline(always)]
fn decode_compressed(pc: u32, c: u32, f: bool, isa: u32) -> Insn {
    let f3 = (c >> 13) & 7;
    let rd = ((c >> 7) & 0x1f) as u8;             // also rs1 in the CI/CR formats
    let rs2 = ((c >> 2) & 0x1f) as u8;
    let rdp = (((c >> 2) & 7) + 8) as u8;         // rd'/rs2'
    let rs1p = (((c >> 7) & 7) + 8) as u8;        // rs1'
    let imm6 = sext((((c >> 12) & 1) << 5) | ((c >> 2) & 0x1f), 6);
    // CL/CS word offset and the two stack-pointer forms
    let lw_off = ((((c >> 5) & 1) << 6) | (((c >> 10) & 7) << 3) | (((c >> 6) & 1) << 2)) as i32;
    let cj_off = sext((((c >> 12) & 1) << 11) | (((c >> 11) & 1) << 4) | (((c >> 9) & 3) << 8) | (((c >> 8) & 1) << 10)
                      | (((c >> 7) & 1) << 6) | (((c >> 6) & 1) << 7) | (((c >> 3) & 7) << 1) | (((c >> 2) & 1) << 5), 12);
    let cb_off = sext((((c >> 12) & 1) << 8) | (((c >> 10) & 3) << 3) | (((c >> 5) & 3) << 6)
                      | (((c >> 3) & 3) << 1) | (((c >> 2) & 1) << 5), 9);
    let mk = |op: Op, rd: u8, rs1: u8, rs2: u8, imm: i32, comp: Comp| Insn::new(op, rd, rs1, rs2, imm, 2, c).c(comp);

    match (c & 3, f3) {
        // ---- quadrant 0
        (0, 0) => {
            let imm = ((((c >> 11) & 3) << 4) | (((c >> 7) & 0xf) << 6) | (((c >> 6) & 1) << 2) | (((c >> 5) & 1) << 3)) as i32;
            if imm == 0 { return Insn { raw: c, ..ILL2 }; }                       // canonical illegal / reserved
            mk(Op::Addi, rdp, 2, 0, imm, Comp::Addi4spn)
        }
        (0, 2) => mk(Op::Lw, rdp, rs1p, 0, lw_off, Comp::Lw),
        (0, 3) if f => mk(Op::Flw, rdp, rs1p, 0, lw_off, Comp::Flw),
        (0, 4) | (2, 5) if isa & ISA_ZC != 0 => decode_zc(c, isa),
        (0, 6) => mk(Op::Sw, 0, rs1p, rdp, lw_off, Comp::Sw),
        (0, 7) if f => mk(Op::Fsw, 0, rs1p, rdp, lw_off, Comp::Fsw),
        // ---- quadrant 1
        (1, 0) if rd == 0 => mk(Op::Addi, 0, 0, 0, imm6, Comp::Nop),
        (1, 0) => mk(Op::Addi, rd, rd, 0, imm6, Comp::Addi),
        (1, 1) => mk(Op::Jal, 1, 0, 0, pc.wrapping_add(cj_off as u32) as i32, Comp::Jal),
        (1, 2) => mk(Op::Addi, rd, 0, 0, imm6, Comp::Li),
        (1, 3) if rd == 2 => {
            let imm = sext((((c >> 12) & 1) << 9) | (((c >> 6) & 1) << 4) | (((c >> 5) & 1) << 6)
                           | (((c >> 3) & 3) << 7) | (((c >> 2) & 1) << 5), 10);
            if imm == 0 { return Insn { raw: c, ..ILL2 }; }
            mk(Op::Addi, 2, 2, 0, imm, Comp::Addi16sp)
        }
        (1, 3) if rd != 0 => {
            let imm = sext((((c >> 12) & 1) << 17) | (((c >> 2) & 0x1f) << 12), 18);
            if imm == 0 { return Insn { raw: c, ..ILL2 }; }
            mk(Op::Lui, rd, 0, 0, imm, Comp::Lui)
        }
        (1, 4) => {
            let shamt = ((((c >> 12) & 1) << 5) | ((c >> 2) & 0x1f)) as i32;
            match (c >> 10) & 3 {
                0 if shamt < 32 => mk(Op::Srli, rs1p, rs1p, 0, shamt, Comp::Srli),
                1 if shamt < 32 => mk(Op::Srai, rs1p, rs1p, 0, shamt, Comp::Srai),
                2 => mk(Op::Andi, rs1p, rs1p, 0, imm6, Comp::Andi),
                3 if (c >> 12) & 1 == 0 => {
                    let op = match (c >> 5) & 3 { 0 => Op::Sub, 1 => Op::Xor, 2 => Op::Or, _ => Op::And };
                    let comp = match (c >> 5) & 3 { 0 => Comp::Sub, 1 => Comp::Xor, 2 => Comp::Or, _ => Comp::And };
                    mk(op, rs1p, rs1p, rdp, 0, comp)
                }
                3 if isa & ISA_ZC != 0 => decode_zc(c, isa),
                _ => Insn { raw: c, ..ILL2 }, // RV64-only arithmetic or unsupported custom shifts.
            }
        }
        (1, 5) => mk(Op::Jal, 0, 0, 0, pc.wrapping_add(cj_off as u32) as i32, Comp::J),
        (1, 6) => mk(Op::Beq, 0, rs1p, 0, pc.wrapping_add(cb_off as u32) as i32, Comp::Beqz),
        (1, 7) => mk(Op::Bne, 0, rs1p, 0, pc.wrapping_add(cb_off as u32) as i32, Comp::Bnez),
        // ---- quadrant 2
        // RV32C shamt[5] = 1 belongs to custom extensions, not the standard shifts.
        (2, 0) if c & (1 << 12) == 0 => mk(Op::Slli, rd, rd, 0, rs2 as i32, Comp::Slli),
        (2, 2) if rd != 0 => {
            let imm = ((((c >> 12) & 1) << 5) | (((c >> 4) & 7) << 2) | (((c >> 2) & 3) << 6)) as i32;
            mk(Op::Lw, rd, 2, 0, imm, Comp::Lwsp)
        }
        (2, 3) if f => mk(Op::Flw, rd, 2, 0, lwsp_off(c), Comp::Flwsp),   // f0 is a valid destination
        (2, 4) => match ((c >> 12) & 1, rd, rs2) {
            (0, 0, _) => Insn { raw: c, ..ILL2 },
            (0, _, 0) => mk(Op::Jalr, 0, rd, 0, 0, Comp::Jr),
            (0, _, _) => mk(Op::Add, rd, 0, rs2, 0, Comp::Mv),
            (1, 0, 0) => mk(Op::Ebreak, 0, 0, 0, 0, Comp::Ebreak),
            (1, _, 0) => mk(Op::Jalr, 1, rd, 0, 0, Comp::Jalr),
            (_, _, _) => mk(Op::Add, rd, rd, rs2, 0, Comp::Add),
        },
        (2, 6) => {
            let imm = ((((c >> 9) & 0xf) << 2) | (((c >> 7) & 3) << 6)) as i32;
            mk(Op::Sw, 0, 2, rs2, imm, Comp::Swsp)
        }
        (2, 7) if f => mk(Op::Fsw, 0, 2, rs2, swsp_off(c), Comp::Fswsp),
        _ => Insn { raw: c, ..ILL2 }, // Includes the D forms, and the F forms on a core without F.
    }
}

/// C.FLWSP's offset, laid out as C.LWSP's; C.FSWSP's as C.SWSP's.
#[inline(always)]
fn lwsp_off(c: u32) -> i32 { ((((c >> 12) & 1) << 5) | (((c >> 4) & 7) << 2) | (((c >> 2) & 3) << 6)) as i32 }
#[inline(always)]
fn swsp_off(c: u32) -> i32 { ((((c >> 9) & 0xf) << 2) | (((c >> 7) & 3) << 6)) as i32 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_compressed_encoding_preserves_raw_bits() {
        for f in [false, true] {
            for raw in 0..=u16::MAX as u32 {
                if raw & 3 == 3 { continue; }
                let insn = decode_compressed(0, raw, f, if f { crate::state::MISA_F } else { 0 });
                assert_eq!(insn.raw, raw, "encoding {raw:04x}");
                assert_eq!(insn.len, 2);
            }
        }
    }

    /// F changes exactly the four RV32FC slots (C.FLW, C.FSW, C.FLWSP, C.FSWSP); without it they stay
    /// illegal and every other compressed encoding decodes the same.
    #[test]
    fn f_adds_only_the_compressed_fp_loads_and_stores() {
        for raw in 0..=u16::MAX as u32 {
            if raw & 3 == 3 { continue; }
            let (plain, with_f) = (decode_compressed(0, raw, false, 0), decode_compressed(0, raw, true, crate::state::MISA_F));
            let slot = (raw & 3, (raw >> 13) & 7);
            if matches!(slot, (0, 3) | (0, 7) | (2, 3) | (2, 7)) {
                assert_eq!(plain.op, Op::Illegal, "{raw:04x}");
                assert!(matches!(with_f.op, Op::Flw | Op::Fsw), "{raw:04x}");
            } else {
                assert_eq!(plain, with_f, "{raw:04x}");
            }
        }
    }

    /// Without F every F major opcode is illegal; with it, the base opcodes decode the same.
    #[test]
    fn f_opcodes_are_illegal_without_f() {
        let mut seed = 0x1234_5678u32;
        for _ in 0..200_000 {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
            let w = seed | 3;
            let (plain, with_f) = (decode32(0x4000_0000, w, false, false), decode32(0x4000_0000, w, true, false));
            if matches!(w & 0x7f, 0x07 | 0x27 | 0x43 | 0x47 | 0x4b | 0x4f | 0x53) {
                assert_eq!(plain.op, Op::Illegal, "{w:08x}");
                assert!(with_f.op == Op::Illegal || with_f.op.is_f(), "{w:08x}");
            } else {
                assert_eq!(plain, with_f, "{w:08x}");
                assert!(!plain.op.is_f(), "{w:08x}");
            }
        }
    }

    /// Zcb and Zcmp take only quadrant 0 funct3 100, quadrant 1 funct6 100111 and quadrant 2
    /// funct3 101; every other compressed encoding decodes the same, and every one keeps its bits.
    #[test]
    fn zc_adds_only_its_three_slots() {
        let s31 = crate::state::MISA_RV32IMAFC | ISA_ZB | ISA_ZC;
        for raw in 0..=u16::MAX as u32 {
            if raw & 3 == 3 { continue; }
            let (plain, zc) = (decode_compressed(0, raw, true, crate::state::MISA_RV32IMAFC), decode_compressed(0, raw, true, s31));
            assert_eq!((zc.raw, zc.len), (raw, 2), "{raw:04x}");
            let slot = matches!((raw & 3, (raw >> 13) & 7), (0, 4) | (2, 5)) || (raw & 3 == 1 && raw >> 10 == 0b100111);
            if slot { assert_eq!(plain.op, Op::Illegal, "{raw:04x}"); } else { assert_eq!(plain, zc, "{raw:04x}"); }
        }
    }

    #[test]
    fn insn_stays_sixteen_bytes() {
        assert_eq!(std::mem::size_of::<Insn>(), 16);
    }

    #[test]
    fn rv32_compressed_shifts_reject_custom_high_shamt_bit() {
        // RISC-V C v2.0: shamt[5] = 1 is custom space for all three RV32C shifts.
        // https://docs.riscv.org/reference/isa/unpriv/c-st-ext.html
        for (base, op) in [(0x0082, Op::Slli), (0x8001, Op::Srli), (0x8401, Op::Srai)] {
            for shamt in 0..64 {
                let raw = base | (shamt & 31) << 2 | (shamt >> 5) << 12;
                let insn = decode_compressed(0, raw, false, 0);
                assert_eq!(insn.raw, raw);
                assert_eq!(insn.len, 2);
                assert_eq!(insn.op, if shamt < 32 { op } else { Op::Illegal }, "{raw:04x}");
                if shamt < 32 { assert_eq!(insn.imm, shamt as i32); }
            }
        }
    }
}
