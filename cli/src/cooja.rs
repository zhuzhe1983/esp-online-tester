//! `--cooja`: the ESP32-C6 as an *external mote* of Cooja-NG, driven in lock-step over NDJSON
//! on stdin/stdout (csim `docs/design/external-nodes-plan.md` §4, protocol v1).
//!
//! Cooja-NG owns the clock. It sends `hello` once, then `step {t, in}` per slice, then `stop`;
//! we answer each with exactly one `done {t, wake, out}`. Times are nanoseconds of simulation
//! time; ours is derived from the bus's cycle counter at 160 MHz — 6.25 ns per cycle, so a step
//! to `t` runs the guest to the first cycle at or after `t` (`cycle_of_ns`) and every event is
//! stamped with the cycle it happened at (`ns_of_cycle`). The counter never restarts, not even
//! across a chip reset, so the mapping is monotonic for the life of the process.
//!
//! What makes the lock-step exact rather than "within a slice":
//! - `Machine::run_until_cycle` stops at the target cycle and at the instruction that starts a
//!   transmission (`SocBus::take_host_event`), so a `tx` event carries the time of the
//!   `TX_START` write and the reply goes out before the slice ends;
//! - an `rx` inside a slice is injected at its own time: the guest runs to `rx.t` and the frame is
//!   handed to the radio as csim reports it, at its start — its first preamble byte, so the SFD
//!   follows five byte times later, `RX_DONE` after the whole PPDU, and the acknowledgement
//!   192 µs after that (`rx_on_air: false` takes `rx.t` as the frame's end instead: complete at
//!   once) — and the run continues;
//! - a guest in `wfi` costs nothing: time jumps to the next device deadline or the slice end.
//!
//! `wake` is what we ask csim for next: the next device deadline when the guest sleeps, `t +
//! slice` when it is busy, and never later than an input we still hold (one that arrived in a
//! slice we cut short). Nothing here reads a host clock or host randomness.
use emu_core::Core;
use esp32c6::radio::RadioState;
use esp_soc::{RunUntil, SocBus, Stop};
use std::io::{BufRead, Write};

/// 160 MHz: 25/4 ns per cycle, exactly.
pub const NS_NUM: u64 = 25;
pub const NS_DEN: u64 = 4;
/// How precisely a `log` or `radio` event is stamped: the console and the radio state are
/// looked at this often while the guest is busy (an idle guest prints nothing, and its skips
/// are not cut). 20 µs is ~3200 instructions, shorter than one ISR log line takes to print.
pub const LOG_STEP_NS: u64 = 20_000;
pub fn ns_of_cycle(c: u64) -> u64 { ((c as u128 * NS_NUM as u128 / NS_DEN as u128).min(u64::MAX as u128)) as u64 }
fn ns_after_cycle(c: u64) -> u64 { ns_of_cycle(c).saturating_add(u64::from(!c.is_multiple_of(NS_DEN))) }
/// The first cycle at or after `ns`.
pub fn cycle_of_ns(ns: u64) -> u64 { (ns as u128 * NS_DEN as u128).div_ceil(NS_NUM as u128) as u64 }

pub use esp_soc::json::{parse_json, Json};

use esp_soc::web::{hex_decode, hex_encode};

// ------------------------------------------------------------------ the peer
/// What `hello` told us.
#[derive(Clone, Debug, Default)]
pub struct Hello { pub id: i64, pub seed: i64, pub x: f64, pub y: f64, pub max_frame: i64, pub args: Option<Json> }

/// Knobs a run is started with (the command line, `hello.args` may override `slice_us`).
#[derive(Clone, Debug)]
pub struct Config {
    /// how long a busy guest runs before asking csim to step it again
    pub slice_ns: u64,
    /// which guest consoles become `log` events: bit0 USB-Serial/JTAG, bit1 UART0, bit2 UART1
    pub console_mask: u32,
    /// an `rx` at `t` is the start of the frame on the air (its first preamble byte: the SFD
    /// five byte times later, `RX_DONE` after the whole PPDU), which is when csim hands a
    /// frame-consuming mote a frame; `false` takes `t` as the frame's end (complete at once)
    pub rx_on_air: bool,
    /// narrate the exchange on stderr
    pub verbose: bool,
    /// reboot after guest reset requests, matching the CLI's ROM boot policy
    pub reboot: bool,
}
/// 100 µs busy slices: a transmission reaches csim's medium at the end of the slice it started
/// in, so the slice bounds how late it is. ~5000 exchanges per busy second at ~20 µs each.
impl Default for Config { fn default() -> Self { Config { slice_ns: 100_000, console_mask: 2, rx_on_air: true, verbose: false, reboot: true } } }

