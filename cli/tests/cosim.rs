//! The P4 and its C6 co-processor in lock-step (`esp32sim::cosim`). The first tests need no
//! firmware: both cores spin in a loop while the test drives the P4's SDMMC host and GPIO through
//! its bus, as ESP-Hosted's host does, and watches where the C6 stands. The last ones run the
//! fetched ESP-Hosted pair from the mask ROMs.
#[path = "../../tests/common.rs"]
mod common;
use common::*;
use emu_core::{Bus, Core};
use esp32sim::cosim::{c6_cycle_of_ns, p4_cycle_of_ns, CoSim, Config, CoprocSpec};
use esp_soc::SocBus;

const SDMMC: u32 = 0x5008_3000;
const GPIO: u32 = 0x500e_0000;
const J_SELF: [u8; 4] = [0x6f, 0, 0, 0];

/// Both chips spinning: each in its RAM, and each ROM's reset vector a loop too.
fn spinning(cfg: Config) -> CoSim {
    let mut p4 = esp32p4::machine([0x30, 0xed, 0xa0, 0, 0, 1], 1 << 20);
    p4.bus.load_bytes(0x4ff0_0000, &J_SELF).unwrap();
    p4.bus.load_bytes(0x4fc0_0000, &J_SELF).unwrap();
    p4.cores[0].pc = 0x4ff0_0000;
    let mut c6 = esp32c6::machine([0x40, 0x4c, 0xca, 0, 0, 1], 1 << 20);
    c6.bus.load_bytes(0x4080_0000, &J_SELF).unwrap();
    c6.bus.load_bytes(0x4000_0000, &J_SELF).unwrap();
    c6.cores[0].pc = 0x4080_0000;
    CoSim::new(p4, c6, cfg)
}

/// Slot 1's clock on pin 18 through the GPIO matrix (IO MUX function 1, SD_CARD_CCLK_2 out),
/// four lines, as `sdmmc_host_init_slot` leaves it.
fn route_slot1(cs: &mut CoSim) {
    cs.p4.bus.write32(0x500e_1004 + 4 * 18, 1 << 12).unwrap();
    cs.p4.bus.write32(GPIO + 0x558 + 4 * 18, 0).unwrap();
    cs.p4.bus.write32(SDMMC + 0x18, 1 << 1).unwrap();
}

/// A command on slot 1 with a short response; returns RESP0, or None on a response timeout.
fn command(cs: &mut CoSim, index: u32, arg: u32) -> Option<u32> {
    cs.p4.bus.write32(SDMMC + 0x44, u32::MAX).unwrap();
    cs.p4.bus.write32(SDMMC + 0x28, arg).unwrap();
    cs.p4.bus.write32(SDMMC + 0x2c, 1 << 31 | 1 << 16 | 1 << 6 | index).unwrap();
    let rint = cs.p4.bus.read32(SDMMC + 0x44).unwrap();
    assert_ne!(rint & 1 << 2, 0, "CMD_DONE");
    (rint & 1 << 8 == 0).then(|| cs.p4.bus.read32(SDMMC + 0x30).unwrap())
}

/// Run the P4 alone to `ns`, as it runs inside a slice before the C6 catches up.
fn p4_alone(cs: &mut CoSim, ns: u64) {
    cs.p4.max_cycles = p4_cycle_of_ns(ns);
    cs.p4.run(u64::MAX);
    cs.p4.max_cycles = u64::MAX;
}

#[test]
fn slices_end_with_both_chips_at_the_same_instant() {
    let mut cs = spinning(Config::default());
    cs.run_until(1_000_000).unwrap();
    assert_eq!(cs.p4.bus.cycles(), p4_cycle_of_ns(1_000_000));
    assert_eq!(cs.c6.borrow().now_cycles(), c6_cycle_of_ns(1_000_000));
    assert_eq!((cs.p4.bus.cycles(), cs.c6.borrow().m.bus.cycles()), (360_000, 160_000), "1 ms at 360 and 160 MHz");
    assert_eq!(cs.summary.slices, 100, "10 µs slices");
}

