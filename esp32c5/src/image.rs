//! C5 MP image validation; SRAM app-direct and unsigned Flash containers have separate contracts.
//! SRAM contract: IDF v5.5.4; Flash format/loader rules: v5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0.
use crate::bus::{LP_BASE, LP_SIZE, SRAM_BASE, SRAM_SIZE};
fn ram(addr: u32, len: u32) -> bool {
    [(SRAM_BASE, SRAM_SIZE), (LP_BASE, LP_SIZE)].into_iter().any(|(base, size)|
        addr >= base && addr.checked_add(len).is_some_and(|end| end <= base + size as u32))
}
pub fn parse(data: &[u8]) -> Result<esp_soc::image::AppImage, String> {
    let h = data.get(..24).ok_or("C5 app-direct needs a complete ESP image header")?;
    if h[0] != 0xe9 || h[12..14] != [0x17, 0] { return Err("C5 app-direct requires ESP32-C5 chip ID0x17, not another chip's image".into()); }
    if !(1..=16).contains(&h[1]) { return Err("C5 app-direct needs1..16 SRAM segments".into()); }
    // v5.5.4 bootloader_common_loader.c uses the full field; byte14 is deprecated
    // (esp_app_format.h) and must not be misread as a minor revision on C5.
    let min = u16::from_le_bytes([h[15], h[16]]);
    let max = u16::from_le_bytes([h[17], h[18]]);
    if min > 100 || (max != 0 && max < 100) { return Err("C5 app-direct models MP rev1.0; the image excludes revision100".into()); }
    let image = esp_soc::image::parse(data)?;
    let mut end = 24usize; let mut checksum = 0xef;
    for (i, s) in image.segments.iter().enumerate() {
        if s.len == 0 || !ram(s.load_addr, s.len) { return Err(format!("C5 app-direct only supports SRAM segments; {:#010x}+{:#x} needs unimplemented flash/MMU or peripherals", s.load_addr, s.len)); }
        if s.load_addr & 3 != 0 || s.len & 3 != 0 { return Err("C5 app-direct segment addresses and lengths must be4-byte aligned".into()); }
        if image.segments[..i].iter().any(|old| s.load_addr < old.load_addr + old.len && old.load_addr < s.load_addr + s.len) { return Err("C5 app-direct rejects overlapping segments".into()); }
        end = s.file_off as usize + s.len as usize;
        checksum = data[s.file_off as usize..end].iter().fold(checksum, |c, b| c ^ b);
    }
    if image.entry & 1 != 0 || !image.segments.iter().any(|s| image.entry >= s.load_addr && image.entry.checked_add(2).is_some_and(|end| end <= s.load_addr + s.len)) { return Err("C5 app-direct entry must be aligned and inside a loaded SRAM segment".into()); }
    let checksum_at = end.checked_add(16).ok_or("C5 image size overflows")? / 16 * 16 - 1;
    if data.get(checksum_at) != Some(&checksum) { return Err("C5 app image checksum is missing or invalid".into()); }
    if data[end..checksum_at].iter().any(|&b| b != 0) { return Err("C5 app image checksum padding must be zero".into()); }
    match h[23] {
        0 => {},
        1 => {
            let digest = data.get(checksum_at + 1..checksum_at + 33).ok_or("C5 image SHA-256 digest is truncated")?;
            if sha256(&data[..=checksum_at]).as_slice() != digest { return Err("C5 app image SHA-256 digest is invalid".into()); }
        }
        _ => return Err("C5 image hash-appended flag must be0or1".into()),
    }
    let footer_end = checksum_at + 1 + usize::from(h[23]) * 32;
    if data[footer_end..].iter().any(|&b| b != 0xff) { return Err("C5 app-direct only accepts erased0xff bytes after its image footer".into()); }
    Ok(image)
}
/// C5 IDF v5.5.3 bootloader Kconfig / ESP_PRIMARY_BOOTLOADER_OFFSET.
pub const BOOTLOADER_OFFSET: usize = 0x2000;
const FLASH_LIMIT: usize = 32 << 20;
const MAPPED_LOW: u32 = 0x4200_0000;
const MAPPED_HIGH: u32 = 0x4400_0000;
const PAGE: usize = 0x10000;

