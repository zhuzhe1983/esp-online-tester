//! Opt-in C3 link controller, derived from the rev3 ROM and ESP-IDF v5.5.5 controller.
//! Fields remain inferred unless explicitly marked checked on C3 rev v0.3.
//! See docs/evidence/ble-c3-advertising/README.md for derivation and limits.
use std::collections::VecDeque;
use emu_core::ClockDomain;
use esp_periph::{Device, RegRam, WriteEffect};
use esp_soc::ble::peer::{hex, Command, ReadStep, UuidRead};

const REQUEST: u32 = 1 << 31;
const TIMER: u32 = 1 << 11;
const HALF_US_CYCLES: u64 = crate::periph::CPU_HZ / 2_000_000;
const PERIOD: u64 = (1 << 28) * 625;

#[derive(Default)]
pub struct BleLc {
    ram: RegRam,
    pub(crate) accessible: bool,
    state: Option<Box<State>>,
}

#[derive(Default)]
struct State {
    cycles: u64,
    reset: Option<u64>,
    latch: Option<u64>,
    alarm: Option<u64>,
    raw: u32,
    fifo: VecDeque<u32>,
    kicks: VecDeque<u8>,
    event: Option<Event>,
    scan_response: Vec<u8>,
    dropped: u64,
    pub_log: bool,
    host: Host,
    connection: Option<Connection>,
    packets: VecDeque<String>,
}

#[derive(Default)]
struct Host {
    scanning: bool,
    connecting: bool,
    read_request: Option<UuidRead>,
}

impl BleLc {
    pub(crate) fn enable(&mut self) {
        self.state = Some(Box::default());
        self.accessible = true;
        // Inferred reset configuration; initialized readbacks checked on C3 rev v0.3.
        // +48 is preserved by guest RMW; +7c is never written by the probe guest.
        self.ram.write(0x048, 0x0003_fff7);
        self.ram.write(0x07c, 0xe400_e400);
    }
    pub fn observe(&mut self, log: bool) { if let Some(s) = &mut self.state { s.pub_log = log; } }
    pub fn scan(&mut self, enabled: bool) { if let Some(s) = &mut self.state { s.host.scanning = enabled; } }
    pub fn command(&mut self, command: &str) -> Result<(), String> {
        let command: Command = command.parse()?;
        let s = self.state.as_mut().ok_or("full BLE is disabled")?;
        match command {
            Command::Connect => {
                if s.connection.is_some() { return Err("central is already connected".into()) }
                s.host.connecting = true;
            }
            Command::Disconnect => {
                let c = s.connection.as_mut().ok_or("central is not connected")?;
                if c.terminating || c.stopped { return Err("central is already stopping".into()) }
                // LL_TERMINATE_IND, remote-user reason 0x13. The guest owns cleanup.
                c.outgoing.push_back(vec![3,2,2,0x13]);
                c.terminating = true;
            }
            Command::CentralStop => s.connection.as_mut().ok_or("central is not connected")?.silent = true,
            Command::ReadUuid(service, characteristic) => {
                if s.host.read_request.is_some() || s.connection.as_ref().is_some_and(|c| c.gatt.is_some()) {
                    return Err("a BLE read is already pending".into());
                }
                s.host.read_request = Some(UuidRead::new(service, characteristic));
            }
            _ => return Err("full BLE supports connect, disconnect, central-stop and read-uuid SERVICE CHARACTERISTIC".into()),
        }
        Ok(())
    }
    pub fn pending_commands(&self) -> usize {
        self.state.as_ref().map_or(0, |s| usize::from(s.host.connecting) + usize::from(s.host.read_request.is_some())
            + usize::from(s.connection.as_ref().is_some_and(|c| c.gatt.is_some())))
    }
    pub fn scanning(&self) -> bool { self.state.as_ref().is_some_and(|s| s.host.scanning) }
    pub fn observing(&self) -> bool { self.state.as_ref().is_some_and(|s| s.pub_log) }
    pub fn take_observation(&mut self) -> Option<String> {
        let s = self.state.as_mut()?;
        if s.dropped != 0 { return Some(format!("[ble-observer] dropped={}", std::mem::take(&mut s.dropped))) }
        s.packets.pop_front()
    }
    pub(crate) fn reset_controller(&mut self) {
        let Some(old) = self.state.take() else { return };
        self.ram = RegRam::new();
        self.enable();
        let s = self.state.as_mut().unwrap();
        s.cycles = old.cycles;
        s.pub_log = old.pub_log;
        s.host = old.host;
        s.packets = old.packets;
        s.dropped = old.dropped;
    }
    pub(crate) fn keep_host(&mut self, old: &mut Self) {
        if let (Some(s), Some(old)) = (&mut self.state, &mut old.state) {
            s.host = std::mem::take(&mut old.host);
            s.packets = std::mem::take(&mut old.packets);
            s.dropped = old.dropped;
        }
    }
    pub fn enabled(&self) -> bool { self.state.is_some() }
}

fn half(bytes: &[u8], offset: usize) -> u16 { u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) }

struct Event {
    entry: usize,
    due: u64,
    advertising: Option<Advertising>,
    phase: ScanPhase,
    connection_phase: u8,
}

#[derive(Default)]
enum ScanPhase {
    #[default]
    Advertising,
    Request(u8),
    Receive(u8, Vec<u8>),
    Response(u8),
}

struct Advertising {
    activity: u16,
    channels: u8,
    pdu: Vec<u8>,
    scan_response: Vec<u8>,
}

impl State {
    fn raise(&mut self, source: u32) {
        self.raw |= source;
        // r_sch_prog_end_isr completes one ET per FIFO END, never coalesce ENDs.
        self.fifo.push_back(source);
    }
    fn observe(&mut self, line: impl FnOnce() -> String) {
        if !self.pub_log { return }
        let line = line();
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("{line}");
        if self.packets.len() == 1024 { self.packets.pop_front(); self.dropped += 1; }
        self.packets.push_back(line);
    }
}

impl BleLc {
    // C3 rev3 ROM em_base_reg_lut (0x3ff1f518), condensed to region starts.
    // r_emi_get_mem_addr_by_offset chooses a register by this static table and
    // asserts its programmed logical start. A zero register cannot inherit a neighbor.
    const EM_STARTS: [u32; 39] = [
        0, 0x400, 0xc00, 0x1000, 0x1400, 0x1c00, 0x2000, 0x2400,
        0x2c00, 0x3400, 0x3c00, 0x4400, 0x4c00, 0x5400, 0x5c00, 0x6400,
        0x6c00, 0x7400, 0x7800, 0x7c00, 0x8000, 0x8400, 0x8800, 0x8c00,
        0x9000, 0x9400, 0x9800, 0x9c00, 0xa000, 0xa400, 0xa800, 0xac00,
        0xb000, 0xb400, 0xb800, 0xbc00, 0xc000, 0xc400, 0xc800,
    ];
    fn mapped(&self, offset: u32, len: usize, sram: &[u8]) -> Option<usize> {
        let i = Self::EM_STARTS.partition_point(|&start| start <= offset).checked_sub(1)?;
        let start = Self::EM_STARTS[i];
        let end = Self::EM_STARTS.get(i + 1).copied().unwrap_or(0xcc00);
        if offset.checked_add(len.try_into().ok()?)? > end { return None }
        let entry = self.ram.read(0x204 + i as u32 * 4);
        if entry >> 18 != start / 4 || entry & 0x3ffff == 0 { return None }
        let addr = (0x3fc00000 | ((entry << 2) & 0xffffc)).checked_add(offset - start)?;
        let index = addr.checked_sub(crate::bus::DRAM_LOW)? as usize + crate::bus::DRAM_IN_SRAM;
        sram.get(index..index.checked_add(len)?)?;
        Some(index)
    }

