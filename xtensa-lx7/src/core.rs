//! `emu_core::Core` for the LX7: the machine-facing surface over `Cpu`, `step` and the block
//! interpreter. Nothing here changes behaviour; each method is the line the S3 machine used to
//! write itself.
use crate::bus::Bus;
use crate::exec::Trap;
use crate::state::{Cpu, EXCM_LEVEL, INT_ABOVE, INTTYPE_EDGE, INTTYPE_LEVEL, INTTYPE_NMI, TIMER_INTERRUPT};
use emu_core::StepOutcome;

const AR: [&str; 16] = ["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9", "a10", "a11", "a12", "a13", "a14", "a15"];

impl emu_core::Core for Cpu {
    /// The 32 interrupt lines after the interrupt matrix. External level inputs follow the
    /// line state; external edge and NMI inputs latch rising edges inside the core.
    type Irq = u32;
    fn reset(&mut self) { Cpu::reset(self) }
    fn pc(&self) -> u32 { self.pc }
    fn set_pc(&mut self, pc: u32) { self.pc = pc; }
    fn waiting(&self) -> bool { self.waiting }
    fn insn_count(&self) -> u64 { self.insn_count }
    fn set_irq(&mut self, lines: u32) {
        let rising = lines & !self.ext_irq_lines & (INTTYPE_EDGE | INTTYPE_NMI);
        self.interrupt = (self.interrupt & !INTTYPE_LEVEL) | (lines & INTTYPE_LEVEL) | rising;
        self.ext_irq_lines = lines;
    }
    fn irq_pending(&self) -> bool { self.check_interrupts_pending() != 0 }
    fn irq_bits(irq: &u32) -> u32 { *irq }
    fn advance_cycles(&mut self, cycles: u32) { self.advance_ccount(cycles) }
    fn set_approximate_cpi(&mut self, cycles: u32) { self.approximate_cpi = cycles.max(1); }
    fn step_charges_cpi(&self) -> bool { true }
    fn take_timing_extra(&mut self) -> u32 { std::mem::take(&mut self.timing_extra) }
    /// EX257: a sleeping core with no CCOMPARE line both enabled and not yet raised (the usual
    /// idle core: the RTOS tick comes from a device timer) answers here, inline in the scheduler's
    /// idle round; any other core searches its armed lines in `timer_wake`, out of line.
    #[inline]
    fn cycles_until_wake(&self) -> Option<u64> {
        let armed = self.intenable & !self.interrupt & TIMER_LINES;
        if !self.waiting || armed == 0 { return None; }
        self.timer_wake(armed)
    }
    fn step<B: Bus>(&mut self, bus: &mut B) -> StepOutcome { crate::exec::step_outcome(self, bus) }
    fn run<B: Bus>(&mut self, bus: &mut B, budget: u32) -> (u32, Option<Trap>) { crate::block::run_block(self, bus, budget) }
    #[cfg(target_arch = "wasm32")]
    fn run_prepared<B: Bus>(&mut self, bus: &mut B, budget: u32) -> Option<(u32, Option<Trap>)> { crate::block::run_memo(self, bus, budget) }
    fn set_boundaries(&mut self, bloom: u64) { if self.boundary_bloom != bloom { self.blocks.flush(); self.boundary_bloom = bloom; } }
    fn set_block_observation(&mut self, enabled: bool) { self.blocks.observed = enabled; }
    fn flush_caches(&mut self) { self.blocks.flush(); }
    fn set_jit(&mut self, on: bool) { self.blocks.jit_enabled = on; }
    fn code_cache_stats(&self) -> Option<(u64, u64, u64, usize)> { Some((self.blocks.builds, self.blocks.flushes, self.blocks.compiled, self.blocks.code_bytes())) }
    fn regs(&self, out: &mut Vec<(&'static str, u32)>) {
        for (i, n) in AR.iter().enumerate() { out.push((n, self.get_ar(i as u8))); }
        out.push(("ps", self.ps)); out.push(("wb", self.windowbase));
    }
    /// At a function's entry the window is still the caller's. A windowed function — it begins
    /// with `entry` — was reached by a `callN` that left the return address in a(4N) and the
    /// arguments from a(4N+2), N in PS.CALLINC. A call0-ABI function (ROM assembly, code built
    /// `-mno-windowed`) has them in a0/a2 whatever CALLINC still holds: `call0` does not touch
    /// it, on silicon or here.
    fn arg<B: emu_core::Bus>(&self, bus: &mut B, n: usize) -> u32 { self.get_ar(self.frame_base(bus) + 2 + n as u8) }
    /// Synthetic return from a function entry whose `entry` has not executed: the value goes
    /// where the caller will read it, the pc comes from the frame's return address, and there
    /// is no window rotation to undo.
    fn return_from_stub<B: emu_core::Bus>(&mut self, bus: &mut B, v: u32) {
        let w = self.frame_base(bus);
        let ra = self.get_ar(w);
        self.set_ar(w + 2, v);
        self.pc = (ra & 0x3fff_ffff) | (self.pc & 0xc000_0000);
        self.insn_count += 1; self.advance_ccount(1);
    }
    fn disasm(&self, pc: u32, bytes: [u8; 4]) -> String { crate::disasm::format(&crate::decode::decode(pc, bytes)) }
    fn insn_len(bytes: [u8; 4]) -> u32 { crate::decode::decode(0, bytes).len as u32 }
    const TRACE_WIDTH: usize = 32;
    fn trace_regs(&self) -> String { format!("a0={:08x} a1={:08x} a2={:08x} a3={:08x} ps={:06x} wb={}", self.get_ar(0), self.get_ar(1), self.get_ar(2), self.get_ar(3), self.ps, self.windowbase) }
    fn trace_trap(&self, core: usize, pc: u32, trap: &Trap) -> Option<String> {
        match trap {
            Trap::Exception(c) => Some(format!("          ** core{} exception cause {} at {:08x} -> {:08x} (excvaddr {:08x})", core, c, pc, self.pc, self.excvaddr)),
            Trap::Interrupt(irq) => Some(format!("          ** core{} interrupt {} at {:08x} -> {:08x}", core, irq, pc, self.pc)),
            _ => None,
        }
    }
    fn regtrace_line(&self, pc: u32) -> String {
        let mut s = format!("{:08x}", pc);
        for i in 0..16u8 { s += &format!(" {:08x}", self.get_ar(i)); }
        s += &format!(" {:08x} {:x}", self.ps, self.windowbase);
        s
    }
    fn dump(&self, core: usize, sym: &dyn Fn(u32) -> String) -> String {
        let c = self;
        let mut s = format!("core{}: ", core);
        s += &format!("pc={:08x} {}  ps={:08x} wb={} ws={:04x} sar={} lcount={} exccause={} excvaddr={:08x} epc1={:08x} intenable={:08x} interrupt={:08x} ccount={} insns={}\n",
            c.pc, sym(c.pc), c.ps, c.windowbase, c.windowstart, c.sar, c.lcount, c.exccause, c.excvaddr, c.epc[1], c.intenable, c.interrupt, c.ccount, c.insn_count);
        for i in 0..16 { s += &format!("a{:<2}={:08x} ", i, c.get_ar(i)); if i % 8 == 7 { s += "\n"; } }
        s
    }
    fn probe_args<B: emu_core::Bus>(&self, bus: &mut B) -> String { format!("a2={:#x} a3={:#x} a4={:#x}", self.arg(bus, 0), self.arg(bus, 1), self.arg(bus, 2)) }
    fn return_address<B: emu_core::Bus>(&self, bus: &mut B) -> u32 { (self.get_ar(self.frame_base(bus)) & 0x3fff_ffff) | (self.pc & 0xc000_0000) }
}

/// INTENABLE/INTERRUPT bits of the three CCOMPARE lines.
const TIMER_LINES: u32 = (1 << TIMER_INTERRUPT[0]) | (1 << TIMER_INTERRUPT[1]) | (1 << TIMER_INTERRUPT[2]);

impl Cpu {
    /// Cycles until the nearest CCOMPARE match among `armed` (enabled, not yet raised timer lines)
    /// that PS.INTLEVEL/EXCM does not mask.
    #[inline(never)]
    fn timer_wake(&self, armed: u32) -> Option<u64> {
        let mask_level = if self.excm() { self.intlevel().max(EXCM_LEVEL) } else { self.intlevel() };
        let armed = armed & INT_ABOVE[mask_level as usize];
        self.ccompare.iter().zip(TIMER_INTERRUPT).filter_map(|(&compare, irq)| {
            if armed & (1 << irq) == 0 { return None; }
            let delta = compare.wrapping_sub(self.ccount);
            Some(if delta == 0 { 1u64 << 32 } else { delta as u64 })
        }).min()
    }

