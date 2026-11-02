use super::*;
use std::sync::{Arc, Mutex};

const SPI2: u32 = 0x6002_4000;
const GDMA: u32 = 0x6003_f000;
const FIRST_DESC: u32 = 0x3fc9_0100;

struct ProbeBoard {
    events: Arc<Mutex<Vec<String>>>,
}

impl crate::board::BoardModel for ProbeBoard {
    fn name(&self) -> &'static str { "probe" }
    fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
        self.events.lock().expect("probe mutex poisoned").push(format!("gpio:{changes:?}"));
    }
    fn spi_transfer(&mut self, host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        self.events.lock().expect("probe mutex poisoned").push(format!("spi:{host}:{tx:02x?}:{rx_len}"));
        (0..rx_len).map(|i| 0x50 + i as u8).collect()
    }
}

struct FixedDeadlineBoard {
    deadline: u64,
}

impl crate::board::BoardModel for FixedDeadlineBoard {
    fn name(&self) -> &'static str { "fixed-deadline-test" }
    fn next_deadline(&self) -> Option<u64> { Some(self.deadline) }
}

const M2M_SRC: u32 = 0x3fc9_2000;
const M2M_DST: u32 = 0x3fc9_6000;

fn m2m_pattern(i: u32, seed: u32) -> u8 { ((i + seed) % 251) as u8 }

fn m2m_desc(bus: &mut SocBus, at: u32, dw0: u32, buf: u32, next: u32) {
    bus.write32(at, dw0).unwrap();
    bus.write32(at + 4, buf).unwrap();
    bus.write32(at + 8, next).unwrap();
}

/// Channel 0 the way `esp_async_memcpy` sets up a copy: MEM_TRANS_EN on IN, AUTO_WRBACK on
/// OUT when asked, SUC_EOF enabled, RX started before TX.
fn m2m_start(bus: &mut SocBus, in0: u32, out0: u32, auto_wrback: bool) {
    bus.write32(GDMA, 1 << 4).unwrap();                                 // IN_CONF0: MEM_TRANS_EN
    bus.write32(GDMA + 0x60, if auto_wrback { 1 << 2 } else { 0 }).unwrap();   // OUT_CONF0: AUTO_WRBACK
    bus.write32(GDMA + 0x10, 1 << 1).unwrap();                          // IN_INT_ENA: SUC_EOF
    bus.write32(GDMA + 0x20, (1 << 22) | (in0 & 0xf_ffff)).unwrap();    // IN_LINK start
    bus.write32(GDMA + 0x80, (1 << 21) | (out0 & 0xf_ffff)).unwrap();   // OUT_LINK start
}

/// One scheduling round; ticks are deferred up to the next timer deadline, so flush them.
fn m2m_round(bus: &mut SocBus) {
    emu_core::Bus::tick(bus, 1);
    bus.flush_ticks();
}

/// The source split over two OUT descriptors (the second with EOF), the destination over
/// IN descriptors of 4095 bytes: after one round the bytes are across, the IN descriptors
/// carry length/owner/SUC_EOF, the EOF address is the last IN descriptor, both sides report
/// their interrupts and stop.
#[test]
fn gdma_copies_memory_to_memory_when_mem_trans_en_is_set() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    let (out0, out1, in0, in1) = (0x3fc9_0100u32, 0x3fc9_0110u32, 0x3fc9_0200u32, 0x3fc9_0210u32);
    let n = 5000u32;
    for i in 0..n { bus.write8(M2M_SRC + i, m2m_pattern(i, 0)).unwrap(); }
    for i in 0..n + 4 { bus.write8(M2M_DST + i, 0xee).unwrap(); }       // the word after the copy must stay untouched
    m2m_desc(&mut bus, out0, (1 << 31) | (3000 << 12) | 3000, M2M_SRC, out1);
    m2m_desc(&mut bus, out1, (1 << 31) | (1 << 30) | (2000 << 12) | 2000, M2M_SRC + 3000, 0);
    m2m_desc(&mut bus, in0, (1 << 31) | 4095, M2M_DST, in1);
    m2m_desc(&mut bus, in1, (1 << 31) | 4095, M2M_DST + 4095, 0);
    m2m_start(&mut bus, in0, out0, false);
    assert!(bus.periph.gdma.inp[0].running && bus.periph.gdma.out[0].running);
    m2m_round(&mut bus);
    for i in 0..n { assert_eq!(bus.read8(M2M_DST + i).unwrap(), m2m_pattern(i, 0), "byte {i}"); }
    assert_eq!(bus.read8(M2M_DST + n).unwrap(), 0xee);
    let (d0, d1) = (bus.read32(in0).unwrap(), bus.read32(in1).unwrap());
    assert_eq!(((d0 >> 12) & 0xfff, d0 >> 30), (4095, 0), "first IN descriptor: full, owner cpu, no eof");
    assert_eq!(((d1 >> 12) & 0xfff, d1 >> 30), (905, 1), "second IN descriptor: the rest, owner cpu, suc_eof");
    assert_eq!(bus.read32(out0).unwrap() >> 31, 1, "AUTO_WRBACK off: the OUT descriptors keep their owner");
    let (r, o) = (bus.periph.gdma.inp[0], bus.periph.gdma.out[0]);
    assert_eq!((r.eof_desc, r.int_raw & 0b11, r.running), (in1, 0b11, false));
    assert_eq!((o.eof_desc, o.int_raw & 0b1011, o.running), (out1, 0b1011, false));
    assert!(r.irq(), "IN_SUC_EOF is the interrupt the async memcpy driver waits for");
    assert_eq!(bus.read32(GDMA + 0x28).unwrap(), in1);                  // IN_SUC_EOF_DES_ADDR
}

/// Bulk reads (PIE 128-bit loads) return exactly what per-byte reads do, or decline: swept over
/// SRAM, its instruction-bus alias, 256-byte and entry edges, unmapped flash and peripherals.
#[test]
fn read_bulk_matches_per_byte_reads_or_declines() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    for i in 0..0x2_0000u32 { bus.write8(0x3fc8_8000 + i, (i.wrapping_mul(0x9e37_79b9) >> 24) as u8).unwrap(); }
    let mut served = 0;
    for base in [0x3fc8_8000u32, 0x4037_8000, 0x3fc9_f000, 0x4200_0000, 0x3c00_0000, 0x6000_8000] {
        for k in 0..0x200u32 {
            let addr = base + k * 0x100 - 8 * (k % 3);
            let mut out = [0u8; 16];
            if emu_core::Bus::read_bulk(&mut bus, addr, &mut out) {
                served += 1;
                for (i, b) in out.iter().enumerate() { assert_eq!(bus.read8(addr + i as u32).ok(), Some(*b), "{addr:#x}+{i}"); }
            }
        }
    }
    assert!(served > 0x200, "SRAM and its alias are served in bulk ({served})");
    assert!(!emu_core::Bus::read_bulk(&mut bus, 0x6000_8000, &mut [0u8; 16]), "peripherals never are");
}

/// Two copies back to back with AUTO_WRBACK on, as IDF always configures it: the second
/// start after the first completed must land too (the pocket-tank freeze was the second copy).
#[test]
fn gdma_m2m_back_to_back_copies_with_auto_wrback() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    let (out0, in0, n) = (0x3fc9_0100u32, 0x3fc9_0200u32, 4000u32);
    for (copy, seed) in [(0, 7u32), (1, 101)] {
        for i in 0..n { bus.write8(M2M_SRC + i, m2m_pattern(i, seed)).unwrap(); }
        m2m_desc(&mut bus, out0, (1 << 31) | (1 << 30) | (n << 12) | n, M2M_SRC, 0);
        m2m_desc(&mut bus, in0, (1 << 31) | 4095, M2M_DST, 0);
        bus.write32(GDMA + 0x14, u32::MAX).unwrap();                    // IN_INT_CLR, as the EOF ISR does
        bus.write32(GDMA + 0x74, u32::MAX).unwrap();                    // OUT_INT_CLR
        m2m_start(&mut bus, in0, out0, true);
        m2m_round(&mut bus);
        for i in 0..n { assert_eq!(bus.read8(M2M_DST + i).unwrap(), m2m_pattern(i, seed), "copy {copy} byte {i}"); }
        assert_eq!(bus.read32(out0).unwrap() >> 31, 0, "copy {copy}: AUTO_WRBACK hands the OUT descriptor back");
        let d = bus.read32(in0).unwrap();
        assert_eq!(((d >> 12) & 0xfff, d >> 30), (n, 1), "copy {copy}: IN length and SUC_EOF, owner cpu");
        let (r, o) = (bus.periph.gdma.inp[0], bus.periph.gdma.out[0]);
        assert_eq!((r.int_raw & 0b11, r.running, o.int_raw & 0b1011, o.running), (0b11, false, 0b1011, false), "copy {copy}");
    }
}

/// A ring of zero-length OUT descriptors that stay DMA-owned, AUTO_WRBACK off, never reaches
/// the end of a chain: the walk stops at its step budget with OUT_DSCR_ERR instead of hanging.
#[test]
fn gdma_m2m_ring_of_empty_out_descriptors_stops_at_the_step_budget() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    let (out0, out1, in0) = (0x3fc9_0100u32, 0x3fc9_0110u32, 0x3fc9_0200u32);
    m2m_desc(&mut bus, out0, 1 << 31, M2M_SRC, out1);
    m2m_desc(&mut bus, out1, 1 << 31, M2M_SRC, out0);
    m2m_desc(&mut bus, in0, (1 << 31) | 4095, M2M_DST, 0);
    m2m_start(&mut bus, in0, out0, false);
    m2m_round(&mut bus);
    let (r, o) = (bus.periph.gdma.inp[0], bus.periph.gdma.out[0]);
    assert_eq!((o.int_raw & (1 << 2), o.running), (1 << 2, false), "OUT_DSCR_ERR and the OUT side stops");
    assert_eq!((r.desc, r.int_raw, r.buf_pos), (in0, 0, 0), "nothing reached the IN side");
    assert_eq!(bus.read32(in0).unwrap(), (1 << 31) | 4095);
}

/// OUT parks on a descriptor the CPU still owns, with the IN buffer part-filled. Waiting does
/// not re-dirty interrupts every round; once software hands the descriptor over, the copy
/// resumes where it stopped, including the position inside the IN buffer.
#[test]
fn gdma_m2m_parked_pair_resumes_where_it_stopped_and_stays_quiet_meanwhile() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    let (out0, out1, in0, in1) = (0x3fc9_0100u32, 0x3fc9_0110u32, 0x3fc9_0200u32, 0x3fc9_0210u32);
    let n = 5000u32;
    for i in 0..n { bus.write8(M2M_SRC + i, m2m_pattern(i, 3)).unwrap(); }
    m2m_desc(&mut bus, out0, (1 << 31) | (3000 << 12) | 3000, M2M_SRC, out1);
    m2m_desc(&mut bus, out1, (1 << 30) | (2000 << 12) | 2000, M2M_SRC + 3000, 0);   // CPU-owned for now
    m2m_desc(&mut bus, in0, (1 << 31) | 4095, M2M_DST, in1);
    m2m_desc(&mut bus, in1, (1 << 31) | 4095, M2M_DST + 4095, 0);
    m2m_start(&mut bus, in0, out0, true);
    m2m_round(&mut bus);
    let (r, o) = (bus.periph.gdma.inp[0], bus.periph.gdma.out[0]);
    assert_eq!((o.desc, o.buf_pos, o.running, o.int_raw & 0b111), (out1, 0, true, 0b101), "first descriptor done, parked on the second");
    assert_eq!((r.desc, r.buf_pos, r.int_raw), (in0, 3000, 0), "IN buffer part-filled, not closed");
    let mut dirty_rounds = 0;
    for _ in 0..100 {
        bus.irq_dirty = false;
        m2m_round(&mut bus);
        dirty_rounds += usize::from(bus.irq_dirty);
    }
    assert_eq!(dirty_rounds, 0, "a parked pair does not re-dirty interrupts every round");
    bus.write32(out1, (1 << 31) | (1 << 30) | (2000 << 12) | 2000).unwrap();   // software hands it over
    m2m_round(&mut bus);
    for i in 0..n { assert_eq!(bus.read8(M2M_DST + i).unwrap(), m2m_pattern(i, 3), "byte {i}"); }
    let (d0, d1) = (bus.read32(in0).unwrap(), bus.read32(in1).unwrap());
    assert_eq!(((d0 >> 12) & 0xfff, d0 >> 30), (4095, 0));
    assert_eq!(((d1 >> 12) & 0xfff, d1 >> 30), (905, 1));
    let (r, o) = (bus.periph.gdma.inp[0], bus.periph.gdma.out[0]);
    assert_eq!((r.running, r.eof_desc, o.running, o.eof_desc), (false, in1, false, out1));
}

