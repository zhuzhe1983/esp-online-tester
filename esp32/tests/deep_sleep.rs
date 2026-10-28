use emu_core::Bus as _;
use esp_soc::SocBus as _;

const RTC: u32 = 0x3ff4_8000;

fn machine() -> esp32::Machine {
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.console.capture = true;
    m.bus.load_bytes(0x4008_0000, &[0x06, 0xff, 0xff]).unwrap(); // j self
    m.bus.load_bytes(0x4000_0400, &[0x06, 0xff, 0xff]).unwrap();
    m.cores[0].pc = 0x4008_0000;
    m.bus.write32(0x3ff0_0030, 1).unwrap();
    m.bus.write32(0x3ff0_002c, 0).unwrap();
    m.run(128); // release the secondary core before configuring sleep
    for c in &mut m.cores {
        c.pc = 0x4008_0000;
        c.ps = 0;
    }
    m.bus.gpio_set_input(0, true);
    m.bus.write32(0x3ff4_8498, (1 << 19) | (1 << 13)).unwrap(); // RTC mux/input
    m.bus.write32(0x3ff4_84bc, 11 << 27).unwrap(); // RTC IO11 = GPIO0
    m.bus.write32(RTC + 0x38, 1 << 11).unwrap(); // EXT0 enable, low level
    m.bus.write32(RTC + 0x84, 1 << 31).unwrap(); // power down digital domain
    m
}

#[test]
fn sleep_halts_both_cores_and_ext0_resets_with_retained_memory() {
    let mut m = machine();
    m.bus.write32(0x5000_0000, 0x1234_5678).unwrap();
    m.bus.write32(0x3ff8_0000, 0x8765_4321).unwrap();
    for c in &mut m.cores {
        c.intenable = u32::MAX;
        c.interrupt = u32::MAX; // includes NMI and local timer
        c.ccompare = [c.ccount + 100; 3];
    }
    let counts: Vec<_> = m.cores.iter().map(|c| c.insn_count).collect();
    let pcs: Vec<_> = m.cores.iter().map(|c| c.pc).collect();
    let before = m.bus.cycles;
    m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
    assert!(matches!(m.run(4096), esp32::Stop::MaxInsns));
    assert!(m.bus.cycles > before);
    for (i, c) in m.cores.iter().enumerate() {
        assert_eq!(c.insn_count, counts[i], "powered-off core {i} executed");
        assert_eq!(c.pc, pcs[i]);
    }
    assert!(m.bus.periph.rtc.0.slow_ticks > 0);
    assert!(!m.bus.sw_reset());
    m.bus.gpio_set_input(2, false); // wrong GPIO cannot wake GPIO0
    assert!(!m.bus.sw_reset());
    m.bus.gpio_set_input(0, false);
    assert!(matches!(m.run(100), esp32::Stop::SwReset));
    assert_eq!(m.bus.reset_cause(), 5);
    assert_eq!(m.bus.read32(RTC + 0x38).unwrap() & 0x7ff, 1);
    m.reboot();
    assert_eq!(m.bus.read32(RTC + 0x34).unwrap() & 0xfff, 5 | (5 << 6));
    assert_eq!(m.bus.read32(RTC + 0x38).unwrap() & 0x7ff, 1);
    m.bus.write32(RTC + 0x38, 0).unwrap(); // cannot clear read-only cause
    assert_eq!(m.bus.read32(RTC + 0x38).unwrap() & 0x7ff, 1);
    assert_eq!(m.bus.read32(0x5000_0000).unwrap(), 0x1234_5678);
    assert_eq!(m.bus.read32(0x3ff8_0000).unwrap(), 0x8765_4321);
    assert!(!m.cores[0].waiting);
    assert_eq!(m.cores[0].pc, 0x4000_0400);
}

#[test]
fn ext0_enable_mux_polarity_and_already_active_level() {
    for mode in 0..5 {
        let mut m = machine();
        match mode {
            0 => m.bus.write32(RTC + 0x38, 0).unwrap(),  // disabled
            1 => m.bus.write32(0x3ff4_8498, 0).unwrap(), // not RTC mux
            2 => m.bus.write32(0x3ff4_84bc, 31 << 27).unwrap(), // invalid RTC IO
            3 => m.bus.write32(RTC + 0x80, 1 << 20).unwrap(), // RTC peripheral off
            _ => m.bus.write32(RTC + 0x60, 1 << 30).unwrap(), // high level
        }
        m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
        if mode == 4 {
            assert!(
                m.bus.sw_reset(),
                "already active EXT0 must wake immediately"
            );
            assert_eq!(
                m.bus.read32(RTC + 0x40).unwrap() & 2,
                0,
                "EXT0 is not a reject source"
            );
        } else {
            m.bus.gpio_set_input(0, false);
            assert!(!m.bus.sw_reset(), "mode {mode}");
        }
    }
}

