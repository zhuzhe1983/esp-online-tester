//! Newly authored RV32 guest GPIO and SDIO mailbox programs, independent of ROMs.
use emu_core::Bus;
use esp_soc::SocBus;
fn li(w:&mut Vec<u32>,r:u32,v:u32) {w.push((v.wrapping_add(0x800)&0xfffff000)|(r<<7)|0x37);w.push(((v&4095)<<20)|(r<<15)|(r<<7)|0x13);}
fn store(w:&mut Vec<u32>,a:u32,v:u32) {li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
fn execute(m:&mut esp32c6::Machine,w:Vec<u32>) {m.console.capture=true;m.console.mask=0;m.console.all.clear();m.bus.load_bytes(0x40800000,&w.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();m.cores[0].pc=0x40800000;m.run(6000);assert_eq!(m.exceptions,0);}
fn coil(m:&mut esp32c6::Machine,p:u32,mux:bool,enabled:bool) {
 let mut w=vec![];store(&mut w,0x60090004+4*p,if mux {1<<12}else{0});store(&mut w,0x60091554+4*p,128);store(&mut w,0x60091004,0);store(&mut w,if enabled {0x60091024}else{0x60091028},1<<p);store(&mut w,0x60091008,1<<p);store(&mut w,0x6009100c,1<<p);
 for b in b"SWITCHC6 COIL\n" {store(&mut w,0x60000000,u32::from(*b));}w.push(0x6f);execute(m,w);assert!(m.console.all.ends_with(b"SWITCHC6 COIL\n"));
}
#[test]fn switchc6_guest_latches_only_its_real_coils_and_retains_relay_through_reset() {
 let mut m=esp32c6::machine([0;6],4<<20);m.bus.board=esp32c6::board::make_board("m5-switchc6").unwrap();m.bus.attach_board_devices();
 for(p,mux,enable)in[(13,true,true),(15,false,true),(15,true,false)] {coil(&mut m,p,mux,enable);assert_eq!(m.bus.board.leds().unwrap().0,[[0;3]]);}
 coil(&mut m,15,true,true);assert_eq!(m.bus.board.leds().unwrap().0,[[0,255,0]]);m.bus.reboot([0;6]);assert_eq!(m.bus.board.leds().unwrap().0,[[0,255,0]]);coil(&mut m,14,true,true);assert_eq!(m.bus.board.leds().unwrap().0,[[0;3]]);
 assert_eq!(m.bus.board.named_pin("button"),Some(7));m.bus.gpio_set_input(7,false);assert_eq!(m.bus.read32(0x6009103c).unwrap()&(1<<7),0);
}
#[test]fn a172_native_sdio_mailbox_requires_every_documented_pad() {
 let mut m=esp32c6::machine([0;6],4<<20);m.bus.board=esp32c6::board::make_board("m5-stamp-addon-c6").unwrap();m.bus.attach_board_devices();assert!(m.bus.sdio_command(5,0).is_err());
 let mut w=vec![];for pin in 18..=23 {store(&mut w,0x60090004+4*pin,1<<9);}
 store(&mut w,0x60016004,0x232013);store(&mut w,0x60018078,0x41313732);
 for b in b"A172 SDIO READY\n" {store(&mut w,0x60000000,u32::from(*b));}w.push(0x6f);execute(&mut m,w);assert!(m.console.all.ends_with(b"A172 SDIO READY\n"));
 m.bus.sdio_command(5,0xff8000).unwrap();m.bus.sdio_command(3,0).unwrap();m.bus.sdio_command(7,1<<16).unwrap();
 // Enable function1 through CCCR, then read a mailbox written by the guest.
 m.bus.sdio_command(52,(1<<31)|(2<<9)|2).unwrap();let r=m.bus.sdio_command(52,(1<<28)|(0x78<<9)).unwrap();assert!(matches!(r,esp_periph::sdmmc::SdResponse::Short(v) if v&255==0x32));
 m.bus.sdio_command(52,(1<<31)|(1<<28)|(0x79<<9)|0x5a).unwrap();assert_eq!(m.bus.read32(0x60018078).unwrap(),0x41315a32);
 for pin in 18..=23 {m.bus.write32(0x60090004+4*pin,1<<12|1<<9).unwrap();assert!(m.bus.sdio_command(52,(1<<28)|(0x78<<9)).is_err(),"wrong IO_MUX pin{pin}");m.bus.write32(0x60090004+4*pin,1<<9).unwrap();}
 m.bus.reboot([0;6]);assert!(m.bus.sdio_command(5,0).is_err(),"MCU reset releases SDIO routes");
}
