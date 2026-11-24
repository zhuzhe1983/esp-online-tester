//! Actual LX6 loads/stores reach physical GPIO/I2C/RMT routes, not a host-painted display.
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
        let mut m=esp32::machine([0;6],4<<20);
        m.bus.board=esp32::board::make_board(name).unwrap();m.bus.attach_board_devices();
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
    fn read(&mut self,address:u8,register:u8,count:usize)->Vec<u8> {
        assert_eq!(self.transaction(address,&[register],0).0&INT_NACK,0);let(s,b)=self.transaction(address,&[],count);assert_eq!(s&INT_NACK,0);b
    }
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
fn servo_oled_real_guest_i2c_mux_address_display_off_and_reboot() {
    let mut g=Guest::new("waveshare-servo-driver-esp32");g.i2c_route(21,22);
    assert_eq!(g.transaction(0x3c,&[0,0xaf,0xa8,31,0xa0,0xc0,0x20,0,0x21,1,1,0x22,0,0],0).0&INT_NACK,0);
    assert_eq!(g.transaction(0x3c,&[0x40,4],0).0&INT_NACK,0);let(w,h,pixels,_)=g.m.bus.board.display().unwrap();assert_eq!((w,h),(128,32));assert_eq!(pixels[257],0xffff);assert_eq!(pixels.iter().filter(|&&p|p!=0).count(),1);
    g.i2c_route(22,21);assert_ne!(g.transaction(0x3c,&[0,0xae],0).0&INT_NACK,0);g.i2c_route(21,22);
    assert_ne!(g.transaction(0x3d,&[0,0xae],0).0&INT_NACK,0);assert_eq!(g.m.bus.board.display().unwrap().2,pixels);
    g.transaction(0x3c,&[0,0xae],0);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0));g.transaction(0x3c,&[0,0xaf],0);
    g.m.reboot();g.m.bus.load_bytes(0x40080000,&[0x32,0x64,0,0x52,0x24,0]).unwrap();assert_eq!(g.m.bus.board.display().unwrap().2,pixels,"MCU reset is not OLED power loss");
    assert!(Guest::new("waveshare-servo-driver-esp32").m.bus.board.display().unwrap().2.iter().all(|&p|p==0));
}
#[test]
fn servo_two_schematic_leds_follow_real_guest_rmt_and_boot_input() {
    let mut g=Guest::new("waveshare-servo-driver-esp32");g.rmt(22,&[0x22,0x44,0x66,0x11,0x33,0x55]);assert_eq!(g.m.bus.board.leds().unwrap().1,0);
    g.put(GPIO+0x530+4*22,256);g.rmt(23,&[0x22,0x44,0x66,0x11,0x33,0x55]);assert_eq!(g.m.bus.board.leds().unwrap().0,[[0x44,0x22,0x66],[0x33,0x11,0x55]]);
    g.mux(0,2);assert_ne!(g.get(GPIO+0x3c)&1,0);g.m.bus.gpio_set_input(0,false);assert_eq!(g.get(GPIO+0x3c)&1,0);g.m.bus.gpio_set_input(0,true);assert_ne!(g.get(GPIO+0x3c)&1,0);
}
#[test]
fn devkit_exact_module_profiles_have_gpio27_rgb_and_no_implicit_screen() {
    for(name,flash,psram)in esp32::board::waveshare_devkit::PROFILES {
        let mut g=Guest::new(name);g.m.bus.set_flash_size(flash<<20);g.m.bus.set_psram_size(psram<<20).unwrap();
        assert_eq!(esp32::board::waveshare_devkit::memory_mib(name),Some((flash,psram)));assert!(g.m.bus.board.display().is_none());
        if psram==8 {for(off,value)in[(0x34,5),(0x1c,0xf0000000),(0x20,0x5c000001),(0x24,0x7000009f),(4,0),(0x2c,0x3f),(0,1<<18)] {g.put(0x3ff42000+off,value);}assert_eq!(g.get(0x3ff42080),0x425d0d,"guest sees8MiB virtual PSRAM RDID; not physical vendor identification");}
        g.rmt(23,&[1,2,3]);assert_eq!(g.m.bus.board.leds().unwrap().1,0);g.put(GPIO+0x530+4*23,256);
        g.rmt(27,&[0x12,0x34,0x56]);assert_eq!(g.m.bus.board.leds().unwrap().0,[[0x34,0x12,0x56]]);
        g.mux(0,2);g.m.bus.gpio_set_input(0,false);assert_eq!(g.get(GPIO+0x3c)&1,0);g.m.bus.gpio_set_input(0,true);assert_ne!(g.get(GPIO+0x3c)&1,0);
    }
    assert!(esp32::board::waveshare_devkit::make_board("waveshare-esp32-devkit-wrover-e-n4r2").is_none());
}

