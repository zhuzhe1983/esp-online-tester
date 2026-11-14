//! User-mode NAT: guest TCP and UDP flows are relayed through a host transport (`NatTransport`):
//! natively `StdTransport`, ordinary host sockets, which also carries `--hostfwd` inbound TCP to
//! the station.
//!
//! TCP uses bounded buffers in both directions. SYN, data and FIN share one retransmission queue,
//! so a dropped virtual-air frame cannot discard acknowledged guest bytes or strand a handshake.
//! This is a small relay, without window scaling, SACK or congestion control.

use std::collections::VecDeque;
use std::io::{self, ErrorKind, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use crate::net::packet::{dns_question, dns_reply, ethernet, ip_packet, transport_checksum, udp_packet, GATEWAY_MAC};

// The host side is a trait; `Nat::new` keeps the std::net sockets. The browser build has neither
// sockets nor threads, so it picks its own transport.
#[cfg(not(target_arch = "wasm32"))]
mod std_transport;
mod transport;
#[cfg(not(target_arch = "wasm32"))]
pub use std_transport::StdTransport;
pub use transport::{Connect, FlowId, NatTransport, Resolve};

const MSS: usize = 1400;
const WINDOW: u16 = 5840;
const RETRANSMIT_US: u64 = 300_000;
const IDLE_CLOSE_US: u64 = 120_000_000;
const TIME_WAIT_US: u64 = 30_000_000;
const MAX_FLOWS: usize = 64;
const UDP_BATCH: usize = 16;
/// Accepted inbound connections waiting for the station MAC; a new one drops the oldest.
const MAX_PENDING: usize = 16;
const GATEWAY_IP: [u8; 4] = [10, 0, 2, 2];
const STATION_IP: [u8; 4] = [10, 0, 2, 15];
const INBOUND_PORT_BASE: u16 = 50_000;
/// Name lookups the transport resolves itself, waiting at once; a new one drops the oldest.
const MAX_DNS_PENDING: usize = 32;
/// A lookup the transport has not finished by then is forgotten (the guest has long retried).
const DNS_PENDING_US: u64 = 30_000_000;
const FIN: u8 = 0x01;
const SYN: u8 = 0x02;
const RST: u8 = 0x04;
const ACK: u8 = 0x10;
const PSH: u8 = 0x08;

fn ip(a: &[u8; 4]) -> Ipv4Addr { Ipv4Addr::from(*a) }

/// A guest TCP flow and its transport flow, while there is one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FlowState { Connecting(FlowId), Connected(FlowId), TimeWait, Closed }
impl FlowState {
    fn id(self) -> Option<FlowId> {
        match self { FlowState::Connecting(id) | FlowState::Connected(id) => Some(id), _ => None }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum GuestWrite { Open, Draining, Closed }

struct Sent {
    seq: u32,
    flags: u8,
    data: Vec<u8>,
    at_us: u64,
}
impl Sent {
    fn len(&self) -> usize { self.data.len() + usize::from(self.flags & (SYN | FIN) != 0) }
}

/// One `--hostfwd` listener: a host TCP port that becomes a SYN to the station.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFwdSpec {
    pub bind: SocketAddr,
    pub guest_ip: [u8; 4],
    pub guest_port: u16,
}

impl HostFwdSpec {
    /// `tcp:[hostaddr]:hostport-[guestaddr]:guestport`. Empty host binds loopback; empty guest is 10.0.2.15.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let rest = match spec.strip_prefix("tcp:") {
            Some(rest) => rest,
            None if spec.starts_with("udp:") => return Err("UDP host forwarding is not supported".into()),
            None => return Err(format!("invalid --hostfwd {spec:?}: expected tcp:HOST:HOSTPORT-GUEST:GUESTPORT")),
        };
        let (left, right) = rest.split_once('-')
            .ok_or_else(|| format!("invalid --hostfwd {spec:?}: expected tcp:HOST:HOSTPORT-GUEST:GUESTPORT"))?;
        let (host, host_port) = left.rsplit_once(':')
            .ok_or_else(|| format!("invalid --hostfwd {spec:?}: missing host port"))?;
        let (guest, guest_port) = right.rsplit_once(':')
            .ok_or_else(|| format!("invalid --hostfwd {spec:?}: missing guest port"))?;
        let host_ip: IpAddr = if host.is_empty() {
            Ipv4Addr::LOCALHOST.into()
        } else {
            host.parse().map_err(|_| format!("invalid --hostfwd host address {host:?}"))?
        };
        let host_port: u16 = host_port.parse().map_err(|_| format!("invalid --hostfwd host port {host_port:?}"))?;
        let guest_ip = if guest.is_empty() {
            STATION_IP
        } else {
            guest.parse::<Ipv4Addr>().map_err(|_| format!("invalid --hostfwd guest address {guest:?}"))?.octets()
        };
        let guest_port: u16 = guest_port.parse().map_err(|_| format!("invalid --hostfwd guest port {guest_port:?}"))?;
        if guest_port == 0 { return Err("--hostfwd guest port must be 1..=65535".into()); }
        Ok(Self { bind: SocketAddr::new(host_ip, host_port), guest_ip, guest_port })
    }
}

