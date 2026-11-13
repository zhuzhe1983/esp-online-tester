//! ESP32-S3 Processor Instruction Extensions (PIE): the `ee.*` SIMD instructions on eight 128-bit Q
//! registers, the 40-bit ACCX and the 2x160-bit QACC accumulators. Encodings come from the TRM's
//! per-instruction "Instruction Word" layouts (`pie_table.rs`, generated), semantics from the
//! "Operation" pseudo-code of the same chapter. 24-bit forms use op0 = 4 or the two QRST
//! custom slots (op0 = 0, op1 = 6/7); 32-bit forms use op0 = 0xe/0xf.
//! PIE is coprocessor 3: executing any of these with CPENABLE[3] clear raises the CP3-disabled exception,
//! which is how FreeRTOS lazily saves/restores the state per task.
use crate::bus::Bus;
use crate::decode::Insn;
use crate::exec::Trap;
use crate::state::{exc, Cpu};
pub use crate::pie_table::{Role, OPS};

pub struct Field { pub role: Role, pub pieces: &'static [(u8, u8, u8)], pub signed: bool, pub scale: u8 }
pub struct PieInsn { pub name: &'static str, pub len: u8, pub mask: u32, pub value: u32, pub fields: &'static [Field], pub kind: Kind }

#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Mode { None, Ip, Xp, Incp }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum LdKind { None, Ip, Xp, Ldbc }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum ArithOp { Adds, Subs, Max, Min, Mul { signed: bool } }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Cmp { Eq, Lt, Gt }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Vld128(Mode), Vld64 { high: bool, mode: Mode }, LdUsar(Mode), Ldbc { w: u8, mode: Mode }, Ldhbc16, Ldqa { w: u8, signed: bool, mode: Mode },
    LdAccx, StAccx, LdQacc { h: bool, high32: bool }, StQacc { h: bool, high32: bool }, LdUa, StUa,
    Ldf { n: u8, mode: Mode }, Stf { n: u8, mode: Mode }, Vst128(Mode), Vst64 { high: bool, mode: Mode },
    MoviA, MoviQ, ZeroQ, ZeroQacc, ZeroAccx, MovQacc { w: u8, signed: bool },
    Andq, Orq, Xorq, Notq, Vsl32, Vsr32, Slcxxp, Srcxxp, Slci, Srci, SrcQ { qup: bool, ld: Mode }, Srcmb { w: u8 }, SrsAccx,
    Arith { op: ArithOp, w: u8, ld: bool, st: bool }, Vcmp { cmp: Cmp, w: u8 }, Vrelu { w: u8 }, Vprelu { w: u8 }, Vzip { w: u8 }, Vunzip { w: u8 },
    Vmulas { signed: bool, w: u8, accx: bool, ld: LdKind, qup: bool }, Vsmulas { w: u8, ld: bool }, Cmul { store: bool },
    LdQr, StQr, MvQr, Unimpl,
}

/// Decode a PIE instruction word (bytes 0..3 of the fetch, little-endian). Returns the table index.
pub fn decode(w: u32) -> Option<usize> {
    let len = match w & 0xf {
        0 if matches!((w >> 16) & 0xf, 6 | 7) => 3,
        4 => 3,
        0xe | 0xf => 4,
        _ => return None,
    };
    let w = if len == 3 { w & 0xff_ffff } else { w };
    OPS.iter().position(|p| p.len == len && (w & p.mask) == p.value)
}

pub struct Ops { v: [i32; 10], r: [Option<Role>; 10], n: usize }
impl Ops {
    pub fn get(&self, role: Role) -> i32 { for k in 0..self.n { if self.r[k] == Some(role) { return self.v[k]; } } 0 }
    pub fn has(&self, role: Role) -> bool { (0..self.n).any(|k| self.r[k] == Some(role)) }
}
pub fn extract(w: u32, p: &PieInsn) -> Ops {
    let mut o = Ops { v: [0; 10], r: [None; 10], n: 0 };
    for f in p.fields {
        let mut val = 0u32; let mut width = 0u32;
        for &(hi, lo, wp) in f.pieces { let n = (hi - lo + 1) as u32; val |= ((w >> wp) & ((1 << n) - 1)) << lo; width = width.max(hi as u32 + 1); }
        let mut v = val as i32;
        if f.signed && width < 32 && (val >> (width - 1)) & 1 == 1 { v = val as i32 - (1i32 << width); }
        v = v.wrapping_mul(f.scale as i32);
        if o.n < 10 { o.v[o.n] = v; o.r[o.n] = Some(f.role); o.n += 1; }
    }
    o
}
/// Windowed AR operands from the extension's typed field descriptions.
pub fn gpr_mask(w: u32, idx: usize) -> u16 {
    let o = extract(w, &OPS[idx]);
    let mut mask = 0;
    for k in 0..o.n {
        if matches!(o.r[k], Some(Role::As | Role::Ad | Role::Au | Role::Ax | Role::Ay | Role::At)) {
            mask |= 1 << o.v[k];
        }
    }
    mask
}

/// Highest AR index used (window-overflow check).
pub fn max_ar(w: u32, idx: usize) -> u8 {
    crate::operands::GprEffects { unclassified: gpr_mask(w, idx), ..Default::default() }.max_ar()
}

pub fn format(w: u32, idx: usize) -> String {
    let p = &OPS[idx]; let o = extract(w, p);
    let mut parts = Vec::new();
    for k in 0..o.n {
        let r = o.r[k].unwrap();
        parts.push(match r {
            Role::Qa | Role::Qa0 | Role::Qa1 | Role::Qm | Role::Qs | Role::Qs0 | Role::Qs1 | Role::Qu | Role::Qu1 | Role::Qv | Role::Qx | Role::Qy | Role::Qz | Role::Qz1 => format!("q{}", o.v[k]),
            Role::As | Role::Ad | Role::Au | Role::Ax | Role::Ay | Role::At => format!("a{}", o.v[k]),
            Role::Fu0 | Role::Fu1 | Role::Fu2 | Role::Fu3 | Role::Fv0 | Role::Fv1 | Role::Fv2 | Role::Fv3 => format!("f{}", o.v[k]),
            _ => format!("{}", o.v[k]),
        });
    }
    if parts.is_empty() { p.name.to_string() } else { format!("{}\t{}", p.name, parts.join(", ")) }
}

