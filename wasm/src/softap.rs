//! Opt-in host station for a guest SoftAP; payloads are real guest Ethernet, never synthetic HTTP.
use super::{bytes, log, Emu};

/// Configure `{enabled,ssid?,password?}` before or after boot. No secrets appear in status.
/// Returns 0 on success, 1 for invalid configuration or unsupported chip.
/// # Safety
/// `e` must be live/exclusive; `ptr` must be readable for `len` bytes (at most 1024).
#[no_mangle]
pub unsafe extern "C" fn esp32sim_softap_configure(e: *mut Emu, ptr: *const u8, len: usize) -> u32 {
    if len > 1024 || ptr.is_null() { return 1; }
    let e = unsafe { &mut *e };
    let result = std::str::from_utf8(unsafe { bytes(ptr, len) }).map_err(|_| "SoftAP configuration is not UTF-8".into())
        .and_then(esp_soc::wifi::softap::ClientConfig::parse).and_then(|cfg| e.m.bus().configure_softap_client(cfg));
    match result { Ok(()) => { e.softap_out.clear(); 0 }, Err(reason) => { log(&format!("[emu] SoftAP: {reason}")); 1 } }
}

/// Drive enabled-client retry timers with guest time, then snapshot JSON status. Returns byte length.
/// # Safety
/// `e` must be a live emulator with exclusive access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_softap_poll(e: *mut Emu) -> usize {
    let e = unsafe { &mut *e }; e.m.bus().poll_softap_client(); e.softap_status = e.m.bus().softap_client_status(); e.softap_status.len()
}
/// JSON status bytes, valid until the next poll or emulator deletion.
/// # Safety
/// `e` must be a live readable emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_softap_status_ptr(e: *const Emu) -> *const u8 { unsafe { &*e }.softap_status.as_ptr() }

/// Inject a complete Ethernet frame sourced from the virtual client's MAC (02:53:49:4d:00:03).
/// # Safety
/// `e` must be live/exclusive; `ptr` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_softap_receive(e: *mut Emu, ptr: *const u8, len: usize) -> u32 {
    if !(14..=1518).contains(&len) || ptr.is_null() { return 1; }
    unsafe { &mut *e }.m.bus().receive_softap_frame(unsafe { bytes(ptr, len) }).is_err() as u32
}
/// Drain guest SoftAP Ethernet frames. Returns batch length; replaces the last batch.
/// # Safety
/// `e` must be live/exclusive.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_softap_take(e: *mut Emu) -> usize {
    let e = unsafe { &mut *e }; e.softap_out = e.m.bus().take_softap_frames(); e.softap_out.len()
}
/// Frame pointer, valid until the next take or emulator deletion; null for an invalid index.
/// # Safety
/// `e` must be live and readable.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_softap_ptr(e: *const Emu, index: usize) -> *const u8 { unsafe { &*e }.softap_out.get(index).map_or(std::ptr::null(), |f| f.as_ptr()) }
/// Frame length, zero for an invalid index.
/// # Safety
/// `e` must be live and readable.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_softap_len(e: *const Emu, index: usize) -> usize { unsafe { &*e }.softap_out.get(index).map_or(0, Vec::len) }
