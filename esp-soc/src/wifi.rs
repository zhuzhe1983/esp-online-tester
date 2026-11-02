//! The virtual air interface: 802.11 frame helpers and a minimal access point the emulated MAC
//! "hears". The AP beacons, answers probe requests, and completes open-system authentication and
//! association; data frames are handed to the network backend (docs/networking-plan.md).

#[cfg(test)]
mod tests;
mod mac;
pub use mac::{WifiMac, TXQ_COUNT};

pub fn mac_str(m: &[u8]) -> String { m.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(":") }

fn ies(f: &[u8], body: usize) -> Vec<(u8, &[u8])> {
    let mut v = Vec::new(); let mut i = body;
    while i + 2 <= f.len() { let (id, l) = (f[i], f[i + 1] as usize); if i + 2 + l > f.len() { break; } v.push((id, &f[i + 2..i + 2 + l])); i += 2 + l; }
    v
}
fn mgmt_body_offset(subtype: u16) -> usize { match subtype { 8 | 5 => 24 + 12, 0 => 24 + 4, 1 => 24 + 6, 11 => 24 + 6, _ => 24 } }

/// One-line description of an 802.11 frame (frame control, addresses, SSID for management frames).
pub fn describe(f: &[u8]) -> String {
    if f.len() < 24 { return format!("{} bytes (short): {:02x?}", f.len(), f); }
    let fc = u16::from_le_bytes([f[0], f[1]]);
    let (ty, st) = ((fc >> 2) & 3, (fc >> 4) & 0xf);
    let kind = match (ty, st) {
        (0, 0) => "assoc-req", (0, 1) => "assoc-resp", (0, 4) => "probe-req", (0, 5) => "probe-resp", (0, 8) => "beacon",
        (0, 10) => "disassoc", (0, 11) => "auth", (0, 12) => "deauth", (0, 13) => "action",
        (1, 11) => "rts", (1, 12) => "cts", (1, 13) => "ack", (1, 10) => "ps-poll",
        (2, 0) => "data", (2, 4) => "null", (2, 8) => "qos-data", (2, 12) => "qos-null", _ => "?",
    };
    let mut s = format!("{} bytes {} ({}/{}) a1={} a2={} a3={}", f.len(), kind, ty, st, mac_str(&f[4..10]), mac_str(&f[10..16]), mac_str(&f[16..22]));
    if ty == 0 { for (id, d) in ies(f, mgmt_body_offset(st)) { if id == 0 { s += &format!(" ssid='{}'", String::from_utf8_lossy(d)); } } }
    if ty == 2 && f.len() >= 32 { let off = if st & 8 != 0 { 26 } else { 24 }; if f.len() >= off + 8 && f[off] == 0xaa { let et = u16::from_be_bytes([f[off + 6], f[off + 7]]); s += &format!(" ethertype={:#06x}", et); } }
    s
}

/// True for a beacon frame (management subtype 8).
pub fn is_beacon(f: &[u8]) -> bool { f.len() >= 2 && f[0] & 0x0c == 0 && (f[0] >> 4) & 0xf == 8 }

/// RSN information element advertised in beacons and echoed in handshake message 3: WPA2-PSK, CCMP.
pub const RSN_IE: &[u8] = &[48, 20, 1, 0, 0x00, 0x0f, 0xac, 4, 1, 0, 0x00, 0x0f, 0xac, 4, 1, 0, 0x00, 0x0f, 0xac, 2, 0, 0];

#[derive(Clone, Debug)]
pub struct ApConfig {
    pub ssid: String, pub bssid: [u8; 6], pub channel: u8, pub psk: Option<String>,
    /// the network's clock at power-on, seconds since 1970 (`epoch=`); without it the host's clock
    /// at the first SNTP answer
    pub epoch_s: Option<u64>,
}

impl ApConfig {
    /// Parse the shared CLI/browser AP configuration. Unknown keys are errors so a
    /// misspelled passphrase option cannot silently configure an open network.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut cfg = Self {
            ssid: "esp32sim".into(), bssid: [0x02, 0x53, 0x49, 0x4d, 0x00, 0x01], channel: 6, psk: None, epoch_s: None,
        };
        if spec.is_empty() { return Ok(cfg); }
        for entry in spec.split(',') {
            let (key, value) = entry.split_once('=')
                .ok_or_else(|| format!("invalid WiFi option '{entry}': expected key=value"))?;
            match key {
                "ssid" => cfg.ssid = value.to_string(),
                "chan" | "channel" | "ch" => {
                    cfg.channel = value.parse::<u8>().ok().filter(|n| (1..=14).contains(n))
                        .ok_or_else(|| format!("invalid WiFi channel '{value}': expected 1 through 14"))?;
                }
                "psk" | "password" | "pass" => cfg.psk = Some(value.to_string()),
                "epoch" => cfg.epoch_s = Some(crate::net::parse_epoch(value)?),
                "bssid" => cfg.bssid = parse_bssid(value)?,
                _ => return Err(format!("unknown WiFi option '{key}'")),
            }
        }
        Ok(cfg)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaState { Idle, Authenticated, Associated }

/// A frame the AP puts on the air, with the emulated time (µs) it should reach the station.
pub struct AirFrame { pub at_us: u64, pub frame: Vec<u8> }

/// WPA2 four-way handshake state (AP side).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WpaState {
    #[default]
    Idle,
    AwaitingMessage2,
    AwaitingMessage4,
    Installed,
}

#[derive(Default)]
pub struct Wpa {
    pub pmk: [u8; 32],
    pub anonce: [u8; 32],
    pub gtk: [u8; 16],
    pub replay: u64,
    pub state: WpaState,
}

