//! Read-only flash access for browser device-storage snapshots. The host copies while the
//! worker is between run calls; this adds no work to the guest's instruction or tick paths.
use crate::Emu;

/// Length of the primary chip's flash array, including erased bytes. Co-processor
/// flash is separate and is not part of this snapshot ABI.
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



struct SharedSd(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
impl esp_periph::sdmmc::SdStore for SharedSd {
    fn len(&self)->u64 {self.0.lock().expect("SD image mutex poisoned").len() as u64}
    fn read(&mut self,lba:u64,buf:&mut [u8;512])->std::io::Result<()> {let bytes=self.0.lock().expect("SD image mutex poisoned");let at=usize::try_from(lba).ok().and_then(|v|v.checked_mul(512)).ok_or(std::io::ErrorKind::UnexpectedEof)?;let end=at.checked_add(512).ok_or(std::io::ErrorKind::UnexpectedEof)?;buf.copy_from_slice(bytes.get(at..end).ok_or(std::io::ErrorKind::UnexpectedEof)?);Ok(())}
    fn write(&mut self,lba:u64,data:&[u8;512])->std::io::Result<()> {let mut bytes=self.0.lock().expect("SD image mutex poisoned");let at=usize::try_from(lba).ok().and_then(|v|v.checked_mul(512)).ok_or(std::io::ErrorKind::UnexpectedEof)?;let end=at.checked_add(512).ok_or(std::io::ErrorKind::UnexpectedEof)?;bytes.get_mut(at..end).ok_or(std::io::ErrorKind::UnexpectedEof)?.copy_from_slice(data);Ok(())}
}
pub(crate) fn load_sd(e:&mut Emu,data:&[u8])->u32 {
    if e.booted || data.len()>128<<20 || data.len()<1<<20 || !data.len().is_multiple_of(512<<10) {crate::log("[emu] SD image must be 1..128 MiB, a multiple of 512 KiB, and loaded before boot");return 1;}
    let bytes=std::sync::Arc::new(std::sync::Mutex::new(data.to_vec()));
    match e.m.bus().attach_sd_card(Box::new(SharedSd(bytes.clone())),Default::default()) {
        Ok(_)=>{e.sd_image=Some(bytes);0},Err(error)=>{crate::log(&format!("[emu] SD card: {error}"));1}
    }
}
/// Current SD image length. No card returns zero.
/// # Safety
/// `e` must be a live emulator with no overlapping mutable access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_sd_len(e:*const Emu)->usize {
    // SAFETY: Caller supplies a live shared emulator.
    unsafe {&*e}.sd_image.as_ref().map_or(0,|s|s.lock().expect("SD image mutex poisoned").len())
}
/// Read-only SD image pointer, valid until the next mutable emulator call. Do not free it.
/// # Safety
/// `e` must be a live emulator with no overlapping access; copy before running the guest.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_sd_ptr(e:*const Emu)->*const u8 {
    // SAFETY: Caller supplies a live shared emulator; media buffers never resize during a run.
    unsafe {&*e}.sd_image.as_ref().map_or(std::ptr::null(),|s|s.lock().expect("SD image mutex poisoned").as_ptr())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{esp32sim_new, esp32sim_delete, esp32sim_load_at, esp32sim_cycles, esp32sim_insns};
    use crate::machine::MachineKind;

    #[test]
    fn flash_export_keeps_contents_through_chip_reboot_and_never_ticks() {
        for board in ["none", "esp32", "esp32c3", "esp32c6", "esp32p4", "esp32p4-rev3", "esp32p4+c6", "esp32p4-rev3+c6", "waveshare-p4-lcd35"] {
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
                if board.ends_with("+c6") {
                    assert_eq!(esp32sim_load_at(e, 0x80110000, [0x43, 0x36].as_ptr(), 2), 0);
                    assert_eq!(std::slice::from_raw_parts(esp32sim_flash_ptr(e).add(0x110000), data.len()), data, "co-processor writes must not replace primary flash");
                }
                match &mut (*e).m { MachineKind::S3(m) => { m.reboot(); }, MachineKind::Esp32(m) => { m.reboot(); }, MachineKind::C3(m) => { m.reboot(); }, MachineKind::C6(m) => { m.reboot(); }, MachineKind::P4(m) => { m.reboot(); },
                    MachineKind::P4Rev3(m) => { m.reboot(); }, MachineKind::P4C6(m) => { m.p4.reboot(); }, MachineKind::P4Rev3C6(m) => { m.p4.reboot(); } }
                assert_eq!(std::slice::from_raw_parts(esp32sim_flash_ptr(e).add(0x110000), data.len()), data, "{board}: physical flash survives a chip reset");
                esp32sim_delete(e);
            }
        }
    }
    #[test]
    fn sd_export_retains_written_blocks_across_reset_and_checks_attachment() {
        use esp_periph::sdmmc::SdStore;
        // SAFETY: This test owns the emulator and every buffer for their full lifetime.
        unsafe {
            let name="papermono";let e=esp32sim_new(name.as_ptr(),name.len(),16,8);let image=vec![0;1<<20];
            assert_eq!(esp32sim_sd_len(e),0);assert_eq!(load_sd(&mut *e,&image),0);
            let mut media=SharedSd((*e).sd_image.as_ref().unwrap().clone());media.write(7,&[0x5a;512]).unwrap();
            let before=(esp32sim_cycles(e),esp32sim_insns(e));assert_eq!(esp32sim_sd_len(e),1<<20);
            assert_eq!(std::slice::from_raw_parts(esp32sim_sd_ptr(e).add(7*512),512),[0x5a;512]);
            assert_eq!((esp32sim_cycles(e),esp32sim_insns(e)),before);
            (*e).m.s3_mut().unwrap().reboot();assert_eq!(std::slice::from_raw_parts(esp32sim_sd_ptr(e).add(7*512),512),[0x5a;512]);
            (*e).booted=true;assert_eq!(load_sd(&mut *e,&image),1);esp32sim_delete(e);
        }
    }

}
