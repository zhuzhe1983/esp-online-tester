//! Real LX7 load/store guests touch actual S3 GPIO/I2C/RMT/SPI MMIO.
//! These are original diagnostics, not manufacturer firmware boot acceptance.
use emu_core::{Bus,Core};
use esp_soc::{SocBus,Stop};
use esp32s3::board::{self,waveshare_industrial::{Profile,memory_mib}};
const PC:u32=0x4037_0000;const GPIO:u32=0x6000_4000;const SPI:u32=0x6002_4000;
struct Guest {m:esp32s3::Machine}
impl Guest {
    fn new(name:&str)->Self {let mut m=esp32s3::machine([0;6]);m.bus.board=board::make_board(name).unwrap();m.bus.attach_board_devices();let(f,p)=memory_mib(name).or_else(||board::waveshare_rgb_matrix::memory_mib(name)).unwrap();m.bus.set_flash_size(f<<20);m.bus.set_psram_size(p<<20).unwrap();m.bus.load_bytes(PC,&[0x32,0x64,0,0x52,0x24,0]).unwrap();m.cores[0].ps=0;m.cores[0].set_jit(false);Self{m}}
    fn put(&mut self,address:u32,value:u32){self.m.cores[0].pc=PC;self.m.cores[0].set_ar(4,address);self.m.cores[0].set_ar(3,value);assert!(matches!(self.m.run(1),Stop::MaxInsns));}
    fn get(&mut self,address:u32)->u32 {self.m.cores[0].pc=PC+3;self.m.cores[0].set_ar(4,address);assert!(matches!(self.m.run(1),Stop::MaxInsns));self.m.cores[0].get_ar(5)}
    fn route(&mut self,pin:u8,signal:u32){self.put(0x6000_9004+4*u32::from(pin),1<<12|1<<9);self.put(GPIO+0x554+4*u32::from(pin),signal);if signal==256{self.put(GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));}}
    fn high(&mut self,pin:u8,value:bool){self.put(GPIO+if pin<32{if value{8}else{12}}else if value{20}else{24},1<<(pin%32));}
    fn i2c_route(&mut self,b:u32,sda:u8,scl:u8){for(p,s)in[(sda,90+b*2),(scl,89+b*2)]{self.route(p,s);self.put(GPIO+0x154+4*s,0x80|u32::from(p));}}
    fn i2c(&mut self,b:u32,address:u8,reg:u8,data:&[u8],n:u8)->Option<Vec<u8>>{
        let base=[0x6001_3000,0x6002_7000][b as usize];self.put(base+0x24,u32::MAX);self.put(base+0x18,3<<12);
        for v in[address<<1,reg].into_iter().chain(data.iter().copied()){self.put(base+0x1c,u32::from(v));}
        let mut commands=vec![6<<11,1<<11|1<<8|(data.len()+2)as u32];if n>0{self.put(base+0x1c,u32::from(address<<1|1));commands.extend([6<<11,1<<11|1<<8|1,3<<11|u32::from(n)]);}commands.push(2<<11);
        for(i,c)in commands.into_iter().enumerate(){self.put(base+0x58+4*i as u32,c);}self.put(base+4,1<<5);self.m.bus.tick(100_000);
        if self.get(base+0x20)&(1<<10)!=0{return None;}assert_ne!(self.get(base+0x20)&(1<<7),0);Some((0..n).map(|_|self.get(base+0x1c)as u8).collect())
    }
    fn rmt(&mut self){for i in 0..24{self.put(0x6001_6800+4*i,0x8000|32|64<<16);}self.put(0x6001_6860,0);self.put(0x6001_6020,1<<16|1<<8|1);self.m.bus.tick(100_000);}
    fn spi(&mut self,bytes:&[u8]){self.put(SPI+0x10,1<<27);self.put(SPI+0x1c,(bytes.len()*8-1)as u32);for(i,c)in bytes.chunks(4).enumerate(){self.put(SPI+0x98+4*i as u32,c.iter().enumerate().fold(0,|v,(j,b)|v|u32::from(*b)<<(8*j)));}self.put(SPI,1<<24);self.m.bus.flush_ticks();}
    fn diag(&self)->String {self.m.bus.board.hardware_diagnostics().unwrap()}
}
#[test]
fn all_exact_profiles_have_their_own_identity_memory_and_physical_buttons_without_screens(){
    for p in Profile::ALL {let mut g=Guest::new(p.name());let(f,ps)=p.memory_mib();assert_eq!(g.m.bus.flash.len(),f<<20);assert_eq!(g.m.bus.psram.len(),ps<<20);assert!(g.m.bus.board.display().is_none());assert!(g.m.bus.board.leds().is_none());assert_eq!(g.m.bus.board.named_pin("boot"),p.boot_button().then_some(0));assert_eq!(g.m.bus.board.named_pin("power"),p.camera().then_some(15));
        g.route(0,256);g.put(GPIO+0x28,1);assert_eq!(g.get(GPIO+0x3c)&1,1);g.m.bus.gpio_set_input(0,false);assert_eq!(g.get(GPIO+0x3c)&1,0);g.m.bus.gpio_set_input(0,true);assert_eq!(g.get(GPIO+0x3c)&1,1);
        assert!(g.m.bus.board.set_camera_picture(esp_soc::picture::Picture{w:1,h:1,rgb:vec![0;3]}).is_err());assert!(g.m.bus.board.camera_frame().is_none());
    }
    assert!(board::make_board("waveshare-s3-lr1121-unknown").is_none());
}
#[test]
fn rs485_wled_guest_codec_registers_and_external_connector_observation_use_exact_routes(){
    let mut g=Guest::new(Profile::Rs485Wled.name());g.i2c_route(0,15,14);assert_eq!(g.i2c(0,0x40,0x3d,&[],2),Some(vec![0x72,0x72]),"ES7210 single-register reads do not auto-increment");
    assert_eq!(g.i2c(0,0x40,0x3e,&[],1),Some(vec![0x10]));assert_eq!(g.i2c(0,0x18,0xfd,&[],1),None,"ES8389 cannot inherit an ES8311 identity");
    g.i2c(0,0x40,0x43,&[0x18],0).unwrap();assert_eq!(g.i2c(0,0x40,0x43,&[],1),Some(vec![0x18]));g.i2c(0,0x40,0,&[0xff],0).unwrap();assert_eq!(g.i2c(0,0x40,0x43,&[],1),Some(vec![0]));
    g.i2c_route(0,14,15);assert_eq!(g.i2c(0,0x40,0x3d,&[],1),None);g.i2c_route(1,15,14);assert_eq!(g.i2c(1,0x40,0x3d,&[],1),Some(vec![0x72]));
    g.route(3,81);g.rmt();assert!(g.diag().contains("\"externalLedRmtFrames\":[0, 0]"));g.route(2,81);g.rmt();assert!(g.diag().contains("\"externalLedRmtFrames\":[1, 0]"));g.route(2,256);g.route(16,81);g.rmt();assert!(g.diag().contains("\"externalLedRmtFrames\":[1, 1]"));assert!(g.diag().contains("\"externalLedRmtBits\":[24, 24]"));assert!(g.m.bus.board.leds().is_none(),"No attached LED strip or onboard addressable LED was invented");
}
#[test]
fn can2_both_antenna_skus_use_guest_clock_bcd_rtc_and_reject_wrong_pins_addresses(){
    for p in[Profile::Can2,Profile::Can2U]{let mut g=Guest::new(p.name());g.i2c_route(0,39,38);assert_eq!(g.i2c(0,0x51,4,&[],1),Some(vec![0x80]));g.i2c(0,0x51,4,&[0x59,0x59,0x23,0x31,6,0x12,0x24],0).unwrap();
        g.m.bus.tick(240_000_000);g.m.bus.board.advance_to(g.m.bus.cycles);assert_eq!(g.i2c(0,0x51,4,&[],7),Some(vec![0,0,0,1,0,1,0x25]));
        g.i2c(0,0x51,0,&[0x20],0).unwrap();g.m.bus.tick(480_000_000);g.m.bus.board.advance_to(g.m.bus.cycles);assert_eq!(g.i2c(0,0x51,4,&[],1),Some(vec![0]));g.i2c_route(0,38,39);assert_eq!(g.i2c(0,0x51,4,&[],1),None);g.i2c_route(1,39,38);assert_eq!(g.i2c(1,0x51,4,&[],1),Some(vec![0]));assert_eq!(g.i2c(1,0x50,4,&[],1),None);
    }
}
#[test]
fn four_camera_options_use_exact_shared_audio_bus_without_invented_sensor_ids(){
    for p in[Profile::CamOv5640,Profile::CamOv3660,Profile::CamGc2145,Profile::CamGc0308]{let mut g=Guest::new(p.name());g.i2c_route(0,8,7);assert_eq!(g.i2c(0,0x18,0xfd,&[],1),Some(vec![0x83]));assert_eq!(g.i2c(0,0x18,0xfe,&[],1),Some(vec![0x11]));assert_eq!(g.i2c(0,0x40,0x3d,&[],1),Some(vec![0x72]));
        for address in[0x24,0x3c,0x21]{assert_eq!(g.i2c(0,address,0,&[],1),None,"Unsupported expander/sensor must not masquerade as a generic register device");}g.i2c_route(0,7,8);assert_eq!(g.i2c(0,0x18,0xfd,&[],1),None);g.i2c_route(1,8,7);assert_eq!(g.i2c(1,0x18,0xfd,&[],1),Some(vec![0x83]));
        g.route(15,256);g.put(GPIO+0x28,1<<15);assert_ne!(g.get(GPIO+0x3c)&(1<<15),0);g.m.bus.gpio_set_input(15,false);assert_eq!(g.get(GPIO+0x3c)&(1<<15),0);assert!(g.m.bus.board.sd_card_slot().is_none(),"Physical slot is cataloged but unimplemented CH32 mode/power gating is not bypassed");
    }
}
#[test]
fn lr1121_hf_lf_bare_kit_guest_spi_observes_exact_pins_reset_and_no_fabricated_reply(){
    for p in[Profile::Lr1121Hf,Profile::Lr1121HfKit,Profile::Lr1121Lf,Profile::Lr1121LfKit]{let mut g=Guest::new(p.name());for(pin,signal)in[(40,101),(45,103),(42,110),(39,256)]{g.route(pin,signal);}g.put(SPI+0x20,0x3e);g.high(39,true);g.spi(&[1,2,3,4]);assert!(g.diag().contains("\"radioSpiObservedBytes\":4"));
        g.route(40,102);g.spi(&[5,6]);assert!(g.diag().contains("\"radioSpiObservedBytes\":4"));g.route(40,101);g.high(39,false);g.spi(&[7,8]);assert!(g.diag().contains("\"radioSpiObservedBytes\":4"));g.high(39,true);g.put(SPI+0x20,0x3f);g.spi(&[9,10]);assert!(g.diag().contains("\"radioSpiObservedBytes\":4"));assert!(g.m.bus.board.lora_snapshot().is_none());assert!(g.m.bus.board.sd_card_slot().is_none());
    }
}
#[test]
fn bare_rgb_matrix_preserves_32f16p_and_real_rtc_but_has_no_panel(){
    let mut g=Guest::new("waveshare-s3-rgb-matrix");assert_eq!(g.m.bus.flash.len(),32<<20);assert_eq!(g.m.bus.psram.len(),16<<20);assert!(g.m.bus.board.display().is_none());g.i2c_route(0,47,48);assert_eq!(g.i2c(0,0x51,4,&[],1),Some(vec![0x80]));assert_eq!(g.i2c(0,0x18,0xfd,&[],1),Some(vec![0x83]));g.i2c_route(0,48,47);assert_eq!(g.i2c(0,0x51,4,&[],1),None);assert_eq!(g.m.bus.board.sd_card_slot(),Some(([1,44,17,255,255,255],false)));assert!(g.m.bus.board.display().is_none());
}
