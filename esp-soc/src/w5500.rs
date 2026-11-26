//! W5500 digital SPI/MACRAW subset. WIZnet W5500 datasheet v1.1.0 (2013),
//! §§2.2-2.4, 3, 4.1-4.2: VDM/FDM, block addressing, ring pointers and IRQs.
//! Socket 0 MACRAW only. TCP/UDP offload, PPPoE, PHY analog timing and FCS are not modeled.
use crate::ethernet::EthernetLink;

#[derive(Clone,Copy)]
enum SpiFrame {Header([u8;3],usize),Data{address:u16,block:u8,write:bool,left:Option<usize>}}
pub struct W5500 {
    pub link:EthernetLink,
    common:[u8;64],sockets:[[u8;48];8],tx:[Vec<u8>;8],rx:[Vec<u8>;8],
    tx_read:[u16;8],rx_write:[u16;8],rx_read:[u16;8],spi:Option<SpiFrame>,powered:bool,
    pub transmitted:u64,pub received:u64,pub dropped:u64,
}
impl Default for W5500 {fn default()->Self {Self::new()}}
impl W5500 {
    pub fn new()->Self {let mut chip=Self{link:EthernetLink::new(),common:[0;64],sockets:[[0;48];8],tx:std::array::from_fn(|_|Vec::new()),rx:std::array::from_fn(|_|Vec::new()),tx_read:[0;8],rx_write:[0;8],rx_read:[0;8],spi:None,powered:true,transmitted:0,received:0,dropped:0};chip.reset();chip}
    pub fn reset(&mut self) {
        self.common=[0;64];self.common[0x19]=7;self.common[0x1a]=0xd0;self.common[0x1b]=8;self.common[0x26]=255;self.common[0x27]=255;self.common[0x2e]=0xb8;
        self.sockets=[[0;48];8];self.tx_read=[0;8];self.rx_write=[0;8];self.rx_read=[0;8];
        for n in 0..8 {self.sockets[n][0x12]=5;self.sockets[n][0x13]=0xb4;self.sockets[n][0x16]=128;self.sockets[n][0x1e]=2;self.sockets[n][0x1f]=2;self.sockets[n][0x2c]=255;self.sockets[n][0x2d]=0x40;self.tx[n]=vec![0;2048];self.rx[n]=vec![0;2048];}
        self.spi=None;self.link.reset();
    }
    pub fn set_powered(&mut self,present:bool) {if self.powered&&!present {self.reset();}self.powered=present;}
    pub fn end_transaction(&mut self) {self.spi=None;}
    fn link_live(&self)->bool {let phy=self.common[0x2e];self.powered&&phy&0x80!=0&&phy&0x78!=0x70}
    pub fn macraw_open(&self)->bool {self.sockets[0][3]==0x42&&self.link_live()}
    pub fn irq_asserted(&self)->bool {self.powered&&(self.common[0x15]&self.common[0x16]!=0||self.socket_interrupts()&self.common[0x18]!=0)}
    fn socket_interrupts(&self)->u8 {(0..8).fold(0,|v,n|v|if self.sockets[n][2]&self.sockets[n][0x2c]!=0 {1<<n}else{0})}
    fn word(&self,n:usize,reg:usize)->u16 {u16::from_be_bytes([self.sockets[n][reg],self.sockets[n][reg+1]])}
    fn put_word(&mut self,n:usize,reg:usize,value:u16) {self.sockets[n][reg..reg+2].copy_from_slice(&value.to_be_bytes());}
    fn received_size(&self,n:usize)->usize {usize::from(self.rx_write[n].wrapping_sub(self.rx_read[n]))}
    fn transmit_size(&self,n:usize)->usize {usize::from(self.word(n,0x24).wrapping_sub(self.tx_read[n]))}
    fn read_byte(&self,block:u8,address:u16)->u8 {
        let a=usize::from(address);
        if block==0 {return match a {0x17=>self.socket_interrupts(),0x2e=>(self.common[0x2e]&0xf8)|if self.link_live(){7}else{0},0x39=>4,_=>self.common.get(a).copied().unwrap_or(0)};}
        let n=usize::from(block>>2);if n>=8{return 255;}
        match block&3 {
            1=>{let value=match a {0x20|0x21=>Some(self.tx[n].len().saturating_sub(self.transmit_size(n)).min(u16::MAX as usize)as u16),0x22|0x23=>Some(self.tx_read[n]),0x26|0x27=>Some(self.received_size(n)as u16),0x2a|0x2b=>Some(self.rx_write[n]),_=>None};if let Some(value)=value {value.to_be_bytes()[a&1]}else{self.sockets[n].get(a).copied().unwrap_or(0)}},
            2=>if self.tx[n].is_empty(){255}else{self.tx[n][a%self.tx[n].len()]},
            3=>if self.rx[n].is_empty(){255}else{self.rx[n][a%self.rx[n].len()]},
            _=>255,
        }
    }
    fn write_byte(&mut self,block:u8,address:u16,value:u8,now_us:u64) {
        let a=usize::from(address);
        if block==0 {match a {0=>if value&0x80!=0 {self.reset();}else{self.common[0]=value;},0x15=>self.common[a]&= !value,0x17|0x28..=0x2d|0x39=>{},0x2e=>self.common[a]=value&0xf8,_=>if let Some(dst)=self.common.get_mut(a){*dst=value;}}return;}
        let n=usize::from(block>>2);if n>=8{return;}
        match block&3 {
            1=>match a {
                1=>self.command(n,value,now_us),2=>self.sockets[n][2]&= !value,3|0x20..=0x23|0x26..=0x27|0x2a..=0x2b=>{},
                0x1e|0x1f=>if self.sockets[n][3]==0&&[0,1,2,4,8,16].contains(&value) {self.sockets[n][a]=value;let memory=if a==0x1e {&mut self.rx[n]}else{&mut self.tx[n]};memory.resize(usize::from(value)*1024,0);},
                _=>if let Some(dst)=self.sockets[n].get_mut(a){*dst=value;},
            },
            2 if !self.tx[n].is_empty()=>{let at=a%self.tx[n].len();self.tx[n][at]=value;},
            // RX memory is owned by the MAC, never a writable TX shortcut.
            _=>{},
        }
    }
    fn valid_buffers(&self)->bool {(0..8).map(|n|usize::from(self.sockets[n][0x1e])).sum::<usize>()<=16&&(0..8).map(|n|usize::from(self.sockets[n][0x1f])).sum::<usize>()<=16}
    fn command(&mut self,n:usize,value:u8,now_us:u64) {
        match value {
            1=>{self.sockets[n][3]=0;self.sockets[n][2]=0;self.tx_read[n]=0;self.rx_read[n]=0;self.rx_write[n]=0;self.put_word(n,0x24,0);self.put_word(n,0x28,0);
                if n==0&&self.sockets[n][0]&15==4&&self.valid_buffers()&&!self.tx[n].is_empty()&&!self.rx[n].is_empty(){while self.link.next_frame().is_some(){}self.sockets[n][3]=0x42;}},
            0x10=>{self.sockets[n][3]=0;self.sockets[n][2]=0;self.rx_read[n]=self.rx_write[n];},
            0x20 if n==0&&self.macraw_open()=>{
                let length=self.transmit_size(n);if !(14..=1518).contains(&length)||length>self.tx[n].len(){self.dropped=self.dropped.saturating_add(1);return;}
                let frame:Vec<_>=(0..length).map(|i|self.tx[n][(usize::from(self.tx_read[n])+i)%self.tx[n].len()]).collect();
                if self.link.transmit(&frame,now_us).is_ok(){self.tx_read[n]=self.word(n,0x24);self.sockets[n][2]|=0x10;self.transmitted=self.transmitted.saturating_add(1);}else{self.sockets[n][2]|=8;}
            },
            0x40=>{let target=self.word(n,0x28);if usize::from(target.wrapping_sub(self.rx_read[n]))<=self.received_size(n){self.rx_read[n]=target;}},
            _=>{},
        }
    }
    fn accept_frame(&self,frame:&[u8])->bool {
        if !self.macraw_open()||!(14..=1518).contains(&frame.len()){return false;}
        let mode=self.sockets[0][0];let broadcast=frame[..6]==[255;6];let multicast=frame[0]&1!=0&&!broadcast;
        if mode&0x40!=0&&broadcast||mode&0x20!=0&&multicast||mode&0x10!=0&&frame[12..14]==[0x86,0xdd]{return false;}
        mode&0x80==0||broadcast||frame[..6]==self.common[9..15]
    }
    pub fn poll(&mut self,now_us:u64) {
        self.link.poll(now_us);
        while let Some(frame)=self.link.next_frame() {
            let size=frame.len()+2;
            if !self.accept_frame(&frame)||size>self.rx[0].len().saturating_sub(self.received_size(0)){self.dropped=self.dropped.saturating_add(1);continue;}
            for b in (size as u16).to_be_bytes().into_iter().chain(frame) {let at=usize::from(self.rx_write[0])%self.rx[0].len();self.rx[0][at]=b;self.rx_write[0]=self.rx_write[0].wrapping_add(1);}
            self.sockets[0][2]|=4;self.received=self.received.saturating_add(1);
        }
    }
    fn clock_byte(&mut self,byte:u8,now_us:u64)->u8 {
        match self.spi.take().unwrap_or(SpiFrame::Header([0;3],0)) {
            SpiFrame::Header(mut header,n)=>{header[n]=byte;self.spi=Some(if n==2 {let mode=header[2]&3;SpiFrame::Data{address:u16::from_be_bytes([header[0],header[1]]),block:header[2]>>3,write:header[2]&4!=0,left:if mode==0{None}else{Some(1<<(mode-1))}}}else{SpiFrame::Header(header,n+1)});255},
            SpiFrame::Data{address,block,write,left}=>{let result=if write {self.write_byte(block,address,byte,now_us);255}else{self.read_byte(block,address)};self.spi=if left==Some(1){None}else{Some(SpiFrame::Data{address:address.wrapping_add(1),block,write,left:left.map(|n|n-1)})};result},
        }
    }
    /// Command/address phases do not occupy controller RX FIFO positions. In full
    /// duplex only the following data phase is concurrent; half duplex clocks RX
    /// after all TX bytes. Physical CS, not each FIFO chunk, terminates VDM frames.
    pub fn transfer(&mut self,tx:&[u8],rx_len:usize,prefix_len:usize,full_duplex:bool,now_us:u64)->Vec<u8> {
        if !self.powered||prefix_len>tx.len()||rx_len>16*1024*1024{return vec![255;rx_len.min(16*1024*1024)];}
        self.poll(now_us);for &b in &tx[..prefix_len]{self.clock_byte(b,now_us);}
        let tx=&tx[prefix_len..];let mut result=Vec::with_capacity(rx_len);
        if full_duplex {for at in 0..tx.len().max(rx_len){let b=self.clock_byte(tx.get(at).copied().unwrap_or(0),now_us);if at<rx_len{result.push(b);}}}
        else {for &b in tx{self.clock_byte(b,now_us);}for _ in 0..rx_len{result.push(self.clock_byte(0,now_us));}}
        self.poll(now_us);result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(c:&mut W5500,block:u8,reg:u16,data:&[u8]) {let mut b=reg.to_be_bytes().to_vec();b.push((block<<3)|4);b.extend(data);c.transfer(&b,0,0,false,0);c.end_transaction();}
    fn read(c:&mut W5500,block:u8,reg:u16,n:usize)->Vec<u8> {let mut b=reg.to_be_bytes().to_vec();b.push(block<<3);let r=c.transfer(&b,n,0,false,0);c.end_transaction();r}
    fn open(c:&mut W5500) {write(c,1,0,&[4]);write(c,1,1,&[1]);assert_eq!(read(c,1,3,1),[0x42]);}
    fn frame(size:usize,tag:u8)->Vec<u8> {let mut b=vec![tag;size];b[..6].fill(255);b[12]=0x88;b[13]=0xb5;b}
    #[test]
    fn spi_vdm_fdm_prefix_and_readonly_identity() {
        let mut c=W5500::new();assert_eq!(read(&mut c,0,0x39,1),[4]);write(&mut c,0,0x39,&[9]);assert_eq!(read(&mut c,0,0x39,1),[4]);
        c.transfer(&[0,9,5,0x12,0,10,5,0x34],0,0,false,0);c.end_transaction();assert_eq!(read(&mut c,0,9,2),[0x12,0x34]);
        let b=c.transfer(&[0,0x39,0,0],4,0,true,0);c.end_transaction();assert_eq!(b,[255,255,255,4]);
        assert_eq!(c.transfer(&[0,0x39,0],1,3,true,0),[4]);c.end_transaction();
        c.transfer(&[0,9],0,0,false,0);c.end_transaction();assert_eq!(read(&mut c,0,0x39,1),[4],"CS discards incomplete header");
    }
    #[test]
    fn macraw_transmit_receive_irq_ring_wrap_and_recv_commit() {
        let mut c=W5500::new();c.link.set_relay(true);open(&mut c);write(&mut c,0,0x18,&[1]);
        let a=frame(1500,0x36);write(&mut c,2,0,&a);write(&mut c,1,0x24,&1500u16.to_be_bytes());write(&mut c,1,1,&[0x20]);assert_eq!(c.link.take_frames().as_slice(),std::slice::from_ref(&a));assert!(c.irq_asserted());assert_eq!(read(&mut c,1,0x22,2),1500u16.to_be_bytes());write(&mut c,1,2,&[0x10]);assert!(!c.irq_asserted());
        let b=frame(1000,0x57);write(&mut c,2,1500,&b);write(&mut c,1,0x24,&2500u16.to_be_bytes());write(&mut c,1,1,&[0x20]);assert_eq!(c.link.take_frames().as_slice(),std::slice::from_ref(&b),"TX crosses 2KiB ring end");
        c.link.receive(&a).unwrap();c.poll(0);assert_eq!(read(&mut c,1,0x26,2),1502u16.to_be_bytes());assert_eq!(read(&mut c,3,0,1502),[1502u16.to_be_bytes().to_vec(),a].concat());write(&mut c,1,0x28,&1502u16.to_be_bytes());assert_eq!(read(&mut c,1,0x26,2),1502u16.to_be_bytes(),"pointer write alone does not release memory");write(&mut c,1,1,&[0x40]);assert_eq!(read(&mut c,1,0x26,2),[0,0]);
        c.link.receive(&b).unwrap();c.poll(1);assert_eq!(read(&mut c,3,1502,1002),[1002u16.to_be_bytes().to_vec(),b].concat(),"RX crosses ring end");
        write(&mut c,1,0x28,&0xffffu16.to_be_bytes());write(&mut c,1,1,&[0x40]);assert_eq!(read(&mut c,1,0x26,2),1002u16.to_be_bytes(),"invalid RX pointer cannot underflow");
    }
    #[test]
    fn closed_unsupported_socket_filter_overflow_reset_and_power_fail_closed() {
        let mut c=W5500::new();c.link.set_relay(true);write(&mut c,1,0,&[1]);write(&mut c,1,1,&[1]);assert_eq!(read(&mut c,1,3,1),[0],"TCP offload is not fabricated");write(&mut c,5,0,&[4]);write(&mut c,5,1,&[1]);assert_eq!(read(&mut c,5,3,1),[0],"MACRAW only socket0");open(&mut c);
        write(&mut c,1,0,&[0x44]);c.link.receive(&frame(64,1)).unwrap();c.poll(0);assert_eq!(read(&mut c,1,0x26,2),[0,0],"broadcast block");write(&mut c,1,0,&[4]);let large=frame(1500,2);c.link.receive(&large).unwrap();c.link.receive(&large).unwrap();c.poll(0);assert_eq!(read(&mut c,1,0x26,2),1502u16.to_be_bytes(),"full RX ring drops whole next frame");
        write(&mut c,0,0,&[0x80]);assert_eq!(read(&mut c,1,3,1),[0]);assert_eq!(read(&mut c,1,0x26,2),[0,0]);open(&mut c);c.set_powered(false);assert_eq!(read(&mut c,0,0x39,1),[255]);c.set_powered(true);assert_eq!(read(&mut c,1,3,1),[0]);
    }
}
