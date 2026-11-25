//! Explicit, fixed-image offline Wi-Fi driver compatibility. Not a radio model.
use super::{log, Emu, MachineKind};

/// Enable the selected C5 driver profile after loading Flash/ROM and before boot.
/// No automatic fallback, network association or IP configuration is supplied.
/// # Safety
/// `e` must be live and exclusive; `p` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_c5_wifi_driver_compat(e:*mut Emu,p:*const u8,len:usize)->u32 {
    let e=unsafe {&mut *e};
    if e.booted||len>128 {log("[emu] C5 offline driver profile must be selected before boot");return 1;}
    let MachineKind::C5(m)=&mut e.m else {log("[emu] C5 offline driver profile requires the supported C5 product");return 1;};
    let profile=unsafe {super::text(p,len)};
    match esp32c5::enable_wifi_driver_compat(m,profile) {
        Ok(())=>{log("[emu] C5 offline Wi-Fi driver compatibility enabled; no native Wi-Fi, AP, association or IP; driver configuration is session-RAM only");0}
        Err(reason)=>{log(&format!("[emu] C5 offline driver profile rejected: {reason}"));1}
    }
}

/// Copy a pure driver-compatibility snapshot, returning its required length.
/// Returns zero when no profile is installed. Short buffers remain untouched.
/// # Safety
/// `e` must be live and readable; `p` must be writable for `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_c5_wifi_driver_compat_state(e:*const Emu,p:*mut u8,capacity:usize)->usize {
    let e=unsafe {&*e};
    let MachineKind::C5(m)=&e.m else {return 0;};
    let Some(json)=m.bus.wifi_driver_compat_snapshot() else {return 0;};
    if json.len()<=capacity {unsafe {std::ptr::copy_nonoverlapping(json.as_ptr(),p,json.len());}}
    json.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{esp32sim_new,esp32sim_delete,esp32sim_cycles,esp32sim_insns};
    #[test]
    fn offline_driver_abi_does_not_enable_unselected_or_other_chip_machines() {
        for board in ["none","esp32c3","esp32c6","stamp-c5","waveshare-c5-lcd273"] {
            // SAFETY: This test owns each handle and every supplied buffer.
            unsafe {
                let (flash,psram)=if board=="stamp-c5"{(4,0)}else{(32,8)};
                let e=esp32sim_new(board.as_ptr(),board.len(),flash,psram);assert!(!e.is_null());
                let before=(esp32sim_cycles(e),esp32sim_insns(e));let mut out=[0xa5;16];
                assert_eq!(esp32sim_c5_wifi_driver_compat_state(e,out.as_mut_ptr(),out.len()),0);
                assert_eq!(esp32sim_c5_wifi_driver_compat(e,b"unknown".as_ptr(),7),1);
                assert_eq!(esp32sim_c5_wifi_driver_compat_state(e,out.as_mut_ptr(),out.len()),0);
                assert_eq!(out,[0xa5;16]);assert_eq!((esp32sim_cycles(e),esp32sim_insns(e)),before);
                esp32sim_delete(e);
            }
        }
    }
}
