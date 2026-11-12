//! ESP32-S3 TRM v1.8 §1.8.37, p113: EE.LDXQ.32 indexed word load.
//! Independent bit layout and result assertions, including the EDCBook word.
use xtensa_lx7::{decode, pie, pie_timing, step, Bus, Cpu, Fault, FlatRam, Op, Trap};
use xtensa_lx7::state::exc;

const PC: u32 = 0x4037_0000;
const DATA: u32 = 0x3fc9_0000;

fn word(qu: u32, qs: u32, as_: u32, sel4: u32, sel8: u32) -> u32 {
    0xe070_0d0e | qu << 16 | (qs & 3) << 14 | qs >> 2 | as_ << 4
        | (sel4 & 1) << 19 | (sel4 >> 1) << 24 | (sel8 & 1) << 23 | (sel8 >> 1) << 12
}

struct ReadWord { address: u32, value: u32, fail: bool, reads: Vec<u32> }
impl ReadWord { fn new(address: u32, value: u32) -> Self { Self { address, value, fail: false, reads: Vec::new() } } }
impl Bus for ReadWord {
    fn note_code_page(&mut self, _: u32) {}
    fn read8(&mut self, _: u32) -> Result<u8, Fault> { panic!("LDXQ must use one word access") }
    fn read16(&mut self, _: u32) -> Result<u16, Fault> { panic!("LDXQ must use one word access") }
    fn read32(&mut self, a: u32) -> Result<u32, Fault> { self.reads.push(a); assert_eq!(a, self.address); if self.fail { Err(Fault::Unmapped) } else { Ok(self.value) } }
    fn write8(&mut self, _: u32, _: u8) -> Result<(), Fault> { panic!("LDXQ must not store") }
    fn write16(&mut self, _: u32, _: u16) -> Result<(), Fault> { panic!("LDXQ must not store") }
    fn write32(&mut self, _: u32, _: u32) -> Result<(), Fault> { panic!("LDXQ must not store") }
    fn fetch(&mut self, _: u32) -> Result<[u8; 4], Fault> { panic!("Direct PIE execution must not fetch") }
}
fn cpu() -> Cpu { let mut c = Cpu::new(0); c.pc = PC; c.ps = 0; c.cpenable = 8; c }

#[test]
fn actual_edcbook_encoding_has_two_distinct_selectors_and_advances_four_bytes() {
    let raw = 0xe072_3d4e_u32;
    assert_eq!(word(2, 0, 4, 0, 6), raw);
    let insn = decode(PC, raw.to_le_bytes());
    assert_eq!((insn.op, insn.len, insn.imm2), (Op::Pie, 4, 4));
    assert_eq!(pie::format(raw, insn.imm as usize), "ee.ldxq.32\tq2, q0, a4, 0, 6");
    let (mut c, mut ram) = (cpu(), FlatRam::new(PC, 0x100));
    c.qr[0] = 1 << 96; c.qr[2] = u128::MAX; c.set_ar(4, PC + 0x40);
    ram.mem[..4].copy_from_slice(&raw.to_le_bytes()); ram.mem[0x44..0x48].copy_from_slice(&0x1234_5678_u32.to_le_bytes());
    step(&mut c, &mut ram).unwrap();
    assert_eq!(c.pc, PC + 4); assert_eq!(c.qr[2], (u128::MAX << 32) | 0x1234_5678);
}