/// An inbound connection the transport accepted, waiting for the station MAC.
struct Pending {
    id: FlowId,
    guest_ip: [u8; 4],
    guest_port: u16,
    host_port: u16,
}

struct Tcp {
    guest_mac: [u8; 6], guest_ip: [u8; 4], guest_port: u16, dst_ip: [u8; 4], dst_port: u16,
    inbound: bool,
    host_port: u16,
    state: FlowState,
    guest_write: GuestWrite,
    host_closed: bool,
    our_seq: u32,
    guest_seq: u32,
    guest_window: u16,
    to_host: VecDeque<u8>,
    unacked: VecDeque<Sent>,
    last_activity_us: u64,
}

impl Tcp {
    fn handshake_pending(&self) -> bool { self.unacked.front().is_some_and(|s| s.flags & SYN != 0) }
    fn in_flight(&self) -> usize { self.unacked.iter().map(Sent::len).sum() }

    fn acknowledge(&mut self, ack: u32) {
        let Some(first) = self.unacked.front() else { return };
        let mut count = ack.wrapping_sub(first.seq) as usize;
        if count > self.in_flight() { return; } // old or beyond anything sent
        while count > 0 {
            let sent = self.unacked.front_mut().unwrap();
            let n = sent.len();
            if count < n {
                sent.data.drain(..count);
                sent.seq = ack;
                break;
            }
            count -= n;
            self.unacked.pop_front();
        }
    }

    fn accept(&mut self, seq: u32, data: &[u8], fin: bool) {
        if self.guest_write != GuestWrite::Open || seq != self.guest_seq { return; }
        if data.len() > WINDOW as usize - self.to_host.len() { return; }
        self.to_host.extend(data);
        self.guest_seq = self.guest_seq.wrapping_add(data.len() as u32);
        if fin {
            self.guest_seq = self.guest_seq.wrapping_add(1);
            self.guest_write = GuestWrite::Draining;
        }
    }

    fn segment(&self, flags: u8, data: &[u8], seq: u32) -> Vec<u8> {
        let mut seg = Vec::with_capacity(20 + data.len());
        seg.extend_from_slice(&self.dst_port.to_be_bytes());
        seg.extend_from_slice(&self.guest_port.to_be_bytes());
        seg.extend_from_slice(&seq.to_be_bytes());
        seg.extend_from_slice(&self.guest_seq.to_be_bytes());
        seg.extend_from_slice(&[0x50, flags]);
        seg.extend_from_slice(&(WINDOW - self.to_host.len() as u16).to_be_bytes());
        seg.extend_from_slice(&[0; 4]);
        seg.extend_from_slice(data);
        let check = transport_checksum(&self.dst_ip, &self.guest_ip, 6, &seg);
        seg[16..18].copy_from_slice(&check.to_be_bytes());
        ethernet(&self.guest_mac, &GATEWAY_MAC, 0x0800, &ip_packet(6, &self.dst_ip, &self.guest_ip, &seg))
    }

