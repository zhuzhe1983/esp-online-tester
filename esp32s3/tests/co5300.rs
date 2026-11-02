//! GPIO38 any-edge setup follows ESP-IDF v6.1:
//! components/soc/esp32s3/register/soc/gpio_reg.h (PIN38 +0x10c,
//! INT_TYPE bits 9:7, INT_ENA bits 17:13, IN1 +0x40, STATUS1_W1TC +0x58,
//! PCPU_INT1 +0x68), components/esp_hal_gpio/include/hal/gpio_types.h (ANYEDGE = 3).
//! components/soc/esp32s3/include/soc/interrupts.h names GPIO source 16;
//! register/soc/interrupt_core0_reg.h routes it at +0x40. These fields match v5.5.4.
//! The SPI command fixture uses the same version's register/soc/spi_reg.h:
//! CMD.USR bit 24, ADDR +0x04, USER +0x10 (COMMAND/ADDR/MOSI bits 31/30/27),
//! USER1 +0x14 (ADDR_BITLEN at 27), USER2 +0x18 (COMMAND_BITLEN at 28),
//! MS_DLEN +0x1c and W0 +0x98. Source addresses come from register/soc/reg_base.h.
use emu_core::Bus;
use esp_soc::board::{BoardEdge, BoardModel};
use esp_soc::devices::co5300::{Co5300, Co5300Scan};

struct PanelBoard(Co5300);

impl BoardModel for PanelBoard {
    fn name(&self) -> &'static str { "co5300-test-fixture" }
    fn spi_transfer(&mut self, host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        if host == 2 { self.0.transaction(tx); }
        vec![0xff; rx_len]
    }
    fn input_levels(&self) -> Vec<(u8, bool)> { self.0.tear_input().into_iter().collect() }
    fn next_deadline(&self) -> Option<u64> { self.0.next_deadline() }
    fn advance_to(&mut self, cycle: u64) { self.0.advance_to(cycle); }
    fn take_edges(&mut self) -> Vec<BoardEdge> { self.0.take_edges() }
    fn display(&self) -> Option<(u32,u32,Vec<u16>,u64)> {
        let state=self.0.display_state();
        let frame=if state.is_none_or(|s|s.visible()) {self.0.frame.clone()}else{vec![0;self.0.frame.len()]};
        Some((self.0.width() as u32,self.0.height() as u32,frame,state.map_or(0,|s|s.revision)))
    }
}

fn command(bus: &mut esp32s3::bus::SocBus, command: u8, data: &[u8]) {
    const SPI2: u32 = 0x6002_4000;
    bus.write32(SPI2 + 0x10, (1 << 31) | (1 << 30) | (u32::from(!data.is_empty()) << 27)).unwrap();
    bus.write32(SPI2 + 0x14, 23 << 27).unwrap();
    bus.write32(SPI2 + 0x18, (7 << 28) | 0x02).unwrap();
    bus.write32(SPI2 + 0x04, (command as u32) << 16).unwrap();
    bus.write32(SPI2 + 0x1c, (data.len() as u32 * 8).saturating_sub(1)).unwrap();
    let mut bytes = [0; 4];
    bytes[..data.len()].copy_from_slice(data);
    bus.write32(SPI2 + 0x98, u32::from_le_bytes(bytes)).unwrap();
    bus.write32(SPI2, 1 << 24).unwrap();
}

#[test]
fn real_spi_register_commands_separate_gram_writes_from_display_enable() {
    let mut bus=esp32s3::bus::SocBus::new(1024,1024,[0;6]);
    bus.board=Box::new(PanelBoard(Co5300::new(1,1,0).with_display_state()));
    bus.attach_board_devices();
    command(&mut bus,0x3a,&[0x55]);
    command(&mut bus,0x2c,&[0xf8,0]);
    assert_eq!(bus.board.display().unwrap().2,[0]);
    command(&mut bus,0x11,&[]);
    assert_eq!(bus.board.display().unwrap().2,[0]);
    command(&mut bus,0x29,&[]);
    assert_eq!(bus.board.display().unwrap().2,[0xf800]);
    command(&mut bus,0x53,&[0x20]);
    assert_eq!(bus.board.display().unwrap().2,[0]);
    command(&mut bus,0x51,&[0xa0]);
    assert_eq!(bus.board.display().unwrap().2,[0xf800]);
    let on_revision=bus.board.display().unwrap().3;
    command(&mut bus,0x28,&[]);
    assert_eq!(bus.board.display().unwrap().2,[0]);
    assert_ne!(bus.board.display().unwrap().3,on_revision);
    command(&mut bus,0x29,&[]);
    assert_eq!(bus.board.display().unwrap().2,[0xf800]);
    command(&mut bus,0x10,&[]);
    assert_eq!(bus.board.display().unwrap().2,[0]);
    command(&mut bus,0x11,&[]);
    assert_eq!(bus.board.display().unwrap().2,[0xf800]);
    command(&mut bus,0x01,&[]);
    assert_eq!(bus.board.display().unwrap().2,[0]);
}

#[test]
fn scan_crossing_reaches_gpio38_status_and_the_cpu_interrupt_route() {
    const GPIO: u32 = 0x6000_4000;
    let mut bus = esp32s3::bus::SocBus::new(1024, 1024, [0; 6]);
    bus.board = Box::new(PanelBoard(Co5300::new(468, 466, 6).with_scan(Co5300Scan {
        lines: 500, period: 15_000, pulse_width: 120, pin: 38,
    })));
    bus.attach_board_devices();
    bus.gpio_events = Some(Vec::new());
    bus.write32(GPIO + 0x10c, (3 << 7) | (1 << 13)).unwrap();
    bus.write32(GPIO + 0x58, 1 << 6).unwrap();
    bus.write32(0x600c_2040, 9).unwrap();
    command(&mut bus, 0x44, &320u16.to_be_bytes());
    command(&mut bus, 0x35, &[0]);
    bus.tick(9_599);
    assert_eq!(bus.read32(GPIO + 0x40).unwrap() & (1 << 6), 0);
    assert!(!bus.periph.gpio.irq());
    assert_eq!(bus.periph.cpu_lines(0), 0);

    bus.tick(1);
    assert_ne!(bus.read32(GPIO + 0x40).unwrap() & (1 << 6), 0);
    assert_eq!(bus.read32(GPIO + 0x68).unwrap(), 1 << 6);
    assert!(bus.periph.gpio.irq());
    assert_eq!(bus.periph.cpu_lines(0), 1 << 9);
    assert!(bus.irq_dirty);
    assert_eq!(bus.gpio_events.as_deref(), Some(&[(9_600, 38, true)][..]));

    bus.write32(GPIO + 0x58, 1 << 6).unwrap();
    assert_eq!(bus.periph.cpu_lines(0), 0);
    bus.tick(120);
    assert_eq!(bus.read32(GPIO + 0x40).unwrap() & (1 << 6), 0);
    assert_eq!(bus.read32(GPIO + 0x68).unwrap(), 1 << 6);
    assert_eq!(bus.periph.cpu_lines(0), 1 << 9);
    assert_eq!(bus.gpio_events.as_deref(), Some(&[(9_600, 38, true), (9_720, 38, false)][..]));

    bus.write32(GPIO + 0x58, 1 << 6).unwrap();
    command(&mut bus, 0x34, &[]);
    bus.tick(30_000);
    assert_eq!(bus.read32(GPIO + 0x68).unwrap(), 0);
    assert_eq!(bus.periph.cpu_lines(0), 0);
}
