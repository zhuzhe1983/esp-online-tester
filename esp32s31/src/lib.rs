//! S31 bring-up subset: two functional HP harts, SRAM/ROM, UART0 and CLIC; not a product runtime.
pub mod bus;
pub mod interrupts;
pub mod trng;
pub mod rtc_wdt;
pub mod cache;
pub mod pads;
pub mod efuse;
pub mod clocks;
pub mod usb_console;
pub mod bus_monitor;
pub mod mspi;
pub mod sha;
pub mod key_manager;
pub mod analog;
pub mod power;
mod brownout;
mod pmu_policy;
mod mpll;
mod mspi_pads;
mod psram_controller;
pub mod pms;
pub mod apm;
pub mod reset;
pub mod core_control;
pub mod mwdt;
pub mod rtc_timer;
pub mod rtc_calibration;
pub mod modem_lpcon;
pub mod startup_clocks;
pub mod systimer;
mod rom;
pub use bus::{S31Bus, AccessFault};
use riscv_rv32::{Cpu, Trap};

/// Diagnostic observation after an instruction at the address retires successfully.
#[derive(Debug, Default)]
pub struct PcWatch { pub address: u32, pub retired: u64, pub first_cycle: Option<u64> }

/// This executes the RV32IMAFC subset, not S31's complete custom ISA. No
/// board devices are installed. Two harts interleave one interpreter instruction at a time;
/// each round advances one nominal CPU quantum, not two parallel hardware cycles. XTAL/CPLL domain rates are functional;
/// RC_FAST/RC_SLOW are nominal selector rates, not oscillator calibration or CPU timing. The Flash command
/// subset requires explicit NOR identity and array inputs; no product identity is guessed. Functional
/// MMU mappings use explicitly supplied flash/PSRAM buffers. Unlike a P4 alias, its
/// bus uses the S31 addresses and reports the first unsupported access with the guest PC.
pub struct Bringup { upstream_fpu_memory_reason: bool, pub compatibility_fpu_memory_traps: [u64; 2], pub cpu: Cpu, pub cpu1: Cpu, pub bus: S31Bus, rtc_timer_from_cpu: bool, rtc_calibration_from_cpu: bool, systimer_from_cpu: bool, pub pc_watch: Option<PcWatch>, pub pc_watch1: Option<PcWatch>, next_core: usize, pub last_core: usize, pub quanta: u64 }
#[derive(Debug, PartialEq, Eq)]
pub enum Stop { Budget, Waiting, Trap(Trap), Reset(rtc_wdt::ResetRequest), PowerReset(power::ResetCause), CpuReset(reset::CpuReset), MwdtReset { group: usize, action: mwdt::Reset } }
pub struct Run { pub steps: u64, pub stop: Stop }

impl Default for Bringup { fn default() -> Self { Self::new() } }
impl Bringup {
    pub fn new() -> Self {
        let mut cpu = Cpu::new_esp32s31();
        cpu.pc = bus::ROM_BASE;
        let mut cpu1 = Cpu::new_esp32s31(); cpu1.hartid = 1; cpu1.pc = bus::ROM_BASE;
        Self { upstream_fpu_memory_reason: false, compatibility_fpu_memory_traps: [0; 2], cpu, cpu1, bus: S31Bus::new(), rtc_timer_from_cpu: false, rtc_calibration_from_cpu: false, systimer_from_cpu: false, pc_watch: None, pc_watch1: None, next_core: 0, last_core: 0, quanta: 0 }
    }

