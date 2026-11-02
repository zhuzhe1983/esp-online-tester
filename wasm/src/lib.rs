//! The emulator as a WebAssembly module. A C ABI, hand-driven from `web/wasm/worker.js` — no
//! bindgen, no dependencies. The machine produces exactly the messages the WebSocket UI speaks
//! (`docs/web-ui.md`); here they are queued in a `WebServer::queued()` sink and handed to JS.
//!
//! Lifecycle: `esp32sim_new` → `esp32sim_load` (ROM, bootloader, partition table, app, ELF
//! symbols, script) → optional `esp32sim_wifi` → `esp32sim_boot` → repeated `esp32sim_run(cycles,
//! unix_ms)` with `esp32sim_out_*` draining the outbox after each slice and `esp32sim_in_*`
//! feeding the page's inputs.
use esp_soc::web::{json_escape, WebServer};
use esp_soc::{Machine, Soc, SocBus};

mod machine;
#[cfg(any(test, feature = "psram-tests"))]
pub mod psram_tests;
mod network;
mod ethernet;
mod ble;
pub use ble::*;
pub use ethernet::*;
mod s3;
#[cfg(target_arch = "wasm32")]
mod browser_jit;
use machine::MachineKind;
pub use network::*;
pub use s3::*;
#[cfg(target_arch = "wasm32")]
pub use browser_jit::*;
#[cfg(target_arch = "wasm32")]
use browser_jit::BrowserJit;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" { fn host_log(ptr: *const u8, len: usize); }
#[cfg(not(target_arch = "wasm32"))]
unsafe fn host_log(ptr: *const u8, len: usize) {
    // SAFETY: The caller provides a readable string pointer for exactly `len` bytes.
    let message = unsafe { std::slice::from_raw_parts(ptr, len) };
    eprintln!("{}", String::from_utf8_lossy(message));
}

fn log(s: &str) {
    // SAFETY: `s` is readable for its length and the host does not retain the pointer.
    unsafe { host_log(s.as_ptr(), s.len()); }
}

pub struct Emu {
    m: MachineKind,
    /// the last drained outbox: (1 text | 2 binary, payload), addressed by index from JS
    out: Vec<(u8, Vec<u8>)>,
    ethernet_out: Vec<Vec<u8>>,
    ble_out: String,
    booted: bool,
    /// a mask ROM ELF was loaded: a ROM boot without one would run from empty memory and print nothing
    rom_loaded: bool,
    #[cfg(target_arch = "wasm32")]
    jit: BrowserJit,
}

/// Match the browser form's 32 MiB per-memory limit before any allocation.
fn mib_bytes(mib: u32) -> Option<usize> {
    mib.checked_mul(1 << 20).filter(|&bytes| bytes <= 32 << 20).map(|bytes| bytes as usize)
}

/// Borrow an ABI buffer.
///
/// # Safety
/// For nonzero `len`, `ptr` must be non-null and readable for `len` bytes. The memory must remain
/// unchanged and valid for the returned lifetime. A null pointer is accepted only when `len` is 0.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 { &[] } else {
        // SAFETY: The caller supplies the validity, immutability, and lifetime guarantees above.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// Borrow a UTF-8 ABI buffer, treating invalid UTF-8 as an empty string.
///
/// # Safety
/// The pointer and length must satisfy `bytes`'s contract.
unsafe fn text<'a>(ptr: *const u8, len: usize) -> &'a str {
    // SAFETY: The caller satisfies `bytes`'s pointer and lifetime contract.
    std::str::from_utf8(unsafe { bytes(ptr, len) }).unwrap_or("")
}

/// Buffers the page fills before handing them to `esp32sim_load` / `esp32sim_in_*`.
#[no_mangle] pub extern "C" fn esp32sim_alloc(len: usize) -> *mut u8 { let mut v = vec![0u8; len.max(1)]; let p = v.as_mut_ptr(); std::mem::forget(v); p }
/// Release a buffer returned by `esp32sim_alloc`.
///
/// # Safety
/// `ptr` must be the live pointer returned by `esp32sim_alloc(len)`. It must not be used again.
#[no_mangle] pub unsafe extern "C" fn esp32sim_free(ptr: *mut u8, len: usize) {
    // SAFETY: The caller returns the allocation with the same length and unique ownership.
    drop(unsafe { Vec::from_raw_parts(ptr, len.max(1), len.max(1)) });
}

