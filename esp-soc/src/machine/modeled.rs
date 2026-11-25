//! Event frontiers and recorded accesses for priced execution.
use super::*;
use emu_core::{ExecutionFacts, Fault, MemoryAccessKind, StepKind};

/// Records only accesses made synchronously by `Core::step`. Generated direct-memory access is
/// disabled so every load and store passes through one of the typed methods below.
struct RecordingBus<'a, B> { bus: &'a mut B, accesses: Vec<MemoryAccess> }

impl<'a, B> RecordingBus<'a, B> {
    fn new(bus: &'a mut B, mut accesses: Vec<MemoryAccess>) -> Self {
        accesses.clear();
        Self { bus, accesses }
    }

    fn finish(mut self, bytes: Option<[u8; 4]>, pc: u32) -> Vec<MemoryAccess> {
        if let Some(bytes) = bytes {
            self.accesses.retain(|access| access.kind != MemoryAccessKind::Fetch);
            self.accesses.insert(0, MemoryAccess {
                kind: MemoryAccessKind::Fetch, address: pc, width: 4,
                value: u32::from_le_bytes(bytes), fault: None,
            });
        }
        self.accesses
    }
}

impl<B: Bus> Bus for RecordingBus<'_, B> {
    fn check_atomic32(&mut self, addr: u32, read: bool, write: bool) -> Result<(), Fault> { self.bus.check_atomic32(addr, read, write) }
    fn reserve(&mut self, addr: u32) { self.bus.reserve(addr) }
    fn reservation_held(&mut self, addr: u32) -> bool { self.bus.reservation_held(addr) }
    fn read8(&mut self, address: u32) -> Result<u8, Fault> {
        let result = self.bus.read8(address);
        self.accesses.push(MemoryAccess { kind: MemoryAccessKind::Read, address, width: 1, value: result.unwrap_or(0) as u32, fault: result.err() });
        result
    }
    fn read16(&mut self, address: u32) -> Result<u16, Fault> {
        let result = self.bus.read16(address);
        self.accesses.push(MemoryAccess { kind: MemoryAccessKind::Read, address, width: 2, value: result.unwrap_or(0) as u32, fault: result.err() });
        result
    }
    fn read32(&mut self, address: u32) -> Result<u32, Fault> {
        let result = self.bus.read32(address);
        self.accesses.push(MemoryAccess { kind: MemoryAccessKind::Read, address, width: 4, value: result.unwrap_or(0), fault: result.err() });
        result
    }
    fn write8(&mut self, address: u32, value: u8) -> Result<(), Fault> {
        let result = self.bus.write8(address, value);
        self.accesses.push(MemoryAccess { kind: MemoryAccessKind::Write, address, width: 1, value: value as u32, fault: result.err() });
        result
    }
    fn write16(&mut self, address: u32, value: u16) -> Result<(), Fault> {
        let result = self.bus.write16(address, value);
        self.accesses.push(MemoryAccess { kind: MemoryAccessKind::Write, address, width: 2, value: value as u32, fault: result.err() });
        result
    }
    fn write32(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        let result = self.bus.write32(address, value);
        self.accesses.push(MemoryAccess { kind: MemoryAccessKind::Write, address, width: 4, value, fault: result.err() });
        result
    }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        let result = self.bus.fetch(pc);
        self.accesses.push(MemoryAccess {
            kind: MemoryAccessKind::Fetch, address: pc, width: 4,
            value: result.map(u32::from_le_bytes).unwrap_or(0), fault: result.err(),
        });
        result
    }
    fn page_versions(&self) -> &[u32] { self.bus.page_versions() }
    fn code_page(&mut self, pc: u32) -> u32 { self.bus.code_page(pc) }
    fn note_code_page(&mut self, vidx: u32) { self.bus.note_code_page(vidx); }
    fn note_pc(&mut self, pc: u32) { self.bus.note_pc(pc); }
    fn block_break(&self) -> bool { self.bus.block_break() }
    fn fast_mem(&mut self) -> Option<emu_core::bus::FastMem> { None }
    fn tick(&mut self, cycles: u32) -> u32 { self.bus.tick(cycles) }
}

