//! The machine without a ROM: the scheduler's idle skipping, a second core released from reset,
//! what a chip reset keeps, action-script parsing against the board, console capture. Programs
//! are two instructions long; the goldens cover real firmware.
use emu_core::{Bus, Core};
use esp32s3::board::{make_board, NoBoard};
use esp_soc::{ScriptAction, Stop};

const IRAM: u32 = 0x4037_0000;
const RESET: u32 = 0x4000_0400;
const WAITI_LOOP: [u8; 6] = [0x00, 0x70, 0x00, 0x06, 0xff, 0xff];   // waiti 0 ; j .
const SPIN: [u8; 3] = [0x06, 0xff, 0xff];                             // j .   (objdump: ffff06)

fn machine() -> esp32s3::Machine { let mut m = esp32s3::machine([1, 2, 3, 4, 5, 6]); m.console.capture = true; m }
/// q256: for tests written in 64-instruction rounds, whatever the target's default quantum.
fn machine64() -> esp32s3::Machine { let mut m = machine(); m.quantum = 64; m }
fn park(m: &mut esp32s3::Machine, core: usize, at: u32, prog: &[u8]) { esp_soc::SocBus::load_bytes(&mut m.bus, at, prog).unwrap(); m.cores[core].pc = at; m.cores[core].ps = 0; }

#[test]
fn peripheral_block_names_cover_aes_neighbors() {
    use esp32s3::periph::Peripherals;
    assert_eq!(Peripherals::block_name_pub(0x39), "USB_WRAP");
    assert_eq!(Peripherals::block_name_pub(0x3a), "AES");
    assert_eq!(Peripherals::block_name_pub(0x3b), "SHA");
}

/// A core in `waiti` with nothing pending costs no instructions: a millisecond of emulated time
/// passes in a few hundred scheduling steps.
#[test]
fn idle_cores_skip_time() {
    let mut m = machine();
    park(&mut m, 0, IRAM, &WAITI_LOOP);
    m.max_cycles = 240_000;                                   // 1 ms
    assert!(matches!(m.run(u64::MAX), Stop::Halted));
    assert!(m.bus.cycles >= 240_000);
    assert!(m.insns() < 1000, "{} instructions for 1 ms of sleep", m.insns());
    assert!(m.cores[0].waiting());
}

#[test]
fn idle_stops_at_requested_cycle_and_instruction_limits() {
    for (max_cycles, max_insns, expected, halted) in [(1, u64::MAX, 1, true), (1000, 3, 3, false)] {
        let mut m = machine();
        m.cores[0].waiting = true;
        m.cores[0].ps = 0;
        m.max_cycles = max_cycles;
        let stop = m.run(max_insns);
        assert_eq!(matches!(stop, Stop::Halted), halted);
        assert_eq!(m.bus.cycles, expected);
    }
}

#[test]
fn idle_scripts_fire_at_their_deadlines_in_both_entry_paths() {
    for until in [false, true] {
        let mut m = machine();
        m.cores[0].waiting = true;
        m.cores[0].ps = 0;
        m.script.events = vec![(5, ScriptAction::Serial("A".into())), (7, ScriptAction::Stop)];
        if until {
            assert!(matches!(m.run_until_cycle(100), esp_soc::RunUntil::Stop(Stop::Halted)));
        } else {
            assert!(matches!(m.run(u64::MAX), Stop::Halted));
        }
        assert_eq!(m.bus.cycles, 7);
        assert_eq!(m.script.pos, 2);
        assert_eq!(m.bus.periph.usb.rx.iter().copied().collect::<Vec<_>>(), b"A");
    }
}

#[test]
fn idle_cut_includes_each_enabled_cores_timer() {
    use esp_soc::observe::{Ctx, Observer, Wants};
    use std::sync::{Arc, Mutex};
    struct Rounds(Arc<Mutex<Vec<u64>>>);
    impl Observer<esp32s3::S3> for Rounds {
        fn name(&self) -> &'static str { "round-cuts" }
        fn wants(&self) -> Wants { Wants::ROUND }
        fn on_round(&mut self, cx: &Ctx) { self.0.lock().unwrap().push(cx.cycles); }
    }
    for (core, until) in [(0, false), (1, false), (0, true)] {
        let mut m = machine();
        if core == 1 {
            park(&mut m, 0, IRAM, &SPIN);
            esp_soc::SocBus::load_bytes(&mut m.bus, RESET, &SPIN).unwrap();
            m.bus.write32(0x600c_0000, 0b010).unwrap();
            m.run(64); // release and initialize core 1 before arming its timer
        }
        for c in &mut m.cores { c.waiting = true; c.ps = 0; }
        let now = m.bus.cycles;
        m.cores[core].intenable = 1 << xtensa_lx7::state::TIMER_INTERRUPT[0];
        let at = m.cores[core].ccount.wrapping_add(3);
        m.cores[core].write_sr(xtensa_lx7::state::sr::CCOMPARE0, at);
        let rounds = Arc::new(Mutex::new(Vec::new()));
        m.add_observer(Box::new(Rounds(rounds.clone())));
        m.max_cycles = now + 9;
        if until { m.run_until_cycle(now + 9); } else { m.run(u64::MAX); }
        let cuts = rounds.lock().unwrap();
        assert!(cuts.contains(&(now + 3)), "core {core}, until={until}, cuts={cuts:?}");
    }
}