/// The error paths: an exhausted IN chain, a CPU-owned descriptor on either side, and a
/// fault writing the destination, which raises IN_DSCR_ERR and writes nothing back.
#[test]
fn gdma_m2m_descriptor_errors_and_faults() {
    let (out0, in0) = (0x3fc9_0100u32, 0x3fc9_0200u32);
    let run = |out_dw0: u32, in_dw0: u32, in_buf: u32| {
        let mut bus = SocBus::new(1024, 1024, [0; 6]);
        for i in 0..4095 {
            bus.write8(M2M_SRC + i, m2m_pattern(i, 9)).unwrap();
            bus.write8(M2M_DST + i, 0xee).unwrap();
        }
        m2m_desc(&mut bus, out0, out_dw0, M2M_SRC, 0);
        m2m_desc(&mut bus, in0, in_dw0, in_buf, 0);
        m2m_start(&mut bus, in0, out0, true);
        m2m_round(&mut bus);
        bus
    };
    let full_out = (1u32 << 31) | (1 << 30) | (4095 << 12) | 4095;

    let bus = run(full_out, (1 << 31) | 1000, M2M_DST);                 // the only IN buffer holds 1000 bytes
    let (r, o) = (bus.periph.gdma.inp[0], bus.periph.gdma.out[0]);
    assert_eq!((r.int_raw & 0b1_1011, r.running), (0b1_0001, false), "IN_DONE, then IN_DSCR_EMPTY");
    assert_eq!((o.buf_pos, o.running, o.int_raw), (1000, true, 0), "OUT waits mid-descriptor");

    let mut bus = run(full_out, 4095, M2M_DST);                         // the CPU owns the IN descriptor
    assert_eq!(bus.periph.gdma.inp[0].int_raw & (1 << 3), 1 << 3, "IN_DSCR_ERR");
    assert_eq!((bus.periph.gdma.out[0].buf_pos, bus.read8(M2M_DST).unwrap()), (0, 0xee), "nothing copied");

    let mut bus = run(full_out & !(1 << 31), (1 << 31) | 4095, M2M_DST);   // the CPU owns the OUT descriptor
    assert_eq!(bus.periph.gdma.out[0].int_raw & (1 << 2), 1 << 2, "OUT_DSCR_ERR");
    assert_eq!((bus.periph.gdma.inp[0].buf_pos, bus.read8(M2M_DST).unwrap()), (0, 0xee), "nothing copied");

    let mut bus = run(full_out, (1 << 31) | 4095, DRAM_HIGH);           // the IN buffer is unmapped
    let r = bus.periph.gdma.inp[0];
    assert_eq!((r.int_raw, r.running), (1 << 3, false), "IN_DSCR_ERR alone: no IN_DONE or IN_SUC_EOF");
    assert_eq!(bus.read32(in0).unwrap(), (1 << 31) | 4095, "the IN descriptor is not written back");
}

fn dma_bus() -> SocBus {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.gdma.out[0].peri_sel = 0;
    bus.periph.gdma.out[0].desc = FIRST_DESC;
    bus.periph.gdma.out[0].running = true;
    bus
}

fn start_dma(bus: &mut SocBus, bits: u32) {
    bus.write32(SPI2 + 0x30, 1 << 28).expect("SPI DMA configuration failed");
    bus.write32(SPI2 + 0x10, 1 << 27).expect("SPI user configuration failed");
    bus.write32(SPI2 + 0x1c, bits - 1).expect("SPI data length failed");
    bus.write32(SPI2, 1 << 24).expect("SPI command failed");
}

#[test]
fn spi2_dma_without_a_bound_channel_aborts_and_accepts_cpu_commands() {
    for timing in [false, true] {
        let mut bus = SocBus::new(1024, 1024, [0; 6]);
        bus.spi2_timing = timing;
        assert!(bus.periph.gdma.out_channel_for(0).is_none());
        start_dma(&mut bus, 8);
        bus.tick(256);
        assert_eq!(bus.read32(SPI2).unwrap() & (1 << 24), 0, "unbound DMA must not stay busy");
        assert!(bus.periph.spi2.dma_tx_pending.is_none());
        assert_ne!(bus.periph.spi2.int_raw & (1 << 12), 0);
        assert_eq!(bus.periph.spi2.transfers, 0, "abort must not reach the board");
        let events = Arc::new(Mutex::new(Vec::new()));
        bus.board = Box::new(ProbeBoard { events: events.clone() });
        bus.write32(SPI2 + 0x30, 0).unwrap();
        bus.write32(SPI2 + 0x98, 0xa5).unwrap();
        bus.write32(SPI2, 1 << 24).unwrap();
        assert_eq!(bus.periph.spi2.transfers, 1);
        assert_eq!(&*events.lock().unwrap(), &["spi:2:[a5]:0"]);
    }
}

fn assert_dma_fault_and_recovery(bus: &mut SocBus, expected: DmaDescriptorFault) {
    assert_eq!(bus.spi2_dma_fault, Some(expected));
    assert_eq!(bus.periph.gdma.out[0].int_raw & 0xf, 1 << 2);
    assert!(!bus.periph.gdma.out[0].running);
    assert_ne!(bus.periph.spi2.int_raw & (1 << 12), 0);
    assert_eq!(bus.periph.spi2.transfers, 0);
    assert!(bus.periph.spi2.dma_tx_pending.is_none());
    assert!(!bus.periph.spi2.has_pending_transfer());

    bus.write32(GDMA + 0x74, 1 << 2).expect("GDMA interrupt clear failed");
    bus.write32(SPI2 + 0x38, 1 << 12).expect("SPI interrupt clear failed");
    assert_eq!(bus.periph.gdma.out[0].int_raw & (1 << 2), 0);
    assert_eq!(bus.periph.spi2.int_raw & (1 << 12), 0);

    let events = Arc::new(Mutex::new(Vec::new()));
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    bus.write32(SPI2 + 0x30, 0).expect("CPU mode setup failed");
    bus.write32(SPI2 + 0x1c, 7).expect("CPU data length setup failed");
    bus.write32(SPI2 + 0x98, 0xa5).expect("CPU data setup failed");
    bus.write32(SPI2, 1 << 24).expect("recovery transaction failed");
    assert_eq!(bus.spi2_dma_fault, None);
    assert_eq!(bus.periph.spi2.transfers, 1);
    assert_eq!(&*events.lock().expect("probe mutex poisoned"), &["spi:2:[a5]:0"]);
}

#[test]
fn idf_shaped_read_uses_ms_dlen_and_a_following_cpu_transfer_completes() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    bus.gpio_events = Some(Vec::new());
    bus.periph.gpio.changes.push((12, false));

    bus.write32(SPI2 + 0x10, (1 << 31) | (1 << 28)).expect("SPI setup failed");
    bus.write32(SPI2 + 0x18, (7 << 28) | 0x9f).expect("SPI command phase failed");
    bus.write32(SPI2 + 0x1c, 7).expect("SPI response length failed");
    bus.write32(SPI2 + 0x20, 0x3e).expect("SPI miscellaneous setup failed");
    bus.write32(SPI2, 1 << 24).expect("SPI command failed");

    assert_eq!(bus.periph.spi2.w[0] & 0xff, 0x50);
    assert_ne!(bus.periph.spi2.int_raw & (1 << 12), 0);
    assert_eq!(&*events.lock().expect("probe mutex poisoned"), &["gpio:[(12, false)]", "spi:2:[9f]:1"]);
    assert_eq!(bus.gpio_events.as_deref(), Some(&[(0, 12, false)][..]));

    bus.write32(SPI2 + 0x30, 0).expect("CPU mode setup failed");
    bus.write32(SPI2 + 0x10, 1 << 27).expect("CPU transfer setup failed");
    bus.write32(SPI2 + 0x98, 0xa5).expect("CPU data setup failed");
    bus.write32(SPI2, 1 << 24).expect("second SPI command failed");
    assert_eq!(bus.periph.spi2.transfers, 2);
    assert_eq!(events.lock().expect("probe mutex poisoned").last().map(String::as_str), Some("spi:2:[a5]:0"));
}

#[test]
fn cpu_command_does_not_replace_a_parked_dma_transfer_on_the_bus() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.gdma.out[0].desc = FIRST_DESC; // bound but stopped, eligible for a later RESTART
    bus.board = Box::new(ProbeBoard { events: events.clone() });

    bus.write32(SPI2 + 0x30, 1 << 28).expect("SPI DMA setup failed");
    bus.write32(SPI2 + 0x10, 1 << 27).expect("DMA transfer setup failed");
    bus.write32(SPI2 + 0x1c, 7).expect("SPI data length failed");
    bus.write32(SPI2, 1 << 24).expect("DMA command failed");
    assert_eq!(bus.periph.spi2.dma_tx_pending, Some(8));
    assert_eq!(bus.periph.spi2.transfers, 0);
    assert!(events.lock().expect("probe mutex poisoned").is_empty());

    bus.write32(SPI2 + 0x30, 0).expect("CPU mode setup failed");
    bus.write32(SPI2 + 0x98, 0xa5).expect("CPU data setup failed");
    bus.write32(SPI2, 1 << 24).expect("CPU command failed");

    assert_eq!(bus.periph.spi2.dma_tx_pending, Some(8));
    assert_eq!(bus.periph.spi2.transfers, 0);
    assert!(events.lock().expect("probe mutex poisoned").is_empty());
}

#[test]
fn timed_spi2_dma_keeps_owner_and_interrupt_pending_until_wire_deadline() {
    const DATA: u32 = 0x3fc9_0200;
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = dma_bus();
    bus.spi2_timing = true;
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    bus.write32(DATA, 0x4433_2211).unwrap();
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, DATA).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    bus.periph.gdma.out[0].conf0 = 1 << 2;
    bus.write32(SPI2 + 0x0c, 1 << 12).unwrap(); // 80 MHz / 2
    start_dma(&mut bus, 32);
    let deadline = (32 * crate::periph::CPU_HZ).div_ceil(40_000_000);
    bus.tick(deadline as u32 - 1);
    assert_eq!(bus.periph.spi2.transfers, 0);
    assert_eq!(bus.read32(FIRST_DESC).unwrap() >> 31, 1);
    assert_eq!(bus.periph.spi2.int_raw & (1 << 12), 0);
    assert!(events.lock().unwrap().is_empty());
    bus.tick(1);
    assert_eq!(bus.periph.spi2.transfers, 1);
    assert_eq!(bus.read32(FIRST_DESC).unwrap() >> 31, 0);
    assert_ne!(bus.periph.spi2.int_raw & (1 << 12), 0);
    assert_eq!(&*events.lock().unwrap(), &["spi:2:[11, 22, 33, 44]:0"]);
}

/// mmio-s1: SPI2/GDMA writes mark interrupt inputs dirty exactly when their sources move,
/// including a DMA completion the last write triggers; other devices keep the blanket rule.
#[test]
fn spi2_and_gdma_writes_dirty_interrupts_only_when_their_sources_change() {
    const DATA: u32 = 0x3fc9_0200;
    let mut bus = dma_bus();
    bus.write32(DATA, 0x4433_2211).unwrap();
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, DATA).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    bus.periph.gdma.out[0].conf0 = 1 << 2;
    let dirty = |bus: &mut SocBus, addr: u32, v: u32| { bus.irq_dirty = false; bus.write32(addr, v).unwrap(); bus.irq_dirty };
    assert!(!dirty(&mut bus, SPI2 + 0x08, 0x1234), "configuration write");
    assert!(!dirty(&mut bus, GDMA + 0x70, 1 << 3), "GDMA enable without a raw event");
    assert!(!dirty(&mut bus, SPI2 + 0x34, 1 << 12), "SPI2 enable without a raw event");
    bus.write32(SPI2 + 0x30, 1 << 28).unwrap();
    bus.write32(SPI2 + 0x10, 1 << 27).unwrap();
    assert!(!dirty(&mut bus, SPI2 + 0x1c, 31));
    assert!(dirty(&mut bus, SPI2, 1 << 24), "the command completes the DMA and raises both sources");
    assert_ne!(bus.periph.spi2.int_raw & (1 << 12), 0);
    assert!(!dirty(&mut bus, SPI2 + 0x34, 1 << 12), "unchanged enable");
    assert!(dirty(&mut bus, SPI2 + 0x38, 1 << 12), "SPI2 clear drops its source");
    assert!(dirty(&mut bus, GDMA + 0x74, 1 << 3), "GDMA clear drops its source");
    assert!(!dirty(&mut bus, GDMA + 0x74, 1 << 3), "clearing again changes nothing");
    assert!(dirty(&mut bus, 0x6000_0010, 1), "UART writes keep the blanket rule");
}