impl<S: Soc> Machine<S> {
    pub(super) fn run_approximate_jit_frontiers(&mut self, max_insns: u64) -> Stop {
        let (cpi, quantum) = self.approximate_jit_timing.unwrap();
        self.stub_bloom = self.stubs.keys().chain(S::function_hooks(&self.bus)).fold(0, |m, &pc| m | pc_bit(pc));
        self.probe_bloom = self.fn_probes.keys().fold(0, |m, &pc| m | pc_bit(pc));
        for core in &mut self.cores {
            core.set_boundaries(self.stub_bloom | self.probe_bloom);
            core.set_block_observation(self.probes.contains(Wants::BLOCK | Wants::TRAP_PC));
        }
        let trace = self.has_observer("trace");
        let force_idle = self.probes.contains(Wants::NO_IDLE_SKIP);
        let blocks = !self.probes.contains(Wants::INSN);
        let mut on = [false; 4];
        let mut instructions = 0;
        loop {
            if instructions >= max_insns { self.drain_console(); return Stop::MaxInsns; }
            if let Err(stop) = self.settle_modeled_time(&mut on, trace, force_idle) { self.drain_console(); return stop; }
            let now = self.bus.cycles();
            let Some(core) = (0..S::CORES)
                .filter(|&i| on[i] && (force_idle || !self.cores[i].waiting() || self.cores[i].irq_pending()))
                .filter(|&i| self.model_ready_at[i] <= now)
                .min_by_key(|&i| (self.model_ready_at[i], i))
            else { self.drain_console(); return Stop::Halted; };
            // EX139: a core running alone may batch up to the next deadline; device registers are
            // then reached only at settled time (the batch stops in front of them).
            let solo = blocks && self.vq_max > 1 && self.bus.can_defer() && (0..S::CORES).all(|i| i == core || !on[i] || (self.cores[i].waiting() && !self.cores[i].irq_pending()));
            let mut cycles = u64::from(if solo { quantum.max(4096) } else { quantum }) * u64::from(cpi);
            if let Some(delta) = self.bus.next_deadline() { cycles = cycles.min(delta.max(1)); }
            if let Some(&(at, _)) = self.script.events.get(self.script.pos) { cycles = cycles.min(at.saturating_sub(now).max(1)); }
            for (i, &enabled) in on.iter().enumerate().take(S::CORES) {
                if i != core && enabled && self.model_ready_at[i] > now {
                    cycles = cycles.min(self.model_ready_at[i] - now);
                }
            }
            cycles = cycles.min(self.max_cycles - now);
            let budget = cycles.div_ceil(u64::from(cpi)).max(1) as u32;
            // Keep cheap blocks together. A priced memory access yields immediately after its
            // block, exposing the stall to the other CPU without settling devices per ALU block.
            // Retired slots and elapsed cycles have different units: control prices are cycles.
            let (mut retired, mut spent, mut extras) = (0u32, 0u64, 0u32);
            let penalty = loop {
                self.bus.begin_timing_batch(core, now + spent);
                if solo { self.bus.set_defer(spent > 0); }
                let left = (cycles.saturating_sub(spent).div_ceil(u64::from(cpi)).max(1) as u32).min(budget);
                let (done, stop) = if blocks { self.step_blocks(core, left) } else {
                    let stop = self.step_core(core);
                    if !self.cores[core].step_charges_cpi() { self.cores[core].advance_cycles(cpi - 1); }
                    (1, stop)
                };
                if let Some(stop) = stop { self.bus.set_defer(false); self.drain_console(); return stop; }
                if self.bus.sw_reset() {
                    self.bus.set_defer(false);
                    let extra = self.cores[core].take_timing_extra();
                    let penalty = self.bus.take_timing_penalty();
                    let work = retired + done.max(1);
                    self.cores[core].advance_cycles(penalty.saturating_add(extras).saturating_add(extra));
                    self.run_steps += u64::from(work);
                    let cycles = spent + u64::from(done.max(1)) * u64::from(cpi) + u64::from(extra) + u64::from(penalty);
                    return self.finish_reset(cycles);
                }
                let deferred = solo && self.bus.take_deferred();
                // EX138: priced control flow spends the batch's cycles without ending it.
                let extra = self.cores[core].take_timing_extra();
                extras = extras.saturating_add(extra);
                let slots = if deferred { done } else { done.max(1) };
                retired += slots;
                spent += u64::from(slots) * u64::from(cpi) + u64::from(extra);
                let penalty = self.bus.take_timing_penalty();
                if penalty != 0 || spent >= cycles || self.cores[core].waiting() || deferred { break penalty; }
            };
            if solo { self.bus.set_defer(false); }
            self.cores[core].advance_cycles(penalty.saturating_add(extras));
            self.model_ready_at[core] = now + spent.max(u64::from(cpi)) + u64::from(penalty);
            instructions += u64::from(retired.max(1));
            self.run_steps += u64::from(retired.max(1));
            if instructions & 0xffff < u64::from(retired.max(1)) { self.drain_console(); }
        }
    }

