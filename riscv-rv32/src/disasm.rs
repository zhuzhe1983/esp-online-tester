//! objdump-compatible formatting, for the differential decoder test.

use crate::decode::{Comp, Insn, Op};

pub const XREG: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4", "a5",
    "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6",
];

pub const FREG: [&str; 32] = [
    "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "fs0", "fs1", "fa0", "fa1", "fa2", "fa3", "fa4", "fa5",
    "fa6", "fa7", "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9", "fs10", "fs11", "ft8", "ft9", "ft10", "ft11",
];

fn r(n: u8) -> &'static str { XREG[(n & 31) as usize] }
fn fr(n: u8) -> &'static str { FREG[(n & 31) as usize] }

/// objdump appends the rounding mode as an operand unless it is `dyn`.
fn rm(i: &Insn) -> &'static str {
    match i.imm { 0 => ",rne", 1 => ",rtz", 2 => ",rdn", 3 => ",rup", 4 => ",rmm", 7 => "", _ => ",unknown" }
}

/// GNU objdump prints a bare CSR name when it knows one, else the number in hex.
fn csr_name(n: u32) -> String {
    let s = match n {
        0x000 => "ustatus", 0x004 => "uie", 0x005 => "utvec", 0x040 => "uscratch", 0x041 => "uepc", 0x042 => "ucause", 0x043 => "utval", 0x044 => "uip",
        0x001 => "fflags", 0x002 => "frm", 0x003 => "fcsr",
        0x302 => "medeleg", 0x303 => "mideleg", 0x306 => "mcounteren", 0x320 => "mcountinhibit",
        0x300 => "mstatus", 0x301 => "misa", 0x304 => "mie", 0x305 => "mtvec",
        0x340 => "mscratch", 0x341 => "mepc", 0x342 => "mcause", 0x343 => "mtval", 0x344 => "mip",
        0x7a0 => "tselect", 0x7a1 => "tdata1", 0x7a2 => "tdata2", 0x7a3 => "tdata3", 0x7a5 => "tcontrol",
        0xb00 => "mcycle", 0xb02 => "minstret", 0xb80 => "mcycleh", 0xb82 => "minstreth",
        0xc00 => "cycle", 0xc01 => "time", 0xc02 => "instret", 0xc80 => "cycleh", 0xc81 => "timeh", 0xc82 => "instreth",
        0xf11 => "mvendorid", 0xf12 => "marchid", 0xf13 => "mimpid", 0xf14 => "mhartid",
        0x3a0..=0x3a3 => return format!("pmpcfg{}", n - 0x3a0),
        0x3b0..=0x3bf => return format!("pmpaddr{}", n - 0x3b0),
        _ => return format!("{:#x}", n),
    };
    s.to_string()
}

/// Xesploop as objdump prints it: the loop, then the operands, offsets in decimal and relative to
/// the instruction (objdump's `# target` comment is not part of them).
fn format_hwlp(i: &Insn) -> String {
    use crate::decode::hwlp_count_imm;
    use Op::*;
    let (w, l) = (i.raw, i.rd);
    let off12 = (w >> 20) << 1;
    match i.op {
        EspLpSetup => format!("esp.lp.setup\t{},{},{}", l, r(i.rs1), off12),
        EspLpSetupi => format!("esp.lp.setupi\t{},{},{}", l, hwlp_count_imm(w), ((w >> 15) & 0x1f) << 1 | ((w >> 8) & 0xf) << 6),
        EspLpStarti => format!("esp.lp.starti\t{},{}", l, off12),
        EspLpEndi => format!("esp.lp.endi\t{},{}", l, off12),
        EspLpCount => format!("esp.lp.count\t{},{}", l, r(i.rs1)),
        EspLpCounti => format!("esp.lp.counti\t{},{}", l, i.imm),
        _ => unreachable!(),
    }
}

