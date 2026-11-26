//! Authored Xtensa load/store guests exercise exact K136/K146 matrix,SPI3,I2C0.
//! This is a digital board diagnostic, not official firmware or hardware proof.
//! MMIO: ESP-IDF v5.5.4 S3 spi_reg.h/i2c_reg.h/gpio_sig_map.h, same routed
//! indices as existing S3 bus probes; source receipts in
//! web/platform/knowledge/cloudzao-claw-154.json (ESP-IDF source records).
use emu_core::Core;
use esp_soc::{Stop,web::WebServer};
const IRAM:u32=0x4037_0000;
const GPIO:u32=0x6000_4000;
const SPI:u32=0x6002_5000;
const I2C:u32=0x6001_3000;
fn machine(board:&str)->esp32s3::Machine {
    let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board(board).unwrap();m.bus.attach_board_devices();
    // s32i a2,a3,0; j .  followed by l32i a2,a3,0; j .
    m.bus.load_bytes(IRAM,&[0x22,0x63,0,0x06,0xff,0xff,0,0,0x22,0x23,0,0x06,0xff,0xff]).unwrap();m.cores[0].ps=0;m.cores[0].set_jit(false);m
}
fn write(m:&mut esp32s3::Machine,address:u32,value:u32) {m.cores[0].pc=IRAM;m.cores[0].set_ar(2,value);m.cores[0].set_ar(3,address);assert!(matches!(m.run(64),Stop::MaxInsns));}
fn read(m:&mut esp32s3::Machine,address:u32)->u32 {m.cores[0].pc=IRAM+8;m.cores[0].set_ar(3,address);assert!(matches!(m.run(64),Stop::MaxInsns));m.cores[0].get_ar(2)}
fn mode(m:&mut esp32s3::Machine,pin:u8,signal:u32) {write(m,0x6000_9004+4*u32::from(pin),1<<12|1<<9);write(m,GPIO+0x554+4*u32::from(pin),signal);if signal==256 {write(m,GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));}}
fn output(m:&mut esp32s3::Machine,pin:u8,high:bool) {write(m,GPIO+if pin<32{if high{8}else{12}}else if high{20}else{24},1<<(pin%32));}
fn transfer(m:&mut esp32s3::Machine,bytes:&[u8]) {
    if bytes.is_empty(){return;}write(m,SPI+0x10,1<<27);write(m,SPI+0x1c,(bytes.len()*8-1) as u32);
    for (i,chunk) in bytes.chunks(4).enumerate() {let value=chunk.iter().enumerate().fold(0,|v,(j,b)|v|(u32::from(*b)<<(j*8)));write(m,SPI+0x98+4*i as u32,value);}write(m,SPI,1<<24);
}
fn command(m:&mut esp32s3::Machine,pin:u8,c:u8,data:&[u8]) {output(m,pin,false);transfer(m,&[c]);output(m,pin,true);transfer(m,data);}
fn i2c_route(m:&mut esp32s3::Machine,sda:u8,scl:u8) {for (pin,signal) in [(sda,90),(scl,89)] {mode(m,pin,signal);write(m,GPIO+0x154+4*signal,0x80|u32::from(pin));}}
fn i2c_pointer(m:&mut esp32s3::Machine,address:u8,register:u8)->bool {
    for (off,value) in [(0x24,u32::MAX),(0x18,3<<12),(0x1c,u32::from(address)<<1),(0x1c,u32::from(register)),(0x58,6<<11),(0x5c,1<<11|1<<8|2),(0x60,2<<11),(4,1<<5)] {write(m,I2C+off,value);}
    assert!(matches!(m.run(100_000),Stop::MaxInsns));read(m,I2C+0x20)&(1<<10)==0
}
fn i2c_read(m:&mut esp32s3::Machine,address:u8,n:u32)->Option<Vec<u8>> {
    for (off,value) in [(0x24,u32::MAX),(0x18,3<<12),(0x1c,u32::from(address)<<1|1),(0x58,6<<11),(0x5c,1<<11|1<<8|1),(0x60,3<<11|n),(0x64,2<<11),(4,1<<5)] {write(m,I2C+off,value);}
    assert!(matches!(m.run(100_000),Stop::MaxInsns));(read(m,I2C+0x20)&(1<<10)==0).then(||(0..n).map(|_|read(m,I2C+0x1c) as u8).collect())
}

