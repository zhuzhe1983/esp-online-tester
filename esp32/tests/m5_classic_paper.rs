//! Real LX6 instruction fixture. Every GPIO/SPI/I2C write and input read below
//! executes guest s32i/l32i through the SoC, not direct panel mutation.
//! Protocol constants are transcribed from pinned M5EPD/M5GFX source receipts.
use emu_core::{Bus,Core};
use esp_soc::SocBus as _;
use esp_periph::i2c::INT_NACK;
const OFFSETS:[u32;40]=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
const GPIO:u32=0x3ff44000;const MUX:u32=0x3ff49000;const I2C:u32=0x3ff53000;
const HZ:u32=esp32::periph::CPU_HZ as u32;
struct Guest {m:esp32::Machine}
impl Guest {
    fn new(name:&str)->Self {let mut m=esp32::machine([0;6],4<<20);m.bus.board=esp32::board::make_board(name).unwrap();m.bus.attach_board_devices();m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();let mut g=Self{m};
        // ESP-IDF5.5.4 clk_tree_ll.h / dport_reg.h: PLL240MHz, 1:1 reference cycles.
        g.put(0x3ff48070,1<<27);g.put(0x3ff0003c,2);g}
    fn put(&mut self,a:u32,v:u32) {let cpu=&mut self.m.cores[0];cpu.pc=0x40080000;cpu.set_ar(4,a);cpu.set_ar(3,v);assert!(cpu.step(&mut self.m.bus).trap().is_none(),"guest store {a:#x}");}
    fn get(&mut self,a:u32)->u32 {let cpu=&mut self.m.cores[0];cpu.pc=0x40080003;cpu.set_ar(4,a);assert!(cpu.step(&mut self.m.bus).trap().is_none(),"guest load {a:#x}");cpu.get_ar(5)}
    fn mux(&mut self,pin:u8,f:u32) {self.put(MUX+OFFSETS[pin as usize],(f<<12)|(1<<9));}
    fn output(&mut self,pin:u8,level:bool) {self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),256);self.put(GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));self.level(pin,level);}
    fn level(&mut self,pin:u8,high:bool) {self.put(GPIO+match(pin<32,high){(true,true)=>8,(true,false)=>12,(false,true)=>20,(false,false)=>24},1<<(pin%32));}
    fn input(&mut self,pin:u8)->bool {self.mux(pin,2);self.get(GPIO+if pin<32{0x3c}else{0x40})&(1<<(pin%32))!=0}
    fn spi_route(&mut self,clk:u8,mosi:u8,miso:u8) {for(pin,sig)in[(clk,8),(mosi,10)] {self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),sig);}self.mux(miso,2);self.put(GPIO+0x130+4*9,0x80|u32::from(miso));}
    fn spi(&mut self,data:&[u8])->Vec<u8> {const SPI:u32=0x3ff64000;assert!(!data.is_empty()&&data.len()<=64);for(i,c)in data.chunks(4).enumerate(){let mut w=[0;4];w[..c.len()].copy_from_slice(c);self.put(SPI+0x80+4*i as u32,u32::from_le_bytes(w));}self.put(SPI+0x1c,(1<<27)|(1<<28)|1);self.put(SPI+0x28,data.len()as u32*8-1);self.put(SPI+0x2c,data.len()as u32*8-1);self.put(SPI,1<<18);let mut r=Vec::new();for i in 0..data.len().div_ceil(4){r.extend(self.get(SPI+0x80+4*i as u32).to_le_bytes());}r.truncate(data.len());r}
    fn paper_frame(&mut self,words:&[u16]) {self.level(15,false);let tx:Vec<_>=words.iter().flat_map(|w|w.to_be_bytes()).collect();self.spi(&tx);self.level(15,true);}
    fn paper_cmd(&mut self,c:u16,args:&[u16]) {self.paper_frame(&[0x6000,c]);for &w in args {self.paper_frame(&[0,w]);}}
    fn paper_ready(&mut self) {self.spi_route(14,12,13);self.output(4,true);self.output(15,true);self.output(23,true);self.output(2,true);self.paper_cmd(1,&[]);self.paper_cmd(0x11,&[4,1]);}
    fn paper_read(&mut self,words:usize)->Vec<u8> {self.level(15,false);self.spi(&[0x10,0]);let r=self.spi(&vec![0;2*(words+1)]);self.level(15,true);r}
    fn ink_cmd(&mut self,c:u8,data:&[u8]) {self.level(9,false);self.level(15,false);self.spi(&[c]);if !data.is_empty(){self.level(15,true);self.spi(data);}self.level(9,true);}
    fn ink_ready(&mut self) {self.spi_route(18,23,34);self.output(9,true);self.output(15,false);self.output(0,true);self.output(12,true);self.ink_cmd(0x12,&[]);self.m.bus.tick(HZ/1000);self.ink_cmd(0x01,&[199,0,0]);self.ink_cmd(0x11,&[3]);self.ink_cmd(0x3c,&[5]);self.ink_cmd(0x18,&[0x80]);self.ink_cmd(0x0c,&[0x8b,0x9c,0x96,0x0f]);self.ink_cmd(0x21,&[0]);self.ink_cmd(0x44,&[0,24]);self.ink_cmd(0x45,&[0,0,199,0]);self.ink_cmd(0x4e,&[0]);self.ink_cmd(0x4f,&[0,0]);
        // Pinned M5GFX D67 initialization: F8 powers analog/clock without DISPLAY.
        self.ink_cmd(0x22,&[0xf8]);self.ink_cmd(0x20,&[]);self.m.bus.tick(HZ/1000);assert_eq!(self.m.bus.board.display_frames(),0);}
    fn ink_refresh(&mut self) {self.ink_cmd(0x22,&[0x14]);self.ink_cmd(0x20,&[]);}
    fn i2c_route(&mut self,sda:u8,scl:u8) {for(pin,sig)in[(sda,30),(scl,29)]{self.mux(pin,2);self.put(GPIO+0x130+4*sig,(1<<7)|u32::from(pin));self.put(GPIO+0x530+4*u32::from(pin),sig);}}
    fn transaction(&mut self,addr:u8,data:&[u8],read:usize)->(u32,Vec<u8>) {self.put(I2C+0x24,u32::MAX);self.put(I2C+0x18,3<<12);self.put(I2C+0x1c,(u32::from(addr)<<1)|u32::from(read!=0));for &b in data{self.put(I2C+0x1c,u32::from(b));}let mut cmds=vec![0,(1<<11)|(1<<8)|(1+data.len()as u32)];if read!=0{cmds.push((2<<11)|(1<<10)|read as u32);}cmds.push(3<<11);for(i,c)in cmds.into_iter().enumerate(){self.put(I2C+0x58+4*i as u32,c);}self.put(I2C+4,1<<5);let status=self.get(I2C+0x20);let b=(0..read).map(|_|self.get(I2C+0x1c)as u8).collect();(status,b)}
    fn rtc_read(&mut self)->Vec<u8> {assert_eq!(self.transaction(0x51,&[2],0).0&INT_NACK,0);let(s,b)=self.transaction(0x51,&[],7);assert_eq!(s&INT_NACK,0);b}
}
#[test]
fn paper_exact_info_four_rotations_gray_pixels_busy_and_bounds() {
    for rotation in 0..4 {
        let mut g=Guest::new("m5paper-v11");g.paper_ready();g.paper_cmd(0x302,&[]);assert_eq!(g.paper_read(4),[0,0,3,192,2,28,0x36,0xe0,0,0x12]);
        g.paper_cmd(0x21,&[0x120|rotation,0,0,4,1]);g.paper_frame(&[0,0x05af]);g.paper_cmd(0x22,&[]);
        let (x,y,w,h)=match rotation {0=>(0,0,4,1),1=>(0,536,1,4),2=>(956,539,4,1),_=>(959,0,1,4)};
        // Wrong buffer cannot schedule a refresh.
        g.paper_cmd(0x37,&[x,y,w,h,2,0,0x10]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),0);
        g.paper_cmd(0x37,&[x,y,w,h,2,0x36e0,0x12]);assert!(!g.input(27));assert_eq!(g.m.bus.board.display_frames(),0);g.m.bus.tick(HZ/100);assert!(!g.input(27));g.m.bus.tick(HZ/100);assert!(g.input(27));
        let(_,_,p,_)=g.m.bus.board.display().unwrap();
        for(n,gray)in[0,0x52aa,0xad55,0xffff].into_iter().enumerate(){let(nx,ny)=match rotation {0=>(n,0),1=>(0,539-n),2=>(959-n,539),_=>(959,n)};assert_eq!(p[(959-nx)*540+ny],gray,"rotation{rotation} pixel{n}");}
        assert_eq!(p.iter().filter(|&&p|p!=0xffff).count(),3);
        g.paper_cmd(0x21,&[0x120|rotation,65535,0,65535,1]);g.paper_frame(&[0,0]);g.paper_cmd(0x37,&[65535,0,65535,1,2,0x36e0,0x12]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),1);
    }
}
#[test]
fn paper_gpio_mux_cs_miso_power_sleep_and_optical_retention() {
    let mut g=Guest::new("m5paper-v11");g.paper_ready();g.paper_cmd(0x302,&[]);g.put(GPIO+0x130+4*9,0x80|19);assert!(g.paper_read(4).iter().all(|&b|b==255));g.put(GPIO+0x130+4*9,0x80|13);
    g.level(23,false);g.paper_cmd(0x302,&[]);assert!(g.paper_read(4).iter().all(|&b|b==255));g.level(23,true);
    g.paper_cmd(0x11,&[4,1]);g.paper_cmd(0x21,&[0x120,959,0,1,1]);g.paper_frame(&[0,0x0fff]);g.paper_cmd(0x22,&[]);
    g.level(4,false);g.paper_cmd(0x37,&[959,0,1,1,2,0x36e0,0x12]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),0,"shared SD CS must be deasserted");g.level(4,true);
    g.mux(12,3);g.paper_cmd(0x37,&[959,0,1,1,2,0x36e0,0x12]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),0);g.mux(12,2);
    g.paper_cmd(0x37,&[959,0,1,1,2,0x36e0,0x12]);g.m.bus.tick(HZ/20);let p=g.m.bus.board.display().unwrap().2;assert_eq!(p[0],0);
    g.paper_cmd(3,&[]);g.paper_cmd(0x37,&[0,0,960,540,2,0x36e0,0x12]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),1);
    g.level(23,false);assert_eq!(g.m.bus.board.display().unwrap().2,p,"EPD power loss preserves bistable image");g.level(23,true);g.paper_cmd(1,&[]);g.paper_cmd(0x37,&[959,0,1,1,2,0x36e0,0x12]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display().unwrap().2[0],0xffff,"power loss discarded volatile image RAM");
    g.m.reboot();assert_eq!(g.m.bus.board.display().unwrap().2[0],0xffff,"MCU reset retains visible glass");
    assert_eq!(Guest::new("m5paper-v11").m.bus.board.display_frames(),0);
}
#[test]
fn coreink_d67_guest_window_refresh_reset_sleep_and_old_m09_negative() {
    let mut g=Guest::new("coreink-d67");g.ink_ready();g.ink_cmd(0x44,&[1,1]);g.ink_cmd(0x45,&[2,0,2,0]);g.ink_cmd(0x4e,&[1]);g.ink_cmd(0x4f,&[2,0]);
    g.ink_cmd(0x24,&[0x7f]);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==65535));g.ink_refresh();assert!(g.input(4));g.m.bus.tick(HZ/100);assert!(g.input(4));g.m.bus.tick(HZ/100);assert!(!g.input(4));let p=g.m.bus.board.display().unwrap().2;assert_eq!(p[408],0);assert_eq!(p.iter().filter(|&&p|p==0).count(),1);
    g.level(0,false);g.ink_cmd(0x24,&[0]);g.ink_refresh();g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),1);assert_eq!(g.m.bus.board.display().unwrap().2,p);g.level(0,true);
    // Old M09 refresh0x12 is SSD1681 software reset, never a fake image update.
    g.ink_cmd(0x13,&[0]);g.ink_cmd(0x12,&[]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),1);
    g.ink_cmd(0x10,&[1]);g.ink_cmd(0x24,&[0]);g.ink_refresh();g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),1);g.level(0,false);g.level(0,true);
    g.mux(23,3);g.ink_cmd(0x22,&[0xf7]);g.ink_cmd(0x20,&[]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display_frames(),1);g.mux(23,2);
    assert_eq!(esp32::board::documented_memory_mib("coreink-d67"),Some((4,0)));assert_eq!((g.get(0x3ff5a00c)>>9)&7,5,"PICO-D4 package identity");g.m.reboot();assert_eq!(g.m.bus.board.display().unwrap().2,p,"MCU reboot preserves the visible glass");
}
#[test]
fn both_boards_real_guest_buttons_and_rtc_with_wrong_route_and_address_nacks() {
    for name in["m5paper-v11","coreink-d67"] {
        let mut g=Guest::new(name);assert_eq!(g.m.bus.board.name(),name);
        for pin in if name=="coreink-d67"{vec![37,38,39,5,27]}else{vec![37,38,39]} {assert!(g.input(pin));g.m.bus.gpio_set_input(pin,false);assert!(!g.input(pin));g.m.bus.gpio_set_input(pin,true);assert!(g.input(pin));}
        g.i2c_route(21,22);assert_eq!(g.transaction(0x51,&[0,0],0).0&INT_NACK,0);assert_eq!(g.transaction(0x51,&[2,0x59,0x59,0x23,0x28,6,2,0x24],0).0&INT_NACK,0);g.m.bus.tick(HZ);assert_eq!(g.rtc_read(),[0,0,0,0x29,0,2,0x24]);
        let initial=g.rtc_read();for _ in 0..5{assert_eq!(g.rtc_read(),initial,"host reads do not advance guest calendar");}
        g.i2c_route(32,33);assert_ne!(g.transaction(0x51,&[2],0).0&INT_NACK,0);g.i2c_route(21,22);assert_ne!(g.transaction(0x52,&[2],0).0&INT_NACK,0);assert_ne!(g.transaction(0x44,&[0],0).0&INT_NACK,0,"unmodeled SHT30 does not fake ACK");
        assert_eq!(g.rtc_read(),initial);
    }
}

