//! ESP32-C5 MP (rev 1.0) independent functional bring-up subset. No C3/C6 SoC alias.
//! SRAM-linked guests and the actual mask ROM execute on RV32IMAC with standard CLIC.
//! Unsupported MMIO faults; this is not yet an ESP-IDF/UIFlow product runtime.
pub mod bus;
pub mod board;
pub mod soc;
pub mod image;
mod rom;
use riscv_rv32::{Cpu, Trap};
pub use bus::C5Bus;
pub use soc::{machine, Machine, C5};

pub(crate) fn core() -> Cpu {
    let mut cpu = Cpu::new_rv32imac(); cpu.clic = true; cpu.clic_std = true; cpu.user_counters = true;
    cpu.pc = bus::ROM_BASE; cpu
}

pub struct Bringup { pub cpu: Cpu, pub bus: C5Bus }
#[derive(Debug, PartialEq, Eq)]
pub enum Stop { Budget, Waiting, Trap(Trap) }
pub struct Run { pub steps: u64, pub stop: Stop }
impl Default for Bringup { fn default() -> Self { Self::new() } }
impl Bringup {
    pub fn new() -> Self {
        // IDF v5.5.4 esp32c5 soc_caps.h and riscv/interrupt_clic.h: RV32IMAC,
        // standard mintstatus/mintthresh, single HP hart. No F or custom ISA claimed.
        let cpu = core();
        Self { cpu, bus: C5Bus::new() }
    }
    /// Explicit SRAM/LP-SRAM ELF loading, not synthetic success of a flash bootloader.
    /// Validate every segment and entry before changing any memory.
    pub fn load_elf(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.get(18..20) != Some(&[0xf3, 0][..]) { return Err("C5 needs a RISC-V ELF32".into()); }
        let elf = esp_soc::elf::parse(bytes)?;
        if elf.segments.is_empty() { return Err("ELF has no loadable segments".into()); }
        for s in &elf.segments { if s.data.len() > s.memsz as usize { return Err("ELF file size exceeds memory size".into()); } self.bus.check_ram(s.vaddr, s.memsz as usize)?; }
        self.bus.check_ram(elf.entry, 2)?;
        if !elf.segments.iter().any(|s| elf.entry >= s.vaddr && elf.entry.checked_add(2).zip(s.vaddr.checked_add(s.memsz)).is_some_and(|(entry_end, end)| entry_end <= end)) { return Err("ELF entry is outside its loaded segments".into()); }
        for s in &elf.segments { self.bus.load_ram(s.vaddr, &s.data, s.memsz as usize)?; }
        self.cpu.pc = elf.entry; Ok(())
    }
    /// Load the official MP ROM and reconstruct only its ELF-declared initializers.
    /// The guest reset loop performs the copies. Neither instructions nor return values change.
    pub fn load_rom_elf(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.get(18..20) != Some(&[0xf3, 0][..]) { return Err("C5 ROM needs a RISC-V ELF32".into()); }
        let elf = esp_soc::elf::parse(bytes)?;
        if elf.entry != bus::ROM_BASE { return Err("C5 ROM entry must be 0x40000000".into()); }
        // The beta rev0 has a different CLIC contract. Accept only the audited MP
        // ROM layout; receipts additionally pin the complete public ELF SHA-256.
        if elf.by_name.get("_data_table_start") != Some(&0x4003_b154) || elf.by_name.get("_data_table_end") != Some(&0x4003_b340) { return Err("C5 requires the audited rev100 ROM layout; beta rev0 is unsupported".into()); }
        let image = rom::image(&elf)?;
        self.bus.rom.copy_from_slice(&image); self.cpu.pc = elf.entry; Ok(())
    }
    pub fn run(&mut self, budget: u64) -> Run {
        for steps in 0..budget {
            self.cpu.irq = self.bus.refresh_irq(self.cpu.mintthresh);
            if self.cpu.waiting && self.cpu.irq.is_none() { return Run { steps, stop: Stop::Waiting }; }
            emu_core::Bus::note_pc(&mut self.bus, self.cpu.pc);
            let result = riscv_rv32::step(&mut self.cpu, &mut self.bus);
            self.bus.advance(1);
            if let Err(trap) = result {
                if let Trap::Interrupt(line) = trap { self.bus.interrupt_taken(line); continue; }
                return Run { steps: steps + 1, stop: Stop::Trap(trap) };
            }
        }
        Run { steps: budget, stop: Stop::Budget }
    }
}