#[test]
fn mixed_idle_rounds_end_at_the_sleeping_cores_timer() {
    for vq in [1, 8] {
        for sleeping in [0, 1] {
            for wake in [3, 67, 131] {
                let mut m = machine();
                m.vq_max = vq;
                for core in &mut m.cores { core.set_jit(false); }
                park(&mut m, 0, IRAM, &SPIN);
                esp_soc::SocBus::load_bytes(&mut m.bus, RESET, &SPIN).unwrap();
                m.bus.write32(0x600c_0000, 0b010).unwrap();
                m.run(64);
                m.cores[sleeping].waiting = true;
                m.cores[sleeping].ps = 0;
                m.cores[sleeping].intenable = 1 << xtensa_lx7::state::TIMER_INTERRUPT[0];
                let before: Vec<_> = m.cores.iter().map(|c| c.ccount).collect();
                m.cores[sleeping].write_sr(xtensa_lx7::state::sr::CCOMPARE0, before[sleeping].wrapping_add(wake));
                let now = m.bus.cycles;
                m.max_cycles = now + u64::from(wake);
                assert!(matches!(m.run(u64::MAX), Stop::Halted));
                assert_eq!(m.bus.cycles, now + u64::from(wake), "vq={vq}, sleeping={sleeping}");
                for (core, before) in m.cores.iter().zip(before) {
                    assert_eq!(core.ccount.wrapping_sub(before), wake, "both core clocks stay at the device horizon");
                }
                assert!(m.cores[sleeping].irq_pending());
                if std::env::var_os("ESP32SIM_VQ_NATIVE").is_some() && vq > 1 && wake == 131 {
                    assert!(m.vq_stats[0] > 0, "exercise the deferred multi-quantum path");
                }
            }
        }
    }
}

#[test]
fn software_reset_stops_before_the_siblings_store_and_charges_only_executed_time() {
    use esp_soc::observers::PcHist;
    const STORE: [u8; 3] = [0x22, 0x63, 0x00]; // s32i a2,a3,0
    const SRAM: u32 = 0x3fc9_0000;
    for (mode, cpi) in [(0, 1), (1, 1), (2, 1), (2, 3), (3, 1), (3, 3)] {
        let mut m = machine();
        m.vq_max = 1;
        if mode == 1 { m.add_observer(Box::new(PcHist::new(1))); }
        if mode >= 2 { m.set_approximate_jit_timing(cpi, 64).unwrap(); }
        if mode == 3 { m.set_approximate_jit_frontiers(true).unwrap(); }
        park(&mut m, 0, IRAM, &SPIN);
        esp_soc::SocBus::load_bytes(&mut m.bus, RESET, &SPIN).unwrap();
        m.bus.write32(0x600c_0000, 0b010).unwrap();
        m.max_cycles = 64 * u64::from(cpi);
        m.run(u64::MAX);
        m.max_cycles = u64::MAX;
        park(&mut m, 0, IRAM + 16, &STORE);
        park(&mut m, 1, IRAM + 32, &STORE);
        m.cores[0].set_ar(2, 1 << 31);
        m.cores[0].set_ar(3, 0x6000_8000); // RTC_CNTL_OPTIONS0.SW_SYS_RST
        m.cores[1].set_ar(2, 0xdead_beef);
        m.cores[1].set_ar(3, SRAM);
        let before = (m.bus.cycles, m.cores[1].insn_count(), m.run_steps(), m.cores[0].ccount);
        assert!(matches!(m.run(128), Stop::SwReset), "mode={mode}");
        assert_eq!(m.bus.read32(SRAM).unwrap(), 0, "core 1 must not store after core 0 resets, mode={mode}");
        assert_eq!((m.bus.cycles, m.cores[1].insn_count()), (before.0 + u64::from(cpi), before.1), "only the reset instruction's time, mode={mode}");
        assert_eq!(m.cores[0].ccount - before.3, cpi);
        assert_eq!(m.run_steps() - before.2, 1);
        m.reboot();
        assert_eq!(m.run_steps(), before.2 + 1, "run budget survives chip reset");
    }
}

#[test]
fn precise_trap_observer_reports_fault_after_resumed_hardware_loop() {
    use esp_soc::observe::{Ctx, Observer, Wants};
    use std::sync::{Arc, Mutex};
    struct Traps {
        traps: Arc<Mutex<Vec<(u32, emu_core::Trap)>>>,
        fragments: Arc<Mutex<Vec<(u32, u32)>>>,
        combined: bool,
    }
    impl Observer<esp32s3::S3> for Traps {
        fn name(&self) -> &'static str { "precise-traps" }
        fn wants(&self) -> Wants { if self.combined { Wants::TRAP_PC | Wants::BLOCK } else { Wants::TRAP_PC } }
        fn on_trap(&mut self, _: &Ctx, _: usize, _: &xtensa_lx7::Cpu, pc: u32, trap: &emu_core::Trap) {
            self.traps.lock().unwrap().push((pc, *trap));
        }
        fn on_block(&mut self, _: &Ctx, _: usize, pc: u32, used: u32) { self.fragments.lock().unwrap().push((pc, used)); }
    }
    for (until, combined) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut m = machine();
        // Independently assembled: addi.n a3,a3,-1 ; quou a2,a4,a3.
        park(&mut m, 0, IRAM, &[0x0b, 0x33, 0x30, 0x24, 0xc2]);
        m.cores[0].pc = IRAM + 2;
        m.cores[0].lbeg = IRAM;
        m.cores[0].lend = IRAM + 5;
        m.cores[0].lcount = 3;
        m.cores[0].set_ar(3, 2);
        m.cores[0].set_ar(4, 12);
        m.dbg.stop_after_exceptions = 1;
        let traps = Arc::new(Mutex::new(Vec::new()));
        let fragments = Arc::new(Mutex::new(Vec::new()));
        m.add_observer(Box::new(Traps { traps: traps.clone(), fragments: fragments.clone(), combined }));
        if until {
            assert!(matches!(m.run_until_cycle(64), esp_soc::RunUntil::Stop(Stop::Exceptions(1))));
        } else { assert!(matches!(m.run(64), Stop::Exceptions(1))); }
        assert_eq!(*traps.lock().unwrap(), [(IRAM + 2, emu_core::Trap::Exception(6))]);
        assert_eq!(m.cores[0].get_ar(3), 0);
        if combined {
            assert_eq!(*fragments.lock().unwrap(), [(IRAM + 2, 1), (IRAM, 1), (IRAM + 2, 1), (IRAM, 1), (IRAM + 2, 1)]);
        } else { assert!(fragments.lock().unwrap().is_empty()); }
    }
}

