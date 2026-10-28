//! Native NimBLE transport. Guest allocators and mbuf operations retain packet ownership.
use crate::bus::{SocBus, FLASH_HIGH, FLASH_LOW, SRAM_HIGH, SRAM_LOW};
use emu_core::{Bus, Core};
use esp_soc::ble::peer::Session;
use riscv_rv32::Cpu;
use std::collections::HashMap;

const INIT: u32 = 16;
const TASK: u32 = 64;
const FREE_CMD: u32 = 80;
const INIT_NEXT: u32 = 112;
const TASK_NEXT: u32 = 116;
const HEAP_SIZE: u32 = 512 + 16 * 288;
const INVALID_ARG: u32 = 0x102;
const INVALID_STATE: u32 = 0x103;

#[derive(Clone, Copy)]
enum Hook {
    Init,
    InitNext,
    TaskNext,
    Enable,
    Disable,
    Deinit,
    Status,
    Release,
    Register,
    Alloc,
    Free,
    Command,
    Acl,
    PowerGet,
    PowerSet,
}
#[derive(Default)]
enum Receive {
    #[default]
    Poll,
    Retry(u8, Vec<u8>),
    Event(Vec<u8>),
    Acl(Vec<u8>),
    Appended(u32, u32),
    Delivered,
    Freed,
}

#[derive(Default)]
pub struct Ble {
    pub session: Session,
    pub hooks: Vec<u32>,
    kinds: HashMap<u32, Hook>,
    symbols: HashMap<&'static str, u32>,
    base: u32,
    init_step: u8,
    status: u32,
    heap: u32,
    callback: u32,
    task_created: bool,
    receive: Receive,
    pub(crate) original_flash: Option<(usize, Vec<u8>)>,
}

impl Ble {
    pub fn enable(&mut self, symbols: &HashMap<String, u32>, init_size: u32, idf_version: &str) -> Result<(), String> {
        let get = |name: &str| symbols.get(name).copied().ok_or_else(|| format!("BLE requires ELF symbol {name}"));
        let entry = get("esp_bt_controller_init")?;
        if !supported_idf(idf_version) { return Err(format!("C6 BLE requires esp_app_desc.idf_ver v5.5.x, got {idf_version:?}")); }
        let base = entry.checked_add(3).ok_or("BLE invalid init address")? & !3;
        if !(FLASH_LOW..FLASH_HIGH).contains(&entry)
            || entry.checked_add(init_size).is_none_or(|end| end.saturating_sub(base) < CODE.len() as u32)
        { return Err("BLE controller init has no room for the guest task trampoline".into()); }
        let mut next = Self { base, ..Self::default() };
        for (name, kind) in [
            ("esp_bt_controller_init", Hook::Init),
            ("esp_bt_controller_enable", Hook::Enable),
            ("hci_transport_host_callback_register", Hook::Register),
            ("r_ble_hci_trans_buf_alloc", Hook::Alloc),
            ("r_ble_hci_trans_buf_free", Hook::Free),
            ("ble_transport_to_ll_cmd_impl", Hook::Command),
            ("ble_transport_to_ll_acl_impl", Hook::Acl),
        ] { next.kinds.insert(get(name)?, kind); }
        for (name, kind) in [
            ("esp_bt_controller_disable", Hook::Disable),
            ("esp_bt_controller_deinit", Hook::Deinit),
            ("esp_bt_controller_get_status", Hook::Status),
            ("esp_bt_controller_mem_release", Hook::Release),
            ("esp_bt_mem_release", Hook::Release),
            ("esp_ble_tx_power_get", Hook::PowerGet),
            ("esp_ble_tx_power_set", Hook::PowerSet),
        ] { if let Some(&pc) = symbols.get(name) { next.kinds.insert(pc, kind); } }
        for name in [
            "npl_freertos_funcs_init",
            "npl_freertos_funcs_get",
            "esp_register_npl_funcs",
            "npl_freertos_mempool_init",
            "nimble_port_get_dflt_eventq",
            "npl_freertos_eventq_init",
            "calloc",
            "malloc",
            "free",
            "r_os_mempool_init",
            "r_os_mbuf_pool_init",
            "r_os_msys_register",
            "r_os_msys_get_pkthdr",
            "r_os_mbuf_append",
            "r_os_mbuf_free_chain",
            "xTaskCreatePinnedToCore",
            "vTaskDelay",
        ] { next.symbols.insert(name, get(name)?); }
        next.kinds.insert(base + INIT_NEXT, Hook::InitNext);
        next.kinds.insert(base + TASK_NEXT, Hook::TaskNext);
        next.hooks = next.kinds.keys().copied().collect();
        *self = next;
        Ok(())
    }
    pub fn reset(&mut self) {
        self.session.reset();
        self.status = 0;
        self.heap = 0;
        self.callback = 0;
        self.task_created = false;
        self.receive = Receive::Poll;
        self.init_step = 0;
    }
    pub fn command(&mut self, command: &str, cycles: u64) -> Result<(), String> {
        if self.hooks.is_empty() { return Err("BLE requires --ble and the application ELF".into()); }
        self.session.advance_to(cycles);
        let result = self.session.command(command);
        self.log(cycles);
        result
    }
    /// The devices around the station (`--ble-nearby`), replacing the ones there were.
    pub fn set_nearby(&mut self, list: Vec<esp_soc::ble::peer::Nearby>, cycles: u64) -> Result<(), String> {
        if self.hooks.is_empty() { return Err("BLE requires --ble and the application ELF".into()); }
        self.session.advance_to(cycles);
        self.session.set_nearby(list);
        self.log(cycles);
        Ok(())
    }
    fn log(&mut self, cycles: u64) {
        for line in self.session.drain_log() { eprintln!("[ble] t={:.6}s {line}", cycles as f64 / crate::periph::CPU_HZ as f64); }
    }
}

