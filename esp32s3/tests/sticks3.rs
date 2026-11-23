//! K150 SoC pin routing and guest GPIO read probe, not an official firmware boot.
//! MMIO conventions: ESP-IDF v4.4.4 esp32s3 gpio_reg.h/spi_reg.h/i2c_reg.h,
//! matching existing pin_transport.rs tests. This does not assert silicon timing.
use emu_core::{Bus,Core};
use esp_soc::{Stop,web::WebServer};
fn route_i2c(b:&mut esp32s3::bus::SocBus,sda:u32,scl:u32) {
    for (pin,sig) in [(sda,92),(scl,91)] {b.write32(0x60009004+4*pin,1<<12|1<<9).unwrap();b.write32(0x60004554+4*pin,sig).unwrap();b.write32(0x60004154+4*sig,0x80|pin).unwrap();}
}
fn i2c(b:&mut esp32s3::bus::SocBus,address:u8,data:&[u8],read:bool)->(bool,u8) {
    let base=0x60027000;for (o,v) in [(0x24,u32::MAX),(0x18,3<<12)] {b.write32(base+o,v).unwrap();}
    b.write32(base+0x1c,((address<<1)|u8::from(read)) as u32).unwrap();for &x in data {b.write32(base+0x1c,x as u32).unwrap();}
    let cmds=if read {vec![6<<11,1<<11|1<<8|1,3<<11|1,2<<11]}else{vec![6<<11,1<<11|1<<8|(data.len() as u32+1),2<<11]};
    for (i,v) in cmds.into_iter().enumerate() {b.write32(base+0x58+4*i as u32,v).unwrap();}b.write32(base+4,1<<5).unwrap();b.tick(100_000);b.flush_ticks();
    (b.read32(base+0x20).unwrap()&(1<<10)==0,b.read32(base+0x1c).unwrap() as u8)
}
fn wr(b:&mut esp32s3::bus::SocBus,address:u8,reg:u8,value:u8) {assert!(i2c(b,address,&[reg,value],false).0);}
fn rd(b:&mut esp32s3::bus::SocBus,address:u8,reg:u8)->u8 {assert!(i2c(b,address,&[reg],false).0);let (ack,v)=i2c(b,address,&[],true);assert!(ack);v}
fn level(b:&mut esp32s3::bus::SocBus,pin:u32,high:bool) {
    b.write32(0x60009004+4*pin,1<<12).unwrap();b.write32(0x60004554+4*pin,256).unwrap();
    b.write32(if pin<32{0x60004024}else{0x60004030},1<<(pin%32)).unwrap();b.write32(match (pin<32,high){(true,true)=>0x60004008,(true,false)=>0x6000400c,(false,true)=>0x60004014,(false,false)=>0x60004018},1<<(pin%32)).unwrap();
}
fn spi(b:&mut esp32s3::bus::SocBus,dc:bool,bytes:&[u8]) {
    level(b,45,dc);for (i,chunk) in bytes.chunks(4).enumerate(){let mut word=[0;4];word[..chunk.len()].copy_from_slice(chunk);b.write32(0x60024098+4*i as u32,u32::from_le_bytes(word)).unwrap();}
    for (o,v) in [(0x10,1<<27),(0x1c,(bytes.len()*8-1) as u32),(0,1<<24)] {b.write32(0x60024000+o,v).unwrap();}b.tick(100_000);b.flush_ticks();
}
#[test]
fn routed_i2c_power_and_spi_framebuffer_have_independent_wrong_pin_failures() {
    let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board("sticks3").unwrap();m.bus.attach_board_devices();let b=&mut m.bus;
    route_i2c(b,47,48);assert_eq!(rd(b,0x68,0),0x24);assert_eq!(rd(b,0x18,0xfd),0x83);assert_eq!(rd(b,0x18,0xfe),0x11);assert_eq!(rd(b,0x6e,0),0x50);
    // Grove 9/10 is NOT the internal I2C bus; no silent controller-only device.
    route_i2c(b,9,10);assert!(!i2c(b,0x68,&[0],false).0);route_i2c(b,47,48);
    for (reg,v) in [(0x16,0),(0x10,4),(0x13,0x1b),(0x11,4)] {wr(b,0x6e,reg,v);}
    level(b,21,true);level(b,38,true);
    for (pin,sig) in [(40,101),(39,103),(41,110)] {b.write32(0x60009004+4*pin,1<<12|1<<9).unwrap();b.write32(0x60004554+4*pin,sig).unwrap();}b.write32(0x60024020,0x3e).unwrap();
    for (c,d) in [(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x2a,vec![0,52,0,53]),(0x2b,vec![0,40,0,40]),(0x2c,vec![0xf8,0,0x07,0xe0])] {spi(b,false,&[c]);if !d.is_empty(){spi(b,true,&d);}}
    assert_eq!(&b.board.display().unwrap().2[..2],&[0xf800,0x07e0]);println!("K150 MMIO probe: BMI270=24 ES8311=83/11 PM1=50 framebuffer[0..2]=f800,07e0");
    b.write32(0x60004554+4*40,100).unwrap();spi(b,false,&[0x28]);assert_eq!(b.board.display().unwrap().2[0],0xf800,"wrong clock cannot turn panel off");
    wr(b,0x6e,0x11,0);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));wr(b,0x6e,0x11,4);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));
}
#[test]
fn queued_buttons_reach_actual_guest_gpio_reads_and_pico_identity() {
    let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board("m5stack-k150").unwrap();esp32s3::board::configure_chip_identity(m.bus.board.name(),&mut m.bus.periph.efuse);m.bus.attach_board_devices();
    assert_eq!((m.bus.periph.efuse.ram.read(0x50)>>21)&7,1);let web=WebServer::queued();m.web=Some(web.clone());
    for pin in [11,12] {m.bus.write32(0x60009004+4*pin,1<<12|1<<9).unwrap();}
    const IRAM:u32=0x40370000;m.bus.load_bytes(IRAM,&[0x22,0x23,0,0x06,0xff,0xff]).unwrap(); // l32i a2,a3,0; j .
    m.cores[0].ps=0;m.cores[0].set_ar(3,0x6000403c);m.cores[0].set_jit(false);
    let read=|m:&mut esp32s3::Machine|{m.cores[0].pc=IRAM;assert!(matches!(m.run(64),Stop::MaxInsns));m.cores[0].get_ar(2)&((1<<11)|(1<<12))};
    assert_eq!(read(&mut m),0x1800);for (pin,remaining) in [(11,1<<12),(12,1<<11)] {web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));assert_eq!(read(&mut m),remaining);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));assert_eq!(read(&mut m),0x1800);}
    for pin in [11,12] {web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));}assert_eq!(read(&mut m),0);
    for pin in [11,12] {web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));}m.run(0);assert_eq!(read(&mut m),0x1800);
    println!("K150 guest instruction probe: GPIO11/12 idle=1800, individual=1000/0800, both=0000, released=1800; not official firmware");
}
