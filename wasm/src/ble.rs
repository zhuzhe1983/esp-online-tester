//! Explicit BLE modes: symbol-backed virtual HCI, C3 full link, or C61 digital radio observation.
use super::{log, Emu, MachineKind};
use esp_soc::SocBus;

/// Enable the S3 virtual HCI controller using a verified application ELF's symbols.
/// This models controller/HCI exchange, not a physical radio or a connected phone.
/// Call only before boot. ELF payloads are parsed for symbols; no segment is loaded.
/// The caller must match the symbols to the actual firmware before enabling this mode.
/// # Safety
/// `e` must be live and exclusive; `ptr` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_hci(e: *mut Emu, ptr: *const u8, len: usize) -> u32 {
    let e = unsafe { &mut *e };
    if e.booted { log("[emu] virtual HCI must be enabled before boot"); return 1 }
    let MachineKind::S3(m) = &mut e.m else { log("[emu] browser virtual HCI currently requires S3"); return 1 };
    if len > 256 * 1024 * 1024 { log("[emu] virtual HCI ELF exceeds 256 MiB"); return 1 }
    let result = esp_soc::elf::parse(unsafe { super::bytes(ptr, len) })
        .and_then(|elf| m.bus.enable_ble(&elf));
    match result {
        Ok(()) => { log("[emu] virtual HCI controller enabled from application symbols; no physical BLE radio or automatic pairing"); 0 }
        Err(reason) => { log(&format!("[emu] virtual HCI: {reason}")); 1 }
    }
}

fn flash_patch(e: &Emu) -> Option<&(usize, Vec<u8>)> {
    match &e.m { MachineKind::S3(m) => m.bus.ble.original_flash.as_ref(), _ => None }
}

/// Original byte count for the temporary virtual-HCI code overlay (zero when absent).
/// Browser storage must substitute these bytes when copying physical flash.
/// # Safety
/// `e` must be live and readable without overlapping mutable access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_hci_flash_len(e: *const Emu) -> usize {
    flash_patch(unsafe { &*e }).map_or(0, |(_, bytes)| bytes.len())
}

/// Physical flash offset of the original-byte overlay; meaningful only when its length is nonzero.
/// # Safety
/// `e` must be live and readable without overlapping mutable access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_hci_flash_offset(e: *const Emu) -> usize {
    flash_patch(unsafe { &*e }).map_or(0, |(offset, _)| *offset)
}

/// Read-only original bytes. Copy before the next mutable emulator call; never write/free them.
/// # Safety
/// `e` must be live and readable without overlapping mutable access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_hci_flash_ptr(e: *const Emu) -> *const u8 {
    flash_patch(unsafe { &*e }).map_or(std::ptr::null(), |(_, bytes)| bytes.as_ptr())
}

/// Enable the full controller before boot. Returns 1 for other chips or after boot.
/// # Safety
/// `e` must be a live emulator with exclusive access for this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_full(e: *mut Emu) -> u32 {
    let e = unsafe { &mut *e };
    if e.booted { log("[emu] full BLE must be enabled before boot"); return 1 }
    match &mut e.m {
        MachineKind::C3(m) => match m.bus.enable_ble_full(true) {
            Ok(()) => 0,
            Err(reason) => { log(&format!("[emu] {reason}")); 1 }
        },
        // C61 executes its actual MMIO controller without an HCI overlay. Its
        // bounded DMA observation queue exists independently of this opt-in.
        MachineKind::C61(_) => 0,
        _ => { log("[emu] full BLE requires C3 or C61; C61 supports legacy radio observation only"); 1 }
    }
}