    /// Explicit RTC clock-domain input for bring-up fixtures. CPU cycles are not silently
    /// converted to RTC time before the S31 clock tree has been implemented.
    pub fn advance_rtc_ticks(&mut self, ticks: u64) { self.bus.rtc_timer.advance(ticks); self.bus.rtc_wdt.advance_with_cpu_gates(ticks, false, false, self.bus.reset.watchdog_targets()); }
    /// Opt-in RTC_TIMER-only timing approximation. Other watchdog domains still
    /// require explicit ticks; this does not claim a complete clock scheduler.
    pub fn set_rtc_timer_from_cpu(&mut self, enabled: bool) { self.rtc_timer_from_cpu = enabled; }
    /// Separate opt-in: calibration counting is not silently enabled by RTC_TIMER.
    pub fn set_rtc_calibration_from_cpu(&mut self, enabled: bool) { self.rtc_calibration_from_cpu = enabled; }
    pub fn set_systimer_from_cpu(&mut self, enabled: bool) { self.systimer_from_cpu = enabled; }
    pub fn advance_systimer_ticks(&mut self, ticks: u64) { self.bus.systimer.advance_ticks(ticks, self.bus.cores.stalled()); }
    pub fn advance_calibration_xtal_ticks(&mut self, ticks: u64) {
        let hz = self.bus.clocks.calibration_hz(self.bus.rtc_calibration.source());
        self.bus.rtc_calibration.advance_xtal(ticks, hz, self.bus.mwdt[0].clock_read());
    }
    pub fn advance_mwdt_xtal_ticks(&mut self, group: usize, ticks: u64) -> Result<(), emu_core::Fault> {
        self.bus.mwdt.get_mut(group).ok_or(emu_core::Fault::Unmapped)?.advance_xtal(ticks)
    }

    /// Load an explicitly SRAM/ROM-linked ELF. This does not accept a flash .bin or bypass its
    /// bootloader. All segment bounds are checked before any bytes are changed.
    pub fn load_elf(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.get(18..20) != Some(&[0xf3, 0][..]) { return Err("S31 bring-up needs a RISC-V ELF32".into()); }
        let elf = esp_soc::elf::parse(bytes)?;
        if elf.segments.is_empty() { return Err("ELF has no loadable segments".into()); }
        for seg in &elf.segments { self.bus.check_load(seg.vaddr, seg.memsz as usize)?; }
        self.bus.check_load(elf.entry, 2)?;
        for seg in &elf.segments { self.bus.load(seg.vaddr, &seg.data, seg.memsz as usize)?; }
        self.cpu.pc = elf.entry;
        Ok(())
    }

    /// Load the real mask ROM and reconstruct its omitted ROM-side initializer payloads
    /// from matching ELF RAM sections. The guest reset/unpack loop still performs every
    /// copy itself. No ROM opcode, jump target or guest data value is invented.
    pub fn load_rom_elf(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.get(18..20) != Some(&[0xf3, 0][..]) { return Err("S31 ROM probe needs a RISC-V ELF32".into()); }
        let elf = esp_soc::elf::parse(bytes)?;
        if elf.entry != bus::ROM_BASE { return Err("S31 ROM reset entry must be 0x2f800000".into()); }
        let image = rom::image(&elf)?;
        self.bus.load(bus::ROM_BASE, &image, image.len())?;
        self.cpu.pc = elf.entry;
        self.cpu.x[2] = 0x2f07_cfb0; // v6.1 S31 soc.h: SOC_ROM_STACK_START.
        Ok(())
    }

    /// Development-only compatibility with observed esp-emulator0.51.0 S31
    /// EXT_ILL for FP memory instructions. Public IDF6.1 soc_caps instead names
    /// the load/store reason-bit bug; preserve strict default and record opt-in.
    /// This changes only the reason CSR on the real FS-Off exception,not FS,
    /// PC,the trapping instruction or the guest's own handler.
    pub fn set_upstream_fpu_memory_reason(&mut self, enabled: bool) { self.upstream_fpu_memory_reason = enabled; }