    fn send(&mut self, flags: u8, data: Vec<u8>, now_us: u64) -> Vec<u8> {
        let frame = self.segment(flags, &data, self.our_seq);
        let sent = Sent { seq: self.our_seq, flags, data, at_us: now_us };
        self.our_seq = self.our_seq.wrapping_add(sent.len() as u32);
        self.unacked.push_back(sent);
        frame
    }

    fn retransmit(&mut self, now_us: u64) -> Option<Vec<u8>> {
        let sent = self.unacked.front_mut()?;
        if now_us.wrapping_sub(sent.at_us) < RETRANSMIT_US { return None; }
        sent.at_us = now_us;
        let sent = self.unacked.front().unwrap();
        Some(self.segment(sent.flags, &sent.data, sent.seq))
    }

    fn closed(&self) -> bool {
        self.state == FlowState::Closed
    }

    /// Forget the transport flow, if there is one, and move to `next` (TimeWait or Closed).
    fn release(&mut self, transport: &mut dyn NatTransport, next: FlowState, abort: bool) {
        if let Some(id) = self.state.id() { transport.tcp_close(id, abort); }
        self.state = next;
    }
}

/// Preserve every unwritten byte across short writes and WouldBlock. Returns bytes actually sent.
fn flush_pending(writer: &mut impl Write, pending: &mut VecDeque<u8>) -> io::Result<usize> {
    let mut written = 0;
    while !pending.is_empty() {
        match writer.write(pending.as_slices().0) {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(n) => { pending.drain(..n); written += n; }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => return Err(e),
        }
    }
    Ok(written)
}

/// A transport flow as `Write`, for `flush_pending`.
struct FlowWriter<'a> { transport: &'a mut dyn NatTransport, id: FlowId }
impl Write for FlowWriter<'_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> { self.transport.tcp_write(self.id, data) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

struct Udp {
    guest_mac: [u8; 6], guest_ip: [u8; 4], guest_port: u16, dst_ip: [u8; 4], dst_port: u16,
    reply_src: [u8; 4], // DNS's guest-visible address, before host resolver redirection
    id: FlowId,
    last_activity_us: u64,
}

/// Current TCP occupancy, independent of cumulative connection statistics.
///
/// Only `active` counts against the 64-flow table limit: a new SYN on a full table evicts a
/// TimeWait entry. A small `active` still does not mean a new connect will be admitted: the limit
/// of 8 host connects in progress is `StdTransport`'s, process-wide, shared by every `Nat` on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TcpFlowCounts {
    /// Connecting and connected flows, including half-close and unacknowledged FIN.
    pub active: usize,
    /// Completed flows retained to retry the final ACK.
    pub time_wait: usize,
}

/// A guest's DNS query the transport is resolving.
struct DnsQuery {
    qid: u32,
    guest_mac: [u8; 6], guest_ip: [u8; 4], guest_port: u16, dns_ip: [u8; 4],
    name: String,
    qtype: u16,
    /// the query's header and question, which the answer repeats
    query: Vec<u8>,
    at_us: u64,
}

fn qtype_name(qtype: u16) -> &'static str { if qtype == 1 { "A" } else { "AAAA" } }

pub struct Nat {
    tcp: Vec<Tcp>,
    udp: Vec<Udp>,
    dns: VecDeque<DnsQuery>,
    transport: Box<dyn NatTransport>,
    pending: Vec<Pending>,
    sta_mac: [u8; 6],
    next_eph: u16,
    isn: u32,
    pub resolver: [u8; 4],
    /// Address used as the source of inbound connections on this virtual interface.
    pub gateway_ip: [u8; 4],
    pub log: bool,
    pub tcp_opened: u64, pub tcp_refused: u64, pub tcp_accepted: u64,
    /// `--hostfwd` listeners on the transport (`StdTransport::listen`), for the report line
    pub hostfwd: u64,
    pub udp_flows: u64,
    pub udp_evicted: u64, pub udp_send_errors: u64,
    pub bytes_to_host: u64, pub bytes_to_guest: u64,
    /// answers to DNS queries the transport resolved itself
    pub dns_answers: u64,
}