/// Take one UTF-8 observation, returning its byte length (zero when empty).
/// # Safety
/// `e` must be a live emulator with exclusive access for this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_take(e: *mut Emu) -> usize {
    let e = unsafe { &mut *e };
    e.ble_out = match &mut e.m {
        MachineKind::C3(m) => m.bus.periph.ble_lc.take_observation().unwrap_or_default(),
        MachineKind::C61(m) => m.bus.periph.ble_mac.take_tx().map(|frame|format!("[ble-c61] us={} channel={} pdu={}",frame.timestamp_us,frame.channel,esp_soc::ble::peer::hex(&frame.pdu))).unwrap_or_default(),
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

/// Replace C61's virtual nearby legacy advertisers, one `Nearby::parse` spec
/// per line; an empty buffer removes all peers. At most 32 peers / 16 KiB.
/// Inputs enter real controller RX DMA; this does not inject HCI/guest callbacks.
/// # Safety
/// `e` must be live and exclusive; `ptr` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_ble_nearby(e: *mut Emu, ptr: *const u8, len: usize) -> u32 {
    let e = unsafe { &mut *e };
    let MachineKind::C61(m) = &mut e.m else { return 1 };
    if len > 16 * 1024 { return 1 }
    // SAFETY: the caller supplies a readable buffer, bounded above before access.
    let Ok(spec) = std::str::from_utf8(unsafe { super::bytes(ptr,len) }) else { return 1 };
    let specs: Vec<String> = spec.lines().map(str::to_owned).collect();
    let result = esp_soc::ble::peer::Nearby::parse_all(&specs).and_then(|peers|m.bus.ble_nearby(peers));
    match result { Ok(()) => 0, Err(reason) => { log(&format!("[emu] C61 nearby: {reason}")); 1 } }
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

#[cfg(test)]
mod tests {
    use super::*;

    // Metadata only: independent ELF encoder, no firmware code or loadable segment.
    fn symbol_elf(init_size: u32) -> Vec<u8> {
        let names: [(&str, u32, u32); 9] = [
            ("esp_bt_controller_init", 0x4038_8000, init_size),
            ("xTaskCreatePinnedToCore", 0x4038_8400, 0),
            ("vTaskDelay", 0x4038_8500, 0),
            ("esp_bt_controller_enable", 0x4038_8600, 0),
            ("esp_vhci_host_register_callback", 0x4038_8610, 0),
            ("esp_vhci_host_check_send_available", 0x4038_8620, 0),
            ("esp_vhci_host_send_packet", 0x4038_8630, 0),
            ("_bt_controller_bss_start", 0x3fc9_0000, 0),
            ("_bt_controller_bss_end", 0x3fc9_1000, 0),
        ];
        let mut strings = vec![0];
        let mut symbols = vec![0; 16];
        for (name, value, size) in names {
            let at = strings.len() as u32; strings.extend(name.bytes()); strings.push(0);
            symbols.extend(at.to_le_bytes()); symbols.extend(value.to_le_bytes()); symbols.extend(size.to_le_bytes());
            symbols.extend([0x12, 0, 0xf1, 0xff]); // STB_GLOBAL, STT_FUNC, SHN_ABS
        }
        let mut data = vec![0; 52 + 3 * 40];
        data[..6].copy_from_slice(b"\x7fELF\x01\x01");
        data[32..36].copy_from_slice(&52u32.to_le_bytes());
        data[46..48].copy_from_slice(&40u16.to_le_bytes());
        data[48..50].copy_from_slice(&3u16.to_le_bytes());
        let sym_offset = data.len() as u32;
        data.extend(&symbols);
        let str_offset = data.len() as u32;
        data.extend(&strings);
        for (slot, kind, offset, size, link, entry) in [(1, 2u32, sym_offset, symbols.len() as u32, 2u32, 16u32), (2, 3, str_offset, strings.len() as u32, 0, 0)] {
            let at = 52 + slot * 40;
            for (field, value) in [(4, kind), (16, offset), (20, size), (24, link), (36, entry)] {
                data[at + field..at + field + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        data
    }

    fn machine(board: &str) -> Box<Emu> {
        // SAFETY: the string lives for the call; ownership of the returned allocation is unique.
        unsafe { Box::from_raw(crate::esp32sim_new(board.as_ptr(), board.len(), 4, 0)) }
    }

    fn enable(emu: &mut Emu, data: &[u8]) -> u32 {
        // SAFETY: the machine is exclusively borrowed and the input slice is readable.
        unsafe { esp32sim_ble_hci(emu, data.as_ptr(), data.len()) }
    }

    #[test]
    fn virtual_hci_is_explicit_metadata_only_and_cold() {
        let mut emu = machine("none");
        let original = emu.m.flash().to_vec();
        assert!(!emu.m.bus().ble_enabled());
        assert_eq!(enable(&mut emu, &symbol_elf(0x400)), 0);
        assert!(emu.m.bus().ble_enabled());
        assert_eq!(emu.m.flash(), original);
        assert_eq!(emu.m.insns(), 0.0);
        let MachineKind::S3(m) = &emu.m else { panic!("expected S3"); };
        assert!(m.stubs.is_empty());
        assert!(!m.bus.ble.task_created);
        assert!(!m.bus.ble.session.is_advertising());
        assert_eq!(m.bus.ble.status, 0, "guest still has to initialize and enable the controller");
        emu.booted = true;
        assert_eq!(enable(&mut emu, &symbol_elf(0x400)), 1);
    }

    #[test]
    fn virtual_hci_rejects_invalid_or_incompatible_symbols_without_enabling() {
        let mut emu = machine("none");
        for data in [vec![], b"not an ELF".to_vec(), symbol_elf(8)] {
            assert_eq!(enable(&mut emu, &data), 1);
            assert!(!emu.m.bus().ble_enabled());
        }
        let mut p4 = machine("esp32p4");
        assert_eq!(enable(&mut p4, &symbol_elf(0x400)), 1);
        assert!(!p4.m.bus().ble_enabled());
    }

    #[test]
    fn virtual_hci_flash_overlay_exposes_original_bytes_and_disappears_on_reboot() {
        use esp_soc::ble::vhci::{install, VhciBus, INIT, XTENSA_CODE};
        let mut emu = machine("none");
        let mut symbols = esp_soc::elf::parse(&symbol_elf(0x400)).unwrap();
        for address in symbols.by_name.values_mut().filter(|address| **address >= 0x4038_8000) {
            *address = *address - 0x4038_8000 + esp32s3::bus::IBUS_LOW;
        }
        let original = {
            let MachineKind::S3(m) = &mut emu.m else { panic!("expected S3"); };
            let b = &mut m.bus;
            b.ble.enable(&symbols.by_name, 0x400, &<esp32s3::bus::SocBus as VhciBus>::abi()).unwrap();
            let mut original: Vec<u8> = (0..INIT as usize + XTENSA_CODE.len()).map(|i| i as u8).collect();
            original[..3].copy_from_slice(&[0x36, 0x41, 0]);
            b.flash[..original.len()].copy_from_slice(&original);
            b.mmu[0] = 0;
            install(b).unwrap();
            assert_ne!(&b.flash[..original.len()], &original);
            b.flash[0x9000] = 0x5a; // Unrelated guest Flash data remains exportable.
            original
        };
        // SAFETY: the test owns the emulator and copies before its next mutation.
        unsafe {
            let n = esp32sim_ble_hci_flash_len(&*emu);
            let offset = esp32sim_ble_hci_flash_offset(&*emu);
            let overlay = std::slice::from_raw_parts(esp32sim_ble_hci_flash_ptr(&*emu), n);
            let mut exported = emu.m.flash().to_vec();
            exported[offset..offset + n].copy_from_slice(overlay);
            assert_eq!(&exported[..original.len()], &original);
            assert_eq!(exported[0x9000], 0x5a);
            emu.m.s3_mut().unwrap().reboot();
            assert_eq!(esp32sim_ble_hci_flash_len(&*emu), 0);
            assert_eq!(&emu.m.flash()[..original.len()], &original);
            assert_eq!(emu.m.flash()[0x9000], 0x5a);
        }
    }
}
