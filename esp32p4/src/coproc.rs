//! The ESP32-P4 and an ESP32-C6 co-processor joined over SDIO, as on boards that give the P4
//! WiFi through ESP-Hosted: two machines in one process, stepped in lock-step on one clock.
//!
//! Time is nanoseconds from power-on, cut into slices of `Config::slice_ns` on a fixed grid. The P4
//! runs a slice to its end, then the C6 catches up to the same instant, then the lines between the
//! chips are exchanged. The SDIO bus is host-driven, so each transaction is where the two meet
//! exactly: when the P4's SDMMC host starts a command, the C6 first runs to that nanosecond
//! (`SdioLink`), the command and its data are carried out against the C6's slave as it stands
//! then, and the C6 continues from there in its next run. The C6 never runs ahead of the P4. What
//! the P4 senses without a command, the slave's interrupt on DAT1, is sampled at every command and
//! at every slice end, so it reaches the P4 at most a slice late, as a frame reaches csim's medium
//! at the end of the slice it started in (`--cooja`). The C6's EN pin follows a P4 GPIO, read at
//! slice ends: driven low it holds the C6 in reset, released (high, or not driven: the pull-up) it
//! powers the C6 up again from its ROM.
//!
//! Nothing reads a host clock or host randomness, and the order of every step is fixed, so a run
//! is the same to the instruction every time, however `run_until` is called.
use esp32c6::Machine as C6Machine;
use esp_periph::sdmmc::{SdCmdError, SdDataError, SdDevice, SdResponse};
use esp_soc::{RunUntil, SocBus, Stop};
use std::cell::RefCell;
use std::rc::Rc;

/// 360 MHz: 9 P4 cycles in 25 ns.
pub fn p4_cycle_of_ns(ns: u64) -> u64 { (ns as u128 * 9).div_ceil(25) as u64 }
/// 160 MHz: 4 C6 cycles in 25 ns. The C6 runs to the last cycle not after `ns`.
pub fn c6_cycle_of_ns(ns: u64) -> u64 { (ns as u128 * 4 / 25) as u64 }

/// How a co-simulation runs.
#[derive(Clone, Debug)]
pub struct Config {
    /// how long the P4 runs before the C6 catches up and the lines are exchanged; a multiple of
    /// 25 ns puts every slice end on a cycle of both chips
    pub slice_ns: u64,
    /// the P4 GPIO the C6's EN pin follows, and whether it is inverted (high holds the C6 in reset)
    pub en_pin: u8,
    pub en_inverted: bool,
    /// bring a chip back up through its ROM after it resets itself
    pub reboot_p4: bool,
    pub reboot_c6: bool,
    /// which P4 consoles go to the output: bit0 USB-Serial/JTAG, bit1 UART0, bit2 UART1
    pub p4_console: u32,
    /// the C6's consoles, the same way
    pub c6_console: u32,
}

/// ESP-Hosted's reset line on the reference board is GPIO 54 (esp_hosted 2.12.13 Kconfig,
/// ESP_HOSTED_SDIO_GPIO_RESET_SLAVE, default 54 on the ESP32-P4), active high: low resets
/// (CONFIG_ESP_HOSTED_SDIO_RESET_ACTIVE_HIGH, "Low will trigger reset").
pub const DEFAULT_EN_PIN: u8 = 54;
/// The shortest EN low that resets the C6: t_RST, 50 µs (ESP32-C6 Series Datasheet, "Chip
/// Power-up and Reset"). A GPIO set to output before its level is written can glitch EN for a
/// slice.
pub const EN_RESET_NS: u64 = 50_000;
/// 10 µs: an SDIO interrupt waits at most this long for the P4 to see it.
pub const DEFAULT_SLICE_NS: u64 = 10_000;

impl Default for Config {
    fn default() -> Self { Config { slice_ns: DEFAULT_SLICE_NS, en_pin: DEFAULT_EN_PIN, en_inverted: false, reboot_p4: true, reboot_c6: true, p4_console: 2, c6_console: 2 } }
}

