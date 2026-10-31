//! Scriptable central and Battery Service peripheral on the packet link.
use super::*;

/// Iterate complete AD structures; zero padding or a truncated structure ends the list.
pub fn ad_structures(mut data: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    std::iter::from_fn(move || {
        let len = *data.first()? as usize;
        if len == 0 || len + 1 > data.len() { data = &[]; return None }
        let field = (data[1], &data[2..len + 1]);
        data = &data[len + 1..];
        Some(field)
    })
}

/// Decode the name and service UUID fields shared by HCI and radio observers.
pub fn advertising_fields(data: &[u8]) -> Vec<String> {
    let mut fields = Vec::new();
    for (kind, v) in ad_structures(data) {
        match kind {
            8 | 9 => fields.push(format!("name={:?}", String::from_utf8_lossy(v))),
            2 | 3 => { for u in v.as_chunks::<2>().0 { fields.push(format!("service={}", uuid(u))); } }
            6 | 7 => { for u in v.as_chunks::<16>().0 { fields.push(format!("service={}", uuid(u))); } }
            _ => {}
        }
    }
    fields
}

const ADV: &[u8] = b"\x02\x01\x06\x03\x03\x0f\x18\x09\x09esp32sim";

/// A Bluetooth device around the station (`--ble-nearby`): the guest's scans report it with its
/// address, advertising data, scan response and signal, as a scannable advertiser that does not
/// accept connections (ADV_SCAN_IND). `addr` is in HCI order (least significant byte first);
/// `addr_type` 0 is public, 1 random.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nearby { pub addr: [u8; 6], pub addr_type: u8, pub rssi: i8, pub adv: Vec<u8>, pub scan_response: Vec<u8> }

/// The most devices around the peer reports.
pub const NEARBY_MAX: usize = 32;

impl Nearby {
    /// `--ble-nearby`'s `key=value,...`: `name` (a Complete Local Name after the LE General
    /// Discoverable Flags, at most 26 bytes so both fit the 31-byte advertising data), `adv` and
    /// `rsp` (the advertising data and the scan response as hex, at most 31 bytes each; `adv`
    /// replaces the data `name` builds), `addr` (as printed, most significant byte first; default:
    /// a random static address derived from the advertising data, the same every run; a random
    /// address may not have the reserved top bits 10, Vol 6 Part B 1.3.2), `type` `random` (default)
    /// or `public`, `rssi` in dBm, -127 to +20 or 127 for "not available" as an LE Advertising
    /// Report carries it (Vol 4 Part E 7.7.65.2; default -60). A device needs `name` or `adv`, not
    /// both.
    pub fn parse(spec: &str) -> Result<Nearby, String> {
        let (mut name, mut adv, mut rsp, mut addr, mut public, mut rssi) = (None, None, Vec::new(), None, false, -60i8);
        let data = |key: &str, value: &str| hex_bytes(value).filter(|d| d.len() <= 31)
            .ok_or_else(|| format!("invalid nearby-device {key} '{value}': expected hex, at most 31 bytes"));
        for entry in spec.split(',') {
            let (key, value) = entry.split_once('=').ok_or_else(|| format!("invalid nearby-device option '{entry}': expected key=value"))?;
            match key {
                "name" => name = Some(value.to_string()),
                "adv" => adv = Some(data(key, value)?),
                "rsp" => rsp = data(key, value)?,
                "addr" => addr = Some(parse_addr(value).ok_or_else(|| format!("invalid nearby-device addr '{value}': expected six hex octets, aa:bb:cc:dd:ee:ff"))?),
                "type" => public = match value {
                    "public" => true,
                    "random" => false,
                    _ => return Err(format!("invalid nearby-device type '{value}': expected public or random")),
                },
                "rssi" => rssi = value.parse().ok().filter(|r: &i8| (-127..=20).contains(r) || *r == 127)
                    .ok_or_else(|| format!("invalid nearby-device rssi '{value}': expected -127 through 20 dBm, or 127 (not available)"))?,
                _ => return Err(format!("unknown nearby-device option '{key}'")),
            }
        }
        let adv = match (adv, name) {
            (Some(_), Some(_)) => return Err("a nearby device takes name or adv, not both".into()),
            (Some(adv), None) => adv,
            (None, Some(name)) if (1..=26).contains(&name.len()) => {
                let mut d = vec![2, 0x01, 0x06, name.len() as u8 + 1, 0x09];
                d.extend_from_slice(name.as_bytes());
                d
            }
            (None, Some(name)) => return Err(format!("invalid nearby-device name '{name}': 1 to 26 bytes fit the advertising data")),
            (None, None) => return Err("a nearby device needs a name or advertising data (adv=)".into()),
        };
        let addr = addr.unwrap_or_else(|| {
            // FNV-1a over the advertising data; a random static address has its top two bits set
            let h = adv.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &c| (h ^ u64::from(c)).wrapping_mul(0x100_0000_01b3)).to_le_bytes();
            [h[0], h[1], h[2], h[3], h[4], h[5] | 0xc0]
        });
        if !public && addr[5] >> 6 == 0b10 {
            return Err(format!("invalid nearby-device addr {}: a random address's top bits 10 are reserved", addr_text(&addr)));
        }
        Ok(Nearby { addr, addr_type: u8::from(!public), rssi, adv, scan_response: rsp })
    }

    /// Every `--ble-nearby` spec, at most `NEARBY_MAX`, each on an address of its own and none on
    /// the virtual peer's or the guest's: two advertisers on one address would answer as one.
    pub fn parse_all(specs: &[String]) -> Result<Vec<Nearby>, String> {
        if specs.len() > NEARBY_MAX { return Err(format!("more than {NEARBY_MAX} nearby devices")); }
        let list = specs.iter().map(|s| Self::parse(s)).collect::<Result<Vec<_>, _>>()?;
        for (i, n) in list.iter().enumerate() {
            if n.addr == super::PEER { return Err(format!("nearby device {} has the virtual peer's address", addr_text(&n.addr))); }
            if n.addr == super::ADDRESS { return Err(format!("nearby device {} has the guest's address", addr_text(&n.addr))); }
            if list[..i].iter().any(|o| o.addr == n.addr) { return Err(format!("two nearby devices share the address {}", addr_text(&n.addr))); }
        }
        Ok(list)
    }
}

/// An address as printed (most significant byte first) in HCI order (least significant first).
fn parse_addr(text: &str) -> Option<[u8; 6]> {
    let parts: Vec<_> = text.split(':').collect();
    if parts.len() != 6 || parts.iter().any(|p| p.len() != 2) { return None; }
    let mut a = [0u8; 6];
    for (b, p) in a.iter_mut().rev().zip(&parts) { *b = u8::from_str_radix(p, 16).ok()?; }
    Some(a)
}