/// Xespv2p2 as objdump prints it: operands in the syntax's order, and the `sat`/`dyn` defaults of
/// the saturating and rounding forms left out (`trunc` and a named rounding mode printed).
fn format_pie(i: &Insn) -> String {
    use crate::decode::{pie_q, pie_qw, pie_rm, pie_sat};
    use Op::*;
    let w = i.raw;
    let q = |lo: u32| format!("q{}", pie_q(w, lo));
    let qw = format!("q{}", pie_qw(w));
    let sat = if pie_sat(i.op, w) { "" } else { ",trunc" };
    let rm = || match pie_rm(i.op, w) { 0 => ",rdn", 1 => ",rup", 2 => ",raz", 3 => ",rtz", 4 => ",rhaz", 5 => ",rhtz", 6 => ",rne", _ => "" };
    let (rs1, rd) = (r(i.rs1), r(i.rd));
    match i.op {
        EspVmulU16 => format!("esp.vmul.u16\t{},{},{}{}{}", q(7), q(29), q(26), sat, rm()),
        EspVmulU16LdIncp => format!("esp.vmul.u16.ld.incp\t{},{},{},{},{}{}{}", q(10), rs1, q(7), q(29), q(26), sat, rm()),
        EspVaddU16 | EspVsubU16 | EspVsubU8 => {
            let n = match i.op { EspVaddU16 => "esp.vadd.u16", EspVsubU16 => "esp.vsub.u16", _ => "esp.vsub.u8" };
            format!("{}\t{},{},{}{}", n, q(20), q(29), q(26), sat)
        }
        EspVaddU16LdIncp => format!("esp.vadd.u16.ld.incp\t{},{},{},{},{}{}", q(10), rs1, q(20), q(29), q(26), sat),
        EspVminU16 | EspVcmpEqU16 | EspAndq | EspOrq | EspXorq => {
            let n = match i.op { EspVminU16 => "esp.vmin.u16", EspVcmpEqU16 => "esp.vcmp.eq.u16", EspAndq => "esp.andq", EspOrq => "esp.orq", _ => "esp.xorq" };
            format!("{}\t{},{},{}", n, q(7), q(29), q(26))
        }
        EspZeroQ => format!("esp.zero.q\t{}", q(7)),
        EspVextU8 => format!("esp.vext.u8\t{},{},{}", q(7), q(20), qw),
        EspVunzip8 => format!("esp.vunzip.8\t{},{}", q(29), q(26)),
        EspVzipt8 => format!("esp.vzipt.8\t{},{},{}", q(29), q(26), qw),
        EspVunzipt8 => format!("esp.vunzipt.8\t{},{},{}", q(29), q(26), qw),
        EspSrcQ => format!("esp.src.q\t{},{},{}", q(7), qw, q(26)),
        EspVld128Ip => format!("esp.vld.128.ip\t{},{},{}", q(10), rs1, i.imm),
        EspVst128Ip => format!("esp.vst.128.ip\t{},{},{}", q(10), rs1, i.imm),
        EspVldbc16Ip => format!("esp.vldbc.16.ip\t{},{},{}", q(10), rs1, i.imm),
        EspLd128UsarIp => format!("esp.ld.128.usar.ip\t{},{},{}", q(10), rs1, i.imm),
        EspMovxWSar => format!("esp.movx.w.sar\t{}", rs1),
        EspMovxRSar => format!("esp.movx.r.sar\t{}", rd),
        EspMovxWCfg => format!("esp.movx.w.cfg\t{}", rs1),
        EspMovxRCfg => format!("esp.movx.r.cfg\t{}", rd),
        EspMovi32A => format!("esp.movi.32.a\t{},{},{}", q(26), rd, i.imm),
        _ => unreachable!(),
    }
}

/// objdump (and GNU as) print a compressed instruction as the 32-bit instruction it expands to,
/// so that is what `format` does; `format_compressed` gives the literal `c.*` form instead.
/// This is the text for a core without F (the C3 and C6).
pub fn format(i: &Insn) -> String { format_for(0, i) }

