//! Real RV32 guest SPI controller reads a modeled MAX31855 frame; no patched return.
use emu_core::Bus;
use esp_soc::SocBus;
use esp_periph::i2c::env4::Environment;
fn li(w:&mut Vec<u32>,r:u32,v:u32) {w.push((v.wrapping_add(0x800)&0xfffff000)|(r<<7)|0x37);w.push(((v&4095)<<20)|(r<<15)|(r<<7)|0x13);}
fn store(w:&mut Vec<u32>,a:u32,v:u32) {li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
fn sample(m:&mut esp32c3::Machine,miso:u32,clock:u32)->u32 {
 let mut w=vec![];store(&mut w,0x60004554+4*6,128);
 for (p,s) in [(clock,63),(7,68)] {store(&mut w,0x60009004+4*p,1<<12|1<<9);store(&mut w,0x60004554+4*p,s);}
 store(&mut w,0x60009004+4*miso,1<<12|1<<9);store(&mut w,0x60004154+64*4,0x40|miso);
 store(&mut w,0x60024020,0x3e);store(&mut w,0x60024010,1<<28);store(&mut w,0x6002401c,31);store(&mut w,0x60024000,1<<24);
 li(&mut w,5,0x60024098);w.push((5<<15)|(2<<12)|(7<<7)|3);li(&mut w,5,0x3fc88000);w.push((7<<20)|(5<<15)|(2<<12)|0x23);
 for b in b"KMETER SPI READ\n" {store(&mut w,0x60000000,u32::from(*b));}w.push(0x6f);
 m.console.capture=true;m.console.mask=0;m.console.all.clear();m.bus.load_bytes(0x40380000,&w.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();m.cores[0].pc=0x40380000;m.run(6000);assert_eq!(m.exceptions,0);assert!(m.console.all.ends_with(b"KMETER SPI READ\n"));m.bus.read32(0x3fc88000).unwrap().swap_bytes()
}
#[test]fn guest_reads_signed_thermocouple_conversion_and_open_probe_with_route_negatives() {
 let mut m=esp32c3::machine([0;6],4<<20);m.bus.board=esp32c3::board::make_board("m5-kmeter").unwrap();m.bus.attach_board_devices();
 assert_eq!(sample(&mut m,5,6),0x00011901,"open probe is the initial physical environment");
 for temp in [123.25,-30.5,1350.0] {
  let cycle=m.bus.cycles();assert!(m.bus.board.accessory_environment_at(cycle,"max31855k","thermocouple",true,Environment{temperature_c:temp,..Default::default()}));
  m.bus.board.advance_to(cycle+esp32c3::periph::CPU_HZ/10);let word=sample(&mut m,5,6);assert_eq!((word as i32)>>18,(temp*4.0)as i32);assert_eq!((word>>4)&4095,400);
 }
 assert_eq!(sample(&mut m,4,6),u32::MAX);assert_eq!(sample(&mut m,5,8),u32::MAX);
 let cycle=m.bus.cycles();assert!(m.bus.board.accessory_environment_at(cycle,"max31855k","thermocouple",false,Environment::default()));m.bus.board.advance_to(cycle+esp32c3::periph::CPU_HZ/10);assert_eq!(sample(&mut m,5,6),0x00011901);
 assert!(!m.bus.board.accessory_environment_at(cycle,"max31855k","thermocouple",true,Environment{temperature_c:1400.0,..Default::default()}));
}
