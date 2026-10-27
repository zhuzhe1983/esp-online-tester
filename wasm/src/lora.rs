//! Cold, explicit virtual LoRa packet input/observation. No RF or remote relay.
use crate::*;

/// Return 0 for an accepted digital packet, 1 for unsupported/state/config
/// mismatch, 2 for malformed input. Acceptance schedules receive completion;
/// it is not evidence that the guest handled the packet.
/// # Safety
/// `e` is a live exclusive emulator; `p` is readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_lora_receive(e:*mut Emu,p:*const u8,len:usize)->u32 {
    if len>4096 {return 2;}
    let Some(input)=esp_soc::devices::sx1262::LoRaInput::parse(unsafe {text(p,len)}) else {return 2;};
    let Some(m)=(unsafe {&mut *e}).m.s3_mut() else {return 1;};
    if m.bus.board.lora_receive_at(m.bus.cycles,&input) {0}else{1}
}

/// Copy a JSON snapshot; return its required length, or 0 if unsupported.
/// No bytes are written when capacity is too small. Does not drain TX history.
/// # Safety
/// `e` is a live exclusive emulator; `p` is writable for `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_lora_snapshot(e:*mut Emu,p:*mut u8,capacity:usize)->usize {
    let Some(m)=(unsafe {&mut *e}).m.s3_mut() else {return 0;};
    let Some(json)=m.bus.board.lora_snapshot() else {return 0;};
    if json.len()<=capacity {unsafe {std::ptr::copy_nonoverlapping(json.as_ptr(),p,json.len());}}
    json.len()
}
