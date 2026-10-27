//! Reconstruct omitted ROM-side initializer payloads from the same ELF's RAM sections.
//! The fixed Espressif 20260528 S31 ROM _init/unpackloop uses 12-byte records:
//! (RAM start,RAM end,ROM source), bounded by _data_table_start/_data_table_end.
//! No instruction replacement or host-side execution of that copy loop occurs.
use crate::bus::{ROM_BASE, ROM_SIZE, SRAM_BASE, SRAM_SIZE};
use esp_soc::elf::Elf;
fn range(address: u32, size: usize, base: u32, capacity: usize) -> Result<std::ops::Range<usize>, String> {
    let start = address.checked_sub(base).ok_or("ROM initializer address below window")? as usize;
    let end = start.checked_add(size).filter(|&end| end <= capacity).ok_or("ROM initializer range exceeds window")?;
    Ok(start..end)
}
fn insert(image: &mut [u8], present: &mut [bool], address: u32, data: &[u8]) -> Result<(), String> {
    let r = range(address, data.len(), ROM_BASE, image.len())?;
    for (i, byte) in r.clone().zip(data) {
        if present[i] && image[i] != *byte { return Err("Conflicting S31 ROM ELF payloads/initializer overlap".into()); }
    }
    image[r.clone()].copy_from_slice(data); present[r].fill(true); Ok(())
}
pub(crate) fn image(elf: &Elf) -> Result<Vec<u8>, String> {
    let mut image = vec![0; ROM_SIZE]; let mut present = vec![false; ROM_SIZE];
    for seg in &elf.segments {
        if seg.vaddr == 0 && seg.flags == 4 { continue; } // debugger-only .eh_frame
        if seg.vaddr >= ROM_BASE {
            range(seg.vaddr, seg.memsz as usize, ROM_BASE, ROM_SIZE)?;
            insert(&mut image, &mut present, seg.vaddr, &seg.data)?;
        } else { range(seg.vaddr, seg.memsz as usize, SRAM_BASE, SRAM_SIZE)?; }
    }
    for section in &elf.sections {
        if section.is_bss || section.data.is_empty() { continue; }
        if section.addr >= ROM_BASE { insert(&mut image, &mut present, section.addr, &section.data)?; }
        else { range(section.addr, section.data.len(), SRAM_BASE, SRAM_SIZE)?; }
    }
    let start = *elf.by_name.get("_data_table_start").ok_or("S31 ROM ELF lacks _data_table_start")?;
    let end = *elf.by_name.get("_data_table_end").ok_or("S31 ROM ELF lacks _data_table_end")?;
    let len = end.checked_sub(start).ok_or("S31 ROM initializer table is reversed")? as usize;
    if !len.is_multiple_of(12) { return Err("S31 ROM initializer table is not 12-byte records".into()); }
    let table = range(start, len, ROM_BASE, ROM_SIZE)?;
    if present[table.clone()].contains(&false) { return Err("S31 ROM initializer table has missing bytes".into()); }
    // Copy records before inserting data so a malformed source overlapping the table cannot
    // change the remaining control records. Conflicting existing bytes are always rejected.
    let records = image[table].to_vec();
    for record in records.as_chunks::<12>().0 {
        let word = |i| u32::from_le_bytes(record[i..i + 4].try_into().unwrap());
        let (dest, end, source) = (word(0), word(4), word(8));
        let count = end.checked_sub(dest).ok_or("S31 ROM initializer destination is reversed")? as usize;
        if count == 0 { continue; }
        if dest & 3 != 0 || source & 3 != 0 || count & 3 != 0 { return Err("S31 ROM initializer is not word aligned".into()); }
        range(dest, count, SRAM_BASE, SRAM_SIZE)?; range(source, count, ROM_BASE, ROM_SIZE)?;
        let section = elf.sections.iter().find(|s| !s.is_bss && s.addr == dest && s.data.len() == count)
            .ok_or("S31 ROM initializer has no matching explicit ELF RAM payload")?;
        insert(&mut image, &mut present, source, &section.data)?;
    }
    Ok(image)
}
#[cfg(test)]
mod tests {
    use super::*;
    use esp_soc::elf::Section;
    fn fixture() -> Elf {
        let mut e = Elf::default(); e.by_name.insert("_data_table_start".into(), ROM_BASE + 0x20); e.by_name.insert("_data_table_end".into(), ROM_BASE + 0x2c);
        let data: Vec<_> = [SRAM_BASE + 0x100, SRAM_BASE + 0x104, ROM_BASE + 0x100].into_iter().flat_map(u32::to_le_bytes).collect();
        e.sections.push(Section { name: ".text.table".into(), addr: ROM_BASE + 0x20, data, is_bss: false });
        e.sections.push(Section { name: ".data.fixture".into(), addr: SRAM_BASE + 0x100, data: 0x12345678u32.to_le_bytes().to_vec(), is_bss: false }); e
    }
    #[test]
    fn reconstructs_only_explicit_payload_and_rejects_missing_or_conflicting_data() {
        let e = fixture(); assert_eq!(&image(&e).unwrap()[0x100..0x104], &0x12345678u32.to_le_bytes());
        let mut e = fixture(); e.sections.pop(); assert!(image(&e).unwrap_err().contains("no matching"));
        let mut e = fixture(); e.sections.push(Section { name: ".text".into(), addr: ROM_BASE + 0x100, data: vec![0; 4], is_bss: false });
        assert!(image(&e).unwrap_err().contains("Conflicting"));
    }
    #[test]
    fn malformed_tables_fail_before_any_guest_memory_is_loaded() {
        let mut e = fixture(); e.by_name.insert("_data_table_end".into(), ROM_BASE + 0x2d); assert!(image(&e).is_err());
        let mut e = fixture(); e.sections[0].data[8..12].copy_from_slice(&u32::MAX.to_le_bytes()); assert!(image(&e).is_err());
        let mut e = fixture(); e.sections[0].data[4..8].copy_from_slice(&(SRAM_BASE + 0x100000).to_le_bytes()); assert!(image(&e).is_err());
        let mut e = fixture(); e.by_name.clear(); assert!(image(&e).is_err());
    }
}
