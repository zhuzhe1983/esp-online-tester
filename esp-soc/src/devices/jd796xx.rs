//! Seeed BWRY digital endpoint for the 2.13 / 2.9 inch JD796xx driver paths.
//! Seeed_GFX2 556df7cb0b4a097d9b81026a26fd5511f9c934d6,
//! Driver_JD79676.cpp / Driver_JD79667.cpp and Seeed_Panel_Configs.h.
//! These are the manufacturer's compatibility-driver names, not a silicon
//! identification claim (the GDEY0213F51 panel is also documented as JD79661).
//! Native PSR direction, bounded 128-pixel storage rows, four wire colors,
//! power/sleep and active-low BUSY only. Other PSR scan directions, OTP/LUTs,
//! analog waveforms, temperature and physical refresh time are not modeled.
use crate::board::{BoardEdge,VirtualCycle};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Variant {Jd79676,Jd79667}
impl Variant {pub fn dimensions(self)->(usize,usize){match self{Self::Jd79676=>(122,250),Self::Jd79667=>(128,296)}}}
pub struct Jd796xx {
    pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,
    width:usize,height:usize,ram:Vec<u8>,command:u8,params:[u8;4],count:usize,at:usize,psr:u8,
    ready:bool,power:bool,sleep:bool,cycle:VirtualCycle,deadline:Option<VirtualCycle>,refresh:bool,
    edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,
}
impl Jd796xx {
    pub fn new(variant:Variant,busy_pin:u8,cycles_per_ms:u64)->Self {let(width,height)=variant.dimensions();Self{
        frame:vec![0xffff;width*height],frames:0,version:0,ram_bytes_written:0,width,height,ram:vec![0;32*height],command:0,params:[0;4],count:0,at:0,psr:15,
        ready:false,power:false,sleep:false,cycle:0,deadline:None,refresh:false,edges:Vec::new(),busy_pin,cycles_per_ms}}
    pub fn reset(&mut self) {self.command=0;self.count=0;self.at=0;self.psr=15;self.ready=false;self.power=false;self.sleep=false;self.refresh=false;
        if self.deadline.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}}
    pub fn power_loss(&mut self) {self.reset();self.ram.fill(0);}
    pub fn busy_high(&self)->bool {self.deadline.is_none()}
    pub fn next_deadline(&self)->Option<VirtualCycle> {self.deadline}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn busy_for(&mut self,ms:u64){if self.deadline.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}self.deadline=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));}
    pub fn advance_to(&mut self,cycle:VirtualCycle){self.cycle=cycle;if let Some(end)=self.deadline.filter(|d|*d<=cycle){self.deadline=None;if self.refresh{
        for y in 0..self.height{for x in 0..self.width{let color=(self.ram[y*32+x/4]>>(6-2*(x&3)))&3;self.frame[y*self.width+x]=[0xffff,0,0xffe0,0xf800][usize::from(color)];}}
        self.frames+=1;self.version+=1;self.refresh=false;
    }self.edges.push(BoardEdge{cycle:end,pin:self.busy_pin,level:true});}}
    pub fn write(&mut self,data:bool,bytes:&[u8]){for &b in bytes{if self.sleep{continue;}
        if !data{self.command=b;self.count=0;self.at=0;match b{
        0x04=>{self.power=true;self.busy_for(1);},0x02=>{self.power=false;self.refresh=false;self.busy_for(1);},_=>{}}}else{self.data(b);}}}
    fn data(&mut self,b:u8){
        if self.command==0x10{if self.at<self.ram.len(){self.ram[self.at]=b;self.ram_bytes_written+=1;self.at+=1;}return;}
        if self.count<4{self.params[self.count]=b;}self.count=self.count.saturating_add(1);match(self.command,self.count){
            (0x00,1)=>self.psr=b,
            (0x61,4)=>self.ready=u16::from_be_bytes([self.params[0],self.params[1]])==128&&usize::from(u16::from_be_bytes([self.params[2],self.params[3]]))==self.height,
            (0x12,1) if b==0&&self.power&&self.ready&&self.psr&15==15=>{self.refresh=true;self.busy_for(20);},
            (0x07,1) if b==0xa5=>{self.sleep=true;self.power=false;self.refresh=false;},_=>{}
        }
    }
}
#[cfg(test)] mod tests {
    use super::*;
    fn send(p:&mut Jd796xx,c:u8,d:&[u8]){p.write(false,&[c]);p.write(true,d);}
    fn power(p:&mut Jd796xx){let height=p.height;send(p,0x61,&[0,128,(height>>8) as u8,height as u8]);send(p,0x04,&[]);p.advance_to(p.cycle+1);}
    fn refresh(p:&mut Jd796xx){send(p,0x12,&[0]);p.advance_to(p.cycle+20);}
    #[test] fn four_wire_colors_crop_hidden_pixels_and_wait_for_activation(){
        let mut p=Jd796xx::new(Variant::Jd79676,4,1);power(&mut p);let mut row=vec![0;32];row[0]=0x1b;row[30]=0x0f;row[31]=255;send(&mut p,0x10,&row);p.write(true,&[0x40]);
        assert!(p.frame.iter().all(|v|*v==0xffff));send(&mut p,0x12,&[0]);p.advance_to(20);assert_eq!(p.frames,0);p.advance_to(21);
        assert_eq!(&p.frame[..4],&[0xffff,0,0xffe0,0xf800]);assert_eq!(&p.frame[120..124],&[0xffff,0xffff,0,0xffff]);assert_eq!(p.frame.len(),122*250);
    }
    #[test] fn wrong_resolution_direction_sleep_and_power_do_not_fabricate_frames(){
        let mut p=Jd796xx::new(Variant::Jd79667,4,1);power(&mut p);send(&mut p,0x10,&[0x40]);refresh(&mut p);assert_eq!(p.frame[0],0);
        p.reset();power(&mut p);refresh(&mut p);assert_eq!(p.frame[0],0);p.power_loss();assert_eq!(p.frame[0],0);power(&mut p);refresh(&mut p);assert_eq!(p.frame[0],0xffff);
        let count=p.frames;send(&mut p,0x00,&[3]);refresh(&mut p);assert_eq!(p.frames,count);send(&mut p,0x00,&[31]);send(&mut p,0x61,&[255;4]);refresh(&mut p);assert_eq!(p.frames,count);
        send(&mut p,0x07,&[0xa5]);power(&mut p);refresh(&mut p);assert_eq!(p.frames,count);
        for c in 0..=255{p.reset();send(&mut p,c,&[255;256]);p.advance_to(p.cycle+100);}assert_eq!(p.frame.len(),128*296);assert_eq!(p.ram.len(),32*296);
    }
}
