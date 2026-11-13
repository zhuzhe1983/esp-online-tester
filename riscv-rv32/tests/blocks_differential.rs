//! EX265: decoded blocks against the interpreter. Two identical cores on two identical buses
//! run the same sequence of `Core::run` calls, one with blocks (`set_jit(true)`) and one
//! stepping (`set_jit(false)`, the oracle), from random programs that mix every major opcode,
//! compressed forms, loads and stores into their own code (self-modifying code), device reads
//! with side effects, device writes that end a dispatch (`block_break`), traps, interrupts
//! presented between runs, budget cuts and machine boundaries. After every run the iterations,
//! the trap and the whole architectural state (and memory) must agree.
use emu_core::{Bus, Core, Fault, Trap};
use riscv_rv32::Cpu;

const RAM: u32 = 0x4038_0000;
const RAM_SIZE: usize = 0x4000;
const MMIO: u32 = 0x6000_0000;
const HANDLER: u32 = RAM + 0x100;
const CODE: u32 = RAM + 0x200;
const CODE_BYTES: usize = 0x1000;

/// RAM with a code version per 256-byte page, read-ahead, and a device page whose reads count
/// and whose writes end the dispatch, as a SoC bus does.
#[derive(Clone)]
struct TestBus { ram: Vec<u8>, ver: Vec<u32>, reads: u32, regs: [u32; 4], brk: bool, versions: bool }

impl TestBus {
    fn new() -> Self { TestBus { ram: vec![0; RAM_SIZE], ver: vec![0; RAM_SIZE >> 8], reads: 0, regs: [0; 4], brk: false, versions: true } }
    fn off(&self, a: u32, n: usize) -> Option<usize> {
        let o = a.wrapping_sub(RAM) as usize;
        (o.checked_add(n)? <= RAM_SIZE).then_some(o)
    }
    fn mmio(a: u32, n: usize) -> Option<usize> { (a >= MMIO && a as u64 + n as u64 <= MMIO as u64 + 16).then(|| ((a - MMIO) / 4) as usize) }
    fn read(&mut self, a: u32, n: usize) -> Result<u32, Fault> {
        if let Some(o) = self.off(a, n) { let mut b = [0; 4]; b[..n].copy_from_slice(&self.ram[o..o + n]); return Ok(u32::from_le_bytes(b)); }
        if let Some(r) = Self::mmio(a, n) { self.reads = self.reads.wrapping_add(1); return Ok(self.regs[r] ^ self.reads); }
        Err(Fault::Unmapped)
    }
    fn write(&mut self, a: u32, v: u32, n: usize) -> Result<(), Fault> {
        if let Some(o) = self.off(a, n) {
            self.ram[o..o + n].copy_from_slice(&v.to_le_bytes()[..n]);
            let first = o.saturating_sub(3) >> 8;
            for v in &mut self.ver[first..=((o + n - 1) >> 8)] { *v = v.wrapping_add(1); }
            return Ok(());
        }
        if let Some(r) = Self::mmio(a, n) { self.regs[r] = v; self.brk = true; return Ok(()); }
        Err(Fault::Unmapped)
    }
}

impl Bus for TestBus {
    fn note_code_page(&mut self, _vidx: u32) {}
    fn read8(&mut self, a: u32) -> Result<u8, Fault> { self.read(a, 1).map(|v| v as u8) }
    fn read16(&mut self, a: u32) -> Result<u16, Fault> { self.read(a, 2).map(|v| v as u16) }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.read(a, 4) }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), Fault> { self.write(a, v as u32, 1) }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), Fault> { self.write(a, v as u32, 2) }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), Fault> { self.write(a, v, 4) }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        let o = self.off(pc, 2).ok_or(Fault::Unmapped)?;
        let mut b = [0; 4];
        for (i, x) in b.iter_mut().enumerate() { if o + i < RAM_SIZE { *x = self.ram[o + i]; } }
        Ok(b)
    }
    fn fetch_ahead(&mut self, pc: u32) -> Option<[u8; 4]> { self.fetch(pc).ok() }
    fn page_versions(&self) -> &[u32] { if self.versions { &self.ver } else { &[] } }
    fn code_page(&mut self, pc: u32) -> u32 { match self.off(pc, 1) { Some(o) => (o >> 8) as u32, None => u32::MAX } }
    fn block_break(&self) -> bool { self.brk }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 16) as u32 }
    fn below(&mut self, n: u32) -> u32 { self.next() % n }
}