#[test]
fn robot_qmi8658c_samples_guest_clock_and_rejects_wrong_pins_and_unfitted_oled() {
    let mut g=Guest::new("waveshare-general-driver-robots");assert!(g.m.bus.board.display().is_none());g.i2c_route(32,33);
    assert_eq!(g.read(0x6b,0,1),[5]);assert_ne!(g.transaction(0x6a,&[0],0).0&INT_NACK,0,"SA0 ground is6B; source variable_L is not the physical strap");
    assert_ne!(g.transaction(0x3c,&[0xaf],0).0&INT_NACK,0,"optional external OLED is absent");g.m.bus.tick(800_000);
    assert!(g.m.bus.board.set_imu_sample([0.5,-1.,2.],[0.;3]));
    for(r,v)in[(2,0x40),(3,0x13),(8,1)] {assert_eq!(g.transaction(0x6b,&[r,v],0).0&INT_NACK,0);}
    g.m.bus.tick(80_000);assert_eq!(g.read(0x6b,0x35,6),[0,16,0,224,0,64],"4g scale:4096,-8192,16384");
    g.transaction(0x6b,&[2,0x60],0);assert_eq!(g.read(0x6b,0x35,6),[16,0,224,0,64,0],"CTRL1bit5 explicitly selects big-endian");
    g.i2c_route(21,22);assert_ne!(g.transaction(0x6b,&[0],0).0&INT_NACK,0);g.i2c_route(32,33);assert_eq!(g.read(0x6b,0,1),[5]);
}
#[test]
fn robot_spi_card_real_guest_startup_and_motor_gpio_observation() {
    let mut g=Guest::new("waveshare-general-driver-robots");g.m.bus.board.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
    for(pin,signal)in[(14,8),(13,10)]{g.mux(pin,2);g.put(GPIO+0x530+4*u32::from(pin),signal);}
    g.mux(12,2);g.put(GPIO+0x130+4*9,0x80|12);g.output(15,true);g.spi(&[255;10]);g.level(15,false);
    assert_eq!(&g.spi(&[0x40,0,0,0,0,0x95,255,255])[6..8],&[255,1]);
    assert_eq!(&g.spi(&[0x48,0,0,1,0xaa,0x87,255,255,255,255,255,255])[6..12],&[255,1,0,0,1,0xaa]);
    g.put(GPIO+0x130+4*9,0x80|19);assert!(g.spi(&[0x48,0,0,1,0xaa,0x87,255,255,255,255,255,255]).iter().all(|&b|b==255));
    for pin in[25,17,21,22,23,26] {g.output(pin,true);}
    assert!(g.m.bus.board.hardware_diagnostics().unwrap().contains("[true,true,true,true,true,true]"));g.mux(17,3);
    assert!(g.m.bus.board.hardware_diagnostics().unwrap().contains("[true,null,true,true,true,true]"));
    assert!(g.m.bus.board.hardware_diagnostics().unwrap().contains("\"mechanicalMotorDynamics\":false"));
}
#[test]
fn epaper_driver_has_no_panel_and_only_observes_exact_connector_output() {
    let mut g=Guest::new("waveshare-epaper-esp32-driver-v3");assert!(g.m.bus.board.display().is_none());
    for(pin,signal)in[(13,8),(14,10)] {g.mux(pin,2);g.put(GPIO+0x530+4*u32::from(pin),signal);}
    for(pin,level)in[(15,false),(27,false),(26,true)] {g.output(pin,level);}
    assert_eq!(g.spi(&[0x12]),[0xff]);assert!(g.m.bus.board.hardware_diagnostics().unwrap().contains("\"spiOutputCommandBytes\":1"));
    g.level(27,true);g.spi(&[1,2,3]);assert!(g.m.bus.board.hardware_diagnostics().unwrap().contains("\"spiOutputDataBytes\":3"));
    let before=g.m.bus.board.hardware_diagnostics();g.mux(14,3);g.spi(&[4,5]);assert_eq!(g.m.bus.board.hardware_diagnostics(),before);g.mux(14,2);
    g.level(26,false);g.spi(&[4,5]);assert_eq!(g.m.bus.board.hardware_diagnostics(),before);g.level(26,true);g.level(15,true);g.spi(&[4,5]);assert_eq!(g.m.bus.board.hardware_diagnostics(),before);
    assert!(g.m.bus.board.input_levels().iter().all(|&(pin,_)|pin!=25),"no fabricated BUSY from an unconnected panel");
}
