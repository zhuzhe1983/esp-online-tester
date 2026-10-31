//! `emu_core::Core` for the RV32 cores (C3, C6, P4): one instruction per `step`, no decode cache,
//! block cache or JIT; `run` loops `step` and stops where a block would. The interrupt input is
//! the one line the SoC's interrupt controller has already arbitrated; on a CLIC core (the P4) it
//! also carries the level and the vectoring attribute (`exec::clic_take`).
use crate::bus::Bus;
use crate::exec::Trap;
use crate::state::Cpu;
use emu_core::StepOutcome;

const X: [&str; 32] = ["zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4", "a5",
                       "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6"];

impl emu_core::Core for Cpu {
    type Irq = Option<u32>;
    fn reset(&mut self) { Cpu::reset(self) }
    fn pc(&self) -> u32 { self.pc }
    fn set_pc(&mut self, pc: u32) { self.pc = pc; }
    fn waiting(&self) -> bool { self.waiting }
    fn insn_count(&self) -> u64 { self.insn_count }
    fn set_irq(&mut self, line: Option<u32>) { self.irq = line; }
    /// A CLIC core in CLINT mode takes nothing the SoC presents (`exec::step_outcome`).
    fn irq_pending(&self) -> bool { self.irq.is_some() && (!self.clic || self.clic_mode()) }
    fn irq_bits(irq: &Option<u32>) -> u32 { irq.map_or(0, |l| 1 << (l & 31)) }
    fn advance_cycles(&mut self, cycles: u32) { self.cycle_count += cycles as u64; }
    fn idle_advance(&mut self, cycles: u32) { self.insn_count += cycles as u64; self.cycle_count += cycles as u64; }
    fn step<B: Bus>(&mut self, bus: &mut B) -> StepOutcome { crate::exec::step_outcome(self, bus) }
    fn set_boundaries(&mut self, bloom: u64) { self.boundary_bloom = bloom; }
    /// One instruction at a time, but like a block: stop when the bus reports that an interrupt
    /// line may have moved, so the machine re-derives the core's input before the next instruction,
    /// and at a pc the machine wants to see first (a stub or a probe), so it is exact there too.
    fn run<B: Bus>(&mut self, bus: &mut B, budget: u32) -> (u32, Option<Trap>) {
        for i in 0..budget {
            if let Err(t) = crate::exec::step(self, bus) { return (i + 1, Some(t)); }
            if bus.block_break() || (self.boundary_bloom != 0 && self.boundary_bloom & emu_core::core::pc_bit(self.pc) != 0) { return (i + 1, None); }
        }
        (budget, None)
    }
    fn regs(&self, out: &mut Vec<(&'static str, u32)>) { for (&name, &value) in X[1..].iter().zip(&self.x[1..]) { out.push((name, value)); } out.push(("mstatus", self.mstatus)); }
    fn arg<B: emu_core::Bus>(&self, _bus: &mut B, n: usize) -> u32 { self.x[10 + n] }
    fn return_from_stub<B: emu_core::Bus>(&mut self, _bus: &mut B, v: u32) { self.x[10] = v; self.pc = self.x[1]; self.insn_count += 1; self.retired_count += 1; self.cycle_count += 1; }
    fn disasm(&self, pc: u32, bytes: [u8; 4]) -> String { crate::disasm::format_for(self.misa, &crate::decode::decode_for(self.misa, pc, bytes)).replace('\t', " ") }
    fn insn_len(bytes: [u8; 4]) -> u32 { crate::decode::decode(0, bytes).len as u32 }
    const TRACE_WIDTH: usize = 28;
    fn trace_regs(&self) -> String { format!("ra={:08x} sp={:08x} a0={:08x} a1={:08x}", self.x[1], self.x[2], self.x[10], self.x[11]) }
    fn regtrace_line(&self, pc: u32) -> String {
        let mut s = format!("{:08x}", pc);
        for value in &self.x[1..] { s += &format!(" {:08x}", value); }
        s += &format!(" {:08x}", self.mstatus);
        s
    }
    fn dump(&self, core: usize, sym: &dyn Fn(u32) -> String) -> String {
        format!("core{}: pc={:#010x} {}  mtvec={:#010x} mcause={:#010x} mepc={:#010x} mstatus={:#010x} insns={}\n", core, self.pc, sym(self.pc), self.mtvec, self.mcause, self.mepc, self.mstatus, self.insn_count)
    }
    fn has_trap_handler(&self) -> bool { self.mtvec != 0 }
    fn probe_args<B: emu_core::Bus>(&self, _bus: &mut B) -> String { format!("a0={:#x} a1={:#x} a2={:#x}", self.x[10], self.x[11], self.x[12]) }
    fn return_address<B: emu_core::Bus>(&self, _bus: &mut B) -> u32 { self.x[1] }
}

#[cfg(test)]
mod tests {
    use emu_core::{CacheOperation, ControlEventKind, Core, FlatRam, StepKind, Trap};
    use crate::state::{csr, mstatus};
    /// `addi x1, x0, 5; j .` through the trait.
    #[test]
    fn core_runs_steps() {
        let mut ram = FlatRam::new(0x4038_0000, 64);
        ram.mem[..8].copy_from_slice(&[0x93, 0x00, 0x50, 0x00, 0x6f, 0x00, 0x00, 0x00]);   // addi ra,zero,5 ; j 0
        let mut cpu = crate::Cpu::new(); cpu.pc = 0x4038_0000;
        let (used, trap) = cpu.run(&mut ram, 4);
        assert_eq!((used, trap), (4, None)); assert_eq!(cpu.x[1], 5); assert_eq!(Core::pc(&cpu), 0x4038_0004);
        let mut r = Vec::new(); cpu.regs(&mut r); assert_eq!(r[0], ("ra", 5));
    }

    #[test]
    fn step_facts_keep_compressed_and_full_fetch_windows() {
        let base = 0x4038_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0x21, 0x43, 0xaa, 0xbb]); // c.li t1, 8
        let mut cpu = crate::Cpu::new(); cpu.pc = base;
        let compressed = cpu.step(&mut ram);
        assert_eq!((compressed.pc, compressed.next_pc, compressed.bytes, compressed.length, compressed.kind),
            (base, base + 2, Some([0x21, 0x43, 0xaa, 0xbb]), 2, StepKind::Retired));

        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0xb7, 0x02, 0x38, 0x40]); // lui t0, 0x40380
        let mut cpu = crate::Cpu::new(); cpu.pc = base;
        let full = cpu.step(&mut ram);
        assert_eq!((full.bytes, full.length, full.kind), (Some([0xb7, 0x02, 0x38, 0x40]), 4, StepKind::Retired));
    }

    #[test]
    fn step_facts_distinguish_interrupts_and_trapping_instructions() {
        let base = 0x4038_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0x73, 0, 0, 0]); // ecall
        let mut cpu = crate::Cpu::new(); cpu.pc = base; cpu.mtvec = base + 0x20;
        cpu.mstatus |= mstatus::MIE; cpu.irq = Some(3);
        let interrupt = cpu.step(&mut ram);
        assert_eq!(interrupt.bytes, None);
        assert_eq!(interrupt.kind, StepKind::TrapBefore(Trap::Interrupt(3)));
        assert_eq!((cpu.insn_count, cpu.retired_count, cpu.cycle_count), (1, 0, 1));

        cpu.pc = base; cpu.irq = None;
        let ecall = cpu.step(&mut ram);
        assert_eq!((ecall.bytes, ecall.length), (Some([0x73, 0, 0, 0]), 4));
        assert_eq!(ecall.kind, StepKind::TrapDuring(Trap::Exception(11)));
        assert_eq!((cpu.insn_count, cpu.retired_count, cpu.cycle_count), (2, 0, 2));
    }

    #[test]
    fn timing_only_advance_separates_cycles_from_retirement() {
        let base = 0x4038_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..8].copy_from_slice(&[0x93, 0x00, 0x50, 0x00, 0x73, 0, 0, 0]); // addi ra,zero,5; ecall
        let mut cpu = crate::Cpu::new(); cpu.pc = base; cpu.mtvec = base + 0x20;
        assert_eq!(cpu.step(&mut ram).kind, StepKind::Retired);
        cpu.advance_cycles(9);
        assert_eq!((cpu.insn_count, cpu.retired_count, cpu.cycle_count), (1, 1, 10));
        let instret = cpu.read_csr(csr::MINSTRET); let cycles = cpu.read_csr(csr::MCYCLE);
        assert_eq!((instret, cycles), (1, 10));
        cpu.idle_advance(4);
        assert_eq!((cpu.insn_count, cpu.retired_count, cpu.cycle_count), (5, 1, 14));
        assert!(matches!(cpu.step(&mut ram).kind, StepKind::TrapDuring(Trap::Exception(11))));
        assert_eq!((cpu.insn_count, cpu.retired_count, cpu.cycle_count), (6, 1, 15));
        let instret = cpu.read_csr(csr::MINSTRET); let cycles = cpu.read_csr(csr::MCYCLE);
        assert_eq!((instret, cycles), (1, 15));
    }

    #[test]
    fn fence_i_is_an_explicit_cache_control_event() {
        let base = 0x4038_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0x0f, 0x10, 0, 0]);
        let mut cpu = crate::Cpu::new(); cpu.pc = base;
        let event = cpu.step(&mut ram).control.unwrap();
        assert_eq!(event.kind, ControlEventKind::Cache(CacheOperation::FenceInstruction));
        assert_eq!(event.address, 0);
    }

    #[test]
    fn stub_return_preserves_guest_counter_accounting() {
        let mut ram = FlatRam::new(0x4038_0000, 64);
        let mut cpu = crate::Cpu::new();
        cpu.x[1] = 0x4038_0100;
        cpu.return_from_stub(&mut ram, 7);
        assert_eq!((cpu.pc, cpu.x[10]), (0x4038_0100, 7));
        assert_eq!((cpu.insn_count, cpu.retired_count, cpu.cycle_count), (1, 1, 1));
        let instret = cpu.read_csr(csr::MINSTRET); let cycles = cpu.read_csr(csr::MCYCLE);
        assert_eq!((instret, cycles), (1, 1));
    }

    #[test]
    fn counter_writes_wrap_without_debug_overflow() {
        let mut cpu = crate::Cpu::new();
        cpu.write_csr(csr::MCYCLE, 0xffff_fff0);
        cpu.write_csr(csr::MCYCLEH, 0x8000_0000);
        assert_eq!(cpu.read_csr(csr::MCYCLE), 0xffff_fff0);
        assert_eq!(cpu.read_csr(csr::MCYCLEH), 0x8000_0000);
        cpu.write_csr(csr::MINSTRET, 0xffff_ffe0);
        cpu.write_csr(csr::MINSTRETH, 0x9000_0000);
        assert_eq!(cpu.read_csr(csr::MINSTRET), 0xffff_ffe0);
        assert_eq!(cpu.read_csr(csr::MINSTRETH), 0x9000_0000);
    }
}