/// End-of-run figures for the report.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub steps: u64, pub yields: u64, pub tx: u64, pub rx: u64, pub rx_dropped: u64, pub logs: u64,
    pub sim_ns: u64, pub stopped: Option<String>,
}

#[derive(Clone, Debug)]
enum Input { Rx { t: u64, frame: Vec<u8>, rssi: i8, lqi: u8 }, Serial { t: u64, data: Vec<u8> }, Ignored { t: u64 } }
impl Input { fn t(&self) -> u64 { match self { Input::Rx { t, .. } | Input::Serial { t, .. } | Input::Ignored { t } => *t } } }

/// Read the `hello` line (the first message) before the machine exists: the node id names the
/// MAC address unless the command line did.
pub fn read_hello(input: &mut dyn BufRead) -> Result<Hello, String> {
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line).map_err(|e| e.to_string())? == 0 { return Err("stdin closed before hello".into()); }
        if line.trim().is_empty() { continue; }
        let msg = parse_json(line.trim_end())?;
        if msg.str_or("type", "") != "hello" { return Err(format!("expected hello, got {}", line.trim_end())); }
        return Ok(Hello {
            id: msg.i64_or("id", 0), seed: msg.i64_or("seed", 0),
            x: msg.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0), y: msg.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
            max_frame: msg.i64_or("max_frame", 152), args: msg.get("args").cloned(),
        });
    }
}

/// A MAC for node `id`: locally administered, the id in the low bytes. What the guest derives
/// its EUI-64 (and so its link-layer address) from, so it is a function of the config alone.
pub fn mac_for_node(id: i64) -> [u8; 6] { let id = id as u32; [0x02, 0x00, 0x00, (id >> 16) as u8, (id >> 8) as u8, id as u8] }

struct Peer<'a> {
    m: &'a mut esp32c6::Machine,
    cfg: Config,
    pending: Vec<Input>,
    out: Vec<String>,
    partial: [Vec<u8>; 3],
    radio_reported: Option<&'static str>,
    summary: Summary,
    halted: bool,
}

impl<'a> Peer<'a> {
    fn now_cycles(&self) -> u64 { self.m.bus.cycles() }
    fn now_ns(&self) -> u64 { ns_of_cycle(self.now_cycles()) }

