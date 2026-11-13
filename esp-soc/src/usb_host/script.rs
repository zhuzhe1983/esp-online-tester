//! A command file for the host on the USB port (`--usb-host FILE`): one command a line, run in
//! order, each in emulated time; `#` starts a comment. What each did goes to stderr as
//! `[usb-host] t=<s> ...`. The first command that fails ends the script, unless the line starts
//! with `?`, which logs the failure and goes on. A failure that leaves the port without a device
//! drops the interface and the volume, as Linux's disconnect does.
//!
//! ```text
//! attach [connect_timeout_ms]     VBUS on, wait for the device, reset, enumerate, configure
//! detach | reset                  VBUS off; reset and enumerate again
//! port-reset                      usb_reset_device: reset, re-address, check, configure again
//! suspend | resume | idle <ms>    stop SOFs; resume; let time pass
//! rate <MB/s>                     cap the host's bulk payload per microframe
//! msc                             open the mass-storage interface: GET MAX LUN, INQUIRY
//! recovery linux | bot            after a transport error or timeout: Linux usb-storage's port
//!                                 reset (the default), or BOT 1.0's reset recovery
//! timeout <ms>                    how long a command may take before it is aborted (30000)
//! inject vbus-off <ms>            VBUS goes that long from now, whatever the host is doing
//! ready <interval_ms> <timeout_ms>  TEST UNIT READY until it passes, REQUEST SENSE after each failure
//! poll <interval_ms> <ms>         TEST UNIT READY every interval for a while
//!                                 (a NOT READY seen by either drops the mounted volume)
//! capacity | mode-sense | prevent | allow | sync | eject
//! read <lba> <count>              READ(10), prints the data's hash
//! write <lba> <count> <byte>      WRITE(10) of one byte value
//! verify <lba> <count> <byte>     READ(10) and compare
//! mount | ls                      the FAT32 volume on the LUN; its root directory
//! fat-write <name> <bytes> <chunk_kib> [gap_ms] [fat_every]  a file of `file_bytes`, a chunk due
//!                                 every gap_ms from the start (`fat32::write_paced`)
//! fat-read <name>                 read it back; compared with `file_bytes` if this script wrote it
//! uart0 <text>                    a line typed at the firmware's console
//! cdc <ms>                        open the CDC-ACM interface (line coding 115200 8N1, DTR+RTS
//!                                 raised) and print what its bulk IN endpoint sends, for that long
//! ```
use super::fat32::{file_bytes, write_paced, Fat32};
use super::msc::{Msc, MscError, Recovery};
use esp_periph::usb_otg::LinkFault;
use super::{UsbError, UsbHost, UsbPort};

/// FNV-1a, to name what was read.
pub fn hash(data: &[u8]) -> u64 { data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)) }

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Attach(u64), Detach, Reset, PortReset, Suspend, Resume, Idle(u64), Rate(f64), Msc, Recovery(Recovery), Timeout(u64), InjectVbusOff(u64),
    Ready(u64, u64), Poll(u64, u64), Capacity, ModeSense, Prevent(bool), Sync, Eject,
    Read(u64, u32), Write(u64, u32, u8), Verify(u64, u32, u8), Mount, Ls, FatWrite { name: String, bytes: usize, chunk: usize, gap_ms: u64, fat_every: u32 }, FatRead(String),
    Uart0(String),
    /// Read the CDC-ACM console for a while, first sending a line (CR LF appended) when given.
    Cdc(u64, Option<String>),
    /// `?`: a failure is logged and the script goes on
    Tolerate(Box<Cmd>),
}

/// The largest time (ms) and block count a line may give: a day of emulated time, 32 MiB of
/// 512-byte blocks, a 1 GiB file. Larger values are usage errors, not overflows or huge buffers.
const MAX_MS: u64 = 86_400_000;
const MAX_BLOCKS: u64 = 1 << 16;
const MAX_FILE: u64 = 1 << 30;