#[test]
fn rtc_memory_power_down_and_force_power_up() {
    for (pwc, expected) in [
        ((1 << 17) | (1 << 14), 0),
        ((1 << 15) | (1 << 12), 0),
        ((1 << 17) | (1 << 14) | (1 << 16) | (1 << 13), 0x1122_3344),
    ] {
        let mut m = machine();
        m.bus.write32(0x5000_0000, 0x1122_3344).unwrap();
        m.bus.write32(0x3ff8_0000, 0x1122_3344).unwrap();
        m.bus.write32(RTC + 0x80, pwc).unwrap();
        let pages = [
            m.bus.code_page(0x5000_0000) as usize,
            m.bus.code_page(0x400c_0000) as usize,
        ];
        let versions = pages.map(|p| m.bus.page_versions()[p]);
        m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
        for (p, v) in pages.into_iter().zip(versions) {
            assert_eq!(
                m.bus.page_versions()[p] != v,
                expected == 0,
                "RAM loss must invalidate decoded code"
            );
        }
        m.bus.gpio_set_input(0, false);
        assert_eq!(m.bus.reboot([0; 6]), 5);
        assert_eq!(m.bus.read32(0x5000_0000).unwrap(), expected);
        assert_eq!(m.bus.read32(0x3ff8_0000).unwrap(), expected);
    }
}

#[test]
fn board_input_edge_wakes_without_guest_polling() {
    struct WakePin {
        at: u64,
        due: bool,
        sent: bool,
    }
    impl esp_soc::BoardModel for WakePin {
        fn name(&self) -> &'static str {
            "test-wake-pin"
        }
        fn advance_to(&mut self, cycle: u64) {
            self.due = cycle >= self.at;
        }
        fn next_deadline(&self) -> Option<u64> {
            (!self.sent).then_some(self.at)
        }
        fn take_edges(&mut self) -> Vec<esp_soc::board::BoardEdge> {
            if self.due && !self.sent {
                self.sent = true;
                vec![esp_soc::board::BoardEdge {
                    cycle: self.at,
                    pin: 0,
                    level: false,
                }]
            } else {
                Vec::new()
            }
        }
    }
    let mut m = machine();
    m.bus.board = Box::new(WakePin {
        at: m.bus.cycles + 2000,
        due: false,
        sent: false,
    });
    m.bus.attach_board_devices();
    m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
    let before = m.insns();
    assert!(matches!(m.run(4096), esp32::Stop::SwReset));
    assert_eq!(m.insns(), before);
    assert_eq!(m.bus.reset_cause(), 5);
}

#[test]
fn ext0_input_release_is_a_wake_event() {
    let mut m = machine();
    m.bus.gpio_set_input(0, false);
    m.bus.write32(RTC + 0x60, 1 << 30).unwrap();
    m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
    assert!(!m.bus.sw_reset());
    m.bus.gpio_release_input(0);
    assert!(m.bus.sw_reset());
    assert_eq!(m.bus.reset_cause(), 5);
}

#[test]
fn secondary_release_cannot_escape_power_down() {
    let mut m = esp32::machine([0; 6], 4 << 20);
    m.bus.write32(0x3ff0_0030, 1).unwrap();
    m.bus.write32(0x3ff0_002c, 0).unwrap();
    m.bus.write32(RTC + 0x84, 1 << 31).unwrap();
    m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
    assert!(matches!(m.run(1024), esp32::Stop::MaxInsns));
    assert_eq!(m.insns(), 0);
}

#[test]
fn light_sleep_does_not_enter_deep_sleep() {
    let mut m = machine();
    m.bus.write32(RTC + 0x84, 0).unwrap();
    m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
    let before = m.insns();
    m.run(128);
    assert_eq!(m.insns(), before, "light sleep also parks the CPUs");
    m.bus.gpio_set_input(0, false);
    assert!(!m.bus.sw_reset());
    m.run(128);
    assert!(m.insns() > before, "light sleep resumes without reset");
}

struct UnitCost;
impl emu_core::CostModel for UnitCost {
    fn lifecycle(&mut self, _: &emu_core::LifecycleFacts) -> Result<(), String> {
        Ok(())
    }
    fn cycles(&mut self, _: &emu_core::ExecutionFacts<'_>) -> Result<u32, String> {
        Ok(1)
    }
}

