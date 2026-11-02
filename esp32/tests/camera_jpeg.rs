//! Host-encoded JPEG through the unmodified public esp32-camera driver on the
//! AI-Thinker model: verbatim delivery, host changes, VGA/QVGA, raw/JPEG reinit.
use esp_soc::picture::{Jpeg, JpegError, Picture};
use std::{fs, path::Path};

/// Independent oracle: examples/classic-camera/make-jpeg.py (file, length, SHA-256).
const JPEGS: [(&str, usize, &str); 4] = [
    ("qvga-a.jpg", 2489, "7a7f480aa5bf001fda3760c1c7dda6327295c02ec32939595f5814596f8932d0"),
    ("qvga-b.jpg", 2487, "a711e4317a9af4fd9efcbdf70c412a2d123dc16c6efa5d38b97b65423edec968"),
    ("vga-a.jpg", 6218, "f602a49340beba9cbf3c7b8ee37d34db066baf7b7a9f545e52742eeaca576998"),
    ("vga-b.jpg", 11400, "680228c782f1beff1ae24e8349ce9b8e8c14386103a941bf81a417fce720cde4"),
];
/// Independent MSB-first RGB565 oracle: examples/classic-camera/expected-frame.py.
const RAW: &str = "c67913e0afba1703e8c9c47e02b1ae2ea62e47596a8440d83f1ebba14aed6cf1";

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/classic-camera")
}
fn host(name: &str) -> Vec<u8> {
    let data = fs::read(fixture().join("jpeg").join(name)).unwrap();
    assert_eq!(data.len(), JPEGS.iter().find(|j| j.0 == name).unwrap().1);
    data
}
fn picture() -> Picture {
    Picture {
        w: 160,
        h: 120,
        rgb: (0..160 * 120)
            .flat_map(|p| [(p % 256) as u8, ((p / 160) % 256) as u8, ((p * 7) % 256) as u8])
            .collect(),
    }
}

fn run(app: &str, psram: bool) {
    let roms = std::env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR");
    let mut m = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    if psram {
        esp_soc::SocBus::set_psram_size(&mut m.bus, 4 << 20).unwrap();
    }
    m.bus.board = esp32::board::make_board("ai-thinker-cam").unwrap();
    m.bus.attach_board_devices();
    m.load_rom(&fs::read(Path::new(&roms).join("esp32_rev300_rom.elf")).unwrap()).unwrap();
    for (offset, name) in [
        (0x1000, "bootloader/bootloader.bin"),
        (0x8000, "partition_table/partition-table.bin"),
        (0x10000, app),
    ] {
        m.write_flash(offset, &fs::read(fixture().join(name)).unwrap()).unwrap();
    }
    let set = |m: &mut esp32::Machine, name: &str| m.bus.board.set_camera_jpeg(Jpeg::parse(host(name)).unwrap());
    m.bus.board.set_camera_picture(picture()).unwrap();
    // Before the guest configures the sensor there is no size to check yet.
    assert_eq!(set(&mut m, "vga-a.jpg"), Ok(()));
    // Refusal must not replace the first frame that the guest will read below.
    let no_scan = [0xff, 0xd8, 0xff, 0xc0, 0, 8, 8, 1, 0xe0, 2, 0x80, 1, 0xff, 0xd9];
    assert_eq!(m.load_input(esp_soc::LoadKind::CameraPicture, &no_scan), Err(JpegError::Malformed.to_string()));
    let incomplete_sos = [0xff,0xd8, 0xff,0xc0,0,11,8,0,1,0,1,1,1,0x11,0, 0xff,0xda,0,2, 0xff,0xd9];
    assert_eq!(m.load_input(esp_soc::LoadKind::CameraPicture, &incomplete_sos), Err(JpegError::Malformed.to_string()));
    let dnl = [0xff,0xd8, 0xff,0xc0,0,11,8,0,1,0,1,1,1,0x11,0,
        0xff,0xda,0,8,1,1,0,0,63,0, 0x42, 0xff,0xdc,0,4,0,1, 0xff,0xd9];
    assert_eq!(m.load_input(esp_soc::LoadKind::CameraPicture, &dnl), Err(JpegError::Malformed.to_string()));
    m.boot_rom();
    let mut handled = 0;
    let mut text = String::new();
    for _ in 0..2_000_000 {
        let _ = m.run(10_000);
        m.drain_console();
        text = String::from_utf8_lossy(&m.console.all).replace("\r\n", "\n");
        if text.contains("C10 PASS\n") || text.contains("assert failed") || text.contains("abort()") {
            break;
        }
        let markers = ["C10 REPLACE vga-b\n", "C10 QVGA qvga-a\n", "C10 REPLACE qvga-b\n"];
        if let Some(marker) = markers.get(handled).filter(|marker| text.contains(**marker)) {
            match handled {
                0 => assert_eq!(set(&mut m, "vga-b.jpg"), Ok(())),
                1 => {
                    // The sensor is now QVGA: a VGA JPEG and a truncated one are refused.
                    assert_eq!(set(&mut m, "vga-a.jpg"), Err(JpegError::SizeMismatch { jpeg: (640, 480), sensor: (320, 240) }));
                    let mut cut = host("qvga-a.jpg");
                    cut.truncate(cut.len() - 1);
                    assert_eq!(Jpeg::parse(cut), Err(JpegError::Malformed));
                    assert_eq!(set(&mut m, "qvga-a.jpg"), Ok(()));
                }
                _ => assert_eq!(set(&mut m, "qvga-b.jpg"), Ok(())),
            }
            eprintln!("host acted on {marker:?}");
            handled += 1;
        }
    }
    println!("{text}");
    assert_eq!(handled, 3, "{text}");
    assert!(text.contains("C10 PASS\n"), "fixture did not finish");
    let size = text.lines().find_map(|l| l.strip_prefix("C10 psram=")).unwrap();
    assert_eq!(size != "0", psram, "{size}");
    let at = if psram { "PSRAM" } else { "DRAM" };
    let hash = |name: &str| JPEGS.iter().find(|j| j.0 == name).unwrap();
    let frames: Vec<_> = text.lines().filter(|l| l.contains(" frame=")).collect();
    let mut expected = Vec::new();
    for (phase, file, w, h) in [("vga-a", "vga-a.jpg", 640, 480), ("vga-b", "vga-b.jpg", 640, 480),
        ("qvga-a", "qvga-a.jpg", 320, 240), ("qvga-b", "qvga-b.jpg", 320, 240), ("reinit", "qvga-b.jpg", 320, 240)] {
        let (_, len, sha) = hash(file);
        for n in 0..2 {
            expected.push(format!("C10 {phase} frame={n} width={w} height={h} len={len} JPEG {at} sha256={sha}"));
        }
    }
    for n in 0..2 {
        expected.push(format!("C10 raw frame={n} width=160 height=120 len=38400 RGB565 {at} sha256={RAW}"));
    }
    for line in &expected {
        assert_eq!(frames.iter().filter(|l| *l == line).count(), 1, "missing {line}\n{text}");
    }
    // A queued frame may predate the host change: either JPEG, never anything else.
    let queued: Vec<_> = frames.iter().filter(|l| l.starts_with("C10 queued")).collect();
    assert_eq!(queued.len(), 3);
    for (line, old, new, (w, h)) in [(queued[0], "vga-a.jpg", "vga-b.jpg", (640, 480)),
        (queued[1], "vga-b.jpg", "qvga-a.jpg", (320, 240)), (queued[2], "qvga-a.jpg", "qvga-b.jpg", (320, 240))] {
        assert!([old, new].iter().any(|f| {
            let (_, len, sha) = hash(f);
            **line == format!("C10 queued frame=0 width={w} height={h} len={len} JPEG {at} sha256={sha}")
        }), "{line}");
    }
    assert_eq!(frames.len(), expected.len() + 3);
}