// ------------------------------------------------------------------ lane helpers
#[inline] fn lane(q: u128, w: u32, i: u32) -> i64 { let v = ((q >> (w * i)) as u64) & ((1u64 << w) - 1); ((v << (64 - w)) as i64) >> (64 - w) }
#[inline] fn lane_u(q: u128, w: u32, i: u32) -> u64 { ((q >> (w * i)) as u64) & ((1u64 << w) - 1) }
#[inline] fn set_lane(q: &mut u128, w: u32, i: u32, v: u64) { let m = ((1u128 << w) - 1) << (w * i); *q = (*q & !m) | ((v as u128) << (w * i) & m); }
#[inline] fn sat(v: i64, bits: u32) -> i64 { let hi = (1i64 << (bits - 1)) - 1; let lo = -(1i64 << (bits - 1)); v.clamp(lo, hi) }
#[inline] fn usat(v: i64, bits: u32) -> i64 { v.clamp(0, (1i64 << bits) - 1) }
#[inline] fn sext(v: i64, bits: u32) -> i64 { (v << (64 - bits)) >> (64 - bits) }

/// One signed complex product, truncated to two 16-bit lanes after the SAR shift.
fn fft_cmul(x: u128, y: u128, sel: u32, sar: u32) -> u32 {
    let pair = sel / 2;
    let (xr, xi) = (lane(x, 16, 2 * pair), lane(x, 16, 2 * pair + 1));
    let (yr, yi) = (lane(y, 16, 2 * pair), lane(y, 16, 2 * pair + 1));
    let (re, im) = if sel & 1 == 0 { (xr * yr + xi * yi, xi * yr - xr * yi) }
                   else { (xr * yr - xi * yi, xi * yr + xr * yi) };
    u32::from((re >> sar) as u16) | (u32::from((im >> sar) as u16) << 16)
}

struct Qacc { lo: u128, hi: u32 }
impl Qacc {
    fn from(a: &[u32; 5]) -> Qacc { Qacc { lo: a[0] as u128 | (a[1] as u128) << 32 | (a[2] as u128) << 64 | (a[3] as u128) << 96, hi: a[4] } }
    fn store(&self, a: &mut [u32; 5]) { a[0] = self.lo as u32; a[1] = (self.lo >> 32) as u32; a[2] = (self.lo >> 64) as u32; a[3] = (self.lo >> 96) as u32; a[4] = self.hi; }
    fn get(&self, lo: u32, w: u32) -> u64 {
        let m = (1u128 << w) - 1;
        let v = if lo + w <= 128 { (self.lo >> lo) & m } else if lo >= 128 { ((self.hi as u128) >> (lo - 128)) & m } else { ((self.lo >> lo) | ((self.hi as u128) << (128 - lo))) & m };
        v as u64
    }
    fn set(&mut self, lo: u32, w: u32, v: u64) {
        let m = (1u128 << w) - 1; let v = (v as u128) & m;
        if lo + w <= 128 { self.lo = (self.lo & !(m << lo)) | (v << lo); }
        else if lo >= 128 { let s = lo - 128; self.hi = (self.hi & !((m as u32) << s)) | ((v as u32) << s); }
        else { let nlo = 128 - lo; self.lo = (self.lo & !(m << lo)) | (v << lo); let mh = (m >> nlo) as u32; self.hi = (self.hi & !mh) | (((v >> nlo) as u32) & mh); }
    }
}
/// QACC lane `i` for element width `w` (8 -> 16 lanes of 20 bits, 16 -> 8 lanes of 40 bits): (half is H, bit offset, lane width).
#[inline] fn qlane(w: u32, i: u32) -> (bool, u32, u32) { if w == 8 { (i >= 8, (i % 8) * 20, 20) } else { (i >= 4, (i % 4) * 40, 40) } }
fn qacc_get(cpu: &Cpu, w: u32, i: u32, signed: bool) -> i64 { let (h, off, lw) = qlane(w, i); let a = Qacc::from(if h { &cpu.qacc_h } else { &cpu.qacc_l }); let v = a.get(off, lw) as i64; if signed { sext(v, lw) } else { v } }
fn qacc_set(cpu: &mut Cpu, w: u32, i: u32, v: i64) { let (h, off, lw) = qlane(w, i); let arr = if h { &mut cpu.qacc_h } else { &mut cpu.qacc_l }; let mut a = Qacc::from(arr); a.set(off, lw, v as u64); a.store(arr); }
fn accx_get(cpu: &Cpu) -> i64 { sext(cpu.accx[0] as i64 | (((cpu.accx[1] & 0xff) as i64) << 32), 40) }
fn accx_set(cpu: &mut Cpu, v: i64) { cpu.accx[0] = v as u32; cpu.accx[1] = ((v >> 32) & 0xff) as u32; }
fn src(qs0: u128, qs1: u128, sh_bytes: u32) -> u128 { let sh = (sh_bytes & 0xf) * 8; if sh == 0 { qs0 } else { (qs0 >> sh) | (qs1 << (128 - sh)) } }

// ------------------------------------------------------------------ memory
fn ld<B: Bus>(cpu: &mut Cpu, bus: &mut B, a: u32, bytes: u32) -> Result<u128, Trap> {
    let a = a & !(bytes - 1);
    let mut v = 0u128;
    let mut k = 0;
    while k < bytes {
        if bytes >= 4 { match bus.read32(a + k) { Ok(x) => v |= (x as u128) << (8 * k), Err(_) => return Err(cpu.raise_mem(exc::LOAD_PROHIBITED, a + k)) } k += 4; }
        else if bytes == 2 { match bus.read16(a) { Ok(x) => v = x as u128, Err(_) => return Err(cpu.raise_mem(exc::LOAD_PROHIBITED, a)) } k += 2; }
        else { match bus.read8(a) { Ok(x) => v = x as u128, Err(_) => return Err(cpu.raise_mem(exc::LOAD_PROHIBITED, a)) } k += 1; }
    }
    Ok(v)
}
fn st<B: Bus>(cpu: &mut Cpu, bus: &mut B, a: u32, bytes: u32, v: u128) -> Result<(), Trap> {
    let a = a & !(bytes - 1);
    let mut k = 0;
    while k < bytes { if bus.write32(a + k, (v >> (8 * k)) as u32).is_err() { return Err(cpu.raise_mem(exc::STORE_PROHIBITED, a + k)); } k += 4; }
    Ok(())
}

pub fn exec<B: Bus>(cpu: &mut Cpu, bus: &mut B, i: &Insn) -> Result<(), Trap> {
    if cpu.cpenable & (1 << 3) == 0 { return Err(cpu.raise(exc::COPROCESSOR0_DISABLED + 3)); }
    // The optional PIE cost hypotheses are charged in the table executor only.
    if i.r & PACKED != 0 && cpu.approximate_pie_mode == 0 { return exec_packed(cpu, bus, i); }
    exec_table(cpu, bus, i)
}