/// A command is where the two meet: the C6 runs to the command's time before it answers, and is
/// never ahead of the P4.
#[test]
fn the_c6_meets_each_command_at_its_time() {
    let mut cs = spinning(Config::default());
    cs.run_until(100_000).unwrap();
    route_slot1(&mut cs);
    p4_alone(&mut cs, 157_000);
    assert_eq!(cs.c6.borrow().now_cycles(), c6_cycle_of_ns(100_000), "the C6 waits behind the P4");
    let r4 = command(&mut cs, 5, 0).expect("the slave answers CMD5");
    assert_eq!(r4, 0xa0ff_8000, "R4: ready, two functions, 2.7-3.6 V");
    let p4_ns = cs.p4.bus.periph.now_ns();
    assert_eq!(cs.c6.borrow().now_cycles(), c6_cycle_of_ns(p4_ns), "the C6 at the command's nanosecond");
    assert!(p4_ns >= 157_000);
    // the rest of IDF's sequence: CMD5 with the OCR, CMD3, CMD7, then CMD52 (sdmmc_card_init)
    command(&mut cs, 5, 0x30_0000).unwrap();
    let rca = command(&mut cs, 3, 0).unwrap() & 0xffff_0000;
    assert_eq!(rca, 0x1_0000);
    command(&mut cs, 7, rca).unwrap();
    assert_eq!(command(&mut cs, 52, 0x08 << 9), Some(0x1000 | esp32c6::sdio::CARD_CAP as u32), "CMD52 reads CCCR 0x08");
    assert_eq!(command(&mut cs, 8, 0x1aa), None, "an I/O-only card does not answer CMD8");
}

/// A P4 chip reset starts its device clock again but not the shared one: the C6 still meets each
/// command at the command's time.
#[test]
fn the_c6_meets_each_command_at_its_time_after_a_p4_reset() {
    let mut cs = spinning(Config::default());
    cs.run_until(100_000).unwrap();
    cs.p4.bus.request_reset(esp_periph::RST_SW_SYS);
    cs.run_until(110_000).unwrap();
    assert_eq!(cs.summary.p4_resets, 1);
    route_slot1(&mut cs);
    p4_alone(&mut cs, 157_000);
    command(&mut cs, 5, 0).expect("the slave answers CMD5");
    let p4_ns = cs.p4.bus.periph.now_ns();
    assert!(p4_ns >= 157_000, "the P4's SD bus counts from power-on, not from the reset: {p4_ns} ns");
    assert_eq!(cs.c6.borrow().now_cycles(), c6_cycle_of_ns(p4_ns), "the C6 at the command's nanosecond");
    assert_eq!(cs.c6.borrow().late_commands, 0);
}

/// The slave's interrupt reaches the P4 on DAT1 by the end of the slice: D1's input level goes low
/// and the slot's RINTSTS SDIO bit is set on that edge; the host clearing it releases D1.
#[test]
fn dat1_carries_the_slave_interrupt_to_the_p4() {
    let mut cs = spinning(Config::default());
    route_slot1(&mut cs);
    for (cmd, arg) in [(5, 0), (5, 0x30_0000), (3, 0)] { command(&mut cs, cmd, arg); }
    command(&mut cs, 7, 0x1_0000).unwrap();
    command(&mut cs, 52, 1 << 31 | 0x04 << 9 | 0x03).unwrap();          // IENM and IEN1
    assert!(cs.p4.bus.periph.gpio.g.level(15), "D1 high: nothing pending");
    {
        let mut c = cs.c6.borrow_mut();
        c.m.bus.write32(0x6001_80dc, 0xff).unwrap();                  // SLCHOST FUNC1_INT_ENA
        c.m.bus.write32(0x6001_705c, 1 << 3).unwrap();                // SLC INTVEC_TOHOST
    }
    cs.run_until(10_000).unwrap();
    assert!(!cs.p4.bus.periph.gpio.g.level(15), "D1 low at the slice end");
    assert_ne!(cs.p4.bus.read32(SDMMC + 0x44).unwrap() & 1 << 17, 0, "RINTSTS SDIO interrupt, slot 1");
    // the host's interrupt clear: CMD52 to INT_CLR, and DAT1 is sampled after the command
    command(&mut cs, 52, 1 << 31 | 1 << 28 | 0xd4 << 9 | 1 << 3).unwrap();
    assert!(cs.p4.bus.periph.gpio.g.level(15), "D1 released after the clearing command");
}

