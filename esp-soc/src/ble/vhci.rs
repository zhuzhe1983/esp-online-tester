//! Controller lifecycle and legacy VHCI packet delivery in a guest FreeRTOS task.
use emu_core::Core;
use std::collections::HashMap;

pub const INIT: u32 = 48;
pub const TASK: u32 = 112;
pub const POLL: u32 = 176;
pub const CREATED: u32 = 192;
const BUFFER_SIZE: u32 = 512;
const INVALID_STATE: u32 = 0x103;
const INVALID_ARG: u32 = 0x102;

// Xtensa little-endian, no-transform assembly, relative to controller_init.
// Literals at +4: create, task, name, delay, poll, created, buffer, stack size.
// +48: entry a1,64; l32r a8,create; l32r a10,task; l32r a11,name;
// l32r a12,stack; movi a13,0; movi a14,23; movi a15,0; movi a9,0;
// s32i a9,a1,0; callx8 a8; l32r a8,created; callx8 a8; mov a2,a10; retw.
// +112: entry a1,48; again: l32r a8,poll; callx8 a8; beqz a10,sleep;
// mov a8,a10; l32r a10,buffer; l32i a11,a10,0; addi a10,a10,16;
// callx8 a8; j again; sleep: movi a10,1; l32r a8,delay; callx8 a8; j again.
// +176/+192: entry a1,32; retw. These two entries are host-intercepted.
pub const XTENSA_CODE: &[u8] = &[
    0x36, 0x81, 0x00, 0x81, 0xf4, 0xff, 0xa1, 0xf4, 0xff, 0xb1, 0xf4, 0xff, 0xc1, 0xf9, 0xff, 0xd2, 0xa0, 0x00, 0xe2,
    0xa0, 0x17, 0xf2, 0xa0, 0x00, 0x92, 0xa0, 0x00, 0x92, 0x61, 0x00, 0xe0, 0x08, 0x00, 0x81, 0xf1, 0xff, 0xe0, 0x08,
    0x00, 0xa0, 0x2a, 0x20, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x36, 0x61, 0x00, 0x81, 0xe8, 0xff, 0xe0, 0x08, 0x00, 0x16, 0x1a, 0x01,
    0xa0, 0x8a, 0x20, 0xa1, 0xe7, 0xff, 0xb2, 0x2a, 0x00, 0xa2, 0xca, 0x10, 0xe0, 0x08, 0x00, 0x06, 0xf9, 0xff, 0xa2,
    0xa0, 0x01, 0x81, 0xdf, 0xff, 0xe0, 0x08, 0x00, 0x06, 0xf6, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x36, 0x41, 0x00, 0x90, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x36, 0x41, 0x00, 0x90, 0x00, 0x00,
];

pub struct Abi {
    pub code: &'static [u8],
    pub ram: std::ops::Range<u32>,
    pub hz: u64,
}

pub trait VhciBus: crate::SocBus {
    fn abi() -> Abi;
    fn ble(&mut self) -> &mut Ble;
    fn valid_callback(&mut self, pc: u32) -> bool;
    fn flash_offset(&mut self, pc: u32) -> Option<usize>;
    fn ble_debug(&self) -> bool;
    fn valid_packet_range(&self, address: u32, len: usize) -> bool {
        let ram = Self::abi().ram;
        address >= ram.start && address.checked_add(len as u32).is_some_and(|end| end <= ram.end)
    }
}

#[derive(Clone, Copy)]
enum Hook {
    Init,
    Created,
    Poll,
    Enable,
    Disable,
    Deinit,
    Status,
    Release,
    Register,
    Available,
    Send,
    PowerGet,
    PowerSet,
}

#[derive(Default)]
pub struct Ble {
    pub session: crate::ble::peer::Session,
    pub hooks: Vec<u32>,
    kinds: HashMap<u32, Hook>,
    entry: u32,
    pub init: u32,
    pub create: u32,
    pub delay: u32,
    pub buffer: u32,
    pub status: u32,
    pub callbacks: [u32; 2],
    pub send_ready: bool,
    yield_to_host: bool,
    pub task_created: bool,
    pub original_flash: Option<(usize, Vec<u8>)>,
}

impl Ble {
    pub fn restore_flash(&mut self, flash: &mut [u8]) {
        super::restore_patched_flash(&mut self.original_flash, flash);
    }

