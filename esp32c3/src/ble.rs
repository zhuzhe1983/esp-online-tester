//! C3 RISC-V calling glue for the virtual VHCI controller.
use crate::bus::{SocBus, DRAM_HIGH, DRAM_LOW};
use emu_core::Bus;
use esp_soc::ble::vhci::{Abi, Ble, VhciBus};

pub fn intercept(cpu: &mut riscv_rv32::Cpu, bus: &mut SocBus) -> bool {
    esp_soc::ble::vhci::intercept(cpu, bus, |cpu, pc| {
        cpu.pc = pc;
        cpu.insn_count += 1;
        cpu.retired_count += 1;
        cpu.cycle_count += 1;
    })
}

impl VhciBus for SocBus {
    fn abi() -> Abi { Abi { code: CODE, ram: DRAM_LOW..DRAM_HIGH, hz: crate::periph::CPU_HZ, config_mode: None } }
    fn ble(&mut self) -> &mut Ble { &mut self.ble }
    fn valid_callback(&mut self, pc: u32) -> bool {
        pc & 1 == 0
            && ((crate::bus::IRAM_LOW..crate::bus::IRAM_HIGH).contains(&pc)
                || (crate::bus::IBUS_LOW..crate::bus::IBUS_HIGH).contains(&pc))
            && self.fetch(pc).is_ok()
    }
    fn flash_offset(&mut self, pc: u32) -> Option<usize> { self.flash_off(pc) }
    fn ble_debug(&self) -> bool { self.debug.has("ble") }
}

