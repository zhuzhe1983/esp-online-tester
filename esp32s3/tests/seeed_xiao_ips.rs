//! Physical S3 GPIO matrix/SPI/I2C boundaries for the three Seeed IPS boards.
//! Addresses/signals follow ESP-IDF 87912cd291 (factory descriptor:
//! v5.5.2-729-g87912cd291), components/soc/esp32s3/{register,include}/soc/
//! {gpio,spi,i2c}_reg.h and gpio_sig_map.h. These match the existing v5.5.1
//! pin_transport fixtures: FSPI CLK/D/CS0 101/103/110, I2C0 SCL/SDA 89/90.
use emu_core::Bus;

fn machine(id:&str)->esp32s3::Machine {let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board(id).unwrap();m.bus.attach_board_devices();m}
fn mux(bus:&mut esp32s3::bus::SocBus,pin:u8,signal:u32) {bus.write32(0x6000_9004+4*pin as u32,1<<12|1<<9).unwrap();bus.write32(0x6000_4554+4*pin as u32,signal).unwrap();}
fn gpio(bus:&mut esp32s3::bus::SocBus,pin:u8,high:bool) {let bank=u32::from(pin>=32)*12;bus.write32(0x6000_4024+bank,1<<(pin%32)).unwrap();bus.write32(0x6000_4008+bank+if high {0}else{4},1<<(pin%32)).unwrap();}
fn spi(bus:&mut esp32s3::bus::SocBus,dc:bool,byte:u8) {
    gpio(bus,4,dc);
    for (off,value) in [(0x10,1<<27),(0x1c,7),(0x98,byte as u32),(0x20,0x3e),(0,1<<24)] {bus.write32(0x6002_4000+off,value).unwrap();}
}
fn cmd(bus:&mut esp32s3::bus::SocBus,c:u8,data:&[u8]) {spi(bus,false,c);for &b in data {spi(bus,true,b);}}

#[test]
fn gpio_matrix_routes_guest_spi_and_mismatches_leave_gram_unchanged() {
    let mut m=machine("seeed-xiao-ips-114");let bus=&mut m.bus;
    for (pin,signal) in [(7,101),(9,103),(3,110),(4,256),(12,256),(13,256)] {mux(bus,pin,signal);}
    gpio(bus,13,true);gpio(bus,12,true);cmd(bus,0x11,&[]);cmd(bus,0x29,&[]);cmd(bus,0x3a,&[0x55]);cmd(bus,0x21,&[]);cmd(bus,0x36,&[0]);
    cmd(bus,0x2a,&[0,52,0,52]);cmd(bus,0x2b,&[0,40,0,40]);cmd(bus,0x2c,&[0xf8,0]);
    assert_eq!(bus.board.display().unwrap().2[0],0xf800);
    mux(bus,9,256);cmd(bus,0x2c,&[0,0x1f]);assert_eq!(bus.board.display().unwrap().2[0],0xf800,"SPI must not write when MOSI leaves its signal route");
    mux(bus,9,103);gpio(bus,13,false);cmd(bus,0x11,&[]);assert!(bus.board.display().unwrap().2.iter().all(|p|*p==0));
}

fn touch_read(bus:&mut esp32s3::bus::SocBus,sda:u8,scl:u8)->(bool,Vec<u8>) {
    for (pin,signal) in [(sda,90),(scl,89)] {mux(bus,pin,signal);bus.write32(0x6000_4154+4*signal,0x80|pin as u32).unwrap();}
    let base=0x6001_3000;
    for (off,value) in [(0x24,u32::MAX),(0x18,3<<12),(0x1c,0xc6),(0x1c,1),(0x1c,0xc7),
        (0x58,6<<11),(0x5c,1<<11|1<<8|2),(0x60,6<<11),(0x64,1<<11|1<<8|1),
        (0x68,3<<11|6),(0x6c,2<<11),(0x04,1<<5)] {bus.write32(base+off,value).unwrap();}
    bus.tick(100_000);let ack=bus.read32(base+0x20).unwrap()&(1<<10)==0;let data=(0..6).map(|_|bus.read32(base+0x1c).unwrap() as u8).collect();(ack,data)
}
#[test]
fn touch_is_read_through_routed_hardware_i2c_and_reset_nacks() {
    let mut m=machine("seeed-xiao-ips-147");let bus=&mut m.bus;mux(bus,13,256);gpio(bus,13,true);
    bus.board.touch(20,301,true);
    assert_eq!(touch_read(bus,5,6),(true,vec![0,1,0,151,1,45]));
    assert!(!touch_read(bus,6,5).0,"swapping SDA/SCL must not find the controller");
    gpio(bus,13,false);assert!(!touch_read(bus,5,6).0,"reset controller must NACK");
}