/// Six hexadecimal octets separated by colons.
fn parse_bssid(value: &str) -> Result<[u8; 6], String> {
    let invalid = || format!("invalid WiFi BSSID '{value}': expected six hexadecimal octets");
    let mut bssid = [0u8; 6];
    let mut octets = value.split(':');
    for byte in &mut bssid {
        let octet = octets.next().filter(|s| s.len() == 2 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(invalid)?;
        *byte = u8::from_str_radix(octet, 16).map_err(|_| invalid())?;
    }
    if octets.next().is_some() { return Err(invalid()); }
    Ok(bssid)
}

/// A network around the station, heard on the air but not joinable: it answers probe requests,
/// so a scan lists it with its channel, signal and security (the `bssid`, `ssid`, `primary`,
/// `rssi` and `authmode` of IDF's `wifi_ap_record_t`, v5.5.5
/// components/esp_wifi/include/esp_wifi_types_generic.h:320-349; v4.4.8
/// components/esp_wifi/include/esp_wifi_types.h:191-210), and nothing else, so connecting to it
/// fails as to a network out of reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconAp { pub ssid: String, pub bssid: [u8; 6], pub channel: u8, pub rssi: i8, pub secure: bool }

/// The most beacon-only networks an access point carries.
pub const BEACONS_MAX: usize = 32;

impl BeaconAp {
    /// `--wifi-nearby`'s `key=value,...`: `ssid` (1 to 32 bytes, required), `chan` 1 through 14
    /// (default 1), `rssi` in dBm (default -60), `secure` 0 or 1 (default 1, WPA2-PSK in the scan),
    /// `bssid` (default: a locally administered address derived from the SSID, the same every run).
    pub fn parse(spec: &str) -> Result<BeaconAp, String> {
        let mut b = BeaconAp { ssid: String::new(), bssid: [0; 6], channel: 1, rssi: -60, secure: true };
        let mut bssid = None;
        for entry in spec.split(',') {
            let (key, value) = entry.split_once('=').ok_or_else(|| format!("invalid nearby-network option '{entry}': expected key=value"))?;
            match key {
                "ssid" => b.ssid = value.to_string(),
                "chan" | "channel" | "ch" => b.channel = value.parse::<u8>().ok().filter(|n| (1..=14).contains(n))
                    .ok_or_else(|| format!("invalid nearby-network channel '{value}': expected 1 through 14"))?,
                "rssi" => b.rssi = value.parse::<i8>().map_err(|_| format!("invalid nearby-network rssi '{value}': expected -128 through 127 dBm"))?,
                "secure" => b.secure = match value { "1" | "true" | "yes" => true, "0" | "false" | "no" => false,
                    _ => return Err(format!("invalid nearby-network secure '{value}': expected 0 or 1")) },
                "bssid" => bssid = Some(parse_bssid(value)?),
                _ => return Err(format!("unknown nearby-network option '{key}'")),
            }
        }
        if !(1..=32).contains(&b.ssid.len()) { return Err("a nearby network needs an ssid of 1 to 32 bytes".into()); }
        b.bssid = bssid.unwrap_or_else(|| {
            // FNV-1a over the SSID, in a locally administered unicast address (bit 1 of the first octet)
            let h = b.ssid.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, c| (h ^ u64::from(c)).wrapping_mul(0x100_0000_01b3));
            let x = h.to_le_bytes();
            [0x02, x[0], x[1], x[2], x[3], x[4]]
        });
        Ok(b)
    }

    /// Every `--wifi-nearby` spec, at most `BEACONS_MAX` of them, each on a BSSID of its own (two
    /// specs with one SSID and no `bssid` derive the same one).
    pub fn parse_all(specs: &[String]) -> Result<Vec<BeaconAp>, String> {
        if specs.len() > BEACONS_MAX { return Err(format!("more than {BEACONS_MAX} nearby networks")); }
        let list = specs.iter().map(|s| Self::parse(s)).collect::<Result<Vec<_>, _>>()?;
        for (i, b) in list.iter().enumerate() {
            if let Some(o) = list[..i].iter().find(|o| o.bssid == b.bssid) {
                return Err(format!("nearby networks '{}' and '{}' share the BSSID {}", o.ssid, b.ssid, mac_str(&b.bssid)));
            }
        }
        Ok(list)
    }
}

pub struct VirtualAp {
    pub cfg: ApConfig,
    pub wpa: Wpa,
    pub state: StaState,
    pub sta: [u8; 6],
    pub aid: u16,
    pub next_beacon_us: u64,
    pub beacon_interval_us: u64,
    pub seq: u16,
    pub queue: Vec<AirFrame>,
    pub log: bool,
    pub stats: (u64, u64, u64),   // beacons, probe responses, data frames from the station
    pub pn: u64,                  // CCMP packet number for frames we send
    /// The networks around, beacon-only and not joinable (`set_beacons`).
    pub beacons: Vec<BeaconAp>,
}