/// EN on the P4's GPIO 54: driven low it holds the C6 where it is; driven high again it powers the
/// C6 up from its ROM, which then runs on the shared clock from that instant.
#[test]
fn en_holds_the_c6_and_powers_it_up_again() {
    let mut cs = spinning(Config::default());
    cs.run_until(50_000).unwrap();
    cs.p4.bus.write32(GPIO + 0x18, 1 << 22).unwrap();                 // OUT1_W1TC: GPIO 54 low
    cs.p4.bus.write32(GPIO + 0x30, 1 << 22).unwrap();                 // ENABLE1_W1TS
    cs.run_until(60_000).unwrap();
    let held = cs.c6.borrow().m.bus.cycles();
    assert!(cs.c6.borrow().held);
    assert_eq!(held, c6_cycle_of_ns(60_000), "EN seen at the slice end");
    cs.run_until(200_000).unwrap();
    assert_eq!(cs.c6.borrow().m.bus.cycles(), held, "held in reset");
    route_slot1(&mut cs);
    assert_eq!(command(&mut cs, 5, 0), None, "a chip in reset does not answer");
    cs.p4.bus.write32(GPIO + 0x14, 1 << 22).unwrap();                 // OUT1_W1TS: high
    cs.run_until(210_000).unwrap();
    {
        let c = cs.c6.borrow();
        assert!(!c.held && c.power_ups == 1);
        assert_eq!(c.m.cores[0].pc(), 0x4000_0000, "from the ROM's reset vector");
        assert_eq!(c.m.bus.reset_cause(), esp_periph::RST_POWERON);
        assert_eq!(c.now_cycles(), c6_cycle_of_ns(210_000));
    }
    cs.p4.bus.write32(GPIO + 0x34, 1 << 22).unwrap();                 // ENABLE1_W1TC: the pull-up
    cs.run_until(300_000).unwrap();
    assert_eq!(cs.summary.en_changes, 2, "an undriven EN stays high");
    let inverted = spinning(Config { en_inverted: true, ..Config::default() });
    assert!(!inverted.c6.borrow().held, "undriven: not in reset either way");
}

/// The card selected, function 1 on, four lines: what ESP-Hosted's card init leaves.
fn card_ready(cs: &mut CoSim) {
    route_slot1(cs);
    for (cmd, arg) in [(5, 0), (5, 0x30_0000), (3, 0)] { command(cs, cmd, arg); }
    command(cs, 7, 0x1_0000).unwrap();
    command(cs, 52, 1 << 31 | 0x07 << 9 | 0x02).unwrap();              // CCCR bus width: 4 bits
    command(cs, 52, 1 << 31 | 0x02 << 9 | 0x02).unwrap();              // I/O enable: function 1
}

const C6_DESC: u32 = 0x4081_0000;
const C6_BUF: u32 = 0x4082_0000;

/// A packet of `len` bytes queued on the C6's send link as the driver queues one (4 KiB
/// descriptors, EOF on the last, PKT_LEN after the START); returns the descriptors.
fn slave_sends(cs: &mut CoSim, len: u32) -> Vec<u32> {
    let mut c = cs.c6.borrow_mut();
    let b = &mut c.m.bus;
    let n = len.div_ceil(4096);
    let descs: Vec<u32> = (0..n).map(|i| C6_DESC + 16 * i).collect();
    for (i, &d) in descs.iter().enumerate() {
        let size = (len - 4096 * i as u32).min(4096);
        let last = i as u32 == n - 1;
        for (o, v) in [(0, size | size << 14 | (last as u32) << 30 | 1 << 31), (4, C6_BUF + 4096 * i as u32), (8, if last { 0 } else { d + 16 })] { b.write32(d + o, v).unwrap(); }
    }
    for i in 0..len { b.write8(C6_BUF + i, (i % 251) as u8).unwrap(); }
    b.write32(0x6001_7000, 1 << 6).unwrap();                            // SLCCONF0: RX_AUTO_WRBACK
    b.write32(0x6001_7040, descs[0]).unwrap();                          // SLC0RX_LINK_ADDR
    b.write32(0x6001_703c, 1 << 29).unwrap();                           // SLC0RX_LINK: START
    b.write32(0x6001_70f4, 1 << 20 | len).unwrap();                     // SLC0_LEN_CONF: LEN_WR
    descs
}

const P4_DESC: u32 = 0x4ff2_0000;
const P4_BUF: u32 = 0x4ff3_0000;

