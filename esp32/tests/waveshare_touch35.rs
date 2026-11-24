//! Actual LX6 s32i/l32i instructions drive classic ESP32 GPIO/I2C/SPI MMIO.
//! No host framebuffer/register injection. Sources: pinned Waveshare factory BSP;
//! register addresses/selectors follow IDF v5.5.4 ESP32 headers (same classic IDF4.4 map).
use emu_core::{Bus,Core};
use esp_soc::SocBus as _;
use esp_periph::i2c::INT_NACK;
const GPIO:u32=0x3ff44000;
const MUX:u32=0x3ff49000;
const SPI:u32=0x3ff64000;
const I2C:u32=0x3ff53000;
struct Guest {m:esp32::Machine}
impl Guest {
    fn new()->Self {
        let mut m=esp32::machine([0;6],16<<20);m.bus.set_psram_size(2<<20).unwrap();
        m.bus.board=esp32::board::make_board("waveshare-esp32-touch35").unwrap();m.bus.attach_board_devices();
        // s32i a3,a4,0; l32i a5,a4,0. Each operation is executed by LX6.
        m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();Self{m}
    }
    fn put(&mut self,a:u32,v:u32) {
        let cpu=&mut self.m.cores[0];cpu.pc=0x40080000;cpu.set_ar(4,a);cpu.set_ar(3,v);
        assert!(cpu.step(&mut self.m.bus).trap().is_none(),"guest store {a:#x}");
    }
    fn get(&mut self,a:u32)->u32 {
        let cpu=&mut self.m.cores[0];cpu.pc=0x40080003;cpu.set_ar(4,a);
        assert!(cpu.step(&mut self.m.bus).trap().is_none(),"guest load {a:#x}");cpu.get_ar(5)
    }
    fn mux(&mut self,pin:u8,function:u32) {
        let off=match pin {0=>0x44,5=>0x6c,15=>0x3c,18=>0x70,19=>0x74,21=>0x7c,22=>0x80,23=>0x8c,25=>0x24,26=>0x28,27=>0x2c,36=>4,37=>8,_=>panic!("test pad")};
        self.put(MUX+off,(function<<12)|(1<<9));
    }
    fn output(&mut self,pin:u8,high:bool) {self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),256);self.put(GPIO+0x24,1<<pin);self.level(pin,high);}
    fn level(&mut self,pin:u8,high:bool) {self.put(GPIO+if high{8}else{12},1<<pin);}
    fn i2c_route(&mut self,sda:u8,scl:u8) {
        for (pin,signal)in[(sda,30),(scl,29)] {self.mux(pin,2);self.put(GPIO+0x130+4*signal,(1<<7)|u32::from(pin));self.put(GPIO+0x530+4*u32::from(pin),signal);}
    }
    fn transaction(&mut self,address:u8,bytes:&[u8],read:usize)->(u32,Vec<u8>) {
        self.put(I2C+0x24,u32::MAX);self.put(I2C+0x18,3<<12);self.put(I2C+0x1c,(u32::from(address)<<1)|u32::from(read!=0));
        for &byte in bytes {self.put(I2C+0x1c,u32::from(byte));}
        let mut commands=vec![0,(1<<11)|(1<<8)|(1+bytes.len()as u32)];
        if read!=0 {commands.push((2<<11)|(1<<10)|read as u32);}
        commands.push(3<<11);for (i,c)in commands.into_iter().enumerate(){self.put(I2C+0x58+4*i as u32,c);}
        self.put(I2C+4,1<<5);let status=self.get(I2C+0x20);let bytes=(0..read).map(|_|self.get(I2C+0x1c)as u8).collect();(status,bytes)
    }
    fn write(&mut self,address:u8,register:u8,value:u8) {assert_eq!(self.transaction(address,&[register,value],0).0&INT_NACK,0);}
    fn read(&mut self,address:u8,register:u8,count:usize)->Vec<u8> {
        assert_eq!(self.transaction(address,&[register],0).0&INT_NACK,0);let(s,b)=self.transaction(address,&[],count);assert_eq!(s&INT_NACK,0);b
    }
    fn spi_route(&mut self) {
        for(pin,signal)in[(18,8),(23,10)]{self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),signal);}
        self.mux(19,2);self.put(GPIO+0x130+4*9,0x80|19);
        for(pin,high)in[(5,true),(15,true),(27,false),(25,true)]{self.output(pin,high);}
    }
    fn spi(&mut self,data:&[u8],read:bool)->Vec<u8> {
        assert!(!data.is_empty()&&data.len()<=64);
        for(i,c)in data.chunks(4).enumerate(){let mut w=[0;4];w[..c.len()].copy_from_slice(c);self.put(SPI+0x80+4*i as u32,u32::from_le_bytes(w));}
        self.put(SPI+0x1c,(1<<27)|if read{(1<<28)|1}else{0});
        self.put(SPI+0x28,data.len()as u32*8-1);self.put(SPI+0x2c,data.len()as u32*8-1);self.put(SPI,1<<18);
        if !read{return Vec::new();}let mut result=Vec::new();for i in 0..data.len().div_ceil(4){result.extend(self.get(SPI+0x80+4*i as u32).to_le_bytes());}result.truncate(data.len());result
    }
    fn command(&mut self,c:u8,data:&[u8]) {
        self.level(5,false);self.level(27,false);self.spi(&[c],false);
        if !data.is_empty(){self.level(27,true);self.spi(data,false);}self.level(5,true);
    }
    fn init_panel(&mut self) {
        self.i2c_route(21,22);self.spi_route();self.write(0x20,3,0xf8);self.write(0x20,1,0xf8);self.write(0x20,1,0xff);
        for(c,d)in[(0x11,&[][..]),(0x29,&[][..]),(0x3a,&[0x55][..]),(0x36,&[8][..]),(0x2a,&[0,10,0,10][..]),(0x2b,&[0,20,0,20][..])]{self.command(c,d);}
    }
}