pub struct ValidatedImage { pub image: esp_soc::image::AppImage, pub image_bytes: usize }
#[derive(Debug, PartialEq, Eq)]
pub struct AppPartition { pub offset: usize, pub size: usize, pub subtype: u8, pub image_bytes: usize, pub entry: u32 }
#[derive(Debug, PartialEq, Eq)]
pub struct FlashLayout { pub bootloader_offset: usize, pub partition_table_offset: usize, pub apps: Vec<AppPartition> }

/// One C5 MP ESP image, followed only by erased bytes. This does not boot it or
/// validate secure-boot signatures. Absolute mapped alignment needs the Flash
/// placement supplied to `validate_merged`, not a guessed app-direct offset.
pub fn parse_flash(data: &[u8]) -> Result<ValidatedImage, String> {
    if data.len() > FLASH_LIMIT { return Err("C5 single image exceeds the32MiB Flash window".into()); }
    let image = flash_prefix(data)?;
    if data[image.image_bytes..].iter().any(|&b| b != 0xff) { return Err("C5 single Flash image accepts only erased0xff bytes after its footer; use the merged layout for a container".into()); }
    Ok(image)
}

fn skipped(address: u32) -> bool { matches!(address, 0 | 4) }
fn mapped(address: u32) -> bool { (MAPPED_LOW..MAPPED_HIGH).contains(&address) }
fn flash_prefix(data: &[u8]) -> Result<ValidatedImage, String> {
    // IDF v5.5.3 esp_app_format.h and bootloader_common_loader.c: full revision
    // fields, including 0/0xffff as an unset maximum. Byte14 is deprecated.
    let h = data.get(..24).ok_or("C5 Flash image header is truncated")?;
    if h[0] != 0xe9 || h[12..14] != [0x17, 0] { return Err("C5 Flash image requires chip ID0x17".into()); }
    if !(1..=16).contains(&h[1]) { return Err("C5 Flash image needs1..16 segments".into()); }
    let min = u16::from_le_bytes([h[15], h[16]]); let max = u16::from_le_bytes([h[17], h[18]]);
    if min > 100 || (max != 0 && max != 0xffff && max < 100) { return Err("C5 Flash image excludes MP revision100".into()); }
    if h[23] > 1 { return Err("C5 image hash-appended flag must be0or1".into()); }
    let image = esp_soc::image::parse(data)?;
    let mut ranges = Vec::<std::ops::Range<u32>>::new(); let mut end = 24usize;
    for s in &image.segments {
        end = (s.file_off as usize).checked_add(s.len as usize).filter(|&n| n <= FLASH_LIMIT).ok_or("C5 image exceeds the32MiB Flash window")?;
        if s.load_addr & 3 != 0 || s.len & 3 != 0 { return Err("C5 Flash segment addresses and lengths must be4-byte aligned".into()); }
        // esp_image_format.c::should_load: 0 is a padding block, 4 is reserved
        // for an unused MD5 block. Neither is RAM, but their bytes are hashed.
        if skipped(s.load_addr) { continue; }
        let high = s.load_addr.checked_add(s.len).ok_or("C5 segment address overflows")?;
        // Exact C5 v5.5.3 soc.h: HP384KiB, LP16KiB, shared IROM/DROM32MiB.
        if !ram(s.load_addr, s.len) && !(mapped(s.load_addr) && high <= MAPPED_HIGH) { return Err(format!("C5 Flash segment {:#010x}+{:#x} is outside SRAM/LP/Flash", s.load_addr, s.len)); }
        if s.len != 0 {
            if ranges.iter().any(|r| r.start < high && s.load_addr < r.end) { return Err("C5 Flash image has overlapping loaded segments".into()); }
            ranges.push(s.load_addr..high);
        }
    }
    if image.entry & 1 != 0 || !ranges.iter().any(|r| image.entry >= r.start && image.entry.checked_add(2).is_some_and(|end| end <= r.end)) { return Err("C5 Flash entry must be aligned and inside a loaded segment".into()); }
    let checksum = image.segments.iter().fold(0xef, |sum, s| data[s.file_off as usize..s.file_off as usize + s.len as usize].iter().fold(sum, |a, b| a ^ b));
    let checksum_at = end.checked_add(16).ok_or("C5 image footer overflows")? / 16 * 16 - 1;
    if data.get(checksum_at) != Some(&checksum) { return Err("C5 Flash image checksum is missing or invalid".into()); }
    // process_checksum hashes the complete padding, without requiring zeros.
    // C5 has SOC_SPI_MEM_SUPPORT_CACHE_32BIT_ADDR_MAP, so no legacy16MiB cap.
    let image_bytes = checksum_at + 1 + usize::from(h[23]) * 32;
    if h[23] == 1 {
        let digest = data.get(checksum_at + 1..image_bytes).ok_or("C5 Flash SHA-256 digest is truncated")?;
        if sha256(&data[..=checksum_at]).as_slice() != digest { return Err("C5 Flash SHA-256 digest is invalid".into()); }
    }
    Ok(ValidatedImage { image, image_bytes })
}

