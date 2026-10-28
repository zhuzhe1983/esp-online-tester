//! Bounded host PCM input and opt-in live output capture; no browser device permission policy.
use crate::{bytes, Emu};
use esp_periph::i2s::PcmSource;

const MAX_PUSH_FRAMES: usize = 16384;

fn logical_input(e: &mut Emu, port: u32) -> Option<&mut PcmSource> {
    let bus = e.m.bus();
    // Finish guest time that precedes this host operation before adding a new microphone block.
    // Otherwise deferred peripheral ticks could consume newly arrived PCM retroactively.
    bus.flush_ticks();
    bus.i2s_input(port as usize)
}

/// Configure a logical I2S controller source at 8–96 kHz, replacing its previous host input.
/// Returns 0 on success, 1 for an unsupported port or invalid rate. The guest controls RX clocks,
/// format and DMA; this input bypasses physical GPIO routing and codec gain/power semantics.
/// # Safety
/// `e` must be a live exclusively accessible emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_input_configure(e: *mut Emu, port: u32, rate: u32) -> u32 {
    let Ok(source) = PcmSource::stream(rate, None) else { return 1 };
    let Some(input) = logical_input(unsafe { &mut *e }, port) else { return 1 };
    *input = source; 0
}

/// Push little-endian PCM16 mono or interleaved stereo. Mono is duplicated into both lanes.
/// Returns accepted frames (which can be less than requested), or -1 for malformed/unconfigured
/// input. At most 16384 frames per call; the queue holds two seconds at the declared source rate.
/// Rejecting excess input preserves the queued prefix; the caller must handle backpressure.
/// # Safety
/// `e` is live and exclusive; non-null `ptr` must be readable for `len` bytes during this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_input_push(e: *mut Emu, port: u32, channels: u32, ptr: *const u8, len: usize) -> i32 {
    if !matches!(channels, 1 | 2) { return -1; }
    let width = channels as usize * 2;
    if !len.is_multiple_of(width) || len / width > MAX_PUSH_FRAMES || len != 0 && ptr.is_null() { return -1; }
    let Some(input) = logical_input(unsafe { &mut *e }, port) else { return -1 };
    if input.stream_stats().is_none() { return -1; }
    let data = unsafe { bytes(ptr, len) };
    let frames: Vec<_> = data.chunks_exact(width).map(|sample| {
        let left = i16::from_le_bytes([sample[0], sample[1]]);
        [left, if channels == 1 { left } else { i16::from_le_bytes([sample[2], sample[3]]) }]
    }).collect();
    input.push(&frames) as i32
}

/// Clear queued input and held sample/phase, retaining configuration and cumulative counters.
/// # Safety
/// `e` must be a live exclusively accessible emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_input_clear(e: *mut Emu, port: u32) -> u32 {
    let Some(input) = logical_input(unsafe { &mut *e }, port) else { return 1 };
    input.clear(); 0
}

/// Detach the logical stream, release queued samples and restore the silent controller source.
/// # Safety
/// `e` must be a live exclusively accessible emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_input_disable(e: *mut Emu, port: u32) -> u32 {
    let Some(input) = logical_input(unsafe { &mut *e }, port) else { return 1 };
    *input = PcmSource::default(); 0
}

/// Copy a logical input snapshot into the emulator's audio-state buffer; returns its byte length.
/// Unsupported ports return zero. Counts describe the PCM source, not application consumption.
/// # Safety
/// `e` must be a live exclusively accessible emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_input_state(e: *mut Emu, port: u32) -> usize {
    let e = unsafe { &mut *e };
    let Some(input) = logical_input(e, port) else { e.audio_state.clear(); return 0 };
    e.audio_state = if let Some(s) = input.stream_stats() {
        format!("{{\"configured\":true,\"port\":{port},\"sampleRate\":{},\"queuedFrames\":{},\"capacityFrames\":{},\"pushedFrames\":{},\"consumedFrames\":{},\"underrunFrames\":{},\"rejectedFrames\":{},\"clearedFrames\":{}}}",
            s.sample_rate, s.queued_frames, s.capacity_frames, s.pushed_frames, s.consumed_frames, s.underrun_frames, s.rejected_frames, s.cleared_frames)
    } else { format!("{{\"configured\":false,\"port\":{port},\"sampleRate\":0,\"queuedFrames\":0,\"capacityFrames\":0,\"pushedFrames\":0,\"consumedFrames\":0,\"underrunFrames\":0,\"rejectedFrames\":0,\"clearedFrames\":0}}") };
    e.audio_state.len()
}