/// The C6 and where it stands on the shared clock.
pub struct Coproc {
    pub m: C6Machine,
    /// the C6's machine cycle 0 on the shared clock, in C6 cycles: a power-up after EN moves it
    origin: u64,
    /// EN is low: the chip is held in reset and runs nothing
    pub held: bool,
    /// when EN went low
    held_at: u64,
    /// EN pulses shorter than a reset takes, which the chip rode out
    pub en_glitches: u64,
    /// why the C6 stopped for good, if it did
    pub halted: Option<String>,
    reboot: bool,
    console_mask: u32,
    /// console bytes not yet collected
    pub console: Vec<u8>,
    /// power-ups after EN, chip resets the C6 asked for
    pub power_ups: u64,
    pub resets: u64,
    /// commands whose time the C6 had already passed: the two clocks disagree
    pub late_commands: u64,
    /// the faults the link injects, and their state
    pub faults: FaultState,
}

impl Coproc {
    pub fn new(m: C6Machine) -> Self {
        Coproc { m, origin: 0, held: false, held_at: 0, en_glitches: 0, halted: None, reboot: true, console_mask: 2, console: Vec::new(), power_ups: 0, resets: 0, late_commands: 0, faults: FaultState::default() }
    }

    /// Where the C6 stands on the shared clock, in its own cycles.
    pub fn now_cycles(&self) -> u64 { self.origin + self.m.bus.cycles() }

    pub fn running(&self) -> bool { !self.held && self.halted.is_none() }

    /// The C6 machine cycle (its own count, from its first power-up) at shared time `ns`.
    fn machine_cycle_of_ns(&self, ns: u64) -> u64 { c6_cycle_of_ns(ns).saturating_sub(self.origin) }

    /// Run the C6 to the last of its cycles at or before `ns`, stopping where a data phase the
    /// host started hands its descriptors back. A chip in reset or stopped stays where it is.
    pub fn advance_to(&mut self, ns: u64) {
        let target = c6_cycle_of_ns(ns);
        if self.faults.wedged {
            self.m.bus.sdio_apply_all();
            return;
        }
        loop {
            if self.m.bus.sdio_next_due().is_some_and(|d| d <= self.m.bus.cycles()) { self.m.bus.sdio_apply_due(); }
            if !self.running() || self.now_cycles() >= target { break; }
            let end = target - self.origin;
            let r = self.m.run_until_cycle(self.m.bus.sdio_next_due().map_or(end, |d| d.min(end)));
            self.collect_console();
            match r {
                RunUntil::Reached | RunUntil::Yield => {}
                RunUntil::Stop(Stop::SwReset) if self.reboot => { self.m.reboot(); self.resets += 1; }
                RunUntil::Stop(s) => self.halted = Some(format!("{s:?}")),
            }
        }
        self.collect_console();
    }

    /// EN low at `ns`: the C6 runs up to then and stops.
    fn hold(&mut self, ns: u64) {
        self.advance_to(ns);
        (self.held, self.held_at) = (true, ns);
    }

    /// EN high at `ns`: a power-on reset, the chip starting again from its ROM, if EN was low
    /// long enough to reset it; after a shorter pulse the chip runs on.
    fn release(&mut self, ns: u64) {
        self.held = false;
        if ns - self.held_at < EN_RESET_NS { self.en_glitches += 1; return; }
        if self.halted.is_some() { return; }
        self.m.reboot();
        self.m.bus.set_reset_cause(esp_periph::RST_POWERON);
        self.origin = c6_cycle_of_ns(ns).saturating_sub(self.m.bus.cycles());
        self.power_ups += 1;
    }

    fn collect_console(&mut self) {
        // what the machine drained itself on a stop, then what is still in the UARTs
        for (bit, backlog) in [(1, std::mem::take(&mut self.m.console.usb)), (2, std::mem::take(&mut self.m.console.uart0))] {
            if self.console_mask & bit != 0 { self.console.extend_from_slice(&backlog); }
        }
        self.m.console.all.clear();
        let streams = self.m.bus.console_take();
        for (i, s) in streams.iter().enumerate().take(3) {
            if self.console_mask & (1 << i) != 0 { self.console.extend_from_slice(s); }
        }
    }
}

/// The C6's SDIO slave as a device on the P4's SD bus. A command first brings the C6 to the
/// command's time.
pub struct SdioLink(pub Rc<RefCell<Coproc>>);

