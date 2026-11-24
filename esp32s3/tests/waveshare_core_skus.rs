//! Exact core-family SKU diagnostics through real LX7 S32I/L32I instructions.
//! GPIO/RMT/LEDC and mapped memory checks do not certify original factory firmware.
use emu_core::{Bus,Core};
use esp_soc::{SocBus,Stop};
use esp32s3::board::{self,waveshare_core::Variant};
const PC:u32=0x4037_0000;const GPIO:u32=0x6000_4000;
struct Guest{m:esp32s3::Machine}
impl Guest {
    fn new(v:Variant)->Self {let mut m=esp32s3::machine([0;6]);m.bus.board=board::make_board(v.name()).unwrap();m.bus.attach_board_devices();let(f,p)=v.memory_mib();m.bus.set_flash_size(f<<20);m.bus.set_psram_size(p<<20).unwrap();m.bus.set_flash_octal(v.flash_octal());let mut g=Self{m};g.program();g}
    fn program(&mut self){self.m.bus.load_bytes(PC,&[0x32,0x64,0,0x52,0x24,0]).unwrap();self.m.cores[0].ps=0;self.m.cores[0].set_jit(false);}
    fn put(&mut self,a:u32,v:u32){self.m.cores[0].pc=PC;self.m.cores[0].set_ar(4,a);self.m.cores[0].set_ar(3,v);assert!(matches!(self.m.run(1),Stop::MaxInsns));}
    fn get(&mut self,a:u32)->u32{self.m.cores[0].pc=PC+3;self.m.cores[0].set_ar(4,a);assert!(matches!(self.m.run(1),Stop::MaxInsns));self.m.cores[0].get_ar(5)}
    fn route(&mut self,p:u8,s:u32){self.put(0x6000_9004+4*u32::from(p),1<<12|1<<9);self.put(GPIO+0x554+4*u32::from(p),s);if s==256{self.put(GPIO+if p<32{0x24}else{0x30},1<<(p%32));}}
    fn high(&mut self,p:u8,v:bool){self.put(GPIO+if p<32{if v{8}else{12}}else if v{20}else{24},1<<(p%32));}
    fn rmt(&mut self,bytes:&[u8]){let bits:Vec<bool>=bytes.iter().flat_map(|b|(0..8).map(move|i|b&(0x80>>i)!=0)).collect();for(i,&b)in bits.iter().enumerate(){self.put(0x6001_6800+4*i as u32,0x8000|if b{64|32<<16}else{32|64<<16});}self.put(0x6001_6800+4*bits.len()as u32,0);self.put(0x6001_6020,4<<16|1<<8|1);self.m.bus.tick(200_000);}
}
#[test]
fn sixteen_exact_skus_keep_physical_memory_boot_and_absent_interfaces(){
    for v in Variant::ALL{let mut g=Guest::new(v);let(f,p)=v.memory_mib();assert_eq!(g.m.bus.flash.len(),f<<20);assert_eq!(g.m.bus.psram.len(),p<<20);assert!(g.m.bus.board.display().is_none());assert!(g.m.bus.board.camera_frame().is_none());assert!(g.m.bus.board.sd_card_slot().is_none());assert!(g.m.bus.board.i2c_devices().is_empty());assert_eq!(g.m.bus.board.named_pin("boot"),v.boot());
        if v.nano(){assert!(g.m.bus.board.input_levels().is_empty());assert_eq!(g.m.bus.board.leds().unwrap().0.len(),2);}else{g.route(0,256);g.put(GPIO+0x28,1);assert_eq!(g.get(GPIO+0x3c)&1,1);g.m.bus.gpio_set_input(0,false);assert_eq!(g.get(GPIO+0x3c)&1,0);g.m.bus.gpio_set_input(0,true);assert_eq!(g.get(GPIO+0x3c)&1,1);}
        assert_eq!(g.get(0x6000_703c)&(1<<9),u32::from(v.flash_octal())<<9);
    }
}
#[test]
fn zero_grb_and_pico_devkit_rgb_use_actual_rmt_routes_and_reject_wrong_or_inverted_pins(){
    for v in Variant::ALL.into_iter().filter(|v|!v.nano()){let mut g=Guest::new(v);let zero=v.name().contains("zero");let pin=if v.name().contains("devkit"){38}else{21};let wrong=if pin==21{38}else{21};g.route(wrong,81);g.rmt(&[0x12,0x34,0x56]);assert_eq!(g.m.bus.board.leds().unwrap().1,0);g.route(wrong,256);g.route(pin,81);g.rmt(&[0x12,0x34,0x56]);assert_eq!(g.m.bus.board.leds().unwrap().0,[if zero{[0x34,0x12,0x56]}else{[0x12,0x34,0x56]}]);let version=g.m.bus.board.leds().unwrap().1;g.put(GPIO+0x554+u32::from(pin)*4,81|1<<9);g.rmt(&[255;3]);assert_eq!(g.m.bus.board.leds().unwrap().1,version);g.m.reboot();assert_eq!(g.m.bus.board.leds().unwrap().0,[if zero{[0x34,0x12,0x56]}else{[0x12,0x34,0x56]}],"A powered addressable LED retains its latched light across MCU reset");g.program();g.route(pin,81);g.rmt(&[0x01,0x02,0x03]);assert_eq!(g.m.bus.board.leds().unwrap().0,[if zero{[2,1,3]}else{[1,2,3]}]);assert_eq!(Guest::new(v).m.bus.board.leds().unwrap().0,[[0;3]],"Fresh platform device starts with fresh LED state");}
}
#[test]
fn nano_header_option_retains_discrete_active_low_rgb_and_separate_gpio48_indicator(){
    for v in[Variant::Nano,Variant::NanoM]{let mut g=Guest::new(v);for pin in[46,0,45,48]{g.route(pin,256);g.high(pin,pin==48);}g.m.bus.refresh_board_outputs();assert_eq!(g.m.bus.board.leds().unwrap().0,[[255;3],[255,255,0]]);g.high(46,true);g.high(0,true);g.high(45,true);g.m.bus.refresh_board_outputs();assert_eq!(g.m.bus.board.leds().unwrap().0[0],[0;3]);for(a,x)in[(0x600c0018,1<<11),(0x600190d0,3),(0x600190a0,8|(256<<4)|(1<<25)),(0x60019008,64<<4),(0x6001900c,1<<31),(0x60019000,4|(1<<4))]{g.put(a,x);}g.m.bus.tick(10000);g.route(46,73);g.m.bus.refresh_board_outputs();assert_eq!(g.m.bus.board.leds().unwrap().0[0],[191,0,0]);g.put(GPIO+0x554+4*46,73|1<<9);g.m.bus.refresh_board_outputs();assert_eq!(g.m.bus.board.leds().unwrap().0[0],[64,0,0]);g.put(GPIO+0x34,1<<(46-32));g.m.bus.refresh_board_outputs();assert_eq!(g.m.bus.board.leds().unwrap().0[0],[0;3]);}
}
#[test]
fn n32r16v_last_flash_and_psram_pages_do_not_alias_and_opi_survives_chip_reboot(){
    use esp32s3::bus::{MMU_TABLE,MMU_SPIRAM};
    for v in[Variant::DevKitN32R16V,Variant::DevKitN32R16VM]{let mut g=Guest::new(v);g.m.bus.flash[0..4].copy_from_slice(&0x12345678u32.to_le_bytes());g.m.bus.flash[0x1ff0000..0x1ff0004].copy_from_slice(&0xaabbccddu32.to_le_bytes());g.put(MMU_TABLE,0);g.put(MMU_TABLE+4,511);assert_eq!(g.get(0x3c000000),0x12345678);assert_eq!(g.get(0x3c010000),0xaabbccdd);g.put(MMU_TABLE+8,MMU_SPIRAM);g.put(MMU_TABLE+12,MMU_SPIRAM|255);g.put(0x3c020000,0x76543210);g.put(0x3c030000,0x0f1e2d3c);assert_eq!(g.get(0x3c020000),0x76543210);assert_eq!(g.get(0x3c030000),0x0f1e2d3c);assert_eq!(g.m.bus.periph.spi1.jedec,[0xc2,0x80,0x39]);g.m.bus.periph.spi1.flash_cr2=2;g.m.reboot();g.program();assert_eq!(g.get(0x6000703c)&(1<<9),1<<9);assert!(g.m.bus.periph.spi0.octal_flash&&g.m.bus.periph.spi1.octal_flash);assert_eq!(g.m.bus.periph.spi1.flash_cr2,2);assert_eq!(g.m.bus.periph.spi1.psram_mr[2],0x8d);assert_eq!((g.m.bus.flash.len(),g.m.bus.psram.len()),(32<<20,16<<20));g.put(MMU_TABLE+4,511);assert_eq!(g.get(0x3c010000),0xaabbccdd);}
}