/// Review (mmio blocker): timed SPI2 submission collects its descriptors through the bus, and a
/// descriptor word can be a device register. Here the EOF descriptor sits in unmodelled register
/// storage just below USB Serial/JTAG, so its next word pops the USB FIFO, presents the queued
/// packet and raises RECV_PKT: interrupts must be dirty at submission, not at the wire deadline.
#[test]
fn timed_spi2_descriptor_collection_dirties_interrupts() {
    const DATA: u32 = 0x3fc9_0200;
    const DESC: u32 = 0x6003_7ff8; // + 8 is the USB Serial/JTAG FIFO
    const LINE: u32 = 5;
    let mut bus = dma_bus();
    bus.spi2_timing = true;
    bus.write32(DATA, 0x4433_2211).unwrap();
    bus.write32(DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(DESC + 4, DATA).unwrap();
    bus.periph.gdma.out[0].desc = DESC;
    bus.periph.intmatrix.map[0][crate::periph::SRC_USB_SERIAL_JTAG] = LINE;
    bus.periph.usb.host_input(&[0x11]);
    bus.periph.usb.host_input(&[0x22]);
    bus.periph.usb.int_raw = 0;
    bus.periph.usb.int_ena = 1 << 2;
    bus.write32(SPI2 + 0x0c, 1 << 12).unwrap();
    bus.write32(SPI2 + 0x30, 1 << 28).unwrap();
    bus.write32(SPI2 + 0x10, 1 << 27).unwrap();
    bus.write32(SPI2 + 0x1c, 31).unwrap();
    let refresh = |bus: &mut SocBus| <SocBus as esp_soc::SocBus>::refresh_irq(bus);
    refresh(&mut bus);
    assert!(!bus.periph.usb.irq());
    assert_eq!(bus.periph.cpu_lines_both().0 & (1 << LINE), 0);
    bus.irq_dirty = false;
    bus.write32(SPI2, 1 << 24).unwrap();
    assert!(bus.spi2_scheduled.is_some() && bus.periph.spi2.transfers == 0, "still on the wire");
    assert_eq!(bus.periph.usb.rx.iter().copied().collect::<Vec<_>>(), [0x22], "the next word read the FIFO");
    assert!(bus.periph.usb.irq(), "the queued packet raised RECV_PKT");
    assert!(bus.irq_dirty && bus.block_break(), "USB moved during submission: interrupts are dirty now");
    assert!(refresh(&mut bus));
    assert_ne!(bus.periph.cpu_lines_both().0 & (1 << LINE), 0);
}

#[test]
fn spi2_data_phase_comes_from_gdma_descriptor() {
    const DATA: u32 = 0x3fc9_0200;
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = dma_bus();
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    bus.write32(DATA, 0x4433_2211).expect("test data write failed");
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, DATA).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, 0).expect("descriptor link write failed");
    bus.periph.gdma.out[0].conf0 = 1 << 2;

    start_dma(&mut bus, 32);

    assert_eq!(&*events.lock().expect("probe mutex poisoned"), &["spi:2:[11, 22, 33, 44]:0"]);
    assert_eq!(bus.read32(FIRST_DESC).expect("descriptor read failed") >> 31, 0);
    assert_eq!(bus.periph.gdma.out[0].int_raw & 0xb, 0xb);
}

#[test]
fn dma_payload_crosses_a_tlb_mapping_boundary() {
    const DATA: u32 = 0x3fc8_fffe;
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = dma_bus();
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    for (offset, byte) in [0x11, 0x22, 0x33, 0x44].into_iter().enumerate() {
        bus.write8(DATA + offset as u32, byte).expect("test data write failed");
    }
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, DATA).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, 0).expect("descriptor link write failed");

    start_dma(&mut bus, 32);

    assert_eq!(&*events.lock().expect("probe mutex poisoned"), &["spi:2:[11, 22, 33, 44]:0"]);
    assert_eq!(bus.spi2_dma_fault, None);
}

#[test]
fn dma_payload_reports_the_first_unmapped_address() {
    const DATA: u32 = DRAM_HIGH - 1;
    let mut bus = dma_bus();
    bus.write8(DATA, 0xaa).expect("test data write failed");
    bus.write32(FIRST_DESC, 2 | (2 << 12) | (1 << 30) | (1 << 31)).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, DATA).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, 0).expect("descriptor link write failed");

    start_dma(&mut bus, 16);

    let expected = DmaDescriptorFault::BufferRead { descriptor: FIRST_DESC, address: DRAM_HIGH, fault: Fault::Unmapped };
    assert_eq!(bus.spi2_dma_fault, Some(expected));
    assert_eq!(bus.last_fault, Some((DRAM_HIGH, false)));
    assert_dma_fault_and_recovery(&mut bus, expected);
}

#[test]
fn descriptor_control_read_failure_is_typed() {
    let mut bus = dma_bus();
    bus.periph.gdma.out[0].desc = DRAM_HIGH;

    start_dma(&mut bus, 8);

    assert_dma_fault_and_recovery(&mut bus, DmaDescriptorFault::Read {
        descriptor: DRAM_HIGH,
        word: DmaDescriptorWord::Control,
        fault: Fault::Unmapped,
    });
}

#[test]
fn descriptor_buffer_read_failure_is_typed() {
    let mut bus = dma_bus();
    bus.write32(FIRST_DESC, 1 | (1 << 12) | (1 << 30) | (1 << 31)).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, 0).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, 0).expect("descriptor link write failed");

    start_dma(&mut bus, 8);

    assert_dma_fault_and_recovery(&mut bus, DmaDescriptorFault::BufferRead {
        descriptor: FIRST_DESC,
        address: 0,
        fault: Fault::Unmapped,
    });
}

#[test]
fn descriptor_cycle_is_typed() {
    let mut bus = dma_bus();
    bus.write32(FIRST_DESC, 1 << 31).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, 0).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, FIRST_DESC).expect("descriptor link write failed");

    start_dma(&mut bus, 8);

    assert_dma_fault_and_recovery(&mut bus, DmaDescriptorFault::Cycle { descriptor: FIRST_DESC });
}

#[test]
fn cpu_owned_descriptor_is_typed() {
    let mut bus = dma_bus();
    bus.write32(FIRST_DESC, 1 | (1 << 12)).expect("descriptor write failed");

    start_dma(&mut bus, 8);

    assert_dma_fault_and_recovery(&mut bus, DmaDescriptorFault::NotOwned { descriptor: FIRST_DESC });
}

#[test]
fn out_descriptor_length_is_not_limited_by_size() {
    const DATA: u32 = 0x3fc9_0200;
    let mut bus = dma_bus();
    bus.write32(DATA, 0xbbaa).expect("test data write failed");
    bus.write32(FIRST_DESC, 1 | (2 << 12) | (1 << 30) | (1 << 31)).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, DATA).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, 0).expect("descriptor link write failed");

    start_dma(&mut bus, 16);

    assert_eq!(bus.spi2_dma_fault, None);
    assert_eq!(bus.periph.gdma.out[0].int_raw & 0xb, 0xb);
    assert_eq!(bus.periph.spi2.transfers, 1);
}

#[test]
fn short_descriptor_chain_is_typed() {
    const DATA: u32 = 0x3fc9_0200;
    let mut bus = dma_bus();
    bus.write8(DATA, 0xaa).expect("test data write failed");
    bus.write32(FIRST_DESC, 1 | (1 << 12) | (1 << 30) | (1 << 31)).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, DATA).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, 0).expect("descriptor link write failed");

    start_dma(&mut bus, 16);

    assert_dma_fault_and_recovery(&mut bus, DmaDescriptorFault::PayloadTooShort { expected: 2, actual: 1 });
}

#[test]
fn read_only_auto_writeback_descriptor_is_typed() {
    const DATA: u32 = 0x3fc9_0200;
    let mut bus = dma_bus();
    bus.periph.gdma.out[0].desc = IROM_MASK_LOW;
    bus.periph.gdma.out[0].conf0 = 1 << 2;
    bus.write8(DATA, 0xaa).expect("test data write failed");
    bus.irom[0..4].copy_from_slice(&(1u32 | (1 << 12) | (1 << 30) | (1 << 31)).to_le_bytes());
    bus.irom[4..8].copy_from_slice(&DATA.to_le_bytes());
    bus.irom[8..12].copy_from_slice(&0u32.to_le_bytes());

    start_dma(&mut bus, 8);

    assert_dma_fault_and_recovery(&mut bus, DmaDescriptorFault::Writeback {
        descriptor: IROM_MASK_LOW,
        fault: Fault::Prohibited,
    });
}

#[test]
fn descriptor_step_budget_is_typed() {
    let mut bus = dma_bus();
    for step in 0..=SPI2_DMA_DESCRIPTOR_STEP_BUDGET {
        let descriptor = FIRST_DESC + step as u32 * 12;
        bus.write32(descriptor, 1 << 31).expect("descriptor write failed");
        bus.write32(descriptor + 4, 0).expect("descriptor buffer write failed");
        bus.write32(descriptor + 8, descriptor + 12).expect("descriptor link write failed");
    }

    start_dma(&mut bus, 0x40000);

    assert_dma_fault_and_recovery(&mut bus, DmaDescriptorFault::StepBudgetExceeded {
        budget: SPI2_DMA_DESCRIPTOR_STEP_BUDGET,
    });
}

#[test]
fn short_ms_dlen_retires_an_overlong_eof_descriptor() {
    const DATA: u32 = 0x3fc9_0200;
    let mut bus = dma_bus();
    bus.write32(DATA, 0x4433_2211).expect("test data write failed");
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).expect("descriptor write failed");
    bus.write32(FIRST_DESC + 4, DATA).expect("descriptor buffer write failed");
    bus.write32(FIRST_DESC + 8, 0).expect("descriptor link write failed");

    start_dma(&mut bus, 16);

    assert_eq!(bus.spi2_dma_fault, None);
    assert_eq!(bus.periph.gdma.out[0].int_raw & 0xb, 0xb);
    assert_eq!(bus.periph.gdma.out[0].eof_desc, FIRST_DESC);
    assert!(!bus.periph.gdma.out[0].running);
    assert_eq!(bus.periph.spi2.transfers, 1);
}

#[test]
fn narrow_mmio_writes_are_rejected_without_device_or_time_side_effects() {
    const USB: u32 = 0x6003_8000;
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.tick_budget = MAX_TICK_DEFER;
    bus.periph.usb.host_input(&[0x11, 0x22]);
    bus.periph.usb.int_raw |= 2;
    bus.periph.usb.int_ena = 0x1122_3344;
    assert_eq!(Bus::tick(&mut bus, 37), 0);
    bus.periph.misc.mmio_log = Some(Vec::new());
    for lane in 0..4 {
        assert_eq!(bus.write8(USB + lane, 0x55), Err(Fault::Prohibited));
        assert_eq!(bus.write8(USB + 0x14 + lane, 0xff), Err(Fault::Prohibited));
        assert_eq!(bus.write8(USB + 0x10 + lane, 0xff), Err(Fault::Prohibited));
    }
    for lane in [0, 2] {
        assert_eq!(bus.write16(USB + lane, 0x5566), Err(Fault::Prohibited));
        assert_eq!(bus.write16(USB + 0x14 + lane, 0xffff), Err(Fault::Prohibited));
    }
    assert_eq!(bus.last_fault, Some((USB + 0x16, true)));
    assert_eq!(bus.tick_pending, 37);
    assert!(!bus.irq_dirty);
    assert!(bus.periph.misc.mmio_log.as_ref().unwrap().is_empty());
    assert_eq!(bus.periph.usb.rx.iter().copied().collect::<Vec<_>>(), [0x11, 0x22]);
    assert!(bus.periph.usb.tx_fifo.is_empty());
    assert_eq!(bus.periph.usb.int_raw, 6);
    assert_eq!(bus.periph.usb.int_ena, 0x1122_3344);

    bus.write32(USB, 0x55).unwrap();
    assert_eq!(bus.periph.usb.tx_fifo, [0x55]);
    assert_eq!(bus.periph.usb.rx.len(), 2);
    bus.write32(USB + 0x14, 2).unwrap();
    assert_eq!(bus.periph.usb.int_raw, 4);
    bus.write32(USB + 0x10, 0xabcd).unwrap();
    assert_eq!(bus.periph.usb.int_ena, 0xabcd);
}

#[test]
fn radio_analog_registers_take_byte_and_half_word_access() {
    // NRXPD_CTRL (NRX +0xd4) is 8 bits wide in Espressif's SVD (esp32s3 PAC 0.36,
    // nrx/nrxpd_ctrl.rs: `type Ux = u8`), and esp-hal 1.2.2's sleep setup clears its
    // force-power-up bits with l8ui/s8i. FE, FE2 and BB are the radio's other analog blocks.
    const NRXPD_CTRL: u32 = 0x6001_ccd4;
    const BB: u32 = 0x6001_d000;
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.write32(NRXPD_CTRL, 0x2a).unwrap();
    assert_eq!(bus.read8(NRXPD_CTRL), Ok(0x2a));
    bus.write8(NRXPD_CTRL, 0x2a & 0xd5).unwrap();
    assert_eq!(bus.read32(NRXPD_CTRL), Ok(0));
    bus.write16(BB + 0x56, 0xbeef).unwrap();
    assert_eq!(bus.read32(BB + 0x54), Ok(0xbeef_0000));
    assert_eq!(bus.read16(BB + 0x56), Ok(0xbeef));
    assert_eq!(bus.write8(0x6003_8000, 0), Err(Fault::Prohibited), "other blocks keep the rule");
}

#[test]
fn unsupported_mmio_reads_do_not_pop_fifos_or_advance_time() {
    const USB: u32 = 0x6003_8000;
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.tick_budget = MAX_TICK_DEFER;
    bus.periph.usb.host_input(&[0x11, 0x22]);
    assert_eq!(Bus::tick(&mut bus, 37), 0);
    bus.periph.misc.mmio_log = Some(Vec::new());
    for lane in 0..4 {
        assert_eq!(bus.read8(USB + lane), Err(Fault::Prohibited));
        assert_eq!(bus.read16(USB + lane), Err(Fault::Prohibited));
        assert_eq!(bus.read8(MMU_TABLE + lane), Err(Fault::Prohibited));
    }
    for lane in 1..4 {
        assert_eq!(bus.read32(MMU_TABLE + lane), Err(Fault::Misaligned));
        assert_eq!(bus.read32(USB + lane), Err(Fault::Misaligned));
    }
    assert_eq!(bus.last_fault, Some((USB + 3, false)));
    assert_eq!(bus.tick_pending, 37);
    assert!(!bus.irq_dirty);
    assert!(bus.periph.misc.mmio_log.as_ref().unwrap().is_empty());
    assert_eq!(bus.periph.usb.rx.iter().copied().collect::<Vec<_>>(), [0x11, 0x22]);
    assert_eq!(bus.read32(USB), Ok(0x11));
    assert_eq!(bus.periph.usb.rx.iter().copied().collect::<Vec<_>>(), [0x22]);
    assert_eq!(bus.tick_pending, 0);
}