#[test]
fn ordinary_trap_observers_keep_block_execution_and_post_trap_state() {
    use esp_soc::observe::{Ctx, Observer, Wants};
    use std::sync::{Arc, Mutex};
    struct Traps { seen: Arc<Mutex<Vec<(u32, u32)>>>, combined: bool }
    impl Observer<esp32s3::S3> for Traps {
        fn name(&self) -> &'static str { "ordinary-traps" }
        fn wants(&self) -> Wants { if self.combined { Wants::TRAP | Wants::BLOCK } else { Wants::TRAP } }
        fn on_trap(&mut self, _: &Ctx, _: usize, cpu: &xtensa_lx7::Cpu, pc: u32, _: &emu_core::Trap) {
            self.seen.lock().unwrap().push((pc, cpu.epc[1]));
        }
    }
    for until in [false, true] {
        for combined in [false, true] {
            let mut m = machine();
            // Three addi.n a3,a3,-1 instructions, then quou a2,a4,a3 traps.
            park(&mut m, 0, IRAM, &[0x0b, 0x33, 0x0b, 0x33, 0x0b, 0x33, 0x30, 0x24, 0xc2]);
            m.cores[0].set_ar(3, 3);
            m.cores[0].set_ar(4, 12);
            m.dbg.stop_after_exceptions = 1;
            let seen = Arc::new(Mutex::new(Vec::new()));
            m.add_observer(Box::new(Traps { seen: seen.clone(), combined }));
            if until { m.run_until_cycle(64); } else { m.run(64); }
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].1, IRAM + 6);
            assert!((IRAM..IRAM + 6).contains(&seen[0].0), "trap follows earlier instructions in the same run: {seen:?}");
            assert_eq!(m.cores[0].blocks.observed, combined, "ordinary TRAP alone permits retained loops");
        }
    }
}

#[test]
fn quiet_display_publication_remains_live_during_continuous_changes() {
    use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
    struct Display(Arc<AtomicU64>);
    impl esp_soc::board::BoardModel for Display {
        fn name(&self) -> &'static str { "continuous-display" }
        fn display_version(&self) -> u64 { self.0.load(Ordering::Relaxed) }
        fn display_quiet_push(&self) -> bool { true }
        fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
            let version = self.display_version();
            Some((1, 1, vec![version as u16], version))
        }
    }
    const PUSH: u64 = 240_000_000 / 50;   // one page-push interval at the default display_push_hz
    let mut m = machine();
    let version = Arc::new(AtomicU64::new(0));
    m.bus.board = Box::new(Display(version.clone()));
    m.cores[0].waiting = true;
    m.cores[0].ps = 0;
    let web = esp_soc::web::WebServer::queued();
    m.web = Some(web.clone());
    for push in 1..=6 {
        version.store(push, Ordering::Relaxed);
        m.run_until_cycle(push * PUSH);
        let frames: Vec<_> = web.take_outbox().into_iter().filter(|(kind, data)| *kind == 2 && data[0] == 1).collect();
        if push % 2 == 0 {
            assert_eq!(frames, [(2, vec![1, 1, 0, 1, 0, push as u8, 0])]);
        } else { assert!(frames.is_empty()); }
    }
    version.store(7, Ordering::Relaxed);
    m.run_until_cycle(7 * PUSH);
    web.take_outbox();
    m.run_until_cycle(8 * PUSH); // a quiet interval publishes the pending version
    assert!(web.take_outbox().iter().any(|(kind, data)| *kind == 2 && data == &[1, 1, 0, 1, 0, 7, 0]));
}

#[test]
fn queued_display_output_does_not_build_an_unused_socket_snapshot() {
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    struct Display(Arc<AtomicUsize>);
    impl esp_soc::board::BoardModel for Display {
        fn name(&self) -> &'static str { "counted-display" }
        fn display_version(&self) -> u64 { 1 }
        fn display(&self) -> Option<(u32, u32, Vec<u16>, u64)> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some((1, 1, vec![0x1234], 1))
        }
    }
    let mut m = machine();
    let calls = Arc::new(AtomicUsize::new(0));
    m.bus.board = Box::new(Display(calls.clone()));
    m.cores[0].waiting = true;
    m.cores[0].ps = 0;
    let web = esp_soc::web::WebServer::queued();
    m.web = Some(web.clone());
    m.run_until_cycle(4_800_000);
    assert_eq!(calls.load(Ordering::Relaxed), 1, "only the live frame needs serialization");
    assert!(web.take_outbox().iter().any(|(kind, data)| *kind == 2 && data == &[1, 1, 0, 1, 0, 0x34, 0x12]));
    m.run_until_cycle(9_600_000);
    assert_eq!(calls.load(Ordering::Relaxed), 1, "unchanged queued display needs no snapshot");
}

/// Core 1 sits in reset until SYSTEM_CORE_1_CONTROL_0 releases it, then runs from the reset vector.
#[test]
fn core1_runs_when_released() {
    let mut m = machine();
    park(&mut m, 0, IRAM, &SPIN);
    esp_soc::SocBus::load_bytes(&mut m.bus, RESET, &SPIN).unwrap();
    m.max_cycles = 64 * 100;
    m.run(u64::MAX);
    assert_eq!(m.cores[1].insn_count(), 0, "held in reset");
    m.bus.write32(0x600c_0000, 0b010).unwrap();                // clock on, not stalled, not in reset
    m.max_cycles = 64 * 300;
    m.run(u64::MAX);
    assert!(m.cores[1].insn_count() > 0, "core 1 ran");
    assert_eq!(m.cores[1].pc(), RESET);
    assert_eq!(m.cores[1].prid, 0xABAB);
}

#[test]
fn browser_offchip_blocks_are_single_core_scheduler_transactions() {
    let mut m = machine64();
    park(&mut m, 0, IRAM, &SPIN);
    assert_eq!(m.browser_external_block_budget(0), None);
    assert_eq!(m.browser_external_block_budget(1), None);
    assert_eq!(m.browser_external_block_budget(63), None);
    assert_eq!(m.browser_external_block_budget(64), Some(64));
    assert_eq!(m.browser_external_block_budget(128), Some(64));
    assert!(m.finish_browser_external_quantum().is_none());
    assert_eq!(m.bus.cycles, 64);

    m.bus.write32(0x600c_0000, 0b010).unwrap();
    assert_eq!(m.browser_external_block_budget(7), None, "core 1 is running");
}

