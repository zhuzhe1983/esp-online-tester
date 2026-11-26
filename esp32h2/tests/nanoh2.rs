mod common;
use emu_core::Bus;
use esp_soc::SocBus;
fn drive(b:&mut esp32h2::H2Bus,pin:u32,level:bool) {
 b.write32(0x60090004+pin*4,1<<12).unwrap();b.write32(0x60091554+pin*4,128).unwrap();
 b.write32(0x60091024,1<<pin).unwrap();b.write32(if level {0x60091008}else{0x6009100c},1<<pin).unwrap();
}
#[test]
fn nanoh2_guest_rmt_requires_its_real_supply_and_pin() {
 let mut m=esp32h2::machine([0;6],4<<20);m.bus.attach_board(esp32h2::board::make_board("nanoh2").unwrap());
 assert_eq!(m.bus.board.named_pin("button-a"),Some(9));assert_eq!(m.bus.board.named_pin("power"),None);
 m.bus.write_flash(0,&common::fixture(11)).unwrap();m.boot_app(0).unwrap();m.run_until_cycle(10_000);
 assert_eq!(m.bus.board.leds().unwrap().0,[[0;3];2],"unpowered WS2812 discards real guest RMT");
 drive(&mut m.bus,10,true);m.bus.write_flash(0,&common::fixture(11)).unwrap();m.boot_app(0).unwrap();let until=m.bus.cycles+10_000;m.run_until_cycle(until);
 assert_eq!(m.bus.board.leds().unwrap().0[0],[0x34,0x12,0x56]);assert_eq!(m.bus.console_take()[1],b"H2 SRAM chip16 RGB8\nH2 SRAM chip16 RGB8\n");
 drive(&mut m.bus,4,true);assert_eq!(m.bus.board.leds().unwrap().0[1],[0,0,255]);
 m.bus.write32(0x60091028,1<<10).unwrap();assert_eq!(m.bus.board.leds().unwrap().0[0],[0;3]);
 drive(&mut m.bus,10,true);assert_eq!(m.bus.board.leds().unwrap().0[0],[0;3],"power restoration does not replay stale pixels");
 let bits=vec![true;24];m.bus.board.rmt_frame(8,&bits);assert_eq!(m.bus.board.leds().unwrap().0[0],[0;3],"Waveshare/C6 RGB pins are not this SKU");
 m.bus.gpio_set_input(9,false);assert_eq!(m.bus.read32(0x6009103c).unwrap()&(1<<9),0);
}