#[test]
fn waveshare_classic_guest_pixels_reject_wrong_routes_reset_and_unsupported_pixel_modes() {
    let mut g=Guest::new();g.init_panel();g.command(0x2c,&[0xf8,0]);
    let(w,h,pixels,v)=g.m.bus.board.display().unwrap();assert_eq!((w,h),(320,480));assert_eq!(pixels[20*320+10],0xf800);assert_eq!(pixels.iter().filter(|&&p|p!=0).count(),1);
    g.mux(23,3);g.command(0x2c,&[0,31]);assert_eq!(g.m.bus.board.display().unwrap().2,pixels,"wrong MOSI cannot change GRAM");g.mux(23,2);
    for pin in[18,27] {g.mux(pin,3);g.command(0x2c,&[0,31]);assert_eq!(g.m.bus.board.display().unwrap().2,pixels,"disconnected SCLK/DC cannot write GRAM");g.mux(pin,2);}
    g.level(15,false);g.command(0x2c,&[0,31]);g.level(15,true);assert_eq!(g.m.bus.board.display().unwrap().2,pixels,"two selected devices cannot be prioritized");
    g.command(0x3a,&[0x66]);g.command(0x2c,&[0,31]);assert_eq!(g.m.bus.board.display().unwrap().2,pixels,"unsupported RGB666 is not decoded as RGB565");g.command(0x3a,&[0x55]);
    g.level(25,false);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0));assert!(g.m.bus.board.display_version()>v);g.level(25,true);
    g.write(0x20,1,0xfe);g.command(0x2c,&[0,31]);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0));
    g.write(0x20,1,0xff);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0),"reset requires sleep-out/display-on again");
    g.command(0x11,&[]);g.command(0x29,&[]);assert_eq!(g.m.bus.board.display().unwrap().2[20*320+10],0x001f,"hardware reset retains GRAM; default BGR changes interpretation");
}