impl SdDevice for SdioLink {
    fn command(&mut self, now: u64, cmd: u8, arg: u32) -> Result<SdResponse, SdCmdError> {
        let mut c = self.0.borrow_mut();
        if c6_cycle_of_ns(now) < c.now_cycles() { c.late_commands += 1; }
        c.advance_to(now);
        if !c.running() || c.faults.dead { return Err(SdCmdError::NoResponse); }
        let fault = c.faults.on_command(now, cmd, arg);
        if fault == Some(FaultKind::CmdTimeout) { return Err(SdCmdError::NoResponse); }
        let r = c.m.bus.sdio_command(cmd, arg);
        c.m.sync_irq();
        match (fault, r) {
            (Some(FaultKind::RespCrc), Ok(resp)) => Err(SdCmdError::Crc(resp)),
            (_, r) => r,
        }
    }
    /// The bytes move now; the slave hands its descriptors back when the C6 reaches `now`, the
    /// end of the data phase on the bus (at once unless the bus is timed or the C6 is behind).
    fn read_data(&mut self, now: u64, buf: &mut [u8]) -> Result<(), SdDataError> {
        let mut c = self.0.borrow_mut();
        if !c.running() || c.faults.dead { return Err(SdDataError::Timeout); }
        let done = c.machine_cycle_of_ns(now);
        let r = c.m.bus.sdio_read_data(buf, done);
        c.m.sync_irq();
        r?;
        let last = c.m.bus.periph.optional.sdio.transfer_left() == 0;
        match c.faults.on_read_block(buf.len(), last) {
            Some(FaultKind::ReadTimeout) => Err(SdDataError::Timeout),
            Some(_) => Err(SdDataError::Crc),
            None => Ok(()),
        }
    }
    fn write_data(&mut self, now: u64, data: &[u8]) -> Result<(), SdDataError> {
        let mut c = self.0.borrow_mut();
        if !c.running() || c.faults.dead { return Err(SdDataError::NoCrcStatus); }
        let fault = c.faults.on_write_block();
        if fault == Some(FaultKind::WriteCrc) { return Err(SdDataError::Crc); }
        let done = c.machine_cycle_of_ns(now);
        let r = c.m.bus.sdio_write_data(data, done);
        c.m.sync_irq();
        if fault == Some(FaultKind::WriteNoStatus) { return Err(SdDataError::NoCrcStatus); }
        r
    }
    fn bus_width(&self) -> Option<u8> { let c = self.0.borrow(); (c.running() && !c.faults.dead).then(|| c.m.bus.sdio_bus_width()) }
    fn interrupt(&self) -> bool {
        let c = self.0.borrow();
        c.running() && !c.faults.dead && (c.faults.spurious || c.m.bus.sdio_interrupt())
    }
    fn edge_lost(&self) -> bool { self.0.borrow().faults.drop_edge.take() }
    fn io(&self) -> bool { true }
    fn name(&self) -> &str { "esp32c6" }
}

/// A fault the link injects. Each fires at a point the run itself fixes, the Nth command or data
/// transfer of its kind or a shared time, never a host clock or randomness, so a run with faults
/// is as reproducible as one without.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultKind {
    /// the Nth CMD53 read of function 1's FIFO: the slave gives its data (all of it, or up to byte
    /// K), the host sees a data CRC error
    ReadCrc,
    /// the same with a data read timeout
    ReadTimeout,
    /// the Nth CMD53 read of the register window (ESP-Hosted's combined INT_RAW..PKT_LEN read)
    /// fails its CRC
    RegReadCrc,
    /// the Nth CMD53 write to the FIFO: the card reports a CRC error and drops the block
    WriteCrc,
    /// the card takes the Nth write's data, its CRC status never comes
    WriteNoStatus,
    /// the Nth command never reaches the slave: a response timeout
    CmdTimeout,
    /// the slave carries the Nth command out; its response fails its CRC
    RespCrc,
    /// from a time on the C6's CPU stops; its SDIO hardware goes on answering, TOKEN and PKT_LEN
    /// frozen with it
    Wedge,
    /// from a time on nothing answers on the bus
    DeadBus,
    /// the first DAT1 assertion from a time on goes unlatched by the host (D1 still low)
    DropIrq,
    /// D1 held low for a while with nothing pending
    SpuriousIrq,
    /// TOKEN1, as the host reads it, shows no free receive buffer for a while (the buffers the
    /// host has filled, none since loaded), while the driver goes on loading them
    TokenStall,
}

