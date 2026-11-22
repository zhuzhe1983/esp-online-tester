//! UC8179 800x480 / 648x480 monochrome digital endpoint, not an optical waveform model.
//! Command and window contract: Seeed-Studio/Seeed_GFX2
//! 556df7cb0b4a097d9b81026a26fd5511f9c934d6, Driver_UC8179.{h,cpp}.
//! Full/partial 1bpp updates, power, sleep and active-low BUSY are represented.
//! Four-gray LUT interpretation, OTP, temperature and physical timing are absent.
use crate::board::{BoardEdge,VirtualCycle};

pub struct Uc8179 {
    pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,
    width:usize,height:usize,ram:[Vec<u8>;2],command:u8,params:[u8;9],count:usize,at:usize,
    window:[usize;4],partial:bool,ready:bool,power:bool,sleep:bool,
    cycle:VirtualCycle,busy_until:Option<VirtualCycle>,refresh:bool,
    edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,
}
impl Uc8179 {
    pub fn new(busy_pin:u8,cycles_per_ms:u64)->Self {Self::with_width(800,busy_pin,cycles_per_ms)}
    /// Seeed GFX2 Config_Seeed_ePaper_5inch83_BW_UC8179. The glass, not a
    /// guest TRES write, chooses the bounded storage and visible resolution.
    pub fn seeed_583(busy_pin:u8,cycles_per_ms:u64)->Self {Self::with_width(648,busy_pin,cycles_per_ms)}
    fn with_width(width:usize,busy_pin:u8,cycles_per_ms:u64)->Self {let height=480;let bytes=width*height/8;Self{frame:vec![0xffff;width*height],frames:0,version:0,ram_bytes_written:0,width,height,ram:[vec![255;bytes],vec![255;bytes]],command:0,params:[0;9],count:0,at:0,window:[0,width-1,0,height-1],partial:false,ready:false,power:false,sleep:false,cycle:0,busy_until:None,refresh:false,edges:Vec::new(),busy_pin,cycles_per_ms}}
    // bb_epaper 8e26f00c's UC8179 full-update path writes RAM, then resets and
    // reinitializes before DISPLAY_REFRESH. RESET must not erase those planes.
    pub fn reset(&mut self) {self.command=0;self.count=0;self.at=0;self.window=[0,self.width-1,0,self.height-1];self.partial=false;self.ready=false;self.power=false;self.sleep=false;self.refresh=false;if self.busy_until.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}}
    /// Lose volatile RAM when the board removes panel power, retaining the
    /// bistable visible image. White is the deterministic uninitialized value.
    pub fn power_loss(&mut self) {self.reset();for r in &mut self.ram {r.fill(255);}}
    pub fn busy_high(&self)->bool {self.busy_until.is_none()}
    pub fn next_deadline(&self)->Option<VirtualCycle> {self.busy_until}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn busy_for(&mut self,ms:u64) {if self.busy_until.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}self.busy_until=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));}
    pub fn advance_to(&mut self,cycle:VirtualCycle) {self.cycle=cycle;if let Some(until)=self.busy_until.filter(|u|*u<=cycle){self.busy_until=None;if self.refresh{self.publish();self.refresh=false;}self.edges.push(BoardEdge{cycle:until,pin:self.busy_pin,level:true});}}
    pub fn write(&mut self,data:bool,bytes:&[u8]) {for &b in bytes {if self.sleep {continue;}
        if data {self.data(b);}else {self.command=b;self.count=0;self.at=0;match b {
        0x04=>{self.power=true;self.busy_for(1);},
        0x02=>{self.power=false;self.refresh=false;self.busy_for(1);},
        0x12 if self.power&&self.ready=>{self.refresh=true;self.busy_for(20);},
        0x91=>self.partial=true,0x92=>{self.partial=false;self.window=[0,self.width-1,0,self.height-1];},_=>{}
    }}}}
    fn data(&mut self,b:u8) {
        if matches!(self.command,0x10|0x13) {
            let [x0,x1,y0,y1]=if self.partial {self.window}else{[0,self.width-1,0,self.height-1]};
            if x0>x1||y0>y1||x1>=self.width||y1>=self.height{return;}
            let cols=x1/8-x0/8+1;let bytes=cols*(y1-y0+1);
            if self.at<bytes {let i=(y0+self.at/cols)*(self.width/8)+x0/8+self.at%cols;self.ram[usize::from(self.command==0x13)][i]=b;self.ram_bytes_written+=1;self.at+=1;}
            return;
        }
        if self.count<self.params.len(){self.params[self.count]=b;}self.count=self.count.saturating_add(1);
        match (self.command,self.count) {
            (0x07,1) if b==0xa5=>{self.sleep=true;self.power=false;self.refresh=false;},
            (0x61,4)=>self.ready=usize::from(u16::from_be_bytes([self.params[0],self.params[1]]))==self.width&&usize::from(u16::from_be_bytes([self.params[2],self.params[3]]))==self.height,
            (0x90,9)=>{self.window=std::array::from_fn(|i|usize::from(u16::from_be_bytes([self.params[i*2],self.params[i*2+1]])));},_=>{}
        }
    }
    fn publish(&mut self) {
        let [x0,x1,y0,y1]=if self.partial {self.window}else{[0,self.width-1,0,self.height-1]};
        if x0>x1||y0>y1||x1>=self.width||y1>=self.height{return;}
        for y in y0..=y1 {for x in x0..=x1 {self.frame[y*self.width+x]=if self.ram[1][y*(self.width/8)+x/8]&(0x80>>(x&7))!=0{0xffff}else{0};}}
        self.frames+=1;self.version+=1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(p:&mut Uc8179,c:u8,d:&[u8]) {p.write(false,&[c]);p.write(true,d);}
    fn power(p:&mut Uc8179) {send(p,0x61,&[3,32,1,224]);send(p,0x04,&[]);p.advance_to(p.cycle+1);}
    #[test]
    fn seeed_583_uses_81_byte_rows_and_rejects_75_inch_resolution() {
        let mut p=Uc8179::seeed_583(4,1);power(&mut p);send(&mut p,0x13,&[0]);send(&mut p,0x12,&[]);p.advance_to(21);assert_eq!(p.frames,0);
        send(&mut p,0x61,&[2,136,1,224]);send(&mut p,0x13,&[255;81]);p.write(true,&[0x7f]);send(&mut p,0x12,&[]);p.advance_to(41);
        assert_eq!(p.frame.len(),648*480);assert_eq!(p.frame[647],0xffff);assert_eq!(&p.frame[648..650],&[0,0xffff]);assert_eq!(p.ram[1].len(),38_880);
    }
    #[test]
    fn new_plane_and_busy_completion_drive_pixels_old_plane_does_not() {
        let mut p=Uc8179::new(13,1);power(&mut p);send(&mut p,0x10,&[0]);send(&mut p,0x12,&[]);p.advance_to(21);assert_eq!(p.frame[0],0xffff);
        send(&mut p,0x13,&[0x7f]);assert_eq!(p.frame[0],0xffff);send(&mut p,0x12,&[]);assert!(!p.busy_high());p.advance_to(40);assert_eq!(p.frame[0],0xffff);p.advance_to(41);assert_eq!(&p.frame[..2],&[0,0xffff]);assert!(p.busy_high());
        p.reset();assert_eq!(p.frame[0],0,"bistable optical image survives reset");
    }
    #[test]
    fn partial_window_preserves_outside_pixels_and_rejects_invalid_ranges() {
        let mut p=Uc8179::new(13,1);power(&mut p);send(&mut p,0x91,&[]);send(&mut p,0x90,&[0,8,0,15,0,2,0,2,1]);send(&mut p,0x13,&[0,0]);send(&mut p,0x12,&[]);p.advance_to(21);
        assert_eq!(p.ram_bytes_written,1);assert_eq!(p.frame[1607],0xffff);assert_eq!(&p.frame[1608..1616],&[0;8]);assert_eq!(p.frame[1616],0xffff);
        send(&mut p,0x90,&[255;9]);send(&mut p,0x13,&[0;20]);send(&mut p,0x12,&[]);p.advance_to(41);assert_eq!(p.frames,1);assert_eq!(p.ram_bytes_written,1);
    }
    #[test]
    fn sleep_power_and_bad_resolution_do_not_publish() {
        let mut p=Uc8179::new(13,1);send(&mut p,0x04,&[]);p.advance_to(1);send(&mut p,0x61,&[255;4]);send(&mut p,0x13,&vec![0;50000]);assert_eq!(p.ram_bytes_written,48000);send(&mut p,0x12,&[]);p.advance_to(100);assert_eq!(p.frames,0);
        send(&mut p,0x07,&[0xa5]);power(&mut p);send(&mut p,0x12,&[]);p.advance_to(200);assert_eq!(p.frames,0);
    }
    #[test]
    fn reset_reinitialization_keeps_written_ram_but_power_loss_does_not() {
        let mut p=Uc8179::new(13,1);power(&mut p);send(&mut p,0x13,&[0x7f]);
        p.reset();power(&mut p);send(&mut p,0x12,&[]);p.advance_to(p.cycle+20);
        assert_eq!(&p.frame[..2],&[0,0xffff]);assert_eq!(p.ram_bytes_written,1);
        p.power_loss();assert_eq!(p.frame[0],0);power(&mut p);send(&mut p,0x12,&[]);p.advance_to(p.cycle+20);
        assert_eq!(&p.frame[..2],&[0xffff;2]);assert_eq!(p.frames,2);
    }
}
