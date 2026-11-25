//! Basic-block interpreter.
//!
//! `step()` pays the full per-instruction price — interrupt check, decode-cache probe and
//! validation, cycle/instruction accounting — on every instruction. Measured on real firmware,
//! that scaffolding is a third of run time and no single piece of it is removable, so this
//! module amortises it: a *block* is a straight-line run of pre-decoded instructions ending
//! at a control transfer or at anything that changes interrupt/timer state, and the checks
//! run once per block.
//!
//! What stays exact:
//! - **Timer interrupts**: a block never runs past the point where a `CCOMPARE` would match
//!   (the distance bounds the block), so the interrupt is flagged at the same instruction
//!   boundary as before. Reads and writes of `CCOUNT`/`CCOMPARE*` are forced to start a
//!   block, so they always see exact time.
//! - **Peripheral interrupts**: the bus reports when a register write may have changed an
//!   interrupt line (`Bus::block_break`) and the block ends there, so delivery latency is
//!   unchanged. Instructions that alter `PS`, `INTENABLE` or interrupt state end blocks.
//! - **Control flow**: after each instruction the actual `pc` is compared with the fall-through
//!   address, so taken branches and exceptions leave the block immediately. The WASM JIT
//!   may retain a safe hardware-loop prefix across backedges within the same budget; it
//!   validates code versions and observer boundaries before each repeat.
//! - **Self-modifying code**: a block remembers the write-version of the (at most two) pages
//!   it was decoded from and is rebuilt when either changes. Stores from within a block
//!   into its own remaining instructions take effect at the next block entry — on silicon
//!   that case needs `isync` anyway, and `isync` ends a block.
//! - **Window overflow** checks stay per instruction; they are the one check that matters.
use crate::bus::Bus;
use crate::decode::{decode, Insn, Op};
use crate::exec::{exec_insn, max_ar, Trap};
use crate::state::{sr, Cpu};

pub use emu_core::core::pc_bit;

#[derive(Clone, Copy)]
pub struct BlockInsn { pub insn: Insn, pub max_ar: u8, /// EX141: a static transfer target whose first instruction straddles a fetch word
 pub straddle: bool, /// Backend entry: native byte offset or WASM instruction index
 pub off: u32 }

// `chain` and `bridge` are read only by the WASM run wrapper (`chain_target`, `bridge_target`).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy)]
struct Entry { pc: u32, start: u32, n: u16, /// EX168 s6: the first instruction needs no exact block-boundary state (fits the padding)
 chain: bool, /** EX171 bridge class of a block without code (0: never) */ #[cfg(target_arch = "wasm32")] bridge: u8,
 #[cfg(all(feature="native-short-call",target_arch="aarch64",any(target_os="macos",target_os="linux")))] short:u8,
 vidx: [u32; 2], ver: [u32; 2], code: u32 }
const _: () = assert!(std::mem::size_of::<Entry>() == 32);
impl Entry { const EMPTY: Entry = Entry { pc: 1, start: 0, n: 0, chain: false, #[cfg(target_arch = "wasm32")] bridge: 0,
 #[cfg(all(feature="native-short-call",target_arch="aarch64",any(target_os="macos",target_os="linux")))] short:0,
 vidx: [0; 2], ver: [0; 2], code: crate::jit::NONE }; }

#[cfg(all(feature="native-short-call",target_arch="aarch64",any(target_os="macos",target_os="linux")))]
#[path="block/native_short_call.rs"]
mod native_short_call;

#[cfg(not(target_arch = "wasm32"))] const ENTRIES: usize = 1 << 17;
#[cfg(target_arch = "wasm32")] const ENTRIES: usize = 1 << 15;
/// Instructions per block. At most 3 bytes each, so a block spans at most two version pages.
pub const MAX_LEN: usize = 32;
/// EX172: interior alias slots (16 bytes each).
const ALIASES: usize = 1 << 12;
/// EX172: alias exception-return PCs onto existing blocks (WASM only; the policy never changes results).
pub const ALIAS: bool = cfg!(target_arch = "wasm32");
/// Sequential-distance heuristic: an arrival 2 or 3 bytes after the last instruction
/// may reuse an interior entry. This can include nearby static branch targets.
pub const ALIAS_SEQ: bool = true;
/// Record the arrival when a dispatch ended at `last` and execution continues right behind it.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub(crate) fn note_sequential(cpu: &mut Cpu, last: u32) {
    if ALIAS && ALIAS_SEQ && cpu.pc.wrapping_sub(last).wrapping_sub(2) <= 1 { cpu.blocks.alias_pc = cpu.pc; }
}
/// A guarded region copy cut here, `k` instructions into the chunk headed at `head`: where the next
/// lookup finds it (sequential by construction, also after a four-byte PIE instruction).
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub(crate) fn tail_hints(cpu: &mut Cpu, head: u32, k: u32) {
    let pc = cpu.pc;
    cpu.blocks.alias_pc = pc;
    cpu.blocks.alias_head = (pc, head);
    // alias-s1: name the decoded resume too, so the next lookup neither misses nor aliases.
    if !cpu.price_control && cpu.boundary_bloom & pc_bit(pc) == 0 { cpu.blocks.name_resume(head, k, pc); }
}
/// memo-s2: the lookup hints a memo'd exit left unwritten, for the dispatch that declined the memo
/// (only a dispatch at the memo's PC reads them). `.4`: a copy cut's chunk head, a chunk-head exit's
/// last retired PC, or 1 when the exit wrote its hints itself (an own-module cut).
#[cfg(target_arch = "wasm32")]
#[cold]
fn memo_hints(cpu: &mut Cpu) {
    let (_, _, param, _, aux) = cpu.blocks.memo;
    if aux == 1 { return; }
    if param >> 16 != 0 { tail_hints(cpu, aux, param >> 16); } else { note_sequential(cpu, aux); }
}
/// Arena size at which decoded entries are rebuilt. The arena never reallocates:
/// native code holds pointers into it; WASM code owns separate retained instruction storage.
#[cfg(not(target_arch = "wasm32"))] const ARENA_MAX: usize = 1 << 20;
#[cfg(target_arch = "wasm32")] const ARENA_MAX: usize = 1 << 17;   // 4 MB per core in the browser
/// Native code cache size; flushed together with the blocks when full.
const CODE_SIZE: usize = 256 << 20;   // address space; pages are only committed as code is written

