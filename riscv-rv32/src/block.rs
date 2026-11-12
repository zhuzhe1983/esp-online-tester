//! Decoded basic blocks (EX265): the RV32 counterpart of the Xtensa block cache (EX001). A
//! block is up to `MAX_LEN` instructions decoded once, read ahead with `Bus::fetch_ahead`, and
//! kept while the bus's code versions (`Bus::page_versions`) of the pages its bytes lie on stay
//! put. Running one skips the per-instruction fetch, decode and interrupt test of `exec::step`;
//! each instruction still goes through `exec::exec_insn`'s body, the interpreter's own execution.
//!
//! Exactness against a loop over `step` (the oracle, `--no-jit`):
//! - The interrupt test is made before a block, with the predicate `step_outcome` applies. Only
//!   a CSR access, `mret` or a trap can change its inputs (mstatus.MIE, the CLIC level and
//!   threshold, mtvec's mode), and each of them ends a block; the SoC changes `irq` only between
//!   `run`s. A core that is waiting, or would take an interrupt, takes the interpreter's step.
//! - Every instruction counts, notes its pc on the bus and retires exactly as `step_outcome`
//!   does, hardware-loop back-edge included (`hwlp::retire`, which leaves the block when it
//!   jumps); a trap mid-block returns at once with the iterations used, `mepc` precise.
//! - `block_break` is tested after every instruction, as `Core::run` does; budget cuts end a
//!   block early; a block never runs past a pc in the machine's boundary bloom (`set_boundaries`
//!   flushes when the bloom changes, and building stops in front of one).
//! - A store can change code, its own block's included: after any instruction that may write,
//!   the block's versions are compared again and the rest of it is dropped if they moved.
//! - Reading ahead has no side effects (`fetch_ahead`), and a block is only built where it says
//!   the fetch succeeds; the first instruction of an unreadable pc is the interpreter's step,
//!   faults and all.
use crate::bus::Bus;
use crate::decode::{decode_for, Comp, Insn, Op};
use crate::exec::{exec_insn_inline, step_outcome, Trap};
use crate::state::Cpu;
use emu_core::core::pc_bit;

/// Instructions per block. A block spans at most two version pages (pages are >= 256 bytes and
/// a block at most 128), checked while building.
pub const MAX_LEN: usize = 32;
/// Direct-mapped block table, indexed by halfword pc.
const ENTRIES: usize = 1 << 16;
/// Decoded instructions kept before everything is thrown away and rebuilt on demand. A new block
/// reuses the slot of the one it evicts when that is long enough, so the arena stays below
/// `ENTRIES * MAX_LEN` and this is only a backstop.
const ARENA_CAP: usize = ENTRIES * MAX_LEN;
/// No version: this pc is not cacheable.
const NO_PAGE: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Entry {
    pc: u32,
    start: u32,
    n: u8,
    /// arena slots this entry owns from `start` (>= n)
    cap: u8,
    /// bit k: instruction k may write memory (then the versions are compared again)
    writes: u32,
    /// it ends in a branch or jump, or no terminator at all: when it lands on its own start the
    /// interrupt test has nothing new to decide (`run`'s `again`)
    plain_end: bool,
    vidx: [u32; 2],
    ver: [u32; 2],
}
const EMPTY: Entry = Entry { pc: 1, start: 0, n: 0, cap: 0, writes: 0, plain_end: false, vidx: [0; 2], ver: [0; 2] };

/// The core's decoded-block cache. Empty until used: `Cpu::new` stays cheap.
pub struct Blocks {
    /// `Core::set_jit`: false runs the interpreter only
    pub on: bool,
    entries: Vec<Entry>,
    arena: Vec<Insn>,
    /// the ISA word the arena was decoded for
    isa: u32,
    pub builds: u64,
    pub flushes: u64,
}

impl Default for Blocks {
    fn default() -> Self { Blocks { on: true, entries: Vec::new(), arena: Vec::new(), isa: 0, builds: 0, flushes: 0 } }
}

impl Blocks {
    /// Forget every block (a reset, new boundaries, `--no-jit`).
    pub fn flush(&mut self) {
        if !self.entries.is_empty() { self.entries.iter_mut().for_each(|e| *e = EMPTY); self.flushes += 1; }
        self.arena.clear();
    }
}

