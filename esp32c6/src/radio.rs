//! The IEEE 802.15.4 MAC (`IEEE802154`, 0x600A3000), as ESP-IDF's `esp_ieee802154` driver
//! drives it: the register layout of `soc/ieee802154_struct.h`, the command byte at 0x00, the
//! event register pair at 0x60/0x64, the DMA addresses the driver points at its TX and RX
//! buffers, and the TX/RX state machine with the timing of the air (32 µs per byte, 6 bytes of
//! PHY overhead around the PSDU): frames leave and arrive, the hardware acknowledges what is
//! addressed to it and waits for the acknowledgement of what it sent, filters addresses and
//! assesses the channel before a CCA transmit.
//!
//! What the driver (`esp_ieee802154_dev.c`, IDF 5.5) actually does, read from its source and
//! confirmed with register traces of Contiki-NG's radio on this model:
//!
//! - **transmit**: `STOP`, `TIMER0_STOP`, `TIMER1_STOP`, read + clear events (the previous
//!   operation is torn down), then (if the PIB changed) freq/power/ED/conf writes, then
//!   `DMA_TX_ADDR` = the frame buffer (`buf[0]` = PSDU length *including* the 2-byte FCS,
//!   `buf[1..]` the MAC frame without FCS — hardware appends the CRC). For a frame with the AR
//!   bit the driver also points `DMA_RX_ADDR` at a free buffer (`set_next_rx_buffer`): that is
//!   where the acknowledgement lands. Then `TX_START` (or `CCA_TX_START`). The ISR expects
//!   `TX_SFD_DONE` and `TX_DONE`; for an AR frame with `CONF.auto_ack_rx` it then moves to
//!   `RX_ACK`, arms TIMER0 for 200 ms with a NO_ACK callback, and waits for `ACK_RX_DONE`, on
//!   which it reads the ACK from the RX buffer (`[len incl. FCS][frame][RSSI][LQI]`, as any
//!   frame) and reports `transmit_done(frame, ack, ack_info)`. Hardware has its own
//!   `ACK_TIMEOUT` register (0x5c, units of 16 µs, `TX_ABORT` reason `RX_ACK_TIMEOUT`); the
//!   driver never programs it, so the 200 ms timer is what fires without an ACK.
//! - **receive**: the same teardown, `DMA_RX_ADDR` = a free 129-byte buffer, `RX_START`. A
//!   frame that passes the address filter raises `RX_SFD_DONE` and then `RX_DONE`; one that
//!   does not is dropped by hardware (`RX_ABORT` reason `FILTER_FAIL`, an event the driver does
//!   not enable). For a frame with the AR bit (frame version 0/1, `CONF.auto_ack_tx`) hardware
//!   sends the acknowledgement itself after the 12-symbol turnaround and raises `ACK_TX_DONE`;
//!   the driver moves to `TX_ACK` on `RX_DONE` and only hands the frame up and re-arms RX on
//!   `ACK_TX_DONE`. Without a filter, a frame *not* addressed to us with the AR bit left the
//!   driver in `TX_ACK` for good: every later transmit was refused before touching hardware
//!   (`TX_ERR_ABORT`) and RX never re-armed — the stage-1 failure in an RPL network.
//!   Before a TX the driver reads `RX_STATUS.rx_state`: `> 1` means "a frame is coming in".
//! - **stop** during a frame raises `RX_ABORT` (reason `RX_STOP`); during a TX, `TX_ABORT`
//!   (reason `TX_STOP`) — abort events fire only for the reasons the driver enabled in
//!   `RX_ABORT_EVENT_EN` / `TX_ABORT_EVENT_EN` (it enables `TX_ACK_TIMEOUT`, `TX_ACK_COEX_BREAK`
//!   and `RX_ACK_TIMEOUT`, `TX_COEX_BREAK`, `TX_SECURITY_ERROR`, `CCA_FAILED`, `CCA_BUSY`).
//! - **CCA**: `CCA_TX_START` transmits unless the channel is busy, which here means a frame
//!   is being received; busy is `TX_ABORT` reason `CCA_BUSY`, and the driver reports
//!   `TX_ERR_CCA_BUSY`.
//!
//! Timing: the acknowledgement starts 192 µs (12 symbols) after the end of the acknowledged
//! frame and occupies 11 bytes of air (6 of PHY overhead, 3 of MAC, 2 of FCS), 352 µs. It goes
//! out through the same host path as a data frame — `tx_started` at its first cycle — so a
//! lock-stepped medium sees it in time for the sender's ACK wait.
//!
//! Not modelled: enhanced ACKs (frame version 2), security, the frame-pending table (the ACK's
//! pending bit is 0), CCA against energy that is not a frame.
//!
//! The DMA side is the bus's: the device cannot see SRAM, so it leaves a `tx_request` (fetch
//! the frame at this address) and an `rx_write` (store this buffer there) for `SocBus` to
//! carry out, which it does right after the register write and right after the device tick.
//!
//! The energy-detect scan (`ED_START`/`ED_DONE`) keeps the synthetic 2.4 GHz scene from before
//! this file existed: a quiet floor with the three non-overlapping WiFi channels on top of it,
//! moving deterministically from one xorshift (the energy-scan golden pins it).
use crate::periph::CPU_HZ;
use emu_core::ClockDomain;
use esp_periph::{Device, RegRam, WriteEffect};
use std::collections::HashSet;

pub const CMD_TX_START: u32 = 0x41;
pub const CMD_RX_START: u32 = 0x42;
pub const CMD_CCA_TX_START: u32 = 0x43;
pub const CMD_ED_START: u32 = 0x44;
pub const CMD_STOP: u32 = 0x45;
pub const CMD_TIMER0_START: u32 = 0x4c;
pub const CMD_TIMER0_STOP: u32 = 0x4d;
pub const CMD_TIMER1_START: u32 = 0x4e;
pub const CMD_TIMER1_STOP: u32 = 0x4f;

pub const EVENT_TX_DONE: u32 = 1 << 0;
pub const EVENT_RX_DONE: u32 = 1 << 1;
pub const EVENT_ACK_TX_DONE: u32 = 1 << 2;
pub const EVENT_ACK_RX_DONE: u32 = 1 << 3;
pub const EVENT_RX_ABORT: u32 = 1 << 4;
pub const EVENT_TX_ABORT: u32 = 1 << 5;
pub const EVENT_ED_DONE: u32 = 1 << 6;
pub const EVENT_TIMER0_OVERFLOW: u32 = 1 << 8;
pub const EVENT_TIMER1_OVERFLOW: u32 = 1 << 9;
pub const EVENT_TX_SFD_DONE: u32 = 1 << 11;
pub const EVENT_RX_SFD_DONE: u32 = 1 << 12;

pub const RX_ABORT_BY_RX_STOP: u32 = 1;
pub const RX_ABORT_BY_FILTER_FAIL: u32 = 5;
pub const RX_ABORT_BY_UNEXPECTED_ACK: u32 = 8;
pub const TX_ABORT_BY_RX_ACK_TIMEOUT: u32 = 16;
pub const TX_ABORT_BY_TX_STOP: u32 = 17;
pub const TX_ABORT_BY_CCA_BUSY: u32 = 25;

/// `CONF` (0x04) bits the model reads.
pub const CONF_AUTO_ACK_TX: u32 = 1 << 0;
pub const CONF_AUTO_ACK_RX: u32 = 1 << 3;
pub const CONF_COORDINATOR: u32 = 1 << 6;
pub const CONF_PROMISCUOUS: u32 = 1 << 7;

/// 802.15.4 O-QPSK at 2.4 GHz: 250 kbit/s, 32 µs per byte.
pub const NS_PER_BYTE: u64 = 32_000;
/// Bytes on the air around the PSDU: 4 preamble + SFD + PHY length.
pub const PHY_OVERHEAD_BYTES: u64 = 6;
/// Preamble + SFD: what precedes the length byte, so where `*_SFD_DONE` lands after a start.
pub const SFD_BYTES: u64 = 5;
/// The acknowledgement follows the acknowledged frame after 12 symbols (aTurnaroundTime).
pub const ACK_TURNAROUND_NS: u64 = 192_000;
/// An immediate ACK: FCF, sequence number (+ 2 FCS on the air).
pub const ACK_MAC_BYTES: u64 = 3;
/// `IEEE802154_MAC_DATE_VERSION` in the register header.
pub const MAC_DATE: u32 = 0x22_0622;