pub struct BlockCache {
    #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-profile"))]
    pub profile: crate::jit::profile::Profile,
    entries: Vec<Entry>,
    arena: Vec<BlockInsn>,
    /// EX140: `exec::static_extras` of every arena instruction, filled when its block is built.
    extras: Vec<u8>,
    /// A block cut short by the caller's budget or a timer deadline resumes here rather than
    /// spawning a new block at the cut point: (entry index, arena index, pc at that index).
    pub(crate) resume: (u32, u32, u32),
    /// EX172: an exception-return, sequential or deferred arrival PC, or 1. A lookup miss there
    /// may enter an existing block at that instruction instead of decoding a new head.
    pub(crate) alias_pc: u32,
    /// tails-s1: a guarded region copy cut at `.0` inside the chunk headed at `.1`.
    pub(crate) alias_head: (u32, u32),
    /// EX172: direct-mapped interior PC -> (head PC, arena start of that build, arena index),
    /// filled on misses. The arena only grows between flushes, so an entry that still has this
    /// head and start is the same build and the index still names `pc`. Cleared by flush.
    aliases: Vec<(u32, u32, u32, u32)>,
    #[cfg(feature = "wasm-jit-tests")]
    pub alias_hits: u64,
    #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-profile"))]
    profile_entry: usize,
    /// EX153: decoded entry of the block the WASM wrapper chained into last; a CUT resumes in it.
    #[cfg(target_arch = "wasm32")]
    pub(crate) chain_ei: u32,
    /// EX171: instructions the chain loop interpreted in place during the current wrapper call.
    #[cfg(target_arch = "wasm32")]
    pub(crate) bridged: u32,
    /// lane-s1: a region exit that ended a quantum: (PC, block code it entered through, region
    /// parameter resuming at that PC, region epoch); PC 1 when none. See `jit::resume`.
    #[cfg(target_arch = "wasm32")]
    pub(crate) memo: (u32, u32, u32, u64, u32),
    pub builds: u64,
    pub flushes: u64,
    /// native code for blocks, when the host supports it and `jit_enabled`
    #[cfg(not(target_arch = "wasm32"))]
    code: Option<crate::jit::CodeCache>,
    #[cfg(target_arch = "wasm32")]
    code: Option<Box<crate::jit::CodeCache>>,
    pub jit_enabled: bool,
    /// A machine observer requires one callback for each individual block execution.
    pub observed: bool,
    pub compiled: u64,
    /// Instructions retired through compiled blocks (including their interpreter helpers).
    pub jit_instructions: u64,
}

impl BlockCache {
    pub fn new() -> Self {
        let code = crate::jit::CodeCache::new(CODE_SIZE);
        // WASM moves the owner out during every compiled call to keep CPU borrows
        // disjoint. Move a pointer instead of all cache collection metadata.
        #[cfg(target_arch = "wasm32")]
        let code = code.map(Box::new);
        BlockCache {
                     #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-profile"))]
                     profile: crate::jit::profile::Profile::default(),
                     #[cfg(target_arch = "wasm32")]
                     chain_ei: u32::MAX,
                     #[cfg(target_arch = "wasm32")]
                     bridged: 0,
                     #[cfg(target_arch = "wasm32")]
                     memo: (1, 0, 0, 0, 1),
                     entries: vec![Entry::EMPTY; ENTRIES], arena: Vec::with_capacity(ARENA_MAX + MAX_LEN), extras: Vec::new(), resume: (0, 0, 1), alias_pc: 1, alias_head: (1, 1), aliases: vec![(1, 0, 0, 0); if ALIAS { ALIASES } else { 0 }], builds: 0, flushes: 0,
                     #[cfg(feature = "wasm-jit-tests")]
                     alias_hits: 0,
                     #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-profile"))]
                     profile_entry: 0,
                     code, jit_enabled: crate::jit::AVAILABLE, observed: false, compiled: 0, jit_instructions: 0 }
    }
    pub fn flush(&mut self) {
        for e in self.entries.iter_mut() { *e = Entry::EMPTY; }
        self.arena.clear(); self.extras.clear(); self.resume = (0, 0, 1); self.alias_pc = 1; self.alias_head = (1, 1);
        // memo-s2: its unwritten hints are forgotten with the others
        #[cfg(target_arch = "wasm32")]
        { self.memo.0 = 1; }
        for a in self.aliases.iter_mut() { a.0 = 1; } self.flushes += 1;
        if let Some(c) = &mut self.code { c.reset(); }
    }
    /// Bytes of native code currently in use.
    pub fn code_bytes(&self) -> usize { self.code.as_ref().map(|c| c.used()).unwrap_or(0) }
    /// Region counters, in the opt-in profile build only.
    pub fn region_report(&self) -> Option<String> {
        #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-profile"))]
        { return self.code.as_ref().map(|c| c.region_stats.report()); }
        #[allow(unreachable_code)]
        None
    }
    /// EX153: a valid decoded entry with compiled code at `pc` whose first instruction needs no
    /// exact block-boundary state.
    #[cfg(target_arch = "wasm32")]
    #[inline(always)]
    pub(crate) fn chain_target(&self, pc: u32, pv: &[u32]) -> Option<(u32, u32)> {
        let ei = Self::index(pc);
        let e = &self.entries[ei];
        (e.pc == pc && e.chain && e.code != crate::jit::NONE && Self::valid(e, pv))
            .then_some((ei as u32, e.code))
    }
    /// EX171: a valid decoded block at `pc` that has no code, consists only of instructions of class
    /// 1..=`BRIDGE_CLASS` and fits in `room`: (arena start, length).
    #[cfg(target_arch = "wasm32")]
    #[inline(always)]
    pub(crate) fn bridge_target(&self, pc: u32, pv: &[u32], room: u32) -> Option<(u32, u32)> {
        let e = &self.entries[Self::index(pc)];
        (e.pc == pc && e.chain && e.bridge.wrapping_sub(1) < BRIDGE_CLASS && e.n as u32 <= room && Self::valid(e, pv)).then_some((e.start, e.n as u32))
    }
    /// alias-s1: a region copy cut at `pc`, `k` instructions into the chunk headed at `head`, resumes
    /// in the head's decoded block, whose instruction `k` it is (same bytes: `find_block` checks the
    /// pages, and the arena range proves the same build, exactly as for a CUT).
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn name_resume(&mut self, head: u32, k: u32, pc: u32) {
        let ei = Self::index(head);
        let e = &self.entries[ei];
        if e.pc == head && k < e.n as u32 { self.resume = (ei as u32, e.start + k, pc); }
    }
    #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-tests"))]
    pub(crate) fn install_test_bridge(&mut self, pc: u32, ops: &[BlockInsn]) {
        let start = self.arena.len() as u32;
        self.arena.extend_from_slice(ops);
        let e = Entry { pc, start, n: ops.len() as u16, chain: !must_start_block(&ops[0].insn),
            bridge: bridge_block_class(ops), ..Entry::EMPTY };
        self.entries[Self::index(pc)] = e;
        assert_eq!(self.bridge_target(pc, &[0], ops.len() as u32), Some((start, ops.len() as u32)));
        assert!(self.bridge_target(pc, &[0], ops.len() as u32 - 1).is_none());
        self.entries[Self::index(pc)].chain = false;
        assert!(self.bridge_target(pc, &[0], ops.len() as u32).is_none());
        self.entries[Self::index(pc)].chain = e.chain;
        assert!(self.bridge_target(pc, &[1], ops.len() as u32).is_none());
    }
    #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-tests"))]
    pub(crate) fn test_code(&self) -> &crate::jit::CodeCache { self.code.as_ref().unwrap() }
    pub fn jit_active(&self) -> bool { self.jit_enabled && self.code.is_some() }
    #[inline(always)]
    fn index(pc: u32) -> usize { ((pc >> 1) ^ (pc >> 16)) as usize & (ENTRIES - 1) }
    #[inline(always)]
    fn valid(e: &Entry, pv: &[u32]) -> bool {
        pv.get(e.vidx[0] as usize).copied().unwrap_or(0) == e.ver[0] && pv.get(e.vidx[1] as usize).copied().unwrap_or(0) == e.ver[1]
    }
}