// RV32IMC trampoline: create task, poll VHCI, call the guest callback, delay one tick.
const CODE: &[u8] = &[
    0x41, 0x11, 0x06, 0xc6, 0x01, 0x00, 0x01, 0x00, 0x97, 0x02, 0x00, 0x00, 0x03, 0xa3, 0xc2, 0xfc, 0x03, 0xa5, 0x02,
    0xfd, 0x83, 0xa5, 0x42, 0xfd, 0x03, 0xa6, 0x82, 0xfe, 0x81, 0x46, 0x5d, 0x47, 0x81, 0x47, 0x01, 0x48, 0x02, 0x93,
    0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x97, 0x02, 0x00, 0x00, 0x03, 0xa3, 0x82, 0xfb, 0x02,
    0x93, 0xb2, 0x40, 0x41, 0x01, 0x82, 0x80, 0x97, 0x02, 0x00, 0x00, 0x03, 0xa3, 0x42, 0xfa, 0x02, 0x93, 0x19, 0xc9,
    0x2a, 0x83, 0x01, 0x00, 0x97, 0x02, 0x00, 0x00, 0x03, 0xa5, 0xc2, 0xf9, 0x0c, 0x41, 0x41, 0x05, 0x02, 0x93, 0xcd,
    0xb7, 0x05, 0x45, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x97, 0x02, 0x00, 0x00, 0x03, 0xa3, 0x82, 0xf7, 0x02, 0x93,
    0xf9, 0xb7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x82, 0x80, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x82, 0x80,
];
#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::Core;
    use esp_soc::ble::vhci::{install, INIT, TASK};
    use riscv_rv32::Cpu;
    use std::collections::HashMap;
    const INVALID_ARG: u32 = 0x102;

    const BASE: u32 = 0x4038_0000;
    const RETURN: u32 = 0x4038_1000;
    const SEND_CB: u32 = BASE + 0x700;
    const RECV_CB: u32 = BASE + 0x710;

    fn fixture() -> (SocBus, HashMap<String, u32>) {
        let mut bus = SocBus::new(0x400000, [0; 6]);
        let symbols: HashMap<String, u32> = [
            ("esp_bt_controller_init", BASE),
            ("xTaskCreatePinnedToCore", BASE + 0x400),
            ("vTaskDelay", BASE + 0x500),
            ("esp_bt_controller_get_status", BASE + 0x600),
            ("esp_bt_controller_enable", BASE + 0x610),
            ("esp_vhci_host_check_send_available", BASE + 0x620),
            ("esp_vhci_host_send_packet", BASE + 0x630),
            ("esp_vhci_host_register_callback", BASE + 0x640),
            ("_bt_controller_bss_start", 0x3fc9_0000),
            ("_bt_controller_bss_end", 0x3fc9_1000),
        ]
        .into_iter()
        .map(|(n, a)| (n.into(), a))
        .collect();
        for &pc in symbols.values().filter(|&&pc| pc >= BASE) { bus.load_bytes(pc, &[0x82, 0x80]).unwrap(); }
        for pc in [SEND_CB, RECV_CB] { bus.load_bytes(pc, &[0x82, 0x80]).unwrap(); }
        bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap();
        (bus, symbols)
    }

    fn caller(pc: u32, arg: u32) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.pc = pc;
        cpu.x[2] = 0x3fcd_0000;
        cpu.x[1] = RETURN;
        cpu.x[10] = arg;
        cpu
    }

    #[test]
    fn trampoline_creates_guest_task_with_riscv_abi_and_returns_esp_ok() {
        let (mut bus, _) = fixture();
        let mut cpu = caller(BASE, DRAM_LOW);
        assert!(intercept(&mut cpu, &mut bus));
        let mut created = false;
        for _ in 0..100 {
            if cpu.pc == RETURN { break; }
            if cpu.pc == bus.ble.create {
                let args: Vec<u32> = (0..6).map(|n| cpu.arg(&mut bus, n)).collect();
                assert_eq!(args, [BASE + TASK, bus.ble.buffer + 4, 4096, 0, 23, 0]);
                assert_eq!(cpu.arg(&mut bus, 6), 0);
                cpu.return_from_stub(&mut bus, 1);
                created = true;
            } else if !intercept(&mut cpu, &mut bus) { assert!(cpu.step(&mut bus).trap().is_none()); }
        }
        assert!(created);
        assert_eq!(cpu.pc, RETURN);
        assert_eq!(cpu.x[10], 0);
        assert_eq!(bus.ble.status, 1);
        assert!(bus.ble.task_created);
        let hooks = bus.ble.hooks.clone();
        bus.ble.reset();
        assert_eq!(bus.ble.status, 0);
        assert!(!bus.ble.task_created);
        assert_eq!(bus.ble.hooks, hooks);
    }

    /// A BLE-only chip's init takes BLE alone (IDF v5.3.6 components/bt/controller/esp32c3/bt.c:1823,
    /// which the S3 builds too), so its enable takes 1 and nothing else (bt.c:2073).
    #[test]
    fn enable_takes_ble_only() {
        let call = |bus: &mut SocBus, pc: u32, arg: u32| { let mut cpu = caller(pc, arg); assert!(intercept(&mut cpu, bus)); cpu.x[10] };
        let (mut bus, symbols) = fixture();
        bus.ble.task_created = true;
        assert_eq!(call(&mut bus, BASE, DRAM_LOW), 0);
        assert_eq!(bus.ble.mode, 1);
        for mode in [0, 2, 3] { assert_eq!(call(&mut bus, symbols["esp_bt_controller_enable"], mode), INVALID_ARG, "enable {mode}"); }
        assert_eq!(bus.ble.status, 1);
        assert_eq!(call(&mut bus, symbols["esp_bt_controller_enable"], 1), 0);
        assert_eq!(bus.ble.status, 2);
    }

    #[test]
    fn guest_task_calls_receive_and_send_ready_then_delays() {
        let (mut bus, _) = fixture();
        install(&mut bus).unwrap();
        bus.ble.status = 2;
        bus.ble.callbacks = [SEND_CB, RECV_CB];
        bus.ble.send_ready = true;
        bus.ble.session.send(&[1, 3, 12, 0]);
        let mut cpu = caller(BASE + TASK, 0);
        let (mut received, mut ready, mut delayed) = (false, false, false);
        for _ in 0..100 {
            if cpu.pc == RECV_CB {
                let (ptr, len) = (cpu.arg(&mut bus, 0), cpu.arg(&mut bus, 1));
                let bytes: Vec<u8> = (0..len).map(|i| bus.read8_unpriced(ptr + i).unwrap()).collect();
                assert_eq!(bytes, [4, 14, 4, 1, 3, 12, 0]);
                cpu.return_from_stub(&mut bus, 0);
                received = true;
            } else if cpu.pc == SEND_CB {
                assert!(delayed, "host must run after receiving a packet");
                cpu.return_from_stub(&mut bus, 0);
                ready = true;
            } else if cpu.pc == bus.ble.delay {
                assert_eq!(cpu.arg(&mut bus, 0), 1);
                delayed = true;
                if ready { break; }
                cpu.return_from_stub(&mut bus, 0);
            } else if !intercept(&mut cpu, &mut bus) { assert!(cpu.step(&mut bus).trap().is_none()); }
        }
        assert!(received && ready && delayed);
    }

    #[test]
    fn halfword_aligned_controller_entry_uses_aligned_literals() {
        let (mut bus, mut symbols) = fixture();
        symbols.insert("esp_bt_controller_init".into(), BASE + 2);
        bus.load_bytes(BASE + 2, &[0x82, 0x80]).unwrap();
        bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap();
        let mut cpu = caller(BASE + 2, DRAM_LOW);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(bus.ble.init, BASE + 4);
        assert_eq!(cpu.pc, BASE + 4 + INIT);
        assert_eq!(bus.read32_unpriced(BASE + 8).unwrap(), bus.ble.create);
    }

    #[test]
    fn invalid_guest_buffers_and_missing_symbols_are_rejected() {
        let (mut bus, mut symbols) = fixture();
        let mut cpu = caller(symbols["esp_vhci_host_register_callback"], 0xffff_fffc);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[10], INVALID_ARG);
        let table = 0x3fc9_2000;
        bus.load_bytes(table, &SEND_CB.to_le_bytes()).unwrap();
        bus.load_bytes(table + 4, &RECV_CB.to_le_bytes()).unwrap();
        let mut cpu = caller(symbols["esp_vhci_host_register_callback"], table);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[10], 0, "a static callback table can live in DROM");
        assert_eq!(bus.ble.callbacks, [SEND_CB, RECV_CB]);
        symbols.remove("vTaskDelay");
        assert!(bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap_err().contains("vTaskDelay"));
    }

    #[test]
    fn machine_dispatches_chip_hooks_as_function_boundaries() {
        struct SingleStep;
        impl esp_soc::Observer<crate::C3> for SingleStep {
            fn name(&self) -> &'static str { "single-step" }
            fn wants(&self) -> esp_soc::Wants { esp_soc::Wants::INSN }
        }
        for single_step in [false, true] {
            let (bus, symbols) = fixture();
            let mut m = esp_soc::Machine::<crate::C3>::new([0; 6], bus);
            m.quantum = 1;
            if single_step { m.add_observer(Box::new(SingleStep)); }
            m.bus.ble.status = 2;
            m.cores[0] = caller(symbols["esp_bt_controller_get_status"], 0);
            assert!(matches!(m.run(1), esp_soc::Stop::MaxInsns));
            assert_eq!(m.cores[0].pc, RETURN);
            assert_eq!(m.cores[0].x[10], 2);
        }
    }

    #[test]
    fn reboot_restores_controller_flash_even_after_mmu_mapping_changes() {
        let (mut bus, symbols) = fixture();
        let init = crate::bus::IBUS_LOW;
        let symbols = symbols
            .into_iter()
            .map(|(name, address)| (name, if address >= BASE { address - BASE + init } else { address }))
            .collect();
        bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap();
        let slot = ((init & 0x1ff_ffff) >> 16) as usize;
        bus.mmu[slot] = 0;
        let mut original: Vec<u8> = (0..INIT as usize + CODE.len()).map(|i| i as u8).collect();
        original[..2].copy_from_slice(&[0x82, 0x80]);
        bus.load_bytes(init, &original).unwrap();
        install(&mut bus).unwrap();
        assert_ne!(&bus.flash[..original.len()], &original);
        bus.ble.status = 2;
        bus.ble.callbacks = [SEND_CB, RECV_CB];
        bus.ble.task_created = true;
        bus.ble.session.send(&[1, 0x0a, 0x20, 1, 1]);
        assert!(bus.ble.session.is_advertising());
        bus.mmu[slot] = 1;
        esp_soc::SocBus::reboot(&mut bus, [0; 6]);
        assert_eq!(&bus.flash[..original.len()], &original);
        assert_eq!(bus.ble.status, 0);
        assert_eq!(bus.ble.callbacks, [0; 2]);
        assert!(!bus.ble.task_created);
        assert!(!bus.ble.session.is_advertising());
        assert!(bus.ble.session.pop_packet().is_none());
        assert!(bus.ble.original_flash.is_none());
        assert!(!bus.ble.hooks.is_empty());
    }
    #[test]
    fn vhci_send_cap_rejects_readable_oversized_packet() {
        let (mut bus, symbols) = fixture();
        bus.ble.status = 2;
        let data = DRAM_LOW + 0x2000;
        // Both packets fit DRAM. A valid H4 prefix alone must not bypass the transport cap.
        let mut bytes = vec![0; 497];
        bytes[..4].copy_from_slice(&[1, 3, 12, 0]);
        bus.load_bytes(data, &bytes).unwrap();
        for len in [496, 497] {
            bus.ble.send_ready = false;
            let mut cpu = caller(symbols["esp_vhci_host_send_packet"], data);
            cpu.x[11] = len;
            assert!(intercept(&mut cpu, &mut bus));
            assert_eq!(bus.ble.send_ready, len == 496);
            assert_eq!(bus.ble.session.pop_packet().is_some(), len == 496);
        }
    }
    #[test]
    fn trampoline_extent_uses_symbol_size_including_alignment() {
        let (mut bus, mut symbols) = fixture();
        symbols.insert("esp_bt_controller_init".into(), BASE + 2);
        let required = INIT + CODE.len() as u32 + 2;
        assert!(bus.ble.enable(&symbols, required - 1, &SocBus::abi()).is_err());
        assert!(bus.ble.enable(&symbols, 0, &SocBus::abi()).is_err());
        assert!(bus.ble.enable(&symbols, required, &SocBus::abi()).is_ok());
    }
}
