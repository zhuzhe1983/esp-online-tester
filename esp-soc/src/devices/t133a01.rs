//! T133A01 dual-controller, 1200x1600 indexed six-color digital subset.
//! Seeed_GFX2 556df7cb0b4a097d9b81026a26fd5511f9c934d6,
//! src/driver/epaper/Driver_T133A01.cpp: init, pushColors and update.
//! Each CS receives 600 pixels of every portrait row. TRES is 1200x800
//! on each controller, as in that manufacturer's initialization sequence.
//! Fixed bounded RAM; ideal colors and synthetic BUSY, not optical waveforms.
use crate::board::{BoardEdge,VirtualCycle};

struct Half {ram:Vec<u8>,command:u8,params:[u8;4],count:usize,at:usize,ready:bool,power:bool,sleep:bool,refresh:bool}
impl Half {fn new()->Self {Self{ram:vec![0x11;480000],command:0,params:[0;4],count:0,at:0,ready:false,power:false,sleep:false,refresh:false}}}
pub struct T133a01 {
    pub frame:Vec<u16>,pub version:u64,pub frames:u64,pub ram_bytes_written:u64,
    halves:[Half;2],cycle:VirtualCycle,busy_until:Option<VirtualCycle>,edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,
}
impl T133a01 {
    pub fn new(busy_pin:u8,cycles_per_ms:u64)->Self {Self{frame:vec![0xffff;1200*1600],version:0,frames:0,ram_bytes_written:0,halves:std::array::from_fn(|_|Half::new()),cycle:0,busy_until:None,edges:Vec::new(),busy_pin,cycles_per_ms}}
    pub fn reset(&mut self) {for h in &mut self.halves {*h=Half::new();}if self.busy_until.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}}
    pub fn busy_high(&self)->bool {self.busy_until.is_none()}
    pub fn next_deadline(&self)->Option<VirtualCycle> {self.busy_until}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn busy_for(&mut self,ms:u64) {if self.busy_until.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}self.busy_until=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));}
    pub fn write(&mut self,selected:u8,data:bool,bytes:&[u8]) {
        let mut busy=0;
        for chip in 0..2 {if selected&(1<<chip)==0{continue;}let h=&mut self.halves[chip];
            for &b in bytes {if h.sleep{continue;}
                if !data {h.command=b;h.count=0;if b==0x10{h.at=0;}else if b==0x04{h.power=true;busy=busy.max(1);}continue;}
                if h.command==0x10 {if h.at<h.ram.len(){h.ram[h.at]=b;h.at+=1;self.ram_bytes_written+=1;}continue;}
                if h.count<h.params.len(){h.params[h.count]=b;}h.count=h.count.saturating_add(1);
                match (h.command,h.count) {
                    (0x61,4)=>h.ready=h.params==[0x04,0xb0,0x03,0x20],
                    (0x12,1) if b==1&&h.ready&&h.power=>{h.refresh=true;busy=20;},
                    (0x02,1) if b==0=>{h.power=false;h.refresh=false;busy=busy.max(1);},
                    (0x07,1) if b==0xa5=>{h.sleep=true;h.power=false;h.refresh=false;},_=>{}
                }
            }
        }if busy!=0{self.busy_for(busy);}
    }
    pub fn advance_to(&mut self,cycle:VirtualCycle) {
        self.cycle=cycle;let Some(until)=self.busy_until.filter(|u|*u<=cycle)else{return;};self.busy_until=None;
        let mut changed=false;
        for (chip,h) in self.halves.iter_mut().enumerate(){if !h.refresh{continue;}h.refresh=false;changed=true;
            for (i,&b) in h.ram.iter().enumerate(){for (half,index) in [b>>4,b&15].into_iter().enumerate(){
                let color=match index{0=>Some(0),1=>Some(0xffff),2=>Some(0xffe0),3=>Some(0xf800),5=>Some(0x001f),6=>Some(0x07e0),_=>None};
                if let Some(c)=color{let pixel=i*2+half;self.frame[(pixel/600)*1200+chip*600+pixel%600]=c;}
            }}
        }if changed{self.frames+=1;self.version+=1;}self.edges.push(BoardEdge{cycle:until,pin:self.busy_pin,level:true});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(p:&mut T133a01,cs:u8,c:u8,d:&[u8]){p.write(cs,false,&[c]);p.write(cs,true,d);}
    #[test]
    fn dual_select_maps_each_half_and_waits_for_valid_powered_refresh() {
        let mut p=T133a01::new(13,1);send(&mut p,3,0x61,&[4,0xb0,3,0x20]);send(&mut p,1,0x10,&[0x23]);send(&mut p,2,0x10,&[0x56]);
        send(&mut p,3,0x12,&[1]);p.advance_to(100);assert_eq!(p.frames,0);send(&mut p,3,0x04,&[]);p.advance_to(101);send(&mut p,3,0x12,&[1]);assert!(!p.busy_high());assert_eq!(p.frame[0],0xffff);
        p.advance_to(121);assert_eq!(&p.frame[..2],&[0xffe0,0xf800]);assert_eq!(&p.frame[600..602],&[0x001f,0x07e0]);assert_eq!(p.frame[1200],0xffff);assert_eq!(p.frames,1);
        send(&mut p,1,0x10,&[0]);send(&mut p,1,0x12,&[1]);p.advance_to(141);assert_eq!(p.frame[0],0);assert_eq!(p.frame[600],0x001f);assert_eq!(p.ram_bytes_written,3);p.reset();assert_eq!(p.frame[600],0x001f);
    }
    #[test]
    fn malformed_resolution_sleep_wrong_select_and_overrun_stay_bounded() {
        let mut p=T133a01::new(13,1);send(&mut p,0,0x10,&[0;4]);assert_eq!(p.ram_bytes_written,0);
        send(&mut p,3,0x61,&[255;4]);send(&mut p,3,0x04,&[]);send(&mut p,3,0x10,&vec![0;500000]);assert_eq!(p.ram_bytes_written,960000);send(&mut p,3,0x12,&[1]);p.advance_to(100);assert_eq!(p.frames,0);
        send(&mut p,3,0x07,&[0xa5]);send(&mut p,3,0x61,&[4,0xb0,3,0x20]);send(&mut p,3,0x04,&[]);send(&mut p,3,0x12,&[1]);p.advance_to(200);assert_eq!(p.frames,0);
    }
}