#[test]
fn browser_style_three_component_jfif() {
    let data = fs::read(fixture().join("jpeg/browser-420.jpg")).unwrap();
    let parsed = Jpeg::parse(data.clone()).unwrap();
    assert_eq!((parsed.w, parsed.h, &*parsed.bytes), (2, 2, &data));
}

#[test]
fn loader_kind_7_takes_jpeg_with_typed_errors() {
    // The browser's picture input (wasm loader kind 7) shares this dispatch.
    let mut m = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    m.bus.board = esp32::board::make_board("ai-thinker-cam").unwrap();
    m.bus.attach_board_devices();
    let jpeg = host("qvga-a.jpg");
    assert_eq!(m.load_input(esp_soc::LoadKind::CameraPicture, &jpeg), Ok(()));
    let malformed = m.load_input(esp_soc::LoadKind::CameraPicture, &jpeg[..jpeg.len() - 1]);
    assert_eq!(malformed, Err(JpegError::Malformed.to_string()));
    // PPM still goes to the raw picture path.
    assert_eq!(m.load_input(esp_soc::LoadKind::CameraPicture, b"P6\n1 1\n255\n\xff\x00\x00"), Ok(()));
    // A board whose camera has no JPEG input says so.
    let mut bare = esp32::machine([2, 0, 0, 0, 0, 1], 4 << 20);
    assert_eq!(bare.load_input(esp_soc::LoadKind::CameraPicture, &jpeg), Err(JpegError::Unsupported.to_string()));
}

#[test]
#[ignore = "ESP32SIM_ROM_DIR must contain esp32_rev300_rom.elf"]
fn public_driver_jpeg_dram_single_framebuffer() {
    run("jpeg.bin", false);
}

#[test]
#[ignore = "ESP32SIM_ROM_DIR must contain esp32_rev300_rom.elf"]
fn public_driver_jpeg_psram_two_framebuffers() {
    run("jpeg-psram.bin", true);
}
