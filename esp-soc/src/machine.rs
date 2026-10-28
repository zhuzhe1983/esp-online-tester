//! `Machine<S>`: the cores and the bus of a `Soc`, scheduled together with device time, plus
//! everything around a run that is the same for every chip — console capture, action scripts,
//! function stubs and probes, tracing and watchpoints, the web UI protocol, real-time pacing,
//! image loading, reboot.
use crate::observe::{Ctx, Observer, Wants};
use crate::soc::{CoreState, RunUntil, Soc, SocBus, Stop};
use crate::web::WebServer;
use crate::{elf, png};
use emu_core::core::pc_bit;
use emu_core::{Bus, Core, CostModel, LifecycleFacts, LifecycleKind, MemoryAccess, Trap};
use std::collections::{BTreeMap, HashMap};

mod modeled;
mod web;

#[derive(Clone, Debug)]
pub enum ScriptAction { Gpio(u8, bool), Serial(String), Uart(usize, String), Stop, Touch(u16, u16, bool), Poke(u32, u32), TouchPad(u8, bool), Ble(String), Analog(u8, esp_periph::AnalogSource),
    /// `waituart0 <timeout_s> <text>`: hold the rest of the script until UART0 prints `text` (or the
    /// timeout passes), then shift every later action by the time actually waited. Fields: text,
    /// timeout in cycles, the cycle the script placed it at.
    WaitUart0(String, u64, u64) }

/// The stop conditions that are not observers.
pub struct Debug { pub stop_on_unimplemented: bool, pub stop_after_exceptions: u64 }

/// Guest console output: everything ever printed (`all`), the per-source backlogs the web UI
/// replays to a late client, and what goes to stdout.
pub struct Console {
    pub all: Vec<u8>,
    pub usb: Vec<u8>,
    pub uart0: Vec<u8>,
    /// which consoles to mirror to stdout: bit0 = USB-CDC, bit1 = UART0, bit2 = UART1/2
    pub mask: u32,
    pub prefix: bool,
    /// keep the text instead of printing it (the WebAssembly build has no stdout worth writing to)
    pub capture: bool,
}

/// Host actions scheduled at emulated times (`--script`, and the UI's encoder detents).
pub struct Script { pub events: Vec<(u64, ScriptAction)>, pub pos: usize, pub log: bool, knob_next: u64,
                   /// UART0 text seen so far (capped) and where the pending wait started looking
                   uart0_seen: Vec<u8>, wait_mark: Option<usize> }

pub struct Realtime {
    pub enabled: bool,
    wall_start: Option<std::time::Instant>,
    last_check: u64,
    pub behind: f64,
    pub resyncs: u64,
    /// Emulated seconds per wall second over the last second or more; `None` until measured.
    /// Unlike `behind`, a resynchronisation does not reset it, so it shows a run that cannot keep up.
    pub speed: Option<f64>,
    speed_mark: Option<(std::time::Instant, u64)>,
    pub log: bool,
    log_last: Option<std::time::Instant>,
    log_insns: (u64, u64),
}

struct WebState { last_push_cycles: u64, /// EX170: `CPU_HZ / display_push_hz()` as of the last evaluation (0 = not evaluated yet); boards are fixed once booted.
    push_interval: u64, audio_sent: usize, ring_updates: u64, grid_updates: Vec<u64>, px_pending: u64, px_sent: u64, px_deferred: bool, cam_pushed: u64, cam_sent: bool }

pub struct Machine<S: Soc> {
    pub mac: [u8; 6],
    pub reboots: u64,
    /// function stubs: at this pc (a function entry), return `value` immediately
    pub stubs: HashMap<u32, u32>,
    /// one bit per pc bucket for `stubs` / `fn_probes`, so the common case costs a shift and a test
    /// instead of hashing every pc (a hash lookup per instruction cost ~16% of run time)
    stub_bloom: u64, probe_bloom: u64,
    pub stub_hits: u64,
    /// function-entry tracing: pc -> name (`--trace-fn PREFIX`)
    pub fn_probes: HashMap<u32, String>,
    pub cores: Vec<S::Core>,
    /// a secondary core held in reset by its SoC registers (reset when released)
    core_held: Vec<bool>,
    /// Instructions each busy core runs per scheduling round (`QUANTUM`: 64 native, 256 on wasm32; the goldens
    /// and pinned totals hold for these); another value changes the interleaving of two busy cores (EX047).
    pub quantum: u64,
    pub bus: S::Bus,
    pub symbols: BTreeMap<u32, String>,
    pub dbg: Debug,
    /// analyses watching the run (`add_observer`); `probes` is the union of what they want
    pub observers: Vec<Box<dyn Observer<S>>>,
    probes: Wants,
    prev_irq: Vec<u32>,
    pub exceptions: u64,
    pub interrupts: u64,
    pub irq_hist: Vec<[u64; 32]>,
    pub script: Script,
    pub max_cycles: u64,
    pub console: Console,
    /// live web UI
    pub web: Option<WebServer>,
    pub camera_input: Option<std::sync::Arc<std::sync::Mutex<Option<crate::picture::Picture>>>>,
    /// The page's Restart (`reset` on the WebSocket) is honoured: the front-end sets this when it
    /// can bring the machine back up after the reset. Otherwise the message is ignored, so a run
    /// that stops at a chip reset is not ended from the page.
    pub web_restart: bool,
    button_reset: bool,
    ws: WebState,
    pub rt: Realtime,
    debug_rom: bool,
    cost: Option<Box<dyn CostModel>>,
    model_accesses: Vec<MemoryAccess>,
    approximate_jit_timing: Option<(u32, u32)>,
    approximate_jit_frontiers: bool,
    model_ready_at: Vec<u64>,
    model_stop: Option<Stop>,
    model_attach_error: Option<&'static str>,
    /// EX133 virtual quanta: most scheduling quanta one core may run in a single budget while
    /// every other core idles (1 = off). Bit-exact with the per-quantum schedule by construction.
    /// Keep optional scheduling state at the tail to preserve the hot fields' offsets.
    pub vq_max: u64,
    /// EX133 counters: multi-quantum runs, quanta they covered, runs stopped at a device register, at waiti.
    pub vq_stats: [u64; 4],
    /// Backoff after runs cut short by frequent device-register accesses.
    vq_skip: u32, vq_penalty: u32,
    /// EX177 both-busy round batching: most whole scheduling rounds the loop may run in one batch
    /// while every enabled core is busy (1 = off). The per-core quanta, their order and their
    /// budgets are unchanged, so the interleaving is the per-round schedule's; only the round
    /// bookkeeping is hoisted out and folded, and the bound keeps every boundary it folds free of
    /// work.
    pub bb_max: u64,
    /// EX177 counters: batches, whole rounds they covered, batches stopped at a device register,
    /// at waiti, batches that ran the whole grant, granted rounds, batches the bound refused, reserved.
    pub bb_stats: [u64; 8],
    run_steps: u64,
}

/// Default scheduling quantum; `Machine::quantum` can change it (not bit-exact with the default). q256: 256 on wasm32,
/// 64 native (M3 CLI at 256: Pocket Tank 5.4% slower, cheap native quantum switches lose to +15% spin-waiting).
const QUANTUM: u64 = if cfg!(target_arch = "wasm32") { 256 } else { 64 };
const GPIO_WAVEFORM_TIMING_ERROR: &str = "GPIO waveform decoding cannot be combined with a timing model";
/// EX133 default for `Machine::vq_max`; a build can pin another with `ESP32SIM_VQ_BUILD=<n>`.
const VQ_DEFAULT: u64 = match option_env!("ESP32SIM_VQ_BUILD") {
    Some(s) => { let b = s.as_bytes(); let (mut i, mut v) = (0, 0u64); while i < b.len() { v = v * 10 + (b[i] - b'0') as u64; i += 1; } v }
    None => if cfg!(target_arch = "wasm32") { 1024 } else { 1 },
};
/// EX177 default for `Machine::bb_max`; a build pins another with `ESP32SIM_BB_BUILD=<n>`.
/// Enabled on wasm32; only a bus that honors deferral can batch rounds at all.
const BB_DEFAULT: u64 = match option_env!("ESP32SIM_BB_BUILD") {
    Some(s) => { let b = s.as_bytes(); let (mut i, mut v) = (0, 0u64); while i < b.len() { v = v * 10 + (b[i] - b'0') as u64; i += 1; } v }
    None => if cfg!(target_arch = "wasm32") { 128 } else { 1 },
};

impl<S: Soc> Machine<S> {
    pub fn new(mac: [u8; 6], bus: S::Bus) -> Self {
        Machine {
            web_restart: false, button_reset: false, mac, reboots: 0, stubs: HashMap::new(), stub_bloom: 0, probe_bloom: 0, stub_hits: 0, fn_probes: HashMap::new(),
            cores: (0..S::CORES).map(|i| S::new_core_with_bus(i, &bus)).collect(), core_held: (0..S::CORES).map(|i| i > 0).collect(), quantum: QUANTUM, run_steps: 0, vq_stats: [0; 4], vq_skip: 0, vq_penalty: 0, vq_max: std::env::var("ESP32SIM_VQ").ok().and_then(|v| v.parse().ok()).unwrap_or(VQ_DEFAULT),
            bb_max: std::env::var("ESP32SIM_BB").ok().and_then(|v| v.parse().ok()).unwrap_or(BB_DEFAULT).min(4096), bb_stats: [0; 8],
            bus, symbols: BTreeMap::new(),
            dbg: Debug { stop_on_unimplemented: true, stop_after_exceptions: u64::MAX },
            observers: Vec::new(), probes: Wants::NONE, prev_irq: vec![0; S::CORES],
            exceptions: 0, interrupts: 0, irq_hist: vec![[0; 32]; S::CORES],
            script: Script { events: Vec::new(), pos: 0, log: true, knob_next: 0, uart0_seen: Vec::new(), wait_mark: None }, max_cycles: u64::MAX,
            console: Console { all: Vec::new(), usb: Vec::new(), uart0: Vec::new(), mask: 3, prefix: false, capture: false },
            camera_input: None, web: None, ws: WebState { last_push_cycles: 0, push_interval: 0, audio_sent: 0, ring_updates: 0, grid_updates: Vec::new(), px_pending: 0, px_sent: 0, px_deferred: false, cam_pushed: u64::MAX, cam_sent: false },
            rt: Realtime { enabled: false, wall_start: None, last_check: 0, behind: 0.0, resyncs: 0, speed: None, speed_mark: None, log: false, log_last: None, log_insns: (0, 0) },
            debug_rom: false, cost: None, model_accesses: Vec::new(), approximate_jit_timing: None, approximate_jit_frontiers: false, model_ready_at: vec![0; S::CORES], model_stop: None, model_attach_error: None,
        }
    }