impl Default for BlockCache { fn default() -> Self { Self::new() } }
/// A clone of a CPU starts with an empty cache; compiled code is tied to the original's arena.
impl Clone for BlockCache { fn clone(&self) -> Self { let mut b = Self::new(); b.jit_enabled = self.jit_enabled; b.observed = self.observed; b } }

/// The instruction ends a block: control transfer, or a change to interrupt/timer/window state
/// that the per-block checks depend on.
pub(crate) fn ends_block(i: &Insn) -> bool {
    use Op::*;
    match i.op {
        Ill | IllN | Break | BreakN | Syscall | Simcall | Waiti | Rsil | Isync | Excw
        | J | Jx | Call0 | Call4 | Call8 | Call12 | Callx0 | Callx4 | Callx8 | Callx12
        | Ret | RetN | Retw | RetwN | Rotw | Rfe | Rfue | Rfde | Rfwo | Rfwu | Rfi | Rfme
        | Beqz | Bnez | Bltz | Bgez | BeqzN | BnezN | Beqi | Bnei | Blti | Bgei | Bltui | Bgeui
        | Bnone | Beq | Blt | Bltu | Ball | Bbc | Bbci | Bany | Bne | Bge | Bgeu | Bnall | Bbs | Bbsi | Bf | Bt
        | Loop | Loopnez | Loopgtz | Wsr | Xsr => true,
        _ => i.len == 0,
    }
}

/// EX171: highest instruction class the chain loop interprets in place (0 disables the bridge).
#[cfg(target_arch = "wasm32")]
pub(crate) const BRIDGE_CLASS: u8 = 2;
/// EX171: an interpreted dispatch of a bridgeable block continues into the next block.
#[cfg(target_arch = "wasm32")]
pub(crate) const BRIDGE_INTERP: bool = true;

/// EX171: interpret the whole no-code block at arena `start` inside the EX153 chain loop. Its
/// instructions are class 1..=BRIDGE_CLASS: no memory access, no device, interrupt, timer or
/// waiting state, so a completed block leaves everything the dispatcher would look at unchanged,
/// exactly like a compiled END exit with no helper. Returns the wrapper result (retired count,
/// exit code); a trap leaves through the wrapper's own trap exits.
#[cfg(target_arch = "wasm32")]
pub(crate) fn bridge<B: Bus>(cpu: &mut Cpu, bus: &mut B, start: u32, n: u32) -> u32 {
    let mut done = 0u32;
    while done < n {
        let e = cpu.blocks.arena[(start + done) as usize];
        if let Some(t) = cpu.check_overflow(e.max_ar) { cpu.jit_trap = Some(t); cpu.blocks.bridged += done; return done | crate::jit::CODE_TRAP_PRE << 16; }
        let at = cpu.pc;
        bus.note_pc(at);
        let r = exec_insn(cpu, bus, &e.insn);
        done += 1;
        if let Err(t) = r { cpu.jit_trap = Some(t); cpu.blocks.bridged += done; return done | crate::jit::CODE_TRAP << 16; }
        // a hardware loop-back inside the block ends it, as it ends an interpreted dispatch
        if cpu.pc != at.wrapping_add(e.insn.len as u32) { break; }
    }
    cpu.blocks.bridged += done;
    done | crate::jit::CODE_END << 16
}

/// EX171 bridge class of an instruction the chain loop might interpret in place.
/// 1: register/branch work with no memory access and no dispatcher-visible state (what the emitter
/// compiles inline without a helper, plus its unsupported pure siblings); 2: returns, loop setup and
/// RSR of registers exact mid-dispatch (EX135); 3: memory (census only); 0: never.
#[cfg(target_arch = "wasm32")]
pub(crate) fn bridge_class(i: &Insn) -> u8 {
    use Op::*;
    match i.op {
        Nop | NopN | Rsync | Esync | Dsync | Memw | Extw
        | Movi | MoviN | Mov | MovN | Add | AddN | Addi | AddiN | Addmi | Sub | Addx2 | Addx4 | Addx8 | Subx2 | Subx4 | Subx8
        | And | Or | Xor | Neg | Abs | Extui | Sext | Clamps | Min | Max | Minu | Maxu
        | Moveqz | Movnez | Movltz | Movgez | Movf | Movt
        | Slli | Srai | Srli | Sll | Srl | Sra | Src | Ssr | Ssl | Ssa8l | Ssa8b | Ssai | Nsa | Nsau
        | Mull | Muluh | Mulsh | Mul16u | Mul16s | Quou | Quos | Remu | Rems | Salt | Saltu
        | Andb | Andbc | Orb | Orbc | Xorb | Any4 | All4 | Any8 | All8
        | J | Jx | Call0 | Call4 | Call8 | Call12 | Callx0 | Callx4 | Callx8 | Callx12
        | Beqz | Bnez | Bltz | Bgez | BeqzN | BnezN | Beqi | Bnei | Blti | Bgei | Bltui | Bgeui
        | Bnone | Beq | Blt | Bltu | Ball | Bbc | Bbci | Bany | Bne | Bge | Bgeu | Bnall | Bbs | Bbsi | Bf | Bt => 1,
        Ret | RetN | Retw | RetwN | Loop | Loopnez | Loopgtz | Entry => 2,
        Rsr if crate::jit::exact_rsr(i.imm as u32) => 2,
        L8ui | L16ui | L16si | L32i | L32iN | L32r | S8i | S16i | S32i | S32iN | L32e | S32e | S32c1i | Lsi | Ssi => 3,
        _ => 0,
    }
}
/// Class of a whole block: 0 if any instruction is 0, else the highest class.
#[cfg(target_arch = "wasm32")]
pub(crate) fn bridge_block_class(ops: &[BlockInsn]) -> u8 {
    ops.iter().map(|b| bridge_class(&b.insn)).try_fold(0u8, |m, c| (c != 0).then_some(m.max(c))).unwrap_or(0)
}