// Independent classic revisions added after the original Paper/D67 checks.
#[test]
fn classic_exact_se_plus2_oled_guest_gpio_spi_and_i2c_identity() {
    for (name,clk,mosi,cs,dc,rst,bl,pin,package) in [
        ("stickc-plus-se",13,15,5,23,18,None,37,5),
        ("stickc-plus2",13,15,5,14,12,Some(27),35,6),
        ("m5stick-oled",18,23,14,27,33,None,35,0),
        ("stickt-original",13,15,5,23,18,None,37,5),
        ("tlite",13,15,5,23,18,None,37,5),
        ("bala-fire-original",18,23,14,27,33,Some(32),39,0),
    ] {
        let mut g=Guest::new(name);assert_eq!((g.get(0x3ff5a00c)>>9)&7,package,"{name}");
        assert!(g.input(pin));g.m.bus.gpio_set_input(pin,false);assert!(!g.input(pin));g.m.bus.gpio_set_input(pin,true);assert!(g.input(pin));
        g.i2c_route(21,22);assert_eq!(g.transaction(0x34,&[3],0).0&INT_NACK==0,matches!(name,"stickc-plus-se"|"stickt-original"|"tlite"));
        assert_eq!(g.transaction(0x68,&[0x75],0).0&INT_NACK==0,matches!(name,"stickc-plus2"|"stickt-original"|"tlite"));
        g.i2c_route(32,33);assert_ne!(g.transaction(0x51,&[2],0).0&INT_NACK,0);
        g.spi_route(clk,mosi,34);for p in[cs,dc,rst]{g.output(p,true);}if let Some(p)=bl{g.output(p,true);}
        let command=|g:&mut Guest,c:u8,data:&[u8]|{g.level(cs,false);g.level(dc,false);g.spi(&[c]);if !data.is_empty(){if name!="m5stick-oled"{g.level(dc,true);}g.spi(data);}g.level(cs,true);};
        if name=="m5stick-oled" {for c in[0xaf,0xa1,0xc8,0xb0,0x10,0]{command(&mut g,c,&[]);}command(&mut g,0xd3,&[96]);command(&mut g,0xdc,&[0]);}
        else {command(&mut g,0x11,&[]);command(&mut g,0x29,&[]);command(&mut g,0x3a,&[0x55]);command(&mut g,0x36,&[if name=="bala-fire-original"{0x28}else{8}]);let(x,y)=if name=="bala-fire-original"{(0,0)}else{(52,40)};command(&mut g,0x2a,&[0,x,0,x]);command(&mut g,0x2b,&[0,y,0,y]);command(&mut g,0x2c,&[]);}
        g.level(cs,false);g.level(dc,true);g.spi(if name=="m5stick-oled"{&[1]}else{&[0xf8,0]});g.level(cs,true);
        let pixel=if name=="m5stick-oled"{0xffff}else{0xf800};assert_eq!(g.m.bus.board.display().unwrap().2[0],pixel,"{name}");
        g.mux(mosi,3);g.level(cs,false);g.spi(&[0,0]);g.level(cs,true);assert_eq!(g.m.bus.board.display().unwrap().2[0],pixel,"wrong IO MUX does not draw");
        g.level(rst,false);assert_eq!(g.m.bus.board.display().unwrap().2[0],0,"reset blanks glass");
    }
}
#[test]
fn paper_original_and_comm_guest_display_touch_scan_and_sd_routes() {
    for name in["m5paper-original","m5paper-comm"] {
        let mut g=Guest::new(name);assert_eq!(g.m.bus.board.name(),name);g.paper_ready();
        for (id,pin) in[("rotary-right",37),("rotary-press",38),("rotary-left",39)]{assert_eq!(g.m.bus.board.named_pin(id),Some(pin));assert!(g.input(pin));g.m.bus.gpio_set_input(pin,false);assert!(!g.input(pin));g.m.bus.gpio_set_input(pin,true);}
        g.paper_cmd(0x21,&[0x120,959,0,1,1]);g.paper_frame(&[0,0]);g.paper_cmd(0x22,&[]);g.paper_cmd(0x37,&[959,0,1,1,2,0x36e0,0x12]);g.m.bus.tick(HZ/20);assert_eq!(g.m.bus.board.display().unwrap().2[0],0);
        g.i2c_route(21,22);g.transaction(0x14,&[0x81,0x4e,0],0);g.m.bus.board.touch(539,959,true);g.m.bus.tick(HZ/100);
        assert_eq!(g.transaction(0x14,&[0x81,0x4e],0).0&INT_NACK,0);let(s,p)=g.transaction(0x14,&[],6);assert_eq!(s&INT_NACK,0);assert_eq!(p,[0x81,0,0x1b,2,0xbf,3]);
        assert_eq!(g.transaction(0x14,&[0x81,0x4e,0],0).0&INT_NACK,0);assert_eq!(g.transaction(0x14,&[0x81,0x4e],0).0&INT_NACK,0);assert_eq!(g.transaction(0x14,&[],1).1,[0]);g.m.bus.board.touch(1,2,false);g.m.bus.tick(HZ/100);g.transaction(0x14,&[0x81,0x4e],0);assert_eq!(g.transaction(0x14,&[],6).1,[0x80,0,1,0,2,0]);
        g.i2c_route(32,33);assert_ne!(g.transaction(0x14,&[0x81,0x4e],0).0&INT_NACK,0);g.i2c_route(21,22);assert_ne!(g.transaction(0x5d,&[0x81,0x4e],0).0&INT_NACK,0);
        use esp_periph::sdmmc::MemStore;g.m.bus.board.attach_spi_sd_card(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();g.level(15,true);g.level(4,true);g.spi(&[255;10]);g.level(4,false);g.spi(&[0x40,0,0,0,0,0x95]);assert_eq!(g.spi(&[255;2]),[255,1]);g.level(4,true);assert_eq!(g.m.bus.board.display_frames(),1,"SD commands do not draw on the e-paper");
    }
}
#[test]
fn tlite_and_original_stickt_synthetic_environment_reaches_real_guest_i2c_spi_bytes() {
    let mut g=Guest::new("tlite");g.i2c_route(0,26);
    g.transaction(0x33,&[0x24,7],0);assert_eq!(g.transaction(0x33,&[],4).1,[0x53,0x49,0x4d,0],"explicitly virtual calibration identity");
    g.transaction(0x33,&[4,0],0);assert_eq!(g.transaction(0x33,&[],2).1,8000u16.to_be_bytes());
    assert!(g.m.bus.board.thermal_environment_at(g.m.bus.cycles,[7000,9000,80,60,12]));g.transaction(0x33,&[0x80,0,0,0x30],0);g.m.bus.tick(HZ/2);g.transaction(0x33,&[4,0],0);assert_eq!(g.transaction(0x33,&[],2).1,7000u16.to_be_bytes());
    g.i2c_route(21,22);assert_ne!(g.transaction(0x33,&[4,0],0).0&INT_NACK,0,"sensor stays on the real HAT pins0/26");
    let mut g=Guest::new("stickt-original");g.output(26,true);g.output(0,true);g.i2c_route(21,22);g.transaction(0x2a,&[0,6,0,1,0,5],0);g.transaction(0x2a,&[0,4,0x48,0x55],0);g.transaction(0x2a,&[0,2],0);g.transaction(0x2a,&[],2);g.m.bus.tick(HZ/100);g.spi_route(2,15,25);g.output(0,false);
    let frame=g.spi(&[0;6]);assert_eq!(&frame[4..6],&8000u16.to_be_bytes());for n in[64,64,30]{g.spi(&vec![0;n]);}
    assert!(g.m.bus.board.thermal_environment_at(g.m.bus.cycles,[7000,9000,80,60,12]));assert_eq!(&g.spi(&[0;6])[4..6],&7000u16.to_be_bytes());
    g.output(0,true);assert_eq!(g.spi(&[0;6]),[255;6]);
}
