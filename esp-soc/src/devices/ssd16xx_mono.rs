//! SSD1680 / SSD1683 bounded monochrome digital endpoints for Seeed panels.
//! Seeed_GFX2 556df7cb0b4a097d9b81026a26fd5511f9c934d6:
//! src/driver/epaper/Driver_SSD1680.cpp, Driver_SSD1683.cpp and
//! src/panel/configs/Seeed_Panel_Configs.h. Byte X and little-endian gate Y,
//! 0x24 new / 0x26 old RAM, 0x22 control followed by 0x20 activation.
//! This models monochrome endpoints and active-high BUSY, not analog LUTs,
//! ghosting, calibrated gray levels or physical refresh time. The 122px glass
//! has 128 storage pixels; the six trailing bits never leak into the next row.
use crate::board::{BoardEdge,VirtualCycle};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Variant {Ssd1680_213,Ssd1680_290,Ssd1683_420}
impl Variant {
    pub fn dimensions(self)->(usize,usize) {match self {Self::Ssd1680_213=>(122,250),Self::Ssd1680_290=>(128,296),Self::Ssd1683_420=>(400,300)}}
}
pub struct Ssd16xxMono {
    pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,
    width:usize,height:usize,stride:usize,ram:[Vec<u8>;2],command:u8,params:[u8;4],count:usize,
    entry:u8,xs:u8,xe:u8,ys:u16,ye:u16,x:u8,y:u16,control1:u8,control2:u8,
    clock:bool,analog:bool,sleep:bool,cycle:VirtualCycle,deadline:Option<VirtualCycle>,
    refresh:bool,edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,
}
impl Ssd16xxMono {
    pub fn new(variant:Variant,busy_pin:u8,cycles_per_ms:u64)->Self {
        let (width,height)=variant.dimensions();let stride=width.div_ceil(8);let bytes=stride*height;
        Self{frame:vec![0xffff;width*height],frames:0,version:0,ram_bytes_written:0,width,height,stride,ram:[vec![255;bytes],vec![255;bytes]],command:0,params:[0;4],count:0,
            entry:3,xs:0,xe:(stride-1) as u8,ys:0,ye:(height-1) as u16,x:0,y:0,control1:0,control2:255,clock:false,analog:false,sleep:false,cycle:0,deadline:None,refresh:false,edges:Vec::new(),busy_pin,cycles_per_ms}
    }
    pub fn reset(&mut self) {
        self.command=0;self.count=0;self.entry=3;self.xs=0;self.xe=(self.stride-1) as u8;self.ys=0;self.ye=(self.height-1) as u16;self.x=0;self.y=0;
        self.control1=0;self.control2=255;self.clock=false;self.analog=false;self.sleep=false;self.refresh=false;
        if self.deadline.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}
    }
    /// Deterministic uninitialized RAM; reset alone retains it, power loss does not.
    pub fn power_loss(&mut self) {self.reset();for r in &mut self.ram {r.fill(255);}}
    pub fn busy(&self)->bool {self.deadline.is_some()}
    pub fn next_deadline(&self)->Option<VirtualCycle> {self.deadline}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn busy_for(&mut self,ms:u64) {if self.deadline.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}self.deadline=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));}
    pub fn advance_to(&mut self,cycle:VirtualCycle) {
        self.cycle=cycle;if let Some(end)=self.deadline.filter(|d|*d<=cycle){self.deadline=None;if self.refresh {
            for y in 0..self.height {for x in 0..self.width {let mut white=self.ram[0][y*self.stride+x/8]&(0x80>>(x&7))!=0;
                if self.control1&8!=0{white= !white;}
                if self.control1&4!=0{white=false;}self.frame[y*self.width+x]=if white{0xffff}else{0};}}
            self.frames+=1;self.version+=1;self.refresh=false;
        }self.edges.push(BoardEdge{cycle:end,pin:self.busy_pin,level:false});}
    }
    pub fn write(&mut self,data:bool,bytes:&[u8]) {for &b in bytes {if self.sleep {continue;}
        if data{self.data(b);}else{
        self.command=b;self.count=0;match b{
            0x12=>{self.reset();self.busy_for(1);},
            0x20=>{if self.control2&0x80!=0{self.clock=true;}
                if self.control2&0x40!=0{self.analog=true;}
                self.refresh=self.control2&4!=0&&self.clock&&self.analog;self.busy_for(if self.refresh{20}else{1});
                if self.control2&2!=0{self.analog=false;}
                if self.control2&1!=0{self.clock=false;}},_=>{}
        }
    }}}
    fn advance_address(&mut self) {
        let nx=|v:u8|if self.entry&1!=0{v.wrapping_add(1)&63}else{v.wrapping_sub(1)&63};
        let ny=|v:u16|if self.entry&2!=0{v.wrapping_add(1)&511}else{v.wrapping_sub(1)&511};
        if self.entry&4==0 {if self.x==self.xe{self.x=self.xs;self.y=if self.y==self.ye{self.ys}else{ny(self.y)};}else{self.x=nx(self.x);}}
        else if self.y==self.ye{self.y=self.ys;self.x=if self.x==self.xe{self.xs}else{nx(self.x)};}else{self.y=ny(self.y);}
    }
    fn data(&mut self,b:u8) {
        if matches!(self.command,0x24|0x26){if usize::from(self.x)<self.stride&&usize::from(self.y)<self.height {
            self.ram[usize::from(self.command==0x26)][usize::from(self.y)*self.stride+usize::from(self.x)]=b;self.ram_bytes_written+=1;
        }self.advance_address();return;}
        if self.count<4{self.params[self.count]=b;}self.count=self.count.saturating_add(1);
        let word=|at:usize|u16::from_le_bytes([self.params[at],self.params[at+1]])&511;
        match(self.command,self.count){(0x10,1)=>self.sleep=b&1!=0,(0x11,1)=>self.entry=b&7,(0x21,1)=>self.control1=b,(0x22,1)=>self.control2=b,
            (0x44,2)=>{self.xs=self.params[0]&63;self.xe=b&63;},(0x45,4)=>{self.ys=word(0);self.ye=word(2);},(0x4e,1)=>self.x=b&63,(0x4f,2)=>self.y=word(0),_=>{}}
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn send(p:&mut Ssd16xxMono,c:u8,d:&[u8]){p.write(false,&[c]);p.write(true,d);}
    fn refresh(p:&mut Ssd16xxMono){send(p,0x22,&[0xf7]);send(p,0x20,&[]);p.advance_to(p.cycle+20);}
    #[test] fn hidden_213_bits_do_not_shift_rows_or_publish_before_busy(){
        let mut p=Ssd16xxMono::new(Variant::Ssd1680_213,4,1);let mut row=vec![255;16];row[15]=0xc0;
        send(&mut p,0x24,&row);p.write(true,&[0x7f]);assert!(p.frame.iter().all(|v|*v==0xffff));
        send(&mut p,0x22,&[0xf7]);send(&mut p,0x20,&[]);p.advance_to(19);assert_eq!(p.frames,0);p.advance_to(20);
        assert_eq!(&p.frame[120..124],&[0xffff,0xffff,0,0xffff]);assert_eq!(p.frame.len(),122*250);assert_eq!(p.ram_bytes_written,17);
        assert_eq!(p.take_edges(),[BoardEdge{cycle:0,pin:4,level:true},BoardEdge{cycle:20,pin:4,level:false}]);
    }
    #[test] fn independent_variants_reverse_windows_sleep_and_power_loss(){
        for variant in [Variant::Ssd1680_290,Variant::Ssd1683_420] {
            let mut p=Ssd16xxMono::new(variant,4,1);send(&mut p,0x11,&[0]);send(&mut p,0x44,&[1,0]);send(&mut p,0x45,&[1,0,0,0]);send(&mut p,0x4e,&[1]);send(&mut p,0x4f,&[1,0]);send(&mut p,0x24,&[0x7f,255,255,255]);
            refresh(&mut p);assert_eq!(p.frame[p.width+8],0);assert_eq!(p.frame[p.width+9],0xffff);
            p.reset();refresh(&mut p);assert_eq!(p.frame[p.width+8],0);p.power_loss();assert_eq!(p.frame[p.width+8],0);refresh(&mut p);assert_eq!(p.frame[p.width+8],0xffff);
            send(&mut p,0x10,&[1]);send(&mut p,0x24,&[0]);let n=p.frames;refresh(&mut p);assert_eq!(p.frames,n);
            for c in 0..=255{p.reset();send(&mut p,c,&[255;256]);p.advance_to(p.cycle+100);}assert_eq!(p.frame.len(),p.width*p.height);
        }
    }
}
