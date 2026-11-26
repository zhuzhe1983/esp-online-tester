//! Independent K117/U120 LX6 guest MMIO verification.
//! ESP-IDF5.5.4 classic register map, compatible with the documented Arduino IDF4.4 path.
use emu_core::{Bus,Core};
use esp_soc::SocBus as _;
use esp_periph::i2c::INT_NACK;
const OFFSETS:[u32;40]=[0x44,0x88,0x40,0x84,0x48,0x6c,0x60,0x64,0x68,0x54,0x58,0x5c,0x34,0x38,0x30,0x3c,0x4c,0x50,0x70,0x74,0x78,0x7c,0x80,0x8c,0x90,0x24,0x28,0x2c,0,0,0,0,0x1c,0x20,0x14,0x18,4,8,0x0c,0x10];
const GPIO:u32=0x3ff44000;
const MUX:u32=0x3ff49000;
const I2C:u32=0x3ff53000;
struct Guest {m:esp32::Machine}
impl Guest {
    fn new(name:&str)->Self {
        let (flash,psram)=esp32::board::documented_memory_mib(name).unwrap();
        let mut m=esp32::machine([0;6],flash<<20);m.bus.set_psram_size(psram<<20).unwrap();
        m.bus.board=esp32::board::make_board(name).unwrap();m.bus.attach_board_devices();
        // s32i a3,a4,0; l32i a5,a4,0. Each operation is executed by LX6.
        m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();let mut g=Self{m};g.put(0x3ff48070,1<<27);g.put(0x3ff0003c,2);g
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
        let off=OFFSETS[pin as usize];
        self.put(MUX+off,(function<<12)|(1<<9));
    }
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
    fn output(&mut self,pin:u8,high:bool) {self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),256);self.put(GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));self.level(pin,high);}
    fn level(&mut self,pin:u8,high:bool) {self.put(GPIO+match(pin<32,high){(true,true)=>8,(true,false)=>12,(false,true)=>20,(false,false)=>24},1<<(pin%32));}
    fn spi(&mut self,data:&[u8])->Vec<u8> {
        const SPI:u32=0x3ff64000;
        for(i,c)in data.chunks(4).enumerate(){let mut w=[0;4];w[..c.len()].copy_from_slice(c);self.put(SPI+0x80+4*i as u32,u32::from_le_bytes(w));}
        self.put(SPI+0x1c,(1<<27)|(1<<28)|1);self.put(SPI+0x28,data.len()as u32*8-1);self.put(SPI+0x2c,data.len()as u32*8-1);self.put(SPI,1<<18);
        let mut result=Vec::new();for i in 0..data.len().div_ceil(4){result.extend(self.get(SPI+0x80+4*i as u32).to_le_bytes());}result.truncate(data.len());result
    }
    fn rmt(&mut self,pin:u8,grb:&[u8]) {
        const RMT:u32=0x3ff56000;self.put(0x3ff000c0,1<<9);self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),87);
        let bits:Vec<_>=grb.iter().flat_map(|byte|(0..8).rev().map(move|bit|byte&(1<<bit)!=0)).collect();
        for(i,&one)in bits.iter().enumerate(){let hi=if one{64}else{32};let lo=100-hi;self.put(RMT+0x800+4*i as u32,hi|0x8000|(lo<<16));}
        self.put(RMT+0x800+4*bits.len()as u32,0);self.put(RMT+0x20,(1<<24)|1);self.put(RMT+0x24,(1<<17)|(1<<19)|(1<<3)|1);self.m.bus.tick(30_000);
    }
}