/// When a fault fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// the Nth (from 1) of its kind; for a read, `byte` limits the data given before the error
    Nth { n: u64, byte: Option<u32> },
    /// from `ns` on the shared clock, for `len` ns where the fault is a window
    At { ns: u64, len: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fault { pub kind: FaultKind, pub when: When }

const FAULT_NAMES: [(&str, FaultKind); 12] = [
    ("read-crc", FaultKind::ReadCrc), ("read-timeout", FaultKind::ReadTimeout), ("reg-read-crc", FaultKind::RegReadCrc),
    ("write-crc", FaultKind::WriteCrc), ("write-nostatus", FaultKind::WriteNoStatus), ("cmd-timeout", FaultKind::CmdTimeout),
    ("resp-crc", FaultKind::RespCrc), ("wedge", FaultKind::Wedge), ("dead-bus", FaultKind::DeadBus), ("drop-irq", FaultKind::DropIrq),
    ("spurious-irq", FaultKind::SpuriousIrq), ("token-stall", FaultKind::TokenStall),
];

impl FaultKind {
    pub fn name(self) -> &'static str { FAULT_NAMES.iter().find(|f| f.1 == self).unwrap().0 }
    /// Counted (the Nth transfer or command) rather than timed.
    pub fn counted(self) -> bool {
        matches!(self, FaultKind::ReadCrc | FaultKind::ReadTimeout | FaultKind::RegReadCrc | FaultKind::WriteCrc | FaultKind::WriteNoStatus | FaultKind::CmdTimeout | FaultKind::RespCrc)
    }
}

/// `1500us`, `2ms`, `3.25s`: emulated nanoseconds.
fn parse_ns(t: &str) -> Option<u64> {
    let (num, scale) = if let Some(n) = t.strip_suffix("us") { (n, 1e3) } else if let Some(n) = t.strip_suffix("ms") { (n, 1e6) } else { (t.strip_suffix('s')?, 1e9) };
    let v: f64 = num.parse().ok()?;
    (v >= 0.0).then(|| (v * scale).round() as u64)
}

impl Fault {
    /// `KIND@N[:K]` for the counted kinds (`read-crc@40`, `read-crc@40:1024`), `KIND@TIME[+LEN]`
    /// for the timed ones (`wedge@4s`, `token-stall@3.5s+200ms`).
    pub fn parse(spec: &str) -> Result<Fault, String> {
        let (name, when) = spec.split_once('@').ok_or_else(|| format!("{spec}: expected KIND@WHEN"))?;
        let kind = FAULT_NAMES.iter().find(|f| f.0 == name).map(|f| f.1)
            .ok_or_else(|| format!("{name}: unknown fault ({})", FAULT_NAMES.iter().map(|f| f.0).collect::<Vec<_>>().join(", ")))?;
        let when = if kind.counted() {
            let (n, byte) = match when.split_once(':') { Some((n, k)) => (n, Some(k.parse().map_err(|_| format!("{spec}: bad byte {k}"))?)), None => (when, None) };
            When::Nth { n: n.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("{spec}: expected a count from 1"))?, byte }
        } else {
            let (t, len) = when.split_once('+').map_or((when, None), |(t, l)| (t, Some(l)));
            let ns = parse_ns(t).ok_or_else(|| format!("{spec}: expected a time such as 3.5s or 200ms"))?;
            let len = match len { Some(l) => parse_ns(l).ok_or_else(|| format!("{spec}: bad length {l}"))?, None => 0 };
            When::At { ns, len }
        };
        Ok(Fault { kind, when })
    }
}

/// The faults of a run and where they stand.
#[derive(Debug, Default)]
pub struct FaultState {
    pub plan: Vec<Fault>,
    /// what fired, when: (fault, shared ns)
    pub fired: Vec<(Fault, u64)>,
    commands: u64,
    fifo_reads: u64,
    reg_reads: u64,
    writes: u64,
    now: u64,
    /// the data phase in progress fails: its kind, and the bytes still given before the error
    data: Option<(FaultKind, u32)>,
    pub wedged: bool,
    pub dead: bool,
    pub spurious: bool,
    drop_edge: std::cell::Cell<bool>,
    /// which timed faults are on
    active: Vec<bool>,
}

impl FaultState {
    fn fire(&mut self, f: Fault) { self.fired.push((f, self.now)); }