#[test]
fn browser_offchip_finish_honors_halt_and_drains_console() {
    for halt in [false, true] {
        let mut m = machine64();
        park(&mut m, 0, IRAM, &SPIN);
        m.max_cycles = if halt { 64 } else { 128 };
        m.bus.periph.usb.tx_out.extend_from_slice(b"finish-output");
        assert_eq!(m.browser_external_block_budget(64), Some(64));
        let stop = m.finish_browser_external_quantum();
        assert_eq!(matches!(stop, Some(Stop::Halted)), halt);
        assert!(m.bus.periph.usb.tx_out.is_empty());
        assert_eq!(m.console.all, b"finish-output");
    }
}

/// A chip reset re-creates the digital peripherals but keeps the efuses, the straps, the RTC
/// domain and the captured audio, and publishes the cause where the ROM reads it.
#[test]
fn reboot_keeps_what_silicon_keeps() {
    let mut m = machine();
    m.bus.periph.efuse.ram.write(0x44, 0xdead_beef);
    m.bus.periph.gpio.strap = 0x7;
    m.bus.periph.rtc.ram.write(0x120, 0x1234);
    m.bus.periph.rtc.slow_ticks = 999;
    m.bus.periph.i2s0.pcm = vec![1, 2, 3]; m.bus.periph.i2s0.frames_out = 3;
    m.bus.periph.uart[0].tx_out = b"gone".to_vec();
    m.bus.periph.systimer.conf = 0xffff;
    m.bus.periph.spi0.jedec[2] = 0x18; m.bus.periph.spi1.jedec[2] = 0x18;   // a 16 MB flash chip
    m.cores[0].pc = IRAM;
    m.bus.periph.rtc.reset_cause = esp_periph::RST_SW_CPU;
    m.bus.periph.usb.connected = false;
    let cause = m.reboot();
    assert_eq!(cause, esp_periph::RST_SW_CPU);
    let p = &m.bus.periph;
    assert_eq!(p.efuse.ram.read(0x44), 0xdead_beef); assert_eq!(p.gpio.strap, 0x7);
    assert_eq!(p.rtc.ram.read(0x120), 0x1234); assert_eq!(p.rtc.slow_ticks, 999);
    assert_eq!(p.rtc.ram.read(0x38), cause | (cause << 6));
    assert_eq!(p.rtc.reset_cause, cause, "reset_cause() reports the cause, not POWERON");
    assert!(!p.usb.connected, "an unplugged USB cable stays unplugged");
    assert_eq!(p.i2s0.pcm, vec![1, 2, 3]);
    assert_eq!((p.spi0.jedec[2], p.spi1.jedec[2]), (0x18, 0x18), "the flash chip keeps its capacity, or IDF finds it smaller than the image header");
    assert!(p.uart[0].tx_out.is_empty() && p.systimer.conf == 0x4600_0000, "digital peripherals are fresh");
    assert_eq!(m.cores[0].pc(), RESET); assert_eq!(m.reboots, 1);
    assert!(!m.dump_regs().contains("core1:"), "core 1 is back in reset");
}

/// The PSRAM array size sets MR2's density (ESP-IDF sizes the octal PSRAM from
/// it alone), in the bus constructor and in `set_psram_size`; 2 MB (quad PSRAM) has no octal code
/// and keeps the reset value.
#[test]
fn psram_size_sets_the_mr2_density() {
    let mut m = machine();
    assert_eq!(m.bus.periph.spi1.psram_mr[2], 0x8b, "the default 2 MB array: MR2 unchanged");
    esp_soc::SocBus::set_psram_size(&mut m.bus, 16 << 20).unwrap();
    assert_eq!((m.bus.psram.len(), m.bus.periph.spi0.psram_mr[2], m.bus.periph.spi1.psram_mr[2]), (16 << 20, 0x8d, 0x8d));
    esp_soc::SocBus::set_psram_size(&mut m.bus, 8 << 20).unwrap();
    assert_eq!(m.bus.periph.spi1.psram_mr[2], 0x8b, "64 Mbit again");
    let bus = esp32s3::bus::SocBus::new(8 << 20, 32 << 20, [0; 6]);
    assert_eq!(bus.periph.spi1.psram_mr[2], 0x8f, "the constructor's size: 256 Mbit");
}

/// An octal flash module. The JEDEC ID is Macronix octal with the capacity
/// plus 0x20 (also when the size changes afterwards), efuse FLASH_TYPE says octal, and a chip reset
/// keeps the flash's protocol (the ROM resets it) and the PSRAM's MR2 (read-only).
#[test]
fn octal_flash_module_survives_a_reboot() {
    let mut m = machine();
    m.bus.set_flash_octal(true);
    esp_soc::SocBus::set_flash_size(&mut m.bus, 32 << 20);
    esp_soc::SocBus::set_psram_size(&mut m.bus, 16 << 20).unwrap();
    assert_eq!((m.bus.periph.spi0.jedec, m.bus.periph.spi1.jedec), ([0xc2, 0x80, 0x39], [0xc2, 0x80, 0x39]));
    assert_eq!(m.bus.periph.efuse.ram.read(0x3c) & (1 << 9), 1 << 9, "EFUSE_FLASH_TYPE: octal");
    m.bus.periph.spi1.flash_cr2 = 2;
    m.bus.periph.rtc.reset_cause = esp_periph::RST_SW_CPU;
    m.reboot();
    let p = &m.bus.periph;
    assert!(p.spi0.octal_flash && p.spi1.octal_flash);
    assert_eq!(p.spi1.flash_cr2, 2, "still in DTR OPI: the flash is not reset with the chip");
    assert_eq!(p.spi1.jedec, [0xc2, 0x80, 0x39]);
    assert_eq!((p.spi0.psram_mr[2], p.spi1.psram_mr[2]), (0x8d, 0x8d), "the PSRAM keeps its density, or ESP-IDF maps half of it after a restart");
    assert_eq!(p.efuse.ram.read(0x3c) & (1 << 9), 1 << 9);
    m.bus.set_flash_octal(false);
    assert_eq!((m.bus.periph.spi1.jedec, m.bus.periph.efuse.ram.read(0x3c) & (1 << 9)), ([0x20, 0x40, 0x19], 0), "back to quad");
}