impl Nat {
    /// The NAT over ordinary host sockets.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(log: bool) -> Self { Self::with_transport(log, Box::new(StdTransport::new())) }

    /// The NAT over another host transport.
    pub fn with_transport(log: bool, transport: Box<dyn NatTransport>) -> Self {
        Self { tcp: Vec::new(), udp: Vec::new(), dns: VecDeque::new(), resolver: transport.resolver(), transport,
            pending: Vec::new(), sta_mac: [0; 6], next_eph: INBOUND_PORT_BASE, isn: 0x1000, gateway_ip: GATEWAY_IP, log,
            tcp_opened: 0, tcp_refused: 0, tcp_accepted: 0, hostfwd: 0,
            udp_flows: 0, udp_evicted: 0, udp_send_errors: 0, bytes_to_host: 0, bytes_to_guest: 0,
            dns_answers: 0 }
    }

    /// The station MAC learned from the virtual network; inbound SYNs wait until it is known.
    pub fn set_station(&mut self, mac: [u8; 6]) {
        if mac != [0; 6] { self.sta_mac = mac; }
    }

    /// Bind one host TCP port through the transport, retaining the helper's dynamic mapping API.
    pub fn listen(&mut self, spec: &HostFwdSpec) -> io::Result<SocketAddr> {
        let bound = self.transport.listen(spec)?;
        self.hostfwd += 1;
        if self.log { eprintln!("[nat] hostfwd tcp {} -> {}:{}", bound, ip(&spec.guest_ip), spec.guest_port); }
        Ok(bound)
    }

    /// Release one mapping and its accepted connections. Other mappings and outbound flows remain.
    pub fn unlisten(&mut self, port: u16) -> bool {
        let removed = self.transport.unlisten(port);
        if !removed { return false; }
        let transport = &mut *self.transport;
        self.pending.retain(|p| {
            if p.host_port != port { return true; }
            transport.tcp_close(p.id, true); false
        });
        self.tcp.retain_mut(|c| {
            if !c.inbound || c.host_port != port { return true; }
            c.release(transport, FlowState::Closed, true); false
        });
        removed
    }

    /// Snapshot TCP transport state without polling sockets, retransmitting or expiring flows.
    /// Closed entries are excluded even before the next poll removes them. Expiration and
    /// transitions (including pending connector results) take effect only through normal polling.
    pub fn tcp_flow_counts(&self) -> TcpFlowCounts {
        let mut counts = TcpFlowCounts::default();
        for flow in &self.tcp {
            match flow.state {
                FlowState::Connecting(_) | FlowState::Connected(_) => counts.active += 1,
                FlowState::TimeWait => counts.time_wait += 1,
                FlowState::Closed => {}
            }
        }
        counts
    }

    /// Drop every flow at once, resetting the transport's (the guest rebooted;
    /// its sockets are gone). Nothing is sent to the guest.
    pub fn abort_all(&mut self) {
        for c in &mut self.tcp { c.release(&mut *self.transport, FlowState::Closed, true); }
        self.tcp.clear();
        for p in self.pending.drain(..) { self.transport.tcp_close(p.id, true); }
        for f in self.udp.drain(..) { self.transport.udp_close(f.id); }
        self.dns.clear();
    }