impl VirtualAp {
    pub fn new(cfg: ApConfig, log: bool) -> Self {
        let mut wpa = Wpa::default();
        if let Some(psk) = &cfg.psk {
            wpa.pmk.copy_from_slice(&esp_periph::crypto::pbkdf2_sha1(psk.as_bytes(), cfg.ssid.as_bytes(), 4096, 32));
            // deterministic nonces/GTK: the emulator must replay identically run to run
            let seed = esp_periph::crypto::sha1(&[&wpa.pmk[..], &cfg.bssid[..]].concat());
            for i in 0..32 { wpa.anonce[i] = seed[i % 20] ^ (i as u8); }
            for i in 0..16 { wpa.gtk[i] = seed[(i + 3) % 20] ^ 0x5a; }
        }
        VirtualAp { cfg, wpa, state: StaState::Idle, sta: [0; 6], aid: 1, next_beacon_us: 100_000, beacon_interval_us: 102_400, seq: 0,
                    queue: Vec::new(), pn: 0, log, stats: (0, 0, 0), beacons: Vec::new() }
    }
    fn hdr(&mut self, fc: u16, a1: &[u8; 6], a3: &[u8; 6]) -> Vec<u8> {
        let mut f = Vec::with_capacity(128);
        f.extend_from_slice(&fc.to_le_bytes()); f.extend_from_slice(&[0, 0]);   // fc, duration
        f.extend_from_slice(a1); f.extend_from_slice(&self.cfg.bssid); f.extend_from_slice(a3);
        f.extend_from_slice(&(self.seq << 4).to_le_bytes()); self.seq = self.seq.wrapping_add(1);
        f
    }
    fn capability(&self) -> u16 { 0x0001 | if self.cfg.psk.is_some() { 0x0010 } else { 0 } | 0x0400 }   // ESS, privacy, short slot
    fn common_ies(&self, f: &mut Vec<u8>) {
        f.push(0); f.push(self.cfg.ssid.len() as u8); f.extend_from_slice(self.cfg.ssid.as_bytes());
        f.extend_from_slice(&[1, 8, 0x82, 0x84, 0x8b, 0x96, 0x0c, 0x12, 0x18, 0x24]);      // supported rates 1 2 5.5 11 (basic) 6 9 12 18
        f.extend_from_slice(&[3, 1, self.cfg.channel]);                                     // DS parameter set
        f.extend_from_slice(&[5, 4, 0, 1, 0, 0]);                                           // TIM
        f.extend_from_slice(&[7, 6, b'S', b'E', b' ', 1, 13, 20]);                          // country
        f.extend_from_slice(&[50, 4, 0x30, 0x48, 0x60, 0x6c]);                              // extended rates 24 36 48 54
        if self.cfg.psk.is_some() { f.extend_from_slice(RSN_IE); }                          // RSN: WPA2-PSK, CCMP
    }
    fn beacon_like(&mut self, subtype: u16, dst: &[u8; 6], now_us: u64) -> Vec<u8> {
        let bssid = self.cfg.bssid;
        let mut f = self.hdr(subtype << 4, dst, &bssid);
        f.extend_from_slice(&now_us.to_le_bytes());                                         // timestamp
        f.extend_from_slice(&100u16.to_le_bytes());                                          // beacon interval (TU)
        f.extend_from_slice(&self.capability().to_le_bytes());
        self.common_ies(&mut f);
        f
    }
    /// A probe response from a network around, as its own BSSID: the fixed fields (timestamp,
    /// beacon interval, capability: ESS, Privacy when secured, Short Slot Time; IEEE 802.11-2020
    /// 9.3.3.10 and 9.4.1.4) and the SSID, Supported Rates, DSSS Parameter Set (its channel),
    /// Extended Supported Rates and, when secured, RSN elements (element ids 0, 1, 3, 50 and 48,
    /// IEEE 802.11-2020 Table 9-92); the rates are this access point's.
    fn beacon_ap_response(&mut self, b: &BeaconAp, dst: &[u8; 6], now_us: u64) -> Vec<u8> {
        let mut f = Vec::with_capacity(96);
        f.extend_from_slice(&(5u16 << 4).to_le_bytes()); f.extend_from_slice(&[0, 0]);
        f.extend_from_slice(dst); f.extend_from_slice(&b.bssid); f.extend_from_slice(&b.bssid);
        f.extend_from_slice(&(self.seq << 4).to_le_bytes()); self.seq = self.seq.wrapping_add(1);
        f.extend_from_slice(&now_us.to_le_bytes());
        f.extend_from_slice(&100u16.to_le_bytes());
        let cap: u16 = 0x0001 | if b.secure { 0x0010 } else { 0 } | 0x0400;
        f.extend_from_slice(&cap.to_le_bytes());
        f.push(0); f.push(b.ssid.len() as u8); f.extend_from_slice(b.ssid.as_bytes());
        f.extend_from_slice(&[1, 8, 0x82, 0x84, 0x8b, 0x96, 0x0c, 0x12, 0x18, 0x24]);
        f.extend_from_slice(&[3, 1, b.channel]);
        f.extend_from_slice(&[50, 4, 0x30, 0x48, 0x60, 0x6c]);
        if b.secure { f.extend_from_slice(RSN_IE); }
        f
    }
    /// The channel and signal (dBm) a frame from the air arrives with, for the chip's RX
    /// descriptor (`rssi` and `channel` of `wifi_pkt_rx_ctrl_t`, IDF v5.5.5
    /// components/esp_wifi/include/local/esp_wifi_types_native.h:36 and :59-60; v4.4.8
    /// components/esp_wifi/include/esp_wifi_types.h:387 and :408-409): a network around's own for
    /// a frame from its BSSID, this access point's channel and -40 dBm otherwise.
    pub fn rx_meta(&self, frame: &[u8]) -> (u8, i8) {
        if frame.len() >= 16 {
            if let Some(b) = self.beacons.iter().find(|b| frame[10..16] == b.bssid) { return (b.channel, b.rssi); }
        }
        (self.cfg.channel, -40)
    }
    fn send(&mut self, at_us: u64, frame: Vec<u8>) {
        if self.log { let d = describe(&frame); if d.contains("auth") || d.contains("assoc") || d.contains("888e") { eprintln!("[wifi] AP -> {}  hex={:02x?}", d, frame); } else { eprintln!("[wifi] AP -> {} (t+{} us)", d, at_us); } }
        self.queue.push(AirFrame { at_us, frame });
    }
    /// True when `step(now_us)` would return nothing and change nothing: no frame queued and the
    /// next beacon not yet due. Keep this next to `step`; it must follow every timer `step` has.
    #[inline]
    pub fn idle_at(&self, now_us: u64) -> bool { self.queue.is_empty() && now_us < self.next_beacon_us }
    /// Time-driven behaviour (beacons). Returns frames due at or before `now_us`.
    pub fn step(&mut self, now_us: u64) -> Vec<AirFrame> {
        let mgmt_pending = self.queue.iter().any(|a| !is_beacon(&a.frame));
        if now_us >= self.next_beacon_us && !mgmt_pending {
            self.next_beacon_us += self.beacon_interval_us;
            if self.next_beacon_us <= now_us { self.next_beacon_us = now_us + self.beacon_interval_us; }
            let b = self.beacon_like(8, &[0xff; 6], now_us); self.stats.0 += 1;
            self.queue.push(AirFrame { at_us: now_us, frame: b });
        }
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.queue).into_iter().partition(|a| a.at_us <= now_us);
        self.queue = later;
        due
    }
    /// The station transmitted `f` at `now_us`. Returns data frames (as 802.11) for the network backend.
    pub fn on_station_tx(&mut self, f: &[u8], now_us: u64) -> Option<Vec<u8>> {
        if f.len() < 24 { return None; }
        let fc = u16::from_le_bytes([f[0], f[1]]);
        let (ty, st) = ((fc >> 2) & 3, (fc >> 4) & 0xf);
        let mut a2 = [0u8; 6]; a2.copy_from_slice(&f[10..16]);
        let to_us = |a1: &[u8]| a1 == [0xff; 6] || a1 == self.cfg.bssid;
        match (ty, st) {
            (0, 4) => {                                                                      // probe request: for us or wildcard?
                let ssid_ok = ies(f, 24).iter().any(|(id, d)| *id == 0 && (d.is_empty() || *d == self.cfg.ssid.as_bytes()));
                if ssid_ok && to_us(&f[4..10]) { let r = self.beacon_like(5, &a2, now_us); self.stats.1 += 1; self.send(now_us + 1500, r); }
                // the networks around answer a wildcard probe, or one for their SSID, sent to the
                // broadcast address or their own with a wildcard BSSID or their own (IEEE
                // 802.11-2020 11.1.4.3.4)
                let asked: Vec<Vec<u8>> = ies(f, 24).into_iter().filter(|(id, _)| *id == 0).map(|(_, d)| d.to_vec()).collect();
                let addressed = |b: &BeaconAp| [&f[4..10], &f[16..22]].iter().all(|a| *a == [0xff; 6] || *a == b.bssid);
                let around: Vec<BeaconAp> = self.beacons.iter()
                    .filter(|b| addressed(b) && asked.iter().any(|s| s.is_empty() || *s == b.ssid.as_bytes())).cloned().collect();
                for (i, b) in around.iter().enumerate() {
                    let r = self.beacon_ap_response(b, &a2, now_us);
                    self.send(now_us + 1900 + 400 * i as u64, r);
                }
            }
            (0, 11) if f.len() >= 30 && to_us(&f[4..10]) => {                                // authentication (open system)
                let (alg, seq) = (u16::from_le_bytes([f[24], f[25]]), u16::from_le_bytes([f[26], f[27]]));
                if self.log { eprintln!("[wifi] station AUTH req alg={} seq={} status={} hex={:02x?}", alg, seq, u16::from_le_bytes([f[28],f[29]]), f); }
                if alg == 0 && seq == 1 {
                    self.sta = a2; self.state = StaState::Authenticated;
                    let mut r = self.hdr(11 << 4, &a2, &self.cfg.bssid.clone());
                    r.extend_from_slice(&[0, 0, 2, 0, 0, 0]);                                // open, seq 2, status success
                    self.send(now_us + 300, r);
                }
            }
            (0, 0) | (0, 2) if to_us(&f[4..10]) && self.state != StaState::Idle => {         // (re)association request
                self.state = StaState::Associated;
                let mut r = self.hdr(1 << 4, &a2, &self.cfg.bssid.clone());
                r.extend_from_slice(&self.capability().to_le_bytes()); r.extend_from_slice(&[0, 0]); r.extend_from_slice(&(0xc000 | self.aid).to_le_bytes());
                r.extend_from_slice(&[1, 8, 0x82, 0x84, 0x8b, 0x96, 0x0c, 0x12, 0x18, 0x24]); r.extend_from_slice(&[50, 4, 0x30, 0x48, 0x60, 0x6c]);
                self.send(now_us + 300, r);
                if self.cfg.psk.is_some() {                       // WPA2: start the four-way handshake
                    self.wpa.replay += 1;
                    let m1 = self.eapol(0x008a, self.wpa.anonce, &[], None);
                    self.send(now_us + 30_000, m1);
                    self.wpa.state = WpaState::AwaitingMessage2;
                }
            }
            (0, 12) | (0, 10) if to_us(&f[4..10]) => { self.state = StaState::Idle; }
            (2, _) if self.state == StaState::Associated && f[4..10] == self.cfg.bssid => {  // data to the DS
                if st == 4 || st == 12 { return None; }                                       // null frames (power save)
                let hdr = if st & 8 != 0 { 26 } else { 24 };
                // Protected frame: the MAC would have encrypted in place, so the descriptor holds
                // plaintext framed by an 8-byte CCMP header and 8 bytes of MIC space. Take those off.
                let plain;
                let f: &[u8] = if fc & 0x4000 != 0 && f.len() > hdr + 16 {
                    let mut v = Vec::with_capacity(f.len() - 16);
                    v.extend_from_slice(&f[..hdr]);
                    v.extend_from_slice(&f[hdr + 8..f.len() - 8]);
                    v[1] &= !0x40;                                                             // clear the protected bit
                    plain = v; &plain
                } else { f };
                if f.len() > hdr + 8 && f[hdr] == 0xaa && f[hdr + 6] == 0x88 && f[hdr + 7] == 0x8e {
                    self.on_eapol(&f[hdr + 8..], now_us);
                    return None;
                }
                self.stats.2 += 1; return Some(f.to_vec());
            }
            _ => {}
        }
        None
    }
    /// Build an EAPOL-Key frame (802.1X over LLC/SNAP in an 802.11 data frame from the DS).
    fn eapol(&mut self, key_info: u16, nonce: [u8; 32], key_data: &[u8], mic_key: Option<&[u8]>) -> Vec<u8> {
        let mut body = Vec::with_capacity(99 + key_data.len());
        body.push(2);                                                    // 802.1X-2004
        body.push(3);                                                    // EAPOL-Key
        body.extend_from_slice(&((95 + key_data.len()) as u16).to_be_bytes());
        body.push(2);                                                    // RSN key descriptor
        body.extend_from_slice(&key_info.to_be_bytes());
        body.extend_from_slice(&16u16.to_be_bytes());                    // key length (CCMP)
        body.extend_from_slice(&self.wpa.replay.to_be_bytes());
        body.extend_from_slice(&nonce);
        body.extend_from_slice(&[0u8; 16]);                              // key IV
        body.extend_from_slice(&[0u8; 8]);                               // key RSC
        body.extend_from_slice(&[0u8; 8]);                               // key ID
        let mic_at = body.len();
        body.extend_from_slice(&[0u8; 16]);
        body.extend_from_slice(&(key_data.len() as u16).to_be_bytes());
        body.extend_from_slice(key_data);
        if let Some(kck) = mic_key {
            let m = esp_periph::crypto::hmac_sha1(kck, &body);
            body[mic_at..mic_at + 16].copy_from_slice(&m[..16]);
        }
        let sta = self.sta; let bssid = self.cfg.bssid;
        let mut f = self.hdr(0x0208, &sta, &bssid);                      // data, from-DS
        f[16..22].copy_from_slice(&bssid);
        f.extend_from_slice(&[0xaa, 0xaa, 0x03, 0, 0, 0, 0x88, 0x8e]);
        f.extend_from_slice(&body);
        f
    }

    /// Handle an EAPOL-Key frame from the station (messages 2 and 4 of the handshake).
    fn on_eapol(&mut self, body: &[u8], now_us: u64) {
        if body.len() < 99 || body[1] != 3 { return; }
        // the MIC covers exactly the 802.1X frame; the 802.11 payload can carry trailing bytes
        let n = 4 + u16::from_be_bytes([body[2], body[3]]) as usize;
        let Some(body) = body.get(..n).filter(|b| b.len() >= 99) else { return; };
        let key_data_len = u16::from_be_bytes([body[97], body[98]]) as usize;
        if 99 + key_data_len != body.len() { return; }
        let key_info = u16::from_be_bytes([body[5], body[6]]);
        let has_mic = key_info & 0x0100 != 0;
        let secure = key_info & 0x0200 != 0;
        if !has_mic { return; }
        if !secure && self.wpa.state == WpaState::AwaitingMessage2 {
            // message 2: take the SNonce and derive the pairwise key
            let mut snonce = [0u8; 32]; snonce.copy_from_slice(&body[17..49]);
            let (aa, spa) = (self.cfg.bssid, self.sta);
            let (lo_mac, hi_mac) = if aa <= spa { (aa, spa) } else { (spa, aa) };
            let (an, sn) = (self.wpa.anonce, snonce);
            let (lo_n, hi_n) = if an <= sn { (an, sn) } else { (sn, an) };
            let mut data = Vec::with_capacity(76);
            data.extend_from_slice(&lo_mac); data.extend_from_slice(&hi_mac);
            data.extend_from_slice(&lo_n); data.extend_from_slice(&hi_n);
            let ptk = esp_periph::crypto::prf(&self.wpa.pmk, "Pairwise key expansion", &data, 384);
            // self-check: recompute the station's own MIC over message 2. If this matches, the PMK,
            // the PTK derivation and the MIC scope are all right and any later failure is elsewhere.
            {
                let mut probe = body.to_vec();
                let mic_at = 81;
                let mut recv = [0u8; 16]; recv.copy_from_slice(&probe[mic_at..mic_at + 16]);
                for b in probe[mic_at..mic_at + 16].iter_mut() { *b = 0; }
                let calc = esp_periph::crypto::hmac_sha1(&ptk[0..16], &probe);
                if self.log {
                    eprintln!("[wifi] WPA2 msg2: PTK derived, station MIC {} (recv {:02x?} calc {:02x?})",
                              if calc[..16] == recv { "VERIFIED" } else { "MISMATCH" }, &recv[..4], &calc[..4]);
                }
            }

            // message 3: RSN IE + the group key, wrapped with the KEK
            let mut kd = Vec::new();
            kd.extend_from_slice(RSN_IE);
            kd.extend_from_slice(&[0xdd, 22, 0x00, 0x0f, 0xac, 0x01, 0x01, 0x00]);   // GTK KDE, key id 1
            kd.extend_from_slice(&self.wpa.gtk);
            if kd.len() % 8 != 0 { kd.push(0xdd); while kd.len() % 8 != 0 { kd.push(0); } }   // pad: one 0xDD then zeros
            let mut kek = [0u8; 16]; kek.copy_from_slice(&ptk[16..32]);
            let wrapped = esp_periph::crypto::aes_key_wrap(&kek, &kd);
            self.wpa.replay += 1;
            let anonce = self.wpa.anonce;
            let m3 = self.eapol(0x13ca, anonce, &wrapped, Some(&ptk[..16]));
            self.send(now_us + 2_000, m3);
            self.wpa.state = WpaState::AwaitingMessage4;
        } else if secure && self.wpa.state == WpaState::AwaitingMessage4 {
            self.wpa.state = WpaState::Installed;
            if self.log { eprintln!("[wifi] WPA2 four-way handshake complete"); }
        }
    }

    /// Wrap an Ethernet frame from the network backend into an 802.11 data frame from the DS.
    pub fn data_from_ds(&mut self, eth: &[u8]) -> Option<Vec<u8>> {
        if self.state != StaState::Associated || eth.len() < 14 { return None; }
        let mut dst = [0u8; 6]; dst.copy_from_slice(&eth[0..6]); let mut src = [0u8; 6]; src.copy_from_slice(&eth[6..12]);
        let bssid = self.cfg.bssid;
        // Once the keys are installed the frame must look encrypted: protected bit, CCMP header and
        // room for the MIC. The payload stays in the clear — as far as firmware is concerned the MAC
        // decrypted it in place.
        let protected = if self.wpa.state == WpaState::Installed { 0x4000 } else { 0 };
        let ccmp_hdr = protected != 0;
        let mut f = self.hdr((2 << 2) | 0x0200 | protected, &dst, &src);                        // data, from-DS
        f[16..22].copy_from_slice(&src); f[10..16].copy_from_slice(&bssid);
        if ccmp_hdr {
            // CCMP header, as the hardware would leave it after decrypting in place
            self.pn += 1;
            let pn = self.pn;
            let keyid = if dst[0] & 1 != 0 { 1 } else { 0 };                                    // group frames use the GTK
            f.extend_from_slice(&[pn as u8, (pn >> 8) as u8, 0, 0x20 | (keyid << 6),
                                  (pn >> 16) as u8, (pn >> 24) as u8, (pn >> 32) as u8, (pn >> 40) as u8]);
        }
        f.extend_from_slice(&[0xaa, 0xaa, 0x03, 0, 0, 0]); f.extend_from_slice(&eth[12..14]);  // LLC/SNAP
        f.extend_from_slice(&eth[14..]);
        if ccmp_hdr { f.extend_from_slice(&[0u8; 8]); }                                         // MIC space
        Some(f)
    }
}