#[test]
fn every_selector_and_q_register_preserves_other_lanes_including_aliases() {
    let indices = [0_u16, 1, 3, 31, 0x7fff, 0x8000, 0xfffe, 0xffff];
    let source = indices.iter().enumerate().fold(0, |v, (n, x)| v | u128::from(*x) << (16 * n));
    for qu in 0..8 { for qs in 0..8 { for sel4 in 0..4 { for sel8 in 0..8 { for low in 0..4 {
        let raw = word(qu, qs, 15, sel4, sel8); let insn = decode(PC, raw.to_le_bytes());
        let mut c = cpu(); c.qr = [0x0123_4567_89ab_cdef_fedc_ba98_7654_3210; 8]; c.qr[qs as usize] = source;
        c.set_ar(15, DATA + low); c.sar_byte = 11; c.accx = [0x1234, 0x56];
        let before = c.qr; let offset = u32::from(indices[sel8 as usize]) * 4; let value = 0x8100_0000 | offset;
        let mut bus = ReadWord::new(DATA + offset, value);
        pie::exec(&mut c, &mut bus, &insn).unwrap();
        let mut expected = before; let shift = 32 * sel4;
        expected[qu as usize] = (before[qu as usize] & !(u128::from(u32::MAX) << shift)) | u128::from(value) << shift;
        assert_eq!(c.qr, expected, "q{qu} q{qs} sel4={sel4} sel8={sel8} low={low}");
        assert_eq!(bus.reads, [DATA + offset]); assert_eq!(c.get_ar(15), DATA + low);
        assert_eq!((c.sar_byte, c.accx), (11, [0x1234, 0x56])); assert_eq!(insn.imm2, 15);
        let effects = pie_timing::insn_effects(&insn);
        assert_eq!((effects.reads, effects.writes, effects.delayed), ((1 << qs) | (1 << qu), 1 << qu, 0));
    } } } } }
}

#[test]
fn index_lane_three_excludes_adjacent_bit_47_and_address_addition_wraps() {
    // TRM's [63:47] operation line conflicts with its eight 16-bit segments;
    // lane 3 is bits [63:48]. The neighboring high bit must not become an index.
    let mut c = cpu(); c.qr[0] = 1 << 47; c.set_ar(4, DATA + 3);
    let mut bus = ReadWord::new(DATA, 0x1122_3344);
    pie::exec(&mut c, &mut bus, &decode(PC, word(2, 0, 4, 0, 3).to_le_bytes())).unwrap();
    c.qr[0] = 8; c.set_ar(4, 0xffff_fff3); let mut bus = ReadWord::new(0x10, 0xaabb_ccdd);
    pie::exec(&mut c, &mut bus, &decode(PC, word(2, 0, 4, 3, 0).to_le_bytes())).unwrap();
    assert_eq!((c.qr[2] >> 96) as u32, 0xaabb_ccdd); assert_eq!(c.get_ar(4), 0xffff_fff3);
}

#[test]
fn fault_keeps_aliased_operands_for_retry_and_reports_aligned_address() {
    let mut c = cpu(); c.qr[2] = u128::MAX; c.set_ar(4, DATA + 3);
    let before = c.qr; let mut bus = ReadWord::new(DATA + 0x3fffc, 0x1234_5678); bus.fail = true;
    let insn = decode(PC, word(2, 2, 4, 3, 6).to_le_bytes());
    assert_eq!(pie::exec(&mut c, &mut bus, &insn), Err(Trap::Exception(exc::LOAD_PROHIBITED)));
    assert_eq!(c.excvaddr, bus.address); assert_eq!(c.epc[1], PC); assert_eq!(c.qr, before); assert_eq!(c.get_ar(4), DATA + 3);
    bus.fail = false; c.pc = PC; c.ps = 0;
    pie::exec(&mut c, &mut bus, &insn).unwrap();
    assert_eq!(c.qr[2], (u128::MAX >> 32) | (0x1234_5678_u128 << 96));
}

#[test]
fn disabled_cp3_traps_before_any_memory_or_q_register_change() {
    let mut c = cpu(); c.cpenable = 0; c.qr = [u128::MAX; 8]; let before = c.qr;
    let mut bus = ReadWord::new(0, 0);
    assert_eq!(pie::exec(&mut c, &mut bus, &decode(PC, 0xe072_3d4e_u32.to_le_bytes())), Err(Trap::Exception(exc::COPROCESSOR0_DISABLED + 3)));
    assert!(bus.reads.is_empty()); assert_eq!(c.qr, before);
}
