//! Newly authored RV32 guest programs; CPU performs HP-I2C and GP-SPI MMIO.
use emu_core::Bus;
use esp_soc::SocBus;
const I:u32=0x60004000;const S:u32=0x60081000;
fn li(w:&mut Vec<u32>,r:u32,v:u32) {w.push((v.wrapping_add(0x800)&0xfffff000)|(r<<7)|0x37);w.push(((v&4095)<<20)|(r<<15)|(r<<7)|0x13);}
fn store(w:&mut Vec<u32>,a:u32,v:u32) {li(w,5,a);li(w,6,v);w.push((6<<20)|(5<<15)|(2<<12)|0x23);}
fn wait(w:&mut Vec<u32>,a:u32,mask:u32,until_set:bool) {li(w,5,a);li(w,7,mask);w.push((5<<15)|(2<<12)|(6<<7)|3);w.push((7<<20)|(6<<15)|(7<<12)|(6<<7)|0x33);w.push(if until_set {0xfe030ce3}else{0xfe031ce3});}
fn pin(w:&mut Vec<u32>,p:u32,s:u32) {store(w,0x60090000+4*p,0x1a00);store(w,0x60091ad4+4*p,s);store(w,0x60091038,1<<p);}
fn i2c(w:&mut Vec<u32>,addr:u32,reg:u32,value:Option<u32>,count:u32) {
 store(w,I+0x24,0x7ffff);store(w,I+0x18,0x408b|3<<12);store(w,I+0x1c,addr<<1);store(w,I+0x1c,reg);store(w,I+0x58,6<<11);
 if let Some(v)=value {store(w,I+0x1c,v);store(w,I+0x5c,1<<11|1<<8|3);store(w,I+0x60,2<<11);}
 else {store(w,I+0x1c,addr<<1|1);for(o,v)in[(0x5c,1<<11|1<<8|2),(0x60,6<<11),(0x64,1<<11|1<<8|1),(0x68,3<<11|(count-1)),(0x6c,3<<11|1<<10|1),(0x70,2<<11)] {store(w,I+o,v);}}
 store(w,I+4,1<<4|1<<5);wait(w,I+0x20,1<<7|1<<10,true);
}
fn lcd(w:&mut Vec<u32>,c:u8,data:&[u8]) {
 for(dc,bytes)in[(false,vec![c]),(true,data.to_vec())] {if bytes.is_empty(){continue;}store(w,if dc {0x60091008}else{0x6009100c},1<<26);store(w,S+0x10,1<<27);store(w,S+0x1c,bytes.len()as u32*8-1);let mut b=[0;4];b[..bytes.len()].copy_from_slice(&bytes);store(w,S+0x98,u32::from_le_bytes(b));store(w,S,1<<24);wait(w,S,1<<24,false);}
}
fn run(m:&mut esp32c5::Machine,mut w:Vec<u32>,marker:&[u8]) {for b in marker {store(&mut w,0x60000000,u32::from(*b));}w.push(0x6f);m.console.capture=true;m.console.mask=0;m.console.all.clear();m.bus.load_bytes(0x40800000,&w.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();m.cores[0].pc=0x40800000;m.run(2_000_000);assert_eq!(m.exceptions,0,"{:?}",m.bus.first_fault);assert!(m.console.all.ends_with(marker),"{:?} {}",m.bus.first_fault,String::from_utf8_lossy(&m.console.all));}
fn display_program()->Vec<u32> {
 let mut w=vec![];store(&mut w,0x60096020,3);store(&mut w,0x60096020,1);store(&mut w,0x60096024,1<<22);
 pin(&mut w,2,47);pin(&mut w,3,46);store(&mut w,0x600912d4+4*47,0x102);store(&mut w,0x600912d4+4*46,0x103);
 store(&mut w,I,239);store(&mut w,I+0x38,240);for o in[0x40,0x44,0x48,0x4c] {store(&mut w,I+o,0);}
 for(a,r,v)in[(0x4f,0x13,0xe7),(0x4f,0x03,0x18),(0x4f,0x05,0x18),(0x4f,0x14,0x3d),(0x4f,0x04,2),(0x4f,0x25,0xe8),(0x4f,0x26,3),(0x4f,0x21,255),(0x4f,0x22,0x8f),(0x6e,0x10,4),(0x6e,0x11,4),(0x6e,0x13,0)] {i2c(&mut w,a,r,Some(v),0);}
 for(p,s)in[(9,56),(7,58),(25,61),(26,256)] {pin(&mut w,p,s);}store(&mut w,S+0xc,1<<12);store(&mut w,S+0x20,0x3e);
 for(c,d)in[(0x11,&[][..]),(0x21,&[][..]),(0x29,&[][..]),(0x3a,&[0x55][..]),(0x2a,&[0,0,0,0][..]),(0x2b,&[0,0,0,0][..]),(0x2c,&[0xf8,0][..])] {lcd(&mut w,c,d);}w
}
#[test]fn toughc5_guest_pixels_and_touch_obey_expander_power_reset_and_real_buses() {
 let mut m=esp32c5::machine([0;6],16<<20);m.bus.configure_psram(8<<20).unwrap();m.bus.attach_board(esp32c5::board::make_board("m5-tough-c5").unwrap());run(&mut m,display_program(),b"TOUGH C5 PIXEL\n");assert_eq!(m.bus.board.display().unwrap().2[0],0xf800);
 let cycle=m.bus.cycles();m.bus.board.touch_at(cycle,123,87,true);let mut w=vec![];i2c(&mut w,0x2e,2,None,5);for n in 0..5 {li(&mut w,5,I+0x1c);w.push((5<<15)|(2<<12)|(7<<7)|3);li(&mut w,5,0x40804000+4*n);w.push((7<<20)|(5<<15)|(2<<12)|0x23);}run(&mut m,w,b"TOUGH TOUCH READ\n");assert_eq!((0..5).map(|n|m.bus.read32(0x40804000+4*n).unwrap()).collect::<Vec<_>>(),[1,0,123,0,87]);
 let mut w=vec![];i2c(&mut w,0x4f,0x22,Some(0),0);run(&mut m,w,b"TOUGH BACKLIGHT OFF\n");assert!(m.bus.board.display().unwrap().2.iter().all(|v|*v==0));
 let mut w=vec![];i2c(&mut w,0x4f,0x22,Some(0x8f),0);i2c(&mut w,0x4f,5,Some(0x10),0);run(&mut m,w,b"TOUGH LCD RESET\n");assert!(m.bus.board.display().unwrap().2.iter().all(|v|*v==0));
 let mut w=vec![];i2c(&mut w,0x6e,0x11,Some(0),0);i2c(&mut w,0x2e,2,None,5);li(&mut w,5,I+0x20);w.push((5<<15)|(2<<12)|(7<<7)|3);li(&mut w,5,0x40804020);w.push((7<<20)|(5<<15)|(2<<12)|0x23);run(&mut m,w,b"TOUGH TOUCH RESET NACK\n");assert_ne!(m.bus.read32(0x40804020).unwrap()&(1<<10),0);
}
