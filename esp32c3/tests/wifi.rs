use emu_core::Bus;
const STATION: [u8; 6] = [2, 0, 0, 0, 0, 3];
/// AES-128 ECB of the FIPS-197 vector through GDMA peripheral 6, which is how ESP-IDF's driver
/// (and with it the WPA2 key unwrap) uses the block: the OUT chain in, the IN chain out, DONE.
#[test]
fn aes_runs_through_gdma() {
    let mut m = esp32c3::machine(STATION, 4 << 20);
    const AES: u32 = 0x6003_a000;
    let key = [0x0001_0203u32, 0x0405_0607, 0x0809_0a0b, 0x0c0d_0e0f];
    let plain: [u8; 16] = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let cipher: [u8; 16] = [
        0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4, 0xc5,
        0x5a,
    ];
    let (out_desc, in_desc, src, dst) = (0x3fc9_0000u32, 0x3fc9_0010, 0x3fc9_0100, 0x3fc9_0200);
    for (i, b) in plain.iter().enumerate() {
        m.bus.write8(src + i as u32, *b).unwrap();
    }
    m.bus
        .write32(out_desc, 1 << 31 | 1 << 30 | 16 << 12 | 16)
        .unwrap();
    m.bus.write32(out_desc + 4, src).unwrap();
    m.bus.write32(in_desc, 1 << 31 | 16).unwrap();
    m.bus.write32(in_desc + 4, dst).unwrap();
    const GDMA: u32 = 0x6003_f000;
    m.bus.write32(GDMA + 0xa0, 6).unwrap(); // channel 0 IN peripheral
    m.bus.write32(GDMA + 0x100, 6).unwrap(); // channel 0 OUT peripheral
    m.bus
        .write32(GDMA + 0x80, (in_desc & 0xfffff) | 1 << 22)
        .unwrap();
    m.bus
        .write32(GDMA + 0xe0, (out_desc & 0xfffff) | 1 << 21)
        .unwrap();
    for (i, k) in key.iter().enumerate() {
        m.bus.write32(AES + 4 * i as u32, k.swap_bytes()).unwrap();
    }
    m.bus.write32(AES + 0x40, 0).unwrap(); // AES-128 encrypt
    m.bus.write32(AES + 0x90, 1).unwrap();
    m.bus.write32(AES + 0x94, 0).unwrap();
    m.bus.write32(AES + 0x98, 1).unwrap();
    m.bus.write32(AES + 0x48, 1).unwrap(); // trigger
    m.bus.tick(160_000);

    assert_eq!(
        m.bus.read32(AES + 0x4c).unwrap(),
        2,
        "aes_hal_wait_done sees DONE"
    );
    let got: Vec<u8> = (0..16).map(|i| m.bus.read8(dst + i).unwrap()).collect();
    assert_eq!(got, cipher);
    let dw0 = m.bus.read32(in_desc).unwrap();
    assert_eq!(
        (dw0 >> 12 & 0xfff, dw0 >> 30),
        (16, 1),
        "16 bytes, SUC_EOF, owner back with the CPU"
    );
    assert!(!m.bus.periph.gdma.gdma.inp[0].running && !m.bus.periph.gdma.gdma.out[0].running);
}

#[test]
fn calibration_and_mac_events_follow_c3_registers() {
    let mut p = esp32c3::periph::Peripherals::new([0; 6]);
    assert_eq!(p.read32(0x60006174) & (1 << 16), 0);
    p.write32(0x60006144, 3);
    p.tick(158);
    assert_eq!(p.read32(0x60006174) & (1 << 16), 0);
    p.tick(2);
    assert_ne!(p.read32(0x60006174) & (1 << 16), 0);
    p.write32(0x60006144, 0);
    p.write32(0x60006144, 3);
    assert_eq!(p.read32(0x60006174) & (1 << 16), 0);
    p.write32(0x6000e000, 1 << 24 | 0xa5 << 16 | 0x0362);
    p.write32(0x6000e004, 0x0362);
    assert_eq!(p.read32(0x6000e004), 0xa50362);
    p.write32(0x60033d14, 2);
    assert_eq!(p.read32(0x60033d14) & 3, 3);
    p.wifi.tx_done(0);
    assert_ne!(p.read32(0x60033c3c) & (1 << 7), 0);
    p.write32(0x60033c40, 1 << 7);
    assert_eq!(p.read32(0x60033c3c), 0);
    p.write32(0x60033cac, 1);
    assert_eq!(p.read32(0x60033cb0), 0);
}

