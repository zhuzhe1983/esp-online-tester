//! The P4's interrupt and hart behaviour through `Machine`, with small programs in L2MEM and the
//! mask ROM's place, no firmware or ROM ELF.
use emu_core::{Bus, Core, CostModel, ExecutionFacts, LifecycleFacts};
use esp32p4::bus::{L2MEM_LOW, ROM_LOW};
use esp32p4::periph::src;
use esp_soc::SocBus;
use riscv_rv32::state::{csr, mstatus};

/// objdump prints each RISC-V instruction as one little-endian word (or half-word) in hex.
fn asm(listing: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for g in listing.split_whitespace() { let w = u32::from_str_radix(g, 16).unwrap(); out.extend_from_slice(&w.to_le_bytes()[..g.len() / 2]); }
    out
}

const PROG: u32 = L2MEM_LOW + 0x1000;
const BASE: u32 = L2MEM_LOW + 0x2000;
const TABLE: u32 = L2MEM_LOW + 0x3000;
const CTRL: u32 = 0x2080_1000;
const THRESH: u32 = 0x2080_0008;
const FROM_CPU: u32 = 0x500E_5010;

/// Core 0 spinning on `j .` at PROG in CLIC mode with MIE set; every vector-table entry and the
/// base hold a `j .` of their own, so the pc says what was taken.
fn clic_machine() -> esp32p4::Machine {
    let mut m = esp32p4::machine([0x30, 0xed, 0xa0, 0x00, 0x11, 0x22], 0x1000);
    m.bus.load_bytes(PROG, &asm("a001")).unwrap();
    m.bus.load_bytes(BASE, &asm("a001")).unwrap();
    for id in 0..48u32 {
        let handler = BASE + 0x100 + 4 * id;
        m.bus.load_bytes(handler, &asm("a001")).unwrap();
        m.bus.load_bytes(TABLE + 4 * id, &handler.to_le_bytes()).unwrap();
    }
    m.boot_rom();
    let c = &mut m.cores[0];
    c.pc = PROG;
    c.write_csr(csr::MTVEC, BASE | 3);
    c.write_csr(csr::MTVT, TABLE);
    c.write_csr(csr::MSTATUS, c.mstatus | mstatus::MIE);
    m
}

/// Route FROM_CPU_INTR`n` to CLIC ID `id` at `level` (1-7), hardware-vectored or not, edge or level.
fn route(m: &mut esp32p4::Machine, n: u32, id: u32, level: u32, shv: bool, edge: bool) {
    m.bus.write32(0x500D_6000 + 4 * (src::FROM_CPU0 as u32 + n), id).unwrap();
    m.bus.write32(CTRL + 4 * id, 1 << 8 | (shv as u32) << 16 | (edge as u32) << 17 | level << 29).unwrap();
}

/// The take rule through the machine: a candidate at or below the CLIC_INT_THRESH level waits,
/// the threshold write that lowers it lets it in at once, MIE alone keeps a higher one out, and
/// with MIE set it nests above the running level (mintstatus) but an equal one does not.
#[test]
fn the_take_rule_is_level_above_threshold_and_mil_with_mie() {
    let mut m = clic_machine();
    route(&mut m, 0, 17, 2, true, false);
    m.bus.write32(THRESH, 0x5f << 24).unwrap();                       // masks levels <= 2
    m.bus.write32(FROM_CPU, 1).unwrap();
    m.run(200);
    assert_eq!((m.interrupts, m.cores[0].pc()), (0, PROG));
    m.bus.write32(THRESH, 0x3f << 24).unwrap();
    m.run(64);
    assert_eq!((m.interrupts, m.cores[0].pc()), (1, BASE + 0x100 + 4 * 17));
    assert_eq!(m.cores[0].read_csr(csr::MINTSTATUS_P4), 0x5f << 24);
    route(&mut m, 1, 18, 2, true, false);                              // the same level
    route(&mut m, 2, 19, 5, false, false);                             // higher, not vectored
    m.bus.write32(FROM_CPU + 4, 1).unwrap();
    m.bus.write32(FROM_CPU + 8, 1).unwrap();
    m.run(64);
    assert_eq!(m.interrupts, 1, "MIE is clear in the handler");
    let c = &mut m.cores[0];
    c.write_csr(csr::MSTATUS, c.mstatus | mstatus::MIE);
    m.run(64);
    assert_eq!((m.interrupts, m.cores[0].pc()), (2, BASE), "the level-5 one nests and enters at the base");
    assert_eq!(m.cores[0].read_csr(csr::MCAUSE) & 0x80ff_0fff, 0x8000_0000 | 0x5f << 16 | 19);
    assert_eq!(m.cores[0].read_csr(csr::MINTSTATUS_P4) >> 24, 0xbf);
    let c = &mut m.cores[0];
    c.write_csr(csr::MSTATUS, c.mstatus | mstatus::MIE);
    m.run(64);
    assert_eq!(m.interrupts, 2, "level 2 does not preempt level 5");
}