    /// A command at `now`: the counted command faults, and the CMD53 data faults armed for the
    /// data phase it starts.
    fn on_command(&mut self, now: u64, cmd: u8, arg: u32) -> Option<FaultKind> {
        if self.plan.is_empty() { return None; }
        self.now = now;
        self.commands += 1;
        self.data = None;
        let mut hit = None;
        if cmd == 53 && arg >> 28 & 7 == 1 {
            let (write, fifo) = (arg >> 31 != 0, arg >> 9 & 0x1_ffff >= esp32c6::sdio::FIFO_START);
            let (counter, kinds): (u64, &[FaultKind]) = match (write, fifo) {
                (false, true) => { self.fifo_reads += 1; (self.fifo_reads, &[FaultKind::ReadCrc, FaultKind::ReadTimeout]) }
                (false, false) => { self.reg_reads += 1; (self.reg_reads, &[FaultKind::RegReadCrc]) }
                (true, true) => { self.writes += 1; (self.writes, &[FaultKind::WriteCrc, FaultKind::WriteNoStatus]) }
                (true, false) => (0, &[]),
            };
            if let Some(f) = self.plan.iter().copied().find(|f| kinds.contains(&f.kind) && matches!(f.when, When::Nth { n, .. } if n == counter)) {
                let byte = match f.when { When::Nth { byte: Some(b), .. } => b, _ => u32::MAX };
                self.data = Some((f.kind, byte));
                self.fire(f);
            }
        }
        let commands = self.commands;
        if let Some(f) = self.plan.iter().copied().find(|f| matches!(f.kind, FaultKind::CmdTimeout | FaultKind::RespCrc) && f.when == When::Nth { n: commands, byte: None }) {
            self.fire(f);
            hit = Some(f.kind);
        }
        hit
    }

    /// After the slave gave a block of a read (`last`: the transfer's end): whether it fails. A
    /// read fails at the block holding byte K, else at its last block, the slave's data given.
    fn on_read_block(&mut self, len: usize, last: bool) -> Option<FaultKind> {
        let (kind, limit) = self.data?;
        let hit = if limit == u32::MAX { last } else { len as u32 > limit };
        self.data = if hit { None } else { Some((kind, limit.saturating_sub(len as u32))) };
        hit.then_some(kind)
    }

    /// The timed faults at a slice end `t`. True when the slave's TOKEN1 hold changes, with the
    /// value to hold.
    fn at_slice_end(&mut self, t: u64) -> Option<bool> {
        if self.plan.is_empty() { return None; }
        self.now = t;
        self.active.resize(self.plan.len(), false);
        let mut token = None;
        for i in 0..self.plan.len() {
            let f = self.plan[i];
            let When::At { ns, len } = f.when else { continue };
            let on = t >= ns && (len == 0 || t < ns + len || matches!(f.kind, FaultKind::Wedge | FaultKind::DeadBus | FaultKind::DropIrq));
            if on == self.active[i] || (self.active[i] && matches!(f.kind, FaultKind::Wedge | FaultKind::DeadBus | FaultKind::DropIrq)) { continue; }
            self.active[i] = on;
            if on { self.fire(f); }
            match f.kind {
                FaultKind::Wedge => self.wedged = true,
                FaultKind::DeadBus => self.dead = true,
                FaultKind::DropIrq => self.drop_edge.set(true),
                FaultKind::SpuriousIrq => self.spurious = on,
                FaultKind::TokenStall => token = Some(on),
                _ => {}
            }
        }
        token
    }

    fn on_write_block(&mut self) -> Option<FaultKind> { self.data.take().map(|d| d.0) }
}
/// `--coproc c6:KEY=VALUE,...`: the co-processor's images and wiring.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoprocSpec {
    pub bootloader: Option<String>, pub ptable: Option<String>, pub app: Option<String>,
    /// the OTA data partition's image and its flash offset (0xD000 in ESP-Hosted's
    /// partitions.esp32c6.csv)
    pub otadata: Option<(String, usize)>,
    pub elfs: Vec<String>, pub stubs: Vec<String>, pub rom: Option<String>,
    pub flash_mb: Option<usize>, pub mac: Option<[u8; 6]>,
    pub en_pin: Option<u8>, pub en_inverted: bool, pub slice_ns: Option<u64>,
    /// where the C6's console goes instead of stdout
    pub console: Option<String>,
    /// faults the link injects (`fault=KIND@WHEN`, repeatable)
    pub faults: Vec<Fault>,
}