/// Cycles of the 160 MHz CPU clock that `bytes` occupy on the air.
pub const fn air_cycles(bytes: u64) -> u64 { bytes * NS_PER_BYTE * (CPU_HZ / 1_000_000) / 1000 }
const fn ns_cycles(ns: u64) -> u64 { ns * (CPU_HZ / 1_000_000) / 1000 }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RadioState {
    /// nothing armed (after `STOP`, after a completed TX/RX, at reset)
    Idle,
    /// `RX_START`: listening
    Rx,
    /// a frame is coming in (energy on the air from its first preamble byte, so CCA sees it;
    /// `RX_SFD_DONE` and then `RX_DONE` still pending)
    RxFrame,
    /// `TX_START` / `CCA_TX_START`: a frame is going out
    Tx,
    /// our AR frame is out: waiting for its acknowledgement (`ACK_RX_DONE`)
    RxAck,
    /// a received AR frame is being acknowledged: turnaround, then the ACK on the air (`ACK_TX_DONE`)
    TxAck,
    /// `ED_START`: an energy scan is sampling
    Ed,
}

/// A countdown in CPU cycles. One armed from a register write is not decremented by the tick
/// that ends the round the write sat in (the round's cycles precede the write), so it starts
/// counting from the instruction after the write; one armed between rounds (a frame injected
/// by the host at an exact cycle) counts from the very next tick; one armed at another
/// countdown's expiry, inside a tick, starts at that expiry: `chain` takes the cycles the
/// expiring tick overshot by, and the rest of that tick is skipped.
#[derive(Clone, Copy, Debug, Default)]
struct Countdown { left: Option<u64>, fresh: bool }
impl Countdown {
    fn arm(&mut self, cycles: u64, from_write: bool) { self.left = Some(cycles); self.fresh = from_write; }
    fn chain(&mut self, cycles: u64, overshoot: u64) { self.left = Some(cycles.saturating_sub(overshoot).max(1)); self.fresh = true; }
    fn clear(&mut self) { self.left = None; self.fresh = false; }
    fn armed(&self) -> bool { self.left.is_some() }
    /// Advance; `Some(overshoot)` if the countdown expired in this tick, with the cycles of the
    /// tick that came after the expiry.
    fn tick(&mut self, cycles: u64) -> Option<u64> {
        let left = self.left?;
        if self.fresh { self.fresh = false; return None; }
        if cycles >= left { self.left = None; Some(cycles - left) } else { self.left = Some(left - cycles); None }
    }
    fn remaining(&self) -> u64 { self.left.unwrap_or(u64::MAX) }
}

/// A frame received from the medium, waiting for its `RX_DONE`.
#[derive(Clone, Debug)]
pub struct RxFrame { pub frame: Vec<u8>, pub rssi: i8, pub lqi: u8 }

/// Radio timer 0/1: microsecond counters (the driver programs thresholds in µs) that raise
/// `TIMERn_OVERFLOW` at the threshold.
#[derive(Clone, Copy, Debug, Default)]
struct RadioTimer { running: bool, threshold: u32, cycles: u64 }
impl RadioTimer {
    const CYCLES_PER_US: u64 = CPU_HZ / 1_000_000;
    fn value(&self) -> u32 { (self.cycles / Self::CYCLES_PER_US) as u32 }
    fn start(&mut self) { self.running = true; self.cycles = 0; }
    fn stop(&mut self) { self.running = false; }
    /// Advance; true if the threshold was reached in this tick (the timer then stops).
    fn tick(&mut self, cycles: u64) -> bool {
        if !self.running { return false; }
        self.cycles += cycles;
        if self.value() >= self.threshold { self.running = false; true } else { false }
    }
    fn deadline(&self) -> Option<u64> {
        if !self.running { return None; }
        Some(((self.threshold as u64) * Self::CYCLES_PER_US).saturating_sub(self.cycles).max(1))
    }
}

/// The addressing fields of a MAC header, as far as the filter needs them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MacHeader {
    pub frame_type: u8, pub ack_request: bool, pub version: u8, pub seq: u8,
    pub dst_pan: Option<u16>, pub dst_short: Option<u16>, pub dst_ext: Option<[u8; 8]>,
    pub src_pan: Option<u16>,
}

/// Parse the frame control field and the addresses (on-air byte order, least significant
/// first). `None` if the frame is shorter than its header says.
pub fn parse_header(f: &[u8]) -> Option<MacHeader> {
    if f.len() < 3 { return None; }
    let fcf = u16::from_le_bytes([f[0], f[1]]);
    let mut h = MacHeader { frame_type: (fcf & 7) as u8, ack_request: fcf & 0x20 != 0, version: ((fcf >> 12) & 3) as u8, seq: f[2], ..Default::default() };
    let pan_comp = fcf & 0x40 != 0;
    let (dst_mode, src_mode) = ((fcf >> 10) & 3, (fcf >> 14) & 3);
    let mut i = 3;
    let take = |i: &mut usize, n: usize| -> Option<&[u8]> { let s = f.get(*i..*i + n)?; *i += n; Some(s) };
    if dst_mode != 0 {
        let p = take(&mut i, 2)?; h.dst_pan = Some(u16::from_le_bytes([p[0], p[1]]));
        match dst_mode { 2 => { let a = take(&mut i, 2)?; h.dst_short = Some(u16::from_le_bytes([a[0], a[1]])); } 3 => { let a = take(&mut i, 8)?; h.dst_ext = Some(a.try_into().unwrap()); } _ => return None }
    }
    if src_mode != 0 {
        if !pan_comp { let p = take(&mut i, 2)?; h.src_pan = Some(u16::from_le_bytes([p[0], p[1]])); } else { h.src_pan = h.dst_pan; }
    }
    Some(h)
}

pub struct Ieee802154 {
    ram: RegRam,
    pub state: RadioState,
    pub events: u32, pub event_en: u32,
    pub freq: u32, pub txpower: u32, pub duration: u32,
    pub dma_tx_addr: u32, pub dma_rx_addr: u32,
    pub rx_length: u32,
    rx_abort_reason: u32, tx_abort_reason: u32,
    // ---- TX
    /// `TX_START` was written: the bus fetches the frame at this address and calls `tx_loaded`
    pub tx_request: Option<u32>,
    /// the MAC frame (no length byte, no FCS) going out, kept until `TX_DONE`; the ACK while one is out
    pub tx_frame: Vec<u8>,
    tx_sfd: Countdown, tx_done: Countdown,
    /// a transmission (a frame or an ACK) started at the instruction that just executed: the host must see it now
    pub tx_started: bool,
    pub tx_count: u64,
    // ---- the ACK we wait for
    /// the sequence number of our AR frame, while in `RxAck`
    ack_wait_seq: u8,
    /// `ACK_TIMEOUT` (0x5c) running, in `RxAck`
    ack_timeout: Countdown,
    pub ack_rx_count: u64,
    // ---- the ACK we send
    ack_turnaround: Countdown, ack_done: Countdown,
    ack_frame: [u8; 3],
    pub ack_tx_count: u64,
    // ---- RX
    rx_in_flight: Option<RxFrame>,
    rx_sfd: Countdown, rx_done: Countdown,
    /// the completed frame's buffer, for the bus to store at `.0`
    pub rx_write: Option<(u32, Vec<u8>)>,
    pub rx_count: u64,
    /// frames the medium offered while the radio was not listening (or mid-frame)
    pub rx_dropped: u64,
    /// frames the address filter rejected
    pub rx_filtered: u64,
    timer: [RadioTimer; 2],
    // ---- energy detect (the synthetic scene)
    pub ed_rss: i8, pub ed_left: Option<u64>, pub scans: u64,
    /// dBm per 802.15.4 channel 11..26, before noise
    pub level_dbm: [i8; 16],
    /// where each channel is drifting to, 1 dB per 100 ms
    pub target_dbm: [i8; 16],
    noise: u32, scene_acc: u64, scene_ticks: u64,
    /// Something besides the scene may be due: an energy scan, a countdown or a radio timer.
    /// Every entry that can arm one (a register write, `receive`, `tx_loaded`) sets it; the tick
    /// clears it once nothing is armed, so an idle radio's tick is the scene alone.
    live: bool,
    // ---- diagnostics
    /// `--debug ieee802154`: narrate commands, events and DMA
    pub dbg: bool,
    /// `--log-periph`: report the first touch of an offset the model does not interpret
    pub log_unknown: bool,
    seen: HashSet<(u32, bool)>,
}
impl Default for Ieee802154 { fn default() -> Self { Self::new() } }

impl Ieee802154 {
    pub fn new() -> Self {
        // WiFi channels 1, 6 and 11 overlap 802.15.4 channels 11-14, 16-19 and 21-24
        let mut level_dbm = [-93i8; 16];
        for (i, l) in [(0, -68), (1, -52), (2, -49), (3, -66), (5, -63), (6, -47), (7, -45), (8, -60), (10, -74), (11, -56), (12, -58), (13, -71), (15, -90)] { level_dbm[i] = l; }
        Ieee802154 {
            ram: RegRam::new(), state: RadioState::Idle, events: 0, event_en: 0, freq: 3, txpower: 0, duration: 0,
            dma_tx_addr: 0, dma_rx_addr: 0, rx_length: 0, rx_abort_reason: 0, tx_abort_reason: 0,
            tx_request: None, tx_frame: Vec::new(), tx_sfd: Countdown::default(), tx_done: Countdown::default(), tx_started: false, tx_count: 0,
            ack_wait_seq: 0, ack_timeout: Countdown::default(), ack_rx_count: 0,
            ack_turnaround: Countdown::default(), ack_done: Countdown::default(), ack_frame: [0; 3], ack_tx_count: 0,
            rx_in_flight: None, rx_sfd: Countdown::default(), rx_done: Countdown::default(), rx_write: None, rx_count: 0, rx_dropped: 0, rx_filtered: 0,
            timer: [RadioTimer::default(); 2],
            ed_rss: -92, ed_left: None, scans: 0, level_dbm, target_dbm: level_dbm, noise: 0x9e37_79b9, scene_acc: 0, scene_ticks: 0, live: false,
            dbg: false, log_unknown: false, seen: HashSet::new(),
        }
    }