fn supported_idf(version: &str) -> bool {
    let Some(patch) = version.strip_prefix("v5.5.").or_else(|| version.strip_prefix("5.5.")) else { return false };
    let end = patch.find(|c: char| !c.is_ascii_digit()).unwrap_or(patch.len());
    end > 0 && (end == patch.len() || patch[end..].starts_with('-'))
}
fn ram(addr: u32, len: usize) -> bool { addr >= SRAM_LOW && addr.checked_add(len as u32).is_some_and(|end| end <= SRAM_HIGH) }
fn packet(bus: &mut SocBus, addr: u32, len: usize) -> Result<Vec<u8>, String> {
    if len > 260 || !ram(addr, len) { return Err("BLE invalid packet pointer or length".into()); }
    (0..len).map(|i| bus.read8_unpriced(addr + i as u32).map_err(|_| "BLE unmapped packet".into())).collect()
}
fn acl(bus: &mut SocBus, mut mbuf: u32) -> Result<Vec<u8>, String> {
    let mut bytes = vec![2];
    let mut seen = Vec::new();
    let mut total = 0;
    while mbuf != 0 {
        if !ram(mbuf, 24) || seen.contains(&mbuf) || seen.len() == 16 { return Err("BLE invalid mbuf chain".into()); }
        if seen.is_empty() {
            if bus.read8_unpriced(mbuf + 5).map_err(|_| "BLE mbuf header")? < 8 { return Err("BLE ACL requires a packet header".into()); }
            total = bus.read16_unpriced(mbuf + 16).map_err(|_| "BLE mbuf length")? as usize;
            if total > 260 { return Err("BLE ACL exceeds controller packet size".into()); }
        }
        seen.push(mbuf);
        let data = bus.read32_unpriced(mbuf).map_err(|_| "BLE mbuf data")?;
        let len = bus.read16_unpriced(mbuf + 6).map_err(|_| "BLE mbuf length")? as usize;
        if len > total.saturating_sub(bytes.len() - 1) { return Err("BLE inconsistent mbuf lengths".into()); }
        bytes.extend(packet(bus, data, len)?);
        mbuf = bus.read32_unpriced(mbuf + 12).map_err(|_| "BLE mbuf next")?;
    }
    if bytes.len() != total + 1 || total < 4 { return Err("BLE incomplete ACL packet".into()); }
    Ok(bytes)
}
fn redirect(cpu: &mut Cpu, pc: u32) {
    cpu.pc = pc;
    cpu.insn_count += 1;
    cpu.retired_count += 1;
    cpu.cycle_count += 1;
}
// The trampoline owns its guest stack frame, so callees may block or switch tasks.
fn call(cpu: &mut Cpu, bus: &mut SocBus, name: &'static str, args: &[u32]) {
    cpu.x[5] = bus.ble.symbols[name];
    cpu.x[10..10 + args.len()].copy_from_slice(args);
    redirect(cpu, cpu.x[1]);
}
fn finish(cpu: &mut Cpu, bus: &mut SocBus, result: u32) {
    cpu.x[5] = 0;
    cpu.return_from_stub(bus, result);
}
fn install(bus: &mut SocBus) -> Result<(), String> {
    let base = bus.ble.base;
    let off = bus.flash_off(base).ok_or("BLE init is not mapped to flash")?;
    if bus.flash_off(base + CODE.len() as u32 - 1) != Some(off + CODE.len() - 1) {
        return Err("BLE init crosses a noncontiguous flash mapping".into());
    }
    if bus.ble.original_flash.is_none() { bus.ble.original_flash = Some((off, bus.flash[off..off + CODE.len()].to_vec())); }
    bus.flash[off..off + CODE.len()].copy_from_slice(CODE);
    Ok(())
}
fn init_next(cpu: &mut Cpu, bus: &mut SocBus) {
    let step = bus.ble.init_step;
    let result = cpu.x[10];
    if matches!(step, 3 | 4 | 8 | 9 | 10) && result != 0 {
        eprintln!("[ble] native initialization step {step} failed: {result:#x}");
        finish(cpu, bus, result);
        return;
    }
    bus.ble.init_step += 1;
    let heap = bus.ble.heap;
    match step {
        0 => call(cpu, bus, "npl_freertos_funcs_init", &[]),
        1 => call(cpu, bus, "npl_freertos_funcs_get", &[]),
        2 if result != 0 => call(cpu, bus, "esp_register_npl_funcs", &[result]),
        3 => call(cpu, bus, "npl_freertos_mempool_init", &[]),
        4 => call(cpu, bus, "nimble_port_get_dflt_eventq", &[]),
        5 if ram(result, 4) => call(cpu, bus, "npl_freertos_eventq_init", &[result]),
        6 => call(cpu, bus, "calloc", &[1, HEAP_SIZE]),
        7 if ram(result, HEAP_SIZE as usize) && result & 3 == 0 => {
            bus.ble.heap = result;
            bus.load_bytes(result + 48, b"ble\0").unwrap();
            call(cpu, bus, "r_os_mempool_init", &[result, 16, 288, result + 512, result + 48]);
        }
        8 => {
            // ESP-IDF's r_os_msys_get_pkthdr selects pools with mp_flags bit 1 set.
            bus.load_bytes(heap + 10, &[2]).unwrap();
            call(cpu, bus, "r_os_mbuf_pool_init", &[heap + 32, heap, 288, 16]);
        }
        9 => call(cpu, bus, "r_os_msys_register", &[heap + 32]),
        10 => call(cpu, bus, "xTaskCreatePinnedToCore", &[bus.ble.base + TASK, heap + 48, 4096, 0, 23, 0, 0]),
        11 if result == 1 => {
            bus.ble.task_created = true;
            bus.ble.status = 1;
            finish(cpu, bus, 0);
        }
        _ => {
            eprintln!("[ble] native initialization step {step} returned invalid result {result:#x}");
            finish(cpu, bus, 0x101);
        }
    }
}
fn receive(cpu: &mut Cpu, bus: &mut SocBus, kind: u8, bytes: Vec<u8>) {
    if bytes.len() > 260 {
        eprintln!("[ble] native packet exceeds 260 bytes; dropping");
        bus.ble.receive = Receive::Poll;
        call(cpu, bus, "vTaskDelay", &[1]);
        return;
    }
    if kind == 4 {
        bus.ble.receive = Receive::Event(bytes);
        call(cpu, bus, "malloc", &[260]);
    } else {
        bus.ble.receive = Receive::Acl(bytes);
        call(cpu, bus, "r_os_msys_get_pkthdr", &[260, 0]);
    }
}
fn task_next(cpu: &mut Cpu, bus: &mut SocBus) {
    let state = std::mem::take(&mut bus.ble.receive);
    let result = cpu.x[10];
    if bus.ble.status != 2 || bus.ble.callback == 0 {
        let owned = match &state {
            Receive::Event(_) if ram(result, 260) => Some(("free", result)),
            Receive::Acl(_) if ram(result, 24) => Some(("r_os_mbuf_free_chain", result)),
            Receive::Appended(mbuf, _) => Some(("r_os_mbuf_free_chain", *mbuf)),
            _ => None,
        };
        if let Some((name, ptr)) = owned {
            bus.ble.receive = Receive::Freed;
            call(cpu, bus, name, &[ptr]);
        } else { call(cpu, bus, "vTaskDelay", &[1]); }
        return;
    }
    match state {
        Receive::Poll if bus.ble.status == 2 && bus.ble.callback != 0 => {
            if let Some(mut bytes) = bus.ble.session.pop_packet() {
                let kind = bytes.remove(0);
                receive(cpu, bus, kind, bytes);
                return;
            }
        }
        Receive::Event(bytes) if ram(result, 260) => {
            bus.load_bytes(result, &bytes).unwrap();
            cpu.x[5] = bus.ble.callback;
            cpu.x[10] = 4;
            cpu.x[11] = result;
            cpu.x[12] = bytes.len() as u32;
            bus.ble.receive = Receive::Delivered;
            redirect(cpu, cpu.x[1]);
            return;
        }
        Receive::Acl(bytes) if ram(result, 24) && bytes.len() <= 260 => {
            let scratch = bus.ble.heap + 64;
            bus.load_bytes(scratch, &bytes).unwrap();
            bus.ble.receive = Receive::Appended(result, bytes.len() as u32);
            call(cpu, bus, "r_os_mbuf_append", &[result, scratch, bytes.len() as u32]);
            return;
        }
        Receive::Appended(mbuf, len) => {
            if result == 0 {
                cpu.x[5] = bus.ble.callback;
                cpu.x[10] = 2;
                cpu.x[11] = mbuf;
                cpu.x[12] = len;
                bus.ble.receive = Receive::Delivered;
                redirect(cpu, cpu.x[1]);
            } else {
                eprintln!("[ble] native ACL append failed: {result}");
                bus.ble.receive = Receive::Freed;
                call(cpu, bus, "r_os_mbuf_free_chain", &[mbuf]);
            }
            return;
        }
        Receive::Retry(kind, bytes) => {
            receive(cpu, bus, kind, bytes);
            return;
        }
        Receive::Event(bytes) => {
            eprintln!("[ble] native event allocation failed; retrying");
            bus.ble.receive = Receive::Retry(4, bytes);
        }
        Receive::Acl(bytes) => {
            eprintln!("[ble] native ACL allocation failed; retrying");
            bus.ble.receive = Receive::Retry(2, bytes);
        }
        Receive::Delivered if result != 0 => { eprintln!("[ble] native receive callback failed: {result}") }
        _ => {}
    }
    // Let the host finish command processing before the next asynchronous event.
    call(cpu, bus, "vTaskDelay", &[1]);
}

