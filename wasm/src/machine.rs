//! Shared machine operations and explicit chip dispatch for the browser ABI.
use super::log;
use esp_soc::observers::{BlockProfile, Coverage, IrqLatency};
use esp_soc::web::{json_escape, WebServer};
use esp_soc::{Machine, Soc, SocBus, Stop};

/// The chip-neutral operations the ABI uses.
pub(super) trait MachineApi {
    fn bus(&mut self) -> &mut dyn SocBus;
    fn load(&mut self, kind: u32, d: &[u8]) -> Result<(), String>;
    fn write_flash(&mut self, off: usize, d: &[u8]) -> Result<(), String>;
    fn boot(&mut self, app_direct: bool) -> Result<(), String>;
    fn board_name(&self) -> String;
    /// The chip, as the `Soc` names it (`esp32s3`, `esp32c3`, `esp32c6`, `esp32`, `esp32p4`).
    fn chip(&self) -> &'static str;
    fn web(&self) -> Option<&WebServer>;
    fn run_slice(&mut self, cycles: u32) -> u32;
    fn cpu_hz(&self) -> f64;
    fn cycles(&self) -> f64;
    fn insns(&self) -> f64;
    fn stub(&mut self, name: &str, value: u32) -> u32;
    fn observer(&mut self, name: &str, arg: &str) -> u32;
    fn reports(&mut self) -> String;
    fn set_jit(&mut self, enabled: bool);
    fn approximate_jit_timing(&mut self, cpi: u32, quantum: u32) -> u32;
    /// The virtual access point, on the chip with the radio.
    fn attach_wifi(&mut self, cfg: esp_soc::wifi::ApConfig) -> Result<(), String> { self.bus().attach_wifi(cfg, None) }
}

impl<S: Soc> MachineApi for Machine<S> {
    fn bus(&mut self) -> &mut dyn SocBus { &mut self.bus }
    fn load(&mut self, kind: u32, d: &[u8]) -> Result<(), String> {
        self.load_input(esp_soc::LoadKind::try_from(kind)?, d)
    }
    fn write_flash(&mut self, off: usize, d: &[u8]) -> Result<(), String> { Machine::write_flash(self, off, d) }
    fn boot(&mut self, app_direct: bool) -> Result<(), String> { if app_direct { self.boot_app(0x10000).map(|_| ()) } else { self.boot_rom(); Ok(()) } }
    fn board_name(&self) -> String { self.bus.board_ref().name().to_string() }
    fn chip(&self) -> &'static str { S::NAME }
    fn web(&self) -> Option<&WebServer> { self.web.as_ref() }
    fn run_slice(&mut self, cycles: u32) -> u32 {
        self.max_cycles = self.bus.cycles() + cycles as u64;
        loop {
            match self.run(u64::MAX) {
                Stop::Halted | Stop::MaxInsns => return 0,
                Stop::SwReset => reboot(self),
                Stop::Unimplemented(pc, raw) => { log(&format!("[emu] unimplemented instruction at {:08x} {} (raw {:#x})", pc, self.sym(pc), raw)); return 2; }
                Stop::Ebreak(pc) => { log(&format!("[emu] ebreak at {:08x} {}", pc, self.sym(pc))); return 3; }
                Stop::Breakpoint(_) => return 3,
                Stop::Exceptions(_) => return 4,
                Stop::Simcall(_) => return 5,
                Stop::Watch(..) => return 6,
                Stop::CostModel { reason, .. } | Stop::CostModelLifecycle { reason, .. } => { log(&format!("[emu] cost model: {}", reason)); return 7; }
            }
        }
    }
    fn cpu_hz(&self) -> f64 { S::CPU_HZ as f64 }
    fn cycles(&self) -> f64 { self.bus.cycles() as f64 }
    fn insns(&self) -> f64 { Machine::insns(self) as f64 }
    fn stub(&mut self, name: &str, value: u32) -> u32 {
        match self.resolve_stub(name) {
            Some(addr) => { self.stubs.insert(addr, value); log(&format!("[emu] stub {} @ {:#x} -> returns {:#x}", name, addr, value)); 0 }
            None => { log(&format!("[emu] stub: no symbol '{}' (load the app ELF first)", name)); 1 }
        }
    }
    fn observer(&mut self, name: &str, arg: &str) -> u32 {
        match name {
            "profile-blocks" => { self.add_observer(Box::new(BlockProfile::new(20))); 0 }
            "coverage" => { self.add_observer(Box::new(Coverage::new(None))); 0 }
            "irq-latency" => { self.add_observer(Box::new(IrqLatency::new(S::CORES))); 0 }
            "trace-fn" => { self.trace_fns(arg); 0 }
            _ => { log(&format!("[emu] unknown observer '{}'", name)); 1 }
        }
    }
    fn reports(&mut self) -> String { Machine::reports(self) }
    fn approximate_jit_timing(&mut self, cpi: u32, quantum: u32) -> u32 {
        match self.set_approximate_jit_timing(cpi, quantum) { Ok(()) => 0, Err(reason) => { log(&reason); 1 } }
    }
    fn set_jit(&mut self, enabled: bool) { for core in &mut self.cores { xtensa_lx7::Core::set_jit(core, enabled); } }
}