/// Host touch keeps its intended board-edge timestamp and is applied at the fast scheduler's next
/// existing bus tick, which is bounded by one instruction quantum.
#[test]
fn host_touch_is_delivered_on_the_next_fast_path_bus_tick() {
    let mut m = machine64();
    park(&mut m, 0, IRAM, &SPIN);
    m.bus.board = Box::new(esp32s3::board::WaveshareAmoled18V2::new());
    m.bus.attach_board_devices();
    m.bus.periph.gpio.write(0x74 + 4 * u32::from(esp32s3::board::PIN_AMOLED_TOUCH_INT), (2 << 7) | (1 << 13));
    m.max_cycles = 64;
    assert!(matches!(m.run(u64::MAX), Stop::Halted));
    let horizon = m.bus.cycles;
    esp_soc::SocBus::observe_gpio(&mut m.bus, true);
    esp_soc::SocBus::touch_input(&mut m.bus, 100, 200, true);
    assert!(esp_soc::SocBus::take_gpio_events(&mut m.bus).is_empty());

    m.max_cycles = horizon + 64;
    assert!(matches!(m.run(u64::MAX), Stop::Halted));

    assert!((horizon + 1..=horizon + 64).contains(&m.bus.cycles));
    assert_eq!(esp_soc::SocBus::take_gpio_events(&mut m.bus),
               [(horizon + 1, esp32s3::board::PIN_AMOLED_TOUCH_INT, false)]);
}

/// Scripts resolve the board's pin names and expand an encoder detent into its quadrature.
#[test]
fn scripts_use_the_board() {
    let mut m = machine();
    m.load_script("1.0 press btn1 50\n2.0 knob cw 2\n3.0 serial hello\n# comment\n4.0 stop\n").unwrap();
    let ev = &m.script.events;
    assert_eq!(ev.len(), 2 + 8 + 1 + 1);
    assert!(ev[0].0 == 240_000_000 && matches!(ev[0].1, ScriptAction::Gpio(17, false)), "{:?}", ev[0]);
    assert_eq!(ev[1].0, 240_000_000 + 12_000_000, "released 50 ms later");
    assert!(matches!(ev[2].1, ScriptAction::Gpio(5, false)), "a CW detent starts with CLK falling");
    assert!(matches!(ev.last().unwrap().1, ScriptAction::Stop));
    assert!(m.load_script("1.0 press nosuchpin").is_err());
    assert!(m.load_script("1.0 frobnicate").is_err());
    m.bus.board = Box::new(NoBoard);
    assert!(m.load_script("1.0 press btn1").is_err(), "no such name on a bare module");
    assert!(m.load_script("1.0 knob cw").is_err(), "no encoder on a bare module");
    assert!(make_board("waveshare-lcd4b").is_some() && make_board("nope").is_none());
}