/// An address as printed (most significant byte first), from its HCI order.
pub fn addr_text(a: &[u8; 6]) -> String { a.iter().rev().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":") }
pub fn hex(b: &[u8]) -> String { b.iter().map(|v| format!("{v:02x}")).collect() }
fn uuid(b: &[u8]) -> String {
    let s = hex(&b.iter().rev().copied().collect::<Vec<_>>());
    if b.len() == 16 {
        format!("{}-{}-{}-{}-{}", &s[..8], &s[8..12], &s[12..16], &s[16..20], &s[20..])
    } else { s }
}

#[derive(Clone, Copy, PartialEq)]
enum Request {
    Services,
    Characteristics,
    Descriptors,
    Read(u16),
    Write(u16),
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Connect,
    CentralStop,
    Disconnect,
    Discover,
    Read(u16),
    Write(u16, Vec<u8>),
    ReadUuid([u8; 16], [u8; 16]),
}
impl std::str::FromStr for Command {
    type Err = String;
    fn from_str(command: &str) -> Result<Self, String> {
        let parts: Vec<_> = command.split_whitespace().collect();
        match parts.as_slice() {
            ["connect"] => return Ok(Self::Connect),
            ["central-stop"] => return Ok(Self::CentralStop),
            ["disconnect"] => return Ok(Self::Disconnect),
            ["discover"] => return Ok(Self::Discover),
            ["read-uuid", service, characteristic] => return Ok(Self::ReadUuid(parse_uuid(service)?, parse_uuid(characteristic)?)),
            ["read" | "subscribe", _] | ["write", _, _] => {}
            _ => return Err("expected connect, central-stop, disconnect, discover, read HANDLE, read-uuid SERVICE CHARACTERISTIC, write HANDLE HEX, subscribe CCC_HANDLE".into()),
        }
        let handle =
            if let Some(h) = parts[1].strip_prefix("0x") { u16::from_str_radix(h, 16) } else { parts[1].parse() }
                .ok()
                .filter(|h| *h != 0)
                .ok_or("invalid BLE attribute handle")?;
        Ok(match parts[0] {
            "read" => Self::Read(handle),
            "subscribe" => Self::Write(handle, vec![1, 0]),
            _ => {
                let value = parts[2];
                let bytes = hex_bytes(value).filter(|b| b.len() <= 20)
                    .ok_or("BLE write requires at most 20 bytes of hexadecimal data")?;
                Self::Write(handle, bytes)
            }
        })
    }
}
fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) { return None }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}
fn parse_uuid(text: &str) -> Result<[u8; 16], String> {
    let digits: String = text.chars().filter(|c| *c != '-').collect();
    let mut bytes: [u8; 16] = hex_bytes(&digits).and_then(|b| b.try_into().ok())
        .ok_or("expected a 128-bit BLE UUID")?;
    bytes.reverse();
    Ok(bytes)
}
fn read_by_type(start: u16, end: u16) -> Vec<u8> {
    vec![8, start as u8, (start >> 8) as u8, end as u8, (end >> 8) as u8, 3, 0x28]
}

pub enum ReadStep { Request(Vec<u8>), Value(Vec<u8>) }
enum UuidStage { Service, Characteristic(u16, u16), Value }
/// ATT discovery and read, independent of HCI or radio transport; default MTU 23.
pub struct UuidRead {
    service: [u8; 16],
    characteristic: [u8; 16],
    stage: UuidStage,
}
impl UuidRead {
    pub fn new(service: [u8; 16], characteristic: [u8; 16]) -> Self {
        Self { service, characteristic, stage: UuidStage::Service }
    }
    pub fn request(&self) -> Vec<u8> {
        let mut p = vec![6, 1, 0, 255, 255, 0, 0x28];
        p.extend(self.service);
        p
    }
    pub fn receive(&mut self, p: &[u8]) -> Result<ReadStep, String> {
        if p.first() == Some(&1) { return Err(format!("ATT error {}", hex(p))) }
        match self.stage {
            UuidStage::Service if p.len() >= 5 && p[0] == 7 && (p.len() - 1).is_multiple_of(4) => {
                let start = word(&p[1..]); let end = word(&p[3..]);
                if start == 0 || end < start { return Err("invalid service range".into()) }
                self.stage = UuidStage::Characteristic(start, end);
                Ok(ReadStep::Request(read_by_type(start, end)))
            }
            UuidStage::Characteristic(start, end) if p.len() > 2 && p[0] == 9 && matches!(p[1], 7 | 21) && (p.len() - 2).is_multiple_of(p[1] as usize) => {
                let mut last = start - 1;
                for item in p[2..].chunks_exact(p[1] as usize) {
                    let declaration = word(item); let handle = word(&item[3..]);
                    if declaration <= last || declaration > end || handle <= declaration || handle > end {
                        return Err("invalid characteristic handles".into());
                    }
                    last = declaration;
                    if item[5..] == self.characteristic {
                        if item[2] & 2 == 0 { return Err("characteristic is not readable".into()) }
                        self.stage = UuidStage::Value;
                        return Ok(ReadStep::Request(att_read(handle).to_vec()));
                    }
                }
                if last == end { return Err("characteristic UUID not found".into()) }
                let next = last + 1;
                self.stage = UuidStage::Characteristic(next, end);
                Ok(ReadStep::Request(read_by_type(next, end)))
            }
            UuidStage::Value if p.first() == Some(&0x0b) => Ok(ReadStep::Value(p[1..].to_vec())),
            _ => Err(format!("invalid ATT discovery response {}", hex(p))),
        }
    }
}
pub fn att_read(handle: u16) -> [u8; 3] { [0x0a, handle as u8, (handle >> 8) as u8] }