/// A load kind or flash offset with this bit set goes to the co-processor (`esp32sim_load`,
/// `esp32sim_load_at`).
pub(super) const COPROC_KIND: u32 = 0x100;
pub(super) const COPROC_FLASH: u32 = 0x8000_0000;

/// The P4 and its ESP32-C6 co-processor in lock-step over SDIO (`esp32p4::coproc`, what the CLI's
/// `--coproc` runs). The page sees the P4: its clock, console and board; the C6's console goes out
/// as the `c6` source, line by line, and its virtual access point is the C6's. A stub named
/// `c6:NAME` is the C6's.
impl<R: esp32p4::Revision> MachineApi for esp32p4::coproc::CoSim<R> {
    fn bus(&mut self) -> &mut dyn SocBus { &mut self.p4.bus }
    fn load(&mut self, kind: u32, d: &[u8]) -> Result<(), String> {
        if kind & COPROC_KIND != 0 { self.c6.borrow_mut().m.load_input(esp_soc::LoadKind::try_from(kind & !COPROC_KIND)?, d) }
        else { self.p4.load_input(esp_soc::LoadKind::try_from(kind)?, d) }
    }
    fn write_flash(&mut self, off: usize, d: &[u8]) -> Result<(), String> {
        let c6 = off as u32 & COPROC_FLASH != 0;
        if c6 { self.c6.borrow_mut().m.write_flash(off & !(COPROC_FLASH as usize), d) } else { Machine::write_flash(&mut self.p4, off, d) }
    }
    /// Both chips from their ROMs: the co-processor has no other way in.
    fn boot(&mut self, app_direct: bool) -> Result<(), String> {
        if app_direct { return Err("the P4 and its co-processor boot from their ROMs".into()); }
        self.p4.boot_rom();
        self.c6.borrow_mut().m.boot_rom();
        Ok(())
    }
    fn board_name(&self) -> String { if R::WAFER.0 >= 3 { "esp32p4-rev3-c6".into() } else { "esp32p4-c6".into() } }
    fn chip(&self) -> &'static str { "esp32p4" }
    fn web(&self) -> Option<&WebServer> { self.p4.web.as_ref() }
    /// `cycles` of the P4 as nanoseconds of the shared clock; the pair runs whole slices.
    fn run_slice(&mut self, cycles: u32) -> u32 {
        let end = self.now_ns + (cycles as u64 * 25).div_ceil(9);
        let r = self.run_until(end);
        // The P4's console reached the page as the machine printed it; the C6's is collected here.
        let lines = std::mem::take(&mut self.lines);
        if let Some(w) = &self.p4.web {
            for (_, line) in lines.iter().filter(|(c6, _)| *c6) {
                w.send_text(&format!("{{\"t\":\"serial\",\"src\":\"c6\",\"data\":\"{}\\n\"}}", json_escape(line)));
            }
        }
        match r {
            Ok(()) | Err(Stop::Halted) | Err(Stop::MaxInsns) => 0,
            Err(Stop::Unimplemented(pc, raw)) => { log(&format!("[emu] p4: unimplemented instruction at {:08x} {} (raw {:#x})", pc, self.p4.sym(pc), raw)); 2 }
            Err(Stop::Ebreak(_) | Stop::Breakpoint(_)) => 3,
            Err(Stop::Exceptions(_)) => 4,
            Err(s) => { log(&format!("[emu] p4: stopped: {s:?}")); 7 }
        }
    }
    fn cpu_hz(&self) -> f64 { <esp32p4::Esp32p4<R> as Soc>::CPU_HZ as f64 }
    fn cycles(&self) -> f64 { self.p4.bus.cycles() as f64 }
    fn insns(&self) -> f64 { (self.p4.insns() + self.c6.borrow().m.insns()) as f64 }
    fn stub(&mut self, name: &str, value: u32) -> u32 {
        match name.strip_prefix("c6:") { Some(n) => MachineApi::stub(&mut self.c6.borrow_mut().m, n, value), None => MachineApi::stub(&mut self.p4, name, value) }
    }
    fn observer(&mut self, name: &str, arg: &str) -> u32 { MachineApi::observer(&mut self.p4, name, arg) }
    fn reports(&mut self) -> String { Machine::reports(&mut self.p4) }
    fn set_jit(&mut self, enabled: bool) { MachineApi::set_jit(&mut self.p4, enabled); MachineApi::set_jit(&mut self.c6.borrow_mut().m, enabled); }
    fn approximate_jit_timing(&mut self, _cpi: u32, _quantum: u32) -> u32 { log("[emu] approximate timing: not with a co-processor"); 1 }
    fn attach_wifi(&mut self, cfg: esp_soc::wifi::ApConfig) -> Result<(), String> { self.c6.borrow_mut().m.bus.attach_wifi(cfg, None) }
}

