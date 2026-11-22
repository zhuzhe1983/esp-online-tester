//! Source-backed UC8175 / UC8253 serial e-paper digital protocol.
//! Extracted without protocol/timing changes from the LilyGO S3 board adapter.
//! Clock rate is supplied by the owning SoC; RAM writes, retained optical image,
//! busy completion and refresh are separate. No analog waveform or ghosting.
//! Sources: docs/platform/research/lilygo-epaper-profiles-2026-11-22.json.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Controller {Uc8253,Uc8175}
impl Controller {
    pub fn dimensions(self)->(usize,usize) {match self {Self::Uc8253=>(240,320),Self::Uc8175=>(80,128)}}
    fn window_bytes(self)->usize {match self {Self::Uc8253=>7,Self::Uc8175=>5}}
    fn refresh_ms(self,partial:bool)->u64 {match (self,partial) {(Self::Uc8253,false)=>1100,(Self::Uc8253,true)=>700,(Self::Uc8175,false)=>2000,(Self::Uc8175,true)=>700}}
}
#[derive(Clone,Copy)]
struct Window {x0:usize,x1:usize,y0:usize,y1:usize}
impl Window {fn bytes(self)->usize {(self.x1-self.x0+1)*(self.y1-self.y0+1)}}
struct Pending {at:u64,image:Option<Vec<u8>>}
pub struct Epaper {
    cpu_hz:u64,controller:Controller,current:Vec<u8>,previous:Vec<u8>,visible:Vec<u8>,
    command:Option<u8>,parameters:[u8;7],parameter_count:usize,cursor:usize,
    window:Option<Window>,partial:bool,configured:bool,powered:bool,sleeping:bool,
    pending:Option<Pending>,pub frames:u64,pub ram_bytes:u64,pub unsupported_commands:u64,resets:u64,
}
impl Epaper {
    pub fn new(controller:Controller,cpu_hz:u64)->Self {
        let (w,h)=controller.dimensions();let bytes=w*h/8;
        Self{cpu_hz,controller,current:vec![255;bytes],previous:vec![255;bytes],visible:vec![255;bytes],command:None,parameters:[0;7],parameter_count:0,cursor:0,window:Some(Window{x0:0,x1:w/8-1,y0:0,y1:h-1}),partial:false,configured:false,powered:false,sleeping:false,pending:None,frames:0,ram_bytes:0,unsupported_commands:0,resets:0}
    }
    pub fn reset(&mut self,clear_ram:bool) {
        let (w,h)=self.controller.dimensions();self.command=None;self.parameter_count=0;self.cursor=0;self.window=Some(Window{x0:0,x1:w/8-1,y0:0,y1:h-1});
        self.partial=false;self.configured=false;self.powered=false;self.sleeping=false;self.pending=None;self.resets+=1;
        if clear_ram {self.current.fill(255);self.previous.fill(255);}
        // An e-paper image persists without power. Reset never invents an optical refresh.
    }
    pub fn busy_high(&self)->bool {self.pending.is_none()}
    pub fn next_deadline(&self)->Option<u64> {self.pending.as_ref().map(|p|p.at)}
    pub fn advance_to(&mut self,cycle:u64)->Option<u64> {
        if self.pending.as_ref().is_some_and(|p|p.at<=cycle) {
            let pending=self.pending.take().expect("due e-paper operation");
            if let Some(image)=pending.image {self.visible=image;self.frames+=1;}
            Some(pending.at)
        } else {None}
    }
    fn start_busy(&mut self,cycle:u64,milliseconds:u64,image:Option<Vec<u8>>) {self.pending=Some(Pending{at:cycle.saturating_add((self.cpu_hz/1000).saturating_mul(milliseconds).max(1)),image});}
    fn area(&self)->Option<Window> {
        if self.partial {self.window} else {let (w,h)=self.controller.dimensions();Some(Window{x0:0,x1:w/8-1,y0:0,y1:h-1})}
    }
    pub fn command(&mut self,cycle:u64,command:u8) {
        if self.sleeping || self.pending.is_some() {return;}
        self.command=Some(command);self.parameter_count=0;self.cursor=0;
        match command {
            0x02=>{self.powered=false;self.start_busy(cycle,50,None);}
            0x04=>{self.powered=true;self.start_busy(cycle,50,None);}
            0x12=>{
                if !self.configured || !self.powered {return;}
                let Some(area)=self.area() else {return;};let (w,_)=self.controller.dimensions();
                let mut image=self.visible.clone();for y in area.y0..=area.y1 {let a=y*w/8+area.x0;let b=y*w/8+area.x1+1;image[a..b].copy_from_slice(&self.current[a..b]);}
                self.start_busy(cycle,self.controller.refresh_ms(self.partial),Some(image));
            }
            0x90=>self.window=None,
            0x91=>self.partial=true,
            0x92=>self.partial=false,
            0x00|0x07|0x10|0x13|0x50=>{},
            0xe0|0xe5 if self.controller==Controller::Uc8253=>{},
            // The Mini's official partial-update setup programs these analog
            // parameters. They do not implement optical waveforms in this model.
            0x01|0x06|0x23|0x24|0x2a|0x30|0x60|0x82|0xd2|0xe3 if self.controller==Controller::Uc8175=>{},
            _=>{self.unsupported_commands+=1;self.command=None;}
        }
    }
    pub fn data(&mut self,value:u8) {
        if self.sleeping || self.pending.is_some() {return;}
        let Some(command)=self.command else {return;};
        if command==0x10 || command==0x13 {
            if !self.configured {return;}
            if let Some(area)=self.area() {
                if self.cursor<area.bytes() {let (w,_)=self.controller.dimensions();let row_bytes=area.x1-area.x0+1;let index=(area.y0+self.cursor/row_bytes)*(w/8)+area.x0+self.cursor%row_bytes;
                    if command==0x13 {self.current[index]=value;}else{self.previous[index]=value;}self.ram_bytes+=1;self.cursor+=1;
                }
            }
            return;
        }
        if self.parameter_count<self.parameters.len() {self.parameters[self.parameter_count]=value;}
        self.parameter_count=self.parameter_count.saturating_add(1);
        match command {
            0x00=>{
                if self.controller==Controller::Uc8253 {
                    if self.parameter_count==1 && value==0x1e {self.reset(false);self.command=Some(0x00);self.parameters[0]=value;self.parameter_count=1;}
                    if self.parameter_count==2 {self.configured=self.parameters[..2]==[0x1f,0x0d];}
                } else if self.parameter_count==1 {self.configured=matches!(value,0x5f|0x6f);}
            }
            0x07 if self.parameter_count==1 && value==0xa5=>{self.sleeping=true;self.powered=false;}
            0x90 if self.parameter_count==self.controller.window_bytes()=>{
                let (w,h)=self.controller.dimensions();let p=self.parameters;
                let (y0,y1)=if self.controller==Controller::Uc8253 {(usize::from(u16::from_be_bytes([p[2],p[3]])),usize::from(u16::from_be_bytes([p[4],p[5]])))}else{(usize::from(p[2]),usize::from(p[3]))};
                let x0=usize::from(p[0])/8;let x1=usize::from(p[1])/8;
                self.window=(x0<=x1 && x1<w/8 && y0<=y1 && y1<h).then_some(Window{x0,x1,y0,y1});
            }
            _=>{}
        }
    }
    pub fn pixels(&self)->Vec<u16> {self.visible.iter().flat_map(|byte|(0..8).rev().map(move |bit|if byte&(1<<bit)==0 {0}else{0xffff})).collect()}
}
