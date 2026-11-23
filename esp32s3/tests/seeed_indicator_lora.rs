//! Real S3 I2C/SPI/GPIO MMIO across the Indicator's expander. ESP-IDF v4.4.7
//! soc/{gpio,i2c,spi}_reg.h and gpio_sig_map.h (factory IDF4.4.7): I2C0
//! SDA/SCL=90/89; FSPI CLK/D/Q=101/103/102; software GPIO=256.
use emu_core::Bus;

fn machine(id:&str)->esp32s3::Machine {let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board(id).unwrap();m.bus.attach_board_devices();m}
fn mux(b:&mut esp32s3::bus::SocBus,p:u8,s:u32) {b.write32(0x6000_9004+4*u32::from(p),1<<12|1<<9).unwrap();b.write32(0x6000_4554+4*u32::from(p),s).unwrap();}
fn i2c_route(b:&mut esp32s3::bus::SocBus,sda:u8,scl:u8) {for (p,s) in [(sda,90),(scl,89)] {mux(b,p,s);b.write32(0x6000_4154+4*s,0x80|u32::from(p)).unwrap();}}
fn i2c(b:&mut esp32s3::bus::SocBus,register:u8,data:&[u8],read:bool)->(bool,u8) {
    let base=0x6001_3000;b.write32(base+0x24,u32::MAX).unwrap();b.write32(base+0x18,3<<12).unwrap();
    b.write32(base+0x1c,0x40).unwrap();b.write32(base+0x1c,u32::from(register)).unwrap();for &v in data {b.write32(base+0x1c,u32::from(v)).unwrap();}
    let mut commands=vec![6<<11,1<<11|1<<8|(data.len()+2) as u32];
    if read {b.write32(base+0x1c,0x41).unwrap();commands.extend([6<<11,1<<11|1<<8|1,3<<11|1]);}
    commands.push(2<<11);for (i,value) in commands.into_iter().enumerate(){b.write32(base+0x58+4*i as u32,value).unwrap();}b.write32(base+4,1<<5).unwrap();b.tick(100_000);
    let ack=b.read32(base+0x20).unwrap()&(1<<10)==0;let value=if read {b.read32(base+0x1c).unwrap() as u8}else{0};(ack,value)
}
fn write(b:&mut esp32s3::bus::SocBus,r:u8,data:&[u8]) {assert!(i2c(b,r,data,false).0);}
fn read(b:&mut esp32s3::bus::SocBus,r:u8)->u8 {let (ack,value)=i2c(b,r,&[],true);assert!(ack);value}
fn spi(b:&mut esp32s3::bus::SocBus,tx:&[u8])->Vec<u8> {
    for (i,c) in tx.chunks(4).enumerate(){let mut bytes=[0;4];bytes[..c.len()].copy_from_slice(c);b.write32(0x6002_4098+4*i as u32,u32::from_le_bytes(bytes)).unwrap();}
    // No hardware CS is routed. The radio transaction belongs to PCA9535 P0.
    for (off,value) in [(0x10,1<<27|1<<28),(0x1c,(tx.len()*8-1) as u32),(0x20,0x3f),(0,1<<24)] {b.write32(0x6002_4000+off,value).unwrap();}
    (0..tx.len().div_ceil(4)).flat_map(|i|b.read32(0x6002_4098+4*i as u32).unwrap().to_le_bytes()).take(tx.len()).collect()
}
fn ready(b:&mut esp32s3::bus::SocBus) {
    i2c_route(b,39,40);write(b,2,&[0xff,0xff]);write(b,6,&[0xfc,0xff]);b.tick(1_000_000);
    assert_eq!(read(b,0)&12,0);mux(b,41,101);mux(b,48,103);mux(b,47,256);b.write32(0x6000_4154+4*102,0x80|47).unwrap();mux(b,42,256);
}

#[test]
fn only_exact_lora_profiles_allocate_a_radio_and_keep_the_gx_geometry() {
    for id in ["seeed-sensecap-indicator","seeed-sensecap-indicator-gx","seeed-sensecap-indicator-dx"] {
        let b=esp32s3::board::make_board(id).unwrap();assert!(!b.uses_spi_pins());assert!(b.lora_snapshot().is_none());assert!(!b.input_levels().iter().any(|(p,_)|*p==42));
    }
    for id in ["seeed-sensecap-indicator-d1l","seeed-sensecap-indicator-d1pro","seeed-sensecap-indicator-meshtastic"] {
        let b=esp32s3::board::make_board(id).unwrap();assert_eq!(b.name(),id);assert!(b.uses_spi_pins());assert!(b.lora_snapshot().unwrap().contains("sx1262-lora-digital"));let frame=b.display().unwrap();assert_eq!((frame.0,frame.1),(480,480));
    }
    assert!(esp32s3::board::make_board("seeed-sensecap-indicator-unknown-lora").is_none());
}

#[test]
fn hardware_i2c_controls_radio_spi_and_rejects_wrong_sda_clock_and_miso() {
    let mut m=machine("seeed-sensecap-indicator-meshtastic");let b=&mut m.bus;ready(b);
    i2c_route(b,40,39);assert!(!i2c(b,2,&[0xfe],false).0);i2c_route(b,39,40);
    let identity=[0x1d,3,0x20,0,0,0,0,0,0,0];assert_eq!(spi(b,&identity),[255;10]);
    write(b,2,&[0xfe]);spi(b,&[0x1d,3]);assert_eq!(&spi(b,&[0x20,0,0,0,0,0,0,0])[2..],b"SX1261");write(b,2,&[0xff]);
    for (pin,signal) in [(41,101),(48,103)] {mux(b,pin,256);write(b,2,&[0xfe]);assert_eq!(spi(b,&identity),[255;10]);write(b,2,&[0xff]);mux(b,pin,signal);}
    b.write32(0x6000_4154+4*102,0x80|46).unwrap();write(b,2,&[0xfe]);assert_eq!(spi(b,&identity),[255;10]);write(b,2,&[0xff]);
    b.write32(0x6000_4154+4*102,0x80|47).unwrap();write(b,2,&[0xfe]);assert_eq!(&spi(b,&identity)[4..],b"SX1261");write(b,2,&[0xff]);
}

#[test]
fn busy_transitions_assert_real_gpio42_until_the_input_port_is_read() {
    let mut m=machine("seeed-sensecap-indicator-d1l");let b=&mut m.bus;ready(b);let irq_mask=1<<(42-32);
    assert_ne!(b.read32(0x6000_4040).unwrap()&irq_mask,0);write(b,2,&[0xfd]);assert_eq!(b.read32(0x6000_4040).unwrap()&irq_mask,0);
    read(b,1);assert_eq!(b.read32(0x6000_4040).unwrap()&irq_mask,0);assert_ne!(read(b,0)&4,0);assert_ne!(b.read32(0x6000_4040).unwrap()&irq_mask,0);
    write(b,2,&[0xff]);b.tick(1_000_000);assert_eq!(b.read32(0x6000_4040).unwrap()&irq_mask,0);assert_eq!(read(b,0)&4,0);assert_ne!(b.read32(0x6000_4040).unwrap()&irq_mask,0);
}