/// 802.11 data frame (to the DS) -> Ethernet frame.
pub fn data_to_eth(f: &[u8]) -> Option<Vec<u8>> {
    if f.len() < 2 { return None; }
    let fc = u16::from_le_bytes([f[0], f[1]]); let st = (fc >> 4) & 0xf;
    let hdr = if st & 8 != 0 { 26 } else { 24 };
    if f.len() < hdr + 8 || f[hdr] != 0xaa { return None; }
    let mut e = Vec::with_capacity(f.len());
    e.extend_from_slice(&f[16..22]);   // dst = addr3
    e.extend_from_slice(&f[10..16]);   // src = addr2
    e.extend_from_slice(&f[hdr + 6..hdr + 8]);
    e.extend_from_slice(&f[hdr + 8..]);
    Some(e)
}

/// FCS (CRC-32) as the MAC appends it.
pub fn fcs(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data { crc ^= b as u32; for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 }; } }
    !crc
}

/// The station's side of the air, the same behind every chip's MAC: the access point it hears, the
/// network behind that access point (or the host's Ethernet relay instead), the pacing of received
/// frames and the counts for the end-of-run report. Each chip's bus moves frames between this and
/// its own descriptors; the descriptor and receive-header layouts are the chip's. The state is
/// private: the bus attaches the access point and network through `SocBus::attach_wifi`, which
/// also refreshes the chip's own scheduling of WiFi work.
#[derive(Default)]
pub struct StationLink {
    ap: Option<VirtualAp>,
    net: Option<crate::net::VirtualNet>,
    /// the host relays raw Ethernet (`set_ethernet_relay`) instead of the virtual network answering
    relay: bool,
    /// Ethernet frames the station sent, for the network or the relay
    eth_tx: Vec<Vec<u8>>,
    /// Ethernet frames for the station, from the network or the relay
    eth_rx: Vec<Vec<u8>>,
    /// when the last frame went into the receive ring
    last_rx_us: u64,
    /// the descriptor that frame went into, 0 before the first one
    last_rx_desc: u32,
    /// when the network last read the host's sockets
    net_polled_us: u64,
    /// frames the station sent and received
    tx_frames: u64,
    rx_frames: u64,
    /// received frames with no usable descriptor
    rx_dropped: u64,
    /// frames on the air before the station's receive ring was set up
    rx_unarmed: u64,
    /// relayed frames that were too long or found the relay queue full
    tx_dropped: u64,
}

