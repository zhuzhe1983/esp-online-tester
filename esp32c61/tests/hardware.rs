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

#[test]
fn c61_adc_interrupt_uses_its_own_matrix_source_and_gpio_limit() {
    let mut b=Bus61::new(4<<20,[0;6]);
    assert!(b.gpio_state(29).is_some());assert!(b.gpio_state(30).is_none());
    b.write32(0x60010000+55*4,21).unwrap();b.write8(0x20801000+21*4+1,1).unwrap();b.write8(0x20801000+21*4+3,0x80).unwrap();
    assert!(b.adc_set_raw(1,123));b.write32(0x6000e040,1<<31).unwrap();b.write32(0x6000e020,(1<<31)|(1<<29)).unwrap();b.refresh_irq();
    let mut lines=[None];C61::irqs(&b,&mut lines);assert_eq!(lines[0].unwrap()&31,5);assert_eq!(b.irq_sources_of(0,5),[55]);
    b.write32(0x6000e04c,1<<31).unwrap();b.refresh_irq();C61::irqs(&b,&mut lines);assert!(lines[0].is_none());
}

#[test]
fn corematrix_power_removes_cpu_execution_and_wake_requests_real_reset() {
    let mut m=esp32c61::machine([0;6],4<<20);m.bus.board=esp32c61::board::make_board("corematrix").unwrap();m.bus.attach_board_devices();
    assert!(m.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).is_ok());
    // A real SRAM loop is held while the PMIC's CPU DCDC rail is off.
    m.bus.write32(0x40800000,0x00128293).unwrap();m.bus.write32(0x40800004,0xffdff06f).unwrap();m.cores[0].pc=0x40800000;
    m.run(100);let before=m.cores[0].x[5];assert!(before>0);
    let mut devices=m.bus.board.i2c_devices();let pm=devices.iter_mut().find(|x|x.1==0x6e).unwrap().2.as_mut();
    assert!(pm.start(false));for b in [0x0c,0xa1] {assert!(pm.write(b));}pm.stop();m.bus.tick(1);
    assert_eq!(C61::core_state(&m.bus,0),esp_soc::CoreState::Held);
    let target=m.bus.cycles+160_000;assert!(matches!(m.run_until_cycle(target),esp_soc::RunUntil::Reached));assert_eq!(m.bus.cycles,target);assert_eq!(m.cores[0].x[5],before);
    assert!(m.bus.board.button_at(target,"power",true));m.bus.tick(1);
    assert_eq!(C61::core_state(&m.bus,0),esp_soc::CoreState::Running);assert!(m.bus.sw_reset());assert_eq!(m.bus.periph.gpio.strap,15);
    m.bus.reboot([0;6]);assert!(!m.bus.sw_reset());assert!(m.bus.controller_powered);
}

