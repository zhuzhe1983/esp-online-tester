//! Write-side SSD1306/SH1106 I2C digital display subset.
//! Solomon Systech SSD1306 rev1.1 (Apr2008), §§8.1.5,8.5,9,10.1;
//! Sino Wealth SH1106 v2.3, I2C Interface, Display RAM and Command Description.
//! See docs/platform/research/lilygo-oled-controller-2026-11-22.json for sources.
//! No oscillator/scroll timing, analog contrast/charge pump, or SH1106 readback.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::I2cDevice;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum OledKind {Ssd1306,Sh1106}
pub struct OledMono {
    kind:OledKind,cols:u16,width:u16,height:u16,offset:u16,ram:Vec<u8>,powered:bool,reset_high:bool,
    on:bool,invert:bool,all_on:bool,segment_remap:bool,com_reverse:bool,mux:u8,start_line:u8,display_offset:u8,
    mode:u8,column:u16,page:u8,column_start:u16,column_end:u16,page_start:u8,page_end:u8,
    column_valid:bool,page_valid:bool,mode_valid:bool,pending:u8,need:u8,used:u8,args:[u8;6],scroll:bool,
    version:u64,frames:u64,pub commands:u64,pub data_bytes:u64,pub rejected:u64,pub unsupported:u64,
}
impl OledMono {
    /// Width/height/offset describe physical glass, never guest RAM bounds.
    /// Oversized host geometry is clamped; all guest writes remain bounds checked.
    pub fn new(kind:OledKind,width:u16,height:u16,column_offset:u16)->Self {
        let cols=if kind==OledKind::Ssd1306 {128}else{132};let offset=column_offset.min(cols-1);
        let mut s=Self{kind,cols,width:width.clamp(1,cols-offset),height:height.clamp(1,64),offset,ram:vec![0;usize::from(cols)*8],powered:true,reset_high:true,
            on:false,invert:false,all_on:false,segment_remap:false,com_reverse:false,mux:64,start_line:0,display_offset:0,
            mode:2,column:0,page:0,column_start:0,column_end:cols-1,page_start:0,page_end:7,column_valid:true,page_valid:true,mode_valid:true,
            pending:0,need:0,used:0,args:[0;6],scroll:false,version:0,frames:0,commands:0,data_bytes:0,rejected:0,unsupported:0};
        s.reset_registers();s.version=0;s
    }
    pub fn dimensions(&self)->(u32,u32) {(u32::from(self.width),u32::from(self.height))}
    pub fn version(&self)->u64 {self.version}
    /// Completed I2C data transactions, not a fabricated refresh/VSYNC counter.
    pub fn frames(&self)->u64 {self.frames}
    fn changed(&mut self) {self.version=self.version.wrapping_add(1);}
    fn ready(&self)->bool {self.powered&&self.reset_high}
    fn reset_registers(&mut self) {
        self.on=false;self.invert=false;self.all_on=false;self.segment_remap=false;self.com_reverse=false;self.mux=64;self.start_line=0;self.display_offset=0;
        self.mode=2;self.column=0;self.page=0;self.column_start=0;self.column_end=self.cols-1;self.page_start=0;self.page_end=7;
        self.column_valid=true;self.page_valid=true;self.mode_valid=true;self.need=0;self.used=0;self.scroll=false;self.changed();
    }
    pub fn set_reset(&mut self,high:bool) {if self.reset_high!=high {self.reset_high=high;if !high {self.reset_registers();}else{self.changed();}}}
    /// Power loss resets settings and deterministically clears otherwise undefined
    /// SRAM. Hardware RES# preserves RAM. Neither operation invents an image.
    pub fn set_power(&mut self,powered:bool) {if self.powered!=powered {self.powered=powered;if !powered {self.ram.fill(0);self.reset_registers();}else{self.changed();}}}
    fn parameter_count(&self,c:u8)->u8 {
        match c {
            0x81|0xa8|0xd3|0xd5|0xd9|0xda|0xdb=>1,
            0x20|0x8d if self.kind==OledKind::Ssd1306=>1,
            0x21|0x22|0xa3 if self.kind==OledKind::Ssd1306=>2,
            0x26|0x27 if self.kind==OledKind::Ssd1306=>6,
            0x29|0x2a if self.kind==OledKind::Ssd1306=>5,
            0xad if self.kind==OledKind::Sh1106=>1,
            _=>0,
        }
    }
    fn command(&mut self,b:u8) {
        if self.need!=0 {self.args[usize::from(self.used)]=b;self.used+=1;if self.used==self.need {self.need=0;self.apply_parameters();}return;}
        self.commands=self.commands.wrapping_add(1);self.pending=b;self.used=0;self.need=self.parameter_count(b);if self.need!=0 {return;}
        match b {
            0x00..=0x0f=>{self.column=(self.column&0xf0)|u16::from(b&15);self.column_valid=self.column<self.cols;}
            0x10..=0x1f=>{self.column=(self.column&15)|(u16::from(b&15)<<4);self.column_valid=self.column<self.cols;}
            0x40..=0x7f=>{self.start_line=b&63;self.changed();}
            0xa0|0xa1=>{self.segment_remap=b==0xa1;self.changed();}
            0xa4|0xa5=>{self.all_on=b==0xa5;self.changed();}
            0xa6|0xa7=>{self.invert=b==0xa7;self.changed();}
            0xae|0xaf=>{self.on=b==0xaf;self.changed();}
            0xb0..=0xb7=>{self.page=b&7;self.page_valid=true;}
            0xc0|0xc8=>{self.com_reverse=b==0xc8;self.changed();}
            0x2e if self.kind==OledKind::Ssd1306=>self.scroll=false,
            0x2f if self.kind==OledKind::Ssd1306=>{self.scroll=true;self.unsupported=self.unsupported.wrapping_add(1);}
            0xe3=>{},
            0x30..=0x33 if self.kind==OledKind::Sh1106=>{}, // analog pump voltage ignored
            _=>self.unsupported=self.unsupported.wrapping_add(1),
        }
    }
    fn apply_parameters(&mut self) {
        let a=self.args;
        match self.pending {
            0x20=>{self.mode_valid=a[0]<=2;if self.mode_valid {self.mode=a[0];}else{self.rejected=self.rejected.wrapping_add(1);}}
            0x21=>{self.column_valid=a[0]<=a[1]&&u16::from(a[1])<self.cols;if self.column_valid {self.column_start=u16::from(a[0]);self.column_end=u16::from(a[1]);self.column=self.column_start;}else{self.rejected=self.rejected.wrapping_add(1);}}
            0x22=>{self.page_valid=a[0]<=a[1]&&a[1]<8;if self.page_valid {self.page_start=a[0];self.page_end=a[1];self.page=self.page_start;}else{self.rejected=self.rejected.wrapping_add(1);}}
            0xa8=>{if (15..64).contains(&a[0]) {self.mux=a[0]+1;self.changed();}else{self.rejected=self.rejected.wrapping_add(1);}}
            0xd3=>{if a[0]<64 {self.display_offset=a[0];self.changed();}else{self.rejected=self.rejected.wrapping_add(1);}}
            0x26|0x27|0x29|0x2a|0xa3=>self.unsupported=self.unsupported.wrapping_add(1),
            _=>{}, // contrast, charge-pump, clock and physical COM wiring are unmodeled
        }
    }
    fn data(&mut self,b:u8)->bool {
        if self.need!=0 || !self.mode_valid || !self.column_valid || !self.page_valid || self.column>=self.cols || self.page>=8 || self.scroll {
            self.rejected=self.rejected.wrapping_add(1);return false;
        }
        let i=usize::from(self.page)*usize::from(self.cols)+usize::from(self.column);
        if self.ram[i]!=b {self.ram[i]=b;self.changed();}self.data_bytes=self.data_bytes.wrapping_add(1);
        match self.mode {
            0=>{if self.column>=self.column_end {self.column=self.column_start;self.page=if self.page>=self.page_end{self.page_start}else{self.page+1};}else{self.column+=1;}}
            1=>{if self.page>=self.page_end {self.page=self.page_start;self.column=if self.column>=self.column_end{self.column_start}else{self.column+1};}else{self.page+=1;}}
            _=>{self.column+=1;if self.column>=self.cols {if self.kind==OledKind::Ssd1306 {self.column=0;}else{self.column_valid=false;}}}
        }true
    }
    pub fn frame(&self)->Vec<u16> {
        let mut out=vec![0;usize::from(self.width)*usize::from(self.height)];if !self.ready()||!self.on {return out;}
        for y in 0..self.height {
            let row=if self.com_reverse {(u16::from(self.mux)+63-y+u16::from(self.display_offset))&63}else{(y+u16::from(self.display_offset))&63};
            if row>=u16::from(self.mux) {continue;}let ram_y=(row+u16::from(self.start_line))&63;
            for x in 0..self.width {
                let seg=x+self.offset;let col=if self.segment_remap {self.cols-1-seg}else{seg};let byte=self.ram[usize::from(ram_y/8*self.cols+col)];
                let on=self.all_on||((byte&(1<<(ram_y%8))!=0)^self.invert);if on {out[usize::from(y*self.width+x)]=0xffff;}
            }
        }out
    }
    pub fn report(&self)->String {format!("{:?} {}x{} RAM={}x64 offset={} powered={} reset={} display={} writes={} transactions={} rejected={} unsupported={} scroll={} analog/readback=unmodeled",self.kind,self.width,self.height,self.cols,self.offset,self.powered,!self.reset_high,self.on,self.data_bytes,self.frames,self.rejected,self.unsupported,self.scroll)}
}

