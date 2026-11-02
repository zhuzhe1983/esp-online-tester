#[path = "support/camera_fixture.rs"]
mod fixture;

#[test]
#[ignore = "needs esp32_rev300_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn public_driver_captures_host_frame() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    fixture::capture(
        &root.join("esp32/tests/fixtures/classic-camera"),
        &esp_soc::rom::require("esp32_rev300_rom.elf"),
    );
}
