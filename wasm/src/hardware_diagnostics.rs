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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hardware_snapshot_is_bounded_cold_and_absent_on_unsupported_boards() {
        for name in ["stopwatch","none","esp32c3","esp32c6"] {
            // SAFETY: Each handle and buffer is exclusively owned for these calls.
            unsafe {
                let e=esp32sim_new(name.as_ptr(),name.len(),16,8);assert!(!e.is_null());
                let before=(esp32sim_cycles(e),esp32sim_insns(e));
                let mut short=[0xa5;8];let len=esp32sim_hardware_diagnostics(e,short.as_mut_ptr(),short.len());
                assert_eq!(short,[0xa5;8],"short destinations must remain untouched");
                if name=="stopwatch" {
                    assert!(len>8&&len<32768);let mut bytes=vec![0;len+1];bytes[len]=0xa5;
                    assert_eq!(esp32sim_hardware_diagnostics(e,bytes.as_mut_ptr(),len),len);
                    assert_eq!(bytes[len],0xa5);
                    let json=std::str::from_utf8(&bytes[..len]).unwrap();
                    assert!(json.contains("\"schemaVersion\":1"));assert!(json.contains("\"findings\":[]"));
                } else {assert_eq!(len,0);}
                assert_eq!((esp32sim_cycles(e),esp32sim_insns(e)),before);
                esp32sim_delete(e);
            }
        }
    }
}
