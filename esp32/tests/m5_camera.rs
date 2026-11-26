//! Actual LX6 instructions drive SCCB, GPIO matrix, LEDC and I2S/DMA MMIO.
//! The fixture checks guest DRAM and native EOF, not a host camera-frame shortcut.
use emu_core::{Bus,Core};
use esp_soc::SocBus as _;
use esp_periph::i2c::INT_NACK;
use esp32::board::m5_camera::{Profile,SensorKind,PROFILES};
const OFFSETS:[u32;40]=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
const GPIO:u32=0x3ff44000;const MUX:u32=0x3ff49000;const I2C:u32=0x3ff53000;const I:u32=0x3ff4f000;const D:u32=0x3ffb0000;const B:u32=0x3ffb1000;
const EOF:u32=1<<9;
struct Guest {m:esp32::Machine,p:&'static Profile}
impl Guest {
    fn new(p:&'static Profile)->Self {let mut m=esp32::machine([0;6],p.flash_mib<<20);m.bus.board=esp32::board::make_board(p.name).unwrap();m.bus.attach_board_devices();m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();let mut g=Self{m,p};g.put(0x3ff48070,1<<27);g.put(0x3ff0003c,2);g}
    // s32i a3,a4,0 / l32i a5,a4,0: same LX6 path used by firmware.
    fn put(&mut self,a:u32,v:u32) {let c=&mut self.m.cores[0];c.pc=0x40080000;c.set_ar(4,a);c.set_ar(3,v);assert!(c.step(&mut self.m.bus).trap().is_none(),"{} store {a:#x}",self.p.name);}
    fn get(&mut self,a:u32)->u32 {let c=&mut self.m.cores[0];c.pc=0x40080003;c.set_ar(4,a);assert!(c.step(&mut self.m.bus).trap().is_none());c.get_ar(5)}
    fn mux(&mut self,p:u8) {self.put(MUX+OFFSETS[p as usize],(2<<12)|(1<<9)|(1<<8));}
    fn output(&mut self,p:u8,high:bool) {self.mux(p);self.put(GPIO+0x530+4*u32::from(p),256);self.level(p,high);self.put(GPIO+if p<32{0x24}else{0x30},1<<(p%32));}
    fn level(&mut self,p:u8,high:bool) {self.put(GPIO+match(p<32,high){(true,true)=>8,(true,false)=>12,(false,true)=>20,(false,false)=>24},1<<(p%32));}
    fn route_sccb(&mut self,sda:u8,scl:u8) {for(p,s)in[(sda,30),(scl,29)] {self.mux(p);self.put(GPIO+0x130+4*s,0x80|u32::from(p));self.put(GPIO+0x530+4*u32::from(p),s);}}
    fn address(&self)->u8 {if self.p.sensor==SensorKind::Ov2640 {0x30}else{0x3c}}
    fn transaction(&mut self,addr:u8,data:&[u8],read:usize)->(u32,Vec<u8>) {self.put(I2C+0x24,u32::MAX);self.put(I2C+0x18,3<<12);self.put(I2C+0x1c,(u32::from(addr)<<1)|u32::from(read!=0));for &b in data{self.put(I2C+0x1c,u32::from(b));}let mut cmds=vec![0,(1<<11)|(1<<8)|(1+data.len()as u32)];if read!=0 {cmds.push((2<<11)|(1<<10)|read as u32);}cmds.push(3<<11);for(i,c)in cmds.into_iter().enumerate(){self.put(I2C+0x58+4*i as u32,c);}self.put(I2C+4,1<<5);let status=self.get(I2C+0x20);let bytes=(0..read).map(|_|self.get(I2C+0x1c)as u8).collect();(status,bytes)}
    fn write(&mut self,r:u16,v:u8) {let data=if self.p.sensor==SensorKind::Ov2640 {vec![r as u8,v]}else{vec![(r>>8)as u8,r as u8,v]};assert_eq!(self.transaction(self.address(),&data,0).0&INT_NACK,0,"{} SCCB {r:#x}",self.p.name);}
    fn read(&mut self,r:u16)->u8 {let data=if self.p.sensor==SensorKind::Ov2640 {vec![r as u8]}else{r.to_be_bytes().to_vec()};assert_eq!(self.transaction(self.address(),&data,0).0&INT_NACK,0);let(status,b)=self.transaction(self.address(),&[],1);assert_eq!(status&INT_NACK,0);b[0]}
    fn configure(&mut self,jpeg:bool) {self.route_sccb(self.p.sda,self.p.scl);self.output(self.p.reset,true);if self.p.sensor==SensorKind::Ov2640 {self.write(0xff,0);for(r,v)in[(0x5a,1),(0x5b,1),(0x5c,0),(0xda,if jpeg{0x12}else{8}),(0xe0,0)]{self.write(r,v);}}else{for(r,v)in[(0x3808,0),(0x3809,4),(0x380a,0),(0x380b,4),(0x4300,0x61),(0x3821,if jpeg{0x20}else{0}),(0x3008,2)]{self.write(r,v);}}}
    fn clock(&mut self) {self.put(0x3ff000c0,(1<<11)|(1<<4));self.put(0x3ff59140,3|(256<<5)|(1<<25));self.put(0x3ff59008,4<<4);self.put(0x3ff5900c,(1<<31)|(1<<30)|(1<<20)|(1<<10));self.put(0x3ff59000,4);self.mux(self.p.xclk);self.put(GPIO+0x530+4*u32::from(self.p.xclk),71);self.m.bus.periph.tick(64);self.get(GPIO+0x3c);}
    fn routes(&mut self) {for(s,p)in(140..148).zip(self.p.dvp).chain([(28,self.p.dvp[8]),(190,self.p.dvp[9]),(191,self.p.dvp[10])]){self.mux(p);self.put(GPIO+0x130+4*s,0x80|u32::from(p));}self.put(GPIO+0x130+4*192,0xb8);}
    fn receiver(&mut self) {self.put(I+0xa8,0x21);self.put(I+0x20,(1<<12)|(1<<16)|(1<<20));self.put(I+8,(1<<7)|(1<<5));}
    fn arm(&mut self) {self.put(D,(1<<31)|128);self.put(D+4,B);self.put(D+8,0);for off in(0..128).step_by(4){self.put(B+off,0xa5a5a5a5);}self.put(I+0x18,u32::MAX);self.put(I+0x24,16);self.put(I+0x60,1<<12);self.put(I+0x14,EOF|(1<<8)|(1<<13));self.put(I+0x34,(D&0xfffff)|(1<<29));}
    fn tick(&mut self,n:u64) {self.m.bus.tick(u32::try_from(n).unwrap());}
    fn capture(&mut self) {use esp32::board::aithinker::{FRAME_CYCLES,VSYNC_CYCLES};let next=self.m.bus.board.next_deadline().expect("streaming VSYNC");let now=self.m.bus.cycles();self.tick(next-now);if self.m.bus.periph.gpio.gpio.level(self.p.dvp[10]){self.tick(VSYNC_CYCLES);}self.tick(FRAME_CYCLES-VSYNC_CYCLES);}
    fn dma_bytes(&mut self)->Vec<u8> {(0..32).flat_map(|i|self.get(B+4*i).to_le_bytes()).collect()}
    fn ready(p:&'static Profile,jpeg:bool)->Self {let mut g=Self::new(p);g.configure(jpeg);g.clock();g.routes();g.receiver();g.m.bus.board.set_camera_picture(esp_soc::picture::Picture{w:1,h:1,rgb:vec![255,0,0]}).unwrap();g}
}
#[test]
fn every_exact_camera_sccb_to_native_i2s_dma_reaches_guest_dram() {
    for p in PROFILES {let mut g=Guest::ready(p,false);g.arm();g.capture();assert_eq!(g.get(I+0xc)&EOF,EOF,"{} native EOF",p.name);assert_eq!(&g.dma_bytes()[..64],&[0,0,248,0].repeat(16),"{} RGB565 DMA",p.name);assert_eq!(g.get(D)&(1<<31),0,"descriptor ownership returned to guest");}
}
#[test]
fn all_exact_sccb_routes_ids_addresses_and_hardware_reset() {
    for p in PROFILES {let mut g=Guest::new(p);g.route_sccb(p.sda,p.scl);if p.sensor==SensorKind::Ov2640 {g.write(0xff,1);assert_eq!(g.read(0x0a),0x26);assert_eq!(g.read(0x0b),0x42);}else{assert_eq!(g.read(0x300a),0x36);assert_eq!(g.read(0x300b),0x60);}
        assert_ne!(g.transaction(g.address()^1,&[],0).0&INT_NACK,0,"wrong address must NACK");g.route_sccb(p.scl,p.sda);assert_ne!(g.transaction(g.address(),&[],0).0&INT_NACK,0,"{} swapped SCCB",p.name);g.route_sccb(p.sda,p.scl);
        g.configure(false);g.clock();g.output(p.reset,false);assert_ne!(g.transaction(g.address(),&[],0).0&INT_NACK,0,"{} reset holds SCCB inactive",p.name);assert!(g.m.bus.camera_frame().is_none());g.level(p.reset,true);assert_eq!(g.transaction(g.address(),&[],0).0&INT_NACK,0);assert!(g.m.bus.camera_frame().is_none(),"reset discards geometry");g.configure(false);assert!(g.m.bus.camera_frame().is_some());
    }
}
#[test]
fn every_camera_rejects_wrong_dvp_lost_xclk_and_sensor_standby() {
    for p in PROFILES {for bad in 0..13 {let mut g=Guest::ready(p,false);match bad {0..=10=>{let signal=if bad<8 {140+bad as u32}else{[28,190,191][bad-8]};g.put(GPIO+0x130+4*signal,0x80|u32::from(p.dvp[bad]^1));},11=>g.put(GPIO+0x530+4*u32::from(p.xclk),256),_=>if p.sensor==SensorKind::Ov2640 {g.write(0xff,1);g.write(9,0x10);}else{g.write(0x3008,0x42);}}g.arm();g.tick(2*esp32::board::aithinker::FRAME_CYCLES);assert_eq!(g.get(I+0xc)&EOF,0,"{} failure {bad}",p.name);assert_eq!(g.dma_bytes(),vec![0xa5;128],"{} failure {bad} cannot fabricate bytes",p.name);}}
}
#[test]
fn every_camera_host_jpeg_goes_verbatim_through_fifo_and_dma() {
    let jpeg=vec![0xff,0xd8,0xff,0xc0,0,11,8,0,4,0,4,1,1,0x11,0,0xff,0xda,0,8,1,1,0,0,63,0,0x3a,0xff,0xd9];
    for p in PROFILES {let mut g=Guest::ready(p,true);g.m.bus.board.set_camera_jpeg(esp_soc::picture::Jpeg::parse(jpeg.clone()).unwrap()).unwrap();g.arm();g.capture();assert_eq!(g.get(I+0xc)&EOF,EOF,"{} JPEG EOF",p.name);let dma=g.dma_bytes();let recovered:Vec<_>=dma.as_chunks::<4>().0.iter().flat_map(|w|[w[2],w[0]]).take(jpeg.len()).collect();assert_eq!(recovered,jpeg,"{} source JPEG bytes",p.name);}
}
#[test]
fn camera_reset_guest_reboot_and_fresh_platform_require_new_capture() {
    for p in PROFILES {
        let mut g=Guest::ready(p,false);g.arm();g.capture();assert_ne!(g.get(I+0xc)&EOF,0);
        g.m.reboot();assert_eq!(g.get(I+0xc)&EOF,0,"MCU reset clears native I2S IRQ");assert!(g.m.bus.camera_frame().is_none(),"MCU reset removed physical XCLK");
        g.configure(false);g.clock();g.routes();g.receiver();g.m.bus.board.set_camera_picture(esp_soc::picture::Picture{w:1,h:1,rgb:vec![0,255,0]}).unwrap();g.arm();g.capture();assert_eq!(&g.dma_bytes()[..64],&[224,0,7,0].repeat(16),"{} reboot must capture new host source",p.name);
        let mut fresh=Guest::new(p);fresh.configure(false);fresh.clock();fresh.routes();fresh.receiver();fresh.arm();fresh.capture();assert_eq!(&fresh.dma_bytes()[..64],&[0;64],"fresh platform must not inherit host picture or sensor bytes");
    }
}
#[test]
fn every_sensor_supply_loss_nacks_sccb_stops_dma_and_requires_reinitialization() {
    for p in PROFILES {
        let mut g=Guest::ready(p,false);g.arm();let cycle=g.m.bus.cycles();assert!(g.m.bus.board.camera_supply_at(cycle,false));assert_ne!(g.transaction(g.address(),&[],0).0&INT_NACK,0);g.tick(2*esp32::board::aithinker::FRAME_CYCLES);assert_eq!(g.get(I+0xc)&EOF,0);assert_eq!(g.dma_bytes(),vec![0xa5;128],"{} unpowered sensor writes no bytes",p.name);
        let cycle=g.m.bus.cycles();assert!(g.m.bus.board.camera_supply_at(cycle,true));assert!(g.m.bus.camera_frame().is_none(),"power return cannot revive stale sensor geometry");g.configure(false);g.arm();g.capture();assert_ne!(g.get(I+0xc)&EOF,0);assert_eq!(&g.dma_bytes()[..64],&[0,0,248,0].repeat(16));
    }
}
#[test]
fn version_pin_and_memory_boundaries_are_explicit() {
    let p=|name|esp32::board::m5_camera::profile(name).unwrap();
    assert_eq!((p("m5stack-u017").sda,p("m5stack-u017").dvp[10]),(25,22));assert_eq!((p("m5stack-u017-b").sda,p("m5stack-u017-b").dvp[10]),(22,25));
    assert_eq!(p("m5stack-u007").dvp[0],17);assert_eq!(p("m5stack-u017-pcba").sda,22);
    assert_eq!(p("m5stack-u121-b").sensor,SensorKind::Ov2640);assert_eq!(p("m5stack-u121-b-v11").sensor,SensorKind::Ov3660);
    for name in["m5stack-u007","m5stack-u109","m5stack-u109-x","m5stack-u109-b"]{assert_eq!(p(name).psram_mib,None);assert_eq!(esp32::board::documented_memory_mib(name),None,"unknown PSRAM is not a physical zero-capacity claim");}
    assert_eq!(esp32::board::documented_memory_mib("m5stack-u038"),Some((4,4)));assert_eq!(esp32::board::documented_memory_mib("m5stack-u121-b-v11"),Some((16,8)));
}

const SPI:u32=0x3ff64000;
impl Guest {
    fn ethernet_route(&mut self) {for(p,s)in[(23,8),(13,10)]{self.mux(p);self.put(GPIO+0x530+4*u32::from(p),s);}self.mux(38);self.put(GPIO+0x130+4*9,0x80|38);self.output(4,true);self.mux(2);self.put(SPI+0x34,7);self.put(SPI+8,0);}
    fn spi_full(&mut self,data:&[u8])->Vec<u8> {assert!(!data.is_empty()&&data.len()<=64);for(i,c)in data.chunks(4).enumerate(){let mut w=[0;4];w[..c.len()].copy_from_slice(c);self.put(SPI+0x80+4*i as u32,u32::from_le_bytes(w));}self.put(SPI+0x1c,(1<<27)|(1<<28)|1);self.put(SPI+0x28,data.len()as u32*8-1);self.put(SPI+0x2c,data.len()as u32*8-1);self.put(SPI,1<<18);let mut r=Vec::new();for i in 0..data.len().div_ceil(4){r.extend(self.get(SPI+0x80+4*i as u32).to_le_bytes());}r.truncate(data.len());r}
    fn eth_write(&mut self,block:u8,reg:u16,data:&[u8]) {for(i,chunk)in data.chunks(61).enumerate(){let mut tx=reg.wrapping_add((i*61)as u16).to_be_bytes().to_vec();tx.push((block<<3)|4);tx.extend(chunk);self.level(4,false);self.spi_full(&tx);self.level(4,true);}}
    fn eth_read(&mut self,block:u8,reg:u16,len:usize)->Vec<u8> {let mut out=Vec::new();let mut at=0;while at<len {let n=(len-at).min(61);let mut tx=reg.wrapping_add(at as u16).to_be_bytes().to_vec();tx.push(block<<3);tx.resize(n+3,0);self.level(4,false);out.extend_from_slice(&self.spi_full(&tx)[3..]);self.level(4,true);at+=n;}out}
    fn eth_open(&mut self) {self.eth_write(1,0,&[4]);self.eth_write(1,1,&[1]);assert_eq!(self.eth_read(1,3,1),[0x42]);}
    fn eth_send(&mut self,frame:&[u8]) {let pointer=self.eth_read(1,0x24,2);let p=u16::from_be_bytes([pointer[0],pointer[1]]);self.eth_write(2,p,frame);self.eth_write(1,0x24,&p.wrapping_add(frame.len()as u16).to_be_bytes());self.eth_write(1,1,&[0x20]);}
    fn eth_read_half_hardware_cs(&mut self,block:u8,reg:u16,len:usize)->Vec<u8> {self.mux(4);self.put(GPIO+0x530+4*4,11);self.put(SPI+0x34,6);self.put(SPI+0x1c,(1<<31)|(1<<30)|(1<<28));self.put(SPI+0x24,(15<<28)|u32::from(u16::from_le_bytes(reg.to_be_bytes())));self.put(SPI+0x20,7<<26);self.put(SPI+4,u32::from(block<<3)<<24);self.put(SPI+0x30,0);self.put(SPI+0x2c,len as u32*8-1);self.put(SPI,1<<18);let mut out=Vec::new();for i in 0..len.div_ceil(4){out.extend(self.get(SPI+0x80+4*i as u32).to_le_bytes());}out.truncate(len);out}
}
fn poe_profiles()->impl Iterator<Item=&'static Profile> {PROFILES.iter().filter(|p|p.name.starts_with("m5stack-u121"))}
fn ethernet_frame(n:usize,tag:u8)->Vec<u8> {let mut b=vec![tag;n];b[..6].fill(255);b[12..14].copy_from_slice(&[0x88,0xb5]);b}
#[test]
fn three_poe_cameras_real_guest_spi_modes_routes_cs_and_prefix_phases() {
    for p in poe_profiles(){let mut g=Guest::new(p);g.ethernet_route();assert_eq!(g.eth_read(0,0x39,1),[4],"{} full duplex W5500 VERSIONR",p.name);assert_eq!(g.eth_read_half_hardware_cs(0,0x39,1),[4],"{} 16-bit command/8-bit address and hardware CS",p.name);g.ethernet_route();
        for(pin,wrong)in[(23u8,22u8),(13,18)] {g.mux(wrong);g.put(GPIO+0x530+4*u32::from(wrong),if pin==23{8}else{10});g.put(GPIO+0x530+4*u32::from(pin),256);assert_eq!(g.eth_read(0,0x39,1),[255],"wrong SPI wire {pin}");g.ethernet_route();}
        g.put(GPIO+0x130+4*9,0x80|37);assert_eq!(g.eth_read(0,0x39,1),[255],"MISO is G38");g.ethernet_route();assert_eq!(g.spi_full(&[0,0x39,0,0]),[255;4],"CS high means disconnected");
        g.level(4,false);g.spi_full(&[0,0x39]);g.level(4,true);assert_eq!(g.eth_read(0,0x39,1),[4],"CS discards incomplete header");
        g.level(4,false);g.put(SPI+8,1<<26);assert_eq!(g.spi_full(&[0,0x39,0,0]),[255;4],"LSB-first is unsupported, no false identity");g.level(4,true);g.put(SPI+8,0);
    }
}
#[test]
fn three_poe_cameras_real_guest_macraw_tx_rx_irq_ring_and_reset() {
    for p in poe_profiles(){let mut g=Guest::new(p);g.ethernet_route();g.m.bus.board.ethernet_link().unwrap().set_relay(true);g.eth_open();g.eth_write(0,0x18,&[1]);
        let sent=ethernet_frame(1500,0x36);g.eth_send(&sent);assert_eq!(g.m.bus.board.ethernet_link().unwrap().take_frames(),[sent]);assert_eq!(g.eth_read(1,2,1),[0x10]);assert_eq!(g.get(GPIO+0x3c)&4,0,"INTn G2 asserted");g.eth_write(1,2,&[0x10]);assert_ne!(g.get(GPIO+0x3c)&4,0);
        let second=ethernet_frame(1000,0x57);g.eth_send(&second);assert_eq!(g.m.bus.board.ethernet_link().unwrap().take_frames().as_slice(),std::slice::from_ref(&second),"TX crosses ring end");g.eth_write(1,2,&[0x10]);
        let a=ethernet_frame(1500,0x26);g.m.bus.board.ethernet_link().unwrap().receive(&a).unwrap();g.tick(esp32::periph::CPU_HZ/1000);assert_eq!(g.eth_read(1,0x26,2),1502u16.to_be_bytes());assert_eq!(g.eth_read(3,0,1502),[1502u16.to_be_bytes().to_vec(),a].concat());g.eth_write(1,0x28,&1502u16.to_be_bytes());g.eth_write(1,1,&[0x40]);g.eth_write(1,2,&[4]);assert_eq!(g.eth_read(1,0x26,2),[0,0]);
        g.m.bus.board.ethernet_link().unwrap().receive(&second).unwrap();g.tick(esp32::periph::CPU_HZ/1000);assert_eq!(g.eth_read(3,1502,1002),[1002u16.to_be_bytes().to_vec(),second].concat(),"RX crosses ring end");
        g.eth_write(0,0,&[0x80]);assert_eq!(g.eth_read(1,3,1),[0]);assert_eq!(g.eth_read(1,0x26,2),[0,0]);assert_ne!(g.get(GPIO+0x3c)&4,0,"MR reset clears interrupt");g.eth_write(1,0,&[1]);g.eth_write(1,1,&[1]);assert_eq!(g.eth_read(1,3,1),[0],"unsupported TCP offload cannot fake OPEN");
        g.eth_open();g.eth_write(0,0x2e,&[0x38]);g.eth_send(&ethernet_frame(64,0x44));assert!(g.m.bus.board.ethernet_link().unwrap().take_frames().is_empty(),"PHY reset blocks transmit");g.eth_write(0,0x2e,&[0xb8]);
        g.m.reboot();assert!(g.spi_full(&[0,0x39,0,0]).iter().all(|&b|b==255),"MCU reset removed SPI routes");g.ethernet_route();assert_eq!(g.eth_read(1,3,1),[0x42],"W5500 has no MCU reset GPIO; its state survives MCU reset");
    }
}
#[test]
fn three_poe_cameras_guest_arp_reaches_independent_wired_virtual_peer() {
    let mac=[2,0x11,0x22,0x33,0x44,0x55];let mut arp=vec![255;6];arp.extend(mac);arp.extend([8,6,0,1,8,0,6,4,0,1]);arp.extend(mac);arp.extend([10,0,2,15]);arp.extend([0;6]);arp.extend([10,0,2,2]);
    for p in poe_profiles(){let mut g=Guest::new(p);g.ethernet_route();g.eth_write(0,9,&mac);g.eth_open();g.eth_send(&arp);let size=g.eth_read(1,0x26,2);assert_eq!(size,[0,44]);let reply=g.eth_read(3,0,44);assert_eq!(&reply[2..8],&mac);assert_eq!(&reply[14..16],&[8,6]);assert_eq!(&reply[22..24],&[0,2]);assert_eq!(&reply[30..34],&[10,0,2,2]);assert_eq!(&reply[40..44],&[10,0,2,15]);assert_eq!(g.m.bus.periph.wifi.link.tx_frames(),0,"wired ARP does not use Wi-Fi transmit path");assert!(g.m.bus.periph.wifi.link.ap().is_none());}
}