/// `board` is a CLI board name (atech14, waveshare-cam, waveshare-lcd4b,
/// waveshare-amoled18-v2, none) for the ESP32-S3, `esp32` (the bare esp32dev module) or a classic
/// board such as `ttgo-tdisplay` for the classic ESP32, `esp32c6` or `waveshare-c6-lcd147` for the
/// C6, `esp32c3` for the C3, or `esp32p4` (rev < 3) or `esp32p4-rev3` for the P4, with `+c6` for its C6
/// co-processor, which are console-only and take no board, or `waveshare-p4-lcd35` for that P4
/// rev >= 3 board (its panel, touch and codec). Null on failure.
/// Flash and PSRAM are each limited to 32 MiB, matching the browser configuration form.
/// Classic ESP32 enables PSRAM only at 4 MiB; other sizes within the cap start without
/// PSRAM (0 is the default), preserving existing callers' configurations.
///
/// # Safety
/// For nonzero `board_len`, `board` must be non-null and readable for `board_len` bytes throughout
/// this call. A null pointer is accepted only when `board_len` is 0.
/// On WASM, destroy the previous emulator before creating another: timing emitter
/// configuration is module-wide, not isolated between simultaneously live emulators.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_new(board: *const u8, board_len: usize, flash_mb: u32, psram_mb: u32) -> *mut Emu {
    std::panic::set_hook(Box::new(|info| log(&format!("[emu] panic: {}", info))));
    // SAFETY: The caller provides a readable board-name buffer for this call.
    let board = unsafe { text(board, board_len) }.to_string();
    let Some(flash_bytes) = mib_bytes(flash_mb.max(1)) else { log("[emu] flash size exceeds the supported memory range"); return std::ptr::null_mut() };
    let Some(psram_bytes) = mib_bytes(psram_mb) else { log("[emu] PSRAM size exceeds the supported memory range"); return std::ptr::null_mut() };
    let c6_board = if matches!(board.as_str(), "esp32c6" | "c6") {
        esp32c6::board::make_board("none")
    } else if matches!(board.as_str(), "none" | "bare") {
        None // Unqualified bare boards select the default S3.
    } else {
        esp32c6::board::make_board(&board)
    };
    // The classic ESP32: `esp32` is the bare module (esp32dev); its boards are named like the CLI's
    // `--board` (ttgo-tdisplay). `none`/`bare` stay the S3's, so the classic factory is not asked.
    // `esp32p4` is rev < 3, `esp32p4-rev3` rev >= 3; `+c6` adds the ESP32-C6 co-processor on SDIO
    // (`esp32p4::coproc`), whose images load with `COPROC_KIND` / `COPROC_FLASH`.
    let (p4_name, coproc) = board.strip_suffix("+c6").map_or((board.as_str(), false), |b| (b, true));
    let p4_board = esp32p4::board::make_board(p4_name).filter(|_| !matches!(p4_name, "none" | "bare"));
    let p4_rev3 = p4_board.is_some() || matches!(p4_name, "esp32p4-rev3" | "p4-rev3");   // the board has a v3.2 chip
    let p4 = p4_rev3 || matches!(p4_name, "esp32p4" | "p4");
    let classic_board = if board == "esp32" {
        esp32::board::make_board("esp32dev")
    } else if p4 || c6_board.is_some() || matches!(board.as_str(), "none" | "bare") {
        None
    } else {
        esp32::board::make_board(&board)
    };
    let m = if let Some(b) = classic_board {
        let mut m = esp32::machine([0x24, 0x6f, 0x28, 0, 0x11, 0x22], flash_bytes);
        m.bus.board = b;
        m.bus.attach_board_devices();
        m.bus.set_flash_size(flash_bytes);
        let classic_psram_bytes = if matches!(psram_mb, 0 | 4) {
            psram_bytes
        } else {
            log("[emu] classic ESP32 PSRAM is 0 or 4 MiB; starting without PSRAM");
            0
        };
        if let Err(reason) = m.bus.set_psram_size(classic_psram_bytes) {
            log(&format!("[emu] {}", reason));
            return std::ptr::null_mut();
        }
        m.console.mask = 2;
        prepare(&mut m);
        MachineKind::Esp32(Box::new(m))
    } else if board == "esp32c3" || board == "c3" {
        let mut m = esp32c3::machine([0x3c, 0x84, 0x27, 0xb6, 0xa7, 0x1c], flash_bytes);
        m.bus.set_flash_size(flash_bytes);
        m.console.mask = 2;                                  // the ROM mirrors its console to UART0 and USB-Serial/JTAG
        prepare(&mut m);
        MachineKind::C3(Box::new(m))
    } else if p4 {
        fn p4_machine<R: esp32p4::Revision>(flash_bytes: usize, psram_bytes: usize, board: Option<esp_soc::Board>) -> Machine<esp32p4::Esp32p4<R>> {
            let mut m = esp32p4::machine_rev::<R>([0x30, 0xed, 0xa0, 0x00, 0x11, 0x22], flash_bytes);
            if let Some(b) = board { m.bus.board = b; m.bus.attach_board_devices(); }
            m.bus.set_flash_size(flash_bytes);
            let _ = m.bus.set_psram_size(psram_bytes);
            m.console.mask = 2;
            prepare(&mut m);
            m
        }
        match (p4_rev3, coproc) {
            (false, false) => MachineKind::P4(Box::new(p4_machine::<esp32p4::RevLt3>(flash_bytes, psram_bytes, p4_board))),
            (true, false) => MachineKind::P4Rev3(Box::new(p4_machine::<esp32p4::RevGe3>(flash_bytes, psram_bytes, p4_board))),
            (false, true) => MachineKind::P4C6(Box::new(esp32p4::coproc::CoSim::new(p4_machine(flash_bytes, psram_bytes, p4_board), coproc_c6(), coproc_config()))),
            (true, true) => MachineKind::P4Rev3C6(Box::new(esp32p4::coproc::CoSim::new(p4_machine(flash_bytes, psram_bytes, p4_board), coproc_c6(), coproc_config()))),
        }
    } else if let Some(b) = c6_board {
        let mut m = esp32c6::machine([0xdc, 0x1e, 0xd5, 0x6e, 0x8c, 0xdc], flash_bytes);
        m.bus.board = b;
        m.bus.attach_board_devices();
        m.bus.set_flash_size(flash_bytes);
        m.console.mask = 2;
        prepare(&mut m);
        MachineKind::C6(Box::new(m))
    } else {
        let mut m = esp32s3::machine([0x44, 0x1b, 0xf6, 0x75, 0xdc, 0xe0]);
        let Some(b) = esp32s3::board::make_board(&board) else { log(&format!("[emu] unknown board '{}'", board)); return std::ptr::null_mut() };
        m.bus.board = b;
        m.bus.attach_board_devices();
        m.bus.set_flash_size(flash_bytes);
        let _ = m.bus.set_psram_size(psram_bytes);
        m.bus.periph.lcd_cam.frame_cycles = esp32s3::periph::CPU_HZ / 10;
        prepare(&mut m);
        MachineKind::S3(Box::new(m))
    };
    // A worker reuses this WASM instance after deleting its previous emulator.
    // No generated code is shared: it was owned by the old CPUs and dropped with them.
    #[cfg(target_arch = "wasm32")]
    {
        use std::sync::atomic::Ordering::Relaxed;
        xtensa_lx7::jit::PRICED.store(false, Relaxed);
        xtensa_lx7::jit::CACHE_PROBES.store(false, Relaxed);
        xtensa_lx7::jit::FETCH_RING.store(false, Relaxed);
        xtensa_lx7::jit::CACHE_SET_MASK.store(63, Relaxed);
    }
    Box::into_raw(Box::new(Emu {
        m,
        out: Vec::new(), ethernet_out: Vec::new(), ble_out: String::new(),
        booted: false, rom_loaded: false,
        #[cfg(target_arch = "wasm32")]
        jit: BrowserJit::default(),
    }))
}

