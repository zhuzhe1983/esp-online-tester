//! Newly authored RV32 SRAM guest, no ROM stubs or manufacturer compatibility claim.
use emu_core::Bus;
use esp_soc::SocBus;
fn li(w:&mut Vec<u32>,r:u32,v:u32) {w.push((v.wrapping_add(0x800)&0xfffff000)|(r<<7)|0x37);w.push(((v&4095)<<20)|(r<<15)|(r<<7)|0x13);}
fn store(w:&mut Vec<u32>,a:u32,v:u32) {li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
fn guest(m:&mut esp32c3::Machine,pin:u32,on:bool,mux:bool,enabled:bool) {
 m.console.capture=true;m.console.mask=0;m.console.all.clear();let mut w=vec![];
 store(&mut w,0x60009004+4*pin,if mux {1<<12}else{0});store(&mut w,0x60004554+4*pin,128);
 store(&mut w,if enabled {0x60004024}else{0x60004028},1<<pin);store(&mut w,if on {0x60004008}else{0x6000400c},1<<pin);
 store(&mut w,0x60009010,1<<12|1<<9);li(&mut w,5,0x6000403c);w.push((5<<15)|(2<<12)|(7<<7)|3);li(&mut w,5,0x3fc88000);w.push((7<<20)|(5<<15)|(2<<12)|0x23);
 for b in b"SSR GPIO GUEST\n" {store(&mut w,0x60000000,u32::from(*b));}w.push(0x6f);
 let bytes:Vec<_>=w.into_iter().flat_map(u32::to_le_bytes).collect();m.bus.load_bytes(0x40380000,&bytes).unwrap();m.cores[0].pc=0x40380000;m.run(5000);
 assert_eq!(m.exceptions,0);assert!(m.console.all.ends_with(b"SSR GPIO GUEST\n"));
}
#[test]fn each_ssr_executes_gpio_control_and_rejects_wrong_route_enable_and_reset() {
 for board in ["m5-acssr","m5-dcssr"] {
  let mut m=esp32c3::machine([0;6],4<<20);m.bus.board=esp32c3::board::make_board(board).unwrap();m.bus.attach_board_devices();
  guest(&mut m,5,true,true,true);assert_eq!(m.bus.board.leds().unwrap().0[1],[0;3]);
  guest(&mut m,4,true,true,true);assert_eq!(m.bus.board.leds().unwrap().0[1],[255,0,0]);
  guest(&mut m,4,true,true,false);assert_eq!(m.bus.board.leds().unwrap().0[1],[0;3]);
  guest(&mut m,4,true,false,true);assert_eq!(m.bus.board.leds().unwrap().0[1],[0;3]);
  guest(&mut m,4,true,true,true);m.bus.gpio_set_input(3,false);guest(&mut m,4,false,true,true);assert_eq!(m.bus.read32(0x3fc88000).unwrap()&(1<<3),0);
  guest(&mut m,4,true,true,true);m.bus.reboot([0;6]);assert_eq!(m.bus.board.leds().unwrap().0[1],[0;3]);
  let bits=vec![true;24];m.bus.board.rmt_frame(3,&bits);assert_eq!(m.bus.board.leds().unwrap().0[0],[0;3]);m.bus.board.rmt_frame(2,&bits);assert_eq!(m.bus.board.leds().unwrap().0[0],[255;3]);
 }
}