/// As `format`, for a core whose `misa` is `misa`. With F, objdump prints the FP CSR accesses as
/// their F aliases (`frrm`, `fscsr`, ...).
pub fn format_for(misa: u32, i: &Insn) -> String {
    use Op::*;
    if misa & crate::state::MISA_F != 0 && matches!(i.op, Csrrw | Csrrs | Csrrwi) && (1..=3).contains(&i.imm) {
        let (read, write, write_imm) = match i.imm { 1 => ("frflags", "fsflags", Some("fsflagsi")), 2 => ("frrm", "fsrm", Some("fsrmi")), _ => ("frcsr", "fscsr", None) };
        match i.op {
            Csrrs if i.rs1 == 0 => return format!("{}\t{}", read, r(i.rd)),
            Csrrw if i.rd == 0 => return format!("{}\t{}", write, r(i.rs1)),
            Csrrw => return format!("{}\t{},{}", write, r(i.rd), r(i.rs1)),
            Csrrwi => if let Some(n) = write_imm { return format!("{}\t{},{}", n, r(i.rd), i.rs1); },
            _ => {}
        }
    }
    // HINT encodings have no 32-bit alias to print, so objdump keeps them in their `c.*` form
    if matches!(i.comp, Comp::Li | Comp::Slli | Comp::Mv | Comp::Add | Comp::Lui) && i.rd == 0 { return format_compressed(i); }
    if i.comp == Comp::Nop && i.imm != 0 { return format_compressed(i); }
    let (rd, rs1, rs2) = (r(i.rd), r(i.rs1), r(i.rs2));
    let m = |s: &str| s.to_string();
    match i.op {
        EspVmulU16 | EspVmulU16LdIncp | EspVaddU16 | EspVaddU16LdIncp | EspVsubU16 | EspVsubU8 | EspVminU16
        | EspVcmpEqU16 | EspAndq | EspOrq | EspXorq | EspZeroQ | EspVextU8 | EspVunzip8 | EspVzipt8 | EspVunzipt8
        | EspSrcQ | EspVld128Ip | EspVst128Ip | EspVldbc16Ip | EspLd128UsarIp
        | EspMovxWSar | EspMovxRSar | EspMovxWCfg | EspMovxRCfg | EspMovi32A => format_pie(i),
        EspLpSetup | EspLpSetupi | EspLpStarti | EspLpEndi | EspLpCount | EspLpCounti => format_hwlp(i),
        Illegal => m("unimp"),
        Lui => format!("lui\t{},{:#x}", rd, (i.imm as u32) >> 12),
        Auipc => format!("auipc\t{},{:#x}", rd, (i.imm as u32) >> 12),
        Jal => if i.rd == 0 { format!("j\t{:x}", i.imm as u32) } else if i.rd == 1 { format!("jal\t{:x}", i.imm as u32) } else { format!("jal\t{},{:x}", rd, i.imm as u32) },
        Jalr => match (i.rd, i.imm) {
            (0, 0) if i.rs1 == 1 => m("ret"),
            (0, 0) => format!("jr\t{}", rs1),
            (1, 0) => format!("jalr\t{}", rs1),
            (1, _) => format!("jalr\t{}({})", i.imm, rs1),
            (0, _) => format!("jr\t{}({})", i.imm, rs1),
            _ => format!("jalr\t{},{}({})", rd, i.imm, rs1),
        },
        Beq | Bne | Blt | Bge | Bltu | Bgeu => {
            let n = match i.op { Beq => "beq", Bne => "bne", Blt => "blt", Bge => "bge", Bltu => "bltu", _ => "bgeu" };
            if i.rs2 == 0 {
                let z = match i.op { Beq => Some("beqz"), Bne => Some("bnez"), Blt => Some("bltz"), Bge => Some("bgez"), _ => None };
                if let Some(z) = z { return format!("{}\t{},{:x}", z, rs1, i.imm as u32); }
            }
            if i.rs1 == 0 && matches!(i.op, Blt | Bge) {
                return format!("{}\t{},{:x}", if i.op == Blt { "bgtz" } else { "blez" }, rs2, i.imm as u32);
            }
            format!("{}\t{},{},{:x}", n, rs1, rs2, i.imm as u32)
        }
        Lb | Lh | Lw | Lbu | Lhu => {
            let n = match i.op { Lb => "lb", Lh => "lh", Lw => "lw", Lbu => "lbu", _ => "lhu" };
            format!("{}\t{},{}({})", n, rd, i.imm, rs1)
        }
        Sb | Sh | Sw => {
            let n = match i.op { Sb => "sb", Sh => "sh", _ => "sw" };
            format!("{}\t{},{}({})", n, rs2, i.imm, rs1)
        }
        Addi => match (i.rd, i.rs1, i.imm) {
            (0, 0, 0) => m("nop"),
            (_, 0, _) => format!("li\t{},{}", rd, i.imm),
            (_, _, 0) => format!("mv\t{},{}", rd, rs1),
            _ => format!("addi\t{},{},{}", rd, rs1, i.imm),
        },
        Slti => format!("slti\t{},{},{}", rd, rs1, i.imm),
        Sltiu => if i.imm == 1 { format!("seqz\t{},{}", rd, rs1) } else { format!("sltiu\t{},{},{}", rd, rs1, i.imm) },
        Xori => if i.imm == -1 { format!("not\t{},{}", rd, rs1) } else { format!("xori\t{},{},{}", rd, rs1, i.imm) },
        Ori => format!("ori\t{},{},{}", rd, rs1, i.imm),
        Andi => if i.imm == 255 { format!("zext.b\t{},{}", rd, rs1) } else { format!("andi\t{},{},{}", rd, rs1, i.imm) },
        Slli => format!("slli\t{},{},{:#x}", rd, rs1, i.imm),
        Srli => format!("srli\t{},{},{:#x}", rd, rs1, i.imm),
        Srai => format!("srai\t{},{},{:#x}", rd, rs1, i.imm),
        Add => if i.rs1 == 0 { format!("mv\t{},{}", rd, rs2) } else { format!("add\t{},{},{}", rd, rs1, rs2) },
        Sub => if i.rs1 == 0 { format!("neg\t{},{}", rd, rs2) } else { format!("sub\t{},{},{}", rd, rs1, rs2) },
        Sltu => if i.rs1 == 0 { format!("snez\t{},{}", rd, rs2) } else { format!("sltu\t{},{},{}", rd, rs1, rs2) },
        Sll => format!("sll\t{},{},{}", rd, rs1, rs2),
        Slt if i.rs1 == 0 => format!("sgtz\t{},{}", rd, rs2),
        Slt if i.rs2 == 0 => format!("sltz\t{},{}", rd, rs1),
        Slt => format!("slt\t{},{},{}", rd, rs1, rs2),
        Xor => format!("xor\t{},{},{}", rd, rs1, rs2),
        Srl => format!("srl\t{},{},{}", rd, rs1, rs2),
        Sra => format!("sra\t{},{},{}", rd, rs1, rs2),
        Or => format!("or\t{},{},{}", rd, rs1, rs2),
        And => format!("and\t{},{},{}", rd, rs1, rs2),
        Mul => format!("mul\t{},{},{}", rd, rs1, rs2),
        Mulh => format!("mulh\t{},{},{}", rd, rs1, rs2),
        Mulhsu => format!("mulhsu\t{},{},{}", rd, rs1, rs2),
        Mulhu => format!("mulhu\t{},{},{}", rd, rs1, rs2),
        Div => format!("div\t{},{},{}", rd, rs1, rs2),
        Divu => format!("divu\t{},{},{}", rd, rs1, rs2),
        Rem => format!("rem\t{},{},{}", rd, rs1, rs2),
        Remu => format!("remu\t{},{},{}", rd, rs1, rs2),
        LrW | ScW | AmoSwapW | AmoAddW | AmoXorW | AmoAndW | AmoOrW | AmoMinW | AmoMaxW | AmoMinuW | AmoMaxuW => {
            let n = match i.op { LrW => "lr.w", ScW => "sc.w", AmoSwapW => "amoswap.w", AmoAddW => "amoadd.w", AmoXorW => "amoxor.w", AmoAndW => "amoand.w",
                                 AmoOrW => "amoor.w", AmoMinW => "amomin.w", AmoMaxW => "amomax.w", AmoMinuW => "amominu.w", _ => "amomaxu.w" };
            let sfx = match i.imm & 3 { 3 => ".aqrl", 2 => ".aq", 1 => ".rl", _ => "" };
            if i.op == LrW { format!("{}{}\t{},({})", n, sfx, rd, rs1) } else { format!("{}{}\t{},{},({})", n, sfx, rd, rs2, rs1) }
        }
        Fence => {
            let (fm, pred, succ) = ((i.imm >> 8) & 0xf, (i.imm >> 4) & 0xf, i.imm & 0xf);
            let set = |s: i32| if s == 0 { "unknown".to_string() } else { "iorw".chars().enumerate().filter(|(k, _)| s & (8 >> k) != 0).map(|(_, c)| c).collect() };
            if fm == 8 && pred == 3 && succ == 3 { m("fence.tso") } else if pred == 0xf && succ == 0xf { m("fence") } else { format!("fence\t{},{}", set(pred), set(succ)) }
        }
        FenceI => m("fence.i"),
        Ecall => m("ecall"),
        Ebreak => m("ebreak"),
        Mret => m("mret"),
        Wfi => m("wfi"),
        Csrrw | Csrrs | Csrrc | Csrrwi | Csrrsi | Csrrci => {
            let n = csr_name(i.imm as u32);
            let imm5 = i.rs1;
            match i.op {
                Csrrw if i.rd == 0 => format!("csrw\t{},{}", n, rs1),
                // GNU objdump's Zicntr aliases: rdcycle, rdtime, rdinstret and their high halves
                Csrrs if i.rs1 == 0 && matches!(i.imm, 0xc00..=0xc02 | 0xc80..=0xc82) => format!("rd{}\t{}", n, rd),
                Csrrs if i.rs1 == 0 => format!("csrr\t{},{}", rd, n),
                Csrrs if i.rd == 0 => format!("csrs\t{},{}", n, rs1),
                Csrrc if i.rd == 0 => format!("csrc\t{},{}", n, rs1),
                Csrrwi if i.rd == 0 => format!("csrwi\t{},{}", n, imm5),
                Csrrsi if i.rd == 0 => format!("csrsi\t{},{}", n, imm5),
                Csrrci if i.rd == 0 => format!("csrci\t{},{}", n, imm5),
                Csrrw => format!("csrrw\t{},{},{}", rd, n, rs1),
                Csrrs => format!("csrrs\t{},{},{}", rd, n, rs1),
                Csrrc => format!("csrrc\t{},{},{}", rd, n, rs1),
                Csrrwi => format!("csrrwi\t{},{},{}", rd, n, imm5),
                Csrrsi => format!("csrrsi\t{},{},{}", rd, n, imm5),
                _ => format!("csrrci\t{},{},{}", rd, n, imm5),
            }
        }
        Sh1add | Sh2add | Sh3add | Andn | Orn | Xnor | Max | Maxu | Min | Minu | Rol | Ror | Bclr | Bext | Binv | Bset => {
            let n = match i.op { Sh1add => "sh1add", Sh2add => "sh2add", Sh3add => "sh3add", Andn => "andn", Orn => "orn", Xnor => "xnor",
                                 Max => "max", Maxu => "maxu", Min => "min", Minu => "minu", Rol => "rol", Ror => "ror",
                                 Bclr => "bclr", Bext => "bext", Binv => "binv", _ => "bset" };
            format!("{}\t{},{},{}", n, rd, rs1, rs2)
        }
        Clz | Ctz | Cpop | SextB | SextH | ZextH | OrcB | Rev8 => {
            let n = match i.op { Clz => "clz", Ctz => "ctz", Cpop => "cpop", SextB => "sext.b", SextH => "sext.h", ZextH => "zext.h", OrcB => "orc.b", _ => "rev8" };
            format!("{}\t{},{}", n, rd, rs1)
        }
        Rori | Bclri | Bexti | Binvi | Bseti => {
            let n = match i.op { Rori => "rori", Bclri => "bclri", Bexti => "bexti", Binvi => "binvi", _ => "bseti" };
            format!("{}\t{},{},{:#x}", n, rd, rs1, i.imm)
        }
        CmPush | CmPop | CmPopret | CmPopretz => {
            let n = match i.op { CmPush => "cm.push", CmPop => "cm.pop", CmPopret => "cm.popret", _ => "cm.popretz" };
            let list = match i.rs2 { 4 => "{ra}".to_string(), 5 => "{ra,s0}".to_string(), 15 => "{ra,s0-s11}".to_string(), l => format!("{{ra,s0-s{}}}", l - 5) };
            format!("{}\t{},{}", n, list, if i.op == CmPush { -i.imm } else { i.imm })
        }
        CmMvsa01 => format!("cm.mvsa01\t{},{}", rs1, rs2),
        CmMva01s => format!("cm.mva01s\t{},{}", rs1, rs2),
        Flw => format!("flw\t{},{}({})", fr(i.rd), i.imm, rs1),
        Fsw => format!("fsw\t{},{}({})", fr(i.rs2), i.imm, rs1),
        FmaddS | FmsubS | FnmsubS | FnmaddS => {
            let n = match i.op { FmaddS => "fmadd.s", FmsubS => "fmsub.s", FnmsubS => "fnmsub.s", _ => "fnmadd.s" };
            format!("{}\t{},{},{},{}{}", n, fr(i.rd), fr(i.rs1), fr(i.rs2), fr(i.rs3()), rm(i))
        }
        FaddS | FsubS | FmulS | FdivS => {
            let n = match i.op { FaddS => "fadd.s", FsubS => "fsub.s", FmulS => "fmul.s", _ => "fdiv.s" };
            format!("{}\t{},{},{}{}", n, fr(i.rd), fr(i.rs1), fr(i.rs2), rm(i))
        }
        FsqrtS => format!("fsqrt.s\t{},{}{}", fr(i.rd), fr(i.rs1), rm(i)),
        FsgnjS | FsgnjnS | FsgnjxS => {
            let (n, alias) = match i.op { FsgnjS => ("fsgnj.s", "fmv.s"), FsgnjnS => ("fsgnjn.s", "fneg.s"), _ => ("fsgnjx.s", "fabs.s") };
            if i.rs1 == i.rs2 { format!("{}\t{},{}", alias, fr(i.rd), fr(i.rs1)) } else { format!("{}\t{},{},{}", n, fr(i.rd), fr(i.rs1), fr(i.rs2)) }
        }
        FminS => format!("fmin.s\t{},{},{}", fr(i.rd), fr(i.rs1), fr(i.rs2)),
        FmaxS => format!("fmax.s\t{},{},{}", fr(i.rd), fr(i.rs1), fr(i.rs2)),
        FcvtWS => format!("fcvt.w.s\t{},{}{}", rd, fr(i.rs1), rm(i)),
        FcvtWuS => format!("fcvt.wu.s\t{},{}{}", rd, fr(i.rs1), rm(i)),
        FcvtSW => format!("fcvt.s.w\t{},{}{}", fr(i.rd), rs1, rm(i)),
        FcvtSWu => format!("fcvt.s.wu\t{},{}{}", fr(i.rd), rs1, rm(i)),
        FmvXW => format!("fmv.x.w\t{},{}", rd, fr(i.rs1)),
        FmvWX => format!("fmv.w.x\t{},{}", fr(i.rd), rs1),
        FeqS => format!("feq.s\t{},{},{}", rd, fr(i.rs1), fr(i.rs2)),
        FltS => format!("flt.s\t{},{},{}", rd, fr(i.rs1), fr(i.rs2)),
        FleS => format!("fle.s\t{},{},{}", rd, fr(i.rs1), fr(i.rs2)),
        FclassS => format!("fclass.s\t{},{}", rd, fr(i.rs1)),
    }
}