/// The co-processor as the CLI's `--coproc` makes it: a bare 4 MB ESP32-C6 with an Espressif-OUI MAC
/// of its own; the page loads its images and boots it with the P4.
fn coproc_c6() -> esp32c6::Machine {
    let mut m = esp32c6::machine([0x40, 0x4c, 0xca, 0x00, 0x11, 0x22], 4 << 20);
    m.bus.set_flash_size(4 << 20);
    m.rt.enabled = false;                                    // std::time does not exist here
    m
}

/// The CLI's defaults: 10 µs slices, EN on GPIO 54, both chips rebooting, both on UART0.
fn coproc_config() -> esp32p4::coproc::Config { esp32p4::coproc::Config::default() }

/// The page is the one client: messages queue in a `WebServer` sink; the worker paces the run.
fn prepare<S: Soc>(m: &mut Machine<S>) {
    m.web = Some(WebServer::queued());
    m.rt.enabled = false;                                    // std::time does not exist here
    m.console.capture = true;
}

/// Destroy an emulator returned by `esp32sim_new`. A null pointer is ignored.
///
/// # Safety
/// A non-null `e` must be the live pointer returned by `esp32sim_new`. The caller must have
/// exclusive access, and the pointer must not be used again.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_delete(e: *mut Emu) {
    if !e.is_null() {
        // SAFETY: The caller returns the live allocation with unique ownership.
        drop(unsafe { Box::from_raw(e) });
    }
}