#[test]
fn waveshare_classic_guest_touch_boot_power_codec_rtc_and_i2c_pin_gate() {
    let mut g=Guest::new();g.i2c_route(21,22);for pin in[0,36,37]{g.mux(pin,2);}
    assert_eq!(g.read(0x34,3,1),[0x4a]);assert_eq!(g.read(0x18,0xfd,1),[0x83]);g.write(0x18,0x32,0xbf);assert_eq!(g.read(0x18,0x32,1),[0xbf]);
    g.write(0x51,4,0);let before=g.m.bus.cycles;
    // The reset CPU runs from 40 MHz XTAL; the board horizon is normalized to 240 MHz.
    g.m.bus.tick(40_000_000);assert_eq!(g.m.bus.cycles-before,esp32::periph::CPU_HZ);assert_eq!(g.read(0x51,4,1),[1]);
    g.m.bus.board.touch_at(g.m.bus.cycles,123,456,true);assert_eq!(g.get(GPIO+0x40)&(1<<5),0);
    assert_eq!(g.read(0x38,2,5),[1,0x80,123,1,200]);g.m.bus.board.touch_at(g.m.bus.cycles,123,456,false);
    assert_ne!(g.get(GPIO+0x40)&(1<<5),0);g.read(0x38,2,1);assert_eq!(g.read(0x38,2,1),[0],"latched contact drains after the driver double read");
    g.write(0x20,3,0xf8);g.write(0x20,1,0xfd);assert_ne!(g.transaction(0x38,&[2],0).0&INT_NACK,0,"touch reset low disconnects control response");
    g.write(0x20,1,0xff);assert_eq!(g.read(0x38,2,1),[0]);
    for(id,pin,active)in[("boot",0,false),("power",36,true)] {
        for pressed in[true,false] {assert!(g.m.bus.board.button_at(g.m.bus.cycles,id,pressed));let reg=GPIO+if pin<32{0x3c}else{0x40};assert_eq!(g.get(reg)&(1<<(pin%32))!=0,if pressed{active}else{!active});}
    }
    g.i2c_route(26,22);assert_ne!(g.transaction(0x20,&[0],0).0&INT_NACK,0);assert_ne!(g.transaction(0x18,&[0xfd],0).0&INT_NACK,0);
    g.i2c_route(21,22);assert_eq!(g.read(0x18,0x32,1),[0xbf]);
    assert_ne!(g.transaction(0x39,&[0],0).0&INT_NACK,0,"no neighboring touch address invented");
    g.m.reboot();g.m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();g.i2c_route(21,22);
    assert_eq!(g.read(0x18,0x32,1),[0xbf],"MCU reboot retains the independently powered codec");
    assert_eq!(g.read(0x51,4,1),[1]);g.m.bus.tick(40_000_000);assert_eq!(g.read(0x51,4,1),[2],"RTC resumes immediately after MCU cycle-origin reset");
    let mut fresh=Guest::new();fresh.i2c_route(21,22);assert_eq!(fresh.read(0x18,0x32,1),[0]);assert_eq!(fresh.read(0x51,4,1),[0x80],"new board starts with documented oscillator-stop indication");
}

#[test]
fn waveshare_classic_guest_spi_card_cmd0_cmd8_and_miso_gate() {
    let mut g=Guest::new();g.spi_route();g.m.bus.board.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
    g.spi(&[255;10],true);g.level(15,false);
    let reply=g.spi(&[0x40,0,0,0,0,0x95,255,255],true);assert_eq!(&reply[6..8],&[255,1],"NCR byte precedes R1");
    let reply=g.spi(&[0x48,0,0,1,0xaa,0x87,255,255,255,255,255,255],true);assert_eq!(&reply[6..12],&[255,1,0,0,1,0xaa]);
    g.put(GPIO+0x130+4*9,0x80|26);let reply=g.spi(&[0x48,0,0,1,0xaa,0x87,255,255,255,255,255],true);assert!(reply.iter().all(|&b|b==255),"wrong MISO cannot read card replies");
    g.level(15,true);
}