pub fn intercept(cpu: &mut Cpu, bus: &mut SocBus) -> bool {
    let Some(&hook) = bus.ble.kinds.get(&cpu.pc) else {
        return false;
    };
    bus.ble.session.advance_to(bus.cycles);
    let arg = cpu.x[10];
    let mut result = 0;
    match hook {
        Hook::Init => {
            if bus.ble.status != 0 {
                result = INVALID_STATE;
            } else if !ram(arg, 4) {
                result = INVALID_ARG;
            } else if bus.ble.task_created {
                bus.ble.status = 1;
            } else if bus.ble.init_step != 0 {
                result = INVALID_STATE;
            } else if let Err(error) = install(bus) {
                eprintln!("[ble] {error}");
                result = INVALID_ARG;
            } else {
                bus.ble.init_step = 0;
                cpu.flush_caches();
                redirect(cpu, bus.ble.base + INIT);
                return true;
            }
        }
        Hook::InitNext => {
            init_next(cpu, bus);
            return true;
        }
        Hook::TaskNext => {
            task_next(cpu, bus);
            return true;
        }
        Hook::Enable => {
            if bus.ble.status != 1 {
                result = INVALID_STATE;
            } else if arg != 1 {
                result = INVALID_ARG;
            } else { bus.ble.status = 2; }
        }
        Hook::Disable => {
            if bus.ble.status != 2 {
                result = INVALID_STATE;
            } else { bus.ble.status = 1; }
        }
        Hook::Deinit => {
            if bus.ble.status != 1 {
                result = INVALID_STATE;
            } else {
                bus.ble.status = 0;
                bus.ble.callback = 0;
                bus.ble.session.reset();
            }
        }
        Hook::Status => result = bus.ble.status,
        // retain the guest task and pool until reboot; teardown needs a guest task join.
        Hook::Release => { if bus.ble.status != 0 { result = INVALID_STATE; } }
        Hook::Register => {
            if arg & 1 != 0
                || !((FLASH_LOW..FLASH_HIGH).contains(&arg) || (SRAM_LOW..SRAM_HIGH).contains(&arg))
                || bus.fetch(arg).is_err()
            {
                result = INVALID_ARG;
            } else { bus.ble.callback = arg; }
        }
        Hook::Alloc => {
            if (1..=3).contains(&arg) {
                cpu.x[10] = 260;
                redirect(cpu, bus.ble.symbols["malloc"]);
                return true;
            }
            result = 0;
        }
        Hook::Free => {
            redirect(cpu, bus.ble.symbols["free"]);
            return true;
        }
        Hook::Command | Hook::Acl => {
            let bytes = if matches!(hook, Hook::Command) {
                packet(bus, arg, 3).and_then(|header| packet(bus, arg, 3 + header[2] as usize)).map(|mut p| {
                    p.insert(0, 1);
                    p
                })
            } else {
                acl(bus, arg)
            };
            match bytes {
                Ok(bytes) if bus.ble.status == 2 => {
                    if bus.debug.has("ble") { eprintln!("[ble] tx {bytes:02x?}"); }
                    bus.ble.session.send(&bytes);
                    bus.ble.log(bus.cycles);
                }
                Err(error) => {
                    eprintln!("[ble] {error}");
                    result = INVALID_ARG;
                }
                _ => result = INVALID_STATE,
            }
            // Both transport entry points consume the caller's allocation, even on failure.
            if ram(arg, if matches!(hook, Hook::Command) { 3 } else { 24 }) {
                cpu.x[5] = bus.ble.symbols[if matches!(hook, Hook::Command) { "free" } else { "r_os_mbuf_free_chain" }];
                cpu.x[11] = result;
                redirect(cpu, bus.ble.base + FREE_CMD);
                return true;
            }
        }
        Hook::PowerGet => result = 5,
        Hook::PowerSet => {}
    }
    cpu.return_from_stub(bus, result);
    true
}

