use emu_core::Bus;

#[test]
fn both_gpspi_software_set_registers_start_irq_and_clear_without_transfer() {
    let mut m=esp32s3::machine([0;6]);
    for (base,other) in [(0x6002_4000,0x6002_5000),(0x6002_5000,0x6002_4000)] {
        m.bus.write32(base+0x44,1<<12).unwrap();assert_eq!(m.bus.read32(base+0x3c).unwrap(),1<<12);
        assert_eq!(m.bus.read32(base+0x40).unwrap(),0);assert_eq!(m.bus.read32(other+0x3c).unwrap(),0);
        m.bus.write32(base+0x34,1<<12).unwrap();assert_eq!(m.bus.read32(base+0x40).unwrap(),1<<12);
        m.bus.write32(base+0x44,0).unwrap();assert_eq!(m.bus.read32(base+0x40).unwrap(),1<<12);
        m.bus.write32(base+0x38,1<<12).unwrap();assert_eq!(m.bus.read32(base+0x40).unwrap(),0);
        m.bus.write32(base+0x44,0xffe0_0000).unwrap();assert_eq!(m.bus.read32(base+0x3c).unwrap(),0);
        assert_eq!(m.bus.read32(base+0x44).unwrap(),0,"write-trigger register has no retained latch");
    }
    assert_eq!(m.bus.periph.spi2.transfers,0);assert_eq!(m.bus.periph.spi3.transfers,0);
}
