//! Synthetic reset-release regressions, without ROM or firmware.
//! ESP-IDF v5.5.5 components/soc/esp32s3/register/soc/system_reg.h:
//! CORE_1_CONTROL_0 (lines 15..33, bits 0..2), CPU_INTR_FROM_CPU_0 (line 681, +0x30).
//! Interrupt source 79: components/soc/esp32s3/include/soc/interrupts.h:98.
//! Core-1 route: components/soc/esp32s3/register/soc/interrupt_core1_reg.h:649
//! (INTERRUPT_CORE1_CPU_INTR_FROM_CPU_0_MAP_REG, +0x93c).
use emu_core::Bus;

struct UnitCost;
impl emu_core::CostModel for UnitCost {
    fn lifecycle(&mut self, _: &emu_core::LifecycleFacts) -> Result<(), String> { Ok(()) }
    fn cycles(&mut self, _: &emu_core::ExecutionFacts<'_>) -> Result<u32, String> { Ok(1) }
}

fn machine(mode: u32, line: u32) -> esp32s3::Machine {
    let mut m = esp32s3::machine([0; 6]);
    m.quantum = 1;
    match mode {
        0 => {},
        1 => m.set_cost_model(Box::new(UnitCost)).unwrap(),
        2 => m.set_approximate_jit_timing(1, 1).unwrap(),
        3 => {
            m.set_approximate_jit_timing(1, 1).unwrap();
            m.set_approximate_jit_frontiers(true).unwrap();
        },
        _ => unreachable!(),
    }
    esp_soc::SocBus::load_bytes(&mut m.bus, 0x4037_0000, &[0x06, 0xff, 0xff]).unwrap(); // j .
    m.cores[0].pc = 0x4037_0000;
    m.cores[0].ps = 15;
    m.bus.write32(0x600c_2800 + 4 * 79, line).unwrap();
    m.bus.write32(0x600c_0030, 1).unwrap();
    m
}

fn assert_line(m: &esp32s3::Machine, line: u32) {
    assert_eq!(m.cores[1].ext_irq_lines & (1 << line), 1 << line,
        "asserted line must survive APP reset/release: line={line}");
    assert_eq!(m.cores[1].interrupt & (1 << line), 1 << line);
}

#[test]
fn held_release_republishes_asserted_level_and_edge() {
    for mode in 0..4 {
        for line in [9, 10] {
            let mut m = machine(mode, line);
            m.run(1); // Sample APP held in reset and publish the source once.
            assert_eq!(m.cores[1].ext_irq_lines & (1 << line), 1 << line);
            m.bus.write32(0x600c_0000, 2).unwrap();
            m.run(1); // Reset APP on release, without a new source transition.
            assert_line(&m, line);
            // Acknowledging an edge must not retrigger it on clean rounds.
            m.cores[1].waiting = true;
            m.cores[1].ps = 15;
            if line == 10 { m.cores[1].write_sr(227, 1 << line); }
            m.run(32);
            assert_eq!(m.cores[1].interrupt & (1 << line), if line == 10 { 0 } else { 1 << line });
            assert!(!m.bus.irq_dirty);
            m.bus.write32(0x600c_0030, 0).unwrap();
            m.run(1);
            assert_eq!(m.cores[1].ext_irq_lines & (1 << line), 0);
            if line == 9 { assert_eq!(m.cores[1].interrupt & (1 << line), 0); }
        }
    }
}

#[test]
fn guest_release_republishes_on_the_following_round() {
    for mode in 0..4 {
        for line in [9, 10] {
            let mut m = machine(mode, line);
            // s32i.n a3,a2,0 ; j . -- release APP from a PRO instruction.
            esp_soc::SocBus::load_bytes(&mut m.bus, 0x4037_0000, &[0x39, 0x02, 0x06, 0xff, 0xff]).unwrap();
            m.cores[0].waiting = false;
            m.cores[0].pc = 0x4037_0000;
            m.cores[0].set_ar(2, 0x600c_0000);
            m.cores[0].set_ar(3, 2);
            m.run(1);
            assert_eq!(m.bus.periph.core1_control(), (true, false, false));
            m.run(1);
            assert_line(&m, line);
        }
    }
}

#[test]
fn repeated_reset_clock_hold_and_stall_release() {
    for mode in 0..4 {
        for line in [9, 10] {
            let mut m = machine(mode, line);
            for _ in 0..3 {
                m.bus.write32(0x600c_0000, 4).unwrap(); // Reset asserted.
                m.run(1);
                m.bus.write32(0x600c_0000, 0).unwrap(); // Clock held.
                m.run(1);
                m.bus.write32(0x600c_0000, 3).unwrap(); // Clock on, stalled.
                m.run(1);
                m.cores[1].cpenable = 0x1234;
                m.script.events.push((m.bus.cycles, esp_soc::ScriptAction::Poke(0x600c_0000, 2)));
                m.run(1);
                assert_eq!(m.cores[1].cpenable, 0x0f, "release must reset core");
                assert_line(&m, line);
                m.cores[1].waiting = true;
                m.cores[1].ps = 15;
                if line == 10 { m.cores[1].write_sr(227, 1 << line); }
                m.cores[1].cpenable = 0x1234;
                m.bus.write32(0x600c_0000, 3).unwrap();
                m.run(1);
                m.bus.write32(0x600c_0000, 2).unwrap();
                m.run(1);
                assert_eq!(m.cores[1].cpenable, 0x1234, "stall alone must not reset core");
                assert_eq!(m.cores[1].interrupt & (1 << line), if line == 10 { 0 } else { 1 << line });
            }
            m.reboot();
            m.run(0);
            assert_eq!(m.cores[0].ext_irq_lines, 0);
            assert_eq!(m.cores[1].ext_irq_lines, 0);
        }
    }
}