const CODE: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x13, 0x01, 0x01,
    0xff, 0x23, 0x26, 0x11, 0x00, 0xef, 0x00, 0x80, 0x05, 0x63, 0x86, 0x02, 0x00, 0xe7, 0x80, 0x02, 0x00, 0x6f, 0xf0,
    0x5f, 0xff, 0x83, 0x20, 0xc1, 0x00, 0x13, 0x01, 0x01, 0x01, 0x67, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xef, 0x00, 0x40, 0x03, 0xe7, 0x80, 0x02, 0x00, 0x6f, 0xf0, 0x9f, 0xff,
    0x00, 0x00, 0x00, 0x00, 0x13, 0x01, 0x01, 0xff, 0x23, 0x26, 0x11, 0x00, 0x23, 0x24, 0xb1, 0x00, 0xe7, 0x80, 0x02,
    0x00, 0x03, 0x25, 0x81, 0x00, 0x83, 0x20, 0xc1, 0x00, 0x13, 0x01, 0x01, 0x01, 0x67, 0x80, 0x00, 0x00, 0x67, 0x80,
    0x00, 0x00, 0x67, 0x80, 0x00, 0x00,
];

#[cfg(test)]
mod tests {
    use super::*;
    const ENTRY: u32 = FLASH_LOW + 2;
    const RETURN: u32 = FLASH_LOW + 0x1000;
    const HEAP: u32 = SRAM_LOW + 0x4000;
    const CALLBACK: u32 = FLASH_LOW + 0x1800;