/// The instruction must be the first of its block: it reads or writes state that is only exact
/// at a block boundary (`CCOUNT`, `CCOMPARE*`, `INTERRUPT`, `ICOUNT`).
pub(crate) fn must_start_block(i: &Insn) -> bool {
    // EX135: PS and INTENABLE change only through instructions that end a block (or a trap), so a
    // read of them is exact anywhere; a write ends its block and needs no boundary in front.
    matches!(i.op, Op::Rsr | Op::Wsr | Op::Xsr)
        && matches!(i.imm as u32, sr::CCOUNT | sr::INTERRUPT | sr::INTCLEAR | sr::ICOUNT | 240..=242)
}

/// Decode a block starting at `pc0` and register it. Only the first fetch can fault: a later
/// unmapped instruction simply ends the block and faults when it is reached as a block start.
fn build<B: Bus>(cpu: &mut Cpu, bus: &mut B, pc0: u32) -> Result<(u32, u32, u16), Trap> {
    match decode_block(cpu, bus, pc0) {
        Some(b) => Ok(b),
        None => Err(cpu.raise_mem(crate::state::exc::IFETCH_ERROR, pc0)),
    }
}

/// `build` without the exception: `None` when the first fetch faults, before anything changes
/// (review T1: speculative head recovery must not enter an exception for a PC it is not at).
fn decode_block<B: Bus>(cpu: &mut Cpu, bus: &mut B, pc0: u32) -> Option<(u32, u32, u16)> {
    let mut bytes = bus.fetch(pc0).ok()?;
    #[cfg(all(
        target_arch = "aarch64",
        any(target_os = "macos", target_os = "linux")
    ))]
    let code_short = cpu.blocks.jit_active() && cpu.blocks.code.as_ref().unwrap().remaining() < crate::jit::MAX_BLOCK_CODE;
    #[cfg(not(all(
        target_arch = "aarch64",
        any(target_os = "macos", target_os = "linux")
    )))]
    let code_short = false;
    if cpu.blocks.arena.len() + MAX_LEN > ARENA_MAX || code_short { cpu.blocks.flush(); }
    let start = cpu.blocks.arena.len() as u32;
    let (mut pc, mut n, mut last) = (pc0, 0u16, pc0);
    loop {
        let i = decode(pc, bytes);
        if n > 0 && (must_start_block(&i) || cpu.boundary_bloom & pc_bit(pc) != 0) { break; }
        cpu.blocks.arena.push(BlockInsn { insn: i, max_ar: max_ar(&i), straddle: cpu.price_control && crate::exec::static_target(&i).is_some_and(|t| crate::exec::straddles(bus, t)), off: 0 });
        n += 1; last = pc;
        pc = pc.wrapping_add(i.len as u32);
        if ends_block(&i) || n as usize == MAX_LEN { break; }
        match bus.fetch(pc) { Ok(b) => bytes = b, Err(_) => break }
    }
    cpu.blocks.extras.truncate(start as usize);
    if cpu.price_control {
        let extras = crate::exec::static_extras(cpu.blocks.arena[start as usize..].iter().map(|b| &b.insn));
        cpu.blocks.extras.extend(extras);
    } else {
        cpu.blocks.extras.resize(cpu.blocks.arena.len(), 0);
    }
    let last_byte = last.wrapping_add(cpu.blocks.arena[(start + n as u32 - 1) as usize].insn.len.max(1) as u32 - 1);
    let vidx0 = bus.code_page(pc0);
    let vidx1 = if last_byte >> 7 != pc0 >> 7 { bus.code_page(last_byte) } else { vidx0 };   // pages are >= 128 B
    // EX110: watch both pages before reading the versions this entry will compare against.
    bus.note_code_page(vidx0);
    if vidx1 != vidx0 { bus.note_code_page(vidx1); }
    let pv = bus.page_versions();
    let ver = [pv.get(vidx0 as usize).copied().unwrap_or(0), pv.get(vidx1 as usize).copied().unwrap_or(0)];
    let ei = BlockCache::index(pc0);
    let mut code = crate::jit::NONE;
    let fast = bus.fast_mem().is_some();
    if cpu.blocks.jit_active() {
        let b = &mut cpu.blocks;
        let (s, e) = (start as usize, start as usize + n as usize);
        if let Some(c) = crate::jit::compile(b.code.as_mut().unwrap(), &mut b.arena[s..e], pc0, fast) { code = c; b.compiled += 1; }
    }
    let chain = !must_start_block(&cpu.blocks.arena[start as usize].insn);
    // EX171: a block the emitter refused may still be interpretable inside a wrapper chain.
    #[cfg(target_arch = "wasm32")]
    let bridge = if code == crate::jit::NONE { bridge_block_class(&cpu.blocks.arena[start as usize..start as usize + n as usize]) } else { 0 };
    #[cfg(all(feature="native-short-call",target_arch="aarch64",any(target_os="macos",target_os="linux")))]
    let short=u8::from(cpu.blocks.arena[(start+u32::from(n)-1) as usize].insn.op==Op::Call8);
    cpu.blocks.entries[ei] = Entry { pc: pc0, start, n, chain, #[cfg(target_arch = "wasm32")] bridge,
        #[cfg(all(feature="native-short-call",target_arch="aarch64",any(target_os="macos",target_os="linux")))] short,
        vidx: [vidx0, vidx1], ver, code };
    cpu.blocks.builds += 1;
    Some((ei as u32, start, n))
}

/// Run a block (or a cut continuation) at `cpu.pc`, at most `budget` instructions.
/// WASM may repeat an admitted hardware-loop prefix within that same budget.
/// Returns `(iterations, trap)` where iterations is what a loop over `step()` would have
/// consumed: executed instructions, plus one for a trap taken before an instruction ran.
pub fn run_block<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32) -> (u32, Option<Trap>) {
    let result = run_block_profiled(cpu, bus, budget);
    // Trap entry is independent of instruction retirement and profiling.
    if cpu.price_control && matches!(result.1, Some(Trap::Exception(_) | Trap::Interrupt(_))) { cpu.timing_extra += 6; }
    result
}