pub fn parse(text: &str) -> Result<Vec<(usize, Cmd)>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let (tolerate, line) = match line.strip_prefix('?') { Some(l) => (true, l.trim_start()), None => (false, line) };
        let ln = n + 1;
        let mut w = line.split_whitespace();
        let cmd = w.next().unwrap();
        let args: Vec<&str> = w.collect();
        let num = |i: usize, what: &str| -> Result<u64, String> {
            let s = args.get(i).ok_or_else(|| format!("line {ln}: {cmd} needs {what}"))?;
            if let Some(h) = s.strip_prefix("0x") { u64::from_str_radix(h, 16) } else { s.parse() }.map_err(|_| format!("line {ln}: {what}: {s}"))
        };
        let at_most = |i: usize, what: &str, max: u64| -> Result<u64, String> {
            let v = num(i, what)?;
            if v > max { Err(format!("line {ln}: {what}: {v} is over {max}")) } else { Ok(v) }
        };
        let ms = |i: usize, what: &str| at_most(i, what, MAX_MS);
        let opt = |i: usize, d: u64, what: &str, max: u64| -> Result<u64, String> { if args.len() > i { at_most(i, what, max) } else { Ok(d) } };
        let byte = |i: usize| -> Result<u8, String> { let s = args.get(i).ok_or_else(|| format!("line {ln}: {cmd} needs a byte"))?; u8::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|_| format!("line {ln}: byte {s}")) };
        let c = match cmd {
            "attach" => Cmd::Attach(opt(0, 10_000, "milliseconds", MAX_MS)?),
            "detach" => Cmd::Detach, "reset" => Cmd::Reset, "port-reset" => Cmd::PortReset, "suspend" => Cmd::Suspend, "resume" => Cmd::Resume,
            "idle" => Cmd::Idle(ms(0, "milliseconds")?),
            "rate" => Cmd::Rate(args.first().and_then(|s| s.parse().ok()).ok_or_else(|| format!("line {ln}: rate <MB/s>"))?),
            "msc" => Cmd::Msc,
            "recovery" => Cmd::Recovery(match args.first().copied() { Some("linux") => Recovery::Linux, Some("bot") => Recovery::Bot, _ => return Err(format!("line {ln}: recovery linux|bot")) }),
            "timeout" => Cmd::Timeout(ms(0, "milliseconds")?),
            "inject" => match args.first().copied() {
                Some("vbus-off") => Cmd::InjectVbusOff(ms(1, "milliseconds from now")?),
                _ => return Err(format!("line {ln}: inject vbus-off <ms>")),
            },
            "ready" => Cmd::Ready(ms(0, "an interval")?, ms(1, "a timeout")?),
            "poll" => Cmd::Poll(ms(0, "an interval")?, ms(1, "a duration")?),
            "capacity" => Cmd::Capacity, "mode-sense" => Cmd::ModeSense, "prevent" => Cmd::Prevent(true), "allow" => Cmd::Prevent(false),
            "sync" => Cmd::Sync, "eject" => Cmd::Eject, "mount" => Cmd::Mount, "ls" => Cmd::Ls,
            "read" => Cmd::Read(num(0, "an LBA")?, at_most(1, "a count", MAX_BLOCKS)? as u32),
            "write" => Cmd::Write(num(0, "an LBA")?, at_most(1, "a count", MAX_BLOCKS)? as u32, byte(2)?),
            "verify" => Cmd::Verify(num(0, "an LBA")?, at_most(1, "a count", MAX_BLOCKS)? as u32, byte(2)?),
            "fat-write" => Cmd::FatWrite { name: args.first().ok_or_else(|| format!("line {ln}: fat-write <name> <bytes> <chunk_kib>"))?.to_string(), bytes: at_most(1, "a size", MAX_FILE)? as usize,
                                           chunk: at_most(2, "a chunk size", MAX_FILE >> 10)? as usize * 1024, gap_ms: opt(3, 0, "milliseconds", MAX_MS)?, fat_every: opt(4, 1, "a FAT interval", u32::MAX as u64)? as u32 },
            "fat-read" => Cmd::FatRead(args.first().ok_or_else(|| format!("line {ln}: fat-read <name>"))?.to_string()),
            "cdc" => Cmd::Cdc(ms(0, "a duration")?, (args.len() > 1).then(|| args[1..].join(" "))),
            "uart0" => Cmd::Uart0(line["uart0".len()..].trim().to_string()),
            _ => return Err(format!("line {ln}: unknown command {cmd}")),
        };
        out.push((ln, if tolerate { Cmd::Tolerate(Box::new(c)) } else { c }));
    }
    Ok(out)
}

/// Where a script stands: the host, then the mass-storage interface it became, the volume on it.
pub struct Runner<'p> {
    host: Option<UsbHost<'p>>,
    msc: Option<Msc<'p>>,
    fs: Option<Fat32>,
    written: Vec<(String, usize)>,
}