    fn fixture() -> (SocBus, HashMap<String, u32>) {
        let mut bus = SocBus::new(4 << 20, [0; 6]);
        bus.mmu[0] = crate::bus::MMU_VALID;
        let names = [
            "esp_bt_controller_enable",
            "hci_transport_host_callback_register",
            "r_ble_hci_trans_buf_alloc",
            "r_ble_hci_trans_buf_free",
            "ble_transport_to_ll_cmd_impl",
            "ble_transport_to_ll_acl_impl",
            "npl_freertos_funcs_init",
            "npl_freertos_funcs_get",
            "esp_register_npl_funcs",
            "npl_freertos_mempool_init",
            "nimble_port_get_dflt_eventq",
            "npl_freertos_eventq_init",
            "calloc",
            "malloc",
            "free",
            "r_os_mempool_init",
            "r_os_mbuf_pool_init",
            "r_os_msys_register",
            "r_os_msys_get_pkthdr",
            "r_os_mbuf_append",
            "r_os_mbuf_free_chain",
            "xTaskCreatePinnedToCore",
            "vTaskDelay",
        ];
        let mut symbols: HashMap<String, u32> =
            names.into_iter().enumerate().map(|(i, name)| (name.into(), FLASH_LOW + 0x200 + i as u32 * 4)).collect();
        symbols.insert("esp_bt_controller_init".into(), ENTRY);
        bus.ble.enable(&symbols, 0x200, "v5.5.4").unwrap();
        (bus, symbols)
    }
    fn caller(pc: u32, arg: u32) -> Cpu {
        let mut cpu = Cpu::new_rv32imac();
        cpu.pc = pc;
        cpu.x[1] = RETURN;
        cpu.x[2] = SRAM_HIGH - 0x100;
        cpu.x[10] = arg;
        cpu
    }