fn mapped_alignment(image: &esp_soc::image::AppImage, offset: usize) -> Result<(), String> {
    // C5 v5.5.3 ext_mem_defs.h / esp_image_format.c::verify_segment_header:
    // shared I/D mapping, fixed default64KiB pages (no configurable-page cap).
    for s in &image.segments {
        if mapped(s.load_addr) && offset.checked_add(s.file_off as usize).is_none_or(|at| at % PAGE != s.load_addr as usize % PAGE) { return Err("C5 mapped segment does not match its physical64KiB Flash page offset".into()); }
    }
    Ok(())
}

/// One standalone image at its actual physical offset, before Flash staging.
fn declared_flash_fits(data:&[u8],flash_bytes:usize)->Result<(),String>{
    let size=data[3]>>4;
    if size>5||(1usize<<20)<<size>flash_bytes{return Err("C5 image declares a Flash capacity larger than the physical board".into());}Ok(())
}
pub fn validate_at(data:&[u8],flash_bytes:usize,offset:usize)->Result<ValidatedImage,String>{
    let image=parse_flash(data)?;
    if offset.checked_add(data.len()).is_none_or(|end|end>flash_bytes){return Err("C5 image placement exceeds physical Flash".into());}
    declared_flash_fits(data,flash_bytes)?;
    mapped_alignment(&image.image,offset)?;Ok(image)
}

/// Raw merged inputs carry no table-offset parameter. Require one completely
/// validated aligned table; never guess among multiple plausible layouts.
pub fn discover_merged(data:&[u8],flash_bytes:usize)->Result<FlashLayout,String>{
    if flash_bytes>FLASH_LIMIT||data.len()>flash_bytes{return Err("C5 merged image exceeds physical Flash".into());}
    let mut found=None;let mut candidates=0;let mut last=None;
    for at in (0x8000..data.len()).step_by(0x1000){
        if data.get(at..at+2)!=Some(&[0xaa,0x50]){continue;}
        candidates+=1;if candidates>16{return Err("C5 merged image has too many table-like sectors; provide an explicit minimal build bundle".into());}
        match validate_merged(data,flash_bytes,at){
            Ok(layout)=>{if found.is_some(){return Err("C5 merged image has multiple valid partition tables; layout is ambiguous".into());}found=Some(layout);}
            Err(error)=>last=Some(error),
        }
    }
    found.ok_or_else(||format!("C5 merged image has no validated partition table: {}",last.unwrap_or_else(||"no aligned table found".into())))
}