#[test]
fn unsupported_mmu_writes_do_not_change_mapping() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.write32(MMU_TABLE, 7).unwrap();
    assert_eq!(bus.write8(MMU_TABLE, 9), Err(Fault::Prohibited));
    assert_eq!(bus.write16(MMU_TABLE, 10), Err(Fault::Prohibited));
    assert_eq!(bus.write32(MMU_TABLE + 1, 11), Err(Fault::Misaligned));
    assert_eq!(bus.mmu[0], 7);
}

#[test]
fn mmio_read_flush_notifies_interrupt_changes_before_the_backstop() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.tick_budget = MAX_TICK_DEFER;
    bus.periph.usb.int_ena = 1 << 1;
    // Leave one cycle before the currently modelled SOF boundary. Reads
    // then flush each short slice so no Bus::tick reaches its backstop.
    bus.periph.usb.tick(crate::periph::CPU_HZ / 4000 - 1);
    bus.irq_dirty = false;
    assert_eq!(Bus::tick(&mut bus, 1), 0);
    assert!(!bus.irq_dirty);
    assert_eq!(bus.read32(0x6003_8008).unwrap() & 2, 2);
    assert!(bus.periph.usb.irq());
    assert!(bus.block_break());

    for _ in 0..5 {
        bus.irq_dirty = false;
        assert_eq!(Bus::tick(&mut bus, 128), 0);
        let _ = bus.read32(0x6003_8008).unwrap();
        assert!(!bus.block_break(), "an unchanged source must not break every polling block");
    }
}

#[test]
fn deferred_extension_bases_include_preoffset_boundary_crossings() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    for armed in [false, true] {
        bus.defer_mmio = armed;
        for (addr, nearby) in [(PERIPH_BASE - 129, false), (PERIPH_BASE - 128, true),
            (PERIPH_BASE, true), (PERIPH_END - 1, true), (PERIPH_END + 127, true),
            (PERIPH_END + 128, false), (DRAM_LOW, false)] {
            bus.mmio_deferred = false;
            assert_eq!(bus.defer_access(addr), armed && nearby, "base {addr:x}");
            assert_eq!(bus.mmio_deferred, armed && nearby);
        }
    }
}

#[test]
fn quiet_backstop_keeps_the_original_cadence_for_active_devices() {
    type Activation = (&'static str, fn(&mut Peripherals));
    let cases: &[Activation] = &[
        ("i2s0", |p| p.i2s0.write(0x24, 1 << 2)),
        ("i2s1", |p| p.i2s1.write(0x24, 1 << 2)),
        ("camera", |p| { p.lcd_cam.set_clock_enabled(true); p.lcd_cam.write(0x04, 1 << 29); p.lcd_cam.write(0x08, 1 << 29); }),
        ("camera-vsync", |p| { p.lcd_cam.set_clock_enabled(true); p.lcd_cam.write(0x04, 1 << 29); p.lcd_cam.write(0x64, 1 << 2); }),
        ("lcd", |p| { p.lcd_cam.lcd_user |= 1 << 27; p.lcd_cam.lcd_ctrl |= 1 << 31; }),
        ("gdma-out", |p| p.gdma.out[0].running = true),
        ("wifi-tx", |p| p.wifi.tx_pending.push((0, 0))),
        ("wifi-ap", |p| p.wifi.link.attach(Some(crate::wifi::VirtualAp::new(crate::wifi::ApConfig {
            ssid: "test".into(), bssid: [0; 6], channel: 1, psk: None, epoch_s: None,
        }, false)), None)),
        ("network", |p| p.wifi.link.attach(None, Some(crate::net::VirtualNet::new(false)))),
        ("aes", |p| p.aes.dma_pending = true),
        ("sha", |p| p.sha.dma_pending = true),
        ("spi-dma", |p| p.spi2.dma_tx_pending = Some(8)),
        ("spi-transfer", |p| p.spi2.write(0, 1 << 24)),
        ("rmt", |p| p.rmt.write(0x20, 1)),
        ("rmt-done", |p| p.rmt.done.push((0, Vec::new()))),
        ("gpio", |p| p.gpio.changes.push((0, true))),
        ("gpio-input", |p| p.gpio.input_changes.push((0, true))),
        ("watchdog", |p| p.rtc.ram.write(0x98, 1 << 31)),
        ("usb-sof", |p| p.usb.int_ena = 1 << 1),
    ];
    for &(name, activate) in cases {
        let mut bus = SocBus::new(1024, 1024, [0; 6]);
        bus.refresh_tick_budget();
        assert_eq!(bus.tick_budget, QUIET_TICK_DEFER, "{name}: initially quiet");
        activate(&mut bus.periph);
        bus.refresh_tick_budget();
        assert_eq!(bus.tick_budget, MAX_TICK_DEFER, "{name}: active cadence");
    }
    // EX157: an armed GDMA IN channel without a producer is passive and stays quiet.
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.gdma.inp[0].running = true;
    bus.refresh_tick_budget();
    assert_eq!(bus.tick_budget, QUIET_TICK_DEFER, "gdma-in alone is passive");
    // Real MMIO writes must switch the cap immediately in both directions.
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.write32(0x6003_8010, 2).unwrap();
    assert_eq!(bus.tick_budget, MAX_TICK_DEFER);
    bus.write32(0x6003_8010, 0).unwrap();
    assert_eq!(bus.tick_budget, QUIET_TICK_DEFER);
}

#[test]
fn periodic_tick_only_requests_irq_refresh_for_events() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.usb.int_ena = 2; // active SOF keeps the original periodic backstop
    for _ in 0..4 {
        assert_eq!(Bus::tick(&mut bus, MAX_TICK_DEFER), 0);
        assert_eq!(bus.tick_pending, 0, "quiet flush still advances time");
    }
    bus.periph.usb.int_ena = 2;
    bus.periph.usb.tick(crate::periph::CPU_HZ / 4000 - 4 * u64::from(MAX_TICK_DEFER) - 1);
    bus.tick_budget = 1;
    assert_eq!(Bus::tick(&mut bus, 1), 1);
    assert!(bus.periph.usb.irq());
    bus.irq_dirty = false;
    assert_eq!(Bus::tick(&mut bus, MAX_TICK_DEFER), 0, "unchanged asserted source");
    bus.irq_dirty = true;
    assert_eq!(Bus::tick(&mut bus, MAX_TICK_DEFER), 1, "preserve prior dirty flag");
}

#[test]
fn host_input_notifies_without_a_periodic_irq_scan() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.usb.int_ena = 4;
    esp_soc::SocBus::serial_input(&mut bus, b"x");
    assert!(bus.irq_dirty);
    assert!(bus.periph.usb.irq());
    bus.irq_dirty = false;
    esp_soc::SocBus::serial_input(&mut bus, b"y");
    assert!(!bus.irq_dirty, "same asserted source");
    bus.periph.gpio.write(0x74 + 4 * 7, (5 << 7) | (1 << 13));
    bus.periph.gpio.set_input(7, true);
    esp_soc::SocBus::gpio_set_input(&mut bus, 7, false);
    assert!(bus.irq_dirty, "host GPIO falling level");
    assert!(!bus.periph.gpio.irq());
}

#[test]
fn gpio_output_level_irqs_notify_for_both_banks_and_polarities() {
    for pin in [7, 40] {
        for typ in [4, 5] {
            let mut bus = SocBus::new(1024, 1024, [0; 6]);
            bus.periph.gpio.write(if pin < 32 { 0x24 } else { 0x30 }, 1 << (pin % 32));
            bus.periph.gpio.write(0x74 + 4 * pin as u32, (typ << 7) | (1 << 13));
            let base = if pin < 32 { 0x6000_4004 } else { 0x6000_4010 };
            let bit = 1 << (pin % 32);
            // OUT, W1TC, W1TS, OUT all change the level in this sequence.
            for (addr, value, high) in [(base, bit, true), (base + 8, bit, false), (base + 4, bit, true), (base, 0, false)] {
                bus.irq_dirty = false;
                bus.write32(addr, value).unwrap();
                assert!(bus.irq_dirty, "pin {pin}, type {typ}, register {addr:x}");
                assert_eq!(bus.periph.gpio.irq(), if typ == 5 { high } else { !high });
            }
            bus.periph.gpio.write(0x74 + 4 * pin as u32, 0);
            bus.irq_dirty = false;
            bus.write32(base + 4, bit).unwrap();
            assert!(!bus.irq_dirty, "ordinary output toggles remain cheap");
        }
    }
}

#[test]
fn periodic_tick_notifies_wifi_tx_and_air_rx() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.write32(FIRST_DESC, 0).unwrap();
    bus.write32(FIRST_DESC + 4, 0).unwrap();
    bus.periph.wifi.tx_pending.push((0, FIRST_DESC));
    bus.irq_dirty = false;
    assert_eq!(Bus::tick(&mut bus, MAX_TICK_DEFER), 1);
    assert!(bus.periph.wifi.irq());

    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.wifi.link.attach(Some(crate::wifi::VirtualAp::new(crate::wifi::ApConfig {
        ssid: "test".into(), bssid: [2, 0, 0, 0, 0, 1], channel: 1, psk: None, epoch_s: None,
    }, false)), None);
    bus.periph.wifi.link.ap_mut().unwrap().queue.push(crate::wifi::AirFrame { at_us: 0, frame: vec![0; 24] });
    bus.periph.wifi.rx_next = FIRST_DESC & 0xfffff;
    bus.write32(FIRST_DESC, 512 | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, FIRST_DESC + 64).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    bus.irq_dirty = false;
    assert_eq!(Bus::tick(&mut bus, (crate::periph::CPU_HZ / 1000) as u32), 1);
    assert_eq!(bus.periph.wifi.link.rx_frames(), 1);
    assert!(bus.periph.wifi.irq());
}

/// A WiFi bus with one open virtual access point and `frames` of 24 bytes queued on the air, the
/// first byte of each a marker so the test can tell them apart in the receive buffer.
fn wifi_bus_with_air_frames(frames: &[u8]) -> SocBus {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.wifi.link.attach(Some(crate::wifi::VirtualAp::new(crate::wifi::ApConfig {
        ssid: "test".into(), bssid: [2, 0, 0, 0, 0, 1], channel: 1, psk: None, epoch_s: None,
    }, false)), None);
    for &marker in frames {
        bus.periph.wifi.link.ap_mut().unwrap().queue.push(crate::wifi::AirFrame { at_us: 0, frame: vec![marker; 24] });
    }
    bus
}

/// One RX descriptor: `512` bytes of buffer at `buf`, owned by the DMA, pointing at `next`.
fn arm_rx_desc(bus: &mut SocBus, at: u32, buf: u32, next: u32) {
    bus.write32(at, 512 | (1 << 31)).unwrap();
    bus.write32(at + 4, buf).unwrap();
    bus.write32(at + 8, next).unwrap();
}

/// One scheduling round of one emulated millisecond: the air delivers at most one frame in a round.
fn wifi_air_round(bus: &mut SocBus) { assert_eq!(Bus::tick(bus, (crate::periph::CPU_HZ / 1000) as u32), 1); }

#[test]
fn air_rx_restarts_at_the_base_when_the_list_runs_dry() {
    // Arduino-ESP32 arms the receive DMA one short list at a time: four descriptors, the last one's
    // `next` null. The frame after that list therefore finds nothing to land in, and the driver's
    // `BASE_RX_DSCR` write — the head of the list it just recycled — is what starts the DMA again.
    let mut bus = wifi_bus_with_air_frames(&[0x11, 0x22, 0x33]);
    let (first, dead, second) = (FIRST_DESC, FIRST_DESC + 0x40, FIRST_DESC + 0x80);
    let dead_base = dead & 0xf_ffff;
    arm_rx_desc(&mut bus, first, first + 0x200, dead);          // the armed list: one descriptor, then a null word
    bus.write32(dead, 0).unwrap();
    bus.write32(0x6003_3088, first & 0xf_ffff).unwrap();        // BASE_RX_DSCR
    wifi_air_round(&mut bus);
    assert_eq!((bus.periph.wifi.link.tx_frames(), bus.periph.wifi.link.rx_frames()), (0, 1));
    assert_eq!(bus.read8_unpriced(first + 0x200 + 48).unwrap_or(0), 0x11, "frame after the 48-byte rx_ctrl header");
    assert_eq!(bus.periph.wifi.rx_next, dead_base);

    // Software recycled the frame and left the next descriptor alone: the DMA stops there.
    arm_rx_desc(&mut bus, first, first + 0x200, dead);
    wifi_air_round(&mut bus);
    assert_eq!(bus.periph.wifi.link.rx_dropped(), 1, "a frame that finds the DMA stopped is lost");

    // The driver hands over the next list, base first and the base write on its own.
    arm_rx_desc(&mut bus, second, second + 0x200, 0);
    bus.write32(0x6003_3088, second & 0xf_ffff).unwrap();
    wifi_air_round(&mut bus);
    assert_eq!(bus.periph.wifi.link.rx_frames(), 2, "the base write restarts the stopped DMA");
    assert_eq!(bus.periph.wifi.rx_next, 0, "the new list ends after one descriptor");
    assert_eq!(bus.read8_unpriced(second + 0x200 + 48).unwrap_or(0), 0x33);
}

