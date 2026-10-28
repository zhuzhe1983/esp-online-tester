//! An image built for the other ESP32-P4 revision is refused before it runs, through the loader
//! the browser build uses (`Machine::load_input`), as the bootloader, the app or inside a whole
//! flash image.
use esp_soc::LoadKind;

/// The committed hello_world with its header's revision range set as a rev >= 3 build sets it
/// (`min` 300 or more), or left as built for rev < 3.
fn image(name: &str, min: u16) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/wasm/fw/public").join(name);
    let mut d = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    if min >= 300 { d[15..17].copy_from_slice(&min.to_le_bytes()); d[17..19].copy_from_slice(&399u16.to_le_bytes()); }
    d
}

#[test]
fn the_loader_refuses_a_rev3_image_and_takes_a_rev_lt3_one() {
    let mut m = esp32p4::machine([0; 6], 2 << 20);
    let why = "was built for ESP32-P4 rev ≥ v3.0 (min_chip_rev_full=300); this model is rev < 3 (v1.0)";
    let e = m.load_input(LoadKind::App, &image("p4-hello_world.bin", 300)).unwrap_err();
    assert!(e.starts_with(&format!("the app {why}")), "{e}");
    let e = m.load_input(LoadKind::Bootloader, &image("p4-hello-bootloader.bin", 300)).unwrap_err();
    assert!(e.starts_with(&format!("the bootloader {why}")), "{e}");
    let mut flash = vec![0xff; 0x10000];
    flash[0x2000..0x2000 + 22976].copy_from_slice(&image("p4-hello-bootloader.bin", 0));
    flash.extend_from_slice(&image("p4-hello_world.bin", 300));
    let e = m.load_input(LoadKind::Flash, &flash).unwrap_err();
    assert!(e.starts_with(&format!("the flash image (app at 0x10000) {why}")), "{e}");
    assert_eq!(m.bus.flash[0x10000..0x10004], [0xff; 4], "nothing written");
    assert_eq!(m.load_input(LoadKind::App, &image("p4-hello_world.bin", 0)), Ok(()));
    assert_eq!(m.load_input(LoadKind::Bootloader, &image("p4-hello-bootloader.bin", 0)), Ok(()));
}

/// The rev >= 3 model takes the same images marked as built for rev >= 3 and refuses them as built.
#[test]
fn the_rev3_loader_takes_a_rev3_image_and_refuses_a_rev_lt3_one() {
    let mut m = esp32p4::machine_rev::<esp32p4::RevGe3>([0; 6], 2 << 20);
    let why = "was built for ESP32-P4 rev < v3.0 (max_chip_rev_full=199); this model is rev ≥ 3 (v3.1)";
    let e = m.load_input(LoadKind::App, &image("p4-hello_world.bin", 0)).unwrap_err();
    assert!(e.starts_with(&format!("the app {why}")), "{e}");
    let e = m.load_input(LoadKind::Bootloader, &image("p4-hello-bootloader.bin", 0)).unwrap_err();
    assert!(e.starts_with(&format!("the bootloader {why}")), "{e}");
    assert_eq!(m.load_input(LoadKind::App, &image("p4-hello_world.bin", 301)), Ok(()));
    assert_eq!(m.load_input(LoadKind::Bootloader, &image("p4-hello-bootloader.bin", 301)), Ok(()));
}