/// Instructions after which the rest of a block may be stale: anything that can store.
#[inline(always)]
fn may_write(op: Op) -> bool {
    use Op::*;
    matches!(op, Sb | Sh | Sw | ScW | AmoSwapW | AmoAddW | AmoXorW | AmoAndW | AmoOrW | AmoMinW | AmoMaxW | AmoMinuW | AmoMaxuW | CmPush | Fsw) || op.is_pie()
}

/// Instructions that end a block: control transfers, and everything that can change what the
/// interrupt test before the next instruction decides (CSR writes, `mret`, `wfi`, traps). A CSR
/// read (`csrrs`/`csrrc` from x0, or with a zero immediate) writes nothing (`exec_insn`) and
/// `read_csr` has no side effects, so a delay loop that polls the cycle counter stays one block.
#[inline(always)]
fn ends_block(i: &Insn) -> bool {
    use Op::*;
    match i.op {
        Csrrs | Csrrc | Csrrsi | Csrrci => i.rs1 != 0,
        Jal | Jalr | Beq | Bne | Blt | Bge | Bltu | Bgeu | Ecall | Ebreak | Mret | Wfi | FenceI | Illegal
        | Csrrw | Csrrwi | CmPopret | CmPopretz => true,
        _ => false,
    }
}

impl Cpu {
    /// Whether `step_outcome` would do something other than execute the instruction at pc:
    /// sleep, or take the interrupt the SoC presents.
    #[inline(always)]
    fn irq_acts(&self) -> bool {
        match self.irq {
            None => false,
            Some(line) => self.mie_enabled() && (!self.clic || (line >> crate::exec::CLIC_IRQ_LEVEL_SHIFT) & 0xff > self.mil.max(self.mintthresh)),
        }
    }
}

/// Look up, or build, the block at `pc`; `None` when there is none to run (the caller steps).
#[inline(always)]
fn lookup<B: Bus>(cpu: &mut Cpu, bus: &mut B, pc: u32) -> Option<Entry> {
    if cpu.blocks.entries.is_empty() { cpu.blocks.entries = vec![EMPTY; ENTRIES]; }
    let ei = (pc >> 1) as usize & (ENTRIES - 1);
    let e = cpu.blocks.entries[ei];
    if e.pc == pc {
        let pv = bus.page_versions();
        if pv.get(e.vidx[0] as usize) == Some(&e.ver[0]) && pv.get(e.vidx[1] as usize) == Some(&e.ver[1]) { return Some(e); }
    }
    build(cpu, bus, pc, ei)
}