fn b_type(op: u32, f3: u32, rs1: u32, rs2: u32, off: i32) -> u32 {
    let o = off as u32;
    op | ((o >> 11) & 1) << 7 | ((o >> 1) & 0xf) << 8 | f3 << 12 | rs1 << 15 | rs2 << 20 | ((o >> 5) & 0x3f) << 25 | ((o >> 12) & 1) << 31
}
fn j_type(rd: u32, off: i32) -> u32 {
    let o = off as u32;
    0x6f | rd << 7 | ((o >> 12) & 0xff) << 12 | ((o >> 11) & 1) << 20 | ((o >> 1) & 0x3ff) << 21 | ((o >> 20) & 1) << 31
}
fn i_type(op: u32, f3: u32, rd: u32, rs1: u32, imm: i32) -> u32 { op | rd << 7 | f3 << 12 | rs1 << 15 | ((imm as u32) & 0xfff) << 20 }
fn s_type(f3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 { let i = imm as u32; 0x23 | (i & 0x1f) << 7 | f3 << 12 | rs1 << 15 | rs2 << 20 | ((i >> 5) & 0x7f) << 25 }

/// One instruction: mostly well-formed common forms with small offsets (so that loops, loads and
/// stores stay in RAM, the code region included), some device accesses, and a share of random
/// words of every major opcode and random compressed halfwords.
fn insn(r: &mut Rng, out: &mut Vec<u8>) {
    let reg = |r: &mut Rng| 8 + r.below(8);                     // s0-a5 hold RAM or device addresses
    let any = |r: &mut Rng| r.below(32);
    let w = match r.below(22) {
        20 | 21 => {
            // a hardware-loop instruction (Xesploop), with a short body and a small count
            let (m, v, _) = riscv_rv32::decode::HWLP_FORMS[r.below(6) as usize];
            let w = v | (r.next() & !m);
            let w = if m >> 20 == 0 { (w & 0x000f_ffff) | (1 + r.below(24)) << 20 } else { w };
            // esp.lp.count/setup take the count from a register: use one that holds a small value
            if (w >> 12) & 7 == 2 || (w >> 12) & 7 == 4 { (w & !(0x1f << 15)) | (16 + r.below(2)) << 15 } else { w }
        }
        0..=3 => i_type(0x13, r.below(8), any(r), any(r), (r.next() as i32) >> 20 & 0x3ff),
        4 | 5 => 0x33 | any(r) << 7 | r.below(8) << 12 | any(r) << 15 | any(r) << 20 | [0, 0x20, 1][r.below(3) as usize] << 25,
        6 | 7 => i_type(0x03, [0, 1, 2, 4, 5][r.below(5) as usize], 1 + r.below(31), reg(r), (r.below(64) as i32 - 32) * 4),
        8 | 9 => s_type(r.below(3), reg(r), any(r), (r.below(64) as i32 - 32) * 4),
        10 | 11 => b_type(0x63, [0, 1, 4, 5, 6, 7][r.below(6) as usize], any(r), any(r), (r.below(48) as i32 - 24) * 2),
        12 => j_type(r.below(2), (r.below(128) as i32 - 64) * 2),
        13 => {
            // CSR: reads of the counters and scratch, writes of mscratch/mstatus/mtvec/mie, a
            // random CSR number now and then
            let csr = [0x340, 0xb00, 0xb02, 0xc00, 0x300, 0x7e2, 0x342, r.next() & 0xfff][r.below(8) as usize];
            let f3 = [1, 2, 3, 5, 6, 7][r.below(6) as usize];
            let rs1 = if r.below(2) == 0 { 0 } else { any(r) };
            // a write that sets mstatus bits may turn MIE on mid-program, as an ISR's exit does;
            // keep it rarer than the rest so the programs do not spend all their time in traps
            let csr = if csr == 0x300 && rs1 != 0 && f3 & 3 == 1 && r.below(3) != 0 { 0x340 } else { csr };
            0x73 | any(r) << 7 | f3 << 12 | rs1 << 15 | csr << 20
        }
        14 => [0x0000_0073, 0x0010_0073, 0x3020_0073, 0x1050_0073, 0x0000_100f, 0x0ff0_000f][r.below(6) as usize],
        15 => { let c = r.next() as u16; out.extend_from_slice(&c.to_le_bytes()); return; }
        16 => i_type(0x67, 0, r.below(2), reg(r), (r.below(16) as i32) * 4),
        17 => 0x2f | any(r) << 7 | 2 << 12 | reg(r) << 15 | any(r) << 20 | [0x02, 0x03, 0x01, 0x00, 0x04, 0x0c, 0x08, 0x10, 0x18][r.below(9) as usize] << 27,
        _ => {
            let op = [0x03, 0x07, 0x0f, 0x13, 0x17, 0x1b, 0x1f, 0x23, 0x27, 0x2f, 0x33, 0x37, 0x43, 0x53, 0x5f, 0x63, 0x67, 0x6f, 0x73][r.below(19) as usize];
            op | (r.next() & !0x7f)
        }
    };
    out.extend_from_slice(&w.to_le_bytes());
}

fn state(c: &Cpu) -> String {
    format!("hwlp=({:x?},{:x?},{:x?},{:x}) pc={:08x} x={:?} f={:?} q={:?} mstatus={:08x} mepc={:08x} mcause={:08x} mtval={:08x} mtvec={:08x} mscratch={:08x} mie={:08x} insns={} retired={} cycles={} waiting={} fcsr={:x} mil={} thresh={} res={:?} pie=({:x},{:x},{:x},{:x}) ext_ill={:x} other={:?}",
        c.hwlp_start, c.hwlp_end, c.hwlp_count, c.hwlp_state, c.pc, c.x, c.f, c.q, c.mstatus, c.mepc, c.mcause, c.mtval, c.mtvec, c.mscratch, c.mie, c.insn_count, c.retired_count, c.cycle_count, c.waiting, c.fcsr, c.mil, c.mintthresh, c.reservation, c.pie_sar, c.pie_sar_byte, c.pie_cfg, c.pie_state, c.ext_ill, { let mut v: Vec<_> = c.csr_other.iter().collect(); v.sort(); v })
}

fn core(kind: u32) -> Cpu {
    match kind { 0 => Cpu::new(), 1 => Cpu::new_rv32imac(), 2 => Cpu::new_esp32p4_rev3(), _ => Cpu::new_esp32s31() }
}

/// Returns the number of `run` calls compared, blocks built and instructions run.
fn one(seed: u64, kind: u32) -> (u64, u64, u64) {
    let mut r = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut bus = TestBus::new();
    // trap handler: skip the trapping instruction (mepc += 4) and return
    let handler = [0x3410_2ff3u32, 0x004f_8f93, 0x341f_9073, 0x3020_0073];
    for (k, w) in handler.iter().enumerate() { bus.ram[0x100 + 4 * k..][..4].copy_from_slice(&w.to_le_bytes()); }
    let mut code = Vec::new();
    while code.len() < CODE_BYTES { insn(&mut r, &mut code); }
    bus.ram[0x200..0x200 + CODE_BYTES].copy_from_slice(&code[..CODE_BYTES]);
    for b in &mut bus.ram[0x200 + CODE_BYTES..] { *b = r.next() as u8; }
    let mut a = core(kind);
    let mut b = core(kind);
    for c in [&mut a, &mut b] {
        c.pc = CODE;
        c.mtvec = HANDLER;
        c.mstatus |= riscv_rv32::state::mstatus::MIE;
        if kind >= 2 { c.mstatus |= riscv_rv32::state::mstatus::FS_INITIAL; c.set_hwlp_state(1); }
    }
    let mut regs = [0u32; 32];
    for (i, x) in regs.iter_mut().enumerate().skip(1) {
        *x = match i { 16 | 17 => r.below(6), 8..=15 => if r.below(6) == 0 { MMIO + 4 * r.below(4) } else { CODE + 4 * r.below((CODE_BYTES / 4) as u32 + 256) }, 2 => RAM + RAM_SIZE as u32 - 64, _ => r.next() };
    }
    a.x = regs; b.x = regs;
    b.set_jit(false);
    let mut bus_b = bus.clone();
    let mut calls = 0;
    for _ in 0..300 {
        // what the machine does between dispatches: refresh (clear the break), present an
        // interrupt now and then, sometimes move the boundaries
        bus.brk = false; bus_b.brk = false;
        let irq = match r.below(8) { 0 => Some(r.below(32) | (1 + r.below(7)) << 8), 1 => Some(r.below(32) | 7 << 8 | 1 << 16), _ => None };
        a.set_irq(irq); b.set_irq(irq);
        if r.below(40) == 0 { let bloom = if r.below(2) == 0 { 0 } else { emu_core::core::pc_bit(CODE + 2 * r.below(0x800)) }; a.set_boundaries(bloom); b.set_boundaries(bloom); }
        if r.below(60) == 0 && a.pc.wrapping_sub(RAM) as usize >= RAM_SIZE { a.pc = CODE; b.pc = CODE; }
        if a.mtvec != HANDLER && r.below(4) == 0 { a.mtvec = HANDLER; b.mtvec = HANDLER; }
        let budget = [1, 2, 3, 7, 64, 256, 1000][r.below(7) as usize];
        let ra = a.run(&mut bus, budget);
        let rb = b.run(&mut bus_b, budget);
        calls += 1;
        let (sa, sb) = (state(&a), state(&b));
        if ra != rb || sa != sb || bus.ram != bus_b.ram || bus.regs != bus_b.regs || bus.reads != bus_b.reads || bus.brk != bus_b.brk {
            panic!("seed {seed} kind {kind} call {calls}: blocks {ra:?} vs interpreter {rb:?}\nblocks      {sa}\ninterpreter {sb}\nram same {}", bus.ram == bus_b.ram);
        }
        // the interpreter oracle must not have been using blocks
        assert_eq!(b.blocks.builds, 0);
    }
    (calls, a.blocks.builds, a.insn_count)
}

#[test]
fn blocks_match_the_interpreter_on_random_programs() {
    let seeds: u64 = std::env::var("RV_BLOCK_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(400);
    let (mut programs, mut calls, mut blocks, mut insns) = (0, 0, 0, 0);
    for kind in 0..4 {
        for seed in 0..seeds {
            let (c, b, i) = one(seed * 4 + kind as u64, kind);
            programs += 1; calls += c; blocks += b; insns += i;
        }
    }
    eprintln!("blocks differential: {programs} programs, {calls} run calls compared, {blocks} blocks built, {insns} instructions");
    assert!(blocks > programs, "the programs must exercise blocks");
}

/// A store into the next instruction of its own block: the interpreter fetches the new word,
/// and so must the block (the rest of it is dropped when its page version moves).
#[test]
fn a_store_into_its_own_block_takes_effect_at_once() {
    let mut bus = TestBus::new();
    // lui a0, 0x00500 ; addi a0,a0,0x513 (a0 = 0x00500513 = addi a0, zero, 5) ; sw a0, 4(s0) ; addi a0, zero, 1 ; j .
    let prog = [0x0050_0537u32, 0x5135_0513, 0x00a4_2223, 0x0010_0513, 0x0000_006f];
    for (k, w) in prog.iter().enumerate() { bus.ram[0x200 + 4 * k..][..4].copy_from_slice(&w.to_le_bytes()); }
    let mut c = Cpu::new();
    c.pc = CODE; c.x[8] = CODE + 8;          // s0 + 4 = the `addi a0, zero, 1` after the store
    let (used, trap) = c.run(&mut bus, 4);
    assert_eq!((used, trap), (4, None));
    assert_eq!(c.x[10], 5, "the stored instruction ran, not the stale one");
    assert!(c.blocks.builds >= 2, "the block was rebuilt after the store");
}

/// A bus without code versions keeps the interpreter: no block is ever built.
#[test]
fn a_bus_without_versions_never_builds() {
    let mut bus = TestBus::new();
    bus.versions = false;
    bus.ram[0x200..0x208].copy_from_slice(&[0x93, 0x00, 0x50, 0x00, 0x6f, 0x00, 0x00, 0x00]);
    let mut c = Cpu::new(); c.pc = CODE;
    assert_eq!(c.run(&mut bus, 100), (100, None));
    assert_eq!(c.blocks.builds, 0);
}

/// A trap in the middle of a block: mepc is the faulting instruction and the iterations count
/// the instructions before it plus the faulting one.
#[test]
fn a_fault_mid_block_is_precise() {
    let mut bus = TestBus::new();
    // addi a0,zero,1 ; addi a1,zero,2 ; lw a2, 0(zero) (faults) ; addi a3,zero,3
    let prog = [0x0010_0513u32, 0x0020_0593, 0x0000_2603, 0x0030_0693];
    for (k, w) in prog.iter().enumerate() { bus.ram[0x200 + 4 * k..][..4].copy_from_slice(&w.to_le_bytes()); }
    let mut c = Cpu::new(); c.pc = CODE; c.mtvec = HANDLER;
    let (used, trap) = c.run(&mut bus, 10);
    assert_eq!((used, trap), (3, Some(Trap::Exception(5))));
    assert_eq!((c.mepc, c.pc, c.x[11], c.x[13]), (CODE + 8, HANDLER, 2, 0));
    assert_eq!((c.insn_count, c.retired_count), (3, 2));
}

/// A hardware loop whose body spans a block: the back-edge after the end instruction must be
/// taken from blocks exactly as the interpreter takes it, count and state included.
#[test]
fn a_hardware_loop_runs_its_body_count_times_in_blocks() {
    // esp.lp.setupi 0, count 5, end = pc + 12 (the third body instruction):
    //   addi a0, a0, 1 ; addi a1, a1, 2 ; addi a2, a2, 3 ; then addi a3, zero, 7 ; j .
    let setupi = 0x0000_502bu32 | 5 << 20 | (12 >> 1) << 15;
    let prog = [setupi, 0x0015_0513, 0x0025_8593, 0x0036_0613, 0x0070_0693, 0x0000_006f];
    for jit in [true, false] {
        let mut bus = TestBus::new();
        for (k, w) in prog.iter().enumerate() { bus.ram[0x200 + 4 * k..][..4].copy_from_slice(&w.to_le_bytes()); }
        let mut c = Cpu::new_esp32s31(); c.pc = CODE; c.set_hwlp_state(1); c.set_jit(jit);
        let (used, trap) = c.run(&mut bus, 1 + 3 * 5 + 1);
        assert_eq!((used, trap), (17, None));
        assert_eq!((c.x[10], c.x[11], c.x[12], c.x[13]), (5, 10, 15, 7), "jit {jit}");
        assert_eq!((c.hwlp_count[0], c.pc, c.hwlp_state & 3), (0, CODE + 20, 3), "jit {jit}");
        assert_eq!(c.blocks.builds > 0, jit);
    }
}
