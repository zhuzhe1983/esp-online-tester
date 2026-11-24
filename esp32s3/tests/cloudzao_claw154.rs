//! Authored Xtensa instructions exercise the real GPIO matrix, SPI3 and I2C0.
//! This is a digital board diagnostic, not official firmware or hardware proof.
//! MMIO: ESP-IDF v5.5.4 S3 spi_reg.h/i2c_reg.h/gpio_sig_map.h, same routed
//! indices as the existing S3 bus probes; receipts in cloudzao-claw-154.json.
use emu_core::Core;
use esp_soc::{Stop,web::WebServer};
const IRAM:u32=0x4037_0000;
const GPIO:u32=0x6000_4000;
const SPI:u32=0x6002_5000;
const I2C:u32=0x6001_3000;
fn machine()->esp32s3::Machine {
    let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board("cloudzao-claw-154").unwrap();m.bus.attach_board_devices();
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
fn command(m:&mut esp32s3::Machine,c:u8,data:&[u8]) {output(m,8,false);transfer(m,&[c]);output(m,8,true);transfer(m,data);}
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
fn guest_spi3_routes_screen_pixels_backlight_reset_and_wrong_wiring() {
    let mut m=machine();for (pin,signal) in [(9,66),(10,68),(14,71)] {mode(&mut m,pin,signal);}write(&mut m,SPI+0x20,0x3e);
    for pin in [8,13,18] {mode(&mut m,pin,256);output(&mut m,pin,true);}
    for (c,data) in [(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x2a,vec![0,0,0,1]),(0x2b,vec![0,0,0,0]),(0x2c,vec![0xf8,0,7,0xe0])] {command(&mut m,c,&data);}
    m.bus.flush_ticks();let frame=m.bus.board.display().unwrap();assert_eq!((frame.0,frame.1),(240,240));assert_eq!(&frame.2[..2],&[0xf800,0x07e0]);
    mode(&mut m,10,67);command(&mut m,0x2c,&[0,31,255,255]);assert_eq!(m.bus.board.display().unwrap().2,frame.2,"wrong MOSI route cannot write");
    mode(&mut m,10,68);output(&mut m,13,false);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));output(&mut m,13,true);assert_eq!(m.bus.board.display().unwrap().2,frame.2,"backlight does not erase GRAM");
    output(&mut m,18,false);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));output(&mut m,18,true);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0),"reset needs new guest initialization");
}
#[test]
fn guest_polled_i2c_rejects_wrong_pins_and_returns_real_touch_registers() {
    let mut m=machine();i2c_route(&mut m,41,42);assert!(i2c_pointer(&mut m,0x15,2));assert_eq!(i2c_read(&mut m,0x15,5),Some(vec![0;5]));
    m.bus.board.touch(22,37,true);assert!(i2c_pointer(&mut m,0x15,2));assert_eq!(i2c_read(&mut m,0x15,5),Some(vec![1,0,218,0,203]));
    m.bus.board.touch(22,37,false);assert!(i2c_pointer(&mut m,0x15,2));assert_eq!(i2c_read(&mut m,0x15,1),Some(vec![0]));
    assert!(m.bus.board.button_at(0,"home",true));assert!(i2c_pointer(&mut m,0x15,2));assert_eq!(i2c_read(&mut m,0x15,5),Some(vec![1,0,40,2,88]));
    assert!(m.bus.board.button_at(0,"home",false));assert!(i2c_pointer(&mut m,0x15,2));assert_eq!(i2c_read(&mut m,0x15,1),Some(vec![0]));
    i2c_route(&mut m,41,40);assert!(!i2c_pointer(&mut m,0x15,2));i2c_route(&mut m,41,42);assert!(!i2c_pointer(&mut m,0x38,2));assert!(!i2c_pointer(&mut m,0x18,0),"ES8311 registers are not invented");
    assert!(m.bus.board.touch_irq().is_none());
}
#[test]
fn guest_reads_boot_and_power_press_release_including_paused_release() {
    let mut m=machine();let web=WebServer::queued();m.web=Some(web.clone());
    for pin in [0u8,47] {
        write(&mut m,0x6000_9004+4*u32::from(pin),1<<12|1<<9);let address=GPIO+if pin<32{0x3c}else{0x40};let mask=1<<(pin%32);
        assert_ne!(read(&mut m,address)&mask,0);web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":1}}"#));assert_eq!(read(&mut m,address)&mask,0);
        web.push_incoming(format!(r#"{{"t":"btn","pin":{pin},"v":0}}"#));m.run(0);assert_ne!(read(&mut m,address)&mask,0);
    }
}