    /// A guest DNS query to `dns_ip`. When the transport resolves names itself
    /// (A and AAAA only) the answer comes from `dns_ip` once it has; false means relay the datagram
    /// to `resolver` instead.
    pub fn dns_query(&mut self, guest_mac: &[u8; 6], guest_ip: &[u8; 4], guest_port: u16, dns_ip: &[u8; 4], msg: &[u8], now_us: u64) -> bool {
        let Some(q) = dns_question(msg) else { return false };
        if q.qtype != 1 && q.qtype != 28 { return false; }
        let Some(qid) = self.transport.resolve(&q.name, q.qtype) else { return false };
        if self.dns.len() >= MAX_DNS_PENDING { self.dns.pop_front(); }
        if self.log { eprintln!("[nat] DNS {} {} -> resolving", q.name, qtype_name(q.qtype)); }
        self.dns.push_back(DnsQuery { qid, guest_mac: *guest_mac, guest_ip: *guest_ip, guest_port, dns_ip: *dns_ip,
            query: msg[..q.end].to_vec(), name: q.name, qtype: q.qtype, at_us: now_us });
        true
    }

    /// Forward a UDP datagram through a transport flow, which accepts replies only from its peer.
    #[allow(clippy::too_many_arguments, reason = "packet fields stay explicit at the protocol boundary")]
    pub fn udp_out(&mut self, gmac: &[u8; 6], gip: &[u8; 4], sport: u16, dip: &[u8; 4], reply_src: &[u8; 4],
                   dport: u16, payload: &[u8], now_us: u64) {
        let idx = self.udp.iter().position(|f| f.guest_ip == *gip && f.guest_port == sport && f.dst_ip == *dip && f.dst_port == dport);
        let idx = match idx {
            Some(i) => i,
            None => {
                let Ok(id) = self.transport.udp_open(SocketAddrV4::new(ip(dip), dport)) else { return };
                if self.udp.len() >= MAX_FLOWS {
                    let oldest = self.udp.iter().enumerate().max_by_key(|(_, f)| now_us.wrapping_sub(f.last_activity_us)).unwrap().0;
                    let evicted = self.udp.remove(oldest);
                    self.transport.udp_close(evicted.id);
                    self.udp_evicted += 1;
                    if self.log { eprintln!("[nat] UDP evicted least recently active flow (table full)"); }
                }
                self.udp.push(Udp { guest_mac: *gmac, guest_ip: *gip, guest_port: sport, dst_ip: *dip, dst_port: dport,
                    reply_src: *reply_src, id, last_activity_us: now_us });
                self.udp_flows += 1;
                if self.log { eprintln!("[nat] UDP {}:{} -> {}:{} ({} bytes)", ip(gip), sport, ip(dip), dport, payload.len()); }
                self.udp.len() - 1
            }
        };
        let flow = &mut self.udp[idx];
        flow.last_activity_us = now_us;
        match self.transport.udp_send(flow.id, payload) {
            Ok(n) => self.bytes_to_host += n as u64,
            Err(e) => {
                self.udp_send_errors += 1;
                if self.log { eprintln!("[nat] UDP {}:{} send failed: {}", ip(dip), dport, e); }
            }
        }
    }