/// kind: 0 mask-ROM ELF, 1 bootloader (chip-specific offset), 2 partition table (0x8000), 3 app (0x10000),
/// 4 ELF for symbols, 5 whole flash image (0x0), 6 script text, 7 camera picture (BMP/PPM, or baseline JPEG
/// delivered verbatim by cameras that accept it),
/// 8 SD card image for the board's socket (P4; held in memory, at least 512 KiB).
/// Returns 0, or 1 with the reason logged.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access. For nonzero `len`,
/// `ptr` must be non-null and readable for `len` bytes throughout this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_load(e: *mut Emu, kind: u32, ptr: *const u8, len: usize) -> u32 {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    // SAFETY: The caller provides a readable input buffer for this call.
    let data = unsafe { bytes(ptr, len) };
    match e.m.load(kind, data) { Ok(()) => { if kind == 0 { e.rom_loaded = true; } 0 } Err(msg) => { log(&format!("[emu] load kind {}: {}", kind, msg)); 1 } }
}

/// Write bytes into flash at an arbitrary offset (a data partition's contents).
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access. For nonzero `len`,
/// `ptr` must be non-null and readable for `len` bytes throughout this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_load_at(e: *mut Emu, offset: u32, ptr: *const u8, len: usize) -> u32 {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    // SAFETY: The caller provides a readable input buffer for this call.
    let data = unsafe { bytes(ptr, len) };
    match e.m.write_flash(offset as usize, data) { Ok(()) => 0, Err(msg) => { log(&format!("[emu] flash {:#x}: {}", offset, msg)); 1 } }
}

/// `--stub NAME[=value]`: return `value` immediately when execution reaches the function's entry.
/// NAME is a symbol (needs the ELF loaded) or a hex address. Returns 1 if it cannot be resolved.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access. For nonzero `len`,
/// `name` must be non-null and readable for `len` bytes throughout this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_stub(e: *mut Emu, name: *const u8, len: usize, value: u32) -> u32 {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    // SAFETY: The caller provides a readable symbol name for this call.
    e.m.stub(unsafe { text(name, len) }, value)
}

/// Parse a complete NAME[=value] stub using the same rules as the CLI.
/// # Safety
/// `e` must be live and exclusively borrowed; `spec` must be readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_stub_spec(e: *mut Emu, spec: *const u8, len: usize) -> u32 {
    let e = unsafe { &mut *e };
    let Ok(spec) = std::str::from_utf8(unsafe { bytes(spec, len) }) else { return 1; };
    match esp_soc::load::stub_spec(spec) {
        Ok((name, value)) => e.m.stub(name, value),
        Err(reason) => { log(&format!("[emu] stub: {reason}")); 1 }
    }
}

/// Attach an analysis: `profile-blocks`, `coverage`, `irq-latency` (no argument), `trace-fn`
/// (arg = symbol prefix as text). Returns 1 for an unknown name.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access. For each nonzero
/// length, its corresponding `name` or `arg` pointer must be non-null and readable throughout this
/// call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_observer(e: *mut Emu, name: *const u8, len: usize, arg: *const u8, arg_len: usize) -> u32 {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    // SAFETY: The caller provides readable name and argument buffers for this call.
    e.m.observer(unsafe { text(name, len) }, unsafe { text(arg, arg_len) })
}

/// Every observer's report so far, as `emu` messages in the outbox.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_reports(e: *mut Emu) {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    let r = e.m.reports();
    if let Some(w) = e.m.web() { for line in r.lines() { w.send_text(&format!("{{\"t\":\"emu\",\"msg\":\"{}\"}}", json_escape(line))); } }
}