    /// Which parts of the model print what they do (`--debug`, `ESP_EMU_DEBUG`).
    pub fn set_debug(&mut self, f: &crate::debug::DebugFlags) { self.rt.log = f.has("rt"); self.debug_rom = f.has("rom"); self.bus.set_debug(f); }
    pub fn seconds(&self) -> f64 { self.bus.cycles() as f64 / S::CPU_HZ as f64 }
    pub fn insns(&self) -> u64 { self.cores.iter().map(|c| c.insn_count()).sum() }
    /// Scheduling steps consumed by `run`, including idle skips, retained across reboots.
    /// This is the budget unit accepted by `run`, not the sum of retired core instructions.
    pub fn run_steps(&self) -> u64 { self.run_steps }
    /// Whether the chip reset that just stopped the run was the board's reset button (the page's
    /// Restart), not the firmware's doing. Reading it clears it.
    pub fn take_button_reset(&mut self) -> bool { std::mem::take(&mut self.button_reset) }

    // ------------------------------------------------------------------ observers
    pub fn add_observer(&mut self, o: Box<dyn Observer<S>>) {
        self.probes = self.probes | o.wants();
        self.observers.push(o);
        self.bus.misc().mmio_log = if self.probes.contains(Wants::MMIO) { Some(Vec::new()) } else { None };
        self.bus.observe_gpio(self.probes.contains(Wants::GPIO));
    }
    /// Attach a timing model before the machine has executed or reset.
    pub fn set_cost_model(&mut self, mut model: Box<dyn CostModel>) -> Result<(), String> {
        if self.bus.board_ref().uses_gpio_waveform() { return Err(GPIO_WAVEFORM_TIMING_ERROR.into()); }
        if self.cost.is_some() || self.approximate_jit_timing.is_some() { return Err("a timing model is already attached".into()); }
        if let Some(reason) = self.model_attach_error { return Err(reason.into()); }
        if self.bus.cycles() != 0 || self.reboots != 0 || self.cores.iter().any(|core| core.insn_count() != 0) {
            return Err("cost model attachment requires a pristine machine with no execution or reset".into());
        }
        model.lifecycle(&LifecycleFacts { kind: LifecycleKind::Attach, chip: S::NAME, cores: S::CORES, cpu_hz: S::CPU_HZ })?;
        self.model_ready_at.fill(0);
        self.cost = Some(model);
        Ok(())
    }
    /// Let cores resume independently after a priced block, instead of waiting for both batches.
    /// Instructions inside each block still execute together, so ordering remains approximate.
    pub fn set_approximate_jit_frontiers(&mut self, enabled: bool) -> Result<(), String> {
        if self.approximate_jit_timing.is_none() || self.insns() != 0 { return Err("configure JIT timing before frontiers and before execution".into()); }
        self.approximate_jit_frontiers = enabled;
        self.model_ready_at.fill(self.bus.cycles());
        Ok(())
    }
    /// Whether the approximate scheduler owns instruction and memory prices.
    pub fn has_approximate_jit_timing(&self) -> bool { self.approximate_jit_timing.is_some() }
    /// Whether either timing model owns scheduling.
    pub fn has_timing_model(&self) -> bool { self.cost.is_some() || self.approximate_jit_timing.is_some() }
    /// Uniform CPI and deadline-bounded instruction batches for an explicitly rough JIT experiment.
    /// Within-batch memory ordering and memory latency are not modeled by this setting.
    pub fn set_approximate_jit_timing(&mut self, cpi: u32, quantum: u32) -> Result<(), String> {
        if self.bus.board_ref().uses_gpio_waveform() { return Err(GPIO_WAVEFORM_TIMING_ERROR.into()); }
        if self.cost.is_some() || self.insns() != 0 { return Err("configure approximate JIT timing before execution, without CostModel".into()); }
        if !(1..=256).contains(&cpi) || !(1..=4096).contains(&quantum) { return Err("CPI must be 1..256 and quantum 1..4096".into()); }
        self.approximate_jit_timing = Some((cpi, quantum));
        for core in &mut self.cores { core.set_approximate_cpi(cpi); }
        Ok(())
    }
    pub fn has_observer(&self, name: &str) -> bool { self.observers.iter().any(|o| o.name() == name) }
    /// Every observer's end-of-run report, in the order they were added (files are written now).
    pub fn reports(&mut self) -> String {
        let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
        self.observers.iter_mut().map(|o| o.report(&cx)).filter(|r| !r.is_empty()).collect::<Vec<_>>().join("\n")
    }
    /// Deliver the MMIO and GPIO events the bus recorded since the last call.
    fn deliver_events(&mut self) {
        if self.probes.contains(Wants::MMIO) {
            let log = self.bus.misc().mmio_log.as_mut().map(std::mem::take).unwrap_or_default();
            if !log.is_empty() { let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ }; for o in &mut self.observers { if o.wants().contains(Wants::MMIO) { for &(pc, a, v, w) in &log { o.on_mmio(&cx, pc, a, v, w); } } } }
        }
        if self.probes.contains(Wants::GPIO) {
            let ev = self.bus.take_gpio_events();
            if !ev.is_empty() { for o in &mut self.observers { if o.wants().contains(Wants::GPIO) { for &(c, p, l) in &ev { o.on_gpio(c, p, l); } } } }
        }
    }

    // ------------------------------------------------------------------ images
    pub fn load_rom(&mut self, rom_elf: &[u8]) -> Result<(), String> {
        let e = elf::parse(rom_elf)?;
        for s in &e.segments {
            if s.data.is_empty() { continue; }
            self.bus.load_bytes(s.vaddr, &s.data)?;
            // the mask ROM also holds the initialiser image at paddr (copied by the reset handler)
            if s.paddr != s.vaddr { let _ = self.bus.load_bytes(s.paddr, &s.data); }
        }
        // RAM initialisers live in sections without program headers (.data.interface.*, .data_*)
        let dbg = self.debug_rom;
        if dbg { eprintln!("[emu] rom: {} segments, {} alloc sections", e.segments.len(), e.sections.len()); }
        for s in &e.sections {
            if dbg { eprintln!("[emu]   section {:<36} addr {:#010x} len {:#x} bss={}", s.name, s.addr, s.data.len(), s.is_bss); }
            if s.is_bss || s.data.is_empty() { continue; }
            if let Err(err) = self.bus.load_bytes(s.addr, &s.data) { eprintln!("[emu] rom section {} @ {:#x}: {}", s.name, s.addr, err); }
        }
        // The reset handler copies RAM initialisers from ROM using a 16-byte-entry table
        // (dst_start, dst_end, rom_src, 0) between _data_start and _data_end. The ELF does
        // not carry the ROM-side copies for the W-only sections, so back-fill them from the
        // RAM contents we just loaded.
        let find = |name: &str| e.by_name.get(name).copied();
        let start = S::ROM_DATA_TABLE.iter().find_map(|n| find(n));
        let end = S::ROM_DATA_TABLE_END.iter().find_map(|n| find(n));
        if let (Some(ds), Some(de)) = (start, end) {
            let mut t = ds; let mut n = 0;
            while t + S::ROM_DATA_TABLE_STRIDE <= de {
                let (Ok(d0), Ok(d1), Ok(src)) = (self.bus.read32_unpriced(t), self.bus.read32_unpriced(t + 4), self.bus.read32_unpriced(t + 8)) else { break };
                if d1 > d0 && d1 - d0 < 0x20000 {
                    let bytes: Vec<u8> = (d0..d1).map(|a| self.bus.read8_unpriced(a).unwrap_or(0)).collect();
                    if self.bus.load_bytes(src, &bytes).is_ok() { n += 1; }
                }
                t += S::ROM_DATA_TABLE_STRIDE;
            }
            if dbg { eprintln!("[emu] rom: back-filled {} initialiser blocks into ROM from table {:#x}..{:#x}", n, ds, de); }
        }
        // C3 rev3 btdm_controller_rom_data_init (0x40002d20) copies this
        // initializer through a ROM pointer, outside the reset-handler table.
        // The ELF contains .data_btdm at its RAM address but omits that ROM copy.
        if S::NAME == "esp32c3" {
            if let (Some(pointer), Some(section)) = (find("_data_start_btdm_rom"), e.sections.iter().find(|s| s.name == ".data_btdm")) {
                let source = self.bus.read32_unpriced(pointer).map_err(|e| format!("BT initializer pointer: {e:?}"))?;
                self.bus.load_bytes(source, &section.data)?;
            }
        }
        self.symbols.extend(e.symbols);
        Ok(())
    }

    pub fn add_symbols(&mut self, elf_bytes: &[u8]) -> Result<(), String> {
        self.symbols.extend(elf::parse(elf_bytes)?.symbols);
        Ok(())
    }

    pub fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> { self.bus.write_flash(offset, data) }

    /// Boot the application image at flash `app_off` the way the 2nd-stage bootloader would.
    pub fn boot_app(&mut self, app_off: usize) -> Result<u32, String> {
        if self.cost.is_some() { return Err("synthetic app boot is unsupported with a cost model; boot from the reset vector".into()); }
        self.model_attach_error = Some("cost model attachment after synthetic app boot is unsupported without a configuration snapshot");
        let entry = self.bus.boot_app(app_off)?;
        S::boot_core(&mut self.cores[0], entry);
        Ok(entry)
    }

    /// Cold boot from the mask ROM reset vector (needs ROM + flash image with bootloader).
    pub fn boot_rom(&mut self) { S::reset_core(&mut self.cores[0], 0); }

    /// Chip reset (software / watchdog): cores back to the reset vector, digital peripherals
    /// re-initialised; SRAM, RTC memories, efuses and the RTC-domain registers survive, as on
    /// silicon. Returns the reset cause that the ROM will report.
    pub fn reboot(&mut self) -> u32 {
        // where core 0 was when the reset took effect (the C6 ROM prints it as `Saved PC`)
        let pc = self.cores[0].pc();
        self.bus.note_pc(pc);
        let cause = self.bus.reboot(self.mac);
        for (i, c) in self.cores.iter_mut().enumerate() { S::reset_core_with_bus(c, &mut self.bus, i); if i > 0 { self.core_held[i] = true; } }
        self.reboots += 1;
        self.model_ready_at.fill(self.bus.cycles());
        if let Some(model) = &mut self.cost {
            let facts = LifecycleFacts { kind: LifecycleKind::ChipReset, chip: S::NAME, cores: S::CORES, cpu_hz: S::CPU_HZ };
            if let Err(reason) = model.lifecycle(&facts) {
                self.model_stop = Some(Stop::CostModelLifecycle { kind: facts.kind, reason });
            }
        }
        cause
    }

    /// Address of a symbol loaded from the ELFs.
    pub fn sym_addr(&self, name: &str) -> Option<u32> { self.symbols.iter().find(|(_, n)| n.as_str() == name).map(|(&a, _)| a) }

    pub fn sym(&self, addr: u32) -> String {
        match self.symbols.range(..=addr).next_back() {
            Some((&a, n)) if addr - a < 0x10000 => if a == addr { n.clone() } else { format!("{}+{:#x}", n, addr - a) },
            _ => String::new(),
        }
    }

    // ------------------------------------------------------------------ console
    pub fn drain_console(&mut self) {
        use std::io::Write;
        let streams = self.bus.console_take();
        let mut o = std::io::stdout();
        let (mask, prefix, capture) = (self.console.mask, self.console.prefix, self.console.capture);
        let mut emit = |bit: u32, tag: &str, d: Vec<u8>, all: &mut Vec<u8>| {
            if d.is_empty() { return; }
            all.extend_from_slice(&d);
            if mask & bit == 0 || capture { return; }
            if prefix { for line in d.split_inclusive(|&b| b == b'\n') { let _ = o.write_all(tag.as_bytes()); let _ = o.write_all(line); } } else { let _ = o.write_all(&d); }
            let _ = o.flush();
        };
        for (i, d) in streams.into_iter().enumerate() {
            let src = ["usb", "uart0", "uart1", "uart2"][i];
            if i < 2 {
                if i == 1 && !d.is_empty() {
                    let seen = &mut self.script.uart0_seen; seen.extend_from_slice(&d);
                    if seen.len() > 1 << 20 { let cut = seen.len() - (1 << 19); seen.drain(..cut); self.script.wait_mark = self.script.wait_mark.map(|m| m.saturating_sub(cut)); }
                }
                let backlog = if i == 0 { &mut self.console.usb } else { &mut self.console.uart0 };
                backlog.extend_from_slice(&d);
                if backlog.len() > 65536 { let cut = backlog.len() - 49152; backlog.drain(..cut); }
            }
            if let Some(w) = &self.web { if !d.is_empty() { w.send_text(&format!("{{\"t\":\"serial\",\"src\":\"{}\",\"data\":\"{}\"}}", src, crate::web::json_escape(&String::from_utf8_lossy(&d)))); } }
            let (bit, tag) = [(1, "[usb]  "), (2, "[uart0] "), (4, "[uart1] "), (4, "[uart2] ")][i];
            emit(bit, tag, d, &mut self.console.all);
        }
    }

    // ------------------------------------------------------------------ interrupts
    /// After a device change: re-derive the lines and present them to every core.
    #[inline]
    fn refresh_irq(&mut self) {
        if !*self.bus.irq_dirty() { return; }
        *self.bus.irq_dirty() = false;
        if self.bus.refresh_irq() { self.present_irqs(); }
        S::peripheral_event(&self.bus, &mut self.cores);
    }
    fn present_irqs(&mut self) {
        let mut irqs = [<S::Core as Core>::Irq::default(); 4];
        S::irqs(&self.bus, &mut irqs[..S::CORES]);
        for (i, c) in self.cores.iter_mut().enumerate() { c.set_irq(irqs[i]); }
        if self.probes.contains(Wants::IRQ) {
            let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
            for (i, irq) in irqs.iter().enumerate().take(S::CORES) {
                let now = S::Core::irq_bits(irq);
                let mut rising = now & !self.prev_irq[i];
                self.prev_irq[i] = now;
                while rising != 0 { let line = rising.trailing_zeros(); rising &= rising - 1; for o in &mut self.observers { if o.wants().contains(Wants::IRQ) { o.on_irq_raised(&cx, i, line); } } }
            }
        }
    }

    // ------------------------------------------------------------------ execution
    /// Execute up to `budget` instructions on `core` the fast way (blocks, JIT). Returns the
    /// iterations consumed (as `step_core` would have counted them) and a stop, if any.
    #[cfg_attr(all(target_arch = "wasm32", feature = "wasm-cpu-profile"), inline(never))]
    #[cfg_attr(all(target_arch = "wasm32", not(feature = "wasm-cpu-profile")), inline)]
    #[cfg_attr(not(target_arch = "wasm32"), inline(always))]
    fn step_blocks(&mut self, core: usize, budget: u32) -> (u32, Option<Stop>) {
        // A primary clock gate can be entered by an MMIO store inside this round.
        // Consume its remaining slots without retiring instructions or CCOUNT.
        // The first test folds away for chips whose primary is always Running.
        if S::core_state(&self.bus, 0) == CoreState::Held && S::core_state(&self.bus, core) == CoreState::Held { return (budget, None); }
        // Core::run returns a trap without its faulting PC. One-instruction fragments make
        // the entry PC exact while retaining callbacks for combined BLOCK/TRAP observers.
        let budget = if self.probes.contains(Wants::TRAP_PC) { budget.min(1) } else { budget };
        S::select_core(&mut self.bus, core);
        let cpu = &mut self.cores[core];
        let pc = cpu.pc();
        // stubs and probes are block boundaries, so testing them at block start is exact
        if (self.stub_bloom | self.probe_bloom) & pc_bit(pc) != 0 && !cpu.waiting() {
            if let Some(name) = self.fn_probes.get(&pc) {
                let (args, ret) = (cpu.probe_args(&mut self.bus), cpu.return_address(&mut self.bus));
                eprintln!("[fn] i={} t={:.4}s c{} {}({}) ret={:#x}", cpu.insn_count(), self.bus.cycles() as f64 / S::CPU_HZ as f64, core, name, args, ret);
            }
            if S::function_hook(cpu, &mut self.bus) { return (1, None); }
            if let Some(&ret) = self.stubs.get(&pc) { cpu.return_from_stub(&mut self.bus, ret); self.stub_hits += 1; return (1, None); }
        }
        let (used, trap) = cpu.run(&mut self.bus, budget);
        self.finish_step(core, pc, used, trap)
    }

    /// The completion of a dispatch at `pc` that retired `used` iterations: observers, trap counts,
    /// interrupt lines, the exception stop.
    #[inline(always)]
    fn finish_step(&mut self, core: usize, pc: u32, used: u32, trap: Option<Trap>) -> (u32, Option<Stop>) {
        if let Some(stop) = self.observe_execution(core, pc, used, trap) { return (used, Some(stop)); }
        self.refresh_irq();
        if self.exceptions >= self.dbg.stop_after_exceptions { return (used, Some(Stop::Exceptions(self.exceptions))); }
        (used, None)
    }

    /// Common completion boundary for blocks and individual instructions. Keeping block
    /// callbacks here makes BLOCK observers compose with observers that force single-stepping.
    /// `pc` is the dispatch-start address, including for trap and Simcall reporting;
    /// the WASM wrapper may have chained through later blocks before it returns.
    #[cfg_attr(target_arch = "wasm32", inline)]
    #[cfg_attr(not(target_arch = "wasm32"), inline(always))]
    fn observe_execution(&mut self, core: usize, pc: u32, used: u32, trap: Option<Trap>) -> Option<Stop> {
        if self.probes.contains(Wants::BLOCK | Wants::TRAP | Wants::TRAP_PC) {
            let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
            let cpu = &self.cores[core];
            for o in &mut self.observers {
                let w = o.wants();
                if w.contains(Wants::BLOCK) && used > 0 { o.on_block(&cx, core, pc, used); }
                if let (true, Some(t)) = (w.contains(Wants::TRAP | Wants::TRAP_PC), &trap) { o.on_trap(&cx, core, cpu, pc, t); }
            }
        }
        match trap {
            None => {}
            Some(Trap::Exception(_)) => { self.exceptions += 1; }
            Some(Trap::Interrupt(irq)) => { self.interrupts += 1; self.irq_hist[core][(irq & 31) as usize] += 1; S::interrupt_taken(&mut self.bus, core, irq); }
            Some(Trap::Unimplemented(p, raw)) => { if self.dbg.stop_on_unimplemented { return Some(Stop::Unimplemented(p, raw)); } }
            Some(Trap::Simcall) => return Some(Stop::Simcall(pc)),
            Some(Trap::Ebreak(p)) => { self.exceptions += 1; if !self.cores[core].has_trap_handler() { return Some(Stop::Ebreak(p)); } }
        }
        None
    }

    /// Observe sleeping PCs without advancing their instruction or device clocks.
    #[inline]
    fn observe_idle_pcs(&mut self, on: &[bool]) -> Option<Stop> {
        if !self.probes.contains(Wants::IDLE_PC) { return None; }
        let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
        for (i, (&enabled, cpu)) in on.iter().zip(&self.cores).enumerate() {
            if enabled && cpu.waiting() && !cpu.irq_pending() {
                for observer in &mut self.observers {
                    if observer.wants().contains(Wants::IDLE_PC) {
                        if let Some(stop) = observer.on_insn(&cx, i, cpu, &mut self.bus, cpu.pc()) { return Some(stop); }
                    }
                }
            }
        }
        None
    }

    /// Execute one instruction on `core` with every per-instruction observer; returns Some(stop) if the run must end.
    #[inline]
    fn step_core(&mut self, core: usize) -> Option<Stop> {
        if S::core_state(&self.bus, 0) == CoreState::Held && S::core_state(&self.bus, core) == CoreState::Held { return None; }
        S::select_core(&mut self.bus, core);
        let cpu = &mut self.cores[core];
        let pc = cpu.pc();
        if self.probe_bloom & pc_bit(pc) != 0 && !cpu.waiting() {
            if let Some(name) = self.fn_probes.get(&pc) {
                let (args, ret) = (cpu.probe_args(&mut self.bus), cpu.return_address(&mut self.bus));
                eprintln!("[fn] i={} t={:.4}s c{} {}({}) ret={:#x}", cpu.insn_count(), self.bus.cycles() as f64 / S::CPU_HZ as f64, core, name, args, ret);
            }
        }
        if self.stub_bloom & pc_bit(pc) != 0 && !cpu.waiting() {
            if S::function_hook(cpu, &mut self.bus) { return None; }
            if let Some(&ret) = self.stubs.get(&pc) { cpu.return_from_stub(&mut self.bus, ret); self.stub_hits += 1; return None; }
        }
        {
            let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
            for o in &mut self.observers { if o.wants().contains(Wants::INSN) { if let Some(stop) = o.on_insn(&cx, core, &self.cores[core], &mut self.bus, pc) { return Some(stop); } } }
        }
        let cpu = &mut self.cores[core];
        self.bus.note_pc(pc);
        let outcome = cpu.step(&mut self.bus);
        if let Some(stop) = self.observe_execution(core, pc, u32::from(outcome.kind != emu_core::StepKind::Idle), outcome.trap()) { return Some(stop); }
        self.refresh_irq();
        {
            let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
            for o in &mut self.observers { if o.wants().contains(Wants::INSN) { if let Some(stop) = o.after_insn(&cx, core, &self.cores[core], &mut self.bus) { return Some(stop); } } }
        }
        if self.exceptions >= self.dbg.stop_after_exceptions { return Some(Stop::Exceptions(self.exceptions)); }
        None
    }

    /// Run until something stops us or the `max_insns` scheduling-step budget is reached. The no-model path
    /// uses complete quanta (`QUANTUM` by default), so a busy round can exceed the budget by up to
    /// `quantum - 1` steps, but the cycle ceiling cuts the final round short.
    /// The modeled path schedules one priced event at a time.
    pub fn run(&mut self, max_insns: u64) -> Stop {
        self.web_poll_input();
        self.refresh_irq();
        if self.bus.board_ref().uses_gpio_waveform() {
            if self.has_timing_model() { return Stop::CostModelLifecycle { kind: LifecycleKind::Attach, reason: GPIO_WAVEFORM_TIMING_ERROR.into() }; }
            let quantum = std::mem::replace(&mut self.quantum, 1);
            let stop = self.run_unmodeled::<false>(max_insns);
            self.quantum = quantum;
            return stop;
        }
        if self.cost.is_some() { self.run_modeled(max_insns) } else if self.approximate_jit_frontiers { self.run_approximate_jit_frontiers(max_insns) } else if self.approximate_jit_timing.is_some() { self.run_unmodeled::<true>(max_insns) } else { self.run_unmodeled::<false>(max_insns) }
    }

    /// The complete unmodelled core-0 quantum a browser-side straight-line block may retire at
    /// the current scheduling boundary. External execution is deliberately limited to the
    /// single-core, unobserved case; returning a partial quantum would move device events relative
    /// to instructions, so a request smaller than a full quantum is rejected. The caller
    /// still has to enforce architectural boundaries such as
    /// CCOMPARE and register-window overflow for the block it proposes.
    pub fn browser_external_block_budget(&self, requested: u32) -> Option<u32> {
        if self.bus.board_ref().uses_gpio_waveform()
            || u64::from(requested) < self.quantum
            || self.cost.is_some()
            || self.approximate_jit_timing.is_some()
            || self.probes.0 != 0
            || !self.stubs.is_empty()
            || !S::function_hooks(&self.bus).is_empty()
            || !self.fn_probes.is_empty()
            || self.script.pos < self.script.events.len()
            || self.bus.sw_reset()
            || self.bus.block_break()
            || self.cores[0].waiting()
            || self.cores[0].irq_pending()
            || (1..S::CORES).any(|core| S::core_state(&self.bus, core) == CoreState::Running)
        {
            return None;
        }
        Some(self.quantum as u32)
    }

    /// Advance shared device time after a quantum accepted by
    /// `browser_external_block_budget`. Architectural core state must already contain the full
    /// quantum's result.
    pub fn finish_browser_external_quantum(&mut self) -> Option<Stop> {
        self.after_round(self.quantum);
        if self.bus.sw_reset() { self.drain_console(); return Some(Stop::SwReset); }
        if self.bus.cycles() >= self.max_cycles { self.drain_console(); return Some(Stop::Halted); }
        self.drain_console();
        None
    }

    fn run_unmodeled<const APPROXIMATE: bool>(&mut self, max_insns: u64) -> Stop {
        assert!(self.quantum != 0, "scheduling quantum must be nonzero");
        let (cpi, max_quantum) = if APPROXIMATE { self.approximate_jit_timing.unwrap() } else { (1, self.quantum as u32) };
        self.stub_bloom = self.stubs.keys().chain(S::function_hooks(&self.bus)).fold(0, |m, &pc| m | pc_bit(pc));
        self.probe_bloom = self.fn_probes.keys().fold(0, |m, &pc| m | pc_bit(pc));
        for c in &mut self.cores {
            c.set_boundaries(self.stub_bloom | self.probe_bloom);
            c.set_block_observation(self.probes.contains(Wants::BLOCK | Wants::TRAP_PC));
        }
        // Per-instruction observers need the slow hooks; only exact-PC trap observers use bounded fragments.
        let blocks = !self.probes.contains(Wants::INSN);
        let slow_path = self.probes.contains(Wants::NO_IDLE_SKIP);
        let can_defer = !APPROXIMATE && self.vq_max > 1 && self.bus.can_defer();
        // EX177: round batching needs the same deferral guard, several cores and a loop whose
        // boundaries do nothing a bound cannot see. Real-time pacing and observers keep it off.
        let can_batch = !APPROXIMATE && self.bb_max > 1 && S::CORES > 1 && self.bus.can_defer()
            && blocks && !slow_path && self.probes.0 == 0 && !self.rt.enabled;
        if self.web.is_some() { self.ws.push_interval = (S::CPU_HZ / self.bus.board_ref().display_push_hz().max(1)).max(1); }
        let trace = self.has_observer("trace");
        let mut n = 0u64;
        let mut on = [true; 4];
        let mut idle = [true; 4];
        loop {
            if n >= max_insns { self.drain_console(); return Stop::MaxInsns; }
            self.apply_script_events();
            self.refresh_irq();
            if self.bus.sw_reset() { self.drain_console(); return Stop::SwReset; }
            if self.bus.cycles() >= self.max_cycles { self.drain_console(); return Stop::Halted; }
            for (i, state) in on.iter_mut().enumerate().take(S::CORES) {
                *state = match S::core_state(&self.bus, i) {
                    CoreState::Reset => { self.core_held[i] = true; false }
                    CoreState::Held => false,
                    CoreState::Running => {
                        if self.core_held[i] { self.core_held[i] = false; S::reset_core_with_bus(&mut self.cores[i], &mut self.bus, i); if trace { eprintln!("          ** core{} released from reset", i); } }
                        true
                    }
                };
            }
            if let Some(stop) = self.observe_idle_pcs(&on[..S::CORES]) { self.drain_console(); return stop; }
            for (i, state) in idle.iter_mut().enumerate().take(S::CORES) { *state = !on[i] || (self.cores[i].waiting() && !self.cores[i].irq_pending()); }
            if idle[..S::CORES].iter().all(|&x| x) && !slow_path {
                // Stop at every known source of new work, including core-local timers. Device
                // deadlines alone do not include CCOMPARE or host-script actions.
                let limit = S::IDLE_CHUNK.min(max_insns - n).min(self.bus.cpu_cycles_for_bus(self.max_cycles - self.bus.cycles()));
                let chunk = self.idle_budget(limit, &on);
                for (i, &enabled) in on.iter().enumerate().take(S::CORES) { if enabled { self.cores[i].idle_advance(chunk as u32); } }
                n += chunk;
                self.run_steps += chunk;
                self.after_round(chunk);
                if self.bus.sw_reset() { self.drain_console(); return Stop::SwReset; }
                if self.bus.cycles() >= self.max_cycles { self.drain_console(); return Stop::Halted; }
                if n & 0xffff < chunk { self.drain_console(); }
                continue;
            }
            // EX133/EX144 virtual quanta: one core alone is busy, so nothing outside it can change until the
            // next device deadline. Let it run several quanta in one budget; the rounds it spans are
            // closed afterwards exactly as the per-quantum schedule would have closed them. A device
            // register access stops in front of its instruction and finishes its quantum the old way.
            let mut resume_at = 0u64;
            // The core that owns `resume_at`.
            let mut resume_core = 0usize;
            // Find the sole busy core only when virtual quanta are eligible.
            let busy = if !APPROXIMATE && can_defer && blocks && !slow_path && self.probes.0 == 0 {
                if self.vq_skip > 0 { self.vq_skip -= 1; usize::MAX }
                else {
                    let mut b = (0..S::CORES).filter(|&i| !idle[i]);
                    match (b.next(), b.next()) { (Some(i), None) => i, _ => usize::MAX }
                }
            } else { usize::MAX };
            if busy != usize::MAX {
                let k = self.vq_quanta(max_insns - n, &on, busy);
                if k > 1 {
                    let total = (k * self.quantum) as u32;
                    let mut left = total;
                    let mut stop = None;
                    self.bus.set_defer(true);
                    while left > 0 {
                        let (used, s) = self.step_blocks(busy, left);
                        left -= used.min(left);
                        if s.is_some() { stop = s; break; }
                        if self.bus.take_deferred() { self.vq_stats[2] += 1; break; }
                        if self.cores[busy].waiting() { self.vq_stats[3] += 1; break; }
                    }
                    self.bus.set_defer(false);
                    let pos = (total - left) as u64;
                    self.vq_stats[0] += 1; self.vq_stats[1] += pos / self.quantum;
                    if pos < 2 * self.quantum { self.vq_penalty = (self.vq_penalty * 2 + 1).min(255); self.vq_skip = self.vq_penalty; } else { self.vq_penalty = 0; }
                    // A stopping instruction belongs to the unfinished round, even when it
                    // consumes its last slot. Preserve the ordinary path's pre-tick return.
                    let completed = if stop.is_some() { pos.saturating_sub(1) / self.quantum } else { pos / self.quantum };
                    if busy == 0 { self.run_steps += pos - completed * self.quantum; }
                    for _ in 0..completed {
                        if let Some(s) = self.vq_close_round(&on, &mut n, busy) { return s; }
                    }
                    if let Some(s) = stop {
                        for (core, &enabled) in self.cores.iter_mut().zip(&on).take(busy) {
                            if enabled { core.idle_advance(self.quantum as u32); }
                        }
                        if busy > 0 && on[0] { self.run_steps += self.quantum; }
                        self.drain_console(); return s;
                    }
                    if pos > 0 && pos.is_multiple_of(self.quantum) { continue; }
                    resume_at = pos % self.quantum;
                    resume_core = busy;
                }
            }
            // EX177: every enabled core is busy, so the schedule of the next rounds is already
            // decided: each core runs one whole quantum in index order. Run K of those rounds in
            // one batch and close them in one fold, with the same per-core budgets and order.
            if can_batch && busy == usize::MAX {
                let (mut enabled, mut all_busy) = (0u32, true);
                for i in 0..S::CORES { if on[i] { enabled += 1; all_busy &= !idle[i]; } }
                if all_busy && enabled > 1 {
                    let k = self.bb_quanta(max_insns - n, n);
                    if k > 1 {
                        self.bb_stats[0] += 1; self.bb_stats[5] += k;
                        if let Err(stop) = self.bb_batch(k, &mut n, &on) { return stop; }
                        continue;
                    } else { self.bb_stats[6] += 1; }
                }
            }
            let quantum = if APPROXIMATE {
                // An instruction can overrun the deadline by at most CPI-1 cycles.
                self.idle_budget(u64::from(max_quantum) * u64::from(cpi), &on)
                    .min(self.bus.cpu_cycles_for_bus(self.max_cycles - self.bus.cycles())).div_ceil(u64::from(cpi)).max(1)
            } else {
                // A sleeping peer's local timer bounds the entire round. Advancing only that
                // peer by a shorter interval would leave its CCOUNT behind shared device time.
                // Use the round-entry idle snapshot: a busy core that reached WAITI in a
                // partial virtual round has already advanced by resume_at. Its remaining
                // timer delta cannot shorten that original round, which must match VQ-off.
                let quantum = self.cores.iter().enumerate().filter(|(i, _)| on[*i] && idle[*i])
                    .filter_map(|(_, core)| core.cycles_until_wake())
                    .fold(self.quantum, |limit, wake| limit.min(wake.max(1)));
                if self.max_cycles != u64::MAX { quantum.min(self.bus.cpu_cycles_for_bus(self.max_cycles - self.bus.cycles())) } else { quantum }
            };
            let elapsed = quantum * u64::from(cpi);
            let mut stalls = [0u64; 4];
            let mut round_elapsed = 0;
            for i in 0..S::CORES {
                if !on[i] || S::core_state(&self.bus, 0) == CoreState::Held && S::core_state(&self.bus, i) == CoreState::Held { continue; }
                if idle[i] && !slow_path {
                    self.cores[i].idle_advance(elapsed as u32);
                    if i == 0 { self.run_steps += quantum; }
                } else if blocks {
                    // Only the core that stopped inside the round carries its resume offset.
                    let budget = (quantum - if i == resume_core { resume_at } else { 0 }) as u32;
                    let mut left = budget;
                    while left > 0 {
                        let (used, stop) = self.step_blocks(i, left);
                        left -= used.min(left);
                        if let Some(stop) = stop {
                            if i == 0 { self.run_steps += u64::from(budget - left); }
                            self.drain_console(); return stop;
                        }
                        if APPROXIMATE {
                            let penalty = self.bus.take_timing_penalty().saturating_add(self.cores[i].take_timing_extra());
                            self.cores[i].advance_cycles(penalty);
                            stalls[i] += u64::from(penalty);
                        }
                        // a reset takes effect at the instruction that requested it: the core's
                        // run already stopped there (the register write broke the block)
                        if self.bus.sw_reset() {
                            if i == 0 { self.run_steps += u64::from(budget - left); }
                            return self.finish_reset(round_elapsed.max((quantum - u64::from(left)) * u64::from(cpi) + stalls[i]));
                        }
                    }
                    if i == 0 { self.run_steps += u64::from(budget); }
                } else {
                    for used in 1..=quantum {
                        let stop = self.step_core(i);
                        if let Some(stop) = stop {
                            if i == 0 { self.run_steps += used; }
                            self.drain_console(); return stop;
                        }
                        if APPROXIMATE {
                            let penalty = self.bus.take_timing_penalty().saturating_add(self.cores[i].take_timing_extra());
                            let top_up = if self.cores[i].step_charges_cpi() { 0 } else { cpi - 1 };
                            self.cores[i].advance_cycles(top_up.saturating_add(penalty));
                            stalls[i] += u64::from(penalty);
                        }
                        if self.bus.sw_reset() {
                            if i == 0 { self.run_steps += used; }
                            return self.finish_reset(round_elapsed.max(used * u64::from(cpi) + stalls[i]));
                        }
                    }
                    if i == 0 { self.run_steps += quantum; }
                }
                round_elapsed = round_elapsed.max(elapsed + stalls[i]);
                if i == 0 { n += quantum; }
            }
            let stall = if APPROXIMATE { *stalls[..S::CORES].iter().max().unwrap() } else { 0 };
            if APPROXIMATE {
                // Coarse lockstep approximation: both cores meet again after the slower batch.
                // This intentionally exposes memory costs before access-level scheduling exists.
                for i in 0..S::CORES {
                    if on[i] { self.cores[i].advance_cycles((stall - stalls[i]) as u32); }
                }
            }
            self.after_round(elapsed + stall);
            if self.bus.sw_reset() { self.drain_console(); return Stop::SwReset; }
            if self.bus.cycles() >= self.max_cycles { self.drain_console(); return Stop::Halted; }
            if n & 0xffff < quantum { self.drain_console(); }
        }
    }

    /// A reset ends the current round immediately. Charge the work already executed, without
    /// dispatching another core or running post-round host actions in the resetting machine.
    fn finish_reset(&mut self, cycles: u64) -> Stop {
        self.bus.tick(cycles as u32);
        self.observe_round();
        self.drain_console();
        Stop::SwReset
    }

    /// EX133/EX144: how many quanta the sole busy core may run in one budget. Every bound keeps the boundaries
    /// inside the run free of work: no device flush, script event, page push, peer wake-up,
    /// cycle or instruction limit may fall due before the last of them.
    fn vq_quanta(&self, insns_left: u64, on: &[bool], busy: usize) -> u64 {
        let Some(deadline) = self.bus.next_deadline() else { return 1 };
        if self.rt.enabled { return 1; }
        let now = self.bus.cycles();
        let mut k = self.vq_max.min(deadline.div_ceil(self.quantum)).min(insns_left.div_ceil(self.quantum))
            .min(self.max_cycles.saturating_sub(now) / self.quantum);
        for (i, (core, &enabled)) in self.cores.iter().zip(on).enumerate() {
            if enabled && i != busy { if let Some(wake) = core.cycles_until_wake() { k = k.min(wake / self.quantum); } }
        }
        if let Some((at, _)) = self.script.events.get(self.script.pos) { k = k.min(at.saturating_sub(now).div_ceil(self.quantum)); }
        if self.web.is_some() { k = k.min(self.ws.push_interval.saturating_sub(now.wrapping_sub(self.ws.last_push_cycles)).div_ceil(self.quantum)); }
        k.max(1)
    }

    /// EX133/EX144: close one quantum that the busy core ran alone, as the scheduling loop does.
    fn vq_close_round(&mut self, on: &[bool], n: &mut u64, busy: usize) -> Option<Stop> {
        for (i, (core, &enabled)) in self.cores.iter_mut().zip(on).enumerate() { if enabled && i != busy { core.idle_advance(self.quantum as u32); } }
        *n += self.quantum;
        self.run_steps += self.quantum;
        self.after_round(self.quantum);
        if self.bus.sw_reset() { self.drain_console(); return Some(Stop::SwReset); }
        if self.bus.cycles() >= self.max_cycles { self.drain_console(); return Some(Stop::Halted); }
        if *n & 0xffff < self.quantum { self.drain_console(); }
        None
    }

    /// EX177: how many whole rounds the busy cores may run in one batch. Every bound keeps the
    /// boundaries the batch folds free of work: no device flush, script event, page push, console
    /// drain, cycle or instruction limit may fall due before the last of them. `next_deadline` is
    /// the bus's own bound on deferred device time, exactly as `vq_quanta` uses it.
    fn bb_quanta(&self, insns_left: u64, n: u64) -> u64 {
        let Some(deadline) = self.bus.next_deadline() else { return 1 };
        let q = self.quantum;
        let now = self.bus.cycles();
        let mut k = self.bb_max.min(deadline.div_ceil(q)).min(insns_left.div_ceil(q))
            .min(self.max_cycles.saturating_sub(now) / q)
            // the console is drained when the scheduling counter crosses a 64Ki boundary
            .min((0x1_0000 - (n & 0xffff)).div_ceil(q));
        if let Some((at, _)) = self.script.events.get(self.script.pos) { k = k.min(at.saturating_sub(now).div_ceil(q)); }
        if self.web.is_some() { k = k.min(self.ws.push_interval.saturating_sub(now.wrapping_sub(self.ws.last_push_cycles)).div_ceil(q)); }
        k.max(1)
    }

    /// EX177: run `k` whole rounds while every enabled core is busy. Each round dispatches the
    /// cores in index order for exactly one quantum each — the per-round schedule's order, budgets
    /// and instruction interleaving — and the rounds are then closed in one fold, which `bb_quanta`
    /// has made equivalent to closing them one at a time. `bus.tick` only accumulates until its
    /// own deadline, so the folded call flushes the same cycles at the same boundary.
    ///
    /// A device-register access stops the batch in front of its instruction; batch-s1 folds the
    /// completed rounds there and finishes that round with access allowed, then re-bounds the batch.
    /// A core that goes to sleep ends it at the end of its round. `Err` is a stop inside a round.
    fn bb_batch(&mut self, mut k: u64, n: &mut u64, on: &[bool]) -> Result<(), Stop> {
        let q = self.quantum;
        let (mut done, mut sleep, mut open) = (0u64, false, false);
        // (core, instructions of its quantum already run, a stop that ended the batch there)
        let mut cut: Option<(usize, u64, Stop)> = None;
        self.bus.set_defer(true);
        'batch: while done < k {
            // EX177: preserve the indexed form used by the measured artifact (see
            // docs/evidence/perf-x4-2026-09-22/codegen/default128-comparison.json).
            #[allow(clippy::needless_range_loop)]
            for i in 0..S::CORES {
                if !on[i] { continue; }
                let mut left = q as u32;
                while left > 0 {
                    // lane-s2b: a start the core prepared runs without step_blocks' stub/probe test (the
                    // memo never names a boundary PC and batches run without observers).
                    let pc = self.cores[i].pc();
                    S::select_core(&mut self.bus, i);
                    let (used, stop) = match self.cores[i].run_prepared(&mut self.bus, left) {
                        Some((used, trap)) => { self.bb_stats[7] += 1; self.finish_step(i, pc, used, trap) }
                        None => self.step_blocks(i, left),
                    };
                    left -= used.min(left);
                    if let Some(stop) = stop { cut = Some((i, q - u64::from(left), stop)); break 'batch; }
                    if self.bus.take_deferred() {
                        // batch-s1: fold the completed rounds, which puts device time where this round
                        // expects it, and finish the round with device access allowed, as the ordinary path does.
                        self.bb_stats[2] += 1;
                        if done > 0 { self.after_round(done * q); self.bb_stats[1] += done; k -= done; done = 0; }
                        self.bus.set_defer(false);
                        open = true;
                        continue;
                    }
                    // Defensive for future buses: a reset is a deferred device-register write today, but
                    // the ordinary path ends the round at that instruction, so end the batch too.
                    if self.bus.sw_reset() { cut = Some((i, q - u64::from(left), Stop::SwReset)); break 'batch; }
                }
                if i == 0 { *n += q; self.run_steps += q; }
                sleep |= self.cores[i].waiting();
            }
            done += 1;
            if sleep { self.bb_stats[3] += 1; break; }
            if open {
                // batch-s1: the accesses may have moved a device deadline or a core's run state. Re-bound
                // from this round's start (core 0 already counted it in `n`); the loop top would re-plan.
                open = false;
                if (0..S::CORES).any(|i| on[i] != (S::core_state(&self.bus, i) == CoreState::Running)) { break; }
                k = self.bb_quanta(k * q, *n - q);
                self.bus.set_defer(true);
            }
        }
        self.bus.set_defer(false);
        self.bb_stats[1] += done;
        self.bb_stats[4] += u64::from(done == k);
        let Some((core, at, stop)) = cut else {
            if done > 0 {
                self.after_round(done * q);
                if self.bus.sw_reset() { self.drain_console(); return Err(Stop::SwReset); }
                if self.bus.cycles() >= self.max_cycles { self.drain_console(); return Err(Stop::Halted); }
                if *n & 0xffff < q { self.drain_console(); }
            }
            return Ok(());
        };
        // A cut leaves `done < k`, so none of the folded boundaries can be the one the bound
        // allowed work at: the fold only moves device time forward for the unfinished round.
        if done > 0 { self.after_round(done * q); }
        // Core 0's part of the unfinished round is its scheduling budget either way: the ordinary
        // path charges only the budget it is given, and a stop charges what it ran.
        if core == 0 { self.run_steps += at; }
        match stop {
            // the bus reset; charge the unfinished round's cycles as its own path would
            Stop::SwReset => Err(self.finish_reset(if core > 0 { q } else { at })),
            s => { self.drain_console(); Err(s) }
        }
    }

    /// Positive idle advance bounded by device work, enabled cores' wakeups and host actions.
    /// Callers settle actions already due and check their own stop bound before using this.
    fn idle_budget(&self, limit: u64, on: &[bool]) -> u64 {
        let mut budget = limit.min(u32::MAX as u64 >> 1);
        if let Some(delta) = self.bus.next_deadline() { budget = budget.min(delta.max(1)); }
        for (core, &enabled) in self.cores.iter().zip(on) {
            if enabled { if let Some(delta) = core.cycles_until_wake() { budget = budget.min(delta.max(1)); } }
        }
        if let Some((at, _)) = self.script.events.get(self.script.pos) {
            budget = budget.min(self.bus.cpu_cycles_for_bus(at.saturating_sub(self.bus.cycles())).max(1));
        }
        budget
    }

    /// Run core 0 until device time reaches `target` (a cycle count of `bus.cycles()`), exactly:
    /// the round is cut short at the target, so time never overshoots by a quantum, and cut
    /// short at the instruction — or the device tick — that raises a host event
    /// (`SocBus::take_host_event`), so a transmission the host must forward is seen at the cycle
    /// it started. Every round is also bounded by the bus's next device deadline
    /// (`SocBus::next_deadline`), so a device event lands at its own cycle rather than at the end
    /// of a scheduling quantum; a core asleep in `wfi` with nothing pending lets time jump to
    /// the target or to that deadline, whichever is first — the deadline is conservative, so an
    /// interrupt is never delivered late by the skip. Single-core chips only (the S3's second
    /// core is not scheduled here), and the unmodeled path only: a cost model is not consulted.
    /// Nothing else about a run changes: stubs, probes, observers, scripts and the console work
    /// as in `run`; `max_cycles` is not consulted.
    pub fn run_until_cycle(&mut self, target: u64) -> RunUntil {
        let quantum = if self.bus.board_ref().uses_gpio_waveform() {
            if self.has_timing_model() { return RunUntil::Stop(Stop::CostModelLifecycle { kind: LifecycleKind::Attach, reason: GPIO_WAVEFORM_TIMING_ERROR.into() }); }
            1
        } else { self.quantum };
        self.web_poll_input();
        self.refresh_irq();
        self.stub_bloom = self.stubs.keys().chain(S::function_hooks(&self.bus)).fold(0, |m, &pc| m | pc_bit(pc));
        self.probe_bloom = self.fn_probes.keys().fold(0, |m, &pc| m | pc_bit(pc));
        for c in &mut self.cores {
            c.set_boundaries(self.stub_bloom | self.probe_bloom);
            c.set_block_observation(self.probes.contains(Wants::BLOCK | Wants::TRAP_PC));
        }
        let blocks = !self.probes.contains(Wants::INSN);
        let no_skip = self.probes.contains(Wants::NO_IDLE_SKIP);
        loop {
            let now = self.bus.cycles();
            if now >= target { return RunUntil::Reached; }
            if self.apply_script_events() { self.drain_console(); return RunUntil::Stop(Stop::Halted); }
            self.refresh_irq();
            if self.bus.sw_reset() { self.drain_console(); return RunUntil::Stop(Stop::SwReset); }
            if let Some(stop) = self.observe_idle_pcs(&[true]) { self.drain_console(); return RunUntil::Stop(stop); }
            let left = target - now;
            let core = &self.cores[0];
            let held = S::core_state(&self.bus, 0) == CoreState::Held;
            if held || core.waiting() && !core.irq_pending() && !no_skip {
                let chunk = self.idle_budget(left, &[!held]);
                if !held { self.cores[0].idle_advance(chunk as u32); }
                if self.after_round(chunk) { self.drain_console(); return RunUntil::Stop(Stop::Halted); }
                if self.bus.take_host_event() { return RunUntil::Yield; }
            } else {
                let mut deadline = self.bus.next_deadline().unwrap_or(u64::MAX).max(1);
                if let Some((at, _)) = self.script.events.get(self.script.pos) {
                    deadline = deadline.min(at.saturating_sub(now).max(1));
                }
                let mut budget = left.min(quantum).min(deadline) as u32;
                let (mut used_total, mut yielded, mut stop) = (0u64, false, None);
                while budget > 0 {
                    let (used, s) = if blocks { self.step_blocks(0, budget) } else { (1, self.step_core(0)) };
                    used_total += used as u64;
                    budget -= used.min(budget);
                    if s.is_some() { stop = s; break; }
                    if self.bus.sw_reset() { return RunUntil::Stop(self.finish_reset(used_total)); }
                    if self.bus.take_host_event() { yielded = true; break; }
                }
                let script_stopped = self.after_round(used_total);
                if let Some(s) = stop { self.drain_console(); return RunUntil::Stop(s); }
                if script_stopped { self.drain_console(); return RunUntil::Stop(Stop::Halted); }
                if yielded || self.bus.take_host_event() { return RunUntil::Yield; }
            }
            if self.bus.sw_reset() { self.drain_console(); return RunUntil::Stop(Stop::SwReset); }
        }
    }

    /// Re-derive the interrupt lines now, after the host changed a device (a frame injected
    /// between rounds), so the next instruction sees them.
    pub fn sync_irq(&mut self) { *self.bus.irq_dirty() = true; self.refresh_irq(); }

    /// Device time, interrupt lines, scripts, web, real-time pacing after a scheduling round.
    #[inline]
    fn after_round(&mut self, cycles: u64) -> bool {
        // device models only change state when they run, so the lines are re-derived after a
        // flush or a register write and never on a fixed cadence
        let ticked = self.bus.tick(cycles as u32) != 0;
        if *self.bus.irq_dirty() || ticked {
            *self.bus.irq_dirty() = false;
            if self.bus.refresh_irq() { self.present_irqs(); }
            S::peripheral_event(&self.bus, &mut self.cores);
        }
        let script_stopped = self.after_round_rest();
        self.observe_round();
        script_stopped
    }

    /// Flush observations even when reset prevents post-round host actions.
    #[inline]
    fn observe_round(&mut self) {
        if self.probes.0 != 0 {
            self.deliver_events();
            if self.probes.contains(Wants::ROUND) { let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ }; for o in &mut self.observers { if o.wants().contains(Wants::ROUND) { o.on_round(&cx); } } }
        }
    }

    /// Keep the common empty/not-due case in the scheduling loop without inlining action
    /// cloning, logging or device dispatch. Read the public script state at each boundary so
    /// host edits between runs and events inserted by web input are observed immediately.
    #[inline]
    fn apply_script_events(&mut self) -> bool {
        if !self.script.events.get(self.script.pos).is_some_and(|(at, _)| *at <= self.bus.cycles()) {
            return false;
        }
        self.apply_due_script_events()
    }

    /// Apply actions at the current boundary without advancing device time.
    #[inline(never)]
    fn apply_due_script_events(&mut self) -> bool {
        let mut stopped = false;
        while self.script.pos < self.script.events.len() && self.script.events[self.script.pos].0 <= self.bus.cycles() {
            if let ScriptAction::WaitUart0(text, timeout, orig) = self.script.events[self.script.pos].1.clone() {
                self.drain_console();
                let now = self.bus.cycles();
                let mark = *self.script.wait_mark.get_or_insert(self.script.uart0_seen.len());
                let hit = self.script.uart0_seen[mark.min(self.script.uart0_seen.len())..].windows(text.len().max(1)).any(|w| w == text.as_bytes());
                if !hit && now < orig.saturating_add(timeout) {
                    self.script.events[self.script.pos].0 = now + S::CPU_HZ / 1000;   // look again in 1 ms, without spinning
                    break;
                }
                let waited = now.saturating_sub(orig);
                if self.script.log { eprintln!("[script] t={:.3}s wait uart0 {} {:?} after {:.3}s", now as f64 / S::CPU_HZ as f64, if hit { "matched" } else { "timeout" }, text, waited as f64 / S::CPU_HZ as f64); }
                self.script.wait_mark = None; self.script.pos += 1;
                for e in self.script.events[self.script.pos..].iter_mut() {
                    e.0 += waited;
                    if let ScriptAction::WaitUart0(_, _, o) = &mut e.1 { *o += waited; }
                }
                continue;
            }
            let (t, a) = self.script.events[self.script.pos].clone(); self.script.pos += 1;
            if self.script.log { eprintln!("[script] t={:.3}s {:?}", t as f64 / S::CPU_HZ as f64, a); }
            match a {
                ScriptAction::Gpio(pin, level) => { self.bus.gpio_set_input(pin, level); *self.bus.irq_dirty() = true; }
                ScriptAction::Serial(text) => self.bus.serial_input(text.as_bytes()),
                ScriptAction::Uart(n, text) => self.bus.uart_input(n, text.as_bytes()),
                ScriptAction::Stop => { self.max_cycles = 0; stopped = true; }
                ScriptAction::TouchPad(pin, touched) => self.bus.set_touch_input(pin, touched),
                ScriptAction::Touch(x, y, d) => { self.bus.touch_input(x, y, d); }
                ScriptAction::Poke(a, v) => { let _ = self.bus.write32_unpriced(a, v); }
                ScriptAction::Analog(pin, mut src) => {
                    // a waveform plays from when it is applied: a `waituart0` before it moves `t`, so the start moves too
                    if let esp_periph::AnalogSource::Wave { start_cycles, .. } = &mut src { *start_cycles = t; }
                    self.bus.analog_set(pin, src)
                }
                ScriptAction::WaitUart0(..) => {}
                ScriptAction::Ble(command) => { if let Err(error) = self.bus.ble_command(&command) { eprintln!("[ble] {error}"); } }
            }
        }
        stopped
    }

    #[inline]
    fn after_round_rest(&mut self) -> bool {
        let stopped = self.apply_script_events();
        // EX170: the cached interval filters the common not-yet-due round without the board call and
        // division; a due round re-derives it from the board before deciding, as before.
        // EX168 s4: the per-round test is two loads and a compare; the re-derivation, the push and the
        // pacing clock live out of line.
        if self.web.is_some() && self.bus.cycles().wrapping_sub(self.ws.last_push_cycles) >= self.ws.push_interval { self.web_push_due(); }
        if self.rt.enabled && self.bus.cycles().wrapping_sub(self.rt.last_check) >= 1 << 16 { self.rt_pace(); }
        stopped
    }

    /// EX168 s4: the due branch of the display push, out of line (EX170 semantics: re-derive the
    /// interval from the board, then decide).
    #[cold]
    #[inline(never)]
    fn web_push_due(&mut self) {
        self.ws.push_interval = (S::CPU_HZ / self.bus.board_ref().display_push_hz().max(1)).max(1);
        if self.bus.cycles().wrapping_sub(self.ws.last_push_cycles) >= self.ws.push_interval { self.ws.last_push_cycles = self.bus.cycles(); self.web_push(); self.web_poll_input(); }
    }

    /// EX168 s4: the real-time pacing clock, out of line.
    #[cold]
    #[inline(never)]
    fn rt_pace(&mut self) {
        self.poll_camera_input();
        {
            self.rt.last_check = self.bus.cycles();
            let start = *self.rt.wall_start.get_or_insert_with(std::time::Instant::now);
            let emulated = std::time::Duration::from_secs_f64(self.bus.cycles() as f64 / S::CPU_HZ as f64);
            let wall = start.elapsed();
            let (now, cycles) = (std::time::Instant::now(), self.bus.cycles());
            match self.rt.speed_mark {
                Some((at, from)) if cycles >= from && now.duration_since(at) >= std::time::Duration::from_secs(1) => {
                    self.rt.speed = Some((cycles - from) as f64 / S::CPU_HZ as f64 / now.duration_since(at).as_secs_f64());
                    self.rt.speed_mark = Some((now, cycles));
                }
                Some((_, from)) if cycles < from => self.rt.speed_mark = Some((now, cycles)),   // the count restarted
                None => self.rt.speed_mark = Some((now, cycles)),
                _ => {}
            }
            if emulated > wall + std::time::Duration::from_millis(2) { std::thread::sleep(emulated - wall); self.rt.behind = 0.0; }
            else if wall > emulated + std::time::Duration::from_millis(50) {
                self.rt.behind = (wall - emulated).as_secs_f64();
                // more than half a second behind: resynchronise (skip the lag) rather than flood the client while catching up
                if wall > emulated + std::time::Duration::from_millis(500) { self.rt.resyncs += 1; self.rt.wall_start = Some(std::time::Instant::now() - emulated); }
            } else { self.rt.behind = 0.0; }
        }
    }

    /// One encoder detent as (pin, level) edges, 2 ms apart. Idle is (1,1); CW: CLK falls while
    /// DT=1, then DT falls, CLK rises, DT rises. CCW: DT first.
    fn quadrature(clk: u8, dt: u8, cw: bool) -> [(u8, bool); 4] {
        if cw { [(clk, false), (dt, false), (clk, true), (dt, true)] } else { [(dt, false), (clk, false), (dt, true), (clk, true)] }
    }

    // ------------------------------------------------------------------ scripts
    /// Parse a script: one action per line, `<seconds> <cmd> [args]`.
    ///   press <pin> [ms]   release <pin>   gpio <pin> <0|1>   serial <text...>   knob <cw|ccw> [detents]   touch <x> <y> <0|1>   poke <addr> <value>   stop
    /// Pins are numbers or the board's names (`btn1`, `sw`, ...); buttons/encoder are active-low with pull-ups (release = 1).
    pub fn load_script(&mut self, text: &str) -> Result<(), String> {
        let hz = S::CPU_HZ as f64;
        let mut ev: Vec<(u64, ScriptAction)> = Vec::new();
        for (ln, line) in text.lines().enumerate() {
            let line = line.trim(); if line.is_empty() || line.starts_with('#') { continue; }
            let mut it = line.splitn(2, char::is_whitespace);
            let t: f64 = it.next().unwrap().parse().map_err(|_| format!("line {}: bad time", ln + 1))?;
            let after = it.next().unwrap_or("").trim_start();
            let cmd = after.split_whitespace().next().unwrap_or("");
            let rest = after[cmd.len()..].trim();
            let board = self.bus.board_ref();
            let pin = |s: &str| -> Result<u8, String> { board.named_pin(s).map(Ok).unwrap_or_else(|| s.parse().map_err(|_| format!("line {}: bad pin {}", ln + 1, s))) };
            let c = (t * hz) as u64;
            match cmd {
                "press" => { let mut p = rest.split_whitespace(); let pn = pin(p.next().unwrap_or(""))?; let ms: f64 = p.next().map(|x| x.parse().unwrap_or(100.0)).unwrap_or(100.0);
                             ev.push((c, ScriptAction::Gpio(pn, false))); ev.push((c + (ms / 1000.0 * hz) as u64, ScriptAction::Gpio(pn, true))); }
                "release" => ev.push((c, ScriptAction::Gpio(pin(rest)?, true))),
                "gpio" => { let mut p = rest.split_whitespace(); let pn = pin(p.next().unwrap_or(""))?; let l = p.next().unwrap_or("1") == "1"; ev.push((c, ScriptAction::Gpio(pn, l))); }
                "poke" => { let mut p = rest.split_whitespace(); let a = u32::from_str_radix(p.next().unwrap_or("0").trim_start_matches("0x"), 16).map_err(|e| e.to_string())?; let v = u32::from_str_radix(p.next().unwrap_or("0").trim_start_matches("0x"), 16).map_err(|e| e.to_string())?; ev.push((c, ScriptAction::Poke(a, v))); }
                "touch" => { let mut p = rest.split_whitespace(); let x: u16 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0); let y: u16 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0); let d = p.next().unwrap_or("1") == "1"; ev.push((c, ScriptAction::Touch(x, y, d))); }
                "touchpad" => {
                    let mut p = rest.split_whitespace(); let pn = pin(p.next().unwrap_or(""))?;
                    if !self.bus.has_touch_pad(pn) { return Err(format!("line {}: GPIO {} has no capacitive touch pad on this chip", ln + 1, pn)); }
                    let touched = match p.next() { Some("0") => false, Some("1") => true, _ => return Err(format!("line {}: touchpad needs 0 or 1", ln + 1)) };
                    if p.next().is_some_and(|v| !v.starts_with('#')) { return Err(format!("line {}: touchpad takes a GPIO and one value", ln + 1)); }
                    ev.push((c, ScriptAction::TouchPad(pn, touched)));
                }
                "serial" => ev.push((c, ScriptAction::Serial(format!("{}\n", rest)))),
                // adc <gpio> <volts>: constant voltage on an analog pad
                "adc" => { let mut p = rest.split_whitespace(); let pn = pin(p.next().unwrap_or(""))?;
                           let v: f32 = p.next().and_then(|x| x.parse().ok()).ok_or_else(|| format!("line {}: adc <gpio> <volts>", ln + 1))?;
                           ev.push((c, ScriptAction::Analog(pn, esp_periph::AnalogSource::Const(v)))); }
                // adcwave <gpio> <file> <rate_hz>: volts (one per line / comma / whitespace separated) played from this time on
                "adcwave" => { let mut p = rest.split_whitespace(); let pn = pin(p.next().unwrap_or(""))?;
                               let f = p.next().ok_or_else(|| format!("line {}: adcwave <gpio> <file> <rate_hz>", ln + 1))?;
                               let rate: f64 = p.next().and_then(|x| x.parse().ok()).ok_or_else(|| format!("line {}: adcwave needs a rate in Hz", ln + 1))?;
                               let text = std::fs::read_to_string(f).map_err(|e| format!("line {}: {}: {}", ln + 1, f, e))?;
                               let samples: Vec<f32> = text.split(|ch: char| ch == ',' || ch.is_whitespace()).filter(|s| !s.is_empty()).map(|s| s.parse::<f32>()).collect::<Result<_, _>>().map_err(|e| format!("line {}: {}: {}", ln + 1, f, e))?;
                               ev.push((c, ScriptAction::Analog(pn, esp_periph::AnalogSource::Wave { samples: std::sync::Arc::new(samples), rate_hz: rate, start_cycles: c }))); }
                "uart0" | "uart1" => ev.push((c, ScriptAction::Uart(if cmd == "uart0" { 0 } else { 1 }, format!("{}\n", rest)))),
                "knob" => {
                    let mut p = rest.split_whitespace(); let dir = p.next().unwrap_or("cw"); let n: usize = p.next().map(|x| x.parse().unwrap_or(1)).unwrap_or(1);
                    let (clk, dt) = board.encoder().ok_or_else(|| format!("line {}: this board has no encoder", ln + 1))?;
                    let step = (0.002 * hz) as u64;   // 2 ms per quadrature phase
                    let mut tc = c;
                    for _ in 0..n {
                        for (pn, l) in Self::quadrature(clk, dt, dir == "cw") { ev.push((tc, ScriptAction::Gpio(pn, l))); tc += step; }
                        tc += step * 4;
                    }
                }
                "ble" => {
                    rest.parse::<crate::ble::peer::Command>().map_err(|e| format!("line {}: {e}", ln + 1))?;
                    if !self.bus.ble_enabled() { return Err(format!("line {}: BLE requires --ble with the application ELF, or C3 --ble full", ln + 1)); }
                    ev.push((c, ScriptAction::Ble(rest.to_string())));
                },
                "stop" => ev.push((c, ScriptAction::Stop)),
                "waituart0" => { let mut p = rest.splitn(2, char::is_whitespace); let to: f64 = p.next().and_then(|x| x.parse().ok()).ok_or_else(|| format!("line {}: waituart0 <timeout_s> <text>", ln + 1))?;
                                 let text = p.next().unwrap_or("").trim().to_string(); if text.is_empty() { return Err(format!("line {}: waituart0 needs text", ln + 1)); }
                                 ev.push((c, ScriptAction::WaitUart0(text, (to * hz) as u64, c))); }
                _ => return Err(format!("line {}: unknown command {}", ln + 1, cmd)),
            }
        }
        ev.sort_by_key(|e| e.0);
        self.script.events = ev; self.script.pos = 0;
        Ok(())
    }

    // ------------------------------------------------------------------ reports and captures
    /// Write captured I2S audio (left channel) as a 16-bit mono WAV.
    pub fn write_wav(&self, path: &str) -> std::io::Result<usize> {
        let (pcm, rate) = self.bus.audio();
        let mut out = Vec::with_capacity(44 + pcm.len() * 2);
        let data_len = (pcm.len() * 2) as u32;
        out.extend_from_slice(b"RIFF"); out.extend_from_slice(&(36 + data_len).to_le_bytes()); out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes()); out.extend_from_slice(&(rate * 2).to_le_bytes()); out.extend_from_slice(&2u16.to_le_bytes()); out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data"); out.extend_from_slice(&data_len.to_le_bytes());
        for s in pcm { out.extend_from_slice(&s.to_le_bytes()); }
        std::fs::write(path, out)?;
        Ok(pcm.len())
    }

    pub fn irq_report(&self) -> String {
        let mut s = String::from("[irq] per core, cpu-int: count (peripheral sources mapped to it)\n");
        for core in 0..S::CORES {
            for irq in 0..32 {
                let n = self.irq_hist[core][irq];
                if n == 0 { continue; }
                let srcs: Vec<String> = self.bus.irq_sources_of(core, irq as u32).iter().map(|src| src.to_string()).collect();
                s += &format!("  core{} int{:<2} {:>9}  sources [{}]\n", core, irq, n, srcs.join(","));
            }
        }
        s
    }

    /// Save the board's display (scaled) as PNG.
    pub fn write_tft_png(&mut self, path: &str, scale: usize) -> std::io::Result<()> {
        self.bus.refresh_board_outputs();
        let Some((w, h, px, _)) = self.bus.board_ref().display() else { return Err(std::io::Error::other("this board has no display")) };
        png::write_png_rgb565(path, &px, w as usize, h as usize, if w > 200 { 1 } else { scale })
    }
    pub fn write_gram_png(&self, path: &str) -> std::io::Result<()> {
        let Some((px, cols, rows)) = self.bus.board_ref().gram() else { return Err(std::io::Error::other("this board has no TFT")) };
        png::write_png_rgb565(path, &px, cols, rows, 2)
    }

    pub fn disasm(&mut self, addr: u32, n: usize) -> String {
        let mut s = String::new(); let mut pc = addr;
        for _ in 0..n {
            let Ok(b) = self.bus.fetch(pc) else { break };
            let text = self.cores[0].disasm(pc, b);
            let len = S::Core::insn_len(b);
            s += &format!("{:08x}: {:<30} {}\n", pc, text, self.sym(pc));
            pc += len;
        }
        s
    }

    pub fn peek(&mut self, addr: u32, words: usize) -> String {
        let mut s = String::new();
        for i in 0..words { let a = addr.wrapping_add((i * 4) as u32); s += &format!("{:08x}: {}\n", a, match self.bus.read32_unpriced(a) { Ok(v) => format!("{:08x}", v), Err(_) => "--------".into() }); }
        s
    }

    pub fn dump_regs(&self) -> String {
        let sym = |a: u32| self.sym(a);
        let mut out = String::new();
        for (i, c) in self.cores.iter().enumerate() { if i == 0 || !self.core_held[i] { out += &c.dump(i, &sym); } }
        out
    }
}
