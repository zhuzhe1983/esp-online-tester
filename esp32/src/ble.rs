//! Classic ESP32 windowed-Xtensa calling glue for the virtual VHCI controller.
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
    fn abi() -> Abi {
        Abi {
            code: CODE,
            ram: DRAM_LOW..DRAM_HIGH,
            hz: crate::periph::CPU_HZ,
            // `esp_bt_controller_config_t.mode`, at 20 from IDF v4.3 to v6.0
            // (components/bt/include/esp32/include/esp_bt.h)
            config_mode: Some(20),
        }
    }
    fn ble(&mut self) -> &mut Ble {
        &mut self.ble
    }
    fn valid_callback(&mut self, pc: u32) -> bool {
        self.fetch(pc)
            .is_ok_and(|bytes| decode(pc, bytes).op == Op::Entry)
    }
    fn flash_offset(&mut self, pc: u32) -> Option<usize> {
        self.flash_off(pc)
    }
    fn ble_debug(&self) -> bool {
        self.debug.has("ble")
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

    const BASE: u32 = 0x4008_0000;
    const RETURN: u32 = 0x4008_1000;
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
            ("_bt_controller_bss_start", 0x3ffc_7104),
            ("_bt_controller_bss_end", 0x3ffc_7c7c),
        ]
        .into_iter()
        .map(|(n, a)| (n.into(), a))
        .collect();
        for &pc in symbols.values().filter(|&&pc| pc >= BASE) {
            bus.load_bytes(pc, &[0x36, 0x41, 0, 0x90, 0, 0]).unwrap();
        }
        for pc in [SEND_CB, RECV_CB] {
            bus.load_bytes(pc, &[0x36, 0x41, 0, 0x90, 0, 0]).unwrap();
        }
        bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap();
        (bus, symbols)
    }

    fn caller(pc: u32, arg: u32) -> Cpu {
        let mut cpu = Cpu::new(0);
        cpu.lx6 = true;
        cpu.ps = ps::WOE | ps::UM | (2 << ps::CALLINC_SHIFT);
        cpu.windowbase = 0;
        cpu.windowstart = 1;
        cpu.pc = pc;
        cpu.set_ar(1, 0x3ffe_0000);
        cpu.set_ar(8, (2 << 30) | (RETURN & 0x3fff_ffff));
        cpu.set_ar(10, arg);
        cpu
    }

    #[test]
    fn trampoline_creates_guest_task_with_windowed_abi_and_returns_esp_ok() {
        let (mut bus, _) = fixture();
        // a config at DRAM_LOW whose `mode` is BLE
        bus.load_bytes(DRAM_LOW + 20, &[1]).unwrap();
        let mut cpu = caller(BASE, DRAM_LOW);
        let before = cpu.ccount;
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.ccount, before + 1);
        assert_eq!(cpu.insn_count, 1);
        let mut created = false;
        for _ in 0..100 {
            if cpu.pc == RETURN {
                break;
            }
            if cpu.pc == bus.ble.create {
                let args: Vec<u32> = (0..6).map(|n| cpu.arg(&mut bus, n)).collect();
                assert_eq!(args, [BASE + TASK, bus.ble.buffer + 4, 4096, 0, 23, 0]);
                assert_eq!(
                    bus.read32_unpriced(cpu.get_ar(1)).unwrap(),
                    0,
                    "core ID is seventh stack argument"
                );
                cpu.return_from_stub(&mut bus, 1);
                created = true;
            } else if !intercept(&mut cpu, &mut bus) {
                assert!(cpu.step(&mut bus).trap().is_none());
            }
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

    /// Init's config names the mode, and enable takes that one only, as IDF's bt.c does (v5.3.6
    /// components/bt/controller/esp32/bt.c:1918); the classic ESP32's config may name 1, 2 or 3.
    #[test]
    fn enable_takes_the_mode_of_the_config_init_took() {
        const INVALID_STATE: u32 = 0x103;
        let call = |bus: &mut SocBus, pc: u32, arg: u32| {
            let mut cpu = caller(pc, arg);
            assert!(intercept(&mut cpu, bus));
            cpu.get_ar(10)
        };
        for mode in 1..=3u32 {
            let (mut bus, symbols) = fixture();
            // the task exists already: init returns at once, as after a deinit
            bus.ble.task_created = true;
            bus.load_bytes(DRAM_LOW + 20, &[mode as u8]).unwrap();
            assert_eq!(call(&mut bus, BASE, DRAM_LOW), 0);
            assert_eq!((bus.ble.status, bus.ble.mode), (1, mode));
            for other in (1..=3).filter(|&m| m != mode) {
                assert_eq!(
                    call(&mut bus, symbols["esp_bt_controller_enable"], other),
                    INVALID_ARG,
                    "mode {mode}, enable {other}"
                );
                assert_eq!(bus.ble.status, 1);
            }
            assert_eq!(call(&mut bus, symbols["esp_bt_controller_enable"], mode), 0);
            assert_eq!(bus.ble.status, 2);
        }
        for mode in [0u8, 4, 0xff] {
            let (mut bus, _) = fixture();
            bus.ble.task_created = true;
            bus.load_bytes(DRAM_LOW + 20, &[mode]).unwrap();
            assert_eq!(call(&mut bus, BASE, DRAM_LOW), INVALID_ARG, "config mode {mode}");
            assert_eq!(bus.ble.status, 0);
        }
        // an init refused for its state leaves the mode of the one that was taken
        let (mut bus, symbols) = fixture();
        bus.ble.task_created = true;
        bus.load_bytes(DRAM_LOW + 20, &[2]).unwrap();
        assert_eq!(call(&mut bus, BASE, DRAM_LOW), 0);
        bus.load_bytes(DRAM_LOW + 20, &[1]).unwrap();
        assert_eq!(call(&mut bus, BASE, DRAM_LOW), INVALID_STATE);
        assert_eq!(call(&mut bus, symbols["esp_bt_controller_enable"], 2), 0);
        // a config that cannot be read, and one whose mode would wrap the address space
        let (mut bus, _) = fixture();
        bus.ble.task_created = true;
        assert_eq!(call(&mut bus, BASE, 0x1000), INVALID_ARG);
        assert_eq!(bus.last_fault, Some((0x1014, false)), "the mode's read missed");
        let (mut bus, _) = fixture();
        bus.ble.task_created = true;
        assert_eq!(call(&mut bus, BASE, 0xffff_fff0), INVALID_ARG);
        assert_eq!(bus.last_fault, None);
        // init(NULL) is refused without reading the guest's memory
        let (mut bus, _) = fixture();
        assert_eq!(call(&mut bus, BASE, 0), INVALID_ARG);
        assert_eq!(bus.last_fault, None);
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
                let bytes: Vec<u8> = (0..len)
                    .map(|i| bus.read8_unpriced(ptr + i).unwrap())
                    .collect();
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
                if ready {
                    break;
                }
                cpu.return_from_stub(&mut bus, 0);
            } else if !intercept(&mut cpu, &mut bus) {
                assert!(cpu.step(&mut bus).trap().is_none());
            }
        }
        assert!(received && ready && delayed);
    }

    #[test]
    fn invalid_guest_buffers_and_missing_symbols_are_rejected() {
        let (mut bus, mut symbols) = fixture();
        let mut cpu = caller(symbols["esp_vhci_host_register_callback"], 0xffff_fffc);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.get_ar(10), INVALID_ARG);
        let table = 0x3ff9_1000;
        bus.load_bytes(table, &SEND_CB.to_le_bytes()).unwrap();
        bus.load_bytes(table + 4, &RECV_CB.to_le_bytes()).unwrap();
        let mut cpu = caller(symbols["esp_vhci_host_register_callback"], table);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(
            cpu.get_ar(10),
            0,
            "a static callback table can live in DROM"
        );
        assert_eq!(bus.ble.callbacks, [SEND_CB, RECV_CB]);
        let start = symbols["_bt_controller_bss_start"];
        let end = symbols["_bt_controller_bss_end"];
        symbols.insert("_bt_controller_bss_start".into(), 0x1000);
        symbols.insert("_bt_controller_bss_end".into(), 0x2000);
        assert!(bus.ble.enable(&symbols, 0x400, &SocBus::abi()).is_err());
        symbols.insert("_bt_controller_bss_start".into(), start);
        symbols.insert("_bt_controller_bss_end".into(), bus.ble.buffer + 511);
        assert!(bus.ble.enable(&symbols, 0x400, &SocBus::abi()).is_err());
        symbols.insert("_bt_controller_bss_end".into(), end);
        symbols.remove("vTaskDelay");
        assert!(bus
            .ble
            .enable(&symbols, 0x400, &SocBus::abi())
            .unwrap_err()
            .contains("vTaskDelay"));
    }

    #[test]
    fn machine_dispatches_chip_hooks_as_function_boundaries() {
        let (bus, symbols) = fixture();
        let mut m = esp_soc::Machine::<crate::Esp32>::new([0; 6], bus);
        m.quantum = 1;
        m.bus.ble.status = 2;
        m.cores[0] = caller(symbols["esp_bt_controller_get_status"], 0);
        assert!(matches!(m.run(1), esp_soc::Stop::MaxInsns));
        assert_eq!(m.cores[0].pc, RETURN);
        assert_eq!(m.cores[0].get_ar(10), 2);
    }
    #[test]
    fn reboot_restores_controller_flash_even_after_mmu_mapping_changes() {
        let (mut bus, symbols) = fixture();
        let init = crate::bus::IBUS_LOW;
        let symbols = symbols
            .into_iter()
            .map(|(name, address)| {
                (
                    name,
                    if address >= BASE {
                        address - BASE + init
                    } else {
                        address
                    },
                )
            })
            .collect();
        bus.ble.enable(&symbols, 0x400, &SocBus::abi()).unwrap();
        let slot = 64 + ((init - 0x4000_0000) >> 16) as usize;
        bus.mmu[0][slot] = 0;
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
        bus.mmu[0][slot] = 1;
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
    fn callback_requires_readable_windowed_function_entry() {
        let (mut bus, _) = fixture();
        assert!(bus.valid_callback(SEND_CB));
        bus.load_bytes(SEND_CB, &[0, 0, 0]).unwrap();
        assert!(!bus.valid_callback(SEND_CB));
        assert!(!bus.valid_callback(u32::MAX));
    }

    #[test]
    fn host_enable_uses_elf_extent_and_reports_enabled_state() {
        use esp_soc::SocBus as _;
        let (_, symbols) = fixture();
        let mut bus = SocBus::new(0, [0; 6]);
        let mut elf = esp_soc::elf::Elf { by_name: symbols, ..Default::default() };
        assert!(!bus.ble_enabled());
        assert!(bus.enable_ble(&elf).is_err());
        elf.symbol_sizes.insert("esp_bt_controller_init".into(), 0x400);
        bus.enable_ble(&elf).unwrap();
        assert!(bus.ble_enabled());
        assert_eq!(bus.ble_pending_commands(), 0);
    }

}