pub struct OledI2c {state:Arc<Mutex<OledMono>>,pins:(u8,u8),control:bool,data:bool,continuation:bool,active:bool,wrote:bool}
impl OledI2c {pub fn new(state:Arc<Mutex<OledMono>>,pins:(u8,u8))->Self {Self{state,pins,control:true,data:false,continuation:false,active:false,wrote:false}}}
impl I2cDevice for OledI2c {
    fn pins(&self)->Option<(u8,u8)> {Some(self.pins)}
    fn start(&mut self,read:bool)->bool {self.stop();self.active=!read&&self.state.lock().expect("OLED mutex poisoned").ready();self.control=true;self.active}
    fn write(&mut self,b:u8)->bool {
        let mut s=self.state.lock().expect("OLED mutex poisoned");if !self.active||!s.ready() {return false;}
        if self.control {if b&0x3f!=0 {self.active=false;s.rejected=s.rejected.wrapping_add(1);return false;}self.data=b&0x40!=0;self.continuation=b&0x80!=0;self.control=false;return true;}
        let accepted=if self.data {let ok=s.data(b);self.wrote|=ok;ok}else{s.command(b);true};self.control=self.continuation;accepted
    }
    fn read(&mut self)->u8 {0xff}
    fn stop(&mut self) {if self.wrote {let mut s=self.state.lock().expect("OLED mutex poisoned");s.frames=s.frames.wrapping_add(1);}self.wrote=false;self.active=false;self.control=true;}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rig(kind:OledKind)->(Arc<Mutex<OledMono>>,OledI2c) {let s=Arc::new(Mutex::new(OledMono::new(kind,128,64,if kind==OledKind::Sh1106{2}else{0})));let d=OledI2c::new(s.clone(),(18,17));(s,d)}
    fn send(d:&mut OledI2c,bytes:&[u8]) {assert!(d.start(false));for &b in bytes {assert!(d.write(b));}d.stop();}
    #[test]
    fn ssd_horizontal_vertical_and_page_modes_match_ram_pointer_contracts() {
        let (s,mut d)=rig(OledKind::Ssd1306);send(&mut d,&[0,0xaf,0x20,0,0x21,2,3,0x22,1,2]);send(&mut d,&[0x40,1,2,4,8]);let p=s.lock().unwrap();assert_eq!(p.ram[130..132],[1,2]);assert_eq!(p.ram[258..260],[4,8]);assert_eq!((p.column,p.page),(2,1));drop(p);
        send(&mut d,&[0,0x20,1,0x21,5,6,0x22,0,1]);send(&mut d,&[0x40,0x10,0x20,0x40,0x80]);let p=s.lock().unwrap();assert_eq!((p.ram[5],p.ram[133],p.ram[6],p.ram[134]),(16,32,64,128));drop(p);
        send(&mut d,&[0,0x20,2,0xb7,0x0f,0x17]);send(&mut d,&[0x40,0x12,0x34]);let p=s.lock().unwrap();assert_eq!((p.ram[1023],p.ram[896],p.page,p.column),(0x12,0x34,7,1));
    }
    #[test]
    fn sh1106_has_132_columns_page_only_and_offset_two_visible_glass() {
        let (s,mut d)=rig(OledKind::Sh1106);send(&mut d,&[0,0xaf,0xb0,0,0x10]);send(&mut d,&[0x40,255,255,1]);let f=s.lock().unwrap().frame();assert_eq!(f[0],0xffff);assert_eq!(f[1],0);assert_eq!(f.iter().filter(|&&p|p!=0).count(),1);
        send(&mut d,&[0,0xb7,3,0x18]);send(&mut d,&[0x40,7]);assert!(d.start(false));d.write(0x40);assert!(!d.write(9));d.stop();let p=s.lock().unwrap();assert_eq!((p.ram[1055],p.ram[924],p.column,p.page),(7,0,132,7));drop(p);
        send(&mut d,&[0,0x20,0x21,0x22]);assert_eq!(s.lock().unwrap().unsupported,3,"SSD window commands are not SH commands");
    }
    #[test]
    fn control_continuations_parameters_across_stops_and_write_only_addressing() {
        let (s,mut d)=rig(OledKind::Ssd1306);send(&mut d,&[0x80,0xaf,0x80,0x20,0,0]);for b in [0x21,2,2,0x22,0,0] {send(&mut d,&[0x80,b]);}send(&mut d,&[0xc0,1,0x40,2]);assert_eq!(s.lock().unwrap().ram[2],2);assert_eq!(s.lock().unwrap().frames(),1);assert_eq!(d.pins(),Some((18,17)));assert!(!d.start(true));assert_eq!(d.read(),255);
        assert!(d.start(false));assert!(!d.write(0x41));assert!(!d.write(255));assert_eq!(s.lock().unwrap().ram[2],2);
    }
    #[test]
    fn display_flip_inverse_all_on_start_line_offset_and_multiplex_do_not_mutate_ram() {
        let (s,mut d)=rig(OledKind::Ssd1306);send(&mut d,&[0,0xb0,0,0x10]);send(&mut d,&[0x40,1]);assert!(s.lock().unwrap().frame().iter().all(|&p|p==0));send(&mut d,&[0,0xaf]);assert_eq!(s.lock().unwrap().frame()[0],65535);
        send(&mut d,&[0,0xa1,0xc8]);assert_eq!(s.lock().unwrap().frame()[8191],65535);send(&mut d,&[0,0xa7]);assert_eq!(s.lock().unwrap().frame().iter().filter(|&&p|p==0).count(),1);send(&mut d,&[0,0xa5]);assert!(s.lock().unwrap().frame().iter().all(|&p|p==65535));
        send(&mut d,&[0,0xa4,0xa6,0xa0,0xc0,0x41]);assert_eq!(s.lock().unwrap().frame()[63*128],65535);send(&mut d,&[0,0x40,0xd3,8]);assert_eq!(s.lock().unwrap().frame()[56*128],65535);send(&mut d,&[0,0xa8,47,0xc8]);assert_eq!(s.lock().unwrap().frame()[55*128],65535);assert!(s.lock().unwrap().frame()[..8*128].iter().all(|&p|p==0));send(&mut d,&[0,0xae]);assert!(s.lock().unwrap().frame().iter().all(|&p|p==0));assert_eq!(s.lock().unwrap().ram[0],1);
    }
    #[test]
    fn invalid_windows_and_interleaved_data_cannot_wrap_or_panic() {
        let (s,mut d)=rig(OledKind::Ssd1306);send(&mut d,&[0,0x20,0,0x21,128,255,0x22,0,7]);assert!(d.start(false));assert!(d.write(0x40));assert!(!d.write(255));d.stop();assert_eq!(s.lock().unwrap().data_bytes,0);
        send(&mut d,&[0,0x21,0,127,0x22,7,2]);assert!(d.start(false));d.write(0x40);assert!(!d.write(255));d.stop();send(&mut d,&[0,0x22,0,7,0x20,3]);assert!(d.start(false));d.write(0x40);assert!(!d.write(255));d.stop();
        send(&mut d,&[0,0x20,0,0x21]);assert!(d.start(false));d.write(0x40);assert!(!d.write(255));d.stop();send(&mut d,&[0,0,0]);send(&mut d,&[0x40,1]);assert_eq!(s.lock().unwrap().ram[0],1);assert_eq!(s.lock().unwrap().ram.iter().map(|&b|u32::from(b)).sum::<u32>(),1);
    }
    #[test]
    fn power_reset_and_unmodeled_scroll_have_explicit_boundaries() {
        let (s,mut d)=rig(OledKind::Ssd1306);send(&mut d,&[0,0xaf]);send(&mut d,&[0x40,1]);s.lock().unwrap().set_reset(false);assert!(!d.start(false));assert_eq!(s.lock().unwrap().ram[0],1);assert!(s.lock().unwrap().frame().iter().all(|&p|p==0));s.lock().unwrap().set_reset(true);send(&mut d,&[0,0xaf]);assert_eq!(s.lock().unwrap().frame()[0],65535);
        send(&mut d,&[0,0x26,0,0,0,7,0,255,0x2f]);assert!(d.start(false));d.write(0x40);assert!(!d.write(255));d.stop();send(&mut d,&[0,0x2e]);send(&mut d,&[0x40,2]);s.lock().unwrap().set_power(false);assert!(!d.start(false));s.lock().unwrap().set_power(true);assert!(s.lock().unwrap().frame().iter().all(|&p|p==0));assert!(s.lock().unwrap().ram.iter().all(|&b|b==0));
    }
}
