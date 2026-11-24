//! Strict H2 chip16 /4MiB SRAM-only ESP image validation.
//! This host loader does not execute a ROM or second-stage bootloader.
use crate::bus::{FLASH_SIZE,LP_BASE,LP_SIZE,SRAM_BASE,SRAM_SIZE};
fn ram(addr:u32,len:u32)->bool{[(SRAM_BASE,SRAM_SIZE),(LP_BASE,LP_SIZE)].into_iter().any(|(base,size)|addr>=base&&addr.checked_add(len).is_some_and(|end|end<=base+size as u32))}
pub fn parse(data:&[u8])->Result<esp_soc::image::AppImage,String>{
 if data.len()>FLASH_SIZE{return Err("H2 app image exceeds the exact4MiB board flash capacity".into());}
 let h=data.get(..24).ok_or("H2 app-direct needs a complete ESP image header")?;
 if h[0]!=0xe9||h[12..14]!=[16,0]{return Err("H2 app-direct requires ESP32-H2 chip ID16, not a C6 or another chip's image".into());}
 if h[3]>>4>2{return Err("H2 app image must declare1,2or4MiB flash, within the exact4MiB board capacity".into());}
 if !(1..=16).contains(&h[1]){return Err("H2 app-direct requires1..16 SRAM segments".into());}
 let min=u16::from_le_bytes([h[15],h[16]]);let max=u16::from_le_bytes([h[17],h[18]]);
 // IDF ignores the deprecated legacy byte14. Neither board source proves the
 // wafer revision. Accept only an unrestricted
 // header; do not invent eFuse revision values to make a restricted image pass.
 if min!=0||!matches!(max,0|0xffff){return Err("H2 wafer revision is unverified; revision-restricted images are not accepted".into());}
 let image=esp_soc::image::parse(data)?;let mut end=24usize;let mut checksum=0xef;
 for(i,s)in image.segments.iter().enumerate(){
  if s.len==0||!ram(s.load_addr,s.len){return Err(format!("H2 app-direct only supports320KiB HP SRAM and4KiB LP SRAM; segment{:#010x}+{:#x} requires an unimplemented region (ROM/flash cache/MMIO)",s.load_addr,s.len));}
  if s.load_addr&3!=0||s.len&3!=0{return Err("H2 SRAM segment addresses and sizes must be4-byte aligned".into());}
  if image.segments[..i].iter().any(|old|s.load_addr<old.load_addr+old.len&&old.load_addr<s.load_addr+s.len){return Err("H2 app-direct rejects overlapping SRAM segments".into());}
  end=s.file_off as usize+s.len as usize;checksum=data[s.file_off as usize..end].iter().fold(checksum,|v,b|v^b);
 }
 if image.entry&1!=0||!image.segments.iter().any(|s|image.entry>=s.load_addr&&image.entry.checked_add(2).is_some_and(|end|end<=s.load_addr+s.len)){return Err("H2 app-direct entry must be aligned and inside a loaded SRAM segment".into());}
 let checksum_at=end.checked_add(16).ok_or("H2 image size overflow")?/16*16-1;
 if data.get(checksum_at)!=Some(&checksum){return Err("H2 image XOR checksum is missing or invalid".into());}
 if data[end..checksum_at].iter().any(|&b|b!=0){return Err("H2 image checksum padding must be zero".into());}
 match h[23]{0=>{},1=>{let digest=data.get(checksum_at+1..checksum_at+33).ok_or("H2 image SHA-256 digest is truncated")?;if sha256(&data[..=checksum_at]).as_slice()!=digest{return Err("H2 image SHA-256 digest is invalid".into());}},_=>return Err("H2 image hash-appended flag must be0or1".into())}
 let footer_end=checksum_at+1+usize::from(h[23])*32;
 if data[footer_end..].iter().any(|&b|b!=0xff){return Err("H2 app-direct requires erased0xff bytes after its image footer".into());}
 Ok(image)
}
/// Host-only validation; this does not expose an H2 SHA hardware device.
pub(crate) fn sha256(data:&[u8])->Vec<u8>{
 let mut padded=data.to_vec();padded.push(0x80);while padded.len()%64!=56{padded.push(0);}padded.extend_from_slice(&(data.len()as u64*8).to_be_bytes());
 let mut sha=esp_periph::Sha::new();for(i,block)in padded.as_chunks::<64>().0.iter().enumerate(){sha.hash_block(block,i==0);}sha.h[..8].iter().flat_map(|v|v.to_be_bytes()).collect()
}
