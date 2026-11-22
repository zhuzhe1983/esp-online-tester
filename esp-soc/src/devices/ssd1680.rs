//! SSD1680 digital monochrome subset for the sourced DEPG0213BN glass.
//! The controller has two 176x296 RAM planes; only 122x250 pixels are visible.
//! RAM writes, a frozen pending update, BUSY and retained optical pixels differ.
//! Source receipts and exact timing conventions: lilygo-ssd1680-controller-2026-11-22.json.
//! Analog LUTs, temperature, red glass, ghosting and physical refresh validation
//! are outside this endpoint. Callers must gate the real SPI/DC/reset/power pins.
use crate::board::{BoardEdge,VirtualCycle};

const ROW_BYTES:usize=22;
const ROWS:usize=296;
const WIDTH:usize=122;
const HEIGHT:usize=250;
struct Pending {at:VirtualCycle,image:Option<Vec<u16>>}
pub struct Ssd1680 {
    pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,
    ram:[Vec<u8>;2],command:u8,parameters:[u8;4],count:usize,entry:u8,
    xs:u8,xe:u8,ys:u16,ye:u16,x:u8,y:u16,x_window:bool,y_window:bool,
    control1:u8,control2:u8,clock:bool,analog:bool,asleep:bool,
    cycle:VirtualCycle,pending:Option<Pending>,edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,timing_ms:[u64;4],
}
impl Ssd1680 {
    pub fn new(busy_pin:u8,cpu_hz:u64)->Self {
        Self{frame:vec![0xffff;WIDTH*HEIGHT],frames:0,version:0,ram_bytes_written:0,
            ram:[vec![255;ROW_BYTES*ROWS],vec![0;ROW_BYTES*ROWS]],command:0,parameters:[0;4],count:0,entry:3,
            xs:0,xe:21,ys:0,ye:295,x:0,y:0,x_window:true,y_window:true,
            control1:0,control2:255,clock:false,analog:false,asleep:false,cycle:0,pending:None,edges:Vec::new(),busy_pin,cycles_per_ms:(cpu_hz/1000).max(1),timing_ms:[4100,750,100,250]}
    }
    /// Explicit source-driver delay conventions, not measured optical timing.
    pub fn set_timing_ms(&mut self,full:u64,partial:u64,power_on:u64,power_off:u64) {self.timing_ms=[full.max(1),partial.max(1),power_on.max(1),power_off.max(1)];}
    pub fn dimensions(&self)->(u32,u32) {(WIDTH as u32,HEIGHT as u32)}
    pub fn busy(&self)->bool {self.asleep||self.pending.is_some()}
    pub fn next_deadline(&self)->Option<VirtualCycle> {self.pending.as_ref().map(|p|p.at)}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    pub fn reset(&mut self,clear_ram:bool) {
        let was_busy=self.busy();self.command=0;self.count=0;self.entry=3;self.xs=0;self.xe=21;self.ys=0;self.ye=295;self.x=0;self.y=0;
        self.x_window=true;self.y_window=true;self.control1=0;self.control2=255;self.clock=false;self.analog=false;self.asleep=false;self.pending=None;
        if clear_ram {self.ram[0].fill(255);self.ram[1].fill(0);}
        if was_busy {self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}
        // An unpowered/reset e-paper panel retains its optical image.
    }
    fn wait(&mut self,milliseconds:u64,image:Option<Vec<u16>>) {
        if !self.busy() {self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}
        self.pending=Some(Pending{at:self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(milliseconds)),image});
    }
    pub fn advance_to(&mut self,cycle:VirtualCycle) {
        self.cycle=self.cycle.max(cycle);
        if self.pending.as_ref().is_some_and(|p|p.at<=self.cycle) {
            let pending=self.pending.take().expect("due SSD1680 operation");
            if let Some(image)=pending.image {self.frame=image;self.frames=self.frames.wrapping_add(1);self.version=self.version.wrapping_add(1);}
            self.edges.push(BoardEdge{cycle:pending.at,pin:self.busy_pin,level:false});
        }
    }
    fn window_valid(&self)->bool {
        self.x_window&&self.y_window&&usize::from(self.xs)<ROW_BYTES&&usize::from(self.xe)<ROW_BYTES&&usize::from(self.ys)<ROWS&&usize::from(self.ye)<ROWS
            &&(if self.entry&1!=0 {self.xs<=self.xe}else{self.xs>=self.xe})&&(if self.entry&2!=0 {self.ys<=self.ye}else{self.ys>=self.ye})
    }
    fn image(&self,partial:bool)->Vec<u16> {
        let mut image=self.frame.clone();let (xa,xb)=(usize::from(self.xs.min(self.xe))*8,usize::from(self.xs.max(self.xe))*8+7);
        let (ya,yb)=(usize::from(self.ys.min(self.ye)),usize::from(self.ys.max(self.ye)));
        for y in 0..HEIGHT {for x in 0..WIDTH {if partial&&(x<xa||x>xb||y<ya||y>yb){continue;}
            let mut white=self.ram[0][y*ROW_BYTES+x/8]&(0x80>>(x%8))!=0;
            match self.control1&15 {4=>white=false,8=>white= !white,_=>{}}
            image[y*WIDTH+x]=if white{0xffff}else{0};
        }}image
    }
    fn activate(&mut self) {
        if self.control2&0x80!=0 {self.clock=true;}
        if self.control2&0x40!=0 {self.analog=true;}
        let partial=matches!(self.control2,0xcc|0xcf);
        let refresh=self.control2&4!=0&&self.clock&&self.analog;
        if refresh&&partial&&!self.window_valid() {return;}
        let image=refresh.then(||self.image(partial));
        let ms=self.timing_ms[if refresh {usize::from(partial)}else if self.control2&3!=0 {3}else{2}];
        self.wait(ms,image);
        if self.control2&2!=0 {self.analog=false;}
        if self.control2&1!=0 {self.clock=false;}
    }
    pub fn write(&mut self,data:bool,bytes:&[u8]) {
        for &byte in bytes {
            // The datasheet forbids interrupting an active update. A hardware
            // reset is a separate board event; SPI cannot escape deep sleep.
            if self.busy() {continue;}
            if data {self.data(byte);}else {
                self.command=byte;self.count=0;
                match byte {0x12=>{self.reset(false);self.wait(10,None);},0x20=>self.activate(),0x44=>self.x_window=false,0x45=>self.y_window=false,_=>{}}
            }
        }
    }
    fn next_x(&mut self)->bool {if self.x==self.xe {self.x=self.xs;true}else {self.x=if self.entry&1!=0{self.x.wrapping_add(1)}else{self.x.wrapping_sub(1)};false}}
    fn next_y(&mut self)->bool {if self.y==self.ye {self.y=self.ys;true}else {self.y=if self.entry&2!=0{self.y.wrapping_add(1)}else{self.y.wrapping_sub(1)};false}}
    fn data(&mut self,byte:u8) {
        if matches!(self.command,0x24|0x26) {
            if !self.window_valid()||self.x<self.xs.min(self.xe)||self.x>self.xs.max(self.xe)||self.y<self.ys.min(self.ye)||self.y>self.ys.max(self.ye) {return;}
            self.ram[usize::from(self.command==0x26)][usize::from(self.y)*ROW_BYTES+usize::from(self.x)]=byte;self.ram_bytes_written+=1;
            if self.entry&4==0 {if self.next_x(){self.next_y();}}else if self.next_y(){self.next_x();}return;
        }
        if self.count<4 {self.parameters[self.count]=byte;}self.count=self.count.saturating_add(1);
        let word=|i:usize|u16::from_le_bytes([self.parameters[i],self.parameters[i+1]]);
        match (self.command,self.count) {
            (0x10,1) if matches!(byte&3,1|3)=>{self.asleep=true;self.clock=false;self.analog=false;self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}
            (0x11,1)=>self.entry=byte&7,(0x21,1)=>self.control1=byte,(0x22,1)=>self.control2=byte,
            (0x44,2)=>{self.xs=self.parameters[0];self.xe=byte;self.x_window=true;}
            (0x45,4)=>{self.ys=word(0);self.ye=word(2);self.y_window=true;}
            (0x4e,1)=>self.x=byte,(0x4f,2)=>self.y=word(0),_=>{}
        }
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn send(p:&mut Ssd1680,c:u8,d:&[u8]) {p.write(false,&[c]);p.write(true,d);}
    fn area(p:&mut Ssd1680,x0:u8,x1:u8,y0:u16,y1:u16) {send(p,0x44,&[x0,x1]);let mut y=y0.to_le_bytes().to_vec();y.extend(y1.to_le_bytes());send(p,0x45,&y);send(p,0x4e,&[x0]);send(p,0x4f,&y0.to_le_bytes());}
    fn finish(p:&mut Ssd1680) {p.advance_to(p.next_deadline().expect("SSD1680 operation"));}
    #[test] fn native_ram_crop_and_frozen_optical_refresh() {
        let mut p=Ssd1680::new(4,1000);area(&mut p,15,15,249,249);send(&mut p,0x24,&[0x3f]);assert!(p.frame.iter().all(|&v|v==0xffff));
        send(&mut p,0x22,&[0xf4]);send(&mut p,0x20,&[]);assert_eq!(p.next_deadline(),Some(4100));assert!(p.busy());
        send(&mut p,0x24,&[255]);p.advance_to(4099);assert_eq!(p.frames,0);finish(&mut p);
        assert_eq!(p.dimensions(),(122,250));assert_eq!(&p.frame[p.frame.len()-2..],&[0,0]);assert_eq!(p.frame.iter().filter(|&&v|v==0).count(),2);
        assert_eq!(p.frames,1);assert_eq!(p.take_edges(),[BoardEdge{cycle:0,pin:4,level:true},BoardEdge{cycle:4100,pin:4,level:false}]);
        // The remaining six bits and controller columns 16..21 are real RAM,
        // but never wrap/alias onto the glass.
        area(&mut p,21,21,295,295);send(&mut p,0x24,&[0]);send(&mut p,0x20,&[]);finish(&mut p);assert_eq!(p.frame.iter().filter(|&&v|v==0).count(),2);
    }
    #[test] fn reverse_and_y_major_windows_are_bounded() {
        let mut p=Ssd1680::new(4,1000);send(&mut p,0x11,&[0]);area(&mut p,1,0,1,0);send(&mut p,0x24,&[1,2,3,4]);assert_eq!([p.ram[0][23],p.ram[0][22],p.ram[0][1],p.ram[0][0]],[1,2,3,4]);
        send(&mut p,0x11,&[7]);area(&mut p,0,1,0,1);send(&mut p,0x24,&[5,6,7,8]);assert_eq!([p.ram[0][0],p.ram[0][22],p.ram[0][1],p.ram[0][23]],[5,6,7,8]);
        for(x0,x1,y0,y1)in[(22,22,0,0),(0,21,296,296),(0,1,5,4),(2,1,0,0)] {area(&mut p,x0,x1,y0,y1);let before=p.ram_bytes_written;send(&mut p,0x24,&[0;128]);assert_eq!(p.ram_bytes_written,before);}
        send(&mut p,0x44,&[0]);send(&mut p,0x24,&[0]);assert!(!p.x_window);
    }
    #[test] fn previous_ram_power_sleep_and_reset_do_not_publish_frames() {
        let mut p=Ssd1680::new(4,1000);area(&mut p,0,0,0,0);send(&mut p,0x26,&[0]);send(&mut p,0x22,&[0xc0]);send(&mut p,0x20,&[]);finish(&mut p);assert_eq!(p.frames,0);
        send(&mut p,0x22,&[0xf4]);send(&mut p,0x20,&[]);finish(&mut p);assert!(p.frame.iter().all(|&v|v==0xffff));
        area(&mut p,0,0,0,0);send(&mut p,0x24,&[0x7f]);send(&mut p,0x22,&[0xcc]);send(&mut p,0x20,&[]);let due=p.next_deadline().unwrap();assert_eq!(due-p.cycle,750);finish(&mut p);assert_eq!(p.frame[0],0);
        send(&mut p,0x10,&[1]);assert!(p.busy());assert_eq!(p.next_deadline(),None);send(&mut p,0x12,&[]);assert!(p.asleep);p.reset(false);assert!(!p.busy());assert_eq!(p.frame[0],0);
        send(&mut p,0x20,&[]);p.reset(true);assert_eq!(p.next_deadline(),None);assert_eq!(p.frame[0],0);assert!(p.ram[0].iter().all(|&v|v==255));
    }
    #[test] fn arbitrary_parameters_cannot_escape_controller_ram() {
        let mut p=Ssd1680::new(4,1000);for c in 0..=255 {p.reset(false);send(&mut p,c,&[255;512]);p.advance_to(p.cycle+10000);}
        assert_eq!(p.frame.len(),122*250);assert_eq!(p.ram[0].len(),22*296);
    }
}