// GPIO4 positive edge after sleep entry: GPIO_PIN4.INT_ENA selects PRO/APP
// normal/NMI sources. ESP-IDF v5.5.4 components/soc/esp32/register/soc/
// gpio_reg.h (GPIO_PIN4), dport_reg.h (PRO/APP GPIO interrupt maps), and
// components/soc/esp32/include/soc/interrupts.h (sources 22/23).
fn late_digital_irq(core: usize, nmi: bool, modeled: bool) {
    struct Edge {
        at: u64,
        sent: bool,
        due: bool,
    }
    impl esp_soc::BoardModel for Edge {
        fn name(&self) -> &'static str {
            "late-digital-irq"
        }
        fn next_deadline(&self) -> Option<u64> {
            (!self.sent).then_some(self.at)
        }
        fn advance_to(&mut self, cycle: u64) {
            self.due = cycle >= self.at;
        }
        fn take_edges(&mut self) -> Vec<esp_soc::board::BoardEdge> {
            if self.due && !self.sent {
                self.sent = true;
                vec![esp_soc::board::BoardEdge {
                    cycle: self.at,
                    pin: 4,
                    level: true,
                }]
            } else {
                vec![]
            }
        }
    }
    // The awake control proves the same late source actually reaches this CPU.
    for asleep in [false, true] {
        let mut m = esp32::machine([0; 6], 4 << 20);
        m.console.capture = true;
        if modeled {
            m.set_cost_model(Box::new(UnitCost)).unwrap();
        }
        for addr in [0x4008_0000, 0x4000_02c0, 0x4000_0180] {
            m.bus.load_bytes(addr, &[0x06, 0xff, 0xff]).unwrap();
        }
        m.cores[0].pc = 0x4008_0000;
        m.cores[0].ps = 15;
        m.bus.write32(0x3ff0_0030, 1).unwrap();
        m.bus.write32(0x3ff0_002c, 0).unwrap();
        m.run(128);
        for c in &mut m.cores {
            c.pc = 0x4008_0000;
            c.ps = 0;
        }
        let line = if nmi { 14 } else { 9 };
        let source = if nmi { 23 } else { 22 };
        let enable = 1 << ((1 - core) * 2 + usize::from(nmi));
        m.cores[core].intenable = 1 << line;
        m.bus.gpio_set_input(4, false);
        m.bus.write32(0x3ff4_9048, (2 << 12) | (1 << 9)).unwrap();
        m.bus
            .write32(0x3ff4_4098, (1 << 7) | (enable << 13))
            .unwrap();
        let map = if core == 0 { 0x3ff0_0104 } else { 0x3ff0_0218 };
        m.bus.write32(map + source * 4, line).unwrap();
        let at = m.bus.cycles + 2000;
        m.bus.board = Box::new(Edge {
            at,
            sent: false,
            due: false,
        });
        m.bus.attach_board_devices();
        if asleep {
            m.bus.write32(RTC + 0x84, 1 << 31).unwrap();
            m.bus.write32(RTC + 0x18, 1 << 31).unwrap();
        }
        let counts: Vec<_> = m.cores.iter().map(|c| c.insn_count).collect();
        let pcs: Vec<_> = m.cores.iter().map(|c| c.pc).collect();
        let interrupts = m.interrupts;
        m.max_cycles = at + 10_000;
        let stop = m.run(4096);
        assert!(
            matches!(stop, esp32::Stop::MaxInsns | esp32::Stop::Halted),
            "{stop:?}, asleep={asleep}"
        );
        assert!(m.bus.cycles >= at, "late GPIO edge was not reached");
        assert!(!m.bus.sw_reset(), "digital IRQ must not request RTC wake");
        assert_ne!(
            m.cores[core].ext_irq_lines & (1 << line),
            0,
            "source not presented"
        );
        if asleep {
            assert_eq!(m.interrupts, interrupts, "powered-off CPU took digital IRQ");
            for (i, c) in m.cores.iter().enumerate() {
                assert_eq!(
                    c.insn_count, counts[i],
                    "powered-off core {i} retired instructions"
                );
                assert_eq!(c.pc, pcs[i]);
                assert_eq!(c.interrupt, 0, "late IRQ escaped power-down parking");
                assert!(c.waiting);
            }
        } else {
            assert!(m.interrupts > interrupts, "awake control did not take IRQ");
            assert!(m.cores[core].insn_count > counts[core]);
        }
    }
}

macro_rules! late_irq_test {
    ($name:ident, $core:expr, $nmi:expr, $modeled:expr) => {
        #[test]
        fn $name() {
            late_digital_irq($core, $nmi, $modeled);
        }
    };
}
late_irq_test!(late_nmi_pro_unmodeled, 0, true, false);
late_irq_test!(late_nmi_app_unmodeled, 1, true, false);
late_irq_test!(late_nmi_pro_modeled, 0, true, true);
late_irq_test!(late_nmi_app_modeled, 1, true, true);
late_irq_test!(late_maskable_pro_unmodeled, 0, false, false);
late_irq_test!(late_maskable_app_unmodeled, 1, false, false);
late_irq_test!(late_maskable_pro_modeled, 0, false, true);
late_irq_test!(late_maskable_app_modeled, 1, false, true);