    fn radio_state_name(&self) -> &'static str {
        match self.m.bus.periph.radio.state { RadioState::Idle => "on", RadioState::Rx | RadioState::RxFrame | RadioState::RxAck | RadioState::Ed => "rx", RadioState::Tx | RadioState::TxAck => "tx" }
    }
    /// A `radio` event when the state changed since the last one, stamped now.
    fn report_radio(&mut self) {
        let name = self.radio_state_name();
        if self.radio_reported == Some(name) { return; }
        if self.radio_reported.is_none() && name == "on" && self.m.bus.periph.radio.tx_count == 0 && self.m.bus.periph.radio.rx_count == 0 && !self.m.bus.periph.radio.listening() { return; }   // never touched: no news
        self.radio_reported = Some(name);
        let (t, ch) = (self.now_ns(), self.m.bus.periph.radio.channel());
        self.out.push(format!("{{\"type\":\"radio\",\"t\":{},\"state\":\"{}\",\"ch\":{}}}", t, name, ch));
    }

    /// Guest console bytes since the last call, as `log` events stamped now (complete lines
    /// only; a partial line waits for its end).
    fn drain_console(&mut self) {
        let streams = self.m.bus.console_take();
        let t = self.now_ns();
        for (i, data) in streams.into_iter().enumerate().take(3) {
            if data.is_empty() || self.cfg.console_mask & (1 << i) == 0 { continue; }
            self.partial[i].extend_from_slice(&data);
            while let Some(nl) = self.partial[i].iter().position(|&b| b == b'\n') {
                let mut line: Vec<u8> = self.partial[i].drain(..=nl).collect();
                line.pop();
                if line.last() == Some(&b'\r') { line.pop(); }
                let text = String::from_utf8_lossy(&line);
                self.out.push(format!("{{\"type\":\"log\",\"t\":{},\"line\":\"{}\"}}", t, esp_soc::web::json_escape(&text)));
                self.summary.logs += 1;
            }
        }
    }

    /// Run the guest to `target_ns`. `Some(T)` if a transmission started at `T` on the way.
    /// Console lines are stamped at the end of the segment that produced them, so a busy guest
    /// is run in segments of `LOG_STEP_NS` (an idle one jumps straight to the target).
    fn run_to(&mut self, target_ns: u64) -> Option<u64> {
        loop {
            let now = self.now_ns();
            if now >= target_ns { return None; }
            let core = &self.m.cores[0];
            let seg = if core.waiting() && !core.irq_pending() {
                // asleep: nothing to stamp until a device wakes it, so the segment ends one step after that
                match self.m.bus.next_deadline() { None => target_ns, Some(dl) => target_ns.min(ns_of_cycle(self.now_cycles().saturating_add(dl)).saturating_add(LOG_STEP_NS)) }
            } else { target_ns.min(now.saturating_add(LOG_STEP_NS)) };
            if let Some(t) = self.run_segment(seg) { return Some(t); }
            if self.halted { return None; }
        }
    }

    fn run_segment(&mut self, target_ns: u64) -> Option<u64> {
        if self.halted { return None; }
        let target = cycle_of_ns(target_ns).min(self.m.max_cycles);
        let result = self.m.run_until_cycle(target);
        let result = if matches!(result, RunUntil::Reached) && self.now_cycles() >= self.m.max_cycles {
            RunUntil::Stop(Stop::Halted)
        } else { result };
        match result {
            RunUntil::Reached => { self.drain_console(); self.report_radio(); None }
            RunUntil::Yield => {
                self.drain_console();
                let t = self.now_ns();
                let radio = &self.m.bus.periph.radio;
                let (ch, frame) = (radio.channel(), hex_encode(&radio.tx_frame));
                self.out.push(format!("{{\"type\":\"tx\",\"t\":{},\"ch\":{},\"frame\":\"{}\"}}", t, ch, frame));
                self.summary.tx += 1; self.summary.yields += 1;
                self.report_radio();
                Some(t)
            }
            RunUntil::Stop(stop) => {
                self.drain_console();
                let t = self.now_ns();
                let why = match &stop {
                    Stop::SwReset => { let cause = self.m.bus.reset_cause(); format!("chip reset, cause {:#x} ({})", cause, esp_periph::reset_cause_name(cause)) }
                    s => format!("{:?}", s),
                };
                self.out.push(format!("{{\"type\":\"log\",\"t\":{},\"line\":\"[emu] stop: {}\"}}", t, esp_soc::web::json_escape(&why)));
                if matches!(stop, Stop::SwReset) && self.cfg.reboot && self.now_cycles() < self.m.max_cycles {
                    // The cycle cap survives reboots, as in the normal run loop.
                    self.m.reboot();
                    eprintln!("[cooja] chip reset at t={} ns: {}", t, why);
                } else {
                    eprintln!("[cooja] guest halted at t={} ns: {}", t, why);
                    self.halted = true; self.summary.stopped = Some(why);
                }
                None
            }
        }
    }

    fn apply(&mut self, input: Input) {
        match input {
            Input::Rx { t, frame, rssi, lqi } => {
                // csim stamps a frame with its start on the air, which may be before the guest's
                // time when the sender is an emulated mote that lags the kernel: the frame then
                // started that long ago, and RX_DONE lands at its true end
                let ago = self.now_cycles().saturating_sub(cycle_of_ns(t));
                let taken = self.m.bus.radio_receive(&frame, rssi, lqi, if self.cfg.rx_on_air { Some(ago) } else { None });
                self.m.sync_irq();
                self.summary.rx += 1;
                if !taken { self.summary.rx_dropped += 1; }
                if self.cfg.verbose { eprintln!("[cooja] rx at t={} ({} bytes, rssi {}): {}", t, frame.len(), rssi, if taken { "taken" } else { "dropped, radio not listening" }); }
            }
            Input::Serial { data, .. } => { self.m.bus.serial_input(&data); self.m.sync_irq(); }
            Input::Ignored { .. } => {}
        }
    }

    /// When to be stepped next: the device deadline while asleep, `now + slice` while busy, and
    /// never past an input still held; always after `t`, so csim makes progress.
    fn wake(&self, t: u64) -> Option<u64> {
        if self.halted { return self.pending.first().map(|i| i.t().max(t.saturating_add(1))); }
        let core = &self.m.cores[0];
        let now = self.now_cycles();
        let mut w = if core.waiting() && !core.irq_pending() {
            self.m.bus.next_deadline().map(|dl| ns_after_cycle(now.saturating_add(dl)))
        } else { Some(ns_of_cycle(now).saturating_add(self.cfg.slice_ns)) };
        if self.m.max_cycles != u64::MAX {
            let cap = ns_after_cycle(self.m.max_cycles);
            w = Some(w.map_or(cap, |w| w.min(cap)));
        }
        if let Some(p) = self.pending.first() { w = Some(w.map_or(p.t(), |w| w.min(p.t()))); }
        w.map(|w| w.max(t.saturating_add(1)))
    }

    fn done(&mut self, output: &mut dyn Write, t: u64, wake: Option<u64>) -> Result<(), String> {
        let out = std::mem::take(&mut self.out);
        let wake = wake.map_or("null".to_string(), |w| w.to_string());
        writeln!(output, "{{\"type\":\"done\",\"t\":{},\"wake\":{},\"out\":[{}]}}", t, wake, out.join(",")).map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())
    }

    /// One `step`: inputs due inside the slice go in at their own times; a transmission cuts
    /// the slice short at its start.
    fn step(&mut self, t: u64, inputs: Vec<Input>, output: &mut dyn Write) -> Result<(), String> {
        self.summary.steps += 1;
        self.pending.extend(inputs);
        self.pending.sort_by_key(|i| i.t());
        loop {
            let next_t = self.pending.first().map(|i| i.t()).filter(|&it| it <= t);
            let target = next_t.unwrap_or(t);
            let late = target < self.now_ns();
            if late && self.cfg.verbose { eprintln!("[cooja] input for t={} arrives with the guest at t={}: taken as started {} ns ago", target, self.now_ns(), self.now_ns() - target); }
            if let Some(tx_t) = self.run_to(target.max(self.now_ns())) {
                let wake = self.wake(tx_t);
                self.summary.sim_ns = tx_t;
                return self.done(output, tx_t, wake);
            }
            match next_t {
                Some(_) => { let input = self.pending.remove(0); self.apply(input); }
                None => break,
            }
        }
        self.report_radio();
        let wake = self.wake(t);
        self.summary.sim_ns = t;
        self.done(output, t, wake)
    }
}

