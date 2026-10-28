//! An opt-in virtual station joining the guest's SoftAP through actual 802.11 management/data frames.
use super::{ies, mac_str, AirFrame, RSN_IE};
use crate::json::Json;

pub const CLIENT_MAC: [u8; 6] = [0x02, 0x53, 0x49, 0x4d, 0x00, 0x03];
const RETRY_US: u64 = 1_000_000;
const FRAME_MAX: usize = 1518;
const QUEUE_MAX: usize = 64;

#[derive(Clone, Default)]
pub struct ClientConfig { pub ssid: Option<String>, pub password: Option<String> }

impl ClientConfig {
    /// JSON allows SSIDs/passphrases containing commas without ambiguous option-string escaping.
    pub fn parse(text: &str) -> Result<Option<Self>, String> {
        let Json::Obj(fields) = crate::json::parse_json(text)? else { return Err("SoftAP client configuration must be an object".into()) };
        let mut enabled = true;
        let mut cfg = Self::default();
        let mut seen = std::collections::HashSet::new();
        for (key, value) in fields {
            if !seen.insert(key.clone()) { return Err(format!("duplicate SoftAP client field: {key}")); }
            match (key.as_str(), value) {
                ("enabled", Json::Bool(v)) => enabled = v,
                ("ssid", Json::Str(v)) if (1..=32).contains(&v.len()) => cfg.ssid = Some(v),
                ("password", Json::Str(v)) if (8..=63).contains(&v.len()) => cfg.password = Some(v),
                _ => return Err(format!("invalid SoftAP client field: {key}")),
            }
        }
        Ok(enabled.then_some(cfg))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State { Discovering, Authenticating, Associating, Handshake, Connected, AuthRequired, Error }
impl State {
    fn name(self) -> &'static str { match self { Self::Discovering => "discovering", Self::Authenticating => "authenticating", Self::Associating => "associating", Self::Handshake => "handshake", Self::Connected => "connected", Self::AuthRequired => "auth-required", Self::Error => "error" } }
}

pub struct VirtualSta {
    pub cfg: ClientConfig,
    state: State,
    pub bssid: Option<[u8; 6]>,
    pub channel: u8,
    ssid: String,
    secure: bool,
    seq: u16,
    next_retry_us: u64,
    retries: u8,
    error: Option<String>,
    ptk: Vec<u8>,
    snonce: [u8; 32],
    replay: u64,
    pn: u64,
    eth_out: Vec<Vec<u8>>,
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub dropped: u64,
}

impl VirtualSta {
    pub fn new(cfg: ClientConfig) -> Self {
        Self { cfg, state: State::Discovering, bssid: None, channel: 1, ssid: String::new(), secure: false, seq: 0, next_retry_us: 0,
            retries: 0, error: None, ptk: Vec::new(), snonce: [0; 32], replay: 0, pn: 0, eth_out: Vec::new(), tx_frames: 0, rx_frames: 0, dropped: 0 }
    }
    pub fn connected(&self) -> bool { self.state == State::Connected }
    pub fn status_json(&self) -> String {
        let s = |s: &str| format!("\"{}\"", crate::web::json_escape(s));
        format!("{{\"enabled\":true,\"state\":{},\"ssid\":{},\"bssid\":{},\"clientMac\":{},\"channel\":{},\"security\":{},\"error\":{},\"txFrames\":{},\"rxFrames\":{},\"droppedFrames\":{}}}",
            s(self.state.name()), if self.ssid.is_empty() { "null".into() } else { s(&self.ssid) }, self.bssid.map_or("null".into(), |b| s(&mac_str(&b))),
            s(&mac_str(&CLIENT_MAC)), self.channel, s(if self.secure { "wpa2-psk" } else { "open" }), self.error.as_ref().map_or("null".into(), |e| s(e)), self.tx_frames, self.rx_frames, self.dropped)
    }
    fn header(&mut self, fc: u16, dst: [u8; 6], address3: [u8; 6]) -> Vec<u8> {
        let mut f = Vec::with_capacity(96);
        f.extend_from_slice(&fc.to_le_bytes()); f.extend_from_slice(&[0; 2]);
        f.extend_from_slice(&dst); f.extend_from_slice(&CLIENT_MAC); f.extend_from_slice(&address3);
        f.extend_from_slice(&(self.seq << 4).to_le_bytes()); self.seq = self.seq.wrapping_add(1);
        f
    }
    fn rates(f: &mut Vec<u8>) { f.extend_from_slice(&[1, 8, 0x82, 0x84, 0x8b, 0x96, 0x0c, 0x12, 0x18, 0x24, 50, 4, 0x30, 0x48, 0x60, 0x6c]); }
    fn request(&mut self) -> Option<Vec<u8>> {
        let f = match self.state {
            State::Discovering => {
                let mut f = self.header(0x0040, [0xff; 6], [0xff; 6]);
                let ssid = self.cfg.ssid.as_deref().unwrap_or("").as_bytes();
                f.extend_from_slice(&[0, ssid.len() as u8]); f.extend_from_slice(ssid); Self::rates(&mut f); f
            }
            State::Authenticating => {
                let ap = self.bssid?;
                let mut f = self.header(0x00b0, ap, ap); f.extend_from_slice(&[0, 0, 1, 0, 0, 0]); f
            }
            State::Associating => {
                let ap = self.bssid?;
                let mut f = self.header(0, ap, ap);
                // IEEE 802.11-2020: ESS, short slot, optional privacy; listen interval 10 beacons.
                let cap: u16 = 0x0401 | if self.secure { 0x10 } else { 0 };
                f.extend_from_slice(&cap.to_le_bytes()); f.extend_from_slice(&10u16.to_le_bytes());
                f.extend_from_slice(&[0, self.ssid.len() as u8]); f.extend_from_slice(self.ssid.as_bytes()); Self::rates(&mut f);
                if self.secure { f.extend_from_slice(RSN_IE); } f
            }
            _ => return None,
        };
        self.tx_frames += 1;
        Some(f)
    }
    fn transition(&mut self, state: State, now_us: u64) { self.state = state; self.retries = 0; self.next_retry_us = now_us; }
    fn fail(&mut self, message: &str) { self.state = State::Error; self.error = Some(message.into()); }
    /// Host-driven timers: called only by the enabled bridge, never from an inactive chip tick.
    pub fn poll(&mut self, now_us: u64) -> Vec<AirFrame> {
        if now_us < self.next_retry_us { return Vec::new(); }
        self.next_retry_us = now_us.saturating_add(RETRY_US);
        if matches!(self.state, State::Authenticating | State::Associating | State::Handshake) {
            self.retries += 1;
            if self.retries > 8 { self.fail("SoftAP did not complete authentication/association"); return Vec::new(); }
        }
        self.request().map(|frame| vec![AirFrame { at_us: now_us, frame }]).unwrap_or_default()
    }
    fn discover(&mut self, f: &[u8], now_us: u64) {
        if self.state != State::Discovering || f.len() < 36 || f[10] & 1 != 0 { return; }
        let fields = ies(f, 36);
        let Some((_, ssid)) = fields.iter().find(|(id, value)| *id == 0 && (1..=32).contains(&value.len())) else { return };
        if self.cfg.ssid.as_ref().is_some_and(|s| s.as_bytes() != *ssid) { return; }
        let Ok(ssid) = std::str::from_utf8(ssid) else { return };
        let secure = u16::from_le_bytes([f[34], f[35]]) & 0x10 != 0;
        self.ssid = ssid.into(); self.secure = secure;
        self.bssid = Some(f[10..16].try_into().unwrap());
        if let Some((_, channel)) = fields.iter().find(|(id, value)| *id == 3 && value.len() == 1 && (1..=14).contains(&value[0])) { self.channel = channel[0]; }
        if secure {
            // Support the same WPA2-PSK/CCMP suite as VirtualAp, not WPA3/SAE, WEP or enterprise.
            let rsn_ok = fields.iter().any(|(id, value)| *id == 48 && value.len() >= 18 && value[..18] == RSN_IE[2..20]);
            if !rsn_ok { self.fail("SoftAP security is not WPA2-PSK/CCMP"); return; }
            if self.cfg.password.is_none() { self.state = State::AuthRequired; self.error = Some("SoftAP requires its WPA2 passphrase".into()); return; }
        }
        self.transition(State::Authenticating, now_us);
    }
    /// An actual guest transmission. Returned frames go back through the chip's RX descriptors.
    pub fn on_guest_tx(&mut self, f: &[u8], now_us: u64) -> Vec<AirFrame> {
        if f.len() < 24 { return Vec::new(); }
        let fc = u16::from_le_bytes([f[0], f[1]]);
        let (ty, subtype) = ((fc >> 2) & 3, (fc >> 4) & 15);
        let to_client = f[4..10] == CLIENT_MAC || f[4] & 1 != 0;
        if ty == 0 && matches!(subtype, 5 | 8) && to_client { self.discover(f, now_us); return self.poll(now_us); }
        if self.bssid.is_none_or(|b| f[10..16] != b) || !to_client { return Vec::new(); }
        self.rx_frames += 1;
        match (ty, subtype) {
            (0, 11) if self.state == State::Authenticating && f.len() >= 30 => {
                if f[24..28] != [0, 0, 2, 0] { return Vec::new(); }
                if f[28..30] != [0, 0] { self.fail("SoftAP rejected open-system authentication"); }
                else { self.transition(State::Associating, now_us); }
            }
            (0, 1) if self.state == State::Associating && f.len() >= 30 => {
                if f[26..28] != [0, 0] { self.fail("SoftAP rejected association"); }
                else { self.transition(if self.secure { State::Handshake } else { State::Connected }, now_us); }
            }
            (0, 10 | 12) => {
                let cfg = self.cfg.clone(); *self = Self::new(cfg); self.next_retry_us = now_us.saturating_add(RETRY_US);
            }
            (2, st) if fc & 0x0300 == 0x0200 && !matches!(st, 4 | 12) => {
                let hdr = if st & 8 != 0 { 26 } else { 24 };
                let (start, end) = if fc & 0x4000 != 0 { (hdr + 8, f.len().saturating_sub(8)) } else { (hdr, f.len()) };
                if end < start + 8 || f.get(start..start + 6) != Some(&[0xaa, 0xaa, 3, 0, 0, 0]) { return Vec::new(); }
                let payload = &f[start..end];
                if payload[6..8] == [0x88, 0x8e] { return self.eapol(&payload[8..], now_us); }
                if !self.connected() { return Vec::new(); }
                let size = 14 + payload.len() - 8;
                if size > FRAME_MAX || self.eth_out.len() >= QUEUE_MAX { self.dropped += 1; return Vec::new(); }
                let mut eth = Vec::with_capacity(size);
                eth.extend_from_slice(&f[4..10]); eth.extend_from_slice(&f[16..22]); eth.extend_from_slice(&payload[6..]); self.eth_out.push(eth);
            }
            _ => {}
        }
        self.poll(now_us)
    }
    fn eapol_frame(&mut self, info: u16, nonce: [u8; 32], data: &[u8], now_us: u64) -> Vec<AirFrame> {
        let Some(ap) = self.bssid else { return Vec::new() };
        let mut body = vec![2, 3]; body.extend_from_slice(&((95 + data.len()) as u16).to_be_bytes());
        body.push(2); body.extend_from_slice(&info.to_be_bytes()); body.extend_from_slice(&16u16.to_be_bytes());
        body.extend_from_slice(&self.replay.to_be_bytes()); body.extend_from_slice(&nonce); body.extend_from_slice(&[0; 48]);
        body.extend_from_slice(&(data.len() as u16).to_be_bytes()); body.extend_from_slice(data);
        let mic = esp_periph::crypto::hmac_sha1(&self.ptk[..16], &body); body[81..97].copy_from_slice(&mic[..16]);
        let mut f = self.header(0x0108, ap, ap); f.extend_from_slice(&[0xaa, 0xaa, 3, 0, 0, 0, 0x88, 0x8e]); f.extend_from_slice(&body);
        self.tx_frames += 1;
        vec![AirFrame { at_us: now_us.saturating_add(300), frame: f }]
    }
    fn eapol(&mut self, bytes: &[u8], now_us: u64) -> Vec<AirFrame> {
        if !matches!(self.state, State::Handshake | State::Connected) || !self.secure || bytes.len() < 99 || bytes[1] != 3 || bytes[4] != 2 { return Vec::new(); }
        let n = 4 + usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
        let Some(body) = bytes.get(..n).filter(|b| b.len() >= 99) else { return Vec::new() };
        if 99 + usize::from(u16::from_be_bytes([body[97], body[98]])) != n { return Vec::new(); }
        let info = u16::from_be_bytes([body[5], body[6]]);
        let replay = u64::from_be_bytes(body[9..17].try_into().unwrap());
        if info & 0x008f != 0x008a || replay < self.replay { return Vec::new(); }
        if info & 0x0100 == 0 && self.state == State::Handshake {
            let (Some(pass), Some(ap)) = (&self.cfg.password, self.bssid) else { return Vec::new() };
            let pmk = esp_periph::crypto::pbkdf2_sha1(pass.as_bytes(), self.ssid.as_bytes(), 4096, 32);
            let anonce: [u8; 32] = body[17..49].try_into().unwrap();
            // Deterministic test peer, like VirtualAp: not a source of security-grade randomness.
            let seed = esp_periph::crypto::sha1(&[&pmk[..], &CLIENT_MAC[..], &anonce[..]].concat());
            for i in 0..32 { self.snonce[i] = seed[i % 20] ^ i as u8; }
            let (a, b) = if ap < CLIENT_MAC { (ap, CLIENT_MAC) } else { (CLIENT_MAC, ap) };
            let (x, y) = if anonce < self.snonce { (anonce, self.snonce) } else { (self.snonce, anonce) };
            self.ptk = esp_periph::crypto::prf(&pmk, "Pairwise key expansion", &[&a[..], &b[..], &x[..], &y[..]].concat(), 384);
            self.replay = replay;
            return self.eapol_frame(0x010a, self.snonce, RSN_IE, now_us);
        }
        if info & 0x0300 == 0x0300 && self.ptk.len() >= 16 {
            let mut check = body.to_vec(); check[81..97].fill(0);
            let mic = esp_periph::crypto::hmac_sha1(&self.ptk[..16], &check);
            if mic[..16] != body[81..97] { self.fail("SoftAP WPA2 key MIC mismatch (check passphrase)"); return Vec::new(); }
            self.replay = replay; self.transition(State::Connected, now_us);
            return self.eapol_frame(0x030a, [0; 32], &[], now_us);
        }
        Vec::new()
    }
    pub fn take_frames(&mut self) -> Vec<Vec<u8>> { std::mem::take(&mut self.eth_out) }
    pub fn receive(&mut self, eth: &[u8], now_us: u64) -> Result<AirFrame, String> {
        if !self.connected() { return Err("SoftAP virtual client is not associated".into()); }
        if !(14..=FRAME_MAX).contains(&eth.len()) { return Err("SoftAP Ethernet frame must be 14..=1518 bytes".into()); }
        if eth[6..12] != CLIENT_MAC { return Err("SoftAP Ethernet source must be the virtual client's MAC".into()); }
        let ap = self.bssid.ok_or("SoftAP has no BSSID")?;
        let protected = if self.secure { 0x4000 } else { 0 };
        let mut f = self.header(0x0108 | protected, ap, eth[..6].try_into().unwrap());
        if self.secure {
            self.pn = self.pn.wrapping_add(1); let pn = self.pn;
            f.extend_from_slice(&[pn as u8, (pn >> 8) as u8, 0, 0x20, (pn >> 16) as u8, (pn >> 24) as u8, (pn >> 32) as u8, (pn >> 40) as u8]);
        }
        f.extend_from_slice(&[0xaa, 0xaa, 3, 0, 0, 0]); f.extend_from_slice(&eth[12..]);
        if self.secure { f.extend_from_slice(&[0; 8]); }
        self.tx_frames += 1;
        Ok(AirFrame { at_us: now_us, frame: f })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wifi::{ApConfig, VirtualAp};

    fn exchange(password: Option<&str>, client_password: Option<&str>) -> (VirtualSta, VirtualAp) {
        let mut cfg = ApConfig::parse("ssid=guest,channel=3").unwrap(); cfg.psk = password.map(str::to_owned);
        let mut ap = VirtualAp::new(cfg, false);
        let mut sta = VirtualSta::new(ClientConfig { ssid: None, password: client_password.map(str::to_owned) });
        for now in (0..500_000).step_by(1_000) {
            for f in sta.poll(now) { ap.on_station_tx(&f.frame, now); }
            for f in ap.step(now) { for response in sta.on_guest_tx(&f.frame, now) { ap.on_station_tx(&response.frame, now); } }
        }
        (sta, ap)
    }
    #[test]
    fn open_peer_discovers_authenticates_and_bridges_both_directions() {
        let (mut sta, mut ap) = exchange(None, None);
        assert!(sta.connected()); assert_eq!(sta.ssid, "guest"); assert_eq!(sta.channel, 3);
        let mut eth = vec![0xff; 6]; eth.extend_from_slice(&CLIENT_MAC); eth.extend_from_slice(&[8, 0, 1, 2, 3]);
        let air = sta.receive(&eth, 1_000_000).unwrap();
        let data = ap.on_station_tx(&air.frame, 1_000_000).unwrap();
        assert_eq!(crate::wifi::data_to_eth(&data).unwrap(), eth);
        eth[..6].copy_from_slice(&CLIENT_MAC); eth[6..12].copy_from_slice(&ap.cfg.bssid);
        sta.on_guest_tx(&ap.data_from_ds(&eth).unwrap(), 1_001_000);
        assert_eq!(sta.take_frames(), vec![eth]); assert!(sta.take_frames().is_empty());
    }
    #[test]
    fn wpa2_verifies_mic_and_requires_correct_credentials() {
        let (mut sta, mut ap) = exchange(Some("test-password"), Some("test-password"));
        assert!(sta.connected(), "{}", sta.status_json());
        assert_eq!(ap.wpa.state, crate::wifi::WpaState::Installed);
        let mut eth = CLIENT_MAC.to_vec(); eth.extend_from_slice(&ap.cfg.bssid); eth.extend_from_slice(&[8, 0, 4, 5, 6]);
        let air = ap.data_from_ds(&eth).unwrap(); assert_ne!(air[1] & 0x40, 0);
        sta.on_guest_tx(&air, 1_000_000); assert_eq!(sta.take_frames(), vec![eth]);
        let (wrong, _) = exchange(Some("test-password"), Some("wrong-password"));
        assert!(!wrong.connected()); assert!(wrong.status_json().contains("MIC mismatch"));
        let (missing, _) = exchange(Some("test-password"), None);
        assert_eq!(missing.state.name(), "auth-required");
        assert!(!sta.status_json().contains("test-password"));
    }
    #[test]
    fn malformed_frames_and_bounded_ethernet() {
        let (mut sta, mut ap) = exchange(None, None);
        let mut eth = CLIENT_MAC.to_vec(); eth.extend_from_slice(&ap.cfg.bssid); eth.extend_from_slice(&[8, 0, 4, 5, 6]);
        let air = ap.data_from_ds(&eth).unwrap();
        for len in 0..32 { sta.on_guest_tx(&air[..len], 1_000_000); }
        assert!(sta.take_frames().is_empty());
        for _ in 0..(QUEUE_MAX + 10) { sta.on_guest_tx(&air, 1_000_000); }
        assert_eq!(sta.take_frames().len(), QUEUE_MAX); assert_eq!(sta.dropped, 10);
        assert!(sta.receive(&[0; 13], 0).is_err()); assert!(sta.receive(&[0; 1519], 0).is_err());
        assert!(sta.receive(&eth, 0).is_err(), "source MAC is the AP, not the client");
        eth[6..12].copy_from_slice(&CLIENT_MAC); assert!(sta.receive(&eth, 0).is_ok());
    }
    #[test]
    fn configuration_is_strict_and_disabling_drops_no_secrets_into_status() {
        assert!(ClientConfig::parse("{}").unwrap().is_some());
        assert!(ClientConfig::parse("{\"enabled\":false}").unwrap().is_none());
        for text in ["[]", "{\"unknown\":1}", "{\"password\":\"short\"}", "{\"ssid\":\"\"}", "{\"enabled\":1}", "{\"ssid\":\"a\",\"ssid\":\"b\"}"] { assert!(ClientConfig::parse(text).is_err(), "{text}"); }
        let cfg = ClientConfig::parse("{\"ssid\":\"a,b\",\"password\":\"comma,password\"}").unwrap().unwrap();
        assert_eq!(cfg.ssid.as_deref(), Some("a,b"));
        assert!(!VirtualSta::new(cfg).status_json().contains("comma,password"));
    }
    #[test]
    fn softap_link_is_opt_in_and_reboot_restarts_discovery() {
        let mut link = crate::wifi::StationLink::default();
        link.configure_softap(None).unwrap();
        assert!(link.configure_softap(Some(Default::default())).is_err());
        link.attach(Some(VirtualAp::new(ApConfig::parse("ssid=host").unwrap(), false)), None);
        let mut frame = vec![0; 30]; frame[10..16].copy_from_slice(&CLIENT_MAC);
        assert!(!link.rx_meta(&frame).2);
        link.configure_softap(Some(ClientConfig { ssid: Some("wanted".into()), password: None })).unwrap();
        link.poll_softap(1_000_000);
        assert!(link.rx_meta(&frame).2);
        assert_eq!(link.ap().unwrap().queue.len(), 1);
        let mut reset = link.surviving_reboot();
        assert_eq!(reset.softap.as_ref().unwrap().cfg.ssid.as_deref(), Some("wanted"));
        assert!(reset.softap_status().contains("discovering"));
        assert!(reset.ap().unwrap().queue.is_empty(), "no pre-reboot frames reach the new guest");
        reset.configure_softap(None).unwrap();
        assert!(!reset.rx_meta(&frame).2);
        assert!(reset.ap().unwrap().queue.is_empty());
        assert!(reset.receive_softap_frame(&[0; 14], 0).is_err());
        reset.poll_softap(2_000_000);
        assert!(reset.ap().unwrap().queue.is_empty());
    }
    #[test]
    fn multicast_and_guest_rejection_are_observed() {
        let (mut sta, mut ap) = exchange(None, None);
        let mut eth = vec![1, 0, 0x5e, 0, 0, 0xfb];
        eth.extend_from_slice(&ap.cfg.bssid); eth.extend_from_slice(&[8, 0, 1, 2]);
        sta.on_guest_tx(&ap.data_from_ds(&eth).unwrap(), 1_000_000);
        assert_eq!(sta.take_frames(), vec![eth]);
        let mut deauth = vec![0; 26]; deauth[0] = 0xc0; deauth[4..10].copy_from_slice(&CLIENT_MAC); deauth[10..16].copy_from_slice(&ap.cfg.bssid); deauth[24] = 3;
        sta.on_guest_tx(&deauth, 1_001_000);
        assert!(!sta.connected()); assert_eq!(sta.state.name(), "discovering");
        let mut wrong = VirtualSta::new(ClientConfig { ssid: Some("different".into()), password: None });
        wrong.on_guest_tx(&ap.beacon_like(8, &[255; 6], 1_002_000), 1_002_000);
        assert!(wrong.bssid.is_none());
    }

}