    pub fn tcp_in(&mut self, gmac: &[u8; 6], gip: &[u8; 4], dip: &[u8; 4], seg: &[u8], now_us: u64) -> Vec<Vec<u8>> {
        if seg.len() < 20 { return Vec::new(); }
        let off = ((seg[12] >> 4) as usize) * 4;
        if off < 20 || off > seg.len() { return Vec::new(); }
        let sport = u16::from_be_bytes([seg[0], seg[1]]);
        let dport = u16::from_be_bytes([seg[2], seg[3]]);
        let seq = u32::from_be_bytes(seg[4..8].try_into().unwrap());
        let ack = u32::from_be_bytes(seg[8..12].try_into().unwrap());
        let flags = seg[13];
        let data = &seg[off..];
        let window = u16::from_be_bytes([seg[14], seg[15]]);
        let idx = self.tcp.iter().position(|c| c.guest_ip == *gip && c.guest_port == sport && c.dst_port == dport && c.dst_ip == *dip);
        if flags & (SYN | ACK | RST) == SYN && idx.is_none() {
            let evict = if self.tcp.len() >= MAX_FLOWS {
                // Finished flows remember final ACKs only while their bounded slots are spare.
                let Some(i) = self.tcp.iter().position(|c| matches!(c.state, FlowState::TimeWait | FlowState::Closed)) else { return Vec::new() };
                Some(i)
            } else { None };
            let Some(id) = self.transport.tcp_connect(SocketAddrV4::new(ip(dip), dport)) else { return Vec::new() };
            if let Some(i) = evict { self.tcp.remove(i); }
            self.isn = self.isn.wrapping_add(0x10000);
            self.tcp.push(Tcp { guest_mac: *gmac, guest_ip: *gip, guest_port: sport, dst_ip: *dip, dst_port: dport,
                inbound: false, host_port: 0, state: FlowState::Connecting(id), guest_write: GuestWrite::Open, host_closed: false,
                our_seq: self.isn, guest_seq: seq.wrapping_add(1), guest_window: window,
                to_host: VecDeque::new(), unacked: VecDeque::new(), last_activity_us: now_us });
            if self.log { eprintln!("[nat] TCP {}:{} -> {}:{} connecting", ip(gip), sport, ip(dip), dport); }
            return Vec::new();
        }
        let Some(i) = idx else { return Vec::new() };
        let inbound_accept = {
            let c = &mut self.tcp[i];
            c.last_activity_us = now_us;
            c.inbound && c.handshake_pending() && flags & (SYN | ACK | RST) == SYN | ACK && ack == c.our_seq
        };
        if inbound_accept {
            let (reply, gip, gport) = {
                let c = &mut self.tcp[i];
                c.acknowledge(ack);
                c.guest_seq = seq.wrapping_add(1);
                c.guest_window = window;
                (vec![c.segment(ACK, &[], c.our_seq)], c.guest_ip, c.guest_port)
            };
            self.tcp_opened += 1;
            self.tcp_accepted += 1;
            if self.log { eprintln!("[nat] TCP hostfwd -> {}:{} accepted", ip(&gip), gport); }
            return reply;
        }
        let c = &mut self.tcp[i];
        if flags & RST != 0 {
            // the guest refused a forwarded connection (nothing listening on the port yet)
            if c.inbound && c.handshake_pending() { self.tcp_refused += 1; }
            c.release(&mut *self.transport, FlowState::Closed, true);
            return Vec::new();
        }
        if c.state == FlowState::TimeWait {
            return if flags & FIN != 0 { vec![c.segment(ACK, &[], c.our_seq)] } else { Vec::new() };
        }
        if c.inbound && c.handshake_pending() {
            return if flags & SYN != 0 { vec![c.segment(SYN, &[], c.unacked[0].seq)] } else { Vec::new() };
        }
        if flags & SYN != 0 {
            if c.inbound { return vec![c.segment(ACK, &[], c.our_seq)]; }
            return if c.handshake_pending() { vec![c.segment(SYN | ACK, &[], c.unacked[0].seq)] } else { Vec::new() };
        }
        if !matches!(c.state, FlowState::Connected(_)) { return Vec::new(); }
        if flags & ACK != 0 { c.acknowledge(ack); c.guest_window = window; }
        if c.handshake_pending() { return Vec::new(); }
        if !data.is_empty() || flags & FIN != 0 {
            c.accept(seq, data, flags & FIN != 0);
            return vec![c.segment(ACK, &[], c.our_seq)];
        }
        Vec::new()
    }

    fn alloc_eph(&mut self) -> Option<u16> {
        for _ in 0..1000u16 {
            let port = self.next_eph;
            self.next_eph = if self.next_eph == u16::MAX { INBOUND_PORT_BASE } else { self.next_eph + 1 };
            if port >= INBOUND_PORT_BASE && !self.tcp.iter().any(|c| c.inbound && c.dst_port == port) { return Some(port); }
        }
        None
    }

