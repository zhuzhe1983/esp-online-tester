//! A packet-level BLE controller and one virtual peer, independent of the guest CPU.
mod opcodes;
pub mod peer;
pub mod vhci;
use opcodes::*;
use std::collections::VecDeque;

/// Complete H4 packets, including the type byte. Time is monotonic guest CPU cycles.
/// `next_deadline` is the next desired service cycle, or None while idle. The machine
/// currently services controllers only at guest adapter polls and script commands;
/// it does not schedule SoC wakeups from this deadline.
/// `advance_to` must drain the link's `to_controller` queue into controller-owned
/// state, even for actions deferred until a later connection event. A Session
/// bounds same-cycle exchanges to 64 rounds; remaining actions wait for the next poll.
pub trait HciController {
    fn send_h4(&mut self, packet: &[u8]);
    fn poll_h4(&mut self) -> Option<Vec<u8>>;
    fn reset(&mut self);
    fn advance_to(&mut self, cycles: u64);
    fn next_deadline(&self) -> Option<u64>;
}

/// Put back the flash bytes a controller adapter patched (VHCI on classic/S3/C3, NimBLE on C6).
pub fn restore_patched_flash(saved: &mut Option<(usize, Vec<u8>)>, flash: &mut [u8]) {
    if let Some((off, original)) = saved.take() {
        flash[off..off + original.len()].copy_from_slice(&original);
    }
}

/// Packet-level link boundary. A replacement controller bridges its radio/link to these queues.
pub type Link = std::rc::Rc<std::cell::RefCell<LinkQueues>>;
#[derive(Default)]
pub struct LinkQueues {
    pub to_peer: VecDeque<LinkEvent>,
    pub to_controller: VecDeque<LinkAction>,
}
pub enum LinkEvent {
    Advertising(Vec<u8>, Vec<u8>),
    AdvertisingStopped,
    Scan(bool),
    /// LE Set Scan Enable turned scanning off.
    ScanStopped,
    Connected(bool),
    Disconnected,
    Reset,
    Data(u16, Vec<u8>),
    Invalid(LinkError),
    Unsupported(u16),
}
pub enum LinkError {
    H4,
    AclLength,
    AclContinuation,
    L2capLength,
}
pub enum LinkAction {
    Connect,
    Advertisement(Vec<u8>, bool),
    Data(u16, Vec<u8>),
    /// A device around the station (`peer::Nearby`) in the guest's scan: an LE Advertising Report,
    /// scannable and not connectable.
    Report { addr: [u8; 6], addr_type: u8, data: Vec<u8>, scan_response: Vec<u8>, rssi: i8, active: bool },
}

const HANDLE: u16 = 1;
const ADDRESS: [u8; 6] = [1, 0, 0, 0x49, 0x53, 2];
const PEER: [u8; 6] = [2, 0, 0, 0x49, 0x53, 2];
/// What an ESP32-S3's controller answers to LE Read Resolving List Size.
const RESOLVING_LIST_SIZE: usize = 10;

fn word(b: &[u8]) -> u16 { u16::from_le_bytes([b[0], b[1]]) }
/// No RF, encryption sessions or pairing. One ACL link is enough for the Arduino examples.
/// `send` and `pop_packet` exchange complete H4 packets including the packet-type byte.
pub struct Controller {
    packets: VecDeque<Vec<u8>>,
    link: Link,
    cycles: u64,
    advertising: bool,
    adv: Vec<u8>,
    scan_response: Vec<u8>,
    connected: bool,
    active_scan: bool,
    fragments: Vec<u8>,
    random: u64,
    local_name: [u8; 248],
    default_link_policy: u16,
    /// LL Privacy's resolving list: the peers' identity addresses (type, address in HCI order).
    resolving: Vec<(u8, [u8; 6])>,
}