/// A receive descriptor's `has_data` bit (word 0 bit 30), the same on the S3, C3 and C6 MACs: the
/// hardware filled it and software has not recycled it yet.
pub const RX_DESC_HAS_DATA: u32 = 1 << 30;
/// The longest Ethernet frame the relay takes or gives, without FCS, and the relay queues' depth.
const RELAY_FRAME_MAX: usize = 1518;
const RELAY_QUEUE: usize = 64;
/// Two received frames are never closer than a frame's airtime.
const RX_GAP_US: u64 = 400;
/// How long the air waits for software to recycle the last descriptor before it delivers anyway.
const RX_STALL_US: u64 = 50_000;
/// Frames from the station are handled the moment they are sent, but reading the host sockets
/// means syscalls: doing that every scheduling round costs more than emulating the CPU. This is
/// well under any timeout the guest's TCP stack cares about.
const NET_POLL_US: u64 = 500;

impl StationLink {
    /// Put an access point and the network behind it on the air, replacing any there were. The
    /// queues, pacing and counts start again; the relay mode stays.
    pub fn attach(&mut self, ap: Option<VirtualAp>, net: Option<crate::net::VirtualNet>) {
        *self = StationLink { ap, net, relay: self.relay, ..Default::default() };
    }
    /// `attach` with a fresh access point for `cfg` and network, logging as `debug` says.
    pub fn attach_new(&mut self, cfg: ApConfig, nat: Option<crate::nat::Nat>, debug: &crate::DebugFlags) {
        let mut net = crate::net::VirtualNet::new(debug.has("net"));
        net.nat = nat;
        net.epoch_us = cfg.epoch_s.map(|s| s.saturating_mul(1_000_000));
        self.attach(Some(VirtualAp::new(cfg, debug.has("wifi-frames"))), Some(net));
    }
    /// What a guest reboot keeps: the host's access point, network and relay mode. The queues, the
    /// pacing and the counts start again.
    pub fn surviving_reboot(self) -> Self { StationLink { ap: self.ap, net: self.net, relay: self.relay, ..Default::default() } }