    #[test]
    fn oversized_acl_is_dropped_before_allocation_without_retry() {
        let (mut bus, symbols) = fixture();
        bus.ble.status = 2;
        bus.ble.callback = CALLBACK;
        let mut cpu = caller(bus.ble.base + TASK_NEXT, 0);
        receive(&mut cpu, &mut bus, 2, vec![0; 261]);
        assert_eq!(cpu.x[5], symbols["vTaskDelay"]);
        assert!(matches!(bus.ble.receive, Receive::Poll));
        for _ in 0..3 {
            task_next(&mut cpu, &mut bus);
            assert_eq!(cpu.x[5], symbols["vTaskDelay"]);
            assert!(!matches!(bus.ble.receive, Receive::Retry(..)));
        }
        receive(&mut cpu, &mut bus, 2, vec![0; 260]);
        assert_eq!(cpu.x[5], symbols["r_os_msys_get_pkthdr"]);
        assert!(matches!(bus.ble.receive, Receive::Acl(_)));
    }

    #[test]
    fn missing_ble_symbols_precede_version_errors() {
        let mut ble = Ble::default();
        for version in ["", "v5.5.4"] {
            assert_eq!(ble.enable(&HashMap::new(), 0, version).unwrap_err(), "BLE requires ELF symbol esp_bt_controller_init");
        }
    }

    #[test]
    fn native_init_calls_guest_npl_allocators_and_creates_task_without_bss() {
        let (mut bus, symbols) = fixture();
        let mut cpu = caller(ENTRY, SRAM_LOW + 0x100);
        let mut calls = Vec::new();
        for _ in 0..150 {
            if cpu.pc == RETURN { break; }
            if !intercept(&mut cpu, &mut bus) {
                if let Some((name, _)) = symbols.iter().find(|(_, pc)| **pc == cpu.pc) {
                    calls.push(name.as_str());
                    let result = match name.as_str() {
                        "npl_freertos_funcs_get" | "nimble_port_get_dflt_eventq" => SRAM_LOW + 0x200,
                        "calloc" => {
                            assert_eq!(&cpu.x[10..12], &[1, HEAP_SIZE]);
                            HEAP
                        }
                        "r_os_mempool_init" => {
                            assert_eq!(&cpu.x[10..15], &[HEAP, 16, 288, HEAP + 512, HEAP + 48]);
                            0
                        }
                        "r_os_mbuf_pool_init" => {
                            assert_eq!(bus.read8_unpriced(HEAP + 10).unwrap(), 2);
                            assert_eq!(&cpu.x[10..14], &[HEAP + 32, HEAP, 288, 16]);
                            0
                        }
                        "xTaskCreatePinnedToCore" => {
                            assert_eq!(&cpu.x[10..17], &[bus.ble.base + TASK, HEAP + 48, 4096, 0, 23, 0, 0]);
                            1
                        }
                        _ => 0,
                    };
                    cpu.return_from_stub(&mut bus, result);
                } else { assert!(cpu.step(&mut bus).trap().is_none()); }
            }
        }
        assert_eq!(
            calls,
            [
                "npl_freertos_funcs_init",
                "npl_freertos_funcs_get",
                "esp_register_npl_funcs",
                "npl_freertos_mempool_init",
                "nimble_port_get_dflt_eventq",
                "npl_freertos_eventq_init",
                "calloc",
                "r_os_mempool_init",
                "r_os_mbuf_pool_init",
                "r_os_msys_register",
                "xTaskCreatePinnedToCore"
            ]
        );
        assert_eq!(cpu.pc, RETURN);
        assert_eq!(cpu.x[2], SRAM_HIGH - 0x100);
        assert_eq!(cpu.x[10], 0);
        assert!(bus.ble.task_created);
        assert_eq!(bus.ble.status, 1);
    }