/// The P4's IDMAC armed for `bytes` in 512-byte blocks into `n` chained 4 KiB descriptors, the
/// first `owned` of them the hardware's (`sdmmc_host_do_transaction`'s four-descriptor ring).
fn idmac_read(cs: &mut CoSim, bytes: u32, n: u32, owned: u32) {
    let b = &mut cs.p4.bus;
    for i in 0..n {
        let d = P4_DESC + 16 * i;
        let last = (i + 1) * 4096 >= bytes;
        let own = if i < owned { 1 << 31 } else { 0 };
        let size = (bytes - 4096 * i).min(4096);
        for (o, v) in [(0, own | 1 << 4 | if i == 0 { 1 << 3 } else { 0 } | if last { 1 << 2 } else { 0 }), (4, size), (8, P4_BUF + 4096 * i), (12, d + 16)] { b.write32(d + o, v).unwrap(); }
    }
    b.write32(SDMMC, 1 << 4 | 1 << 25).unwrap();                       // CTRL: interrupts, internal DMA
    b.write32(SDMMC + 0x80, 1 << 7 | 1 << 1).unwrap();                 // BMOD: DE, fixed burst
    b.write32(SDMMC + 0x20, bytes).unwrap();                           // BYTCNT
    b.write32(SDMMC + 0x1c, 512).unwrap();                             // BLKSIZ
    b.write32(SDMMC + 0x88, P4_DESC).unwrap();                         // DBADDR
    b.write32(SDMMC + 0x44, u32::MAX).unwrap();
    b.write32(SDMMC + 0x8c, 0x337).unwrap();
}

/// CMD53 block read from function 1's FIFO for a packet of `len` bytes, as ESP-Hosted reads one.
fn cmd53_read(cs: &mut CoSim, len: u32) {
    let blocks = len.div_ceil(512);
    cs.p4.bus.write32(SDMMC + 0x28, 1 << 28 | 1 << 27 | 1 << 26 | (0x1_f800 - len) << 9 | blocks).unwrap();
    cs.p4.bus.write32(SDMMC + 0x2c, 1 << 31 | 1 << 16 | 1 << 9 | 1 << 6 | 53).unwrap();
}

/// A read longer than the IDMAC's four descriptors: the host's DMA suspends on the fifth (DU)
/// and resumes on a poll demand after the P4 has run on, so the last bytes cross the bus with
/// the C6 behind. The slave hands back the descriptors those bytes came from when the C6
/// reaches that time, not before.
#[test]
fn a_resumed_read_hands_the_slave_descriptors_back_at_its_time() {
    let mut cs = spinning(Config::default());
    card_ready(&mut cs);
    let len = 18_944;                                                   // 37 blocks, 4.6 descriptors
    let descs = slave_sends(&mut cs, len);
    idmac_read(&mut cs, len, 5, 4);
    cmd53_read(&mut cs, len);
    assert_ne!(cs.p4.bus.read32(SDMMC + 0x8c).unwrap() & 1 << 4, 0, "IDSTS DU: suspended on the fifth descriptor");
    let first = cs.c6.borrow().m.bus.cycles();
    let word = |cs: &CoSim, a: u32| cs.c6.borrow_mut().m.bus.read32(a).unwrap();
    assert_eq!(word(&cs, descs[3]) >> 31, 0, "the first 16 KiB went back at once");
    assert_eq!(word(&cs, descs[4]) >> 31, 1);
    let later = cs.p4.bus.periph.now_ns() + 50_000;
    p4_alone(&mut cs, later);
    let resume_ns = cs.p4.bus.periph.now_ns();
    let d = P4_DESC + 16 * 4;
    let w0 = cs.p4.bus.read32(d).unwrap();
    cs.p4.bus.write32(d, w0 | 1 << 31).unwrap();
    cs.p4.bus.write32(SDMMC + 0x84, 1).unwrap();                       // PLDMND
    assert_ne!(cs.p4.bus.read32(SDMMC + 0x44).unwrap() & 1 << 3, 0, "DATA_OVER");
    assert_eq!(cs.c6.borrow().m.bus.cycles(), first, "the C6 is behind the resume");
    assert_eq!(word(&cs, descs[4]) >> 31, 1, "its last descriptor waits for the C6 to get there");
    assert_eq!(word(&cs, 0x6001_7004) & 1 << 17, 0, "no RX_EOF yet");
    cs.run_until(resume_ns.div_ceil(10_000) * 10_000).unwrap();
    assert_eq!(word(&cs, descs[4]) >> 31, 0, "handed back");
    assert_ne!(word(&cs, 0x6001_7004) & 1 << 17, 0, "RX_EOF");
    let got: Vec<u8> = (0..len).map(|i| cs.p4.bus.read8(P4_BUF + i).unwrap()).collect();
    assert!(got.iter().enumerate().all(|(i, &b)| b == (i % 251) as u8), "the packet, in order");
    assert_eq!(cs.c6.borrow().m.bus.periph.optional.sdio.stats.to_host, len as u64);
}

