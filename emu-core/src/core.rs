//! A CPU core as the machine sees it. The machine schedules cores, delivers interrupt state,
//! counts traps and drives tracing through this trait; everything architectural (register
//! windows, CSRs, vectors) stays inside the core crate.
use crate::bus::{Bus, Fault};

/// Why `step`/`run` returned early. Architectural traps have already been taken (the pc points
/// at the handler); the emulator-level ones are reported so the machine can stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trap {
    /// an architectural exception was taken (vectored); emulation continues normally
    Exception(u32),
    /// an interrupt was taken (Xtensa interrupt number / RISC-V line)
    Interrupt(u32),
    /// instruction not implemented by the emulator (pc, raw word)
    Unimplemented(u32, u32),
    /// A model boundary at this pc, without taking an architectural exception.
    /// No instruction word is claimed when fetch itself could not be qualified.
    ModelLimit(u32),
    /// `simcall` — Xtensa semihosting request
    Simcall,
    /// `ebreak` — a panic, an assert, or a debugger breakpoint in a RISC-V guest
    Ebreak(u32),
}

/// Cache operations whose effective address or occurrence cannot be reconstructed from bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheOperation {
    DataPrefetchRead, DataPrefetchWrite, DataPrefetchReadOnce, DataPrefetchWriteOnce,
    DataHitWriteback, DataHitWritebackInvalidate, DataHitInvalidate, DataIndexInvalidate,
    DataPrefetchLocked, DataHitUnlock, DataIndexUnlock,
    InstructionPrefetch, InstructionHitInvalidate, InstructionIndexInvalidate,
    InstructionPrefetchLocked, InstructionHitUnlock, InstructionIndexUnlock,
    FenceInstruction,
}

/// TLB operations whose effective address cannot be reconstructed from the instruction bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlbOperation {
    ReadInstructionEntry0, ReadInstructionEntry1, ReadDataEntry0, ReadDataEntry1,
    ProbeInstruction, ProbeData, InvalidateInstruction, InvalidateData,
    WriteInstruction, WriteData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlEventKind { Cache(CacheOperation), Tlb(TlbOperation) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlEvent {
    pub kind: ControlEventKind,
    /// The effective address, or zero for an addressless operation such as RISC-V `fence.i`.
    pub address: u32,
}

/// Whether an instruction retired, the core remained idle, or a trap occurred on either side of
/// execution. A trap-before outcome has bytes when decoding succeeded before an overflow check,
/// and no bytes when no instruction was fetched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    Retired,
    Idle,
    TrapBefore(Trap),
    TrapDuring(Trap),
}

/// Allocation-free facts produced by one slow-path core step. `bytes` supplies the conceptual
/// fetch even on an LX7 decode-cache hit. A machine can separately wrap the bus to collect
/// CPU-originated memory accesses without including device or DMA traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepOutcome {
    pub pc: u32,
    pub next_pc: u32,
    pub bytes: Option<[u8; 4]>,
    pub length: u8,
    pub kind: StepKind,
    /// Current cores execute at most one cache or TLB control operation per instruction.
    pub control: Option<ControlEvent>,
}

impl StepOutcome {
    #[inline]
    pub fn result(self) -> Result<(), Trap> {
        match self.kind {
            StepKind::Retired | StepKind::Idle => Ok(()),
            StepKind::TrapBefore(trap) | StepKind::TrapDuring(trap) => Err(trap),
        }
    }

    #[inline]
    pub fn trap(&self) -> Option<Trap> {
        match self.kind {
            StepKind::Retired | StepKind::Idle => None,
            StepKind::TrapBefore(trap) | StepKind::TrapDuring(trap) => Some(trap),
        }
    }
}

pub trait Core {
    /// Interrupt input as the SoC computes it: the Xtensa takes its 32 level lines as a mask,
    /// the RISC-V takes the one line its interrupt controller has arbitrated.
    type Irq: Copy + PartialEq + Default;