    /// the channel the frequency register selects: freq = 3 + 5 * (channel - 11)
    pub fn channel(&self) -> u8 { (11 + ((self.freq.saturating_sub(3)) / 5) as u8).clamp(11, 26) }
    fn conf(&self, bit: u32) -> bool { self.ram.read(0x04) & bit != 0 }
    /// `CONF.promiscuous`
    pub fn promiscuous(&self) -> bool { self.conf(CONF_PROMISCUOUS) }
    /// Listening (for anything, or for an ACK), or receiving a frame.
    pub fn listening(&self) -> bool { matches!(self.state, RadioState::Rx | RadioState::RxFrame | RadioState::RxAck) }
    pub fn transmitting(&self) -> bool { matches!(self.state, RadioState::Tx | RadioState::TxAck) }

    /// The multipan entries `CONF.multipan_mask` enables: (PAN id, short address, extended
    /// address in on-air order — least significant byte first, which is how the driver's
    /// `set_extended_address` takes it and how Contiki's port writes it).
    fn addresses(&self) -> Vec<(u16, u16, [u8; 8])> {
        let mask = (self.ram.read(0x04) >> 28) & 0xf;
        (0..4).filter(|i| mask & (1 << i) != 0).map(|i| {
            let base = 0x08 + 0x10 * i;
            let (short, pan, e0, e1) = (self.ram.read(base) as u16, self.ram.read(base + 4) as u16, self.ram.read(base + 8), self.ram.read(base + 12));
            let mut ext = [0u8; 8]; ext[..4].copy_from_slice(&e0.to_le_bytes()); ext[4..].copy_from_slice(&e1.to_le_bytes());
            (pan, short, ext)
        }).collect()
    }

    /// Third-level filtering, as the standard has it and as hardware must for the driver's
    /// state machine to hold: a destination PAN of ours or 0xffff, a destination address of
    /// ours or the short broadcast; a frame without a destination only for a coordinator on its
    /// PAN, a beacon on its source PAN. Promiscuous takes everything with a header.
    pub fn accepts(&self, h: &MacHeader) -> bool {
        if self.promiscuous() { return true; }
        if h.frame_type == 2 { return false; }                       // an ACK we are not waiting for
        let mine = self.addresses();
        // The PAN and destination address must match the same enabled multipan entry.
        let pan_ok = |pan: u16, own_pan: u16| pan == 0xffff || own_pan == pan || own_pan == 0xffff;
        match (h.dst_pan, h.dst_short, h.dst_ext) {
            (Some(pan), Some(0xffff), _) => pan == 0xffff || mine.iter().any(|m| pan_ok(pan, m.0)),
            (Some(pan), Some(short), _) => mine.iter().any(|m| pan_ok(pan, m.0) && m.1 == short),
            (Some(pan), _, Some(ext)) => mine.iter().any(|m| pan_ok(pan, m.0) && m.2 == ext),
            _ => match h.src_pan {
                Some(pan) if h.frame_type == 0 => pan == 0xffff || mine.iter().any(|m| pan_ok(pan, m.0)),
                Some(pan) => self.conf(CONF_COORDINATOR) && (pan == 0xffff || mine.iter().any(|m| pan_ok(pan, m.0))),
                None => false,
            },
        }
    }

    // ------------------------------------------------------------------ energy scan scene
    pub fn set_channel_dbm(&mut self, channel: u8, dbm: i8) { if (11..=26).contains(&channel) { let i = (channel - 11) as usize; self.level_dbm[i] = dbm; self.target_dbm[i] = dbm; } }
    fn rand(&mut self) -> u32 { self.noise ^= self.noise << 13; self.noise ^= self.noise >> 17; self.noise ^= self.noise << 5; self.noise }
    /// One 100 ms step of the scene: levels drift toward their targets; every 2.5 s something
    /// happens — a WiFi network moves (channel 1, 6 or 11: 802.15.4 channels 11-14, 16-19, 21-24)
    /// or a burst lands on one channel and is left to fade back to the floor.
    fn scene_step(&mut self) {
        self.scene_ticks += 1;
        if self.scene_ticks.is_multiple_of(25) {
            let r = self.rand();
            if r.is_multiple_of(4) {
                let ch = (self.rand() % 16) as usize; self.target_dbm[ch] = self.target_dbm[ch].max(-50 - (self.rand() % 12) as i8); self.level_dbm[ch] = self.target_dbm[ch];
            } else {
                let base = [0usize, 5, 10][(r / 4 % 3) as usize];
                let peak = -44 - (self.rand() % 28) as i8;
                for (k, drop) in [(0, 6), (1, 0), (2, 2), (3, 9)] { self.target_dbm[base + k] = peak - drop; }
            }
        }
        for i in 0..16 {
            let wifi = matches!(i, 0..=3 | 5..=8 | 10..=13);
            if !wifi && self.target_dbm[i] > -93 && self.scene_ticks.is_multiple_of(5) { self.target_dbm[i] -= 1; }   // a burst fades
            let (l, t) = (self.level_dbm[i], self.target_dbm[i]);
            self.level_dbm[i] = if l < t { l + 1 } else if l > t { l - 1 } else { l };
        }
    }
    fn sample(&mut self) -> i8 {
        let jitter = (self.rand() % 7) as i8 - 3;
        (self.level_dbm[(self.channel() - 11) as usize] as i16 + jitter as i16).clamp(-127, 0) as i8
    }

    // ------------------------------------------------------------------ the medium's side
    /// A frame (MAC header + payload, no FCS) from the medium. `started_ago` = `Some(cycles)`:
    /// the frame started on the air that many cycles ago (0: now), first preamble byte first —
    /// `RX_SFD_DONE` follows `SFD_BYTES` into it and `RX_DONE` when the whole PPDU is up, either
    /// at once if it already is; `None`: the frame is complete now, SFD and `RX_DONE` together.
    /// Listening: the address filter decides. Waiting
    /// for an ACK: only the ACK with our sequence number counts. Anything else is dropped, and
    /// counted. Returns whether the frame was taken.
    pub fn receive(&mut self, frame: &[u8], rssi: i8, lqi: u8, started_ago: Option<u64>) -> bool {
        self.live = true;
        let header = parse_header(frame);
        let taken = match (self.state, header) {
            (RadioState::Rx, Some(h)) if frame.len() <= 125 => {
                if self.accepts(&h) { true } else {
                    self.rx_filtered += 1;
                    self.rx_abort(RX_ABORT_BY_FILTER_FAIL);
                    if self.dbg { eprintln!("[802.15.4] rx of {} bytes filtered: type {} dst {:?}/{:?}", frame.len(), h.frame_type, h.dst_pan, h.dst_short); }
                    return false;
                }
            }
            (RadioState::RxAck, Some(h)) => h.frame_type == 2 && frame.len() == 3 && h.seq == self.ack_wait_seq,
            _ => false,
        };
        if !taken {
            self.rx_dropped += 1;
            if self.dbg { eprintln!("[802.15.4] rx of {} bytes dropped: state {:?}", frame.len(), self.state); }
            return false;
        }
        if self.state == RadioState::Rx { self.state = RadioState::RxFrame; }
        self.rx_in_flight = Some(RxFrame { frame: frame.to_vec(), rssi, lqi });
        // `started_ago` is measured from the frame's FIRST PREAMBLE BYTE, not from the SFD:
        // that is what the host's `rx.t` names, and the frame handed over has both the 6-byte
        // PHY header and the 2-byte FCS stripped.  So what is left is the whole thing on the
        // air -- preamble + SFD + length + PSDU + FCS -- minus what has already gone by.
        // Counting from the SFD instead lands RX_DONE five byte times (160 us) early, and with
        // it every acknowledgement timed off RX_DONE.
        //
        // The SFD itself lands `SFD_BYTES` into the frame, exactly as the transmitter raises
        // `TX_SFD_DONE`: the preamble is on the air first. IDF timestamps a frame by reading
        // esp_timer in the `RX_SFD_DONE` handler, so this cycle is the received timestamp.
        let sfd_in = started_ago.map_or(0, |ago| air_cycles(SFD_BYTES).saturating_sub(ago));
        if sfd_in > 0 {
            self.rx_sfd.arm(sfd_in, false);
        } else {
            self.events |= EVENT_RX_SFD_DONE;
        }
        let left = started_ago.map(|ago| air_cycles(PHY_OVERHEAD_BYTES + frame.len() as u64 + 2).saturating_sub(ago));
        if let Some(left) = left.filter(|&l| l > 0) {
            self.rx_done.arm(left, false);
            if self.dbg { eprintln!("[802.15.4] rx {} bytes rssi {} lqi {}: SFD in {} cycles, RX_DONE in {} cycles", frame.len(), rssi, lqi, sfd_in, left); }
        } else {
            if self.dbg { eprintln!("[802.15.4] rx {} bytes rssi {} lqi {}: complete now", frame.len(), rssi, lqi); }
            self.finish_rx(None);
        }
        true
    }