#[test]
#[ignore="requires ESP32SIM_ROM_DIR containing the public esp32c61_rev100_rom.elf fetched by CI"]
fn c61_pmic_download_command_enters_actual_mask_rom_loader() {
    let rom=std::path::PathBuf::from(std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to the fetched ROM directory")).join("esp32c61_rev100_rom.elf");
    let mut m=esp32c61::machine([0;6],4<<20);m.console.capture=true;m.console.mask=0;
    m.bus.board=esp32c61::board::make_board("corematrix").unwrap();m.bus.attach_board_devices();m.load_rom(&std::fs::read(rom).expect("read esp32c61_rev100_rom.elf")).unwrap();
    let mut devices=m.bus.board.i2c_devices();let pm=devices.iter_mut().find(|x|x.1==0x6e).unwrap().2.as_mut();
    assert!(pm.start(false));for b in [0x0c,0xa3] {assert!(pm.write(b));}pm.stop();m.bus.tick(1);assert!(m.bus.sw_reset());assert_eq!(m.bus.periph.gpio.strap,7);
    m.reboot();m.run(1_000_000);let text=String::from_utf8_lossy(&m.console.all);
    assert_eq!(m.exceptions,0,"{text}");assert!(text.contains("ESP-ROM:esp32c61"),"{text}");assert!(text.contains("waiting for download"),"{text}");
}

#[test]
fn c61_uart_matrix_native_pins_and_source_clock_are_not_c6_values() {
    let mut b=Bus61::new(4<<20,[0;6]);
    b.write32(0x60090028,1<<9).unwrap();b.write32(0x60000014,80).unwrap();
    assert_eq!(b.periph.uart_route(0).tx_pins,1<<11);assert_eq!(b.periph.uart_route(0).rx_pin,Some(10));assert_eq!(b.periph.uart_route(0).baud,Some(500_000));
    for pin in [21,22] {b.write32(0x60090000+pin*4,(1<<12)|(1<<9)).unwrap();}
    b.write32(0x60091ad4+21*4,72).unwrap();b.write32(0x600912d4+72*4,0x100|22).unwrap();b.write32(0x60091038,1<<21).unwrap();
    b.write32(0x60006014,160).unwrap();b.write32(0x6009601c,(1<<22)|(2<<20)).unwrap();
    let route=b.periph.uart_route(2);assert_eq!(route.tx_pins,1<<21);assert_eq!(route.rx_pin,Some(22));assert_eq!(route.baud,Some(500_000));
    b.periph.uart_pin_input(&esp_soc::uart::UartInput::new(22,500_000,b"C61".to_vec()));assert_eq!(b.read32(0x60006000).unwrap(),b'C' as u32);
    b.periph.uart_pin_input(&esp_soc::uart::UartInput::new(22,115_200,vec![1]));assert_ne!(b.periph.uart[2].int_raw&esp_periph::uart::INT_FRM_ERR,0);
    b.write32(0x60091ad4+21*4,72|(1<<9)).unwrap();assert_eq!(b.periph.uart_route(2).tx_pins,0);
    b.write32(0x6009601c,0).unwrap();assert_eq!(b.periph.uart_route(2).baud,None);
}

#[test]
fn c61_ledc_clock_gate_and_nine_bit_matrix_inversion_control_pwm() {
    let mut b=Bus61::new(4<<20,[0;6]);b.write32(0x60090054,1<<12).unwrap();b.write32(0x60091ad4+21*4,0).unwrap();b.write32(0x60091038,1<<21).unwrap();
    for (off,value) in [(0xa0,8|(256<<5)|(1<<26)),(8,64<<4),(12,1<<31),(0,4|(1<<4))] {b.write32(0x60007000+off,value).unwrap();}b.tick(10000);
    assert_eq!(b.pwm_output(21),Some((156250.,16384)));b.write32(0x60091ad4+21*4,1<<9).unwrap();assert_eq!(b.pwm_output(21),Some((156250.,49151)));
    b.write32(0x60096038,(1<<22)|(2<<20)).unwrap();assert_eq!(b.pwm_output(21),Some((312500.,49151)));
    b.write32(0x60096038,(1<<22)|(1<<20)).unwrap();assert_eq!(b.pwm_output(21),Some((68359.375,49151)));
    b.write32(0x60096038,0).unwrap();assert!(b.pwm_output(21).is_none());b.write32(0x60096038,1<<22).unwrap();b.write32(0x60096034,3).unwrap();assert!(b.pwm_output(21).is_none());
}

#[test]
fn c61_ecc_source64_and_pcr_reset_clear_accelerator_state() {
    let mut b=Bus61::new(16<<20,[0;6]);assert_eq!(b.periph.spi0.0.jedec[2],24);assert_eq!(b.periph.spi1.0.jedec[2],24);
    b.write32(0x6008b100,42).unwrap();b.write32(0x6008b014,1).unwrap();b.write32(0x6008b01c,0x25).unwrap();
    assert_eq!(b.read32(0x6008b00c).unwrap(),1);b.refresh_irq();assert_eq!(b.periph.intmtx.status[2]&1,1);
    b.write32(0x600960ac,1).unwrap();assert_eq!(b.read32(0x6008b100).unwrap(),42);
    b.write32(0x600960ac,3).unwrap();b.write32(0x600960ac,1).unwrap();b.refresh_irq();assert_eq!(b.periph.intmtx.status[2]&1,0);
    assert_eq!(b.read32(0x6008b100).unwrap(),0);assert_eq!(b.read32(0x6008b014).unwrap(),0);assert_eq!(b.read32(0x6008b00c).unwrap(),0);
}

#[test]
fn c61_efuse_idle_state_is_read_only_after_synchronous_reload() {
    use emu_core::Bus;
    let mut b=esp32c61::bus::SocBus::new(1<<20,[0;6]);
    assert_eq!(b.read32(0x600b49d0).unwrap()&15,1);
    b.write32(0x600b49d0,0).unwrap();b.write32(0x600b49d4,1).unwrap();
    assert_eq!(b.read32(0x600b49d0).unwrap()&15,1);assert_eq!(b.read32(0x600b49d4).unwrap(),0);
}