impl<'p> Runner<'p> {
    pub fn new(port: &'p mut dyn UsbPort) -> Self { let mut h = UsbHost::new(port); h.log = true; Runner { host: Some(h), msc: None, fs: None, written: Vec::new() } }

    pub fn host(&mut self) -> &mut UsbHost<'p> { match &mut self.msc { Some(m) => &mut m.host, None => self.host.as_mut().unwrap() } }
    fn msc(&mut self) -> Result<&mut Msc<'p>, String> { self.msc.as_mut().ok_or_else(|| "no mass-storage interface open (msc)".to_string()) }
    fn fs(&mut self) -> Result<(&mut Fat32, &mut Msc<'p>), String> {
        match (&mut self.fs, &mut self.msc) { (Some(f), Some(m)) => Ok((f, m)), _ => Err("no volume mounted (mount)".into()) }
    }
    fn say(&mut self, what: std::fmt::Arguments) { self.host().say(what); }

    /// Run the commands; the line and the error of the first that fails.
    pub fn run(&mut self, cmds: &[(usize, Cmd)]) -> Result<(), (usize, String)> {
        for (ln, c) in cmds { self.step(c).map_err(|e| (*ln, e))?; }
        Ok(())
    }

    pub fn step(&mut self, c: &Cmd) -> Result<(), String> {
        let r = match c { Cmd::Tolerate(c) => self.act(c), c => self.act(c) };
        if r.is_err() && !self.host().port().link().is_some_and(|l| l.port().connected) && (self.msc.is_some() || self.fs.is_some()) {
            self.say(format_args!("no device on the port: interface and volume dropped"));
            self.back_to_host();
        }
        match (r, c) {
            (Err(e), Cmd::Tolerate(_)) => { self.say(format_args!("failed, going on: {e}")); Ok(()) }
            (r, _) => r,
        }
    }

    fn act(&mut self, c: &Cmd) -> Result<(), String> {
        let ms = 1_000_000u64;
        let e = |e: MscError| e.to_string();
        let u = |e: UsbError| e.to_string();
        match c {
            Cmd::Tolerate(c) => self.act(c)?,
            Cmd::Attach(to) => { self.back_to_host(); self.host().enumerate(to * ms).map_err(u)? }
            Cmd::PortReset => { self.host().reset_device().map_err(u)?; }
            Cmd::Recovery(r) => { let m = self.msc()?; m.recovery = *r; m.host.say(format_args!("recovery: {r:?}")); }
            Cmd::Timeout(t) => { let m = self.msc()?; m.timeout_ns = t * ms; m.host.say(format_args!("command timeout {t} ms")); }
            Cmd::InjectVbusOff(t) => {
                let at = self.host().now_ns() + t * ms;
                self.host().port().link().ok_or("no USB host attached")?.inject(at, LinkFault::VbusOff);
                self.say(format_args!("VBUS goes in {t} ms"));
            }
            Cmd::Detach => { self.host().power_off().map_err(u)?; self.say(format_args!("VBUS off")); self.back_to_host(); }
            Cmd::Reset => { self.back_to_host(); self.host().enumerate(10_000 * ms).map_err(u)?; }
            Cmd::Suspend => { self.host().suspend().map_err(u)?; self.say(format_args!("suspended")); }
            Cmd::Resume => { self.host().resume().map_err(u)?; self.say(format_args!("resumed")); }
            Cmd::Idle(t) => self.host().idle_ns(t * ms).map_err(u)?,
            Cmd::Rate(mbps) => {
                let bytes = (*mbps * 1e6) as u64;
                let l = self.host().port().link().ok_or("no USB host attached")?;
                l.timing = l.timing.with_rate(bytes);
                self.say(format_args!("bulk rate capped at {mbps} MB/s"));
            }
            Cmd::Cdc(dur, send) => {
                let h = self.host();
                let cfg = h.config.clone().ok_or("no configured device (attach)")?;
                let comm = cfg.interfaces.iter().find(|f| f.class == 0x02 && f.subclass == 0x02).map(|f| f.number).ok_or("the device has no CDC-ACM interface")?;
                let ep = cfg.interfaces.iter().filter(|f| f.class == 0x0a).flat_map(|f| f.endpoints.iter())
                    .find(|e| e.address & 0x80 != 0 && e.attributes & 3 == 2).cloned().ok_or("the CDC data interface has no bulk IN endpoint")?;
                // CDC PSTN 1.2 §6.3.10 SET_LINE_CODING, §6.3.12 SET_CONTROL_LINE_STATE (DTR | RTS).
                let mut coding = 115_200u32.to_le_bytes().to_vec(); coding.extend([0, 0, 8]);
                h.control(super::setup_packet(0x21, 0x20, 0, comm as u16, 7), coding).map_err(u)?;
                h.control(super::setup_packet(0x21, 0x22, 3, comm as u16, 0), Vec::new()).map_err(u)?;
                h.say(format_args!("CDC-ACM interface {comm}: DTR+RTS up, reading endpoint {:02x}", ep.address));
                if let Some(text) = send {
                    let out = cfg.interfaces.iter().filter(|f| f.class == 0x0a).flat_map(|f| f.endpoints.iter())
                        .find(|e| e.address & 0x80 == 0 && e.attributes & 3 == 2).cloned().ok_or("the CDC data interface has no bulk OUT endpoint")?;
                    h.bulk_out(out.address, out.mps, format!("{text}\r\n").into_bytes(), 1000 * ms).map_err(u)?;
                    h.say(format_args!("CDC: sent {:?}", text));
                }
                let end = h.now_ns() + dur * ms;
                let mut total = 0usize;
                while h.now_ns() < end {
                    match h.bulk_in(ep.address, ep.mps, ep.mps as usize * 8, 2 * ms) {
                        Ok(d) => { total += d.len(); use std::io::Write; let mut o = std::io::stdout().lock(); let _ = o.write_all(&d); let _ = o.flush(); }
                        Err(UsbError::Timeout) => {}
                        Err(x) => return Err(x.to_string()),
                    }
                }
                h.say(format_args!("CDC: {total} bytes received"));
            }
            Cmd::Msc => {
                let host = self.host.take().ok_or("mass storage is open already")?;
                let mut m = Msc::open(host).map_err(e)?;
                let i = m.inquiry().map_err(e)?;
                m.host.say(format_args!("INQUIRY: type {}, {}, {:?} {:?} {:?}", i.device_type, if i.removable { "removable" } else { "fixed" }, i.vendor, i.product, i.revision));
                self.msc = Some(m);
            }
            Cmd::Ready(iv, to) => {
                let m = self.msc()?;
                let failed = m.wait_ready(iv * ms, to * ms).map_err(e)?;
                m.host.say(format_args!("TEST UNIT READY passed after {failed} failed polls"));
                if failed > 0 { self.medium_changed(); }
            }
            Cmd::Poll(iv, dur) => {
                let m = self.msc()?;
                let end = m.host.now_ns() + dur * ms;
                let (mut ok, mut bad, mut last) = (0, 0, None);
                while m.host.now_ns() < end {
                    match m.test_unit_ready() {
                        Ok(()) => { ok += 1; if last.is_some() { m.host.say(format_args!("TEST UNIT READY: ready")); last = None; } }
                        Err(MscError::Check(s)) => { bad += 1; if last != Some(s) { m.host.say(format_args!("TEST UNIT READY: {s}")); last = Some(s); } }
                        Err(x) => return Err(x.to_string()),
                    }
                    m.host.idle_ns(iv * ms).map_err(u)?;
                }
                m.host.say(format_args!("polled: {ok} ready, {bad} not ready"));
                if bad > 0 { self.medium_changed(); }
            }
            Cmd::Capacity => {
                let m = self.msc()?;
                let (n, bs) = m.read_capacity().map_err(e)?;
                m.host.say(format_args!("READ CAPACITY: {n} blocks of {bs} bytes ({} MiB)", (n * bs as u64) >> 20));
            }
            Cmd::ModeSense => { let m = self.msc()?; let wp = m.mode_sense().map_err(e)?; m.host.say(format_args!("MODE SENSE: {}", if wp { "write-protected" } else { "writable" })); }
            Cmd::Prevent(p) => { let m = self.msc()?; m.prevent_removal(*p).map_err(e)?; m.host.say(format_args!("{} medium removal", if *p { "prevent" } else { "allow" })); }
            // optional for the device (SBC-3): a host logs the refusal and goes on, as Linux's sd does
            Cmd::Sync => {
                let m = self.msc()?;
                match m.sync_cache() {
                    Ok(()) => m.host.say(format_args!("SYNCHRONIZE CACHE")),
                    Err(MscError::Check(s)) => m.host.say(format_args!("SYNCHRONIZE CACHE refused: {s}")),
                    Err(x) => return Err(x.to_string()),
                }
            }
            Cmd::Eject => { let m = self.msc()?; m.start_stop(false, true).map_err(e)?; m.host.say(format_args!("ejected")); }
            Cmd::Read(lba, n) => {
                let m = self.msc()?;
                let t0 = m.host.now_ns();
                let d = m.read10(*lba, *n).map_err(e)?;
                let dt = m.host.now_ns() - t0;
                m.host.say(format_args!("READ(10) {n} blocks at {lba}: hash {:016x} in {} us", hash(&d), dt / 1000));
            }
            Cmd::Write(lba, n, b) | Cmd::Verify(lba, n, b) => {
                let m = self.msc()?;
                let len = *n as usize * m.block_size as usize;
                let t0 = m.host.now_ns();
                if let Cmd::Write(..) = c {
                    m.write10(*lba, &vec![*b; len]).map_err(e)?;
                    let dt = m.host.now_ns() - t0;
                    m.host.say(format_args!("WRITE(10) {n} blocks of {b:02x} at {lba} in {} us", dt / 1000));
                } else {
                    let d = m.read10(*lba, *n).map_err(e)?;
                    let bad = d.iter().filter(|&&x| x != *b).count();
                    m.host.say(format_args!("verify {n} blocks at {lba}: {}", if bad == 0 { format!("all {b:02x}") } else { format!("{bad} bytes differ") }));
                    if bad > 0 { return Err(format!("{bad} bytes differ")); }
                }
            }
            Cmd::Mount => {
                let m = self.msc()?;
                let fs = Fat32::mount(m)?;
                m.host.say(format_args!("FAT32 {:?} at LBA {}: {} clusters of {} bytes, data from LBA {}", fs.label, fs.volume_lba, fs.clusters, fs.cluster_bytes(), fs.data_lba));
                self.fs = Some(fs);
            }
            Cmd::Ls => {
                let (fs, m) = self.fs()?;
                let list = fs.list_root(m)?;
                let names: Vec<String> = list.iter().map(|d| format!("{} ({})", d.name, d.size)).collect();
                m.host.say(format_args!("root: {}", if names.is_empty() { "empty".to_string() } else { names.join(", ") }));
            }
            Cmd::FatWrite { name, bytes, chunk, gap_ms, fat_every } => {
                let data = file_bytes(name, *bytes);
                let (fs, m) = self.fs()?;
                let st = write_paced(fs, m, name, &data, *chunk, gap_ms * ms, *fat_every)?;
                m.host.say(format_args!("fat-write {name}: {}", st.summary(gap_ms * ms)));
                self.written.push((name.clone(), *bytes));
            }
            Cmd::FatRead(name) => {
                let expect = self.written.iter().find(|w| &w.0 == name).map(|w| file_bytes(name, w.1));
                let (fs, m) = self.fs()?;
                let t0 = m.host.now_ns();
                let d = fs.read_file(m, name)?;
                let dt = (m.host.now_ns() - t0).max(1);
                let check = match &expect { Some(x) if *x == d => ", matches what was written", Some(_) => ", DIFFERS from what was written", None => "" };
                m.host.say(format_args!("fat-read {name}: {} bytes, hash {:016x}, {:.2} MB/s{check}", d.len(), hash(&d), d.len() as f64 * 1e3 / dt as f64));
                if expect.is_some_and(|x| x != d) { return Err(format!("{name} differs")); }
            }
            Cmd::Uart0(text) => { let t = format!("{text}\n"); let h = self.host(); h.port().console(&t); h.say(format_args!("console: {text}")); }
        }
        Ok(())
    }

    /// A NOT READY seen: what the host knew of the volume may be stale, so it is mounted again.
    fn medium_changed(&mut self) {
        if self.fs.take().is_some() { self.say(format_args!("medium changed: volume dropped")); }
    }

    /// Back to the bare host (after a detach or a reset the interface is gone).
    fn back_to_host(&mut self) {
        if let Some(m) = self.msc.take() { self.host = Some(m.host); }
        self.fs = None;
    }
}