/// Start from the mask ROM (the normal path) or, with `app_direct` set, straight into the app image.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_boot(e: *mut Emu, app_direct: u32) -> u32 {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    // a ROM boot without a ROM runs from empty memory and prints nothing: say so (the page refuses it
    // earlier; the ABI stays permissive for callers that drive the machine another way)
    if app_direct == 0 && !e.rom_loaded { log("[emu] boot: no mask ROM loaded: the run will print nothing (pick the chip's ROM ELF, or boot the app directly)"); }
    if let Err(msg) = e.m.boot(app_direct != 0) { log(&format!("[emu] boot: {}", msg)); return 1; }
    // the WebSocket server announces the board in its per-client hello; here there is one client
    let name = e.m.board_name();
    let chip = e.m.chip();
    if let Some(w) = e.m.web() { w.send_text(&format!("{{\"t\":\"board\",\"name\":\"{}\",\"chip\":\"{}\"}}", name, chip)); }
    e.booted = true; 0
}

/// Run for `cycles` more emulated cycles. Returns 0 while the machine can go on; otherwise a stop
/// code: 2 unimplemented instruction, 3 breakpoint/ebreak, 4 exception limit, 5 semihosting call.
/// A chip reset (esp_restart, watchdog) reboots through the ROM and keeps going, like the CLI.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_run(e: *mut Emu, cycles: u32, unix_ms: f64) -> u32 {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    if !e.booted { return 9; }
    #[cfg(target_arch = "wasm32")] esp_soc::host::set_unix_time_ms(unix_ms as u64);
    let _ = unix_ms;
    e.m.run_slice(cycles)
}

/// The emulated CPU clock, so the driver paces the right chip: 240 MHz on the S3, 160 on the C3.
///
/// # Safety
/// `e` must point to a live emulator, and no mutable access may overlap this call.
#[no_mangle] pub unsafe extern "C" fn esp32sim_cpu_hz(e: *mut Emu) -> f64 {
    // SAFETY: The caller provides shared access to a live emulator without overlapping mutation.
    unsafe { &*e }.m.cpu_hz()
}
/// Return the current emulated cycle count.
///
/// # Safety
/// `e` must point to a live emulator, and no mutable access may overlap this call.
#[no_mangle] pub unsafe extern "C" fn esp32sim_cycles(e: *mut Emu) -> f64 {
    // SAFETY: The caller provides shared access to a live emulator without overlapping mutation.
    unsafe { &*e }.m.cycles()
}
/// Return the current emulated instruction count.
///
/// # Safety
/// `e` must point to a live emulator, and no mutable access may overlap this call.
#[no_mangle] pub unsafe extern "C" fn esp32sim_insns(e: *mut Emu) -> f64 {
    // SAFETY: The caller provides shared access to a live emulator without overlapping mutation.
    unsafe { &*e }.m.insns()
}

#[cfg(target_arch = "wasm32")]
#[cfg(feature = "cpu-profile")]
#[no_mangle] pub extern "C" fn esp32sim_kernel_census(i: u32) -> f64 { xtensa_lx7::jit::CENSUS.get(i as usize).map_or(0.0, |n| n.load(std::sync::atomic::Ordering::Relaxed) as f64) }

/// Drain what the machine sent since the last call; then index it with the accessors below.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_out_take(e: *mut Emu) -> u32 {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    e.out = e.m.web().map(|w| w.take_outbox()).unwrap_or_default();
    e.out.len() as u32
}
/// Return the kind of one message from the last output drain, or 0 for an invalid index.
///
/// # Safety
/// `e` must point to a live emulator, and no mutable access may overlap this call.
#[no_mangle] pub unsafe extern "C" fn esp32sim_out_kind(e: *mut Emu, i: u32) -> u32 {
    // SAFETY: The caller provides shared access to a live emulator without overlapping mutation.
    unsafe { &*e }.out.get(i as usize).map(|m| m.0 as u32).unwrap_or(0)
}
/// Return a message's data pointer, or null for an invalid index.
///
/// # Safety
/// `e` must point to a live emulator, and no mutable access may overlap this call. A non-null
/// result remains valid until the next `esp32sim_out_take` or `esp32sim_delete` call for `e`.
#[no_mangle] pub unsafe extern "C" fn esp32sim_out_ptr(e: *mut Emu, i: u32) -> *const u8 {
    // SAFETY: The caller provides shared access to a live emulator without overlapping mutation.
    unsafe { &*e }.out.get(i as usize).map(|m| m.1.as_ptr()).unwrap_or(std::ptr::null())
}
/// Return a message's data length, or 0 for an invalid index.
///
/// # Safety
/// `e` must point to a live emulator, and no mutable access may overlap this call.
#[no_mangle] pub unsafe extern "C" fn esp32sim_out_len(e: *mut Emu, i: u32) -> usize {
    // SAFETY: The caller provides shared access to a live emulator without overlapping mutation.
    unsafe { &*e }.out.get(i as usize).map(|m| m.1.len()).unwrap_or(0)
}

