//! GDEY0154D67 / SSD1681 200x200 digital monochrome endpoint.
//! Good Display GDEY0154D67 datasheet pp12-21; M5GFX 0.1.17 ff11e09a.
//! Byte-addressed X, retained optical frame, explicit update and bounded busy.
//! Analog LUT waveforms, temperature, ghosting and red/gray glass are not modeled.
use crate::board::{BoardEdge,VirtualCycle};
const BYTES:usize=25*200;
pub struct Ssd1681 {pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,ram:[Vec<u8>;2],cmd:u8,params:[u8;4],count:usize,read_at:usize,entry:u8,xs:u8,xe:u8,ys:u16,ye:u16,x:u8,y:u16,control1:u8,control2:u8,clock:bool,analog:bool,asleep:bool,cycle:VirtualCycle,deadline:Option<VirtualCycle>,refresh:bool,edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64}
impl Ssd1681 {
 pub fn new(busy_pin:u8,cycles_per_ms:u64)->Self {Self{frame:vec![0xffff;200*200],frames:0,version:0,ram_bytes_written:0,ram:[vec![0xff;BYTES],vec![0;BYTES]],cmd:0,params:[0;4],count:0,read_at:0,entry:3,xs:0,xe:0x15,ys:0,ye:0x127,x:0,y:0,control1:0,control2:255,clock:false,analog:false,asleep:false,cycle:0,deadline:None,refresh:false,edges:Vec::new(),busy_pin,cycles_per_ms}}
 pub fn reset(&mut self) {self.cmd=0;self.count=0;self.read_at=0;self.entry=3;self.xs=0;self.xe=0x15;self.ys=0;self.ye=0x127;self.x=0;self.y=0;self.control1=0;self.control2=255;self.clock=false;self.analog=false;self.asleep=false;self.refresh=false;if self.deadline.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}}
 pub fn busy(&self)->bool {self.deadline.is_some()}
 pub fn next_deadline(&self)->Option<VirtualCycle> {self.deadline}
 pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
 fn wait(&mut self,ms:u64) {if self.deadline.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}self.deadline=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));}
 pub fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;if let Some(end)=self.deadline.filter(|t|*t<=cycle){self.deadline=None;if self.refresh{for y in 0..200{for x in 0..200{let mut white=self.ram[0][y*25+x/8]&(0x80>>(x%8))!=0;if self.control1&8!=0{white= !white;}
  if self.control1&4!=0{white=false;}self.frame[y*200+x]=if white{0xffff}else{0};}}self.version+=1;self.frames+=1;self.refresh=false;}self.edges.push(BoardEdge{cycle:end,pin:self.busy_pin,level:false});}}
 pub fn write(&mut self,data:bool,bytes:&[u8]) {for &b in bytes{if self.asleep{continue;}
  if !data{self.cmd=b;self.count=0;self.read_at=0;match b{0x12=>{self.reset();self.wait(1);},0x20=>{if self.control2&0x80!=0{self.clock=true;}
  if self.control2&0x40!=0{self.analog=true;}self.refresh=self.control2&4!=0&&self.clock&&self.analog;self.wait(if self.refresh{20}else{1});if self.control2&2!=0{self.analog=false;}
  if self.control2&1!=0{self.clock=false;}},_=>{}}}else{self.data(b);}}}
 fn advance_address(&mut self) {if self.entry&4==0{if self.x==self.xe{self.x=self.xs;self.y=if self.y==self.ye{self.ys}else if self.entry&2!=0{self.y.wrapping_add(1)&0x1ff}else{self.y.wrapping_sub(1)&0x1ff};}else{self.x=if self.entry&1!=0{self.x.wrapping_add(1)&63}else{self.x.wrapping_sub(1)&63};}}else if self.y==self.ye{self.y=self.ys;self.x=if self.x==self.xe{self.xs}else if self.entry&1!=0{self.x.wrapping_add(1)&63}else{self.x.wrapping_sub(1)&63};}else{self.y=if self.entry&2!=0{self.y.wrapping_add(1)&0x1ff}else{self.y.wrapping_sub(1)&0x1ff};}}
 fn data(&mut self,b:u8) {if matches!(self.cmd,0x24|0x26){if self.x<25&&self.y<200{self.ram[usize::from(self.cmd==0x26)][self.y as usize*25+self.x as usize]=b;self.ram_bytes_written+=1;}self.advance_address();return;}
  if self.count<4{self.params[self.count]=b;}self.count=self.count.saturating_add(1);let word=|a:usize|u16::from_le_bytes([self.params[a],self.params[a+1]])&0x1ff;match(self.cmd,self.count){(0x10,1)=>self.asleep=b&1!=0,(0x11,1)=>self.entry=b&7,(0x21,1)=>self.control1=b,(0x22,1)=>self.control2=b,(0x44,2)=>{self.xs=self.params[0]&63;self.xe=b&63;},(0x45,4)=>{self.ys=word(0);self.ye=word(2);},(0x4e,1)=>self.x=b&63,(0x4f,2)=>self.y=word(0),_=>{}}}
 pub fn read(&mut self)->u8 {let value=if self.asleep{255}else if self.cmd==0x2f&&self.read_at==0{1|if self.busy(){4}else{0}}else{0};self.read_at=self.read_at.saturating_add(1);value}
}
#[cfg(test)] mod tests {
 use super::*;
 fn send(p:&mut Ssd1681,c:u8,d:&[u8]){p.write(false,&[c]);p.write(true,d);}
 #[test] fn byte_windows_refresh_busy_and_optical_retention(){let mut p=Ssd1681::new(1,1);send(&mut p,0x44,&[1,1]);send(&mut p,0x45,&[2,0,2,0]);send(&mut p,0x4e,&[1]);send(&mut p,0x4f,&[2,0]);send(&mut p,0x24,&[0x7f]);assert!(p.frame.iter().all(|x|*x==65535));send(&mut p,0x22,&[0xf7]);send(&mut p,0x20,&[]);assert!(p.busy());p.advance_to(20);assert_eq!(p.frame[2*200+8],0);assert_eq!(p.frame[2*200+9],65535);assert_eq!(p.frames,1);p.reset();assert_eq!(p.frame[408],0);send(&mut p,0x10,&[1]);send(&mut p,0x24,&[255]);assert_eq!(p.ram[0][51],0x7f);p.reset();send(&mut p,0x2f,&[]);assert_eq!(p.read(),1);assert_eq!(p.read(),0);}
 #[test] fn reverse_windows_and_guest_bounds_do_not_alias_or_panic(){let mut p=Ssd1681::new(1,1);send(&mut p,0x11,&[0]);send(&mut p,0x44,&[1,0]);send(&mut p,0x45,&[1,0,0,0]);send(&mut p,0x4e,&[1]);send(&mut p,0x4f,&[1,0]);send(&mut p,0x24,&[1,2,3,4]);assert_eq!([p.ram[0][26],p.ram[0][25],p.ram[0][1],p.ram[0][0]],[1,2,3,4]);for c in 0..=255{p.reset();send(&mut p,c,&[255;256]);p.advance_to(1_000_000);}assert_eq!(p.frame.len(),40000);}
}
