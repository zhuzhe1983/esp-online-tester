//! Bounded IT8951 SPI host interface for the reTerminal E1003's ED103TC2.
//! Contract: Seeed_GFX2 556df7cb0b4a097d9b81026a26fd5511f9c934d6,
//! Driver_IT8951.{cpp,h}, Driver_ED103TC2.cpp. Commands/registers are MSB-first;
//! packed image endianness is selected by LD_IMG. No LUT/temperature calibration.
//! The virtual image buffer address and firmware identity are explicitly synthetic.
use std::collections::VecDeque;
use crate::board::{BoardEdge,VirtualCycle};

#[derive(Clone,Copy,PartialEq)]
enum Phase {Preamble,Command,Data,Read,Invalid}
pub struct It8951 {
    pub frame:Vec<u16>,pub frames:u64,pub version:u64,pub ram_bytes_written:u64,
    ram:Vec<u8>,phase:Phase,byte:Option<u8>,command:u16,args:[u16;5],count:usize,reply:VecDeque<u8>,
    load:Option<[usize;4]>,format:u16,at:usize,awake:bool,packed:bool,base:[u16;2],bitmap:bool,colors:u16,
    area:Option<[usize;4]>,cycle:VirtualCycle,busy_until:Option<VirtualCycle>,edges:Vec<BoardEdge>,busy_pin:u8,cycles_per_ms:u64,
}
impl It8951 {
    pub const WIDTH:usize=1872;pub const HEIGHT:usize=1404;
    pub fn new(busy_pin:u8,cycles_per_ms:u64)->Self {Self{frame:vec![0xffff;Self::WIDTH*Self::HEIGHT],frames:0,version:0,ram_bytes_written:0,ram:vec![255;Self::WIDTH*Self::HEIGHT],phase:Phase::Preamble,byte:None,command:0,args:[0;5],count:0,reply:VecDeque::new(),load:None,format:0,at:0,awake:true,packed:false,base:[0,0x10],bitmap:false,colors:0x00ff,area:None,cycle:0,busy_until:None,edges:Vec::new(),busy_pin,cycles_per_ms}}
    pub fn reset(&mut self) {self.end_transaction();self.command=0;self.count=0;self.load=None;self.at=0;self.awake=true;self.packed=false;self.base=[0,0x10];self.bitmap=false;self.colors=0x00ff;self.area=None;self.ram.fill(255);if self.busy_until.take().is_some(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true});}}
    pub fn end_transaction(&mut self){self.phase=Phase::Preamble;self.byte=None;self.reply.clear();}
    pub fn busy_high(&self)->bool {self.busy_until.is_none()}
    pub fn report(&self)->String {format!("IT8951 cmd={:04x} args={:04x?} count={} packed={} awake={} base={:04x?} load={:?} words={} busy={} bitmap={}",self.command,self.args,self.count,self.packed,self.awake,self.base,self.load,self.at,!self.busy_high(),self.bitmap)}
    pub fn next_deadline(&self)->Option<VirtualCycle> {self.busy_until}
    pub fn take_edges(&mut self)->Vec<BoardEdge> {std::mem::take(&mut self.edges)}
    pub fn transfer(&mut self,tx:&[u8],rx_len:usize)->Vec<u8> {
        let mut rx=Vec::with_capacity(rx_len);
        // SPI.transfer16 clocks full-duplex even when the caller discards RX.
        // A preamble cannot return the reply it has not finished requesting:
        // sample MISO before consuming this clock's MOSI byte. TX dummy bytes
        // in Read phase and rx_len describe the same clocks, not two streams.
        for i in 0..tx.len().max(rx_len) {
            let out=if self.phase==Phase::Read{self.reply.pop_front().unwrap_or(255)}else{255};if i<rx_len{rx.push(out);}
            if let Some(&b)=tx.get(i){if matches!(self.phase,Phase::Read|Phase::Invalid){continue;}
                if let Some(a)=self.byte.take(){self.word(u16::from_be_bytes([a,b]));}else{self.byte=Some(b);}
            }
        }
        rx
    }
    fn word(&mut self,w:u16) {match self.phase {
        Phase::Preamble=>{self.phase=match w{0x6000=>Phase::Command,0=>Phase::Data,0x1000=>{self.prepare_read();Phase::Read},_=>Phase::Invalid};},
        Phase::Command=>{self.command=w;self.args=[0;5];self.count=0;self.phase=Phase::Invalid;match w{1=>self.awake=true,2|3=>{self.awake=false;self.load=None;},0x22=>self.load=None,_=>{}}},
        Phase::Data=>self.data(w),_=>{}
    }}
    fn window(a:&[u16])->Option<[usize;4]> {let [x,y,w,h]=std::array::from_fn(|i|usize::from(a[i]));if w!=0&&h!=0&&x.checked_add(w)?<=Self::WIDTH&&y.checked_add(h)?<=Self::HEIGHT{Some([x,y,w,h])}else{None}}
    fn data(&mut self,w:u16) {
        if self.load.is_some()&&matches!(self.command,0x20|0x21)&&self.count>=if self.command==0x20{1}else{5}{self.pixel_word(w);return;}
        if self.count<self.args.len(){self.args[self.count]=w;}self.count=self.count.saturating_add(1);
        match (self.command,self.count) {
            (0x11,2)=>match self.args[0]{4=>self.packed=w&1!=0,0x208=>self.base[0]=w,0x20a=>self.base[1]=w,0x113a=>self.bitmap=w&4!=0,0x1250=>self.colors=w,_=>{}},
            (0x20,1)=>self.begin_load(Some([0,0,Self::WIDTH,Self::HEIGHT])),
            (0x21,5)=>self.begin_load(Self::window(&self.args[1..5])),
            (0x34,5) if self.awake&&w<=4=>{self.area=Self::window(&self.args[..4]);if self.area.is_some(){if self.busy_until.is_none(){self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false});}self.busy_until=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(20).max(1)));}},_=>{}
        }
    }
    fn begin_load(&mut self,window:Option<[usize;4]>) {
        self.format=self.args[0];self.at=0;
        // The model deliberately accepts only the native rotation and the two
        // image formats exercised by Seeed's normal 1bpp/Gray16 path.
        self.load=if self.awake&&self.packed&&self.base==[0,0x10]&&self.format&0xf==0&&matches!((self.format>>4)&0xf,2|3){window}else{None};
    }
    fn pixel_word(&mut self,w:u16) {
        let Some([x,y,width,height])=self.load else{return;};let bits=if self.format&0x30==0x20{4}else{8};let per=16/bits;let words=width.div_ceil(per);
        if self.at>=words*height{return;}
        // ITE IT8951 For D v0.2.4.3, Fig7-17 (printed p29): little-endian
        // packed Pn occupies the low bits of the MSB-first SPI word. Swapping
        // wire bytes here would exchange adjacent eight-pixel bitmap blocks.
        for n in 0..per {let col=(self.at%words)*per+n;if col>=width{continue;}let shift=if self.format&0x100==0{n*bits}else{16-(n+1)*bits};let value=((w>>shift)&((1<<bits)-1))as u8;
            self.ram[(y+self.at/words)*Self::WIDTH+x+col]=if bits==4{value*17}else{value};
        }self.at+=1;self.ram_bytes_written+=2;
    }
    fn prepare_read(&mut self) {
        self.reply.clear();self.reply.extend([0,0]);
        let words:Vec<u16>=match self.command {
            0x0302=>{let mut info=vec![0;20];info[..4].copy_from_slice(&[Self::WIDTH as u16,Self::HEIGHT as u16,0,0x10]);for (i,w) in b"SIMULATED\0\0\0\0\0\0\0".as_chunks::<2>().0.iter().enumerate(){info[4+i]=u16::from_be_bytes([w[0],w[1]]);}info},
            0x10 if self.count>=1=>vec![match self.args[0]{4=>u16::from(self.packed),0x208=>self.base[0],0x20a=>self.base[1],0x113a=>if self.bitmap{4}else{0},0x1250=>self.colors,0x1224=>u16::from(!self.busy_high()),_=>0}],
            // Named nominal endpoints, no claim of physical VCOM calibration.
            0x39=>vec![1400],0x40=>vec![25],_=>vec![0xffff]
        };for w in words{self.reply.extend(w.to_be_bytes());}
    }
    pub fn advance_to(&mut self,cycle:VirtualCycle) {
        self.cycle=cycle;let Some(until)=self.busy_until.filter(|u|*u<=cycle)else{return;};self.busy_until=None;
        if let Some([x,y,w,h])=self.area.take(){for row in y..y+h{for col in x..x+w{
            // BGVR=(background<<8)|foreground in tconDisplayArea1bpp;
            // bit 1 selects foreground, the low byte, not the high byte.
            let g=if self.bitmap {let bit=self.ram[row*Self::WIDTH+col/8]&(1<<(col%8))!=0;if bit{self.colors as u8}else{(self.colors>>8)as u8}}else{self.ram[row*Self::WIDTH+col]};
            let g=u16::from(g);self.frame[row*Self::WIDTH+col]=((g>>3)<<11)|((g>>2)<<5)|(g>>3);
        }}self.frames+=1;self.version+=1;}self.edges.push(BoardEdge{cycle:until,pin:self.busy_pin,level:true});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(p:&mut It8951,w:&[u16]) {let data:Vec<u8>=w.iter().flat_map(|x|x.to_be_bytes()).collect();p.transfer(&data,0);p.end_transaction();}
    fn cmd(p:&mut It8951,c:u16,args:&[u16]){frame(p,&[0x6000,c]);for &a in args{frame(p,&[0,a]);}}
    fn read(p:&mut It8951,len:usize)->Vec<u8>{p.transfer(&[0x10,0],0);let r=p.transfer(&[],len);p.end_transaction();r}
    #[test]
    fn info_and_read_transaction_require_continuous_chip_select() {
        let mut p=It8951::new(13,1);cmd(&mut p,0x302,&[]);assert_eq!(&read(&mut p,10),&[0,0,7,80,5,124,0,0,0,16]);
        p.transfer(&[0x10,0],0);p.end_transaction();assert_eq!(p.transfer(&[],4),[255;4]);
        frame(&mut p,&[0x1234,0x302]);assert_eq!(p.transfer(&[],4),[255;4]);
    }
    #[test]
    fn full_duplex_read_preamble_does_not_consume_the_dummy_or_first_result() {
        let mut p=It8951::new(13,1);cmd(&mut p,0x302,&[]);
        assert_eq!(p.transfer(&[0x10,0],2),[255;2]);
        assert_eq!(p.transfer(&[0,0],2),[0,0]);
        assert_eq!(p.transfer(&[0,0],2),(It8951::WIDTH as u16).to_be_bytes());
        assert_eq!(p.transfer(&[0,0],2),(It8951::HEIGHT as u16).to_be_bytes());p.end_transaction();
        cmd(&mut p,0x10,&[0x1224]);p.transfer(&[0x10,0],2);p.transfer(&[0,0],2);
        assert_eq!(p.transfer(&[0,0],2),[0,0],"idle LUT must not appear permanently busy");
    }
    #[test]
    fn packed_gray_writes_need_format_bounds_and_display_completion() {
        let mut p=It8951::new(13,1);cmd(&mut p,0x11,&[4,1]);cmd(&mut p,0x21,&[0x20,0,0,4,1]);frame(&mut p,&[0,0xfa50]);cmd(&mut p,0x22,&[]);
        assert_eq!(p.frame[0],0xffff);cmd(&mut p,0x34,&[0,0,4,1,2]);assert!(!p.busy_high());p.advance_to(20);assert_eq!(&p.frame[..4],&[0,0x52aa,0xad55,0xffff]);assert_eq!(p.frames,1);assert_eq!(p.frame[4],0xffff);
        cmd(&mut p,0x21,&[0x20,65535,0,65535,65535]);frame(&mut p,&[0,0]);assert_eq!(p.ram_bytes_written,2);
        cmd(&mut p,3,&[]);cmd(&mut p,0x34,&[0,0,4,1,2]);p.advance_to(40);assert_eq!(p.frames,1);p.reset();assert_eq!(p.frame[0],0);
    }
    #[test]
    fn bitmap_uses_explicit_register_and_preserves_outside_area() {
        let mut p=It8951::new(13,1);cmd(&mut p,0x11,&[4,1]);cmd(&mut p,0x21,&[0x30,0,0,2,1]);frame(&mut p,&[0,0xfffe]);cmd(&mut p,0x22,&[]);cmd(&mut p,0x11,&[0x113a,4]);cmd(&mut p,0x11,&[0x1250,0x00ff]);cmd(&mut p,0x34,&[0,0,8,1,1]);p.advance_to(20);
        assert_eq!(p.frame[0],0);assert_eq!(&p.frame[1..9],&[0xffff;8]);
    }
    #[test]
    fn packed_word_endianness_changes_pixel_order_without_reversing_pixel_bits() {
        for (format,expected) in [(0x30,[0xe4,0x12]),(0x130,[0x12,0xe4])]{
            let mut p=It8951::new(13,1);cmd(&mut p,0x11,&[4,1]);cmd(&mut p,0x21,&[format,0,0,2,1]);frame(&mut p,&[0,0x12e4]);cmd(&mut p,0x22,&[]);
            assert_eq!(&p.ram[..2],&expected);
        }
    }
}