/// Any PIE instruction from its table entry, extracting the operands from the word on every
/// execution. The reference the packed path is tested against.
fn exec_table<B: Bus>(cpu: &mut Cpu, bus: &mut B, i: &Insn) -> Result<(), Trap> {
    let p = &OPS[i.imm as usize];
    let w = i.raw;
    let o = extract(w, p);
    use Role::*;
    macro_rules! q { ($r:expr) => { cpu.qr[o.get($r) as usize & 7] }; }
    macro_rules! setq { ($r:expr, $v:expr) => { { let v: u128 = $v; cpu.qr[o.get($r) as usize & 7] = v; } }; }
    macro_rules! ar { ($r:expr) => { cpu.get_ar(o.get($r) as u8) }; }
    macro_rules! setar { ($r:expr, $v:expr) => { { let v: u32 = $v; cpu.set_ar(o.get($r) as u8, v); } }; }
    macro_rules! post { ($mode:expr) => { match $mode { Mode::Ip => setar!(As, ar!(As).wrapping_add(o.get(Imm) as u32)), Mode::Xp => setar!(As, ar!(As).wrapping_add(ar!(Ad))), Mode::Incp => setar!(As, ar!(As).wrapping_add(16)), Mode::None => {} } }; }
    let sar = cpu.sar & 0x3f;
    match p.kind {
        Kind::Vld128(m) => { let v = ld(cpu, bus, ar!(As), 16)?; setq!(Qu, v); post!(m); }
        Kind::Vld64 { high, mode } => { let v = ld(cpu, bus, ar!(As), 8)? as u64; let mut q = q!(Qu); if high { q = (q & 0xffff_ffff_ffff_ffff) | ((v as u128) << 64); } else { q = (q & !0xffff_ffff_ffff_ffffu128) | v as u128; } setq!(Qu, q); post!(mode); }
        Kind::LdUsar(m) => { let a = ar!(As); let v = ld(cpu, bus, a, 16)?; setq!(Qu, v); cpu.sar_byte = a & 0xf; post!(m); }
        Kind::Ldbc { w, mode } => { let v = ld(cpu, bus, ar!(As), (w / 8) as u32)?; let n = 128 / w as u32; let mut q = 0u128; for k in 0..n { q |= v << (k * w as u32); } setq!(Qu, q); post!(mode); }
        Kind::Ldhbc16 => { let d = ld(cpu, bus, ar!(As), 16)?; let (mut a, mut b) = (0u128, 0u128); for k in 0..4 { let lo = lane_u(d, 16, k); let hi = lane_u(d, 16, 4 + k); set_lane(&mut a, 16, 2 * k, lo); set_lane(&mut a, 16, 2 * k + 1, lo); set_lane(&mut b, 16, 2 * k, hi); set_lane(&mut b, 16, 2 * k + 1, hi); } setq!(Qu, a); setq!(Qu1, b); post!(Mode::Incp); }
        Kind::Ldqa { w, signed, mode } => { let d = ld(cpu, bus, ar!(As), 16)?; let n = 128 / w as u32; for k in 0..n { let v = if signed { lane(d, w as u32, k) } else { lane_u(d, w as u32, k) as i64 }; qacc_set(cpu, w as u32, k, v); } post!(mode); }
        Kind::LdAccx => { let v = ld(cpu, bus, ar!(As), 8)? as u64; accx_set(cpu, sext(v as i64 & 0xff_ffff_ffff, 40)); post!(Mode::Ip); }
        Kind::StAccx => { let v = (accx_get(cpu) as u64) & 0xff_ffff_ffff; st(cpu, bus, ar!(As), 8, v as u128)?; post!(Mode::Ip); }
        Kind::LdQacc { h, high32 } => { let a = ar!(As); if high32 { let v = ld(cpu, bus, a, 4)? as u32; if h { cpu.qacc_h[4] = v; } else { cpu.qacc_l[4] = v; } } else { let v = ld(cpu, bus, a, 16)?; let arr = if h { &mut cpu.qacc_h } else { &mut cpu.qacc_l }; for (k, word) in arr[..4].iter_mut().enumerate() { *word = (v >> (32 * k)) as u32; } } post!(Mode::Ip); }
        Kind::StQacc { h, high32 } => { let arr = if h { cpu.qacc_h } else { cpu.qacc_l }; if high32 { st(cpu, bus, ar!(As), 4, arr[4] as u128)?; } else { let v = arr[0] as u128 | (arr[1] as u128) << 32 | (arr[2] as u128) << 64 | (arr[3] as u128) << 96; st(cpu, bus, ar!(As), 16, v)?; } post!(Mode::Ip); }
        Kind::LdUa => { let v = ld(cpu, bus, ar!(As), 16)?; for (k, word) in cpu.ua_state.iter_mut().enumerate() { *word = (v >> (32 * k)) as u32; } post!(Mode::Ip); }
        Kind::StUa => { let u = cpu.ua_state; let v = u[0] as u128 | (u[1] as u128) << 32 | (u[2] as u128) << 64 | (u[3] as u128) << 96; st(cpu, bus, ar!(As), 16, v)?; post!(Mode::Ip); }
        Kind::Ldf { n, mode } => { let v = ld(cpu, bus, ar!(As), 4 * n as u32)?; let roles = [Fu0, Fu1, Fu2, Fu3]; for (k, &role) in roles.iter().enumerate().take(n as usize) { let r = o.get(role) as usize & 15; cpu.fr[r] = (v >> (32 * k)) as u32; } post!(mode); }
        Kind::Stf { n, mode } => { let roles = [Fv0, Fv1, Fv2, Fv3]; let mut v = 0u128; for (k, &role) in roles.iter().enumerate().take(n as usize) { v |= (cpu.fr[o.get(role) as usize & 15] as u128) << (32 * k); } st(cpu, bus, ar!(As), 4 * n as u32, v)?; post!(mode); }
        Kind::Vst128(m) => { st(cpu, bus, ar!(As), 16, q!(Qv))?; post!(m); }
        Kind::Vst64 { high, mode } => { let q = q!(Qv); st(cpu, bus, ar!(As), 8, if high { q >> 64 } else { q & 0xffff_ffff_ffff_ffff })?; post!(mode); }
        Kind::MoviA => { let v = lane_u(q!(Qs), 32, o.get(Sel) as u32 & 3) as u32; setar!(Au, v); }
        Kind::MoviQ => { let mut q = q!(Qu); set_lane(&mut q, 32, o.get(Sel) as u32 & 3, ar!(As) as u64); setq!(Qu, q); }
        Kind::ZeroQ => setq!(Qa, 0),
        Kind::ZeroQacc => { cpu.qacc_h = [0; 5]; cpu.qacc_l = [0; 5]; }
        Kind::ZeroAccx => { cpu.accx = [0; 2]; }
        Kind::MovQacc { w, signed } => { let d = q!(Qs); let n = 128 / w as u32; for k in 0..n { let v = if signed { lane(d, w as u32, k) } else { lane_u(d, w as u32, k) as i64 }; qacc_set(cpu, w as u32, k, v); } }
        Kind::Andq => setq!(Qa, q!(Qx) & q!(Qy)),
        Kind::Orq => setq!(Qa, q!(Qx) | q!(Qy)),
        Kind::Xorq => setq!(Qa, q!(Qx) ^ q!(Qy)),
        Kind::Notq => setq!(Qa, !q!(Qx)),
        Kind::Vsl32 => { let s = q!(Qs); let mut r = 0u128; for k in 0..4 { let v = lane_u(s, 32, k) as u32; set_lane(&mut r, 32, k, if sar >= 32 { 0 } else { (v << sar) as u64 }); } setq!(Qa, r); }
        Kind::Vsr32 => { let s = q!(Qs); let mut r = 0u128; for k in 0..4 { let v = lane_u(s, 32, k) as u32; set_lane(&mut r, 32, k, if sar >= 32 { 0 } else { (v >> sar) as u64 }); } setq!(Qa, r); }
        Kind::Slcxxp | Kind::Slci => { let sh = if p.kind == Kind::Slci { (o.get(Sar) as u32 + 1) * 8 } else { ((ar!(As) & 0xf) + 1) * 8 }; let (q0, q1) = (q!(Qs0), q!(Qs1)); let (lo, hi) = if sh >= 128 { (0, q0) } else { (q0 << sh, (q1 << sh) | (q0 >> (128 - sh))) }; setq!(Qs0, lo); setq!(Qs1, hi); if p.kind == Kind::Slcxxp { post!(Mode::Xp); } }
        Kind::Srcxxp | Kind::Srci => { let sh = if p.kind == Kind::Srci { (o.get(Sar) as u32 + 1) * 8 } else { ((ar!(As) & 0xf) + 1) * 8 }; let (q0, q1) = (q!(Qs0), q!(Qs1)); let (lo, hi) = if sh >= 128 { (q1, 0) } else { ((q0 >> sh) | (q1 << (128 - sh)), q1 >> sh) }; setq!(Qs0, lo); setq!(Qs1, hi); if p.kind == Kind::Srcxxp { post!(Mode::Xp); } }
        Kind::SrcQ { qup, ld: ldm } => {
            let (q0, q1) = (q!(Qs0), q!(Qs1)); let r = src(q0, q1, cpu.sar_byte);
            if ldm == Mode::None { setq!(Qa, r); if qup { setq!(Qs0, q1); } }
            else { setq!(Qs0, r); let v = ld(cpu, bus, ar!(As), 16)?; setq!(Qu, v); post!(ldm); }
        }
        Kind::Srcmb { w } => { let shift = ar!(As) & if w == 8 { 0x1f } else { 0x3f }; let n = 128 / w as u32; let mut r = 0u128; for k in 0..n { let v = qacc_get(cpu, w as u32, k, true) >> shift; qacc_set(cpu, w as u32, k, v); set_lane(&mut r, w as u32, k, sat(v, w as u32) as u64); } setq!(Qu, r); }
        Kind::SrsAccx => { let v = accx_get(cpu) >> (ar!(As) & 0x3f); accx_set(cpu, v); setar!(Au, sat(v, 32) as u32); }
        Kind::Arith { op, w, ld: ldq, st: stq } => {
            let (x, y) = (q!(Qx), q!(Qy)); let wd = w as u32; let n = 128 / wd; let mut r = 0u128;
            for k in 0..n {
                let (a, b) = (lane(x, wd, k), lane(y, wd, k));
                let v = match op {
                    ArithOp::Adds => sat(a + b, wd), ArithOp::Subs => sat(a - b, wd), ArithOp::Max => a.max(b), ArithOp::Min => a.min(b),
                    ArithOp::Mul { signed } => if signed { (a * b) >> sar } else { ((lane_u(x, wd, k) as i64 * lane_u(y, wd, k) as i64) as u64 >> sar) as i64 },
                };
                set_lane(&mut r, wd, k, v as u64);
            }
            let dst = if o.has(Qz) { Qz } else { Qa };
            setq!(dst, r);
            if ldq { let v = ld(cpu, bus, ar!(As), 16)?; setq!(Qu, v); post!(Mode::Incp); }
            if stq { st(cpu, bus, ar!(As), 16, q!(Qv))?; post!(Mode::Incp); }
        }
        Kind::Vcmp { cmp, w } => { let (x, y) = (q!(Qx), q!(Qy)); let wd = w as u32; let n = 128 / wd; let mut r = 0u128; for k in 0..n { let (a, b) = (lane(x, wd, k), lane(y, wd, k)); let t = match cmp { Cmp::Eq => a == b, Cmp::Lt => a < b, Cmp::Gt => a > b }; if t { set_lane(&mut r, wd, k, u64::MAX); } } setq!(Qa, r); }
        Kind::Vrelu { w } => { let wd = w as u32; let n = 128 / wd; let alpha = sext(ar!(Ax) as i64, wd); let sh = ar!(Ay) & if w == 8 { 0x1f } else { 0x3f }; let mut q = q!(Qs); for k in 0..n { let v = lane(q, wd, k); if v <= 0 { set_lane(&mut q, wd, k, ((v * alpha) >> sh) as u64); } } setq!(Qs, q); }
        Kind::Vprelu { w } => { let wd = w as u32; let n = 128 / wd; let (x, y) = (q!(Qx), q!(Qy)); let sh = ar!(Ay) & if w == 8 { 0x1f } else { 0x3f }; let mut r = x; for k in 0..n { let v = lane(x, wd, k); if v <= 0 { set_lane(&mut r, wd, k, ((v * lane(y, wd, k)) >> sh) as u64); } } setq!(Qz, r); }
        Kind::Vzip { w } => { let wd = w as u32; let n = 128 / wd; let (a, b) = (q!(Qs0), q!(Qs1)); let (mut r0, mut r1) = (0u128, 0u128); for k in 0..n / 2 { set_lane(&mut r0, wd, 2 * k, lane_u(a, wd, k)); set_lane(&mut r0, wd, 2 * k + 1, lane_u(b, wd, k)); set_lane(&mut r1, wd, 2 * k, lane_u(a, wd, n / 2 + k)); set_lane(&mut r1, wd, 2 * k + 1, lane_u(b, wd, n / 2 + k)); } setq!(Qs0, r0); setq!(Qs1, r1); }
        Kind::Vunzip { w } => { let wd = w as u32; let n = 128 / wd; let (a, b) = (q!(Qs0), q!(Qs1)); let (mut r0, mut r1) = (0u128, 0u128); for k in 0..n / 2 { set_lane(&mut r0, wd, k, lane_u(a, wd, 2 * k)); set_lane(&mut r0, wd, n / 2 + k, lane_u(b, wd, 2 * k)); set_lane(&mut r1, wd, k, lane_u(a, wd, 2 * k + 1)); set_lane(&mut r1, wd, n / 2 + k, lane_u(b, wd, 2 * k + 1)); } setq!(Qs0, r0); setq!(Qs1, r1); }
        Kind::Vmulas { signed, w, accx, ld: ldk, qup } => {
            let (x, y) = (q!(Qx), q!(Qy)); let wd = w as u32; let n = 128 / wd;
            if accx {
                let mut sum = accx_get(cpu);
                for k in 0..n { sum += if signed { lane(x, wd, k) * lane(y, wd, k) } else { (lane_u(x, wd, k) * lane_u(y, wd, k)) as i64 }; }
                accx_set(cpu, if signed { sat(sum, 40) } else { usat(sum, 40) });
            } else {
                let aw = if w == 8 { 20 } else { 40 };
                for k in 0..n { let prod = if signed { lane(x, wd, k) * lane(y, wd, k) } else { (lane_u(x, wd, k) * lane_u(y, wd, k)) as i64 }; let acc = qacc_get(cpu, wd, k, signed) + prod; qacc_set(cpu, wd, k, if signed { sat(acc, aw) } else { usat(acc, aw) }); }
            }
            match ldk {
                LdKind::None => {}
                LdKind::Ip => { let v = ld(cpu, bus, ar!(As), 16)?; setq!(Qu, v); post!(Mode::Ip); }
                LdKind::Xp => { let v = ld(cpu, bus, ar!(As), 16)?; setq!(Qu, v); post!(Mode::Xp); }
                LdKind::Ldbc => { let v = ld(cpu, bus, ar!(As), (w / 8) as u32)?; let mut q = 0u128; for k in 0..n { q |= v << (k * wd); } setq!(Qu, q); setar!(As, ar!(As).wrapping_add((w / 8) as u32)); }
            }
            if qup { let r = src(q!(Qs0), q!(Qs1), cpu.sar_byte); setq!(Qs0, r); }
        }
        Kind::Vsmulas { w, ld: ldq } => {
            let (x, y) = (q!(Qx), q!(Qy)); let wd = w as u32; let n = 128 / wd; let t = lane(y, wd, o.get(Sel) as u32 % n); let aw = if w == 8 { 20 } else { 40 };
            for k in 0..n { let acc = qacc_get(cpu, wd, k, true) + lane(x, wd, k) * t; qacc_set(cpu, wd, k, sat(acc, aw)); }
            if ldq { let v = ld(cpu, bus, ar!(As), 16)?; setq!(Qu, v); post!(Mode::Incp); }
        }
        Kind::Cmul { store: false } => {
            // Read before committing Qz so a load fault can restart with aliased inputs.
            let loaded = ld(cpu, bus, ar!(As), 16)?;
            let sel = o.get(Sel) as u32;
            // TRM 1.8.11 defines Qz updates only for sel8 = 0..5. The final pair is
            // computed by ST.XP, which stores the complete FFT result without changing Qz.
            if sel < 6 {
                let v = fft_cmul(q!(Qx), q!(Qy), sel, sar);
                let mut r = q!(Qz); set_lane(&mut r, 32, sel / 2, v as u64); setq!(Qz, r);
            }
            setq!(Qu, loaded);
            post!(Mode::Xp);
        }
        Kind::Cmul { store: true } => {
            let (sel, upd) = (o.get(Sel) as u32, o.get(Upd));
            // TRM 1.8.12 specifies sel8 = 6/7 and upd4 = 0/1/2 only. Do not invent
            // values for its undefined temporary or undocumented store layout.
            if sel < 6 || upd > 2 { return Err(Trap::Unimplemented(cpu.pc, w)); }
            let x = q!(Qx);
            let mut v = q!(Qv);
            // Signed shift is an emulator assumption: TRM pseudocode does not specify
            // arithmetic versus logical shifting here. Negative lanes need silicon validation.
            if upd != 0 {
                for k in 0..4 { set_lane(&mut v, 16, k, (lane(x, 16, k) >> o.get(Sar)) as u64); }
                if upd == 2 {
                    // The final FFT stage exchanges the middle 32-bit groups.
                    let (lo, hi) = (lane_u(v, 32, 1), lane_u(v, 32, 2));
                    set_lane(&mut v, 32, 1, hi); set_lane(&mut v, 32, 2, lo);
                }
            }
            set_lane(&mut v, 32, 3, fft_cmul(x, q!(Qy), sel, sar) as u64);
            st(cpu, bus, ar!(As), 16, v)?;
            post!(Mode::Xp);
        }
        Kind::LdQr => { let v = ld(cpu, bus, ar!(As).wrapping_add(o.get(Imm) as u32), 16)?; setq!(Qu, v); }
        Kind::StQr => { st(cpu, bus, ar!(As).wrapping_add(o.get(Imm) as u32), 16, q!(Qs))?; }
        Kind::MvQr => { let s = if o.has(Qs) { q!(Qs) } else { q!(Qx) }; let dst = if o.has(Qu) { Qu } else { Qa }; setq!(dst, s); }
        Kind::Unimpl => return Err(Trap::Unimplemented(cpu.pc, w)),
    }
    let extra = match cpu.approximate_pie_mode {
        1 if matches!(p.kind, Kind::Vld128(_) | Kind::Vst128(_) | Kind::LdUsar(_)
            | Kind::SrcQ { ld: Mode::Ip | Mode::Xp, .. } | Kind::LdQr | Kind::StQr) => 1,
        2 if matches!(p.kind, Kind::SrcQ { ld: Mode::Ip | Mode::Xp, .. }) => 2,
        _ => 0,
    };
    if extra != 0 {
        bus.add_timing_penalty(extra);
        cpu.approximate_pie_events += 1;
        cpu.approximate_pie_cycles += u64::from(extra);
    }
    Ok(())
}

