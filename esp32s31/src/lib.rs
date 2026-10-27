//! S31 bring-up subset: one HP hart, SRAM/ROM, UART0 and CLIC; not a product runtime.
pub mod bus;
pub mod interrupts;
pub use bus::{S31Bus, AccessFault};
use riscv_rv32::{Cpu, Trap};

/// This executes the RV32IMAFC subset, not S31's complete custom ISA. No
/// second hart, flash MMU, PSRAM, clock tree or board devices are installed. Unlike a P4 alias, its
/// bus uses the S31 addresses and reports the first unsupported access with the guest PC.
pub struct Bringup { pub cpu: Cpu, pub bus: S31Bus }
#[derive(Debug, PartialEq, Eq)]
pub enum Stop { Budget, Waiting, Trap(Trap) }
pub struct Run { pub steps: u64, pub stop: Stop }

impl Default for Bringup { fn default() -> Self { Self::new() } }
impl Bringup {
    pub fn new() -> Self {
        let mut cpu = Cpu::new_esp32s31();
        cpu.pc = bus::ROM_BASE;
        Self { cpu, bus: S31Bus::new() }
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

    /// Inspect the beginning of the real mask ROM, without patching instructions. Espressif's
    /// 20260528 S31 ELF has a non-executable .eh_frame PT_LOAD at zero; it is debugger metadata,
    /// not mapped chip memory. ROM-side copies of RAM initializer tables are not reconstructed
    /// yet, so this is only an early-ROM probe, not a complete reset/boot implementation.
    pub fn load_rom_elf(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.get(18..20) != Some(&[0xf3, 0][..]) { return Err("S31 ROM probe needs a RISC-V ELF32".into()); }
        let elf = esp_soc::elf::parse(bytes)?;
        if elf.entry != bus::ROM_BASE { return Err("S31 ROM reset entry must be 0x2f800000".into()); }
        for seg in &elf.segments {
            if seg.vaddr == 0 && seg.flags == 4 { continue; }
            self.bus.check_load(seg.vaddr, seg.memsz as usize)?;
        }
        for section in &elf.sections {
            if !section.is_bss && !section.data.is_empty() { self.bus.check_load(section.addr, section.data.len())?; }
        }
        for seg in &elf.segments {
            if seg.vaddr == 0 && seg.flags == 4 { continue; }
            self.bus.load(seg.vaddr, &seg.data, seg.memsz as usize)?;
        }
        for section in &elf.sections {
            if !section.is_bss && !section.data.is_empty() { self.bus.load(section.addr, &section.data, section.data.len())?; }
        }
        self.cpu.pc = elf.entry;
        self.cpu.x[2] = 0x2f07_cfb0; // v6.1 S31 soc.h: SOC_ROM_STACK_START.
        Ok(())
    }

    pub fn run(&mut self, budget: u64) -> Run {
        for steps in 0..budget {
            self.cpu.irq = self.bus.refresh_irq(self.cpu.mintthresh);
            if self.cpu.waiting && self.cpu.irq.is_none() { return Run { steps, stop: Stop::Waiting }; }
            if let Err(trap) = riscv_rv32::step(&mut self.cpu, &mut self.bus) {
                if let Trap::Interrupt(line) = trap { self.bus.interrupt_taken(line); continue; }
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
}