    pub fn run(&mut self, budget: u64) -> Run {
        for steps in 0..budget {
            if let Some(reset) = self.bus.reset.request { return Run { steps, stop: Stop::CpuReset(reset) }; }
            for (group, wdt) in self.bus.mwdt.iter().enumerate() { if let Some(action) = wdt.reset { return Run { steps, stop: Stop::MwdtReset { group, action } }; } }
            if let Some(reset) = self.bus.power.reset_request { return Run { steps, stop: Stop::PowerReset(reset) }; }
            if let Some(reset) = self.bus.rtc_wdt.reset_request { return Run { steps, stop: Stop::Reset(reset) }; }
            for (core, reset) in self.bus.cores.take_resets().into_iter().enumerate() {
                if reset {
                    let cpu = if core == 0 { &mut self.cpu } else { &mut self.cpu1 };
                    cpu.reset(); cpu.pc = bus::ROM_BASE; self.bus.reset_hart(core);
                }
            }
            let mut ready = [false; 2];
            for (core, cpu) in [&mut self.cpu, &mut self.cpu1].into_iter().enumerate() {
                self.bus.set_hart(core); cpu.irq = self.bus.refresh_irq(cpu.mintthresh);
                ready[core] = self.bus.cores.running(core) && (!cpu.waiting || cpu.irq.is_some());
            }
            let core = if ready[self.next_core] { self.next_core } else if ready[self.next_core ^ 1] { self.next_core ^ 1 } else { return Run { steps, stop: Stop::Waiting }; };
            // One round is hart0 then hart1 when both can run. Skipping a gated/WFI
            // hart does not consume an instruction or double the optional RTC time.
            // This is a deterministic functional interleaving, not bus arbitration or
            // measured multi-core timing. Preserve round position across run() budgets.
            let tick = core == 0 || self.next_core == 0;
            self.next_core = core ^ 1; self.last_core = core; self.bus.set_hart(core);
            if tick {
                self.quanta = self.quanta.wrapping_add(1);
                self.bus.clocks.mpll.advance(1, self.bus.analog.master_enabled() && self.bus.clocks.modem_on());
                self.bus.clocks.advance_calibration(1);
            }
            let cpu = if core == 0 { &mut self.cpu } else { &mut self.cpu1 };
            self.bus.bus_monitor.observe_core(core, cpu.pc, cpu.x[2]);
            let pc = cpu.pc; let cycles = cpu.cycle_count; let hz = self.bus.clocks.rates().cpu_hz;
            // A hardware-vectored interrupt can read its table before the core
            // calls note_pc for an instruction. Attribute that fault to this hart/PC.
            emu_core::Bus::note_pc(&mut self.bus, pc);
            let result = riscv_rv32::step(cpu, &mut self.bus);
            if result.is_ok() {
                let watch = if core == 0 { &mut self.pc_watch } else { &mut self.pc_watch1 };
                if let Some(watch) = watch.as_mut().filter(|w| w.address == pc) { watch.retired = watch.retired.wrapping_add(1); watch.first_cycle.get_or_insert(cycles); }
            }
            if self.rtc_timer_from_cpu && tick { self.bus.rtc_timer.advance_cpu_cycles(cpu.cycle_count.wrapping_sub(cycles), hz, self.bus.clocks.rtc_slow_hz()); }
            if self.rtc_calibration_from_cpu && tick {
                let source = self.bus.clocks.calibration_hz(self.bus.rtc_calibration.source());
                self.bus.rtc_calibration.advance_cpu(cpu.cycle_count.wrapping_sub(cycles), hz, source, self.bus.mwdt[0].clock_read());
            }
            if self.systimer_from_cpu && tick { self.bus.systimer.advance_cpu(cpu.cycle_count.wrapping_sub(cycles), hz, self.bus.clocks.rc_fast_hz(), self.bus.cores.stalled()); }
            if let Err(trap) = result {
                if let Trap::Interrupt(line) = trap { self.bus.interrupt_taken(line); continue; }
                if self.upstream_fpu_memory_reason && trap == Trap::Exception(2) && cpu.fs_off() && cpu.has_ext_ill {
                    let insn = riscv_rv32::decode_for(cpu.misa, cpu.mepc, cpu.mtval.to_le_bytes());
                    if matches!(insn.op, riscv_rv32::Op::Flw | riscv_rv32::Op::Fsw) {
                        cpu.ext_ill |= riscv_rv32::state::ext_ill::FPU;
                        self.compatibility_fpu_memory_traps[core] = self.compatibility_fpu_memory_traps[core].wrapping_add(1);
                    }
                }
                if matches!(trap, Trap::Exception(_) | Trap::Ebreak(_)) { self.bus.bus_monitor.exception_core(core, cpu.mcause, cpu.mtval, cpu.mepc); }
                return Run { steps: steps + 1, stop: Stop::Trap(trap) };
            }
        }
        Run { steps: budget, stop: Stop::Budget }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_fp_memory_reason_is_explicit_and_never_repairs_state_or_other_faults() {
        for (word, fs, enabled, exception, reason, count) in [
            (0x00002507u32, 0, false, 2, 0, 0), // flw fa0,0(zero), strict default
            (0x00002507, 0, true, 2, 1, 1),
            (0x00002507, 0x2000, true, 5, 0, 0), // Actual unmapped data read, not FS-Off
            (0xffffffff, 0, true, 2, 0, 0), // Unrelated illegal instruction
            (0x00000053, 0, true, 2, 1, 0), // FADD's existing reason requires no compatibility
        ] {
            let mut m = Bringup::new(); m.bus.load(bus::SRAM_BASE, &word.to_le_bytes(), 4).unwrap();
            m.cpu.pc = bus::SRAM_BASE; m.cpu.mtvec = bus::SRAM_BASE + 0x100; m.cpu.mstatus |= fs;
            if enabled { m.set_upstream_fpu_memory_reason(true); }
            assert_eq!(m.run(1).stop, Stop::Trap(Trap::Exception(exception)));
            assert_eq!(m.cpu.mepc, bus::SRAM_BASE); assert_eq!(m.cpu.pc, bus::SRAM_BASE + 0x100);
            assert_eq!(m.cpu.mstatus & 0x6000, fs); assert_eq!(m.cpu.ext_ill, reason);
            assert_eq!(m.compatibility_fpu_memory_traps, [count, 0]); assert_eq!(m.cpu.f[10], 0);
        }
        let mut m = Bringup::new(); m.set_upstream_fpu_memory_reason(true); m.set_upstream_fpu_memory_reason(false);
        m.bus.load(bus::SRAM_BASE, &0x00002507u32.to_le_bytes(), 4).unwrap(); m.cpu.pc = bus::SRAM_BASE;
        assert_eq!(m.run(1).stop, Stop::Trap(Trap::Exception(2))); assert_eq!(m.cpu.ext_ill, 0);
        assert_eq!(m.compatibility_fpu_memory_traps, [0, 0]);
    }
    #[test]
    fn upstream_fp_memory_reason_is_attributed_to_the_executing_hart() {
        let mut m = Bringup::new(); m.set_upstream_fpu_memory_reason(true);
        m.bus.load(bus::SRAM_BASE, &0x0000006fu32.to_le_bytes(), 4).unwrap(); m.cpu.pc = bus::SRAM_BASE;
        m.bus.load(bus::ROM_BASE, &0x00002507u32.to_le_bytes(), 4).unwrap(); m.bus.cores.write(core_control::CTRL1, 3).unwrap();
        assert_eq!(m.run(4).stop, Stop::Trap(Trap::Exception(2))); assert_eq!(m.last_core, 1);
        assert_eq!(m.cpu1.mepc, bus::ROM_BASE); assert!(m.cpu1.fs_off()); assert_eq!(m.cpu1.ext_ill, 1);
        assert_eq!(m.cpu.ext_ill, 0); assert_eq!(m.compatibility_fpu_memory_traps, [0, 1]);
    }
    #[test]
    fn guest_executes_uart_store_and_traps_at_unmapped_address() {
        let mut m = Bringup::new();
        // lui t0,0x2038a; addi t1,zero,65; sw t1,0(t0); lw t2,0(zero).
        let code = [0x2038_a2b7u32, 0x0410_0313, 0x0062_a023, 0x0000_2383];
        let bytes: Vec<_> = code.iter().flat_map(|w| w.to_le_bytes()).collect();
        m.bus.load(bus::SRAM_BASE, &bytes, bytes.len()).unwrap(); m.cpu.pc = bus::SRAM_BASE;
        let run = m.run(20);
        assert_eq!(run.steps, 4); assert_eq!(m.bus.uart0.tx_out, b"A");
        assert_eq!(run.stop, Stop::Trap(Trap::Exception(5)));
        assert_eq!(m.cpu.mepc, bus::SRAM_BASE + 12); assert_eq!(m.cpu.mtval, 0);
        let f = m.bus.first_fault.as_ref().unwrap();
        assert_eq!((f.pc, f.address, f.bytes, f.write), (bus::SRAM_BASE + 12, 0, 4, false));
    }
    #[test]
    fn dual_hart_rounds_survive_budget_boundaries_and_only_advance_one_quantum() {
        fn machine() -> Bringup {
            let mut m = Bringup::new(); m.bus.load(bus::ROM_BASE, &0x0000006fu32.to_le_bytes(), 4).unwrap();
            m.bus.cores.write(core_control::CTRL1, 3).unwrap(); m
        }
        let mut a = machine(); let mut b = machine();
        assert_eq!(a.run(10).stop, Stop::Budget);
        for _ in 0..10 { assert_eq!(b.run(1).stop, Stop::Budget); }
        assert_eq!([a.cpu.insn_count, a.cpu1.insn_count], [5, 5]);
        assert_eq!([b.cpu.insn_count, b.cpu1.insn_count], [5, 5]); assert_eq!((a.quanta, b.quanta), (5, 5));
        a.bus.cores.write(core_control::STALL, 0x00860000).unwrap(); a.run(6);
        assert_eq!([a.cpu.insn_count, a.cpu1.insn_count], [11, 5]); assert_eq!(a.quanta, 11);
        a.bus.cores.write(core_control::STALL, 0x86000000 | 0x00ff0000).unwrap(); a.run(6);
        assert_eq!([a.cpu.insn_count, a.cpu1.insn_count], [11, 11]); assert_eq!(a.quanta, 16); // First resumed hart1 step completes the preceding half-round.
    }
    #[test]
    fn gated_second_hart_does_not_execute_and_its_fault_is_attributed_after_release() {
        let mut m = Bringup::new();
        m.bus.load(bus::SRAM_BASE, &0x0000006fu32.to_le_bytes(), 4).unwrap(); m.cpu.pc = bus::SRAM_BASE;
        m.run(8); assert_eq!(m.cpu1.insn_count, 0);
        m.bus.cores.write(core_control::BOOT1, bus::SRAM_BASE).unwrap(); m.run(8); assert_eq!(m.cpu1.insn_count, 0);
        // Actual ROM reset location is executed, not the BOOT1 value. This tiny
        // synthetic ROM reads unmapped address0 to make fault attribution observable.
        m.bus.load(bus::ROM_BASE, &0x00002383u32.to_le_bytes(), 4).unwrap();
        m.bus.cores.write(core_control::CTRL1, 3).unwrap();
        assert_eq!(m.run(4).stop, Stop::Trap(Trap::Exception(5)));
        let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.hart, f.pc, f.address), (1, bus::ROM_BASE, 0));
        assert_eq!(m.cpu1.mepc, bus::ROM_BASE); assert_eq!(m.cpu1.hartid, 1);
    }

    #[test]
    fn second_hart_vector_fetch_fault_does_not_inherit_first_hart_pc() {
        use emu_core::Bus;
        let mut m = Bringup::new(); m.bus.load(bus::SRAM_BASE, &0x0000006fu32.to_le_bytes(), 4).unwrap();
        m.cpu.pc = bus::SRAM_BASE; m.run(1); m.bus.cores.write(core_control::CTRL0, 1).unwrap();
        m.bus.cores.write(core_control::CTRL1, 3).unwrap(); m.cpu1.pc = bus::ROM_BASE + 0x100;
        m.cpu1.mtvec = bus::ROM_BASE | 3; m.cpu1.mtvt = 0; m.cpu1.mstatus |= 8;
        m.bus.write32(bus::UART0_BASE + 0xc, 2).unwrap();
        m.bus.interrupts.matrix_write(1, 0x24, 0x211);
        m.bus.interrupts.clic[1].write(0x1044, 1 << 29 | 1 << 16 | 1 << 8, 15);
        assert_eq!(m.run(2).stop, Stop::Trap(Trap::Exception(1)));
        let f = m.bus.first_fault.as_ref().unwrap(); assert_eq!((f.hart, f.pc, f.address), (1, bus::ROM_BASE + 0x100, 0x44));
        assert_eq!(m.cpu1.mtval, 0x44);
    }

}