pub struct Peer {
    link: Link,
    log: Vec<String>,
    commands: VecDeque<Command>,
    advertising: bool,
    connected: bool,
    connecting: bool,
    guest_central: bool,
    pending: Option<Request>,
    discovery_start: u16,
    value: Vec<u8>,
    /// The devices around (`set_nearby`), and the guest's scan, active or passive, while it runs.
    nearby: Vec<Nearby>,
    scanning: Option<bool>,
}
impl Peer {
    fn new(link: Link) -> Self {
        Self {
            link,
            log: Vec::new(),
            commands: VecDeque::new(),
            advertising: false,
            connected: false,
            connecting: false,
            guest_central: false,
            pending: None,
            discovery_start: 1,
            value: vec![100],
            nearby: Vec::new(),
            scanning: None,
        }
    }
    /// The devices around, replacing the ones there were; a running scan hears the new list at once.
    pub fn set_nearby(&mut self, list: Vec<Nearby>) {
        self.nearby = list;
        if let Some(active) = self.scanning { self.report_nearby(active); }
    }
    fn report_nearby(&mut self, active: bool) {
        for n in &self.nearby {
            self.link.borrow_mut().to_controller.push_back(LinkAction::Report {
                addr: n.addr, addr_type: n.addr_type, data: n.adv.clone(), scan_response: n.scan_response.clone(), rssi: n.rssi, active,
            });
            let fields: Vec<_> = [&n.adv, &n.scan_response].into_iter().flat_map(|data| advertising_fields(data)).collect();
            self.log.push(format!("scan report addr={} rssi={} {}", addr_text(&n.addr), n.rssi, fields.join(" ")));
        }
    }
    fn advertisement(&mut self, adv: Vec<u8>, scan_response: Vec<u8>) {
        let fields: Vec<_> = [&adv, &scan_response].into_iter().flat_map(|data| advertising_fields(data)).collect();
        self.log.push(format!("advertising {} data={}", fields.join(" "), hex(&adv)));
    }
    pub fn command(&mut self, command: &str) -> Result<(), String> {
        let command = command.parse()?;
        if matches!(command, Command::CentralStop | Command::Disconnect | Command::ReadUuid(..)) { return Err("central-stop/disconnect/read-uuid requires full BLE".into()) }
        self.commands.push_back(command);
        self.run_commands();
        Ok(())
    }
    fn run_commands(&mut self) {
        while self.pending.is_none() {
            let Some(command) = self.commands.front() else { break };
            if matches!(command, Command::Connect) {
                if !self.advertising || self.connected { break; }
                self.link.borrow_mut().to_controller.push_back(LinkAction::Connect);
                self.advertising = false;
                self.connecting = true;
                self.commands.pop_front();
                break;
            }
            if !self.connected || self.guest_central { break; }
            match self.commands.pop_front().unwrap() {
                Command::Connect | Command::CentralStop | Command::Disconnect | Command::ReadUuid(..) => unreachable!(),
                Command::Discover => self.discover(Request::Services, 1),
                Command::Read(handle) => {
                    self.pending = Some(Request::Read(handle));
                    self.att(&att_read(handle));
                }
                Command::Write(handle, value) => {
                    self.pending = Some(Request::Write(handle));
                    let mut p = vec![0x12, handle as u8, (handle >> 8) as u8];
                    p.extend(value);
                    self.att(&p);
                }
            }
        }
    }
    fn att(&mut self, pdu: &[u8]) { self.l2cap(4, pdu); }
    fn l2cap(&mut self, cid: u16, pdu: &[u8]) { self.link.borrow_mut().to_controller.push_back(LinkAction::Data(cid, pdu.to_vec())); }
    fn poll(&mut self) {
        loop {
            let event = self.link.borrow_mut().to_peer.pop_front();
            let Some(event) = event else { break };
            match event {
                LinkEvent::Advertising(adv, response) => {
                    self.advertising = true;
                    self.advertisement(adv, response);
                }
                LinkEvent::AdvertisingStopped => self.advertising = false,
                LinkEvent::Scan(active) => {
                    self.link.borrow_mut().to_controller.push_back(LinkAction::Advertisement(ADV.to_vec(), active));
                    self.log.push("scan report name=esp32sim service=180f".into());
                    self.scanning = Some(active);
                    self.report_nearby(active);
                }
                LinkEvent::ScanStopped => self.scanning = None,
                LinkEvent::Connected(central) => {
                    self.connecting = false;
                    self.connected = true;
                    self.advertising = false;
                    self.guest_central = central;
                    self.log.push(format!(
                        "connected handle=0x{HANDLE:04x} guest={}",
                        if central { "central" } else { "peripheral" }
                    ));
                }
                LinkEvent::Disconnected => {
                    self.connecting = false;
                    self.connected = false;
                    self.pending = None;
                    self.log.push("disconnected".into());
                }
                LinkEvent::Reset => {
                    self.connecting = false;
                    self.connected = false;
                    self.advertising = false;
                    self.pending = None;
                }
                LinkEvent::Data(cid, p) => match cid {
                    4 => self.receive_att(&p),
                    5 if p.len() >= 4 && p[0] == 0x12 => self.l2cap(5, &[0x13, p[1], 2, 0, 0, 0]),
                    6 => self.l2cap(6, &[5, 5]),
                    _ => self.log.push(format!("unsupported L2CAP cid=0x{cid:04x}")),
                },
                LinkEvent::Invalid(reason) => self.log.push(
                    match reason {
                        LinkError::H4 => "invalid H4 packet",
                        LinkError::AclLength => "invalid ACL length or connection handle",
                        LinkError::AclContinuation => "invalid ACL continuation",
                        LinkError::L2capLength => "invalid L2CAP length",
                    }
                    .into(),
                ),
                LinkEvent::Unsupported(op) => self.log.push(format!("unsupported HCI opcode=0x{op:04x}")),
            }
            self.run_commands();
        }
    }
    fn discover(&mut self, kind: Request, start: u16) {
        self.pending = Some(kind);
        self.discovery_start = start;
        let p = match kind {
            Request::Characteristics => read_by_type(start, u16::MAX),
            Request::Services => vec![0x10, start as u8, (start >> 8) as u8, 0xff, 0xff, 0, 0x28],
            _ => vec![4, start as u8, (start >> 8) as u8, 0xff, 0xff],
        };
        self.att(&p);
    }
    fn discovery_next(&mut self, kind: Request) {
        match kind {
            Request::Services => self.discover(Request::Characteristics, 1),
            Request::Characteristics => self.discover(Request::Descriptors, 1),
            Request::Descriptors => self.log.push("discovery complete".into()),
            _ => {}
        }
    }
    fn receive_att(&mut self, p: &[u8]) {
        if p.is_empty() {
            self.log.push("empty ATT packet".into());
            return;
        }
        if p[0] == 2 && p.len() == 3 {
            self.att(&[3, 23, 0]);
            return;
        }
        if p[0] == 0x1b || p[0] == 0x1d {
            if p.len() >= 3 {
                self.log.push(format!(
                    "notification handle=0x{:04x} value={} text={:?}",
                    word(&p[1..]),
                    hex(&p[3..]),
                    String::from_utf8_lossy(&p[3..])
                ));
            }
            if p[0] == 0x1d { self.att(&[0x1e]); }
            return;
        }
        if self.guest_central {
            self.peripheral_att(p);
            return;
        }
        let Some(kind) = self.pending.take() else {
            self.log.push(format!("unsolicited ATT {}", hex(p)));
            return;
        };
        if p[0] == 1 && p.len() == 5 {
            if p[4] == 0x0a && matches!(kind, Request::Services | Request::Characteristics | Request::Descriptors) {
                self.discovery_next(kind);
            } else {
                self.log.push(format!(
                    "ATT error request=0x{:02x} handle=0x{:04x} error=0x{:02x}",
                    p[1],
                    word(&p[2..]),
                    p[4]
                ));
            }
            return;
        }
        match kind {
            Request::Read(handle) if p[0] == 0x0b => self.log.push(format!(
                "read handle=0x{handle:04x} value={} text={:?}",
                hex(&p[1..]),
                String::from_utf8_lossy(&p[1..])
            )),
            Request::Write(handle) if p == [0x13] => self.log.push(format!("write complete handle=0x{handle:04x}")),
            Request::Services | Request::Characteristics | Request::Descriptors => {
                let expected = match kind {
                    Request::Services => 0x11,
                    Request::Characteristics => 9,
                    _ => 5,
                };
                if p[0] != expected || p.len() < 2 {
                    self.log.push(format!("invalid ATT discovery response {}", hex(p)));
                    return;
                }
                let size = if kind == Request::Descriptors {
                    match p[1] {
                        1 => 4,
                        2 => 18,
                        _ => 0,
                    }
                } else {
                    p[1] as usize
                };
                let allowed = match kind {
                    Request::Services => matches!(size, 6 | 20),
                    Request::Characteristics => matches!(size, 7 | 21),
                    _ => matches!(size, 4 | 18),
                };
                if !allowed || p.len() == 2 || !(p.len() - 2).is_multiple_of(size) {
                    self.log.push(format!("invalid ATT entry size {}", hex(p)));
                    return;
                }
                let mut last = 0;
                for item in p[2..].chunks_exact(size) {
                    let handle = word(item);
                    let end = if kind == Request::Services { word(&item[2..]) } else { handle };
                    if handle < self.discovery_start || handle <= last || end < handle {
                        self.log.push("invalid non-progressing ATT discovery handles".into());
                        return;
                    }
                    last = end;
                    match kind {
                        Request::Services => {
                            last = word(&item[2..]);
                            self.log.push(format!(
                                "service start=0x{:04x} end=0x{last:04x} uuid={}",
                                word(item),
                                uuid(&item[4..])
                            ));
                        }
                        Request::Characteristics => self.log.push(format!(
                            "characteristic declaration=0x{last:04x} handle=0x{:04x} properties=0x{:02x} uuid={}",
                            word(&item[3..]),
                            item[2],
                            uuid(&item[5..])
                        )),
                        _ => self.log.push(format!("attribute handle=0x{last:04x} uuid={}", uuid(&item[2..]))),
                    }
                }
                if last == u16::MAX {
                    self.discovery_next(kind);
                } else { self.discover(kind, last + 1); }
            }
            _ => self.log.push(format!("unexpected ATT response {}", hex(p))),
        }
    }
    fn peripheral_att(&mut self, p: &[u8]) {
        let error = |code, handle, reason| vec![1, code, handle as u8, (handle >> 8) as u8, reason];
        let response = match p[0] {
            0x10 if p.len() == 7 && word(&p[5..]) == 0x2800 => {
                if word(&p[1..]) == 1 && word(&p[3..]) >= 1 {
                    vec![0x11, 6, 1, 0, 3, 0, 0x0f, 0x18]
                } else { error(p[0], word(&p[1..]), 0x0a) }
            }
            8 if p.len() == 7 && word(&p[5..]) == 0x2803 => {
                if (1..=2).contains(&word(&p[1..])) && word(&p[3..]) >= 2 {
                    vec![9, 7, 2, 0, 0x0a, 3, 0, 0x19, 0x2a]
                } else { error(p[0], word(&p[1..]), 0x0a) }
            }
            4 if p.len() == 5 => {
                let mut response = vec![5, 1];
                for (handle, kind) in [(1u16, 0x2800u16), (2, 0x2803), (3, 0x2a19)] {
                    if (word(&p[1..])..=word(&p[3..])).contains(&handle) {
                        response.extend_from_slice(&handle.to_le_bytes());
                        response.extend_from_slice(&kind.to_le_bytes());
                    }
                }
                if response.len() == 2 {
                    error(p[0], word(&p[1..]), 0x0a)
                } else { response }
            }
            0x0a if p.len() == 3 && word(&p[1..]) == 3 => {
                let mut r = vec![0x0b];
                r.extend_from_slice(&self.value);
                r
            }
            0x12 | 0x52 if p.len() >= 3 && word(&p[1..]) == 3 => {
                self.value = p[3..].to_vec();
                if p[0] == 0x52 { return; }
                vec![0x13]
            }
            _ => error(p[0], if p.len() >= 3 { word(&p[1..]) } else { 0 }, 0x0a),
        };
        self.att(&response);
    }
}

