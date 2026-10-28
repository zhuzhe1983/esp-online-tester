//! SDHC SPI byte framing (SD Physical Layer Simplified v9.10, chapter 7).
//! Reuses the native card's capacity/CID/CSD, block storage, data phases and erase.
//! SdFat 2.3.1 SdSpiCard.cpp is the independent host contract. Immediate digital
//! transfers only: no analog startup, wire CRC faults, programming delay or SDSC.
use super::*;
use std::collections::VecDeque;

pub struct SpiSdCard {card:SdCard,command:Vec<u8>,reply:VecDeque<u8>,write:Vec<u8>,write_token:bool,multi_read:bool,multi_write:bool,crc:bool,spi:bool,clocks:u8}
impl SpiSdCard {
    pub fn new(store:Box<dyn SdStore>,config:SdCardConfig)->Result<Self,String> {
        if config.timed_bus || config.prg_ns!=0 {return Err("SPI SD currently supports immediate digital transfers only".into());}
        Ok(Self{card:SdCard::with_config(store,config)?,command:Vec::with_capacity(6),reply:VecDeque::new(),write:Vec::with_capacity(514),write_token:false,multi_read:false,multi_write:false,crc:false,spi:false,clocks:0})
    }
    pub fn describe(&self)->String {self.card.describe()}
    pub fn report(&self)->String {format!("commands={} read-blocks={} written-blocks={}",self.card.commands,self.card.blocks_read,self.card.blocks_written)}
    pub fn deselect(&mut self) {
        self.command.clear();self.reply.clear();self.write.clear();self.write_token=false;self.multi_read=false;self.multi_write=false;
        self.card.data=Phase::None;if self.spi && self.card.state!=CardState::Idle {self.card.state=CardState::Tran;}
    }
    pub fn transfer(&mut self,selected:bool,tx:&[u8],rx_len:usize)->Vec<u8> {
        let clocks=tx.len().max(rx_len);let mut rx=Vec::with_capacity(rx_len);
        if !selected {self.deselect();self.clocks=self.clocks.saturating_add(clocks.min(10) as u8);return vec![0xff;rx_len];}
        for i in 0..clocks {let value=self.byte(tx.get(i).copied().unwrap_or(0xff));if i<rx_len {rx.push(value);}}
        rx
    }
    fn r1(&self)->u8 {u8::from(self.card.state==CardState::Idle)}
    fn block(&mut self,bytes:&[u8]) {self.reply.push_back(0xff);self.reply.push_back(0xfe);self.reply.extend(bytes);self.reply.extend(crc16(bytes).to_be_bytes());}
    fn byte(&mut self,input:u8)->u8 {
        // A command is accepted during a multiple read so CMD12 can stop it.
        if self.command.is_empty() && !self.write_token && !matches!(self.card.data,Phase::Write{..}) && input&0xc0==0x40 {self.command.push(input);if self.multi_read {self.reply.clear();}return self.reply.pop_front().unwrap_or(0xff);}
        if !self.command.is_empty() {
            let out=self.reply.pop_front().unwrap_or(0xff);self.command.push(input);
            if self.command.len()==6 {let packet:Vec<u8>=std::mem::take(&mut self.command);self.execute(&packet);}
            return out;
        }
        if let Some(out)=self.reply.pop_front() {return out;}
        if self.write_token {
            self.write.push(input);
            if self.write.len()==514 {
                let valid=!self.crc || crc16(&self.write[..512])==u16::from_be_bytes([self.write[512],self.write[513]]);
                let code=if !valid {0x0b}else if self.card.write_data(0,&self.write[..512]).is_ok(){0x05}else{0x0d};
                self.reply.push_back(code);self.write.clear();self.write_token=false;
            }
        } else if matches!(self.card.data,Phase::Write{..}) {
            if input==0xfd && self.multi_write {self.card.data=Phase::None;self.card.state=CardState::Tran;self.multi_write=false;}
            else if input==if self.multi_write {0xfc}else{0xfe} {self.write_token=true;self.write.clear();}
        } else if self.multi_read {
            let mut block=[0;512];if self.card.read_data(0,&mut block).is_ok(){self.block(&block);}else{self.reply.push_back(0x09);self.multi_read=false;}
        }
        0xff
    }
    fn execute(&mut self,p:&[u8]) {
        let cmd=p[0]&63;let arg=u32::from_be_bytes(p[1..5].try_into().unwrap());
        self.reply.push_back(0xff); // NCR: at least one byte before response.
        if (self.crc || cmd==0 || cmd==8) && p[5]!=(crc7(&p[..5])<<1|1) {self.reply.push_back(self.r1()|8);return;}
        if !self.spi && (cmd!=0 || self.clocks<10) {self.reply.clear();return;}
        if cmd==0 {self.card.reset();self.spi=true;self.crc=false;self.multi_read=false;self.multi_write=false;self.reply.push_back(1);return;}
        if cmd==59 {self.crc=arg&1!=0;self.reply.push_back(self.r1());return;}
        if cmd==58 {self.reply.push_back(self.r1());self.reply.extend((OCR_VOLTAGE|if self.card.state!=CardState::Idle {OCR_READY|OCR_CCS}else{0}).to_be_bytes());return;}
        if matches!(cmd,9|10) && self.card.state==CardState::Tran {let register=if cmd==9{self.card.csd()}else{self.card.cid()};self.reply.push_back(0);self.block(&register.to_be_bytes());return;}
        if cmd==16 && arg!=512 {self.reply.push_back(self.r1()|0x40);return;}
        let app=self.card.app_cmd;
        // SPI ACMD41 doesn't carry the native bus's voltage window or assign RCA.
        let result=self.card.command(0,cmd,if app&&cmd==41 {arg|OCR_VOLTAGE}else{arg});
        if self.card.state==CardState::Ready {self.card.state=CardState::Tran;}
        match result {
            Ok(SdResponse::Short(value))=>{
                let error=if matches!(cmd,8|41) {0}else{(u8::from(value&ILLEGAL_COMMAND!=0)*4)|(u8::from(value&OUT_OF_RANGE!=0)*0x20)|(u8::from(value&(ERASE_PARAM|ERASE_SEQ_ERROR)!=0)*0x40)};
                self.reply.push_back(self.r1()|error);
                if cmd==8 {self.reply.extend(value.to_be_bytes());}
                else if cmd==13 {self.reply.push_back(u8::from(value&(CC_ERROR|ERROR)!=0)*4);}
                if let Phase::Read{buf,..}=&self.card.data {
                    let n=buf.len();let mut bytes=vec![0;n];if self.card.read_data(0,&mut bytes).is_ok(){self.block(&bytes);}
                    self.multi_read=cmd==18;
                }
                if matches!(cmd,24|25) {self.multi_write=cmd==25;}
                if cmd==12 {self.multi_read=false;self.multi_write=false;}
            }
            _=>self.reply.push_back(self.r1()|4),
        }
    }
}
fn crc16(bytes:&[u8])->u16 {let mut crc=0u16;for b in bytes {crc^=u16::from(*b)<<8;for _ in 0..8 {crc=if crc&0x8000!=0 {(crc<<1)^0x1021}else{crc<<1};}}crc}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(s:&mut SpiSdCard,c:u8,a:u32)->Vec<u8> {let mut p=vec![0x40|c];p.extend(a.to_be_bytes());p.push(crc7(&p)<<1|1);s.transfer(true,&p,0);s.transfer(true,&[],2)}
    fn ready()->SpiSdCard {let mut s=SpiSdCard::new(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();s.transfer(false,&[255;10],0);assert_eq!(command(&mut s,0,0),[255,1]);assert_eq!(command(&mut s,8,0x1aa),[255,1]);assert_eq!(s.transfer(true,&[],4),[0,0,1,0xaa]);for _ in 0..2 {assert_eq!(command(&mut s,55,0),[255,1]);command(&mut s,41,1<<30);}assert_eq!(s.r1(),0);s}
    #[test]
    fn spi_framing_crc_partial_write_and_real_store() {
        let mut s=ready();assert_eq!(command(&mut s,58,0),[255,0]);assert_eq!(s.transfer(true,&[],4),[0xc0,0xff,0x80,0]);
        assert_eq!(command(&mut s,59,1),[255,0]);assert_eq!(command(&mut s,24,3),[255,0]);
        let mut block=vec![0xfe];block.extend([0x5a;512]);block.extend([0,0]);s.transfer(true,&block,0);assert_eq!(s.transfer(true,&[],1),[0x0b]);
        let crc=crc16(&[0x5a;512]).to_be_bytes();block[513]=crc[0];block[514]=crc[1];s.transfer(true,&block,0);assert_eq!(s.transfer(true,&[],1),[5]);
        assert_eq!(command(&mut s,17,3),[255,0]);assert_eq!(s.transfer(true,&[],2),[255,0xfe]);assert_eq!(s.transfer(true,&[],512),vec![0x5a;512]);assert_eq!(s.transfer(true,&[],2),crc);
        assert_eq!(command(&mut s,24,3),[255,0]);s.transfer(true,&[0xfe,1,2,3],0);s.deselect();assert_eq!(s.card.blocks_written,1);
        assert_eq!(command(&mut s,17,3),[255,0]);s.transfer(true,&[],2);assert_eq!(s.transfer(true,&[],512),vec![0x5a;512]);s.deselect();
        s.transfer(true,&[0x51,0,0],0);s.deselect();assert_eq!(s.transfer(true,&[0,3,255],3),[255;3]);
        assert_eq!(command(&mut s,17,2048),[255,0x20]);assert_eq!(s.transfer(true,&[],2),[255,255]);
    }
    #[test]
    fn startup_select_command_crc_and_multi_block_stop() {
        let mut s=SpiSdCard::new(Box::new(MemStore(vec![0;1<<20])),Default::default()).unwrap();assert_eq!(command(&mut s,0,0),[255,255]);s.transfer(false,&[255;10],0);s.transfer(true,&[0x40,0,0,0,0,0],0);assert_eq!(s.transfer(true,&[],2),[255,9]);
        let mut s=ready();assert_eq!(command(&mut s,25,0),[255,0]);for value in [1,2] {let mut data=vec![0xfc];data.extend([value;512]);data.extend([255,255]);s.transfer(true,&data,0);assert_eq!(s.transfer(true,&[],1),[5]);}s.transfer(true,&[0xfd],0);assert_eq!(s.card.blocks_written,2);
        assert_eq!(command(&mut s,18,0),[255,0]);assert_eq!(s.transfer(true,&[],2),[255,0xfe]);assert_eq!(s.transfer(true,&[],512),vec![1;512]);s.transfer(true,&[],2);s.transfer(true,&[],3);assert_eq!(s.transfer(true,&[],512),vec![2;512]);s.transfer(true,&[],2);assert_eq!(command(&mut s,12,0),[255,0]);assert_eq!(s.transfer(true,&[],5),[255;5]);
    }
}