    pub fn enable(&mut self, symbols: &HashMap<String, u32>, init_size: u32, abi: &Abi) -> Result<(), String> {
        let get = |name: &str| symbols.get(name).copied().ok_or_else(|| format!("BLE requires ELF symbol {name}"));
        let entry = get("esp_bt_controller_init")?;
        let init = entry.checked_add(3).ok_or("BLE invalid init address")? & !3;
        if entry.checked_add(init_size).is_none_or(|end| end.saturating_sub(init) < INIT + abi.code.len() as u32) {
            return Err("BLE controller init has no room for the guest task trampoline".into());
        }
        let buffer = get("_bt_controller_bss_start")?;
        let end = get("_bt_controller_bss_end")?;
        if !abi.ram.contains(&buffer)
            || end > abi.ram.end
            || buffer & 3 != 0
            || end.saturating_sub(buffer) < BUFFER_SIZE
        { return Err("BLE controller BSS is too small or outside DRAM".into()); }
        let mut next = Self {
            entry,
            init,
            buffer,
            create: get("xTaskCreatePinnedToCore")?,
            delay: get("vTaskDelay")?,
            ..Self::default()
        };
        for (name, kind) in [
            ("esp_bt_controller_init", Hook::Init),
            ("esp_bt_controller_enable", Hook::Enable),
            ("esp_vhci_host_register_callback", Hook::Register),
            ("esp_vhci_host_check_send_available", Hook::Available),
            ("esp_vhci_host_send_packet", Hook::Send),
        ] { next.kinds.insert(get(name)?, kind); }
        for (name, kind) in [
            ("esp_bt_controller_get_status", Hook::Status),
            ("esp_bt_controller_rom_mem_release", Hook::Release),
            ("esp_bt_controller_disable", Hook::Disable),
            ("esp_bt_controller_deinit", Hook::Deinit),
            ("esp_bt_controller_mem_release", Hook::Release),
            ("esp_bt_mem_release", Hook::Release),
            ("esp_ble_tx_power_get", Hook::PowerGet),
            ("esp_ble_tx_power_set", Hook::PowerSet),
        ] { if let Some(&address) = symbols.get(name) { next.kinds.insert(address, kind); } }
        next.kinds.insert(init + POLL, Hook::Poll);
        next.kinds.insert(init + CREATED, Hook::Created);
        next.hooks = next.kinds.keys().copied().collect();
        *self = next;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.session.reset();
        self.status = 0;
        self.callbacks = [0; 2];
        self.send_ready = false;
        self.yield_to_host = false;
        self.task_created = false;
    }

    pub fn command(&mut self, command: &str, cycles: u64, hz: u64) -> Result<(), String> {
        if self.hooks.is_empty() { return Err("BLE requires --ble and the application ELF".into()); }
        self.session.advance_to(cycles);
        let result = self.session.command(command);
        self.log(cycles, hz);
        result
    }

    /// The devices around the station (`--ble-nearby`), replacing the ones there were.
    pub fn set_nearby(&mut self, list: Vec<crate::ble::peer::Nearby>, cycles: u64, hz: u64) -> Result<(), String> {
        if self.hooks.is_empty() { return Err("BLE requires --ble and the application ELF".into()); }
        self.session.advance_to(cycles);
        self.session.set_nearby(list);
        self.log(cycles, hz);
        Ok(())
    }

    fn log(&mut self, cycles: u64, hz: u64) {
        for line in self.session.drain_log() { eprintln!("[ble] t={:.6}s {line}", cycles as f64 / hz as f64); }
    }
}

pub fn install<B: VhciBus>(bus: &mut B) -> Result<(), String> {
    let abi = B::abi();
    let b = bus.ble();
    let (entry, init, buffer) = (b.entry, b.init, b.buffer);
    let literals = [b.create, init + TASK, buffer + 4, b.delay, init + POLL, init + CREATED, buffer, 4096];
    if !bus.valid_callback(entry) { return Err("BLE controller init does not match the guest calling convention".into()); }
    if let Some(offset) = bus.flash_offset(init + 4) {
        let len = INIT as usize + abi.code.len() - 4;
        if bus.flash_offset(init + 4 + len as u32 - 1) != Some(offset + len - 1) {
            return Err("BLE controller init crosses a noncontiguous flash mapping".into());
        }
        if bus.ble().original_flash.is_none() {
            let original = (0..len)
                .map(|i| bus.read8_unpriced(init + 4 + i as u32))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "BLE cannot read trampoline memory")?;
            bus.ble().original_flash = Some((offset, original));
        }
    }
    for (i, value) in literals.into_iter().enumerate() { bus.load_bytes(init + 4 + i as u32 * 4, &value.to_le_bytes())?; }
    bus.load_bytes(init + INIT, abi.code)?;
    bus.load_bytes(buffer + 4, b"vhci\0")?;
    Ok(())
}