/// UTF-8 snapshot bytes, valid until the next audio_input_state call or emulator deletion.
/// # Safety
/// `e` must be live and readable. Do not modify or free the returned buffer.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_state_ptr(e: *const Emu) -> *const u8 {
    unsafe { &*e }.audio_state.as_ptr()
}

/// Enable draining the S3 TX capture after each web publication; default native capture is unchanged.
/// The existing type-2 mono PCM16 output remains unchanged. This does not mute output or alter
/// guest clocks. Returns 1 for unsupported TX chips or an invalid boolean; mode changes clear
/// prior capture, so enabling starts at the current stream rather than replaying old samples.
/// # Safety
/// `e` must be a live exclusively accessible emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_output_streaming(e: *mut Emu, enabled: u32) -> u32 {
    if enabled > 1 { return 1; }
    let Some(machine) = (unsafe { &mut *e }).m.s3_mut() else { return 1 };
    machine.bus.flush_ticks();
    machine.set_audio_streaming(enabled != 0); 0
}

/// Flush pending S3 TX samples to the existing binary outbox without advancing guest time.
/// # Safety
/// `e` must be a live exclusively accessible emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_audio_output_flush(e: *mut Emu) -> u32 {
    let Some(machine) = (unsafe { &mut *e }).m.s3_mut() else { return 1 };
    machine.bus.flush_ticks();
    machine.flush_audio_output(); 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{esp32sim_delete, esp32sim_new};

    unsafe fn state(e: *mut Emu, port: u32) -> String {
        let len = unsafe { esp32sim_audio_input_state(e, port) };
        String::from_utf8(unsafe { bytes(esp32sim_audio_state_ptr(e), len) }.to_vec()).unwrap()
    }

    #[test]
    fn input_rejects_malformed_frames_and_preserves_bounded_prefix() {
        // SAFETY: This test owns the handle and readable buffers until deletion.
        unsafe {
            let e = esp32sim_new(b"stopwatch".as_ptr(), 9, 16, 8);
            assert!(!e.is_null());
            assert_eq!(esp32sim_audio_input_push(e, 0, 1, [1, 0].as_ptr(), 2), -1);
            assert_eq!(esp32sim_audio_input_configure(e, 0, 7999), 1);
            assert_eq!(esp32sim_audio_input_configure(e, 2, 8000), 1);
            assert_eq!(esp32sim_audio_input_configure(e, 0, 8000), 0);
            assert_eq!(esp32sim_audio_input_push(e, 0, 1, std::ptr::null(), 2), -1);
            assert_eq!(esp32sim_audio_input_push(e, 0, 3, [0; 6].as_ptr(), 6), -1);
            assert_eq!(esp32sim_audio_input_push(e, 0, 2, [0; 3].as_ptr(), 3), -1);
            assert_eq!(esp32sim_audio_input_push(e, 0, 1, [0; 2].as_ptr(), 2 * (MAX_PUSH_FRAMES + 1)), -1);
            let data = vec![0x12; 2 * MAX_PUSH_FRAMES];
            assert_eq!(esp32sim_audio_input_push(e, 0, 1, data.as_ptr(), data.len()), 16000);
            assert_eq!(esp32sim_audio_input_push(e, 0, 1, [0; 2].as_ptr(), 2), 0);
            let s = state(e, 0);
            assert!(s.contains("\"queuedFrames\":16000"), "{s}");
            assert!(s.contains("\"rejectedFrames\":385"), "{s}");
            assert_eq!(esp32sim_audio_input_clear(e, 0), 0);
            let s = state(e, 0);
            assert!(s.contains("\"clearedFrames\":16000"), "{s}");
            assert!(s.contains("\"queuedFrames\":0"), "{s}");
            assert_eq!(esp32sim_audio_input_disable(e, 0), 0);
            assert!(state(e, 0).contains("\"configured\":false"));
            esp32sim_delete(e);
        }
    }

    #[test]
    fn configured_pcm_reaches_guest_dma_and_survives_chip_reset() {
        // Same IDF 5.5.5 RX/GDMA register setup as cli/tests/i2s_rx.rs; the host now enters by ABI.
        // SAFETY: This test owns the handle and readable buffers until deletion.
        unsafe {
            let e = esp32sim_new(b"stopwatch".as_ptr(), 9, 16, 8);
            assert_eq!(esp32sim_audio_input_configure(e, 0, 16000), 0);
            let samples = [111i16, -111, 1234, -2345];
            let pcm: Vec<_> = samples.into_iter().flat_map(i16::to_le_bytes).collect();
            assert_eq!(esp32sim_audio_input_push(e, 0, 2, pcm.as_ptr(), pcm.len()), 2);
            let bus = (*e).m.bus();
            bus.reboot([0; 6]);
            // A 16 kHz source feeding an 8 kHz guest consumes two source frames per guest frame.
            for (address, value) in [
                (0x6000f030, (1 << 26) | (2 << 27) | 25),
                (0x6000f028, (24 << 7) | (15 << 13) | (15 << 18)),
                (0x6000f050, (1 << 16) | 3), (0x6000f064, 4),
                (0x3fc90000, 4 | (1 << 31)), (0x3fc90004, 0x3fc90100), (0x3fc90008, 0),
                (0x6003f048, 3), (0x6003f020, 0x90000 | (1 << 22)), (0x6000f020, 4),
            ] { bus.write32(address, value).unwrap(); }
            bus.tick(30000); bus.flush_ticks();
            assert_eq!(bus.read32(0x3fc90100).unwrap(), 0xf6d7_04d2);
            let s = state(e, 0);
            assert!(s.contains("\"consumedFrames\":2"), "{s}");
            assert!(s.contains("\"queuedFrames\":0"), "{s}");
            esp32sim_delete(e);
        }
    }

    #[test]
    fn chip_capabilities_are_not_inferred_from_the_family_name() {
        // SAFETY: This test owns each handle until deletion.
        unsafe {
            for (board, ports, output) in [("none", 2, 0), ("esp32", 2, 1), ("esp32c3", 1, 1), ("esp32c6", 1, 1), ("esp32p4", 0, 1)] {
                let e = esp32sim_new(board.as_ptr(), board.len(), 4, 0);
                for port in 0..3 { assert_eq!(esp32sim_audio_input_configure(e, port, 48000), u32::from(port >= ports), "{board}/{port}"); }
                assert_eq!(esp32sim_audio_output_streaming(e, 1), output, "{board}");
                assert_eq!(esp32sim_audio_output_streaming(e, 2), 1);
                esp32sim_delete(e);
            }
        }
    }

    #[test]
    fn streaming_publishes_each_sample_once_and_releases_both_captures() {
        // SAFETY: This test owns the handle until deletion.
        unsafe {
            let e = esp32sim_new(b"none".as_ptr(), 4, 4, 0);
            let machine = (*e).m.s3_mut().unwrap();
            machine.bus.periph.i2s0.sample_rate = 24000;
            machine.bus.periph.i2s0.pcm.extend_from_slice(&[111, -222]);
            machine.flush_audio_output();
            let first = machine.web.as_ref().unwrap().take_outbox();
            assert!(first.iter().any(|(kind, data)| *kind == 2 && data == &[2, 0xc0, 0x5d, 0, 0, 111, 0, 34, 255]));
            assert_eq!(machine.bus.periph.i2s0.pcm, [111, -222], "legacy capture remains intact");
            machine.flush_audio_output();
            assert!(machine.web.as_ref().unwrap().take_outbox().is_empty());
            assert_eq!(esp32sim_audio_output_streaming(e, 1), 0);
            for n in 0..1000i16 {
                let machine = (*e).m.s3_mut().unwrap();
                machine.bus.periph.i2s0.pcm.extend_from_slice(&[n, -n]);
                machine.bus.periph.i2s1.pcm.push(1); // the unselected controller must not accumulate
                assert_eq!(esp32sim_audio_output_flush(e), 0);
                let machine = (*e).m.s3_mut().unwrap();
                let out = machine.web.as_ref().unwrap().take_outbox();
                assert_eq!(out.len(), 1);
                assert_eq!(out[0].1.len(), 9);
                assert!(machine.bus.periph.i2s0.pcm.is_empty());
                assert!(machine.bus.periph.i2s1.pcm.is_empty());
            }
            esp32sim_delete(e);
        }
    }
}
