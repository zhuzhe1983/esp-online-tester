use emu_core::Bus;

// One AES-128 ECB block through GDMA channel 0, as IDF's crypto DMA drives it. The OUT chain
// holds the plaintext, the IN chain receives the ciphertext. GDMA channel 0 registers in the
// C3/C6 layout (IDF v5.5.4 soc/{esp32c3,esp32c6}/register/soc/gdma_reg.h): IN_CONF1 0x74,
// IN_LINK 0x80, IN_PERI_SEL 0xa0, OUT_CONF1 0xd4, OUT_LINK 0xe0, OUT_PERI_SEL 0x100.
const OUT_DESC: u32 = RAM;
const IN_DESC: u32 = RAM + 0x20;
const SRC: u32 = RAM + 0x100;
const DST: u32 = RAM + 0x200;
/// AES-128 of a zero block under a zero key (FIPS-197 known answer)
const CIPHER: [u8; 16] = [0x66, 0xe9, 0x4b, 0xd4, 0xef, 0x8a, 0x2c, 0x3b, 0x88, 0x4c, 0xfa, 0x59, 0xca, 0x34, 0x2b, 0x2e];

fn fixture() -> SocBus {
    let mut bus = SocBus::new(4 << 20, [0; 6]);
    for (addr, word) in [(OUT_DESC, (1 << 31) | (1 << 30) | (16 << 12) | 16), (OUT_DESC + 4, SRC), (OUT_DESC + 8, 0),
        (IN_DESC, (1 << 31) | 16), (IN_DESC + 4, DST), (IN_DESC + 8, 0)] {
        bus.write32(addr, word).unwrap();
    }
    bus.write32(GDMA + 0x100, 6).unwrap();          // OUT_PERI_SEL: AES
    bus.write32(GDMA + 0xa0, 6).unwrap();           // IN_PERI_SEL: AES
    bus.write32(AES + 0x90, 1).unwrap();            // DMA_ENABLE
    bus.write32(AES + 0x98, 1).unwrap();            // BLOCK_NUM
    bus
}
fn run(bus: &mut SocBus) {
    bus.write32(GDMA + 0x80, (1 << 22) | (IN_DESC & 0xf_ffff)).unwrap();   // IN_LINK: START
    bus.write32(GDMA + 0xe0, (1 << 21) | (OUT_DESC & 0xf_ffff)).unwrap();  // OUT_LINK: START
    bus.write32(AES + 0x48, 1).unwrap();                                   // TRIGGER
    for _ in 0..4 {
        if bus.read32(AES + 0x4c).unwrap() == 2 { break; }
        bus.tick(64);
    }
}
fn dst(bus: &mut SocBus) -> Vec<u8> { (0..16).map(|i| bus.read8(DST + i).unwrap()).collect() }

#[test]
fn aes_dma_encrypts_into_the_in_chain() {
    let mut bus = fixture();
    run(&mut bus);
    assert_eq!(bus.read32(AES + 0x4c).unwrap(), 2, "AES done");
    assert_eq!(dst(&mut bus), CIPHER);
    assert_eq!(bus.read32(IN_DESC).unwrap(), (1 << 30) | (16 << 12) | 16, "length, SUC_EOF, owner to the CPU");
    let (out, inp) = channels(&mut bus);
    assert_eq!((out.int_raw & 0x1f, out.running), (0b1011, false), "OUT_DONE, OUT_EOF, OUT_TOTAL_EOF");
    assert_eq!((inp.int_raw & 0x1f, inp.running, inp.desc, inp.eof_desc), (0b11, false, 0, IN_DESC), "IN_DONE, IN_SUC_EOF");
}

#[test]
fn aes_dma_out_owner_check_raises_out_dscr_err() {
    let mut bus = fixture();
    bus.write32(GDMA + 0xd4, 1 << 12).unwrap();                  // OUT_CHECK_OWNER
    bus.write32(OUT_DESC, (1 << 30) | (16 << 12) | 16).unwrap();  // owned by the CPU
    run(&mut bus);
    let (out, _) = channels(&mut bus);
    assert_eq!((out.int_raw & 0xc, out.running), (1 << 2, false), "OUT_DSCR_ERR, no OUT_TOTAL_EOF");
}

#[test]
fn aes_dma_out_buffer_outside_sram_raises_out_dscr_err() {
    let mut bus = fixture();
    bus.write32(OUT_DESC + 4, 0x5000_0000).unwrap();
    run(&mut bus);
    let (out, _) = channels(&mut bus);
    assert_eq!((out.int_raw & 0xc, out.running), (1 << 2, false), "OUT_DSCR_ERR, no OUT_TOTAL_EOF");
}

#[test]
fn aes_dma_in_owner_check_raises_in_dscr_err() {
    let mut bus = fixture();
    bus.write32(GDMA + 0x74, 1 << 12).unwrap();                  // IN_CHECK_OWNER
    bus.write32(IN_DESC, 16).unwrap();                           // owned by the CPU
    run(&mut bus);
    assert_eq!(dst(&mut bus), [0; 16], "nothing received");
    assert_eq!(bus.read32(IN_DESC).unwrap(), 16, "descriptor untouched");
    let (_, inp) = channels(&mut bus);
    assert_eq!((inp.int_raw & 0x1f, inp.running), (1 << 3, false), "IN_DSCR_ERR only");
}

#[test]
fn aes_dma_short_in_chain_raises_in_dscr_empty() {
    let mut bus = fixture();
    bus.write32(IN_DESC, (1 << 31) | 8).unwrap();                // room for half the block
    run(&mut bus);
    assert_eq!(dst(&mut bus)[..8], CIPHER[..8]);
    assert_eq!(dst(&mut bus)[8..], [0; 8]);
    assert_eq!(bus.read32(IN_DESC).unwrap(), (8 << 12) | 8, "filled, owner to the CPU, no SUC_EOF");
    let (_, inp) = channels(&mut bus);
    assert_eq!((inp.int_raw & 0x1f, inp.running), (1 | 1 << 3 | 1 << 4, false), "IN_DONE, IN_DSCR_ERR, IN_DSCR_EMPTY");
}