#[test]
fn air_rx_base_write_does_not_rewind_a_running_list() {
    // Software rewrites BASE_RX_DSCR every time it recycles descriptors. While the DMA runs, that must
    // leave its pointer alone: rewinding re-delivers into descriptors the stack has already moved past.
    let mut bus = wifi_bus_with_air_frames(&[0x11, 0x22]);
    let (first, second) = (FIRST_DESC, FIRST_DESC + 0x40);
    arm_rx_desc(&mut bus, first, first + 0x200, second);
    arm_rx_desc(&mut bus, second, second + 0x200, 0);
    bus.write32(0x6003_3088, first & 0xf_ffff).unwrap();
    wifi_air_round(&mut bus);
    assert_eq!((bus.periph.wifi.link.rx_frames(), bus.periph.wifi.rx_next), (1, (second & 0xf_ffff)));

    arm_rx_desc(&mut bus, first, first + 0x200, second);        // recycle descriptor 0
    bus.write32(0x6003_3088, first & 0xf_ffff).unwrap();        // and rewrite the base, as software does
    wifi_air_round(&mut bus);
    assert_eq!(bus.periph.wifi.link.rx_frames(), 2);
    assert_eq!(bus.read8_unpriced(second + 0x200 + 48).unwrap_or(0), 0x22, "landed in the next descriptor, not back at the base");
    assert_eq!(bus.read32(first).unwrap() >> 30 & 1, 0, "descriptor 0 was not filled a second time");
}

fn read_flush(bus: &mut SocBus, cycles: u32) {
    bus.tick_budget = MAX_TICK_DEFER;
    bus.irq_dirty = false;
    assert_eq!(Bus::tick(bus, cycles), 0);
    bus.read32(0x6003_8008).unwrap();
}

#[test]
fn read_flush_reports_timer_and_rmt_threshold_sources() {
    for timer in 0..5 {
        let mut bus = SocBus::new(1024, 1024, [0; 6]);
        if timer < 3 {
            let st = &mut bus.periph.systimer;
            st.conf = (1 << 30) | (1 << (24 - timer));
            st.armed[timer] = true;
            st.target[timer] = 1;
            st.int_ena = 1 << timer;
        } else {
            let tg = &mut bus.periph.timg[timer - 3];
            tg.t[0].config = (1 << 31) | (1 << 30) | (1 << 13) | (1 << 10);
            tg.t[0].alarm = 1;
            tg.int_ena = 1;
        }
        read_flush(&mut bus, 15);
        assert!(bus.block_break(), "timer {timer}");
        read_flush(&mut bus, 15);
        assert!(!bus.block_break(), "unchanged timer {timer}");
    }
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.rmt.write(0x20, 1);
    bus.periph.rmt.ch[0].tx_lim = 1;
    bus.periph.rmt.mem[0] = 100 | (100 << 16);
    bus.periph.rmt.int_ena = 1 << 8;
    read_flush(&mut bus, 1);
    assert!(bus.periph.rmt.irq());
    assert!(bus.periph.rmt.ch[0].running, "threshold precedes completion");
    assert!(bus.block_break());
}

#[test]
fn read_flush_reports_pcnt_without_a_gpio_interrupt() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.gpio.write(0x154 + 4 * 33, 0x80 | 7);
    bus.periph.pcnt.conf[0][0] = (1 << 18) | (1 << 14);
    bus.periph.pcnt.conf[0][1] = 1;
    bus.periph.pcnt.int_ena = 1;
    bus.periph.gpio.set_input(7, false);
    bus.periph.gpio.set_input(7, true);
    read_flush(&mut bus, 1);
    assert!(!bus.periph.gpio.irq());
    assert!(bus.periph.pcnt.irq());
    assert!(bus.block_break());
}

#[test]
fn read_flush_reports_terminal_i2s_and_lcd_dma_descriptors() {
    for peripheral in [3, 4, 5] {
        let mut bus = dma_bus();
        bus.periph.gdma.out[0].peri_sel = peripheral;
        bus.periph.gdma.out[0].int_ena = 0xb;
        // An exhausted final descriptor, with no further sample or frame to
        // publish: the descriptor completion alone must notify the CPU.
        bus.write32(FIRST_DESC, (1 << 30) | (1 << 31)).unwrap();
        bus.write32(FIRST_DESC + 4, 0).unwrap();
        bus.write32(FIRST_DESC + 8, 0).unwrap();
        match peripheral {
            3 => { bus.periph.i2s0.write(0x2c, 15 << 13); bus.periph.i2s0.write(0x54, (1 << 16) | 3); bus.periph.i2s0.write(0x24, 4); bus.periph.i2s0.sample_rate = crate::periph::CPU_HZ as u32; }
            4 => { bus.periph.i2s1.write(0x2c, 15 << 13); bus.periph.i2s1.write(0x54, (1 << 16) | 3); bus.periph.i2s1.write(0x24, 4); bus.periph.i2s1.sample_rate = crate::periph::CPU_HZ as u32; }
            _ => {
                bus.periph.lcd_cam.lcd_user = 1 << 27;
                bus.periph.lcd_cam.lcd_ctrl = 1 << 31;
                bus.periph.lcd_cam.lcd_ctrl1 = 511 << 8;
            }
        }
        read_flush(&mut bus, 1);
        assert!(bus.periph.gdma.out[0].irq(), "DMA {peripheral}");
        assert!(bus.block_break(), "DMA {peripheral}");
    }
}

#[test]
fn read_flush_reports_falling_board_level_interrupt() {
    struct FallingEdge;
    impl crate::board::BoardModel for FallingEdge {
        fn name(&self) -> &'static str { "falling-edge" }
        fn take_edges(&mut self) -> Vec<crate::board::BoardEdge> {
            vec![crate::board::BoardEdge { cycle: 1, pin: 7, level: false }]
        }
    }
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(FallingEdge);
    bus.periph.gpio.write(0x74 + 4 * 7, (5 << 7) | (1 << 13));
    bus.periph.gpio.set_input(7, true);
    assert!(bus.periph.gpio.irq());
    read_flush(&mut bus, 1);
    assert!(!bus.periph.gpio.irq());
    assert!(bus.block_break());
}

#[test]
fn empty_tick_flush_does_not_break_a_block() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.flush_ticks();
    assert!(!bus.block_break());
}

#[test]
fn host_touch_uses_the_current_bus_horizon_and_keeps_its_edge_timestamp() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(crate::board::WaveshareAmoled18V2::new());
    bus.gpio_events = Some(Vec::new());
    bus.periph.gpio.write(0x74 + 4 * u32::from(crate::board::PIN_AMOLED_TOUCH_INT), (2 << 7) | (1 << 13));
    bus.tick_budget = MAX_TICK_DEFER;

    assert_eq!(Bus::tick(&mut bus, 37), 0);
    assert_eq!(bus.tick_pending, 37);
    esp_soc::SocBus::touch_input(&mut bus, 100, 200, true);
    assert_eq!(bus.tick_pending, 37);
    assert_eq!(bus.tick_budget, 38);
    assert!(bus.gpio_events.as_deref().is_some_and(<[_]>::is_empty));

    Bus::tick(&mut bus, 64);

    assert!(!bus.periph.gpio.level(crate::board::PIN_AMOLED_TOUCH_INT));
    assert!(bus.periph.gpio.irq());
    assert!(bus.irq_dirty);
    assert_eq!(bus.gpio_events.as_deref(), Some(&[(38, crate::board::PIN_AMOLED_TOUCH_INT, false)][..]));
}

#[test]
fn no_edge_touch_keeps_pending_cycles_in_the_deadline_threshold() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(FixedDeadlineBoard { deadline: 300 });
    bus.tick_budget = MAX_TICK_DEFER;

    assert_eq!(Bus::tick(&mut bus, 100), 0);
    esp_soc::SocBus::touch_input(&mut bus, 0, 0, false);
    let horizon = 300.min(QUIET_TICK_DEFER);
    assert_eq!((bus.cycles, bus.tick_pending, bus.tick_budget), (100, 100, horizon));
    assert_eq!(Bus::tick(&mut bus, horizon - 101), 0);
    assert_eq!(Bus::tick(&mut bus, 1), 0);
    assert_eq!(bus.tick_pending, 0, "the deadline still flushes device time");
}

#[test]
fn reattaching_board_inputs_notifies_configured_level_irqs() {
    struct InputBoard(bool);
    impl crate::board::BoardModel for InputBoard {
        fn name(&self) -> &'static str { "input-restoration" }
        fn input_levels(&self) -> Vec<(u8, bool)> { vec![(7, self.0)] }
    }
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.gpio.write(0x74 + 4 * 7, (5 << 7) | (1 << 13));
    for level in [false, true, false] {
        bus.board = Box::new(InputBoard(level));
        bus.irq_dirty = false;
        bus.attach_board_devices();
        assert!(bus.irq_dirty, "board restoration changes the input to {level}");
        assert_eq!(bus.periph.gpio.irq(), level);
        bus.irq_dirty = false;
        bus.attach_board_devices();
        assert!(!bus.irq_dirty, "restoring the same input is quiet");
    }
}

#[test]
fn reboot_reattaches_amoled_i2c_devices_and_restores_board_input_levels() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(crate::board::WaveshareAmoled18V2::new());
    bus.attach_board_devices();
    for address in [0x15, 0x20, 0x34, 0x51, 0x6b] {
        assert!(bus.periph.i2c[0].has_device(address));
    }

    Bus::tick(&mut bus, (crate::periph::CPU_HZ / 120) as u32);
    esp_soc::SocBus::touch_input(&mut bus, 100, 200, true);
    Bus::tick(&mut bus, 64);
    assert!(!bus.periph.gpio.level(crate::board::PIN_AMOLED_TE));
    assert!(!bus.periph.gpio.level(crate::board::PIN_AMOLED_TOUCH_INT));

    esp_soc::SocBus::reboot(&mut bus, [0; 6]);

    for address in [0x15, 0x20, 0x34, 0x51, 0x6b] {
        assert!(bus.periph.i2c[0].has_device(address));
    }
    assert!(!bus.periph.gpio.level(crate::board::PIN_AMOLED_TE));
    assert!(!bus.periph.gpio.level(crate::board::PIN_AMOLED_TOUCH_INT));
}

#[test]
fn non_mmio_gpio_activation_restores_cadence_without_losing_pending_time() {
    struct InputBoard;
    impl crate::board::BoardModel for InputBoard {
        fn name(&self) -> &'static str { "pcnt-input" }
        fn input_levels(&self) -> Vec<(u8, bool)> { vec![(4, false)] }
    }
    for attach in [false, true] {
        for pending in [0, 100, 300] {
            let mut bus = SocBus::new(1024, 1024, [0; 6]);
            bus.periph.gpio.write(0x154 + 4 * 33, 0x80 | 4);
            bus.periph.pcnt.conf[0][0] = (1 << 16) | (1 << 14); // falling increment, threshold 0
            bus.periph.pcnt.conf[0][1] = 1;
            bus.periph.pcnt.int_ena = 1;
            Bus::tick(&mut bus, 64);
            Bus::tick(&mut bus, pending);
            assert_eq!(bus.tick_budget, QUIET_TICK_DEFER);
            if attach {
                bus.board = Box::new(InputBoard);
                bus.attach_board_devices();
            } else {
                esp_soc::SocBus::gpio_set_input(&mut bus, 4, false);
            }
            assert_eq!(bus.tick_pending, pending, "activation must preserve elapsed device time");
            assert_eq!(bus.next_deadline(), MAX_TICK_DEFER.saturating_sub(pending).max(1) as u64);
            let until = bus.next_deadline() as u32;
            Bus::tick(&mut bus, until);
            assert_eq!(bus.tick_pending, 0);
            assert_eq!(bus.periph.pcnt.cnt[0], 1, "attach={attach}, pending={pending}");
            assert!(bus.periph.pcnt.irq());
            assert!(bus.irq_dirty);
        }
    }
}


#[test]
fn timed_spi2_dma_accounts_for_both_half_duplex_phases() {
    for (user, ctrl, clocks) in [
        ((1 << 27) | (1 << 28), 0, 64),
        ((1 << 27) | (1 << 28) | 1, 0, 32),
        ((1 << 27) | (1 << 28) | (1 << 13), 0, 40),
        ((1 << 27) | (1 << 28), 1 << 15, 40),
    ] {
        let mut bus = dma_bus();
        bus.spi2_timing = true;
        bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
        bus.write32(FIRST_DESC + 4, FIRST_DESC + 16).unwrap();
        bus.write32(FIRST_DESC + 8, 0).unwrap();
        bus.write32(SPI2 + 0x30, 1 << 28).unwrap();
        bus.write32(SPI2 + 0x0c, 1 << 31).unwrap();
        bus.write32(SPI2 + 0x08, ctrl).unwrap();
        bus.write32(SPI2 + 0x10, user).unwrap();
        bus.write32(SPI2 + 0x1c, 31).unwrap();
        bus.write32(SPI2, 1 << 24).unwrap();
        let deadline = clocks * (crate::periph::CPU_HZ / 80_000_000);
        bus.tick(deadline as u32 - 1);
        assert_eq!(bus.periph.spi2.transfers, 0, "USER={user:#x} CTRL={ctrl:#x}");
        assert_ne!(bus.read32(SPI2).unwrap() & (1 << 24), 0);
        bus.tick(1);
        assert_eq!(bus.periph.spi2.transfers, 1);
        assert_eq!(bus.read32(SPI2).unwrap() & (1 << 24), 0);
    }
}

