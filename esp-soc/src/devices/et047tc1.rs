//! ET047TC1 source/gate digital subset. M5GFX 0.2.15 Panel_EPD::blit_dmabuf.
//! 960x540, four two-bit drive symbols per byte, MSB first. This preserves a
//! black/white optical endpoint; analog pulse integration and 16 gray levels are not modeled.
pub struct Et047tc1 {pub frame:Vec<u16>,pub pending:Vec<u16>,pub row:usize,pub scanning:bool,pub frames:u64,pub version:u64}
impl Default for Et047tc1 {fn default()->Self {Self::new()}}
impl Et047tc1 {
    pub fn new()->Self {Self{frame:vec![0xffff;960*540],pending:Vec::new(),row:0,scanning:false,frames:0,version:0}}
    pub fn stop_scan(&mut self) {self.pending.clear();self.scanning=false;}
    pub fn begin_scan(&mut self) {self.row=0;self.scanning=true;self.pending.clear();}
    pub fn line(&mut self,words:&[u16]) {self.pending.clear();self.pending.extend_from_slice(&words[..words.len().min(248)]);}
    pub fn latch(&mut self) {
        if !self.scanning || self.row>=540 || self.pending.len()<240 {self.pending.clear();return;}
        let mut changed=false;
        for x in 0..960 {let command=(self.pending[x/4]>>(6-2*(x%4)))&3;let pixel=&mut self.frame[self.row*960+x];let next=match command {1=>0,2=>0xffff,_=>*pixel};changed|=*pixel!=next;*pixel=next;}
        self.pending.clear();self.row+=1;if changed {self.version+=1;}
        if self.row==540 {self.frames+=1;}
    }
    pub fn portrait(&self)->Vec<u16> {let mut frame=vec![0xffff;540*960];for y in 0..540 {for x in 0..960 {frame[x*540+539-y]=self.frame[y*960+x];}}frame}
}