fn run_block_profiled<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32) -> (u32, Option<Trap>) {
    #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-profile"))]
    {
        if cpu.blocks.profile.sample() {
            let pc = cpu.pc;
            cpu.blocks.profile_entry = if cpu.blocks.resume.2 == pc { cpu.blocks.resume.0 as usize } else { BlockCache::index(pc) };
            let before = cpu.blocks.jit_instructions;
            let start = crate::jit::profile::now();
            let result = run_block_inner(cpu, bus, budget);
            let elapsed = crate::jit::profile::now() - start;
            let e = cpu.blocks.entries[cpu.blocks.profile_entry];
            let ops = &cpu.blocks.arena[e.start as usize..(e.start + e.n as u32) as usize];
            let fast = bus.fast_mem().is_some();
            // Attribute resumed execution to its decoder block head, matching JIT names.
            cpu.blocks.profile.record(e.pc, before != cpu.blocks.jit_instructions, result.0, elapsed, ops, fast);
            return result;
        }
    }
    run_block_inner(cpu, bus, budget)
}

// Keep this boundary visible to a sampling profiler without adding per-block clocks.
#[cfg_attr(all(target_arch = "wasm32", feature = "wasm-cpu-profile"), inline(never))]
fn run_block_inner<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32) -> (u32, Option<Trap>) {
    // memo-s2: the common no-interrupt case without the call and its returned Option
    if cpu.check_interrupts_pending() != 0 { if let Some(t) = cpu.check_interrupts() { return (1, Some(t)); } }
    if cpu.waiting { cpu.advance_ccount(cpu.approximate_cpi); return (1, None); }
    #[cfg(target_arch = "wasm32")]
    {
        if cpu.blocks.memo.0 == cpu.pc {
            if let Some(r) = resume(cpu, bus, budget) { return r; }
            memo_hints(cpu);
        }
        // lane-s1: only the dispatch right after the exit may use it
        cpu.blocks.memo.0 = 1;
    }
    let (ei, k, end) = match find_block(cpu, bus) { Ok(b) => b, Err(t) => return (1, Some(t)) };
    #[cfg(all(target_arch = "wasm32", feature = "wasm-jit-profile"))]
    { cpu.blocks.profile_entry = ei as usize; }
    cpu.blocks.resume.2 = 1;
    cpu.blocks.alias_pc = 1;

    run_decoded(cpu, bus, budget, ei, k, end)
}

/// A store may bump a code-page version without changing this block's instructions.
/// Preserve its dependency history across a budget/step cut when the bytes still match.
/// Changed instructions still force decoding and compilation through the normal path.
fn refresh_priced_continuation<B: Bus>(cpu: &mut Cpu, bus: &mut B, ei: u32) {
    let e = &cpu.blocks.entries[ei as usize];
    if !cpu.price_control || e.pc == 1 || BlockCache::valid(e, bus.page_versions()) { return; }
    let mut pc = e.pc;
    for cached in &cpu.blocks.arena[e.start as usize..(e.start + e.n as u32) as usize] {
        let Ok(bytes) = bus.fetch(pc) else { return; };
        if decode(pc, bytes) != cached.insn { return; }
        pc = pc.wrapping_add(cached.insn.len as u32);
    }
    let indices = [bus.code_page(e.pc), bus.code_page(pc.wrapping_sub(1))];
    if indices != e.vidx { return; }
    let pv = bus.page_versions();
    cpu.blocks.entries[ei as usize].ver = indices.map(|i| pv.get(i as usize).copied().unwrap_or(0));
}

/// EX172: an aliasable arrival missed the entry table. If `pc` is an instruction
/// boundary strictly inside a valid decoded block that has code, run that block from there: the
/// same thing a budget cut and its resume do. Block boundaries are not architectural (interrupt,
/// timer and device state only change at instructions that end every block containing them).
#[cold]
#[inline(never)]
fn alias_lookup(cpu: &mut Cpu, pv: &[u32], pc: u32) -> Option<(u32, u32, u32)> {
    let b = &mut cpu.blocks;
    let slot = BlockCache::index(pc) & (ALIASES - 1);
    let (apc, head, start, k) = b.aliases[slot];
    if apc == pc {
        let ei = BlockCache::index(head);
        let e = &b.entries[ei];
        if e.pc == head && e.start == start && e.code != crate::jit::NONE && BlockCache::valid(e, pv) {
            #[cfg(feature = "wasm-jit-tests")]
            { b.alias_hits += 1; }
            return Some((ei as u32, k, e.start + e.n as u32));
        }
    }
    for back in 1..=(3 * (MAX_LEN as u32 - 1)) {
        let head = pc.wrapping_sub(back);
        let ei = BlockCache::index(head);
        let e = &b.entries[ei];
        if e.pc != head || e.code == crate::jit::NONE || !BlockCache::valid(e, pv) { continue; }
        let mut p = head;
        for k in e.start..e.start + e.n as u32 {
            if p == pc {
                let hit = (ei as u32, k, e.start + e.n as u32);
                b.aliases[slot] = (pc, head, e.start, k);
                #[cfg(feature = "wasm-jit-tests")]
                { b.alias_hits += 1; }
                return Some(hit);
            }
            if p.wrapping_sub(head) > back { break; }
            p = p.wrapping_add(b.arena[k as usize].insn.len as u32);
        }
    }
    None
}

/// tails-s1: after a guarded region copy cut mid-chunk, decode the chunk's head block (the one the
/// own-module tail cut it replaces would have run) and resume inside it, instead of a new head here.
#[cold]
#[inline(never)]
fn head_lookup<B: Bus>(cpu: &mut Cpu, bus: &mut B, pc: u32) -> Option<(u32, u32, u32)> {
    // One use per hint; flush forgets it with the decoded blocks it was recorded against.
    let (at, head) = std::mem::replace(&mut cpu.blocks.alias_head, (1, 1));
    if at != pc || head == pc { return None; }
    let (ei, start, n) = decode_block(cpu, bus, head)?;
    // Walk the block just built, not `alias_lookup`'s 93-byte scan: a cut late in a chunk of
    // four-byte PIE instructions lies up to 124 bytes past its head (review T2).
    let b = &mut cpu.blocks;
    if b.entries[ei as usize].code == crate::jit::NONE { return None; }
    let mut p = head;
    for k in start..start + n as u32 {
        if p == pc {
            b.aliases[BlockCache::index(pc) & (ALIASES - 1)] = (pc, head, start, k);
            #[cfg(feature = "wasm-jit-tests")]
            { b.alias_hits += 1; }
            return Some((ei, k, start + n as u32));
        }
        p = p.wrapping_add(b.arena[k as usize].insn.len as u32);
    }
    None
}

