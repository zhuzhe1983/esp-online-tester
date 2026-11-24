mod common;
use emu_core::{Bus,Fault};
use esp_soc::{Soc,SocBus};
use esp32h2::{H2,H2Bus,bus::*};
#[test]fn both_exact_h2_boards_execute_actual_rv32_sram_program(){
 for name in["waveshare-h2-zero","waveshare-h2-dev-kit-n4"]{
  let mut m=esp32h2::machine([0;6],FLASH_SIZE);m.bus.attach_board(esp32h2::board::make_board(name).unwrap());m.bus.write_flash(0,&common::fixture(8)).unwrap();assert_eq!(m.boot_app(0).unwrap(),SRAM_BASE);
  assert!(matches!(m.run_until_cycle(10_000),esp_soc::RunUntil::Reached));
  assert_eq!(m.bus.console_take()[1],b"H2 SRAM chip16 RGB8\n");assert_eq!(m.bus.board.leds().unwrap().0,[[0x34,0x12,0x56]]);assert_eq!(m.bus.board.leds().unwrap().1,1);
  assert_ne!(m.bus.read32(SRAM_BASE+0x4000).unwrap()&(1<<9),0);assert_eq!(m.bus.read32(LP_BASE).unwrap(),0x48325352);assert!(m.bus.first_fault.is_none());assert_eq!(m.bus.board.named_pin("boot"),Some(9));assert_eq!(m.bus.board.named_pin("reset"),None);
  m.bus.gpio_set_input(9,false);m.bus.request_reset(1);m.reboot();assert_eq!(m.bus.read32(0x6009103c).unwrap()&(1<<9),0);
  let until=m.bus.cycles+10_000;assert!(matches!(m.run_until_cycle(until),esp_soc::RunUntil::Reached));assert_eq!(m.bus.read32(SRAM_BASE+0x4000).unwrap()&(1<<9),0);assert_eq!(m.bus.console_take()[1],b"H2 SRAM chip16 RGB8\n");
 }
 const{assert!(!H2::ROM_BOOT_DEFAULT)};assert_eq!(H2::NAME,"esp32h2");assert!(esp32h2::board::make_board("waveshare-c6-zero").is_none());
}
#[test]fn wrong_physical_rgb_pin_does_not_update_the_h2_led(){
 let mut m=esp32h2::machine([0;6],FLASH_SIZE);m.bus.attach_board(esp32h2::board::make_board("waveshare-h2-zero").unwrap());m.bus.write_flash(0,&common::fixture(10)).unwrap();m.boot_app(0).unwrap();m.run_until_cycle(10_000);assert_eq!(m.bus.board.leds().unwrap().1,0);assert_eq!(m.bus.console_take()[1],b"H2 SRAM chip16 RGB8\n");
}
#[test]fn h2_rmt_xtal_channel_divider_and_clock_gate_are_real(){
 let mut b=H2Bus::new();common::configure_rmt(&mut b,8,1,&[0x8000|13|(27<<16),0]);
 b.advance(119);assert_eq!(b.read32(0x60007038).unwrap()&1,0);b.advance(1);assert_eq!(b.read32(0x60007038).unwrap()&1,1);
 b.write32(0x60007044,1).unwrap();b.write32(0x60007010,(1<<16)|1).unwrap(); // DIV0 means256
 b.advance(30_719);assert_eq!(b.read32(0x60007038).unwrap()&1,0);b.advance(1);assert_eq!(b.read32(0x60007038).unwrap()&1,1);
 b.write32(0x60007044,1).unwrap();b.write32(0x60007010,(1<<16)|(1<<8)|1).unwrap();b.advance(60);b.write32(0x60096034,0).unwrap();b.advance(5000);assert!(b.read32(0x60007038).is_err());b.write32(0x60096034,1).unwrap();assert_eq!(b.read32(0x60007038).unwrap()&1,0);b.advance(59);assert_eq!(b.read32(0x60007038).unwrap()&1,0);b.advance(1);assert_eq!(b.read32(0x60007038).unwrap()&1,1);
}
#[test]fn h2_fractional_clock_carries_remainder_and_rejects_rc_fast_and_mid_transfer_retime(){
 let mut b=H2Bus::new();common::configure_rmt(&mut b,8,1,&[0x8000|13|(27<<16),0]);b.advance(120);b.write32(0x60007044,1).unwrap();
 // HAL formula1+DIV_NUM+DIV_A/DIV_B:1+1/2=>each RMT tick4.5CPU cycles.
 b.write32(0x60096038,(1<<21)|(2<<6)|1).unwrap();b.write32(0x60007010,(1<<16)|(1<<8)|1).unwrap();
 for _ in 0..179{b.advance(1);}assert_eq!(b.read32(0x60007038).unwrap()&1,0);assert_eq!(b.write32(0x60096038,1<<21),Err(Fault::Prohibited));b.advance(1);assert_eq!(b.read32(0x60007038).unwrap()&1,1);
 b.write32(0x60096038,(1<<21)|(1<<20)).unwrap();assert_eq!(b.write32(0x60007010,(1<<16)|(1<<8)|1),Err(Fault::Prohibited));
 assert!(b.write32(0x60096038,(1<<21)|1).is_err());
}
#[test]fn interrupted_route_unwritten_symbols_and_reset_never_invent_led_frames(){
 let mut b=H2Bus::new();b.attach_board(esp32h2::board::make_board("waveshare-h2-zero").unwrap());let mut symbols=vec![0x8000|13|(27<<16);24];symbols.push(0);common::configure_rmt(&mut b,8,1,&symbols);b.advance(120);b.write32(0x60091574,128).unwrap();b.write32(0x60091574,71).unwrap();b.advance(2880);assert_eq!(b.board.leds().unwrap().1,0);
 b.write32(0x60096034,3).unwrap();assert!(b.read32(0x60007038).is_err());b.write32(0x60096034,1).unwrap();b.write32(0x60007068,0x8500001b).unwrap();b.write32(0x60007010,(1<<16)|(1<<8)|1).unwrap();b.advance(1_000);assert_ne!(b.read32(0x60007038).unwrap()&(1<<4),0);assert_eq!(b.read32(0x60007038).unwrap()&1,0);assert_eq!(b.board.leds().unwrap().1,0);
}
#[test]fn exact_h2_memory_and_pad_boundaries_do_not_alias_c6(){
 let mut b=H2Bus::new();b.write32(SRAM_BASE+SRAM_SIZE as u32-4,0x1234).unwrap();b.write32(LP_BASE+LP_SIZE as u32-4,0x5678).unwrap();assert_eq!(b.read32(LP_BASE+LP_SIZE as u32-4).unwrap(),0x5678);
 for a in[SRAM_BASE+SRAM_SIZE as u32,LP_BASE+LP_SIZE as u32,0x60006010,0x42000000,ROM_BASE,0x600c2000,0x60010000]{assert!(b.read32(a).is_err(),"{a:x}");assert!(b.write32(a,1).is_err(),"{a:x}");}
 assert!(b.fetch(ROM_BASE).is_err());assert!(b.load_bytes(ROM_BASE,&[0;4]).is_err());assert!(b.write16(0x60007068,0).is_err());assert_eq!(b.read32(SRAM_BASE+1),Err(Fault::Misaligned));
 for p in[6,7,15,16,17,18,19,20,21,28]{b.gpio_set_input(p,false);assert!(b.gpio_state(p).is_none());assert!(b.write32(0x60090004+u32::from(p)*4,1<<12).is_err());}
 b.write32(0x60091004,u32::MAX).unwrap();assert_eq!(b.read32(0x60091004).unwrap(),VALID_PADS as u32);
 let mut out=[Some(7)];H2::irqs(&b,&mut out);assert_eq!(out,[None]);
}
#[test]fn uart_fifo_polling_respects_resets_and_never_aliases_usb(){
 let mut b=H2Bus::new();b.uart_input(0,b"AB");assert_eq!(b.read32(0x6000001c).unwrap()&255,2);assert_eq!(b.read32(0x60000000).unwrap(),65);b.write32(0x60000020,0x50001c).unwrap();assert_eq!(b.read32(0x6000001c).unwrap()&255,0);b.write32(0x60000020,0x10001c).unwrap();
 b.serial_input(b"USB");assert_eq!(b.read32(0x6000001c).unwrap()&255,0);b.write32(0x60000000,90).unwrap();assert_eq!(b.console_take(),[vec![],b"Z".to_vec(),vec![],vec![]]);
 b.write32(0x60096000,0).unwrap();assert!(b.write32(0x60000000,88).is_err());b.write32(0x60096000,3).unwrap();assert!(b.read32(0x6000001c).is_err());b.write32(0x60096000,1).unwrap();assert_eq!(b.read32(0x6000001c).unwrap()&255,0);
 assert!(b.write32(0x60000020,0x18001c).is_err()); // unsupported autobaud
 assert!(b.write32(0x6000000c,1<<8).is_err()); // no fabricated RX timeout
}
#[test]fn h2_strict_image_checks_are_transactional_and_capacity_specific(){
 let ok=common::fixture(8);assert!(esp32h2::image::parse(&ok).is_ok());let mut legacy=common::image(SRAM_BASE,&[(SRAM_BASE,vec![0;4])],false);legacy[14]=7;for code in[0,1,2]{legacy[3]=code<<4;assert!(esp32h2::image::parse(&legacy).is_ok());}
 let mut cases=vec![];for chip in[5,9,13,23]{let mut d=ok.clone();d[12]=chip;cases.push(d);}let mut d=ok.clone();d[3]=0x30;cases.push(d);let mut d=ok.clone();d[15]=1;cases.push(d);let mut d=ok.clone();d[17..19].copy_from_slice(&100u16.to_le_bytes());cases.push(d);let mut d=ok.clone();*d.last_mut().unwrap()^=1;cases.push(d);let mut d=ok.clone();d[32]^=1;cases.push(d);let mut d=ok.clone();d.push(0);cases.push(d);
 for(a,n)in[(0x42000000,4),(SRAM_BASE+SRAM_SIZE as u32-4,8),(LP_BASE+LP_SIZE as u32-4,8),(ROM_BASE,4)]{cases.push(common::image(a,&[(a,vec![0;n])],false));}
 cases.push(common::image(SRAM_BASE,&[(SRAM_BASE,vec![0;8]),(SRAM_BASE+4,vec![0;4])],false));cases.push(common::image(SRAM_BASE+8,&[(SRAM_BASE,vec![0;4])],false));
 for d in cases{let mut b=H2Bus::new();b.sram.fill(0xa5);b.lp_sram.fill(0x5a);b.write_flash(0,&d).unwrap();assert!(b.boot_app(0).is_err());assert!(b.sram.iter().all(|&v|v==0xa5));assert!(b.lp_sram.iter().all(|&v|v==0x5a));}
 let mut b=H2Bus::new();b.set_flash_size(8<<20);assert!(b.write_flash(0,&ok).is_err());assert!(b.boot_app(0).is_err());b.set_flash_size(FLASH_SIZE);b.write_flash(0,&ok).unwrap();assert_eq!(b.boot_app(0).unwrap(),SRAM_BASE);assert!(b.set_psram_size(4<<20).is_err());
}
