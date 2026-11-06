use emu_core::Bus;
use esp_soc::{Soc, SocBus};
use esp32c61::{C61, bus::SocBus as Bus61};

#[test]
fn c61_memory_boundaries_and_psram_mmu_do_not_alias_c6() {
    let mut b = Bus61::new(16 << 20, [0; 6]); b.set_psram_size(8 << 20).unwrap();
    assert!(b.write32(0x4084fffc, 42).is_ok()); assert!(b.write32(0x40850000, 42).is_err());
    assert!(b.read32(0x40040000).is_err()); assert!(b.read32(0x50000000).is_err());
    assert!(b.read32(0x42000000).is_err());
    b.write32(0x60002380, 511).unwrap(); b.write32(0x6000237c, (1 << 10) | (1 << 9) | 3).unwrap();
    b.write32(0x43ff0004, 0x12345678).unwrap(); assert_eq!(&b.psram[0x30004..0x30008], &0x12345678u32.to_le_bytes());
    b.write32(0x6000237c, 1 << 10).unwrap(); assert!(b.write32(0x43ff0004, 1).is_err());
    b.write32(0x6000237c, 1 << 9).unwrap(); assert!(b.read32(0x43ff0004).is_err());
}

#[test]
fn c61_gpio_mux_spacing_and_adc_channel_map_are_independent() {
    let mut b = Bus61::new(4 << 20, [0; 6]);
    b.write32(0x60090000, (1 << 12) | (1 << 9)).unwrap();
    b.write32(0x600912d4 + 47 * 4, 0x100).unwrap(); b.write32(0x60091ad4, 47).unwrap();
    assert_eq!(esp32c61::periph::PINS.routes(&b.periph.gpio, &b.periph.io_mux).i2c_pin(47), Some(0));
    b.write32(0x60091390, 0x80).unwrap(); // C6's input-select bit is inversion on C61
    assert_eq!(esp32c61::periph::PINS.routes(&b.periph.gpio, &b.periph.io_mux).i2c_pin(47), None);
    assert!(b.adc_set_raw(3, 1234)); assert!(!b.adc_set_raw(2, 1234));
    b.write32(0x6000e020, (1 << 31) | (1 << 25) | (1 << 29)).unwrap();
    assert_eq!(b.read32(0x6000e02c).unwrap(), 1234); assert_eq!(b.read32(0x6000e044).unwrap(), 1 << 31);
    b.write32(0x6000e04c, 1 << 31).unwrap(); assert_eq!(b.read32(0x6000e044).unwrap(), 0);
}

#[test]
fn c61_standard_clic_uses_byte_enables_and_its_own_source_numbers() {
    let mut b = Bus61::new(4 << 20, [0; 6]);
    b.write32(0x60010000 + 19 * 4, 20).unwrap();
    b.write8(0x20801000 + 20 * 4 + 1, 1).unwrap(); b.write8(0x20801000 + 20 * 4 + 3, 0x80).unwrap();
    b.write32(0x600c5090, 1).unwrap(); b.refresh_irq();
    let mut lines = [None]; C61::irqs(&b, &mut lines); assert!(lines[0].is_some());
    assert_eq!(b.read32(0x60010108).unwrap(), 1 << 19);
    b.write8(0x20801000 + 20 * 4 + 1, 0).unwrap(); C61::irqs(&b, &mut lines); assert!(lines[0].is_none());
}

fn image(min: u16, max: u16) -> Vec<u8> {
    let mut d = vec![0; 36]; d[0] = 0xe9; d[1] = 1; d[12] = 20;
    d[15..17].copy_from_slice(&min.to_le_bytes()); d[17..19].copy_from_slice(&max.to_le_bytes());
    d[24..28].copy_from_slice(&0x40800000u32.to_le_bytes()); d[28..32].copy_from_slice(&4u32.to_le_bytes()); d
}
#[test]
fn c61_rejects_wrong_chip_or_revision_before_loading_application() {
    for (min,max,valid) in [(0,0,true),(100,199,true),(100,100,true),(101,199,false),(0,99,false)] {
        assert_eq!(C61::check_image(&image(min,max)).is_ok(), valid);
    }
    let mut d = image(100,199); d[12] = 13; assert!(C61::check_image(&d).is_err());
    let mut b = Bus61::new(4 << 20,[0;6]); b.write_flash(0x10000,&d).unwrap(); b.sram[0] = 0xa5;
    assert!(b.boot_app(0x10000).is_err()); assert_eq!(b.sram[0],0xa5);
}