impl CoprocSpec {
    pub fn parse(spec: &str) -> Result<CoprocSpec, String> {
        let rest = spec.strip_prefix("c6:").or_else(|| (spec == "c6").then_some("")).ok_or("the co-processor is an ESP32-C6: c6:KEY=VALUE,...")?;
        let mut s = CoprocSpec::default();
        for kv in rest.split(',').filter(|kv| !kv.is_empty()) {
            let (k, v) = kv.split_once('=').ok_or_else(|| format!("{kv}: expected KEY=VALUE"))?;
            match k {
                "bootloader" => s.bootloader = Some(v.into()),
                "ptable" => s.ptable = Some(v.into()),
                "app" => s.app = Some(v.into()),
                "otadata" => {
                    let (path, off) = match v.rsplit_once('@') {
                        Some((p, o)) => (p, usize::from_str_radix(o.trim_start_matches("0x"), 16).map_err(|_| format!("otadata: bad offset {o}"))?),
                        None => (v, 0xd000),
                    };
                    s.otadata = Some((path.into(), off));
                }
                "elf" => s.elfs.push(v.into()),
                "stub" => s.stubs.push(v.into()),
                "rom" => s.rom = Some(v.into()),
                "flash-mb" => s.flash_mb = Some(v.parse().map_err(|_| format!("flash-mb: {v}"))?),
                "mac" => {
                    let b: Vec<u8> = v.split(':').filter_map(|x| u8::from_str_radix(x, 16).ok()).collect();
                    s.mac = Some(b.try_into().map_err(|_| format!("mac: {v}: expected xx:xx:xx:xx:xx:xx"))?);
                }
                "en" => {
                    let (inv, pin) = match v.strip_prefix('!') { Some(p) => (true, p), None => (false, v) };
                    s.en_pin = Some(pin.parse().ok().filter(|&p: &u8| p < 64).ok_or_else(|| format!("en: {v}: a GPIO 0-63, ! for an inverted line"))?);
                    s.en_inverted = inv;
                }
                "slice-us" => s.slice_ns = Some(v.parse::<u64>().ok().filter(|&u| u > 0).ok_or_else(|| format!("slice-us: {v}"))?.saturating_mul(1000)),
                "console" => s.console = Some(v.into()),
                "fault" => s.faults.push(Fault::parse(v)?),
                _ => return Err(format!("unknown key {k:?} (bootloader, ptable, app, otadata, elf, stub, rom, flash-mb, mac, en, slice-us, console, fault)")),
            }
        }
        if s.app.is_none() { return Err("app=FILE is needed".into()); }
        Ok(s)
    }
}

/// End-of-run figures.
#[derive(Clone, Debug, Default)]
pub struct Summary { pub slices: u64, pub en_changes: u64, pub p4_resets: u64, pub stopped: Option<String> }

/// The pair on one clock, the P4 of either revision.
pub struct CoSim<R: crate::Revision = crate::RevLt3> {
    pub p4: esp_soc::Machine<crate::Esp32p4<R>>,
    pub c6: Rc<RefCell<Coproc>>,
    pub cfg: Config,
    pub now_ns: u64,
    en: bool,
    /// P4 console bytes not yet collected
    pub p4_console: Vec<u8>,
    /// complete console lines in the order the slices produced them: (from the C6, text)
    pub lines: Vec<(bool, String)>,
    pub summary: Summary,
}

impl<R: crate::Revision> CoSim<R> {
    /// Join two machines that are set up and booted: the C6's slave goes on the P4's SD bus at
    /// ESP-Hosted's pins. Both consoles are collected here rather than printed by the machines.
    pub fn new(mut p4: esp_soc::Machine<crate::Esp32p4<R>>, mut c6: C6Machine, cfg: Config) -> Self {
        p4.console.capture = true;
        c6.console.capture = true;
        let mut coproc = Coproc::new(c6);
        (coproc.reboot, coproc.console_mask) = (cfg.reboot_c6, cfg.c6_console);
        let c6 = Rc::new(RefCell::new(coproc));
        p4.bus.attach_sdio(crate::periph::SdPins::HOSTED, Box::new(SdioLink(c6.clone())));
        let mut s = CoSim { p4, c6, cfg, now_ns: 0, en: true, p4_console: Vec::new(), lines: Vec::new(), summary: Summary::default() };
        s.en = s.en_level();
        s
    }

