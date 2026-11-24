//! Original LX7 instructions exercise exact tail-board physical GPIO/I2C/RMT/SPI.
//! These are original diagnostics, not manufacturer firmware boot acceptance.
use emu_core::{Bus,Core};
use esp_soc::{SocBus,Stop};
use esp32s3::board;
const PC:u32=0x4037_0000;const GPIO:u32=0x6000_4000;const SPI:u32=0x6002_4000;
struct Guest {m:esp32s3::Machine}
impl Guest {
    fn new(name:&str)->Self {let mut m=esp32s3::machine([0;6]);m.bus.board=board::make_board(name).unwrap();m.bus.attach_board_devices();let(f,p)=if name=="waveshare-s3-matrix8"{(4,2)}else{(16,8)};m.bus.set_flash_size(f<<20);m.bus.set_psram_size(p<<20).unwrap();m.bus.load_bytes(PC,&[0x32,0x64,0,0x52,0x24,0]).unwrap();m.cores[0].ps=0;m.cores[0].set_jit(false);Self{m}}
    fn put(&mut self,address:u32,value:u32){self.m.cores[0].pc=PC;self.m.cores[0].set_ar(4,address);self.m.cores[0].set_ar(3,value);assert!(matches!(self.m.run(1),Stop::MaxInsns));}
    fn get(&mut self,address:u32)->u32 {self.m.cores[0].pc=PC+3;self.m.cores[0].set_ar(4,address);assert!(matches!(self.m.run(1),Stop::MaxInsns));self.m.cores[0].get_ar(5)}
    fn route(&mut self,pin:u8,signal:u32){self.put(0x6000_9004+4*u32::from(pin),1<<12|1<<9);self.put(GPIO+0x554+4*u32::from(pin),signal);if signal==256{self.put(GPIO+if pin<32{0x24}else{0x30},1<<(pin%32));}}
    fn high(&mut self,pin:u8,value:bool){self.put(GPIO+if pin<32{if value{8}else{12}}else if value{20}else{24},1<<(pin%32));}
    fn i2c_route(&mut self,b:u32,sda:u8,scl:u8){for(p,s)in[(sda,90+b*2),(scl,89+b*2)]{self.route(p,s);self.put(GPIO+0x154+4*s,0x80|u32::from(p));}}
    fn i2c(&mut self,b:u32,address:u8,data:&[u8],n:u8)->Option<Vec<u8>>{
        let base=[0x6001_3000,0x6002_7000][b as usize];self.put(base+0x24,u32::MAX);self.put(base+0x18,3<<12);
        let mut commands=vec![];if !data.is_empty()||n==0{for v in[address<<1].into_iter().chain(data.iter().copied()){self.put(base+0x1c,u32::from(v));}commands.extend([6<<11,1<<11|1<<8|(data.len()+1)as u32]);}
 if n>0{self.put(base+0x1c,u32::from(address<<1|1));commands.extend([6<<11,1<<11|1<<8|1,3<<11|u32::from(n)]);}commands.push(2<<11);
        for(i,c)in commands.into_iter().enumerate(){self.put(base+0x58+4*i as u32,c);}self.put(base+4,1<<5);self.m.bus.tick(100_000);
        if self.get(base+0x20)&(1<<10)!=0{return None;}assert_ne!(self.get(base+0x20)&(1<<7),0);Some((0..n).map(|_|self.get(base+0x1c)as u8).collect())
    }
    fn rmt(&mut self){for i in 0..24{self.put(0x6001_6800+4*i,0x8000|32|64<<16);}self.put(0x6001_6860,0);self.put(0x6001_6020,1<<16|1<<8|1);self.m.bus.tick(100_000);}
    fn spi(&mut self,bytes:&[u8]){self.put(SPI+0x10,1<<27);self.put(SPI+0x1c,(bytes.len()*8-1)as u32);for(i,c)in bytes.chunks(4).enumerate(){self.put(SPI+0x98+4*i as u32,c.iter().enumerate().fold(0,|v,(j,b)|v|u32::from(*b)<<(8*j)));}self.put(SPI,1<<24);self.m.bus.flush_ticks();}
}

