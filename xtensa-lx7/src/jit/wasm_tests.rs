//! Differential tests executed by tools/wasm-jit-test.mjs in an actual WASM runtime.
//! Constructed instructions exercise the emitter independently of encoding; the scheduler
//! suites use real encoded instructions and prove that hot dispatch actually happens.
use super::*;
use crate::bus::{tlb_index, TLB_ENTRIES};
use crate::{Fault, FlatRam, Insn, Op, Trap};
pub(super) static PS_INLINE_TAKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) static PS_REGION_TAKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) static RETW_INLINE_TAKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) static LEAF_RETURNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) static GUARDED_TAKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) static STORE_RUN_TAKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) static STORE_RUN_DONE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[path = "wasm_tests/reboot.rs"]
mod reboot;
#[path = "wasm_tests/pie_accx.rs"]
mod pie_accx;
const BASE: u32 = 0x4037_0000;
/// A small window above the fast mapping that only the slow bus path can reach.
const SLOW: u32 = BASE + 0x1_0000;
struct Ram {
    ram: FlatRam,
    fetch_fault: Option<u32>,
    versions: Vec<u32>,
    tlb: Vec<TlbEntry>,
    fast: bool,
    readonly: bool,
    /// EX110: does decoded code depend on this mapping? Cleared by `unwatch`, set again by
    /// `note_code_page`, and honored by `wrote` exactly as the generated store honors
    /// `TlbEntry.code`, so both paths must produce the same counters.
    watched: bool,
    /// store-s1: `note_code_page` leaves an unwatched mapping unwatched (a program that never
    /// writes its own code, standing in for data in another mapping).
    pinned: bool,
    noted: u32,
    slow: [u8; 256],
    slow_writes: u32,
    slow_reads: u32,
    defer_armed: bool,
    deferred: bool,
    #[cfg(feature = "wasm-cache-inline")]
    inline_cache: Option<(Vec<emu_core::bus::FastCacheLine>, u64)>,
    #[cfg(feature = "wasm-cache-inline")]
    helper_accesses: u32,
}
impl Ram {
    fn new(fast: bool, readonly: bool) -> Self {
        let mut ram = FlatRam::new(BASE, 65536);
        for (i, b) in ram.mem.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37);
        }
        let mut tlb = vec![TlbEntry::EMPTY; TLB_ENTRIES];
        tlb[tlb_index(BASE)] = TlbEntry {
            lo: BASE,
            hi: BASE + 65536,
            base: ram.mem.as_mut_ptr(),
            vbase: 0,
            writable: (!readonly) as u16,
            off: 0,
            src: 0,
            code: 1,
            span: 0,
        }
        .with_span();
        Self {
            ram,
            fetch_fault: None,
            versions: vec![0; 256],
            tlb,
            fast,
            readonly,
            watched: true,
            pinned: false,
            noted: 0,
            slow: [0x5a; 256],
            slow_writes: 0,
            slow_reads: 0,
            defer_armed: false,
            deferred: false,
            #[cfg(feature = "wasm-cache-inline")]
            inline_cache: None,
            #[cfg(feature = "wasm-cache-inline")]
            helper_accesses: 0,
        }
    }
    /// The same write-version rule as the real S3 bus (`esp32s3/src/bus.rs` `bump`):
    /// an instruction can begin up to three bytes before a page boundary, so a write
    /// into the first three bytes of a page also changes instructions whose code page
    /// is the previous one.
    fn wrote(&mut self, a: u32, n: u32) {
        if !self.watched { return }
        let off = a - BASE;
        for p in off / 256..=(off + n - 1) / 256 {
            self.versions[p as usize] += 1;
        }
        if off & 255 < emu_core::bus::PREV_PAGE_BYTES && off >= 256 {
            self.versions[(off / 256 - 1) as usize] += 1;
        }
    }
    /// EX110: no decoded consumer depends on this mapping yet, so neither path records writes.
    fn unwatch(&mut self) {
        self.watched = false;
        for e in self.tlb.iter_mut() { e.code = 0; }
    }
}
impl Bus for Ram {
    fn read8(&mut self, a: u32) -> Result<u8, Fault> {
        if (SLOW..SLOW + 256).contains(&a) { return Ok(self.slow[(a - SLOW) as usize]); }
        self.ram.read8(a)
    }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> {
        self.ram.read16(a)
    }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> {
        self.slow_reads += 1;
        #[cfg(feature = "wasm-cache-inline")]
        if self.inline_cache.is_some() { self.helper_accesses += 1; }
        self.ram.read32(a)
    }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> {
        if self.readonly {
            return Err(Fault::Prohibited);
        }
        if (SLOW..SLOW + 256).contains(&a) { self.slow[(a - SLOW) as usize] = v; return Ok(()); }
        self.ram.write8(a, v)?;
        self.wrote(a, 1);
        Ok(())
    }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> {
        if self.readonly {
            return Err(Fault::Prohibited);
        }
        self.ram.write16(a, v)?;
        self.wrote(a, 2);
        Ok(())
    }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> {
        #[cfg(feature = "wasm-cache-inline")]
        if self.inline_cache.is_some() { self.helper_accesses += 1; }
        if self.readonly {
            return Err(Fault::Prohibited);
        }
        if (SLOW..SLOW + 253).contains(&a) {
            self.slow_writes += 1;
            self.slow[(a - SLOW) as usize..(a - SLOW) as usize + 4].copy_from_slice(&v.to_le_bytes());
            return Ok(());
        }
        self.ram.write32(a, v)?;
        self.wrote(a, 4);
        Ok(())
    }
    fn fetch(&mut self, a: u32) -> Result<[u8; 4], Fault> {
        if self.fetch_fault == Some(a) { return Err(Fault::Prohibited); }
        self.ram.fetch(a)
    }
    fn page_versions(&self) -> &[u32] {
        &self.versions
    }
    fn code_page(&mut self, a: u32) -> u32 {
        a.wrapping_sub(BASE) / 256
    }
    fn fast_mem(&mut self) -> Option<FastMem> {
        self.fast.then_some(FastMem {
            tlb: self.tlb.as_ptr(),
            page_ver: self.versions.as_mut_ptr(),
        })
    }
    fn note_pc(&mut self, pc: u32) {
        self.noted = pc;
    }
    fn note_code_page(&mut self, _vidx: u32) {
        if self.pinned { return }
        self.watched = true;
        for e in self.tlb.iter_mut() { if e.hi > e.lo { e.code = 1; } }
    }
    fn defer_armed(&self) -> bool { self.defer_armed }
    fn defer_access(&mut self, addr: u32) -> bool {
        if self.defer_armed && (SLOW..SLOW + 256).contains(&addr) {
            self.deferred = true; true
        } else { false }
    }
    fn deferred(&self) -> bool { self.deferred }
    #[cfg(feature = "wasm-cache-inline")]
    fn fast_cache(&mut self) -> Option<emu_core::bus::FastCache> {
        self.inline_cache.as_mut().map(|(lines, hits)| emu_core::bus::FastCache { lines: lines.as_mut_ptr(), hits })
    }
}