    /// EN as the P4 drives it: the pin's output level while enabled, else the pull-up.
    fn en_level(&self) -> bool { self.p4.bus.periph.gpio.drive(self.cfg.en_pin).is_none_or(|l| l ^ self.cfg.en_inverted) }

    /// Run both chips in whole slices until `end_ns` is reached or passed: slices end on a fixed
    /// grid, so a run is the same however it is split into calls. `Err` when the P4 stopped for
    /// another reason than a reset it comes back from.
    pub fn run_until(&mut self, end_ns: u64) -> Result<(), Stop> {
        while self.now_ns < end_ns {
            let t = self.now_ns + self.cfg.slice_ns.max(25);
            let r = self.run_p4(t);
            self.c6.borrow_mut().advance_to(t);
            self.exchange(t);
            self.take_lines();
            r?;
            self.now_ns = t;
            self.summary.slices += 1;
        }
        Ok(())
    }

    fn run_p4(&mut self, t: u64) -> Result<(), Stop> {
        let target = p4_cycle_of_ns(t);
        while self.p4.bus.cycles() < target {
            self.p4.max_cycles = target;
            let stop = self.p4.run(u64::MAX);
            let script_stop = self.p4.max_cycles != target;
            self.p4.max_cycles = u64::MAX;
            self.collect_p4_console();
            if script_stop { self.summary.stopped = Some("the script stopped the run".into()); return Err(Stop::Halted); }
            match stop {
                Stop::Halted => {}
                Stop::SwReset if self.cfg.reboot_p4 => {
                    let cause = self.p4.bus.reset_cause();
                    eprintln!("[emu] p4 chip reset at t={:.3}s: cause {:#x} ({})", self.p4.seconds(), cause, esp_periph::reset_cause_name(cause));
                    self.p4.reboot();
                    self.summary.p4_resets += 1;
                }
                s => { self.summary.stopped = Some(format!("{s:?}")); return Err(s); }
            }
        }
        Ok(())
    }

    /// The lines between the chips at a slice end: DAT1 into the P4, the P4's EN line into the C6.
    /// EN is sampled here only, so it takes effect up to a slice late and a pulse shorter than a
    /// slice can be missed (ESP-Hosted's is 10 ms).
    fn exchange(&mut self, t: u64) {
        {
            let mut c = self.c6.borrow_mut();
            if let Some(hold) = c.faults.at_slice_end(t) {
                let sdio = &mut c.m.bus.periph.optional.sdio;
                sdio.token1_hold = hold.then(|| sdio.token1_exhausted());
            }
        }
        self.p4.bus.sync_sdio();
        self.p4.sync_irq();
        let en = self.en_level();
        if en != self.en {
            self.en = en;
            self.summary.en_changes += 1;
            let mut c = self.c6.borrow_mut();
            if en { c.release(t) } else { c.hold(t) }
            drop(c);
            self.p4.bus.sync_sdio();
            self.p4.sync_irq();
        }
    }

    fn collect_p4_console(&mut self) {
        for (bit, backlog) in [(1, std::mem::take(&mut self.p4.console.usb)), (2, std::mem::take(&mut self.p4.console.uart0))] {
            if self.cfg.p4_console & bit != 0 { self.p4_console.extend_from_slice(&backlog); }
        }
        self.p4.console.all.clear();
        let streams = self.p4.bus.console_take();
        for (i, s) in streams.iter().enumerate().take(3) {
            if self.cfg.p4_console & (1 << i) != 0 { self.p4_console.extend_from_slice(s); }
        }
    }

    /// Move the complete console lines of this slice to `lines`: the P4's first, then the C6's.
    fn take_lines(&mut self) {
        fn lines(buf: &mut Vec<u8>, c6: bool, out: &mut Vec<(bool, String)>) {
            let Some(end) = buf.iter().rposition(|&b| b == b'\n') else { return };
            let done: Vec<u8> = buf.drain(..=end).collect();
            out.extend(done[..end].split(|&b| b == b'\n').map(|l| (c6, String::from_utf8_lossy(l.strip_suffix(b"\r").unwrap_or(l)).into_owned())));
        }
        lines(&mut self.p4_console, false, &mut self.lines);
        lines(&mut self.c6.borrow_mut().console, true, &mut self.lines);
    }
}
