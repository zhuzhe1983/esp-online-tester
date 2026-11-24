//! C5 AHB-GDMA v2: three pairs, full LINK_ADDR and C5 LINK control bits.
//! Only the SPI2 producer/consumer is connected. No C6 packed-LINK alias.
use emu_core::{Bus, Fault};
use esp_periph::{RegRam, gdma::{DescriptorWalk, Gdma, GdmaAhb, GDMA_DESCRIPTOR_STEP_BUDGET}};
use crate::{bus::{C5Bus, SRAM_BASE, LP_BASE, ROM_BASE}, transport_regs};

pub(crate) struct C5Dma {
    pub channels:Gdma, link:[[u32;3];2], auto_return:[bool;3], config:RegRam,
    pub done:[[u32;3];2], pub start:u32, pub end:u32,
}
impl C5Dma {
    pub fn new()->Self {
        let mut channels=Gdma::new();
        for ch in channels.out.iter_mut().take(3) { ch.conf0=8; }
        let mut config=RegRam::new();
        for off in (0..0x428).step_by(4) { if let Some((reset,_))=transport_regs::config(1,off) {config.write(off,reset);} }
        Self{channels,link:[[0;3];2],auto_return:[true;3],config,done:[[0;3];2],start:0,end:u32::MAX}
    }
    fn half(off:u32)->Option<(bool,usize,u32)> {
        let rel=off.checked_sub(0x70)?;let ch=(rel/0xc0) as usize;let k=rel%0xc0;
        if ch>=3{return None;}
        if k<=0x30{Some((false,ch,k))}else if (0x60..=0x90).contains(&k){Some((true,ch,k-0x60))}else{None}
    }
    fn plain(off:u32)->bool {matches!(off,0x64|0x68|0x2dc|0x2e0|0x304|0x308|0x32c|0x330|0x354|0x358|0x37c|0x380|0x3a4|0x3a8|0x3dc|0x400|0x404)}
    pub fn read(&self,off:u32)->Option<u32> {
        transport_regs::config(1,off)?;
        if let Some((out,ch,k))=Self::half(off) {
            let running=if out{self.channels.out[ch].running}else{self.channels.inp[ch].running};
            return Some(match k {
                0x10=>if out{u32::from(!running)<<3}else{u32::from(self.auto_return[ch])|u32::from(!running)<<4},
                0x08=>transport_regs::config(1,off)?.0, // no fictional FIFO prefetch; SPI2 has a validated readiness override
                0x0c=>return None, // CPU FIFO push/pop is outside this linked-descriptor subset
                _=>self.channels.read(GdmaAhb::channel(off)?),
            });
        }
        Some(match off {
            0..=0x5c=>self.channels.read(GdmaAhb::map_c6(off)?),
            0x3ac..=0x3b4=>self.link[0][((off-0x3ac)/4) as usize],
            0x3b8..=0x3c0=>self.link[1][((off-0x3b8)/4) as usize],
            0x3c4=>self.start,0x3c8=>self.end,
            0x410..=0x424=>{let n=((off-0x410)/4) as usize;self.done[n%2][n/2]},
            _ if Self::plain(off)=>self.config.read(off),
            _=>return None,
        })
    }
    pub fn write(&mut self,off:u32,value:u32)->bool {
        let Some((_,mask))=transport_regs::config(1,off)else{return false;};let v=value&mask;
        if let Some((out,ch,k))=Self::half(off) {
            if k==0x0c {return false;}
            if k==0x10 {
                let (stop,start,restart)=if out{(v&1!=0,v&2!=0,v&4!=0)}else{(v&2!=0,v&4!=0,v&8!=0)};
                let addr=self.link[usize::from(out)][ch];
                if out {let c=&mut self.channels.out[ch];if start||restart {c.desc=addr;c.buf_pos=0;c.running=c.conf0&1==0;}
                if stop{c.running=false;}}
                else {self.auto_return[ch]=v&1!=0;let c=&mut self.channels.inp[ch];if start||restart{c.desc=addr;c.buf_pos=0;c.rx_eof_pos=0;c.running=c.conf0&1==0;}
                if stop{c.running=false;}}
                return true;
            }
            if let Some(shared)=GdmaAhb::channel(off) {
                self.channels.write(shared,v);
                // C5 reset is held until cleared, unlike the older shared self-clear convention.
                if k==0 {if out{self.channels.out[ch].conf0=v;}else{self.channels.inp[ch].conf0=v;}}
                return true;
            }
        }
        match off {
            0..=0x5c=>{
                if off.is_multiple_of(16) {if off<0x30{self.channels.inp[(off/16) as usize].int_raw&=!v;}else{self.channels.out[((off-0x30)/16) as usize].int_raw&=!v;}}
                else if let Some(shared)=GdmaAhb::map_c6(off){self.channels.write(shared,v);}else{return false;}
            },
            0x3ac..=0x3b4=>self.link[0][((off-0x3ac)/4) as usize]=v,
            0x3b8..=0x3c0=>self.link[1][((off-0x3b8)/4) as usize]=v,
            0x3c4=>self.start=v,0x3c8=>self.end=v,
            0x410..=0x424=>{},
            _ if Self::plain(off)=>self.config.write(off,v),
            _=>return false,
        } true
    }
    pub fn out(&self)->Option<usize>{(0..3).find(|&n|{let c=&self.channels.out[n];c.running&&c.peri_sel==1&&c.conf0&0x43==0})}
    pub fn input(&self)->Option<usize>{(0..3).find(|&n|{let c=&self.channels.inp[n];c.running&&c.peri_sel==1&&c.conf0&0x33==0})}
    pub fn fifo(&self,ch:usize,bus:&mut C5Bus)->u32 {
        let c=&self.channels.out[ch];let off=0xd8+ch as u32*0xc0;
        let mut available=0;
        if c.running&&c.peri_sel==1&&c.conf0&0x43==0 {
            let mut mem=DmaMemory::new(bus,self.start,self.end,c.conf0);let mut walk=DescriptorWalk::new(1);
            if let Ok((_,d))=walk.read(&mut mem,c.desc) {
                if d.length<=d.size&&c.buf_pos<d.length&&(c.conf1&(1<<12)==0||d.owner_dma){
                    let n=(d.length-c.buf_pos).min(32);if d.buf.checked_add(c.buf_pos).is_some_and(|a|mem.range_valid(a,n as usize,false)){available=n;}
                }
            }
        }
        (transport_regs::config(1,off).map_or(0,|v|v.0)&!0xff)|(available<<2)|(u32::from(available==0)*2)
    }
    /// A complete SPI2 request either gets its exact length or no board callback.
    /// Descriptors consumed before an error retain their real progress/ownership.
    pub fn transmit(&mut self,bus:&mut C5Bus,want:usize)->Option<Result<Vec<u8>,()>> {
        let ch=self.out()?;let mut c=self.channels.out[ch];let mut data=Vec::with_capacity(want);
        let mut walk=DescriptorWalk::new(GDMA_DESCRIPTOR_STEP_BUDGET);let mut visited=std::collections::HashSet::new();
        let mut mem=DmaMemory::new(bus,self.start,self.end,c.conf0);let mut failed=false;
        while c.running&&c.desc!=0&&data.len()<want {
            if !visited.insert(c.desc){failed=true;break;}
            let Ok((control,d))=walk.read(&mut mem,c.desc)else{failed=true;break;};
            if d.length>d.size||c.buf_pos>d.length||(c.conf1&(1<<12)!=0&&!d.owner_dma){failed=true;break;}
            let remaining=(d.length-c.buf_pos) as usize;let count=remaining.min(want-data.len());
            let Some(address)=d.buf.checked_add(c.buf_pos)else{failed=true;break;};
            if !mem.range_valid(address,count,false){failed=true;break;}
            let mut bytes=Vec::with_capacity(count);
            for i in 0..count {match mem.read8(address+i as u32){Ok(v)=>bytes.push(v),Err(_)=>{failed=true;break;}}}
            if failed{break;} data.extend(bytes);c.buf_pos+=count as u32;
            if count<remaining{break;}
            if c.conf0&4!=0&&mem.write32(c.desc,control&!(1<<31)).is_err(){failed=true;break;}
            c.int_raw|=1;self.done[1][ch]=c.desc;
            if d.eof{c.int_raw|=2;c.eof_desc=c.desc;}
            c.desc=d.next;c.buf_pos=0;
            if d.eof||d.next==0{c.running=false;c.int_raw|=8;break;}
        }
        failed|=data.len()!=want;
        if failed{c.running=false;c.int_raw|=4;}
        self.channels.out[ch]=c;Some(if failed{Err(())}else{Ok(data)})
    }
    pub fn receive(&mut self,bus:&mut C5Bus,data:&[u8])->bool {
        let Some(ch)=self.input()else{return false;};let mut c=self.channels.inp[ch];
        let mut mem=DmaMemory::new(bus,self.start,self.end,(c.conf0&0xc0)<<2);let mut changed=false;let desc=c.desc;
        let ok=c.receive(&mut mem,data,Some(data.len() as u32),true,|a|a>=0x6000_0000,&mut changed);
        if ok{self.done[0][ch]=if c.eof_desc!=0{c.eof_desc}else{desc};c.running=false;}
        self.channels.inp[ch]=c;ok
    }
}

