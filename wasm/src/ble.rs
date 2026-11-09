//! Passive observation of the C3 full link controller, and the virtual VHCI controller (`--ble`)
//! with its peer, driven from the page.
use super::{log, Emu, MachineKind};
use esp_soc::SocBus;

/// Enable the full controller before boot. Returns 1 for other chips or after boot.
/// # Safety
/// `e` must be a live emulator with exclusive access for this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_full(e: *mut Emu) -> u32 {
    let e = unsafe { &mut *e };
    if e.booted { log("[emu] full BLE must be enabled before boot"); return 1 }
    let MachineKind::C3(m) = &mut e.m else { log("[emu] full BLE requires C3"); return 1 };
    if let Err(reason) = m.bus.enable_ble_full(true) { log(&format!("[emu] {reason}")); return 1 }

    0
}

/// Take one UTF-8 observation, returning its byte length (zero when empty).
/// # Safety
/// `e` must be a live emulator with exclusive access for this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_take(e: *mut Emu) -> usize {
    let e = unsafe { &mut *e };
    e.ble_out = match &mut e.m {
        MachineKind::C3(m) => m.bus.periph.ble_lc.take_observation().unwrap_or_default(),
        _ => String::new(),
    };
    e.ble_out.len()
}

/// Bytes from `ble_take`, valid until the next take or emulator deletion.
/// # Safety
/// `e` must be live and readable. The returned bytes must not be modified.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_ptr(e: *const Emu) -> *const u8 {
    unsafe { &*e }.ble_out.as_ptr()
}


/// Toggle the virtual active scanner. Returns 1 unless C3 full mode is enabled.
/// # Safety
/// `e` must be a live emulator with exclusive access for this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_scan(e: *mut Emu, enabled: u32) -> u32 {
    let e = unsafe { &mut *e };
    let MachineKind::C3(m) = &mut e.m else { return 1 };
    if !m.bus.periph.ble_lc.enabled() { return 1 }
    m.bus.periph.ble_lc.scan(enabled != 0);
    0
}


/// Send `connect`, `read-uuid SERVICE CHARACTERISTIC`, `disconnect` or
/// `central-stop` to C3 full mode.
/// Returns 0 on success, 1 for invalid commands, other chips or disabled full mode.
/// # Safety
/// `e` must be live and exclusive; `ptr` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_command(e: *mut Emu, ptr: *const u8, len: usize) -> u32 {
    let e = unsafe { &mut *e };
    let command = unsafe { super::text(ptr, len) };
    let MachineKind::C3(m) = &mut e.m else { return 1 };
    u32::from(m.bus.periph.ble_lc.command(command).is_err())
}

/// Enable the virtual BLE controller (the CLI's `--ble`) on the S3, C3, C6 or classic ESP32
/// before boot. `elf` is the application ELF, whose symbols locate the guest's controller
/// functions. The controller's `[ble]` lines are kept for `esp32sim_ble_virtual_take`. Returns 0,
/// or 1 after boot, for a bad ELF, another chip or an ELF the controller cannot use; the reason
/// goes to `host_log`.
/// # Safety
/// `e` must be live and exclusive; `ptr` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_virtual(e: *mut Emu, ptr: *const u8, len: usize) -> u32 {
    let e = unsafe { &mut *e };
    if e.booted { log("[emu] the virtual BLE controller must be enabled before boot"); return 1 }
    let elf = match esp_soc::elf::parse(unsafe { super::bytes(ptr, len) }) {
        Ok(elf) => elf,
        Err(reason) => { log(&format!("[emu] ble: {reason}")); return 1 }
    };
    let bus = e.m.bus();
    if let Err(reason) = bus.enable_ble(&elf) { log(&format!("[emu] ble: {reason}")); return 1 }
    bus.ble_keep_log();
    0
}

/// Give the virtual peer a command, as the CLI script's `ble` commands take it: `connect`,
/// `discover`, `read HANDLE`, `write HANDLE HEX` or `subscribe CCC_HANDLE` (the others are full
/// BLE's). It runs from the current emulated time; the results come as `[ble]` lines. Returns 0,
/// or 1 for a bad command or no virtual controller (the reason goes to `host_log`).
/// # Safety
/// `e` must be live and exclusive; `ptr` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_virtual_command(e: *mut Emu, ptr: *const u8, len: usize) -> u32 {
    let e = unsafe { &mut *e };
    let command = unsafe { super::text(ptr, len) };
    match e.m.bus().ble_command(command) {
        Ok(()) => 0,
        Err(reason) => { log(&format!("[emu] ble: {reason}")); 1 }
    }
}

/// Take the oldest kept `[ble]` line of the virtual controller (without the prefix), returning
/// its UTF-8 byte length (zero when none). The bytes are at `esp32sim_ble_ptr`, valid until the
/// next take or emulator deletion. Lines lost to the 1024-line bound come first as `dropped=N`.
/// # Safety
/// `e` must be a live emulator with exclusive access for this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_virtual_take(e: *mut Emu) -> usize {
    let e = unsafe { &mut *e };
    e.ble_out = e.m.bus().ble_log_take().unwrap_or_default();
    e.ble_out.len()
}
