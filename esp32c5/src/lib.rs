//! ESP32-C5 MP (rev 1.0) independent functional bring-up subset. No C3/C6 SoC alias.
//! SRAM-linked guests and the actual mask ROM execute on RV32IMAC with standard CLIC.
//! Unsupported MMIO faults; this is not yet an ESP-IDF/UIFlow product runtime.
pub mod bus;
pub mod board;
pub mod soc;
pub mod image;
mod rom;
mod mspi;
mod mspi_regs;
mod quad_psram;
mod transport;
mod transport_dma;
mod transport_regs;
mod lp_peri;
mod lp_wdt;
mod lp_timer;
mod timg_wdt;
mod i2c;
mod i2c_regs;
mod cache;
mod sha;
#[cfg(test)] mod factory_tests;
#[cfg(test)] mod boot_watchdog_tests;
#[cfg(test)] mod boot_saradc_tests;
#[cfg(test)] mod lp_timer_bus_tests;
#[cfg(test)] mod boot_pmu_bus_tests;
#[cfg(test)] mod boot_ocode_bus_tests;
#[cfg(test)] mod boot_apm_bus_tests;
#[cfg(test)] mod memory_policy_bus_tests;
#[cfg(test)] mod modem_lp_bus_tests;
#[cfg(test)] mod boot_peripheral_bus_tests;
#[cfg(test)] mod startup_irq_bus_tests;
#[cfg(test)] mod boot_brownout_bus_tests;
#[cfg(test)] mod ordinary_wdt_bus_tests;
#[cfg(test)] mod security_clock_bus_tests;
#[cfg(test)] mod boot_security_bus_tests;
#[cfg(test)] mod usb_monitor_bus_tests;
#[cfg(test)] mod coex_gate_bus_tests;
#[cfg(test)] mod modem_wifi_bus_tests;
#[cfg(test)] mod unaligned_bus_tests;
#[cfg(test)] mod lp_pad_mux_bus_tests;
mod boot_analog;
mod boot_clock;
mod modem_lp_clock;
mod modem_wifi_clock;
mod boot_bias;
mod boot_pmu;
mod boot_apm;
mod boot_peripheral_clock;
mod boot_brownout;
mod boot_security;
mod software_interrupt;
mod boot_saradc;
mod clock_tree;
mod clock_calibration;
mod efuse;
mod usb_detached;
mod systimer;
mod bus_monitor;
mod cpu_monitor;
use riscv_rv32::{Cpu, Trap};
pub use bus::C5Bus;
pub use soc::{machine, rom_machine, load_rom, Machine, C5};

pub(crate) fn core() -> Cpu {
    let mut cpu = Cpu::new_rv32imac(); cpu.clic = true; cpu.clic_std = true; cpu.user_counters = true;
    // Explicit functional subset, not a claim that C5 silicon lacks U mode.
    cpu.machine_only=true;
    cpu.pc = bus::ROM_BASE; cpu
}

pub struct Bringup { pub cpu: Cpu, pub bus: C5Bus }
#[derive(Debug, PartialEq, Eq)]
pub enum Stop { Budget, Waiting, Reset(u32), Trap(Trap) }
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
        self.cpu.pc = elf.entry; self.bus.rom_boot=false; self.bus.deselect_validated_rom_profile(); Ok(())
    }
    /// Load the official MP ROM and reconstruct only its ELF-declared initializers.
    /// The guest reset loop performs the copies. Neither instructions nor return values change.
    pub fn load_rom_elf(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (elf,image)=rom::decode(bytes)?;
        if self.bus.reset_requested{return Err("C5 ROM replacement requires applying the pending reset first".into());}
        self.cpu.c5_machine_context().map_err(|e|format!("C5 cold ROM needs supported Machine context: {e:?}"))?;
        self.bus.select_validated_rom_profile()?;
        self.cpu.enable_c5_memory_protection().map_err(|e|format!("C5 CPU memory policy unavailable: {e:?}"))?;
        self.bus.rom.copy_from_slice(&image); self.cpu.pc = elf.entry; self.bus.rom_boot=true; self.bus.cpu_monitor(); Ok(())
    }
    pub fn run(&mut self, budget: u64) -> Run {
        for steps in 0..budget {
            if self.bus.reset_requested{return Run{steps,stop:Stop::Reset(self.bus.reset_cause&0x1f)};}
            self.cpu.irq = self.bus.refresh_irq(self.cpu.mintthresh);
            if self.cpu.waiting && self.cpu.irq.is_none() { return Run { steps, stop: Stop::Waiting }; }
            emu_core::Bus::note_pc(&mut self.bus, self.cpu.pc);
            let pc=self.cpu.pc;
            if let Some(m)=&self.bus.monitor{m.borrow_mut().begin_core(pc,&self.cpu);}
            let result = riscv_rv32::step(&mut self.cpu, &mut self.bus);
            if let Some(m)=&self.bus.monitor{let mut m=m.borrow_mut();if let Err(trap)=&result{m.trap(pc,trap);}else{m.end();}}
            self.bus.advance(1);
            if self.bus.reset_requested{return Run{steps:steps+1,stop:Stop::Reset(self.bus.reset_cause&0x1f)};}
            if let Err(trap) = result {
                if let Trap::Interrupt(line) = trap { self.bus.interrupt_taken(line); continue; }
                return Run { steps: steps + 1, stop: Stop::Trap(trap) };
            }
        }
        Run { steps: budget, stop: Stop::Budget }
    }
}

mod wifi_compat;
pub use wifi_compat::enable_wifi_driver_compat;
