//! S2 data-only external RAM mapping subset. ESP-IDF v5.5.4 ESP32-S2
//! reg_base.h, ext_mem_defs.h, mmu_ll.h and extmem_reg.h, retained under
//! docs/research/waveshare-classic-s2-2026-11-24/esp32-s2/psram-sources.
//! 384 raw64KiB entries; DBUS0/1/2 mask + DCACHE enable; explicit SPIRAM target.
//! No cache tags/latency, SPI RAM initialization/RDID, instruction PSRAM,
//! flash mapping, ROM or ESP-IDF boot is implemented by these data accesses.
use emu_core::Fault;
pub const EXTMEM:u32=0x61800000;
pub const MMU:u32=0x61801000;
const INVALID:u32=1<<14;
const FLASH:u32=1<<15;
const SPIRAM:u32=1<<16;
pub struct Psram {pub(crate) bytes:Vec<u8>,mmu:[u32;384],enabled:bool,mask:u8}
impl Default for Psram {fn default()->Self {Self{bytes:Vec::new(),mmu:[INVALID;384],enabled:false,mask:7}}}
impl Psram {
    pub fn configure(&mut self,size:usize)->Result<(),String> {
        if !matches!(size,0|8_388_608) {return Err("S2 diagnostic data-PSRAM supports only0 or8MiB; no SPI initialization or ROM boot".into());}
        self.bytes=vec![0;size];self.mmu.fill(INVALID);self.enabled=false;self.mask=7;Ok(())
    }
    pub fn len(&self)->usize {self.bytes.len()}
    pub fn is_empty(&self)->bool {self.bytes.is_empty()}
    fn entry(address:u32)->Option<usize> {address.checked_sub(MMU).filter(|&v|v<384*4&&v&3==0).map(|v|(v/4)as usize)}
    fn physical(&self,address:u32,width:u8)->Result<usize,Fault> {
        let bus=match address {0x3fc00000..=0x3ff7ffff=>0,0x3f800000..=0x3fbfffff=>1,0x3f500000..=0x3f7fffff=>2,_=>return Err(Fault::Unmapped)};
        if !self.enabled||self.mask&(1<<bus)!=0 {return Err(Fault::Unmapped);}
        let entry=self.mmu[192+bus*64+((address&0x3fffff)>>16)as usize];
        if entry&(INVALID|FLASH|SPIRAM)!=SPIRAM {return Err(Fault::Unmapped);}
        let offset=(((entry&0x3fff)as usize)<<16)|(address as usize&0xffff);
        if offset.checked_add(usize::from(width)).is_none_or(|end|end>self.bytes.len()) {return Err(Fault::Unmapped);}
        Ok(offset)
    }
    pub fn read(&self,address:u32,width:u8)->Option<Result<u32,Fault>> {
        if width==4 {
            if address==EXTMEM {return Some(Ok(u32::from(self.enabled)));}
            if address==EXTMEM+4 {return Some(Ok(u32::from(self.mask)));}
            if let Some(i)=Self::entry(address) {return Some(Ok(self.mmu[i]));}
        }
        if !(0x3f500000..0x3ff80000).contains(&address) {return None;}
        Some(self.physical(address,width).map(|offset|{let mut value=[0;4];value[..usize::from(width)].copy_from_slice(&self.bytes[offset..offset+usize::from(width)]);u32::from_le_bytes(value)}))
    }
    pub fn write(&mut self,address:u32,value:u32,width:u8)->Option<Result<(),Fault>> {
        if width==4 {
            if address==EXTMEM {self.enabled=value&1!=0;return Some(Ok(()));}
            if address==EXTMEM+4 {self.mask=(value&7)as u8;return Some(Ok(()));}
            if let Some(i)=Self::entry(address) {self.mmu[i]=value&0x1ffff;return Some(Ok(()));}
        }
        if !(0x3f500000..0x3ff80000).contains(&address) {return None;}
        Some(self.physical(address,width).map(|offset|self.bytes[offset..offset+usize::from(width)].copy_from_slice(&value.to_le_bytes()[..usize::from(width)])))
    }
}