/// Validate a merged unsigned C5 Flash container without changing guest state.
/// Partition offsets come from the caller's build context and table; this does
/// not select an OTA slot, verify signatures, or require data partitions in BIN.
/// Only the primary bootloader and populated apps are inspected; other payloads
/// stay opaque. Partition records in the reserved bootloader/table area are rejected.
pub fn validate_merged(data: &[u8], flash_bytes: usize, partition_table_offset: usize) -> Result<FlashLayout, String> {
    if flash_bytes == 0 || flash_bytes > FLASH_LIMIT || !flash_bytes.is_multiple_of(0x1000) || data.len() > flash_bytes { return Err("C5 merged image exceeds its sector-aligned physical Flash capacity (maximum32MiB)".into()); }
    let table_end = partition_table_offset.checked_add(0x1000).filter(|&end| end <= flash_bytes).ok_or("C5 partition table exceeds physical Flash")?;
    if partition_table_offset < 0x8000 || !partition_table_offset.is_multiple_of(0x1000) { return Err("C5 partition table offset must be sector aligned at0x8000orabove".into()); }
    let table = data.get(partition_table_offset..partition_table_offset + 0xc00).ok_or("C5 merged image has a truncated partition table")?;
    let boot = flash_prefix(data.get(BOOTLOADER_OFFSET..partition_table_offset).ok_or("C5 bootloader at0x2000 is missing")?).map_err(|e| format!("C5 bootloader at0x2000: {e}"))?;
    declared_flash_fits(&data[BOOTLOADER_OFFSET..],flash_bytes)?;
    mapped_alignment(&boot.image, BOOTLOADER_OFFSET)?;
    // IDF v5.5.3 esp_flash_partitions.h / flash_partitions.c: 32-byte
    // records, at most0xc00 bytes, optional MD5 at+16 and a required terminator.
    let mut parts = Vec::<(usize, usize, u8, u8)>::new(); let mut md5_seen = false; let mut terminated = false;
    for (i, e) in table.as_chunks::<32>().0.iter().enumerate() {
        let magic = u16::from_le_bytes([e[0], e[1]]);
        if magic == 0x50aa {
            if md5_seen { return Err("C5 partition record follows its MD5 checksum".into()); }
            let offset = u32::from_le_bytes([e[4], e[5], e[6], e[7]]) as usize; let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
            let end = offset.checked_add(size).filter(|&end| end <= flash_bytes).ok_or("C5 partition exceeds physical Flash")?;
            if size == 0 || offset < table_end || !offset.is_multiple_of(if e[2] == 0 { PAGE } else { 0x1000 }) || e[2] == 0 && !size.is_multiple_of(0x1000) { return Err("C5 partition is empty, misaligned, or overlaps the bootloader/table region".into()); }
            if parts.iter().any(|&(at, len, _, _)| offset < at + len && at < end) { return Err("C5 partition table has overlapping partitions".into()); }
            if e[2] == 0 && parts.iter().any(|&(_, _, ty, sub)| ty == 0 && sub == e[3]) { return Err("C5 partition table has duplicate app subtypes".into()); }
            parts.push((offset, size, e[2], e[3]));
        } else if magic == 0xebeb {
            if md5_seen { return Err("C5 partition table has duplicate MD5 records".into()); }
            if md5(&table[..i * 32]).as_slice() != &e[16..32] { return Err("C5 partition table MD5 is invalid".into()); }
            md5_seen = true;
        } else if e[..4] == [0xff; 4] && !parts.is_empty() { terminated = true; break; }
        else { return Err("C5 partition table has invalid magic or is empty".into()); }
    }
    if !terminated { return Err("C5 partition table has no terminating entry".into()); }
    let mut apps = Vec::new();
    for (offset, size, ty, subtype) in parts {
        if ty != 0 || offset >= data.len() { continue; }
        let bytes = &data[offset..(offset + size).min(data.len())];
        if bytes.iter().all(|&b| b == 0xff) { continue; } // unprovided/erased OTA slots are not corrupt apps
        let app = flash_prefix(bytes).map_err(|e| format!("C5 app partition at{offset:#x}: {e}"))?;
        declared_flash_fits(bytes,flash_bytes)?;
        mapped_alignment(&app.image, offset)?;
        apps.push(AppPartition { offset, size, subtype, image_bytes: app.image_bytes, entry: app.image.entry });
    }
    if apps.is_empty() { return Err("C5 merged image has no populated app at a declared partition offset".into()); }
    Ok(FlashLayout { bootloader_offset: BOOTLOADER_OFFSET, partition_table_offset, apps })
}

