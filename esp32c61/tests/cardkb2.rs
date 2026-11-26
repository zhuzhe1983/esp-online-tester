//! Newly authored RV32 program performs the physical row/column scan itself.
use emu_core::Bus;
use esp_soc::SocBus;
use esp32c61::board::cardkb2::COLUMNS;
fn li(w:&mut Vec<u32>,r:u32,v:u32) {w.push((v.wrapping_add(0x800)&0xfffff000)|(r<<7)|0x37);w.push(((v&4095)<<20)|(r<<15)|(r<<7)|0x13);}
fn store(w:&mut Vec<u32>,a:u32,v:u32) {li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
fn program(bad_mux:bool,disabled:bool)->Vec<u8> {
 let mut w=vec![];
 for p in 0..4 {store(&mut w,0x60090000+4*p,if bad_mux {0}else{1<<12});store(&mut w,0x60091ad4+4*p,256);}
 for p in COLUMNS {store(&mut w,0x60090000+4*u32::from(p),1<<12|1<<9|1<<7);}
 store(&mut w,0x6009103c,15);store(&mut w,0x60091038,if disabled {0}else{15});
 for r in 0..4 {
  store(&mut w,0x60091004,1<<r);li(&mut w,5,0x60091064);w.push((5<<15)|(2<<12)|(7<<7)|3);li(&mut w,5,0x40804000+4*r);w.push((7<<20)|(5<<15)|(2<<12)|0x23);
 }
 store(&mut w,0x60091004,0);
 for b in b"CARDKB2 SCAN42\n" {store(&mut w,0x60000000,u32::from(*b));}w.push(0x6f);w.into_iter().flat_map(u32::to_le_bytes).collect()
}
fn scan(m:&mut esp32c61::Machine,bad:bool,off:bool)->[u32;4] {
 m.console.capture=true;m.console.mask=0;m.console.all.clear();m.bus.load_bytes(0x40800000,&program(bad,off)).unwrap();m.cores[0].pc=0x40800000;m.run(6000);assert_eq!(m.exceptions,0);
 assert!(m.console.all.ends_with(b"CARDKB2 SCAN42\n"));std::array::from_fn(|r|m.bus.read32(0x40804000+4*r as u32).unwrap())
}
#[test]fn every_populated_key_is_scanned_by_the_c61_guest() {
 let mut m=esp32c61::machine([0;6],4<<20);m.bus.board=esp32c61::board::make_board("m5-cardkb2").unwrap();m.bus.attach_board_devices();
 let mask=COLUMNS.into_iter().fold(0,|m,p|m|1<<p);assert_eq!(scan(&mut m,false,false).map(|v|v&mask),[0;4]);
 for r in 0..4 {for c in 0..11 {let valid=!(c==10&&(r==0||r==3));let cycle=m.bus.cycles();assert_eq!(m.bus.board.matrix_key_at(cycle,r,c,true),valid);if !valid {continue;}
  let read=scan(&mut m,false,false).map(|v|v&mask);let mut want=[0;4];want[r as usize]=1<<COLUMNS[c as usize];assert_eq!(read,want,"r{r}c{c}");
  let cycle=m.bus.cycles();assert!(m.bus.board.matrix_key_at(cycle,r,c,false));assert_eq!(scan(&mut m,false,false).map(|v|v&mask),[0;4]);
 }}
 assert!(!m.bus.board.matrix_key_at(0,4,0,true));assert!(!m.bus.board.matrix_key_at(0,0,11,true));
 m.bus.board.matrix_key_at(0,2,3,true);assert_eq!(scan(&mut m,true,false).map(|v|v&mask),[0;4]);assert_eq!(scan(&mut m,false,true).map(|v|v&mask),[0;4]);
 let mut want=[0;4];want[2]=1<<7;assert_eq!(scan(&mut m,false,false).map(|v|v&mask),want);m.bus.reboot([0;6]);assert_eq!(scan(&mut m,false,false).map(|v|v&mask),want,"held physical switch survives MCU reset");
}