#[test]
fn timed_spi2_dma_preserves_live_gdma_configuration_and_irq_clear() {
    let mut bus = dma_bus();
    bus.spi2_timing = true;
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, FIRST_DESC + 16).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    bus.periph.gdma.out[0].int_raw = 1 << 2; // an older descriptor error
    start_dma(&mut bus, 32);
    bus.write32(GDMA + 0x74, 1 << 2).unwrap();
    bus.write32(GDMA + 0x70, 1 << 1).unwrap();
    bus.write32(GDMA + 0x60, 1 << 2).unwrap();
    bus.write32(GDMA + 0x64, 7).unwrap();
    bus.write32(GDMA + 0xa4, 3).unwrap();
    bus.tick(32 * (crate::periph::CPU_HZ / 80_000_000) as u32);
    let channel = bus.periph.gdma.out[0];
    assert_eq!(channel.int_raw, (1 << 0) | (1 << 1) | (1 << 3));
    assert_eq!(channel.int_ena, 1 << 1);
    assert_eq!((channel.conf0, channel.conf1, channel.pri), (1 << 2, 7, 3));
    assert!(channel.irq());
}

#[test]
fn timed_spi2_dma_completion_yields_to_out_reset() {
    let mut bus = dma_bus();
    bus.spi2_timing = true;
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, FIRST_DESC + 16).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    start_dma(&mut bus, 32);
    bus.tick(4);
    bus.write32(GDMA + 0x60, 1).unwrap(); // OUT_RST while the data phase is on the wire
    bus.tick(128); // past the original deadline
    assert_eq!(bus.periph.spi2.transfers, 0, "reset discards the scheduled completion");
    assert_ne!(bus.periph.spi2.int_raw & (1 << 12), 0, "aborted commands raise TRANS_DONE");
    assert_eq!(bus.read32(SPI2).unwrap() & (1 << 24), 0, "USR must report idle after reset");
    let channel = &bus.periph.gdma.out[0];
    assert_eq!((channel.desc, channel.buf_pos, channel.running), (0, 0, false));
    assert_eq!(channel.int_raw & ((1 << 0) | (1 << 1) | (1 << 3)), 0, "reset raises no done events");
    assert_eq!(bus.periph.spi2.dma_tx_pending, None);
}

#[test]
fn timed_spi2_dma_completion_yields_to_out_rebind() {
    const SECOND: u32 = FIRST_DESC + 32;
    let mut bus = dma_bus();
    bus.spi2_timing = true;
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, FIRST_DESC + 16).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    start_dma(&mut bus, 32);
    bus.tick(4);
    bus.write32(SECOND, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(SECOND + 4, FIRST_DESC + 16).unwrap();
    bus.write32(SECOND + 8, 0).unwrap();
    bus.write32(GDMA + 0x80, (SECOND & 0xF_FFFF) | (1 << 21)).unwrap(); // OUT_LINK START on the live channel
    bus.tick(128);
    assert_eq!(bus.periph.spi2.transfers, 0, "rebinding aborts the in-flight transaction");
    let channel = &bus.periph.gdma.out[0];
    assert_eq!(channel.desc, SECOND, "the new descriptor chain must survive completion");
    assert!(channel.running);
    assert_eq!(bus.periph.spi2.dma_tx_pending, None);
}

#[test]
fn timed_spi2_dma_stop_survives_completion_and_gates_the_next_usr() {
    const SECOND_DESC: u32 = 0x3fc9_0140;
    let mut bus = dma_bus();
    bus.spi2_timing = true;
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, FIRST_DESC + 16).unwrap();
    bus.write32(FIRST_DESC + 8, SECOND_DESC).unwrap(); // nonterminal: the snapshot stays running
    bus.write32(SECOND_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(SECOND_DESC + 4, FIRST_DESC + 16).unwrap();
    bus.write32(SECOND_DESC + 8, 0).unwrap();
    start_dma(&mut bus, 32);
    bus.tick(4);
    bus.write32(GDMA + 0x80, 1 << 20).unwrap(); // OUT_LINK STOP on the live channel
    assert!(!bus.periph.gdma.out[0].running);
    bus.tick(128); // the captured payload still completes
    assert_eq!(bus.periph.spi2.transfers, 1);
    assert!(!bus.periph.gdma.out[0].running, "completion must not resurrect the stopped channel");
    assert_eq!(bus.periph.gdma.out[0].desc, SECOND_DESC, "the next descriptor stays unfetched");
    start_dma(&mut bus, 32);
    bus.tick(128);
    assert_eq!(bus.periph.spi2.transfers, 1, "a stopped channel cannot serve a later USR");
    bus.write32(GDMA + 0x80, (SECOND_DESC & 0xF_FFFF) | (1 << 22)).unwrap(); // RESTART
    bus.tick(128);
    assert_eq!(bus.periph.spi2.transfers, 2, "RESTART lets the pending USR complete");
}


#[test]
fn gdma_m2m_productive_copy_resumes_after_work_budget() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    const COUNT: u32 = 2050;
    const OUT: u32 = 0x3fca_0000;
    const IN: u32 = 0x3fcb_0000;
    for i in 0..COUNT {
        let last = i + 1 == COUNT;
        m2m_desc(&mut bus, OUT + i * 16, (1 << 31) | ((last as u32) << 30) | (1 << 12) | 1,
            M2M_SRC + i, if last { 0 } else { OUT + (i + 1) * 16 });
        m2m_desc(&mut bus, IN + i * 16, (1 << 31) | 1,
            M2M_DST + i, if last { 0 } else { IN + (i + 1) * 16 });
        bus.write8(M2M_SRC + i, i as u8).unwrap();
    }
    m2m_start(&mut bus, IN, OUT, false);
    m2m_round(&mut bus);
    assert!(bus.periph.gdma.out[0].running);
    assert_eq!(bus.periph.gdma.out[0].int_raw & (1 << 2), 0);
    m2m_round(&mut bus);
    assert!(!bus.periph.gdma.out[0].running);
    for i in 0..COUNT { assert_eq!(bus.read8(M2M_DST + i).unwrap(), i as u8); }
}

/// Pin the public bus contract independently of the differential test bus.
#[test]
fn cpu_store_versions_cover_instruction_overlap() {
    for width in [1u32, 2, 4] {
        for off in [0u32, 1, 2, 3, 252, 253, 254, 255] {
            if off % width != 0 { continue; }
            let mut bus = SocBus::new(1024, 1024, [0; 6]);
            let addr = DRAM_LOW + 256 + off;
            let page = bus.code_page(addr) as usize;
            bus.note_code_page(page as u32);
            let before = bus.page_versions().to_vec();
            match width {
                1 => bus.write8(addr, 1).unwrap(),
                2 => bus.write16(addr, 1).unwrap(),
                _ => bus.write32(addr, 1).unwrap(),
            }
            for (i, (&old, &new)) in before.iter().zip(bus.page_versions()).enumerate() {
                let expected = u32::from(i == page || (off < 3 && i + 1 == page));
                assert_eq!(new.wrapping_sub(old), expected, "width={width} offset={off} page={i}");
            }
        }
    }
}

/// shell-s2: every version `stable_pages` covers changes only together with its epoch, and the
/// page generated stores can reach through the EX180 previous-page rule stays outside the range.
#[test]
fn stable_pages_move_their_epoch() {
    use esp_soc::SocBus as _;
    // Whether a covered version changed; if one did, the epoch must have moved.
    fn check(bus: &mut SocBus, what: &str, op: impl FnOnce(&mut SocBus)) -> bool {
        let (lo, hi, epoch) = bus.stable_pages();
        let before = bus.page_versions()[lo as usize..hi as usize].to_vec();
        op(bus);
        let changed = bus.stable_pages().0 != lo || bus.page_versions()[lo as usize..hi as usize] != before[..];
        assert!(!changed || bus.stable_pages().2 != epoch, "{what}: a covered version changed without the epoch");
        changed
    }
    let mut bus = SocBus::new(4 << 16, 2 << 16, [0; 6]);
    assert!(check(&mut bus, "flash write", |b| b.write_flash(0x1_0010, &[1, 2, 3]).unwrap()));
    assert!(check(&mut bus, "remap", |b| b.write32(MMU_TABLE + 4, 1).unwrap()));
    assert!(check(&mut bus, "load through the flash mapping", |b| b.load_bytes(IBUS_LOW + 0x1_0020, &[7]).unwrap()));
    assert!(check(&mut bus, "SPI flash write-back", |b| b.note_written(SRC_FLASH, 0x2_0000, 4)));
    assert!(check(&mut bus, "PSRAM resize", |b| b.set_psram_size(4 << 16).unwrap()));
    // hop-s2b: mask ROM below flash is covered from its second page on, and its loads move the epoch.
    assert_eq!(bus.stable_pages().0, bus.code_page(IROM_MASK_LOW) + 1, "the first ROM page is left to per-page compares");
    assert!(check(&mut bus, "ROM load", |b| b.load_bytes(IROM_MASK_LOW + 0x1000, &[1, 2]).unwrap()));
    assert!(check(&mut bus, "ROM load at the last ROM byte", |b| b.load_bytes(IROM_MASK_HIGH - 1, &[3]).unwrap()));
    bus.write32(MMU_TABLE, MMU_SPIRAM).unwrap();
    let first = bus.code_page(DBUS_LOW);
    bus.note_code_page(first);
    let before = bus.page_versions().to_vec();
    for width in [1, 2, 4] {
        assert!(!check(&mut bus, "store to the first bytes of PSRAM", |b| match width {
            1 => b.write8(DBUS_LOW, 1).unwrap(),
            2 => b.write16(DBUS_LOW, 1).unwrap(),
            _ => b.write32(DBUS_LOW, 1).unwrap(),
        }));
    }
    assert_eq!(bus.stable_pages().1, first - 1, "the last flash page is left to per-page compares");
    assert_eq!(bus.page_versions()[first as usize - 1], before[first as usize - 1] + 3);
}

#[test]
fn released_and_same_cycle_inputs_restore_pcnt_cadence() {
    struct Falling(bool);
    impl crate::board::BoardModel for Falling {
        fn name(&self) -> &'static str { "falling" }
        fn take_edges(&mut self) -> Vec<crate::board::BoardEdge> {
            if std::mem::take(&mut self.0) {
                vec![crate::board::BoardEdge { cycle: 64, pin: 4, level: false }]
            } else { Vec::new() }
        }
    }
    for release in [true, false] {
        let mut bus = SocBus::new(1024, 1024, [0; 6]);
        bus.periph.gpio.write(0x154 + 4 * 33, 0x80 | 4);
        bus.periph.pcnt.conf[0][0] = (1 << 16) | (1 << 14);
        bus.periph.pcnt.conf[0][1] = 1;
        bus.periph.pcnt.int_ena = 1;
        bus.periph.gpio.set_input(4, true);
        bus.periph.gpio.set_pulls(4, false, true);
        Bus::tick(&mut bus, 64);
        assert_eq!(bus.tick_budget, QUIET_TICK_DEFER);
        if release {
            esp_soc::SocBus::gpio_release_input(&mut bus, 4);
        } else {
            bus.board = Box::new(Falling(true));
            bus.attach_board_devices();
            bus.read32(0x6000_403c).unwrap();
        }
        assert_eq!(bus.next_deadline(), u64::from(MAX_TICK_DEFER));
        Bus::tick(&mut bus, MAX_TICK_DEFER);
        assert_eq!(bus.periph.pcnt.cnt[0], 1);
        assert!(bus.periph.pcnt.irq());
    }
}

#[test]
fn bare_gpio_toggle_loop_keeps_quiet_cadence_and_no_pcnt_work() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(esp_soc::NoBoard);
    bus.attach_board_devices();
    bus.write32(0x6000_4024, 1 << 4).unwrap();
    bus.write32(0x6000_4030, 1 << 8).unwrap();
    bus.flush_ticks();
    for _ in 0..100 {
        for offset in [8, 12, 20, 24] {
            bus.write32(0x6000_4000 + offset, if offset < 16 { 1 << 4 } else { 1 << 8 }).unwrap();
            bus.read32(0x6000_403c).unwrap();
            bus.read32(0x6000_4040).unwrap();
            assert!(bus.periph.gpio.input_changes.is_empty());
            assert_eq!(bus.tick_budget, QUIET_TICK_DEFER);
            assert!(!bus.cadence_active());
            bus.tick(1);
        }
    }
    assert_eq!(bus.periph.pcnt.cnt, [0; 4]);
    assert_eq!(bus.periph.gpio.input_changes.capacity(), 0);
}

#[test]
fn bare_input_reads_do_not_refresh_board_deadlines() {
    struct Idle;
    impl crate::board::BoardModel for Idle {
        fn name(&self) -> &'static str { "idle" }
        fn uses_gpio_edges(&self) -> bool { false }
        fn next_deadline(&self) -> Option<u64> { panic!("unexpected budget refresh"); }
    }
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board_edges = false;
    bus.tick_pending = 0;
    bus.board = Box::new(Idle);
    bus.read32(0x6000_403c).unwrap();
    bus.read32(0x6000_4040).unwrap();
}