    /// Register offset of the frame a stub or probe at pc sees: 4·CALLINC when the function
    /// starts with `entry` (a windowed call is pending), 0 otherwise (call0 ABI).
    fn frame_base<B: emu_core::Bus>(&self, bus: &mut B) -> u8 {
        match bus.fetch(self.pc) {
            Ok(bytes) if crate::decode::decode(self.pc, bytes).op == crate::decode::Op::Entry => self.call_window(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use emu_core::{Bus, CacheOperation, ControlEventKind, Core, Fault, FlatRam, StepKind, TlbOperation, Trap};
    use crate::state::{exc, sr, TIMER_INTERRUPT};

    #[test]
    fn offchip_lines_preserve_core_interrupts_and_track_levels() {
        use crate::state::{INTTYPE_LEVEL, INTTYPE_PROFILING, INTTYPE_SOFTWARE, INTTYPE_TIMER};
        let mut cpu = crate::Cpu::new(0);
        let internal = INTTYPE_SOFTWARE | INTTYPE_TIMER | INTTYPE_PROFILING;
        // The interrupt matrix cannot manufacture internal timer/software/profiling requests.
        cpu.set_irq(internal);
        assert_eq!(cpu.interrupt, 0);
        cpu.interrupt = internal;
        cpu.set_irq(INTTYPE_LEVEL);
        assert_eq!(cpu.interrupt, internal | INTTYPE_LEVEL);
        cpu.set_irq(0);
        assert_eq!(cpu.interrupt, internal);
    }

    #[test]
    fn offchip_edges_latch_until_cleared_and_require_a_new_rising_edge() {
        use crate::state::{sr, INTTYPE_EDGE};
        let mut cpu = crate::Cpu::new(0);
        cpu.set_irq(INTTYPE_EDGE);
        cpu.set_irq(0);
        assert_eq!(cpu.interrupt, INTTYPE_EDGE, "deasserting the input keeps the edge latch");
        cpu.write_sr(sr::INTCLEAR, INTTYPE_EDGE);
        assert_eq!(cpu.interrupt, 0);
        cpu.set_irq(INTTYPE_EDGE);
        assert_eq!(cpu.interrupt, INTTYPE_EDGE);
        cpu.write_sr(sr::INTCLEAR, INTTYPE_EDGE);
        cpu.set_irq(INTTYPE_EDGE);
        assert_eq!(cpu.interrupt, 0, "a held input does not retrigger a cleared latch");
        cpu.set_irq(0);
        cpu.set_irq(INTTYPE_EDGE);
        assert_eq!(cpu.interrupt, INTTYPE_EDGE);
    }

    #[test]
    fn nmi_bypasses_masks_wakes_waiti_and_acknowledges_its_edge() {
        use crate::state::{ps, sr, vec, INTTYPE_NMI, NMI_INTERRUPT};
        for intlevel in [0, 3, 7, 15] {
            let mut cpu = crate::Cpu::new(0);
            cpu.ps = ps::EXCM | intlevel;
            cpu.intenable = 0;
            cpu.waiting = true;
            let (pc, saved_ps) = (cpu.pc, cpu.ps);
            cpu.set_irq(INTTYPE_NMI);
            cpu.write_sr(sr::INTCLEAR, INTTYPE_NMI);
            assert!(cpu.irq_pending());
            assert_eq!(cpu.check_interrupts(), Some(Trap::Interrupt(NMI_INTERRUPT)));
            assert_eq!((cpu.epc[7], cpu.eps[7], cpu.pc), (pc, saved_ps, cpu.vecbase + vec::NMI));
            assert!(!cpu.waiting);
            assert!(!cpu.irq_pending());
            cpu.set_irq(INTTYPE_NMI);
            assert_eq!(cpu.check_interrupts(), None, "a held NMI fires only once");
            cpu.set_irq(0);
            cpu.set_irq(INTTYPE_NMI);
            assert_eq!(cpu.check_interrupts(), Some(Trap::Interrupt(NMI_INTERRUPT)));
        }
    }

    #[test]
    fn reset_rearms_offchip_edge_detection() {
        use crate::state::{INTTYPE_EDGE, INTTYPE_NMI};
        let mut cpu = crate::Cpu::new(0);
        let edges = INTTYPE_EDGE | INTTYPE_NMI;
        cpu.set_irq(edges);
        cpu.reset();
        assert_eq!(cpu.interrupt, 0);
        cpu.set_irq(edges);
        assert_eq!(cpu.interrupt, edges);
    }
    /// `movi a2, 5; j .` through the trait, on the block path and the step path.
    #[test]
    fn approximate_block_cost_charges_ccount_and_cuts_at_timer() {
        let base = 0x4037_0000;
        let mut ram = FlatRam::new(base, 64);
        // Four movi.n instructions; timer deadline lies inside the second priced instruction.
        ram.mem[..8].copy_from_slice(&[0x0c, 0x12, 0x0c, 0x23, 0x0c, 0x34, 0x0c, 0x45]);
        let mut cpu = crate::Cpu::new(0);
        cpu.pc = base;
        cpu.ps = 0;
        cpu.write_sr(sr::CCOMPARE0, 5);
        Core::set_approximate_cpi(&mut cpu, 3);
        let (used, trap) = cpu.run(&mut ram, 8);
        assert_eq!(trap, None);
        assert_eq!(used, 2);
        assert_eq!(cpu.insn_count, 2);
        assert_eq!(cpu.ccount, 6);
        assert_ne!(cpu.interrupt & (1 << TIMER_INTERRUPT[0]), 0);
    }

    #[test]
    fn core_runs_a_block() {
        let mut ram = FlatRam::new(0x4037_0000, 64);
        ram.mem[..6].copy_from_slice(&[0x22, 0xa0, 0x05, 0x06, 0xff, 0xff]);   // movi a2,5 ; j -4 (to itself)
        let mut cpu = crate::Cpu::new(0);
        cpu.pc = 0x4037_0000; cpu.ps = 0;
        let (used, trap) = cpu.run(&mut ram, 8);
        assert_eq!(trap, None); assert!(used >= 2, "{}", used);
        assert_eq!(cpu.get_ar(2), 5); assert_eq!(Core::pc(&cpu), 0x4037_0003);
        let mut cpu2 = crate::Cpu::new(0); cpu2.pc = 0x4037_0000; cpu2.ps = 0;
        assert_eq!(cpu2.step(&mut ram).result(), Ok(())); assert_eq!(cpu2.get_ar(2), 5);
        let mut r = Vec::new(); cpu2.regs(&mut r); assert_eq!(r[2], ("a2", 5));
    }

    /// A stub at a windowed function's entry fires before its `entry` rotated the window, so
    /// the return goes through the frame the `call8` set up: value in a10, pc from a8, the
    /// caller's other registers and the window untouched.
    #[test]
    fn stub_returns_through_the_pending_call_window() {
        let base = 0x4037_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..3].copy_from_slice(&[0x25, 0x00, 0x00]);   // call8 base+4
        ram.mem[4..7].copy_from_slice(&[0x36, 0x41, 0x00]);  // entry a1, 32: a windowed function
        let mut cpu = crate::Cpu::new(0); cpu.pc = base; cpu.ps = crate::state::ps::WOE;
        cpu.set_ar(3, 0x1234); cpu.set_ar(10, 0xdead);
        assert_eq!(cpu.step(&mut ram).result(), Ok(()));
        assert_eq!(Core::pc(&cpu), base + 4);
        assert_eq!(cpu.return_address(&mut ram), base + 3);
        assert_eq!(cpu.arg(&mut ram, 0), 0xdead);
        let wb = cpu.windowbase;
        cpu.return_from_stub(&mut ram, 7);
        assert_eq!(Core::pc(&cpu), base + 3);
        assert_eq!((cpu.get_ar(10), cpu.get_ar(3), cpu.windowbase), (7, 0x1234, wb));
    }

    /// `call0` leaves PS.CALLINC as the last `callN` set it, so a stub on a call0-ABI function
    /// (no `entry`) must read a0/a2 even though CALLINC still says 2.
    #[test]
    fn stub_on_a_call0_function_ignores_a_stale_callinc() {
        let base = 0x4037_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..3].copy_from_slice(&[0x05, 0x00, 0x00]);   // call0 base+4
        ram.mem[4..7].copy_from_slice(&[0x22, 0xa0, 0x05]);  // movi a2, 5: no entry here
        let mut cpu = crate::Cpu::new(0); cpu.pc = base;
        cpu.ps = crate::state::ps::WOE | (2 << crate::state::ps::CALLINC_SHIFT);   // a call8 ran earlier
        cpu.set_ar(2, 0xbeef); cpu.set_ar(10, 0x1111);
        assert_eq!(cpu.step(&mut ram).result(), Ok(()));
        assert_eq!(Core::pc(&cpu), base + 4);
        assert_eq!(cpu.return_address(&mut ram), base + 3);
        assert_eq!(cpu.arg(&mut ram, 0), 0xbeef);
        cpu.return_from_stub(&mut ram, 9);
        assert_eq!(Core::pc(&cpu), base + 3);
        assert_eq!((cpu.get_ar(2), cpu.get_ar(10)), (9, 0x1111));
    }

    #[test]
    fn step_facts_keep_the_full_fetch_window_on_cache_hits() {
        let base = 0x4037_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0x0c, 0x03, 0xaa, 0xbb]);
        let mut cpu = crate::Cpu::new(0); cpu.pc = base; cpu.ps = 0;
        let first = cpu.step(&mut ram);
        assert_eq!((first.pc, first.next_pc, first.bytes, first.length, first.kind),
            (base, base + 2, Some([0x0c, 0x03, 0xaa, 0xbb]), 2, StepKind::Retired));