pub fn format_compressed(i: &Insn) -> String {
    let (rd, rs1, rs2) = (r(i.rd), r(i.rs1), r(i.rs2));
    match i.comp {
        Comp::Addi4spn => format!("c.addi4spn\t{},sp,{}", rd, i.imm),
        Comp::Lw => format!("c.lw\t{},{}({})", rd, i.imm, rs1),
        Comp::Sw => format!("c.sw\t{},{}({})", rs2, i.imm, rs1),
        Comp::Nop => if i.imm != 0 { format!("c.nop\t{}", i.imm) } else { "c.nop".to_string() },
        Comp::Addi => format!("c.addi\t{},{}", rd, i.imm),
        Comp::Jal => format!("c.jal\t{:x}", i.imm as u32),
        Comp::Li => format!("c.li\t{},{}", rd, i.imm),
        Comp::Addi16sp => format!("c.addi16sp\tsp,{}", i.imm),
        Comp::Lui => format!("c.lui\t{},{:#x}", rd, ((i.imm as u32) >> 12) & 0xfffff),
        Comp::Srli => format!("c.srli\t{},{:#x}", rd, i.imm),
        Comp::Srai => format!("c.srai\t{},{:#x}", rd, i.imm),
        Comp::Andi => format!("c.andi\t{},{}", rd, i.imm),
        Comp::Sub => format!("c.sub\t{},{}", rd, rs2),
        Comp::Xor => format!("c.xor\t{},{}", rd, rs2),
        Comp::Or => format!("c.or\t{},{}", rd, rs2),
        Comp::And => format!("c.and\t{},{}", rd, rs2),
        Comp::J => format!("c.j\t{:x}", i.imm as u32),
        Comp::Beqz => format!("c.beqz\t{},{:x}", rs1, i.imm as u32),
        Comp::Bnez => format!("c.bnez\t{},{:x}", rs1, i.imm as u32),
        Comp::Slli => format!("c.slli\t{},{:#x}", rd, i.imm),
        Comp::Lwsp => format!("c.lwsp\t{},{}(sp)", rd, i.imm),
        Comp::Jr => format!("c.jr\t{}", rs1),
        Comp::Mv => format!("c.mv\t{},{}", rd, rs2),
        Comp::Ebreak => "c.ebreak".to_string(),
        Comp::Jalr => format!("c.jalr\t{}", rs1),
        Comp::Add => format!("c.add\t{},{}", rd, rs2),
        Comp::Swsp => format!("c.swsp\t{},{}(sp)", rs2, i.imm),
        Comp::Flw => format!("c.flw\t{},{}({})", fr(i.rd), i.imm, rs1),
        Comp::Fsw => format!("c.fsw\t{},{}({})", fr(i.rs2), i.imm, rs1),
        Comp::Flwsp => format!("c.flwsp\t{},{}(sp)", fr(i.rd), i.imm),
        Comp::Fswsp => format!("c.fswsp\t{},{}(sp)", fr(i.rs2), i.imm),
        Comp::Lbu => format!("c.lbu\t{},{}({})", rd, i.imm, rs1),
        Comp::Lhu => format!("c.lhu\t{},{}({})", rd, i.imm, rs1),
        Comp::Lh => format!("c.lh\t{},{}({})", rd, i.imm, rs1),
        Comp::Sb => format!("c.sb\t{},{}({})", rs2, i.imm, rs1),
        Comp::Sh => format!("c.sh\t{},{}({})", rs2, i.imm, rs1),
        Comp::ZextB => format!("c.zext.b\t{}", rd),
        Comp::SextB => format!("c.sext.b\t{}", rd),
        Comp::ZextH => format!("c.zext.h\t{}", rd),
        Comp::SextH => format!("c.sext.h\t{}", rd),
        Comp::Not => format!("c.not\t{}", rd),
        Comp::Mul => format!("c.mul\t{},{}", rd, rs2),
        Comp::None => format(i),
    }
}