    pub fn ap(&self) -> Option<&VirtualAp> { self.ap.as_ref() }
    pub fn ap_mut(&mut self) -> Option<&mut VirtualAp> { self.ap.as_mut() }
    pub fn net(&self) -> Option<&crate::net::VirtualNet> { self.net.as_ref() }
    pub fn net_mut(&mut self) -> Option<&mut crate::net::VirtualNet> { self.net.as_mut() }
    pub fn relay(&self) -> bool { self.relay }
    /// The descriptor the last received frame went into, 0 before the first one.
    pub fn last_rx_desc(&self) -> u32 { self.last_rx_desc }
    /// Ethernet frames the station sent that the network or the relay has not taken yet.
    pub fn eth_tx_pending(&self) -> usize { self.eth_tx.len() }
    pub fn tx_frames(&self) -> u64 { self.tx_frames }
    pub fn rx_frames(&self) -> u64 { self.rx_frames }
    pub fn rx_dropped(&self) -> u64 { self.rx_dropped }
    pub fn rx_unarmed(&self) -> u64 { self.rx_unarmed }
    pub fn tx_dropped(&self) -> u64 { self.tx_dropped }
    /// The networks around (beacon-only, not joinable), replacing the ones there were; `None`
    /// without an access point. A network with the access point's own SSID or BSSID would be that
    /// access point, so it is left out: the ones left out come back with the reason.
    pub fn set_beacons(&mut self, beacons: Vec<BeaconAp>) -> Option<Vec<(BeaconAp, &'static str)>> {
        let ap = self.ap.as_mut()?;
        let mut left_out = Vec::new();
        ap.beacons = beacons.into_iter().filter_map(|b| {
            let why = if b.ssid == ap.cfg.ssid { "the access point's SSID" } else if b.bssid == ap.cfg.bssid { "the access point's BSSID" } else { return Some(b) };
            left_out.push((b, why));
            None
        }).collect();
        Some(left_out)
    }

