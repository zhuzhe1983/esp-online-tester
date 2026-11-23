//! Digital endpoints for two exact manufacturer examples, not interchangeable
//! names for every T5 panel. Source-backed nominal gray levels are decoded from
//! the GDEW0213T5 example's two planes; custom analog LUT effects are unmodeled.
#[derive(Clone,Copy,Eq,PartialEq)] pub enum Kind {Gray213,Il91874}
#[derive(Clone,Copy)] struct Area {x:usize,y:usize,bytes:usize,rows:usize}
struct Pending {at:u64,image:Option<Vec<u16>>}
pub struct SerialPaper {kind:Kind,width:usize,height:usize,ram:[Vec<u8>;2],frame:Vec<u16>,command:u8,parameters:[u8;8],count:usize,cursor:usize,area:Option<Area>,configured:bool,resolution:bool,powered:bool,asleep:bool,pending:Option<Pending>,cpu_hz:u64,pub frames:u64,pub ram_bytes:u64}
impl SerialPaper {
    pub fn new(kind:Kind,cpu_hz:u64)->Self {let(w,h)=if kind==Kind::Gray213{(104,212)}else{(176,264)};Self{kind,width:w,height:h,ram:[vec![255;w*h/8],vec![255;w*h/8]],frame:vec![0xffff;w*h],command:0xff,parameters:[0;8],count:0,cursor:0,area:None,configured:false,resolution:kind==Kind::Il91874,powered:false,asleep:false,pending:None,cpu_hz,frames:0,ram_bytes:0}}
    pub fn dimensions(&self)->(u32,u32) {(self.width as u32,self.height as u32)}
    pub fn pixels(&self)->Vec<u16> {self.frame.clone()}
    pub fn busy(&self)->bool {self.pending.is_some()}
    pub fn next_deadline(&self)->Option<u64> {self.pending.as_ref().map(|p|p.at)}
    pub fn reset(&mut self,clear_ram:bool) {self.command=0xff;self.count=0;self.cursor=0;self.area=None;self.configured=false;self.resolution=self.kind==Kind::Il91874;self.powered=false;self.asleep=false;self.pending=None;if clear_ram{self.ram[0].fill(255);self.ram[1].fill(255);}}
    pub fn advance_to(&mut self,cycle:u64) {if self.pending.as_ref().is_some_and(|p|p.at<=cycle){if let Some(p)=self.pending.take(){if let Some(image)=p.image{self.frame=image;self.frames+=1;}}}}
    fn wait(&mut self,cycle:u64,ms:u64,image:Option<Vec<u16>>) {self.pending=Some(Pending{at:cycle.saturating_add((self.cpu_hz/1000).saturating_mul(ms).max(1)),image});}
    fn full(&self)->Area {Area{x:0,y:0,bytes:self.width/8,rows:self.height}}
    fn area(&self)->Option<Area> {
        let p=self.parameters;let x=usize::from(u16::from_be_bytes([p[0],p[1]&0xf8]));let y=usize::from(u16::from_be_bytes([p[2],p[3]]));let w=usize::from(u16::from_be_bytes([p[4],p[5]&0xf8]));let h=usize::from(u16::from_be_bytes([p[6],p[7]]));
        (w>0&&h>0&&x+w<=self.width&&y+h<=self.height).then_some(Area{x:x/8,y,bytes:w/8,rows:h})
    }
    fn refresh(&mut self,cycle:u64,area:Area) {
        if !self.powered||!self.configured||!self.resolution{return;}let mut image=self.frame.clone();
        for y in area.y..area.y+area.rows {for x in area.x*8..(area.x+area.bytes)*8 {let i=y*self.width/8+x/8;let mask=0x80>>(x%8);
            let value=if self.kind==Kind::Gray213 {u16::from(self.ram[0][i]&mask!=0)*2+u16::from(self.ram[1][i]&mask!=0)}else if self.ram[1][i]&mask!=0{3}else{0};
            // Four nominal debug tones. Their optical reflectance is unknown.
            image[y*self.width+x]=[0x0000,0x52aa,0xad55,0xffff][usize::from(value)];
        }}
        self.wait(cycle,if self.kind==Kind::Gray213{3000}else{6000},Some(image));
    }
    pub fn write(&mut self,cycle:u64,dc:bool,bytes:&[u8]) {
        for &byte in bytes {if self.asleep||self.busy(){continue;}
            if dc {self.data(cycle,byte);}else {self.command=byte;self.count=0;self.cursor=0;
            match byte {
                0x02=>{self.powered=false;self.wait(cycle,50,None);},0x04=>{self.powered=true;self.wait(cycle,50,None);},0x12=>self.refresh(cycle,self.full()),
                0x10|0x13=>self.area=Some(self.full()),0x14..=0x16 if self.kind==Kind::Il91874=>self.area=None,
                0x61=>self.resolution=false,
                0x00|0x01|0x03|0x06|0x07|0x11|0x20..=0x25|0x30|0x50|0x60|0x71|0x82|0xf8=>{},
                _=>self.command=0xff,
            }
        }}
    }
    fn data(&mut self,cycle:u64,byte:u8) {
        let full_ram=matches!(self.command,0x10|0x13);let partial_ram=self.kind==Kind::Il91874&&matches!(self.command,0x14|0x15)&&self.count>=8;
        if full_ram||partial_ram {
            if !self.configured||!self.resolution{return;}
            if let Some(a)=self.area {if self.cursor<a.bytes*a.rows {let i=(a.y+self.cursor/a.bytes)*(self.width/8)+a.x+self.cursor%a.bytes;let plane=usize::from(matches!(self.command,0x13|0x15));self.ram[plane][i]=byte;self.cursor+=1;self.ram_bytes+=1;}}return;
        }
        if self.count<8{self.parameters[self.count]=byte;}self.count=self.count.saturating_add(1);
        match(self.command,self.count) {
            (0x00,1)=>self.configured=if self.kind==Kind::Gray213{byte==0x3f}else{matches!(byte,0xaf|0xbf)},
            (0x61,3) if self.kind==Kind::Gray213=>self.resolution=self.parameters[..3]==[104,0,212],
            (0x61,4) if self.kind==Kind::Il91874=>self.resolution=self.parameters[..4]==[0,176,1,8],
            (0x07,1) if byte==0xa5=>{self.asleep=true;self.powered=false;},
            (0x14|0x15,8) if self.kind==Kind::Il91874=>self.area=self.area(),
            (0x16,8) if self.kind==Kind::Il91874=>{if let Some(area)=self.area(){self.refresh(cycle,area);}},_=>{}
        }
    }
}
