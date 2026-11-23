//! UC8151D 104x212 / 128x296 flexible monochrome digital endpoints.
//! UltraChip UC8151d_B0.6 pp11,22,26, manufacturer-hosted:
//! https://files.seeedstudio.com/wiki/Other_Display/213-flexible/IC%20Driver%20UC8151D.pdf
//! SHA256 d9fa26cc9c0b6959bffbe1105af5b897fd91fd40abb084be825e70c9fe79895d.
//! Seeed_GFX2 556df7cb0b4a097d9b81026a26fd5511f9c934d6 Driver_UC8151D
//! and seeed_ep.inl provide the GDEW0213I5FD / GDEW029I6FD OTP init paths.
//! TRES is THREE bytes; PTL is SEVEN bytes, unlike UC8179. In particular the
//! current GFX2 nine-byte setAddrWindow is not silently corrected here.
//! No OTP contents, red mode, analog LUT/ghosting or measured timing claim.
use crate::board::{BoardEdge,VirtualCycle};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Variant {Flex213,Flex290}
impl Variant {pub fn dimensions(self)->(usize,usize){match self{Self::Flex213=>(104,212),Self::Flex290=>(128,296)}}}
pub struct Uc8151 {
    pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,
    width:usize,height:usize,ram:[Vec<u8>;2],command:u8,params:[u8;7],count:usize,at:usize,psr:u8,
    window:[usize;4],partial:bool,ready:bool,origin:bool,power:bool,sleep:bool,
    cycle:VirtualCycle,deadline:Option<VirtualCycle>,refresh:bool,edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,
}
impl Uc8151 {
    pub fn new(variant:Variant,busy_pin:u8,cycles_per_ms:u64)->Self {let(width,height)=variant.dimensions();let bytes=width*height/8;Self{
        frame:vec![0xffff;width*height],frames:0,version:0,ram_bytes_written:0,width,height,ram:[vec![255;bytes],vec![255;bytes]],command:0,params:[0;7],count:0,at:0,psr:15,
        window:[0,width-1,0,height-1],partial:false,ready:false,origin:true,power:false,sleep:false,cycle:0,deadline:None,refresh:false,edges:Vec::new(),busy_pin,cycles_per_ms}}
    pub fn reset(&mut self){self.command=0;self.count=0;self.at=0;self.psr=15;self.window=[0,self.width-1,0,self.height-1];self.partial=false;self.ready=false;self.origin=true;self.power=false;self.sleep=false;self.refresh=false;
        if self.deadline.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}}
    pub fn power_loss(&mut self){self.reset();for r in &mut self.ram{r.fill(255);}}
    pub fn busy_high(&self)->bool{self.deadline.is_none()}
    pub fn next_deadline(&self)->Option<VirtualCycle>{self.deadline}
    pub fn take_edges(&mut self)->Vec<BoardEdge>{std::mem::take(&mut self.edges)}
    fn busy_for(&mut self,ms:u64){if self.deadline.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}self.deadline=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));}
    pub fn advance_to(&mut self,cycle:VirtualCycle){self.cycle=cycle;if let Some(end)=self.deadline.filter(|d|*d<=cycle){self.deadline=None;if self.refresh{self.publish();self.refresh=false;}self.edges.push(BoardEdge{cycle:end,pin:self.busy_pin,level:true});}}
    fn window(&self)->Option<[usize;4]>{let w=if self.partial{self.window}else{[0,self.width-1,0,self.height-1]};let[x0,x1,y0,y1]=w;
        (x0<=x1&&y0<=y1&&x1<self.width&&y1<self.height).then_some(w)}
    pub fn write(&mut self,data:bool,bytes:&[u8]){for &b in bytes{if self.sleep{continue;}
        if data{self.data(b);}else{self.command=b;self.count=0;self.at=0;match b{
        0x04=>{self.power=true;self.busy_for(1);},0x02=>{self.power=false;self.refresh=false;self.busy_for(1);},
        0x12 if self.power&&self.ready&&self.origin&&self.psr&0x13==0x13=>{self.refresh=true;self.busy_for(20);},
        0x91=>self.partial=true,0x92=>self.partial=false,_=>{}
    }}}}
    fn data(&mut self,b:u8){
        if matches!(self.command,0x10|0x13){if let Some([x0,x1,y0,y1])=self.window(){let cols=x1/8-x0/8+1;let len=cols*(y1-y0+1);
            if self.at<len{let i=(y0+self.at/cols)*(self.width/8)+x0/8+self.at%cols;self.ram[usize::from(self.command==0x13)][i]=b;self.at+=1;self.ram_bytes_written+=1;}}
            return;}
        if self.count<7{self.params[self.count]=b;}self.count=self.count.saturating_add(1);match(self.command,self.count){
            (0x00,1)=>{if b&1==0{self.reset();}else{self.psr=b;}},
            (0x07,1) if b==0xa5=>{self.sleep=true;self.power=false;self.refresh=false;},
            (0x61,3)=>self.ready=usize::from(self.params[0]&0xf8)==self.width&&usize::from(u16::from_be_bytes([self.params[1]&1,b]))==self.height,
            (0x65,3)=>self.origin=self.params[0]&0xf8==0&&self.params[1]&1==0&&b==0,
            (0x90,7)=>self.window=[usize::from(self.params[0]&0xf8),usize::from(self.params[1]|7),usize::from(u16::from_be_bytes([self.params[2]&1,self.params[3]])),usize::from(u16::from_be_bytes([self.params[4]&1,self.params[5]]))],_=>{}
        }
    }
    fn publish(&mut self){if let Some([x0,x1,y0,y1])=self.window(){for y in y0..=y1{for x in x0..=x1{
        let dx=if self.psr&4!=0{x}else{self.width-1-x};let dy=if self.psr&8!=0{y}else{self.height-1-y};
        self.frame[dy*self.width+dx]=if self.ram[1][y*(self.width/8)+x/8]&(0x80>>(x&7))!=0{0xffff}else{0};
    }}self.frames+=1;self.version+=1;}}
}
#[cfg(test)] mod tests{
    use super::*;
    fn send(p:&mut Uc8151,c:u8,d:&[u8]){p.write(false,&[c]);p.write(true,d);}
    fn power(p:&mut Uc8151){let (width,height)=(p.width,p.height);send(p,0x04,&[]);p.advance_to(p.cycle+1);send(p,0x00,&[0x1f]);send(p,0x61,&[width as u8,(height>>8) as u8,height as u8]);}
    fn refresh(p:&mut Uc8151){send(p,0x12,&[]);p.advance_to(p.cycle+20);}
    #[test] fn official_otp_init_three_byte_resolution_and_wire_planes(){
        for variant in [Variant::Flex213,Variant::Flex290]{let mut p=Uc8151::new(variant,4,1);power(&mut p);send(&mut p,0x10,&[0]);refresh(&mut p);assert_eq!(p.frame[0],0xffff);
            send(&mut p,0x13,&[0x7f]);send(&mut p,0x12,&[]);assert!(!p.busy_high());p.advance_to(p.cycle+19);assert_eq!(p.frame[0],0xffff);p.advance_to(p.cycle+1);assert_eq!(&p.frame[..2],&[0,0xffff]);
            send(&mut p,0x00,&[0x13]);refresh(&mut p);assert_eq!(p.frame[p.width*p.height-1],0);
            p.power_loss();assert_eq!(p.frame[p.width*p.height-1],0);power(&mut p);refresh(&mut p);assert!(p.frame.iter().all(|v|*v==0xffff));
        }
    }
    #[test] fn partial_window_uses_seven_not_nine_bytes_and_never_reallocates(){
        let mut p=Uc8151::new(Variant::Flex213,4,1);power(&mut p);send(&mut p,0x91,&[]);send(&mut p,0x90,&[8,15,0,2,0,2,1]);send(&mut p,0x13,&[0]);refresh(&mut p);
        assert_eq!(p.frame[2*104+7],0xffff);assert_eq!(&p.frame[2*104+8..2*104+16],&[0;8]);assert_eq!(p.frame[2*104+16],0xffff);
        send(&mut p,0x90,&[0,8,0,15,0,2,0,2,1]);send(&mut p,0x13,&[0;20]);refresh(&mut p);assert_eq!(p.frames,1,"UC8179-shaped invalid range cannot alias to a valid UC8151 window");
        for c in 0..=255{p.reset();send(&mut p,c,&[255;256]);p.advance_to(p.cycle+100);}assert_eq!(p.frame.len(),104*212);assert_eq!(p.ram[1].len(),13*212);
    }
}