#[cfg_attr(not(target_arch = "wasm32"), inline(always))]
fn find_block<B: Bus>(cpu: &mut Cpu, bus: &mut B) -> Result<(u32, u32, u32), Trap> {
    let pc = cpu.pc;
    Ok({
        let (rei, rk, rpc) = cpu.blocks.resume;
        let resumed = if rpc == pc {
            refresh_priced_continuation(cpu, bus, rei);
            let e = &cpu.blocks.entries[rei as usize];
            (e.pc != 1 && BlockCache::valid(e, bus.page_versions()) && rk >= e.start && rk < e.start + e.n as u32)
                .then_some((rei, rk, e.start + e.n as u32))
        } else { None };
        if let Some(hit) = resumed { hit } else {
            let ei = BlockCache::index(pc);
            let e = &cpu.blocks.entries[ei];
            if e.pc == pc && BlockCache::valid(e, bus.page_versions()) { (ei as u32, e.start, e.start + e.n as u32) }
            else if let Some(hit) = (ALIAS && cpu.blocks.alias_pc == pc && !cpu.price_control && !cpu.blocks.observed
                && cpu.boundary_bloom & pc_bit(pc) == 0).then(|| alias_lookup(cpu, bus.page_versions(), pc).or_else(|| head_lookup(cpu, bus, pc))).flatten() { hit }
            else { let (ei, s, n) = build(cpu, bus, pc)?; (ei, s, s + n as u32) }
        }
    })
}

/// Share decoded dependency prices and continuation state with single stepping.
/// The model intentionally resets scoreboards at decoded block boundaries, not budget cuts.
pub(crate) fn step_extra<B: Bus>(cpu: &mut Cpu, bus: &mut B, i: &Insn) -> u32 {
    let Ok((ei, k, end)) = find_block(cpu, bus) else { return 0; };
    cpu.blocks.resume = if k + 1 < end { (ei, k + 1, cpu.pc.wrapping_add(i.len as u32)) } else { (0, 0, 1) };
    cpu.blocks.extras[k as usize] as u32
}

// Only WASM can continue into another block here. Keep the native return path direct.
#[cfg(not(target_arch = "wasm32"))]
use self::run_decoded_once as run_decoded;
#[cfg(not(target_arch = "wasm32"))]
type DecodedResult = (u32, Option<Trap>);
#[cfg(target_arch = "wasm32")]
type DecodedResult = (u32, Option<Trap>, Option<(u32, u32, u32)>);

#[cfg(target_arch = "wasm32")]
fn run_decoded<B: Bus>(cpu: &mut Cpu, bus: &mut B, mut budget: u32, mut ei: u32, mut k: u32, mut end: u32) -> (u32, Option<Trap>) {
    let mut total = 0;
    loop {
        let (done, trap, next) = run_decoded_once(cpu, bus, budget, ei, k, end);
        total += done;
        let Some(next) = next else { return (total, trap) };
        budget -= done;
        (ei, k, end) = next;
    }
}

/// Never run past a CCOMPARE match: the timer interrupt must land on the same instruction.
#[inline(always)]
fn timer_limit(cpu: &mut Cpu, mut limit: u32) -> u32 {
    #[cfg(not(target_arch = "wasm32"))]
    for i in 0..3 {
        let d = cpu.ccompare[i].wrapping_sub(cpu.ccount);
        let d = if cpu.approximate_cpi == 1 { d } else { d.div_ceil(cpu.approximate_cpi) };
        if d != 0 && d < limit { limit = d; }
    }
    // event-s1: the nearest comparator bounds the others (div_ceil is monotonic), so one distance.
    #[cfg(target_arch = "wasm32")]
    {
        #[cfg(feature = "wasm-jit-tests")]
        { let kept = cpu.event_at; cpu.refresh_event(); assert_eq!(kept, cpu.event_at, "ccount/ccompare changed without refresh_event"); }
        let d = cpu.event_at.wrapping_sub(cpu.ccount).wrapping_add(1);
        let d = if cpu.approximate_cpi == 1 { d } else { d.div_ceil(cpu.approximate_cpi) };
        if d != 0 && d < limit { limit = d; }
    }
    limit
}

/// lane-s1: a dispatch at the PC where this core's last quantum-ending region exit left goes
/// straight into that region (`jit::resume`), skipping the lookup and the Hot re-derivation.
/// Pricing keeps the ordinary path.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
fn resume<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32) -> Option<(u32, Option<Trap>)> {
    if cpu.price_control || cpu.blocks.observed || !cpu.blocks.jit_enabled { return None; }
    let limit = timer_limit(cpu, budget.min(0xffff));
    let fm = bus.fast_mem();
    let cache = cpu.blocks.code.take()?;
    // SAFETY: helpers match B, `fm` describes this exclusive bus borrow, and the cache is owned here.
    let r = unsafe { crate::jit::resume(&cache, cpu, bus, crate::jit::Helpers::shared::<B>(), limit, fm) };
    cpu.blocks.code = Some(cache);
    let r = r?;
    // a region never cuts; a chained own module that cuts names its entry in `chain_ei`
    Some(retire(cpu, bus, r, 0, 0, 0))
}

/// lane-s2b: the memo'd dispatch for a round loop, with run_block_inner's checks in front of it
/// (interrupts, WAITI, the PC); a decline leaves the ordinary dispatch exactly as run_block_inner
/// would (hints written, memo consumed). One out-of-line copy of the memo path.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn run_memo<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32) -> Option<(u32, Option<Trap>)> {
    if cpu.blocks.memo.0 != cpu.pc { return None; }
    run_memo_hit(cpu, bus, budget)
}
#[cfg(target_arch = "wasm32")]
#[inline(never)]
fn run_memo_hit<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32) -> Option<(u32, Option<Trap>)> {
    if cpu.check_interrupts_pending() != 0 || cpu.waiting { return None; }
    let r = resume(cpu, bus, budget);
    if r.is_none() { memo_hints(cpu); cpu.blocks.memo.0 = 1; }
    r
}