/// A hardware-vectored edge interrupt is no longer pending once a core takes it; a level one
/// stays pending until its source drops.
#[test]
fn taking_a_vectored_edge_interrupt_clears_it() {
    let mut m = clic_machine();
    route(&mut m, 0, 20, 1, true, true);
    m.bus.write32(FROM_CPU, 1).unwrap();
    m.bus.refresh_irq();
    m.bus.write32(FROM_CPU, 0).unwrap();                               // a pulse
    m.bus.refresh_irq();
    assert!(m.bus.periph.clic.core[0].pending(20));
    m.run(64);
    assert_eq!(m.interrupts, 1);
    assert!(!m.bus.periph.clic.core[0].pending(20));
    assert_eq!(m.cores[0].pc(), BASE + 0x100 + 4 * 20);
}

/// The ROM's mode 1 (UART download mode waits for its UART interrupt there): a presented CLIC interrupt is taken
/// at the mtvec base, not vectored, with the CLIC ID as mcause (the ROM's dispatcher subtracts 16). The level in
/// service masks its own level, so the handler setting MIE again does not re-enter it, and `mret` restores it.
#[test]
fn clint_mode_takes_clic_interrupts_at_the_base_by_level() {
    let mut m = clic_machine();
    m.cores[0].write_csr(csr::MTVEC, BASE | 1);
    route(&mut m, 0, 17, 1, true, false);
    m.bus.write32(FROM_CPU, 1).unwrap();
    m.run(64);
    assert_eq!((m.interrupts, m.cores[0].pc()), (1, BASE));
    assert_eq!(m.cores[0].read_csr(csr::MCAUSE), 0x8000_0000 | 17);
    let c = &mut m.cores[0];
    c.write_csr(csr::MSTATUS, c.mstatus | mstatus::MIE);             // the ROM's handler does this at once
    m.run(64);
    assert_eq!(m.interrupts, 1, "the level in service masks itself");
    route(&mut m, 1, 18, 5, false, false);                           // a higher level nests
    m.bus.write32(FROM_CPU + 4, 1).unwrap();
    m.run(64);
    assert_eq!((m.interrupts, m.cores[0].pc()), (2, BASE));
    assert_eq!(m.cores[0].read_csr(csr::MCAUSE), 0x8000_0000 | 18, "nested: still the plain ID, no previous level in it");
}

/// Every instruction costs one cycle, so the modeled scheduler interleaves the cores one
/// instruction at a time.
struct OneCycle;
impl CostModel for OneCycle {
    fn lifecycle(&mut self, _facts: &LifecycleFacts) -> Result<(), String> { Ok(()) }
    fn cycles(&mut self, _facts: &ExecutionFacts<'_>) -> Result<u32, String> { Ok(1) }
}

/// Both harts start at the reset vector; hart 0 runs lr.w, two nops and sc.w on a word of L2MEM,
/// hart 1 stores to the same word in between. Through the modeled path (`RecordingBus`) the
/// store breaks hart 0's reservation and its SC fails; with core 1 held in reset it succeeds.
#[test]
fn a_store_from_the_other_hart_fails_the_sc_through_the_modeled_path() {
    // csrr t0,mhartid; bnez t0,c1; lui a1,0x4ff40; lr.w a0,(a1); nop; nop; sc.w a2,a3,(a1); j .
    // c1: lui a1,0x4ff40; sw a3,0(a1); j .
    let prog = asm("f14022f3 00029b63 4ff405b7 1005a52f 0001 0001 18d5a62f a001 4ff405b7 c194 a001");
    for release in [true, false] {
        let mut m = esp32p4::machine([0x30, 0xed, 0xa0, 0x00, 0x11, 0x22], 0x1000);
        m.bus.load_bytes(ROM_LOW, &prog).unwrap();
        m.set_cost_model(Box::new(OneCycle)).unwrap();
        m.boot_rom();
        if release {
            let clk = m.bus.read32(0x500E_6014).unwrap();
            m.bus.write32(0x500E_6014, clk | 1 << 4).unwrap();                                   // CORE1_CPU_CLK_EN
            m.bus.write32(0x500E_60C0, 0).unwrap();                                              // RST_EN_CORE1_GLOBAL off
        }
        m.run(40);
        assert_eq!(m.cores[0].pc(), ROM_LOW + 0x18, "hart 0 is parked after its sc.w");
        assert_eq!(m.cores[0].x[12], release as u32, "SC result with core 1 {}", if release { "running" } else { "held" });
        if release { assert_eq!(m.cores[1].pc(), ROM_LOW + 0x20); }
    }
}
