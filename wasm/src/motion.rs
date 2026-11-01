//! Cold observation of firmware-driven board mechanics. Never sets a servo target.
use crate::*;

/// Copy a JSON motion snapshot; return its required length, or 0 if unsupported.
/// Insufficient capacity leaves the destination untouched. Observation does not advance time.
/// # Safety
/// `e` is a live exclusive emulator; `p` is writable for `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_motion_snapshot(e:*mut Emu,p:*mut u8,capacity:usize)->usize {
    let bus=(unsafe {&mut *e}).m.bus();
    let Some(json)=bus.board().motion_snapshot() else {return 0;};
    if json.len()<=capacity {unsafe {std::ptr::copy_nonoverlapping(json.as_ptr(),p,json.len());}}
    json.len()
}
