//! Cold board checks derived from actual guest register and bus transactions.
use crate::*;

/// Copy a JSON hardware snapshot; return its required length, or 0 if unsupported.
/// Insufficient capacity leaves the destination untouched. Does not advance time.
/// # Safety
/// `e` is a live exclusive emulator; `p` is writable for `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_hardware_diagnostics(e:*mut Emu,p:*mut u8,capacity:usize)->usize {
    let bus=(unsafe {&mut *e}).m.bus();
    let Some(json)=bus.board().hardware_diagnostics() else {return 0;};
    if json.len()<=capacity {unsafe {std::ptr::copy_nonoverlapping(json.as_ptr(),p,json.len());}}
    json.len()
}