    /// The station sent `frame` at `now_us`: the access point hears it, and what it passes on as
    /// data goes to the network or the relay.
    pub fn station_tx(&mut self, frame: &[u8], now_us: u64) {
        self.tx_frames += 1;
        let Some(ap) = &mut self.ap else { return };
        let Some(eth) = ap.on_station_tx(frame, now_us).and_then(|data| data_to_eth(&data)) else { return };
        if !self.relay || (eth.len() <= RELAY_FRAME_MAX && self.eth_tx.len() < RELAY_QUEUE) { self.eth_tx.push(eth); } else { self.tx_dropped += 1; }
    }

    /// True while the last received frame's airtime lasts: no frame is due, whatever the ring holds.
    pub fn in_rx_gap(&self, now_us: u64) -> bool { now_us.wrapping_sub(self.last_rx_us) < RX_GAP_US }

    /// True when `next_rx` has nothing to deliver at `now_us`, whatever the ring holds: within the
    /// airtime gap, or with the access point idle (`VirtualAp::idle_at`) and nothing from the
    /// network. `next_rx` would change nothing then, so skipping it is exact. The bus asks this
    /// every scheduling round while WiFi is on, before it reads the ring.
    #[inline]
    pub fn nothing_due(&self, now_us: u64) -> bool {
        self.in_rx_gap(now_us) || self.ap.as_ref().is_none_or(|ap| ap.idle_at(now_us) && self.eth_rx.is_empty())
    }

