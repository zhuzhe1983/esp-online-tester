//! S31 bring-up subset: one HP hart, SRAM/ROM, UART0 and CLIC; not a product runtime.
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
mod rom;
pub use bus::{S31Bus, AccessFault};
use riscv_rv32::{Cpu, Trap};

/// This executes the RV32IMAFC subset, not S31's complete custom ISA. No
/// second hart, PLL/RC clock sources or board devices are installed. The Flash command
/// subset requires explicit NOR identity and array inputs; no product identity is guessed. Functional
/// MMU mappings use explicitly supplied flash/PSRAM buffers. Unlike a P4 alias, its
/// bus uses the S31 addresses and reports the first unsupported access with the guest PC.
pub struct Bringup { pub cpu: Cpu, pub bus: S31Bus }
#[derive(Debug, PartialEq, Eq)]
pub enum Stop { Budget, Waiting, Trap(Trap), Reset(rtc_wdt::ResetRequest) }
pub struct Run { pub steps: u64, pub stop: Stop }

impl Default for Bringup { fn default() -> Self { Self::new() } }
impl Bringup {
    pub fn new() -> Self {
        let mut cpu = Cpu::new_esp32s31();
        cpu.pc = bus::ROM_BASE;
        Self { cpu, bus: S31Bus::new() }
    }

    /// Explicit RTC clock-domain input for bring-up fixtures. CPU cycles are not silently
    /// converted to RTC time before the S31 clock tree has been implemented.
    pub fn advance_rtc_ticks(&mut self, ticks: u64) { self.bus.rtc_wdt.advance(ticks, false, false); }

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

    pub fn run(&mut self, budget: u64) -> Run {
        for steps in 0..budget {
            if let Some(reset) = self.bus.rtc_wdt.reset_request { return Run { steps, stop: Stop::Reset(reset) }; }
            self.bus.bus_monitor.observe(self.cpu.pc, self.cpu.x[2]);
            self.cpu.irq = self.bus.refresh_irq(self.cpu.mintthresh);
            if self.cpu.waiting && self.cpu.irq.is_none() { return Run { steps, stop: Stop::Waiting }; }
            if let Err(trap) = riscv_rv32::step(&mut self.cpu, &mut self.bus) {
                if let Trap::Interrupt(line) = trap { self.bus.interrupt_taken(line); continue; }
                if matches!(trap, Trap::Exception(_) | Trap::Ebreak(_)) { self.bus.bus_monitor.exception(self.cpu.mcause, self.cpu.mtval, self.cpu.mepc); }
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