#[test]fn actual_lx7_spi_panel_routes_gate_final_visible_pixels(){
 for(name,clk,mosi,cs,dc,rst,bl,gap)in[("waveshare-s3-lcd28",40,45,42,41,39,5,0),("waveshare-s3-touch-lcd28-v1",40,45,42,41,39,5,0),("waveshare-s3-touch-lcd28-v2",40,45,42,41,39,5,0),("waveshare-s3-touch-lcd169",6,7,5,4,8,15,20)]{
  let mut g=Guest::new(name);for(p,s)in[(clk,101),(mosi,103),(cs,110),(dc,256),(rst,256),(bl,256)]{g.route(p,s);}g.put(SPI+0x20,0x3e);g.high(rst,true);g.high(bl,true);
  for(c,data)in[(0x11,vec![]),(0x29,vec![]),(0x3a,vec![0x55]),(0x36,vec![0]),(0x2a,vec![0,0,0,0]),(0x2b,vec![0,gap,0,gap]),(0x2c,vec![0xf8,0])]{g.high(dc,false);g.spi(&[c]);if !data.is_empty(){g.high(dc,true);g.spi(&data);}}
  assert_eq!(g.m.bus.board.display().unwrap().2[0],0xf800,"{name}");g.high(bl,false);g.m.bus.refresh_board_outputs();assert_eq!(g.m.bus.board.display().unwrap().2[0],0);g.high(bl,true);g.route(clk,102);g.spi(&[0,0]);assert_eq!(g.m.bus.board.display().unwrap().2[0],0xf800);g.high(rst,false);g.m.bus.refresh_board_outputs();assert_eq!(g.m.bus.board.display().unwrap().2[0],0);
 }
}
#[test]fn actual_lx7_ch422_commands_and_touch_readback_obey_pads_and_reset(){
 let mut g=Guest::new("waveshare-s3-touch-lcd7");g.i2c_route(0,8,9);g.route(4,256);g.high(4,false);assert_eq!(g.i2c(0,0x24,&[1],0),Some(vec![]));assert!(g.i2c(0,0x38,&[0xfd],0).is_some());g.m.bus.refresh_board_outputs();assert!(g.i2c(0,0x5d,&[0x81,0x4e],1).is_none());g.i2c(0,0x38,&[255],0).unwrap();g.m.bus.refresh_board_outputs();g.i2c(0,0x5d,&[0x81,0x4e,0],0).unwrap();g.m.bus.board.touch(123,234,true);g.m.bus.tick(2_500_000);g.m.bus.board.advance_to(g.m.bus.cycles);assert_eq!(g.i2c(0,0x5d,&[0x81,0x4e],6),Some(vec![0x81,0,123,0,234,0]));assert!(g.i2c(0,0x24,&[2,1],0).is_none());g.i2c_route(0,9,8);assert!(g.i2c(0,0x38,&[255],0).is_none());g.i2c_route(1,8,9);assert_eq!(g.i2c(1,0x26,&[],1),Some(vec![255]));
}
#[test]fn actual_lx7_rmt_decodes_vendor_rgb_bytes_only_on_matrix_gpio14(){
 let mut g=Guest::new("waveshare-s3-matrix8");g.route(13,81);g.rmt();assert!(g.m.bus.board.leds().unwrap().0.iter().all(|&p|p==[0,0,0]));g.route(14,81);g.rmt();assert_eq!(g.m.bus.board.leds().unwrap().0[0],[0,0,0]);
 // One logical1 followed by23 zeros: transmitted bytes80 00 00 => RGB128,0,0.
 g.put(0x6001_6800,0x8000|64|32<<16);for i in 1..24{g.put(0x6001_6800+4*i,0x8000|32|64<<16);}g.put(0x6001_6860,0);g.put(0x6001_6020,1<<16|1<<8|1);g.m.bus.tick(100_000);g.m.bus.board.advance_to(g.m.bus.cycles);assert_eq!(g.m.bus.board.leds().unwrap().0[0],[128,0,0]);assert_eq!(g.m.bus.board.leds().unwrap().0.len(),64);
}