/// Guest H4 endpoint plus an independently replaceable controller and peer link.
pub struct Session {
    pub controller: Box<dyn HciController>,
    pub peer: Peer,
    cycles: u64,
}
impl Default for Session {
    fn default() -> Self {
        let link = Link::default();
        Self::new(Box::new(Controller::new(link.clone())), link)
    }
}
impl Session {
    pub fn new(controller: Box<dyn HciController>, link: Link) -> Self { Self { controller, peer: Peer::new(link), cycles: 0 } }
    /// A guest reset; the devices around stay, as they are not the chip's.
    pub fn reset(&mut self) {
        self.controller.reset();
        let nearby = std::mem::take(&mut self.peer.nearby);
        self.peer = Peer::new(self.peer.link.clone());
        self.peer.nearby = nearby;
        self.cycles = 0;
    }
    pub fn advance_to(&mut self, cycles: u64) {
        self.cycles = cycles;
        for _ in 0..64 {
            self.controller.advance_to(cycles);
            self.peer.poll();
            if self.peer.link.borrow().to_controller.is_empty() { break; }
        }
    }
    pub fn send(&mut self, packet: &[u8]) {
        self.controller.send_h4(packet);
        self.advance_to(self.cycles);
    }
    pub fn pop_packet(&mut self) -> Option<Vec<u8>> {
        self.advance_to(self.cycles);
        self.controller.poll_h4()
    }
    pub fn command(&mut self, command: &str) -> Result<(), String> {
        self.peer.command(command)?;
        self.advance_to(self.cycles);
        Ok(())
    }
    pub fn pending_commands(&self) -> usize {
        self.peer.commands.len() + usize::from(self.peer.pending.is_some()) + usize::from(self.peer.connecting)
    }
    pub fn drain_log(&mut self) -> Vec<String> { std::mem::take(&mut self.peer.log) }
    /// `Peer::set_nearby`, then what it sent goes out.
    pub fn set_nearby(&mut self, list: Vec<Nearby>) {
        self.peer.set_nearby(list);
        self.advance_to(self.cycles);
    }
    pub fn is_advertising(&self) -> bool { self.peer.advertising }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cmd(c: &mut Session, op: u16, p: &[u8]) {
        let mut packet = vec![1, op as u8, (op >> 8) as u8, p.len() as u8];
        packet.extend_from_slice(p);
        c.send(&packet);
    }
    fn acl(c: &mut Session, p: &[u8]) {
        let mut packet = vec![2, 1, 0x20, (p.len() + 4) as u8, 0, p.len() as u8, 0, 4, 0];
        packet.extend_from_slice(p);
        c.send(&packet);
    }
    fn connect(c: &mut Session) {
        cmd(c, 0x200a, &[1]);
        c.command("connect").unwrap();
        while c.pop_packet().is_some() {}
    }
    #[test]
    fn full_controller_commands_do_not_queue_in_hci_mode() {
        let mut session = Session::default();
        for command in ["central-stop", "disconnect", "read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8"] {
            assert!(session.command(command).unwrap_err().contains("requires full BLE"));
            assert_eq!(session.pending_commands(), 0);
        }
    }