    /// The frame in flight is complete: lay it out as the driver reads it and raise `RX_DONE`
    /// — or `ACK_RX_DONE` for the acknowledgement we were waiting for. A frame with the AR bit
    /// (frame version 0 or 1, `CONF.auto_ack_tx`) is acknowledged from here: the turnaround
    /// starts now — `overshoot` is how far the tick that completed the frame ran past its end.
    fn finish_rx(&mut self, overshoot: Option<u64>) {
        let Some(rx) = self.rx_in_flight.take() else { if self.state == RadioState::RxFrame { self.state = RadioState::Idle; } return };
        let mut buf = Vec::with_capacity(rx.frame.len() + 3);
        buf.push((rx.frame.len() + 2) as u8);
        buf.extend_from_slice(&rx.frame);
        buf.push(rx.rssi as u8);
        buf.push(rx.lqi);
        self.rx_length = (rx.frame.len() + 2) as u32;
        self.rx_write = Some((self.dma_rx_addr, buf));
        if self.state == RadioState::RxAck {
            self.ack_timeout.clear();
            self.ack_rx_count += 1;
            self.events |= EVENT_ACK_RX_DONE;
            self.state = RadioState::Idle;
            if self.dbg { eprintln!("[802.15.4] ACK_RX_DONE: seq {} to {:#010x}", rx.frame[2], self.dma_rx_addr); }
            return;
        }
        self.rx_count += 1;
        self.events |= EVENT_RX_DONE;
        if self.dbg { eprintln!("[802.15.4] RX_DONE: {} bytes to {:#010x}", rx.frame.len(), self.dma_rx_addr); }
        let h = parse_header(&rx.frame).unwrap_or_default();
        if h.ack_request && h.version <= 1 && self.conf(CONF_AUTO_ACK_TX) {
            self.state = RadioState::TxAck;
            self.ack_frame = [0x02, (h.version << 4), h.seq];
            match overshoot { Some(o) => self.ack_turnaround.chain(ns_cycles(ACK_TURNAROUND_NS), o), None => self.ack_turnaround.arm(ns_cycles(ACK_TURNAROUND_NS), false) }
            if self.dbg { eprintln!("[802.15.4] acknowledging seq {} in {} cycles", h.seq, self.ack_turnaround.remaining()); }
        } else {
            self.state = RadioState::Idle;
        }
    }

    /// The bus fetched the frame `TX_START` pointed at: `psdu` is the MAC frame without its
    /// length byte and without the FCS (the hardware appends the CRC). Starts the air-time
    /// clock and flags the start for the host.
    pub fn tx_loaded(&mut self, psdu: Vec<u8>) {
        self.live = true;
        self.tx_sfd.arm(air_cycles(SFD_BYTES), true);
        self.tx_done.arm(air_cycles(PHY_OVERHEAD_BYTES + psdu.len() as u64 + 2), true);
        if self.dbg { eprintln!("[802.15.4] tx {} bytes on channel {}: TX_DONE in {} cycles", psdu.len(), self.channel(), self.tx_done.remaining()); }
        self.tx_frame = psdu;
        self.tx_started = true;
        self.tx_count += 1;
    }

    /// The host's yield flag: a transmission began at the instruction (or the device tick)
    /// that just ran.
    pub fn take_tx_started(&mut self) -> bool { std::mem::take(&mut self.tx_started) }

    // ------------------------------------------------------------------ commands
    fn command(&mut self, cmd: u32) {
        if self.dbg { eprintln!("[802.15.4] cmd {:#04x} in state {:?}", cmd, self.state); }
        match cmd {
            CMD_CCA_TX_START if self.state == RadioState::RxFrame => {
                // the channel carries a frame: busy, and the receive goes on
                self.tx_abort(TX_ABORT_BY_CCA_BUSY);
                if self.dbg { eprintln!("[802.15.4] CCA busy: a frame is being received"); }
            }
            CMD_TX_START | CMD_CCA_TX_START => {
                self.abort_rx_silently();
                self.state = RadioState::Tx;
                self.tx_request = Some(self.dma_tx_addr);
            }
            CMD_RX_START => { self.abort_rx_silently(); self.state = RadioState::Rx; }
            CMD_ED_START => {
                self.state = RadioState::Ed;
                self.ed_left = Some((self.duration.max(1) as u64) * 16 * (CPU_HZ / 1_000_000));   // symbols of 16 µs
            }
            CMD_STOP => {
                if self.state == RadioState::RxFrame { self.rx_abort(RX_ABORT_BY_RX_STOP); }
                if self.state == RadioState::Tx && self.tx_done.armed() { self.tx_abort(TX_ABORT_BY_TX_STOP); }
                self.abort_rx_silently();
                self.tx_sfd.clear(); self.tx_done.clear();
                self.ed_left = None;
                self.state = RadioState::Idle;
            }
            CMD_TIMER0_START => { self.timer[0].start(); }
            CMD_TIMER0_STOP => { self.timer[0].stop(); }
            CMD_TIMER1_START => { self.timer[1].start(); }
            CMD_TIMER1_STOP => { self.timer[1].stop(); }
            _ => { if self.dbg || self.log_unknown { eprintln!("[802.15.4] unsupported command {:#04x}", cmd); } }
        }
    }
    /// Leave `Rx`/`RxFrame`/`RxAck`/`TxAck` for another operation: whatever was in flight is lost.
    fn abort_rx_silently(&mut self) {
        if self.rx_in_flight.take().is_some() { self.rx_dropped += 1; }
        self.rx_sfd.clear(); self.rx_done.clear(); self.ack_timeout.clear(); self.ack_turnaround.clear(); self.ack_done.clear();
    }
    /// `RX_ABORT` with a reason, if the driver enabled that reason (`RX_ABORT_EVENT_EN`, bit reason-1).
    fn rx_abort(&mut self, reason: u32) {
        self.rx_abort_reason = reason;
        if self.ram.read(0x68) & (1 << (reason - 1)) != 0 { self.events |= EVENT_RX_ABORT; }
    }
    /// `TX_ABORT` with a reason, if the driver enabled that reason (`TX_ABORT_EVENT_EN`, bit reason-1).
    fn tx_abort(&mut self, reason: u32) {
        self.tx_abort_reason = reason;
        if self.ram.read(0x78) & (1 << (reason - 1)) != 0 { self.events |= EVENT_TX_ABORT; }
    }

    fn note_unknown(&mut self, off: u32, write: bool, v: u32) {
        if !(self.dbg || self.log_unknown) { return; }
        if self.seen.insert((off, write)) {
            eprintln!("[802.15.4] {} unmodelled register +0x{:03x}{}", if write { "W" } else { "R" }, off, if write { format!(" = {:#x}", v) } else { String::new() });
        }
    }

    fn rx_status(&self) -> u32 {
        let rx_state = match self.state { RadioState::Rx | RadioState::RxAck => 1, RadioState::RxFrame => 2, _ => 0 };
        let sync = if self.state == RadioState::RxFrame { (1 << 20) | (1 << 21) } else { 0 };
        (self.rx_abort_reason & 0x1f) << 4 | rx_state << 16 | sync
    }
    fn tx_status(&self) -> u32 {
        let tx_state = if self.transmitting() { 1 } else { 0 };
        tx_state | (self.tx_abort_reason & 0x1f) << 4
    }
    fn txrx_status(&self) -> u32 {
        let (tx, rx, ed) = (self.transmitting(), self.listening(), self.state == RadioState::Ed);
        (tx as u32) << 8 | (rx as u32) << 9 | (ed as u32) << 10
    }
}