// ------------------------------------------------------------------ packed fast path
/// `Insn::r` bit of a PIE instruction whose operands `pack` extracted at decode time.
pub const PACKED: u8 = 0x80;

/// Operands of the hot PIE instructions (on-device inference loops over these), extracted once
/// when the instruction is decoded and carried in the `r`/`s`/`t` its `Insn` otherwise leaves
/// zero: `r` = PACKED | third Q register << 4 | AR register, `s` = first Q | second Q << 4,
/// `t` = the post-increment / 16 (each packed load and store scales its immediate by 16).
/// Anything `exec_packed` does not run gets zeros and keeps the table path. The packed values
/// exceed 15, so `Insn::gpr_effects` masks the fields it would otherwise shift by.
pub fn pack(w: u32, idx: usize) -> (u8, u8, u8) {
    use Role::*;
    let p = &OPS[idx];
    let o = extract(w, p);
    let q = |v: i32| (v & 7) as u8;
    let a = (o.get(As) & 15) as u8;
    let imm = o.get(Imm);
    let step = (imm % 16 == 0 && (-128..=127).contains(&(imm / 16))).then_some((imm / 16) as i8 as u8);
    match (p.kind, step) {
        (Kind::ZeroAccx, _) => (PACKED, 0, 0),
        (Kind::Vld128(Mode::Ip), Some(t)) => (PACKED | a, q(o.get(Qu)), t),
        (Kind::Vst128(Mode::Ip), Some(t)) => (PACKED | a, q(o.get(Qv)), t),
        (Kind::Vmulas { signed: true, w: 8 | 16, accx: true, ld: LdKind::None, qup: false }, _) =>
            (PACKED, q(o.get(Qx)) | (q(o.get(Qy)) << 4), 0),
        (Kind::Vmulas { signed: true, w: 8 | 16, accx: true, ld: LdKind::Ip, qup: false }, Some(t)) =>
            (PACKED | (q(o.get(Qu)) << 4) | a, q(o.get(Qx)) | (q(o.get(Qy)) << 4), t),
        _ => (0, 0, 0),
    }
}