    pub(super) fn run_modeled(&mut self, max_insns: u64) -> Stop {
        if let Some(stop) = &self.model_stop { return stop.clone(); }
        self.stub_bloom = self.stubs.keys().chain(S::function_hooks(&self.bus)).fold(0, |mask, &pc| mask | pc_bit(pc));
        self.probe_bloom = self.fn_probes.keys().fold(0, |mask, &pc| mask | pc_bit(pc));
        for core in &mut self.cores { core.set_boundaries(self.stub_bloom | self.probe_bloom); core.flush_caches(); }
        for observer in &mut self.observers { observer.on_modeled_run(); }

        let trace = self.has_observer("trace");
        let force_idle = self.probes.contains(Wants::NO_IDLE_SKIP);
        let mut on = [false; 4];
        let mut events = 0u64;
        loop {
            if let Some(stop) = self.refresh_modeled_core_states(&mut on, trace) {
                self.model_stop = Some(stop.clone()); self.drain_console(); return stop;
            }
            if events >= max_insns { self.drain_console(); return Stop::MaxInsns; }
            if let Err(stop) = self.settle_modeled_time(&mut on, trace, force_idle) {
                if matches!(stop, Stop::CostModelLifecycle { .. }) { self.model_stop = Some(stop.clone()); }
                self.drain_console(); return stop;
            }

            let now = self.bus.cycles();
            let Some(core) = (0..S::CORES)
                .filter(|&i| on[i] && (force_idle || !self.cores[i].waiting() || self.cores[i].irq_pending()))
                .filter(|&i| self.model_ready_at[i] <= now)
                .min_by_key(|&i| (self.model_ready_at[i], i))
            else { self.drain_console(); return Stop::Halted };

            let start = now;
            let pc = self.cores[core].pc();
            self.run_steps += 1;
            let cost = match self.step_core_modeled(core) {
                Ok(cost) => cost,
                Err(stop) => {
                    if matches!(stop, Stop::CostModel { .. } | Stop::CostModelLifecycle { .. }) { self.model_stop = Some(stop.clone()); }
                    self.drain_console(); return stop;
                }
            };
            let Some(ready) = start.checked_add(cost as u64) else {
                let stop = Stop::CostModel { core, pc, reason: "cost model cycle frontier overflow".into() };
                self.model_stop = Some(stop.clone());
                self.drain_console();
                return stop;
            };
            if cost > 1 { self.cores[core].advance_cycles(cost - 1); }
            self.model_ready_at[core] = ready;
            events += 1;

            if let Err(stop) = self.settle_modeled_time(&mut on, trace, force_idle) {
                if matches!(stop, Stop::CostModelLifecycle { .. }) { self.model_stop = Some(stop.clone()); }
                self.drain_console(); return stop;
            }
            if events & 0xffff == 0 { self.drain_console(); }
        }
    }

    fn refresh_modeled_core_states(&mut self, on: &mut [bool; 4], trace: bool) -> Option<Stop> {
        for (i, state) in on.iter_mut().enumerate().take(S::CORES) {
            *state = match S::core_state(&self.bus, i) {
                CoreState::Reset => { self.core_held[i] = true; false }
                CoreState::Held => false,
                CoreState::Running => {
                    if self.core_held[i] {
                        self.core_held[i] = false;
                        S::reset_core_with_bus(&mut self.cores[i], &mut self.bus, i);
                        self.model_ready_at[i] = self.bus.cycles();
                        if trace { eprintln!("          ** core{} released from reset", i); }
                        let facts = LifecycleFacts { kind: LifecycleKind::CoreReset(i), chip: S::NAME, cores: S::CORES, cpu_hz: S::CPU_HZ };
                        if let Some(model) = &mut self.cost {
                            if let Err(reason) = model.lifecycle(&facts) { return Some(Stop::CostModelLifecycle { kind: facts.kind, reason }); }
                        }
                    }
                    true
                }
            };
        }
        None
    }

    /// Advance the shared device horizon until at least one active core can start an event.
    fn settle_modeled_time(&mut self, on: &mut [bool; 4], trace: bool, force_idle: bool) -> Result<(), Stop> {
        loop {
            if let Some(stop) = self.refresh_modeled_core_states(on, trace) { return Err(stop); }
            self.after_round_rest();
            self.refresh_irq();
            if self.bus.sw_reset() { return Err(Stop::SwReset); }
            let now = self.bus.cycles();
            if now >= self.max_cycles { return Err(Stop::Halted); }

            if let Some(stop) = self.observe_idle_pcs(&on[..S::CORES]) { return Err(stop); }
            let next_core = (0..S::CORES)
                .filter(|&i| on[i] && (force_idle || !self.cores[i].waiting() || self.cores[i].irq_pending()))
                .map(|i| self.model_ready_at[i].max(now))
                .min();
            if next_core == Some(now) { return Ok(()); }

            let mut target = next_core;
            if let Some(delta) = self.bus.next_deadline() {
                let deadline = now.saturating_add(delta.max(1));
                target = Some(target.map_or(deadline, |current| current.min(deadline)));
            }
            if let Some(&(deadline, _)) = self.script.events.get(self.script.pos) {
                let deadline = deadline.max(now.saturating_add(1));
                target = Some(target.map_or(deadline, |current| current.min(deadline)));
            }
            for (i, &enabled) in on.iter().enumerate().take(S::CORES) {
                if enabled && self.cores[i].waiting() && !self.cores[i].irq_pending() {
                    if let Some(delta) = self.cores[i].cycles_until_wake() {
                        let deadline = self.model_ready_at[i].max(now).saturating_add(delta.max(1));
                        target = Some(target.map_or(deadline, |current| current.min(deadline)));
                    }
                }
            }
            let Some(target) = target.map(|target| target.min(self.max_cycles)) else { return Err(Stop::Halted); };
            if target <= now { return Err(Stop::Halted); }
            self.advance_modeled_time(target, on);
        }
    }

