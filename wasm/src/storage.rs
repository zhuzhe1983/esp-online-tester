//! Read-only flash access for browser device-storage snapshots. The host copies while the
//! worker is between run calls; this adds no work to the guest's instruction or tick paths.
use crate::Emu;

/// Length of the current flash array, including erased bytes.
///
/// # Safety
/// `e` must point to a live emulator with no overlapping mutable access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_flash_len(e: *const Emu) -> usize {
    // SAFETY: The caller guarantees a live shared emulator borrow.
    unsafe { &*e }.m.flash().len()
}

/// Read-only pointer to the current flash array. Copy it before another emulator call.
/// The pointer is not owned by the caller and must not be written or freed.
///
/// # Safety
/// `e` must point to a live emulator with no overlapping mutable access. The returned
/// bytes are valid only until the next mutable call or emulator deletion.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_flash_ptr(e: *const Emu) -> *const u8 {
    // SAFETY: The caller guarantees a live shared emulator borrow.
    unsafe { &*e }.m.flash().as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{esp32sim_new, esp32sim_delete, esp32sim_load_at, esp32sim_cycles, esp32sim_insns};
    use crate::machine::MachineKind;

    #[test]
    fn flash_export_keeps_contents_through_chip_reboot_and_never_ticks() {
        for board in ["none", "esp32", "esp32c3", "esp32c6", "esp32p4"] {
            // SAFETY: This test owns each handle and every source buffer until deletion.
            unsafe {
                let e = esp32sim_new(board.as_ptr(), board.len(), 8, 0);
                assert!(!e.is_null(), "{board}");
                assert_eq!(esp32sim_flash_len(e), 8 << 20);
                let data = [0x4e, 0x56, 0x53, 0x31];
                assert_eq!(esp32sim_load_at(e, 0x110000, data.as_ptr(), data.len()), 0);
                let before = (esp32sim_cycles(e), esp32sim_insns(e));
                assert_eq!(std::slice::from_raw_parts(esp32sim_flash_ptr(e).add(0x110000), data.len()), data);
                assert_eq!((esp32sim_cycles(e), esp32sim_insns(e)), before);
                match &mut (*e).m { MachineKind::S3(m) => { m.reboot(); }, MachineKind::Esp32(m) => { m.reboot(); }, MachineKind::C3(m) => { m.reboot(); }, MachineKind::C6(m) => { m.reboot(); }, MachineKind::P4(m) => { m.reboot(); } }
                assert_eq!(std::slice::from_raw_parts(esp32sim_flash_ptr(e).add(0x110000), data.len()), data, "{board}: physical flash survives a chip reset");
                esp32sim_delete(e);
            }
        }
    }
}