    #[test]
    fn receive_task_allocates_event_transfers_ownership_and_yields() {
        let (mut bus, symbols) = fixture();
        install(&mut bus).unwrap();
        bus.ble.heap = HEAP;
        bus.ble.status = 2;
        bus.ble.callback = CALLBACK;
        bus.ble.session.send(&[1, 3, 12, 0]);
        let mut cpu = caller(bus.ble.base + TASK, 0);
        let event = SRAM_LOW + 0x8000;
        let mut delivered = false;
        let mut yielded = false;
        for _ in 0..40 {
            if cpu.pc == symbols["malloc"] {
                assert_eq!(cpu.x[10], 260);
                cpu.return_from_stub(&mut bus, event);
            } else if cpu.pc == CALLBACK {
                assert_eq!(&cpu.x[10..13], &[4, event, 6]);
                assert_eq!(packet(&mut bus, event, 6).unwrap(), [14, 4, 1, 3, 12, 0]);
                cpu.return_from_stub(&mut bus, 0);
                delivered = true;
            } else if cpu.pc == symbols["vTaskDelay"] {
                assert!(delivered);
                assert_eq!(cpu.x[10], 1);
                yielded = true;
                break;
            } else if !intercept(&mut cpu, &mut bus) { assert!(cpu.step(&mut bus).trap().is_none()); }
        }
        assert!(delivered && yielded);
        bus.ble.receive = Receive::Acl(vec![1, 0x20, 0, 0]);
        cpu = caller(bus.ble.base + TASK_NEXT, event);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[5], symbols["r_os_mbuf_append"]);
        assert_eq!(&cpu.x[10..13], &[event, HEAP + 64, 4]);
        cpu = caller(bus.ble.base + TASK_NEXT, 1);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[5], symbols["r_os_mbuf_free_chain"]);
        assert_eq!(cpu.x[10], event, "failed append must return mbuf to its guest pool");
    }

    #[test]
    fn native_tx_frees_commands_and_chained_acl_and_rejects_bad_chains() {
        let (mut bus, symbols) = fixture();
        install(&mut bus).unwrap();
        bus.ble.status = 2;
        bus.load_bytes(HEAP, &[3, 12, 0]).unwrap();
        let mut cpu = caller(symbols["ble_transport_to_ll_cmd_impl"], HEAP);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.pc, bus.ble.base + FREE_CMD);
        let mut freed = false;
        for _ in 0..12 {
            if cpu.pc == symbols["free"] {
                assert_eq!(cpu.x[10], HEAP);
                freed = true;
                cpu.return_from_stub(&mut bus, 0xdeadbeef);
            } else if cpu.pc == RETURN {
                break;
            } else { assert!(cpu.step(&mut bus).trap().is_none()); }
        }
        assert!(freed);
        assert_eq!(cpu.x[10], 0, "free's return register is not an HCI status");
        for (addr, next, data) in [(HEAP, HEAP + 32, HEAP + 64), (HEAP + 32, 0, HEAP + 66)] {
            bus.load_bytes(addr, &data.to_le_bytes()).unwrap();
            bus.load_bytes(addr + 4, &[0, 8, 2, 0]).unwrap();
            bus.load_bytes(addr + 12, &next.to_le_bytes()).unwrap();
        }
        bus.load_bytes(HEAP + 16, &4u16.to_le_bytes()).unwrap();
        bus.load_bytes(HEAP + 64, &[1, 0x20, 0, 0]).unwrap();
        assert_eq!(acl(&mut bus, HEAP).unwrap(), [2, 1, 0x20, 0, 0]);
        cpu = caller(symbols["ble_transport_to_ll_acl_impl"], HEAP);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.pc, bus.ble.base + FREE_CMD);
        assert_eq!(cpu.x[5], symbols["r_os_mbuf_free_chain"]);
        assert_eq!(cpu.x[10], HEAP);
        bus.load_bytes(HEAP + 32 + 12, &HEAP.to_le_bytes()).unwrap();
        assert!(acl(&mut bus, HEAP).unwrap_err().contains("chain"));
        assert!(packet(&mut bus, SRAM_HIGH - 1, 3).is_err());
    }

    #[test]
    fn receive_allocation_retries_and_shutdown_frees_untransferred_packets() {
        let (mut bus, symbols) = fixture();
        bus.ble.status = 2;
        bus.ble.callback = CALLBACK;
        bus.ble.receive = Receive::Event(vec![14, 4, 1, 3, 12, 0]);
        let mut cpu = caller(bus.ble.base + TASK_NEXT, 0);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[5], symbols["vTaskDelay"]);
        assert!(matches!(bus.ble.receive, Receive::Retry(4, _)));
        cpu = caller(bus.ble.base + TASK_NEXT, 0);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[5], symbols["malloc"]);
        bus.ble.status = 0;
        bus.ble.callback = 0;
        cpu = caller(bus.ble.base + TASK_NEXT, HEAP);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[5], symbols["free"]);
        assert_eq!(cpu.x[10], HEAP);
        bus.ble.receive = Receive::Appended(HEAP, 4);
        cpu = caller(bus.ble.base + TASK_NEXT, 0);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[5], symbols["r_os_mbuf_free_chain"]);
    }

    #[test]
    fn reboot_restores_physical_flash_and_keeps_hook_configuration() {
        let (mut bus, _) = fixture();
        let original = bus.flash[..256].to_vec();
        install(&mut bus).unwrap();
        assert_ne!(&bus.flash[..256], &original);
        let hooks = bus.ble.hooks.clone();
        bus.mmu[0] = crate::bus::MMU_VALID | 1;
        esp_soc::SocBus::reboot(&mut bus, [0; 6]);
        assert_eq!(&bus.flash[..256], &original);
        assert_eq!(bus.ble.hooks, hooks);
        assert_eq!(bus.ble.heap, 0);
        assert!(!bus.ble.task_created);
        assert!(bus.ble.original_flash.is_none());
    }
    #[test]
    fn native_mbuf_bounds_precede_data_reads() {
        let (mut bus, _) = fixture();
        bus.load_bytes(HEAP, &SRAM_HIGH.to_le_bytes()).unwrap();
        bus.load_bytes(HEAP + 4, &[0, 8, 5, 0]).unwrap();
        bus.load_bytes(HEAP + 16, &4u16.to_le_bytes()).unwrap();
        assert_eq!(acl(&mut bus, HEAP).unwrap_err(), "BLE inconsistent mbuf lengths");
        bus.load_bytes(HEAP + 16, &261u16.to_le_bytes()).unwrap();
        assert_eq!(acl(&mut bus, HEAP).unwrap_err(), "BLE ACL exceeds controller packet size");
    }
    #[test]
    fn native_tx_errors_free_owned_buffers_and_preserve_status() {
        let (mut bus, symbols) = fixture();
        install(&mut bus).unwrap();
        bus.load_bytes(HEAP, &[3, 12, 0]).unwrap();
        for (name, free, status, expected) in [
            ("ble_transport_to_ll_cmd_impl", "free", 0, INVALID_STATE),
            ("ble_transport_to_ll_acl_impl", "r_os_mbuf_free_chain", 2, INVALID_ARG),
        ] {
            bus.ble.status = status;
            let mut cpu = caller(symbols[name], HEAP);
            assert!(intercept(&mut cpu, &mut bus));
            let mut frees = 0;
            for _ in 0..12 {
                if cpu.pc == RETURN { break; }
                if cpu.pc == symbols[free] {
                    assert_eq!(cpu.x[10], HEAP);
                    frees += 1;
                    cpu.return_from_stub(&mut bus, 0xdeadbeef);
                } else { assert!(cpu.step(&mut bus).trap().is_none()); }
            }
            assert_eq!(frees, 1);
            assert_eq!(cpu.pc, RETURN);
            assert_eq!(cpu.x[10], expected);
            assert_eq!(cpu.x[2], SRAM_HIGH - 0x100);
        }
    }
    #[test]
    fn native_acl_callback_receives_length() {
        let (mut bus, _) = fixture();
        bus.ble.status = 2;
        bus.ble.callback = CALLBACK;
        bus.ble.receive = Receive::Appended(HEAP, 19);
        let mut cpu = caller(bus.ble.base + TASK_NEXT, 0);
        assert!(intercept(&mut cpu, &mut bus));
        assert_eq!(cpu.x[5], CALLBACK);
        assert_eq!(&cpu.x[10..13], &[2, HEAP, 19]);
    }
    #[test]
    fn native_enable_checks_version_and_symbol_extent() {
        let (mut bus, symbols) = fixture();
        for version in ["", "v4.4.7", "v5.4.1", "v5.50.0", "v5.5.", "v5.5.x", "v5.5.4garbage", "v6.0.0"] {
            assert!(bus.ble.enable(&symbols, 512, version).unwrap_err().contains("idf_ver"));
        }
        for version in ["v5.5.0", "v5.5.4", "5.5.4", "v5.5.4-12-gabcd"] {
            assert!(bus.ble.enable(&symbols, CODE.len() as u32 + 2, version).is_ok());
        }
        assert!(bus.ble.enable(&symbols, CODE.len() as u32 + 1, "v5.5.4").unwrap_err().contains("room"));
    }
}