/// Chip-specific ABI calls borrow an S3 directly; no runtime type erasure or downcasts.
pub(super) enum MachineKind {
    S3(Box<esp32s3::Machine>),
    C3(Box<esp32c3::Machine>),
    C6(Box<esp32c6::Machine>),
    Esp32(Box<esp32::Machine>),
    P4(Box<esp32p4::Machine>),
    P4Rev3(Box<esp_soc::Machine<esp32p4::P4Rev3>>),
    P4C6(Box<esp32p4::coproc::CoSim<esp32p4::RevLt3>>),
    P4Rev3C6(Box<esp32p4::coproc::CoSim<esp32p4::RevGe3>>),
}

impl MachineKind {
    /// Read-only primary-chip flash export; a co-processor has its own flash and is not
    /// included. No guest instruction, tick or cache state changes.
    pub fn flash(&self) -> &[u8] {
        match self { Self::S3(m) => &m.bus.flash, Self::C3(m) => &m.bus.flash, Self::C6(m) => &m.bus.flash, Self::Esp32(m) => &m.bus.flash, Self::P4(m) => &m.bus.flash,
            Self::P4Rev3(m) => &m.bus.flash, Self::P4C6(m) => &m.p4.bus.flash, Self::P4Rev3C6(m) => &m.p4.bus.flash }
    }
    pub fn s3_mut(&mut self) -> Option<&mut esp32s3::Machine> {
        match self { Self::S3(m) => Some(m), _ => None }
    }
}

impl std::ops::Deref for MachineKind {
    type Target = dyn MachineApi;
    fn deref(&self) -> &Self::Target {
        match self { Self::S3(m) => m.as_ref(), Self::C3(m) => m.as_ref(), Self::C6(m) => m.as_ref(), Self::Esp32(m) => m.as_ref(), Self::P4(m) => m.as_ref(),
            Self::P4Rev3(m) => m.as_ref(), Self::P4C6(m) => m.as_ref(), Self::P4Rev3C6(m) => m.as_ref() }
    }
}

impl std::ops::DerefMut for MachineKind {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self { Self::S3(m) => m.as_mut(), Self::C3(m) => m.as_mut(), Self::C6(m) => m.as_mut(), Self::Esp32(m) => m.as_mut(), Self::P4(m) => m.as_mut(),
            Self::P4Rev3(m) => m.as_mut(), Self::P4C6(m) => m.as_mut(), Self::P4Rev3C6(m) => m.as_mut() }
    }
}

/// Report software reset once for both ordinary slices and receipt-priced sidecar commits.
pub(super) fn reboot<S: Soc>(machine: &mut Machine<S>) {
    let cause = machine.bus.reset_cause();
    let note = format!("[emu] chip reset at t={:.3}s: cause {:#x} ({})",
        machine.seconds(), cause, esp_periph::reset_cause_name(cause));
    log(&note);
    if let Some(web) = &machine.web {
        web.send_text(&format!("{{\"t\":\"emu\",\"msg\":\"{}\"}}", json_escape(&note)));
    }
    machine.reboot();
}
