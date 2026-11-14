//! Browser factory boundary: production S31 machine, exact Mosaico V1.2 memory and cold I/O.
use super::*;

#[test]
fn mosaico_factory_uses_s31_profile_and_rejects_other_memory_sizes() {
    // SAFETY: these tests exclusively own every returned handle until deletion.
    unsafe {
        for (flash, psram) in [(8, 16), (16, 8), (32, 16), (16, 0)] {
            assert!(esp32sim_new(b"mosaico".as_ptr(), 7, flash, psram).is_null());
        }
        for name in ["mosaico", "esp-mosaico"] {
            let e = esp32sim_new(name.as_ptr(), name.len(), 16, 16);
            assert!(!e.is_null());
            let MachineKind::S31(m) = &mut (*e).m else { panic!("must use production S31 machine"); };
            assert_eq!(m.bus.board.name(), "mosaico");
            assert_eq!(m.bus.flash.len(), 16 << 20);
            let inputs = m.bus.board_inputs.as_ref().unwrap();
            assert_eq!(inputs.flash_id, [0xc8, 0x40, 0x18]);
            assert_eq!(inputs.psram_bytes, 16 << 20);
            assert!(inputs.efuse.contains(&(0x88, 0x0102)), "V1.2 board revision");
            assert_eq!(m.bus.inner.pads.strap, 0x10);
            assert_eq!(m.bus.board.named_pin("ai"), Some(7));
            assert_eq!(m.bus.board.named_pin("boot"), Some(61));
            m.bus.inner.pads.write(esp32s31::pads::IO_MUX_BASE + 61 * 4, 1 << 9).unwrap();
            assert_ne!(m.bus.gpio_input() & (1 << 61), 0);
            m.bus.gpio_set_input(61, false);
            assert_eq!(m.bus.gpio_input() & (1 << 61), 0, "high GPIO bank reaches the board");
            let (width, height, pixels, _) = m.bus.board.display().unwrap();
            assert_eq!((width, height, pixels.len()), (480, 480, 480 * 480));
            esp32sim_delete(e);
        }
    }
}

#[test]
fn s31_live_audio_drains_capture_without_changing_guest_time() {
    // SAFETY: handle is uniquely owned here; output is copied before mutation/deletion.
    unsafe {
        let e = esp32sim_new(b"mosaico".as_ptr(), 7, 16, 16);
        for port in 0..3 { assert_eq!(esp32sim_audio_input_configure(e, port, 48000), u32::from(port >= 2)); }
        assert_eq!(esp32sim_audio_output_streaming(e, 1), 0);
        let MachineKind::S31(m) = &mut (*e).m else { panic!("S31"); };
        let before = m.bus.cycles();
        m.bus.inner.i2s[0].sample_rate = 24000;
        m.bus.inner.i2s[0].pcm.extend_from_slice(&[111, -222]);
        m.flush_audio_output();
        let first = m.web.as_ref().unwrap().take_outbox();
        assert!(first.iter().any(|(kind, data)| *kind == 2 && data == &[2, 0xc0, 0x5d, 0, 0, 111, 0, 34, 255]));
        assert!(m.bus.inner.i2s.iter().all(|i| i.pcm.is_empty()));
        m.flush_audio_output();
        assert!(m.web.as_ref().unwrap().take_outbox().is_empty(), "no duplicate audio");
        assert_eq!(m.bus.cycles(), before);
        assert_eq!(m.insns(), 0);
        esp32sim_delete(e);
    }
}