#[test]
fn routed_output_edges_reach_pcnt_and_retain_queue_capacity() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(esp_soc::NoBoard);
    bus.attach_board_devices();
    bus.write32(0x6000_4024, 1 << 4).unwrap();
    bus.write32(0x6000_4154 + 33 * 4, 0x80 | 4).unwrap();
    bus.periph.pcnt.conf[0][0] = (1 << 16) | (1 << 18);
    for n in 1..=4 {
        bus.write32(0x6000_4008, 1 << 4).unwrap();
        assert_eq!(bus.periph.gpio.input_changes, [(4, true)]);
        let capacity = bus.periph.gpio.input_changes.capacity();
        bus.tick(1);
        bus.flush_ticks();
        assert_eq!(bus.periph.gpio.input_changes.capacity(), capacity);
        assert_eq!(bus.periph.pcnt.cnt[0], 2 * n - 1);
        bus.write32(0x6000_400c, 1 << 4).unwrap();
        bus.tick(1);
        bus.flush_ticks();
        assert_eq!(bus.periph.pcnt.cnt[0], 2 * n);
    }
}

#[test]
fn output_irq_cache_tracks_resolved_input_not_output_latch() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.gpio.write(0x74 + 4 * 4, (3 << 7) | (1 << 13));
    bus.write32(0x6000_4024, 1 << 4).unwrap();
    bus.periph.gpio.set_input(4, false);
    bus.irq_dirty = false;
    bus.write32(0x6000_4008, 1 << 4).unwrap();
    assert!(!bus.irq_dirty, "host drive masks the output transition");
    esp_soc::SocBus::gpio_release_input(&mut bus, 4);
    assert!(bus.irq_dirty);
}

#[test]
fn unrelated_gdma_channel_does_not_stage_rmt() {
    let mut bus = dma_bus();
    bus.periph.gdma.out[0].peri_sel = 9;
    bus.write32(FIRST_DESC, (1 << 31) | (1 << 30) | (4 << 12) | 4).unwrap();
    bus.write32(FIRST_DESC + 4, FIRST_DESC + 64).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    bus.write32(FIRST_DESC + 64, 0).unwrap();
    bus.write32(PERIPH_BASE + 0x3f074, u32::MAX).unwrap();
    assert!(bus.periph.gdma.out[0].running, "DMA_ACCESS_EN clear: GDMA writes do not stage");
    bus.periph.rmt.write(0x2c, 1 << 25);
    bus.write32(PERIPH_BASE + 0x3f000 + crate::periph::GDMA_CH_STRIDE + 0x74, u32::MAX).unwrap();
    bus.write32(0x6001_6028, 0).unwrap();
    assert_eq!(bus.periph.rmt.dma_fifo_len(), 0);
    assert!(bus.periph.gdma.out[0].running, "only CH3 CONF0 and the selected channel stage");
    bus.write32(PERIPH_BASE + 0x3f074, u32::MAX).unwrap();
    assert_eq!(bus.periph.rmt.dma_fifo_len(), 1);
    assert!(!bus.periph.gdma.out[0].running);
}

#[test]
fn rmt_dma_stages_symbols_and_returns_descriptors_without_a_tick_pump() {
    for auto in [0, 4] {
    let mut bus = dma_bus();
    let data = FIRST_DESC + 64;
    bus.periph.gdma.out[0].peri_sel = 9;
    bus.periph.gdma.out[0].conf0 = auto;
    bus.periph.gdma.out[0].conf1 = 1 << 12;
    bus.write32(FIRST_DESC, (1 << 31) | (8 << 12) | 8).unwrap();
    bus.write32(FIRST_DESC + 4, data).unwrap();
    bus.write32(FIRST_DESC + 8, FIRST_DESC + 16).unwrap();
    bus.write32(FIRST_DESC + 16, (1 << 31) | (1 << 30) | (4 << 12) | 4).unwrap();
    bus.write32(FIRST_DESC + 20, data + 8).unwrap();
    bus.write32(FIRST_DESC + 24, 0).unwrap();
    bus.write32(data, 0x8000 | 35 | (15 << 16)).unwrap();
    bus.write32(data + 4, 0x8000 | 10 | (40 << 16)).unwrap();
    bus.write32(data + 8, 0).unwrap();
    bus.write32(0x6001_602c, (1 << 25) | (2 << 8) | 1).unwrap();
    assert_eq!(bus.periph.rmt.dma_fifo_len(), 3);
    assert_eq!(bus.periph.gdma.out[0].int_raw & 15, 11);
    assert_eq!(bus.periph.gdma.out[0].eof_desc, FIRST_DESC + 16);
    assert!(!bus.periph.gdma.out[0].running);
    assert_eq!(bus.read32(FIRST_DESC).unwrap() >> 31, u32::from(auto == 0));
    bus.periph.rmt.tick(1);
    assert!(bus.periph.rmt.done.is_empty());
    bus.periph.rmt.tick(600);
    assert_eq!(bus.periph.rmt.done, vec![(3, vec![true, false])]);
    assert_eq!(bus.periph.rmt.int_raw & (1 << 3), 1 << 3);
    assert!(!bus.periph.rmt.is_running());
}
}

#[test]
fn rmt_dma_rejects_malformed_chains_and_bounds_cycles() {
    for case in 0..9 {
        let mut bus = dma_bus();
        let data = FIRST_DESC + 64;
        bus.periph.gdma.out[0].peri_sel = 9;
        bus.periph.gdma.out[0].conf1 = 1 << 12;
        let word = match case { 0 => 4 | (4 << 12), 1 => (1 << 31) | 3 | (4 << 12),
            2 => (1 << 31) | 3 | (3 << 12), 5 => 1 << 31, _ => (1 << 31) | 4 | (4 << 12) };
        bus.write32(FIRST_DESC, word).unwrap();
        bus.write32(FIRST_DESC + 4, match case { 3 => data + 1, 4 => 0xffff_fffc, _ => data }).unwrap();
        bus.write32(FIRST_DESC + 8, if case == 5 { FIRST_DESC } else { 0 }).unwrap();
        if case == 6 { bus.periph.gdma.out[0].desc = 0x6001_602c; }
        if case == 7 { bus.periph.rmt.dma_fifo().resize(24 * 4096 + 1, 1); }
        if case == 8 {
            // OUT_CONF0/CONF1/INT_RAW read as an owned EOF descriptor with a RAM buffer.
            let c = &mut bus.periph.gdma.out[0];
            c.desc = 0x6003_f060; c.conf0 = (3 << 30) | (4 << 12) | 4; c.conf1 = 0x3fc9_1140;
        }
        bus.write32(0x6001_602c, 1 << 25).unwrap();
        assert!(!bus.periph.gdma.out[0].running, "case {case}");
        assert_eq!(bus.periph.gdma.out[0].int_raw & 4, 4, "case {case}");
        assert_eq!(bus.periph.rmt.int_raw & (1 << 28), 1 << 28);
        assert_eq!(bus.periph.rmt.dma_fifo_len(), 0);
    }
}

/// IDF v5.5.5 esp_driver_rmt/src/rmt_tx.c DMA mode: two ring nodes, SUC_EOF on both,
/// owner check and automatic write-back (rmt_tx.c:42-46, 79-95), refilled from the OUT_EOF
/// callback and continued with gdma_append (1105-1132); the last node's link is cut
/// (rmt_tx_mark_eof, 602-649). Two ping-pong halves of two symbols carry a four-half frame.
#[test]
fn rmt_dma_follows_the_idf_ping_pong_ring() {
    const OUT_LINK: u32 = 0x6003_f080;
    const OUT_INT_CLR: u32 = 0x6003_f074;
    let (n0, n1) = (FIRST_DESC, FIRST_DESC + 16);
    let (b0, b1) = (FIRST_DESC + 64, FIRST_DESC + 72);
    let one = 0x8000 | 35 | (15 << 16);
    let zero = 0x8000 | 10 | (40 << 16);
    let mut bus = dma_bus();
    let fill = |bus: &mut SocBus, node: u32, buf: u32, symbols: [u32; 2], next: u32| {
        bus.write32(buf, symbols[0]).unwrap();
        bus.write32(buf + 4, symbols[1]).unwrap();
        bus.write32(node, (1 << 31) | (1 << 30) | (8 << 12) | 8).unwrap();
        bus.write32(node + 4, buf).unwrap();
        bus.write32(node + 8, next).unwrap();
    };
    let c = &mut bus.periph.gdma.out[0];
    c.running = false; c.peri_sel = 9; c.conf0 = 1 << 2; c.conf1 = 1 << 12; c.int_ena = 1 << 1;
    bus.write32(0x6001_602c, 1 << 25).unwrap();        // rmt_ll_tx_enable_dma
    fill(&mut bus, n0, b0, [one, zero], n1);
    fill(&mut bus, n1, b1, [one, one], n0);
    bus.write32(OUT_LINK, (n0 & 0xf_ffff) | (1 << 21)).unwrap();   // gdma_start
    assert_eq!((bus.periph.rmt.dma_fifo_len(), bus.periph.gdma.out[0].eof_desc), (2, n0));
    bus.write32(0x6001_602c, (1 << 25) | 1).unwrap();             // tx_start
    assert_eq!(bus.periph.rmt.dma_fifo_len(), 4, "the second node is ready too");
    let mut halves = [([zero, zero], n1), ([one, 0], 0)].into_iter();
    let mut refill = n0;
    while bus.periph.gdma.out[0].running {
        bus.write32(OUT_INT_CLR, 1 << 1).unwrap();                 // GDMA ISR clears, then the callback
        let Some((symbols, next)) = halves.next() else { break };
        assert_eq!(bus.read32(refill).unwrap() >> 31, 0, "consumed node returned to the CPU");
        let buf = if refill == n0 { b0 } else { b1 };
        fill(&mut bus, refill, buf, symbols, next);
        bus.write32(OUT_LINK, (n0 & 0xf_ffff) | (1 << 22)).unwrap(); // gdma_append: RESTART
        refill = if refill == n0 { n1 } else { n0 };
    }
    bus.write32(OUT_INT_CLR, 1 << 1).unwrap();
    assert!(!bus.periph.gdma.out[0].running);
    assert_eq!(bus.periph.gdma.out[0].int_raw & (1 << 2), 0, "no descriptor error");
    assert_eq!(bus.periph.rmt.dma_fifo_len(), 8);
    bus.periph.rmt.tick(10_000);
    assert_eq!(bus.periph.rmt.done, vec![(3, vec![true, false, true, true, false, false, true])]);
}

#[test]
fn rmt_dma_fresh_start_discards_ring_resume_with_or_without_reset() {
    const OUT_CONF0: u32 = 0x6003_f060;
    const OUT_LINK: u32 = 0x6003_f080;
    let (n0, n1, b0, b1) = (FIRST_DESC, FIRST_DESC + 16, FIRST_DESC + 64, FIRST_DESC + 68);
    let control = (1 << 31) | (1 << 30) | (4 << 12) | 4;
    for reset in [true, false] {
        let mut bus = dma_bus();
        for (node, buf, next, symbol) in [(n0, b0, n1, 0x8000 | 35 | (15 << 16)), (n1, b1, n0, 0x8000 | 10 | (40 << 16))] {
            bus.write32(node, control).unwrap();
            bus.write32(node + 4, buf).unwrap();
            bus.write32(node + 8, next).unwrap();
            bus.write32(buf, symbol).unwrap();
        }
        let c = &mut bus.periph.gdma.out[0];
        c.running = false; c.peri_sel = 9; c.conf0 = 1 << 2; c.conf1 = 1 << 12;
        bus.write32(0x6001_602c, 1 << 25).unwrap();
        bus.write32(OUT_LINK, (n0 & 0xf_ffff) | (1 << 21)).unwrap();
        assert_eq!(bus.periph.gdma.out[0].eof_desc, n0);
        assert_eq!(bus.periph.rmt.dma_resume(), n1);
        // Hand the first node back to DMA and relink it without disabling RMT DMA.
        bus.write32(n0, control).unwrap();
        if reset {
            bus.write32(OUT_CONF0, (1 << 2) | 1).unwrap();
            assert_eq!(bus.periph.rmt.dma_resume(), 0, "OUT_RST clears continuation even before START");
        }
        bus.write32(OUT_LINK, (n0 & 0xf_ffff) | (1 << 21)).unwrap();
        assert_eq!(bus.periph.gdma.out[0].eof_desc, n0, "START uses its new link, not the stale resume node");
        assert_eq!(bus.periph.rmt.dma_resume(), n1);
        assert_eq!(bus.periph.rmt.dma_fifo_len(), 2);
        assert!(bus.periph.rmt.dma_fifo().iter().all(|&symbol| symbol == 0x8000 | 35 | (15 << 16)));
    }
}