    #[test]
    fn hex_bytes_and_read_by_type_preserve_wire_values() {
        assert_eq!(hex_bytes("00aAFF"), Some(vec![0,170,255]));
        for bad in ["0", "0g", "é0"] { assert_eq!(hex_bytes(bad), None); }
        assert_eq!("write 1 00aAFF".parse::<Command>().unwrap(), Command::Write(1, vec![0,170,255]));
        for bad in ["write 1 0", "write 1 zz", "write 1 é0", "write 1 000000000000000000000000000000000000000000"] { assert!(bad.parse::<Command>().is_err()); }
        assert_eq!(read_by_type(0x1234, 0x5678), [8,0x34,0x12,0x78,0x56,3,0x28]);
    }

    #[test]
    fn uuid_read_discovers_handles_and_rejects_invalid_ranges() {
        let Command::ReadUuid(service, characteristic) = "read-uuid 4fafc201-1fb5-459e-8fcc-c5c9c331914b beb5483e-36e1-4688-b7f5-ea07361b26a8".parse().unwrap() else { panic!("UUID command") };
        let mut read = UuidRead::new(service, characteristic);
        assert_eq!(&read.request()[7..], &service);
        assert!(matches!(read.receive(&[7,14,0,16,0]).unwrap(), ReadStep::Request(p) if p == [8,14,0,16,0,3,0x28]));
        let mut declaration = vec![9,21,15,0,2,16,0]; declaration.extend(characteristic);
        assert!(matches!(read.receive(&declaration).unwrap(), ReadStep::Request(p) if p == [10,16,0]));
        assert!(matches!(read.receive(&[11,42]).unwrap(), ReadStep::Value(p) if p == [42]));
        let mut invalid = UuidRead::new(service, characteristic);
        assert!(invalid.receive(&[7,16,0,14,0]).is_err());
        assert!("read-uuid zz ab".parse::<Command>().is_err());
    }

    #[test]
    fn uuid_read_validates_pages_permissions_and_errors() {
        let service = [1;16]; let characteristic = [2;16];
        assert_eq!(parse_uuid("00010203-0405-0607-0809-0a0b0c0d0e0f").unwrap(), [15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0]);
        for text in ["", "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", "é000000000000000000000000000000"] { assert!(parse_uuid(text).is_err()); }
        for bad in [&[7,0,0,16,0][..], &[7,16,0,14,0], &[7,1,0], &[1,6,0,0,10]] {
            assert!(UuidRead::new(service, characteristic).receive(bad).is_err());
        }
        let page = |declaration, handle, properties, uuid: [u8;16]| {
            let mut p = vec![9,21,declaration,0,properties,handle,0]; p.extend(uuid); p
        };
        for bad in [page(0,2,2,characteristic), page(17,18,2,characteristic), page(2,2,2,characteristic),
            page(2,17,2,characteristic), page(2,3,0,characteristic), vec![9,0], vec![9,21,1]] {
            let mut read = UuidRead::new(service, characteristic); read.receive(&[7,1,0,16,0]).unwrap();
            assert!(read.receive(&bad).is_err());
        }
        let mut read = UuidRead::new(service, characteristic); read.receive(&[7,1,0,16,0]).unwrap();
        assert!(matches!(read.receive(&page(2,3,2,[3;16])).unwrap(), ReadStep::Request(p) if p == [8,3,0,16,0,3,0x28]));
        assert!(matches!(read.receive(&page(4,5,2,characteristic)).unwrap(), ReadStep::Request(p) if p == [10,5,0]));
        assert!(read.receive(&[9]).is_err());
        assert!(matches!(read.receive(&[11]).unwrap(), ReadStep::Value(p) if p.is_empty()));
    }

    #[test]
    fn deferred_controller_is_bounded_and_pending_commands_survive() {
        struct Deferred { link: Link, calls: std::rc::Rc<std::cell::Cell<usize>> }
        impl HciController for Deferred {
            fn send_h4(&mut self, _: &[u8]) {}
            fn poll_h4(&mut self) -> Option<Vec<u8>> { None }
            fn reset(&mut self) {}
            fn next_deadline(&self) -> Option<u64> { Some(100) }
            fn advance_to(&mut self, cycles: u64) {
                self.calls.set(self.calls.get() + 1);
                if cycles >= 100 {
                    let mut link = self.link.borrow_mut();
                    while let Some(action) = link.to_controller.pop_front() {
                        if matches!(action, LinkAction::Connect) { link.to_peer.push_back(LinkEvent::Connected(false)); }
                    }
                }
            }
        }
        let link = Link::default();
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut c = Session::new(Box::new(Deferred { link: link.clone(), calls: calls.clone() }), link.clone());
        c.command("connect").unwrap();
        assert_eq!(c.pending_commands(), 1);
        link.borrow_mut().to_peer.push_back(LinkEvent::Advertising(ADV.to_vec(), Vec::new()));
        calls.set(0);
        c.advance_to(99);
        assert_eq!(calls.get(), 64);
        assert_eq!(c.pending_commands(), 1);
        assert_eq!(link.borrow().to_controller.len(), 1);
        c.advance_to(100);
        assert_eq!(c.pending_commands(), 0);
        c.command("read 3").unwrap();
        c.command("write 3 01").unwrap();
        assert_eq!(c.pending_commands(), 2);
        link.borrow_mut().to_peer.push_back(LinkEvent::Data(4, vec![0x0b, 42]));
        c.advance_to(101);
        assert_eq!(c.pending_commands(), 1);
        link.borrow_mut().to_peer.push_back(LinkEvent::Data(4, vec![0x13]));
        c.advance_to(102);
        assert_eq!(c.pending_commands(), 0);
        c.command("read 3").unwrap();
        c.reset();
        assert_eq!(c.pending_commands(), 0);
    }

