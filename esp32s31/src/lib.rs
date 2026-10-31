//! ESP32-S31 on `esp_soc::Machine` / `Soc` (same path as C3/C6/P4).
//! Dual HP harts, SRAM/ROM, UART0/USB console and CLIC; not a product runtime.
pub mod bus;
pub mod soc;
pub mod interrupts;
pub mod trng;
pub mod rtc_wdt;
pub mod cache;
pub mod pads;
pub mod efuse;
pub mod clocks;
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
pub mod periph;
mod rom;
pub use bus::{S31Bus, AccessFault};
pub use soc::{machine, Machine, S31, SocBus, CPU_HZ, RESET_VECTOR, ROM_STACK};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_control;
    use emu_core::Bus;
    use riscv_rv32::{step, Cpu, Trap};

    /// Tiny dual-hart interleaver for hermetic bus tests (not a public runner).
    struct Dual {
        cpu: Cpu,
        cpu1: Cpu,
        bus: S31Bus,
        next: usize,
        last: usize,
    }
    impl Dual {
        fn new() -> Self {
            let mut cpu = Cpu::new_esp32s31();
            cpu.pc = bus::ROM_BASE;
            let mut cpu1 = Cpu::new_esp32s31();
            cpu1.hartid = 1;
            cpu1.pc = bus::ROM_BASE;
            Self { cpu, cpu1, bus: S31Bus::new(), next: 0, last: 0 }
        }
        fn run(&mut self, budget: u64) -> (u64, Option<Trap>) {
            for steps in 0..budget {
                let mut ready = [false; 2];
                for (core, cpu) in [&mut self.cpu, &mut self.cpu1].into_iter().enumerate() {
                    self.bus.set_hart(core);
                    cpu.irq = self.bus.refresh_irq(cpu.mintthresh);
                    ready[core] = self.bus.cores.running(core) && (!cpu.waiting || cpu.irq.is_some());
                }
                let core = if ready[self.next] {
                    self.next
                } else if ready[self.next ^ 1] {
                    self.next ^ 1
                } else {
                    return (steps, None);
                };
                self.next = core ^ 1;
                self.last = core;
                self.bus.set_hart(core);
                let cpu = if core == 0 { &mut self.cpu } else { &mut self.cpu1 };
                emu_core::Bus::note_pc(&mut self.bus, cpu.pc);
                match step(cpu, &mut self.bus) {
                    Ok(()) => {}
                    Err(Trap::Interrupt(line)) => {
                        self.bus.interrupt_taken(line);
                        continue;
                    }
                    Err(trap) => return (steps + 1, Some(trap)),
                }
            }
            (budget, None)
        }
    }

    #[test]
    fn s31_default_ext_ill_flags_fp_memory_ops_like_p4_rev3() {
        // FLW with FS Off: illegal + EXT_ILL.FPU (default for S31 / P4 rev >= 3).
        let mut bus = S31Bus::new();
        bus.load(bus::SRAM_BASE, &0x00002507u32.to_le_bytes(), 4).unwrap();
        let mut cpu = Cpu::new_esp32s31();
        cpu.pc = bus::SRAM_BASE;
        cpu.mtvec = bus::SRAM_BASE + 0x100;
        assert!(cpu.ext_ill_all_fp);
        assert!(matches!(step(&mut cpu, &mut bus), Err(Trap::Exception(2))));
        assert_eq!(cpu.mepc, bus::SRAM_BASE);
        assert_eq!(cpu.pc, bus::SRAM_BASE + 0x100);
        assert!(cpu.fs_off());
        assert_eq!(cpu.ext_ill, 1);
        assert_eq!(cpu.f[10], 0);

        // FS Initial: FLW becomes a real load and faults on unmapped address 0.
        let mut bus = S31Bus::new();
        bus.load(bus::SRAM_BASE, &0x00002507u32.to_le_bytes(), 4).unwrap();
        let mut cpu = Cpu::new_esp32s31();
        cpu.pc = bus::SRAM_BASE;
        cpu.mtvec = bus::SRAM_BASE + 0x100;
        cpu.mstatus |= 0x2000;
        assert!(matches!(step(&mut cpu, &mut bus), Err(Trap::Exception(5))));
        assert_eq!(cpu.ext_ill, 0);

        // Unrelated illegal instruction does not set FPU reason.
        let mut bus = S31Bus::new();
        bus.load(bus::SRAM_BASE, &0xffff_ffffu32.to_le_bytes(), 4).unwrap();
        let mut cpu = Cpu::new_esp32s31();
        cpu.pc = bus::SRAM_BASE;
        cpu.mtvec = bus::SRAM_BASE + 0x100;
        assert!(matches!(step(&mut cpu, &mut bus), Err(Trap::Exception(2))));
        assert_eq!(cpu.ext_ill, 0);
    }

    #[test]
    fn s31_ext_ill_fpu_reason_is_attributed_to_the_executing_hart() {
        let mut m = Dual::new();
        m.bus.load(bus::SRAM_BASE, &0x0000006fu32.to_le_bytes(), 4).unwrap();
        m.cpu.pc = bus::SRAM_BASE;
        m.bus.load(bus::ROM_BASE, &0x00002507u32.to_le_bytes(), 4).unwrap();
        m.bus.cores.write(core_control::CTRL1, 3).unwrap();
        let (steps, trap) = m.run(4);
        assert!(matches!(trap, Some(Trap::Exception(2))), "steps={steps} trap={trap:?}");
        assert_eq!(m.last, 1);
        assert_eq!(m.cpu1.mepc, bus::ROM_BASE);
        assert!(m.cpu1.fs_off());
        assert_eq!(m.cpu1.ext_ill, 1);
        assert_eq!(m.cpu.ext_ill, 0);
    }

    #[test]
    fn guest_executes_uart_store_and_traps_at_unmapped_address() {
        let mut bus = S31Bus::new();
        // lui t0,0x2038a; addi t1,zero,65; sw t1,0(t0); lw t2,0(zero).
        let code = [0x2038_a2b7u32, 0x0410_0313, 0x0062_a023, 0x0000_2383];
        let bytes: Vec<_> = code.iter().flat_map(|w| w.to_le_bytes()).collect();
        bus.load(bus::SRAM_BASE, &bytes, bytes.len()).unwrap();
        let mut cpu = Cpu::new_esp32s31();
        cpu.pc = bus::SRAM_BASE;
        let mut steps = 0u64;
        let trap = loop {
            steps += 1;
            match step(&mut cpu, &mut bus) {
                Ok(()) if steps < 20 => continue,
                Ok(()) => panic!("expected trap"),
                Err(t) => break t,
            }
        };
        assert_eq!(steps, 4);
        assert_eq!(bus.periph.uart0.tx_out, b"A");
        assert_eq!(trap, Trap::Exception(5));
        assert_eq!(cpu.mepc, bus::SRAM_BASE + 12);
        assert_eq!(cpu.mtval, 0);
        let f = bus.first_fault.as_ref().unwrap();
        assert_eq!((f.pc, f.address, f.bytes, f.write), (bus::SRAM_BASE + 12, 0, 4, false));
    }

    #[test]
    fn gated_second_hart_does_not_execute_and_its_fault_is_attributed_after_release() {
        let mut m = Dual::new();
        m.bus.load(bus::SRAM_BASE, &0x0000006fu32.to_le_bytes(), 4).unwrap();
        m.cpu.pc = bus::SRAM_BASE;
        m.run(8);
        assert_eq!(m.cpu1.insn_count, 0);
        m.bus.cores.write(core_control::BOOT1, bus::SRAM_BASE).unwrap();
        m.run(8);
        assert_eq!(m.cpu1.insn_count, 0);
        m.bus.load(bus::ROM_BASE, &0x00002383u32.to_le_bytes(), 4).unwrap();
        m.bus.cores.write(core_control::CTRL1, 3).unwrap();
        let (_, trap) = m.run(4);
        assert_eq!(trap, Some(Trap::Exception(5)));
        let f = m.bus.first_fault.as_ref().unwrap();
        assert_eq!((f.hart, f.pc, f.address), (1, bus::ROM_BASE, 0));
        assert_eq!(m.cpu1.mepc, bus::ROM_BASE);
        assert_eq!(m.cpu1.hartid, 1);
    }

    #[test]
    fn second_hart_vector_fetch_fault_does_not_inherit_first_hart_pc() {
        let mut m = Dual::new();
        m.bus.load(bus::SRAM_BASE, &0x0000006fu32.to_le_bytes(), 4).unwrap();
        m.cpu.pc = bus::SRAM_BASE;
        m.run(1);
        m.bus.cores.write(core_control::CTRL0, 1).unwrap();
        m.bus.cores.write(core_control::CTRL1, 3).unwrap();
        m.cpu1.pc = bus::ROM_BASE + 0x100;
        m.cpu1.mtvec = bus::ROM_BASE | 3;
        m.cpu1.mtvt = 0;
        m.cpu1.mstatus |= 8;
        m.bus.write32(bus::UART0_BASE + 0xc, 2).unwrap();
        m.bus.interrupts.matrix_write(1, 0x24, 0x211);
        m.bus.interrupts.clic[1].write(0x1044, 1 << 29 | 1 << 16 | 1 << 8, 15);
        let (_, trap) = m.run(2);
        assert_eq!(trap, Some(Trap::Exception(1)));
        let f = m.bus.first_fault.as_ref().unwrap();
        assert_eq!((f.hart, f.pc, f.address), (1, bus::ROM_BASE + 0x100, 0x44));
        assert_eq!(m.cpu1.mtval, 0x44);
    }

    #[test]
    fn machine_path_constructs_and_steps() {
        let mut m = machine([0; 6], 1 << 20);
        m.bus.load_bytes(bus::SRAM_BASE, &0x0000_006fu32.to_le_bytes()).unwrap();
        m.cores[0].pc = bus::SRAM_BASE;
        let stop = m.run(8);
        assert!(matches!(stop, esp_soc::Stop::MaxInsns), "{stop:?}");
        assert!(m.cores[0].insn_count >= 8);
    }
}