    /// Take the connections the transport accepted; past `MAX_PENDING` the oldest is closed.
    fn accept_inbound(&mut self) {
        while let Some((id, guest_ip, guest_port)) = self.transport.tcp_accept() {
            let host_port = self.transport.tcp_listener_port(id).unwrap_or(0);
            self.pending.push(Pending { id, guest_ip, guest_port, host_port });
        }
        if self.pending.len() > MAX_PENDING {
            for p in self.pending.drain(0..self.pending.len() - MAX_PENDING) { self.transport.tcp_close(p.id, false); }
        }
    }

    fn start_pending(&mut self, now_us: u64, out: &mut Vec<Vec<u8>>) {
        if self.sta_mac == [0; 6] { return; }
        let pending = std::mem::take(&mut self.pending);
        for p in pending {
            if self.tcp.len() >= MAX_FLOWS {
                if let Some(i) = self.tcp.iter().position(|c| matches!(c.state, FlowState::TimeWait | FlowState::Closed)) {
                    self.tcp.remove(i);
                } else { self.pending.push(p); continue; }
            }
            let Some(eph) = self.alloc_eph() else { self.pending.push(p); continue; };
            self.isn = self.isn.wrapping_add(0x10000);
            let mut c = Tcp { guest_mac: self.sta_mac, guest_ip: p.guest_ip, guest_port: p.guest_port,
                dst_ip: self.gateway_ip, dst_port: eph, inbound: true, host_port: p.host_port, state: FlowState::Connected(p.id),
                guest_write: GuestWrite::Open, host_closed: false, our_seq: self.isn, guest_seq: 0,
                guest_window: WINDOW, to_host: VecDeque::new(), unacked: VecDeque::new(), last_activity_us: now_us };
            if self.log { eprintln!("[nat] TCP hostfwd SYN {}:{} -> {}:{}", ip(&self.gateway_ip), eph, ip(&c.guest_ip), c.guest_port); }
            out.push(c.send(SYN, Vec::new(), now_us));
            self.tcp.push(c);
        }
    }

