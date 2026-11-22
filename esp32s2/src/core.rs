//! S2 option gate around the shared Xtensa integer interpreter, never the S3 JIT.
//! ESP-IDF v5.5.4 esp32s2/config/core-isa.h: 64 ARs, windowed/density,
//! MUL/DIV/NSA/MINMAX/SEXT/CLAMPS, but no LOOP/BOOL/FPU/MAC16/S32C1I/PIE.
use emu_core::{Bus, Core, StepKind, StepOutcome, Trap};
use xtensa_lx7::{Cpu, decode::{decode, Insn, Op}, state::sr};

pub struct S2Core { inner: Cpu }
impl Default for S2Core { fn default() -> Self { Self::new() } }
impl S2Core {
    pub fn new() -> Self {
        // PRID 0 is the diagnostic profile's single core index, not a measured
        // silicon identity. CONFIGID values are the S2 generated ISA header.
        let mut inner = Cpu::new(0);
        inner.configid = [0xc2ec_fafe, 0x2207_5f76];
        inner.pc = crate::bus::RESET_VECTOR;
        inner.set_jit(false);
        Self { inner }
    }
    fn allowed(i: &Insn) -> bool {
        use Op::*;
        match i.op {
            Ill | IllN | Nop | NopN | Break | BreakN | Syscall | Simcall | Waiti | Rsil |
            Isync | Rsync | Esync | Dsync | Excw | Memw | Extw |
            J | Jx | Call0 | Call4 | Call8 | Call12 | Callx0 | Callx4 | Callx8 | Callx12 |
            Ret | RetN | Retw | RetwN | Entry | Movsp | Rotw | Rfe | Rfue | Rfde | Rfwo | Rfwu | Rfi |
            L32e | S32e | Beqz | Bnez | Bltz | Bgez | BeqzN | BnezN |
            Beqi | Bnei | Blti | Bgei | Bltui | Bgeui |
            Bnone | Beq | Blt | Bltu | Ball | Bbc | Bbci | Bany | Bne | Bge | Bgeu | Bnall | Bbs | Bbsi |
            L8ui | L16ui | L16si | L32i | L32iN | L32r | L32ai | S8i | S16i | S32i | S32iN | S32ri |
            Movi | MoviN | Mov | MovN | Add | AddN | Addi | AddiN | Addmi | Sub | Addx2 | Addx4 | Addx8 | Subx2 | Subx4 | Subx8 |
            And | Or | Xor | Neg | Abs | Extui | Sext | Clamps | Min | Max | Minu | Maxu |
            Moveqz | Movnez | Movltz | Movgez | Slli | Srai | Srli | Sll | Srl | Sra | Src |
            Ssr | Ssl | Ssa8l | Ssa8b | Ssai | Nsa | Nsau | Mull | Muluh | Mulsh | Mul16u | Mul16s | Quou | Quos | Remu | Rems => true,
            // Deliberately limited special-register surface: no unsupported loop,
            // coprocessor, cache/MMU, external-register or IRQ-controller behavior.
            Rsr | Wsr | Xsr => matches!(i.imm as u32,
                sr::SAR | sr::WINDOWBASE | sr::WINDOWSTART | sr::CONFIGID0 | sr::CONFIGID1 |
                177..=183 | sr::DEPC | 194..=199 | 209..=215 | sr::PS | sr::VECBASE |
                sr::EXCCAUSE | sr::DEBUGCAUSE | sr::CCOUNT | sr::PRID | sr::EXCVADDR | 244..=247),
            Rur | Wur => i.imm == 231, // THREADPTR only; S3 TIE/FPU user registers are absent.
            _ => false,
        }
    }
}
impl std::ops::Deref for S2Core { type Target = Cpu; fn deref(&self) -> &Cpu { &self.inner } }
impl std::ops::DerefMut for S2Core { fn deref_mut(&mut self) -> &mut Cpu { &mut self.inner } }
impl Core for S2Core {
    type Irq = u32;
    fn reset(&mut self) { self.inner.reset(); self.inner.configid = [0xc2ec_fafe, 0x2207_5f76]; self.inner.pc = crate::bus::RESET_VECTOR; self.inner.set_jit(false); }
    fn pc(&self) -> u32 { self.inner.pc }
    fn set_pc(&mut self, pc: u32) { self.inner.pc = pc; }
    fn waiting(&self) -> bool { self.inner.waiting }
    fn insn_count(&self) -> u64 { self.inner.insn_count }
    fn set_irq(&mut self, _irq: u32) {} // The S2 interrupt matrix and core timers are outside this diagnostic subset.
    fn irq_pending(&self) -> bool { false }
    fn irq_bits(_irq: &u32) -> u32 { 0 }
    fn advance_cycles(&mut self, cycles: u32) { self.inner.ccount = self.inner.ccount.wrapping_add(cycles); }
    fn set_approximate_cpi(&mut self, cycles: u32) { self.inner.set_approximate_cpi(cycles); }
    fn step_charges_cpi(&self) -> bool { true }
    fn take_timing_extra(&mut self) -> u32 { self.inner.take_timing_extra() }
    fn step<B: Bus>(&mut self, bus: &mut B) -> StepOutcome {
        let pc = self.inner.pc;
        if !self.inner.waiting {
            if let Ok(bytes) = bus.fetch(pc) {
                let i = decode(pc, bytes);
                if i.len > 3 || !Self::allowed(&i) {
                    return StepOutcome { pc, next_pc: pc, bytes: Some(bytes), length: i.len,
                        kind: StepKind::TrapBefore(Trap::Unimplemented(pc, i.raw)), control: None };
                }
            }
        }
        // Do not delegate run/run_prepared: each executed instruction must pass
        // the S2 gate. No shared interpreter/JIT hot path is modified.
        self.inner.step(bus)
    }
    fn flush_caches(&mut self) { self.inner.flush_caches(); }
    fn set_jit(&mut self, _on: bool) { self.inner.set_jit(false); }
    fn code_cache_stats(&self) -> Option<(u64, u64, u64, usize)> { self.inner.code_cache_stats() }
    fn regs(&self, out: &mut Vec<(&'static str, u32)>) { self.inner.regs(out); }
    fn arg<B: Bus>(&self, bus: &mut B, n: usize) -> u32 { self.inner.arg(bus, n) }
    fn return_from_stub<B: Bus>(&mut self, bus: &mut B, value: u32) { self.inner.return_from_stub(bus, value); }
    fn disasm(&self, pc: u32, bytes: [u8; 4]) -> String { self.inner.disasm(pc, bytes) }
    fn insn_len(bytes: [u8; 4]) -> u32 { Cpu::insn_len(bytes) }
    const TRACE_WIDTH: usize = Cpu::TRACE_WIDTH;
    fn trace_regs(&self) -> String { self.inner.trace_regs() }
    fn trace_trap(&self, core: usize, pc: u32, trap: &Trap) -> Option<String> { self.inner.trace_trap(core, pc, trap) }
    fn regtrace_line(&self, pc: u32) -> String { self.inner.regtrace_line(pc) }
    fn dump(&self, core: usize, sym: &dyn Fn(u32) -> String) -> String { self.inner.dump(core, sym) }
    fn probe_args<B: Bus>(&self, bus: &mut B) -> String { self.inner.probe_args(bus) }
    fn return_address<B: Bus>(&self, bus: &mut B) -> u32 { self.inner.return_address(bus) }
}