#[cold]
#[inline(never)]
fn build<B: Bus>(cpu: &mut Cpu, bus: &mut B, pc: u32, ei: usize) -> Option<Entry> {
    let isa = cpu.isa();
    let b = &mut cpu.blocks;
    if b.isa != isa || b.arena.len() + MAX_LEN > ARENA_CAP { b.flush(); b.isa = isa; }
    let v0 = bus.code_page(pc);
    if v0 == NO_PAGE { return None; }
    let mut v1 = v0;
    const NOP: Insn = Insn { op: Op::Illegal, rd: 0, rs1: 0, rs2: 0, imm: 0, len: 4, comp: Comp::None, raw: 0 };
    let mut buf = [NOP; MAX_LEN];
    let mut n = 0;
    let mut writes = 0u32;
    let mut plain_end = true;
    let mut at = pc;
    for (k, slot) in buf.iter_mut().enumerate() {
        if k > 0 && cpu.boundary_bloom & pc_bit(at) != 0 { break; }
        let Some(bytes) = bus.fetch_ahead(at) else { break; };
        let i = decode_for(isa, at, bytes);
        let last = at.wrapping_add(i.len as u32 - 1);
        // both ends of the instruction must lie on the block's (at most two) pages
        let (pa, pb) = (bus.code_page(at), bus.code_page(last));
        if pa == NO_PAGE || pb == NO_PAGE { break; }
        let mut fits = true;
        for p in [pa, pb] {
            if p != v0 && p != v1 { if v1 == v0 { v1 = p; } else { fits = false; } }
        }
        if !fits { break; }
        if may_write(i.op) { writes |= 1 << k; }
        *slot = i;
        n = k + 1;
        if ends_block(&i) {
            plain_end = matches!(i.op, Op::Jal | Op::Jalr | Op::Beq | Op::Bne | Op::Blt | Op::Bge | Op::Bltu | Op::Bgeu | Op::CmPopret | Op::CmPopretz);
            break;
        }
        at = at.wrapping_add(i.len as u32);
    }
    if n == 0 { return None; }
    // EX110's order: watch the pages, then read the versions the block will compare.
    bus.note_code_page(v0);
    if v1 != v0 { bus.note_code_page(v1); }
    let pv = bus.page_versions();
    let (Some(&ver0), Some(&ver1)) = (pv.get(v0 as usize), pv.get(v1 as usize)) else { return None; };
    // the evicted block's slots, if they are enough; nothing else refers to them
    let old = b.entries[ei];
    let (start, cap) = if old.pc != EMPTY.pc && old.cap as usize >= n { (old.start as usize, old.cap) } else {
        let start = b.arena.len();
        b.arena.extend_from_slice(&buf[..n]);
        (start, n as u8)
    };
    b.arena[start..start + n].copy_from_slice(&buf[..n]);
    let e = Entry { pc, start: start as u32, n: n as u8, cap, writes, plain_end, vidx: [v0, v1], ver: [ver0, ver1] };
    b.entries[ei] = e;
    b.builds += 1;
    Some(e)
}

/// One instruction through the interpreter, with `Core::run`'s stop tests.
#[inline(always)]
fn step_one<B: Bus>(cpu: &mut Cpu, bus: &mut B) -> Result<bool, Trap> {
    step_outcome(cpu, bus).result()?;
    Ok(bus.block_break() || (cpu.boundary_bloom != 0 && cpu.boundary_bloom & pc_bit(cpu.pc) != 0))
}

/// `Core::run` with blocks: the same iterations, state and stops as a loop over `step`.
pub fn run<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u32) -> (u32, Option<Trap>) {
    let mut used = 0u32;
    // A block that ran whole, stored nothing and branched back to its own start is still valid
    // and the interrupt test still passes: nothing in it could change either.
    let mut again: Option<Entry> = None;
    while used < budget {
        let e = if again.is_some() { again.take() } else if cpu.waiting || cpu.irq_acts() { None } else { let pc = cpu.pc; lookup(cpu, bus, pc) };
        let Some(e) = e else {
            used += 1;
            match step_one(cpu, bus) {
                Err(t) => return (used, Some(t)),
                Ok(true) => return (used, None),
                Ok(false) => continue,
            }
        };
        let n = (e.n as u32).min(budget - used);
        let mut done = 0;
        while done < n {
            let i = cpu.blocks.arena[(e.start + done) as usize];
            let pc = cpu.pc;
            bus.note_pc(pc);
            cpu.insn_count += 1; cpu.cycle_count += 1;
            done += 1;
            if let Err(t) = exec_insn_inline(cpu, bus, &i, pc) { return (used + done, Some(t)); }
            cpu.retired_count += 1;
            // a hardware loop's back-edge after its last instruction, as `step_outcome` takes it;
            // the rest of the block is then not where execution goes
            let next = cpu.pc;
            crate::hwlp::retire(cpu, pc);
            let looped = cpu.pc != next;
            if bus.block_break() { return (used + done, None); }
            if looped { break; }
            if e.writes & (1 << (done - 1)) != 0 && done < n {
                let pv = bus.page_versions();
                // the rest of this block may be stale: carry on from a fresh lookup
                if pv.get(e.vidx[0] as usize) != Some(&e.ver[0]) || pv.get(e.vidx[1] as usize) != Some(&e.ver[1]) { break; }
            }
        }
        used += done;
        if cpu.boundary_bloom != 0 && cpu.boundary_bloom & pc_bit(cpu.pc) != 0 { return (used, None); }
        if done == e.n as u32 && e.writes == 0 && e.plain_end && cpu.pc == e.pc { again = Some(e); }
    }
    (budget, None)
}