/// With the SD bus timed, a CMD53's data phase ends after its wire time: the slave's descriptors
/// go back and RX_DONE rises when the C6 reaches that moment, not at the command.
#[test]
fn a_timed_read_hands_the_slave_descriptors_back_after_its_wire_time() {
    let mut cs = spinning(Config::default());
    card_ready(&mut cs);
    cs.p4.bus.periph.sdmmc.timed = true;
    let descs = slave_sends(&mut cs, 1024);
    idmac_read(&mut cs, 1024, 1, 1);
    let t0 = cs.p4.bus.periph.now_ns();
    cmd53_read(&mut cs, 1024);
    let word = |cs: &CoSim, a: u32| cs.c6.borrow_mut().m.bus.read32(a).unwrap();
    assert_eq!(word(&cs, descs[0]) >> 31, 1, "at the command the data is still on the wire");
    let due = cs.c6.borrow().m.bus.sdio_next_due().expect("a hand-back queued");
    assert!(due > cs.c6.borrow().m.bus.cycles());
    cs.run_until(t0 + 100_000).unwrap();
    assert_eq!(word(&cs, descs[0]) >> 31, 0, "handed back once the C6 passed the data phase's end");
    assert_ne!(word(&cs, 0x6001_7004) & 1 << 16, 0, "RX_DONE");
    assert!(cs.c6.borrow().m.bus.sdio_next_due().is_none());
    assert_ne!(cs.p4.bus.read32(SDMMC + 0x44).unwrap() & 1 << 3, 0, "DATA_OVER on the P4 by then");
}

/// EN low for less than the C6's t_RST (50 µs) does not reset it: the chip runs on from where it
/// was, as when a GPIO is made an output a moment before its level is written.
#[test]
fn a_short_en_pulse_does_not_reset_the_c6() {
    let mut cs = spinning(Config::default());
    cs.run_until(50_000).unwrap();
    cs.p4.bus.write32(GPIO + 0x18, 1 << 22).unwrap();                 // OUT1_W1TC: GPIO 54 low
    cs.p4.bus.write32(GPIO + 0x30, 1 << 22).unwrap();                 // ENABLE1_W1TS
    cs.run_until(70_000).unwrap();
    assert!(cs.c6.borrow().held);
    cs.p4.bus.write32(GPIO + 0x14, 1 << 22).unwrap();                 // high again, 20 µs later
    cs.run_until(100_000).unwrap();
    let c = cs.c6.borrow();
    assert_eq!((c.power_ups, c.en_glitches, c.held), (0, 1, false), "ridden out");
    assert_eq!(c.now_cycles(), c6_cycle_of_ns(100_000), "and caught up with the shared clock");
    assert_eq!(c.m.cores[0].pc(), 0x4080_0000, "still in its loop, not back in the ROM");
}

#[test]
fn the_faults_parse() {
    use esp32sim::cosim::{Fault, FaultKind, When};
    assert_eq!(Fault::parse("read-crc@40").unwrap(), Fault { kind: FaultKind::ReadCrc, when: When::Nth { n: 40, byte: None } });
    assert_eq!(Fault::parse("read-crc@40:1024").unwrap().when, When::Nth { n: 40, byte: Some(1024) });
    assert_eq!(Fault::parse("wedge@4s").unwrap(), Fault { kind: FaultKind::Wedge, when: When::At { ns: 4_000_000_000, len: 0 } });
    assert_eq!(Fault::parse("token-stall@3.5s+200ms").unwrap().when, When::At { ns: 3_500_000_000, len: 200_000_000 });
    assert_eq!(Fault::parse("spurious-irq@1500us+20us").unwrap().when, When::At { ns: 1_500_000, len: 20_000 });
    for bad in ["read-crc@0", "read-crc@4s", "wedge@40", "melt@1s", "wedge", "token-stall@1s+x"] { assert!(Fault::parse(bad).is_err(), "{bad}"); }
    let s = CoprocSpec::parse("c6:app=a.bin,fault=dead-bus@2s,fault=resp-crc@7").unwrap();
    assert_eq!(s.faults.iter().map(|f| f.kind).collect::<Vec<_>>(), [FaultKind::DeadBus, FaultKind::RespCrc]);
}