#[test]
fn rmt_dma_ring_waits_for_a_cpu_owned_node_and_resumes_after_restart() {
    const OUT_LINK: u32 = 0x6003_f080;
    let (n0, n1, buf) = (FIRST_DESC, FIRST_DESC + 16, FIRST_DESC + 64);
    let mut bus = dma_bus();
    for (node, control, next) in [(n0, (1 << 31) | (1 << 30) | (4 << 12) | 4, n1), (n1, (1 << 30) | (4 << 12) | 4, n0)] {
        bus.write32(node, control).unwrap();
        bus.write32(node + 4, buf).unwrap();
        bus.write32(node + 8, next).unwrap();
    }
    bus.write32(buf, 0x8000 | 35 | (15 << 16)).unwrap();
    let c = &mut bus.periph.gdma.out[0];
    c.running = false; c.peri_sel = 9; c.conf0 = 1 << 2; c.conf1 = 1 << 12;
    bus.write32(0x6001_602c, 1 << 25).unwrap();
    bus.write32(OUT_LINK, (n0 & 0xf_ffff) | (1 << 21)).unwrap();
    assert_eq!(bus.periph.rmt.dma_fifo_len(), 1);
    // Node 1 still belongs to the CPU: the channel waits without a descriptor error.
    bus.write32(OUT_LINK, (n0 & 0xf_ffff) | (1 << 22)).unwrap();
    assert!(bus.periph.gdma.out[0].running);
    assert_eq!((bus.periph.gdma.out[0].int_raw & (1 << 2), bus.periph.rmt.dma_fifo_len()), (0, 1));
    bus.write32(n1, (1 << 31) | (1 << 30) | (4 << 12) | 4).unwrap();
    bus.write32(n1 + 8, 0).unwrap();
    bus.write32(OUT_LINK, (n0 & 0xf_ffff) | (1 << 22)).unwrap();
    assert_eq!(bus.periph.rmt.dma_fifo_len(), 2);
    assert_eq!(bus.periph.gdma.out[0].eof_desc, n1);
    assert!(!bus.periph.gdma.out[0].running);
}

// ------------------------------------------------------------------ SPI3

const SPI3: u32 = 0x6002_5000;

/// SPI3 is its own GP-SPI block at DR_REG_SPI3_BASE (IDF v5.5.5
/// components/soc/esp32s3/register/soc/reg_base.h:39) on ETS_SPI3_INTR_SOURCE 22
/// (components/soc/esp32s3/include/soc/interrupts.h:41). A CPU-FIFO transfer, as the Arduino SPI
/// driver makes it for `SPIClass(HSPI)` (arduino-esp32 3.3.12 cores/esp32/esp32-hal-spi.h:36,
/// esp32-hal-spi.c:593: HSPI is SPI3_HOST on the S3), reaches the board as host 3, raises
/// TRANS_DONE on SPI3's source and leaves SPI2 alone.
#[test]
fn spi3_cpu_transfer_reaches_the_board_as_host_3_on_its_own_source() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    bus.write32(SPI3 + 0x34, 1 << 12).unwrap();                         // TRANS_DONE enabled
    bus.write32(SPI3 + 0x10, (1 << 27) | (1 << 28) | 1).unwrap();       // MOSI + MISO, full duplex
    bus.write32(SPI3 + 0x1c, 15).unwrap();
    bus.write32(SPI3 + 0x98, 0x3412).unwrap();
    bus.write32(SPI3, 1 << 24).unwrap();
    assert_eq!(&*events.lock().unwrap(), &["spi:3:[12, 34]:2"]);
    assert_eq!(bus.read32(SPI3).unwrap() & (1 << 24), 0, "USR done");
    assert_eq!(bus.read32(SPI3 + 0x98).unwrap() & 0xffff, 0x5150, "MISO in W0");
    assert_eq!((bus.periph.spi3.transfers, bus.periph.spi2.transfers), (1, 0));
    let status = bus.periph.source_status();
    let on = |src: usize| status[src / 32] & (1 << (src % 32)) != 0;
    assert!(on(crate::periph::SRC_SPI3) && !on(crate::periph::SRC_SPI2));
    bus.write32(SPI3 + 0x38, 1 << 12).unwrap();
    assert!(!bus.periph.spi3.irq());
}

/// SPI3's DMA data phase comes from the GDMA OUT channel bound to trigger 1
/// (SOC_GDMA_TRIG_PERIPH_SPI3, IDF v5.5.5 components/soc/esp32s3/include/soc/gdma_channel.h:12);
/// a channel bound to SPI2 (trigger 0) does not feed it.
#[test]
fn spi3_dma_reads_the_gdma_channel_of_trigger_1() {
    const DATA: u32 = 0x3fc9_0200;
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    bus.write32(DATA, 0x4433_2211).unwrap();
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30) | (1 << 31)).unwrap();
    bus.write32(FIRST_DESC + 4, DATA).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    bus.periph.gdma.out[0].peri_sel = 0;                                // SPI2's channel, idle
    bus.periph.gdma.out[1].peri_sel = 1;
    bus.periph.gdma.out[1].desc = FIRST_DESC;
    bus.periph.gdma.out[1].running = true;
    bus.write32(SPI3 + 0x30, 1 << 28).unwrap();
    bus.write32(SPI3 + 0x10, 1 << 27).unwrap();
    bus.write32(SPI3 + 0x1c, 31).unwrap();
    bus.write32(SPI3, 1 << 24).unwrap();
    assert_eq!(&*events.lock().unwrap(), &["spi:3:[11, 22, 33, 44]:0"]);
    assert_eq!(bus.periph.spi3.transfers, 1);
    assert!(bus.periph.spi3.dma_tx_pending.is_none());
    assert_ne!(bus.periph.gdma.out[1].int_raw & (1 << 1), 0, "OUT_EOF on SPI3's channel");
    assert_eq!(bus.spi3_dma_fault, None);
}

/// A pin-aware board sees SPI3's routes through the matrix (IDF v5.5.5
/// components/soc/esp32s3/include/soc/gpio_sig_map.h:136-147, :226): SPI3_CLK 66, SPI3_D 68,
/// SPI3_Q 67 in, SPI3_CS0 71; LEDC_LS_SIG_OUT0 (73, right after SPI3_CS1 72) is no select, and
/// SPI2 is routed nowhere.
#[test]
fn spi3_routes_come_from_its_matrix_signals() {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    for (pin, signal) in [(6u32, 66u32), (5, 68), (7, 71), (15, 73)] {
        bus.write32(0x6000_4554 + 4 * pin, signal).unwrap();            // FUNC_OUT_SEL_CFG
        bus.write32(0x6000_9004 + 4 * pin, (1 << 12) | (1 << 9)).unwrap();   // IO_MUX: GPIO function, input enable
    }
    bus.write32(0x6000_4154 + 4 * 67, 0x80 | 4).unwrap();               // FUNC_IN_SEL: SPI3_Q from GPIO4
    bus.write32(0x6000_9004 + 4 * 4, (1 << 12) | (1 << 9)).unwrap();
    bus.write32(SPI3 + 0x10, (1 << 27) | (1 << 28)).unwrap();
    bus.write32(SPI3 + 0x20, 0x3e).unwrap();                            // CS0 only
    let pins = bus.periph.spi3_pins();
    assert_eq!((pins.sclk, pins.mosi, pins.miso, pins.cs), (1 << 6, 1 << 5, Some(4), 1 << 7));
    assert_eq!(bus.periph.spi2_pins().sclk, 0, "SPI2 is routed nowhere");
}

/// A descriptor fault on SPI3's DMA is recorded in `spi3_dma_fault` and cleared by the next USR
/// write, as SPI2's is (`assert_dma_fault_and_recovery`): a CPU-FIFO retry reaches the board as host 3.
#[test]
fn spi3_dma_fault_is_cleared_by_the_next_transaction() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(ProbeBoard { events: events.clone() });
    bus.write32(FIRST_DESC, 4 | (4 << 12) | (1 << 30)).unwrap();        // owner bit clear: not the DMA's
    bus.write32(FIRST_DESC + 4, 0x3fc9_0200).unwrap();
    bus.write32(FIRST_DESC + 8, 0).unwrap();
    bus.periph.gdma.out[1].peri_sel = 1;
    bus.periph.gdma.out[1].desc = FIRST_DESC;
    bus.periph.gdma.out[1].running = true;
    bus.write32(SPI3 + 0x30, 1 << 28).unwrap();                         // DMA TX
    bus.write32(SPI3 + 0x10, 1 << 27).unwrap();
    bus.write32(SPI3 + 0x1c, 31).unwrap();
    bus.write32(SPI3, 1 << 24).unwrap();
    assert_eq!(bus.spi3_dma_fault, Some(DmaDescriptorFault::NotOwned { descriptor: FIRST_DESC }));
    assert_eq!(bus.periph.spi3.transfers, 0);
    assert!(events.lock().unwrap().is_empty());
    bus.write32(SPI3 + 0x30, 0).unwrap();                               // CPU mode
    bus.write32(SPI3 + 0x1c, 7).unwrap();
    bus.write32(SPI3 + 0x98, 0xa5).unwrap();
    bus.write32(SPI3, 1 << 24).unwrap();
    assert_eq!(bus.spi3_dma_fault, None);
    assert_eq!(bus.periph.spi3.transfers, 1);
    assert_eq!(&*events.lock().unwrap(), &["spi:3:[a5]:0"]);
}

/// A frame from a network around reaches the RX descriptor with that network's channel and RSSI
/// (`VirtualAp::rx_meta`): `wifi_pkt_rx_ctrl_t`'s `rssi` in word 0's low byte, `channel` and
/// `secondary_channel` in word 2 bits 16-23 (IDF v5.5.5
/// components/esp_wifi/include/local/esp_wifi_types_native.h:36, :59-60); a frame from the access
/// point keeps its channel and -40 dBm.
#[test]
fn wifi_rx_ctrl_carries_a_nearby_networks_channel_and_rssi() {
    use esp_soc::wifi::{ApConfig, BeaconAp, VirtualAp};
    const DESC: u32 = 0x3fc9_0100;
    const BUF: u32 = 0x3fc9_0200;
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.periph.wifi.link.attach(Some(VirtualAp::new(ApConfig::parse("ssid=demo,chan=6").unwrap(), false)), None);
    let home = [0x02, 1, 2, 3, 4, 5];
    let left_out = bus.periph.wifi.link.set_beacons(vec![BeaconAp { ssid: "Home".into(), bssid: home, channel: 1, rssi: -60, secure: true }]);
    assert_eq!(left_out, Some(vec![]));
    let ap = bus.periph.wifi.link.ap().unwrap().cfg.bssid;
    for (from, chan, rssi) in [(home, 1u32, -60i8), (ap, 6, -40)] {
        bus.write32(DESC, (1 << 31) | 0xfff).unwrap();                     // owner DMA, size 4095
        bus.write32(DESC + 4, BUF).unwrap();
        bus.write32(DESC + 8, 0).unwrap();
        bus.periph.wifi.rx_next = DESC & 0xf_ffff;
        let mut frame = vec![0u8; 24];
        frame[0] = 0x80;                                                     // beacon
        frame[4..10].fill(0xff);
        frame[10..16].copy_from_slice(&from);
        bus.wifi_rx_deliver(&frame, 1234);
        assert_eq!(bus.read32(BUF).unwrap() & 0xff, u32::from(rssi as u8), "RSSI of {from:02x?}");
        assert_eq!(bus.read32(BUF + 8).unwrap(), (chan << 16) | (chan << 20), "channel of {from:02x?}");
    }
}

/// EX254: a register write first delivers the deferred device time, then leaves the tick
/// budget its own post-write state derives (the flush inside the write no longer refreshes
/// it; the write's closing refresh does). The board deadline is absolute, so the budget also
/// shows that the flushed time was counted exactly once.
#[test]
fn mmio_write_delivers_pending_time_and_leaves_the_post_write_budget() {
    const USB_INT_ENA: u32 = 0x6003_8010;
    for (pending, deadline) in [(37u32, 5_000u64), (300, 400), (100, 40_000)] {
        let mut bus = SocBus::new(1024, 1024, [0; 6]);
        bus.board = Box::new(FixedDeadlineBoard { deadline });
        bus.refresh_tick_budget();
        let expect = |bus: &SocBus, cap: u32| (deadline - bus.cycles).clamp(1, u64::from(cap)) as u32;
        assert_eq!(bus.tick_budget, expect(&bus, QUIET_TICK_DEFER));
        for (on, cap, step) in [(2, MAX_TICK_DEFER, pending), (0, QUIET_TICK_DEFER, 1)] {
            let before = bus.cycles;
            assert_eq!(Bus::tick(&mut bus, step), 0, "{pending}/{deadline}: stays deferred");
            assert_eq!((bus.tick_pending, bus.periph.clock_cycles()), (step, before));
            bus.write32(USB_INT_ENA, on).unwrap();                          // SOF on: active cadence
            assert_eq!(bus.tick_pending, 0, "{pending}/{deadline}: the write flushed");
            assert_eq!(bus.periph.clock_cycles(), before + u64::from(step), "devices saw the time once");
            assert_eq!(bus.tick_budget, expect(&bus, cap), "{pending}/{deadline} int_ena={on}");
            assert_eq!(bus.next_deadline(), u64::from(expect(&bus, cap)));
            let got = bus.tick_budget;
            bus.refresh_tick_budget();
            assert_eq!(bus.tick_budget, got, "the write left a fresh derivation");
        }
    }
    // The pending time reaches the devices before the written value: the SOF due at the
    // flushed cycle is raised first, so the INT_CLR write clears it.
    for (reg, value, raw) in [(0x6003_8014u32, 2u32, 0u32), (0x6003_8010, 2, 2)] {
        let mut bus = SocBus::new(1024, 1024, [0; 6]);
        bus.tick_budget = MAX_TICK_DEFER;
        bus.periph.usb.tick(crate::periph::CPU_HZ / 4000 - 1);
        assert_eq!(Bus::tick(&mut bus, 1), 0);
        assert_eq!(bus.periph.usb.int_raw & 2, 0, "SOF still deferred");
        bus.write32(reg, value).unwrap();
        assert_eq!(bus.periph.usb.int_raw & 2, raw, "write {reg:#x}={value}");
    }
}
