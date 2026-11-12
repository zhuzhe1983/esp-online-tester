use emu_core::bus::Bus as _;
use std::{fs, path::Path};

fn picture(replacement: bool) -> esp_soc::picture::Picture {
    esp_soc::picture::Picture {
        w: 160,
        h: 120,
        rgb: (0..160 * 120)
            .flat_map(|p| {
                let rgb = [
                    (p % 256) as u8,
                    ((p / 160) % 256) as u8,
                    ((p * 7) % 256) as u8,
                ];
                rgb.map(|v| if replacement { 255 - v } else { v })
            })
            .collect(),
    }
}

#[test]
#[ignore = "needs esp32_rev300_rom.elf (ESP32SIM_ROM_DIR or ESP-IDF)"]
fn public_driver_camera_lifecycle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut m = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    m.bus.board = esp32::board::make_board("ai-thinker-cam").unwrap();
    m.bus.attach_board_devices();
    m.load_rom(&fs::read(esp_soc::rom::require("esp32_rev300_rom.elf")).unwrap())
        .unwrap();
    for (offset, name) in [
        (0x1000, "bootloader/bootloader.bin"),
        (0x8000, "partition_table/partition-table.bin"),
        (0x10000, "lifecycle.bin"),
    ] {
        m.write_flash(
            offset,
            &fs::read(
                root.join("esp32/tests/fixtures/classic-camera")
                    .join(name),
            )
            .unwrap(),
        )
        .unwrap();
    }
    m.boot_rom();
    for boot in 0..2 {
        m.console.all.clear();
        m.bus.board.set_camera_picture(picture(false)).unwrap();
        let mut replaced = false;
        let mut deinitialized = false;
        // Budget includes idle FreeRTOS delays as well as retired instructions.
        for _ in 0..300_000 {
            let _ = m.run(10_000);
            m.drain_console();
            let text = String::from_utf8_lossy(&m.console.all).replace("\r\n", "\n");
            if !replaced && text.contains("C9 REPLACE\n") {
                m.bus.board.set_camera_picture(picture(true)).unwrap();
                replaced = true;
            }
            if !deinitialized && text.contains("C9 deinit=ESP_OK sensor=NULL\n") {
                // IDF v5.4.3 soc/i2s_reg.h: INT_ST +0x10; CONF.RX_START bit 5.
                assert_eq!(
                    m.bus.read32(0x3ff4_f010).unwrap(),
                    0,
                    "deinit retained a pending camera IRQ"
                );
                assert_eq!(
                    m.bus.read32(0x3ff4_f008).unwrap() & (1 << 5),
                    0,
                    "deinit left RX running"
                );
                deinitialized = true;
            }
            if text.contains("C9 PASS\n") || text.contains("abort() was called") {
                break;
            }
        }
        let text = String::from_utf8_lossy(&m.console.all).replace("\r\n", "\n");
        assert!(replaced, "boot {boot}: no replacement marker: {text}");
        assert!(text.contains("C9 init=ESP_OK\n"), "boot {boot}: init");
        let records: Vec<_> = text
            .lines()
            .filter(|l| l.starts_with("C9 ") && l.contains(" frame="))
            .collect();
        assert_eq!(
            records.len(),
            7,
            "boot {boot}: missing/duplicate frame: {text}"
        );
        let initial = "c67913e0afba1703e8c9c47e02b1ae2ea62e47596a8440d83f1ebba14aed6cf1";
        let replacement = "1e1f681f056ffcf2cde1b0dd089c7cc69d09b01e3bf61299eb7437488480c669";
        for (phase, hash) in [
            ("initial", initial),
            ("replacement", replacement),
            ("reinit", replacement),
        ] {
            for n in 0..2 {
                let expected = format!(
                    "C9 {phase} frame={n} width=160 height=120 len=38400 RGB565 sha256={hash}"
                );
                assert!(
                    records.contains(&expected.as_str()),
                    "boot {boot}: {phase} stale/incorrect frame: {text}"
                );
            }
        }
        assert!(
            text.contains("C9 sensor=QVGA/YUV422\n"),
            "boot {boot}: sensor poison was not exercised"
        );
        assert!(
            text.contains("C9 deinit=ESP_OK sensor=NULL\n"),
            "boot {boot}: deinit"
        );
        assert!(text.contains("C9 reinit=ESP_OK\n"), "boot {boot}: reinit");
        assert!(text.contains("C9 PASS\n"), "boot {boot}: did not finish");
        if boot == 0 {
            // Reset the same machine while the reinitialized driver is live.
            m.reboot();
            assert_eq!(m.reboots, 1);
            // IDF v5.4.3 soc/i2s_reg.h: RAW/ST/ENA at +12/+16/+20.
            for offset in [12, 16, 20] {
                assert_eq!(
                    m.bus.read32(0x3ff4_f000 + offset).unwrap(),
                    0,
                    "reboot retained camera IRQ state"
                );
            }
        }
    }
}