#[test]
fn the_coproc_spec_parses_its_keys() {
    let s = CoprocSpec::parse("c6:bootloader=b.bin,ptable=p.bin,app=a.bin,otadata=o.bin@0xe000,stub=bb_init=0,stub=0x4200=1,en=!53,slice-us=5,flash-mb=8,mac=02:00:00:00:00:01").unwrap();
    assert_eq!((s.bootloader.as_deref(), s.ptable.as_deref(), s.app.as_deref()), (Some("b.bin"), Some("p.bin"), Some("a.bin")));
    assert_eq!(s.otadata, Some(("o.bin".into(), 0xe000)));
    assert_eq!(s.stubs, ["bb_init=0", "0x4200=1"]);
    assert_eq!((s.en_pin, s.en_inverted, s.slice_ns, s.flash_mb, s.mac), (Some(53), true, Some(5000), Some(8), Some([2, 0, 0, 0, 0, 1])));
    assert_eq!(CoprocSpec::parse("c6:app=a.bin,otadata=o.bin").unwrap().otadata, Some(("o.bin".into(), 0xd000)));
    for bad in ["s3:app=a", "c6:bootloader=b", "c6:app=a,colour=red", "c6:app=a,en=64", "c6:app=a,slice-us=0", "c6:app"] {
        assert!(CoprocSpec::parse(bad).is_err(), "{bad}");
    }
}

// ------------------------------------------------------------------ with firmware
const FW: &str = "web/wasm/fw/public";

/// The fetched ESP-Hosted slave on a C6 booted from its mask ROM, the virtual access point on
/// its radio, no NAT behind it, and the network's clock fixed so SNTP answers the same every run.
fn hosted_slave() -> esp32c6::Machine {
    let read = |name: &str| std::fs::read(root().join(FW).join(name)).unwrap_or_else(|e| panic!("{FW}/{name}: {e} (tools/fetch-p4-firmware.sh fetches it)"));
    let mut c6 = esp32c6::machine([0x40, 0x4c, 0xca, 0x00, 0x11, 0x22], 4 << 20);
    c6.bus.set_flash_size(4 << 20);
    c6.load_rom(&std::fs::read(rom("esp32c6_rev0")).unwrap()).unwrap();
    for (off, f) in [(0, "c6-hosted-bootloader.bin"), (0x8000, "c6-hosted-ptable.bin"), (0xd000, "c6-hosted-otadata.bin"), (0x10000, "c6-hosted-slave.bin")] { c6.write_flash(off, &read(f)).unwrap(); }
    // bb_init, the PHY's baseband calibration (riscv32-esp-elf-nm on network_adapter.elf)
    c6.stubs.insert(0x420b_ac5c, 0);
    c6.bus.attach_wifi(esp_soc::wifi::ApConfig::parse("ssid=esp32sim,psk=esp32sim-pass,epoch=1767225600").unwrap(), None).unwrap();
    c6.boot_rom();
    c6
}

/// The fetched ESP-Hosted pair, booted from the mask ROMs: IDF's station example on the P4,
/// ESP-Hosted's slave on the C6.
fn hosted_pair(cfg: Config) -> CoSim {
    let read = |name: &str| std::fs::read(root().join(FW).join(name)).unwrap_or_else(|e| panic!("{FW}/{name}: {e} (tools/fetch-p4-firmware.sh fetches it)"));
    let mut p4 = esp32p4::machine([0x30, 0xed, 0xa0, 0x00, 0x11, 0x22], 2 << 20);
    p4.bus.set_flash_size(2 << 20);
    p4.load_rom(&std::fs::read(rom("esp32p4_rev0")).unwrap()).unwrap();
    for (off, f) in [(0x2000, "p4-hosted-bootloader.bin"), (0x8000, "p4-hosted-ptable.bin"), (0x10000, "p4-hosted-station.bin")] { p4.write_flash(off, &read(f)).unwrap(); }
    p4.boot_rom();
    CoSim::new(p4, hosted_slave(), cfg)
}

fn transcript(cs: &mut CoSim) -> String {
    cs.lines.drain(..).map(|(c6, l)| format!("{}{l}\n", if c6 { "[c6] " } else { "" })).collect()
}

/// Determinism of the lock-step: the same inputs give the same run to the instruction, however
/// the run is split into calls, and every console line in the same order.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn the_pair_runs_the_same_whichever_way_it_is_driven() {
    let mut whole = hosted_pair(Config::default());
    whole.run_until(2_000_000_000).unwrap();
    let mut pieces = hosted_pair(Config::default());
    let mut t = 0;
    while t < 2_000_000_000 { t += 7_919_000; pieces.run_until(t.min(2_000_000_000)).unwrap(); }
    let (a, b) = (transcript(&mut whole), transcript(&mut pieces));
    assert!(a.contains("H_SDIO_DRV: Card init success") && a.contains("transport: Identified slave [esp32c6]"), "the transport came up:\n{a}");
    assert_eq!(a, b);
    assert_eq!((whole.p4.insns(), whole.c6.borrow().m.insns()), (pieces.p4.insns(), pieces.c6.borrow().m.insns()));
    assert_eq!(whole.p4.interrupts, pieces.p4.interrupts);
}

