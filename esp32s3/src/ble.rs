//! S3 windowed-Xtensa calling glue for the virtual VHCI controller.
use crate::bus::{SocBus, DRAM_HIGH, DRAM_LOW};
use emu_core::Bus;
use esp_soc::ble::vhci::{Abi, Ble, VhciBus, XTENSA_CODE as CODE};
use xtensa_lx7::decode::{decode, Op};

pub fn intercept(cpu: &mut xtensa_lx7::Cpu, bus: &mut SocBus) -> bool {
    esp_soc::ble::vhci::intercept(cpu, bus, |cpu, pc| {
        cpu.pc = pc;
        cpu.insn_count += 1;
        emu_core::Core::advance_cycles(cpu, 1);
    })
}

impl VhciBus for SocBus {
    fn abi() -> Abi { Abi { code: CODE, ram: DRAM_LOW..DRAM_HIGH, hz: crate::periph::CPU_HZ, config_mode: None } }
    fn ble(&mut self) -> &mut Ble { &mut self.ble }
    fn valid_callback(&mut self, pc: u32) -> bool { self.fetch(pc).is_ok_and(|bytes| decode(pc, bytes).op == Op::Entry) }
    fn flash_offset(&mut self, pc: u32) -> Option<usize> { self.flash_off(pc) }
    fn ble_debug(&self) -> bool { self.debug.has("ble") }
    fn valid_packet_range(&self, address: u32, len: usize) -> bool {
        use crate::bus::{DBUS_LOW, DBUS_HIGH, MMU_INVALID, MMU_SPIRAM};
        let Some(end) = address.checked_add(len as u32) else { return false };
        if address >= DRAM_LOW && end <= DRAM_HIGH { return true; }
        if len == 0 || address < DBUS_LOW || end > DBUS_HIGH { return false; }
        let first = ((address - DBUS_LOW) >> 16) as usize;
        let last = ((end - 1 - DBUS_LOW) >> 16) as usize;
        self.mmu[first..=last].iter().all(|entry| {
            entry & (MMU_INVALID | MMU_SPIRAM) == MMU_SPIRAM
                && ((entry & 0x3fff) as usize + 1) * 0x10000 <= self.psram.len()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::Core;
    use esp_soc::ble::vhci::{install, INIT, TASK};
    use std::collections::HashMap;
    use xtensa_lx7::Cpu;
    const INVALID_ARG: u32 = 0x102;
    use xtensa_lx7::state::ps;

    const BASE: u32 = 0x4038_8000;
    const RETURN: u32 = 0x4038_9000;
    const SEND_CB: u32 = BASE + 0x700;
    const RECV_CB: u32 = BASE + 0x710;

    fn fixture() -> (SocBus, HashMap<String, u32>) {
        let mut bus = SocBus::new(0x400000, 0, [0; 6]);
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
        for &pc in symbols.values().filter(|&&pc| pc >= BASE) { bus.load_bytes(pc, &[0x36, 0x41, 0, 0x90, 0, 0]).unwrap(); }
        for pc in [SEND_CB, RECV_CB] { bus.load_bytes(pc, &[0x36, 0x41, 0, 0x90, 0, 0]).unwrap(); }
        bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap();
        (bus, symbols)
    }

    fn caller(pc: u32, arg: u32) -> Cpu {
        let mut cpu = Cpu::new(0);
        cpu.ps = ps::WOE | ps::UM | (2 << ps::CALLINC_SHIFT);
        cpu.windowbase = 0;
        cpu.windowstart = 1;
        cpu.pc = pc;
        cpu.set_ar(1, 0x3fcf_0000);
        cpu.set_ar(8, (2 << 30) | (RETURN & 0x3fff_ffff));
        cpu.set_ar(10, arg);
        cpu
    }

    #[test]
    fn trampoline_creates_guest_task_with_windowed_abi_and_returns_esp_ok() {
        let (mut bus, _) = fixture();
        let mut cpu = caller(BASE, DRAM_LOW);
        assert!(intercept(&mut cpu, &mut bus));
        let mut created = false;
        for _ in 0..100 {
            if cpu.pc == RETURN { break; }
            if cpu.pc == bus.ble.create {
                let args: Vec<u32> = (0..6).map(|n| cpu.arg(&mut bus, n)).collect();
                assert_eq!(args, [BASE + TASK, bus.ble.buffer + 4, 4096, 0, 23, 0]);
                assert_eq!(bus.read32_unpriced(cpu.get_ar(1)).unwrap(), 0, "core ID is seventh stack argument");
                cpu.return_from_stub(&mut bus, 1);
                created = true;
            } else if !intercept(&mut cpu, &mut bus) { assert!(cpu.step(&mut bus).trap().is_none()); }
        }
        assert!(created);
        assert_eq!(cpu.pc, RETURN);
        assert_eq!(cpu.get_ar(10), 0);
        assert_eq!(cpu.windowbase, 0);
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
        let call = |bus: &mut SocBus, pc: u32, arg: u32| { let mut cpu = caller(pc, arg); assert!(intercept(&mut cpu, bus)); cpu.get_ar(10) };
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
    fn invalid_guest_buffers_and_missing_symbols_are_rejected() {
        let (mut bus, mut symbols) = fixture();
        let mut cpu = caller(symbols["esp_vhci_host_register_callback"], 0xffff_fffc);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.get_ar(10), INVALID_ARG);
        let table = 0x3fc9_2000;
        bus.load_bytes(table, &SEND_CB.to_le_bytes()).unwrap();
        bus.load_bytes(table + 4, &RECV_CB.to_le_bytes()).unwrap();
        let mut cpu = caller(symbols["esp_vhci_host_register_callback"], table);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.get_ar(10), 0, "a static callback table can live in DROM");
        assert_eq!(bus.ble.callbacks, [SEND_CB, RECV_CB]);
        symbols.remove("vTaskDelay");
        assert!(bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap_err().contains("vTaskDelay"));
    }

    #[test]
    fn machine_dispatches_chip_hooks_as_function_boundaries() {
        struct SingleStep;
        impl esp_soc::Observer<crate::S3> for SingleStep {
            fn name(&self) -> &'static str { "single-step" }
            fn wants(&self) -> esp_soc::Wants { esp_soc::Wants::INSN }
        }
        for single_step in [false, true] {
            let (bus, symbols) = fixture();
            let mut m = esp_soc::Machine::<crate::S3>::new([0; 6], bus);
            m.quantum = 1;
            if single_step { m.add_observer(Box::new(SingleStep)); }
            m.bus.ble.status = 2;
            m.cores[0] = caller(symbols["esp_bt_controller_get_status"], 0);
            assert!(matches!(m.run(1), esp_soc::Stop::MaxInsns));
            assert_eq!(m.cores[0].pc, RETURN);
            assert_eq!(m.cores[0].get_ar(10), 2);
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
        original[..3].copy_from_slice(&[0x36, 0x41, 0]);
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
            cpu.set_ar(11, len);
            assert!(intercept(&mut cpu, &mut bus));
            assert_eq!(bus.ble.send_ready, len == 496);
            assert_eq!(bus.ble.session.pop_packet().is_some(), len == 496);
        }
    }
    #[test]
    fn vhci_accepts_mapped_psram_packets_without_accepting_flash_or_unmapped_pages() {
        use crate::bus::{DBUS_LOW, MMU_INVALID, MMU_SPIRAM};
        let (mut bus, symbols) = fixture();
        esp_soc::SocBus::set_psram_size(&mut bus, 2 << 20).unwrap();
        bus.ble.status = 2;
        let data = DBUS_LOW + 0xfffe;
        for (first, second, accepted) in [
            (MMU_SPIRAM, MMU_SPIRAM | 1, true),
            (MMU_SPIRAM, MMU_INVALID, false),
            (0, 1, false),
            (MMU_SPIRAM, MMU_SPIRAM | 32, false),
        ] {
            bus.write32(crate::bus::MMU_TABLE, first).unwrap();
            bus.write32(crate::bus::MMU_TABLE + 4, second).unwrap();
            if accepted { bus.load_bytes(data, &[1, 3, 12, 0]).unwrap(); }
            bus.ble.send_ready = false;
            let mut cpu = caller(symbols["esp_vhci_host_send_packet"], data);
            cpu.set_ar(11, 4);
            assert!(intercept(&mut cpu, &mut bus));
            assert_eq!(bus.ble.send_ready, accepted);
            assert_eq!(bus.ble.session.pop_packet().is_some(), accepted);
        }
        assert!(!bus.valid_packet_range(u32::MAX - 1, 4));
        assert!(!bus.valid_packet_range(0x6000_0000, 4));
        assert!(!bus.valid_packet_range(DRAM_HIGH - 2, 4));
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