/// Account a compiled call's result: retired instructions, the refused device access, traps and
/// the continuation of a cut.
#[inline(always)]
#[allow(unused_variables)]
fn retire<B: Bus>(cpu: &mut Cpu, bus: &mut B, r: u32, ei: u32, k: u32, end: u32) -> (u32, Option<Trap>) {
    let (mut done, exit) = (r & 0xffff, (r >> 16) & 7);
    // EX133: the helper refused a device-register access; its instruction was counted
    // but did not run, and the pc still names it.
    if exit == crate::jit::CODE_TRAP && bus.deferred() {
        done -= 1;
        // EX172 s3: the refused instruction is dispatched again, usually from mid-block.
        if ALIAS && ALIAS_SEQ { cpu.blocks.alias_pc = cpu.pc; }
    }

    // EX171: instructions the chain loop interpreted in place are not compiled instructions.
    #[cfg(target_arch = "wasm32")]
    { cpu.blocks.jit_instructions += (done - cpu.blocks.bridged.min(done)) as u64; }
    #[cfg(not(target_arch = "wasm32"))]
    { cpu.blocks.jit_instructions += done as u64; }
    cpu.insn_count += done as u64;
    cpu.advance_ccount(done * cpu.approximate_cpi);
    match exit {
        crate::jit::CODE_TRAP => (done, cpu.jit_trap.take()),
        crate::jit::CODE_TRAP_PRE => (done + 1, cpu.jit_trap.take()),
        crate::jit::CODE_CUT => {
            #[cfg(target_arch = "wasm32")]
            {
                // The WASM wrapper accounts for repeated prefixes when returning
                // the next decoded index; no PC scan is needed here.
                let (ei, end) = if cpu.blocks.chain_ei != u32::MAX {
                    let e = &cpu.blocks.entries[cpu.blocks.chain_ei as usize];
                    (cpu.blocks.chain_ei, e.start + e.n as u32)
                } else { (ei, end) };
                let index = cpu.blocks.entries[ei as usize].start + (r >> 19);
                if index < end { cpu.blocks.resume = (ei, index, cpu.pc); }
            }
            #[cfg(not(target_arch = "wasm32"))]
            if k + done < end { cpu.blocks.resume = (ei, k + done, cpu.pc); }
            (done, None)
        }
        _ => (done, None),
    }
}

/// One block (or WASM wrapper chain). WASM also returns the next block to continue with.
#[inline(always)]
fn run_decoded_once<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32, ei: u32, mut k: u32, end: u32) -> DecodedResult {
    #[cfg(all(feature="native-short-call",target_arch="aarch64",any(target_os="macos",target_os="linux")))]
    if cpu.blocks.entries[ei as usize].short==1 {native_short_call::try_fuse(cpu,bus,ei as usize);}
    #[cfg(not(target_arch = "wasm32"))]
    let limit = {
        let room=(end-k).min(budget);
        #[cfg(all(feature="native-short-call",target_arch="aarch64",any(target_os="macos",target_os="linux")))]
        let room=if cpu.blocks.entries[ei as usize].short==2 {budget.min(0xffff)} else {room};
        timer_limit(cpu,room)
    };
    // WASM code bounds itself by its block: pass the round's remaining credit so a retained
    // loop or a region may continue past the block, under the same CCOMPARE deadline.
    #[cfg(target_arch = "wasm32")]
    let mut limit = timer_limit(cpu, budget.min(0xffff));

    let code = cpu.blocks.entries[ei as usize].code;
    // Native code has no price collector or deferred-access guard. Keep its default
    // fast path, but execute opt-in pricing and deferred quanta in the interpreter.
    // WASM records exact instruction ranges, up to 64 per compiled call.
    #[cfg(target_arch = "wasm32")]
    if cpu.price_control && cpu.icache_fill != 0 { limit = limit.min(64); }
    let observed_timing = cpu.price_control && cfg!(not(target_arch = "wasm32"));
    let native_deferred = cfg!(not(target_arch = "wasm32")) && bus.defer_armed();
    if !observed_timing && !native_deferred && code != crate::jit::NONE && cpu.blocks.jit_enabled && crate::jit::ready(cpu.blocks.code.as_ref().unwrap(), code, cpu.lend) {
        let entry = cpu.blocks.arena[k as usize].off;
        let fm = bus.fast_mem();
        #[cfg(not(target_arch = "wasm32"))]
        let r = {
            // Copy the executable pointer, ending the cache borrow before borrowing Cpu.
            // Native execution needs no cache metadata while generated code is running.
            let target = cpu.blocks.code.as_ref().unwrap().entry_point(code);
            let helpers = crate::jit::Helpers::shared::<B>();
            // SAFETY: `code` and `entry` identify published code in this CPU's live cache.
            // Neither generated code nor its interpreter helpers reset or compile the cache.
            // The arena stays unmoved, and fallback helpers copy instructions before borrowing
            // Cpu. Helpers are static for B; `fm` describes this exclusive bus borrow.
            unsafe { crate::jit::run(target, cpu, bus, helpers, limit, entry, fm) }
        };
        #[cfg(target_arch = "wasm32")]
        let r = {
            // WASM needs retained block metadata during execution, so move its owning cache
            // outside Cpu before holding that shared reference alongside the exclusive CPU.
            let cache = cpu.blocks.code.take().unwrap();
            let helpers = crate::jit::Helpers::shared::<B>();
            // SAFETY: `code` and `entry` identify live code in this locally owned cache;
            // helpers match B and `fm` describes this exclusive bus borrow.
            let r = unsafe { crate::jit::run(&cache, code, cpu, bus, helpers, limit, entry, fm) };
            cpu.blocks.code = Some(cache);
            r
        };
        let (done, trap) = retire(cpu, bus, r, ei, k, end);
        #[cfg(target_arch = "wasm32")]
        return (done, trap, None);
        #[cfg(not(target_arch = "wasm32"))]
        return (done, trap);
    }

    #[cfg(target_arch = "wasm32")]
    let room = limit;
    let limit = limit.min(end - k);
    #[cfg(feature = "wasm-jit-profile")]
    let (census_core, census_why) = {
        let core = crate::census::core(cpu);
        let en = &cpu.blocks.entries[ei as usize];
        let ops = &cpu.blocks.arena[en.start as usize..(en.start + en.n as u32) as usize];
        let why = crate::census::fallback(core, en.pc, ops, bus.fast_mem().is_some(), code, cpu.blocks.jit_enabled);
        (core, why)
    };
    let (mut done, mut trap, mut pre, mut broke) = (0u32, None, false, false);
    let mut seq = false;
    while done < limit {
        let e = cpu.blocks.arena[k as usize];
        #[cfg(feature = "wasm-jit-profile")]
        {
            let mut c = crate::census::get();
            if let Some(total) = c.interp_total.get_mut(census_core as usize) { *total += 1; }
            *c.interp.entry((census_core, crate::census::name(&e.insn), census_why.0.clone())).or_default() += 1;
            *c.blockers.entry((census_core, census_why.1.clone())).or_default() += 1;
        }
        if let Some(t) = cpu.check_overflow(e.max_ar) { trap = Some(t); pre = true; break; }
        let at = cpu.pc;
        if crate::exec::defer_instruction(cpu, bus, &e.insn) { break; }
        bus.note_pc(at);
        if cpu.price_control && cpu.icache_fill != 0 {
            cpu.touch_fetch_lines(at, at.wrapping_add(e.insn.len.max(1) as u32 - 1));
        }
        let expected = at.wrapping_add(e.insn.len as u32);
        let r = exec_insn(cpu, bus, &e.insn);
        seq = cpu.pc == expected;
        done += 1; k += 1;
        if cpu.price_control && r.is_ok() {
            let taken = crate::exec::control_taken(cpu, &e.insn);
            cpu.timing_extra += crate::exec::control_price(e.insn.op, taken) + cpu.blocks.extras[k as usize - 1] as u32
                + u32::from(taken && crate::exec::transfers(e.insn.op) && crate::exec::straddles(bus, cpu.pc));
        }
        if let Err(t) = r { trap = Some(t); break; }
        if cpu.pc != expected || bus.block_break() { broke = true; break; }
    }
    cpu.insn_count += done as u64;
    cpu.advance_ccount(done * cpu.approximate_cpi);
    if ALIAS && ALIAS_SEQ && trap.is_none() && seq && k < end { cpu.blocks.alias_pc = cpu.pc; }
    // cut short by the budget or a timer deadline while still inside the block: resume there
    if trap.is_none() && !broke && k < end { cpu.blocks.resume = (ei, k, cpu.pc); }
    // EX171 s3: a completed block of pure register/branch work changed nothing the dispatcher would look at
    // (no interrupt input, device, waiting or timer state; credit and the CCOMPARE deadline remain), so the
    // next block starts right here, under a freshly derived deadline, as the next dispatch would.
    #[cfg(target_arch = "wasm32")]
    if BRIDGE_INTERP && trap.is_none() && k == end && done < room && !cpu.price_control && !cpu.blocks.observed
        && cpu.blocks.entries[ei as usize].bridge.wrapping_sub(1) < BRIDGE_CLASS
        && k - done == cpu.blocks.entries[ei as usize].start
        // like EX153, a dispatch at a probed PC stays one block long
        && !bus.block_break() && cpu.boundary_bloom & (pc_bit(cpu.pc) | pc_bit(cpu.blocks.entries[ei as usize].pc)) == 0
    {
        let nei = BlockCache::index(cpu.pc);
        let next = &cpu.blocks.entries[nei];
        if next.pc == cpu.pc && BlockCache::valid(next, bus.page_versions()) && !must_start_block(&cpu.blocks.arena[next.start as usize].insn) {
            return (done, None, Some((nei as u32, next.start, next.start + next.n as u32)));
        }
    }
    #[cfg(target_arch = "wasm32")]
    { (done + pre as u32, trap, None) }
    #[cfg(not(target_arch = "wasm32"))]
    { (done + pre as u32, trap) }
}

