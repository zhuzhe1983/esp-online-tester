use super::*;

fn image(entry: u32, segments: &[(u32, &[u8])], hash: bool) -> Vec<u8> {
    let mut d = vec![0; 24]; d[0] = 0xe9; d[1] = segments.len() as u8; d[2] = 2; d[3] = 0x5f;
    d[4..8].copy_from_slice(&entry.to_le_bytes()); d[8] = 0xee; d[12] = 23; d[15] = 100; d[17] = 199; d[23] = u8::from(hash);
    let mut sum = 0xef;
    for &(at, bytes) in segments {
        d.extend_from_slice(&at.to_le_bytes()); d.extend_from_slice(&(bytes.len() as u32).to_le_bytes()); d.extend_from_slice(bytes);
        sum = bytes.iter().fold(sum, |a, b| a ^ b);
    }
    d.resize(d.len() / 16 * 16 + 16, 0); *d.last_mut().unwrap() = sum;
    if hash { d.extend_from_slice(&sha256(&d)); } d
}
fn table(parts: &[(u8, u8, u32, u32)], hash: bool) -> Vec<u8> {
    let mut d = vec![255; 0xc00];
    for (i, &(ty, sub, at, size)) in parts.iter().enumerate() {
        let p = &mut d[i * 32..i * 32 + 32]; p[..4].copy_from_slice(&[0xaa,0x50,ty,sub]);
        p[4..8].copy_from_slice(&at.to_le_bytes()); p[8..12].copy_from_slice(&size.to_le_bytes()); p[12..].fill(0);
    }
    if hash { let n = parts.len() * 32; let digest = md5(&d[..n]); d[n..n + 2].copy_from_slice(&[0xeb,0xeb]); d[n + 16..n + 32].copy_from_slice(&digest); } d
}
fn app() -> Vec<u8> { image(0x4200_0020, &[(0x4200_0020, &[0x13,0,0,0]), (LP_BASE, &[1,2,3,4])], true) }
fn merged(app_offset: usize, pt: usize, hash: bool) -> Vec<u8> {
    let boot = image(0x4084_bba0, &[(0x4084_bba0, &[0x13,0,0,0])], true); let app = app();
    let t = table(&[(1,2,(pt + 0x1000) as u32,0x1000), (0,0,app_offset as u32,0x10000), (0,0x10,(app_offset + 0x10000) as u32,0x10000)], hash);
    let mut d = vec![255; app_offset + app.len()]; d[BOOTLOADER_OFFSET..BOOTLOADER_OFFSET + boot.len()].copy_from_slice(&boot);
    d[pt..pt + t.len()].copy_from_slice(&t); d[app_offset..].copy_from_slice(&app); d
}
fn error<T>(r: Result<T, String>) -> String { r.err().expect("expected image rejection") }

#[test]
fn merged_discovery_requires_one_complete_layout_and_the_physical_capacity(){
    for pt in [0x8000,0x9000]{let d=merged(0x20000,pt,true);assert_eq!(discover_merged(&d,32<<20).unwrap().partition_table_offset,pt);assert!(discover_merged(&d,4<<20).is_err());}
    let mut ambiguous=merged(0x20000,0x9000,true);let other=merged(0x20000,0x8000,true);
    ambiguous[0x8000..0x8c00].copy_from_slice(&other[0x8000..0x8c00]);
    assert!(error(discover_merged(&ambiguous,32<<20)).contains("multiple valid"));
    let mut missing=merged(0x20000,0x8000,true);missing[0x8000]=0;assert!(discover_merged(&missing,32<<20).is_err());
    let mut excessive=vec![255;0x20000];for n in 0..17{excessive[0x8000+n*0x1000..0x8002+n*0x1000].copy_from_slice(&[0xaa,0x50]);}
    assert!(error(discover_merged(&excessive,32<<20)).contains("too many"));
}
#[test]
fn common_c5_loader_validates_before_flash_mutation_and_keeps_sram_direct_strict(){
    use esp_soc::LoadKind;
    let d=merged(0x20000,0x9000,true);let mut m=crate::machine([0;6],32<<20);m.bus.sram.fill(0xa5);
    m.load_input(LoadKind::Flash,&d).unwrap();assert_eq!(&m.bus.flash[..d.len()],d);assert!(m.bus.sram.iter().all(|&b|b==0xa5));
    let before=m.bus.flash.clone();let mut bad=d.clone();bad[0x20020]^=1;
    assert!(m.load_input(LoadKind::Flash,&bad).is_err());assert_eq!(m.bus.flash,before);
    assert!(m.load_input(LoadKind::App,&app()).is_err(),"no validated ROM: app-direct remains SRAM only");assert_eq!(m.bus.flash,before);
    let mut small=crate::machine([0;6],4<<20);assert!(small.load_input(LoadKind::Flash,&d).is_err());assert!(small.bus.flash.iter().all(|&b|b==255));
    // Raw storage-page restore remains data, with no ESP header requirement.
    m.write_flash(0x12000,&[1,2,3,4]).unwrap();assert_eq!(&m.bus.flash[0x12000..0x12004],&[1,2,3,4]);
}
#[test]
fn placement_rejects_wrong_page_offsets_and_capacity_without_guessing_app_address(){
    let d=app();assert!(validate_at(&d,32<<20,0x20000).is_ok());
    assert!(error(validate_at(&d,32<<20,0x21000)).contains("physical64KiB"));
    assert!(validate_at(&d,4<<20,0x20000).is_err());assert!(validate_at(&d,32<<20,usize::MAX).is_err());
}

