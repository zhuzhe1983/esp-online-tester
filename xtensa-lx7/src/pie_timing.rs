//! Q-register operand effects of PIE instructions for the static readiness table (EX146).
//! Operand masks from the EX058 prototype (810f38c5); only the measured loaded-Q results are
//! marked delayed (EX080: VLD, LD.USAR and the loaded Qu of SRC.Q.LD are usable at issue + 2).
use crate::pie::{Kind, LdKind, Mode, Ops, PieInsn, Role};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QEffects {
    pub reads: u8,
    pub writes: u8,
    /// Only the independently measured loaded-Q result paths belong here.
    pub delayed: u8,
}

pub fn effects(p: &PieInsn, o: &Ops) -> QEffects {
    use Role::*;
    let bits = |roles: &[Role]| roles.iter().fold(0, |mask, &r| {
        mask | if o.has(r) { 1 << (o.get(r) as u8 & 7) } else { 0 }
    });
    let mut e = QEffects::default();
    let dst = if o.has(Qz) { Qz } else { Qa };
    match p.kind {
        Kind::Vld128(_) | Kind::LdUsar(_) => { e.writes = bits(&[Qu]); e.delayed = e.writes; }
        Kind::Vld64 { .. } | Kind::MoviQ => { e.reads = bits(&[Qu]); e.writes = e.reads; }
        Kind::Ldbc { .. } | Kind::LdQr => e.writes = bits(&[Qu]),
        Kind::Ldxq32 => { e.reads = bits(&[Qs, Qu]); e.writes = bits(&[Qu]); }
        Kind::Ldhbc16 => e.writes = bits(&[Qu, Qu1]),
        Kind::Vst128(_) | Kind::Vst64 { .. } => e.reads = bits(&[Qv]),
        Kind::MoviA | Kind::MovQacc { .. } | Kind::StQr => e.reads = bits(&[Qs]),
        Kind::ZeroQ => e.writes = bits(&[Qa]),
        Kind::Andq | Kind::Orq | Kind::Xorq | Kind::Vcmp { .. } => {
            e.reads = bits(&[Qx, Qy]); e.writes = bits(&[Qa]);
        }
        Kind::Notq => { e.reads = bits(&[Qx]); e.writes = bits(&[Qa]); }
        Kind::Vsl32 | Kind::Vsr32 => { e.reads = bits(&[Qs]); e.writes = bits(&[Qa]); }
        Kind::Slcxxp | Kind::Srcxxp | Kind::Slci | Kind::Srci | Kind::Vzip { .. } | Kind::Vunzip { .. } => {
            e.reads = bits(&[Qs0, Qs1]); e.writes = e.reads;
        }
        Kind::SrcQ { qup, ld } => {
            e.reads = bits(&[Qs0, Qs1]);
            e.writes = if ld == Mode::None { bits(&[Qa]) | if qup { bits(&[Qs0]) } else { 0 } }
                else { bits(&[Qs0, Qu]) };
            // Hardware consumer pairs distinguish the shifted next-cycle
            // result from the loaded Qu, which needs one intervening cycle.
            if ld != Mode::None { e.delayed = bits(&[Qu]); }
        }
        Kind::Srcmb { .. } => e.writes = bits(&[Qu]),
        Kind::Arith { ld, st, .. } => {
            e.reads = bits(&[Qx, Qy]) | if st { bits(&[Qv]) } else { 0 };
            e.writes = bits(&[dst]) | if ld { bits(&[Qu]) } else { 0 };
        }
        Kind::Vrelu { .. } => { e.reads = bits(&[Qs]); e.writes = e.reads; }
        Kind::Vprelu { .. } => { e.reads = bits(&[Qx, Qy]); e.writes = bits(&[Qz]); }
        Kind::Vmulas { ld, qup, .. } => {
            e.reads = bits(&[Qx, Qy]) | if qup { bits(&[Qs0, Qs1]) } else { 0 };
            e.writes = if ld != LdKind::None { bits(&[Qu]) } else { 0 }
                | if qup { bits(&[Qs0]) } else { 0 };
        }
        Kind::Vsmulas { ld, .. } => {
            e.reads = bits(&[Qx, Qy]); e.writes = if ld { bits(&[Qu]) } else { 0 };
        }
        Kind::Cmul { store: false } => {
            e.reads = bits(&[Qx, Qy]);
            if o.get(Sel) < 6 { e.reads |= bits(&[Qz]); e.writes |= bits(&[Qz]); }
            e.writes |= bits(&[Qu]);
        }
        Kind::Cmul { store: true } => e.reads = bits(&[Qx, Qy, Qv]),
        Kind::MvQr => { e.reads = bits(&[Qs, Qx]); e.writes = bits(&[Qu, Qa]); }
        Kind::Ldqa { .. } | Kind::LdAccx | Kind::StAccx | Kind::LdQacc { .. }
        | Kind::StQacc { .. } | Kind::LdUa | Kind::StUa | Kind::Ldf { .. }
        | Kind::Stf { .. } | Kind::ZeroQacc | Kind::ZeroAccx | Kind::SrsAccx | Kind::Unimpl => {}
    }
    e
}


/// Effects of a decoded PIE instruction (`Insn::imm` is its table index, `raw` the word).
pub fn insn_effects(i: &crate::decode::Insn) -> QEffects {
    let p = &crate::pie::OPS[i.imm as usize];
    effects(p, &crate::pie::extract(i.raw, p))
}