fn cpu(seed: u32) -> Cpu {
    let mut c = Cpu::new(0);
    c.pc = BASE;
    c.ps = 0;
    c.vecbase = BASE + 0x8000;
    c.windowbase = seed % 16;
    c.sar = seed % 64;
    let mut x = seed;
    for r in &mut c.ar {
        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
        *r = x;
    }
    c
}
thread_local! { static CONTEXT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
fn same(a: &Cpu, b: &Cpu) {
    let cx = CONTEXT.with(|c| c.borrow().clone());
    assert_eq!(a.ar, b.ar, "registers at {:x} [{cx}]", a.pc);
    assert_eq!(a.fr, b.fr, "float register bits");
    assert_eq!(a.br, b.br, "boolean registers");
    assert_eq!(a.cpenable, b.cpenable);
    assert_eq!(a.fcr, b.fcr);
    assert_eq!(a.fsr, b.fsr);
    assert_eq!(a.pc, b.pc, "PC");
    assert_eq!(a.ps, b.ps);
    assert_eq!(a.sar, b.sar);
    assert_eq!(a.windowbase, b.windowbase);
    assert_eq!(a.windowstart, b.windowstart);
    assert_eq!((a.lbeg, a.lend, a.lcount), (b.lbeg, b.lend, b.lcount), "loop state [{cx}]");
    assert_eq!((a.interrupt, a.intenable), (b.interrupt, b.intenable));
    assert_eq!((a.scompare1, a.vecbase, a.prid, a.depc), (b.scompare1, b.vecbase, b.prid, b.depc));
    assert_eq!((a.eps, a.excsave, a.misc), (b.eps, b.excsave, b.misc));
    assert_eq!(a.epc, b.epc);
    assert_eq!(a.exccause, b.exccause);
    assert_eq!(a.insn_count, b.insn_count);
    assert_eq!(a.ccount, b.ccount);
    assert_eq!(a.timing_extra, b.timing_extra, "timing extras [{cx}]");
    assert_eq!(a.excvaddr, b.excvaddr, "EXCVADDR [{cx}]");
    assert_eq!(a.qr, b.qr, "PIE Q registers [{cx}]");
    assert_eq!(a.accx, b.accx, "PIE ACCX [{cx}]");
    assert_eq!((a.qacc_h, a.qacc_l, a.sar_byte), (b.qacc_h, b.qacc_l, b.sar_byte), "PIE QACC / SAR_BYTE [{cx}]");
}
fn insn(op: Op) -> BlockInsn {
    let i = Insn {
        op,
        r: 3,
        s: 4,
        t: 5,
        imm: 3,
        imm2: 7,
        len: 3,
        raw: 0,
    };
    BlockInsn {
        insn: i,
        max_ar: crate::exec::max_ar(&i),
        straddle: false,
        off: 0,
    }
}
/// Inputs shared by the constructed-instruction differential suites. Omitted flags
/// select ordinary RAM, no loop end and no forced window overflow.
#[derive(Clone, Copy, Default)]
struct Case {
    seed: u32,
    entry: u32,
    budget: u32,
    addr: Option<u32>,
    fast: bool,
    readonly: bool,
    loop_end: bool,
    overflow: bool,
    /// EX110: run with no decoded consumer for the mapping, so version bookkeeping is skipped.
    unwatched: bool,
    /// Bytes cut off the end of the mapping and of its backing memory, so the fast mapping's
    /// length is not a multiple of the access width (EX173).
    shrink: u32,
}

fn compare(block: &mut [BlockInsn], case: Case, configure: impl Fn(&mut Cpu)) {
    // EX156: a hinted loop end selects the guarded body; without the hint the same state
    // must still take the checked body.
    if case.loop_end { compare_hinted(block, case, &configure, BASE + 6); }
    compare_hinted(block, case, &configure, 0);
}

fn compare_hinted(block: &mut [BlockInsn], case: Case, configure: &impl Fn(&mut Cpu), hint: u32) -> bool {
    let Case { seed, entry, budget, addr, fast, readonly, loop_end, overflow, unwatched: _, shrink } = case;
    let priced = PRICED.load(std::sync::atomic::Ordering::Relaxed);
    CONTEXT.with(|c| *c.borrow_mut() = format!("{:?} seed={seed} entry={entry} budget={budget} fast={fast} loop_end={loop_end} overflow={overflow} priced={priced}",
        block.iter().map(|b| b.insn.op).collect::<Vec<_>>()));
    let (mut ra, mut rb) = (Ram::new(fast, readonly), Ram::new(fast, readonly));
    if case.unwatched { ra.unwatch(); rb.unwatch(); }
    for r in [&mut ra, &mut rb] {
        // `truncate` keeps the allocation, so the entry's host base stays valid: an access the
        // probe wrongly admits reads or writes bytes the bus itself now refuses.
        let len = r.ram.mem.len() - shrink as usize;
        r.ram.mem.truncate(len);
        let slot = tlb_index(BASE);
        r.tlb[slot].hi -= shrink;
        r.tlb[slot] = r.tlb[slot].with_span();
    }
    for bi in block.iter_mut() {
        bi.straddle = priced && crate::exec::static_target(&bi.insn).is_some_and(|pc| crate::exec::straddles(&mut ra, pc));
    }
    let extras = crate::exec::static_extras(block.iter().map(|bi| &bi.insn));
    let mut cc = CodeCache::new(0).unwrap();
    let code = queue(&mut cc, block, BASE, fast);
    for _ in 0..HOT {
        ready(&cc, code, hint);
    }
    assert!(ready(&cc, code, hint), "compiled module must execute");
    let (mut a, mut b) = (cpu(seed), cpu(seed));
    for c in [&mut a, &mut b] {
        c.price_control = priced;
        c.pc = BASE + entry * 3;
        if let Some(addr) = addr {
            c.set_ar(4, addr.wrapping_sub(3));
        }
        if loop_end {
            c.lend = BASE + 6;
            c.lbeg = BASE;
            c.lcount = 2;
        }
        if overflow {
            c.ps = ps::WOE;
            c.windowstart = 1 << ((c.windowbase + 1) % 16);
        }
        configure(c);
    }
    GUARDED_TAKEN.store(0, std::sync::atomic::Ordering::Relaxed);
    let fm = rb.fast_mem();
    let result = unsafe {
        run(
            &cc,
            code,
            &mut b,
            &mut rb,
            &Helpers::new::<Ram>(),
            budget,
            entry,
            fm,
        )
    };
    let done = result & 0xffff;
    let exit = (result >> 16) & 7;
    if exit == CODE_CUT {
        let next = (result >> 19) as usize;
        assert!(next < block.len(), "cut continuation must be inside the block");
        let pc = block[..next].iter().fold(BASE, |pc, bi| pc.wrapping_add(bi.insn.len as u32));
        assert_eq!(b.pc, pc, "cut continuation index must match the architectural PC");
    }
    let mut count = 0;
    let mut trap = None;
    let mut pre = false;
    let repeat = loop_len(&cc, code, &a, &mut ra).is_some();
    for _ in 0..budget {
        let index = a.pc.wrapping_sub(BASE) / 3;
        let Some(instruction) = block.get(index as usize) else { break; };
        if let Some(t) = a.check_overflow(instruction.max_ar) {
            trap = Some(t);
            pre = true;
            break;
        }
        let pc = a.pc;
        ra.note_pc(pc);
        let r = exec_insn(&mut a, &mut ra, &instruction.insn);
        count += 1;
        if priced && r.is_ok() {
            // The same accounting boundary as the ordinary block interpreter.
            // h_exec prices helper control flow; the emitter must not charge it twice.
            let taken = crate::exec::control_taken(&a, &instruction.insn);
            a.timing_extra += crate::exec::control_price(instruction.insn.op, taken)
                + u32::from(extras[index as usize])
                + u32::from(taken && crate::exec::transfers(instruction.insn.op) && crate::exec::straddles(&mut ra, a.pc));
        }
        if let Err(t) = r {
            trap = Some(t);
            break;
        }
        // A pre-instruction trap after a completed prefix must still be checked
        // on the next iteration; it retires no additional instruction.
        if (a.pc != pc + 3 && !(repeat && a.pc == a.lbeg)) || (count == done && exit != CODE_TRAP_PRE) {
            break;
        }
    }
    assert_eq!(count, done);
    assert_eq!(pre, exit == CODE_TRAP_PRE);
    assert_eq!(trap, b.jit_trap.take());
    same(&a, &b);
    assert_eq!(ra.ram.mem, rb.ram.mem);
    assert_eq!(ra.versions, rb.versions, "page versions [{}]", CONTEXT.with(|c| c.borrow().clone()));
    if done > 0 {
        assert_eq!(ra.noted, rb.noted);
    }
    GUARDED_TAKEN.load(std::sync::atomic::Ordering::Relaxed) != 0
}

#[path = "wasm_tests/arithmetic.rs"]
mod arithmetic;
#[path = "wasm_tests/control.rs"]
mod control;
#[path = "wasm_tests/float.rs"]
mod float;
#[path = "wasm_tests/loops.rs"]
mod loops;
#[path = "wasm_tests/memory.rs"]
mod memory;
#[path = "wasm_tests/scheduler.rs"]
mod scheduler;
#[path = "wasm_tests/timing.rs"]
mod timing;
#[path = "wasm_tests/regions.rs"]
mod regions;
#[path = "wasm_tests/asm.rs"]
mod asm;

pub use float::fma_sweep;
// One canonical list is used by both the monolithic entry point and the
// process-isolated Node runner. Large Cartesian products are partitioned only
// along their existing outer loop; every operand/budget case still runs once.
const TEST_GROUPS: &[fn() -> u32] = &[
    #[cfg(feature = "wasm-cache-inline")]
    memory::inline_cache_hits,
    memory::page_boundary_stores,
    memory::straddling_instruction_rewrite,
    arithmetic::basic_ops,
    arithmetic::division,
    memory::loads_and_stores,
    memory::probe_boundaries,
    control::helper_continuation,
    control::interpreted_bridges,
    control::bridge_classes,
    || { scheduler::wrapper_bridge_guards(); scheduler::scheduler(); scheduler::wrapper_chain(); 1 },
    || { scheduler::interior_alias(); scheduler::interior_alias_deferred(); scheduler::interior_alias_instruction_bytes(); scheduler::ps_terminal_chain(); scheduler::event_writers(); 5 },
    || reboot::esp32sim_test_region_ownership(),
    memory::extension_deferral,
    memory::flat_ram_bounds,
    regions::regions,
    pie_accx::run_tests,
    pie_accx::held_and_coalesced,
    memory::code_page_flag,
    || { scheduler::retention(); 1 },
    || { loops::hardware_loop_scheduler(); 1 },
    || { crate::block::ownership_tests::compiled_helpers_follow_the_current_bus_type(); 1 },
    || arithmetic::integer_ops_part(0),
    || arithmetic::integer_ops_part(1),
    || arithmetic::integer_ops_part(2),
    || float::floating_point_part(0),
    || float::floating_point_part(1),
    || float::floating_point_part(2),
    || float::floating_point_part(3),
    float::floating_point_guard_proof,
    float::fma_halfway_fallback,
    loops::hardware_loops,
    loops::store_runs,
    control::window_masks,
    control::window_quads,
    control::terminal_helpers,
    control::special_register_blocks,
    control::ps_terminals,
    control::windowed_return,
    control::whole_block_guards,
    || control::entry_and_shifts_part(0),
    || control::entry_and_shifts_part(1),
    || control::entry_and_shifts_part(2),
    || control::entry_and_shifts_part(3),
    || control::entry_and_shifts_part(4),
    || control::entry_and_shifts_part(5),
    || control::entry_and_shifts_part(6),
    || control::entry_and_shifts_part(7),
    || control::entry_and_shifts_part(8),
    || control::entry_and_shifts_part(9),
    || control::entry_and_shifts_part(10),
    control::guarded_loop_sites,
    control::pie_wide_shifts,
    || timing::priced_cases_part(0),
    || timing::priced_cases_part(1),
    || timing::priced_cases_part(2),
    || timing::priced_cases_part(3),
    || timing::priced_cases_part(4),
    || timing::priced_cases_part(5),
    || timing::priced_cases_part(6),
    || timing::priced_cases_part(7),
    || timing::priced_cases_part(8),
    || timing::priced_cases_part(9),
    || timing::priced_cases_part(10),
    || timing::priced_cases_part(11),
    || timing::priced_cases_part(12),
    || timing::priced_cases_part(13),
    || timing::priced_cases_part(14),
    || timing::priced_cases_part(15),
    || timing::priced_cases_part(16),
    || timing::priced_cases_part(17),
    || timing::priced_cases_part(18),
    || timing::priced_cases_part(19),
    || timing::priced_cases_part(20),
    || timing::priced_cases_part(21),
    || timing::priced_cases_part(22),
];

pub fn test_group_count() -> u32 { TEST_GROUPS.len() as u32 }
pub fn run_test_group(group: u32) -> u32 {
    TEST_GROUPS.get(group as usize).expect("invalid JIT test group")()
}
pub fn run_tests() -> u32 { TEST_GROUPS.iter().map(|run| run()).sum() }
