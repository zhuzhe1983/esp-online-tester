mod common;
use emu_core::Bus;
use esp_soc::SocBus;
fn guest(tx:u32,rx:u32,enabled:bool,div:u32)->Vec<u8> {
 let mut w=vec![];common::store(&mut w,0x60090004+4*24,1<<12);common::store(&mut w,0x60091554+4*24,128);common::store(&mut w,0x60000014,div);
 for p in [tx,rx] {common::store(&mut w,0x60090004+4*p,1<<12|1<<9);}
 common::store(&mut w,0x60091554+4*tx,6);common::store(&mut w,0x60091154+4*6,0x80|rx);common::store(&mut w,0x60091024,if enabled {1<<tx}else{0});
 for b in b"H2 HOST READY\n" {common::store(&mut w,0x60000000,u32::from(*b));}
 // Poll actual FIFO count, read one received character, store and echo it.
 common::li(&mut w,5,0x6000001c);w.push((5<<15)|(2<<12)|(6<<7)|3);w.push((255<<20)|(6<<15)|(7<<12)|(6<<7)|0x13);w.push(0xfe030ce3);
 common::li(&mut w,5,0x60000000);w.push((5<<15)|(2<<12)|(7<<7)|3);w.push((7<<20)|(5<<15)|(2<<12)|0x23);common::li(&mut w,5,0x40804000);w.push((7<<20)|(5<<15)|(2<<12)|0x23);w.push(0x6f);
 let data=w.into_iter().flat_map(u32::to_le_bytes).collect();let mut image=common::image(0x40800000,&[(0x40800000,data)],false);image[3]=0x10;image
}
fn run(board:&str,tx:u32,rx:u32,on:bool,div:u32)->esp32h2::Machine {
 let mut m=esp32h2::machine([0;6],2<<20);m.console.capture=true;m.console.mask=0;m.bus.attach_board(esp32h2::board::make_board(board).unwrap());
 m.bus.write_flash(0,&guest(tx,rx,on,div)).unwrap();m.boot_app(0).unwrap();m.run(3000);assert!(m.console.all.ends_with(b"H2 HOST READY\n"));assert_eq!(m.exceptions,0);m
}
#[test]fn two_exact_h2_gateways_execute_source_routed_uart_and_capacity_contracts() {
 for board in ["m5-unit-gateway-h2","m5-module-gateway-h2"] {
  let mut m=run(board,24,23,true,0xc00115);assert!(m.bus.board.report().contains("tx_count=14"));m.bus.uart_input(0,b"A");m.run(1000);assert_eq!(m.bus.read32(0x40804000).unwrap(),65);assert!(m.console.all.ends_with(b"A"));
  for (tx,rx,on,div) in [(25,23,true,0xc00115),(24,22,true,0xc00115),(24,23,false,0xc00115),(24,23,true,694)] {
   let mut n=run(board,tx,rx,on,div);n.bus.uart_input(0,b"B");n.run(1000);
   if rx!=23||div==694 {assert_eq!(n.bus.read32(0x40804000).unwrap(),0,"wrong host RX pin/baud is disconnected");}
   if tx!=24||!on||div==694 {assert!(n.bus.board.report().contains("tx_count=0"),"wrong host TX pin/drive/baud");}
  }
  let mut bad=guest(24,23,true,0xc00115);bad[3]=0x20;assert!(m.bus.write_flash(0,&bad).is_ok());assert!(m.boot_app(0).is_err(),"4MiB image cannot claim N2 capacity");
  assert!(m.bus.write_flash((2<<20)-1,&[1,2]).is_err());assert!(m.bus.read32(0x42000000).is_err());assert!(m.bus.read32(0x600a0000).is_err());
  m.bus.gpio_set_input(9,false);assert_eq!(m.bus.gpio_input()&(1<<9),0);
 }
}
