//! Original LX7 diagnostics: actual GPIO matrix, I2C/9-bit control, LCD_CAM and GDMA.
//! These diagnostics are not substituted manufacturer firmware.
use emu_core::{Bus,Core};
use esp_soc::{SocBus,Stop};
use esp32s3::board;
const PC:u32=0x4037_0000;const GPIO:u32=0x6000_4000;
struct Guest {m:esp32s3::Machine}
impl Guest {
    fn new(name:&str)->Self {let mut m=esp32s3::machine([0;6]);m.bus.board=board::make_board(name).unwrap();m.bus.attach_board_devices();let(f,p)=if name=="waveshare-s3-matrix8"{(4,2)}else if name=="waveshare-s3-geek"{(16,2)}else{(16,8)};m.bus.set_flash_size(f<<20);m.bus.set_psram_size(p<<20).unwrap();m.bus.load_bytes(PC,&[0x32,0x64,0,0x52,0x24,0]).unwrap();m.cores[0].ps=0;m.cores[0].set_jit(false);Self{m}}
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
}

fn rgb_dma(g:&mut Guest,w:u32,h:u32){const DESC:u32=0x3fc88000;const DATA:u32=0x3fc8a000;let total=w*h*2;let mut data=vec![0;4092];for(i,p)in[(0,0xf800u16),((w*h/2)as usize,0x07e0),((w*h-1)as usize,0x001f)]{let at=(2*i)%4092;data[at..at+2].copy_from_slice(&p.to_le_bytes());}g.m.bus.load_bytes(DATA,&data).unwrap();let(mut pos,mut index)=(0,0);while pos<total{let len=(total-pos).min(4092);let last=pos+len==total;for(i,v)in[1<<31|u32::from(last)<<30|len<<12|len,DATA,if last{0}else{DESC+12*(index+1)}].into_iter().enumerate(){g.put(DESC+12*index+4*i as u32,v);}pos+=len;index+=1;}for(a,v)in[(0x60041014,0),(0x600c001c,1<<8),(0x6003f0a8,5),(0x6003f080,(DESC&0xfffff)|(1<<21)),(0x60041000,(1<<31)|(3<<29)|(20<<9)|(1<<6)),(0x60041020,((w-1)<<8)|((w+21)<<20)),(0x6004101c,(1<<31)|((h-1)<<11)|((h+38)<<21)),(0x60041014,(1<<27)|(1<<24)|(1<<23))]{g.put(a,v);}for _ in 0..240{g.m.bus.tick(100_000);}g.m.bus.refresh_board_outputs();}

fn route_rgb(g:&mut Guest,controls:[u8;4],pads:[u8;16]){for(p,s)in controls.into_iter().zip([154,151,152,150]){g.route(p,s);}for(i,p)in pads.into_iter().enumerate(){g.route(p,133+i as u32);}}
const STANDARD:[u8;16]=[14,38,18,17,10,39,0,45,48,47,21,1,2,42,41,40];
#[test]fn actual_lx7_rgb_matrix_delivery_for_byte_and_word_expander_boards(){
 for(name,w,h,sda,scl,wide,pads)in[
  ("waveshare-s3-touch-lcd7b",1024,600,8,9,false,STANDARD),
  ("waveshare-s3-lcd7b",1024,600,8,9,false,STANDARD),
  ("waveshare-s3-touch-lcd7c",800,480,47,48,true,[14,38,18,17,10,39,0,45,9,8,21,1,2,42,41,40]),
  ("waveshare-s3-touch-lcd43c",800,480,8,9,true,STANDARD)]{
  let mut g=Guest::new(name);g.i2c_route(0,sda,scl);g.route(4,256);g.high(4,false);
  for register in[2,3]{let mut bytes=vec![register,255];if wide{bytes.push(255);}g.i2c(0,0x24,&bytes,0).unwrap();}
  route_rgb(&mut g,[7,46,3,5],pads);rgb_dma(&mut g,w,h);
  let f=g.m.bus.board.display().unwrap();assert_eq!([f.2[0],f.2[(w*h/2)as usize],f.2[(w*h-1)as usize]],[0xf800,0x07e0,0x001f],"{name}: {}",g.m.bus.board.report());
  let count=g.m.bus.board.display_frames();g.route(pads[8],132);rgb_dma(&mut g,w,h);assert_eq!(g.m.bus.board.display_frames(),count,"wrong data signal must not deliver a frame");
  route_rgb(&mut g,[7,46,3,5],pads);g.route(7,153);rgb_dma(&mut g,w,h);assert_eq!(g.m.bus.board.display_frames(),count,"wrong pixel clock signal");
  route_rgb(&mut g,[7,46,3,5],pads);let bytes=if wide{vec![3,0xfb,255]}else{vec![3,0xfb]};g.i2c(0,0x24,&bytes,0).unwrap();rgb_dma(&mut g,w,h);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0),"independent backlight gate");
 }
}
fn serial_word(g:&mut Guest,expander:bool,dc:bool,value:u8){let word=(u16::from(dc)<<8)|u16::from(value);if expander{g.i2c(0,0x20,&[1,0xf9],0).unwrap();g.i2c(0,0x20,&[1,0xf8],0).unwrap();for bit in(0..9).rev(){let base=0xf8|if(word>>bit)&1!=0{2}else{0};g.i2c(0,0x20,&[1,base],0).unwrap();g.i2c(0,0x20,&[1,base|4],0).unwrap();}g.i2c(0,0x20,&[1,0xf9],0).unwrap();}else{g.high(0,true);g.high(2,false);g.high(0,false);for bit in(0..9).rev(){g.high(2,false);g.high(1,(word>>bit)&1!=0);g.high(2,true);}g.high(0,true);}}
#[test]fn actual_lx7_st7701_serial_init_then_rgb_dma_for_two_distinct_wiring_sets(){
 for(name,expander,w,h,controls,pads)in[
  ("waveshare-s3-touch-lcd4b",true,480,480,[9,46,3,17],[40,41,42,2,1,21,8,18,45,38,39,10,11,12,13,14]),
  ("waveshare-s3-lcd316",false,320,820,[41,38,39,40],[21,5,45,48,47,14,13,12,11,10,9,17,46,3,8,18])]{
  let mut g=Guest::new(name);if expander{g.i2c_route(0,47,48);g.i2c(0,0x20,&[3,0x40],0).unwrap();g.i2c(0,0x20,&[1,0xf9],0).unwrap();g.route(4,256);g.high(4,false);}else{for p in[0,2,1,16,6]{g.route(p,256);}g.high(16,true);g.high(6,true);}
  for c in[0x11,0x29,0x3a]{serial_word(&mut g,expander,false,c);}serial_word(&mut g,expander,true,0x55);
  route_rgb(&mut g,controls,pads);rgb_dma(&mut g,w,h);let f=g.m.bus.board.display().unwrap();assert_eq!([f.2[0],f.2[(w*h/2)as usize],f.2[(w*h-1)as usize]],[0xf800,0x07e0,0x001f],"{name}: {}",g.m.bus.board.report());
  let count=g.m.bus.board.display_frames();g.route(controls[0],153);rgb_dma(&mut g,w,h);assert_eq!(g.m.bus.board.display_frames(),count,"actual incorrect PCLK mux");
  route_rgb(&mut g,controls,pads);if expander{g.i2c(0,0x20,&[1,0x79],0).unwrap();}else{g.high(16,false);}rgb_dma(&mut g,w,h);assert_eq!(g.m.bus.board.display_frames(),count);assert!(g.m.bus.board.display().unwrap().2.iter().all(|&p|p==0),"reset gates and clears delivered content");
 }
}