    #[test]
    fn initialization_advertising_and_scanning_use_standard_packets() {
        let mut c = Session::default();
        cmd(&mut c, 0x080f, &[3, 0]);
        assert_eq!(c.pop_packet().unwrap(), [4, 14, 4, 1, 15, 8, 0]);

        let mut name = [0u8; 248];
        name[..8].copy_from_slice(b"esp32sim");
        cmd(&mut c, 0x0c13, &name);
        c.pop_packet();
        cmd(&mut c, 0x0c14, &[]);
        let response = c.pop_packet().unwrap();
        assert_eq!(&response[..7], &[4, 14, 252, 1, 20, 12, 0]);
        assert_eq!(&response[7..], name);
        cmd(&mut c, 0x1004, &[1]);
        assert_eq!(c.pop_packet().unwrap(), [4, 14, 14, 1, 4, 16, 0, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0]);
        let mut data = [0u8; 32];
        data[0] = ADV.len() as u8;
        data[1..1 + ADV.len()].copy_from_slice(ADV);
        cmd(&mut c, 0x2008, &data);
        cmd(&mut c, 0x200a, &[1]);
        assert!(c.is_advertising());
        let log = c.drain_log();
        assert!(log[0].contains("name=\"esp32sim\""));
        assert!(log[0].contains("service=180f"));
        while c.pop_packet().is_some() {}
        cmd(&mut c, 0x200c, &[1, 1]);
        c.pop_packet();
        let report = c.pop_packet().unwrap();
        assert_eq!(&report[..7], &[4, 0x3e, (ADV.len() + 12) as u8, 2, 1, 0, 0]);
        assert!(report.windows(8).any(|s| s == b"esp32sim"));
        c.send(&[1, 3, 12, 1]);
        assert_eq!(c.pop_packet().unwrap(), [4, 14, 4, 1, 3, 12, 0x12]);
        cmd(&mut c, 0x7777, &[]);
        assert_eq!(c.pop_packet().unwrap(), [4, 14, 4, 1, 0x77, 0x77, 1]);
    }
    #[test]
    fn central_discovers_reads_writes_subscribes_and_receives_notifications() {
        let mut c = Session::default();
        connect(&mut c);
        c.command("discover").unwrap();
        assert_eq!(&c.pop_packet().unwrap()[9..], &[0x10, 1, 0, 0xff, 0xff, 0, 0x28]);
        acl(&mut c, &[0x11, 6, 1, 0, 5, 0, 0x0f, 0x18]);
        acl(&mut c, &[1, 0x10, 6, 0, 0x0a]);
        acl(&mut c, &[9, 7, 2, 0, 0x1a, 3, 0, 0x19, 0x2a]);
        acl(&mut c, &[1, 8, 3, 0, 0x0a]);
        acl(&mut c, &[5, 1, 4, 0, 2, 0x29]);
        acl(&mut c, &[1, 4, 5, 0, 0x0a]);
        assert!(c.peer.pending.is_none());
        assert!(c.peer.log.iter().any(|s| s.contains("uuid=180f")));
        c.command("read 3").unwrap();

        acl(&mut c, b"\x0bhello");
        c.command("write 0x3 4849").unwrap();
        acl(&mut c, &[0x13]);
        c.command("subscribe 4").unwrap();
        acl(&mut c, &[0x13]);
        acl(&mut c, &[0x1b, 3, 0, 0, 0, 0, 0]);
        assert!(c.peer.log.iter().any(|s| s.contains("text=\"hello\"")));
        assert!(c.peer.log.iter().any(|s| s.contains("notification handle=0x0003 value=00000000")));
        assert!(c.command("write 3 invalid").is_err());
    }
    #[test]
    fn malformed_commands_and_discovery_do_not_advance_state() {
        let mut c = Session::default();
        cmd(&mut c, 0x0c13, &[1]);
        assert_eq!(c.pop_packet().unwrap().last(), Some(&0x12));
        cmd(&mut c, 0x0c35, &[1]);
        assert_eq!(c.pop_packet().unwrap().last(), Some(&0x12));
        cmd(&mut c, 0xfd09, &[1]);
        assert_eq!(c.pop_packet().unwrap().last(), Some(&0x12));
        connect(&mut c);
        c.command("discover").unwrap();
        while c.pop_packet().is_some() {}
        acl(&mut c, &[0x11, 6, 2, 0, 1, 0, 0x0f, 0x18]);
        assert!(c.peer.pending.is_none());
        assert!(c.drain_log().iter().any(|s| s.contains("non-progressing")));
        assert_eq!(std::iter::from_fn(|| c.pop_packet()).count(), 1, "only ACL credit, no repeated discovery request");
    }
    #[test]
    fn acl_reassembly_rejects_bad_lengths_and_virtual_peripheral_answers_att() {
        let mut c = Session::default();
        let mut p = [0; 25];
        p[6..12].copy_from_slice(&PEER);
        cmd(&mut c, 0x200d, &p);
        while c.pop_packet().is_some() {}
        c.send(&[2, 1, 0x20, 5, 0, 3, 0, 4, 0, 0x0a]);
        c.send(&[2, 1, 0x10, 2, 0, 3, 0]);
        let packets: Vec<_> = std::iter::from_fn(|| c.pop_packet()).collect();
        assert_eq!(&packets[2][9..], &[0x0b, 100]);
        c.send(&[2, 1, 0x20, 5, 0]);
        assert!(c.drain_log().iter().any(|s| s.contains("invalid ACL")));
        acl(&mut c, &[0x12, 3, 0, 42]);
        acl(&mut c, &[0x0a, 3, 0]);
        assert_eq!(&std::iter::from_fn(|| c.pop_packet()).last().unwrap()[9..], &[0x0b, 42]);
    }
    #[test]
    fn commands_wait_for_advertising_and_serialize_att() {
        let mut c = Session::default();
        c.command("connect").unwrap();
        c.command("read 3").unwrap();
        c.command("write 3 2a").unwrap();
        assert!(c.pop_packet().is_none());
        // Host initialization must not discard commands queued before advertising.
        cmd(&mut c, 0x0c03, &[]);
        c.pop_packet();
        cmd(&mut c, 0x200a, &[1]);
        c.pop_packet(); // command complete
        assert_eq!(c.pop_packet().unwrap()[1], 0x3e);
        assert_eq!(&c.pop_packet().unwrap()[9..], &[0x0a, 3, 0]);
        assert!(c.pop_packet().is_none());
        acl(&mut c, &[0x0b, 100]);
        c.pop_packet(); // ACL credit
        assert_eq!(&c.pop_packet().unwrap()[9..], &[0x12, 3, 0, 42]);
        assert!(c.pop_packet().is_none());
        acl(&mut c, &[0x13]);
        assert!(c.peer.pending.is_none());
        assert!(c.peer.commands.is_empty());
    }