/// DMA never invokes CPU MMIO or records an unrelated CPU access fault.
/// Full-address bounds are inclusive; external bursts above32 bytes are refused.
struct DmaMemory<'a>{bus:&'a mut C5Bus,start:u32,end:u32,burst:u32}
impl<'a>DmaMemory<'a>{
    fn new(bus:&'a mut C5Bus,start:u32,end:u32,conf:u32)->Self{Self{bus,start,end,burst:(conf>>8)&3}}
    fn at(address:u32,len:usize,base:u32,size:usize)->Option<usize>{let n=address.checked_sub(base)? as usize;n.checked_add(len).filter(|&end|end<=size).map(|_|n)}
    fn range_valid(&self,address:u32,len:usize,write:bool)->bool {
        if len==0{return address>=self.start&&address<=self.end;}
        let Some(last)=address.checked_add(len as u32-1)else{return false;};
        if address<self.start||last>self.end{return false;}
        if Self::at(address,len,SRAM_BASE,self.bus.sram.len()).is_some()||Self::at(address,len,LP_BASE,self.bus.lp_sram.len()).is_some(){return true;}
        if !write&&Self::at(address,len,ROM_BASE,self.bus.rom.len()).is_some(){return true;}
        // Check every mapped page boundary, not just endpoints across discontiguous MMU entries.
        let mut at=address;
        loop{
            let Some((psram,off))=self.bus.mspi.translate(at)else{return false;};
            let cap=if psram{self.bus.psram.len()}else{self.bus.flash.len()};
            if off>=cap||(psram&&(self.burst==3||!self.bus.mspi.psram_access(write)))||(!psram&&write){return false;}
            if at==last{return true;}let boundary=(at|0x1fff).saturating_add(1);at=boundary.min(last);
        }
    }
    fn read(&self,a:u32,width:usize)->Result<u32,Fault>{
        if a&(width as u32-1)!=0{return Err(Fault::Misaligned);}
        if !self.range_valid(a,width,false){return Err(Fault::Prohibited);}
        let mut bytes=[0;4];
        for (i,b) in bytes.iter_mut().enumerate().take(width){let a=a+i as u32;
            *b=if let Some(n)=Self::at(a,1,SRAM_BASE,self.bus.sram.len()){self.bus.sram[n]}
            else if let Some(n)=Self::at(a,1,LP_BASE,self.bus.lp_sram.len()){self.bus.lp_sram[n]}
            else if let Some(n)=Self::at(a,1,ROM_BASE,self.bus.rom.len()){self.bus.rom[n]}
            else{let (ram,n)=self.bus.mspi.translate(a).ok_or(Fault::Unmapped)?;if ram{self.bus.psram[n]}else{self.bus.flash[n]}};
        }Ok(u32::from_le_bytes(bytes))
    }
    fn write(&mut self,a:u32,v:u32,width:usize)->Result<(),Fault>{
        if a&(width as u32-1)!=0{return Err(Fault::Misaligned);}
        if !self.range_valid(a,width,true){return Err(Fault::Prohibited);}
        for (i,b) in v.to_le_bytes().iter().enumerate().take(width){let a=a+i as u32;
            if let Some(n)=Self::at(a,1,SRAM_BASE,self.bus.sram.len()){self.bus.sram[n]= *b;}
            else if let Some(n)=Self::at(a,1,LP_BASE,self.bus.lp_sram.len()){self.bus.lp_sram[n]= *b;}
            else{let (_,n)=self.bus.mspi.translate(a).ok_or(Fault::Unmapped)?;self.bus.psram[n]= *b;}
        }Ok(())
    }
}
impl Bus for DmaMemory<'_>{
    fn note_code_page(&mut self,_a:u32){}
    fn fetch(&mut self,_a:u32)->Result<[u8;4],Fault>{Err(Fault::Prohibited)}
    fn read8(&mut self,a:u32)->Result<u8,Fault>{self.read(a,1).map(|v|v as u8)}
    fn read16(&mut self,a:u32)->Result<u16,Fault>{self.read(a,2).map(|v|v as u16)}
    fn read32(&mut self,a:u32)->Result<u32,Fault>{self.read(a,4)}
    fn write8(&mut self,a:u32,v:u8)->Result<(),Fault>{self.write(a,v as u32,1)}
    fn write16(&mut self,a:u32,v:u16)->Result<(),Fault>{self.write(a,v as u32,2)}
    fn write32(&mut self,a:u32,v:u32)->Result<(),Fault>{self.write(a,v,4)}
}
