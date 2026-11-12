use std::{fs, path::Path};

/// Boot the unmodified public esp32-camera driver; no symbol stubs or DMA shortcuts.
pub fn capture(build: &Path, rom: &Path) {
    let mut m = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    m.bus.board = esp32::board::make_board("ai-thinker-cam").unwrap();
    m.bus.attach_board_devices();
    m.bus
        .board
        .set_camera_picture(esp_soc::picture::Picture {
            w: 160,
            h: 120,
            rgb: (0..160 * 120)
                .flat_map(|p| {
                    [
                        (p % 256) as u8,
                        ((p / 160) % 256) as u8,
                        ((p * 7) % 256) as u8,
                    ]
                })
                .collect(),
        })
        .unwrap();
    m.load_rom(&fs::read(rom).expect("readable classic ROM ELF"))
        .unwrap();
    for (offset, name) in [
        (0x1000, "bootloader/bootloader.bin"),
        (0x8000, "partition_table/partition-table.bin"),
        (0x10000, "classic_camera.bin"),
    ] {
        m.write_flash(offset, &fs::read(build.join(name)).unwrap())
            .unwrap();
    }
    m.max_cycles = 240_000_000 * 20;
    m.boot_rom();
    let stop = m.run(300_000_000);
    m.drain_console();
    let console = String::from_utf8_lossy(&m.console.all);
    println!("{console}");
    eprintln!("capture stop={stop:?}");
    assert!(console.contains("C8 esp_camera_init=ESP_OK (0)"));
    // Independent MSB-first RGB565 oracle: examples/classic-camera/expected-frame.py.
    let hash = "c67913e0afba1703e8c9c47e02b1ae2ea62e47596a8440d83f1ebba14aed6cf1";
    let frames: Vec<_> = console
        .lines()
        .filter(|l| l.starts_with("C8 frame="))
        .collect();
    assert_eq!(frames.len(), 2, "fixture did not return exactly two frames");
    for (n, line) in frames.iter().enumerate() {
        assert_eq!(
            *line,
            format!("C8 frame={n} width=160 height=120 len=38400 RGB565 sha256={hash}")
        );
    }
    assert!(console.contains("C8 PASS"), "fixture did not finish");
}