#[cfg(any(test, feature = "wasm-jit-tests"))]
pub(crate) mod ownership_tests {
    use super::*;
    use crate::bus::{FastMem, TlbEntry, TLB_ENTRIES};
    use crate::{Fault, FlatRam};

    // Distinct Bus types with deliberately identical layout: the regression observes wrong
    // helper selection without relying on a layout mismatch or invalid memory access.
    #[repr(transparent)]
    struct TaggedBus<const VALUE: u32>(FlatRam);
    impl<const VALUE: u32> Bus for TaggedBus<VALUE> {
        fn note_code_page(&mut self, _vidx: u32) {} // All writes already update versions, or this bus has no decode cache.
        fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.0.read8(a) }
        fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.0.read16(a) }
        fn read32(&mut self, _: u32) -> Result<u32, Fault> { Ok(VALUE) }
        fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.0.write8(a, v) }
        fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.0.write16(a, v) }
        fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.0.write32(a, v) }
        fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> { self.0.fetch(pc) }
        fn page_versions(&self) -> &[u32] { self.0.page_versions() }
        fn fast_mem(&mut self) -> Option<FastMem> {
            // Admit compiled loads but force their slow helper path.
            static TLB: [TlbEntry; TLB_ENTRIES] = [TlbEntry::EMPTY; TLB_ENTRIES];
            Some(FastMem { tlb: TLB.as_ptr(), page_ver: &mut self.0.ver })
        }
    }
    fn bus<const VALUE: u32>() -> TaggedBus<VALUE> {
        let mut ram = FlatRam::new(0x4037_0000, 64);
        // l32i.n a3,a4,0; j self
        ram.mem[..5].copy_from_slice(&[0x38, 0x04, 0x06, 0xff, 0xff]);
        TaggedBus(ram)
    }

    #[cfg_attr(test, test)]
    pub(crate) fn compiled_helpers_follow_the_current_bus_type() {
        let mut cpu = Cpu::new(0);
        cpu.ps = 0;
        let mut first = bus::<11>();
        let mut second = bus::<22>();
        // A WASM slow load exits immediately; limit both backends to that instruction.
        for _ in 0..40 {
            cpu.pc = first.0.base;
            assert_eq!(run_block(&mut cpu, &mut first, 1), (1, None));
            assert_eq!(cpu.get_ar(3), 11);
        }
        let compiled = cpu.blocks.jit_instructions;
        cpu.pc = second.0.base;
        assert_eq!(run_block(&mut cpu, &mut second, 1), (1, None));
        assert_eq!(cpu.get_ar(3), 22);
        if crate::jit::AVAILABLE {
            assert!(compiled > 0, "must exercise compiled helpers");
            assert_eq!(cpu.blocks.jit_instructions, compiled + 1);
        }
        // Repeated switches reuse the compiled block but must select each bus's helpers.
        for _ in 0..4 {
            cpu.pc = first.0.base;
            assert_eq!(run_block(&mut cpu, &mut first, 1), (1, None));
            assert_eq!(cpu.get_ar(3), 11);
            cpu.pc = second.0.base;
            assert_eq!(run_block(&mut cpu, &mut second, 1), (1, None));
            assert_eq!(cpu.get_ar(3), 22);
        }
        if crate::jit::AVAILABLE {
            assert_eq!(cpu.blocks.jit_instructions, compiled + 9);
            assert!(cpu.blocks.code.is_some(), "execution must restore cache ownership");
        }
        // Execution must restore cache ownership for subsequent invalidation and reuse.
        cpu.blocks.flush();
        cpu.pc = first.0.base;
        assert_eq!(run_block(&mut cpu, &mut first, 1), (1, None));
        assert_eq!(cpu.get_ar(3), 11);
    }
}