    fn advertising(&self, et: usize, sram: &[u8]) -> Result<Advertising, &'static str> {
        // Inferred CS halfword pointer: r_sch_prog_ble_push_hack, ET+8.
        let cs = self.mapped(half(sram, et + 8) as u32 * 2, 90, sram).ok_or("unmapped control structure")?;
        // Inferred CS fields: r_lld_adv_start 0x400186b8 (TX pointer),
        // 0x4001887e..ca (AdvA), 0x40018942 (channel map bits 7:5).
        // Legacy advertising CS format 4: r_lld_adv_start_set_cs 0x400181e4..e8.
        if half(sram, cs) & 0x1f != 4 { return Err("unsupported control structure format") }
        let channels = ((half(sram, cs + 38) >> 5) & 7) as u8;
        let first = half(sram, cs + 28) as u32;
        let mut next = first;
        let mut advertising = None;
        let mut scan_response = Vec::new();
        // Inferred nine 14-byte TX descriptors per activity: r_lld_adv_start 0x40018684..b8.
        for _ in 0..9 {
            let tx = self.mapped(next, 14, sram).ok_or("unmapped TX descriptor")?;
            // Inferred TX link/header/data: r_lld_adv_start_init_evt_param 0x40017b9e / 0x40017c50 (15-bit link),
            // r_lld_adv_adv_data_set 0x40015858/64 (length includes six-byte AdvA).
            let header = half(sram, tx + 2);
            let len = (header >> 8) as usize;
            if !(6..=37).contains(&len) { return Err("invalid legacy PDU length") }
            let data = self.mapped(half(sram, tx + 4) as u32, len - 6, sram).ok_or("unmapped AD buffer")?;
            let mut pdu = header.to_le_bytes().to_vec();
            pdu.extend_from_slice(&sram[cs + 6..cs + 12]);
            pdu.extend_from_slice(&sram[data..data + len - 6]);
            match header & 15 {
                0 | 2 | 6 if advertising.is_none() => advertising = Some(pdu),
                4 if scan_response.is_empty() => scan_response = pdu,
                _ => return Err("unsupported advertising descriptor chain"),
            }
            next = (half(sram, tx) & 0x7fff) as u32;
            if next == 0 || next == first {
                return Ok(Advertising { activity: half(sram, cs + 2) & 31, channels, pdu: advertising.ok_or("missing advertising PDU")?, scan_response });
            }
        }
        Err("unterminated TX descriptor chain")
    }

    pub(crate) fn service(&mut self, sram: &mut [u8]) {
        let Some(s) = &mut self.state else { return };
        let now = s.cycles;
        if s.event.is_none() {
            if let Some(index) = s.kicks.pop_front() {
                let Some(entry) = self.mapped(index as u32 * 16, 16, sram) else {
                    self.state.as_mut().unwrap().observe(|| "[ble-error] unmapped event entry".into());
                    return;
                };
                // Inferred coarse/fine timestamp: r_sch_prog_push 0x40030f1a / 0x40030f3c / 0x40030f9e.
                let coarse = half(sram, entry + 2) as u64 | ((half(sram, entry + 4) as u64 & 0xfff) << 16);
                let target = coarse * 625 + 624u64.saturating_sub(half(sram, entry + 6) as u64);
                let delta = (target + PERIOD - now / HALF_US_CYCLES % PERIOD) % PERIOD;
                // Inferred: a past target fires now, rather than after wrap; hardware question.
                let due = if delta >= PERIOD / 2 { now } else { (now / HALF_US_CYCLES + delta) * HALF_US_CYCLES };
                self.state.as_mut().unwrap().event = Some(Event { entry, due, advertising: None, phase: ScanPhase::Advertising, connection_phase: 0 });
            }
        }
        let Some(mut event) = self.state.as_mut().unwrap().event.take() else { return };
        if event.due > now {
            self.state.as_mut().unwrap().event = Some(event);
            return;
        }
        let cs = self.mapped(half(sram, event.entry + 8) as u32 * 2, 90, sram);
        if let Some(cs) = cs {
            // Connection format 3 checked on C3 rev v0.3; event timing remains inferred.
            if half(sram, cs) & 31 == 3 {
                match self.connection_event(&mut event, cs, sram) {
                    Ok(true) => self.state.as_mut().unwrap().event = Some(event),
                    Ok(false) => {},
                    Err(reason) => {
                        self.state.as_mut().unwrap().observe(|| format!("[ble-error] {reason}"));
                        self.complete(event.entry, 4, sram);
                    }
                }
                return;
            }
        }
        if event.advertising.is_none() {
            if self.state.as_mut().unwrap().connection.take().is_some() {
                self.state.as_mut().unwrap().observe(|| format!("[ble-state] disconnected advertising_resumed hus={}", event.due / HALF_US_CYCLES));
            }
            match self.advertising(event.entry, sram) {
                Ok(advertising) => {
                    let s = self.state.as_mut().unwrap();
                    if advertising.scan_response != s.scan_response {
                        s.scan_response.clone_from(&advertising.scan_response);
                        if !s.scan_response.is_empty() {
                            let data = &advertising.scan_response[8..];
                            s.observe(|| if data.is_empty() { "[ble-config] SCAN_RSP empty".into() } else {
                                format!("[ble-config] SCAN_RSP {} data={}", ad_fields(data), hex(data))
                            });
                        }
                    }
                    // Inferred START bit4; unmasked advertising snapshot checked on C3 rev v0.3.
                    if self.ram.read(0x0c) & (1 << 4) != 0 { s.raise(1 << 4); } else { s.raw |= 1 << 4; }
                    event.advertising = Some(advertising);
                }
                Err(reason) => {
                    self.state.as_mut().unwrap().observe(|| format!("[ble-error] {reason}"));
                    self.complete(event.entry, 4, sram);
                    return;
                }
            }
        }
        let advertising = event.advertising.as_mut().unwrap();
        match event.phase {
            ScanPhase::Request(channel) => {
                let pdu = if self.state.as_ref().unwrap().host.connecting && advertising.pdu[0] & 15 == 0 { connect_request(&advertising.pdu) } else { scan_request(&advertising.pdu) };
                self.state.as_mut().unwrap().observe(|| format!("[ble-central] hus={} channel={channel} type={} pdu={}",
                    event.due / HALF_US_CYCLES, if pdu[0] & 15 == 5 { "CONNECT_IND" } else { "SCAN_REQ" }, hex(&pdu)));
                event.due += airtime(pdu.len());
                event.phase = ScanPhase::Receive(channel, pdu);
            }
            ScanPhase::Receive(channel, pdu) => {
                if let Err(reason) = self.receive(advertising.activity, channel, event.due - airtime(pdu.len()), &pdu, sram) {
                    self.state.as_mut().unwrap().observe(|| format!("[ble-error] {reason}"));
                    self.complete(event.entry, 4, sram);
                    return;
                }
                if pdu[0] & 15 == 5 {
                    let anchor = event.due + 17500 * HALF_US_CYCLES;
                    let s = self.state.as_mut().unwrap();
                    s.connection = Some(Connection { anchor, ..Connection::default() });
                    s.host.connecting = false;
                    self.complete(event.entry, 3, sram);
                    return;
                }
                event.due += 300 * HALF_US_CYCLES;
                event.phase = ScanPhase::Response(channel);
            }
            ScanPhase::Response(channel) => {
                self.emit(event.due, channel, &advertising.scan_response);
                event.due += airtime(advertising.scan_response.len()) + 300 * HALF_US_CYCLES;
                event.phase = ScanPhase::Advertising;
            }
            ScanPhase::Advertising if advertising.channels != 0 => {
                let channel = advertising.channels.trailing_zeros() as u8;
                advertising.channels &= !(1 << channel);
                self.emit(event.due, 37 + channel, &advertising.pdu);
                if (self.state.as_ref().unwrap().host.connecting && advertising.pdu[0] & 15 == 0)
                    || (self.scanning() && matches!(advertising.pdu[0] & 15, 0 | 6) && !advertising.scan_response.is_empty()) {
                    // Model choice: a virtual scanner responds exactly T_IFS after the PDU ends.
                    event.due += airtime(advertising.pdu.len()) + 300 * HALF_US_CYCLES;
                    event.phase = ScanPhase::Request(37 + channel);
                } else {
                    event.due += airtime(advertising.pdu.len()) + 600 * HALF_US_CYCLES;
                }
            }
            ScanPhase::Advertising => {
                self.complete(event.entry, 3, sram);
                return;
            }
        }
        self.state.as_mut().unwrap().event = Some(event);
    }

    fn emit(&mut self, cycles: u64, channel: u8, pdu: &[u8]) {
        self.state.as_mut().unwrap().observe(|| {
            let address = pdu[2..8].iter().rev().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":");
            let kind = match pdu[0] & 15 { 0 => "ADV_IND", 2 => "ADV_NONCONN_IND", 4 => "SCAN_RSP", _ => "ADV_SCAN_IND" };
            format!("[ble-air] hus={} channel={channel} type={kind} AdvA={address} {} pdu={}",
                cycles / HALF_US_CYCLES, ad_fields(&pdu[8..]), hex(pdu))
        });
    }

    fn receive(&mut self, activity: u16, channel: u8, cycles: u64, pdu: &[u8], sram: &mut [u8]) -> Result<(), &'static str> {
        if pdu.len() < 2 || pdu.len() != pdu[1] as usize + 2 || channel > 39 {
            return Err("invalid received PDU");
        }
        // RX ring stride 20 and next links checked on C3 rev v0.3 (ProbeB).
        // Head/ownership timing inferred: r_lld_core_init 0x4203cd80 / 0x4203ccb0.
        let logical = self.ram.read(0x24) & 0x7fff;
        let rx = self.mapped(logical, 20, sram).ok_or("unmapped RX descriptor")?;
        let link = half(sram, rx);
        if link & 0x8000 != 0 { return Err("RX descriptor still owned by guest") }
        // Inferred payload pointer: r_lld_adv_pkt_rx_send_scan_req_evt 0x4001644c..5e.
        let buffer = half(sram, rx + 18);
        if buffer == 0 { return Err("RX descriptor has no buffer") }
        let data = self.mapped(buffer as u32, pdu.len() - 2, sram).ok_or("unmapped RX buffer")?;
        sram[data..data + pdu.len() - 2].copy_from_slice(&pdu[2..]);
        // Inferred success/status/header/activity: r_lld_adv_pkt_rx 0x400165cc..d8,
        // r_lld_rxdesc_check 0x400203aa..ca. Only error-free host packets are supported.
        // Inferred RSSI/channel: r_lld_con_rx_channel_assess 0x40019dbc..dc8.
        // r_rf_rssi_convert 0x4002e026 sign-extends the low byte. Fixed -40 dBm is
        // a virtual-radio choice, not a signal-strength measurement.
        // Inferred timestamp: r_lld_con_rx_sync_time_update 0x4001a08e..ce / 0x4001a138..158.
        // Inferred 1M sync offset: r_lld_core_init 0x4203cdd6..ee writes
        // lld_exp_sync_pos_tab[0] = 40 + LC+0x90[14:8] microseconds.
        // r_lld_adv_pkt_rx_connect_post 0x40015e24..ca subtracts it and normalizes.
        let hus = cycles / HALF_US_CYCLES + 2 * (40 + ((self.ram.read(0x90) >> 8) & 127) as u64);
        let coarse = hus / 625;
        for (off, value) in [(2, 0), (4, half(pdu, 0)), (6, (channel as u16) << 8 | (-40i8 as u8 as u16)),
            (8, coarse as u16), (10, (coarse >> 16) as u16 & 0xfff),
            (12, activity << 11 | (624 - hus % 625) as u16), (14, 0)] {
            put_half(sram, rx + off, value);
        }
        // RX+14 is inferred resolving-list pointer, zero for this public peer:
        // r_lld_adv_pkt_rx_send_scan_req_evt 0x40016470.
        // Publish ownership last; r_lld_rxdesc_free recycles buffers and clears this bit.
        put_half(sram, rx, link | 0x8000);
        self.ram.write(0x24, (link & 0x7fff) as u32);
        Ok(())
    }

    // The virtual central selects one fixed, valid CONNECT_IND parameter set.
    // Guest event entries determine receive windows; central anchors never follow
    // a late guest window. Missing windows must not silently shift the radio clock.
    fn connection_event(&mut self, event: &mut Event, cs: usize, sram: &mut [u8]) -> Result<bool, &'static str> {
        let mut c = self.state.as_mut().unwrap().connection.take().ok_or("connection event without central")?;
        let activity = half(sram, cs + 2) & 31;
        match event.connection_phase {
            0 => {
                if let Some(read) = self.state.as_mut().unwrap().host.read_request.take() {
                    if c.features {
                        c.att(&read.request());
                    } else {
                        // Virtual central: Bluetooth 5.0, no optional LL features.
                        c.version();
                        c.outgoing.push_back(vec![3,9,8,0,0,0,0,0,0,0,0]);
                    }
                    c.gatt = Some(read);
                }
                // Inferred programmed unmapped channel: r_lld_con_evt_start_cbk 0x4001ae4e..aeaa.
                let control = half(sram, cs + 22);
                if control & (1 << 14) != 0 { return Err("CSA#2 not supported") }
                // Channel map +34..38 and duplicated hop +22[12:8]/+39 checked on C3 rev v0.3.
                // CSA bit14 agrees with the hardware central selecting CSA#2.
                // Channel advancement remains inferred: r_lld_con_start
                // 0x4001baf2..bb8c / 0x4001ba30..36. The guest programs the
                // previous unmapped channel; hardware adds one hop for this event.
                let map = sram[cs + 34..cs + 39].iter().enumerate().fold(0u64, |v, (i, b)| v | ((*b as u64) << (8 * i)));
                c.channel = csa1((control & 63) as u8, ((control >> 8) & 31) as u8, map)?;
                if c.channel != csa1(((c.events * 5) % 37) as u8, 5, (1 << 37) - 1)? {
                    return Err("central/guest channel mismatch");
                }
                let s = self.state.as_mut().unwrap();
                if c.events == 0 { s.observe(|| "[ble-state] connected interval_us=30000 csa=1".into()); }
                s.observe(|| format!("[ble-connection] event={} window_hus={} anchor_hus={} channel={}",
                    c.events, event.due / HALF_US_CYCLES, c.anchor / HALF_US_CYCLES, c.channel));
                put_half(sram, event.entry, (half(sram, event.entry) & !0x38) | (2 << 3));
                // Inferred CS+26: r_lld_con_evt_start_cbk 0x4001aec0..af14 /
                // 0x4001b172..18c uses 2-us units, or 625-us units with bit 15.
                let window = half(sram, cs + 26);
                let width = (window & 0x7fff) as u64 * if window & 0x8000 == 0 { 4 } else { 1250 };
                if c.anchor < event.due || c.anchor > event.due + width * HALF_US_CYCLES {
                    return Err("central anchor outside receive window");
                }
                event.due = c.anchor;
                event.connection_phase = 1;
            }
            1 => {
                if (c.terminating && c.outgoing.is_empty()) || c.silent {
                    if !c.stopped {
                        self.state.as_mut().unwrap().observe(|| format!("[ble-central] stopped hus={}", c.anchor / HALF_US_CYCLES));
                        c.stopped = true;
                    }
                    self.complete(event.entry, 3, sram);
                    c.anchor += 60_000 * HALF_US_CYCLES;
                    c.events += 1;
                    self.state.as_mut().unwrap().connection = Some(c);
                    return Ok(false);
                }
                let mut pdu = c.outgoing.front().cloned().unwrap_or_else(|| vec![1,0]);
                pdu[0] = c.central.header(pdu[0]);
                self.state.as_mut().unwrap().observe(|| format!("[ble-central] hus={} channel={} type=DATA pdu={}", event.due / HALF_US_CYCLES, c.channel, hex(&pdu)));
                c.central.sent = true;
                event.due += airtime(pdu.len());
                c.incoming = pdu;
                event.connection_phase = 2;
            }
            2 => {
                let pdu = std::mem::take(&mut c.incoming);
                // Inferred TX ownership/link/header: r_lld_con_tx 0x4001a410..16,
                // 0x4001a570..58e; r_lld_con_tx_prog 0x4001ac2e..66.
                if c.peripheral.acknowledge(pdu[0]) {
                    if let Some(packet) = c.pending.take() {
                        if let Some(tx) = packet.descriptor {
                            let next = half(sram, tx) & 0x7fff;
                            put_half(sram, tx, next | 0x8000);
                            put_half(sram, cs + 28, next);
                            // Inferred TX IRQ bit 1: r_rwble_isr_hack 0x40386ad2..ec.
                            // Live ip+0x6d4 -> 0x4000154c -> r_sch_prog_tx_isr.
                            // Bit 6 routes ip+0x6d0 to r_sch_prog_skip_isr instead.
                            self.state.as_mut().unwrap().raise(1 << 1);
                        }
                    }
                }
                if c.peripheral.accept(pdu[0]) {
                    self.receive(activity, c.channel, event.due - airtime(pdu.len()), &pdu, sram)?;
                    // r_rwble_isr_hack 0x40386aee..b08 -> r_sch_prog_rx_isr,
                    // then r_lld_con_rx_isr. ET must already be active (state 2).
                    self.state.as_mut().unwrap().raise(1 << 2);
                }
                event.due += 300 * HALF_US_CYCLES;
                event.connection_phase = 3;
            }
            3 => {
                if c.pending.is_none() {
                    let logical = half(sram, cs + 28) as u32;
                    // An ACK can leave CS+28 at link 0, the null link. Logical 0 is the
                    // event table, never a TX descriptor: send an empty PDU, as for a
                    // descriptor the guest still owns. Inferred; null-link handling was
                    // not traced in the ROM.
                    let tx = if logical == 0 { None } else {
                        Some(self.mapped(logical, 14, sram).ok_or("unmapped connection TX")?)
                    };
                    let packet = if let Some(tx) = tx.filter(|&tx| half(sram, tx) & 0x8000 == 0) {
                        let header = half(sram, tx + 2);
                        let len = (header >> 8) as usize;
                        let data = self.mapped(half(sram, tx + 4) as u32, len, sram).ok_or("unmapped connection payload")?;
                        let mut pdu = header.to_le_bytes().to_vec();
                        pdu.extend_from_slice(&sram[data..data + len]);
                        TxPacket { pdu, descriptor: Some(tx) }
                    } else {
                        TxPacket { pdu: vec![1, 0], descriptor: None }
                    };
                    c.pending = Some(packet);
                }
                // r_lld_con_tx 0x4001a410..16 only consumes descriptors with bit 15
                // set. Keep this descriptor and its payload until the peer ACKs it.
                let packet = c.pending.take().unwrap();
                let pdu = &packet.pdu;
                let header = c.peripheral.header(pdu[0]);
                c.peripheral.sent = true;
                self.state.as_mut().unwrap().observe(|| format!("[ble-air] hus={} channel={} type=DATA pdu={header:02x}{}", event.due / HALF_US_CYCLES, c.channel, hex(&packet.pdu[1..])));
                if c.central.acknowledge(header) { c.outgoing.pop_front(); }
                if c.central.accept(header) {
                    match header & 3 {
                        3 if pdu.len() > 2 => match pdu[2] {
                            0x0e | 8 => c.outgoing.push_back(vec![3,9,9,0,0,0,0,0,0,0,0]),
                            9 if !c.features => {
                                c.features = true;
                                if let Some(read) = &c.gatt { let request = read.request(); c.att(&request); }
                            }
                            0x0c => c.version(),
                            9 => {},
                            _ => return Err("unsupported LL control procedure"),
                        },
                        2 => {
                            // The default ATT MTU of 23 fits in one 27-byte LL PDU.
                            // Add L2CAP reassembly before supporting larger negotiated MTUs.
                            if pdu.len() < 6 || half(pdu, 2) as usize + 6 != pdu.len() || half(pdu, 4) != 4 {
                                return Err("unsupported L2CAP packet");
                            }
                            let read = c.gatt.as_mut().ok_or("unsolicited ATT response")?;
                            match read.receive(&pdu[6..]) {
                                Ok(ReadStep::Request(request)) => c.att(&request),
                                Ok(ReadStep::Value(value)) => {
                                    self.state.as_mut().unwrap().observe(|| format!("[ble-att] value={} text={:?}", hex(&value), String::from_utf8_lossy(&value)));
                                    c.gatt = None;
                                }
                                Err(error) => {
                                    self.state.as_mut().unwrap().observe(|| format!("[ble-att] {error}"));
                                    c.gatt = None;
                                }
                            }
                        }
                        1 if pdu.len() == 2 => {},
                        _ => return Err("unsupported LL data fragment"),
                    }
                }
                event.due += airtime(pdu.len());
                if header & 16 != 0 && event.due + 300 * HALF_US_CYCLES + airtime(2) < c.anchor + 60_000 * HALF_US_CYCLES {
                    event.due += 300 * HALF_US_CYCLES;
                    event.connection_phase = 1;
                } else {
                    event.connection_phase = 4;
                }
                c.pending = Some(packet);
            }
            _ => {
                self.complete(event.entry, 3, sram);
                c.anchor += 60_000 * HALF_US_CYCLES;
                c.events += 1;
                self.state.as_mut().unwrap().connection = Some(c);
                return Ok(false);
            }
        }
        self.state.as_mut().unwrap().connection = Some(c);
        Ok(true)
    }

    fn complete(&mut self, entry: usize, status: u16, sram: &mut [u8]) {
        // Inferred successful ET state 3 / aborted state 4:
        // r_sch_prog_end_isr_handler supplies callback a2=(state==4).
        let value = (half(sram, entry) & !0x38) | (status << 3);
        sram[entry..entry + 2].copy_from_slice(&value.to_le_bytes());
        // Inferred END interrupt bit 5: r_rwble_isr_hack;
        // r_ip_funcs_p+0x6c0 resolves to r_sch_prog_end_isr_hack.
        self.state.as_mut().unwrap().raise(1 << 5);
    }
}