#[test]
fn atomu_independent_package_gpio39_and_gpio27_rmt_guest_paths() {
    let mut g=Guest::new("atomu");assert_eq!(g.m.bus.board.name(),"atomu");
    assert_eq!((g.get(0x3ff5a00c)>>9)&7,5);assert!(g.m.bus.board.display().is_none());
    g.mux(39,2);assert_ne!(g.get(GPIO+0x40)&0x80,0);g.m.bus.gpio_set_input(39,false);assert_eq!(g.get(GPIO+0x40)&0x80,0);g.m.bus.gpio_set_input(39,true);assert_ne!(g.get(GPIO+0x40)&0x80,0);
    g.rmt(26,&[0x12,0x34,0x56]);assert_eq!(g.m.bus.board.leds().unwrap().1,0);g.put(GPIO+0x530+4*26,256);
    g.rmt(27,&[0x12,0x34,0x56]);assert_eq!(g.m.bus.board.leds().unwrap().0,[[0x34,0x12,0x56]]);
    g.m.reboot();g.m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();g.mux(39,2);assert_ne!(g.get(GPIO+0x40)&0x80,0);
    let mut fresh=Guest::new("atomu");assert_eq!(fresh.m.bus.board.leds().unwrap().1,0);fresh.rmt(27,&[1,2,3]);assert_eq!(fresh.m.bus.board.leds().unwrap().0,[[2,1,3]]);
}
impl Guest {
    fn lcd_ready(&mut self) {for(pin,sig)in[(13,8),(15,10)] {self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),sig);}for pin in [5,18] {self.output(pin,true);}self.output(4,false);self.output(23,false);for(c,p)in[(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x36,vec![8])] {self.lcd_cmd(c,&p);}}
    fn lcd_cmd(&mut self,c:u8,p:&[u8]) {self.level(5,false);self.level(23,false);self.spi(&[c]);if !p.is_empty() {self.level(23,true);self.spi(p);}self.level(5,true);}
    fn lcd_pixel(&mut self,x:u16,y:u16,color:u16) {let[a,b]=x.to_be_bytes();self.lcd_cmd(0x2a,&[a,b,a,b]);let[a,b]=y.to_be_bytes();self.lcd_cmd(0x2b,&[a,b,a,b]);self.lcd_cmd(0x2c,&color.to_be_bytes());}
}
#[test]
fn unit_lcd_real_spi_pixels_rotations_backlight_reset_and_no_local_slave_ack() {
    let mut g=Guest::new("unit-lcd");g.lcd_ready();g.lcd_pixel(52,40,0xf800);assert_eq!(g.m.bus.board.display().unwrap().2[0],0xf800);
    g.lcd_cmd(0x36,&[0x68]);g.lcd_pixel(40,53,0x07e0);assert_eq!(g.m.bus.board.display().unwrap().2[134],0x07e0);
    let before=g.m.bus.board.display().unwrap().2;g.put(GPIO+0x530+4*13,256);g.lcd_pixel(40,53,0xffff);assert_eq!(g.m.bus.board.display().unwrap().2,before);g.put(GPIO+0x530+4*13,8);
    g.level(5,true);g.level(23,true);g.spi(&[0,0]);assert_eq!(g.m.bus.board.display().unwrap().2,before);
    g.lcd_cmd(0x28,&[]);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0));g.lcd_cmd(0x29,&[]);assert_eq!(g.m.bus.board.display().unwrap().2,before);
    g.level(4,true);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0));g.level(4,false);assert_eq!(g.m.bus.board.display().unwrap().2,before);
    g.level(18,false);g.lcd_pixel(40,53,0xffff);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0));g.level(18,true);
    g.i2c_route(21,22);assert_ne!(g.transaction(0x3e,&[0x04],0).0&INT_NACK,0,"external host address is not an internal local device");
}
impl Guest {
    fn mlx_words(&mut self,reg:u16,words:usize)->Vec<u16> {assert_eq!(self.transaction(0x33,&reg.to_be_bytes(),0).0&INT_NACK,0);let(s,b)=self.transaction(0x33,&[],words*2);assert_eq!(s&INT_NACK,0);b.as_chunks::<2>().0.iter().map(|v|u16::from_be_bytes([v[0],v[1]])).collect()}
    fn mlx_write(&mut self,reg:u16,value:u16) {let b:Vec<_>=reg.to_be_bytes().into_iter().chain(value.to_be_bytes()).collect();assert_eq!(self.transaction(0x33,&b,0).0&INT_NACK,0);}
}
#[test]
fn thermal2_guest_reads_sensor_raw_words_calibration_ready_subpages_and_rgb() {
    let mut g=Guest::new("unit-thermal2");g.i2c_route(21,22);
    assert_eq!(g.mlx_words(0x2430,4),[10000,16384,8,0x20ff]);assert_eq!(g.mlx_words(0x2440,2),[0x10;2]);
    assert_eq!(g.mlx_words(0x8000,1)[0]&8,0);g.m.bus.tick(120_000_000);assert_eq!(g.mlx_words(0x8000,1)[0]&9,9);
    // Manufacturer equation, applied to raw data read by actual guest l32i.
    let gain=g.mlx_words(0x070a,1)[0];let ptat=g.mlx_words(0x0720,1)[0];let art=g.mlx_words(0x0700,1)[0];let vdd=g.mlx_words(0x072a,1)[0]as i16;
    assert_eq!(gain,10000);assert_eq!((f64::from(vdd)+8224.0)/1024.0+3.3,3.3);assert_eq!(f64::from(ptat)/(8.0*f64::from(ptat)+f64::from(art))*262144.0,16384.0);
    assert!(g.m.bus.board.thermal_environment_at(g.m.bus.cycles,[1234,4321,0,0,0]));g.mlx_write(0x8000,0x30);assert_eq!(g.mlx_words(0x8000,1)[0]&9,1);
    let old=g.mlx_words(0x0400,2);assert_eq!(old,[1000,1000]);g.m.bus.tick(120_000_000);assert_eq!(g.mlx_words(0x8000,1)[0]&9,8);assert_eq!(g.mlx_words(0x0400,2),[4321,1234]);
    // Read-only EEPROM; wrong wires, old Thermal address and external slave are NACK.
    g.mlx_write(0x2430,0xffff);assert_eq!(g.mlx_words(0x2430,1),[10000]);assert_ne!(g.transaction(0x32,&[0],0).0&INT_NACK,0);g.i2c_route(33,32);assert_ne!(g.transaction(0x33,&[0x24,0],0).0&INT_NACK,0);g.i2c_route(21,22);
    assert!(!g.m.bus.board.thermal_environment_at(g.m.bus.cycles,[1,2,32,0,0]));
    g.mlx_write(0x800d,0x1b81);g.mlx_write(0x8000,0x30);g.m.bus.tick(3_750_000);assert_eq!(g.mlx_words(0x8000,1)[0]&8,8,"64Hz refresh is timed by guest cycles");
    // Internal factory uses SPI MOSI27 with100/110 symbols, not a fake RGB setter.
    g.mux(27,2);g.put(GPIO+0x530+4*27,10);let mut symbols=Vec::new();for b in [0x12u8,0x34,0x56] {for i in (0..8).rev(){symbols.extend([true,b&(1<<i)!=0,false]);}}symbols.resize(256,false);let bytes:Vec<_>=symbols.as_chunks::<8>().0.iter().map(|b|b.iter().fold(0u8,|v,b|(v<<1)|u8::from(*b))).collect();g.spi(&bytes);assert_eq!(g.m.bus.board.leds().unwrap().0,[[0x34,0x12,0x56]]);
    g.mux(39,2);assert_ne!(g.get(GPIO+0x40)&0x80,0);g.m.bus.gpio_set_input(39,false);assert_eq!(g.get(GPIO+0x40)&0x80,0);assert!(g.m.bus.board.display().is_none());
}
impl Guest {
    fn hdmi_ready(&mut self) {for(pin,sig)in[(5,8),(19,10)] {self.mux(pin,2);self.put(GPIO+0x530+4*u32::from(pin),sig);}self.mux(22,2);self.put(GPIO+0x130+4*9,0x80|22);self.output(33,true);}
    fn hdmi(&mut self,b:&[u8])->Vec<u8> {self.level(33,false);let r=self.spi(b);self.level(33,true);r}
}
#[test]
fn atom_psram_display_guest_fpga_spi_external_monitor_and_lt8618_registers() {
    let mut g=Guest::new("atom-psram-display");assert_eq!(esp32::board::documented_memory_mib("atom-psram-display"),Some((8,2)));assert_eq!((g.get(0x3ff5a00c)>>9)&7,6);
    // Actual PSRAM-backed address accesses, not inherited AtomLite capacity.
    for (off,value) in [(0x34,5),(0x1c,1<<31),(0x24,0x70000035),(0,1<<18)] {g.put(0x3ff42000+off,value);}
    for (off,value) in [(0x34,0),(0x54,(23<<22)|(1<<28)|0x34),(0x5c,0x700000eb),(0x60,0x70000038)] {g.put(0x3ff43000+off,value);}
    for base in [0x3ff00040,0x3ff00058] {g.put(base,8);g.put(base+4,0);}
    for base in [0x3ff11200,0x3ff13200] {g.put(base,0);g.put(base+63*4,63);}
    g.put(0x3f800000,0x12345678);g.put(0x3f9ffffc,0xaabbccdd);assert_eq!(g.get(0x3f800000),0x12345678);assert_eq!(g.get(0x3f9ffffc),0xaabbccdd);
    g.hdmi_ready();g.level(33,false);g.spi(&[4]);assert_eq!(g.spi(&[0;6]),[0,255,b'H',b'D',1,0]);g.level(33,true);
    // Public M5GFX commands set a4x3 logical external monitor, then RGB565 RAM.
    g.hdmi(&[0x18,1,1,0,4,0,3,0xde]);assert_eq!(g.m.bus.board.display().unwrap().0,4);
    g.hdmi(&[0x2a,0,1,0,2,0x2b,0,1,0,1,0x42,0xf8,0,0x07,0xe0]);let(w,h,p,_)=g.m.bus.board.display().unwrap();assert_eq!((w,h),(4,3));assert_eq!((p[5],p[6]),(0xf800,0x07e0));assert_eq!(p.iter().filter(|&&p|p!=0).count(),2);
    g.hdmi(&[0x6a,0,0,0,0,0,0,0,0,0,0x1f]);assert_eq!(g.m.bus.board.display().unwrap().2[0],0x001f);
    g.level(33,false);g.spi(&[0x2a,0,1,0,2,0x2b,0,1,0,1,0x82]);g.put(GPIO+0x130+4*9,0x80|23);assert_eq!(g.spi(&[0;4]),[255;4]);g.put(GPIO+0x130+4*9,0x80|22);assert_eq!(g.spi(&[0;4]),[0xf8,0,0x07,0xe0]);g.level(33,true);
    // Copy source rectangle, destination(2,2), bounds clamped by model memory.
    g.hdmi(&[0x23,0,1,0,1,0,2,0,1,0,2,0,2]);assert_eq!(&g.m.bus.board.display().unwrap().2[10..12],&[0xf800,0x07e0]);
    let before=g.m.bus.board.display().unwrap().2;g.put(GPIO+0x530+4*5,256);g.hdmi(&[0x62,0,0,0,0,0xff,0xff]);assert_eq!(g.m.bus.board.display().unwrap().2,before);g.put(GPIO+0x530+4*5,8);
    g.level(33,true);g.spi(&[0x62,0,0,0,0,0xff,0xff]);assert_eq!(g.m.bus.board.display().unwrap().2,before);
    g.i2c_route(25,21);g.transaction(0x39,&[255,0x81],0);g.transaction(0x39,&[0x30,0xea],0);assert_eq!(g.transaction(0x39,&[0x30],0).0&INT_NACK,0);assert_eq!(g.transaction(0x39,&[],1).1,[0xea]);
    g.i2c_route(21,25);assert_ne!(g.transaction(0x39,&[0x30],0).0&INT_NACK,0);assert!(g.m.bus.board.hardware_diagnostics().unwrap().contains("external-hdmi-fpga-memory"));
    g.mux(39,2);assert_ne!(g.get(GPIO+0x40)&0x80,0);g.m.bus.gpio_set_input(39,false);assert_eq!(g.get(GPIO+0x40)&0x80,0);g.rmt(27,&[1,2,3]);assert_eq!(g.m.bus.board.leds().unwrap().0,[[2,1,3]]);
}