pub fn intercept<C: Core, B: VhciBus>(cpu: &mut C, bus: &mut B, redirect: impl FnOnce(&mut C, u32)) -> bool {
    let abi = B::abi();
    let Some(&hook) = bus.ble().kinds.get(&cpu.pc()) else {
        return false;
    };
    let cycles = bus.cycles();
    bus.ble().session.advance_to(cycles);
    let arg = cpu.arg(bus, 0);
    let mut result = 0;
    match hook {
        Hook::Init => {
            if bus.ble().status != 0 {
                result = INVALID_STATE;
            } else if arg == 0 {
                result = INVALID_ARG;
            } else if bus.ble().task_created {
                bus.ble().status = 1;
            } else {
                match install(bus) {
                    Ok(()) => {
                        cpu.flush_caches();
                        redirect(cpu, bus.ble().init + INIT);
                        return true;
                    }
                    Err(error) => {
                        eprintln!("[ble] {error}");
                        result = u32::MAX;
                    }
                }
            }
        }
        Hook::Created => {
            if arg == 1 {
                bus.ble().task_created = true;
                bus.ble().status = 1;
            } else {
                eprintln!("[ble] controller task creation failed: {arg:#x}");
                result = u32::MAX;
            }
        }
        Hook::Enable => {
            if bus.ble().status != 1 {
                result = INVALID_STATE;
            } else if arg != 1 && arg != 3 {
                result = INVALID_ARG;
            } else { bus.ble().status = 2; }
        }
        Hook::Disable => {
            if bus.ble().status != 2 {
                result = INVALID_STATE;
            } else { bus.ble().status = 1; }
        }
        Hook::Deinit => {
            if bus.ble().status != 1 {
                result = INVALID_STATE;
            } else {
                bus.ble().status = 0;
                bus.ble().callbacks = [0; 2];
                bus.ble().send_ready = false;
                bus.ble().yield_to_host = false;
                // keep one task per boot; delete it when memory release is modeled.
                bus.ble().session.reset();
            }
        }
        Hook::Status => result = bus.ble().status,
        // The injected task and packet buffer occupy controller memory for this boot.
        Hook::Release => { if bus.ble().status != 0 { result = INVALID_STATE; } }
        Hook::Register => {
            if arg == 0 || arg > u32::MAX - 7 {
                result = INVALID_ARG;
            } else if let (Ok(send), Ok(recv)) = (bus.read32_unpriced(arg), bus.read32_unpriced(arg + 4)) {
                if !bus.valid_callback(send) || !bus.valid_callback(recv) {
                    result = INVALID_ARG;
                } else {
                    bus.ble().callbacks = [send, recv];
                    bus.ble().send_ready = true;
                }
            } else { result = INVALID_ARG; }
        }
        Hook::Available => result = u32::from(bus.ble().status == 2),
        Hook::Send => {
            let len = cpu.arg(bus, 1) as usize;
            if bus.ble().status != 2
                || len == 0
                || len > (BUFFER_SIZE - 16) as usize
                || !bus.valid_packet_range(arg, len)
            {
                eprintln!("[ble] rejected invalid VHCI packet pointer, length or controller state");
            } else {
                let packet: Result<Vec<u8>, _> = (0..len).map(|i| bus.read8_unpriced(arg + i as u32)).collect();
                if let Ok(packet) = packet {
                    if bus.ble_debug() { eprintln!("[ble] tx {:02x?}", packet); }
                    bus.ble().session.send(&packet);
                    bus.ble().send_ready = true;
                    let cycles = bus.cycles();
                    bus.ble().log(cycles, abi.hz);
                }
            }
        }
        Hook::Poll => {
            // A command-complete callback wakes the host. Let it finish the operation
            // before delivering the next event, especially scan-enable followed by reports.
            if std::mem::take(&mut bus.ble().yield_to_host) {
                cpu.return_from_stub(bus, 0);
                return true;
            }
            if bus.ble().status == 2 && bus.ble().callbacks[1] != 0 {
                if let Some(packet) = bus.ble().session.pop_packet() {
                    let buffer = bus.ble().buffer;
                    if packet.len() <= (BUFFER_SIZE - 16) as usize
                        && bus.load_bytes(buffer, &(packet.len() as u32).to_le_bytes()).is_ok()
                        && bus.load_bytes(buffer + 16, &packet).is_ok()
                    {
                        if bus.ble_debug() { eprintln!("[ble] rx {:02x?}", packet); }
                        bus.ble().yield_to_host = true;
                        result = bus.ble().callbacks[1];
                    } else { eprintln!("[ble] rejected oversized or unmapped controller packet"); }
                } else if bus.ble().send_ready {
                    bus.ble().send_ready = false;
                    result = bus.ble().callbacks[0];
                }
            }
        }
        Hook::PowerGet => result = 5,
        Hook::PowerSet => {}
    }
    cpu.return_from_stub(bus, result);
    true
}