#[derive(Default)]
struct Connection {
    anchor: u64,
    events: u32,
    channel: u8,
    central: Sequence,
    peripheral: Sequence,
    pending: Option<TxPacket>,
    stopped: bool,
    silent: bool,
    incoming: Vec<u8>,
    terminating: bool,
    outgoing: VecDeque<Vec<u8>>,
    features: bool,
    version_sent: bool,
    gatt: Option<UuidRead>,
}
impl Connection {
    fn version(&mut self) {
        // Bluetooth Core v5.0 Vol 6 Part B 5.1.5: send LL_VERSION_IND once,
        // including when the peripheral initiates it before any host ATT command.
        if !self.version_sent {
            self.outgoing.push_back(vec![3,6,0x0c,9,0xff,0xff,1,0]);
            self.version_sent = true;
        }
    }
    fn att(&mut self, pdu: &[u8]) {
        let mut packet = vec![2, (pdu.len() + 4) as u8, pdu.len() as u8, 0, 4, 0];
        packet.extend_from_slice(pdu);
        self.outgoing.push_back(packet);
    }
}
struct TxPacket {
    pdu: Vec<u8>,
    descriptor: Option<usize>,
}

#[derive(Default)]
struct Sequence { sn: u8, nesn: u8, sent: bool }
impl Sequence {
    fn header(&self, base: u8) -> u8 { (base & !12) | (self.sn << 3) | (self.nesn << 2) }
    fn acknowledge(&mut self, header: u8) -> bool {
        if self.sent && (header >> 2) & 1 != self.sn {
            self.sn ^= 1;
            self.sent = false;
            true
        } else { false }
    }
    fn accept(&mut self, header: u8) -> bool {
        if (header >> 3) & 1 == self.nesn {
            self.nesn ^= 1;
            true
        } else { false }
    }
}