// ------------------------------------------------------------------ faults, on the hosted pair

/// The hosted pair with `faults`, run to `secs`: the P4's console lines, the C6's.
fn faulted(faults: &[&str], secs: f64) -> (CoSim, Vec<String>, Vec<String>) {
    let mut cs = hosted_pair(Config::default());
    cs.c6.borrow_mut().faults.plan = faults.iter().map(|f| esp32sim::cosim::Fault::parse(f).unwrap()).collect();
    cs.run_until((secs * 1e9) as u64).unwrap();
    let (mut p4, mut c6) = (Vec::new(), Vec::new());
    for (from_c6, l) in cs.lines.drain(..) { if from_c6 { c6.push(l) } else { p4.push(l) } }
    assert_eq!(cs.c6.borrow().faults.fired.len(), faults.len(), "every fault fired");
    (cs, p4, c6)
}
fn has(lines: &[String], s: &str) -> bool { lines.iter().any(|l| l.contains(s)) }
fn after(lines: &[String], first: &str, then: &str) -> bool { lines.iter().skip_while(|l| !l.contains(first)).any(|l| l.contains(then)) }

/// The C6 resets itself while the link is busy: its slave forgets the card, the RPC in flight is
/// never answered, and ESP-Hosted's host gives up on it after 5 s, restarts the P4 and resets the
/// C6 on GPIO 54 to start again.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn a_c6_reset_mid_traffic_ends_in_a_host_restart() {
    let mut cs = hosted_pair(Config::default());
    cs.run_until(2_030_000_000).unwrap();
    cs.c6.borrow_mut().m.bus.request_reset(esp_periph::RST_SW_SYS);
    cs.run_until(10_000_000_000).unwrap();
    let (p4, c6): (Vec<_>, Vec<_>) = cs.lines.drain(..).partition(|l| !l.0);
    let (p4, c6): (Vec<String>, Vec<String>) = (p4.into_iter().map(|l| l.1).collect(), c6.into_iter().map(|l| l.1).collect());
    assert!(has(&c6, "rst:0x3 (LP_SW_HPSYS)"), "the C6 came back through its ROM");
    assert!(has(&p4, "Response not received for [0x118](Req_WifiStart)"), "the RPC in flight is lost");
    assert!(after(&p4, "Rebooting...", "Reset slave using GPIO[54]"), "the host restarts and resets the C6");
    let c = cs.c6.borrow();
    assert_eq!((c.resets, c.power_ups, cs.summary.p4_resets, cs.summary.en_changes), (1, 2, 1, 4));
    assert!(c6.iter().filter(|l| l.contains("Sta mode connected")).count() == 1 && after(&c6, "rst:0x1 (POWERON)", "Sta mode connected"), "joined again after the reset");
}

/// A CMD53 FIFO read fails its CRC after the slave gave the data: the packet (an RPC response) is
/// lost, the RPC times out after 5 s, and the station example aborts and restarts the P4.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_read_crc_loses_the_packet_and_the_rpc_times_out() {
    let (cs, p4, _) = faulted(&["read-crc@5"], 8.0);
    assert!(has(&p4, "sdmmc_send_cmd returned 0x109") && has(&p4, "Failed to read data"));
    assert!(has(&p4, "Response not received for [0x11c](Req_WifiSetConfig)"));
    assert_eq!(cs.summary.p4_resets, 1, "the station example aborts");
}

/// The same with a data timeout (ESP_ERR_TIMEOUT, 0x107).
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_read_timeout_loses_the_packet() {
    let (cs, p4, _) = faulted(&["read-timeout@5"], 8.0);
    assert!(has(&p4, "sdmmc_send_cmd returned 0x107") && has(&p4, "Failed to read data"));
    assert_eq!(cs.summary.p4_resets, 1);
}