impl Default for Controller { fn default() -> Self { Self::new(Link::default()) } }
impl Controller {
    pub fn new(link: Link) -> Self {
        Self {
            packets: VecDeque::new(),
            link,
            cycles: 0,
            advertising: false,
            adv: Vec::new(),
            scan_response: Vec::new(),
            connected: false,
            active_scan: false,
            fragments: Vec::new(),
            random: 0x3253_5045_424c_4531,
            local_name: [0; 248],
            default_link_policy: 0,
            resolving: Vec::new(),
        }
    }
    pub fn pop_packet(&mut self) -> Option<Vec<u8>> { self.packets.pop_front() }
    fn emit(&mut self, event: LinkEvent) { self.link.borrow_mut().to_peer.push_back(event); }
    fn event(&mut self, event: u8, data: &[u8]) {
        let mut p = vec![4, event, data.len() as u8];
        p.extend_from_slice(data);
        self.packets.push_back(p);
    }
    fn complete(&mut self, op: u16, response: &[u8]) {
        let mut p = vec![1, op as u8, (op >> 8) as u8];
        p.extend_from_slice(response);
        self.event(0x0e, &p);
    }
    fn status(&mut self, op: u16, status: u8) { self.event(0x0f, &[status, 1, op as u8, (op >> 8) as u8]); }
    fn l2cap(&mut self, cid: u16, pdu: &[u8]) {
        let len = pdu.len() as u16;
        let mut p = vec![2, HANDLE as u8, 0x20];
        p.extend_from_slice(&(len + 4).to_le_bytes());
        p.extend_from_slice(&len.to_le_bytes());
        p.extend_from_slice(&cid.to_le_bytes());
        p.extend_from_slice(pdu);
        self.packets.push_back(p);
    }
    fn connect(&mut self, guest_central: bool) {
        self.connected = true;
        self.advertising = false;
        let mut e = vec![1, 0, HANDLE as u8, 0, if guest_central { 0 } else { 1 }, 0];
        e.extend_from_slice(&PEER);
        e.extend_from_slice(&[24, 0, 0, 0, 0xf4, 1, 0]);
        self.event(0x3e, &e);
        self.emit(LinkEvent::Connected(guest_central));
    }
    fn advertisement(&mut self) { self.emit(LinkEvent::Advertising(self.adv.clone(), self.scan_response.clone())); }
    pub fn send(&mut self, packet: &[u8]) {
        match packet.first() {
            Some(1) if packet.len() >= 4 => {
                let op = word(&packet[1..]);
                if packet.len() != 4 + packet[3] as usize {
                    self.complete(op, &[0x12]);
                    return;
                }
                self.hci(op, &packet[4..]);
            }
            Some(2) if packet.len() >= 5 => self.acl(packet),
            _ => self.emit(LinkEvent::Invalid(LinkError::H4)),
        }
    }
    fn hci(&mut self, op: u16, p: &[u8]) {
        let expected = match op {
            RESET
            | READ_LOCAL_NAME
            | READ_LOCAL_VERSION
            | READ_LOCAL_COMMANDS
            | READ_LOCAL_FEATURES
            | READ_BUFFER_SIZE
            | READ_BD_ADDR
            | LE_READ_BUFFER_SIZE
            | LE_READ_LOCAL_FEATURES
            | LE_READ_ADVERTISING_TX_POWER
            | LE_CREATE_CONNECTION_CANCEL
            | LE_READ_FILTER_ACCEPT_LIST_SIZE
            | LE_CLEAR_FILTER_ACCEPT_LIST
            | LE_RAND
            | LE_READ_SUPPORTED_STATES
            | LE_CLEAR_RESOLVING_LIST
            | LE_READ_RESOLVING_LIST_SIZE
            | ESP_CLEAR_LEGACY_ADVERTISING => Some(0),
            READ_LOCAL_EXTENDED_FEATURES
            | WRITE_SCAN_ENABLE
            | WRITE_SYNCHRONOUS_FLOW_CONTROL_ENABLE
            | WRITE_INQUIRY_MODE
            | WRITE_SIMPLE_PAIRING_MODE
            | WRITE_ERRONEOUS_DATA_REPORTING
            | WRITE_SECURE_CONNECTIONS_HOST_SUPPORT
            | LE_SET_ADVERTISING_ENABLE
            | SET_CONTROLLER_TO_HOST_FLOW_CONTROL
            | ESP_ENABLE_CSA2
            | LE_SET_ADDRESS_RESOLUTION_ENABLE
            | VENDOR_FD82 => Some(1),
            WRITE_DEFAULT_LINK_POLICY
            | WRITE_CONNECTION_ACCEPT_TIMEOUT
            | WRITE_PAGE_TIMEOUT
            | WRITE_LE_HOST_SUPPORT
            | LE_SET_SCAN_ENABLE
            | LE_READ_CHANNEL_MAP
            | LE_READ_REMOTE_FEATURES
            | READ_RSSI
            | READ_REMOTE_VERSION
            | LE_SET_RPA_TIMEOUT
            | ESP_UPDATE_ADV_REPORT_FLOW_CONTROL => Some(2),
            DISCONNECT | WRITE_CLASS_OF_DEVICE | VENDOR_FC82 => Some(3),
            ESP_SET_LE_EVENT_MASK => Some(4),
            SET_EVENT_MASK | SET_EVENT_MASK_PAGE_2 | LE_SET_EVENT_MASK | LE_SET_PRIVACY_MODE => Some(8),
            LE_SET_RANDOM_ADDRESS => Some(6),
            LE_SET_ADVERTISING_PARAMETERS => Some(15),
            LE_SET_ADVERTISING_DATA | LE_SET_SCAN_RESPONSE_DATA | LE_ENCRYPT => Some(32),
            LE_SET_SCAN_PARAMETERS | HOST_BUFFER_SIZE | LE_ADD_FILTER_ACCEPT_LIST | LE_REMOVE_FILTER_ACCEPT_LIST => { Some(7) }
            LE_REMOVE_DEVICE_FROM_RESOLVING_LIST | LE_READ_PEER_RESOLVABLE_ADDRESS | LE_READ_LOCAL_RESOLVABLE_ADDRESS => Some(7),
            LE_CREATE_CONNECTION => Some(25),
            LE_ADD_DEVICE_TO_RESOLVING_LIST => Some(39),
            LE_CONNECTION_UPDATE => Some(14),
            WRITE_LOCAL_NAME => Some(248),
            ESP_SET_ADV_REPORT_FLOW_CONTROL | LE_SET_HOST_CHANNEL_CLASSIFICATION => Some(5),
            HOST_COMPLETED_PACKETS => p.first().map(|n| 1 + 4 * *n as usize),
            _ => None,
        };
        if expected.is_some_and(|n| n != p.len()) || (op == HOST_COMPLETED_PACKETS && p.is_empty()) {
            self.complete(op, &[0x12]);
            return;
        }
        match op {
            RESET => {
                self.advertising = false;
                self.connected = false;
                self.emit(LinkEvent::Reset);
                self.fragments.clear();
                self.adv.clear();
                self.scan_response.clear();
                self.active_scan = false;
                self.packets.clear();
                self.resolving.clear();
                self.complete(op, &[0]);
            }
            WRITE_DEFAULT_LINK_POLICY => {
                self.default_link_policy = word(p);
                self.complete(op, &[0]);
            }
            WRITE_LOCAL_NAME => {
                self.local_name.copy_from_slice(p);
                self.complete(op, &[0]);
            }
            READ_LOCAL_NAME => {
                let mut response = vec![0];
                response.extend_from_slice(&self.local_name);
                self.complete(op, &response);
            }
            READ_LOCAL_VERSION => self.complete(op, &[0, 8, 0, 0, 8, 0xff, 0xff, 0, 0]),
            READ_LOCAL_COMMANDS => {
                let mut commands = [0u8; 65];
                // Bluetooth Core Vol 4, Part E, 6.27. One status byte precedes the bitfield.
                for (octet, bits) in [
                    (0, 0x20),
                    (2, 0x80),
                    (5, 0xd0),
                    (7, 0xab),
                    (10, 0xf0),
                    (14, 0xf8),
                    (15, 0x22),
                    (18, 8),
                    (22, 4),
                    (25, 0xf7),
                    (26, 0xff),
                    (27, 0xff),
                    (28, 8),
                    // LE Add / Remove Device To / From Resolving List, Clear Resolving List, Read
                    // Resolving List Size, Read Peer Resolvable Address; Read Local Resolvable
                    // Address, Set Address Resolution Enable, Set Resolvable Private Address
                    // Timeout; Set Privacy Mode.
                    (34, 0xf8),
                    (35, 0x07),
                    (39, 0x04),
                ] { commands[octet + 1] = bits; }
                self.complete(op, &commands);
            }
            READ_LOCAL_FEATURES => self.complete(op, &[0, 0, 0, 0, 0, 0x40, 0, 0, 0x80]),
            READ_LOCAL_EXTENDED_FEATURES => {
                if p[0] > 1 {
                    self.complete(op, &[0x12]);
                    return;
                }
                let mut out = vec![0, p[0], 1];
                out.extend_from_slice(if p[0] == 0 {
                    &[0, 0, 0, 0, 0x40, 0, 0, 0x80]
                } else {
                    &[2, 0, 0, 0, 0, 0, 0, 0]
                });
                self.complete(op, &out);
            }
            READ_BUFFER_SIZE => self.complete(op, &[0, 0xfb, 0, 0, 8, 0, 0, 0]),
            READ_BD_ADDR => {
                let mut v = vec![0];
                v.extend_from_slice(&ADDRESS);
                self.complete(op, &v);
            }
            LE_READ_BUFFER_SIZE => self.complete(op, &[0, 0xfb, 0, 8]),
            // LE feature bit 6, LL Privacy (Bluetooth Core 5.4 Vol 6 Part B 4.6): a host stack
            // sizes its resolving list only when the controller reports it, and IDF's Bluedroid
            // adds its default entry either way (#296).
            LE_READ_LOCAL_FEATURES => self.complete(op, &[0, 0x40, 0, 0, 0, 0, 0, 0, 0]),
            LE_READ_ADVERTISING_TX_POWER => self.complete(op, &[0, 0]),
            LE_READ_FILTER_ACCEPT_LIST_SIZE => self.complete(op, &[0, 8]),
            LE_READ_SUPPORTED_STATES => self.complete(op, &[0, 0xff, 0x07, 0, 0, 0, 0, 0, 0]),
            LE_SET_ADVERTISING_DATA | LE_SET_SCAN_RESPONSE_DATA => {
                if p[0] > 31 {
                    self.complete(op, &[0x12]);
                    return;
                }
                let data = p[1..1 + p[0] as usize].to_vec();
                if op == LE_SET_ADVERTISING_DATA {
                    self.adv = data;
                } else { self.scan_response = data; }
                self.complete(op, &[0]);
                if self.advertising { self.advertisement(); }
            }
            LE_SET_ADVERTISING_ENABLE => {
                if p[0] > 1 {
                    self.complete(op, &[0x12]);
                    return;
                }
                self.advertising = p[0] == 1;
                self.complete(op, &[0]);
                if self.advertising {
                    self.advertisement();
                } else { self.emit(LinkEvent::AdvertisingStopped); }
            }
            LE_SET_SCAN_PARAMETERS => {
                if p[0] > 1 {
                    self.complete(op, &[0x12]);
                    return;
                }
                self.active_scan = p[0] == 1;
                self.complete(op, &[0]);
            }
            LE_SET_SCAN_ENABLE => {
                if p[0] > 1 || p[1] > 1 {
                    self.complete(op, &[0x12]);
                    return;
                }
                self.complete(op, &[0]);
                if p[0] == 1 { self.emit(LinkEvent::Scan(self.active_scan)); } else { self.emit(LinkEvent::ScanStopped); }
            }
            LE_CREATE_CONNECTION => {
                if p[4] != 0 || p[5] != 0 || p[6..12] != PEER {
                    self.status(op, 0x12);
                    return;
                }
                self.status(op, if self.connected { 0x0c } else { 0 });
                if !self.connected { self.connect(true); }
            }
            DISCONNECT => {
                if !self.connected || word(p) != HANDLE {
                    self.status(op, 2);
                    return;
                }
                self.status(op, 0);
                self.event(5, &[0, HANDLE as u8, 0, p[2]]);
                self.connected = false;
                self.fragments.clear();
                self.emit(LinkEvent::Disconnected);
            }
            LE_READ_CHANNEL_MAP => self.complete(op, &[0, p[0], p[1], 0xff, 0xff, 0xff, 0xff, 0x1f]),
            LE_READ_REMOTE_FEATURES => {
                self.status(op, 0);
                self.event(0x3e, &[4, 0, HANDLE as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            }
            READ_REMOTE_VERSION => {
                self.status(op, 0);
                self.event(0x0c, &[0, HANDLE as u8, 0, 8, 0xff, 0xff, 0, 0]);
            }
            LE_CONNECTION_UPDATE => {
                self.status(op, 0);
                let mut e = vec![3, 0, HANDLE as u8, 0];
                e.extend_from_slice(&p[2..4]);
                e.extend_from_slice(&p[6..10]);
                self.event(0x3e, &e);
            }
            LE_ENCRYPT => {
                let mut key: [u8; 16] = p[..16].try_into().unwrap();
                key.reverse();
                let mut data: [u8; 16] = p[16..].try_into().unwrap();
                data.reverse();
                let mut encrypted = esp_periph::crypto::aes128_encrypt_block(&key, &data);
                encrypted.reverse();
                let mut response = vec![0];
                response.extend_from_slice(&encrypted);
                self.complete(op, &response);
            }
            LE_RAND => {
                // repeatable bytes for unpaired fixtures, never a cryptographic entropy source.
                self.random ^= self.random << 13;
                self.random ^= self.random >> 7;
                self.random ^= self.random << 17;
                let mut response = vec![0];
                response.extend_from_slice(&self.random.to_le_bytes());
                self.complete(op, &response);
            }
            // The resolving list (Bluetooth Core 5.4 Vol 4 Part E 7.8.38-7.8.45, 7.8.77), with the
            // status codes an ESP32-S3's controller answers. It keeps identity addresses only: no
            // address is resolved against the IRKs and no resolvable private address is made.
            LE_ADD_DEVICE_TO_RESOLVING_LIST => {
                let entry = (p[0], p[1..7].try_into().unwrap());
                let status = if p[0] > 1 || self.resolving.contains(&entry) {
                    0x12
                } else if self.resolving.len() == RESOLVING_LIST_SIZE {
                    0x07
                } else {
                    self.resolving.push(entry);
                    0
                };
                self.complete(op, &[status]);
            }
            LE_REMOVE_DEVICE_FROM_RESOLVING_LIST => {
                let entry = (p[0], p[1..7].try_into().unwrap());
                let found = self.resolving.iter().position(|e| *e == entry);
                if let Some(i) = found { self.resolving.remove(i); }
                self.complete(op, &[if found.is_some() { 0 } else { 2 }]);
            }
            LE_CLEAR_RESOLVING_LIST => {
                self.resolving.clear();
                self.complete(op, &[0]);
            }
            LE_READ_RESOLVING_LIST_SIZE => self.complete(op, &[0, RESOLVING_LIST_SIZE as u8]),
            // No resolvable private address exists, so Unknown Connection Identifier, as the S3
            // answers for an entry it has made none for.
            LE_READ_PEER_RESOLVABLE_ADDRESS | LE_READ_LOCAL_RESOLVABLE_ADDRESS => self.complete(op, &[2, 0, 0, 0, 0, 0, 0]),
            LE_SET_ADDRESS_RESOLUTION_ENABLE => self.complete(op, &[if p[0] > 1 { 0x12 } else { 0 }]),
            // The S3 refuses 0 s only, 3601 s too is accepted.
            LE_SET_RPA_TIMEOUT => self.complete(op, &[if word(p) == 0 { 0x12 } else { 0 }]),
            LE_SET_PRIVACY_MODE => {
                let entry = (p[0], p[1..7].try_into().unwrap());
                let status = if p[7] > 1 { 0x12 } else if self.resolving.contains(&entry) { 0 } else { 2 };
                self.complete(op, &[status]);
            }
            READ_RSSI => self.complete(op, &[0, p[0], p[1], (-35i8) as u8]),
            HOST_COMPLETED_PACKETS => {} // Host Number Of Completed Packets has no completion event.
            SET_EVENT_MASK
            | WRITE_CONNECTION_ACCEPT_TIMEOUT
            | WRITE_PAGE_TIMEOUT
            | WRITE_SCAN_ENABLE
            | WRITE_CLASS_OF_DEVICE
            | WRITE_SYNCHRONOUS_FLOW_CONTROL_ENABLE
            | SET_CONTROLLER_TO_HOST_FLOW_CONTROL
            | HOST_BUFFER_SIZE
            | WRITE_INQUIRY_MODE
            | WRITE_SIMPLE_PAIRING_MODE
            | WRITE_ERRONEOUS_DATA_REPORTING
            | SET_EVENT_MASK_PAGE_2
            | WRITE_LE_HOST_SUPPORT
            | WRITE_SECURE_CONNECTIONS_HOST_SUPPORT
            | LE_SET_EVENT_MASK
            | LE_SET_RANDOM_ADDRESS
            | LE_SET_ADVERTISING_PARAMETERS
            | LE_CREATE_CONNECTION_CANCEL
            | LE_CLEAR_FILTER_ACCEPT_LIST
            | LE_ADD_FILTER_ACCEPT_LIST
            | LE_REMOVE_FILTER_ACCEPT_LIST
            | LE_SET_HOST_CHANNEL_CLASSIFICATION
            | VENDOR_FC82
            | ESP_SET_ADV_REPORT_FLOW_CONTROL
            | ESP_UPDATE_ADV_REPORT_FLOW_CONTROL
            | ESP_CLEAR_LEGACY_ADVERTISING
            | ESP_ENABLE_CSA2
            | ESP_SET_LE_EVENT_MASK
            | VENDOR_FD82 => self.complete(op, &[0]),
            _ => {
                self.emit(LinkEvent::Unsupported(op));
                self.complete(op, &[1]);
            }
        }
    }
    fn acl(&mut self, packet: &[u8]) {
        let header = word(&packet[1..]);
        if packet.len() != 5 + word(&packet[3..]) as usize || header & 0xfff != HANDLE || !self.connected {
            self.emit(LinkEvent::Invalid(LinkError::AclLength));
            return;
        }
        let boundary = (header >> 12) & 3;
        if boundary == 0 || boundary == 2 {
            self.fragments.clear();
        } else if boundary != 1 || self.fragments.is_empty() {
            self.emit(LinkEvent::Invalid(LinkError::AclContinuation));
            return;
        }
        self.fragments.extend_from_slice(&packet[5..]);
        self.event(0x13, &[1, HANDLE as u8, 0, 1, 0]);
        if self.fragments.len() < 4 { return; }
        let len = word(&self.fragments) as usize;
        if len > 517 || self.fragments.len() > len + 4 {
            self.fragments.clear();
            self.emit(LinkEvent::Invalid(LinkError::L2capLength));
            return;
        }
        if self.fragments.len() < len + 4 { return; }
        let data = std::mem::take(&mut self.fragments);
        self.emit(LinkEvent::Data(word(&data[2..]), data[4..].to_vec()));
    }
}
impl HciController for Controller {
    fn send_h4(&mut self, packet: &[u8]) { self.send(packet); }
    fn poll_h4(&mut self) -> Option<Vec<u8>> { self.pop_packet() }
    fn reset(&mut self) {
        *self.link.borrow_mut() = LinkQueues::default();
        *self = Self::new(self.link.clone());
    }
    fn advance_to(&mut self, cycles: u64) {
        self.cycles = cycles;
        loop {
            let action = self.link.borrow_mut().to_controller.pop_front();
            let Some(action) = action else { break };
            match action {
                LinkAction::Connect if self.advertising && !self.connected => self.connect(false),
                LinkAction::Advertisement(data, active) => {
                    let mut e = vec![2, 1, 0, 0];
                    e.extend_from_slice(&PEER);
                    e.push(data.len() as u8);
                    e.extend(data);
                    e.push((-35i8) as u8);
                    self.event(0x3e, &e);
                    if active {
                        e[2] = 4;
                        self.event(0x3e, &e);
                    }
                }
                LinkAction::Data(cid, pdu) if self.connected => self.l2cap(cid, &pdu),
                LinkAction::Report { addr, addr_type, data, scan_response, rssi, active } => {
                    // LE Advertising Report (Bluetooth Core 5.4, Vol 4 Part E 7.7.65.2): ADV_SCAN_IND
                    // (0x02, scannable, not connectable: Vol 6 Part B 2.3.1.4), and for an active scan
                    // SCAN_RSP (0x04), an empty one too, as a scannable advertiser answers every scan
                    // request.
                    for (kind, payload) in [(2u8, &data), (4u8, &scan_response)] {
                        if kind == 4 && !active { continue; }
                        let mut e = vec![2, 1, kind, addr_type];
                        e.extend_from_slice(&addr);
                        e.push(payload.len() as u8);
                        e.extend_from_slice(payload);
                        e.push(rssi as u8);
                        self.event(0x3e, &e);
                    }
                }
                _ => {}
            }
        }
    }
    fn next_deadline(&self) -> Option<u64> { (!self.link.borrow().to_controller.is_empty()).then_some(self.cycles) }
}