#[test]
fn gdma_interrupt_bits_and_mmio_observers() {
    let mut p = esp32c3::periph::Peripherals::new(STATION);
    p.misc.mmio_log = Some(Vec::new()); p.misc.cur_pc = 0x42000000;
    // C3 gdma_reg.h: DONE, SUC_EOF, ERR_EOF, DSCR_ERR, DSCR_EMPTY, FIFO_OVF, FIFO_UDF.
    for ch in 0..3 {
        let base = 0x6003f000 + ch as u32 * 16;
        for (input, bits) in [(true, &[0, 1, 2, 5, 7, 9, 10][..]), (false, &[3, 4, 6, 8, 11, 12][..])] {
            for (logical, bit) in bits.iter().enumerate() {
                p.gdma.gdma.inp[ch].int_raw = if input { 1 << logical } else { 0 };
                p.gdma.gdma.out[ch].int_raw = if input { 0 } else { 1 << logical };
                p.write32(base + 8, 0);
                assert_eq!(p.read32(base), 1 << bit);
                assert_eq!(p.read32(base + 4), 0);
                p.write32(base + 8, 1 << bit);
                assert_eq!(p.gdma.gdma.inp[ch].int_ena, if input { 1 << logical } else { 0 });
                assert_eq!(p.gdma.gdma.out[ch].int_ena, if input { 0 } else { 1 << logical });
                assert_eq!(p.read32(base + 8), 1 << bit);
                assert_eq!(p.read32(base + 4), 1 << bit);
                assert_ne!(p.source_status()[1] & (1 << (12 + ch)), 0);
                p.write32(base + 12, 1 << bit);
                assert_eq!(p.read32(base), 0);
                assert_eq!(p.source_status()[1] & (1 << (12 + ch)), 0);
            }
        }
    }
    let log = p.misc.mmio_log.as_ref().unwrap();
    assert!(log.contains(&(0x42000000, 0x6003f008, 1 << 5, true)));
    assert!(log.contains(&(0x42000000, 0x6003f004, 1 << 5, false)));
}

#[test]
fn iq_completion_is_polled_without_a_clock_or_deadline() {
    use esp_periph::Device;
    let mut p = esp32c3::periph::Peripherals::new(STATION);
    assert_eq!(p.fe_iq.clock(), None); assert!(!p.fe_iq.has_deadline());
    p.tick(1000); p.write32(0x60006144, 3);
    assert_eq!(p.fe_iq.clock(), None); assert!(!p.fe_iq.has_deadline());
    p.tick(159); assert_eq!(p.read32(0x60006174), 0);
    p.tick(1); assert_eq!(p.read32(0x60006174), 1 << 16);
    p.tick(1); p.write32(0x60006144, 0); p.write32(0x60006144, 3);
    p.tick(158); assert_eq!(p.read32(0x60006174), 0);
    p.tick(1); assert_eq!(p.read32(0x60006174), 1 << 16); // 80 APB edges, even with an odd start cycle
}

#[test]
fn idle_rounds_skip_feature_work() {
    let mut m = esp32c3::machine(STATION, 4 << 20);
    assert!(!m.bus.periph.work_pending);
    assert_eq!(m.bus.tick(100), 1);
    assert!(!m.bus.periph.work_pending);
    m.bus.write32(0x60033d08, (1 << 31) | 0x90000).unwrap();
    assert!(m.bus.periph.work_pending);
    m.bus.tick(160);
    assert_eq!(m.bus.periph.wifi.link.tx_frames(), 1);
    assert!(!m.bus.periph.work_pending, "the completed TX leaves no idle work");
    assert_eq!(m.bus.periph.source_status()[0] & 1, 1);
    m.bus.write32(0x60033c40, u32::MAX).unwrap();
    assert_eq!(m.bus.periph.source_status()[0] & 1, 0);
}

#[test]
fn configured_ap_keeps_work_scheduled_across_reboot() {
    use esp_soc::SocBus;
    let mut m = esp32c3::machine(STATION, 4 << 20);
    m.bus.periph.wifi.link.attach(Some(esp_soc::wifi::VirtualAp::new(esp_soc::wifi::ApConfig::parse("").unwrap(), false)), None);
    m.bus.periph.refresh_work();
    assert!(m.bus.periph.work_pending);
    m.bus.reboot(STATION);
    assert!(m.bus.periph.work_pending);
    m.bus.tick(16_000_000);
    assert_eq!(m.bus.periph.wifi.link.ap().unwrap().stats.0, 1);
}