    fn reset(&mut self);
    fn pc(&self) -> u32;
    fn set_pc(&mut self, pc: u32);
    /// Halted by `waiti`/`wfi` until an interrupt arrives.
    fn waiting(&self) -> bool;
    fn insn_count(&self) -> u64;
    /// Present the SoC's interrupt state. Called after the machine re-derives it; must be cheap.
    fn set_irq(&mut self, irq: Self::Irq);
    /// An interrupt could be taken now (the idle skip is only allowed when this is false).
    fn irq_pending(&self) -> bool;
    /// The lines asserted in an `Irq` value, one bit per line (for observers).
    fn irq_bits(irq: &Self::Irq) -> u32;
    /// Let cycles pass without retiring an instruction. This advances architectural cycle
    /// counters and raises core-local timer interrupts that fall due.
    fn advance_cycles(&mut self, cycles: u32);
    /// Experimental uniform instruction cost for the fast block path.
    fn set_approximate_cpi(&mut self, _cycles: u32) {}
    /// Whether a single `step` already advances the cycle counter by the approximate CPI.
    /// When false, the approximate scheduler tops each step up by CPI-1 itself.
    fn step_charges_cpi(&self) -> bool { false }
    /// EX138: collect the priced control-flow cycles accrued since the last call.
    fn take_timing_extra(&mut self) -> u32 { 0 }
    /// Existing no-model idle accounting. Cores may count scheduler-skipped time as host work;
    /// model-added cycle deltas use `advance_cycles` and never call this method.
    fn idle_advance(&mut self, cycles: u32) { self.advance_cycles(cycles); }
    /// Cycles until a sleeping core's own timer can wake it. The value may be 2^32 when a
    /// wrapping 32-bit comparison register equals the current counter.
    fn cycles_until_wake(&self) -> Option<u64> { None }
    /// Execute one slow-path event and return the facts needed by a machine-level timing model.
    fn step<B: Bus>(&mut self, bus: &mut B) -> StepOutcome;
    /// Execute up to `budget` instructions the fast way (blocks, JIT). Returns the iterations a
    /// loop over `step` would have consumed — executed instructions, plus one for a trap taken
    /// before an instruction ran — and the trap that ended the run, if any.
    fn run<B: Bus>(&mut self, bus: &mut B, budget: u32) -> (u32, Option<Trap>) {
        for i in 0..budget { if let Some(t) = self.step(bus).trap() { return (i + 1, Some(t)); } }
        (budget, None)
    }
    /// lane-s2b: `run` for a dispatch the core has already prepared (the start a quantum-ending
    /// exit left), when nothing it does not check itself can intervene there; `None` otherwise.
    fn run_prepared<B: Bus>(&mut self, _bus: &mut B, _budget: u32) -> Option<(u32, Option<Trap>)> { None }
    /// pcs the machine intercepts (stubs, probes) as a bloom over `pc_bit`: a fast path must
    /// stop at every one of them so the machine can look.
    fn set_boundaries(&mut self, _bloom: u64) {}
    /// Preserve individual fast-path block callbacks instead of combining repeated blocks.
    fn set_block_observation(&mut self, _enabled: bool) {}
    /// Throw away decoded/compiled code (after loading an image or changing boundaries).
    fn flush_caches(&mut self) {}
    /// Enable/disable native code generation, if the core has it (`--no-jit`: the interpreter is the oracle).
    fn set_jit(&mut self, _on: bool) {}
    /// (blocks built, cache flushes, blocks compiled, native code bytes) for the end-of-run report.
    fn code_cache_stats(&self) -> Option<(u64, u64, u64, usize)> { None }
    /// Registers worth printing in a trace line, in the core's conventional order.
    fn regs(&self, out: &mut Vec<(&'static str, u32)>);
    /// Argument `n` of the function about to be entered, per the core's calling convention
    /// (RISC-V: a0 + n; Xtensa: the frame the call set up, which the instruction at pc decides —
    /// hence the bus). For function probes and stubs.
    fn arg<B: Bus>(&self, bus: &mut B, n: usize) -> u32;
    /// Return from the function about to be entered with `v`, as if it ran: the stub mechanism.
    fn return_from_stub<B: Bus>(&mut self, bus: &mut B, v: u32);
    /// Disassemble the instruction bytes at `pc` for a trace line.
    fn disasm(&self, pc: u32, bytes: [u8; 4]) -> String;
    /// Length in bytes of the instruction these bytes start (for walking a listing).
    fn insn_len(bytes: [u8; 4]) -> u32;
    /// Column width of the disassembly in a trace line.
    const TRACE_WIDTH: usize;
    /// The registers a trace line shows after the disassembly, e.g. `a0=... ps=... wb=...`.
    fn trace_regs(&self) -> String;
    /// A trace line for a trap just taken, if the core annotates them.
    fn trace_trap(&self, _core: usize, _pc: u32, _trap: &Trap) -> Option<String> { None }
    /// One line of the compact register trace used for hardware comparison (`--regtrace`).
    fn regtrace_line(&self, pc: u32) -> String;
    /// The full register dump for the end-of-run report; `sym` names an address.
    fn dump(&self, core: usize, sym: &dyn Fn(u32) -> String) -> String;
    /// Whether the guest has installed a trap handler (an `ebreak` without one is a stop).
    fn has_trap_handler(&self) -> bool { true }
    /// The function-probe argument summary, e.g. `a2=.. a3=.. a4=..`, and the return address.
    fn probe_args<B: Bus>(&self, bus: &mut B) -> String;
    fn return_address<B: Bus>(&self, bus: &mut B) -> u32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryAccessKind { Fetch, Read, Write }

/// One CPU-originated bus access. Faulting accesses keep the attempted address, width and value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryAccess {
    pub kind: MemoryAccessKind,
    pub address: u32,
    pub width: u8,
    pub value: u32,
    pub fault: Option<Fault>,
}

/// The complete facts for one slow-path execution event. Accesses are in program order and
/// begin with one conceptual fetch whenever `outcome.bytes` is present.
pub struct ExecutionFacts<'a> {
    pub core: usize,
    pub outcome: StepOutcome,
    pub accesses: &'a [MemoryAccess],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleKind { Attach, ChipReset, CoreReset(usize) }

/// Facts that do not belong to an instruction. Initial attachment is supported only before
/// execution, reset or synthetic app boot. The model owns the defaults for each chip configuration
/// it accepts; configuration changes made by guest code arrive as bus accesses.
pub struct LifecycleFacts {
    pub kind: LifecycleKind,
    pub chip: &'static str,
    pub cores: usize,
    pub cpu_hz: u64,
}

/// Prices slow-path execution events for the shared-time machine scheduler.
pub trait CostModel {
    fn lifecycle(&mut self, facts: &LifecycleFacts) -> Result<(), String>;
    fn cycles(&mut self, facts: &ExecutionFacts<'_>) -> Result<u32, String>;
    /// Shared simulated time for models that account for resource occupancy.
    /// Like `cycles`, this is called after instruction effects, not before memory access.
    fn cycles_at(&mut self, facts: &ExecutionFacts<'_>, _now: u64) -> Result<u32, String> {
        self.cycles(facts)
    }
}

/// Bloom bit for a pc; the machine's stub/probe tables and the cores' block boundaries agree on it.
#[inline(always)]
pub fn pc_bit(pc: u32) -> u64 { 1u64 << ((pc >> 2) & 63) }