    /// The next frame for the station's receive ring, if one is due: the access point's beacons
    /// and responses, then what the network sends. `ring_busy` says the last delivered descriptor
    /// still has data. The S3's library was seen to indicate a frame up the 802.11 stack only
    /// while the descriptor ring is shallow: with several filled descriptors pending it switches
    /// to batch block-recycle and drops them. So the air waits until software has recycled the
    /// last one, which is what a real radio sees at low traffic, but not for ever: after
    /// `RX_STALL_US` the frame goes anyway, as a real ring would overflow. The C3 and C6 are
    /// paced the same way; their station goldens pass with it, but it was not measured on their
    /// silicon. Management responses (auth, assoc, probe) go before beacons, so a connect
    /// exchange is not crowded out by beacon traffic; the rest waits in the access point's queue.
    /// The caller delivers the frame and then reports `rx_delivered` or `drop_rx`.
    pub fn next_rx(&mut self, now_us: u64, ring_busy: bool) -> Option<Vec<u8>> {
        if self.in_rx_gap(now_us) { return None; }
        if ring_busy && now_us.wrapping_sub(self.last_rx_us) < RX_STALL_US { return None; }
        let ap = self.ap.as_mut()?;
        let mut due = ap.step(now_us);
        let eth_in = if self.relay {
            if due.is_empty() && !self.eth_rx.is_empty() { vec![self.eth_rx.remove(0)] } else { Vec::new() }
        } else { std::mem::take(&mut self.eth_rx) };
        for e in eth_in { if let Some(f) = ap.data_from_ds(&e) { due.push(AirFrame { at_us: now_us, frame: f }); } }
        if due.is_empty() { return None; }
        due.sort_by_key(|a| (is_beacon(&a.frame), a.at_us));
        let first = due.remove(0);
        ap.queue.extend(due);
        self.last_rx_us = now_us;
        Some(first.frame)
    }

    /// A frame went into the descriptor at `desc`.
    pub fn rx_delivered(&mut self, desc: u32) { self.last_rx_desc = desc; self.rx_frames += 1; }
    /// A frame found no usable descriptor.
    pub fn drop_rx(&mut self) { self.rx_dropped += 1; }
    /// A frame went by before the driver gave the MAC its receive ring.
    pub fn drop_rx_unarmed(&mut self) { self.rx_unarmed += 1; }

    /// The network behind the access point answers what the station sent at once and reads the
    /// host's sockets every `NET_POLL_US`. Nothing to do in relay mode: the host has the frames.
    pub fn net_step(&mut self, now_us: u64) {
        if self.relay { return; }
        let sta = self.ap.as_ref().map(|ap| ap.sta);
        let Some(net) = self.net.as_mut() else { return };
        if let Some(mac) = sta { if mac != [0; 6] { net.sta_mac = mac; } }
        let out = std::mem::take(&mut self.eth_tx);
        let due = now_us.wrapping_sub(self.net_polled_us) >= NET_POLL_US;
        if out.is_empty() && !due { return; }
        if due { self.net_polled_us = now_us; }
        for e in out { self.eth_rx.extend(net.handle(&e, now_us)); }
        self.eth_rx.extend(net.poll(now_us));
    }

    /// Switch the host's Ethernet relay on or off; a change empties both queues.
    pub fn set_relay(&mut self, enabled: bool) {
        if self.relay != enabled { self.eth_tx.clear(); self.eth_rx.clear(); self.relay = enabled; }
    }

    /// The frames the station sent since the last call, in relay mode.
    pub fn take_relay_frames(&mut self) -> Vec<Vec<u8>> {
        if self.relay { std::mem::take(&mut self.eth_tx) } else { Vec::new() }
    }

    /// A frame from the host for the station, in relay mode.
    pub fn receive_relay_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        if !self.relay || self.ap.is_none() { return Err("Ethernet relay requires relay mode and a virtual AP".into()); }
        if !(14..=RELAY_FRAME_MAX).contains(&frame.len()) { return Err(format!("Ethernet frame must be 14..={RELAY_FRAME_MAX} bytes without FCS")); }
        if self.eth_rx.len() >= RELAY_QUEUE { return Err("Ethernet receive queue full".into()); }
        self.eth_rx.push(frame.to_vec());
        Ok(())
    }

    /// The end-of-run lines, one per line without a final newline: what the station sent and
    /// received, what the access point saw, and what the network behind it answered. Empty when
    /// the station never used the radio and there is no network.
    pub fn report(&self) -> String {
        let mut lines = Vec::new();
        if self.tx_frames + self.rx_frames + self.rx_dropped + self.rx_unarmed > 0 {
            let unarmed = if self.rx_unarmed > 0 { format!(", {} before the receive ring was set up", self.rx_unarmed) } else { String::new() };
            lines.push(format!("[emu] wifi: {} frames sent by the station, {} received ({} dropped: no descriptor{unarmed}){}", self.tx_frames, self.rx_frames, self.rx_dropped, self.ap.as_ref().map_or(String::new(), |ap| format!("; AP: {} beacons, {} probe responses, {} data frames from the station, state {:?}", ap.stats.0, ap.stats.1, ap.stats.2, ap.state))));
        }
        if let Some(n) = &self.net { lines.push(format!("[emu] net: {} DHCP leases, {} ARP replies, {} DNS answers, {} NTP answers, {} TCP refused, {} pings, {} frames ignored", n.dhcp_acks, n.arp_replies, n.dns_answers, n.ntp_answers, n.tcp_rejects, n.pings, n.unhandled));
            if let Some(t) = &n.nat {
                let inbound = if t.hostfwd == 0 { String::new() } else { format!(", {} inbound, {} hostfwd", t.tcp_accepted, t.hostfwd) };
                lines.push(format!("[emu] nat: {} TCP connections ({} failed{}), {} UDP flows ({} evicted, {} send errors), {} bytes out, {} bytes in", t.tcp_opened, t.tcp_refused, inbound, t.udp_flows, t.udp_evicted, t.udp_send_errors, t.bytes_to_host, t.bytes_to_guest));
            } }
        lines.join("\n")
    }
}
