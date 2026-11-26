//! Exact U138 wiring and original LX6 s32i/l32i guest MAC/PHY/DMA tests.
//! ESP-IDF5.4.2 emac_{mac,dma,ext}_struct.h / gpio_sig_map.h;
//! board pins independently checked against SCH_UNIT-PoESP32.pdf b33df28d.
//! These are digital RMII/VirtualNet tests, not ESP-AT firmware or PoE physics.
use emu_core::{Bus,Core};
use esp_soc::SocBus as _;
const GPIO:u32=0x3ff44000;const MUX:u32=0x3ff49000;const MAC:u32=0x3ff69000;const DMA:u32=0x3ff6a000;const EXT:u32=0x3ff6b000;
const TD:u32=0x3ffb1000;const RD:u32=0x3ffb1020;const TX:u32=0x3ffb2000;const RX:u32=0x3ffb3000;const OWN:u32=1<<31;
const OFFSETS:[u32;40]=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
struct Guest {m:esp32::Machine}
impl Guest {
    fn new()->Self {let mut m=esp32::machine([2,0,0,0,0,3],4<<20);m.bus.board=esp32::board::make_board("unit-poesp32").unwrap();m.bus.attach_board_devices();m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();let mut g=Self{m};g.put(0x3ff48070,1<<27);g.put(0x3ff0003c,2);g}
    fn put(&mut self,a:u32,v:u32) {let c=&mut self.m.cores[0];c.pc=0x40080000;c.set_ar(4,a);c.set_ar(3,v);assert!(c.step(&mut self.m.bus).trap().is_none(),"LX6 store {a:#x}");}
    fn get(&mut self,a:u32)->u32 {let c=&mut self.m.cores[0];c.pc=0x40080003;c.set_ar(4,a);assert!(c.step(&mut self.m.bus).trap().is_none(),"LX6 load {a:#x}");c.get_ar(5)}
    fn mux(&mut self,pin:u8,function:u32) {self.put(MUX+OFFSETS[pin as usize],(function<<12)|(1<<9));}
    fn output(&mut self,pin:u8,high:bool) {self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),256);self.put(GPIO+0x24,1<<pin);self.put(GPIO+if high{8}else{12},1<<pin);}
    fn enabled(&mut self) {self.put(0x3ff000cc,1<<14);self.put(0x3ff000d0,0);self.output(5,true);self.mux(23,2);self.put(GPIO+0x530+4*23,200);self.mux(18,2);self.put(GPIO+0x530+4*18,201);self.put(GPIO+0x130+4*201,0x80|18);for p in[21,19,22,25,26,27,0]{self.mux(p,5);}self.put(EXT+8,0x21);self.put(EXT+12,4<<13);self.m.bus.tick(1);}
    fn mdio(&mut self,address:u8,register:u8)->u32 {self.put(MAC+0x10,1|(u32::from(address)<<11)|(u32::from(register)<<6));assert_eq!(self.get(MAC+0x10)&1,1);self.m.bus.tick(7680);assert_eq!(self.get(MAC+0x10)&1,0);self.get(MAC+0x14)}
    fn words(&mut self,a:u32,words:&[u32]) {for(i,&v)in words.iter().enumerate(){self.put(a+4*i as u32,v);}}
    fn bytes(&mut self,a:u32,b:&[u8]) {for(i,c)in b.chunks(4).enumerate(){let mut word=[0;4];word[..c.len()].copy_from_slice(c);self.put(a+4*i as u32,u32::from_le_bytes(word));}}
    fn memory(&mut self,a:u32,n:usize)->Vec<u8> {let mut b=Vec::new();for i in 0..n.div_ceil(4){b.extend(self.get(a+4*i as u32).to_le_bytes());}b.truncate(n);b}
    fn arm(&mut self,frame:&[u8]) {self.bytes(TX,frame);self.words(TD,&[OWN|(1<<28)|(1<<29)|(1<<30)|(1<<20),frame.len()as u32,TX,TD,0,0,0,0]);self.words(RD,&[OWN,(1<<14)|1536,RX,RD,0,0,0,0]);self.put(MAC+0x40,0x300);self.put(MAC+0x44,2);self.put(DMA,1<<7);self.put(DMA+0x10,TD);self.put(DMA+0x0c,RD);self.put(DMA+0x1c,(1<<16)|1|(1<<6));self.put(MAC,12);self.put(DMA+0x18,(1<<13)|2);}
    fn tick(&mut self) {self.m.bus.tick(240_000);self.m.bus.tick(240_000);}
}
fn arp()->Vec<u8> {let mut f=vec![255;6];f.extend([2,0,0,0,0,3,8,6,0,1,8,0,6,4,0,1]);f.extend([2,0,0,0,0,3,10,0,2,15]);f.extend([0;6]);f.extend([10,0,2,2]);f}
fn reply()->Vec<u8> {let mut f=vec![2,0,0,0,0,3];f.extend([2,0x53,0x49,0x4d,0,2,8,6,0,1,8,0,6,4,0,2]);f.extend([2,0x53,0x49,0x4d,0,2,10,0,2,2]);f.extend([2,0,0,0,0,3,10,0,2,15]);f}
#[test]
fn u138_exact_ip101g_mdio_identity_and_guest_arp_reply_dma_ram() {
    let mut g=Guest::new();let c=g.m.bus.board.emac_config().unwrap();assert_eq!(c.phy,esp_soc::board::EthernetPhy::Ip101g);assert_eq!((c.phy_addr,c.mdc,c.mdio,c.rmii,c.ref_clk,c.reset_pin),(1,23,18,[21,19,22,25,26,27],Some(0),Some(5)));
    g.enabled();assert_eq!(g.mdio(1,2),0x0243);assert_eq!(g.mdio(1,3),0x0c54);assert_ne!(g.mdio(1,1)&4,0);assert_eq!(g.mdio(2,2),0xffff);
    g.arm(&arp());g.tick();assert_eq!(g.get(TD)&OWN,0);assert_eq!(g.get(RD),46<<16|(1<<9)|(1<<8));assert_eq!(g.memory(RX,42),reply());assert_ne!(g.get(DMA+0x14)&((1<<6)|1),0);
    g.put(DMA+0x14,u32::MAX);assert_eq!(g.get(DMA+0x14),0);
}
#[test]
fn u138_wrong_mdio_rmii_clock_and_reset_do_not_fabricate_phy_or_dma_success() {
    let mut g=Guest::new();g.enabled();g.put(GPIO+0x130+4*201,0x80|19);assert_eq!(g.mdio(1,2),0xffff);g.put(GPIO+0x130+4*201,0x80|18);
    g.mux(0,2);assert_eq!(g.mdio(1,1)&4,0);g.arm(&arp());g.tick();assert_ne!(g.get(TD)&OWN,0);assert_ne!(g.get(RD)&OWN,0);g.mux(0,5);g.tick();assert_eq!(g.get(TD)&OWN,0);assert_eq!(g.memory(RX,42),reply());
    g.output(5,false);assert_eq!(g.mdio(1,2),0xffff);g.arm(&arp());g.tick();assert_ne!(g.get(TD)&OWN,0);assert_ne!(g.get(RD)&OWN,0);g.output(5,true);assert_eq!(g.mdio(1,2),0x0243);
    g.put(0x3ff000cc,0);g.put(MAC+0x10,1|(1<<11)|(2<<6));g.m.bus.tick(24_000);assert_ne!(g.get(MAC+0x10)&1,0);g.put(0x3ff000cc,1<<14);g.m.bus.tick(24_000);assert_eq!(g.get(MAC+0x10)&1,0);assert_eq!(g.get(MAC+0x14),0x0243);
    g.put(0x3ff000d0,1<<7);assert_eq!(g.get(DMA+0x10),0);assert_eq!(g.get(DMA+0x0c),0);assert_eq!(g.get(MAC+0x10)&1,0);
}
#[test]
fn u138_guest_descriptors_use_same_explicit_host_ethernet_relay() {
    let mut g=Guest::new();g.enabled();g.m.bus.set_ethernet_relay(true).unwrap();g.arm(&arp());g.tick();assert_eq!(g.m.bus.take_ethernet_frames(),vec![arp()]);assert_ne!(g.get(RD)&OWN,0);g.m.bus.receive_ethernet_frame(&reply()).unwrap();g.tick();assert_eq!(g.memory(RX,42),reply());assert_eq!(g.get(RD)&OWN,0);
}