/// The instructions `pack` marked, from their packed operands: the 128-bit loads in one bulk
/// read, the dot products over byte arrays. Must leave exactly the state `exec_table` leaves,
/// including when a load faults after the accumulator has changed.
#[inline]
fn exec_packed<B: Bus>(cpu: &mut Cpu, bus: &mut B, i: &Insn) -> Result<(), Trap> {
    let a = i.r & 15;
    let (q0, q1, q2) = ((i.s & 7) as usize, ((i.s >> 4) & 7) as usize, ((i.r >> 4) & 7) as usize);
    let step = (i32::from(i.t as i8) * 16) as u32;
    match OPS[i.imm as usize].kind {
        Kind::Vld128(_) => {
            let at = cpu.get_ar(a);
            let v = ld128(cpu, bus, at)?;
            cpu.qr[q0] = v;
            cpu.set_ar(a, at.wrapping_add(step));
        }
        Kind::Vst128(_) => {
            let (at, v) = (cpu.get_ar(a), cpu.qr[q0]);
            st(cpu, bus, at, 16, v)?;
            cpu.set_ar(a, at.wrapping_add(step));
        }
        Kind::Vmulas { w, ld: load, .. } => {
            let (x, y) = (cpu.qr[q0].to_le_bytes(), cpu.qr[q1].to_le_bytes());
            let dot = if w == 8 { dot_s8(&x, &y) } else { dot_s16(&x, &y) };
            let acc = sat(accx_get(cpu) + dot, 40);
            accx_set(cpu, acc);
            if load == LdKind::Ip {
                let at = cpu.get_ar(a);
                let v = ld128(cpu, bus, at)?;
                cpu.qr[q2] = v;
                cpu.set_ar(a, at.wrapping_add(step));
            }
        }
        Kind::ZeroAccx => cpu.accx = [0; 2],
        _ => unreachable!("pack marks only the kinds handled here"),
    }
    Ok(())
}