    #[test]
    fn peer_works_with_a_different_timed_controller() {
        struct Timed {
            link: Link,
            now: u64,
            advertised: bool,
        }
        impl HciController for Timed {
            fn send_h4(&mut self, _: &[u8]) {}
            fn poll_h4(&mut self) -> Option<Vec<u8>> { None }
            fn reset(&mut self) {
                self.now = 0;
                self.advertised = false;
                *self.link.borrow_mut() = LinkQueues::default();
            }
            fn next_deadline(&self) -> Option<u64> { (!self.advertised).then_some(100) }
            fn advance_to(&mut self, cycles: u64) {
                self.now = cycles;
                if !self.advertised && cycles >= 100 {
                    self.advertised = true;
                    self.link.borrow_mut().to_peer.push_back(LinkEvent::Advertising(ADV.to_vec(), Vec::new()));
                }
                loop {
                    let action = self.link.borrow_mut().to_controller.pop_front();
                    let Some(action) = action else { break };
                    let event = match action {
                        LinkAction::Connect => LinkEvent::Connected(false),
                        LinkAction::Data(4, p) => {
                            assert_eq!(p, [0x0a, 3, 0]);
                            LinkEvent::Data(4, vec![0x0b, 42])
                        }
                        _ => panic!("unexpected link action"),
                    };
                    self.link.borrow_mut().to_peer.push_back(event);
                }
            }
        }
        let link = Link::default();
        let mut c = Session::new(Box::new(Timed { link: link.clone(), now: 0, advertised: false }), link);
        c.command("connect").unwrap();
        c.command("read 3").unwrap();
        assert_eq!(c.controller.next_deadline(), Some(100));
        c.advance_to(99);
        assert!(c.drain_log().is_empty());
        c.advance_to(100);
        assert!(c.drain_log().iter().any(|s| s.contains("read handle=0x0003 value=2a")));
        assert_eq!(c.controller.next_deadline(), None);
        c.reset();
        assert_eq!(c.controller.next_deadline(), Some(100));
        assert!(!c.peer.connected);
        assert!(c.peer.commands.is_empty());
    }
    /// One LE Advertising Report: (kind, address type, address in HCI order, data, rssi).
    type Report = (u8, u8, [u8; 6], Vec<u8>, i8);
    /// The LE Advertising Reports of the packets the guest gets.
    fn reports(c: &mut Session) -> Vec<Report> {
        let mut out = Vec::new();
        while let Some(p) = c.pop_packet() {
            if p[1] != 0x3e || p[3] != 2 { continue; }
            let n = p[13] as usize;
            out.push((p[5], p[6], p[7..13].try_into().unwrap(), p[14..14 + n].to_vec(), p[14 + n] as i8));
        }
        out
    }
    fn thermo() -> Nearby { Nearby::parse("name=Thermo,rssi=-70,addr=c4:aa:01:02:03:04,type=public,rsp=03030f18").unwrap() }
    /// LE Set Scan Parameters (active or passive), then LE Set Scan Enable.
    fn scan(c: &mut Session, active: bool) {
        cmd(c, 0x200b, &[u8::from(active), 0x10, 0, 0x10, 0, 0, 0]);
        cmd(c, 0x200c, &[1, 0]);
    }

    #[test]
    fn nearby_specs_parse_with_defaults_and_their_own_addresses() {
        let t = thermo();
        assert_eq!(t.adv, b"\x02\x01\x06\x07\x09Thermo");
        assert_eq!((t.addr, t.addr_type, t.rssi), ([4, 3, 2, 1, 0xaa, 0xc4], 0, -70), "addr in HCI order, public");
        assert_eq!(t.scan_response, [3, 3, 0x0f, 0x18]);
        let plain = Nearby::parse("name=Thermo").unwrap();
        assert_eq!((plain.addr_type, plain.rssi, plain.addr[5] & 0xc0), (1, -60, 0xc0), "random static by default");
        assert_eq!(plain.addr, Nearby::parse("name=Thermo,rssi=-90").unwrap().addr, "the same address every time");
        assert_ne!(plain.addr, Nearby::parse("name=Scale").unwrap().addr);
        assert_eq!(Nearby::parse("adv=0201060303aafe").unwrap().adv, [2, 1, 6, 3, 3, 0xaa, 0xfe]);
        // An LE Advertising Report's RSSI: -127 to +20 dBm, or 127 for not available.
        for (rssi, v) in [("-127", -127), ("20", 20), ("127", 127)] { assert_eq!(Nearby::parse(&format!("name=a,rssi={rssi}")).unwrap().rssi, v); }
        // The reserved top bits 10 only refuse a random address; a public one may have them.
        assert_eq!(Nearby::parse("name=a,addr=84:aa:01:02:03:04,type=public").unwrap().addr_type, 0);
        for bad in ["rssi=-60", "name=", &format!("name={}", "x".repeat(27)), &format!("adv={}", "00".repeat(32)), "name=a,rsp=zz",
            "name=a,addr=c4:aa:01:02:03", "name=a,addr=c4:aa:01:02:03:4", "name=a,type=static", "name=a,rssi=loud", "name=a,uuid=180f", "name",
            "name=a,adv=020106", "name=a,rssi=-128", "name=a,rssi=21", "name=a,rssi=126", "name=a,addr=84:aa:01:02:03:04"] {
            assert!(Nearby::parse(bad).is_err(), "{bad}");
        }
        let specs = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(Nearby::parse_all(&specs(&["name=Thermo", "name=Thermo,rssi=-90"])).unwrap_err().contains("share the address"));
        assert!(Nearby::parse_all(&specs(&["name=a,addr=02:53:49:00:00:02"])).unwrap_err().contains("virtual peer"));
        assert!(Nearby::parse_all(&specs(&["name=a,addr=02:53:49:00:00:01"])).unwrap_err().contains("guest"));
        assert!(Nearby::parse_all(&vec!["name=x".to_string(); NEARBY_MAX + 1]).is_err());
        assert_eq!(Nearby::parse_all(&specs(&["name=Thermo", "name=Scale"])).unwrap().len(), 2);
    }