impl Device for Ieee802154 {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            0x00 => 0,
            0x48 => self.freq, 0x4c => self.txpower, 0x50 => self.duration,
            0x54 => (self.ram.read(off) & !0x00ff_0000) | ((self.ed_rss as u8 as u32) << 16),   // ED_SCAN_CFG.ED_RSS; CCA_BUSY stays clear
            0x60 => self.event_en, 0x64 => self.events,
            0x80 => self.rx_status(), 0x84 => self.tx_status(), 0x88 => self.txrx_status(),
            0xa4 => self.rx_length,
            0xa8 => self.timer[0].threshold, 0xac => self.timer[0].value(),
            0xb0 => self.timer[1].threshold, 0xb4 => self.timer[1].value(),
            0xd0 => self.dma_tx_addr, 0xe0 => self.dma_rx_addr,
            0x184 => MAC_DATE,
            // configuration the driver writes and reads back: CONF, the multipan addresses, IFS,
            // ACK timeout, abort enables, pending, PTI, DMA config, the delay table, security
            0x04 | 0x08..=0x44 | 0x58 | 0x5c | 0x68 | 0x6c | 0x70 | 0x78 | 0x7c | 0x90 | 0xb8 | 0xbc | 0xc0..=0xcc | 0xd4 | 0xd8 | 0xe4 | 0xe8 | 0xf0 | 0xf4 | 0x100..=0x120 | 0x128..=0x140 => self.ram.read(off),
            0x144..=0x180 => 0,                                                                  // debug counters
            _ => { self.note_unknown(off, false, 0); self.ram.read(off) }
        }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        self.live = true;
        match off {
            0x00 => self.command(v & 0xff),
            0x48 => self.freq = v & 0x7f, 0x4c => self.txpower = v & 0x1f, 0x50 => self.duration = v & 0xff_ffff,
            0x54 => self.ram.write(off, v & 0x0000_ffff),                                         // ED_RSS / CCA_BUSY are read-only
            0x60 => self.event_en = v & 0x1fff, 0x64 => self.events &= !v,                          // EVENT_STATUS: write 1 to clear
            0xa8 => self.timer[0].threshold = v, 0xb0 => self.timer[1].threshold = v,
            0xd0 => self.dma_tx_addr = v, 0xe0 => self.dma_rx_addr = v,
            0x04 | 0x08..=0x44 | 0x58 | 0x5c | 0x68 | 0x6c | 0x70 | 0x78 | 0x7c | 0x90 | 0xb8 | 0xbc | 0xc0..=0xcc | 0xd4 | 0xd8 | 0xe4 | 0xe8 | 0xf0 | 0xf4 | 0x100..=0x120 | 0x128..=0x140 | 0x180 => self.ram.write(off, v),
            _ => { self.note_unknown(off, true, v); self.ram.write(off, v) }
        }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { (self.events & self.event_en != 0) as u64 }
    fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::Cpu) }
    fn tick(&mut self, cycles: u64) {
        self.scene_acc += cycles;
        while self.scene_acc >= CPU_HZ / 10 { self.scene_acc -= CPU_HZ / 10; self.scene_step(); }
        // ed_left is public and may be armed directly without setting live.
        if !self.live && self.ed_left.is_none() { return; }
        if let Some(left) = self.ed_left {
            if cycles >= left { self.ed_left = None; self.ed_rss = self.sample(); self.scans += 1; self.events |= EVENT_ED_DONE; if self.state == RadioState::Ed { self.state = RadioState::Idle; } } else { self.ed_left = Some(left - cycles); }
        }
        if self.tx_sfd.tick(cycles).is_some() { self.events |= EVENT_TX_SFD_DONE; }
        if self.rx_sfd.tick(cycles).is_some() { self.events |= EVENT_RX_SFD_DONE; }
        if let Some(over) = self.tx_done.tick(cycles) {
            self.events |= EVENT_TX_DONE;
            if self.state == RadioState::Tx {
                let h = parse_header(&self.tx_frame).unwrap_or_default();
                if h.ack_request && self.conf(CONF_AUTO_ACK_RX) {
                    // wait for the acknowledgement; hardware's own timeout only if the driver set one
                    self.state = RadioState::RxAck;
                    self.ack_wait_seq = h.seq;
                    let timeout = self.ram.read(0x5c) & 0xffff;
                    if timeout != 0 { self.ack_timeout.chain(ns_cycles(timeout as u64 * 16_000), over); }
                    if self.dbg { eprintln!("[802.15.4] TX_DONE: waiting for the ACK of seq {}{}", h.seq, if timeout != 0 { format!(", {} µs", timeout * 16) } else { String::new() }); }
                } else {
                    self.state = RadioState::Idle;
                    if self.dbg { eprintln!("[802.15.4] TX_DONE"); }
                }
            }
        }
        if self.ack_timeout.tick(cycles).is_some() && self.state == RadioState::RxAck {
            // The driver has given up. An acknowledgement still on the air is no longer ours to
            // deliver: leaving it armed would let `finish_rx` run a moment later with the state
            // already back to `Idle`, take the data-frame branch, and report the ACK as a
            // received frame. Since the whole PPDU is charged, `RX_DONE` for an ACK lands
            // 352 µs after its first preamble byte, so this is reachable whenever the medium
            // delivers one more than 512 µs after `TX_DONE`.
            self.abort_rx_silently();
            self.tx_abort(TX_ABORT_BY_RX_ACK_TIMEOUT);
            self.state = RadioState::Idle;
            if self.dbg { eprintln!("[802.15.4] no ACK for seq {}: RX_ACK_TIMEOUT", self.ack_wait_seq); }
        }
        if let Some(over) = self.rx_done.tick(cycles) { self.finish_rx(Some(over)); }
        if let Some(over) = self.ack_turnaround.tick(cycles).filter(|_| self.state == RadioState::TxAck) {
            // the acknowledgement goes on the air now: the host sees it as a transmission
            self.tx_frame = self.ack_frame.to_vec();
            self.tx_started = true;
            self.ack_tx_count += 1;
            self.ack_done.chain(air_cycles(PHY_OVERHEAD_BYTES + ACK_MAC_BYTES + 2), over);
            if self.dbg { eprintln!("[802.15.4] ACK seq {} on the air, ACK_TX_DONE in {} cycles", self.ack_frame[2], self.ack_done.remaining()); }
        }
        if self.ack_done.tick(cycles).is_some() && self.state == RadioState::TxAck {
            self.events |= EVENT_ACK_TX_DONE;
            self.state = RadioState::Idle;
            if self.dbg { eprintln!("[802.15.4] ACK_TX_DONE"); }
        }
        if self.timer[0].tick(cycles) { self.events |= EVENT_TIMER0_OVERFLOW; }
        if self.timer[1].tick(cycles) { self.events |= EVENT_TIMER1_OVERFLOW; }
        self.live = self.ed_left.is_some() || self.timer.iter().any(|t| t.running)
            || [&self.tx_sfd, &self.tx_done, &self.rx_sfd, &self.rx_done, &self.ack_timeout, &self.ack_turnaround, &self.ack_done].iter().any(|c| c.armed());
    }
    fn has_deadline(&self) -> bool { true }
    /// The scene only matters to a scan in progress, so it is not a deadline on its own: an idle
    /// radio must not wake the machine every 100 ms.
    fn next_deadline(&self) -> Option<u64> {
        let mut best = u64::MAX;
        if let Some(l) = self.ed_left { best = best.min(l).min(CPU_HZ / 10 - self.scene_acc); }
        for c in [&self.tx_sfd, &self.tx_done, &self.rx_sfd, &self.rx_done, &self.ack_timeout, &self.ack_turnaround, &self.ack_done] { best = best.min(c.remaining()); }
        for t in &self.timer { if let Some(d) = t.deadline() { best = best.min(d); } }
        Some(best)
    }
    fn debug(&mut self, on: bool) { self.dbg = on; }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PAN 0xabcd, short 0x0002, the extended address Contiki's port writes for node 2
    /// (on-air order), auto-ACK both ways, every event and every abort reason enabled.
    fn configured() -> Ieee802154 {
        let mut r = Ieee802154::new();
        r.write(0x04, CONF_AUTO_ACK_TX | CONF_AUTO_ACK_RX | 1 << 28);
        r.write(0x08, 0x0002); r.write(0x0c, 0xabcd);
        r.write(0x10, u32::from_le_bytes([0x02, 0x00, 0x00, 0xfe])); r.write(0x14, u32::from_le_bytes([0xff, 0x00, 0x00, 0x02]));
        r.write(0x60, 0x1fff); r.write(0x68, 0x7fff_ffff); r.write(0x78, 0x7fff_ffff);
        r.write(0xe0, 0x4081_0000);
        r
    }
    fn armed_rx() -> Ieee802154 { let mut r = configured(); r.write(0x00, CMD_RX_START); r }

    /// A data frame, PAN compression, short broadcast destination, extended source.
    const BROADCAST: [u8; 19] = [0x41, 0xd8, 0x01, 0xcd, 0xab, 0xff, 0xff, 0x01, 0x01, 0x01, 0x00, 0x01, 0x74, 0x12, 0x00, 0x07, 0, 0, 0];
    /// A data frame with AR, PAN compression, to short 0x0002 on PAN 0xabcd, from an extended address.
    const UNICAST_AR: [u8; 17] = [0x61, 0xd8, 0x2d, 0xcd, 0xab, 0x02, 0x00, 0x01, 0x01, 0x01, 0x00, 0x01, 0x74, 0x12, 0x00, 0xaa, 0xbb];

    #[test]
    fn header_parse_reads_the_addresses() {
        let h = parse_header(&UNICAST_AR).unwrap();
        assert_eq!((h.frame_type, h.ack_request, h.version, h.seq), (1, true, 1, 0x2d));
        assert_eq!((h.dst_pan, h.dst_short, h.dst_ext, h.src_pan), (Some(0xabcd), Some(0x0002), None, Some(0xabcd)));
        let h = parse_header(&BROADCAST).unwrap();
        assert_eq!((h.ack_request, h.dst_short), (false, Some(0xffff)));
        assert!(parse_header(&[0x41, 0xd8, 0x01, 0xcd]).is_none(), "shorter than its header");
    }

    /// The same frame must occupy the air for the same time whether this radio sends it or
    /// receives it, and its SFD must sit at the same offset either way. Charging RX only one
    /// byte of PHY overhead made a receiver's `RX_DONE` beat the sender's `TX_DONE` by the five
    /// preamble-and-SFD bytes, which no medium can deliver.
    #[test]
    fn rx_and_tx_agree_on_the_air_time() {
        let mac = BROADCAST.to_vec();   // a frame the filter admits, so both paths see the same one

        let mut tx = Ieee802154::new();
        tx.write(0x60, 0x1fff); tx.write(0xd0, 0x4081_6150); tx.write(0x00, CMD_TX_START);
        tx.tx_request.take();
        tx.tx_loaded(mac.clone());
        tx.tick(40);
        let (tx_air, tx_sfd) = (tx.tx_done.remaining(), tx.tx_sfd.remaining());

        let mut rx = armed_rx();
        assert!(rx.receive(&mac, -60, 200, Some(0)));
        let (rx_air, rx_sfd) = (rx.rx_done.remaining(), rx.rx_sfd.remaining());

        assert_eq!(rx_air, tx_air, "a received frame is on the air as long as a sent one");
        assert_eq!(rx_sfd, tx_sfd, "the SFD sits at the same offset either way");
        assert_eq!(tx_air, air_cycles(PHY_OVERHEAD_BYTES + BROADCAST.len() as u64 + 2));
        assert_eq!(tx_sfd, air_cycles(SFD_BYTES));
    }

    /// A frame offered while listening: the preamble is on the air first, so `RX_SFD_DONE`
    /// lands `SFD_BYTES` in and `RX_DONE` at the end of the whole PPDU, with the buffer laid
    /// out as the driver reads it (length incl. FCS, frame, RSSI, LQI).
    #[test]
    fn rx_lands_after_its_air_time() {
        let mut r = armed_rx();
        assert!(r.receive(&BROADCAST, -60, 200, Some(0)));
        assert_eq!(r.events & EVENT_RX_SFD_DONE, 0, "the SFD is still behind the preamble");
        assert_eq!(r.read(0x80) >> 16 & 7, 2, "rx_state says a frame is coming in");
        let sfd = air_cycles(SFD_BYTES);
        r.tick(sfd - 1);
        assert_eq!(r.events & EVENT_RX_SFD_DONE, 0);
        r.tick(1);
        assert_eq!(r.events & EVENT_RX_SFD_DONE, EVENT_RX_SFD_DONE, "SFD after the preamble");
        let air = air_cycles(6 + 19 + 2);
        r.tick(air - sfd - 1);
        assert_eq!(r.events & EVENT_RX_DONE, 0);
        r.tick(1);
        assert_eq!(r.events & EVENT_RX_DONE, EVENT_RX_DONE);
        let (addr, buf) = r.rx_write.take().unwrap();
        assert_eq!(addr, 0x4081_0000);
        assert_eq!(buf.len(), 1 + 19 + 2);
        assert_eq!((buf[0], &buf[1..20], buf[20], buf[21]), (21, &BROADCAST[..], (-60i8) as u8, 200));
        assert_eq!(r.read(0xa4), 21);
        assert_eq!(r.state, RadioState::Idle, "no AR: the driver re-arms RX itself");
    }

    /// A frame the medium reports at its end (Cooja-NG's delivery): SFD and RX_DONE together,
    /// the buffer ready at once, nothing left to count down.
    #[test]
    fn rx_reported_at_its_end_completes_at_once() {
        let mut r = armed_rx();
        assert!(r.receive(&BROADCAST, -50, 255, None));
        assert_eq!(r.events & (EVENT_RX_SFD_DONE | EVENT_RX_DONE), EVENT_RX_SFD_DONE | EVENT_RX_DONE);
        assert_eq!(r.rx_write.take().unwrap().1[0], 21);
        assert_eq!(r.state, RadioState::Idle);
        assert_eq!(r.next_deadline(), Some(u64::MAX));
    }

    /// A frame the medium reports late (its start is in the past): RX_DONE lands at the true end
    /// of its air time, or at once if that is past too.
    #[test]
    fn rx_started_in_the_past_completes_at_its_true_end() {
        let air = air_cycles(6 + 19 + 2);
        let mut r = armed_rx();
        assert!(r.receive(&BROADCAST, -60, 200, Some(air - 1000)));
        assert_eq!(r.next_deadline(), Some(1000));
        r.tick(1000);
        assert_eq!(r.events & EVENT_RX_DONE, EVENT_RX_DONE);
        let mut r = armed_rx();
        assert!(r.receive(&BROADCAST, -60, 200, Some(air + 5)));
        assert_eq!(r.events & EVENT_RX_DONE, EVENT_RX_DONE, "already over: complete now");
    }

    /// Not listening: the frame is dropped and counted, nothing is raised.
    #[test]
    fn rx_while_idle_is_dropped() {
        let mut r = configured();
        assert!(!r.receive(&BROADCAST, -60, 200, Some(0)));
        assert_eq!((r.rx_dropped, r.events), (1, 0));
        let mut r = armed_rx();
        assert!(r.receive(&BROADCAST, -60, 200, Some(0)));
        assert!(!r.receive(&BROADCAST, -60, 200, Some(0)), "a second frame collides with the one in flight");
        assert_eq!(r.rx_dropped, 1);
    }

    /// The address filter: ours, broadcast and the coordinator's PAN pass; another PAN, another
    /// short address, another extended address and a stray ACK are refused with FILTER_FAIL
    /// (and no RX_DONE); promiscuous takes everything.
    #[test]
    fn filter_admits_only_what_is_addressed_to_us() {
        let ours_ext: [u8; 25] = { let mut f = [0u8; 25]; f[..3].copy_from_slice(&[0x61, 0xdc, 0x10]); f[3..5].copy_from_slice(&[0xcd, 0xab]); f[5..13].copy_from_slice(&[0x02, 0x00, 0x00, 0xfe, 0xff, 0x00, 0x00, 0x02]); f[13..21].copy_from_slice(&[1; 8]); f };
        let other_ext: [u8; 25] = { let mut f = ours_ext; f[5] = 0x03; f };
        let other_pan: [u8; 17] = { let mut f = UNICAST_AR; f[3] = 0x34; f[4] = 0x12; f };
        let other_short: [u8; 17] = { let mut f = UNICAST_AR; f[5] = 0x03; f };
        let bcast_pan: [u8; 17] = { let mut f = UNICAST_AR; f[3] = 0xff; f[4] = 0xff; f };
        let ack = [0x02u8, 0x00, 0x2d];
        for (name, frame, ok) in [("ours short", &UNICAST_AR[..], true), ("broadcast", &BROADCAST[..], true), ("ours ext", &ours_ext[..], true), ("broadcast PAN", &bcast_pan[..], true),
                                  ("other PAN", &other_pan[..], false), ("other short", &other_short[..], false), ("other ext", &other_ext[..], false), ("stray ack", &ack[..], false)] {
            let mut r = armed_rx();
            assert_eq!(r.receive(frame, -60, 200, None), ok, "{}", name);
            assert_eq!(r.events & EVENT_RX_DONE != 0, ok, "{}", name);
            if !ok { assert_eq!((r.rx_filtered, r.events & EVENT_RX_ABORT, r.read(0x80) >> 4 & 0x1f), (1, EVENT_RX_ABORT, RX_ABORT_BY_FILTER_FAIL), "{}", name); }
        }
        // a data frame without a destination: a coordinator on that PAN takes it, nobody else does
        let no_dst = [0x01u8, 0x80, 0x05, 0xcd, 0xab, 0x01, 0x00];
        let mut r = armed_rx();
        assert!(!r.receive(&no_dst, -60, 200, None));
        let mut r = configured(); { let c = r.read(0x04); r.write(0x04, c | CONF_COORDINATOR); } r.write(0x00, CMD_RX_START);
        assert!(r.receive(&no_dst, -60, 200, None));
        // promiscuous: the other PAN, and the filter's abort is not raised
        let mut r = configured(); { let c = r.read(0x04); r.write(0x04, c | CONF_PROMISCUOUS); } r.write(0x00, CMD_RX_START);
        assert!(r.receive(&other_pan, -60, 200, None));
        assert_eq!(r.rx_filtered, 0);
        // an abort reason the driver did not enable raises no event
        let mut r = armed_rx(); r.write(0x68, 0);
        assert!(!r.receive(&other_pan, -60, 200, None));
        assert_eq!(r.events & EVENT_RX_ABORT, 0);
    }

    #[test]
    fn multipan_matches_pan_and_address_in_the_same_entry() {
        let mut r = Ieee802154::new();
        r.write(0x04, 3 << 28);
        let entries = [(0xaaaa, 1, [0x11; 8]), (0xbbbb, 2, [0x22; 8])];
        for (i, &(pan, short, ext)) in entries.iter().enumerate() {
            let base = 0x08 + 0x10 * i as u32;
            r.write(base, short); r.write(base + 4, pan);
            r.write(base + 8, u32::from_le_bytes(ext[..4].try_into().unwrap()));
            r.write(base + 12, u32::from_le_bytes(ext[4..].try_into().unwrap()));
        }
        for &(pan, short, ext) in &entries {
            for dst_pan in [pan as u16, 0xffff] {
                let h = MacHeader { frame_type: 1, dst_pan: Some(dst_pan), dst_short: Some(short as u16), ..Default::default() };
                assert!(r.accepts(&h));
                assert!(r.accepts(&MacHeader { dst_short: None, dst_ext: Some(ext), ..h }));
            }
            let other_pan = if pan == 0xaaaa { 0xbbbb } else { 0xaaaa };
            let h = MacHeader { frame_type: 1, dst_pan: Some(other_pan), dst_short: Some(short as u16), ..Default::default() };
            assert!(!r.accepts(&h), "PAN and short address must match one entry");
            assert!(!r.accepts(&MacHeader { dst_short: None, dst_ext: Some(ext), ..h }), "PAN and extended address must match one entry");
        }
    }

    /// A received AR frame passing the filter is acknowledged by hardware: RX_DONE, then the ACK
    /// on the air 192 µs after the frame's end (the host sees it start, with the frame's
    /// sequence number and version), ACK_TX_DONE 352 µs after that, and the frame's own RX_DONE
    /// buffer untouched throughout.
    #[test]
    fn received_ar_frame_is_acknowledged() {
        let mut r = armed_rx();
        assert!(r.receive(&UNICAST_AR, -60, 200, None));
        assert_eq!(r.events & EVENT_RX_DONE, EVENT_RX_DONE);
        assert_eq!(r.state, RadioState::TxAck);
        assert_eq!(r.rx_write.take().unwrap().1[3], 0x2d);
        assert_eq!(r.next_deadline(), Some(ns_cycles(ACK_TURNAROUND_NS)));
        r.tick(ns_cycles(ACK_TURNAROUND_NS) - 1);
        assert!(!r.take_tx_started());
        r.tick(1);
        assert!(r.take_tx_started(), "the ACK starts exactly 12 symbols after the frame ended");
        assert_eq!(r.tx_frame, vec![0x02, 0x10, 0x2d]);
        assert_eq!(r.read(0x88) >> 8 & 1, 1, "tx_proc while the ACK is out");
        assert!(!r.receive(&BROADCAST, -60, 200, None), "half duplex: nothing is received while the ACK is out");
        r.tick(air_cycles(6 + 3 + 2) - 1);
        assert_eq!(r.events & EVENT_ACK_TX_DONE, 0);
        r.tick(1);
        assert_eq!(r.events & EVENT_ACK_TX_DONE, EVENT_ACK_TX_DONE);
        assert_eq!((r.state, r.ack_tx_count), (RadioState::Idle, 1));
        // the same frame without auto-ack-tx is only received
        let mut r = configured(); { let c = r.read(0x04); r.write(0x04, c & !CONF_AUTO_ACK_TX); } r.write(0x00, CMD_RX_START);
        assert!(r.receive(&UNICAST_AR, -60, 200, None));
        assert_eq!(r.state, RadioState::Idle);
    }

    /// Our AR frame: after TX_DONE the radio waits for the acknowledgement. The ACK with our
    /// sequence number lands in the RX buffer as the driver reads it (length 5 incl. FCS) and
    /// raises ACK_RX_DONE; other frames, and an ACK with another sequence number, are ignored.
    #[test]
    fn ar_transmit_waits_for_its_ack() {
        let mut r = configured();
        r.write(0xd0, 0x4081_6150); r.write(0x00, CMD_TX_START);
        assert_eq!(r.tx_request.take(), Some(0x4081_6150));
        r.tx_loaded(UNICAST_AR.to_vec());
        assert!(r.take_tx_started());
        r.tick(40);                                   // the round the write sat in
        r.tick(air_cycles(6 + 17 + 2));
        assert_eq!(r.events & EVENT_TX_DONE, EVENT_TX_DONE);
        assert_eq!(r.state, RadioState::RxAck);
        assert_eq!(r.read(0x80) >> 16 & 7, 1, "rx_state: listening, no frame coming in");
        assert!(!r.receive(&BROADCAST, -60, 200, None), "a data frame is not the ACK");
        assert!(!r.receive(&[0x02, 0x00, 0x2e], -60, 200, None), "another sequence number");
        assert_eq!(r.events & EVENT_ACK_RX_DONE, 0);
        assert!(r.receive(&[0x02, 0x00, 0x2d], -70, 180, None));
        assert_eq!(r.events & (EVENT_ACK_RX_DONE | EVENT_RX_SFD_DONE), EVENT_ACK_RX_DONE | EVENT_RX_SFD_DONE);
        assert_eq!(r.events & EVENT_RX_DONE, 0, "an ACK is not a received frame");
        assert_eq!(r.rx_write.take().unwrap(), (0x4081_0000, vec![5, 0x02, 0x00, 0x2d, (-70i8) as u8, 180]));
        assert_eq!((r.state, r.ack_rx_count, r.next_deadline()), (RadioState::Idle, 1, Some(u64::MAX)));
        // without auto-ack-rx, or without the AR bit, TX_DONE is the end of it
        let mut r = configured(); { let c = r.read(0x04); r.write(0x04, c & !CONF_AUTO_ACK_RX); } r.write(0x00, CMD_TX_START);
        r.tx_loaded(UNICAST_AR.to_vec()); r.tick(1); r.tick(air_cycles(6 + 17 + 2));
        assert_eq!(r.state, RadioState::Idle);
        let mut r = configured(); r.write(0x00, CMD_TX_START);
        r.tx_loaded(BROADCAST.to_vec()); r.tick(1); r.tick(air_cycles(6 + 19 + 2));
        assert_eq!(r.state, RadioState::Idle);
    }

    /// No ACK: with ACK_TIMEOUT programmed (units of 16 µs) hardware gives up with TX_ABORT
    /// reason RX_ACK_TIMEOUT; without it the wait lasts until the driver's own timer and STOP.
    #[test]
    fn ar_transmit_times_out() {
        let mut r = configured(); r.write(0x5c, 54);   // 864 µs
        r.write(0x00, CMD_TX_START); r.tx_loaded(UNICAST_AR.to_vec()); r.tick(1); r.tick(air_cycles(6 + 17 + 2));
        assert_eq!(r.state, RadioState::RxAck);
        assert_eq!(r.next_deadline(), Some(ns_cycles(54 * 16_000)));
        r.tick(ns_cycles(54 * 16_000) - 1);
        assert_eq!(r.events & EVENT_TX_ABORT, 0);
        r.tick(1);
        assert_eq!((r.events & EVENT_TX_ABORT, r.read(0x84) >> 4 & 0x1f, r.state), (EVENT_TX_ABORT, TX_ABORT_BY_RX_ACK_TIMEOUT, RadioState::Idle));
        let mut r = configured();
        r.write(0x00, CMD_TX_START); r.tx_loaded(UNICAST_AR.to_vec()); r.tick(1); r.tick(air_cycles(6 + 17 + 2));
        r.tick(ns_cycles(300_000_000));
        assert_eq!((r.state, r.events & EVENT_TX_ABORT), (RadioState::RxAck, 0), "no hardware timeout: the driver's TIMER0 decides");
        r.write(0x00, CMD_STOP);
        assert_eq!(r.state, RadioState::Idle);
        r.write(0x00, CMD_RX_START);
        assert!(r.receive(&BROADCAST, -60, 200, None), "listening again");
    }

    /// An acknowledgement still on the air when the hardware timeout fires is not ours to
    /// deliver. `RX_DONE` for a 3-byte ACK lands 352 µs after its first preamble byte, so one
    /// handed over more than 512 µs after `TX_DONE` completes when the timeout has already
    /// dropped the state to `Idle` — and `finish_rx` would then take the data-frame branch and
    /// report the ACK as a received frame, into the RX buffer, with `RX_DONE` raised.
    #[test]
    fn a_late_ack_is_dropped_rather_than_reported_as_a_frame() {
        let mut r = configured(); r.write(0x5c, 54);          // 864 µs
        r.write(0x00, CMD_TX_START); r.tx_loaded(UNICAST_AR.to_vec()); r.tick(1); r.tick(air_cycles(6 + 17 + 2));
        assert_eq!(r.state, RadioState::RxAck);
        r.tick(ns_cycles(600_000));                            // the medium starts the ACK 600 µs on
        assert!(r.receive(&[0x02, 0x00, 0x2d], -70, 180, Some(0)));
        assert_eq!(r.events & EVENT_ACK_RX_DONE, 0, "352 µs of air still to come");
        r.tick(ns_cycles(264_000));                            // 864 µs after TX_DONE: the timeout
        assert_eq!((r.events & EVENT_TX_ABORT, r.state), (EVENT_TX_ABORT, RadioState::Idle));
        r.tick(ns_cycles(200_000));                            // past where the ACK would have landed
        assert_eq!(r.events & (EVENT_RX_DONE | EVENT_ACK_RX_DONE), 0, "the ACK is gone, not received");
        assert_eq!((r.rx_count, r.ack_rx_count, r.rx_dropped), (0, 0, 1));
        assert!(r.rx_write.is_none(), "nothing reached the RX buffer");
        assert_eq!(r.next_deadline(), Some(u64::MAX), "no countdown left armed");
    }

    /// CCA: a transmit while a frame is being received is refused with CCA_BUSY and the receive
    /// goes on; on a quiet channel it transmits.
    #[test]
    fn cca_transmit_is_busy_while_a_frame_is_coming_in() {
        let mut r = armed_rx();
        assert!(r.receive(&BROADCAST, -60, 200, Some(0)));
        r.write(0xd0, 0x4081_6150); r.write(0x00, CMD_CCA_TX_START);
        assert_eq!((r.events & EVENT_TX_ABORT, r.read(0x84) >> 4 & 0x1f, r.tx_request), (EVENT_TX_ABORT, TX_ABORT_BY_CCA_BUSY, None));
        assert_eq!(r.state, RadioState::RxFrame);
        r.tick(air_cycles(6 + 19 + 2));
        assert_eq!(r.events & EVENT_RX_DONE, EVENT_RX_DONE);
        let mut r = armed_rx();
        r.write(0x00, CMD_CCA_TX_START);
        assert_eq!((r.tx_request, r.state), (Some(0), RadioState::Tx));
    }

    /// TX_START asks the bus for the frame; once loaded, SFD and DONE follow the air. The tick
    /// that closes the round of the write does not count (it precedes the write).
    #[test]
    fn tx_start_requests_the_frame_then_times_the_air() {
        let mut r = Ieee802154::new();
        r.write(0x60, 0x1fff); r.write(0xd0, 0x4081_6150); r.write(0x00, CMD_TX_START);
        assert_eq!(r.tx_request.take(), Some(0x4081_6150));
        assert_eq!(r.state, RadioState::Tx);
        r.tx_loaded(vec![0u8; 17]);
        assert!(r.take_tx_started() && !r.take_tx_started());
        r.tick(40);                                   // the round the write sat in
        assert_eq!(r.tx_done.remaining(), air_cycles(6 + 17 + 2));
        r.tick(air_cycles(SFD_BYTES));
        assert_eq!(r.events & EVENT_TX_SFD_DONE, EVENT_TX_SFD_DONE);
        r.tick(air_cycles(6 + 17 + 2) - air_cycles(SFD_BYTES) - 1);
        assert_eq!(r.events & EVENT_TX_DONE, 0);
        r.tick(1);
        assert_eq!(r.events & EVENT_TX_DONE, EVENT_TX_DONE);
        assert_eq!(r.state, RadioState::Idle);
        assert_eq!(r.read(0x64), EVENT_TX_SFD_DONE | EVENT_TX_DONE);
        r.write(0x64, EVENT_TX_DONE);
        assert_eq!(r.read(0x64), EVENT_TX_SFD_DONE);
    }

    /// STOP mid-frame raises RX_ABORT with the RX_STOP reason when the driver enabled it.
    #[test]
    fn stop_aborts_a_frame_in_flight() {
        let mut r = armed_rx();
        r.receive(&BROADCAST, -60, 200, Some(0));
        r.write(0x00, CMD_STOP);
        assert_eq!(r.events & EVENT_RX_ABORT, EVENT_RX_ABORT);
        assert_eq!(r.read(0x80) >> 4 & 0x1f, RX_ABORT_BY_RX_STOP);
        assert_eq!(r.state, RadioState::Idle);
        r.tick(air_cycles(100));
        assert_eq!(r.events & EVENT_RX_DONE, 0, "no RX_DONE after a stop");
    }

    /// The deadline is the nearest of what is armed, and nothing when the radio is quiet.
    #[test]
    fn deadline_follows_the_armed_countdowns() {
        let mut r = Ieee802154::new();
        assert_eq!(r.next_deadline(), Some(u64::MAX));
        r.write(0xa8, 100); r.write(0x00, CMD_TIMER0_START);
        assert_eq!(r.next_deadline(), Some(100 * 160));
        r.tick(50 * 160);
        assert_eq!(r.read(0xac), 50);
        r.tick(50 * 160);
        assert_eq!(r.events & EVENT_TIMER0_OVERFLOW, EVENT_TIMER0_OVERFLOW);
        assert_eq!(r.next_deadline(), Some(u64::MAX));
    }

    #[test]
    fn channel_from_frequency() {
        let mut r = Ieee802154::new();
        r.write(0x48, 78);
        assert_eq!(r.channel(), 26);
        r.write(0x48, 3);
        assert_eq!(r.channel(), 11);
    }

    /// Tick `cycles` in scheduling rounds of 64 cycles, as the machine does.
    fn rounds(r: &mut Ieee802154, mut cycles: u64) { while cycles > 0 { let c = cycles.min(64); r.tick(c); cycles -= c; } }

    /// An idle radio's tick is the scene alone: with nothing armed the tick clears `live`. A
    /// register write, `receive` and `tx_loaded` each reopen the full tick after such a quiet
    /// round, and the tick keeps it open while an energy scan, a radio timer or a countdown is
    /// armed, so every event still lands on its own cycle across many rounds.
    #[test]
    fn a_quiet_radio_ticks_only_the_scene_and_every_arming_entry_wakes_it() {
        // receive, after a quiet round: SFD, then RX_DONE with rx_done armed alone
        let mut r = armed_rx();
        r.tick(64);
        assert!(!r.live, "listening with nothing in flight: only the scene ticks");
        assert!(r.receive(&BROADCAST, -60, 200, Some(0)));
        let (sfd, air) = (air_cycles(SFD_BYTES), air_cycles(6 + 19 + 2));
        rounds(&mut r, sfd - 1);
        assert_eq!(r.events & EVENT_RX_SFD_DONE, 0);
        rounds(&mut r, 1);
        assert_eq!(r.events & EVENT_RX_SFD_DONE, EVENT_RX_SFD_DONE);
        rounds(&mut r, air - sfd - 1);
        assert_eq!(r.events & EVENT_RX_DONE, 0);
        rounds(&mut r, 1);
        assert_eq!(r.events & EVENT_RX_DONE, EVENT_RX_DONE);
        assert!(!r.live, "nothing armed after RX_DONE");
        // tx_loaded, after a quiet round that closed the TX_START write
        let mut r = configured();
        r.write(0xd0, 0x4081_6150); r.write(0x00, CMD_TX_START);
        r.tick(64);
        assert!(!r.live && r.tx_request.take().is_some(), "a requested frame arms nothing yet");
        r.tx_loaded(BROADCAST.to_vec());
        r.tick(64);                                   // the round the load sat in
        rounds(&mut r, air - 1);
        assert_eq!(r.events & EVENT_TX_DONE, 0);
        rounds(&mut r, 1);
        assert_eq!((r.events & EVENT_TX_DONE, r.state), (EVENT_TX_DONE, RadioState::Idle));
        // a register write: an energy scan of 3 symbols (48 µs) counted down over rounds
        let mut r = configured();
        r.tick(64);
        assert!(!r.live);
        r.write(0x50, 3); r.write(0x00, CMD_ED_START);
        rounds(&mut r, 3 * 16 * 160 - 1);
        assert_eq!(r.events & EVENT_ED_DONE, 0);
        rounds(&mut r, 1);
        assert_eq!((r.events & EVENT_ED_DONE, r.scans, r.state), (EVENT_ED_DONE, 1, RadioState::Idle));
        // radio timer 1 alone
        let mut r = configured();
        r.tick(64);
        r.write(0xb0, 10); r.write(0x00, CMD_TIMER1_START);
        rounds(&mut r, 10 * 160 - 1);
        assert_eq!(r.events & EVENT_TIMER1_OVERFLOW, 0);
        rounds(&mut r, 1);
        assert_eq!(r.events & EVENT_TIMER1_OVERFLOW, EVENT_TIMER1_OVERFLOW);
        r.tick(64);
        assert!(!r.live, "the overflow stopped the timer: quiet again");
    }
}
