//! Transactional ESP image validator for the S2 diagnostic SRAM contract.
//! Layout: pinned ESP-IDF v5.5.4 esp_app_format.h; no ROM/SDK boot implication.
use crate::bus::{physical_range, IRAM_BASE, SRAM_SIZE};

pub fn parse(data: &[u8]) -> Result<esp_soc::image::AppImage, String> {
    let h = data.get(..24).ok_or("S2 app-direct needs a complete ESP image header")?;
    if h[0] != 0xe9 || h[12..14] != [2, 0] { return Err("S2 app-direct requires ESP32-S2 chip ID 2".into()); }
    if !(1..=16).contains(&h[1]) { return Err("S2 app-direct needs 1..16 SRAM segments".into()); }
    // Revision 0 is the diagnostic profile, not a claim about the board's silicon.
    // No revision-sensitive device behavior is modeled. A nonzero minimum is rejected.
    if h[14] != 0 || u16::from_le_bytes([h[15], h[16]]) != 0 { return Err("S2 diagnostic revision 0 is excluded by this image's minimum revision".into()); }
    let image = esp_soc::image::parse(data)?;
    let mut ranges = Vec::new(); let mut end = 24usize; let mut checksum = 0xef;
    for s in &image.segments {
        if s.len == 0 || s.load_addr & 3 != 0 || s.len & 3 != 0 { return Err("S2 SRAM segment addresses and nonzero lengths must be 4-byte aligned".into()); }
        let range = physical_range(s.load_addr, s.len as usize).ok_or_else(||format!("S2 app-direct rejects non-SRAM segment {:#010x}+{:#x}; ROM/RTC/MMU/flash/peripherals are unsupported", s.load_addr, s.len))?;
        if ranges.iter().any(|old: &std::ops::Range<usize>| range.start < old.end && old.start < range.end) { return Err("S2 app-direct rejects overlapping physical SRAM, including IRAM/DRAM aliases".into()); }
        ranges.push(range);
        end = s.file_off as usize + s.len as usize;
        checksum = data[s.file_off as usize..end].iter().fold(checksum, |c,b| c ^ b);
    }
    if image.entry & 3 != 0 || image.entry < IRAM_BASE || image.entry >= IRAM_BASE + SRAM_SIZE as u32 ||
        !image.segments.iter().any(|s| s.load_addr >= IRAM_BASE && image.entry >= s.load_addr && image.entry.checked_add(3).is_some_and(|end| end <= s.load_addr + s.len)) {
        return Err("S2 app-direct entry must be 4-byte aligned inside a loaded IRAM segment".into());
    }
    let checksum_at = end.checked_add(16).ok_or("S2 image size overflows")? / 16 * 16 - 1;
    if data.get(checksum_at) != Some(&checksum) { return Err("S2 image XOR checksum is missing or invalid".into()); }
    if data[end..checksum_at].iter().any(|&b| b != 0) { return Err("S2 checksum padding must be zero".into()); }
    match h[23] {
        0 => {},
        1 => {
            let digest = data.get(checksum_at+1..checksum_at+33).ok_or("S2 image SHA-256 digest is truncated")?;
            if sha256(&data[..=checksum_at]).as_slice() != digest { return Err("S2 image SHA-256 digest is invalid".into()); }
        }
        _ => return Err("S2 image hash-appended flag must be 0 or 1".into()),
    }
    let footer_end = checksum_at + 1 + usize::from(h[23]) * 32;
    if data[footer_end..].iter().any(|&b| b != 0xff) { return Err("S2 app-direct only accepts erased 0xff bytes after the image footer".into()); }
    Ok(image)
}
pub(crate) fn sha256(data: &[u8]) -> Vec<u8> {
    // Host validation using the existing pure SHA algorithm, not a guest peripheral.
    let mut padded = data.to_vec(); padded.push(0x80);
    while padded.len() % 64 != 56 { padded.push(0); }
    padded.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    let mut sha = esp_periph::Sha::new();
    for (i, block) in padded.as_chunks::<64>().0.iter().enumerate() { sha.hash_block(block, i == 0); }
    sha.h[..8].iter().flat_map(|word| word.to_be_bytes()).collect()
}