fn parse_inputs(msg: &Json) -> Vec<Input> {
    let mut v = Vec::new();
    for ev in msg.get("in").and_then(|a| a.as_arr()).unwrap_or(&[]) {
        let t = ev.i64_or("t", 0).max(0) as u64;
        match ev.str_or("type", "") {
            "rx" => {
                let Some(frame) = ev.get("frame").and_then(|f| f.as_str()).and_then(hex_decode) else { eprintln!("[cooja] rx without a valid frame ignored"); continue };
                v.push(Input::Rx { t, frame, rssi: ev.i64_or("rssi", -60).clamp(-128, 127) as i8, lqi: ev.i64_or("lqi", 255).clamp(0, 255) as u8 });
            }
            "serial" => { if let Some(data) = ev.get("data").and_then(|f| f.as_str()).and_then(hex_decode) { v.push(Input::Serial { t, data }); } }
            _ => v.push(Input::Ignored { t }),
        }
    }
    v
}

/// The exchange, from the reply to `hello` to `stop` (or the end of stdin). The machine is
/// booted already. Returns the run's figures.
pub fn run(m: &mut esp32c6::Machine, cfg: Config, hello: &Hello, input: &mut dyn BufRead, output: &mut dyn Write) -> Result<Summary, String> {
    let mut cfg = cfg;
    if let Some(us) = hello.args.as_ref().and_then(|a| a.get("slice_us")).and_then(|v| v.as_i64()) { if us > 0 { cfg.slice_ns = (us as u64).saturating_mul(1000); } }
    m.console.capture = true;
    let mut peer = Peer { m, cfg, pending: Vec::new(), out: Vec::new(), partial: Default::default(), radio_reported: None, summary: Summary::default(), halted: false };
    if peer.cfg.verbose { eprintln!("[cooja] hello: node {} seed {} at ({}, {}), slice {} µs", hello.id, hello.seed, hello.x, hello.y, peer.cfg.slice_ns / 1000); }
    // the reply to hello carries the first wake: the guest is booting, so "soon"
    peer.drain_console();
    let wake = peer.wake(0);
    peer.done(output, 0, wake)?;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line).map_err(|e| e.to_string())? == 0 { eprintln!("[cooja] stdin closed"); break; }
        let text = line.trim_end();
        if text.is_empty() { continue; }
        let msg = match parse_json(text) { Ok(m) => m, Err(e) => { eprintln!("[cooja] bad line ignored ({}): {:.120}", e, text); continue; } };
        match msg.str_or("type", "") {
            "step" => {
                let t = msg.i64_or("t", 0).max(0) as u64;
                let inputs = parse_inputs(&msg);
                peer.step(t, inputs, output)?;
            }
            "stop" => {
                if peer.cfg.verbose { eprintln!("[cooja] stop at t={}: {}", msg.i64_or("t", 0), msg.str_or("reason", "")); }
                // what the guest had printed without a newline yet is worth seeing on stderr (no reply is due)
                for (i, p) in peer.partial.iter().enumerate() { if !p.is_empty() { eprintln!("[cooja] unterminated console line ({}): {}", ["usb", "uart0", "uart1"][i], String::from_utf8_lossy(p)); } }
                break;
            }
            "hello" => { eprintln!("[cooja] a second hello ignored"); }
            other => { eprintln!("[cooja] unknown message type {:?} ignored", other); }
        }
    }
    let mut s = peer.summary.clone();
    s.rx_dropped = s.rx_dropped.max(peer.m.bus.periph.radio.rx_dropped);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_ns_mapping_is_exact_and_monotonic() {
        for c in [0u64, 1, 4, 7, 160, 1_000_000_007, 1 << 40] {
            let ns = ns_of_cycle(c);
            assert!(cycle_of_ns(ns) <= c && ns_of_cycle(cycle_of_ns(ns)) <= ns);
            assert!(ns_of_cycle(cycle_of_ns(ns + 1)) > ns, "the first cycle at or after ns+1 is not before it");
        }
        assert_eq!(cycle_of_ns(1_000_000_000), 160_000_000);
        assert_eq!(ns_of_cycle(160_000_000), 1_000_000_000);
        assert_eq!(cycle_of_ns(1), 1);
        assert_eq!(cycle_of_ns(7), 2);
        assert_eq!(ns_of_cycle(u64::MAX), u64::MAX);
        assert_eq!(cycle_of_ns(u64::MAX), 2_951_479_051_793_528_259);
    }

    #[test]
    fn json_roundtrip_of_a_step() {
        let m = parse_json(r#"{"type":"step","t":1500000000,"in":[{"type":"rx","t":1400000000,"from":1,"ch":26,"rssi":-65,"frame":"41c856cdab"}],"x":-1.5e2,"s":"a\"b\\né"}"#).unwrap();
        assert_eq!(m.str_or("type", ""), "step");
        assert_eq!(m.i64_or("t", 0), 1_500_000_000);
        let inputs = parse_inputs(&m);
        assert_eq!(inputs.len(), 1);
        match &inputs[0] { Input::Rx { t, frame, rssi, lqi } => { assert_eq!((*t, frame.as_slice(), *rssi, *lqi), (1_400_000_000, &[0x41, 0xc8, 0x56, 0xcd, 0xab][..], -65, 255)); } _ => panic!() }
        assert_eq!(m.get("x").unwrap().as_f64(), Some(-150.0));
        assert_eq!(m.str_or("s", ""), "a\"b\\né");
        assert!(parse_json("{\"a\":1} x").is_err());
        assert!(parse_json("[1,2,]").is_err());
    }

    #[test]
    fn hex_decode_rejects_non_ascii_without_slicing_utf8() {
        assert_eq!(hex_decode("41c8FF"), Some(vec![0x41, 0xc8, 0xff]));
        for text in ["aéa", "é", "abc", "0g", "+1"] { assert_eq!(hex_decode(text), None); }
    }

    #[test]
    fn mac_follows_the_node_id() { assert_eq!(mac_for_node(3), [2, 0, 0, 0, 0, 3]); assert_eq!(mac_for_node(0x1_0203), [2, 0, 0, 1, 2, 3]); }
}