// Bluetooth CSA#1: add hop, then remap through enabled channels in ascending order.
fn csa1(previous: u8, hop: u8, map: u64) -> Result<u8, &'static str> {
    if previous >= 37 || !(5..=16).contains(&hop) || map >> 37 != 0 || map.count_ones() < 2 {
        return Err("invalid CSA#1 parameters");
    }
    let unmapped = (previous + hop) % 37;
    if map & (1 << unmapped) != 0 { return Ok(unmapped) }
    let index = unmapped as u32 % map.count_ones();
    Ok((0..37).filter(|channel| map & (1 << channel) != 0).nth(index as usize).unwrap())
}

fn put_half(sram: &mut [u8], off: usize, value: u16) { sram[off..off + 2].copy_from_slice(&value.to_le_bytes()); }

fn connect_request(advertising: &[u8]) -> Vec<u8> {
    let mut pdu = vec![5 | ((advertising[0] & 0x40) << 1), 34, 1, 0, 0, 0, 0, 2];
    pdu.extend_from_slice(&advertising[2..8]);
    pdu.extend_from_slice(&[0x70, 0x83, 0x32, 0x9a, 0x56, 0x34, 0x12, 1, 6, 0,
        24, 0, 0, 0, 200, 0, 0xff, 0xff, 0xff, 0xff, 0x1f, 5]);
    pdu
}

fn airtime(len: usize) -> u64 { 8 * (len as u64 + 8) * 2 * HALF_US_CYCLES }

fn scan_request(advertising: &[u8]) -> Vec<u8> {
    // Public simulated scanner address 02:00:00:00:00:01; RxAdd follows advertiser TxAdd.
    let mut pdu = vec![3 | ((advertising[0] & 0x40) << 1), 12, 1, 0, 0, 0, 0, 2];
    pdu.extend_from_slice(&advertising[2..8]);
    pdu
}

fn ad_fields(data: &[u8]) -> String {
    let mut fields = esp_soc::ble::peer::advertising_fields(data);
    for (kind, value) in esp_soc::ble::peer::ad_structures(data) {
        match (kind, value) {
            (1, [flags]) => fields.push(format!("flags=0x{flags:02x}")),
            (0x12, [lo, hi, lo2, hi2]) => fields.push(format!("connection_interval_units={}:{}",
                u16::from_le_bytes([*lo, *hi]), u16::from_le_bytes([*lo2, *hi2]))),
            (2 | 3 | 6 | 7 | 8 | 9, _) => {},
            _ => fields.push(format!("ad_{kind:02x}={}", hex(value))),
        }
    }
    fields.join(" ")
}