#[test]
fn independent_digest_vectors_cover_footer_and_partition_algorithms() {
    let hex = |d: &[u8]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
    for (input, expected) in [("", "d41d8cd98f00b204e9800998ecf8427e"), ("abc", "900150983cd24fb0d6963f7d28e17f72"), ("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789", "d174ab98d277d9f5a5611c2c9f419d9f")] { assert_eq!(hex(&md5(input.as_bytes())), expected); }
    assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}
#[test]
fn general_image_accepts_c5_windows_and_nonloaded_padding_without_weakening_sram() {
    let d = image(SRAM_BASE, &[(0, &[1,2,3,4]), (SRAM_BASE, &[0x13,0,0,0]), (0x4200_1000, &[5,6,7,8]), (LP_BASE, &[9,10,11,12]), (4, &[]), (LP_BASE + 8, &[])], true);
    let validated = parse_flash(&d).unwrap(); assert_eq!(validated.image.entry, SRAM_BASE); assert_eq!(validated.image_bytes, d.len());
    assert!(error(parse(&d)).contains("SRAM segments"));
    let sram = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0])], true); assert!(parse(&sram).is_ok()); assert!(parse_flash(&sram).is_ok());
}
#[test]
fn checksum_padding_and_single_image_tail_have_distinct_contracts() {
    let mut d = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0])], false); d[36] = 0xa5;
    assert!(parse_flash(&d).is_ok(), "IDF checksum padding is not required zero"); assert!(error(parse(&d)).contains("padding"));
    d[23] = 1; d.extend_from_slice(&sha256(&d)); assert!(parse_flash(&d).is_ok()); d[36] ^= 1;
    assert!(error(parse_flash(&d)).contains("SHA-256"), "the simple hash includes checksum padding");
    let mut d = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0])], true); let length = d.len(); d.extend_from_slice(&[255; 16]);
    assert_eq!(parse_flash(&d).unwrap().image_bytes, length); d.push(0); assert!(error(parse_flash(&d)).contains("after its footer"));
    let mut d = merged(0x20000, 0x8000, true); d.extend_from_slice(&[1,2,3,4]);
    assert!(validate_merged(&d, 32 << 20, 0x8000).is_ok(), "merged data after an app footer is not single-image tail");
    assert!(parse_flash(&d[0x20000..]).is_err());
}
#[test]
fn corrupted_checksum_digest_header_and_revision_are_rejected() {
    let d = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0])], true);
    for at in [32,47,d.len()-1] { let mut bad = d.clone(); bad[at] ^= 1; assert!(parse_flash(&bad).is_err(), "corruption at{at}"); }
    for chip in [5,13,18,32] { let mut bad = d.clone(); bad[12] = chip; assert!(error(parse_flash(&bad)).contains("chip ID")); }
    for (field, value) in [(15,101u16),(17,99)] { let mut bad = d.clone(); bad[field..field + 2].copy_from_slice(&value.to_le_bytes()); assert!(error(parse_flash(&bad)).contains("revision100")); }
    for maximum in [0u16,0xffff,100,199] {
        let mut good = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0])], false); good[17..19].copy_from_slice(&maximum.to_le_bytes()); good[14] = 255; assert!(parse_flash(&good).is_ok());
    }
    let mut bad = d.clone(); bad[23] = 2; assert!(error(parse_flash(&bad)).contains("flag"));
    for count in [0,17,255] { let mut bad = d.clone(); bad[1] = count; assert!(parse_flash(&bad).is_err()); }
}
#[test]
fn loaded_segment_bounds_overlap_and_entry_are_checked() {
    for address in [0x4000_0000,0x6000_0000,SRAM_BASE + SRAM_SIZE as u32 - 4,LP_BASE + LP_SIZE as u32 - 4,MAPPED_HIGH - 4,0xffff_fffc] {
        let d = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0]),(address, &[0; 8])], false); assert!(parse_flash(&d).is_err(), "target{address:#x}");
    }
    for address in [SRAM_BASE + 2,0x4200_0022] { let d = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0]),(address, &[1,2,3,4])], false); assert!(error(parse_flash(&d)).contains("aligned")); }
    let overlap = image(SRAM_BASE, &[(SRAM_BASE, &[0; 8]),(SRAM_BASE + 4, &[1; 4])], false); assert!(error(parse_flash(&overlap)).contains("overlap"));
    let adjacent = image(SRAM_BASE, &[(SRAM_BASE, &[0; 4]),(SRAM_BASE + 4, &[1; 4])], false); assert!(parse_flash(&adjacent).is_ok());
    for entry in [0,SRAM_BASE + 1,SRAM_BASE + 4] { assert!(error(parse_flash(&image(entry, &[(SRAM_BASE, &[0; 4])], false))).contains("entry")); }
    assert!(parse_flash(&image(SRAM_BASE, &[(SRAM_BASE, &[])], false)).is_err());
}
#[test]
fn c5_32bit_flash_mapping_does_not_inherit_the_legacy_16mib_segment_cap() {
    let bytes = vec![0; (16 << 20) + 4]; let d = image(0x4200_0020, &[(0x4200_0020, &bytes)], false);
    assert!(parse_flash(&d).is_ok()); assert!(parse(&d).is_err());
}
#[test]
fn merged_factory_app_comes_from_table_and_accepts_an_unprovided_ota_slot() {
    for (offset, pt, hash) in [(0x20000,0x8000,true),(0x30000,0x8000,false),(0x20000,0x9000,true)] {
        let layout = validate_merged(&merged(offset, pt, hash), 32 << 20, pt).unwrap();
        assert_eq!(layout, FlashLayout { bootloader_offset:0x2000, partition_table_offset:pt, apps:vec![AppPartition { offset, size:0x10000, subtype:0, image_bytes:app().len(), entry:0x4200_0020 }] });
    }
    let mut d = merged(0x20000, 0x8000, true); let app = app(); d.resize(0x30000 + app.len(), 255); d[0x30000..].copy_from_slice(&app);
    let layout = validate_merged(&d, 32 << 20, 0x8000).unwrap(); assert_eq!(layout.apps.len(), 2); assert_eq!(layout.apps[1].subtype, 0x10);
    d[0x20000..0x20000 + app.len()].fill(255); assert_eq!(validate_merged(&d, 32 << 20, 0x8000).unwrap().apps[0].offset, 0x30000);
}
#[test]
fn merged_rejects_wrong_bootloader_layout_unlisted_and_corrupt_apps() {
    let good = merged(0x20000, 0x8000, false);
    let mut bad = good.clone(); bad[0x2000] = 255; assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("bootloader"));
    let mut bad = good.clone(); bad[0x200c] = 13; assert!(validate_merged(&bad, 32 << 20, 0x8000).is_err());
    let mut bad = good.clone(); bad[0x20000..].fill(255); assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("no populated app"));
    let mut bad = good.clone(); bad[0x20000] = 0; assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("app partition at0x20000"));
    let mut bad = good.clone(); bad[0x20020] ^= 1; assert!(validate_merged(&bad, 32 << 20, 0x8000).is_err());
    let mut bad = good.clone(); bad[0x2000f..0x20011].copy_from_slice(&101u16.to_le_bytes()); assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("revision100"));
    assert!(validate_merged(&good[..good.len() - 1], 32 << 20, 0x8000).is_err());
    assert!(validate_merged(&good, 0x20000, 0x8000).is_err()); assert!(validate_merged(&good, 64 << 20, 0x8000).is_err());
    for pt in [0,0x7000,0x8001,usize::MAX] { assert!(validate_merged(&good, 32 << 20, pt).is_err()); }
}
#[test]
fn table_alignment_capacity_overlap_and_terminator_are_not_magic_only_checks() {
    let good = merged(0x20000, 0x8000, false);
    for parts in [vec![(0,0,0x21000,0x10000)],vec![(0,0,0x20000,0)],vec![(0,0,0x20000,0x10001)],vec![(1,2,0x8000,0x1000),(0,0,0x20000,0x10000)],vec![(0,0,0x20000,0x10000),(1,2,0x20000,0x1000)],vec![(0,0,0x20000,0x10000),(0,0,0x30000,0x10000)],vec![(0,0,0xffff0000,0x20000)]] {
        let mut d = good.clone(); d[0x8000..0x8c00].copy_from_slice(&table(&parts, false)); assert!(validate_merged(&d, 32 << 20, 0x8000).is_err(), "parts{parts:?}");
    }
    let mut bad = good.clone(); bad[0x8000..0x8c00].fill(255); assert!(validate_merged(&bad, 32 << 20, 0x8000).is_err());
    let mut bad = good.clone(); bad[0x8060..0x8c00].fill(0); assert!(validate_merged(&bad, 32 << 20, 0x8000).is_err());
    let mut bad = good.clone(); let full: Vec<_> = (0..96).map(|i| (1,2,0x9000 + i * 0x1000,1)).collect();
    bad[0x8000..0x8c00].copy_from_slice(&table(&full, false)); assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("no terminating"));
    assert!(validate_merged(&good[..0x8bff], 32 << 20, 0x8000).is_err());
}
#[test]
fn table_md5_is_checked_when_present_and_does_not_cover_later_records() {
    let good = merged(0x20000, 0x8000, true);
    for at in [0x800c,0x8070] { let mut bad = good.clone(); bad[at] ^= 1; assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("MD5")); }
    let mut bad = good.clone(); let md5_record = bad[0x8060..0x8080].to_vec(); bad[0x8080..0x80a0].copy_from_slice(&md5_record); assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("duplicate MD5"));
    let mut bad = good.clone(); let record = bad[0x8000..0x8020].to_vec(); bad[0x8080..0x80a0].copy_from_slice(&record); assert!(error(validate_merged(&bad, 32 << 20, 0x8000)).contains("follows its MD5"));
}
#[test]
fn mapped_alignment_uses_actual_image_placement() {
    let mut d = merged(0x20000, 0x8000, false); let bad = image(0x4200_0024, &[(0x4200_0024, &[0; 4])], true);
    d.truncate(0x20000); d.extend_from_slice(&bad);
    assert!(parse_flash(&bad).is_ok(), "standalone validation cannot guess its eventual Flash placement");
    assert!(error(validate_merged(&d, 32 << 20, 0x8000)).contains("physical64KiB"));
}
#[test]
fn truncated_and_oversized_untrusted_fields_return_errors_without_panics() {
    let d = image(SRAM_BASE, &[(SRAM_BASE, &[0x13,0,0,0])], true);
    for n in 0..d.len() { assert!(parse_flash(&d[..n]).is_err(), "truncation{n}"); }
    for length in [1u32,0xffff_fffc,u32::MAX] { let mut bad = d.clone(); bad[28..32].copy_from_slice(&length.to_le_bytes()); assert!(parse_flash(&bad).is_err()); }
}

#[test]
#[ignore = "set ESP32SIM_C5_LCD273_FACTORY to the unchanged official LCD2.73 FactoryOnly BIN"]
fn external_original_lcd273_32f8p_image_validates_as_flash_not_sram_direct() {
    let path = std::env::var("ESP32SIM_C5_LCD273_FACTORY").expect("set ESP32SIM_C5_LCD273_FACTORY to the unchanged official LCD2.73 FactoryOnly BIN");
    let d = std::fs::read(path).expect("read ESP32SIM_C5_LCD273_FACTORY");
    let digest = sha256(&d).iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(digest, "cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942");
    let layout = validate_merged(&d, 32 << 20, 0x8000).unwrap(); assert_eq!(layout.apps.len(), 1);
    assert_eq!(layout.apps[0], AppPartition { offset:0x20000, size:0x4e2000, subtype:0, image_bytes:0x311230, entry:0x4080_02b0 });
    assert!(parse(&d[0x20000..]).is_err()); assert_eq!(parse_flash(&d[0x20000..]).unwrap().image_bytes, 0x311230);
    assert_eq!(parse_flash(&d[0x2000..0x8000]).unwrap().image_bytes, 0x5a80);
}