    #[test]
    fn a_scan_reports_the_devices_around_after_the_virtual_peer() {
        for active in [true, false] {
            let mut c = Session::default();
            c.set_nearby(vec![thermo()]);
            assert!(reports(&mut c).is_empty(), "nothing before a scan");
            scan(&mut c, active);
            let r = reports(&mut c);
            let peer: Vec<_> = r.iter().filter(|x| x.2 == super::super::PEER).collect();
            let around: Vec<_> = r.iter().filter(|x| x.2 == thermo().addr).map(|x| (x.0, x.1, x.3.clone(), x.4)).collect();
            assert!(!peer.is_empty(), "the virtual peer first");
            let mut expected = vec![(2, 0, thermo().adv, -70)];
            if active { expected.push((4, 0, vec![3, 3, 0x0f, 0x18], -70)); }
            assert_eq!(around, expected, "ADV_SCAN_IND, and SCAN_RSP for an active scan (active={active})");
            assert!(c.drain_log().iter().any(|l| l.starts_with("scan report addr=c4:aa:01:02:03:04 rssi=-70 name=\"Thermo\"")));
        }
    }

    #[test]
    fn a_running_scan_hears_a_new_list_at_once_and_a_stopped_one_nothing() {
        let mut c = Session::default();
        scan(&mut c, true);
        assert!(reports(&mut c).iter().all(|x| x.2 == super::super::PEER));
        c.set_nearby(vec![thermo()]);
        assert_eq!(reports(&mut c).iter().filter(|x| x.2 == thermo().addr).count(), 2, "ADV_SCAN_IND and SCAN_RSP at once");
        cmd(&mut c, 0x200c, &[0, 0]);
        reports(&mut c);
        c.set_nearby(vec![thermo()]);
        assert!(reports(&mut c).is_empty(), "no scan, no report");
    }

    #[test]
    fn the_devices_around_survive_a_guest_reset() {
        let mut c = Session::default();
        c.set_nearby(vec![thermo()]);
        c.reset();
        scan(&mut c, false);
        assert_eq!(reports(&mut c).iter().filter(|x| x.2 == thermo().addr).count(), 1);
    }

    /// The status of one command's Command Complete.
    fn status(c: &mut Session, op: u16, p: &[u8]) -> u8 {
        cmd(c, op, p);
        let r = c.pop_packet().unwrap();
        assert_eq!(&r[..6], &[4, 14, r[2], 1, op as u8, (op >> 8) as u8]);
        r[6]
    }
    /// LE Add Device To Resolving List for a public address ending in `last` (HCI order).
    fn rl_add(last: u8) -> Vec<u8> {
        let mut p = vec![0, last, 0, 0, 0, 0, 0];
        p.extend((7..39).map(|i| (i + last as usize) as u8));
        p
    }

    #[test]
    fn ll_privacy_is_reported_and_the_resolving_list_answers_as_on_an_s3() {
        let mut c = Session::default();
        cmd(&mut c, 0x2003, &[]);
        let features = c.pop_packet().unwrap();
        assert_eq!(features[7] & 0x40, 0x40, "LE feature bit 6, LL Privacy");
        cmd(&mut c, 0x1002, &[]);
        let commands = c.pop_packet().unwrap();
        assert_eq!((commands[7 + 34] & 0xf8, commands[7 + 35] & 0x07, commands[7 + 39] & 0x04), (0xf8, 0x07, 0x04));
        cmd(&mut c, 0x202a, &[]);
        assert_eq!(c.pop_packet().unwrap(), [4, 14, 5, 1, 0x2a, 0x20, 0, 10]);
        // The sequence the receipt's probe sent to an ESP32-S3, with the statuses it answered.
        let remove = |last: u8| vec![0, last, 0, 0, 0, 0, 0];
        let script: Vec<(u16, Vec<u8>, u8)> = vec![
            (0x2029, vec![], 0),
            (0x2028, remove(0), 0x02),
            (0x2027, rl_add(0), 0),
            (0x2027, rl_add(0), 0x12),
            (0x2028, remove(0), 0),
            (0x2028, remove(0), 0x02),
        ];
        for (op, p, expected) in script { assert_eq!(status(&mut c, op, &p), expected, "0x{op:04x} {p:02x?}"); }
        for last in 1..=10 { assert_eq!(status(&mut c, 0x2027, &rl_add(last)), 0, "entry {last}"); }
        assert_eq!(status(&mut c, 0x2027, &rl_add(11)), 0x07, "the eleventh: Memory Capacity Exceeded");
        let script: Vec<(u16, Vec<u8>, u8)> = vec![
            (0x2029, vec![], 0),
            (0x202d, vec![1], 0),
            (0x202d, vec![2], 0x12),
            (0x2027, rl_add(1), 0),
            (0x202d, vec![0], 0),
            (0x202e, vec![0x84, 0x03], 0),
            (0x202e, vec![0, 0], 0x12),
            (0x202e, vec![0x10, 0x0e], 0),
            (0x202e, vec![0x11, 0x0e], 0),
            (0x204e, vec![0, 1, 0, 0, 0, 0, 0, 1], 0),
            (0x204e, vec![0, 0x7f, 0, 0, 0, 0, 0, 1], 0x02),
            (0x204e, vec![0, 1, 0, 0, 0, 0, 0, 2], 0x12),
        ];
        for (op, p, expected) in script { assert_eq!(status(&mut c, op, &p), expected, "0x{op:04x} {p:02x?}"); }
        for (op, entry) in [(0x202b, 1), (0x202b, 0x7f), (0x202c, 1)] {
            cmd(&mut c, op, &remove(entry));
            assert_eq!(c.pop_packet().unwrap(), [4, 14, 10, 1, op as u8, 0x20, 2, 0, 0, 0, 0, 0, 0]);
        }
        // HCI Reset and a guest reset empty the list.
        status(&mut c, 0x0c03, &[]);
        assert_eq!(status(&mut c, 0x204e, &[0, 1, 0, 0, 0, 0, 0, 1]), 0x02);
        assert_eq!(status(&mut c, 0x2027, &rl_add(1)), 0);
        c.reset();
        assert_eq!(status(&mut c, 0x2028, &remove(1)), 0x02);
        assert!(c.drain_log().iter().all(|l| !l.contains("unsupported")));
    }
}