#[test]
fn vameter_guest_spi_pixels_reset_wrong_route_and_real_encoder_keys() {
 let mut m=machine("vameter");for(pin,signal)in[(36,66),(35,68),(37,71)]{mode(&mut m,pin,signal);}write(&mut m,SPI+0x20,0x3e);
 for p in[34,33,38]{mode(&mut m,p,256);output(&mut m,p,true);}
 for(c,d)in[(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x36,vec![0xc0]),(0x2a,vec![0,0,0,1]),(0x2b,vec![0,0,0,0]),(0x2c,vec![0xf8,0,7,0xe0])]{command(&mut m,34,c,&d);}
 m.bus.flush_ticks();let f=m.bus.board.display().unwrap();assert_eq!((f.0,f.1),(240,240));assert_eq!(&f.2[240*240-2..],&[0x07e0,0xf800]);
 mode(&mut m,35,67);command(&mut m,34,0x2c,&[0,31,255,255]);assert_eq!(m.bus.board.display().unwrap().2,f.2);mode(&mut m,35,68);
 output(&mut m,38,false);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));output(&mut m,38,true);assert_eq!(m.bus.board.display().unwrap().2,f.2);
 output(&mut m,33,false);command(&mut m,34,0x29,&[]);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));output(&mut m,33,true);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));
 assert_eq!(m.bus.board.encoder(),Some((3,1)));let web=WebServer::queued();m.web=Some(web.clone());
 for p in[1u8,2,3]{write(&mut m,0x60009004+4*u32::from(p),1<<12|1<<9);}
 web.push_incoming(r#"{"t":"btn","pin":2,"v":1}"#.into());assert_eq!(read(&mut m,GPIO+0x3c)&4,0);web.push_incoming(r#"{"t":"btn","pin":2,"v":0}"#.into());assert_ne!(read(&mut m,GPIO+0x3c)&4,0);
 web.push_incoming(r#"{"t":"knob","d":1}"#.into());let mut levels=std::collections::BTreeSet::new();for _ in 0..40{let v=read(&mut m,GPIO+0x3c);levels.insert(((v>>3)&1)*2+((v>>1)&1));m.run(80_000);}assert_eq!(levels,[0,1,2,3].into_iter().collect(),"guest sees all four quadrature levels, not a UART substitute");
 i2c_route(&mut m,5,6);assert!(!i2c_pointer(&mut m,0x40,0),"unimplemented INA226 must NACK, not fake a physical measurement");
}
#[test]
fn card_guest_epd_ram_then_busy_refresh_and_retained_pixels() {
 let mut m=machine("xiaozhi-card");for(p,s)in[(45,66),(46,68),(42,71)]{mode(&mut m,p,s);}write(&mut m,SPI+0x20,0x3e);for p in[41,47]{mode(&mut m,p,256);output(&mut m,p,true);}
 for(c,d)in[(0x11,vec![3]),(0x44,vec![0,21]),(0x45,vec![0,0,7,1]),(0x4e,vec![21]),(0x4f,vec![7,1]),(0x24,vec![0xfe])]{command(&mut m,41,c,&d);}
 assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0xffff));command(&mut m,41,0x22,&[0xf7]);command(&mut m,41,0x20,&[]);assert!(m.bus.board.input_levels().contains(&(48,true)));
 command(&mut m,41,0x24,&[0]);m.run(5_000_000);let f=m.bus.board.display().unwrap();assert_eq!((f.0,f.1),(176,264));assert_eq!(f.2.last(),Some(&0));assert_eq!(f.2.iter().filter(|p|**p==0).count(),1);assert_eq!(m.bus.board.display_frames(),1);assert!(m.bus.board.input_levels().contains(&(48,false)));
 mode(&mut m,46,67);command(&mut m,41,0x24,&[0]);command(&mut m,41,0x20,&[]);m.run(5_000_000);assert_eq!(m.bus.board.display_frames(),1);mode(&mut m,46,68);
 output(&mut m,47,false);assert_eq!(m.bus.board.display().unwrap().2,f.2,"electrophoretic image survives reset");command(&mut m,41,0x24,&[0]);output(&mut m,47,true);assert_eq!(m.bus.board.display_frames(),1);
}
#[test]
fn card_guest_touch_reset_wrong_i2c_and_wake_press_release() {
 let mut m=machine("xiaozhi-card");i2c_route(&mut m,2,1);assert!(!i2c_pointer(&mut m,0x38,2),"sharedRST47 not released");mode(&mut m,47,256);output(&mut m,47,true);
 assert!(i2c_pointer(&mut m,0x38,2));assert_eq!(i2c_read(&mut m,0x38,1),Some(vec![0]));m.bus.board.touch(175,263,true);
 for _ in 0..2 {assert!(i2c_pointer(&mut m,0x38,2));assert_eq!(i2c_read(&mut m,0x38,5),Some(vec![1,0x80,175,1,7]));}
 m.bus.board.touch(175,263,false);assert!(i2c_pointer(&mut m,0x38,2));assert_eq!(i2c_read(&mut m,0x38,1),Some(vec![0]));
 i2c_route(&mut m,2,3);assert!(!i2c_pointer(&mut m,0x38,2));i2c_route(&mut m,2,1);assert!(!i2c_pointer(&mut m,0x39,2));assert!(!i2c_pointer(&mut m,0x18,0));assert!(!i2c_pointer(&mut m,0x49,0));assert!(!i2c_pointer(&mut m,0x55,0));
 output(&mut m,47,false);assert!(!i2c_pointer(&mut m,0x38,2));let web=WebServer::queued();m.web=Some(web.clone());write(&mut m,0x60009004+21*4,1<<12|1<<9);web.push_incoming(r#"{"t":"btn","pin":21,"v":1}"#.into());assert_eq!(read(&mut m,GPIO+0x3c)&(1<<21),0);web.push_incoming(r#"{"t":"btn","pin":21,"v":0}"#.into());assert_ne!(read(&mut m,GPIO+0x3c)&(1<<21),0);
}
