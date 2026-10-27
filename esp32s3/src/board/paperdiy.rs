//! K159-B parallel ET047TC1, manufacturer schematic and M5GFX 02107b828acb.
//! A digital black/white endpoint, not calibrated 16-gray electrophoretic physics.
use std::sync::{Arc,Mutex};
use esp_periph::i2c::{I2cDevice,m5::{M5pm1,M5pm1State}};
use esp_soc::board::{BoardModel,BoardEdge,VirtualCycle,LcdI80Pins};
use esp_soc::devices::i2c_bitbang::I2cBitBang;

pub struct PaperDiy {pmic:Arc<Mutex<M5pm1State>>,i2c:I2cBitBang,levels:[bool;2],edges:Vec<BoardEdge>,output:u64,enabled:u64,frame:Vec<u16>,pending:Vec<u16>,row:usize,scanning:bool,frames:u64,version:u64,gpio_events:u64}
impl PaperDiy {
    pub fn new()->Self {Self{pmic:Default::default(),i2c:I2cBitBang::new(),levels:[true;2],edges:Vec::new(),output:0,enabled:0,frame:vec![0xffff;960*540],pending:Vec::new(),row:0,scanning:false,frames:0,version:0,gpio_events:0}}
    fn powered(&self)->bool {let p=self.pmic.lock().expect("PaperDIY PMIC mutex poisoned");!p.power_off_requested && p.driven_high&4!=0 && self.enabled&self.output&(1<<46)!=0}
    fn latch(&mut self) {
        if !self.powered() || !self.scanning || self.row>=540 || self.pending.len()<240 {self.pending.clear();return;}
        let mut changed=false;
        // Panel_EPD::blit_dmabuf packs four pixels per byte, MSB pair first.
        // Commands 1/2 drive black/white; 0/3 hold. Ignore the eight line-padding bytes.
        for x in 0..960 {let command=(self.pending[x/4]>>(6-2*(x%4)))&3;let pixel=&mut self.frame[self.row*960+x];let next=match command {1=>0,2=>0xffff,_=>*pixel};changed|=*pixel!=next;*pixel=next;}
        self.pending.clear();self.row+=1;if changed {self.version+=1;}
        if self.row==540 {self.frames+=1;}
    }
}
impl Default for PaperDiy {fn default()->Self {Self::new()}}
impl BoardModel for PaperDiy {
    fn name(&self)->&'static str {"paperdiy"}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        let previous=self.output;self.output=output;self.enabled=enabled;self.gpio_events+=changes.len() as u64;
        let high=|pin:u8|enabled&(1u64<<pin)==0 || output&(1u64<<pin)!=0;
        let scl=high(42);let sda=self.i2c.drive(scl,high(41));for (i,(pin,level)) in [(41,sda),(42,scl)].into_iter().enumerate(){if self.levels[i]!=level {self.levels[i]=level;self.edges.push(BoardEdge{cycle,pin,level});}}
        if !self.powered() {self.pending.clear();self.scanning=false;return;}
        // SPV low is the gate-driver frame start. The driver's three CKV preamble pulses
        // carry no source data and do not create rows. Latch each actual source line at XLE.
        if previous&(1<<17)!=0 && output&(1<<17)==0 {self.row=0;self.scanning=true;self.pending.clear();}
        if previous&(1<<15)==0 && output&(1<<15)!=0 {self.latch();}
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.i2c.clear();self.i2c.attach(0x6e,Box::new(M5pm1::new(self.pmic.clone())));
        (0..2).flat_map(|bus|self.i2c.controller_devices(Some((41,42))).into_iter().map(move |(address,device)|(bus,address,device))).collect()
    }
    fn input_levels(&self)->Vec<(u8,bool)> {vec![(0,true),(3,true),(4,true),(41,self.levels[0]),(42,self.levels[1]),(48,true)]}
    fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    fn gpio_events(&self)->u64 {self.gpio_events}
    fn lcd_i80(&mut self,pins:LcdI80Pins,data_phase:bool,words:&[u16]) {
        let data=[6,14,7,12,9,11,8,10];
        if data_phase && pins.width==8 && pins.clock&(1<<16)!=0 && pins.select&(1<<13)!=0 && data.iter().enumerate().all(|(i,p)|pins.data[i]&(1<<p)!=0) && self.powered() && self.scanning && self.output&(1<<15)==0 && self.output&(1<<18)!=0 {
            self.pending.clear();self.pending.extend_from_slice(&words[..words.len().min(248)]);
        }
    }
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {let mut frame=vec![0xffff;540*960];for y in 0..540 {for x in 0..960 {frame[x*540+539-y]=self.frame[y*960+x];}}Some((540,960,frame,self.version))}
    fn display_version(&self)->u64 {self.version}
    fn display_frames(&self)->u64 {self.frames}
    fn display_quiet_push(&self)->bool {true}
    fn named_pin(&self,id:&str)->Option<u8> {match id {"button-a"|"btnA"=>Some(4),"button-b"|"btnB"=>Some(3),_=>None}}
    fn report(&self)->String {format!("[paperdiy] ET047TC1 bw frames={} row={} powered={} TF=absent gray/analog/network=unverified",self.frames,self.row,self.powered())}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn power_routes_frame_start_and_latch_control_retained_pixels() {
        let mut b=PaperDiy::new();let mut pins=LcdI80Pins{width:8,clock:1<<16,select:1<<13,..Default::default()};
        for (i,p) in [6,14,7,12,9,11,8,10].into_iter().enumerate(){pins.data[i]=1<<p;}
        let mask=(1<<46)|(1<<18)|(1<<17)|(1<<15);b.gpio_output_at(1,&[],mask,(1<<46)|(1<<17));
        b.lcd_i80(pins,true,&[0x55;248]);assert!(b.pending.is_empty(),"no PMIC rail");
        b.pmic.lock().unwrap().driven_high=4;b.gpio_output_at(2,&[],mask,(1<<46)|(1<<18));
        let wrong=LcdI80Pins{clock:1<<5,..pins};b.lcd_i80(wrong,true,&[0x55;248]);assert!(b.pending.is_empty());
        b.lcd_i80(pins,true,&[0x55;248]);assert_eq!(b.frame[0],0xffff,"source latch not yet strobed");
        b.gpio_output_at(3,&[],mask,(1<<46)|(1<<15));assert_eq!(b.frame[..960],[0;960]);assert_eq!(b.row,1);
        b.gpio_output_at(4,&[],mask,0);assert_eq!(b.frame[0],0,"e-paper retains optical state without power");assert!(!b.scanning);
        assert_eq!(b.named_pin("button-a"),Some(4));assert_eq!(b.named_pin("button-b"),Some(3));assert_eq!(b.named_pin("power"),None);
        assert_eq!(b.input_levels().last(),Some(&(48,true)),"SPI TF card absent");
    }
}