/// Page input in the WebSocket JSON protocol.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access. For nonzero `len`,
/// `ptr` must be non-null and readable for `len` bytes throughout this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_in_text(e: *mut Emu, ptr: *const u8, len: usize) {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    // SAFETY: The caller provides a readable input buffer for this call.
    let input = unsafe { text(ptr, len) };
    if let Some(w) = e.m.web() { w.push_incoming(input.to_string()); }
}
/// Page input in the WebSocket binary protocol.
///
/// # Safety
/// `e` must point to a live emulator to which the caller has exclusive access. For nonzero `len`,
/// `ptr` must be non-null and readable for `len` bytes throughout this call.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_in_bin(e: *mut Emu, ptr: *const u8, len: usize) {
    // SAFETY: The caller provides exclusive access to a live emulator.
    let e = unsafe { &mut *e };
    // SAFETY: The caller provides a readable input buffer for this call.
    let input = unsafe { bytes(ptr, len) };
    if let Some(w) = e.m.web() {
        if input.len() > 8 << 20 { w.send_text(r#"{"t":"camera-error","message":"input exceeds 8 MiB"}"#); }
        else { w.push_incoming_bin(input.to_vec()); }
    }
}

/// Enable or disable the scheduler-integrated block JIT. The interpreter remains available.
///
/// # Safety
/// `e` must be a live exclusively borrowed emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_set_jit(e: *mut Emu, enabled: u32) {
    // SAFETY: The ABI caller guarantees a live exclusive handle.
    unsafe { &mut *e }.m.set_jit(enabled != 0);
}

/// Configure provisional uniform CPU cost and deadline-bounded batches before execution.
/// CPI and quantum must be nonzero; this API cannot detach the scheduler once configured.
/// # Safety
/// `e` must be a live exclusively borrowed emulator.
#[no_mangle]
pub unsafe extern "C" fn esp32sim_set_approximate_jit_timing(e: *mut Emu, cpi: u32, quantum: u32) -> u32 {
    unsafe { &mut *e }.m.approximate_jit_timing(cpi, quantum)
}

#[cfg(all(target_arch = "wasm32", feature = "jit-tests"))]
mod jit_tests;
#[cfg(all(target_arch = "wasm32", feature = "jit-tests"))]
mod jit_memory_tests;

/// EX170: sweeps the compiled MADD.S/MSUB.S sequence against the fused helper's arithmetic.
/// Returns mismatches (must be 0); logs the halfway-class count.
#[cfg(all(target_arch = "wasm32", feature = "jit-tests"))]
#[no_mangle]
pub extern "C" fn esp32sim_test_fma_sweep(seed: f64, n: u32) -> u32 {
    std::panic::set_hook(Box::new(|info| log(&format!("[fma sweep] {info}"))));
    let (bad, halfway) = xtensa_lx7::jit::tests::fma_sweep(seed as u64, n, &mut |line| log(&line));
    log(&format!("fma sweep seed={seed} n={n} mismatches={bad} halfway={halfway}"));
    bad
}

/// Runs the generated-code differential suite in a real WASM runtime.
#[cfg(all(target_arch = "wasm32", feature = "jit-tests"))]
#[no_mangle]
pub extern "C" fn esp32sim_test_block_jit() -> u32 {
    std::panic::set_hook(Box::new(|info| log(&format!("[jit test] {info}"))));
    xtensa_lx7::jit::tests::run_tests() + jit_tests::run() + jit_memory_tests::run()
}

#[cfg(all(target_arch = "wasm32", feature = "jit-tests"))]
#[path = "../../tests/gpio_waveform.rs"]
mod gpio_waveform_tests;

#[cfg(all(target_arch = "wasm32", feature = "jit-tests"))]
#[no_mangle]
pub extern "C" fn esp32sim_test_gpio_waveform() -> u32 {
    std::panic::set_hook(Box::new(|info| log(&format!("[waveform test] {info}"))));
    gpio_waveform_tests::run();
    24
}
