//! Explicit A172 digital SDIO peer access. No fabricated register response or RF.
use crate::*;
/// CMD3/5/7/52 pass through the C6's real SDIO state machine and current pad gate.
/// Returns0 for a short response,1 unsupported,2 invalid command,3 no response.
/// # Safety
/// `e` is a live exclusive emulator; `out` is writable for one aligned u32.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_sdio_host_command(e:*mut Emu,command:u32,arg:u32,out:*mut u32)->u32 {
    if !matches!(command,3|5|7|52) {return 2;}
    let MachineKind::C6(m)=&mut (unsafe {&mut *e}).m else {return 1;};
    if m.bus.board.name()!="m5-stamp-addon-c6" {return 1;}
    match m.bus.sdio_command(command as u8,arg) {
        Ok(esp_periph::sdmmc::SdResponse::Short(value))=>{unsafe {out.write(value);}0},_=>3,
    }
}