    /// Pump a bounded amount of host traffic, then retransmit and expire flows.
    pub fn poll(&mut self, now_us: u64) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        self.accept_inbound();
        self.start_pending(now_us, &mut out);
        let transport = &mut *self.transport;
        for c in &mut self.tcp {
            if c.closed() { continue; }
            if c.host_closed && c.guest_write == GuestWrite::Closed && c.unacked.is_empty()
                && matches!(c.state, FlowState::Connected(_)) {
                // Release the host flow but keep enough state to re-ACK a lost final ACK.
                c.release(transport, FlowState::TimeWait, false);
                c.last_activity_us = now_us;
            }
            if let FlowState::Connecting(id) = c.state {
                match transport.tcp_state(id) {
                    Connect::Open => {
                        c.state = FlowState::Connected(id);
                        self.tcp_opened += 1;
                        if self.log { eprintln!("[nat] TCP {}:{} connected", ip(&c.dst_ip), c.dst_port); }
                        out.push(c.send(SYN | ACK, Vec::new(), now_us));
                    }
                    Connect::Failed(kind) => {
                        if self.log { eprintln!("[nat] TCP {}:{} failed: {}", ip(&c.dst_ip), c.dst_port, io::Error::from(kind)); }
                        self.tcp_refused += 1;
                        out.push(c.segment(RST | ACK, &[], c.our_seq));
                        c.release(transport, FlowState::Closed, true);
                    }
                    Connect::Pending => {}
                }
            }
            if let FlowState::Connected(id) = c.state {
                if !c.handshake_pending() {
                    let queued = c.to_host.len();
                    match flush_pending(&mut FlowWriter { transport, id }, &mut c.to_host) {
                        Ok(n) => self.bytes_to_host += n as u64,
                        Err(_) => { out.push(c.segment(RST | ACK, &[], c.our_seq)); c.release(transport, FlowState::Closed, true); continue; }
                    }
                    if c.guest_write == GuestWrite::Draining && c.to_host.is_empty() {
                        transport.tcp_shutdown(id);
                        c.guest_write = GuestWrite::Closed;
                    }
                    if c.to_host.len() < queued { out.push(c.segment(ACK, &[], c.our_seq)); } // reopen the receive window
                    // One byte at a zero window becomes a persist probe. The same retransmission
                    // queue retries it, recovering even when the guest's window-update ACK is lost.
                    let send_window = usize::from(c.guest_window.clamp(1, WINDOW));
                    while !c.host_closed && c.in_flight() < send_window {
                        let room = send_window - c.in_flight();
                        let mut buf = [0; MSS];
                        match transport.tcp_read(id, &mut buf[..room.min(MSS)]) {
                            Ok(0) => { c.host_closed = true; out.push(c.send(FIN | ACK, Vec::new(), now_us)); }
                            Ok(n) => {
                                if self.log { eprintln!("[nat] TCP {}:{} -> guest {} bytes (seq {})", ip(&c.dst_ip), c.dst_port, n, c.our_seq); }
                                self.bytes_to_guest += n as u64;
                                c.last_activity_us = now_us;
                                out.push(c.send(PSH | ACK, buf[..n].to_vec(), now_us));
                            }
                            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                            Err(_) => { out.push(c.segment(RST | ACK, &[], c.our_seq)); c.release(transport, FlowState::Closed, true); break; }
                        }
                    }
                }
            }
            if let Some(frame) = c.retransmit(now_us) { out.push(frame); }
        }
        for flow in &mut self.udp {
            let mut buf = [0; 2048];
            for _ in 0..UDP_BATCH {
                let Ok(n) = transport.udp_recv(flow.id, &mut buf) else { break };
                if self.log { eprintln!("[nat] UDP reply {} bytes -> guest port {}", n, flow.guest_port); }
                self.bytes_to_guest += n as u64;
                flow.last_activity_us = now_us;
                let udp = udp_packet(&flow.reply_src, &flow.guest_ip, flow.dst_port, flow.guest_port, &buf[..n]);
                out.push(ethernet(&flow.guest_mac, &GATEWAY_MAC, 0x0800, &ip_packet(17, &flow.reply_src, &flow.guest_ip, &udp)));
            }
        }
        while let Some((qid, result)) = transport.resolved() {
            let Some(i) = self.dns.iter().position(|q| q.qid == qid) else { continue };
            let q = self.dns.remove(i).unwrap();
            let (addrs, ttl, rcode) = match &result { Resolve::Addrs(addrs, ttl) => (addrs.as_slice(), *ttl, 0), Resolve::Failed(rcode) => (&[][..], 0, *rcode) };
            if self.log {
                let answer = if rcode != 0 { format!("rcode {rcode}") } else if q.qtype != 1 || addrs.is_empty() { "(none)".into() }
                    else { addrs.iter().map(|a| ip(a).to_string()).collect::<Vec<_>>().join(", ") };
                eprintln!("[nat] DNS {} {} -> {}", q.name, qtype_name(q.qtype), answer);
            }
            self.dns_answers += 1;
            let udp = udp_packet(&q.dns_ip, &q.guest_ip, 53, q.guest_port, &dns_reply(&q.query, q.qtype, addrs, ttl, rcode));
            out.push(ethernet(&q.guest_mac, &GATEWAY_MAC, 0x0800, &ip_packet(17, &q.dns_ip, &q.guest_ip, &udp)));
        }
        self.dns.retain(|q| now_us.wrapping_sub(q.at_us) < DNS_PENDING_US);
        self.tcp.retain(|c| {
            let timeout = if c.state == FlowState::TimeWait { TIME_WAIT_US } else { IDLE_CLOSE_US };
            let keep = !c.closed() && now_us.wrapping_sub(c.last_activity_us) < timeout;
            if !keep { if let Some(id) = c.state.id() { transport.tcp_close(id, false); } }
            keep
        });
        self.udp.retain(|f| {
            let keep = now_us.wrapping_sub(f.last_activity_us) < IDLE_CLOSE_US;
            if !keep { transport.udp_close(f.id); }
            keep
        });
        out
    }
}

#[cfg(test)]
mod tests;