    fn advance_modeled_time(&mut self, target: u64, on: &[bool; 4]) {
        for (i, &enabled) in on.iter().enumerate().take(S::CORES) {
            if enabled && self.cores[i].waiting() && !self.cores[i].irq_pending() && self.model_ready_at[i] < target {
                let mut remaining = target - self.model_ready_at[i];
                while remaining != 0 {
                    let step = remaining.min(u32::MAX as u64) as u32;
                    self.cores[i].advance_cycles(step);
                    remaining -= step as u64;
                }
                self.model_ready_at[i] = target;
            }
        }
        while self.bus.cycles() < target {
            let step = (target - self.bus.cycles()).min(u32::MAX as u64);
            self.after_round(step);
        }
    }

    fn step_core_modeled(&mut self, core: usize) -> Result<u32, Stop> {
        S::select_core(&mut self.bus, core);
        let pc = self.cores[core].pc();
        if self.probe_bloom & pc_bit(pc) != 0 && !self.cores[core].waiting() {
            if let Some(name) = self.fn_probes.get(&pc) {
                let cpu = &self.cores[core];
                let (args, ret) = (cpu.probe_args(&mut self.bus), cpu.return_address(&mut self.bus));
                eprintln!("[fn] i={} t={:.4}s c{} {}({}) ret={:#x}", cpu.insn_count(), self.bus.cycles() as f64 / S::CPU_HZ as f64, core, name, args, ret);
            }
        }
        if self.stub_bloom & pc_bit(pc) != 0 && !self.cores[core].waiting() && (self.stubs.contains_key(&pc) || S::function_hooks(&self.bus).contains(&pc)) {
            return Err(Stop::CostModel { core, pc, reason: "function stubs are unsupported by modeled execution".into() });
        }
        {
            let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
            for observer in &mut self.observers {
                if observer.wants().contains(Wants::INSN) {
                    if let Some(stop) = observer.on_insn(&cx, core, &self.cores[core], &mut self.bus, pc) { S::observer_stopped(&mut self.bus); return Err(stop); }
                }
            }
        }

        self.bus.note_pc(pc);
        let (outcome, accesses) = {
            let mut bus = RecordingBus::new(&mut self.bus, std::mem::take(&mut self.model_accesses));
            let mut outcome = self.cores[core].step(&mut bus);
            // A control operation is an occurrence, not a decoded intention. Current cores can
            // report it before a privilege or execution failure, so only retirement commits it.
            if !matches!(outcome.kind, StepKind::Retired) { outcome.control = None; }
            let accesses = bus.finish(outcome.bytes, outcome.pc);
            (outcome, accesses)
        };
        // A cost-model event is not an instruction block; retain its explicit unavailable
        // BLOCK contract while sharing trap delivery and accounting with ordinary execution.
        if let Some(stop) = self.observe_execution(core, pc, 0, outcome.trap()) { return Err(stop); }
        self.refresh_irq();
        {
            let cx = Ctx { symbols: &self.symbols, cycles: self.bus.cycles(), cpu_hz: S::CPU_HZ };
            for observer in &mut self.observers {
                if observer.wants().contains(Wants::INSN) {
                    if let Some(stop) = observer.after_insn(&cx, core, &self.cores[core], &mut self.bus) { S::observer_stopped(&mut self.bus); return Err(stop); }
                }
            }
        }
        if self.probes.0 != 0 { self.deliver_events(); }
        if self.bus.sw_reset() { return Err(Stop::SwReset); }
        if self.exceptions >= self.dbg.stop_after_exceptions { return Err(Stop::Exceptions(self.exceptions)); }

        let facts = ExecutionFacts { core, outcome, accesses: &accesses };
        let result = self.cost.as_mut().expect("modeled path requires an attached model").cycles_at(&facts, self.bus.cycles());
        self.model_accesses = accesses;
        match result {
            Ok(0) => Err(Stop::CostModel { core, pc, reason: "cost model returned zero cycles".into() }),
            Ok(cycles) => Ok(cycles),
            Err(reason) => Err(Stop::CostModel { core, pc, reason }),
        }
    }
}
