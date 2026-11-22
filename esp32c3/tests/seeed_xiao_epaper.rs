//! The UC8179 board is reached through actual C3 MMIO and its GPIO signal IDs.
//! ESP-IDF v4.4.7 components/soc/esp32c3/include/soc/{gpio,spi}_reg.h and
//! gpio_sig_map.h: FSPI CLK/D/CS0 = 63/65/68, software GPIO = 128.
use emu_core::Bus;
fn mux(bus:&mut esp32c3::bus::SocBus,pin:u8,signal:u32) {bus.write32(0x6000_9004+4*u32::from(pin),1<<12|1<<9).unwrap();bus.write32(0x6000_4554+4*u32::from(pin),signal).unwrap();}
fn gpio(bus:&mut esp32c3::bus::SocBus,pin:u8,high:bool) {bus.write32(0x6000_4024,1<<pin).unwrap();bus.write32(if high {0x6000_4008}else{0x6000_400c},1<<pin).unwrap();}
fn spi(bus:&mut esp32c3::bus::SocBus,dc:bool,byte:u8) {gpio(bus,5,dc);for (off,v) in [(0x10,1<<27),(0x1c,7),(0x98,u32::from(byte)),(0x20,0x3e),(0,1<<24)] {bus.write32(0x6002_4000+off,v).unwrap();}}
fn cmd(bus:&mut esp32c3::bus::SocBus,c:u8,d:&[u8]) {spi(bus,false,c);for &b in d {spi(bus,true,b);}}
#[test]
fn c3_signal_routes_clock_the_800x480_panel_and_wrong_mosi_cannot_write() {
    let mut m=esp32c3::machine([0;6],4<<20);m.bus.board=esp32c3::board::make_board("seeed-xiao-epaper-75").unwrap();m.bus.attach_board_devices();let b=&mut m.bus;
    for (pin,signal) in [(8,63),(10,65),(3,68),(5,128),(2,128)] {mux(b,pin,signal);}gpio(b,2,true);
    cmd(b,0x61,&[3,32,1,224]);cmd(b,0x04,&[]);assert_eq!(b.read32(0x6000_403c).unwrap()&(1<<4),0);b.tick(160_000);assert_ne!(b.read32(0x6000_403c).unwrap()&(1<<4),0);cmd(b,0x13,&[0x7f]);cmd(b,0x12,&[]);
    assert_eq!(b.board.display().unwrap().2[0],0xffff);b.tick(3_200_000);let f=b.board.display().unwrap();assert_eq!((f.0,f.1),(800,480));assert_eq!(&f.2[..2],&[0,0xffff]);
    mux(b,10,103);cmd(b,0x13,&[0]);cmd(b,0x12,&[]);b.tick(3_200_000);assert_eq!(b.board.display_frames(),1,"S3 MOSI signal ID must not drive a C3 pin");
    gpio(b,2,false);assert_eq!(b.board.display().unwrap().2[0],0,"physical e-ink image survives controller reset");
    let boot=b.board.named_pin("boot").unwrap();assert_eq!(boot,9);assert_ne!(b.read32(0x6000_403c).unwrap()&(1<<boot),0);
    esp_soc::SocBus::gpio_set_input(b,boot,false);assert_eq!(b.read32(0x6000_403c).unwrap()&(1<<boot),0);
    esp_soc::SocBus::gpio_set_input(b,boot,true);assert_ne!(b.read32(0x6000_403c).unwrap()&(1<<boot),0);
}