/// A 128-bit load: one bulk copy when the bus holds the whole range as plain memory, else the
/// per-word reads, which also raise the exception at the exact faulting word.
#[inline]
fn ld128<B: Bus>(cpu: &mut Cpu, bus: &mut B, a: u32) -> Result<u128, Trap> {
    let a = a & !15;
    let mut bytes = [0u8; 16];
    if bus.read_bulk(a, &mut bytes) { Ok(u128::from_le_bytes(bytes)) } else { ld(cpu, bus, a, 16) }
}

/// Σ x·y over the 16 signed 8-bit lanes. Each product fits an i16, so the compiler can use a
/// widening vector multiply (NEON, SSE, wasm SIMD) instead of 16 scalar steps.
#[inline]
fn dot_s8(x: &[u8; 16], y: &[u8; 16]) -> i64 {
    i64::from(x.iter().zip(y).map(|(&a, &b)| i32::from(i16::from(a as i8) * i16::from(b as i8))).sum::<i32>())
}

/// Σ x·y over the 8 signed 16-bit lanes; the sum can exceed an i32.
#[inline]
fn dot_s16(x: &[u8; 16], y: &[u8; 16]) -> i64 {
    x.as_chunks::<2>().0.iter().zip(y.as_chunks::<2>().0)
        .map(|(&a, &b)| i64::from(i32::from(i16::from_le_bytes(a)) * i32::from(i16::from_le_bytes(b))))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::{Fault, FlatRam};

    const BASE: u32 = 0x3fc9_0000;
    /// Not a multiple of 16: a load aligned into the last partial block runs off the end mid-load.
    const SIZE: usize = 0x1000 - 8;
    const HOT: [&str; 7] = ["ee.vld.128.ip", "ee.vst.128.ip", "ee.vmulas.s8.accx", "ee.vmulas.s8.accx.ld.ip",
                            "ee.vmulas.s16.accx", "ee.vmulas.s16.accx.ld.ip", "ee.zero.accx"];

    /// The same memory without a bulk read, so the packed path takes its per-word fallback.
    struct NoBulk(FlatRam);
    impl Bus for NoBulk {
        fn note_code_page(&mut self, vidx: u32) { self.0.note_code_page(vidx); }
        fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.0.read8(a) }
        fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.0.read16(a) }
        fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.0.read32(a) }
        fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.0.write8(a, v) }
        fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.0.write16(a, v) }
        fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.0.write32(a, v) }
        fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> { self.0.fetch(pc) }
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
        fn u32(&mut self) -> u32 { self.next() as u32 }
        /// Random lanes, or every lane at an extreme of the 8- or 16-bit range.
        fn q(&mut self) -> u128 {
            match self.next() % 6 {
                0 => u128::from_le_bytes([0x80; 16]),
                1 => u128::from_le_bytes([0x7f; 16]),
                2 => u128::from_le_bytes([0x00, 0x80].repeat(8).try_into().unwrap()),
                3 => u128::from_le_bytes([0xff, 0x7f].repeat(8).try_into().unwrap()),
                _ => (u128::from(self.next()) << 64) | u128::from(self.next()),
            }
        }
        /// A 40-bit accumulator, often a few products away from saturating either way.
        fn accx(&mut self) -> i64 {
            let near = (self.next() % 400_000) as i64;
            match self.next() % 3 { 0 => (1i64 << 39) - 1 - near, 1 => -(1i64 << 39) + near, _ => sext(self.next() as i64, 40) }
        }
        /// Mostly inside memory at any alignment; sometimes running off its end, below it, or unmapped.
        fn address(&mut self) -> u32 {
            match self.next() % 12 {
                0 => BASE + SIZE as u32 - 4,
                1 => BASE - 8,
                2 => 0x1000_0000,
                _ => BASE + self.u32() % (SIZE as u32 - 32),
            }
        }
    }

    fn state(c: &Cpu) -> (Vec<u32>, [u128; 8], [u32; 2], u32, u32, u32) {
        ((0..16u8).map(|n| c.get_ar(n)).collect(), c.qr, c.accx, c.exccause, c.excvaddr, c.pc)
    }

    fn ram(fill: &[u8]) -> FlatRam { let mut r = FlatRam::new(BASE, SIZE); r.mem.copy_from_slice(fill); r }

    /// Every hot instruction, decoded from random operand bits, run three ways from one random
    /// state: packed with bulk reads, packed with per-word reads, and from the table. All three
    /// must agree on the result, every register, the Q registers, ACCX, the exception state and
    /// memory.
    #[test]
    fn packed_execution_matches_the_table() {
        let mut r = Rng(0x9e37_79b9_7f4a_7c15);
        for name in HOT {
            let idx = OPS.iter().position(|p| p.name == name).unwrap();
            let p = &OPS[idx];
            let (mut runs, mut faults) = (0, 0);
            for _ in 0..4000 {
                let width = if p.len == 3 { 0xff_ffff } else { u32::MAX };
                let w = (r.u32() & !p.mask & width) | p.value;
                if decode(w) != Some(idx) { continue; }
                let insn = crate::decode::decode(0x4037_0000, w.to_le_bytes());
                assert_eq!((insn.op, insn.imm as usize), (crate::decode::Op::Pie, idx), "{name}");
                assert_ne!(insn.r & PACKED, 0, "{name} {w:#x} decodes packed");
                let table = Insn { r: 0, s: 0, t: 0, ..insn };

                let mut cpu = Cpu::new(0);
                cpu.cpenable = if r.next().is_multiple_of(16) { 0 } else { 1 << 3 };
                for n in 0..16u8 { let v = r.address(); cpu.set_ar(n, v); }
                for q in cpu.qr.iter_mut() { *q = r.q(); }
                let acc = r.accx();
                accx_set(&mut cpu, acc);
                let fill: Vec<u8> = (0..SIZE).map(|_| r.next() as u8).collect();
                let (mut cpu_word, mut cpu_table) = (cpu.clone(), cpu.clone());
                let (mut bulk, mut word, mut reference) = (ram(&fill), NoBulk(ram(&fill)), ram(&fill));

                let want = exec(&mut cpu_table, &mut reference, &table);
                assert_eq!(exec(&mut cpu, &mut bulk, &insn), want, "{name} {w:#x} bulk");
                assert_eq!(exec(&mut cpu_word, &mut word, &insn), want, "{name} {w:#x} per-word");
                assert_eq!(state(&cpu), state(&cpu_table), "{name} {w:#x} bulk state");
                assert_eq!(state(&cpu_word), state(&cpu_table), "{name} {w:#x} per-word state");
                assert!(bulk.mem == reference.mem && word.0.mem == reference.mem, "{name} {w:#x} memory");
                runs += 1;
                faults += usize::from(want.is_err());
            }
            assert!(runs >= 1000, "{name}: only {runs} runs");
            if name.ends_with(".ip") { assert!(faults >= 50, "{name}: only {faults} faults exercised"); }
        }
    }

    /// Packed operands exceed 15 in `r`/`s`/`t`. The register-effect analysis every block build
    /// runs must not shift by them (a debug build panicked there) and must see what the table sees.
    #[test]
    fn packed_fields_leave_register_effects_unchanged() {
        for name in HOT {
            let p = OPS.iter().find(|p| p.name == name).unwrap();
            let mut w = p.value;
            for f in p.fields { for &(hi, lo, wp) in f.pieces { w |= ((1u32 << (hi - lo + 1)) - 1) << wp; } }
            let insn = crate::decode::decode(0x4037_0000, w.to_le_bytes());
            assert!(insn.r & PACKED != 0 && (insn.r > 15 || insn.s > 15 || insn.t > 15), "{name}");
            let mut table = insn;
            (table.r, table.s, table.t) = (0, 0, 0);
            assert!(insn.gpr_effects() == table.gpr_effects(), "{name}");
            assert_eq!(crate::exec::max_ar(&insn), crate::exec::max_ar(&table), "{name}");
        }
    }

    fn w24(w: u32) -> Vec<u8> { vec![w as u8, (w >> 8) as u8, (w >> 16) as u8] }
    fn w16(w: u32) -> Vec<u8> { vec![w as u8, (w >> 8) as u8] }
    /// Assemble a PIE instruction from its table entry, the inverse of `extract`.
    fn pie_op(name: &str, fields: &[(Role, i32)]) -> Vec<u8> {
        let p = OPS.iter().find(|p| p.name == name).unwrap();
        let mut w = p.value;
        for &(role, v) in fields {
            let f = p.fields.iter().find(|f| f.role == role).unwrap();
            let v = (v / f.scale as i32) as u32;
            for &(hi, lo, wp) in f.pieces { let n = hi - lo + 1; w |= ((v >> lo) & ((1 << n) - 1)) << wp; }
        }
        (0..p.len).map(|k| (w >> (8 * k)) as u8).collect()
    }

    /// The interpreter, the block interpreter and the native JIT (on AArch64 hosts; blocks
    /// elsewhere) must leave identical state after a PIE loop: packed loads and multiply-
    /// accumulates, RUR ACCX, a table-path PIE instruction, and a packed PIE instruction as
    /// the last one before LEND, so the JIT's PIE helper also takes the loop back-edge.
    #[test]
    fn pie_loop_agrees_across_step_blocks_and_jit() {
        use Role::*;
        let base = 0x4037_0000u32;
        let mut p = Vec::new();
        p.extend(w24(0x76 | (10 << 8) | (9 << 12) | ((22 - 4) << 16)));                               // 0  loopnez a10, 22
        p.extend(pie_op("ee.vld.128.ip", &[(Qu, 0), (As, 8), (Imm, 16)]));                         // 3
        p.extend(pie_op("ee.vld.128.ip", &[(Qu, 4), (As, 9), (Imm, 16)]));                         // 6
        p.extend(pie_op("ee.vmulas.s8.accx.ld.ip", &[(Qu, 5), (As, 9), (Imm, 16), (Qx, 0), (Qy, 4)])); // 9
        p.extend(w24(0xe30000 | (11 << 12)));                                                      // 13 rur.accx_0 a11
        p.extend(pie_op("ee.vsubs.s8", &[(Qa, 2), (Qx, 0), (Qy, 4)]));                             // 16 table path
        p.extend(pie_op("ee.vmulas.s16.accx", &[(Qx, 2), (Qy, 5)]));                               // 19 last before LEND
        p.extend(w16(0xd | (8 << 4) | (12 << 8)));                                                 // 22 mov.n a8, a12
        p.extend(w16(0xd | (9 << 4) | (13 << 8)));                                                 // 24 mov.n a9, a13
        p.extend(w16(0xc | (10 << 8) | (5 << 12)));                                                // 26 movi.n a10, 5
        p.extend(pie_op("ee.zero.accx", &[]));                                                     // 28
        p.extend(w24(0x6 | ((0u32.wrapping_sub(31 + 4) & 0x3ffff) << 6)));                         // 31 j 0
        let mut results = Vec::new();
        for mode in 0..3 {
            let mut ram = FlatRam::new(base, 64 * 1024);
            ram.mem[..p.len()].copy_from_slice(&p);
            for (k, b) in ram.mem[0x1000..0x1200].iter_mut().enumerate() { *b = ((k as u32).wrapping_mul(0x9e37_79b9) >> 24) as u8; }
            let mut cpu = Cpu::new(0);
            cpu.pc = base; cpu.ps = 0; cpu.cpenable = 1 << 3;
            for (r, v) in [(8, base + 0x1000), (9, base + 0x1100), (10, 5), (12, base + 0x1000), (13, base + 0x1100)] { cpu.set_ar(r, v); }
            let mut done = 0u32;
            if mode == 0 {
                while done < 3000 { crate::step(&mut cpu, &mut ram).expect("no traps"); done += 1; }
            } else {
                cpu.blocks.jit_enabled = mode == 2;
                while done < 3000 { let (used, t) = crate::block::run_block(&mut cpu, &mut ram, 3000 - done); assert!(t.is_none(), "mode {mode}: {t:?}"); done += used; }
                if mode == 2 && crate::jit::AVAILABLE { assert!(cpu.blocks.jit_instructions > 0, "the JIT ran none of the loop"); }
            }
            assert_eq!(done, 3000);
            results.push((cpu.ar, cpu.pc, cpu.lcount, cpu.lend, cpu.insn_count, cpu.ccount, cpu.qr, cpu.accx, ram.mem[0x1000..0x1200].to_vec()));
        }
        assert_ne!(results[0].7, [0, 0], "the loop accumulated");
        assert_eq!(results[1], results[0], "block interpreter differs from single steps");
        assert_eq!(results[2], results[0], "JIT differs from single steps");
    }

    #[test]
    fn only_the_handled_kinds_are_packed() {
        for (i, p) in OPS.iter().enumerate() {
            let packed = pack(p.value, i).0 & PACKED != 0;
            assert_eq!(packed, HOT.contains(&p.name), "{}", p.name);
        }
    }

    #[test]
    fn dot_products_reach_their_extremes_without_overflow() {
        assert_eq!(dot_s8(&[0x80; 16], &[0x80; 16]), 16 * 128 * 128);
        assert_eq!(dot_s8(&[0x80; 16], &[0x7f; 16]), -16 * 128 * 127);
        let min16: [u8; 16] = [0x00, 0x80].repeat(8).try_into().unwrap();
        assert_eq!(dot_s16(&min16, &min16), 8 * (1i64 << 30));
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    struct Ram { inner: emu_core::FlatRam, penalty: u32 }
    impl Bus for Ram {
        fn note_code_page(&mut self, vidx: u32) { self.inner.note_code_page(vidx); }
        fn read8(&mut self, a: u32) -> Result<u8, crate::Fault> { self.inner.read8(a) }
        fn read16(&mut self, a: u32) -> Result<u16, crate::Fault> { self.inner.read16(a) }
        fn read32(&mut self, a: u32) -> Result<u32, crate::Fault> { self.inner.read32(a) }
        fn write8(&mut self, a: u32, v: u8) -> Result<(), crate::Fault> { self.inner.write8(a, v) }
        fn write16(&mut self, a: u32, v: u16) -> Result<(), crate::Fault> { self.inner.write16(a, v) }
        fn write32(&mut self, a: u32, v: u32) -> Result<(), crate::Fault> { self.inner.write32(a, v) }
        fn fetch(&mut self, a: u32) -> Result<[u8; 4], crate::Fault> { self.inner.fetch(a) }
        fn add_timing_penalty(&mut self, cycles: u32) { self.penalty += cycles; }
    }
    #[test]
    fn selective_pie_costs_only_charge_successful_selected_instructions() {
        for (name, costs) in [("ee.vld.128.ip", [0, 1, 0]), ("ee.vst.128.ip", [0, 1, 0]),
            ("ee.ld.128.usar.ip", [0, 1, 0]), ("ee.src.q.ld.ip", [0, 1, 2]), ("ee.src.q", [0, 0, 0])] {
            let p = OPS.iter().find(|p| p.name == name).unwrap();
            let i = crate::decode::decode(0, p.value.to_le_bytes());
            for (mode, expected) in costs.into_iter().enumerate() {
                let mut cpu = Cpu::new(0);
                cpu.cpenable = 8;
                cpu.approximate_pie_mode = mode as u32;
                let mut ram = Ram { inner: emu_core::FlatRam::new(0, 64), penalty: 0 };
                exec(&mut cpu, &mut ram, &i).unwrap();
                assert_eq!(ram.penalty, expected, "{name} mode {mode}");
                assert_eq!(cpu.approximate_pie_cycles, u64::from(expected));
                assert_eq!(cpu.approximate_pie_events, u64::from(expected != 0));
                cpu.cpenable = 0;
                assert!(exec(&mut cpu, &mut ram, &i).is_err());
                assert_eq!(ram.penalty, expected);
            }
        }
    }
}