// RFC1321 section3: the legacy IDF partition-table checksum, not authentication.
// Private cold loader code: no guest MD5 peripheral or additional dependency.
fn md5(data: &[u8]) -> [u8; 16] {
    const K: [u32; 64] = [
        0xd76aa478,0xe8c7b756,0x242070db,0xc1bdceee,0xf57c0faf,0x4787c62a,0xa8304613,0xfd469501,
        0x698098d8,0x8b44f7af,0xffff5bb1,0x895cd7be,0x6b901122,0xfd987193,0xa679438e,0x49b40821,
        0xf61e2562,0xc040b340,0x265e5a51,0xe9b6c7aa,0xd62f105d,0x02441453,0xd8a1e681,0xe7d3fbc8,
        0x21e1cde6,0xc33707d6,0xf4d50d87,0x455a14ed,0xa9e3e905,0xfcefa3f8,0x676f02d9,0x8d2a4c8a,
        0xfffa3942,0x8771f681,0x6d9d6122,0xfde5380c,0xa4beea44,0x4bdecfa9,0xf6bb4b60,0xbebfbc70,
        0x289b7ec6,0xeaa127fa,0xd4ef3085,0x04881d05,0xd9d4d039,0xe6db99e5,0x1fa27cf8,0xc4ac5665,
        0xf4292244,0x432aff97,0xab9423a7,0xfc93a039,0x655b59c3,0x8f0ccc92,0xffeff47d,0x85845dd1,
        0x6fa87e4f,0xfe2ce6e0,0xa3014314,0x4e0811a1,0xf7537e82,0xbd3af235,0x2ad7d2bb,0xeb86d391,
    ];
    const S: [u32; 16] = [7,12,17,22,5,9,14,20,4,11,16,23,6,10,15,21];
    let mut bytes = data.to_vec(); bytes.push(0x80); while bytes.len() % 64 != 56 { bytes.push(0); }
    bytes.extend_from_slice(&(data.len() as u64).wrapping_mul(8).to_le_bytes());
    let mut state = [0x67452301u32,0xefcdab89,0x98badcfe,0x10325476];
    for block in bytes.as_chunks::<64>().0 {
        let mut words = [0u32; 16]; for (w, b) in words.iter_mut().zip(block.as_chunks::<4>().0) { *w = u32::from_le_bytes(*b); }
        let [mut a, mut b, mut c, mut d] = state;
        for (i, k) in K.iter().enumerate() {
            let (f, g) = match i / 16 { 0 => ((b & c) | (!b & d), i), 1 => ((d & b) | (!d & c), (5 * i + 1) % 16), 2 => (b ^ c ^ d, (3 * i + 5) % 16), _ => (c ^ (b | !d), (7 * i) % 16) };
            let next = b.wrapping_add(a.wrapping_add(f).wrapping_add(*k).wrapping_add(words[g]).rotate_left(S[i / 16 * 4 + i % 4]));
            a = d; d = c; c = b; b = next;
        }
        for (s, v) in state.iter_mut().zip([a,b,c,d]) { *s = s.wrapping_add(v); }
    }
    let mut digest = [0u8; 16]; for (d, s) in digest.as_chunks_mut::<4>().0.iter_mut().zip(state) { *d = s.to_le_bytes(); } digest
}

pub(crate) fn sha256(data: &[u8]) -> Vec<u8> {
    // Host loader validation using the existing pure SHA block implementation;
    // this does not expose or claim a C5 guest SHA peripheral.
    let mut padded = data.to_vec(); padded.push(0x80);
    while padded.len() % 64 != 56 { padded.push(0); }
    padded.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    let mut sha = esp_periph::Sha::new();
    for (i, block) in padded.as_chunks::<64>().0.iter().enumerate() { sha.hash_block(block, i == 0); }
    sha.h[..8].iter().flat_map(|word| word.to_be_bytes()).collect()
}

#[cfg(test)]
mod tests;
