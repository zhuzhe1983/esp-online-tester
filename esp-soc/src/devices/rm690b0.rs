//! RM690B0 480x600 GRAM, explicit Waveshare RGB565/QSPI presentation subset.
//! RM690B0 public V0.3 pp54/76-86/89/91/110 and pinned 2.41 V2 factory.
//! Datasheet MADCTR p81 documents MX/BGR but omits D5/D4; factory writes30h.
//! We explicitly adopt that BSP's MV(20h) address transpose, with10h opaque scan
//! direction, not measured glass semantics. Unknown bits are rejected. No CO5300
//! state/brightness/reset inheritance. Unknown reset RAM uses deterministic zero.
use crate::{board::SpiQspiPins,devices::DcsPanel};
pub struct Rm690b0Bsp{
 memory:DcsPanel,pub brightness:u8,pub control:u8,pub page:u8,header:[u8;4],header_len:usize,parameters:usize,args:[u8;4],command:u8,normal:bool,discard:bool,window:[u16;4],pub changes:u64,pub rejected:u64,pub opaque_bytes:u64,
}
impl Default for Rm690b0Bsp{fn default()->Self{Self::new()}}
impl Rm690b0Bsp{
 pub fn new()->Self{let mut memory=DcsPanel::new(480,600);memory.colmod=0x77;Self{memory,brightness:0,control:0x28,page:0,header:[0;4],header_len:0,parameters:0,args:[0;4],command:0,normal:false,discard:false,window:[0,479,0,599],changes:0,rejected:0,opaque_bytes:0}}
 pub fn reset(&mut self){let(frames,pixels,changes,rejected)=(self.memory.frames,self.memory.pixels_written,self.changes,self.rejected);*self=Self::new();self.memory.frames=frames;self.memory.pixels_written=pixels;self.changes=changes.wrapping_add(1);self.rejected=rejected;}
 pub fn deselect(&mut self){self.header_len=0;self.parameters=0;self.normal=false;self.discard=false;}
 pub fn reject(&mut self){self.discard=true;self.rejected+=1;}
 pub fn frames(&self)->u64{self.memory.frames}
 pub fn pixels_written(&self)->u64{self.memory.pixels_written}
 pub fn madctl(&self)->u8{self.memory.madctl}
 pub fn nonzero_gram(&self)->usize{self.memory.gram.iter().filter(|&&p|p!=0).count()}
 pub fn pixel_format(&self)->u8{self.memory.colmod}
 fn rgb565(&self)->bool{let p=self.memory.colmod;if p&0x80!=0{(p>>4)&7==5}else{p&7==5}}
 fn orientation_supported(&self)->bool{self.memory.madctl&!0x78==0}
 pub fn visible(&self)->bool{self.memory.on&&!self.memory.sleeping&&self.rgb565()&&self.orientation_supported()&&(self.control&0x20==0||self.brightness>0)}
 pub fn version(&self)->u64{self.changes.wrapping_add(self.memory.pixels_written)}
 /// Fixed manufacturer's landscape window: physical columns16..465, rows0..599.
 /// Only this window follows the BSP; no calibrated LCD/glass coordinate claim.
 pub fn frame_landscape(&self)->Vec<u16>{let mut f=vec![0;600*450];if !self.visible(){return f;}let d=if self.control&0x20!=0{u32::from(self.brightness)}else{255};for y in 0..450{for x in 0..600{let mut p=self.memory.gram[x*480+y+16];if self.memory.madctl&8!=0{p=(p&0x7e0)|(p>>11)|((p&31)<<11);}
 if self.memory.inverted{p^=0xffff;}let p=u32::from(p);f[y*600+x]=((((p>>11)*d/255)<<11)|((((p>>5)&63)*d/255)<<5)|((p&31)*d/255))as u16;}}f}
 pub fn transfer(&mut self,p:SpiQspiPins,b:&[u8])->bool{if self.discard{return false;}let bits=u64::from(p.command_bits)+u64::from(p.address_bits)+u64::from(p.data_bits);if p.unsupported_mode||p.sio||p.full_duplex||p.dummy_cycles!=0||p.clock_idle_high||p.clock_output_edge||p.read_bits!=0||p.read_width!=0||!p.command_bits.is_multiple_of(8)||!p.address_bits.is_multiple_of(8)||!p.data_bits.is_multiple_of(8)||bits!=b.len()as u64*8||(p.command_bits==0)!=(p.command_width==0)||(p.address_bits==0)!=(p.address_width==0)||(p.data_bits==0)!=(p.data_width==0){self.reject();return false;}
 let mut at=0;for(n,width)in[(usize::from(p.command_bits)/8,p.command_width),(usize::from(p.address_bits)/8,p.address_width),(p.data_bits as usize/8,p.data_width)]{if n==0{continue;}
 if !self.phase(&b[at..at+n],width){self.reject();return false;}at+=n;}true}
 fn phase(&mut self,mut b:&[u8],width:u8)->bool{if self.header_len<4{if width!=1{return false;}let n=(4-self.header_len).min(b.len());self.header[self.header_len..self.header_len+n].copy_from_slice(&b[..n]);self.header_len+=n;b=&b[n..];if self.header_len==4{let[op,z1,c,z2]=self.header;if !matches!(op,2|0x32)||z1!=0||z2!=0||op==0x32&&!matches!(c,0x2c|0x3c){return false;}self.command=c;self.normal=self.page==0&&matches!(c,1|0x10|0x11|0x20|0x21|0x28|0x29|0x2a|0x2b|0x2c|0x3c|0x36|0x3a|0x51|0x53);if self.normal{if c==1{let h=self.header;self.reset();self.header=h;self.header_len=4;self.command=c;self.normal=true;}else{self.memory.dc=false;self.memory.byte(c);self.memory.dc=true;self.changes+=1;}}}}
 if b.is_empty(){return true;}
 if self.header_len!=4||width!=if self.header[0]==0x32{4}else{1}{return false;}
 if !self.normal{if self.command==0xfe{if self.parameters+b.len()>1{return false;}self.page=b[0];self.changes+=1;}self.parameters+=b.len();self.opaque_bytes+=b.len()as u64;return true;}
 if matches!(self.command,0x2c|0x3c){let[x0,x1,y0,y1]=self.window;let(cols,rows)=if self.memory.madctl&0x20!=0{(600,480)}else{(480,600)};if self.header[0]!=0x32||!self.rgb565()||!self.orientation_supported()||x0>x1||y0>y1||usize::from(x1)>=cols||usize::from(y1)>=rows{return false;}}
 else{let needed=match self.command{0x2a|0x2b=>4,0x36|0x3a|0x51|0x53=>1,_=>0};if self.parameters+b.len()>needed{return false;}for(i,&v)in b.iter().enumerate(){self.args[self.parameters+i]=v;}match self.command{0x51=>self.brightness=b[0],0x53=>self.control=b[0],0x2a|0x2b if self.parameters+b.len()==4=>{let a=u16::from_be_bytes([self.args[0],self.args[1]]);let z=u16::from_be_bytes([self.args[2],self.args[3]]);let start=if self.command==0x2a{0}else{2};self.window[start]=a;self.window[start+1]=z;},_=>{}}}
 for &v in b{self.memory.byte(v);}self.parameters+=b.len();self.changes+=1;true}
}
#[cfg(test)]mod tests{use super::*;
 fn p(n:usize,w:u8)->SpiQspiPins{SpiQspiPins{data_width:w,data_bits:n as u32*8,..Default::default()}}
 fn cmd(s:&mut Rm690b0Bsp,c:u8,a:&[u8]){assert!(s.transfer(p(4,1),&[2,0,c,0]));if !a.is_empty(){assert!(s.transfer(p(a.len(),1),a));}s.deselect();}
 fn pixel(s:&mut Rm690b0Bsp,c:u8,a:&[u8])->bool{assert!(s.transfer(p(4,1),&[0x32,0,c,0]));let ok=s.transfer(p(a.len(),4),a);s.deselect();ok}
 #[test]fn rm_defaults_brightness_page_and_landscape_window_are_independent(){let mut s=Rm690b0Bsp::new();assert_eq!((s.pixel_format(),s.brightness,s.control,s.page),(0x77,0,0x28,0));cmd(&mut s,0xfe,&[0x20]);cmd(&mut s,0x29,&[]);assert!(!s.memory.on);cmd(&mut s,0xfe,&[0]);cmd(&mut s,0x36,&[0x30]);cmd(&mut s,0x3a,&[0x55]);cmd(&mut s,0x11,&[]);cmd(&mut s,0x29,&[]);cmd(&mut s,0x2a,&[0,13,0,14]);cmd(&mut s,0x2b,&[0,23,0,23]);assert!(pixel(&mut s,0x2c,&[0xf8,0,0x07,0xe0]));assert!(!s.visible());cmd(&mut s,0x51,&[255]);let f=s.frame_landscape();assert_eq!(&f[7*600+13..7*600+15],&[0xf800,0x07e0]);cmd(&mut s,0x51,&[0]);assert!(!s.visible());cmd(&mut s,0x53,&[0]);assert!(s.visible());cmd(&mut s,0x01,&[]);assert_eq!((s.pixel_format(),s.brightness,s.control),(0x77,0,0x28));assert_eq!(s.memory.gram.iter().filter(|v|**v!=0).count(),0,"unknown reset RAM is zero policy, never inherited CO retention");}
 #[test]fn wrong_phases_reserved_bits_and_out_of_range_do_not_alias_valid_pixels(){let mut s=Rm690b0Bsp::new();cmd(&mut s,0x3a,&[0x55]);cmd(&mut s,0x36,&[0x80]);assert!(!pixel(&mut s,0x2c,&[0xff,0xff]));assert_eq!(s.pixels_written(),0);cmd(&mut s,0x36,&[0x30]);cmd(&mut s,0x2a,&[2,88,2,88]);cmd(&mut s,0x2b,&[0,16,0,16]);assert!(!pixel(&mut s,0x2c,&[0xff,0xff]));assert_eq!(s.pixels_written(),0);assert!(!s.transfer(p(4,4),&[2,0,0x11,0]));assert!(!s.transfer(p(4,1),&[2,0,0x29,0]));s.deselect();cmd(&mut s,0x11,&[]);assert!(!s.memory.sleeping);}
}