/// The combined register read fails: ESP-Hosted posts a transport failure and restarts the host
/// at once (H_TRANSPORT_RESTART_ON_FAILURE); the restarted pair takes its lease again.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_reg_read_crc_restarts_the_host() {
    let (cs, p4, _) = faulted(&["reg-read-crc@10"], 6.0);
    assert!(has(&p4, "failed to read registers"));
    assert!(after(&p4, "failed to read registers", "wifi station: got ip:10.0.2.15"), "up again after the restart");
    let c = cs.c6.borrow();
    assert_eq!((cs.summary.p4_resets, c.power_ups), (1, 2), "the C6 reset again by GPIO 54, once");
    // The restarted P4 makes GPIO 54 an output a few instructions before it writes the level
    // ("A restarted P4 glitches EN"); whether a slice end catches that depends on where the slices
    // fall (the ROM's console after the restart moves it), here not. Each one caught would be two
    // more EN changes, which the C6 rides out; `a_short_en_pulse_does_not_reset_the_c6` covers
    // that path.
    assert_eq!((c.en_glitches, cs.summary.en_changes), (0, 4), "no glitch caught at these slices");
}

/// A write's CRC status reports an error (the card drops the block), or never comes (the card
/// took it): ESP-Hosted retries the write and the lease comes on time.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_write_errors_are_retried() {
    for (fault, code) in [("write-crc@5", "0x109"), ("write-nostatus@5", "0x107")] {
        let (cs, p4, _) = faulted(&[fault], 4.0);
        assert!(has(&p4, &format!("sdmmc_send_cmd returned {code}")) && has(&p4, "Failed to send data"), "{fault}");
        assert!(has(&p4, "I (3771) wifi station: got ip:10.0.2.15"), "{fault}: the lease on time");
        assert_eq!(cs.summary.p4_resets, 0);
    }
}

/// One command lost on the way (RTO) or answered with a bad CRC after the slave acted on it: one
/// error, and the link goes on.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_a_lost_or_garbled_command_is_survived() {
    for (fault, code) in [("cmd-timeout@100", "0x107"), ("resp-crc@100", "0x109")] {
        let (cs, p4, _) = faulted(&[fault], 4.0);
        assert!(has(&p4, &format!("sdmmc_send_cmd returned {code}")), "{fault}");
        assert!(has(&p4, "wifi station: got ip:10.0.2.15"), "{fault}");
        assert_eq!(cs.summary.p4_resets, 0);
    }
}

/// The C6's CPU stops while its SDIO hardware answers on, or the bus goes dead, or one DAT1 edge
/// is lost: ESP-Hosted's receive task waits for an interrupt without a timeout, so the host stalls
/// without a word, no error and no restart, and the lease never comes.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_a_wedged_slave_a_dead_bus_or_a_lost_edge_stall_the_host_silently() {
    for fault in ["wedge@2.5s", "dead-bus@2.5s", "drop-irq@2.05s"] {
        let (mut cs, p4, _) = faulted(&[fault], 8.0);
        assert!(!has(&p4, "got ip") && !p4.iter().any(|l| l.starts_with("E (")), "{fault}: silence");
        assert_eq!(p4.last().map(String::as_str), Some("I (2091) H_API: esp_wifi_remote_connect"), "{fault}");
        assert_eq!(cs.summary.p4_resets, 0, "{fault}");
        if fault.starts_with("wedge") { assert_eq!(cs.c6.borrow().now_cycles(), c6_cycle_of_ns(2_500_000_000), "the C6 stopped where it was"); }
        if fault.starts_with("drop") {
            assert!(!cs.p4.bus.periph.gpio.g.level(15), "D1 low: the slave interrupts");
            assert_eq!(cs.p4.bus.read32(SDMMC + 0x44).unwrap() & 1 << 17, 0, "but RINTSTS never saw the edge");
        }
    }
}

/// D1 low for 20 ms with nothing pending: the host reads the registers, finds nothing, and the
/// lease comes as without it.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_a_spurious_interrupt_is_harmless() {
    let (cs, p4, _) = faulted(&["spurious-irq@2.5s+20ms"], 4.0);
    assert!(has(&p4, "I (3771) wifi station: got ip:10.0.2.15") && !p4.iter().any(|l| l.starts_with("E (")));
    assert_eq!(cs.summary.p4_resets, 0);
}

/// TOKEN1 shows no free receive buffer from before the host's first write until 2.5 s: the host
/// polls it with growing intervals (sdio_is_write_buffer_available) and its traffic waits, so the
/// lease comes half a second late.
#[test]
#[ignore = "needs the ESP32-P4 and ESP32-C6 mask ROM ELFs"]
fn fault_exhausted_slave_buffers_hold_the_host_back() {
    let (cs, p4, _) = faulted(&["token-stall@1s+1.5s"], 5.0);
    assert!(has(&p4, "I (4271) wifi station: got ip:10.0.2.15"), "half a second late");
    assert!(cs.c6.borrow().m.bus.periph.optional.sdio.stats.commands > 150, "TOKEN1 polled");
}