impl Device for BleLc {
    fn read(&mut self, off: u32) -> u32 {
        let Some(s) = &self.state else { return self.ram.read(off) };
        if !self.accessible { return 0 }
        match off {
            // Inferred identity required by app r_lld_core_init.
            // Identity and feature word checked on C3 rev v0.3; subfields inferred.
            0x004 => 0x0900_1b00,
            0x008 => 0x0f22_d0b0,
            // Inferred unmasked status; snapshot bits checked on C3 rev v0.3.
            0x014 => s.raw,
            // RF status checked on C3 rev v0.3 after the guest writes zero.
            // Bit meaning and other RF states remain inferred.
            0x070 => self.ram.read(off) | 2,
            // ROM r_cali_phase_match_p searches phase fields [10:8]/[6:4]
            // and tests bit12. Successful 2/2 readback checked on C3 rev v0.3;
            // the phase acceptance rule and instantaneous completion are inferred.
            0x0f8 => {
                let v = self.ram.read(off) & !(1 << 12);
                v | (u32::from(v & 0x771 == 0x221) << 12)
            },
            // Inferred masked status, read by ROM r_rwble_isr (0x4002e8ee).
            0x010 => s.fifo.iter().fold(s.raw, |raw, entry| raw | entry) & self.ram.read(0x00c),
            // Inferred W1C readback for read/modify/write acknowledgements in r_rwble_isr.
            0x018 => 0,
            // Inferred FIFO: r_rwble_isr_hack extracts count [9:5]
            // and status [30:10], then writes bit 0 to pop. r_lld_core_init resets bit 31.
            0x2d8 => (self.ram.read(off) & 0x1e) | ((s.fifo.len() as u32) << 5) | (s.fifo.front().copied().unwrap_or(0) << 10),
            _ => self.ram.read(off),
        }
    }

    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        let Some(s) = &mut self.state else {
            self.ram.write(off, v);
            return WriteEffect::NONE;
        };
        if !self.accessible { return WriteEffect::NONE }
        match off {
            // r_rwip_timer_hus_set cancels by masking TIMER. The FIFO-mode ISR
            // dispatches popped entries without a mask check (ROM 0x4002e722).
            0x00c => {
                if v & TIMER == 0 {
                    s.fifo.retain(|&source| source != TIMER);
                    s.alarm = None;
                }
                self.ram.write(off, v);
            }
            0x004 | 0x008 | 0x010 | 0x014 | 0x020 => {},
            // Checked on C3 rev v0.3: guest writes 00640064, reads 00000064.
            // Width beyond the exercised low byte remains inferred.
            0x08c => self.ram.write(off, v & 0xffff),
            // Inferred index/request: r_sch_prog_ble_push_hack.
            0x100 => {
                self.ram.write(off, v);
                if v & REQUEST != 0 && s.kicks.len() < 16 { s.kicks.push_back((v & 15) as u8); }
            }
            // Inferred reset/disable completion: r_rwip_driver_init / r_rwble_hw_disable.
            // One half-microsecond latency is a model choice, not a measured hardware delay.
            0x000 => {
                self.ram.write(off, v);
                if v & REQUEST != 0 { s.reset = Some(s.cycles + HALF_US_CYCLES); }
            }
            // Latch completion and 2 half-us ticks/us checked on C3 rev v0.3.
            // Atomicity and 80-cycle completion remain inferred.
            // Inferred atomic snapshot: r_rwip_time_get polls bit 31, masks 28 bits,
            // and returns 624 minus the fine register. Reads never advance time.
            0x01c => {
                if v & REQUEST != 0 {
                    self.ram.write(off, self.ram.read(off) | REQUEST);
                    s.latch = Some(s.cycles + HALF_US_CYCLES);
                }
            }
            // Inferred W1C: r_rwip_timer_hus_set and r_rwble_isr.
            0x018 => s.raw &= !v,
            0x2d8 => {
                self.ram.write(off, v & 0x1e);
                if v & REQUEST != 0 { s.fifo.clear(); } else if v & 1 != 0 { s.fifo.pop_front(); }
            }
            // Inferred alarm pair: r_rwip_timer_hus_set writes coarse then 624-hus.
            // Inferred: a past target fires now, rather than after wrap; hardware question.
            0x0f0 => {
                self.ram.write(off, v & 0x3ff);
                let target = (self.ram.read(0x0ec) as u64 & 0x0fff_ffff) * 625
                    + 624u64.saturating_sub((v & 0x3ff) as u64);
                let now = s.cycles / HALF_US_CYCLES;
                let delta = (target + PERIOD - now % PERIOD) % PERIOD;
                s.alarm = Some(if delta >= PERIOD / 2 { s.cycles } else { (now + delta) * HALF_US_CYCLES });
            }
            _ => self.ram.write(off, v),
        }
        WriteEffect::NONE
    }

    fn clock(&self) -> Option<ClockDomain> { self.state.as_ref().map(|_| ClockDomain::Cpu) }
    fn has_deadline(&self) -> bool { self.enabled() }
    fn next_deadline(&self) -> Option<u64> {
        let s = self.state.as_ref()?;
        let half_slot = (self.ram.read(0) & 0x100 != 0 && self.ram.read(0x0c) & 1 != 0 && s.raw & 1 == 0)
            .then(|| (s.cycles / (625 * HALF_US_CYCLES) + 1) * (625 * HALF_US_CYCLES));
        [s.reset, s.latch, s.alarm, half_slot, s.event.as_ref().map(|e| e.due)].into_iter().flatten().min().map(|t| t.saturating_sub(s.cycles).max(1))
    }
    fn irq_sources(&self) -> u64 {
        self.state.as_ref().is_some_and(|s| s.fifo.iter().fold(s.raw, |raw, entry| raw | entry) & self.ram.read(0x00c) != 0) as u64
    }
    fn tick(&mut self, ticks: u64) {
        let Some(s) = &mut self.state else { return };
        // Inferred half-slot status bit0; init/adv snapshots checked on C3 rev v0.3.
        if self.ram.read(0) & 0x100 != 0 && s.cycles / (625 * HALF_US_CYCLES) != (s.cycles + ticks) / (625 * HALF_US_CYCLES) {
            if self.ram.read(0x0c) & 1 != 0 && s.raw & 1 == 0 { s.raise(1); } else { s.raw |= 1; }
        }
        s.cycles += ticks;
        if s.reset.is_some_and(|t| t <= s.cycles) {
            s.reset = None;
            s.alarm = None;
            s.event = None;
            s.kicks.clear();
            s.connection = None;
            s.raw = 0;
            s.fifo.clear();
            self.ram.write(0, self.ram.read(0) & !REQUEST);
        }
        if s.latch.is_some_and(|t| t <= s.cycles) {
            // Snapshot at completion, even if the scheduler advances beyond the deadline.
            let hus = s.latch.take().unwrap() / HALF_US_CYCLES;
            self.ram.write(0x01c, (hus / 625) as u32 & 0x0fff_ffff);
            self.ram.write(0x020, 624 - (hus % 625) as u32);
        }
        if s.alarm.is_some_and(|t| t <= s.cycles) {
            s.alarm = None;
            s.raise(TIMER);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advertising_fixture() -> (BleLc, Vec<u8>) {
        let mut d = BleLc::default();
        d.enable();
        d.observe(true);
        for (i, &start) in BleLc::EM_STARTS.iter().enumerate() {
            d.write(0x204 + i as u32 * 4, (start / 4) << 18 | (0x20000 + start / 4));
        }
        let mut ram = vec![0; 0x14000];
        let base = crate::bus::DRAM_IN_SRAM;
        for (offset, value) in [(0, 2), (2, 10), (6, 624), (8, 0x200),
            (0x400, 4), (0x41c, 0x1400), (0x426, 0xe0),
            (0x1400, 0x140e), (0x1402, 0x0900), (0x1404, 0x2c00),
            (0x140e, 0x1400), (0x1410, 0x0a04), (0x1412, 0x2400)] {
            ram[base + offset..base + offset + 2].copy_from_slice(&(value as u16).to_le_bytes());
        }
        ram[base + 0x406..base + 0x40c].copy_from_slice(&[6, 5, 4, 3, 2, 1]);
        ram[base + 0x2c00..base + 0x2c03].copy_from_slice(&[2, 1, 6]);
        ram[base + 0x2400..base + 0x2404].copy_from_slice(&[3, 9, b'x', b'y']);
        (d, ram)
    }

    #[test]
    #[ignore = "needs the ESP32-C3 mask ROM ELF (ESP32SIM_ROM_DIR or ESP-IDF)"]
    fn mapping_boundaries_match_rom_em_base_reg_lut() {
        let bytes = std::fs::read(esp_soc::rom::require("esp32c3_rev3_rom.elf")).unwrap();
        let elf = esp_soc::elf::parse(&bytes).unwrap();
        let addr = elf.by_name["em_base_reg_lut"];
        let section = elf.sections.iter().find(|s| addr >= s.addr && (addr - s.addr) as usize + 51 * 4 <= s.data.len()).unwrap();
        let table = &section.data[(addr - section.addr) as usize..][..51 * 4];
        for (page, entry) in table.as_chunks::<4>().0.iter().enumerate() {
            let index = BleLc::EM_STARTS.partition_point(|&start| start <= page as u32 * 1024) - 1;
            assert_eq!(usize::from(entry[0]), index);
            assert_eq!(u32::from(u16::from_le_bytes([entry[2], entry[3]])), BleLc::EM_STARTS[index]);
        }
        let (mut d, ram) = advertising_fixture();
        assert!(d.mapped(0x2c00, 3, &ram).is_some());
        d.write(0x224, (0x3400 / 4) << 18 | 0x20d00);
        assert!(d.mapped(0x2c00, 3, &ram).is_none());
        d.write(0x224, 0);
        assert!(d.mapped(0x2c00, 3, &ram).is_none());
        assert!(d.mapped(0x2bfc, 8, &ram).is_none());
    }

    #[test]
    fn descriptor_chain_maps_address_data_and_conditional_scan_response() {
        let (mut d, mut ram) = advertising_fixture();
        let et = crate::bus::DRAM_IN_SRAM;
        let a = d.advertising(et, &ram).unwrap();
        assert_eq!(a.pdu, [0, 9, 6, 5, 4, 3, 2, 1, 2, 1, 6]);
        assert_eq!(&a.scan_response[8..], &[3, 9, b'x', b'y']);
        assert_eq!(a.channels, 7);
        ram[et + 0x1403] = 38;
        assert!(d.advertising(et, &ram).is_err());
        ram[et + 0x1403] = 9;
        // An unmapped region must not fall back to the preceding mapping.
        d.write(0x224, 0);
        assert!(d.advertising(et, &ram).is_err());
        assert!(d.mapped(0x2bff, 2, &ram).is_none());
        assert!(d.mapped(u32::MAX, usize::MAX, &ram).is_none());
    }

    #[test]
    fn channels_run_in_time_then_completion_is_masked_and_acknowledged() {
        let (mut d, mut ram) = advertising_fixture();
        d.write(0x100, REQUEST);
        d.service(&mut ram);
        assert!(d.take_observation().is_none());
        d.tick(10 * 625 * HALF_US_CYCLES - 1);
        d.service(&mut ram);
        assert!(d.take_observation().is_none());
        d.tick(1);
        d.service(&mut ram);
        assert!(d.take_observation().unwrap().starts_with("[ble-config] SCAN_RSP name=\"xy\""));
        let first = d.take_observation().unwrap();
        assert!(first.contains("hus=6250 channel=37 type=ADV_IND AdvA=01:02:03:04:05:06"));
        for channel in [38, 39] {
            let delta = d.next_deadline().unwrap();
            d.tick(delta - 1);
            d.service(&mut ram);
            assert!(d.take_observation().is_none());
            d.tick(1);
            d.service(&mut ram);
            assert!(d.take_observation().unwrap().contains(&format!("channel={channel} type=ADV_IND")));
        }
        assert_eq!(d.irq_sources(), 0);
        d.tick(d.next_deadline().unwrap());
        d.service(&mut ram);
        assert_eq!((half(&ram, crate::bus::DRAM_IN_SRAM) >> 3) & 7, 3);
        assert_eq!(d.read(0x10), 0);
        d.write(0xc, 1 << 5);
        assert_eq!(d.read(0x10), 1 << 5);
        assert_eq!(d.irq_sources(), 1);
        assert_eq!(d.read(0x2d8), (1 << 5) | (1 << 15));
        d.write(0x18, 1 << 5);
        d.write(0x2d8, 1);
        assert_eq!(d.irq_sources(), 0);
        assert_eq!(d.read(0x2d8), 0);
        assert!(d.take_observation().is_none());
        assert_eq!(d.next_deadline(), None);
    }

    #[test]
    fn channel_map_reset_and_bad_descriptor_do_not_transmit_extra_packets() {
        for map in 0..8 {
            let (mut d, mut ram) = advertising_fixture();
            ram[crate::bus::DRAM_IN_SRAM + 0x426] = map << 5;
            d.write(0x100, REQUEST);
            d.service(&mut ram);
            for _ in 0..4 {
                if let Some(delta) = d.next_deadline() { d.tick(delta); d.service(&mut ram); }
            }
            let mut emitted = Vec::new();
            while let Some(line) = d.take_observation() {
                if line.starts_with("[ble-air]") { emitted.push(line); }
            }
            let channels: Vec<_> = (0..3).filter(|ch| map & (1 << ch) != 0).map(|ch| ch + 37).collect();
            assert_eq!(emitted.len(), channels.len());
            for (line, channel) in emitted.iter().zip(channels) { assert!(line.contains(&format!("channel={channel}"))); }
        }
        let (mut d, mut ram) = advertising_fixture();
        d.write(0x100, REQUEST);
        d.service(&mut ram);
        d.write(0, REQUEST);
        d.tick(1_000_000);
        d.service(&mut ram);
        assert!(d.take_observation().is_none());
        ram[crate::bus::DRAM_IN_SRAM + 0x1403] = 255;
        d.write(0x100, REQUEST);
        d.service(&mut ram);
        assert!(d.take_observation().unwrap().starts_with("[ble-error]"));
        assert_eq!((half(&ram, crate::bus::DRAM_IN_SRAM) >> 3) & 7, 4);
    }


    #[test]
    fn active_scan_uses_guest_rx_ring_and_ifs_before_response() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        d.scan(true);
        d.write(0x24, 0x1000);
        for index in 0..3 {
            let rx = base + 0x1000 + index as usize * 20;
            ram[rx..rx + 2].copy_from_slice(&(0x1000 + ((index + 1) % 3) * 20u16).to_le_bytes());
            ram[rx + 18..rx + 20].copy_from_slice(&(0x7800 + index * 32u16).to_le_bytes());
        }
        d.write(0x100, REQUEST);
        d.service(&mut ram);
        let mut lines = Vec::new();
        while let Some(delta) = d.next_deadline() {
            d.tick(delta - 1);
            d.service(&mut ram);
            assert!(d.take_observation().is_none(), "radio action before deadline");
            d.tick(1);
            d.service(&mut ram);
            while let Some(line) = d.take_observation() {
                if !line.starts_with("[ble-config]") { lines.push(line); }
            }
        }
        assert_eq!(lines.len(), 9);
        let time = |line: &str| -> u64 { line.split_whitespace().find_map(|v| v.strip_prefix("hus=")).unwrap().parse().unwrap() };
        for (index, exchange) in lines.as_chunks::<3>().0.iter().enumerate() {
            assert!(exchange[0].contains("type=ADV_IND"));
            assert!(exchange[1].contains("type=SCAN_REQ"));
            assert!(exchange[2].contains("type=SCAN_RSP"));
            assert!(exchange[2].contains("name=\"xy\""));
            assert_eq!(time(&exchange[1]) - time(&exchange[0]), airtime(11) / HALF_US_CYCLES + 300);
            assert_eq!(time(&exchange[2]) - time(&exchange[1]), airtime(14) / HALF_US_CYCLES + 300);
            for line in exchange { assert!(line.contains(&format!("channel={}", 37 + index))); }
            let rx = base + 0x1000 + index * 20;
            assert_ne!(half(&ram, rx) & 0x8000, 0);
            assert_eq!(half(&ram, rx + 2), 0);
            assert_eq!(half(&ram, rx + 4), 0x0c03);
            assert_eq!(half(&ram, rx + 6), ((37 + index as u16) << 8) | 216);
            assert_eq!(&ram[base + 0x7800 + index * 32..base + 0x780c + index * 32],
                &[1, 0, 0, 0, 0, 2, 6, 5, 4, 3, 2, 1]);
        }
        assert_eq!(d.read(0x24), 0x1000);
        assert_eq!(d.state.as_ref().unwrap().raw, (1 << 4) | (1 << 5));
        let snapshot = ram.clone();
        assert_eq!(d.receive(0, 37, 0, &scan_request(&[0; 8]), &mut ram), Err("RX descriptor still owned by guest"));
        assert_eq!(snapshot, ram);
        assert!(d.receive(0, 0, 0, &[3, 12], &mut ram).is_err());
    }

    #[test]
    fn connect_command_does_not_change_an_inflight_scan_request() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        d.scan(true); d.write(0x24, 0x1000);
        put_half(&mut ram, base + 0x1000, 0x1014);
        put_half(&mut ram, base + 0x1012, 0x7800);
        d.write(0x100, REQUEST); d.service(&mut ram);
        for _ in 0..2 { d.tick(d.next_deadline().unwrap()); d.service(&mut ram); }
        d.command("connect").unwrap();
        d.tick(d.next_deadline().unwrap()); d.service(&mut ram);
        assert_eq!(half(&ram, base + 0x1004), 0x0c03);
        assert!(d.state.as_ref().unwrap().connection.is_none());
        assert!(d.state.as_ref().unwrap().host.connecting);
    }

    #[test]
    fn scanner_does_not_connect_to_scannable_nonconnectable_advertising() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        put_half(&mut ram, base + 0x1402, 0x0906); // ADV_SCAN_IND
        d.scan(true);
        d.command("connect").unwrap();
        d.write(0x100, REQUEST);
        d.service(&mut ram);
        for _ in 0..2 {
            d.tick(d.next_deadline().unwrap());
            d.service(&mut ram);
        }
        let mut observations = Vec::new();
        while let Some(line) = d.take_observation() { observations.push(line); }
        assert!(observations.iter().any(|line| line.contains("type=SCAN_REQ")));
        assert!(!observations.iter().any(|line| line.contains("type=CONNECT_IND")));
        assert!(d.state.as_ref().unwrap().host.connecting);
    }

    #[test]
    fn connection_anchor_must_fit_the_programmed_receive_window() {
        for (window, anchor, valid) in [(20,1000,true), (20,1080,true), (20,1081,false),
            (20,999,false), (0x8001,2250,true), (0x8001,2251,false)] {
            let (mut d, mut ram) = advertising_fixture();
            let base = crate::bus::DRAM_IN_SRAM;
            let cs = base + 0x400;
            put_half(&mut ram, cs, 3);
            put_half(&mut ram, cs + 22, 5 << 8);
            put_half(&mut ram, cs + 26, window);
            ram[cs + 34..cs + 39].copy_from_slice(&[255,255,255,255,31]);
            d.state.as_mut().unwrap().connection = Some(Connection {
                anchor: anchor * HALF_US_CYCLES, ..Connection::default()
            });
            let mut event = Event { entry: base, due: 1000 * HALF_US_CYCLES,
                advertising: None, phase: ScanPhase::Advertising, connection_phase: 0 };
            assert_eq!(d.connection_event(&mut event, cs, &mut ram).is_ok(), valid);
            if valid { assert_eq!(event.due, anchor * HALF_US_CYCLES); }
        }
    }

    #[test]
    fn csa2_rejected() {
        for (control, result) in [(5 << 8, Ok(true)), ((1 << 14) | (5 << 8), Err("CSA#2 not supported"))] {
            let (mut d, mut ram) = advertising_fixture();
            let base = crate::bus::DRAM_IN_SRAM;
            let cs = base + 0x400;
            put_half(&mut ram, cs + 22, control);
            put_half(&mut ram, cs + 26, 20);
            ram[cs + 34..cs + 39].copy_from_slice(&[255,255,255,255,31]);
            d.state.as_mut().unwrap().connection = Some(Connection { anchor: 1000 * HALF_US_CYCLES, ..Connection::default() });
            let mut event = Event { entry: base, due: 1000 * HALF_US_CYCLES,
                advertising: None, phase: ScanPhase::Advertising, connection_phase: 0 };
            assert_eq!(d.connection_event(&mut event, cs, &mut ram), result);
        }
    }

    #[test]
    fn null_tx_link_sends_an_empty_pdu() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        put_half(&mut ram, base + 0x41c, 0); // CS+28 null link; logical 0 is the event table.
        d.state.as_mut().unwrap().connection = Some(Connection::default());
        let mut event = Event { entry: base, due: 0, advertising: None,
            phase: ScanPhase::Advertising, connection_phase: 3 };
        d.connection_event(&mut event, base + 0x400, &mut ram).unwrap();
        assert!(d.take_observation().unwrap().ends_with("type=DATA pdu=0100"));
        assert!(d.state.as_ref().unwrap().connection.as_ref().unwrap().pending.as_ref().unwrap().descriptor.is_none());
        assert_eq!(half(&ram, base), 2);
    }

    #[test]
    fn unacknowledged_connection_tx_reuses_the_cached_packet() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        put_half(&mut ram, base + 0x1402, 1); // empty LL data PDU
        d.state.as_mut().unwrap().connection = Some(Connection::default());
        let mut event = Event { entry: base, due: 0, advertising: None,
            phase: ScanPhase::Advertising, connection_phase: 3 };
        d.connection_event(&mut event, base + 0x400, &mut ram).unwrap();
        let first = d.take_observation().unwrap();
        put_half(&mut ram, base + 0x1402, 0xffff); // would be invalid if re-read
        event.connection_phase = 3;
        d.connection_event(&mut event, base + 0x400, &mut ram).unwrap();
        let repeated = d.take_observation().unwrap();
        assert_eq!(first.split("pdu=").nth(1), repeated.split("pdu=").nth(1));
        assert_eq!(half(&ram, base + 0x1400) & 0x8000, 0);
    }

    #[test]
    fn host_configuration_survives_controller_reset() {
        let mut d = BleLc::default(); d.enable();
        d.scan(true); d.command("connect").unwrap();
        d.command("read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8").unwrap();
        d.state.as_mut().unwrap().connection = Some(Connection::default());
        d.reset_controller();
        let s = d.state.as_ref().unwrap();
        assert!(s.host.scanning && s.host.connecting && s.host.read_request.is_some());
        assert!(s.connection.is_none());
    }

    #[test]
    fn commands_reject_disabled_busy_and_unsupported_states() {
        let mut d = BleLc::default();
        assert!(d.command("connect").is_err());
        assert!(d.command("central-stop").is_err());
        d.enable();
        for cmd in ["disconnect", "central-stop", "discover", "read 1", "write 1 00"] { assert!(d.command(cmd).is_err()); }
        d.command("connect").unwrap();
        assert_eq!(d.pending_commands(), 1);
        let read = "read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8";
        d.command(read).unwrap();
        assert!(d.command(read).is_err());
        assert_eq!(d.pending_commands(), 2);
        d.state.as_mut().unwrap().connection = Some(Connection::default());
        assert!(d.command("connect").is_err());
        d.command("disconnect").unwrap();
        assert!(d.command("disconnect").is_err());
        assert_eq!(d.state.as_ref().unwrap().connection.as_ref().unwrap().outgoing[0], [3,2,2,0x13]);
        d.command("central-stop").unwrap();
        assert!(d.state.as_ref().unwrap().connection.as_ref().unwrap().silent);
    }

    #[test]
    fn version_reply_is_dispatched_from_guest_tx_and_duplicates_are_suppressed() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        put_half(&mut ram, base + 0x1402, 0x0603);
        ram[base + 0x2c00..base + 0x2c06].copy_from_slice(&[0x0c,9,0xff,0xff,1,0]);
        d.state.as_mut().unwrap().connection = Some(Connection::default());
        let mut e = Event { entry: base, due: 0, advertising: None, phase: ScanPhase::Advertising, connection_phase: 3 };
        d.connection_event(&mut e, base + 0x400, &mut ram).unwrap();
        assert_eq!(d.state.as_ref().unwrap().connection.as_ref().unwrap().outgoing[0], [3,6,0x0c,9,0xff,0xff,1,0]);
        e.connection_phase = 3;
        d.connection_event(&mut e, base + 0x400, &mut ram).unwrap();
        assert_eq!(d.state.as_ref().unwrap().connection.as_ref().unwrap().outgoing.len(), 1);
    }

    #[test]
    fn host_configuration_survives_machine_reboot() {
        let mut m = crate::machine([0;6], 4 << 20);
        m.bus.periph.enable_ble_full(false);
        let d = &mut m.bus.periph.ble_lc;
        d.scan(true); d.command("connect").unwrap();
        d.command("read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8").unwrap();
        m.reboot();
        let h = &m.bus.periph.ble_lc.state.as_ref().unwrap().host;
        assert!(h.scanning && h.connecting && h.read_request.is_some());
    }

    #[test]
    fn central_receive_uses_the_transmitted_packet_and_header() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        d.write(0x24, 0x1000);
        put_half(&mut ram, base + 0x1000, 0x1014);
        put_half(&mut ram, base + 0x1012, 0x7800);
        d.state.as_mut().unwrap().connection = Some(Connection { channel: 5, ..Connection::default() });
        let mut event = Event { entry: base, due: 0, advertising: None, phase: ScanPhase::Advertising, connection_phase: 1 };
        d.connection_event(&mut event, base + 0x400, &mut ram).unwrap();
        let c = d.state.as_mut().unwrap().connection.as_mut().unwrap();
        c.outgoing.push_back(vec![3,2,2,0x13]);
        c.central.sn = 1;
        d.connection_event(&mut event, base + 0x400, &mut ram).unwrap();
        assert_eq!(half(&ram, base + 0x1004), 1);
        assert_eq!(event.connection_phase, 3);
    }

    #[test]
    fn peripheral_version_request_gets_one_response_without_att() {
        let mut c = Connection::default();
        c.version();
        c.version();
        assert_eq!(c.outgoing.len(), 1);
        assert_eq!(c.outgoing[0], [3,6,0x0c,9,0xff,0xff,1,0]);
    }

    #[test]
    fn csa1_hops_and_remaps_sparse_channels() {
        let all = (1 << 37) - 1;
        assert_eq!((0..9).map(|n| csa1((n * 5) % 37, 5, all).unwrap()).collect::<Vec<_>>(),
            [5, 10, 15, 20, 25, 30, 35, 3, 8]);
        let map = (1 << 0) | (1 << 10) | (1 << 36);
        assert_eq!(csa1(0, 5, map), Ok(36)); // 5 % 3 -> third used channel
        assert_eq!(csa1(5, 5, map), Ok(10)); // already enabled
        assert_eq!(csa1(32, 5, map), Ok(0)); // modulo-37 wrap
        for (prev, hop, map) in [(37, 5, all), (0, 4, all), (0, 17, all), (0, 5, 1), (0, 5, 1 << 37)] {
            assert!(csa1(prev, hop, map).is_err());
        }
    }

    #[test]
    fn sequence_retransmits_until_ack_and_rejects_duplicate_payload() {
        let mut central = Sequence::default();
        let mut peripheral = Sequence::default();
        central.sent = true;
        let request = central.header(1);
        assert!(peripheral.accept(request));
        assert!(!peripheral.accept(request));
        peripheral.sent = true;
        let response = peripheral.header(0x13);
        assert_eq!(response, 0x17); // MD preserved, ACK next central SN=1.
        assert!(central.acknowledge(response));
        assert!(!central.acknowledge(response));
        assert!(central.accept(response));
        assert!(!central.accept(response));
        assert!(!peripheral.acknowledge(request)); // retransmit response unchanged
        assert_eq!(peripheral.header(0x13), response);
        assert!(peripheral.acknowledge(central.header(1)));
        assert_eq!(peripheral.sn, 1);
    }

    #[test]
    fn sync_timestamp_recovers_packet_start_across_half_slots_and_wrap() {
        for start in [0, 536, 537, 624, 625, PERIOD - 44, PERIOD + 625] {
            let (mut d, mut ram) = advertising_fixture();
            let base = crate::bus::DRAM_IN_SRAM;
            d.write(0x90, 4 << 8); // Guest lld_exp_sync_pos_tab[0] = 44 us.
            d.write(0x24, 0x1000);
            put_half(&mut ram, base + 0x1000, 0x1014);
            put_half(&mut ram, base + 0x1012, 0x7800);
            d.receive(1, 5, start * HALF_US_CYCLES, &[1, 0], &mut ram).unwrap();
            let rx = base + 0x1000;
            let coarse = half(&ram, rx + 8) as u64 | ((half(&ram, rx + 10) as u64) << 16);
            let fine = half(&ram, rx + 12) as u64 & 1023;
            // r_lld_adv_pkt_rx_connect_post subtracts 2*44, then borrows 625.
            let recovered = (coarse * 625 + 624 - fine + PERIOD - 88) % PERIOD;
            assert_eq!(recovered, start % PERIOD);
            assert_eq!(half(&ram, rx + 12) >> 11, 1);
        }
    }

    #[test]
    fn tx_descriptor_is_released_only_after_peer_ack() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        d.write(0x24, 0x1000);
        put_half(&mut ram, base + 0x1000, 0x1014);
        put_half(&mut ram, base + 0x1012, 0x7800);
        d.state.as_mut().unwrap().connection = Some(Connection {
            channel: 5,
            incoming: vec![1, 0],
            peripheral: Sequence { sent: true, ..Sequence::default() },
            pending: Some(TxPacket { pdu: vec![3, 1, 0x0c], descriptor: Some(base + 0x1400) }),
            ..Connection::default()
        });
        let mut event = Event { entry: base, due: airtime(2), advertising: None,
            phase: ScanPhase::Advertising, connection_phase: 2 };
        d.connection_event(&mut event, base + 0x400, &mut ram).unwrap();
        assert_eq!(half(&ram, base + 0x1400) & 0x8000, 0);
        assert!(d.state.as_ref().unwrap().connection.as_ref().unwrap().pending.is_some());
        d.state.as_mut().unwrap().connection.as_mut().unwrap().incoming = vec![5, 0];
        event.connection_phase = 2;
        d.connection_event(&mut event, base + 0x400, &mut ram).unwrap();
        assert_ne!(half(&ram, base + 0x1400) & 0x8000, 0);
        assert_eq!(half(&ram, base + 0x41c), 0x140e);
        assert!(d.state.as_ref().unwrap().connection.as_ref().unwrap().pending.is_none());
        assert_ne!(d.state.as_ref().unwrap().raw & (1 << 1), 0);
    }

    #[test]
    fn latch_is_atomic_and_wraps_in_modeled_time() {
        let mut d = BleLc::default();
        d.enable();
        for hus in [0, 623, 624, 625, PERIOD - 1, PERIOD, PERIOD + 625] {
            d.state.as_mut().unwrap().cycles = hus * HALF_US_CYCLES;
            d.write(0x01c, REQUEST);
            for _ in 0..10 { assert_ne!(d.read(0x01c) & REQUEST, 0); }
            d.tick(HALF_US_CYCLES - 1);
            assert_ne!(d.read(0x01c) & REQUEST, 0);
            d.tick(1);
            let snapshot = (d.read(0x01c), d.read(0x020));
            assert_eq!(snapshot, (((hus + 1) / 625) as u32 & 0x0fff_ffff, 624 - ((hus + 1) % 625) as u32));
            d.tick(1000);
            assert_eq!(snapshot, (d.read(0x01c), d.read(0x020)));
        }
    }

    #[test]
    fn reset_completes_without_polling_and_cancels_alarm() {
        let mut d = BleLc::default();
        d.enable();
        for control in [0x100, 0] {
            d.write(0x0ec, 10);
            d.write(0x0f0, 624);
            d.write(0, REQUEST | control);
            d.tick(HALF_US_CYCLES - 1);
            assert_eq!(d.read(0), REQUEST | control);
            d.tick(1);
            assert_eq!(d.read(0), control);
            assert_eq!(d.read(4), 0x0900_1b00);
            d.tick(1_000_000);
            assert_eq!(d.state.as_ref().unwrap().raw, u32::from(control & 0x100 != 0));
        }
    }

    #[test]
    fn alarm_mask_ack_rearm_and_wrap() {
        let mut d = BleLc::default();
        d.enable();
        for now in [0, PERIOD - 2, PERIOD + 10] {
            d.state.as_mut().unwrap().cycles = now * HALF_US_CYCLES;
            let target = (now + 3) % PERIOD;
            d.write(0x00c, 0);
            d.write(0x0ec, (target / 625) as u32);
            d.write(0x0f0, 624 - (target % 625) as u32);
            assert_eq!(d.next_deadline(), Some(3 * HALF_US_CYCLES));
            d.tick(3 * HALF_US_CYCLES - 1);
            assert_eq!(d.state.as_ref().unwrap().raw, 0);
            d.tick(1);
            assert_eq!(d.state.as_ref().unwrap().raw, TIMER);
            assert_eq!(d.irq_sources(), 0);
            d.write(0x00c, TIMER);
            assert_eq!(d.read(0x010), TIMER);
            assert_eq!(d.irq_sources(), 1);
            assert_eq!(d.read(0x2d8), (TIMER << 10) | (1 << 5));
            d.write(0x2d8, 1);
            assert_eq!(d.read(0x2d8), 0);
            d.write(0x018, 1);
            assert_eq!(d.irq_sources(), 1);
            d.write(0x018, TIMER);
            assert_eq!(d.irq_sources(), 0);
            d.tick(10_000);
            assert_eq!(d.read(0x010), 0);
        }
    }

    #[test]
    fn event_fine_timestamp_and_past_due_event() {
        for (now, fine, due) in [(0, 400, 6474), (7000, 400, 7000)] {
            let (mut d, mut ram) = advertising_fixture();
            ram[crate::bus::DRAM_IN_SRAM + 6..crate::bus::DRAM_IN_SRAM + 8].copy_from_slice(&(fine as u16).to_le_bytes());
            d.tick(now * HALF_US_CYCLES);
            d.write(0x100, REQUEST);
            d.service(&mut ram);
            if now < due {
                assert!(d.take_observation().is_none());
                assert_eq!(d.next_deadline(), Some((due - now) * HALF_US_CYCLES));
                d.tick((due - now) * HALF_US_CYCLES - 1);
                d.service(&mut ram);
                assert!(d.take_observation().is_none());
                d.tick(1);
                d.service(&mut ram);
            }
            assert!(d.take_observation().unwrap().starts_with("[ble-config]"));
            assert!(d.take_observation().unwrap().contains(&format!("hus={due} channel=37")));
        }
    }

    #[test]
    fn past_due_alarm_is_due_now_not_after_wrap() {
        let mut d = BleLc::default(); d.enable();
        d.tick(1000 * HALF_US_CYCLES);
        d.write(0xc, TIMER);
        d.write(0xec, 1); d.write(0xf0, 624);
        assert_eq!(d.next_deadline(), Some(1));
        d.tick(0);
        assert_eq!(d.read(0x10), TIMER);
    }

    #[test]
    fn each_completed_event_retains_an_end_and_cancelled_timer_stays_out() {
        let (mut d, mut ram) = advertising_fixture();
        let base = crate::bus::DRAM_IN_SRAM;
        ram.copy_within(base..base + 16, base + 16);
        ram[base + 0x426] = 0; // No channels: complete both ETs without an intervening ISR.
        d.write(0xc, TIMER | (1 << 5));
        d.write(0xec, 0); d.write(0xf0, 624);
        d.tick(1);
        assert_eq!(d.read(0x2d8) >> 10, TIMER);
        d.write(0xc, 1 << 5); // r_rwip_timer_hus_set's cancellation, no FIFO pop.
        d.write(0x100, REQUEST); d.write(0x100, REQUEST | 1);
        d.tick(10 * 625 * HALF_US_CYCLES);
        d.service(&mut ram); d.service(&mut ram);
        for pending in [2, 1] {
            assert_eq!((d.read(0x2d8) >> 5) & 31, pending);
            assert_eq!(d.read(0x2d8) >> 10, 1 << 5);
            assert_eq!(d.irq_sources(), 1);
            // The ROM pops before dispatch and acknowledges the completed source.
            d.write(0x2d8, 1); d.write(0x18, 1 << 5);
        }
        assert_eq!(d.read(0x2d8), 0); assert_eq!(d.irq_sources(), 0);
        for entry in [base, base + 16] { assert_eq!((half(&ram, entry) >> 3) & 7, 3); }
    }

    #[test]
    fn half_slot_deadline_and_ack_are_chunk_independent() {
        let mut d = BleLc::default(); d.enable();
        d.write(0, 0x100); d.write(0xc, 1);
        assert_eq!(d.next_deadline(), Some(50_000));
        d.tick(49_999); assert_eq!(d.read(0x14), 0);
        assert_eq!(d.next_deadline(), Some(1));
        d.tick(1); assert_eq!(d.read(0x14), 1); assert_eq!(d.irq_sources(), 1);
        assert_eq!(d.next_deadline(), None);
        d.write(0x2d8, 1); d.write(0x18, 1);
        assert_eq!(d.next_deadline(), Some(50_000));
        d.tick(50_000); assert_eq!(d.read(0x14), 1);
    }

    #[test]
    fn gated_writes_are_ignored_and_bt_reset_cancels_work() {
        let mut p = crate::periph::Peripherals::new([0; 6]);
        p.enable_ble_full(false);
        p.write32(0x6003108c, 12);
        p.write32(0x60026014, 0xffff_ffdf);
        assert_eq!(p.read32(0x6003108c), 0);
        for bit in [3, 9, 10, 11, 12, 13] {
            p.write32(0x6003108c, 34);
            p.write32(0x6003101c, REQUEST);
            p.write32(0x600310ec, 100);
            p.write32(0x600310f0, 624);
            p.write32(0x60026018, 1 << bit);
            p.write32(0x6003108c, 56);
            p.write32(0x60026018, 0);
            assert_eq!(p.read32(0x6003108c), 0);
            assert_eq!(p.ble_lc.next_deadline(), None);
            assert_eq!(p.read32(0x60031014), 0);
        }
    }

    #[test]
    fn disabled_observer_does_not_format_or_queue_and_reboot_keeps_loss_count() {
        let mut d = BleLc::default(); d.enable();
        d.state.as_mut().unwrap().observe(|| panic!("disabled observer formatted a line"));
        assert!(d.take_observation().is_none());
        d.observe(true);
        let s = d.state.as_mut().unwrap();
        s.packets.push_back("queued".into()); s.dropped = 7;
        let mut m = crate::machine([0; 6], 4 << 20);
        m.bus.periph.ble_lc = d;
        m.reboot();
        assert!(m.bus.periph.work_pending);
        assert_eq!(m.bus.periph.misc.active_optional, [0x31]);
        assert_eq!(m.bus.periph.ble_lc.take_observation().as_deref(), Some("[ble-observer] dropped=7"));
        assert_eq!(m.bus.periph.ble_lc.take_observation().as_deref(), Some("queued"));
    }

    #[test]
    fn silicon_readbacks_follow_configuration_and_power_state() {
        let mut p = crate::periph::Peripherals::new([0; 6]);
        p.enable_ble_full(false);
        assert_eq!(p.read32(0x60026014), 0xfffc_e030);
        assert_eq!(p.read32(0x600c0024), 0x0200_1001);
        assert_eq!(p.read32(0x60031004), 0);
        p.write32(0x60026014, 0xffff_ffdf);
        assert_eq!(p.read32(0x60031004), 0x0900_1b00);
        for (addr, mask) in [(0x60026014, 1 << 16), (0x60026018, 1 << 11),
            (0x60008088, 1 << 11), (0x6000808c, 1 << 22)] {
            let before = p.read32(addr);
            p.write32(addr, before ^ mask);
            assert_eq!(p.read32(0x60031004), 0);
            p.write32(addr, before);
            assert_eq!(p.read32(0x60031004), 0x0900_1b00);
        }
        assert_eq!(p.read32(0x60031008), 0x0f22_d0b0);
        assert_eq!(p.read32(0x6003107c), 0xe400_e400);
        let config = p.read32(0x60031048);
        p.write32(0x60031048, config | 0x1f0);
        assert_eq!(p.read32(0x60031048), 0x0003_fff7);
        p.write32(0x60031070, 0);
        assert_eq!(p.read32(0x60031070), 2);
        p.write32(0x6003108c, 0x0064_0064);
        assert_eq!(p.read32(0x6003108c), 100);
        for (value, expected) in [(0x331, 0x331), (0x221, 0x1221), (0x1220, 0x220)] {
            p.write32(0x600310f8, value);
            assert_eq!(p.read32(0x600310f8), expected);
        }
    }

    #[test]
    fn optional_device_routes_source_eight_only_when_enabled() {
        let mut p = crate::periph::Peripherals::new([0; 6]);
        assert_eq!(p.ble_lc.clock(), None);
        assert!(!p.ble_lc.has_deadline());
        p.write32(0x6003101c, REQUEST);
        p.tick(1000);
        assert_eq!(p.read32(0x6003101c), REQUEST);
        assert!(p.misc.active_optional.is_empty());
        p.enable_ble_full(false);
        assert!(p.work_pending);
        assert_eq!(p.misc.active_optional, [0x31]);
        p.write32(0x60026014, 0xffff_ffdf);
        p.write32(0x600c2020, 9);
        p.write32(0x6003100c, TIMER);
        p.write32(0x600310ec, 1);
        p.write32(0x600310f0, 624);
        p.tick(625 * HALF_US_CYCLES);
        p.refresh_lines();
        assert_eq!(p.source_status()[0] & (1 << 8), 1 << 8);
        assert_ne!(p.intc.lines.level & (1 << 9), 0);
        p.write32(0x60031018, TIMER);
        p.write32(0x600312d8, 1);
        p.refresh_lines();
        assert_eq!(p.intc.lines.level & (1 << 9), 0);
    }
}
