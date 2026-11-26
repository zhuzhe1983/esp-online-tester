//! K115 Atom-PSRAM (PICO-V3-02, 8MiB/2MiB), NOT an on-board LCD.
//! M5GFX M5AtomDisplay.h and Panel_M5HDMI.{hpp,cpp}: SPI CLK5/MOSI19/
//! MISO22/CS33, LT8618SX I2C SDA25/SCL21 addr0x39. This captures the
//! FPGA's digital RGB memory for an external monitor; it does not synthesize
//! TMDS, JTAG configuration, EDID, analog PLL lock or a physical HDMI sink.
use std::{collections::VecDeque,sync::{Arc,Mutex}};
use esp_periph::i2c::I2cDevice;
use esp_soc::{BoardModel,board::SpiPins,devices::{Ws2812Chain,i2c_bitbang::I2cBitBang}};
struct Transmitter {page:u8,regs:Vec<u8>,writes:u64}
impl Default for Transmitter {fn default()->Self {Self{page:0,regs:vec![0;65536],writes:0}}}
struct Lt8618 {shared:Arc<Mutex<Transmitter>>,ptr:u8,first:bool}
impl I2cDevice for Lt8618 {
    fn start(&mut self,read:bool)->bool {if !read {self.first=true;}true}
    fn write(&mut self,b:u8)->bool {if self.first {self.ptr=b;self.first=false;}else{let mut s=self.shared.lock().expect("LT8618 state poisoned");if self.ptr==255 {s.page=b;}else{let i=(usize::from(s.page)<<8)|usize::from(self.ptr);s.regs[i]=b;s.writes+=1;}self.ptr=self.ptr.wrapping_add(1);}true}
    fn read(&mut self)->u8 {let s=self.shared.lock().expect("LT8618 state poisoned");let v=if self.ptr==255{s.page}else{s.regs[(usize::from(s.page)<<8)|usize::from(self.ptr)]};self.ptr=self.ptr.wrapping_add(1);v}
}
pub struct AtomPsramDisplay {
    rgb:Ws2812Chain,transmitter:Arc<Mutex<Transmitter>>,pixels:Vec<u16>,w:u16,h:u16,
    window:[u16;4],cursor:[u16;2],packet:Vec<u8>,need:usize,raw:u8,color:[u8;3],
    response:VecDeque<u8>,version:u64,writes:u64,rejected:u64,cs_high:bool,
}
impl Default for AtomPsramDisplay {fn default()->Self {Self::new()}}
impl AtomPsramDisplay {
    pub fn new()->Self {Self{rgb:Ws2812Chain::new(1),transmitter:Default::default(),pixels:vec![0;1280*720],w:1280,h:720,window:[0,1279,0,719],cursor:[0,0],packet:Vec::new(),need:0,raw:0,color:[0;3],response:VecDeque::new(),version:0,writes:0,rejected:0,cs_high:true}}
    fn reset_transaction(&mut self) {self.packet.clear();self.need=0;self.raw=0;self.response.clear();}
    fn pixel(&mut self,x:u16,y:u16,c:[u8;3],alpha:u8) {
        if x>=self.w || y>=self.h {return;}let i=usize::from(y)*usize::from(self.w)+usize::from(x);
        let old=self.pixels[i];let rgb=[(((old>>11)&31)*255/31)as u8,(((old>>5)&63)*255/63)as u8,((old&31)*255/31)as u8];
        let v:[u16;3]=std::array::from_fn(|i|(u16::from(c[i])*u16::from(alpha)+u16::from(rgb[i])*u16::from(255-alpha)+127)/255);
        self.pixels[i]=((v[0]>>3)<<11)|((v[1]>>2)<<5)|(v[2]>>3);self.writes+=1;self.version=self.version.wrapping_add(1);
    }
    fn decode(&mut self,bytes:&[u8])->([u8;3],u8) {let (c,a)=match bytes.len() {
        1=>{let v=bytes[0];([(u16::from(v>>5)*255/7)as u8,(u16::from((v>>2)&7)*255/7)as u8,(u16::from(v&3)*255/3)as u8],255)},
        2=>{let v=u16::from_be_bytes([bytes[0],bytes[1]]);([(((v>>11)&31)*255/31)as u8,(((v>>5)&63)*255/63)as u8,((v&31)*255/31)as u8],255)},
        3=>([bytes[0],bytes[1],bytes[2]],255),4=>([bytes[1],bytes[2],bytes[3]],bytes[0]),_=>(self.color,255)
    };self.color=c;(c,a)}
    fn advance_cursor(&mut self) {if self.cursor[0]>=self.window[1] {self.cursor[0]=self.window[0];self.cursor[1]=if self.cursor[1]>=self.window[3]{self.window[2]}else{self.cursor[1]+1};}else{self.cursor[0]+=1;}}
    fn command_len(command:u8)->usize {match command {0=>1,4=>1,0x18=>8,0x19=>6,0x23=>13,0x2a|0x2b=>5,0x41..=0x45|0x81..=0x83=>1,0x60..=0x64=>5+usize::from(command&7),0x68..=0x6c=>9+usize::from(command&7),0xb0|0xb1=>10,0xb2=>7,_=>0}}
    fn execute(&mut self) {
        let p=std::mem::take(&mut self.packet);let command=p[0];let u16at=|i:usize|u16::from_be_bytes([p[i],p[i+1]]);
        match command {
            0=>{},4=>self.response.extend([0,255,b'H',b'D',1,0]),
            0x2a=>{self.window[0]=u16at(1);self.window[1]=u16at(3);self.cursor[0]=self.window[0];},
            0x2b=>{self.window[2]=u16at(1);self.window[3]=u16at(3);self.cursor[1]=self.window[2];},
            0x41..=0x45=>{self.raw=command&7;self.cursor=[self.window[0],self.window[2]];},
            0x18=>{if p.iter().fold(0u8,|v,b|v.wrapping_add(*b))==255 {let(w,h)=(u16at(3),u16at(5));if w>0&&w<=1280&&h>0&&h<=720 {self.w=w;self.h=h;self.pixels.resize(usize::from(w)*usize::from(h),0);self.version=self.version.wrapping_add(1);}}else{self.rejected+=1;}},
            0x19|0xb0..=0xb2=>{if p.iter().fold(0u8,|v,b|v.wrapping_add(*b))!=255 {self.rejected+=1;}},
            0x60..=0x64=>{let(x,y)=(u16at(1),u16at(3));let(c,a)=self.decode(&p[5..]);self.pixel(x,y,c,a);},
            0x68..=0x6c=>{let(x0,y0,x1,y1)=(u16at(1),u16at(3),u16at(5),u16at(7));let(c,a)=self.decode(&p[9..]);for y in y0..=y1.min(self.h-1) {for x in x0..=x1.min(self.w-1) {self.pixel(x,y,c,a);}}},
            0x23=>{let(x0,y0,x1,y1,dx,dy)=(u16at(1),u16at(3),u16at(5),u16at(7),u16at(9),u16at(11));let old=self.pixels.clone();for y in y0..=y1.min(self.h-1) {for x in x0..=x1.min(self.w-1) {let(nx,ny)=(u32::from(dx)+u32::from(x-x0),u32::from(dy)+u32::from(y-y0));if nx<u32::from(self.w)&&ny<u32::from(self.h) {let dst=ny as usize*usize::from(self.w)+nx as usize;self.pixels[dst]=old[usize::from(y)*usize::from(self.w)+usize::from(x)];self.writes+=1;self.version=self.version.wrapping_add(1);}}}},
            0x81..=0x83=>{for y in self.window[2]..=self.window[3].min(self.h-1) {for x in self.window[0]..=self.window[1].min(self.w-1) {let v=self.pixels[usize::from(y)*usize::from(self.w)+usize::from(x)];match command {0x81=>self.response.push_back(((v>>8)&0xe0)as u8|((v>>6)&0x1c)as u8|((v>>3)&3)as u8),0x82=>self.response.extend(v.to_be_bytes()),_=>self.response.extend([(((v>>11)&31)*255/31)as u8,(((v>>5)&63)*255/63)as u8,((v&31)*255/31)as u8])}}}},_=>{}
        }
    }
    fn byte(&mut self,b:u8) {
        if self.raw!=0 {self.packet.push(b);let n=if self.raw==5 {1}else{usize::from(self.raw)};if self.packet.len()==n {let bytes=std::mem::take(&mut self.packet);let(c,a)=if self.raw==5{(self.color,bytes[0])}else{self.decode(&bytes)};self.pixel(self.cursor[0],self.cursor[1],c,a);self.advance_cursor();}return;}
        if self.packet.is_empty() {self.need=Self::command_len(b);if self.need==0 {self.rejected+=1;return;}}
        self.packet.push(b);if self.packet.len()==self.need {self.execute();}
    }
}
impl BoardModel for AtomPsramDisplay {
    fn name(&self)->&'static str {"atom-psram-display"}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {for &(p,v)in changes {if p==33 {if v&&!self.cs_high {self.reset_transaction();}self.cs_high=v;}}}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,_host:u8,p:SpiPins,tx:&[u8],rx:usize)->Vec<u8> {
        if p.sclk&(1<<5)==0 || p.mosi&(1<<19)==0 || p.cs&(1<<33)==0 {return vec![255;rx];}
        if !self.response.is_empty()&&tx.iter().all(|b|*b==0) {return(0..rx).map(|_|if p.miso==Some(22){self.response.pop_front().unwrap_or(255)}else{255}).collect();}
        for &b in tx {self.byte(b);}vec![255;rx]
    }
    fn rmt_frame(&mut self,pin:u8,bits:&[bool]) {if pin==27 {self.rgb.from_bits(bits);}}
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(39,true),(25,true),(21,true)]}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button"|"btnA"=>Some(39),_=>None}}
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {let mut b=I2cBitBang::new();b.attach(0x39,Box::new(Lt8618{shared:self.transmitter.clone(),ptr:0,first:true}));(0..2).flat_map(|bus|b.controller_devices(Some((25,21))).into_iter().map(move|(a,d)|(bus,a,d))).collect()}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {Some((u32::from(self.w),u32::from(self.h),self.pixels.clone(),self.version))}
    fn display_version(&self)->u64 {self.version}
    fn hardware_diagnostics(&self)->Option<String> {Some(format!("{{\"output\":\"external-hdmi-fpga-memory\",\"width\":{},\"height\":{},\"pixelWrites\":{},\"unsupported\":{},\"lt8618Writes\":{},\"physicalTmds\":false}}",self.w,self.h,self.writes,self.rejected,self.transmitter.lock().expect("LT8618 state poisoned").writes))}
    fn report(&self)->String {format!("[atom-psram-display] external FPGA digital framebuffer {}x{} writes={} rejected={}; LT8618 register transport only, no fabricated ID/PLL-lock/EDID; JTAG/TMDS/audio unmodeled",self.w,self.h,self.writes,self.rejected)}
}