#[test]
fn browser_touch_reaches_the_amoled_controller_and_gpio() {
    let mut m = machine();
    let board = esp32s3::board::WaveshareAmoled18V2::new();
    let touch = board.touch_state.clone();
    m.bus.board = Box::new(board);
    let web = esp_soc::web::WebServer::queued();
    web.push_incoming(r#"{"t":"touch","x":"123","y":"234","down":"1"}"#.to_string());
    m.web = Some(web);
    park(&mut m, 0, IRAM, &SPIN);
    m.max_cycles = 64;

    assert!(matches!(m.run(u64::MAX), Stop::Halted));
    let state = *touch.lock().expect("AMOLED touch state mutex poisoned");
    assert!(state.down);
    assert_eq!((state.x, state.y), (123, 234));

    m.max_cycles = m.bus.cycles + 256;
    assert!(matches!(m.run(u64::MAX), Stop::Halted));
    assert!(!m.bus.periph.gpio.level(esp32s3::board::PIN_AMOLED_TOUCH_INT));
}

/// Console bytes from every stream go to the backlogs and the aggregate; the mask only chooses
/// what stdout gets, and capture keeps stdout out of it entirely.
#[test]
fn console_capture_and_backlog() {
    let mut m = machine();
    m.console.mask = 2;
    m.bus.periph.usb.tx_out = b"usb\n".to_vec();
    m.bus.periph.uart[0].tx_out = b"uart0\n".to_vec();
    m.bus.periph.uart[2].tx_out = b"uart2\n".to_vec();
    m.drain_console();
    assert_eq!(m.console.all, b"usb\nuart0\nuart2\n");
    assert_eq!(m.console.usb, b"usb\n"); assert_eq!(m.console.uart0, b"uart0\n");
    assert!(m.bus.periph.usb.tx_out.is_empty());
}

/// With an observer that wants every instruction the machine single-steps; the block observer
/// runs on the fast path and sees the same instruction total.
#[test]
fn observers_count_the_same_instructions_either_way() {
    use esp_soc::observers::{BlockProfile, PcHist};
    for slow in [false, true] {
        let mut m = machine();
        park(&mut m, 0, IRAM, &[0x0c, 0x03, 0x1b, 0x33, 0x86, 0xfe, 0xff]);   // movi.n a3,0 ; addi.n a3,a3,1 ; j back to the addi (objdump: 030c 331b fffe86)
        if slow { m.add_observer(Box::new(PcHist::new(4))); } else { m.add_observer(Box::new(BlockProfile::new(4))); }
        m.max_cycles = 64 * 50;
        m.run(u64::MAX);
        let r = m.reports();
        assert!(r.contains(&format!("of {} instructions", m.insns())), "{}", r);
    }
}

#[test]
fn block_observers_keep_working_with_instruction_observers() {
    use esp_soc::observers::{BlockProfile, Coverage, PcHist};
    for until in [false, true] {
        let mut m = machine64();
        park(&mut m, 0, IRAM, &[0x0c, 0x03, 0x1b, 0x33, 0x86, 0xfe, 0xff]);
        m.add_observer(Box::new(BlockProfile::new(4)));
        m.add_observer(Box::new(Coverage::new(None)));
        m.add_observer(Box::new(PcHist::new(4)));
        if until { m.run_until_cycle(128); } else { m.run(128); }
        let report = m.reports();
        assert!(report.contains("[profile-blocks] top 4 functions of 128 instructions"), "{report}");
        assert!(report.contains("[coverage] 3 block starts"), "{report}");
    }
}

#[test]
fn breakpoints_stop_at_first_fetch_and_at_a_sleeping_pc() {
    use esp_soc::observers::Breakpoints;
    for asleep in [false, true] {
        let mut m = machine();
        park(&mut m, 0, IRAM, &WAITI_LOOP);
        m.cores[0].waiting = asleep;
        m.add_observer(Box::new(Breakpoints { pcs: vec![IRAM] }));
        assert!(matches!(m.run(64), Stop::Breakpoint(IRAM)));
        assert_eq!(m.cores[0].insn_count(), 0);
        assert_eq!(m.bus.cycles, 0);
    }
}

#[test]
fn queued_web_input_is_ordered_and_does_not_advance_guest_time() {
    let mut m = machine();
    let board = esp32s3::board::WaveshareAmoled18V2::new();
    let touch = board.touch_state.clone();
    m.bus.board = Box::new(board);
    let web = esp_soc::web::WebServer::queued();
    m.web = Some(web.clone());
    for (x, down) in [(10, 1), (20, 1), (30, 0)] {
        web.push_incoming(format!(r#"{{"t":"touch","x":"{x}","y":"40","down":"{down}"}}"#));
    }
    let cycles = m.bus.cycles;
    let insns = m.insns();
    assert!(matches!(m.run(0), Stop::MaxInsns));
    let state = *touch.lock().unwrap();
    assert_eq!((state.x, state.y, state.down), (30, 40, true));
    assert!(state.release_pending, "controller retains an unread press until the guest reads it");
    assert_eq!(m.bus.cycles, cycles);
    assert_eq!(m.insns(), insns);
    assert!(web.poll_incoming().is_empty());
    assert!(web.take_outbox().is_empty(), "input must not force a display publication");

    web.push_incoming(r#"{"t":"touch","x":"50","y":"60","down":"1"}"#.into());
    assert!(matches!(m.run_until_cycle(cycles), esp_soc::RunUntil::Reached));
    let state = *touch.lock().unwrap();
    assert_eq!((state.x, state.y, state.down), (50, 60, true));
    assert!(!state.release_pending, "the next press clears the earlier queued release");
    assert_eq!(m.bus.cycles, cycles);
    assert_eq!(m.insns(), insns);
}


#[test]
fn knob_input_preserves_pending_scripts_at_the_current_horizon() {
    let mut m = machine();
    park(&mut m, 0, IRAM, &SPIN);
    let web = esp_soc::web::WebServer::queued();
    m.web = Some(web.clone());
    m.load_script("0 serial first\n").unwrap();
    web.push_incoming(r#"{"t":"knob","d":"1"}"#.into());
    assert!(matches!(m.run(0), Stop::MaxInsns));
    assert_eq!(m.script.pos, 0);
    assert!(matches!(&m.script.events[0].1, ScriptAction::Serial(s) if s == "first\n"));
    m.max_cycles = 64;
    m.run(u64::MAX);
    assert_eq!(m.script.pos, 1, "the time-zero script executes once");

    let horizon = m.bus.cycles;
    m.script.events.insert(m.script.pos, (horizon, ScriptAction::Serial("second".into())));
    web.push_incoming(r#"{"t":"knob","d":"-1"}"#.into());
    assert!(matches!(m.run(0), Stop::MaxInsns));
    assert_eq!(m.script.pos, 1, "neither skip pending events nor replay consumed ones");
    assert!(matches!(&m.script.events[1].1, ScriptAction::Serial(s) if s == "second"));
    m.max_cycles = horizon + 64;
    m.run(u64::MAX);
    assert_eq!(m.script.pos, 2);
}


#[test]
fn due_script_stop_precedes_execution_and_observes_edits_between_runs() {
    for until in [false, true] {
        let mut m = machine64();
        park(&mut m, 0, IRAM, &SPIN);
        m.script.log = false;
        // A future action must stay pending when the first run completes.
        m.script.events = vec![(128, ScriptAction::Serial("later".into()))];
        if until { m.run_until_cycle(64); } else { m.run(64); }
        assert_eq!(m.bus.cycles, 64);
        assert_eq!(m.script.pos, 0);
        let insns = m.insns();
        // Public host edits must take effect at the next entry boundary, even though the
        // previous check found only a future event. Both due actions precede execution.
        m.script.events.insert(0, (64, ScriptAction::Stop));
        m.script.events.insert(0, (64, ScriptAction::Serial("now".into())));
        if until {
            assert!(matches!(m.run_until_cycle(128), esp_soc::RunUntil::Stop(Stop::Halted)));
        } else {
            assert!(matches!(m.run(64), Stop::Halted));
        }
        assert_eq!(m.bus.cycles, 64);
        assert_eq!(m.insns(), insns);
        assert_eq!(m.script.pos, 2);
        assert_eq!(m.bus.periph.usb.rx.iter().copied().collect::<Vec<_>>(), b"now");
    }
}

/// The Light Grid's chain is column-serpentine on the glass: down the left column, up the middle,
/// down the right. Measured one LED at a time on the board with the `grid-selftest` firmware,
/// motherboard upright. Port 11 holds the same module turned 90° clockwise — the slot decides —
/// so it needs its own map. A frame written in chain order therefore scatters: one chain column
/// per voice puts every voice's lowest step in the right column of the glass.
#[test]
fn light_grid_reports_the_glass_not_the_chain() {
    use esp32s3::board::{GRID_PHYSICAL_PORT7, GRID_PHYSICAL_PORT11};
    use esp_soc::devices::Ws2812Chain;
    let px = |r: u8, g: u8, b: u8| -> Vec<bool> { let v = (g as u32) << 16 | (r as u32) << 8 | b as u32; (0..24).map(|i| v >> (23 - i) & 1 != 0).collect() };
    // chain 0, 1 and 3 were read off the board; 5 and 8 follow from the serpentine, pinned too
    for (chain, cell, name) in [(0usize, (0usize, 0usize), "top-left"), (1, (1, 0), "middle-left"), (3, (2, 1), "bottom-centre"), (5, (0, 1), "top-centre"), (8, (2, 2), "bottom-right")] {
        let mut bits = Vec::new();
        for i in 0..9 { if i == chain { bits.extend(px(51, 0, 0)); } else { bits.extend(px(0, 0, 0)); } }
        let mut g = Ws2812Chain::mapped(&GRID_PHYSICAL_PORT7); g.from_bits(&bits);
        let lit: Vec<usize> = (0..9).filter(|&k| g.leds[k] != [0, 0, 0]).collect();
        assert_eq!(lit, vec![cell.0 * 3 + cell.1], "chain {} lights the {} cell", chain, name);
    }
    // the SID low-on-all frame: chain row 2 = blue, magenta, orange (one per voice column)
    let (blue, magenta, orange) = ((0, 38, 51), (51, 0, 34), (51, 26, 0));
    let mut bits = Vec::new();
    for _ in 0..6 { bits.extend(px(0, 0, 0)); }
    for (r, g, b) in [blue, magenta, orange] { bits.extend(px(r, g, b)); }
    let mut g = Ws2812Chain::mapped(&GRID_PHYSICAL_PORT7); g.from_bits(&bits);
    let cell = |row: usize, col: usize| g.leds[row * 3 + col];
    assert_eq!(cell(0, 2), [0, 38, 51], "voice 0's lowest step (chain 6) is top-right");
    assert_eq!(cell(1, 2), [51, 0, 34], "voice 1's lowest step (chain 7) is middle-right");
    assert_eq!(cell(2, 2), [51, 26, 0], "voice 2's lowest step (chain 8) is bottom-right");
    assert_eq!((0..9).filter(|&k| g.leds[k] != [0, 0, 0]).count(), 3);
    let mut chain = Ws2812Chain::new(9); chain.from_bits(&bits); assert_eq!(chain.leds[6], [0, 38, 51], "an unmapped chain keeps chain order");

    // Port 11 is the same module in a right-column slot: 90 degrees clockwise, so cell
    // (row, col) of port 7 is (col, 2 - row) there. Measured with both grids lit at once.
    for (chain_i, cell7, cell11) in [(0usize, (0usize, 0usize), (0usize, 2usize)), (1, (1, 0), (0, 1)), (3, (2, 1), (1, 0))] {
        let mut b = Vec::new();
        for i in 0..9 { if i == chain_i { b.extend(px(51, 0, 0)); } else { b.extend(px(0, 0, 0)); } }
        let mut g7 = Ws2812Chain::mapped(&GRID_PHYSICAL_PORT7); g7.from_bits(&b);
        let mut g11 = Ws2812Chain::mapped(&GRID_PHYSICAL_PORT11); g11.from_bits(&b);
        assert_eq!(g7.leds[cell7.0 * 3 + cell7.1], [51, 0, 0], "chain {} on port 7", chain_i);
        assert_eq!(g11.leds[cell11.0 * 3 + cell11.1], [51, 0, 0], "chain {} on port 11", chain_i);
    }
}

#[test]
fn zero_display_rate_is_safe_and_virtual_rounds_reuse_the_interval() {
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    struct DisplayRate(Arc<AtomicUsize>);
    impl esp_soc::board::BoardModel for DisplayRate {
        fn name(&self) -> &'static str { "zero-rate" }
        fn display_push_hz(&self) -> u64 { self.0.fetch_add(1, Ordering::Relaxed); 0 }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut m = machine();
    m.bus.board = Box::new(DisplayRate(calls.clone()));
    m.web = Some(esp_soc::web::WebServer::queued());
    m.vq_max = 1024;
    for core in &mut m.cores { core.set_jit(false); }
    park(&mut m, 0, IRAM, &SPIN);
    assert!(matches!(m.run(8192), Stop::MaxInsns));
    assert_eq!(calls.load(Ordering::Relaxed), 1, "one configuration read, no per-round division");
}

#[test]
fn unmatched_breakpoints_preserve_idle_timeline() {
    use esp_soc::observers::Breakpoints;
    for until in [false, true] {
        let mut results = Vec::new();
        for observed in [false, true] {
            let mut m = machine64();
            park(&mut m, 0, IRAM, &WAITI_LOOP);
            m.script.events = vec![(71, ScriptAction::Stop)];
            if observed { m.add_observer(Box::new(Breakpoints { pcs: vec![IRAM + 100] })); }
            if until { m.run_until_cycle(1000); } else { m.run(1000); }
            results.push((m.bus.cycles, m.insns(), m.cores[0].pc, m.cores[0].ccount));
        }
        assert_eq!(results[0], results[1], "until={until}");
        assert_eq!(results[1].0, 71);
    }
}

#[test]
fn block_profile_does_not_count_idle_single_steps() {
    use esp_soc::observers::{BlockProfile, PcHist};
    let mut m = machine();
    park(&mut m, 0, IRAM, &WAITI_LOOP);
    m.add_observer(Box::new(PcHist::new(4)));
    m.add_observer(Box::new(BlockProfile::new(4)));
    m.run(128);
    assert!(m.reports().contains("[profile-blocks] top 4 functions of 1 instructions"));
}

#[test]
fn reset_partial_round_delivers_mmio_and_round_observations() {
    use esp_soc::observe::{Ctx, Observer, Wants};
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Events { writes: Vec<u32>, rounds: Vec<u64> }
    struct Watch(Arc<Mutex<Events>>);
    impl Observer<esp32s3::S3> for Watch {
        fn name(&self) -> &'static str { "reset-observer" }
        fn wants(&self) -> Wants { Wants::MMIO | Wants::ROUND }
        fn on_mmio(&mut self, _: &Ctx, _: u32, addr: u32, _: u32, write: bool) {
            if write { self.0.lock().unwrap().writes.push(addr); }
        }
        fn on_round(&mut self, cx: &Ctx) { self.0.lock().unwrap().rounds.push(cx.cycles); }
    }
    for until in [false, true] {
        let mut m = machine();
        park(&mut m, 0, IRAM, &[0x22, 0x63, 0]); // s32i a2,a3,0
        m.cores[0].set_ar(2, 1 << 31);
        m.cores[0].set_ar(3, 0x6000_8000);
        let events = Arc::new(Mutex::new(Events::default()));
        m.add_observer(Box::new(Watch(events.clone())));
        if until { assert!(matches!(m.run_until_cycle(64), esp_soc::RunUntil::Stop(Stop::SwReset))); }
        else { assert!(matches!(m.run(64), Stop::SwReset)); }
        let events = events.lock().unwrap();
        assert_eq!(events.writes, [0x6000_8000]);
        assert_eq!(events.rounds, [1]);
    }
}

#[test]
fn zero_display_rate_is_safe() {
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    struct DisplayRate(Arc<AtomicUsize>);
    impl esp_soc::board::BoardModel for DisplayRate {
        fn name(&self) -> &'static str { "zero-rate" }
        fn display_push_hz(&self) -> u64 { self.0.fetch_add(1, Ordering::Relaxed); 0 }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut m = machine();
    m.bus.board = Box::new(DisplayRate(calls.clone()));
    m.web = Some(esp_soc::web::WebServer::queued());
    m.vq_max = 1024;
    for core in &mut m.cores { core.set_jit(false); }
    park(&mut m, 0, IRAM, &SPIN);
    assert!(matches!(m.run(8192), Stop::MaxInsns));
    assert!(calls.load(Ordering::Relaxed) > 0);
}

/// A waveform after `waituart0` plays from when it is applied: the wait moves its event, and the
/// first sample has to move with it instead of starting at the cycle the script was parsed for.
#[test]
fn waveform_after_waituart0_starts_at_the_shifted_time() {
    let mut m = machine();
    park(&mut m, 0, IRAM, &SPIN);
    m.script.log = false;
    let ramp: Vec<f32> = (0..1000).map(|i| i as f32 / 1000.0).collect();
    let wave = esp_periph::AnalogSource::Wave { samples: std::sync::Arc::new(ramp), rate_hz: 100_000.0, start_cycles: 0 };
    m.script.events = vec![(0, ScriptAction::WaitUart0("never printed".into(), 1000, 0)), (0, ScriptAction::Analog(1, wave))];
    m.max_cycles = 2_000_000;   // past the 1 ms recheck that ends the timed-out wait
    m.run(u64::MAX);
    assert_eq!(m.script.pos, 2, "the wait timed out and the waveform was applied");
    let applied = m.script.events[1].0;
    assert!(applied > 0, "the wait moved the waveform's event");
    assert_eq!(m.bus.periph.rtc.analog.volts(1, applied), 0.0, "the first sample plays at the moved start");
    assert_eq!(m.bus.periph.rtc.analog.volts(1, applied + 2400), 0.001, "and advances one sample per 10 µs (2400 cycles) from there");
}

#[test]
fn reboot_preserves_camera_host_settings_and_stream_but_resets_capture() {
    use std::sync::{Arc, Mutex};
    use esp_soc::picture::Picture;
    let mut m = machine();
    m.bus.board = make_board("waveshare-cam").unwrap();
    m.bus.attach_board_devices();
    m.bus.board.set_camera_picture(Picture { w: 2, h: 1, rgb: vec![1, 2, 3, 4, 5, 6] }).unwrap();
    let input = Arc::new(Mutex::new(None));
    m.camera_input = Some(input.clone());
    let period = esp32s3::periph::CPU_HZ / 25;
    let mut debug = esp_soc::DebugFlags::default();
    debug.parse("lcd_cam,wifi,spi,mmio");
    m.set_debug(&debug);
    m.bus.periph.misc.log_unknown = true;
    let cam = &mut m.bus.periph.lcd_cam;
    cam.frame_cycles = period;
    cam.acc = 17;
    cam.frames = 9;
    cam.dropped = 3;
    cam.int_raw = 7;
    cam.set_cam_frame(Some(Arc::new(vec![10, 11])));
    let preview = m.bus.board.camera_preview(2, 1).unwrap();
    m.reboot();
    let cam = &mut m.bus.periph.lcd_cam;
    assert_eq!(cam.frame_cycles, period);
    assert!(cam.lcd_log);
    assert_eq!((cam.acc, cam.frames, cam.dropped, cam.int_raw), (0, 0, 0, 0));
    assert!(cam.cam_frame().is_none());
    assert!(!cam.running());
    assert!(!cam.cam_clock_active());
    assert!(!cam.frame_due(period - 1));
    assert!(cam.frame_due(1));
    assert_eq!(m.bus.board.camera_preview(2, 1).unwrap(), preview);
    assert!(Arc::ptr_eq(m.camera_input.as_ref().unwrap(), &input));
    *input.lock().unwrap() = Some(Picture { w: 2, h: 1, rgb: vec![7, 8, 9, 10, 11, 12] });
    assert!(matches!(m.run(0), Stop::MaxInsns));
    assert_eq!(m.bus.board.camera_preview(2, 1).unwrap(), [7, 8, 9, 10, 11, 12]);
    assert!(input.lock().unwrap().is_none());
    assert!(m.bus.periph.wifi.log);
    assert!(m.bus.periph.spi1.log);
    assert!(m.bus.periph.spi2.log);
    assert!(m.bus.periph.misc.log_all);
    assert!(m.bus.periph.misc.log_unknown);
}

#[test]
fn busy_runs_honor_cycle_ceiling_with_virtual_and_batched_rounds() {
    for peer in [false, true] {
        for batching in [false, true] {
            let mut m = machine();
            m.quantum = 256;
            for core in &mut m.cores { core.set_jit(false); }
            m.vq_max = if batching { 16 } else { 1 };
            m.bb_max = if batching { 16 } else { 1 };
            park(&mut m, 0, IRAM, &SPIN);
            if peer {
                esp_soc::SocBus::load_bytes(&mut m.bus, RESET, &SPIN).unwrap();
                // IDF v5.5.5 components/soc/esp32s3/register/soc/system_reg.h:15-33:
                // CORE_1_CONTROL_0 enables its clock and clears reset/stall.
                m.bus.write32(0x600c_0000, 0b010).unwrap();
            }
            for cycles in [1, 255, 256, 257, 1025, 1] {
                m.max_cycles = m.bus.cycles + cycles;
                assert!(matches!(m.run(u64::MAX), Stop::Halted));
                assert_eq!(m.bus.cycles, m.max_cycles);
            }
            if batching && esp_soc::SocBus::can_defer(&m.bus) {
                assert!(if peer { m.bb_stats[0] } else { m.vq_stats[0] } > 0);
            }
        }
    }
}
