//! ED2208/EL040EF1 digital SPI subset, M5GFX 5268353 Panel_ED2208.{hpp,cpp}.
//! Fixed 400x600 RAM; native 4bpp palette, refresh/power/sleep and active-low BUSY.
//! Ideal manufacturer-driver colors, not measured Spectra 6 reflectance or waveforms.
use crate::board::{BoardEdge,VirtualCycle};

pub struct Ed2208 {
    pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,pub invalid_colors:u64,
    ram:Vec<u8>,command:u8,params:[u8;6],count:usize,at:usize,ready:bool,power:bool,sleep:bool,
    cycle:VirtualCycle,busy_until:Option<VirtualCycle>,refresh:bool,edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,
}
impl Ed2208 {
    pub const WIDTH:usize=400;pub const HEIGHT:usize=600;
    pub fn new(busy_pin:u8,cycles_per_ms:u64)->Self {Self{frame:vec![0xffff;Self::WIDTH*Self::HEIGHT],frames:0,version:0,ram_bytes_written:0,invalid_colors:0,ram:vec![0x11;Self::WIDTH*Self::HEIGHT/2],command:0,params:[0;6],count:0,at:0,ready:false,power:false,sleep:false,cycle:0,busy_until:None,refresh:false,edges:Vec::new(),busy_pin,cycles_per_ms}}
    /// Hardware/controller reset retains the visible bistable image. RAM retention is not
    /// claimed; clients must rewrite it before their next refresh.
    pub fn reset(&mut self) {self.command=0;self.count=0;self.at=0;self.ready=false;self.power=false;self.sleep=false;self.refresh=false;self.ram.fill(0x11);if self.busy_until.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}}
    pub fn busy_high(&self)->bool {self.busy_until.is_none()}
    pub fn next_deadline(&self)->Option<VirtualCycle> {self.busy_until}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn busy_for(&mut self,ms:u64) {if self.busy_until.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}self.busy_until=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));}
    pub fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;if let Some(until)=self.busy_until.filter(|u|*u<=cycle) {self.busy_until=None;if self.refresh {self.publish();self.refresh=false;}self.edges.push(BoardEdge{cycle:until,pin:self.busy_pin,level:true});}}
    pub fn write(&mut self,data:bool,bytes:&[u8]) {for &b in bytes {if !data {self.command(b);} else if !self.sleep {self.data(b);}}}
    fn command(&mut self,b:u8) {
        if self.sleep {return;}
        self.command=b;self.count=0;
        match b {0x10=>self.at=0,0x04=>{self.power=true;self.busy_for(1);},_=>{}}
    }
    fn data(&mut self,b:u8) {
        if self.command==0x10 {if self.at<self.ram.len(){self.ram[self.at]=b;self.at+=1;self.ram_bytes_written+=1;}return;}
        if self.count<self.params.len(){self.params[self.count]=b;}self.count=self.count.saturating_add(1);
        match (self.command,self.count) {
            (0x61,4)=>self.ready=u16::from_be_bytes([self.params[0],self.params[1]])==400 && u16::from_be_bytes([self.params[2],self.params[3]])==600,
            // Bounded synthetic completion, not the glass's 15–30 second optical timing.
            (0x12,1) if b==0 && self.ready && self.power=>{self.refresh=true;self.busy_for(20);},
            (0x02,1) if b==0=>{self.power=false;self.refresh=false;self.busy_for(1);},
            (0x07,1) if b==0xa5=>{self.sleep=true;self.power=false;self.refresh=false;},
            _=>{}
        }
    }
    fn publish(&mut self) {
        for (i,&byte) in self.ram.iter().enumerate(){for (half,index) in [byte>>4,byte&15].into_iter().enumerate(){
            let rgb=match index {0=>Some([0u16,0,0]),1=>Some([255,255,255]),2=>Some([255,243,56]),3=>Some([191,0,0]),5=>Some([100,64,255]),6=>Some([67,138,28]),_=>None};
            if let Some([r,g,b])=rgb {self.frame[i*2+half]=((r>>3)<<11)|((g>>2)<<5)|(b>>3);}else{self.invalid_colors+=1;}
        }}self.frames+=1;self.version+=1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(p:&mut Ed2208,c:u8,d:&[u8]) {p.write(false,&[c]);p.write(true,d);}
    #[test]
    fn packed_pixels_publish_only_on_valid_powered_refresh_then_survive_reset() {
        let mut p=Ed2208::new(11,1000);send(&mut p,0x10,&[0x01,0x23,0x56]);send(&mut p,0x12,&[0]);assert_eq!(p.frames,0);assert!(p.busy_high());
        send(&mut p,0x61,&[1,0x90,2,0x58]);send(&mut p,0x04,&[]);p.advance_to(1000);send(&mut p,0x12,&[0]);assert!(!p.busy_high());assert_eq!(p.frame[0],0xffff);
        p.advance_to(21000);assert!(p.busy_high());assert_eq!(p.frames,1);assert_eq!(&p.frame[..6],&[0,0xffff,0xff87,0xb800,0x621f,0x4443]);
        assert!(p.take_edges().contains(&BoardEdge{cycle:21000,pin:11,level:true}));p.reset();assert_eq!(p.frame[0],0);assert_eq!(p.frames,1);
    }
    #[test]
    fn invalid_resolution_sleep_and_overrun_cannot_allocate_or_refresh() {
        let mut p=Ed2208::new(11,1);send(&mut p,0x61,&[0xff;4]);send(&mut p,0x10,&vec![0;200000]);assert_eq!(p.ram_bytes_written,120000);
        send(&mut p,0x04,&[]);p.advance_to(1);send(&mut p,0x12,&[0]);p.advance_to(100);assert_eq!(p.frames,0);
        send(&mut p,0x07,&[0xa5]);send(&mut p,0x61,&[1,0x90,2,0x58]);send(&mut p,0x04,&[]);send(&mut p,0x12,&[0]);p.advance_to(1000);assert_eq!(p.frames,0);
    }
}
