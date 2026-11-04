//! Explicit C5 SRAM app-direct contract. This is not a flash-cache/IDF bootloader.
//! Header layout: ESP-IDF v5.5.4 bootloader_support/include/esp_app_format.h.
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
fn sha256(data: &[u8]) -> Vec<u8> {
    // Host loader validation using the existing pure SHA block implementation;
    // this does not expose or claim a C5 guest SHA peripheral.
    let mut padded = data.to_vec(); padded.push(0x80);
    while padded.len() % 64 != 56 { padded.push(0); }
    padded.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    let mut sha = esp_periph::Sha::new();
    for (i, block) in padded.as_chunks::<64>().0.iter().enumerate() { sha.hash_block(block, i == 0); }
    sha.h[..8].iter().flat_map(|word| word.to_be_bytes()).collect()
}