        cpu.pc = base;
        ram.mem[3] = 0xcc; // bypass Bus writes, so the decode-cache version stays valid
        let hit = cpu.step(&mut ram);
        assert_eq!(hit.bytes, Some([0x0c, 0x03, 0xaa, 0xbb]));

        cpu.pc = base;
        ram.write8(base + 3, 0xdd).unwrap();
        let invalidated = cpu.step(&mut ram);
        assert_eq!(invalidated.bytes, Some([0x0c, 0x03, 0xaa, 0xdd]));

        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0x22, 0xa0, 0x05, 0x7e]);
        let mut cpu = crate::Cpu::new(0); cpu.pc = base; cpu.ps = 0;
        let full = cpu.step(&mut ram);
        assert_eq!((full.bytes, full.length, full.kind), (Some([0x22, 0xa0, 0x05, 0x7e]), 3, StepKind::Retired));
    }

    struct PagedRam { base: u32, mem: [u8; 512], versions: [u32; 2] }
    impl PagedRam {
        fn off(&self, address: u32, width: usize) -> Result<usize, Fault> {
            let offset = address.wrapping_sub(self.base) as usize;
            if offset + width <= self.mem.len() { Ok(offset) } else { Err(Fault::Unmapped) }
        }
    }
    impl Bus for PagedRam {
        fn note_code_page(&mut self, _vidx: u32) {} // All writes already update versions, or this bus has no decode cache.
        fn read8(&mut self, address: u32) -> Result<u8, Fault> { let o = self.off(address, 1)?; Ok(self.mem[o]) }
        fn read16(&mut self, address: u32) -> Result<u16, Fault> { let o = self.off(address, 2)?; Ok(u16::from_le_bytes(self.mem[o..o + 2].try_into().unwrap())) }
        fn read32(&mut self, address: u32) -> Result<u32, Fault> { let o = self.off(address, 4)?; Ok(u32::from_le_bytes(self.mem[o..o + 4].try_into().unwrap())) }
        fn write8(&mut self, address: u32, value: u8) -> Result<(), Fault> { let o = self.off(address, 1)?; self.mem[o] = value; self.versions[o >> 8] += 1; Ok(()) }
        fn write16(&mut self, address: u32, value: u16) -> Result<(), Fault> { let o = self.off(address, 2)?; self.mem[o..o + 2].copy_from_slice(&value.to_le_bytes()); self.versions[o >> 8] += 1; self.versions[(o + 1) >> 8] += 1; Ok(()) }
        fn write32(&mut self, address: u32, value: u32) -> Result<(), Fault> { let o = self.off(address, 4)?; self.mem[o..o + 4].copy_from_slice(&value.to_le_bytes()); for p in o >> 8..=(o + 3) >> 8 { self.versions[p] += 1; } Ok(()) }
        fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> { let o = self.off(pc, 4)?; Ok(self.mem[o..o + 4].try_into().unwrap()) }
        fn page_versions(&self) -> &[u32] { &self.versions }
        fn code_page(&mut self, pc: u32) -> u32 { pc.wrapping_sub(self.base) >> 8 }
    }

    #[test]
    fn decode_cache_validates_the_whole_fetch_window() {
        let base = 0x4037_0000;
        let pc = base + 0xff;
        let mut ram = PagedRam { base, mem: [0; 512], versions: [0; 2] };
        ram.mem[0xff..0x103].copy_from_slice(&[0x0c, 0x03, 0xaa, 0xbb]);
        let mut cpu = crate::Cpu::new(0); cpu.pc = pc; cpu.ps = 0;
        assert_eq!(cpu.step(&mut ram).bytes, Some([0x0c, 0x03, 0xaa, 0xbb]));
        cpu.pc = pc; ram.write8(pc + 3, 0xcc).unwrap();
        assert_eq!(cpu.step(&mut ram).bytes, Some([0x0c, 0x03, 0xaa, 0xcc]));
    }

    #[test]
    fn step_facts_distinguish_interrupts_and_trapping_instructions() {
        let base = 0x4037_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0, 0, 0, 0xaa]);
        let mut cpu = crate::Cpu::new(0); cpu.pc = base; cpu.ps = 0;
        cpu.intenable = 1 << TIMER_INTERRUPT[0]; cpu.interrupt = 1 << TIMER_INTERRUPT[0];
        let interrupt = cpu.step(&mut ram);
        assert_eq!(interrupt.bytes, None);
        assert_eq!(interrupt.kind, StepKind::TrapBefore(Trap::Interrupt(TIMER_INTERRUPT[0])));
        assert_eq!((cpu.insn_count, cpu.ccount), (0, 0));

        cpu.pc = base; cpu.ps = 0; cpu.interrupt = 0;
        let illegal = cpu.step(&mut ram);
        assert_eq!((illegal.bytes, illegal.length), (Some([0, 0, 0, 0xaa]), 3));
        assert_eq!(illegal.kind, StepKind::TrapDuring(Trap::Exception(exc::ILLEGAL)));
        assert_eq!((cpu.insn_count, cpu.ccount), (1, 1));
    }

    #[test]
    fn step_facts_report_cache_and_tlb_effective_addresses() {
        let base = 0x4037_0000;
        let mut ram = FlatRam::new(base, 64);
        ram.mem[..4].copy_from_slice(&[0x52, 0x73, 0x04, 0xaa]); // dhwbi a3, 16
        let mut cpu = crate::Cpu::new(0); cpu.pc = base; cpu.ps = 0; cpu.set_ar(3, 0x3f80_0100);
        let cache = cpu.step(&mut ram).control.unwrap();
        assert_eq!(cache.kind, ControlEventKind::Cache(CacheOperation::DataHitWritebackInvalidate));
        assert_eq!(cache.address, 0x3f80_0110);

        ram.mem[..4].copy_from_slice(&[0x30, 0x33, 0x50, 0xaa]); // ritlb0 a3, a3
        ram.ver += 1; cpu.pc = base; cpu.set_ar(3, 0x3c00_1234);
        let tlb = cpu.step(&mut ram).control.unwrap();
        assert_eq!(tlb.kind, ControlEventKind::Tlb(TlbOperation::ReadInstructionEntry0));
        assert_eq!(tlb.address, 0x3c00_1234);
        assert_eq!(cpu.get_ar(3), 0, "the event retained the pre-execution address");
    }

    #[test]
    fn timing_only_advance_exposes_the_next_ccompare_wake() {
        let mut cpu = crate::Cpu::new(0);
        cpu.waiting = true; cpu.ps = 0; cpu.intenable = 1 << TIMER_INTERRUPT[0];
        cpu.write_sr(sr::CCOUNT, 0xffff_fffd); cpu.write_sr(sr::CCOMPARE0, 1);
        assert_eq!(cpu.cycles_until_wake(), Some(4));
        cpu.advance_cycles(3);
        assert_eq!(cpu.cycles_until_wake(), Some(1));
        assert_eq!(cpu.insn_count, 0);
        cpu.advance_cycles(1);
        assert_ne!(cpu.interrupt & (1 << TIMER_INTERRUPT[0]), 0);
        assert_eq!(cpu.insn_count, 0);
        cpu.interrupt = 0; cpu.write_sr(sr::CCOMPARE0, cpu.ccount);
        assert_eq!(cpu.cycles_until_wake(), Some(1u64 << 32));
    }

    /// EX257: the inline test in front of the wake search must pass every CCOMPARE line, and
    /// only for a sleeping core with that line enabled and not yet raised.
    #[test]
    fn wake_search_sees_each_ccompare_line_of_a_sleeping_core() {
        for (i, &line) in TIMER_INTERRUPT.iter().enumerate() {
            let mut cpu = crate::Cpu::new(0);
            cpu.ps = 0; cpu.write_sr(sr::CCOUNT, 100);
            cpu.write_sr(sr::CCOMPARE0 + i as u32, 100 + 7 + i as u32);
            cpu.intenable = 1 << line;
            assert_eq!(cpu.cycles_until_wake(), None, "line {line}: a running core does not wait");
            cpu.waiting = true;
            assert_eq!(cpu.cycles_until_wake(), Some(7 + i as u64), "line {line}");
            // PS.EXCM masks levels up to 3 (lines 6 and 15), INTLEVEL 5 masks all three
            cpu.ps = 1 << 4;
            assert_eq!(cpu.cycles_until_wake(), (i == 2).then_some(9), "line {line} under EXCM");
            cpu.ps = 5;
            assert_eq!(cpu.cycles_until_wake(), None, "line {line} under INTLEVEL 5");
            cpu.ps = 0;
            // a raised line no longer counts; another armed line still does
            let other = TIMER_INTERRUPT[(i + 1) % 3];
            cpu.intenable |= 1 << other; cpu.interrupt = 1 << line;
            assert_eq!(cpu.cycles_until_wake(), Some(cpu.ccompare[(i + 1) % 3].wrapping_sub(100) as u64), "line {line} raised");
            cpu.intenable = 1 << line;
            cpu.interrupt = 1 << line;
            assert_eq!(cpu.cycles_until_wake(), None, "line {line}: already raised");
            cpu.interrupt = 0; cpu.intenable = TIMER_INTERRUPT.iter().fold(!0, |m, &l| m & !(1 << l));
            assert_eq!(cpu.cycles_until_wake(), None, "line {line}: only other interrupts enabled");
        }
    }
}
