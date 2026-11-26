//! U213 board IO and physical EMAC contract. The Ethernet DMA guest acceptance
//! is in the shared MAC adapter tests, independent of these button/LED checks.
use emu_core::Bus;
use esp_soc::SocBus as _;
const ENTRY:u32=0x4ff1_0000;
const RESULT:u32=0x4ff0_2000;
fn li(c:&mut Vec<u32>,r:u32,v:u32){c.push((v.wrapping_add(0x800)&0xffff_f000)|(r<<7)|0x37);c.push(((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
fn write(c:&mut Vec<u32>,a:u32,v:u32){li(c,1,a);li(c,2,v);c.push(2<<20|1<<15|2<<12|0x23);}
fn run(m:&mut esp32p4::Machine,c:Vec<u32>){let mut code=Vec::new();li(&mut code,3,10_000);code.extend([0xfff1_8193,0xfe01_9ee3]);code.extend(c);li(&mut code,1,0x500e_0040);code.push(1<<15|2<<12|2<<7|3);li(&mut code,1,RESULT);code.push(2<<20|1<<15|2<<12|0x23);code.push(0x0000_006f);m.bus.load_bytes(ENTRY,&code.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();m.cores[0].pc=ENTRY;m.run(50_000);assert_eq!(m.exceptions,0);}
#[test]
fn u213_guest_reads_user45_and_drives_source_correct_common_anode_rgb(){
    let mut m=esp32p4::machine([0;6],16<<20);m.bus.board=esp32p4::board::make_board("m5stack-poe-p4").unwrap();m.bus.attach_board_devices();m.boot_rom();
    let e=m.bus.board.emac_config().unwrap();assert_eq!(e,esp32p4::board::poe_p4::EMAC);assert_eq!((e.phy_addr,e.mdc,e.mdio,e.rmii,e.reset_pin),(1,31,52,[49,34,35,29,30,28],Some(51)));assert!(m.bus.board.display().is_none());
    run(&mut m,Vec::new());assert_ne!(m.bus.read32(RESULT).unwrap()&(1<<13),0);
    assert!(m.bus.board.button_at(m.bus.cycles,"user",true));run(&mut m,Vec::new());assert_eq!(m.bus.read32(RESULT).unwrap()&(1<<13),0);
    let mut c=Vec::new();for pin in [17,18]{write(&mut c,0x500e_1004+pin*4,1<<12|1<<9);write(&mut c,0x500e_0558+pin*4,256);}write(&mut c,0x500e_0024,1<<18);write(&mut c,0x500e_000c,1<<18);run(&mut m,c);assert_eq!(m.bus.board.leds().unwrap().0,[[0,0,0]],"wrong physical GPIO18 cannot light the red die");
    let mut c=Vec::new();write(&mut c,0x500e_0024,1<<17);write(&mut c,0x500e_000c,1<<17);run(&mut m,c);assert_eq!(m.bus.board.leds().unwrap().0,[[255,0,0]],"common-anode red GPIO17 is active low");
    let mut c=Vec::new();write(&mut c,0x500e_0008,1<<17);run(&mut m,c);assert_eq!(m.bus.board.leds().unwrap().0,[[0,0,0]]);
    let mut c=Vec::new();write(&mut c,0x500e_000c,1<<17);run(&mut m,c);m.bus.reboot([0;6]);assert_eq!(m.bus.board.leds().unwrap().0,[[0,0,0]],"MCU reset removes GPIO drive; stale LED state is not output");assert!(m.bus.board.button_at(m.bus.cycles,"user",false));run(&mut m,Vec::new());assert_ne!(m.bus.read32(RESULT).unwrap()&(1<<13),0);
}
